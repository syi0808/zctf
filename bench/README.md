# napi + zctf vs `#[napi(object)]`

이 프로젝트는 임의의 Rust 변환 결과(`TransformResult`)를 두 방식으로 Node.js에
반환한다.

- `transformObject`: napi-rs가 중첩 JavaScript object를 생성
- `transformZctf`: `#[zctf::document]`가 binary document를 encode하고 generated
  lazy view로 읽음
- `transformZctfManual`: 동일한 wire format을 manual writer로 encode

```bash
pnpm --filter zctf-product-benchmark build
pnpm --filter zctf-product-benchmark test
pnpm --filter zctf-product-benchmark bench:quick
pnpm --filter zctf-product-benchmark bench
```

측정에는 Rust 값 생성, N-API transport와 두 접근 패턴이 포함된다.

- `return`: `code`와 list length만 읽는 lazy-access 경로
- `full traversal` / `toObject`: 모든 warning message까지 materialize하는 경로

Vitest v4의 benchmark runner(Tinybench)가 warmup과 sampling을 수행한다. 정식
결과는 `results/latest.json`, quick 결과는 `results/latest-quick.json`에
기록된다. quick run은 환경 확인 용도이고 비교 수치에는 `pnpm bench` 결과를
사용한다. 이전 결과와 비교할 때는 다음처럼 Vitest의 compare 기능을 사용한다.

```bash
pnpm exec vitest bench --compare results/baseline.json
```
