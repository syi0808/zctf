use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    FnArg, ItemFn, LitStr, Pat, Token,
    ext::IdentExt,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
};

struct ExportOption {
    name: syn::Ident,
    _equals: Token![=],
    value: LitStr,
}
impl Parse for ExportOption {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        Ok(Self {
            name: input.call(syn::Ident::parse_any)?,
            _equals: input.parse()?,
            value: input.parse()?,
        })
    }
}

#[proc_macro_attribute]
pub fn export(args: TokenStream, input: TokenStream) -> TokenStream {
    expand(args, parse_macro_input!(input as ItemFn))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

fn expand(args: TokenStream, function: ItemFn) -> syn::Result<proc_macro2::TokenStream> {
    let args = syn::parse::Parser::parse(
        Punctuated::<ExportOption, Token![,]>::parse_terminated,
        args,
    )?;
    let mut js_name = None;
    let mut return_policy = None;
    for argument in args {
        let name = argument.name.to_string();
        match name.as_str() {
            "name" => js_name = Some(argument.value.value()),
            "return" => return_policy = Some(argument.value.value()),
            _ => {
                return Err(syn::Error::new_spanned(
                    argument.name,
                    "unsupported export option",
                ));
            }
        }
    }
    if return_policy.as_deref().unwrap_or("buffer") != "buffer" {
        return Err(syn::Error::new_spanned(
            &function.sig,
            "only return = \"buffer\" is supported",
        ));
    }
    let original_name = &function.sig.ident;
    let js_name = js_name.unwrap_or_else(|| snake_to_camel(&original_name.to_string()));
    let wrapper = format_ident!("__zctf_export_{}", original_name);
    let mut wrapper_inputs = Vec::new();
    let mut call_args = Vec::new();
    for input in &function.sig.inputs {
        match input {
            FnArg::Typed(argument) => {
                let Pat::Ident(ident) = &*argument.pat else {
                    return Err(syn::Error::new_spanned(
                        &argument.pat,
                        "export arguments must be identifiers",
                    ));
                };
                wrapper_inputs.push(argument.clone());
                call_args.push(ident.ident.clone());
            }
            FnArg::Receiver(receiver) => {
                return Err(syn::Error::new_spanned(
                    receiver,
                    "methods cannot be exported",
                ));
            }
        }
    }
    Ok(quote! {
        #function

        #[::napi_derive::napi(js_name = #js_name)]
        pub fn #wrapper(#(#wrapper_inputs),*) -> ::napi::Result<::napi::bindgen_prelude::Buffer> {
            ::zctf_napi::to_buffer(&#original_name(#(#call_args),*))
        }
    })
}

fn snake_to_camel(name: &str) -> String {
    let mut output = String::new();
    let mut uppercase = false;
    for ch in name.chars() {
        if ch == '_' {
            uppercase = true;
        } else if uppercase {
            output.extend(ch.to_uppercase());
            uppercase = false;
        } else {
            output.push(ch);
        }
    }
    output
}
