import { mkdirSync, writeFileSync } from "node:fs";
import { performance } from "node:perf_hooks";
import { resolve } from "node:path";
import native from "./native.js";
import { BenchReportView } from "../../runtime/src/bench-report.view.js";
import {
  ConfigWriter,
  compileConfig,
  compileConfigInto,
  createConfig,
  withCompiledConfig,
} from "../../config/src/transform-config.compiler.js";
import { compileConfigBaseline } from "../../config/src/transform-config.baseline.js";

const quick = process.argv.includes("--quick");
const sizes = quick ? [1_000, 10_000] : [1_000, 10_000, 100_000, 1_000_000];
const result = {
  metadata: {
    timestamp: new Date().toISOString(),
    platform: `${process.platform}-${process.arch}`,
    node: process.version,
    quick,
    unit: "milliseconds",
  },
  rustToJs: [],
  configInput: [],
  mutablePipeline: [],
};
let sink = 0;

function median(values) {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.floor(sorted.length / 2)];
}

function measure(fn, repetitions = 7) {
  for (let i = 0; i < 2; i++) sink ^= Number(fn()) || 0;
  const values = [];
  for (let i = 0; i < repetitions; i++) {
    global.gc?.();
    const start = performance.now();
    sink ^= Number(fn()) || 0;
    values.push(performance.now() - start);
  }
  return Number(median(values).toFixed(3));
}

function repetitionsFor(count) {
  return count >= 1_000_000 ? 3 : count >= 100_000 ? 5 : 7;
}

function measurePerCall(fn, iterations, repetitions = 7) {
  return Number(
    (
      measure(() => {
        let value = 0;
        for (let index = 0; index < iterations; index++) value ^= Number(fn()) || 0;
        return value;
      }, repetitions) / iterations
    ).toFixed(6),
  );
}

console.log("Benchmark 1/3: Rust → JS result");
for (const count of sizes) {
  const repetitions = repetitionsFor(count);
  const row = { count };
  row.objectReturn = measure(() => native.makeReportObject(count).packages.length, repetitions);
  row.jsonReturnAndParse = measure(
    () => JSON.parse(native.makeReportJson(count)).packages.length,
    repetitions,
  );
  row.zctfReturnAndView = measure(
    () => BenchReportView.from(native.makeReportBuffer(count)).packageCount,
    repetitions,
  );

  const buffer = native.makeReportBuffer(count);
  const view = BenchReportView.from(buffer);
  const object = native.makeReportObject(count);
  const json = native.makeReportJson(count);
  row.storageBytes = {
    jsonUtf8: Buffer.byteLength(json),
    zctf: buffer.byteLength,
  };
  row.firstAccess = {
    object: measure(() => object.packages[0].name.length),
    jsonParse: measure(() => JSON.parse(json).packages[0].name.length, repetitions),
    zctf: measure(() => view.packages.get(0).name.length),
  };
  row.sumSizes = {
    object: measure(() => {
      let sum = 0;
      for (const item of object.packages) sum += item.size;
      return sum;
    }, repetitions),
    zctf: measure(() => {
      let sum = 0;
      for (let i = 0; i < view.packages.length; i++) sum += view.packages.get(i).size;
      return sum;
    }, repetitions),
  };
  row.decodeNames = {
    object: measure(() => {
      let length = 0;
      for (const item of object.packages) length += item.name.length;
      return length;
    }, repetitions),
    zctf: measure(() => {
      let length = 0;
      for (let i = 0; i < view.packages.length; i++) length += view.packages.get(i).name.length;
      return length;
    }, repetitions),
  };
  row.toObject = measure(() => view.toObject().packages.length, repetitions);
  result.rustToJs.push(row);
  console.log(`  ${count.toLocaleString()} packages complete`);
}

