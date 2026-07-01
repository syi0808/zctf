import { BinaryDocument } from "../../runtime/src/document.js";
import { FixedListView } from "../../runtime/src/fixed-list.js";
import { BENCH_REPORT, PACKAGE_INFO } from "./layout.generated.js";

const ROOT_OFFSET = BENCH_REPORT.offset;
const PACKAGE_SIZE = PACKAGE_INFO.size;
const BENCH_REPORT_FORMAT = Object.freeze({
  magic: 0x4654_435a,
  versions: [1],
  minimumSize: 64,
  totalLengthOffset: 60,
  strings: {
    tableOffsetField: 32,
    heapOffsetField: 36,
    heapCursorField: 40,
    heapCapacityField: 44,
    countField: 52,
    capacityField: 56,
  },
});

export class PackageInfoView {
  constructor(doc, offset) {
    this.doc = doc;
    this.offset = offset;
  }

  get name() {
    return this.doc.string(this.doc.u32(this.offset));
  }

  set name(value) {
    this.doc.setU32(this.offset, this.doc.allocString(value));
  }

  get version() {
    return this.doc.string(this.doc.u32(this.offset + 4));
  }

  get size() {
    return this.doc.u32(this.offset + 8);
  }

  set size(value) {
    this.doc.setU32(this.offset + 8, value);
  }

  get dependencyCount() {
    return this.doc.u32(this.offset + 12);
  }

  set dependencyCount(value) {
    this.doc.setU32(this.offset + 12, value);
  }

  toObject() {
    return {
      name: this.name,
      version: this.version,
      size: this.size,
      dependencyCount: this.dependencyCount,
    };
  }
}

export class PackageInfoCursor extends PackageInfoView {
  constructor(doc, list) {
    super(doc, list.itemsOffset);
    this.list = list;
  }

  moveTo(index) {
    this.offset = this.list.itemOffset(index);
    return this;
  }
}

export class FixedPackageListView extends FixedListView {
  constructor(doc, offset) {
    super(doc, offset, PACKAGE_SIZE, (document, itemOffset) => {
      return new PackageInfoView(document, itemOffset);
    });
    this.doc = doc;
  }

  cursor() {
    return new PackageInfoCursor(this.doc, this);
  }

  sumSizes() {
    const view = this.document.view;
    const len = this._length;
    let offset = this.itemsOffset + 8;
    let sum = 0;
    for (let i = 0; i < len; i++) {
      sum += view.getUint32(offset, true);
      offset += PACKAGE_SIZE;
    }
    return sum;
  }

  sumDependencyCounts() {
    const view = this.document.view;
    const len = this._length;
    let offset = this.itemsOffset + 12;
    let sum = 0;
    for (let i = 0; i < len; i++) {
      sum += view.getUint32(offset, true);
      offset += PACKAGE_SIZE;
    }
    return sum;
  }

  sumNameByteLengths() {
    const view = this.document.view;
    const strings = this.document.strings;
    const len = this._length;
    let offset = this.itemsOffset;
    let total = 0;
    for (let i = 0; i < len; i++) {
      total += strings.byteLengthUnchecked(view.getUint32(offset, true));
      offset += PACKAGE_SIZE;
    }
    return total;
  }

  sumNameDecodeLengths() {
    const view = this.document.view;
    const strings = this.document.strings;
    const len = this._length;
    let offset = this.itemsOffset;
    let total = 0;
    for (let i = 0; i < len; i++) {
      total += strings.getUnchecked(view.getUint32(offset, true)).length;
      offset += PACKAGE_SIZE;
    }
    return total;
  }

  materializeNames() {
    const result = new Array(this._length);
    const view = this.document.view;
    const strings = this.document.strings;
    let offset = this.itemsOffset;
    for (let i = 0; i < result.length; i++) {
      result[i] = strings.getUnchecked(view.getUint32(offset, true));
      offset += PACKAGE_SIZE;
    }
    return result;
  }

  materializeStrings() {
    const result = new Array(this._length * 2);
    const view = this.document.view;
    const strings = this.document.strings;
    let offset = this.itemsOffset;
    for (let i = 0; i < this._length; i++) {
      result[i * 2] = strings.getUnchecked(view.getUint32(offset, true));
      result[i * 2 + 1] = strings.getUnchecked(view.getUint32(offset + 4, true));
      offset += PACKAGE_SIZE;
    }
    return result;
  }

  countNamesWithPrefix(prefix) {
    if (typeof prefix !== "string") throw new TypeError("prefix must be a string");
    const expected = new TextEncoder().encode(prefix);
    const view = this.document.view;
    const strings = this.document.strings;
    let offset = this.itemsOffset;
    let matches = 0;
    for (let i = 0; i < this._length; i++) {
      const id = view.getUint32(offset, true);
      const [start, length] = strings.rangeUnchecked(id);
      if (length >= expected.length) {
        let match = true;
        for (let j = 0; j < expected.length; j++) {
          if (this.document.bytes[start + j] !== expected[j]) {
            match = false;
            break;
          }
        }
        if (match) matches++;
      }
      offset += PACKAGE_SIZE;
    }
    return matches;
  }

  toObjectArray() {
    const result = new Array(this._length);
    const view = this.document.view;
    const strings = this.document.strings;
    let offset = this.itemsOffset;
    for (let i = 0; i < result.length; i++) {
      result[i] = {
        name: strings.getUnchecked(view.getUint32(offset, true)),
        version: strings.getUnchecked(view.getUint32(offset + 4, true)),
        size: view.getUint32(offset + 8, true),
        dependencyCount: view.getUint32(offset + 12, true),
      };
      offset += PACKAGE_SIZE;
    }
    return result;
  }

  toObjectArrayLegacy() {
    const result = new Array(this._length);
    for (let i = 0; i < result.length; i++) result[i] = this.get(i).toObject();
    return result;
  }

  push({ name, version, size = 0, dependencyCount = 0 }) {
    const item = this.commitPush((offset) => {
      this.doc.setU32(offset, this.doc.allocString(name));
      this.doc.setU32(offset + 4, this.doc.allocString(version));
      this.doc.setU32(offset + 8, size);
      this.doc.setU32(offset + 12, dependencyCount);
    });
    this.doc.setU32(ROOT_OFFSET, this.length);
    return item;
  }
}

export class BenchReportView {
  static from(bytes, options) {
    return new BenchReportView(new BinaryDocument(bytes, BENCH_REPORT_FORMAT, options));
  }

  constructor(doc) {
    this.doc = doc;
    this.packages = new FixedPackageListView(doc, doc.u32(ROOT_OFFSET + 24));
  }

  get packageCount() {
    return this.doc.u32(ROOT_OFFSET);
  }

  get totalSize() {
    return this.doc.u64(ROOT_OFFSET + 8);
  }

  get durationMs() {
    return this.doc.f64(ROOT_OFFSET + 16);
  }

  toObject() {
    return {
      packageCount: this.packageCount,
      totalSize: Number(this.totalSize),
      durationMs: this.durationMs,
      packages: this.packages.toObjectArray(),
    };
  }

  toObjectLegacy() {
    return {
      packageCount: this.packageCount,
      totalSize: Number(this.totalSize),
      durationMs: this.durationMs,
      packages: this.packages.toObjectArrayLegacy(),
    };
  }
}
