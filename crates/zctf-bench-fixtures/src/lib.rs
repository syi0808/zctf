mod config;
mod report;

pub use config::{ConfigSummary, consume_compiled_config, consume_compiled_config_repeated};
pub use report::{
    consume_bench_report, make_bench_report, make_bench_report_compact,
    make_bench_report_direct_string_ref, make_bench_report_sidecar, make_bench_report_soa,
};