console.log("Benchmark 2/3: JS → Rust config");
for (const shape of [
  "small",
  "medium",
  "large",
  "stringHeavy",
  "pluginHeavy",
  "unicodeHeavy",
  "defaultHeavy",
]) {
  const config = createConfig(shape);
  const compiled = compileConfig(config);
  const baselineCompiled = compileConfigBaseline(config);
  const json = JSON.stringify(config);
  const iterations = quick ? 1_000 : shape === "large" || shape.endsWith("Heavy") ? 5_000 : 20_000;
  const writer = new ConfigWriter(compiled.byteLength * 3);
  compileConfigInto(writer, config);
  const handle = native.ConfigHandle.create(compiled);
  const input = "<svg viewBox=\"0 0 24 24\"><path d=\"M0 0h24v24H0z\"/></svg>";
  const perCall = (fn) => measurePerCall(fn, iterations, quick ? 3 : 7);
  const row = {
    shape,
    iterations,
    bytes: {
      json: Buffer.byteLength(json),
      baselineZctf: baselineCompiled.byteLength,
      optimizedZctf: compiled.byteLength,
    },
    strings: {
      known: writer.knownNames,
      ascii: writer.asciiStrings,
      utf8Fallback: writer.utf8Strings,
    },
    compileOnly: {
      baseline: perCall(() => compileConfigBaseline(config).byteLength),
      optimized: perCall(() => compileConfig(config).byteLength),
      tempWriter: perCall(() => compileConfigInto(writer, config).byteLength),
    },
    rustReadOnly: {
      baseline: perCall(() => native.consumeConfigBuffer(baselineCompiled)),
      optimized: perCall(() => native.consumeConfigBuffer(compiled)),
      repeatedView100: perCall(() =>
        native.consumeConfigBufferRepeated(compiled, 100, false),
      ),
      promotedLocal100: perCall(() =>
        native.consumeConfigBufferRepeated(compiled, 100, true),
      ),
    },
    single: {
      napiObject: perCall(() => native.consumeConfigObject(config)),
      jsonStringifyAndRustParse: perCall(() =>
        native.consumeConfigJson(JSON.stringify(config)),
      ),
      baselineCompileAndRead: perCall(() =>
        native.consumeConfigBuffer(compileConfigBaseline(config)),
      ),
      optimizedCompileAndRead: perCall(() =>
        native.consumeConfigBuffer(compileConfig(config)),
      ),
      syncTempCompileAndRead: perCall(() =>
        withCompiledConfig(config, (bytes) => native.consumeConfigBuffer(bytes)),
      ),
    },
    fullTransform: {
      napiObject: perCall(() => native.transformConfigObject(input, config)),
      jsonStringifyAndRustParse: perCall(() =>
        native.transformConfigJson(input, JSON.stringify(config)),
      ),
      baselineCompileAndRead: perCall(() =>
        native.transformConfigBuffer(input, compileConfigBaseline(config)),
      ),
      optimizedCompileAndRead: perCall(() =>
        native.transformConfigBuffer(input, compileConfig(config)),
      ),
      syncTempCompileAndRead: perCall(() =>
        withCompiledConfig(config, (bytes) => native.transformConfigBuffer(input, bytes)),
      ),
    },
    reuse: {
      napiObject: measure(() => {
        let value = 0;
        for (let i = 0; i < iterations; i++) value ^= native.consumeConfigObject(config);
        return value;
      }, quick ? 3 : 5),
      jsonRustParse: measure(() => {
        let value = 0;
        for (let i = 0; i < iterations; i++) value ^= native.consumeConfigJson(json);
        return value;
      }, quick ? 3 : 5),
      compiledRead: measure(() => {
        let value = 0;
        for (let i = 0; i < iterations; i++) value ^= native.consumeConfigBuffer(compiled);
        return value;
      }, quick ? 3 : 5),
      configHandle: measure(() => {
        let value = 0;
        for (let i = 0; i < iterations; i++) value ^= handle.transform("<svg/>");
        return value;
      }, quick ? 3 : 5),
    },
  };
  result.configInput.push(row);
  console.log(`  ${shape} config complete`);
}

