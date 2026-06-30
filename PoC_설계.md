# zctf PoC 설계서

## Zero Cost Transform: Rust ↔ JavaScript Object/Config Memory Interop PoC

## 1. 개요

`zctf`는 **Zero Cost Transform**의 약자다.

이 프로젝트의 1차 목표는 Rust 기반 툴체인이 JavaScript 생태계의 config/options/plugin 구조와 결합할 때 발생하는 **object 변환 비용**, **serialization/deserialization 비용**, **FFI boundary 비용**, **JS heap allocation/GC 비용**을 줄일 수 있는지 검증하는 것이다.

기존 Rust 기반 JS 툴체인은 보통 다음 중 하나의 방식을 사용한다.

```txt
1. JS object를 N-API object/property access로 Rust에서 직접 읽음
2. JS object를 JSON.stringify 후 Rust에서 parse
3. Rust 결과를 JS object tree로 materialize
4. Rust 결과를 JSON/string으로 serialize 후 JS에서 parse
```

`zctf`는 이 비용을 줄이기 위해 다음 구조를 실험한다.

```txt
JS object/config
  → zctf binary writer/compiler
  → shared binary buffer
  → Rust direct offset reader

Rust result/report/options
  → zctf binary snapshot
  → JS generated view
  → object처럼 lazy read/write
```

1차 PoC는 안정성보다 성능 검증이 우선이다. 따라서 memory safety, full validation, race-free concurrency, schema evolution, fallback compatibility는 2차 범위로 둔다.

---

## 2. 문제 정의

Rust 기반 툴체인이 JS 생태계와 결합할 때 가장 큰 병목은 Rust 연산 자체가 아니라 **경계 변환 비용**일 수 있다.

대표 케이스는 다음과 같다.

```txt
JS → Rust:
- config/options object
- plugin options
- transform options
- nested config
- string-heavy config
- array-heavy config

Rust → JS:
- transform result
- diagnostics
- warnings
- dependency report
- benchmark report
- metadata
- stats
```

특히 다음과 같은 JS config는 Rust에서 빠르게 consume하기 어렵다.

```ts
const config = {
  typescript: true,
  jsxRuntime: "automatic",
  plugins: ["svgo", "jsx"],
  svgoConfig: {
    multipass: true,
    plugins: [
      { name: "removeViewBox", active: false },
      { name: "convertColors", params: { currentColor: true } }
    ]
  }
}
```

N-API object access 방식은 field마다 property lookup과 type conversion이 필요하다. JSON 방식은 stringify/parse 비용이 추가된다. `zctf`는 JS object를 직접 넘기지 않고, schema 기반 binary layout으로 compile한 뒤 Rust가 offset 기반으로 읽는 방식을 검증한다.

---

## 3. PoC 목표

### 3.1 1차 목표

`zctf` PoC의 1차 목표는 다음 네 가지다.

```txt
1. JS config/options object를 Rust가 빠르게 consume할 수 있는가?
2. Rust result object를 JS가 JS object materialization 없이 빠르게 볼 수 있는가?
3. JS에서 shared binary buffer에 직접 write하고 Rust가 바로 consume할 수 있는가?
4. 같은 binary layout/runtime을 N-API, Bun FFI, WASM backend에서 재사용할 수 있는가?
```

### 3.2 성능 가설

검증할 핵심 가설은 다음과 같다.

```txt
H1. JS object → Rust direct N-API property read보다
    JS object → binary config compiler → Rust offset reader가 빠를 수 있다.

H2. Rust result → JS object materialization보다
    Rust result → binary buffer → JS lazy view가 빠를 수 있다.

H3. JS mutable setter가 shared buffer에 직접 write하고
    Rust가 같은 buffer를 읽는 방식은 serialization보다 빠를 수 있다.

H4. N-API/Bun FFI/WASM은 transport만 다르고,
    zctf binary layout과 JS view runtime은 재사용할 수 있다.

H5. 특히 config handle cache를 적용하면
    config 1개 × 다수 파일 처리에서 input decode 비용을 크게 줄일 수 있다.
```

---

## 4. 비목표

1차 PoC에서 하지 않는 것:

