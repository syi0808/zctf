# zctf PoC

`PoC_설계.md`의 Rust ↔ JavaScript binary memory interop 가설을 실제로 실행하고
측정하는 PoC다. 하나의 little-endian layout과 JS view runtime을 N-API, Bun FFI,
WASM에서 재사용한다.

## 실행

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

빠른 smoke benchmark는 `npm run bench:quick`이다. 정식 실행은 1,000부터
1,000,000 package, config 7종, 최대 100,000 mutation을 측정한다. config 벤치는
기존 v1과 최적화 v2의 compile/read/full-transform 및 sync temp writer를 분리한다.

결과와 해석은 [docs/BENCHMARKS.md](docs/BENCHMARKS.md), layout은
[docs/MEMORY_LAYOUT.md](docs/MEMORY_LAYOUT.md), 구현 범위는
[docs/DESIGN.md](docs/DESIGN.md)에 있다. 원시 결과는 `benchmark-results/*.json`이다.