console.log("Benchmark 3/3: mutable JS write → Rust read");
for (const count of quick ? [1_000, 10_000] : [10_000, 100_000]) {
  const repetitions = count >= 100_000 ? 5 : 7;
  const objectReport = native.makeReportObject(count);
  const zctfBytes = native.makeReportBuffer(count);
  const zctf = BenchReportView.from(zctfBytes);
  const row = {
    count,
    numeric: {
      objectNapi: measure(() => {
        for (let i = 0; i < count; i++) objectReport.packages[i].size = i + 1;
        return native.consumeReportObject(objectReport.packages);
      }, repetitions),
      objectJson: measure(() => {
        for (let i = 0; i < count; i++) objectReport.packages[i].size = i + 1;
        return native.consumeReportJson(JSON.stringify(objectReport));
      }, repetitions),
      zctf: measure(() => {
        for (let i = 0; i < count; i++) zctf.packages.get(i).size = i + 1;
        return native.consumeReportBuffer(zctfBytes);
      }, repetitions),
    },
  };
  // Each report reserves one additional StringEntry per package, so string mutation
  // gets a fresh report per sample.
  row.string = {
    objectJson: measure(() => {
      const report = native.makeReportObject(count);
      for (let i = 0; i < count; i++) report.packages[i].name = `renamed-${i}`;
      return native.consumeReportJson(JSON.stringify(report));
    }, repetitions),
    zctf: measure(() => {
      const bytes = native.makeReportBuffer(count);
      const report = BenchReportView.from(bytes);
      for (let i = 0; i < count; i++) report.packages.get(i).name = `renamed-${i}`;
      return native.consumeReportBuffer(bytes);
    }, repetitions),
  };
  {
    const bytes = native.makeReportBuffer(count);
    const report = BenchReportView.from(bytes);
    const beforeCursor = report.doc.u32(40);
    const beforeStrings = report.doc.u32(52);
    for (let i = 0; i < count; i++) report.packages.get(i).name = `renamed-${i}`;
    row.string.heapGrowthBytes = report.doc.u32(40) - beforeCursor;
    row.string.stringEntriesAdded = report.doc.u32(52) - beforeStrings;
  }
  row.dependencyCount = {
    objectNapi: measure(() => {
      for (let i = 0; i < count; i++) objectReport.packages[i].dependencyCount = i % 64;
      return native.consumeReportObject(objectReport.packages);
    }, repetitions),
    zctf: measure(() => {
      for (let i = 0; i < count; i++) zctf.packages.get(i).dependencyCount = i % 64;
      return native.consumeReportBuffer(zctfBytes);
    }, repetitions),
  };
  row.listPush = {
    objectNapi: measure(() => {
      const report = native.makeReportObject(count);
      for (let i = 0; i < count; i++) {
        report.packages.push({
          name: `pushed-${i}`,
          version: "2.0.0",
          size: i,
          dependencyCount: i % 8,
        });
      }
      return native.consumeReportObject(report.packages);
    }, repetitions),
    zctf: measure(() => {
      const bytes = native.makeReportBuffer(count);
      const report = BenchReportView.from(bytes);
      for (let i = 0; i < count; i++) {
        report.packages.push({
          name: `pushed-${i}`,
          version: "2.0.0",
          size: i,
          dependencyCount: i % 8,
        });
      }
      return native.consumeReportBuffer(bytes);
    }, repetitions),
  };
  result.mutablePipeline.push(row);
  console.log(`  ${count.toLocaleString()} mutations complete`);
}

result.metadata.sink = sink;
const outputDir = resolve(import.meta.dirname, "../../../benchmark-results");
mkdirSync(outputDir, { recursive: true });
const output = resolve(outputDir, "napi.json");
writeFileSync(output, `${JSON.stringify(result, null, 2)}\n`);
console.log(`Results: ${output}`);
