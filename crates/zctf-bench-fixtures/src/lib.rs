mod config;
mod report;

pub use config::{ConfigSummary, consume_compiled_config, consume_compiled_config_repeated};
pub use report::{
    consume_bench_report, count_report_name_prefix, make_bench_report, make_bench_report_compact,
    make_bench_report_compact_parallel, make_bench_report_compact_sequential,
    make_bench_report_direct_string_ref, make_bench_report_direct_string_ref_parallel,
    make_bench_report_direct_string_ref_sequential, make_bench_report_sidecar,
    make_bench_report_soa, make_bench_report_soa_parallel, make_bench_report_soa_sequential,
    sum_report_dependency_counts, sum_report_name_byte_lengths, sum_report_sizes,
};
