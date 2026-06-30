import assert from "node:assert/strict";
import test from "node:test";
import { BinaryDocument, FixedListView } from "../src/index.js";

const FORMAT = Object.freeze({
  magic: 0x1234_5678,
  versions: [1],
  minimumSize: 64,
  strings: {
    tableOffsetField: 32,
    heapOffsetField: 36,
    heapCursorField: 40,
    heapCapacityField: 44,
    countField: 52,
    capacityField: 56,
  },
});

function makeDocumentBytes() {
  const bytes = new Uint8Array(128);
  const view = new DataView(bytes.buffer);
  const u32 = (offset, value) => view.setUint32(offset, value, true);
  u32(0, FORMAT.magic);
  u32(4, 1);
  u32(32, 64);
  u32(36, 80);
  u32(40, 82);
  u32(44, 16);
  u32(52, 1);
  u32(56, 2);
  u32(64, 0);
  u32(68, 2);
  bytes.set(new TextEncoder().encode("hi"), 80);
  u32(96, 1);
  u32(100, 2);
  u32(104, 4);
  u32(108, 112);
  u32(112, 42);
  return bytes;
}

test("generic document validates and accesses descriptor-defined strings", () => {
  const document = new BinaryDocument(makeDocumentBytes(), FORMAT, { cacheStrings: true });
  assert.equal(document.string(0), "hi");
  assert.equal(document.allocString("ok"), 1);
  assert.equal(document.string(1), "ok");
  assert.throws(() => document.string(2), /out of bounds/);
});

test("generic fixed list validates stride, capacity, and item bounds", () => {
  const document = new BinaryDocument(makeDocumentBytes(), FORMAT);
  const list = new FixedListView(document, 96, 4, (doc, offset) => ({
    get value() {
      return doc.u32(offset);
    },
  }));
  assert.equal(list.get(0).value, 42);
  const pushed = list.commitPush((offset) => document.setU32(offset, 7));
  assert.equal(pushed.value, 7);
  assert.equal(list.length, 2);
  assert.throws(() => list.commitPush(() => {}), /out of bounds/);
});

test("generic document rejects incompatible or malformed inputs", () => {
  const bytes = makeDocumentBytes();
  bytes[0] = 0;
  assert.throws(() => new BinaryDocument(bytes, FORMAT), /magic/);
  assert.throws(() => new BinaryDocument(new Uint8Array(4), FORMAT), /shorter/);
});
