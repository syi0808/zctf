# PoC 벤치마크 케이스

이 문서는 zctf PoC가 어떤 성능 가설을 어떤 workload로 검증하는지 정의한다.
측정 결과는 [`BENCHMARKS.md`](BENCHMARKS.md), 원시 데이터는
`benchmark-results/*.json`에 기록한다. 이 문서의 역할은 결과 보고가 아니라
재현 가능한 케이스와 판정 기준을 고정하는 것이다.

## 1. 검증 대상

| 가설 | 검증할 내용 | 주 벤치마크 |
|---|---|---|
| H1 | JS config를 binary로 compile한 뒤 Rust가 읽는 경로가 object/JSON 입력보다 빠른가 | C3 |
| H2 | Rust 결과를 JS object로 만들지 않고 binary lazy view로 반환하는 경로가 유리한가 | R1, R3, R4 |
| H3 | JS가 shared buffer를 수정하고 Rust가 읽는 전체 pipeline이 object/JSON보다 빠른가 | M1~M4 |
| H4 | N-API, Bun FFI, WASM이 같은 layout과 JS view를 재사용하는가 | T1 |
| H5 | 같은 config를 반복 사용할 때 compile 결과를 재사용하는 것이 유리한가 | C5 |

H1~H3과 H5는 성능 가설이다. H4는 현재 하네스에서 성능 순위가 아니라 transport
호환성만 검증한다.

## 2. 공통 측정 규약

### 2.1 실행 조건

- release native/FFI/WASM artifact를 사용한다.
- N-API 정식 실행은 package `1,000`, `10,000`, `100,000`, `1,000,000`개와
  mutation `10,000`, `100,000`개를 측정한다.
- 각 N-API sample은 5회 warm-up 후 5~7회 측정하고 median을 사용한다.
- sample 직전에 `global.gc()`를 요청한다. 따라서 Node는 `--expose-gc`로 실행한다.
- config처럼 호출 하나가 짧은 케이스는 5,000~20,000회를 한 sample로 묶고
  호출당 ms로 환산한다.
- 비교는 다른 문서의 절대 시간보다 **같은 실행, 같은 행의 기준군**을 우선한다.
- machine, OS, CPU architecture, Node/Bun/Rust 버전, timestamp, build mode를 결과와
  함께 남긴다.

`--quick`은 fixture와 실행 경로 확인용이다. 입력 크기와 반복 수가 다르므로 성능
가설의 통과 여부를 판단하지 않는다.

### 2.2 판정 규칙

비교 비율은 다음처럼 계산한다.

```txt
improvement = baseline time / candidate time
```

- `PASS`: improvement가 `1.10x` 이상이고, 독립된 정식 실행 2회에서 방향이 같다.
- `NEUTRAL`: `0.90x` 이상 `1.10x` 미만이다. 동급으로 판단한다.
- `FAIL`: `0.90x` 미만이거나, 두 정식 실행에서 개선 방향이 재현되지 않는다.
- correctness invariant가 다르면 시간과 무관하게 `INVALID`다.

10% 경계는 현재 하네스가 confidence interval을 계산하지 않는 점을 고려한 실용적인
noise margin이다. 통계적 유의성을 뜻하지 않는다. 개별 케이스에서 별도 기준을
명시하면 그 기준이 우선한다.

### 2.3 실행 전 correctness gate

성능 측정 전 다음 명령이 성공해야 한다.

```sh
pnpm run build
pnpm test
```

최소 invariant는 다음과 같다.

- 모든 representation의 package count와 aggregate 결과가 같다.
- sequential/parallel report encoder의 bytes가 같다.
- compact/mutable/실험 layout의 aggregate와 materialized name이 같다.
- config fixture 7종을 Rust가 읽을 수 있다.
- mutable update와 list push를 Rust consume 결과가 반영한다.
- transport별 length, first name, checksum이 같다.

## 3. 입력 fixture

### 3.1 Report

`BenchReport`는 package마다 `name`, `version`, `size`, `dependencyCount`를 가진다.

| 용도 | 정식 크기 |
|---|---|
| 반환, 순회, layout | 1K, 10K, 100K, 1M packages |
| mutation | 10K, 100K packages |
| Bun FFI/WASM smoke | 1K, 10K, 100K packages |

