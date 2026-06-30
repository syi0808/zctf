import assert from "node:assert/strict";
import test from "node:test";
import native from "../../bench/src/native.js";
import {
  ConfigWriter,
  compileConfig,
  compileConfigInto,
  estimateTransformConfigSize,
  withCompiledConfig,
} from "../../config/src/transform-config.compiler.js";
import { createConfigFixture } from "../../bench/fixtures/config.js";
import { BenchReportView } from "../../bench/fixtures/bench-report.view.js";

test("report view reads, mutates, and materializes native bytes", () => {
  const bytes = native.makeReportBuffer(10);
  const report = BenchReportView.from(bytes);
  assert.equal(report.packageCount, 10);
  assert.equal(report.packages.get(3).name, "package-3");
  report.packages.get(3).size = 99;
  report.packages.get(3).name = "renamed";
  assert.equal(report.packages.get(3).size, 99);
  assert.equal(report.packages.get(3).name, "renamed");
  report.packages.push({ name: "new-package", version: "2.0.0", size: 7 });
  assert.equal(report.packages.length, 11);
  assert.equal(report.packages.get(10).name, "new-package");
  assert.equal(report.toObject().packages.length, 11);
  assert.ok(native.consumeReportBuffer(bytes) > 0);
});

test("compiled config is consumed and cached by native code", () => {
  const config = createConfigFixture("medium");
  const compiled = compileConfig(config);
  assert.ok(native.consumeConfigBuffer(compiled) !== 0);
  const handle = native.ConfigHandle.create(compiled);
  assert.ok(handle.transform("<svg/>") !== 0);
  handle.dispose();
  assert.throws(() => handle.transform("<svg/>"), /disposed/);
});

test("specialized config compiler covers defaults, known names, ASCII, and UTF-8", () => {
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
    const capacity = estimateTransformConfigSize(config);
    const writer = new ConfigWriter(capacity);
    const compiled = compileConfigInto(writer, config);
    assert.ok(compiled.byteLength <= capacity);
    assert.ok(native.consumeConfigBuffer(compiled) !== 0 || shape === "defaultHeavy");
    if (shape === "pluginHeavy") assert.ok(writer.knownNames > 0);
    if (shape === "unicodeHeavy") assert.ok(writer.utf8Strings > 0);
    if (shape === "stringHeavy") assert.ok(writer.asciiStrings > 0);
  }
  assert.ok(
    compileConfig(createConfigFixture("pluginHeavy")).byteLength <
      compileConfig(createConfigFixture("large")).byteLength,
  );
  const defaults = compileConfig(createConfigFixture("defaultHeavy"));
  const defaultsView = new DataView(
    defaults.buffer,
    defaults.byteOffset,
    defaults.byteLength,
  );
  assert.equal(defaultsView.getUint32(32, true), 0);
  assert.equal(defaultsView.getUint32(36, true), 0);
  assert.equal(defaultsView.getUint8(56), 0);
  assert.equal(defaultsView.getUint8(57), 0);
  const known = compileConfig(createConfigFixture("pluginHeavy"));
  const knownView = new DataView(known.buffer, known.byteOffset, known.byteLength);
  assert.equal(knownView.getUint32(24, true), 0);
});

test("plain-object fast path rejects dynamic class instances", () => {
  class Config {}
  assert.throws(() => compileConfig(new Config()), /plain object/);
});

test("sync temp writer does not escape and supports nested calls", () => {
  const config = createConfigFixture("medium");
  const value = withCompiledConfig(config, (bytes) => {
    const outer = native.consumeConfigBuffer(bytes);
    const inner = withCompiledConfig(config, (nested) => native.consumeConfigBuffer(nested));
    return [outer, inner];
  });
  assert.deepEqual(value[0], value[1]);
  assert.throws(() => withCompiledConfig(config, async () => 1), /async boundary/);
  assert.throws(
    () => withCompiledConfig(config, (bytes) => bytes),
    /cannot escape/,
  );
});