```txt
- AST 지원
- recursive object graph
- arbitrary JS object 지원
- Proxy/getter/class instance 지원
- function/plugin callback 지원
- HashMap/Map/Set 지원
- full memory safety
- strict race condition prevention
- schema evolution
- production-ready fallback
- automatic GC/lifetime integration
- full TypeScript generator 완성도
```

1차 PoC는 범용 object graph가 아니라 **작은 struct/config/report 형태**만 다룬다.

---

## 5. 핵심 설계 원칙

## 5.1 JS object를 만들지 않는다

`zctf`의 핵심은 JS object를 더 빨리 만드는 것이 아니다.

```txt
목표:
JS object를 만들지 않고,
JS object처럼 읽고 쓰게 한다.
```

즉, JS에서는 다음처럼 보인다.

```ts
report.packages.get(0).name
report.packages.get(0).size = 12345
```

하지만 실제로는 JS object property가 아니라 shared binary buffer의 offset을 읽고 쓰는 것이다.

---

## 5.2 Rust struct layout을 직접 노출하지 않는다

성능 PoC라도 Rust native struct를 그대로 JS에 노출하지 않는다.

하지 않는 방식:

```rust
#[repr(C)]
struct PackageInfo {
  name: String,
  version: String,
  size: u32,
}
```

이걸 JS에서 직접 읽으려 하지 않는다.

대신 stable binary layout을 둔다.

```txt
PackageInfoRecord:
  name_string_id: u32
  version_string_id: u32
  size: u32
  dependency_count: u32
```

이유:

```txt
- Rust String/Vec 내부 layout은 JS에서 다루기 어렵다.
- pointer는 N-API/Bun/WASM에서 동일하게 표현되지 않는다.
- alignment/padding/endianness 문제가 생긴다.
- WASM backend와 native backend를 공유하기 어렵다.
```

따라서 `zctf`의 공통 단위는 Rust struct가 아니라 **zctf binary record**다.

---

## 5.3 Transport와 layout을 분리한다

`zctf`는 N-API 라이브러리가 아니다.

```txt
zctf-core:
  binary layout, encoder, reader, writer

zctf-runtime:
  JS DataView/view runtime

zctf-napi:
  Node/N-API transport

zctf-bun-ffi:
  Bun FFI transport

zctf-wasm:
  WASM memory transport
```

N-API, Bun FFI, WASM은 bytes를 JS와 Rust 사이에 전달하는 방식만 다르다.

공통 구조:

```txt
Rust/JS object
  → zctf binary buffer
  → zctf reader/view
```

---

## 5.4 Mutable은 append-only로 시작한다

성능 PoC에서는 mutable-like 동작을 지원하되, 복잡한 allocator/free/compaction은 하지 않는다.

```txt
지원:
- fixed numeric field overwrite
- bool/enum field overwrite
- string append + field ref update
- list push with preallocated capacity

미지원:
- string free
- list insert
- list remove
- heap compaction
- arbitrary allocation
```

문자열 변경은 기존 문자열을 지우지 않고 새 문자열을 heap에 append한 뒤 field의 string id만 바꾼다.

```txt
pkg.name = "new-name"

1. string heap에 "new-name" append
2. string table에 offset/len 추가
3. package.name_string_id 갱신
4. old string은 그대로 둠
```

---

## 6. 전체 아키텍처

```txt
┌─────────────────────────────────────────────────────────┐
│                     JavaScript API                       │
│                                                         │
│  transform(svg, config)                                 │
│  compileConfig(config)                                  │
│  BenchReportView.from(bytes)                            │
│  PackageInfoMutableView.from(bytes)                     │
└──────────────────────────┬──────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────┐
│                  zctf JS Runtime                         │
│                                                         │
│  DataViewReader                                         │
│  DataViewWriter                                         │
│  StringHeap                                             │
│  FixedListView                                          │
│  Generated View Classes                                 │
│  Generated Config Compiler                              │
└──────────────────────────┬──────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────┐
│                   Transport Layer                        │
│                                                         │
│  zctf-napi      → external ArrayBuffer / Buffer          │
│  zctf-bun-ffi   → ptr + len + handle                     │
│  zctf-wasm      → memory.buffer + ptr + len              │
└──────────────────────────┬──────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────┐
│                     Rust Core                            │
│                                                         │
│  zctf-core                                              │
│  offset reader/writer                                   │
│  binary snapshot builder                                │
│  config reader                                          │
│  benchmark native functions                             │
└─────────────────────────────────────────────────────────┘
```

