import { mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { performance } from "node:perf_hooks";
import { BenchReportView } from "../fixtures/bench-report.view.js";

const outputArgument = process.argv.find((argument) => argument.startsWith("--output="));
const path = resolve(
  import.meta.dirname,
  "../../../target/wasm32-unknown-unknown/release/zctf_wasm.wasm",
);
const module = await WebAssembly.instantiate(await readFile(path));
const api = module.instance.exports;

const results = [];
for (const count of [1_000, 10_000, 100_000]) {
  const start = performance.now();
  const handle = api.zctf_make_bench_report(count);
  const offset = api.zctf_buffer_ptr(handle);
  const length = api.zctf_buffer_len(handle);
  const bytes = new Uint8Array(api.memory.buffer, offset, length);
  const report = BenchReportView.from(bytes);
  const first = report.packages.get(0).name;
  const checksum = api.zctf_consume_bench_report(handle);
  const elapsed = performance.now() - start;
  const row = { count, elapsedMs: +elapsed.toFixed(3), length, first, checksum: String(checksum) };
  results.push(row);
  console.log(JSON.stringify(row));
  api.zctf_release(handle);
}
const output = outputArgument
  ? resolve(process.cwd(), outputArgument.slice("--output=".length))
  : resolve(import.meta.dirname, "../../../benchmark-results/wasm.json");
const outputDir = resolve(output, "..");
await mkdir(outputDir, { recursive: true });
await writeFile(
  output,
  `${JSON.stringify(
    { metadata: { timestamp: new Date().toISOString(), node: process.version, platform: `${process.platform}-${process.arch}` }, results },
    null,
    2,
  )}\n`,
);
