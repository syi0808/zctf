use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{Attribute, Fields, ItemEnum, ItemStruct, Type, parse_macro_input};
use zctf_schema::{EnumVariant, Field, RecordKind, SchemaFragment, Type as SchemaType};

#[proc_macro_attribute]
pub fn document(_args: TokenStream, input: TokenStream) -> TokenStream {
    expand_struct(
        parse_macro_input!(input as ItemStruct),
        RecordKind::Document,
    )
    .unwrap_or_else(syn::Error::into_compile_error)
    .into()
}

#[proc_macro_attribute]
pub fn record(_args: TokenStream, input: TokenStream) -> TokenStream {
    expand_struct(parse_macro_input!(input as ItemStruct), RecordKind::Record)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro_attribute]
pub fn enum_repr(args: TokenStream, input: TokenStream) -> TokenStream {
    let repr = args.to_string();
    expand_enum(parse_macro_input!(input as ItemEnum), &repr)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[derive(Default)]
struct FieldAttrs {
    js_name: Option<String>,
    skip: bool,
    direct: bool,
}

fn parse_field_attrs(attrs: &[Attribute]) -> syn::Result<FieldAttrs> {
    let mut out = FieldAttrs::default();
    for attr in attrs.iter().filter(|a| a.path().is_ident("zctf")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("skip") {
                out.skip = true;
                return Ok(());
            }
            if meta.path.is_ident("js_name") {
                out.js_name = Some(meta.value()?.parse::<syn::LitStr>()?.value());
                return Ok(());
            }
            if meta.path.is_ident("string") {
                meta.parse_nested_meta(|inner| {
                    if inner.path.is_ident("direct") {
                        out.direct = true;
                        return Ok(());
                    }
                    if inner.path.is_ident("encoding") {
                        let value = inner.value()?.parse::<syn::LitStr>()?;
                        if value.value() != "utf8" {
                            return Err(inner.error("only utf8 encoding is supported"));
                        }
                        return Ok(());
                    }
                    Err(inner.error("unsupported zctf string option"))
                })?;
                return Ok(());
            }
            Err(meta.error("unsupported zctf field attribute"))
        })?;
    }
    Ok(out)
}

fn schema_type(ty: &Type, direct: bool) -> syn::Result<SchemaType> {
    let Type::Path(path) = ty else {
        return Err(syn::Error::new_spanned(ty, "unsupported zctf field type"));
    };
    if path.qself.is_some() {
        return Err(syn::Error::new_spanned(
            ty,
            "qualified types are unsupported",
        ));
    }
    let segment = path.path.segments.last().unwrap();
    let name = segment.ident.to_string();
    let primitive = match name.as_str() {
        "bool" => Some(SchemaType::Bool),
        "u8" => Some(SchemaType::U8),
        "u16" => Some(SchemaType::U16),
        "u32" => Some(SchemaType::U32),
        "u64" => Some(SchemaType::U64),
        "i8" => Some(SchemaType::I8),
        "i16" => Some(SchemaType::I16),
        "i32" => Some(SchemaType::I32),
        "i64" => Some(SchemaType::I64),
        "f32" => Some(SchemaType::F32),
        "f64" => Some(SchemaType::F64),
        "String" => Some(SchemaType::String {
            encoding: "utf8".into(),
            direct,
        }),
        _ => None,
    };
    if let Some(ty) = primitive {
        return Ok(ty);
    }
    if name == "Vec" || name == "Option" {
        let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
            return Err(syn::Error::new_spanned(ty, "generic argument required"));
        };
        let Some(syn::GenericArgument::Type(inner)) = args.args.first() else {
            return Err(syn::Error::new_spanned(ty, "type argument required"));
        };
        let item = Box::new(schema_type(inner, direct)?);
        return Ok(if name == "Vec" {
            SchemaType::List { item }
        } else {
            SchemaType::Option { item }
        });
    }
    if path.path.segments.len() != 1 {
        return Err(syn::Error::new_spanned(
            ty,
            "use an unqualified record or enum type",
        ));
    }
    Ok(SchemaType::Named { name })
}