---

## 7. Binary Layout

## 7.1 Buffer 구조

1차 PoC에서는 single contiguous buffer를 사용한다.

```txt
[Header]
[Root Record]
[Object Region]
[List Region]
[String Table]
[String Heap]
[Scratch Region]
```

### Header

```txt
Header:
  magic: u32
  version: u32
  schema_hash: u64
  flags: u32

  root_offset: u32

  object_region_offset: u32
  list_region_offset: u32
  string_table_offset: u32
  string_heap_offset: u32

  heap_cursor: u32
  heap_capacity: u32

  epoch: u32
  reserved: u32
```

`epoch`는 PoC 수준의 seqlock/race detection을 위해 둔다. hot path에서 강한 lock을 걸지는 않는다.

---

## 7.2 Primitive Field

모든 primitive는 little-endian으로 저장한다.

```txt
u8    1 byte
u16   2 bytes
u32   4 bytes
u64   8 bytes
i32   4 bytes
f64   8 bytes
bool  u8
enum  u8/u16
```

JS는 `DataView`로 읽는다.

```ts
view.getUint32(offset, true)
view.setUint32(offset, value, true)
```

Rust는 offset reader로 읽는다.

```rust
u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
```

성능 측정용 fast path에서는 unchecked reader도 둘 수 있다.

---

## 7.3 String Layout

문자열은 field에 직접 저장하지 않는다.

```txt
StringRef:
  string_id: u32
```

String table:

```txt
StringEntry:
  offset: u32
  len: u32
```

String heap:

```txt
UTF-8 bytes...
```

JS string write:

```txt
1. TextEncoder.encode(value)
2. heap_cursor 위치에 bytes copy
3. StringEntry append
4. field에 string_id write
```

JS string read:

```txt
1. field에서 string_id read
2. StringEntry에서 offset/len read
3. TextDecoder로 decode
4. optional string cache
```

PoC에서는 string cache를 옵션으로 둔다.

---

## 7.4 FixedList Layout

1차 PoC는 dynamic resize를 하지 않는다. list는 capacity를 미리 갖는다.

```txt
FixedListHeader:
  len: u32
  capacity: u32
  item_size: u32
  items_offset: u32
```

`push`는 다음만 수행한다.

```txt
1. len < capacity 확인
2. items_offset + len * item_size 위치에 item write
3. len += 1
```

`insert/remove`는 지원하지 않는다.

---

## 8. 1차 데이터 모델

PoC에서 사용할 benchmark model은 `BenchReport`다.

```rust
struct BenchReport {
    package_count: u32,
    total_size: u64,
    duration_ms: f64,
    packages: FixedList<PackageInfo>,
}

struct PackageInfo {
    name: StringRef,
    version: StringRef,
    size: u32,
    dependency_count: u32,
}
```

JS API 예시:

```ts
const report = BenchReportView.from(bytes)

console.log(report.packageCount)
console.log(report.totalSize)

const pkg = report.packages.get(0)

console.log(pkg.name)
pkg.size = 12345
pkg.name = "zctf"
```

내부적으로는 다음 offset을 읽고 쓴다.

```txt
BenchReport:
  package_count: u32
  total_size: u64
  duration_ms: f64
  packages_list_ref: u32

PackageInfo:
  name_string_id: u32
  version_string_id: u32
  size: u32
  dependency_count: u32
```

---

## 9. JS → Rust Config Input 설계

`zctf`의 중요한 타겟은 JS config/options object를 Rust가 빠르게 consume하는 것이다.

## 9.1 Config Shape

PoC config는 다음 형태로 둔다.

```ts
type TransformConfig = {
  typescript?: boolean
  jsxRuntime?: "automatic" | "classic"
  exportType?: "default" | "named"
  plugins?: string[]
  svgo?: boolean
  svgoConfig?: {
    multipass?: boolean
    floatPrecision?: number
    plugins?: Array<{
      name: string
      active?: boolean
      currentColor?: boolean
    }>
  }
}
```

