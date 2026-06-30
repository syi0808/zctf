import { MAGIC_REPORT, MemoryReader } from "./memory.js";

const decoder = new TextDecoder();
const encoder = new TextEncoder();

export class ZctfDocument extends MemoryReader {
  constructor(bytes, { cacheStrings = false } = {}) {
    super(bytes);
    if (this.u32(0) !== MAGIC_REPORT) throw new TypeError("invalid zctf report magic");
    this.stringTableOffset = this.u32(32);
    this.stringHeapOffset = this.u32(36);
    this.cache = cacheStrings ? new Map() : null;
  }

  string(id) {
    const cached = this.cache?.get(id);
    if (cached !== undefined) return cached;
    const entry = this.stringTableOffset + id * 8;
    const offset = this.stringHeapOffset + this.u32(entry);
    const length = this.u32(entry + 4);
    const value = decoder.decode(this.bytes.subarray(offset, offset + length));
    this.cache?.set(id, value);
    return value;
  }

  allocString(value) {
    const encoded = encoder.encode(value);
    const id = this.u32(52);
    const capacity = this.u32(56);
    let cursor = this.u32(40);
    const heapEnd = this.stringHeapOffset + this.u32(44);
    if (id >= capacity) throw new RangeError("zctf string table capacity exceeded");
    if (cursor + encoded.length > heapEnd) throw new RangeError("zctf string heap capacity exceeded");
    this.bytes.set(encoded, cursor);
    const entry = this.stringTableOffset + id * 8;
    this.setU32(entry, cursor - this.stringHeapOffset);
    this.setU32(entry + 4, encoded.length);
    cursor += encoded.length;
    this.setU32(40, cursor);
    this.setU32(52, id + 1);
    this.cache?.set(id, value);
    return id;
  }
}

