use napi_derive::napi;
use zctf_product_bench_model::{
    TransformResult, transform_code, transform_result, warning_message,
};

#[napi(object)]
pub struct WarningObject {
    pub level: u8,
    pub message: String,
    pub start: u32,
    pub end: u32,
}

#[napi(object)]
pub struct TransformObject {
    pub code: String,
    pub duration_ms: f64,
    pub warnings: Vec<WarningObject>,
}

fn warning_object(index: u32) -> WarningObject {
    WarningObject {
        level: (index % 3) as u8,
        message: warning_message(index),
        start: index * 10,
        end: index * 10 + 5,
    }
}

#[napi]
pub fn transform_object(source: String, warning_count: u32) -> TransformObject {
    TransformObject {
        code: transform_code(&source),
        duration_ms: 1.25,
        warnings: (0..warning_count).map(warning_object).collect(),
    }
}

#[zctf_napi::export(name = "transformZctf", return = "buffer")]
pub fn transform_zctf_document(source: String, warning_count: u32) -> TransformResult {
    transform_result(&source, warning_count)
}

#[napi]
pub fn transform_zctf_manual(
    source: String,
    warning_count: u32,
) -> napi::Result<napi::bindgen_prelude::Buffer> {
    let value = transform_result(&source, warning_count);
    let mut writer =
        zctf::ZctfWriter::with_capacity(value.code.len() + warning_count as usize * 64);
    let root = writer
        .begin_document(
            <TransformResult as zctf::ZctfDocument>::SCHEMA_ID,
            <TransformResult as zctf::ZctfDocument>::LAYOUT_VERSION,
            24,
        )
        .map_err(|error| napi::Error::from_reason(error.to_string()))?;
    writer
        .set_direct_string(root, &value.code)
        .map_err(|error| napi::Error::from_reason(error.to_string()))?;
    writer
        .set_f64(root + 8, value.duration_ms)
        .map_err(|error| napi::Error::from_reason(error.to_string()))?;
    writer
        .write_fixed_list(root + 16, &value.warnings)
        .map_err(|error| napi::Error::from_reason(error.to_string()))?;
    writer
        .finish()
        .map(Into::into)
        .map_err(|error| napi::Error::from_reason(error.to_string()))
}
