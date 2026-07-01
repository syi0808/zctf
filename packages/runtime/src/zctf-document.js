import { MemoryReader } from "./memory.js";

const MAGIC = 0x4654435a;
const HEADER_SIZE = 36;
const decoder = new TextDecoder("utf-8", { ignoreBOM: true });

export class ZctfDocument extends MemoryReader {
  static from(bytes, options) {
    return new ZctfDocument(bytes, options);
  }

  constructor(bytes, { schemaId, layoutVersion = 1 } = {}) {
    super(bytes);
    if (this.bytes.byteLength < HEADER_SIZE) throw new RangeError("zctf document is shorter than its header");
    if (this.u32(0) !== MAGIC) throw new TypeError("invalid zctf document magic");
    if (this.u16(4) !== 1) throw new TypeError(`unsupported zctf format version: ${this.u16(4)}`);
    if (this.u8(6) !== 1) throw new TypeError("only little-endian zctf documents are supported");
    this.schemaId = this.u64(8);
    if (schemaId !== undefined && this.schemaId !== BigInt(schemaId)) {
      throw new TypeError(`zctf schema mismatch: expected ${BigInt(schemaId)}, received ${this.schemaId}`);
    }
    this.layoutVersion = this.u32(16);
    if (layoutVersion !== undefined && this.layoutVersion !== layoutVersion) {
      throw new TypeError(`zctf layout version mismatch: expected ${layoutVersion}, received ${this.layoutVersion}`);
    }
    this.rootOffset = this.u32(20);
    this.stringTableOffset = this.u32(24);
    this.stringHeapOffset = this.u32(28);
    this.documentLength = this.u32(32);
    if (
      this.documentLength !== this.bytes.byteLength ||
      this.rootOffset < HEADER_SIZE ||
      this.rootOffset > this.stringTableOffset ||
      this.stringTableOffset > this.stringHeapOffset ||
      this.stringHeapOffset > this.documentLength ||
      (this.stringHeapOffset - this.stringTableOffset) % 8 !== 0
    ) {
      throw new RangeError("invalid zctf document regions");
    }
    this.stringCount = (this.stringHeapOffset - this.stringTableOffset) / 8;
    this.decodeUtf8 =
      typeof Buffer !== "undefined" && Buffer.isBuffer(this.bytes)
        ? (start, length) => this.bytes.toString("utf8", start, start + length)
        : (start, length) => decoder.decode(this.bytes.subarray(start, start + length));
    for (let id = 0; id < this.stringCount; id++) this.stringRange(id);
  }

  stringRange(id) {
    if (!Number.isInteger(id) || id < 0 || id >= this.stringCount) {
      throw new RangeError("zctf string id out of bounds");
    }
    const entry = this.stringTableOffset + id * 8;
    const start = this.stringHeapOffset + this.u32(entry);
    const length = this.u32(entry + 4);
    this.slice(start, length);
    return [start, length];
  }

  string(id) {
    const [start, length] = this.stringRange(id);
    return this.decodeUtf8(start, length);
  }
}

export class ZctfFixedListView {
  constructor(document, offset, stride, createItem) {
    this.document = document;
    this.offset = offset;
    this.stride = stride;
    this.createItem = createItem;
    this._length = document.u32(offset);
    if (document.u32(offset + 4) !== stride) throw new TypeError("unexpected zctf list stride");
    this.itemsOffset = document.u32(offset + 8);
    document.slice(this.itemsOffset, this._length * stride);
  }

  get length() { return this._length; }

  get(index) {
    if (!Number.isInteger(index) || index < 0 || index >= this._length) {
      throw new RangeError("zctf list index out of bounds");
    }
    return this.createItem(this.document, this.itemsOffset + index * this.stride, index);
  }

  at(index) {
    if (!Number.isInteger(index)) return undefined;
    if (index < 0) index += this._length;
    return index < 0 || index >= this._length ? undefined : this.get(index);
  }

  forEach(callback) {
    for (let index = 0; index < this._length; index++) callback(this.get(index), index, this);
  }

  map(callback) {
    const output = new Array(this._length);
    for (let index = 0; index < this._length; index++) output[index] = callback(this.get(index), index, this);
    return output;
  }

  forEachRaw(callback) {
    let offset = this.itemsOffset;
    for (let index = 0; index < this._length; index++, offset += this.stride) {
      callback(offset, index, this.document);
    }
  }

  toArray() { return this.map((item) => item.toObject()); }

  *[Symbol.iterator]() {
    for (let index = 0; index < this._length; index++) yield this.get(index);
  }
}
