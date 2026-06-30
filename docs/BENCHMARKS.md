# Benchmark 결과

Rust → JS full traversal 개선 후 최신 측정과 M1~M6 적용 내역은
[`full-traversal-개선-결과-2026-06-30.md`](full-traversal-개선-결과-2026-06-30.md)에
정리했다. 아래 Rust → JS 절은 비교를 위한 개선 전 baseline이며, 이후 절은 기존
config/mutable/transport 측정 기록이다.

측정일: 2026-06-30 18:20 KST

환경: Apple arm64, Node v24.11.0, Bun 1.3.11, Rust 1.93.1

단위: ms, warm-up 후 median. 실행 중 명시적으로 GC를 요청했으므로 절대값보다
동일 행의 상대 비교를 우선한다.

최신 원시 데이터: `benchmark-results/napi.json`. 기존 backend 측정은
`bun-ffi.json`, `wasm.json`에 있다.

실행 시간이 증가한 경로만 분리한 목록은
[`실행시간-회귀-2026-06-30.md`](실행시간-회귀-2026-06-30.md)에 있다.

## Rust → JS result (full traversal 개선 전 baseline)

반환값 생성과 JS에서 첫 representation을 얻는 전체 시간이다.

| packages | N-API object | JSON + parse | zctf Buffer + view | object 대비 |
|---:|---:|---:|---:|---:|
| 1,000 | 0.373 | 0.255 | 0.116 | 3.2x |
| 10,000 | 3.726 | 2.456 | 0.906 | 4.1x |
| 100,000 | 37.702 | 24.665 | 9.083 | 4.2x |
| 1,000,000 | 430.860 | 271.153 | 90.758 | 4.7x |

1,000,000개에서 기존 JSON 문자열의 첫 package를 읽으려면 parse 포함
162.866ms, 이미 얻은 zctf view의 first access는 0.020ms였다.

전체 순회 결과는 field 특성에 따라 다르다.

| 1,000,000 packages | object | zctf |
|---|---:|---:|
| 모든 `size` 합산 | 8.935 | 9.711 |
| 모든 `name` 읽기 | 9.116 | 77.831 |
| zctf `toObject()` | - | 211.143 |

숫자 전체 순회는 경쟁 가능하지만 문자열 전체 순회는 zctf가 8.5배 느리다.
TextDecoder 호출 비용 때문이다. zctf의 이점은 materialization을 피하는 partial
access에서 가장 크고, 모든 문자열을 JS string으로 바꾸는 workload에는 맞지 않는다.

## JS → Rust config single-call 최적화

호출 하나가 너무 짧아 timer 해상도에 묻히는 문제를 피하기 위해 shape별 5,000~20,000
호출을 한 sample로 측정하고 호출 수로 나눴다. `baseline`은 변경 전 v1 compiler,
`optimized`는 매번 결과 buffer를 소유하는 v2 compiler, `sync temp`는 compile 직후
동기 native 호출이 끝날 때까지만 임시 buffer를 빌리는 v2 경로다.

### Compile + Rust read

| shape | N-API object | JSON stringify+parse | v1 baseline | v2 owned | v2 sync temp | v1 대비 | object 대비 |
|---|---:|---:|---:|---:|---:|---:|---:|
| small | 0.000630 | 0.000513 | 0.000306 | 0.000357 | 0.000099 | 3.09x | 6.36x |
| medium | 0.004884 | 0.004743 | 0.004189 | 0.001367 | 0.001054 | 3.97x | 4.63x |
| large | 0.040378 | 0.037564 | 0.039582 | 0.010098 | 0.009256 | 4.28x | 4.36x |
| string-heavy | 0.046237 | 0.048008 | 0.045680 | 0.021878 | 0.020446 | 2.23x | 2.26x |
| plugin-heavy | 0.039711 | 0.038035 | 0.041359 | 0.004929 | 0.004658 | 8.88x | 8.53x |
| unicode-heavy | 0.048762 | 0.054126 | 0.046684 | 0.024852 | 0.024152 | 1.93x | 2.02x |
| default-heavy | 0.000657 | 0.000558 | 0.000549 | 0.000418 | 0.000121 | 4.54x | 5.43x |

단위는 모두 호출당 ms다. owned buffer가 필요한 small에서는 v2가 v1보다 17% 느리지만,
문서가 대상으로 둔 compile 후 즉시 native call하는 sync-only path에서는 모든 shape가
개선됐다. medium 이상에서는 owned v2도 v1과 두 baseline보다 빠르다.

### Compile-only와 buffer

| shape | v1 compile | v2 owned | v2 temp | v1 bytes | v2 bytes | string path |
|---|---:|---:|---:|---:|---:|---|
| medium | 0.004095 | 0.001272 | 0.000980 | 682 | 586 | ASCII 20 |
| large | 0.040086 | 0.010067 | 0.009241 | 5,992 | 5,176 | ASCII 200 |
| string-heavy | 0.044969 | 0.021475 | 0.020023 | 15,392 | 14,576 | ASCII 200 |
| plugin-heavy | 0.040797 | 0.004657 | 0.004347 | 5,512 | 1,296 | known ID 200 |
| unicode-heavy | 0.046490 | 0.024743 | 0.023752 | 9,992 | 9,176 | encodeInto 200 |

