export class MemoryReader {
  constructor(bytes) {
    this.bytes = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes);
    this.view = new DataView(this.bytes.buffer, this.bytes.byteOffset, this.bytes.byteLength);
  }

  u8(offset) {
    return this.view.getUint8(offset);
  }

  i8(offset) {
    return this.view.getInt8(offset);
  }

  u16(offset) {
    return this.view.getUint16(offset, true);
  }

  i16(offset) {
    return this.view.getInt16(offset, true);
  }

  u32(offset) {
    return this.view.getUint32(offset, true);
  }

  i32(offset) {
    return this.view.getInt32(offset, true);
  }

  u64(offset) {
    return this.view.getBigUint64(offset, true);
  }

  i64(offset) {
    return this.view.getBigInt64(offset, true);
  }

  f32(offset) {
    return this.view.getFloat32(offset, true);
  }

  f64(offset) {
    return this.view.getFloat64(offset, true);
  }

  slice(offset, length) {
    if (!Number.isSafeInteger(offset) || !Number.isSafeInteger(length) || offset < 0 || length < 0) {
      throw new RangeError("offset and length must be non-negative safe integers");
    }
    const end = offset + length;
    if (end > this.bytes.byteLength) throw new RangeError("memory range out of bounds");
    return this.bytes.subarray(offset, end);
  }

  setU32(offset, value) {
    this.view.setUint32(offset, value, true);
  }

  setU8(offset, value) {
    this.view.setUint8(offset, value);
  }

  setU64(offset, value) {
    this.view.setBigUint64(offset, BigInt(value), true);
  }

  setF64(offset, value) {
    this.view.setFloat64(offset, value, true);
  }
}
