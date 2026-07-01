pub use zctf_napi_macros::export;

pub fn to_buffer<T: zctf::ZctfDocument>(value: &T) -> napi::Result<napi::bindgen_prelude::Buffer> {
    zctf::encode_owned(value)
        .map(Into::into)
        .map_err(|error| napi::Error::from_reason(error.to_string()))
}
