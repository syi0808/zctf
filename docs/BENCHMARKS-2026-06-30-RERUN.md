# 벤치마크 재측정 결과

측정일: 2026-06-30 23:10 KST

이 문서는 기존 [`BENCHMARKS.md`](BENCHMARKS.md)의 측정 항목을 기준으로 현재
구현을 다시 측정한 결과다. 기존 문서와 원시 결과는 수정하지 않았다.

## 요약

- 1,000,000 package 반환에서 compact zctf는 N-API object보다 4.88배,
  JSON 반환 후 parse보다 3.27배 빨랐다.
- 숫자 전체 순회의 bulk API는 object 순회와 zctf get-loop보다 각각 1.60배
  빨랐다.
- 문자열을 모두 JS string으로 decode하는 get-loop는 object 순회보다 9.64배
  느렸다. decode가 필요 없는 byte length 집계는 get-loop보다 14.43배 빨랐다.
- JS → Rust config의 sync temp 경로는 모든 shape에서 v1 baseline보다 빨랐다.
  개선 폭은 2.08~9.87배다.
- 100,000개 mutable pipeline은 numeric update에서 20.64~22.86배,
  string update에서 2.73배, fixed-list push에서 2.71배 빨랐다.
- 직전 전체 측정과 비교한 주요 Rust → JS 수치의 변화는 -2.6~+0.7%였다.

## 측정 조건

| 항목 | 값 |
|---|---|
| machine | Apple arm64 |
| OS | macOS 26.5.1 (25F80) |
| Node | v24.5.0 |
| Bun | 1.3.11 |
| Rust | 1.93.1 |
| build | release |
| 단위 | ms |

N-API 측정은 5회 warm-up 후 5~7회 측정한 median이다. sample 사이에
`global.gc()`를 요청했다. config single-call은 shape별 5,000~20,000회 호출을
한 sample로 묶은 뒤 호출당 시간으로 환산했다. Bun FFI와 WASM transport는
single-shot smoke 측정이므로 backend 순위를 판단하는 자료로 사용하지 않는다.

원시 결과:

- [`napi-2026-06-30-rerun.json`](../benchmark-results/napi-2026-06-30-rerun.json)
- [`bun-ffi-2026-06-30-rerun.json`](../benchmark-results/bun-ffi-2026-06-30-rerun.json)
- [`wasm-2026-06-30-rerun.json`](../benchmark-results/wasm-2026-06-30-rerun.json)

## Rust → JS result

### 반환과 첫 접근

| 1,000,000 packages | 시간 | compact zctf 대비 |
|---|---:|---:|
| N-API object 반환 | 481.782 | 4.88x |
| JSON 반환 + parse | 322.916 | 3.27x |
| zctf mutable 반환 + view | 98.930 | 1.00x |
| zctf compact 반환 + view | 98.805 | 1.00x |

이미 생성된 값의 첫 package name 접근은 object 0.003ms, JSON parse 포함
194.315ms, zctf view 0.032ms였다. zctf는 object처럼 전체 JS object graph를
먼저 만들지 않으면서도 첫 접근 비용을 낮게 유지했다.

### 전체 순회

| 1,000,000 packages | object | zctf get-loop | zctf bulk/optimized |
|---|---:|---:|---:|
| 모든 `size` 합산 | 12.729 | 12.706 | 7.943 |
| 모든 `name` 길이 합산 | 13.356 | 128.804 | 84.236 |
| name byte length 합산 | - | - | 8.925 |
| name array materialize | - | - | 107.630 |
| prefix filter | 28.208 | - | 21.745 |
| `toObject()` | - | 342.319 (legacy) | 240.450 |

숫자 bulk 합산은 object와 get-loop보다 각각 1.60배 빨랐다. 문자열 decode
get-loop는 object보다 9.64배 느리므로 full string traversal에는 맞지 않는다.
문자열을 decode하지 않는 metadata 집계는 decode get-loop보다 14.43배,
byte prefix filter는 object string filter보다 1.30배 빨랐다.

