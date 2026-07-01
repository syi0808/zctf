import { BenchReportView } from "./bench-report.view.js";
import { Buffer } from "node:buffer";

const DIRECT_MAGIC = 0x4452_435a;
const SOA_MAGIC = 0x5341_435a;
const DIRECT_HEADER_SIZE = 32;
const DIRECT_RECORD_SIZE = 24;

function asBytes(bytes) {
  if (!(bytes instanceof Uint8Array)) throw new TypeError("Uint8Array required");
  return bytes;
}

function asBuffer(bytes) {
  return Buffer.isBuffer(bytes)
    ? bytes
    : Buffer.from(bytes.buffer, bytes.byteOffset, bytes.byteLength);
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
    const length = view.getUint32(4, true);
    const itemsOffset = view.getUint32(8, true);
    const heapOffset = view.getUint32(12, true);
    if (
      itemsOffset < DIRECT_HEADER_SIZE ||
      itemsOffset + length * DIRECT_RECORD_SIZE !== heapOffset ||
      heapOffset > bytes.byteLength
    ) {
      throw new RangeError("invalid direct-ref layout");
    }
    return new DirectStringRefReportView(asBuffer(bytes), view, length, itemsOffset);
  }

  constructor(buffer, view, length, itemsOffset) {
    this.buffer = buffer;
    this.bytes = buffer;
    this.view = view;
    this.length = length;
    this.itemsOffset = itemsOffset;
    this.packages = new DirectStringRefPackageListView(this);
  }

  sumSizes() {
    return this.packages.sumSizes();
  }

  sumNameByteLengths() {
    return this.packages.sumNameByteLengths();
  }

  materializeNames() {
    return this.packages.materializeNames();
  }

  materializeStrings() {
    return this.packages.materializeStrings();
  }
}

export class DirectStringRefPackageInfoView {
  constructor(report, offset) {
    this.report = report;
    this.offset = offset;
  }

  get name() {
    const offset = this.report.view.getUint32(this.offset, true);
    const length = this.report.view.getUint32(this.offset + 4, true);
    return this.report.buffer.toString("latin1", offset, offset + length);
  }

  get version() {
    const offset = this.report.view.getUint32(this.offset + 8, true);
    const length = this.report.view.getUint32(this.offset + 12, true);
    return this.report.buffer.toString("latin1", offset, offset + length);
  }

  get size() {
    return this.report.view.getUint32(this.offset + 16, true);
  }

  get dependencyCount() {
    return this.report.view.getUint32(this.offset + 20, true);
  }
}

export class DirectStringRefPackageListView {
  constructor(report) {
    this.report = report;
    this.length = report.length;
  }

  get(index) {
    if (!Number.isInteger(index) || index < 0 || index >= this.length) {
      throw new RangeError("package index out of bounds");
    }
    return new DirectStringRefPackageInfoView(
      this.report,
      this.report.itemsOffset + index * DIRECT_RECORD_SIZE,
    );
  }

  sumSizes() {
    let total = 0;
    let item = this.report.itemsOffset + 16;
    for (let i = 0; i < this.length; i++, item += DIRECT_RECORD_SIZE) {
      total += this.report.view.getUint32(item, true);
    }
    return total;
  }

  sumNameByteLengths() {
    let total = 0;
    let item = this.report.itemsOffset + 4;
    for (let i = 0; i < this.length; i++, item += DIRECT_RECORD_SIZE) {
      total += this.report.view.getUint32(item, true);
    }
    return total;
  }

  materializeNames() {
    const result = new Array(this.length);
    let item = this.report.itemsOffset;
    for (let i = 0; i < result.length; i++, item += DIRECT_RECORD_SIZE) {
      const offset = this.report.view.getUint32(item, true);
      const length = this.report.view.getUint32(item + 4, true);
      result[i] = this.report.buffer.toString("latin1", offset, offset + length);
    }
    return result;
  }

  materializeStrings() {
    const result = new Array(this.length * 2);
    let item = this.report.itemsOffset;
    for (let i = 0; i < this.length; i++, item += DIRECT_RECORD_SIZE) {
      const nameOffset = this.report.view.getUint32(item, true);
      const nameLength = this.report.view.getUint32(item + 4, true);
      const versionOffset = this.report.view.getUint32(item + 8, true);
      const versionLength = this.report.view.getUint32(item + 12, true);
      result[i * 2] = this.report.buffer.toString(
        "latin1",
        nameOffset,
        nameOffset + nameLength,
      );
      result[i * 2 + 1] = this.report.buffer.toString(
        "latin1",
        versionOffset,
        versionOffset + versionLength,
      );
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
