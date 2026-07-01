# zctf

## 1차 제품 SDK

새 제품 경로는 Rust 타입에서 schema fragment, binary encoder, generated JS view와
TypeScript declaration을 만든다.

```rust
#[zctf::document]
struct Foo {
    name: String,
    size: u32,
}

let bytes = zctf::encode_owned(&Foo { name: "demo".into(), size: 3 })?;
```

`ZCTF_SCHEMA_OUT_DIR`를 설정해 Rust build가 schema fragment를 출력하게 한 뒤
codegen을 실행한다.

```bash
ZCTF_SCHEMA_OUT_DIR=target/zctf cargo build
cargo run -p zctf-cli -- codegen \
  --schema target/zctf \
  --out generated \
  --emit js,ts,rust
```

`--check`는 committed generated output drift를 검증한다. 완전히 실행 가능한
N-API object 비교 예제는 [bench/README.md](bench/README.md)에 있다.

`string(direct)`는 record에 absolute offset/byte-length를 기록해 string table
lookup을 생략한다. 일반 `String`은 간접 참조와 schema-neutral access를 위한
StringId layout을 유지한다. `encode_owned()`는 먼저 정확한 document 크기를
측정한 뒤 최종 `Vec<u8>` 하나에 직접 encode한다. 직접 `ZctfDocument`를 구현할
경우 `encode_zctf()`는 두 pass에서 동일한 layout을 생성하는 순수한 함수여야 한다.

Rust ↔ JavaScript 사이에서 little-endian binary document를 검증하고 lazy view로
사용하기 위한 라이브러리와, 원래 PoC의 성능 가설을 재현하는 benchmark fixture다.

재사용 가능한 코드는 다음 경계로 분리되어 있다.

- `zctf-core`: Rust의 bounds-checked `Document`, `FixedList`, `StringTable`, endian I/O
- `@zctf/runtime`: format descriptor 기반 JS `BinaryDocument`, mutable string table,
  fixed-capacity list
- `@zctf/transform-config`: schema-specific config compiler의 독립 배포 단위
- `crates/zctf-bench-fixtures`, `packages/bench/fixtures`: PoC의 Package report,
  sample config, 비교용 v1 compiler. 제품 라이브러리에 포함되지 않는다.

`zctf-core`와 `@zctf/runtime`에는 Package/BenchReport/TransformConfig magic이나
offset이 없다. 도메인 layout은 `schemas/zctf.schema.json`의 output별 generated
artifact 또는 호출자가 넘기는 format descriptor에만 존재한다.

## 검증

필수 환경은 Node.js, Rust, `wasm32-unknown-unknown` target이며 Bun은 transport
비교에만 필요하다.

```bash
npm install
npm run build
npm test
npm run bench
npm run bench:bun
npm run bench:wasm
```

빠른 fixture benchmark는 `npm run bench:quick`이다. 정식 실행은 1,000부터
1,000,000 package, config 7종, 최대 100,000 mutation을 측정한다. config 벤치는
기존 v1과 최적화 v2의 compile/read/full-transform 및 sync temp writer를 분리한다.

패키징 가능 파일은 다음 명령으로 확인할 수 있다.

```bash
npm pack --dry-run --workspace @zctf/runtime
npm pack --dry-run --workspace @zctf/transform-config
cargo package -p zctf-core --allow-dirty
```

벤치마크 케이스와 판정 기준은
[docs/BENCHMARK_CASES.md](docs/BENCHMARK_CASES.md), 기존 PoC 결과와 해석은
[docs/BENCHMARKS.md](docs/BENCHMARKS.md)에 있다. layout은
[docs/MEMORY_LAYOUT.md](docs/MEMORY_LAYOUT.md), 구현 범위는
[docs/DESIGN.md](docs/DESIGN.md)에 있으며 원시 결과는
`benchmark-results/*.json`이다.
