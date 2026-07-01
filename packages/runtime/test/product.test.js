import assert from "node:assert/strict";
import test from "node:test";
import { ZctfDocument, ZctfFixedListView } from "../src/index.js";

test("ZctfDocument validates and reads product strings", () => {
  const bytes = Buffer.alloc(57);
  bytes.write("ZCTF");
  bytes.writeUInt16LE(1, 4);
  bytes[6] = 1;
  bytes.writeBigUInt64LE(7n, 8);
  bytes.writeUInt32LE(1, 16);
  bytes.writeUInt32LE(40, 20);
  bytes.writeUInt32LE(44, 24);
  bytes.writeUInt32LE(52, 28);
  bytes.writeUInt32LE(57, 32);
  bytes.writeUInt32LE(0, 40);
  bytes.writeUInt32LE(0, 44);
  bytes.writeUInt32LE(5, 48);
  bytes.write("hello", 52);
  const document = ZctfDocument.from(bytes, { schemaId: 7n, layoutVersion: 1 });
  assert.equal(document.string(document.u32(document.rootOffset)), "hello");
});

test("ZctfDocument validates string entries lazily", () => {
  const bytes = Buffer.alloc(57);
  bytes.write("ZCTF");
  bytes.writeUInt16LE(1, 4);
  bytes[6] = 1;
  bytes.writeBigUInt64LE(7n, 8);
  bytes.writeUInt32LE(1, 16);
  bytes.writeUInt32LE(40, 20);
  bytes.writeUInt32LE(44, 24);
  bytes.writeUInt32LE(52, 28);
  bytes.writeUInt32LE(57, 32);
  bytes.writeUInt32LE(100, 44);
  bytes.writeUInt32LE(5, 48);
  const document = ZctfDocument.from(bytes, { schemaId: 7n });
  assert.throws(() => document.string(0), RangeError);
});

test("ZctfDocument reads direct offset/length strings", () => {
  const bytes = Buffer.alloc(57);
  bytes.write("ZCTF");
  bytes.writeUInt16LE(1, 4);
  bytes[6] = 1;
  bytes.writeBigUInt64LE(7n, 8);
  bytes.writeUInt32LE(1, 16);
  bytes.writeUInt32LE(40, 20);
  bytes.writeUInt32LE(52, 24);
  bytes.writeUInt32LE(52, 28);
  bytes.writeUInt32LE(57, 32);
  bytes.writeUInt32LE(52, 40);
  bytes.writeUInt32LE(5, 44);
  bytes.write("hello", 52);
  const document = ZctfDocument.from(bytes, { schemaId: 7n });
  assert.equal(document.directString(40), "hello");
});

test("ZctfFixedListView supports iteration helpers", () => {
  const bytes = new Uint8Array(80);
  const view = new DataView(bytes.buffer);
  view.setUint32(40, 2, true);
  view.setUint32(44, 4, true);
  view.setUint32(48, 64, true);
  view.setUint32(64, 10, true);
  view.setUint32(68, 20, true);
  const document = { u32: (offset) => view.getUint32(offset, true), slice: (offset, length) => bytes.subarray(offset, offset + length) };
  const list = new ZctfFixedListView(document, 40, 4, (doc, offset) => ({ value: doc.u32(offset), toObject() { return this.value; } }));
  assert.deepEqual(list.map((item) => item.value), [10, 20]);
  assert.deepEqual(list.toArray(), [10, 20]);
});
