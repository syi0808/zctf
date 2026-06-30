export const MAGIC_REPORT = 0x4654435a;
export const MAGIC_CONFIG = 0x4346435a;
export const HEADER_SIZE = 64;
export const ROOT_OFFSET = 64;
export const PACKAGE_SIZE = 16;

export class MemoryReader {
  constructor(bytes) {
    this.bytes = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes);
    this.view = new DataView(this.bytes.buffer, this.bytes.byteOffset, this.bytes.byteLength);
  }

  u8(offset) {
    return this.view.getUint8(offset);
  }

  u32(offset) {
    return this.view.getUint32(offset, true);
  }

  u64(offset) {
    return this.view.getBigUint64(offset, true);
  }

  f64(offset) {
    return this.view.getFloat64(offset, true);
  }

  setU32(offset, value) {
    this.view.setUint32(offset, value, true);
  }

  setU8(offset, value) {
    this.view.setUint8(offset, value);
  }
}