1M은 주 판정 크기이고 나머지는 scaling과 threshold 확인용이다.

### 3.2 Config

| shape | 의도 |
|---|---|
| `small` | 고정 scalar 위주의 최소 입력 |
| `medium` | 일반적인 nested config와 소수 plugin |
| `large` | plugin/string 항목이 많은 큰 입력 |
| `stringHeavy` | unknown ASCII string write 비용 |
| `pluginHeavy` | known-name ID fast path |
| `unicodeHeavy` | UTF-8 `encodeInto` fallback |
| `defaultHeavy` | default 생략과 packed flag fast path |

H1의 주 판정 대상은 `medium`, `large`, `stringHeavy`, `pluginHeavy`,
`unicodeHeavy`다. `small`과 `defaultHeavy`는 고정 overhead 회귀를 찾는 진단
케이스다.

## 4. Rust → JS result

### R1. 반환 representation 생성

- **목적:** H2의 lazy return 가설 검증.
- **측정 구간:** Rust report 생성부터 JS가 package count를 읽을 수 있는 첫
  representation을 얻을 때까지.
- **기준군:** N-API object 반환, JSON 문자열 반환 + `JSON.parse`.
- **후보:** mutable zctf buffer + view, compact zctf buffer + view.
- **원시 필드:** `rustToJs[].objectReturn`, `jsonReturnAndParse`,
  `zctfReturnAndView.{mutable,compact}`.
- **주 판정:** 100K와 1M에서 compact 후보가 두 기준군 각각보다 `PASS`.
- **검증값:** package count가 입력 count와 같아야 한다.

이 값에는 Rust-side report encode/generation과 boundary conversion이 함께 들어간다.
순수 transport overhead로 해석하지 않는다.

### R2. 이미 생성된 representation의 첫 접근

- **목적:** lazy view를 얻은 뒤 첫 string 접근 비용 진단.
- **비교:** object property access, JSON parse + property access, zctf view access.
- **원시 필드:** `rustToJs[].firstAccess`.
- **판정:** 진단 전용.

세 경로의 측정 구간이 동일하지 않다. object와 zctf는 이미 생성된 값에 접근하지만
JSON만 parse를 포함한다. 따라서 R2 단독으로 H2를 통과시켜서는 안 된다.

### R3. Numeric full traversal

- **목적:** 모든 `size`를 읽는 workload에서 lazy view overhead와 bulk API 효과 검증.
- **기준군:** object loop, JSON parse + loop.
- **후보:** zctf get-loop, cursor, raw loop, JS bulk, Rust native aggregate.
- **원시 필드:** `rustToJs[].sumSizes`.
- **주 판정:** 1M에서 `zctfBulk`가 object loop 대비 `NEUTRAL` 이상이어야 한다.
- **최적화 판정:** `zctfBulk`가 `zctfGetLoop` 대비 `PASS`.
- **검증값:** 모든 경로의 합이 같아야 한다.

`zctfNative`는 aggregate를 Rust에서 수행하므로 JS traversal과 동일한 작업이 아니다.
native offload의 별도 상한선으로만 본다.

### R4. String workload 분리

문자열은 요구 결과에 따라 별도 케이스로 판단한다.

| 하위 케이스 | 기준군 | 후보 | 판정 |
|---|---|---|---|
| R4a 전체 JS string decode | object name-length loop | zctf get/bulk decode | 결과 기록, H2 필수 통과 대상 아님 |
| R4b byte length aggregate | zctf decode loop | zctf byte-length bulk | 1M에서 `PASS` |
| R4c prefix filter | object string filter, JSON parse + filter | zctf byte filter | object 대비 `NEUTRAL` 이상 |
| R4d warm cache | string cache disabled | cache enabled warm | `PASS` |

원시 필드는 `rustToJs[].names`와 `cacheStrings`다. R4b와 R4c는 JS string
materialization이 필요 없는 실제 workload를 나타낸다. 모든 결과의 length 또는 match
count가 같아야 한다.

### R5. Full materialization escape hatch

- **목적:** lazy representation을 명시적으로 object/array로 바꾸는 경로의 회귀 확인.
- **비교:** legacy `toObject()`와 optimized `toObject()`, get-loop name decode와
  `materializeNames()`.
