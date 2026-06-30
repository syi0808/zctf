export class FixedListView {
  constructor(document, offset, stride, createItem) {
    this.document = document;
    this.offset = offset;
    this.stride = stride;
    this.createItem = createItem;
    this.itemsOffset = document.u32(offset + 12);
    if (document.u32(offset + 8) !== stride) throw new TypeError("unexpected fixed-list stride");
    if (this.length > this.capacity) throw new RangeError("fixed-list length exceeds capacity");
    document.slice(this.itemsOffset, this.capacity * stride);
  }

  get length() {
    return this.document.u32(this.offset);
  }

  get capacity() {
    return this.document.u32(this.offset + 4);
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

  commitPush(writeItem) {
    const length = this.length;
    const offset = this.itemOffset(length, true);
    writeItem(offset);
    this.document.setU32(this.offset, length + 1);
    return this.createItem(this.document, offset, length);
  }
}