## 9.2 Config Compile Flow

```txt
JS plain object
  → generated config compiler
  → zctf binary config buffer
  → Rust config reader
  → optional ConfigHandle cache
```

API:

```ts
const compiled = compileConfig({
  typescript: true,
  jsxRuntime: "automatic",
  plugins: ["svgo", "jsx"],
})

native.transformCompiled(svg, compiled)
```

Handle cache API:

```ts
const handle = native.createConfigHandle(compiled)

for (const svg of svgs) {
  native.transformWithConfig(svg, handle)
}

handle.dispose()
```

## 9.3 Config Layout

```txt
TransformConfigRecord:
  presence_bitmap: u64

  typescript: u8
  jsx_runtime: u8
  export_type: u8
  svgo: u8

  plugins_list_ref: u32
  svgo_config_ref: u32
```

Nested `svgoConfig`:

```txt
SvgoConfigRecord:
  presence_bitmap: u64

  multipass: u8
  float_precision: f64
  plugins_list_ref: u32
```

Plugin item:

```txt
SvgoPluginRecord:
  presence_bitmap: u64

  name_string_id: u32
  active: u8
  current_color: u8
```

## 9.4 Enum String Optimization

JS string literal config는 Rust에 string으로 넘기지 않는다.

```ts
jsxRuntime: "automatic" | "classic"
```

compiler에서 enum id로 변환한다.

```txt
automatic = 1
classic = 2
```

Rust는 `u8`만 읽는다.

---

## 10. Rust → JS Result View 설계

Rust가 만든 result/report는 JS object로 변환하지 않고 binary snapshot으로 넘긴다.

```rust
fn make_bench_report(count: u32) -> ZctfBuffer
```

JS:

```ts
const bytes = native.makeBenchReport(100_000)
const report = BenchReportView.from(bytes)

console.log(report.packages.get(0).name)
```

Generated view class:

```ts
class PackageInfoView {
  constructor(private doc: ZctfDocument, private offset: number) {}

  get name(): string {
    const id = this.doc.u32(this.offset + 0)
    return this.doc.string(id)
  }

  set name(value: string) {
    const id = this.doc.allocString(value)
    this.doc.setU32(this.offset + 0, id)
  }

  get size(): number {
    return this.doc.u32(this.offset + 8)
  }

  set size(value: number) {
    this.doc.setU32(this.offset + 8, value)
  }
}
```

---

## 11. Transport Backend 설계

## 11.1 공통 Transport Output

JS runtime은 host별 차이를 몰라야 한다.

```ts
type ZctfBytes =
  | {
      kind: "arraybuffer"
      buffer: ArrayBuffer
      byteOffset: number
      byteLength: number
      release?: () => void
    }
  | {
      kind: "wasm"
      memory: WebAssembly.Memory
      ptr: number
      len: number
      release?: () => void
    }
  | {
      kind: "bun-ptr"
      ptr: number
      len: number
      handle: number
      release?: () => void
    }
```

## 11.2 N-API Backend

N-API backend는 primary backend다.

```txt
Rust Vec<u8>/Box<[u8]>
  → external ArrayBuffer or Buffer
  → JS DataView
```

Node-API는 JavaScript engine 변화에서 addon을 분리하기 위한 ABI-stable API이고, native addon이 JavaScript value와 상호작용할 수 있게 한다. N-API backend는 그 중 external `ArrayBuffer`/`Buffer` 계열 기능을 transport로 사용한다.

목표:

```txt
- Rust-owned binary buffer를 JS에 전달
- JS DataView로 read/write
- JS GC finalize 시 Rust memory release
```

## 11.3 Bun FFI Backend

Bun FFI backend는 performance comparison backend다.

Bun의 `bun:ffi`는 C ABI를 지원하는 Rust/C/C++ 등 native library를 JavaScript에서 호출할 수 있게 한다. 다만 Bun 공식 문서는 `bun:ffi`가 experimental이며 production에서 native code와 상호작용할 때는 Node-API가 더 안정적인 방식이라고 설명한다.