fn expand_struct(mut item: ItemStruct, kind: RecordKind) -> syn::Result<proc_macro2::TokenStream> {
    let Fields::Named(fields) = &mut item.fields else {
        return Err(syn::Error::new_spanned(
            &item,
            "zctf requires a struct with named fields",
        ));
    };
    let mut schema_fields = Vec::new();
    let mut encoded = Vec::new();
    for field in &mut fields.named {
        let ident = field.ident.clone().unwrap();
        let attrs = parse_field_attrs(&field.attrs)?;
        field.attrs.retain(|a| !a.path().is_ident("zctf"));
        if attrs.skip {
            continue;
        }
        if attrs.direct && !supports_direct_string(&field.ty) {
            return Err(syn::Error::new_spanned(
                &field.ty,
                "zctf string(direct) is only supported on String or Option<String>",
            ));
        }
        let rust_name = ident.to_string();
        let schema_ty = schema_type(&field.ty, attrs.direct)?;
        schema_fields.push(Field {
            js_name: attrs
                .js_name
                .unwrap_or_else(|| zctf_schema::snake_to_camel(&rust_name)),
            rust_name: rust_name.clone(),
            ty: schema_ty,
            skip: false,
        });
        encoded.push((
            ident,
            field.ty.clone(),
            rust_name,
            type_id_expr(&field.ty, attrs.direct)?,
            attrs.direct,
        ));
    }
    let fragment = SchemaFragment {
        zctf: "1".into(),
        name: item.ident.to_string(),
        kind,
        layout_version: 1,
        fields: schema_fields,
        repr: None,
        variants: vec![],
    };
    emit_fragment(&fragment)?;
    let name = &item.ident;
    let name_string = name.to_string();
    let kind_string = match kind {
        RecordKind::Document => "document",
        RecordKind::Record => "record",
        RecordKind::Enum => unreachable!(),
    };

    let mut size_steps = Vec::new();
    let mut max_steps = Vec::new();
    let mut write_steps = Vec::new();
    let mut hash_steps = Vec::new();
    for (ident, ty, rust_name, type_id, direct) in &encoded {
        let field_trait = if *direct {
            quote!(::zctf::ZctfDirectField)
        } else {
            quote!(::zctf::ZctfField)
        };
        size_steps.push(quote! {
            cursor = ::zctf::align_up(cursor, <#ty as #field_trait>::ALIGN);
            cursor += <#ty as #field_trait>::SIZE;
        });
        max_steps.push(quote! {
            if <#ty as #field_trait>::ALIGN > max_align {
                max_align = <#ty as #field_trait>::ALIGN;
            }
        });
        if *direct {
            write_steps.push(quote! {
                cursor = ::zctf::align_up(cursor, <#ty as ::zctf::ZctfDirectField>::ALIGN);
                <#ty as ::zctf::ZctfDirectField>::write_direct_field(&self.#ident, writer, offset + cursor)?;
                cursor += <#ty as ::zctf::ZctfDirectField>::SIZE;
            });
        } else {
            write_steps.push(quote! {
                cursor = ::zctf::align_up(cursor, <#ty as ::zctf::ZctfField>::ALIGN);
                <#ty as ::zctf::ZctfField>::write_field(&self.#ident, writer, offset + cursor)?;
                cursor += <#ty as ::zctf::ZctfField>::SIZE;
            });
        }
        hash_steps.push(quote! {
            hash = ::zctf::schema_hash_str(hash, #rust_name);
            hash = ::zctf::schema_hash_u64(hash, #type_id);
        });
    }
    let schema_id_expr = quote! {{
        let mut hash = ::zctf::schema_hash_str(::zctf::SCHEMA_HASH_OFFSET, #kind_string);
        hash = ::zctf::schema_hash_str(hash, #name_string);
        #(#hash_steps)*
        hash
    }};
    let size_expr = quote! {{
        let mut cursor = 0usize;
        let mut max_align = 1usize;
        #(#size_steps)*
        #(#max_steps)*
        ::zctf::align_up(cursor, max_align)
    }};
    let align_expr = quote! {{
        let mut max_align = 1usize;
        #(#max_steps)*
        max_align
    }};
    let record_body = quote! {
        let mut cursor = 0usize;
        #(#write_steps)*
        let _ = cursor;
        Ok(())
    };
    let implementation = if kind == RecordKind::Document {
        quote! {
            impl ::zctf::ZctfSchemaType for #name {
                const TYPE_ID: u64 = #schema_id_expr;
            }
            impl ::zctf::ZctfDocument for #name {
                const SCHEMA_ID: u64 = <Self as ::zctf::ZctfSchemaType>::TYPE_ID;
                const LAYOUT_VERSION: u32 = 1;
                fn encode_zctf(&self, writer: &mut ::zctf::ZctfWriter) -> ::zctf::Result<()> {
                    let offset = writer.begin_document(Self::SCHEMA_ID, Self::LAYOUT_VERSION, #size_expr)?;
                    #record_body
                }
            }
        }
    } else {
        quote! {
            impl ::zctf::ZctfSchemaType for #name {
                const TYPE_ID: u64 = #schema_id_expr;
            }
            impl ::zctf::ZctfRecord for #name {
                const SIZE: usize = #size_expr;
                const ALIGN: usize = #align_expr;
                fn encode_record(&self, writer: &mut ::zctf::ZctfWriter, offset: usize) -> ::zctf::Result<()> {
                    #record_body
                }
            }
            impl ::zctf::ZctfField for #name {
                const SIZE: usize = <Self as ::zctf::ZctfRecord>::SIZE;
                const ALIGN: usize = <Self as ::zctf::ZctfRecord>::ALIGN;
                fn write_field(&self, writer: &mut ::zctf::ZctfWriter, offset: usize) -> ::zctf::Result<()> {
                    <Self as ::zctf::ZctfRecord>::encode_record(self, writer, offset)
                }
            }
        }
    };
    Ok(quote! { #item #implementation })
}

fn supports_direct_string(ty: &Type) -> bool {
    let Type::Path(path) = ty else {
        return false;
    };
    let Some(segment) = path.path.segments.last() else {
        return false;
    };
    if segment.ident == "String" {
        return true;
    }
    if segment.ident != "Option" {
        return false;
    }
    let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
        return false;
    };
    matches!(
        args.args.first(),
        Some(syn::GenericArgument::Type(Type::Path(inner)))
            if inner.path.segments.last().is_some_and(|segment| segment.ident == "String")
    )
}

fn type_id_expr(ty: &Type, direct: bool) -> syn::Result<proc_macro2::TokenStream> {
    let Type::Path(path) = ty else {
        return Err(syn::Error::new_spanned(ty, "unsupported zctf field type"));
    };
    let segment = path.path.segments.last().unwrap();
    let name = segment.ident.to_string();
    let primitive = match name.as_str() {
        "bool" | "u8" | "u16" | "u32" | "u64" | "i8" | "i16" | "i32" | "i64" | "f32" | "f64" => {
            Some(name.clone())
        }
        "String" => Some(format!("string:utf8:{direct}")),
        _ => None,
    };
    if let Some(tag) = primitive {
        return Ok(quote! {
            ::zctf::schema_hash_str(::zctf::SCHEMA_HASH_OFFSET, #tag)
        });
    }
    if name == "Vec" || name == "Option" {
        let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
            return Err(syn::Error::new_spanned(ty, "generic argument required"));
        };
        let Some(syn::GenericArgument::Type(inner)) = args.args.first() else {
            return Err(syn::Error::new_spanned(ty, "type argument required"));
        };
        let inner_id = type_id_expr(inner, direct)?;
        let tag = if name == "Vec" { "list" } else { "option" };
        return Ok(quote! {
            ::zctf::schema_hash_u64(
                ::zctf::schema_hash_str(::zctf::SCHEMA_HASH_OFFSET, #tag),
                #inner_id,
            )
        });
    }
    Ok(quote! { <#ty as ::zctf::ZctfSchemaType>::TYPE_ID })
}

fn expand_enum(mut item: ItemEnum, repr: &str) -> syn::Result<proc_macro2::TokenStream> {
    if !matches!(
        repr,
        "u8" | "u16" | "u32" | "u64" | "i8" | "i16" | "i32" | "i64"
    ) {
        return Err(syn::Error::new_spanned(
            &item,
            "enum repr must be an integer type",
        ));
    }
    let repr_ident = format_ident!("{repr}");
    let mut variants = Vec::new();
    let mut matches = Vec::new();
    let mut next = 0i64;
    for variant in &item.variants {
        if !matches!(variant.fields, Fields::Unit) {
            return Err(syn::Error::new_spanned(
                variant,
                "data-carrying enums are unsupported",
            ));
        }
        let value = if let Some((_, expression)) = &variant.discriminant {
            let syn::Expr::Lit(lit) = expression else {
                return Err(syn::Error::new_spanned(
                    expression,
                    "integer discriminant required",
                ));
            };
            let syn::Lit::Int(integer) = &lit.lit else {
                return Err(syn::Error::new_spanned(
                    expression,
                    "integer discriminant required",
                ));
            };
            integer.base10_parse::<i64>()?
        } else {
            next
        };
        next = value + 1;
        variants.push(EnumVariant {
            name: variant.ident.to_string(),
            value,
        });
        let ident = &variant.ident;
        matches.push(quote! { Self::#ident => #value as #repr_ident });
    }
    item.attrs.retain(|a| !a.path().is_ident("zctf"));
    let fragment = SchemaFragment {
        zctf: "1".into(),
        name: item.ident.to_string(),
        kind: RecordKind::Enum,
        layout_version: 1,
        fields: vec![],
        repr: Some(repr.into()),
        variants,
    };
    emit_fragment(&fragment)?;
    let name = &item.ident;
    let name_string = name.to_string();
    let mut enum_hash_steps = Vec::new();
    for variant in &fragment.variants {
        let variant_name = &variant.name;
        let value = variant.value as u64;
        enum_hash_steps.push(quote! {
            hash = ::zctf::schema_hash_str(hash, #variant_name);
            hash = ::zctf::schema_hash_u64(hash, #value);
        });
    }
    let size: usize = match repr {
        "u8" | "i8" => 1,
        "u16" | "i16" => 2,
        "u32" | "i32" => 4,
        _ => 8,
    };
    let write = match size {
        1 => quote!(writer.set_u8(offset, value as u8)),
        2 => quote!(writer.set_u16(offset, value as u16)),
        4 => quote!(writer.set_u32(offset, value as u32)),
        _ => quote!(writer.set_u64(offset, value as u64)),
    };
    Ok(quote! {
        #item
        impl ::zctf::ZctfSchemaType for #name {
            const TYPE_ID: u64 = {
                let mut hash = ::zctf::schema_hash_str(::zctf::SCHEMA_HASH_OFFSET, "enum");
                hash = ::zctf::schema_hash_str(hash, #name_string);
                hash = ::zctf::schema_hash_str(hash, #repr);
                #(#enum_hash_steps)*
                hash
            };
        }
        impl #name {
            pub const fn to_zctf_repr(&self) -> #repr_ident {
                match self { #(#matches),* }
            }
        }
        impl ::zctf::ZctfField for #name {
            const SIZE: usize = #size;
            const ALIGN: usize = #size;
            fn write_field(&self, writer: &mut ::zctf::ZctfWriter, offset: usize) -> ::zctf::Result<()> {
                let value = self.to_zctf_repr();
                #write
            }
        }
    })
}

fn emit_fragment(fragment: &SchemaFragment) -> syn::Result<()> {
    let directory = std::env::var_os("ZCTF_SCHEMA_OUT_DIR").or_else(|| std::env::var_os("OUT_DIR"));
    let Some(directory) = directory else {
        return Ok(());
    };
    let directory = std::path::PathBuf::from(directory);
    std::fs::create_dir_all(&directory)
        .map_err(|e| syn::Error::new(proc_macro2::Span::call_site(), e.to_string()))?;
    let filename = format!("{}.schema.json", to_kebab(&fragment.name));
    let bytes = serde_json::to_vec_pretty(fragment)
        .map_err(|e| syn::Error::new(proc_macro2::Span::call_site(), e.to_string()))?;
    std::fs::write(directory.join(filename), bytes)
        .map_err(|e| syn::Error::new(proc_macro2::Span::call_site(), e.to_string()))
}

fn to_kebab(value: &str) -> String {
    let mut out = String::new();
    for (index, ch) in value.chars().enumerate() {
        if ch.is_uppercase() {
            if index > 0 {
                out.push('-');
            }
            out.extend(ch.to_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}
