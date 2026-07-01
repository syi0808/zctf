import { mkdirSync, writeFileSync } from "node:fs";
import { performance } from "node:perf_hooks";
import { resolve } from "node:path";
import native from "./native.js";
import { BenchReportView } from "../fixtures/bench-report.view.js";
import {
  DirectStringRefReportView,
  SidecarReportView,
  SoAReportView,
} from "../fixtures/report-layout-variants.js";
import {
  ConfigWriter,
  compileConfig,
  compileConfigInto,
  withCompiledConfig,
} from "../../config/src/transform-config.compiler.js";
import { createConfigFixture } from "../fixtures/config.js";
import { compileConfigBaseline } from "../fixtures/transform-config.baseline.js";

const quick = process.argv.includes("--quick");
const outputArgument = process.argv.find((argument) => argument.startsWith("--output="));
const sizes = quick ? [1_000, 10_000] : [1_000, 10_000, 100_000, 1_000_000];
const result = {
  metadata: {
    timestamp: new Date().toISOString(),
    platform: `${process.platform}-${process.arch}`,
    node: process.version,
    quick,
    unit: "milliseconds",
  },
  reportGeneration: [],
  rustToJs: [],
  configInput: [],
  mutablePipeline: [],
};
let sink = 0;
const utf8Decoder = new TextDecoder();

function median(values) {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.floor(sorted.length / 2)];
}

