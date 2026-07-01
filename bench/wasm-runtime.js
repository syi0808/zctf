import { readFile } from "node:fs/promises";

const wasmPath = new URL(
  "../target/wasm32-unknown-unknown/release/zctf_product_bench_wasm.wasm",
  import.meta.url,
);
const wasmBytes = await readFile(wasmPath);
const { instance } = await WebAssembly.instantiate(wasmBytes);
const wasm = instance.exports;
const encoder = new TextEncoder();

export function transformZctfWasm(source, warningCount) {
  const sourceBytes = encoder.encode(source);
  const inputPointer = wasm.zctf_alloc(sourceBytes.byteLength);
  if (sourceBytes.byteLength !== 0) {
    new Uint8Array(
      wasm.memory.buffer,
      inputPointer,
      sourceBytes.byteLength,
    ).set(sourceBytes);
  }

  let outputPointer;
  try {
    outputPointer = wasm.transform_zctf(
      inputPointer,
      sourceBytes.byteLength,
      warningCount,
    );
  } finally {
    wasm.zctf_free_input(inputPointer, sourceBytes.byteLength);
  }

  if (outputPointer === 0) {
    throw new Error("WASM zctf transform failed");
  }
  const header = new DataView(wasm.memory.buffer, outputPointer, 8);
  const allocationLength = header.getUint32(0, true);
  const documentLength = header.getUint32(4, true);
  if (allocationLength !== documentLength + 8) {
    wasm.zctf_free_output(outputPointer);
    throw new RangeError("invalid WASM zctf allocation header");
  }

  let freed = false;
  return {
    bytes: new Uint8Array(
      wasm.memory.buffer,
      outputPointer + 8,
      documentLength,
    ),
    free() {
      if (!freed) {
        freed = true;
        wasm.zctf_free_output(outputPointer);
      }
    },
  };
}
