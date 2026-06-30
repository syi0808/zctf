import { dlopen, FFIType, ptr, toArrayBuffer } from "bun:ffi";
import { mkdirSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { BenchReportView } from "../../runtime/src/bench-report.view.js";

const extension = process.platform === "darwin" ? "dylib" : process.platform === "win32" ? "dll" : "so";
const prefix = process.platform === "win32" ? "" : "lib";
const library = resolve(import.meta.dirname, `../../../target/release/${prefix}zctf_ffi.${extension}`);
const { symbols } = dlopen(library, {
  zctf_make_bench_report: { args: [FFIType.u32], returns: FFIType.u64 },
  zctf_buffer_ptr: { args: [FFIType.u64], returns: FFIType.ptr },
  zctf_buffer_len: { args: [FFIType.u64], returns: FFIType.u32 },
  zctf_consume_bench_report: { args: [FFIType.u64], returns: FFIType.u64 },
  zctf_release: { args: [FFIType.u64], returns: FFIType.void },
});

const results = [];
for (const count of [1_000, 10_000, 100_000]) {
  const start = performance.now();
  const handle = symbols.zctf_make_bench_report(count);
  const pointer = symbols.zctf_buffer_ptr(handle);
  const length = symbols.zctf_buffer_len(handle);
  const bytes = new Uint8Array(toArrayBuffer(pointer, 0, length));
  const report = BenchReportView.from(bytes);
  const first = report.packages.get(0).name;
  const checksum = symbols.zctf_consume_bench_report(handle);
  const elapsed = performance.now() - start;
  const row = { count, elapsedMs: +elapsed.toFixed(3), length, first, checksum: String(checksum) };
  results.push(row);
  console.log(JSON.stringify(row));
  symbols.zctf_release(handle);
}
const outputDir = resolve(import.meta.dirname, "../../../benchmark-results");
mkdirSync(outputDir, { recursive: true });
writeFileSync(
  resolve(outputDir, "bun-ffi.json"),
  `${JSON.stringify(
    { metadata: { timestamp: new Date().toISOString(), bun: Bun.version, platform: `${process.platform}-${process.arch}` }, results },
    null,
    2,
  )}\n`,
);