function measure(fn, repetitions = 7) {
  for (let i = 0; i < 5; i++) sink ^= Number(fn()) || 0;
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
  return count >= 1_000_000 ? 5 : count >= 100_000 ? 5 : 7;
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

function materializeDirectNamesWithTextDecoder(report) {
  const result = new Array(report.length);
  let item = report.itemsOffset;
  for (let i = 0; i < result.length; i++, item += 24) {
    const offset = report.view.getUint32(item, true);
    const length = report.view.getUint32(item + 4, true);
    result[i] = utf8Decoder.decode(report.bytes.subarray(offset, offset + length));
  }
  return result;
}

console.log("Benchmark 1/3: Rust → JS result");
for (const count of sizes) {
  const repetitions = repetitionsFor(count);
  const row = { count };
  const generationRepetitions = count >= 100_000 ? 3 : repetitions;
  result.reportGeneration.push({
    count,
    compact: {
      sequential: measure(
        () => native.makeReportBufferCompactSequential(count).byteLength,
        generationRepetitions,
      ),
      parallelLocal: measure(
        () => native.makeReportBufferCompactParallel(count).byteLength,
        generationRepetitions,
      ),
      automatic: measure(
        () => native.makeReportBufferCompact(count).byteLength,
        generationRepetitions,
      ),
    },
    directStringRef: {
      sequential: measure(
        () => native.makeReportBufferDirectStringRefSequential(count).byteLength,
        generationRepetitions,
      ),
      parallelLocal: measure(
        () => native.makeReportBufferDirectStringRefParallel(count).byteLength,
        generationRepetitions,
      ),
    },
    soa: {
      sequential: measure(
        () => native.makeReportBufferSoaSequential(count).byteLength,
        generationRepetitions,
      ),
      parallelLocal: measure(
        () => native.makeReportBufferSoaParallel(count).byteLength,
        generationRepetitions,
      ),
    },
  });
  row.objectReturn = measure(() => native.makeReportObject(count).packages.length, repetitions);
  row.jsonReturnAndParse = measure(
    () => JSON.parse(native.makeReportJson(count)).packages.length,
    repetitions,
  );
  row.zctfReturnAndView = {
    mutable: measure(
      () => BenchReportView.from(native.makeReportBufferMutable(count)).packageCount,
      repetitions,
    ),
    compact: measure(
      () => BenchReportView.from(native.makeReportBufferCompact(count)).packageCount,
      repetitions,
    ),
  };

  const buffer = native.makeReportBufferMutable(count);
  const compactBuffer = native.makeReportBufferCompact(count);
  const view = BenchReportView.from(buffer);
  const compactView = BenchReportView.from(compactBuffer);
  const cachedView = BenchReportView.from(compactBuffer, { cacheStrings: true });
  const object = native.makeReportObject(count);
  const json = native.makeReportJson(count);
  row.storageBytes = {
    jsonUtf8: Buffer.byteLength(json),
    zctfMutable: buffer.byteLength,
    zctfCompact: compactBuffer.byteLength,
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
    zctfGetLoop: measure(() => {
      let sum = 0;
      const len = compactView.packages.length;
      for (let i = 0; i < len; i++) sum += compactView.packages.get(i).size;
      return sum;
    }, repetitions),
    zctfCursor: measure(() => {
      let sum = 0;
      const packages = compactView.packages;
      const cursor = packages.cursor();
      const len = packages.length;
      for (let i = 0; i < len; i++) sum += cursor.moveTo(i).size;
      return sum;
    }, repetitions),
    zctfRaw: measure(() => {
      let sum = 0;
      compactView.packages.forEachRaw((offset, _index, doc) => {
        sum += doc.view.getUint32(offset + 8, true);
      });
      return sum;
    }, repetitions),
    zctfBulk: measure(() => compactView.packages.sumSizes(), repetitions),
    zctfBulkMutable: measure(() => view.packages.sumSizes(), repetitions),
    zctfNative: measure(() => native.sumReportSizes(compactBuffer), repetitions),
    jsonParseAndLoop: measure(() => {
      let sum = 0;
      for (const item of JSON.parse(json).packages) sum += item.size;
      return sum;
    }, repetitions),
  };
  row.names = {
    objectLength: measure(() => {
      let length = 0;
      for (const item of object.packages) length += item.name.length;
      return length;
    }, repetitions),
    zctfGetDecodeLength: measure(() => {
      let length = 0;
      const len = compactView.packages.length;
      for (let i = 0; i < len; i++) {
        length += compactView.packages.get(i).name.length;
      }
      return length;
    }, repetitions),
    zctfBulkDecodeLength: measure(
      () => compactView.packages.sumNameDecodeLengths(),
      repetitions,
    ),
    zctfByteLength: measure(
      () => compactView.packages.sumNameByteLengths(),
      repetitions,
    ),
    zctfNativeByteLength: measure(
      () => native.sumReportNameByteLengths(compactBuffer),
      repetitions,
    ),
    zctfMaterializeArray: measure(
      () => compactView.packages.materializeNames().length,
      repetitions,
    ),
    objectPrefixFilter: measure(() => {
      let matches = 0;
      for (const item of object.packages) if (item.name.startsWith("package-9")) matches++;
      return matches;
    }, repetitions),
    zctfBytePrefixFilter: measure(
      () => compactView.packages.countNamesWithPrefix("package-9"),
      repetitions,
    ),
    zctfNativeBytePrefixFilter: measure(
      () => native.countReportNamePrefix(compactBuffer, "package-9"),
      repetitions,
    ),
    jsonParsePrefixFilter: measure(() => {
      let matches = 0;
      for (const item of JSON.parse(json).packages) {
        if (item.name.startsWith("package-9")) matches++;
      }
      return matches;
    }, repetitions),
  };
  row.cacheStrings = {
    disabled: measure(() => compactView.packages.sumNameDecodeLengths(), repetitions),
    enabledWarm: measure(() => cachedView.packages.sumNameDecodeLengths(), repetitions),
  };
  row.toObject = {
    legacy: measure(() => compactView.toObjectLegacy().packages.length, repetitions),
    optimized: measure(() => compactView.toObject().packages.length, repetitions),
  };
  const directBuffer = native.makeReportBufferDirectStringRef(count);
  const soaBuffer = native.makeReportBufferSoa(count);
  const sidecarBuffer = native.makeReportBufferSidecar(count);
  const direct = DirectStringRefReportView.from(directBuffer);
  const soa = SoAReportView.from(soaBuffer);
  const sidecar = SidecarReportView.from(sidecarBuffer);
  row.layoutVariants = {
    storageBytes: {
      aosMutable: buffer.byteLength,
      aosCompact: compactBuffer.byteLength,
      directStringRef: directBuffer.byteLength,
      soa: soaBuffer.byteLength,
      aosSidecar: sidecarBuffer.byteLength,
    },
    sumSizes: {
      aosCompact: row.sumSizes.zctfBulk,
      directStringRef: measure(() => direct.sumSizes(), repetitions),
      soa: measure(() => soa.sumSizes(), repetitions),
      aosSidecar: measure(() => sidecar.sumSizes(), repetitions),
    },
    nameByteLengths: {
      aosCompact: row.names.zctfByteLength,
      directStringRef: measure(() => direct.sumNameByteLengths(), repetitions),
      soa: measure(() => soa.sumNameByteLengths(), repetitions),
    },
    materializeNames: {
      aosCompact: row.names.zctfMaterializeArray,
      directStringRefTextDecoder: measure(
        () => materializeDirectNamesWithTextDecoder(direct).length,
        repetitions,
      ),
      directStringRef: measure(() => direct.materializeNames().length, repetitions),
    },
    nameGetterScan: {
      aosCompact: measure(() => {
        let length = 0;
        for (let i = 0; i < compactView.packages.length; i++) {
          length += compactView.packages.get(i).name.length;
        }
        return length;
      }, repetitions),
      directStringRef: measure(() => {
        let length = 0;
        for (let i = 0; i < direct.packages.length; i++) {
          length += direct.packages.get(i).name.length;
        }
        return length;
      }, repetitions),
    },
    materializeStrings: {
      aosCompact: measure(() => compactView.packages.materializeStrings().length, repetitions),
      directStringRef: measure(() => direct.materializeStrings().length, repetitions),
    },
  };
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
  const config = createConfigFixture(shape);
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
const output = outputArgument
  ? resolve(process.cwd(), outputArgument.slice("--output=".length))
  : resolve(import.meta.dirname, "../../../benchmark-results/napi.json");
const outputDir = resolve(output, "..");
mkdirSync(outputDir, { recursive: true });
writeFileSync(output, `${JSON.stringify(result, null, 2)}\n`);
console.log(`Results: ${output}`);
