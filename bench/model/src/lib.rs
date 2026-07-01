#[zctf::enum_repr(u8)]
pub enum WarningLevel {
    Info,
    Warn,
    Error,
}

impl serde::Serialize for WarningLevel {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u8(match self {
            Self::Info => 0,
            Self::Warn => 1,
            Self::Error => 2,
        })
    }
}

#[zctf::record]
#[derive(serde::Serialize)]
pub struct Warning {
    pub level: WarningLevel,
    #[zctf(string(direct, encoding = "utf8"))]
    pub message: String,
    pub start: u32,
    pub end: u32,
}

#[zctf::document]
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransformResult {
    #[zctf(string(direct, encoding = "utf8"))]
    pub code: String,
    pub duration_ms: f64,
    pub warnings: Vec<Warning>,
}

pub fn transform_code(source: &str) -> String {
    format!("export default function Icon() {{ return {:?}; }}", source)
}

pub fn warning_message(index: u32) -> String {
    format!("warning-{index}: generated benchmark diagnostic")
}

pub fn warning(index: u32) -> Warning {
    Warning {
        level: match index % 3 {
            0 => WarningLevel::Info,
            1 => WarningLevel::Warn,
            _ => WarningLevel::Error,
        },
        message: warning_message(index),
        start: index * 10,
        end: index * 10 + 5,
    }
}

pub fn transform_result(source: &str, warning_count: u32) -> TransformResult {
    TransformResult {
        code: transform_code(source),
        duration_ms: 1.25,
        warnings: (0..warning_count).map(warning).collect(),
    }
}
