import { bench, describe } from "vitest";
import {
  transformObject as napiObject,
  transformZctf as zctfMacro,
  transformZctfManual as zctfManual,
} from "./native.js";
import { TransformResultView } from "./generated/transform-result.view.js";

const source = "<svg viewBox='0 0 24 24'><path d='M0 0h24v24H0z'/></svg>";
const quick = process.env.ZCTF_BENCH_QUICK === "1";
const options = quick
  ? {
      time: 100,
      iterations: 20,
      warmupTime: 50,
      warmupIterations: 10,
    }
  : {
      time: 750,
      iterations: 100,
      warmupTime: 250,
      warmupIterations: 20,
    };

for (const warningCount of [0, 3, 20]) {
  describe(`${warningCount} warnings`, () => {
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
}