- **원시 필드:** `rustToJs[].toObject`, `names.zctfMaterializeArray`.
- **판정:** optimized `toObject()`가 legacy 대비 `PASS`.

이미 materialized된 object 접근보다 빨라야 한다는 요구는 없다. 이 케이스는 H2의
주 성공 조건이 아니라 escape hatch 품질 조건이다.

### R6. Storage와 layout variant

- **목적:** mutable reserve 비용을 readonly snapshot과 분리하고 layout trade-off 확인.
- **후보:** mutable/compact AoS, direct StringRef, SoA, AoS + sidecar.
- **원시 필드:** `rustToJs[].storageBytes`, `layoutVariants`.
- **판정:** compact AoS bytes가 mutable AoS보다 작아야 하며, compact
  `sumSizes`가 mutable 대비 `NEUTRAL` 이상이어야 한다.
- **검증값:** aggregate와 materialized names가 AoS 기준값과 같아야 한다.

실험 layout이 한 workload에서 빨라도 모든 workload의 기본 layout 승격 근거로
사용하지 않는다.

### R7. Report generation 전략

- **목적:** report encoder의 sequential/parallel-local/automatic 선택 회귀 확인.
- **layout:** compact AoS는 sequential/parallel-local/automatic을 비교한다.
  direct StringRef와 SoA는 sequential/parallel-local만 비교한다.
- **원시 필드:** `reportGeneration`.
- **판정:** compact AoS의 automatic은 같은 count에서 더 빠른 명시적 전략 대비
  `NEUTRAL` 이상.
- **검증값:** sequential과 parallel 결과 bytes가 같아야 한다.

작은 입력에서 parallel이 느린 것은 실패가 아니다. automatic threshold 선택이
판정 대상이며, automatic 경로가 없는 실험 layout은 진단값으로만 기록한다.

## 5. JS → Rust config

### C1. Compile only

- **목적:** JS writer 자체 비용과 최적화별 buffer 크기 확인.
- **비교:** v1 baseline, v2 owned, v2 reusable temp writer.
- **원시 필드:** `configInput[].compileOnly`, `bytes`, `strings`.
- **판정:** 주 config shape에서 temp writer가 v1 대비 `PASS`; owned는 별도 기록.
- **검증값:** Rust consume이 성공하고 estimated capacity를 넘지 않아야 한다.

### C2. Rust read only

- **목적:** compile 비용을 제외한 reader validation/offset access 비용 측정.
- **비교:** v1 read, v2 validated read, hot field 100회 direct read, local struct 승격.
- **원시 필드:** `configInput[].rustReadOnly`.
- **판정:** v2 validated read는 호출당 `0.001ms` 이하, local 승격은 direct 100회
  read 대비 `PASS`.

v1/v2는 validation 범위가 다르므로 reader 시간만으로 format 전체의 우열을
결론내리지 않는다.

### C3. Single-call compile + Rust read

- **목적:** H1의 주 검증.
- **측정 구간:** JS config 입력부터 Rust checksum consume 완료까지.
- **기준군:** direct N-API object read, JSON stringify + Rust parse, v1 compile + read.
- **후보:** v2 owned compile + read, v2 sync temp compile + read.
- **원시 필드:** `configInput[].single`.
- **판정:** 주 config shape 각각에서 sync temp가 가장 빠른 일반 입력 기준군
  (`napiObject`, `jsonStringifyAndRustParse`) 대비 `PASS`이고 v1 대비 `PASS`.
- **검증값:** 모든 경로가 같은 config 의미를 consume해야 한다.

owned buffer는 비동기 보관이 필요한 API의 비용이고 sync temp는 호출 범위를 벗어나지
않는 buffer의 비용이다. 수명 계약이 다르므로 서로 대체 가능한 API로 간주하지 않는다.

### C4. Full transform boundary

- **목적:** config 입력과 고정 SVG input이 함께 boundary를 통과하는 경로의 회귀 확인.
- **비교:** C3와 같은 object/JSON/v1/v2 경로.
- **원시 필드:** `configInput[].fullTransform`.
- **판정:** C3와 같은 기준을 적용하되 H1의 보조 근거로 사용.

현재 transform body는 input length와 config checksum을 결합하는 최소 구현이다.
실제 SVG transform 처리량으로 해석하지 않는다.

