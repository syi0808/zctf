import { ZctfDocument } from "./document.js";
import { BENCH_REPORT, PACKAGE_INFO } from "./layout.generated.js";

const ROOT_OFFSET = BENCH_REPORT.offset;
const PACKAGE_SIZE = PACKAGE_INFO.size;

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

export class FixedPackageListView {
  constructor(doc, offset) {
    this.doc = doc;
    this.offset = offset;
    this.itemsOffset = doc.u32(offset + 12);
  }

  get length() {
    return this.doc.u32(this.offset);
  }

  get capacity() {
    return this.doc.u32(this.offset + 4);
  }

  get(index) {
    if (index < 0 || index >= this.length) throw new RangeError("package index out of bounds");
    return new PackageInfoView(this.doc, this.itemsOffset + index * PACKAGE_SIZE);
  }

  push({ name, version, size = 0, dependencyCount = 0 }) {
    const length = this.length;
    if (length >= this.capacity) throw new RangeError("package list capacity exceeded");
    const offset = this.itemsOffset + length * PACKAGE_SIZE;
    this.doc.setU32(offset, this.doc.allocString(name));
    this.doc.setU32(offset + 4, this.doc.allocString(version));
    this.doc.setU32(offset + 8, size);
    this.doc.setU32(offset + 12, dependencyCount);
    this.doc.setU32(this.offset, length + 1);
    this.doc.setU32(ROOT_OFFSET, length + 1);
    return new PackageInfoView(this.doc, offset);
  }
}

export class BenchReportView {
  static from(bytes, options) {
    return new BenchReportView(new ZctfDocument(bytes, options));
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