Bun backend 목표:

```txt
Rust cdylib
  → extern "C" function
  → ptr + len + handle 반환
  → JS가 DataView 또는 Bun pointer read path로 접근
```

PoC mode:

```txt
Mode A:
  ptr → toArrayBuffer → DataView

Mode B:
  ptr → Bun read.* fast path

Mode C:
  JS ArrayBuffer → ptr(buffer) → Rust consume
```

Bun FFI API reference는 `bun:ffi`가 native library를 JavaScript에서 고성능으로 호출할 수 있게 하며, Rust처럼 C ABI를 노출할 수 있는 언어와 함께 사용할 수 있다고 설명한다.

## 11.4 WASM Backend

WASM backend는 browser/portable runtime backend다.

```txt
Rust wasm32
  → WebAssembly.Memory
  → JS DataView(memory.buffer, ptr, len)
```

`WebAssembly.Memory.prototype.grow()`는 기존 `buffer` reference를 detach할 수 있다. MDN은 `grow()` 호출 시 기존 `ArrayBuffer` reference가 detach되고, `grow(0)`도 detach를 일으킨다고 설명한다. 따라서 PoC에서는 view가 살아있는 동안 `memory.grow()`를 금지한다.

PoC rule:

```txt
- WASM memory는 초기에 충분히 크게 할당
- view가 살아있는 동안 memory.grow 금지
- generation check는 2차 범위
```

---

## 12. Mutable Shared Buffer 설계

PoC에서는 JS와 Rust가 같은 buffer를 읽고 쓸 수 있게 한다.

## 12.1 Ownership Phase Assumption

강한 lock을 hot path에 넣지 않는다.

대신 다음 phase-based ownership을 가정한다.

```txt
Phase 1:
  Rust creates buffer

Phase 2:
  JS mutates buffer through generated view

Phase 3:
  JS calls native.consume(buffer)

Phase 4:
  Rust reads buffer synchronously

Phase 5:
  JS resumes after native returns
```

이 가정에서는 JS main thread와 Rust sync native call이 동시에 같은 buffer를 mutate하지 않는다.

Worker/thread concurrency는 2차 범위다.

## 12.2 Epoch Header

최소한의 write detection을 위해 header에 `epoch`를 둔다.

```txt
epoch even:
  stable

epoch odd:
  writing
```

JS write transaction:

```ts
doc.beginWrite()
pkg.size = 12345
pkg.name = "zctf"
doc.endWrite()
```

내부:

```txt
beginWrite:
  epoch += 1

endWrite:
  epoch += 1
```

Rust read:

```txt
1. before_epoch 읽기
2. odd면 retry
3. data 읽기
4. after_epoch 읽기
5. before == after면 accept
```

PoC에서는 atomic memory ordering은 강하게 다루지 않는다. SharedArrayBuffer + Atomics 기반의 strict concurrency는 2차 범위다.

---

## 13. JS Runtime API

## 13.1 Reader/Writer Interface

```ts
interface ZctfMemory {
  u8(offset: number): number
  u32(offset: number): number
  u64(offset: number): bigint
  f64(offset: number): number

  setU8(offset: number, value: number): void
  setU32(offset: number, value: number): void
  setU64(offset: number, value: bigint): void
  setF64(offset: number, value: number): void

  bytes(offset: number, len: number): Uint8Array
  copyBytes(offset: number, bytes: Uint8Array): void
}
```

## 13.2 Document Runtime

```ts
class ZctfDocument {
  constructor(private memory: ZctfMemory) {}

  string(id: number): string
  allocString(value: string): number

  beginWrite(): void
  endWrite(): void

  u32(offset: number): number
  setU32(offset: number, value: number): void
}
```

## 13.3 Generated View

```ts
class BenchReportView {
  static from(bytes: ZctfBytes): BenchReportView

  get packageCount(): number
  set packageCount(value: number)

  get totalSize(): bigint
  set totalSize(value: bigint)

  get durationMs(): number
  set durationMs(value: number)

  get packages(): FixedListView<PackageInfoView>
}
```

---

## 14. Rust Core API

## 14.1 Core Types

