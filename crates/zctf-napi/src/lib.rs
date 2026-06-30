use napi::bindgen_prelude::*;
use napi_derive::napi;
use zctf_core::{
    consume_bench_report as consume_report, consume_compiled_config,
    consume_compiled_config_repeated, make_bench_report,
};

#[napi(object)]
pub struct PackageObject {
    pub name: String,
    pub version: String,
    pub size: u32,
    pub dependency_count: u32,
}

#[napi(object)]
pub struct BenchReportObject {
    pub package_count: u32,
    pub total_size: i64,
    pub duration_ms: f64,
    pub packages: Vec<PackageObject>,
}

#[napi]
pub fn make_report_buffer(count: u32) -> Buffer {
    make_bench_report(count).into()
}

#[napi]
pub fn make_report_object(count: u32) -> BenchReportObject {
    let packages = (0..count)
        .map(|i| PackageObject {
            name: format!("package-{i}"),
            version: format!("1.{}.{}", i % 100, i % 10),
            size: i.wrapping_mul(17),
            dependency_count: i % 32,
        })
        .collect();
    BenchReportObject {
        package_count: count,
        total_size: count as i64 * 1024,
        duration_ms: count as f64 / 100.0,
        packages,
    }
}

#[napi]
pub fn make_report_json(count: u32) -> String {
    let mut out = String::with_capacity(count as usize * 90 + 80);
    out.push_str(&format!(
        "{{\"packageCount\":{count},\"totalSize\":{},\"durationMs\":{},\"packages\":[",
        count as u64 * 1024,
        count as f64 / 100.0
    ));
    for i in 0..count {
        if i != 0 {
            out.push(',');
        }
        out.push_str(&format!(
            "{{\"name\":\"package-{i}\",\"version\":\"1.{}.{}\",\"size\":{},\"dependencyCount\":{}}}",
            i % 100,
            i % 10,
            i.wrapping_mul(17),
            i % 32
        ));
    }
    out.push_str("]}");
    out
}

#[napi]
pub fn consume_report_buffer(bytes: &[u8]) -> Result<i64> {
    consume_report(bytes)
        .map(|value| value as i64)
        .map_err(Error::from_reason)
}

#[napi(object)]
pub struct PluginInput {
    pub name: String,
    pub active: Option<bool>,
    pub current_color: Option<bool>,
}

#[napi(object)]
pub struct SvgoConfigInput {
    pub multipass: Option<bool>,
    pub float_precision: Option<f64>,
    pub plugins: Option<Vec<PluginInput>>,
}

#[napi(object)]
pub struct TransformConfigInput {
    pub typescript: Option<bool>,
    pub jsx_runtime: Option<String>,
    pub export_type: Option<String>,
    pub plugins: Option<Vec<String>>,
    pub svgo: Option<bool>,
    pub svgo_config: Option<SvgoConfigInput>,
}

fn consume_object(config: &TransformConfigInput) -> u64 {
    let mut checksum = config.typescript.unwrap_or_default() as u64
        + config.svgo.unwrap_or_default() as u64
        + config.jsx_runtime.as_ref().map_or(0, |v| v.len() as u64)
        + config.export_type.as_ref().map_or(0, |v| v.len() as u64);
    if let Some(plugins) = &config.plugins {
        for plugin in plugins {
            checksum = checksum.wrapping_add(plugin.len() as u64);
        }
    }
    if let Some(svgo) = &config.svgo_config {
        checksum = checksum
            .wrapping_add(svgo.multipass.unwrap_or_default() as u64)
            .wrapping_add(svgo.float_precision.unwrap_or_default().to_bits());
        if let Some(plugins) = &svgo.plugins {
            for plugin in plugins {
                checksum = checksum
                    .wrapping_add(plugin.name.len() as u64)
                    .wrapping_add(plugin.active.unwrap_or_default() as u64)
                    .wrapping_add(plugin.current_color.unwrap_or_default() as u64);
            }
        }
    }
    checksum
}

#[napi]
pub fn consume_config_object(config: TransformConfigInput) -> i64 {
    consume_object(&config) as i64
}

#[napi]
pub fn consume_config_json(json: String) -> i64 {
    serde_json::from_str::<serde_json::Value>(&json)
        .map(|value| checksum_json(&value) as i64)
        .unwrap_or_default()
}

#[napi]
pub fn consume_config_buffer(bytes: &[u8]) -> Result<i64> {
    consume_compiled_config(bytes)
        .map(|summary| summary.checksum as i64)
        .map_err(Error::from_reason)
}

#[napi]
pub fn consume_config_buffer_repeated(bytes: &[u8], reads: u32, promote: bool) -> Result<i64> {
    consume_compiled_config_repeated(bytes, reads, promote)
        .map(|checksum| checksum as i64)
        .map_err(Error::from_reason)
}

#[napi]
pub fn transform_config_object(input: String, config: TransformConfigInput) -> i64 {
    consume_object(&config).wrapping_add(input.len() as u64) as i64
}

#[napi]
pub fn transform_config_json(input: String, json: String) -> i64 {
    (checksum_json(&serde_json::from_str::<serde_json::Value>(&json).unwrap_or_default()))
        .wrapping_add(input.len() as u64) as i64
}

#[napi]
pub fn transform_config_buffer(input: String, bytes: &[u8]) -> Result<i64> {
    consume_compiled_config(bytes)
        .map(|summary| summary.checksum.wrapping_add(input.len() as u64) as i64)
        .map_err(Error::from_reason)
}

fn checksum_json(value: &serde_json::Value) -> u64 {
    match value {
        serde_json::Value::Null => 0,
        serde_json::Value::Bool(value) => *value as u64,
        serde_json::Value::Number(value) => value.as_f64().unwrap_or_default().to_bits(),
        serde_json::Value::String(value) => value.len() as u64,
        serde_json::Value::Array(values) => values
            .iter()
            .fold(0, |sum, value| sum.wrapping_add(checksum_json(value))),
        serde_json::Value::Object(values) => values.iter().fold(0, |sum, (key, value)| {
            sum.wrapping_add(key.len() as u64)
                .wrapping_add(checksum_json(value))
        }),
    }
}

#[napi]
pub fn consume_report_object(packages: Vec<PackageObject>) -> i64 {
    packages.iter().fold(0u64, |sum, package| {
        sum.wrapping_add(package.size as u64)
            .wrapping_add(package.dependency_count as u64)
            .wrapping_add(package.name.len() as u64)
    }) as i64
}

#[napi]
pub fn consume_report_json(json: String) -> i64 {
    serde_json::from_str::<serde_json::Value>(&json)
        .map(|value| checksum_json(&value) as i64)
        .unwrap_or_default()
}

#[napi]
pub struct ConfigHandle {
    checksum: Option<u64>,
}

#[napi]
impl ConfigHandle {
    #[napi(factory)]
    pub fn create(bytes: &[u8]) -> Result<Self> {
        let summary = consume_compiled_config(bytes).map_err(Error::from_reason)?;
        Ok(Self {
            checksum: Some(summary.checksum),
        })
    }

    #[napi]
    pub fn transform(&self, input: String) -> Result<i64> {
        self.checksum
            .map(|checksum| checksum.wrapping_add(input.len() as u64) as i64)
            .ok_or_else(|| Error::from_reason("ConfigHandle is disposed"))
    }

    #[napi]
    pub fn dispose(&mut self) {
        self.checksum = None;
    }
}