known name ID화가 적용된 plugin-heavy는 string heap write가 0회가 되고 buffer가 76.5%
작아졌다. unknown ASCII는 직접 기록하며, Unicode 200개는 모두 `encodeInto` fallback을
사용했다.

### Full transform boundary

고정 SVG input을 함께 N-API boundary로 넘긴 전체 경로도 별도로 측정했다.

| shape | N-API object | JSON stringify+parse | v1 compile | v2 owned | v2 sync temp |
|---|---:|---:|---:|---:|---:|
| medium | 0.005057 | 0.004792 | 0.004253 | 0.001432 | 0.001122 |
| large | 0.039420 | 0.037318 | 0.039612 | 0.010167 | 0.009326 |
| string-heavy | 0.046444 | 0.048568 | 0.045873 | 0.021876 | 0.020437 |
| plugin-heavy | 0.039889 | 0.037943 | 0.040517 | 0.004958 | 0.004614 |

이 PoC의 transform body는 input length와 config checksum을 결합하는 최소 구현이므로,
표는 실제 SVG 변환 알고리즘 성능이 아니라 compile과 JS↔Rust boundary를 포함한 경로의
회귀 지표다.

### Rust read와 local struct 승격

| shape | v1 read | v2 validated read | hot field 100회 direct | local 승격 후 100회 |
|---|---:|---:|---:|---:|
| small | 0.000035 | 0.000037 | 0.000218 | 0.000076 |
| medium | 0.000051 | 0.000062 | 0.000239 | 0.000092 |
| large | 0.000131 | 0.000322 | 0.000379 | 0.000224 |
| plugin-heavy | 0.000131 | 0.000239 | 0.000327 | 0.000184 |

v2는 모든 offset과 token을 한 번 검증한 뒤 schema-specific unchecked read를 수행한다.
plugin 수에 비례하는 검증 때문에 v1보다 느리지만 최대 0.000322ms로 성공 기준인
0.001ms 미만이다. config hot scalar를 100회 읽는 경우 local struct 승격은 direct
offset read보다 1.5~2.9배 빠르다.

### 개선안 4.1~4.15 적용 범위

| 항목 | 적용 내용 |
|---|---|
| 4.1 schema compiler | generated offset 상수와 고정 field write를 사용하는 전용 compiler |
| 4.2 plain object | config/nested/plugin class instance를 거부하는 direct property fast path |
| 4.3 default skip | false, automatic, default, 0을 presence/write에서 생략 |
| 4.4 enum ID | JSX runtime과 export type을 u8로 기록 |
| 4.5 known name ID | 5개 known plugin/config name을 high-bit u32 token으로 기록 |
| 4.6 ASCII | unknown ASCII를 `charCodeAt`으로 heap에 직접 기록 |
| 4.7 encodeInto | non-ASCII를 preallocated heap에 직접 기록 |
| 4.8 preallocation | 구조 크기와 UTF-8 upper bound를 compile 전에 계산 |
| 4.9 sync temp | reentrancy-safe `withCompiledConfig` 임시 writer |
| 4.10 hot/cold split | root의 scalar hot region과 list/string cold region 분리 |
| 4.11 flattening | multipass/floatPrecision을 root flags/scalar로 승격 |
| 4.12 known params | plugin flag와 kind를 고정 8-byte record로 기록 |
| 4.13 packed flags | root bool 3개와 plugin bool/presence를 bit flags로 통합 |
| 4.14 Rust reader | 전체 구조 검증 후 generated fixed offset unchecked access |
| 4.15 local struct | 반복 hot read용 `TransformConfigLocal` 승격과 비교 벤치 |

## Mutable JS → Rust pipeline

100,000개를 수정/추가한 뒤 Rust가 consume하는 전체 시간:

| case | object/N-API 또는 JSON | zctf | 개선 |
|---|---:|---:|---:|
| size update + N-API consume | 43.682 | 1.793 | 24.4x |
| size update + JSON consume | 41.842 | 1.793 | 23.3x |
| dependency count + N-API consume | 43.490 | 1.861 | 23.4x |
| string update + JSON consume | 82.861 | 31.516 | 2.6x |
| fixed-list push + N-API consume | 127.800 | 46.070 | 2.8x |

numeric과 append-only string/list 모두 전체 pipeline에서는 가설 H3을 지지한다.
100,000 name 변경은 string entry 100,000개와 heap 1,288,890 bytes를 append했다.

## Transport 재사용

100,000 package의 생성, transport view 획득, first string access, Rust consume을 한 번에
수행한 smoke 측정:

| backend | elapsed | bytes | checksum |
|---|---:|---:|---:|
| Bun FFI | 12.540 | 11,200,176 | 85,000,700,000 |
| WASM | 11.642 | 11,200,176 | 85,000,700,000 |

두 backend 모두 N-API와 동일한 `BenchReportView`, buffer length, checksum을 사용한다.
이 값은 single-shot transport 동작 검증용이라 backend 순위의 통계적 근거로 쓰면 안 된다.

## 결론

- H2: partial/lazy access에서 성립. full string materialization에서는 불성립.
- H1: v2 sync temp single-call에서 성립. medium/large/plugin-heavy는 owned v2에서도 성립.
- H5: compiled buffer/ConfigHandle 재사용에서는 큰 폭으로 성립.
- H3: numeric, string, fixed-list mutation 전체 pipeline에서 성립.
- H4: 세 backend에서 동일 layout/runtime/checksum으로 동작함을 확인.
