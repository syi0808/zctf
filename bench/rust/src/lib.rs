use napi_derive::napi;

#[zctf::enum_repr(u8)]
pub enum WarningLevel {
    Info,
    Warn,
    Error,
}

#[zctf::record]
pub struct Warning {
    pub level: WarningLevel,
    pub message: String,
    pub start: u32,
    pub end: u32,
}

#[zctf::document]
pub struct TransformResult {
    #[zctf(string(direct, encoding = "utf8"))]
    pub code: String,
    pub duration_ms: f64,
    pub warnings: Vec<Warning>,
}

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

fn warning(index: u32) -> Warning {
    Warning {
        level: match index % 3 {
            0 => WarningLevel::Info,
            1 => WarningLevel::Warn,
            _ => WarningLevel::Error,
        },
        message: format!("warning-{index}: generated benchmark diagnostic"),
        start: index * 10,
        end: index * 10 + 5,
    }
}

fn result(source: &str, warning_count: u32) -> TransformResult {
    TransformResult {
        code: format!("export default function Icon() {{ return {:?}; }}", source),
        duration_ms: 1.25,
        warnings: (0..warning_count).map(warning).collect(),
    }
}

#[napi]
pub fn transform_object(source: String, warning_count: u32) -> TransformObject {
    let value = result(&source, warning_count);
    TransformObject {
        code: value.code,
        duration_ms: value.duration_ms,
        warnings: value
            .warnings
            .into_iter()
            .map(|warning| WarningObject {
                level: warning.level.to_zctf_repr(),
                message: warning.message,
                start: warning.start,
                end: warning.end,
            })
            .collect(),
    }
}

#[zctf_napi::export(name = "transformZctf", return = "buffer")]
pub fn transform_zctf_document(source: String, warning_count: u32) -> TransformResult {
    result(&source, warning_count)
}

#[napi]
pub fn transform_zctf_manual(
    source: String,
    warning_count: u32,
) -> napi::Result<napi::bindgen_prelude::Buffer> {
    let value = result(&source, warning_count);
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
        .set_string(root, &value.code)
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
