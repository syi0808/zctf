import { BenchReportView } from "./bench-report.view.js";

const DIRECT_MAGIC = 0x4452_435a;
const SOA_MAGIC = 0x5341_435a;
const decoder = new TextDecoder();

function asBytes(bytes) {
  if (!(bytes instanceof Uint8Array)) throw new TypeError("Uint8Array required");
  return bytes;
}

function uint32Column(bytes, offset, length) {
  const absoluteOffset = bytes.byteOffset + offset;
  if (absoluteOffset % 4 === 0) {
    return new Uint32Array(bytes.buffer, absoluteOffset, length);
  }
  const view = new DataView(bytes.buffer, absoluteOffset, length * 4);
  return Object.freeze({
    length,
    at(index) {
      if (!Number.isInteger(index) || index < 0 || index >= length) {
        throw new RangeError("column index out of bounds");
      }
      return view.getUint32(index * 4, true);
    },
    sum() {
      let total = 0;
      for (let i = 0; i < length; i++) total += view.getUint32(i * 4, true);
      return total;
    },
  });
}

function sumColumn(column) {
  if (column instanceof Uint32Array) {
    let total = 0;
    for (let i = 0; i < column.length; i++) total += column[i];
    return total;
  }
  return column.sum();
}

export class DirectStringRefReportView {
  static from(input) {
    const bytes = asBytes(input);
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    if (view.getUint32(0, true) !== DIRECT_MAGIC) throw new TypeError("invalid direct-ref report");
    if (view.getUint32(16, true) !== bytes.byteLength) throw new RangeError("invalid report size");
    return new DirectStringRefReportView(bytes, view);
  }

  constructor(bytes, view) {
    this.bytes = bytes;
    this.view = view;
    this.length = view.getUint32(4, true);
    this.itemsOffset = view.getUint32(8, true);
  }

  sumSizes() {
    let total = 0;
    let item = this.itemsOffset + 16;
    for (let i = 0; i < this.length; i++, item += 24) {
      total += this.view.getUint32(item, true);
    }
    return total;
  }

  sumNameByteLengths() {
    let total = 0;
    let item = this.itemsOffset + 4;
    for (let i = 0; i < this.length; i++, item += 24) {
      total += this.view.getUint32(item, true);
    }
    return total;
  }

  materializeNames() {
    const result = new Array(this.length);
    let item = this.itemsOffset;
    for (let i = 0; i < result.length; i++, item += 24) {
      const offset = this.view.getUint32(item, true);
      const length = this.view.getUint32(item + 4, true);
      result[i] = decoder.decode(this.bytes.subarray(offset, offset + length));
    }
    return result;
  }
}

export class SoAReportView {
  static from(input) {
    const bytes = asBytes(input);
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    if (view.getUint32(0, true) !== SOA_MAGIC) throw new TypeError("invalid SoA report");
    if (view.getUint32(24, true) !== bytes.byteLength) throw new RangeError("invalid report size");
    return new SoAReportView(bytes, view);
  }

  constructor(bytes, view) {
    this.bytes = bytes;
    this.view = view;
    this.length = view.getUint32(4, true);
    this.namesOffset = view.getUint32(8, true);
    this.columns = Object.freeze({
      size: uint32Column(bytes, view.getUint32(12, true), this.length),
      dependencyCount: uint32Column(bytes, view.getUint32(16, true), this.length),
    });
  }

  sumSizes() {
    return sumColumn(this.columns.size);
  }

  sumNameByteLengths() {
    let total = 0;
    let entry = this.namesOffset + 4;
    for (let i = 0; i < this.length; i++, entry += 8) {
      total += this.view.getUint32(entry, true);
    }
    return total;
  }
}

export class SidecarReportView {
  static from(bytes) {
    const report = BenchReportView.from(bytes);
    const length = report.packages.length;
    return new SidecarReportView(
      report,
      Object.freeze({
        size: uint32Column(bytes, report.doc.u32(8), length),
        dependencyCount: uint32Column(bytes, report.doc.u32(12), length),
      }),
    );
  }

  constructor(report, columns) {
    this.report = report;
    this.columns = columns;
  }

  sumSizes() {
    return sumColumn(this.columns.size);
  }
}