```rust
pub struct ZctfBuffer {
    bytes: Box<[u8]>,
}

pub struct ZctfView<'a> {
    bytes: &'a [u8],
}

pub struct ZctfViewMut<'a> {
    bytes: &'a mut [u8],
}
```

## 14.2 Reader

```rust
impl<'a> ZctfView<'a> {
    #[inline(always)]
    pub fn u32(&self, offset: usize) -> u32 {
        u32::from_le_bytes(self.bytes[offset..offset + 4].try_into().unwrap())
    }

    #[inline(always)]
    pub fn f64(&self, offset: usize) -> f64 {
        f64::from_le_bytes(self.bytes[offset..offset + 8].try_into().unwrap())
    }
}
```

## 14.3 Unsafe Fast Reader

PoC benchmark용으로 unchecked reader를 둔다.

```rust
#[inline(always)]
pub unsafe fn u32_unchecked(ptr: *const u8, offset: usize) -> u32 {
    let p = ptr.add(offset) as *const u32;
    u32::from_le(p.read_unaligned())
}
```

---

## 15. ABI 설계

Bun FFI와 WASM을 고려해 C ABI-safe interface를 둔다.

```rust
#[repr(C)]
pub struct ZctfSlice {
    pub ptr: *const u8,
    pub len: usize,
    pub handle: u64,
}

#[no_mangle]
pub extern "C" fn zctf_make_bench_report(
    count: u32,
    out: *mut ZctfSlice,
) -> u32 {
    // create buffer
    // write ptr/len/handle to out
    0
}

#[no_mangle]
pub extern "C" fn zctf_consume_bench_report(
    ptr: *const u8,
    len: usize,
) -> u32 {
    // read buffer
    0
}

#[no_mangle]
pub extern "C" fn zctf_release(handle: u64) {
    // drop buffer
}
```

N-API backend는 이 ABI를 직접 노출하지 않아도 되지만, 내부 core는 같은 buffer 구조를 사용한다.

---

## 16. Benchmark Plan

## 16.1 Benchmark 1: Rust → JS Result

목표:

```txt
Rust result를 JS가 읽을 때,
JS object materialization 대비 zctf view가 빠른지 측정
```

비교군:

```txt
A. N-API object array
B. JSON string → JSON.parse
C. zctf ArrayBuffer → readonly view
D. zctf ArrayBuffer → mutable view
E. zctf view → toObject()
```

데이터 크기:

```txt
1,000 packages
10,000 packages
100,000 packages
1,000,000 packages
```

측정 항목:

```txt
- Rust 생성 시간
- boundary return 시간
- JS first access 시간
- JS partial iteration 시간
- JS full iteration 시간
- toObject() 시간
- heap usage
- GC 영향
```

Partial access:

```txt
첫 번째 package.name만 읽기
모든 package.size만 합산
모든 package.name decode
```

---

## 16.2 Benchmark 2: JS → Rust Config Input

목표:

```txt
JS config object를 Rust가 consume할 때,
N-API direct object read / JSON 대비
compiled config buffer가 빠른지 측정
```

비교군:

```txt
A. N-API direct object read
B. JSON.stringify(config) → Rust parse
C. JS compileConfig(config) → Rust read buffer
D. compileConfig(config) → Rust ConfigHandle → N files reuse
E. generated ConfigBuilder → Rust read buffer
```

Config shape:

```txt
small:
  5 primitive fields

medium:
  nested svgoConfig + plugins 10개

large:
  plugins 100개

string-heavy:
  long plugin names, params strings

enum-heavy:
  string literal options 다수
```

측정 항목:

```txt
- single config consume time
- config compile time
- Rust config read time
- many files amortized time
- allocations
- JS heap usage
```

---

## 16.3 Benchmark 3: Mutable JS Write → Rust Read

목표:

```txt
JS가 shared buffer에 직접 write하고 Rust가 바로 읽는 방식이
serialization보다 빠른지 측정
```

비교군:

```txt
A. JS object mutation → JSON.stringify → Rust parse
B. JS object mutation → N-API direct object read
C. zctf mutable setter → Rust offset read
D. zctf builder → Rust offset read
```

Mutation cases:

```txt
- 100,000개 size field update
- 100,000개 dependency_count update
- 100,000개 name string update
- 100,000개 list push
```

