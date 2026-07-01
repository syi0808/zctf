import { expect, test } from "vitest";
import { transformObject, transformZctf, transformZctfManual } from "../native.js";
import { TransformResultView } from "../generated/transform-result.view.js";
import { transformZctfWasm } from "../wasm-runtime.js";

test("napi object and generated zctf view expose equivalent values", () => {
  const object = transformObject("<svg/>", 3);
  const view = TransformResultView.from(transformZctf("<svg/>", 3));
  expect(view.toObject()).toEqual(object);
});

test("macro and manual writers produce equivalent documents", () => {
  const macro = TransformResultView.from(transformZctf("<svg/>", 20)).toObject();
  const manual = TransformResultView.from(transformZctfManual("<svg/>", 20)).toObject();
  expect(manual).toEqual(macro);
});

test("WASM and N-API expose equivalent generated zctf views", () => {
  const napi = TransformResultView.from(transformZctf("<svg/>", 20)).toObject();
  const wasm = transformZctfWasm("<svg/>", 20);
  try {
    expect(TransformResultView.from(wasm.bytes).toObject()).toEqual(napi);
  } finally {
    wasm.free();
  }
});
