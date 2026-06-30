# 구현 범위

## 구성

- `zctf-core`: 공통 report encoder, report/config offset reader
- `zctf-napi`: Buffer/object/JSON baseline, ConfigHandle
- `zctf-ffi`: Bun용 `ptr + len + handle` ABI
- `zctf-wasm`: WASM linear-memory `ptr + len + handle` ABI
- `packages/runtime`: DataView 기반 readonly/mutable lazy view
- `packages/config`: plain object → binary config compiler
- `packages/bench`: 비교군과 transport별 실행 하네스

N-API, Bun FFI, WASM 모두 `zctf-core`가 생성한 같은 bytes와
`BenchReportView`를 사용한다. backend별 차이는 bytes를 가져오는 adapter에만 있다.

## 구현된 동작

- Rust report snapshot → JS lazy view, full `toObject()` escape hatch
- primitive/string read, numeric overwrite
- append-only string mutation
- preallocated fixed-list push
- JS config compiler, enum ID 변환, Rust direct offset reader
- schema에서 record offset/enum 상수를 생성하는 minimal codegen (`npm run generate`)
- N-API object와 JSON baseline
- compiled config의 Rust `ConfigHandle` cache
- N-API, Bun FFI, WASM transport

## 의도적인 PoC 제한

- schema evolution, strict validation, concurrent mutation, automatic lifetime/GC 연동 없음
- FFI handle registry는 mutex 기반이며 transport 검증용이다.
- WASM view를 보유한 동안 추가 allocation으로 `memory.grow`가 발생하면 기존 view를
  다시 얻어야 한다.
- binary buffer는 mutation/list push 여유 공간을 미리 잡으므로 compact snapshot보다
  크다.
- N-API 호출 하나에서 Rust 내부 생성 시간과 JS boundary conversion 시간을 완전히
  분리할 수 없어 반환 benchmark는 합산값이다.
- OS allocator별 allocation count는 계측하지 않는다. 대신 JSON/zctf byte 크기와
  append-only string heap cursor 증가량을 원시 결과에 기록한다.