측정 항목:

```txt
- JS write time
- Rust consume time
- total pipeline time
- memory growth
- string heap usage
```

중요한 해석:

```txt
JS object field update 자체는 V8이 매우 빠를 수 있다.
따라서 zctf setter가 JS object setter보다 빠르지 않을 수 있다.

하지만 Rust까지 넘기는 전체 pipeline에서는
zctf가 serialization/object conversion 비용을 피할 수 있다.
```

---

## 16.4 Benchmark 4: Transport Comparison

목표:

```txt
같은 zctf binary layout이
N-API, Bun FFI, WASM에서 재사용 가능한지 검증
```

비교군:

```txt
N-API:
  external ArrayBuffer / Buffer

Bun FFI:
  ptr → toArrayBuffer → DataView
  ptr → read.* fast path

WASM:
  memory.buffer direct view
  copy-to-ArrayBuffer fallback
```

측정 항목:

```txt
- transport call overhead
- first access time
- full iteration time
- mutable write time
- Rust consume time
```

---

## 17. Success Criteria

PoC 성공 기준은 다음과 같다.

## 17.1 Rust → JS

```txt
- zctf view first access가 JSON.parse/object materialization보다 명확히 빠를 것
- partial access에서 큰 이점이 있을 것
- full access에서는 최소한 경쟁 가능할 것
- toObject()는 느려도 escape hatch로 동작할 것
```

## 17.2 JS → Rust Config

```txt
- medium/large config에서 compiled buffer 방식이 N-API direct read보다 빠를 것
- many files reuse에서 ConfigHandle이 압도적으로 유리할 것
- enum/string literal conversion이 Rust string decode 비용을 줄일 것
```

## 17.3 Mutable

```txt
- numeric field update + Rust consume pipeline에서 JSON/N-API 방식보다 빠를 것
- string mutation은 TextEncoder 비용이 있더라도 전체 pipeline에서 이점이 있는지 확인할 것
- append-only heap 방식이 PoC 수준에서 충분히 단순하고 빠를 것
```

## 17.4 Transport

```txt
- N-API/Bun FFI/WASM 모두 같은 view runtime을 재사용할 것
- backend별 차이가 transport adapter 내부에만 머물 것
- Bun FFI/WASM은 production 안정성보다 벤치 비교 대상으로 동작할 것
```

---

## 18. Milestone

## M0: Hand-written Layout

```txt
- BenchReport layout 수동 정의
- Rust encoder 수동 작성
- JS view class 수동 작성
- N-API backend 1개만 구현
```

목표:

```txt
Rust → JS readonly view 벤치
```

---

## M1: Mutable View

```txt
- JS setter 구현
- string heap append
- fixed list push
- Rust consume_bench_report 구현
```

목표:

```txt
JS mutable write → Rust read pipeline 벤치
```

---

## M2: Config Compiler

```txt
- TransformConfig schema 수동 정의
- JS compileConfig(config) 구현
- Rust config reader 구현
- N-API direct object read baseline 구현
- JSON baseline 구현
```

목표:

```txt
JS config object input 최적화 벤치
```

---

## M3: ConfigHandle Cache

```txt
- Rust ConfigHandle 저장소 구현
- createConfigHandle
- transformWithConfig
- dispose
```

목표:

```txt
1 config × N files 벤치
```

---

## M4: Bun FFI Backend

```txt
- Rust cdylib export
- zctf_make_bench_report
- zctf_consume_bench_report
- zctf_release
- Bun dlopen wrapper
- ptr/len/handle transport
```

목표:

```txt
N-API vs Bun FFI transport 비교
```

---

## M5: WASM Backend

```txt
- wasm32 build
- wasm memory allocator
- ptr/len return
- JS memory.buffer DataView runtime
- memory.grow 금지
```

목표:

```txt
N-API vs Bun FFI vs WASM 비교
```

---

## M6: Minimal Codegen

```txt
- Rust schema description
- TS view generator
- TS config compiler generator
- primitive/StringRef/FixedList 지원
```

목표:

```txt
수동 구현 제거
```

---

## 19. Repository Structure

