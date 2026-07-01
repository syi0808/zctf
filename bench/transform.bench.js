import { bench, describe } from "vitest";
import {
  transformObject as napiObject,
  transformZctf as zctfMacro,
  transformZctfManual as zctfManual,
} from "./native.js";
import { TransformResultView } from "./generated/transform-result.view.js";

const source = "<svg viewBox='0 0 24 24'><path d='M0 0h24v24H0z'/></svg>";
const quick = process.env.ZCTF_BENCH_QUICK === "1";
function optionsFor(warningCount) {
  if (quick) {
    return {
      time: 100,
      iterations: 20,
      warmupTime: 50,
      warmupIterations: 10,
    };
  }
  if (warningCount >= 1_000) {
    return {
      time: 2_500,
      iterations: 100,
      warmupTime: 500,
      warmupIterations: 20,
    };
  }
  return {
    time: 750,
    iterations: 100,
    warmupTime: 250,
    warmupIterations: 20,
  };
}

const warningCounts = quick ? [0, 20, 1_000] : [0, 3, 20, 100, 1_000, 10_000];

for (const warningCount of warningCounts) {
  const prebuilt = zctfMacro(source, warningCount);
  const options = optionsFor(warningCount);

  describe(`${warningCount} warnings - end to end`, () => {
    bench(
      "napi[object] return",
      () => {
        const value = napiObject(source, warningCount);
        return value.code.length + value.warnings.length;
      },
      options,
    );

    bench(
      "napi + zctf macro/view",
      () => {
        const value = TransformResultView.from(zctfMacro(source, warningCount));
        return value.code.length + value.warnings.length;
      },
      options,
    );

    bench(
      "napi + zctf manual/view",
      () => {
        const value = TransformResultView.from(zctfManual(source, warningCount));
        return value.code.length + value.warnings.length;
      },
      options,
    );

    bench(
      "napi[object] full traversal",
      () => {
        const value = napiObject(source, warningCount);
        return (
          value.code.length +
          value.warnings.reduce((sum, warning) => sum + warning.message.length, 0)
        );
      },
      options,
    );

    bench(
      "napi + zctf toObject",
      () => {
        const value = TransformResultView.from(
          zctfMacro(source, warningCount),
        ).toObject();
        return (
          value.code.length +
          value.warnings.reduce((sum, warning) => sum + warning.message.length, 0)
        );
      },
      options,
    );
  });

  describe(`${warningCount} warnings - stage breakdown`, () => {
    bench(
      "napi + zctf Buffer return only",
      () => zctfMacro(source, warningCount).byteLength,
      options,
    );

    bench(
      "zctf View.from prebuilt Buffer",
      () => TransformResultView.from(prebuilt).offset,
      options,
    );
  });
}