### C5. Config reuse

- **목적:** H5 검증. config 1개를 compile한 뒤 많은 호출에서 재사용한다.
- **기준군:** 호출마다 N-API object read, 미리 만든 JSON의 Rust parse.
- **후보:** 미리 compile한 buffer read.
- **원시 필드:** `configInput[].reuse`.
- **주 판정:** 주 config shape에서 `compiledRead`가 두 기준군 각각 대비 `PASS`.

`configHandle`은 `<svg/>` transform까지 포함하고 다른 세 경로는 config consume만
수행하므로 작업량이 같지 않다. handle lifecycle/integration 진단값으로 기록하되
H5의 직접 성능 판정에는 사용하지 않는다.

## 6. Mutable JS → Rust pipeline

모든 mutation 케이스는 **JS의 전체 update loop + Rust consume**을 함께 측정한다.
100K mutation이 주 판정 크기다.

| ID | workload | 기준군 | 후보 | 원시 필드 |
|---|---|---|---|---|
| M1 | 모든 `size` overwrite | object update + N-API consume, object update + JSON consume | shared buffer update + consume | `mutablePipeline[].numeric` |
| M2 | 모든 `name` append-only 변경 | object update + JSON consume | string heap append + consume | `mutablePipeline[].string` |
| M3 | 모든 `dependencyCount` overwrite | object update + N-API consume | shared buffer update + consume | `mutablePipeline[].dependencyCount` |
| M4 | package count만큼 list push | object push + N-API consume | fixed-capacity list push + consume | `mutablePipeline[].listPush` |

각 케이스에서 후보가 모든 명시된 기준군 대비 `PASS`해야 H3을 지지한다. 결과 checksum,
최종 list length, 변경된 값이 같아야 한다. M2는 추가로 `heapGrowthBytes`와
`stringEntriesAdded`를 기록해 append-only 비용을 노출한다.

## 7. Transport compatibility

### T1. Bun FFI/WASM smoke

- **목적:** H4 검증.
- **측정 구간:** report 생성, ptr/length로 view 획득, first name 접근, Rust consume.
- **입력:** 1K, 10K, 100K packages.
- **원시 필드:** backend별 `results[].elapsedMs`, `length`, `first`, `checksum`.
- **통과 조건:** 같은 count에서 buffer length, first name, checksum이 backend 간
  일치하고 같은 `BenchReportView`가 동작한다.

현재 T1은 warm-up과 반복이 없는 single-shot이다. `elapsedMs`는 smoke 회귀 감지용이며
N-API/Bun FFI/WASM 성능 순위를 정하는 자료가 아니다.

## 8. 실행 명령

```sh
# 빠른 경로 확인
pnpm run bench:quick

# 정식 N-API 결과
node --max-old-space-size=4096 --expose-gc packages/bench/src/napi.bench.js \
  --output=benchmark-results/napi-YYYY-MM-DD.json

# transport smoke
bun packages/bench/src/bun-ffi.bench.js \
  --output=benchmark-results/bun-ffi-YYYY-MM-DD.json
node packages/bench/src/wasm.bench.js \
  --output=benchmark-results/wasm-YYYY-MM-DD.json
```

정식 결과 문서에는 최소한 다음을 포함한다.

1. 환경과 원시 결과 파일
2. 케이스 ID별 기준군/후보 median
3. improvement ratio와 `PASS`/`NEUTRAL`/`FAIL`/`INVALID`
4. correctness invariant 확인 결과
5. 직전 정식 실행 대비 변화와 해석

## 9. 현재 하네스가 답하지 않는 질문

다음 항목은 별도 하네스 없이는 결론내리지 않는다.

- 실제 SVG 변환 알고리즘을 포함한 end-to-end 처리량
- peak RSS, JS heap allocation count, GC pause, native allocator allocation count
- multi-thread contention과 concurrent mutation safety
- N-API/Bun FFI/WASM의 순수 transport overhead 및 통계적 성능 순위
- p95/p99 latency와 confidence interval
- schema evolution과 malformed/untrusted buffer validation 비용

이 범위를 벗어난 주장을 하려면 기존 케이스에 결과 필드를 추가하는 대신 독립된
벤치마크 케이스와 correctness invariant를 먼저 정의한다.