```txt
zctf/
  crates/
    zctf-core/
      src/
        layout.rs
        reader.rs
        writer.rs
        string_heap.rs
        fixed_list.rs
        bench_model.rs

    zctf-napi/
      src/
        lib.rs
        bench.rs
        config.rs

    zctf-ffi/
      src/
        lib.rs
        abi.rs

    zctf-wasm/
      src/
        lib.rs
        memory.rs

  packages/
    runtime/
      src/
        memory.ts
        document.ts
        string-heap.ts
        fixed-list.ts
        bench-report.view.ts

    config/
      src/
        transform-config.compiler.ts
        transform-config.types.ts

    bench/
      src/
        napi.bench.ts
        bun-ffi.bench.ts
        wasm.bench.ts
        baselines/
          json.ts
          napi-object.ts
          plain-object.ts

  examples/
    config-input/
    mutable-report/
    transport-comparison/

  docs/
    DESIGN.md
    BENCHMARKS.md
    MEMORY_LAYOUT.md
```

---

## 20. Risk

## 20.1 JS DataView가 예상보다 느릴 수 있음

DataView getter/setter는 JS object field access보다 느릴 수 있다.

대응:

```txt
- pipeline 전체 비용을 본다.
- JS object update 단독이 아니라 Rust consume까지 측정한다.
- Bun read.* fast path를 실험한다.
- numeric-heavy, string-heavy를 분리한다.
```

## 20.2 String decode/encode가 병목일 수 있음

문자열은 TextEncoder/TextDecoder 비용이 크다.

대응:

```txt
- enum/string literal은 u8/u16 id로 변환
- string cache 옵션
- config key/name interning
- partial access 벤치 분리
```

## 20.3 Mutable string heap이 계속 증가한다

PoC에서는 허용한다.

대응:

```txt
- heap usage를 측정한다.
- capacity overflow 시 throw한다.
- compaction/free-list는 2차 범위로 둔다.
```

## 20.4 WASM memory grow 문제가 생길 수 있음

PoC에서는 view lifetime 중 `memory.grow()`를 금지한다. MDN 기준으로 `WebAssembly.Memory.grow()`는 기존 buffer를 detach할 수 있으므로, production 단계에서는 generation check가 필요하다.

## 20.5 Bun FFI 안정성

Bun 공식 문서상 `bun:ffi`는 experimental이고 known bugs/limitations가 있다. 따라서 1차 PoC에서는 Bun FFI를 production target이 아니라 performance comparison target으로 둔다.

---

## 21. 2차 범위

1차 PoC 이후 고려할 것:

```txt
- derive macro
- schema evolution
- validation
- bounds check mode
- unsafe fast mode
- SharedArrayBuffer + Atomics
- worker/thread safe shared memory
- function registry
- plugin callback
- patch protocol
- AST/IR support
- Map/HashMap support
- enum with payload
- recursive object graph
- memory compaction
- free list allocator
- production fallback
```

---

## 22. 최종 요약

`zctf` PoC는 다음 질문에 답하기 위한 실험이다.

```txt
Rust 기반 JS 툴체인이
JS config/options object를 더 빠르게 받고,
Rust result를 JS object materialization 없이 노출하고,
JS가 shared buffer에 직접 write한 값을 Rust가 바로 consume할 수 있는가?
```

1차 설계의 핵심은 다음과 같다.

```txt
1. AST는 배제한다.
2. 단순 object/config/report만 다룬다.
3. stable binary layout을 둔다.
4. JS generated view가 DataView로 read/write한다.
5. JS config object는 generated compiler로 binary buffer화한다.
6. Rust는 N-API object가 아니라 bytes를 offset으로 읽는다.
7. mutable은 append-only heap + fixed list로 제한한다.
8. race safety는 phase-based ownership + epoch 수준만 둔다.
9. N-API/Bun FFI/WASM은 transport adapter로만 분리한다.
10. 성공 여부는 benchmark로 판단한다.
```

`zctf`의 1차 정체성은 다음 문장으로 요약된다.

```txt
zctf is an experimental Rust ↔ JavaScript binary object interop layer
that avoids object materialization and serialization by letting generated JS views
read and write shared binary memory directly.
```
