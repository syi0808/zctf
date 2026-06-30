export class FixedListView {
  constructor(document, offset, stride, createItem) {
    this.document = document;
    this.offset = offset;
    this.stride = stride;
    this.createItem = createItem;
    this._length = document.u32(offset);
    this._capacity = document.u32(offset + 4);
    this.itemsOffset = document.u32(offset + 12);
    if (document.u32(offset + 8) !== stride) throw new TypeError("unexpected fixed-list stride");
    if (this._length > this._capacity) throw new RangeError("fixed-list length exceeds capacity");
    document.slice(this.itemsOffset, this._capacity * stride);
  }

  get length() {
    return this._length;
  }

  get capacity() {
    return this._capacity;
  }

  itemOffset(index, allowEnd = false) {
    const limit = allowEnd ? this.capacity : this.length;
    if (!Number.isInteger(index) || index < 0 || index >= limit) {
      throw new RangeError("fixed-list index out of bounds");
    }
    return this.itemsOffset + index * this.stride;
  }

  get(index) {
    return this.createItem(this.document, this.itemOffset(index), index);
  }

  forEachRaw(callback) {
    if (typeof callback !== "function") throw new TypeError("callback must be a function");
    const len = this._length;
    let offset = this.itemsOffset;
    for (let index = 0; index < len; index++) {
      callback(offset, index, this.document);
      offset += this.stride;
    }
  }

  commitPush(writeItem) {
    const length = this._length;
    const offset = this.itemOffset(length, true);
    writeItem(offset);
    this._length = length + 1;
    this.document.setU32(this.offset, this._length);
    return this.createItem(this.document, offset, length);
  }
}