warm string cache는 84.195ms에서 61.389ms로 1.37배 개선됐다. 최적화된
`toObject()`는 legacy 경로보다 1.42배 빨랐다.

### 저장 크기와 레이아웃

| layout, 1,000,000 packages | bytes | `sumSizes` | name byte lengths | materialize names |
|---|---:|---:|---:|---:|
| mutable AoS | 112,000,176 | 7.947 | - | - |
| compact AoS | 51,789,002 | 7.943 | 8.925 | 107.630 |
| direct StringRef | 43,788,922 | 10.483 | 10.568 | 104.453 |
| SoA | 29,888,922 | 10.219 | 10.565 | - |
| AoS + sidecar | 59,789,002 | 10.805 | - | - |

compact AoS는 mutable buffer보다 53.8% 작고 bulk 숫자 순회 시간은
동일했다. 실험 레이아웃은 더 작거나 name materialization이 소폭 빠른 경우가
있지만, 주요 bulk 순회에서는 compact AoS가 가장 빨랐다.

## JS → Rust config

### Compile + Rust read

단위는 호출당 ms다.

| shape | N-API object | JSON stringify + parse | v1 baseline | v2 owned | v2 sync temp | v1 대비 |
|---|---:|---:|---:|---:|---:|---:|
| small | 0.000811 | 0.000591 | 0.000348 | 0.000421 | 0.000136 | 2.56x |
| medium | 0.005687 | 0.005332 | 0.005553 | 0.001594 | 0.001244 | 4.46x |
| large | 0.044926 | 0.041901 | 0.049177 | 0.011742 | 0.010762 | 4.57x |
| string-heavy | 0.052878 | 0.054651 | 0.058600 | 0.024845 | 0.023234 | 2.52x |
| plugin-heavy | 0.045610 | 0.042631 | 0.052321 | 0.005799 | 0.005303 | 9.87x |
| unicode-heavy | 0.053771 | 0.065703 | 0.058914 | 0.030092 | 0.028286 | 2.08x |
| default-heavy | 0.000807 | 0.000599 | 0.000398 | 0.000446 | 0.000146 | 2.73x |

sync temp는 모든 shape에서 v1 baseline과 두 일반 입력 경로보다 빨랐다.
owned v2는 medium 이상에서 v1보다 1.96~9.02배 빨랐지만 small과
default-heavy에서는 각각 17%, 12% 느렸다.

plugin-heavy의 optimized buffer는 1,296 bytes로 baseline 5,512 bytes보다
76.5% 작다. known plugin name 200개가 ID로 기록되며 string heap write는 없다.

### Full transform boundary

| shape | N-API object | JSON stringify + parse | v1 baseline | v2 owned | v2 sync temp |
|---|---:|---:|---:|---:|---:|
| medium | 0.005742 | 0.005395 | 0.005334 | 0.001694 | 0.001331 |
| large | 0.045174 | 0.042280 | 0.048598 | 0.011779 | 0.010832 |
| string-heavy | 0.053635 | 0.054925 | 0.057479 | 0.025113 | 0.023398 |
| plugin-heavy | 0.045697 | 0.042718 | 0.052494 | 0.005876 | 0.005419 |

이 경로의 transform body는 SVG 변환 알고리즘이 아니라 input length와 config
checksum을 결합하는 최소 구현이다. 실제 변환 성능이 아니라 compile과
JS↔Rust boundary의 회귀 지표로 해석해야 한다.

### Rust read와 반복 접근

| shape | v1 read | v2 validated read | hot field 100회 direct | local 승격 후 100회 | 승격 개선 |
|---|---:|---:|---:|---:|---:|
| small | 0.000053 | 0.000057 | 0.000272 | 0.000100 | 2.72x |
| medium | 0.000074 | 0.000095 | 0.000307 | 0.000129 | 2.38x |
| large | 0.000174 | 0.000444 | 0.000569 | 0.000384 | 1.48x |
| plugin-heavy | 0.000195 | 0.000415 | 0.000537 | 0.000360 | 1.49x |

