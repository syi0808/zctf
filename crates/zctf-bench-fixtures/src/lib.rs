mod config;
mod report;

pub use config::{ConfigSummary, consume_compiled_config, consume_compiled_config_repeated};
pub use report::{consume_bench_report, make_bench_report};
