import { MemoryReader } from "./memory.js";

const decoder = new TextDecoder();
const encoder = new TextEncoder();

export class BinaryDocument extends MemoryReader {
  constructor(bytes, format, { cacheStrings = false } = {}) {
    super(bytes);
    if (!format || !Number.isInteger(format.magic)) {
      throw new TypeError("a format descriptor with a magic number is required");
    }
    if (this.bytes.byteLength < (format.minimumSize ?? 8)) {
      throw new RangeError("document is shorter than the format minimum");
    }
    if (this.u32(0) !== format.magic) throw new TypeError("invalid document magic");
    this.version = this.u32(4);
    if (format.versions && !format.versions.includes(this.version)) {
      throw new TypeError(`unsupported document version: ${this.version}`);
    }
    if (format.totalLengthOffset !== undefined) {
      const total = this.u32(format.totalLengthOffset);
      if (total < (format.minimumSize ?? 8) || total > this.bytes.byteLength) {
        throw new RangeError("invalid document total length");
      }
      if (total !== this.bytes.byteLength) {
        this.bytes = this.bytes.subarray(0, total);
        this.view = new DataView(this.bytes.buffer, this.bytes.byteOffset, total);
      }
    }
    this.cache = cacheStrings ? new Map() : null;
    this.strings = format.strings ? new MutableStringTable(this, format.strings) : null;
  }

  string(id) {
    if (!this.strings) throw new TypeError("format does not define a string table");
    return this.strings.get(id);
  }

  allocString(value) {
    if (!this.strings) throw new TypeError("format does not define a mutable string table");
    return this.strings.allocate(value);
  }
}

export class MutableStringTable {
  constructor(document, layout) {
    this.document = document;
    this.layout = layout;
    this.tableOffset = document.u32(layout.tableOffsetField);
    this.heapOffset = document.u32(layout.heapOffsetField);
    this.validate();
  }

  validate() {
    const count = this.document.u32(this.layout.countField);
    const capacity = this.document.u32(this.layout.capacityField);
    if (count > capacity) throw new RangeError("string count exceeds capacity");
    const tableEnd = this.tableOffset + capacity * 8;
    const heapEnd = this.heapOffset + this.document.u32(this.layout.heapCapacityField);
    if (tableEnd > this.heapOffset || heapEnd > this.document.bytes.byteLength) {
      throw new RangeError("invalid string table regions");
    }
  }

  range(id) {
    const count = this.document.u32(this.layout.countField);
    if (!Number.isInteger(id) || id < 0 || id >= count) {
      throw new RangeError("string id out of bounds");
    }
    const entry = this.tableOffset + id * 8;
    const offset = this.heapOffset + this.document.u32(entry);
    const length = this.document.u32(entry + 4);
    this.document.slice(offset, length);
    return [offset, length];
  }

  rangeUnchecked(id) {
    const entry = this.tableOffset + id * 8;
    const offset = this.heapOffset + this.document.view.getUint32(entry, true);
    const length = this.document.view.getUint32(entry + 4, true);
    return [offset, length];
  }

  get(id) {
    const cached = this.document.cache?.get(id);
    if (cached !== undefined) return cached;
    const [offset, length] = this.range(id);
    const value = decoder.decode(this.document.slice(offset, length));
    this.document.cache?.set(id, value);
    return value;
  }

  getUnchecked(id) {
    const cached = this.document.cache?.get(id);
    if (cached !== undefined) return cached;
    const [offset, length] = this.rangeUnchecked(id);
    const value = decoder.decode(
      this.document.bytes.subarray(offset, offset + length),
    );
    this.document.cache?.set(id, value);
    return value;
  }

  byteLengthUnchecked(id) {
    return this.document.view.getUint32(this.tableOffset + id * 8 + 4, true);
  }

  allocate(value) {
    if (typeof value !== "string") throw new TypeError("string value required");
    const encoded = encoder.encode(value);
    const id = this.document.u32(this.layout.countField);
    const capacity = this.document.u32(this.layout.capacityField);
    let cursor = this.document.u32(this.layout.heapCursorField);
    const heapEnd = this.heapOffset + this.document.u32(this.layout.heapCapacityField);
    if (id >= capacity) throw new RangeError("zctf string table capacity exceeded");
    if (cursor + encoded.length > heapEnd) throw new RangeError("zctf string heap capacity exceeded");
    this.document.bytes.set(encoded, cursor);
    const entry = this.tableOffset + id * 8;
    this.document.setU32(entry, cursor - this.heapOffset);
    this.document.setU32(entry + 4, encoded.length);
    cursor += encoded.length;
    this.document.setU32(this.layout.heapCursorField, cursor);
    this.document.setU32(this.layout.countField, id + 1);
    this.document.cache?.set(id, value);
    return id;
  }
}
