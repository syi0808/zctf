# napi + zctf vs `#[napi(object)]`

이 프로젝트는 임의의 Rust 변환 결과(`TransformResult`)를 두 방식으로 Node.js에
반환한다.

- `transformObject`: napi-rs가 중첩 JavaScript object를 생성
- `transformZctf`: `#[zctf::document]`가 binary document를 encode하고 generated
  lazy view로 읽음
- `transformZctfManual`: 동일한 wire format을 manual writer로 encode
- `transformZctfWasm`: UTF-8 input을 WASM linear memory로 복사하고 동일한
  Rust model/encoder로 document를 생성한 뒤 explicit free

```bash
pnpm --filter zctf-product-benchmark build
pnpm --filter zctf-product-benchmark test
pnpm --filter zctf-product-benchmark bench:quick
pnpm --filter zctf-product-benchmark bench
```

측정에는 Rust 값 생성, N-API transport와 두 접근 패턴이 포함된다.

- `Buffer return only`: Rust 생성/encode/N-API transport
- `WASM bytes return only`: input copy, Rust/WASM encode, linear-memory handle, free
- `View.from prebuilt Buffer`: JS header/schema validation 고정 비용
- `return`: `code`와 list length만 읽는 lazy-access 전체 경로
- `full traversal` / `toObject`: 모든 warning message까지 materialize하는 경로

정식 측정은 warning 0/3/20/100/1,000/10,000개를 사용한다. quick 측정은
0/20/1,000개만 실행한다.

Vitest v4의 benchmark runner(Tinybench)가 warmup과 sampling을 수행한다. 정식
결과는 `results/latest.json`, quick 결과는 `results/latest-quick.json`에
기록된다. quick run은 환경 확인 용도이고 비교 수치에는 `pnpm bench` 결과를
사용한다. 이전 결과와 비교할 때는 다음처럼 Vitest의 compare 기능을 사용한다.

```bash
pnpm exec vitest bench --compare results/baseline.json
```