v2 validated read는 v1보다 느리지만 최대 0.000444ms다. local struct 승격은
반복 direct offset read보다 1.48~2.72배 빨랐다.

## Mutable JS → Rust pipeline

100,000개를 수정하거나 추가한 뒤 Rust가 consume하는 전체 시간이다.

| case | object/N-API 또는 JSON | zctf | 개선 |
|---|---:|---:|---:|
| size update + N-API consume | 52.601 | 2.301 | 22.86x |
| size update + JSON consume | 47.500 | 2.301 | 20.64x |
| dependency count + N-API consume | 52.168 | 0.449 | 116.19x |
| string update + JSON consume | 93.182 | 34.102 | 2.73x |
| fixed-list push + N-API consume | 147.132 | 54.346 | 2.71x |

100,000 name 변경은 string entry 100,000개와 heap 1,288,890 bytes를
추가했다. dependency count 결과는 다른 mutable case보다 차이가 크므로 후속
측정에서도 별도 관찰이 필요하다.

## Transport 재사용

100,000 package의 생성, transport view 획득, 첫 string 접근, Rust consume을
한 번에 수행한 smoke 결과다.

| backend | elapsed | bytes | checksum |
|---|---:|---:|---:|
| Bun FFI | 12.069 | 11,200,176 | 85,000,700,000 |
| WASM | 12.562 | 11,200,176 | 85,000,700,000 |

두 backend의 buffer length, 첫 package name, checksum이 일치했다. 반복과
warm-up이 없는 측정이므로 0.493ms 차이에 의미를 부여하지 않는다.

## 직전 전체 측정과 비교

직전 결과인 [`napi.json`](../benchmark-results/napi.json)과 이번 결과의
1,000,000 package 주요 항목을 비교했다.

| 항목 | 직전 | 재측정 | 변화 |
|---|---:|---:|---:|
| object 반환 | 494.415 | 481.782 | -2.6% |
| JSON 반환 + parse | 323.734 | 322.916 | -0.3% |
| compact 반환 + view | 98.264 | 98.805 | +0.6% |
| bulk `sumSizes` | 8.090 | 7.943 | -1.8% |
| name decode get-loop | 130.249 | 128.804 | -1.1% |
| name byte length | 9.067 | 8.925 | -1.6% |
| name materialize | 108.965 | 107.630 | -1.2% |
| optimized `toObject()` | 238.868 | 240.450 | +0.7% |

핵심 Rust → JS 지표는 같은 범위에서 재현됐다. mutable pipeline과 config
single-call은 짧은 측정 및 런타임 상태의 영향을 더 크게 받으므로 절대값보다
동일 행의 상대 비교를 우선한다.

## 판단

- H1: v2 sync temp single-call에서 성립한다. owned v2는 small과
  default-heavy를 제외하면 성립한다.
- H2: partial access, numeric bulk, string metadata 경로에서 성립한다.
  full string decode에서는 성립하지 않는다.
- H3: numeric, string, fixed-list mutation 전체 pipeline에서 성립한다.
- H4: N-API, Bun FFI, WASM이 동일 layout과 checksum으로 동작한다.
- H5: compiled buffer와 `ConfigHandle` 재사용 경로의 이점은 원시 결과에서
  유지됐다.

## 실행 및 검증

```sh
pnpm run build
pnpm test
node --max-old-space-size=4096 --expose-gc packages/bench/src/napi.bench.js \
  --output=benchmark-results/napi-2026-06-30-rerun.json
bun packages/bench/src/bun-ffi.bench.js \
  --output=benchmark-results/bun-ffi-2026-06-30-rerun.json
node packages/bench/src/wasm.bench.js \
  --output=benchmark-results/wasm-2026-06-30-rerun.json
```

Rust workspace test 4개와 JS test 10개가 모두 통과했다.
