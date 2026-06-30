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

export class FixedPackageListView extends FixedListView {
  constructor(doc, offset) {
    super(doc, offset, PACKAGE_SIZE, (document, itemOffset) => {
      return new PackageInfoView(document, itemOffset);
    });
    this.doc = doc;
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
    const packages = new Array(this.packages.length);
    for (let i = 0; i < packages.length; i++) packages[i] = this.packages.get(i).toObject();
    return {
      packageCount: this.packageCount,
      totalSize: Number(this.totalSize),
      durationMs: this.durationMs,
      packages,
    };
  }
}
