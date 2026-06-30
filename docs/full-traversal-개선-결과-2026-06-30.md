# Rust → JS full traversal 개선 결과

측정일: 2026-06-30 22:50 KST

환경: Apple arm64, Node v24.5.0, release N-API build

방법: 5회 warm-up 후 5회 측정의 median. 단위는 ms이며 명시적 GC를
sample 사이에 요청했다. 원시 결과는
[`benchmark-results/napi.json`](../benchmark-results/napi.json)에 있다.

## 결론

설계서의 M1~M6를 모두 구현했다. 1,000,000 package에서 generated numeric
bulk API는 get-loop와 object 순회를 모두 앞섰고, string metadata 전용 API는
decode get-loop보다 14.37배 빨랐다. compact buffer는 크기를 53.8% 줄이면서
반환 및 view 생성 시간도 유지했다.

| 1,000,000 packages | 결과 | 비교 |
|---|---:|---:|
| object `sumSizes` | 13.112 | - |
| zctf get-loop `sumSizes` | 12.864 | - |
| zctf cursor `sumSizes` | 10.844 | get-loop 대비 1.19x |
| zctf raw `sumSizes` | 10.922 | get-loop 대비 1.18x |
| zctf bulk `sumSizes` | 8.090 | get-loop 대비 1.59x, object 대비 1.62x |
| name get-loop decode length | 130.249 | - |
| name bulk decode length | 87.319 | get-loop 대비 1.49x |
| name byte length metadata | 9.067 | get-loop decode 대비 14.37x |
| name materialize array | 108.965 | get-loop decode보다도 1.20x 빠름 |
| legacy `toObject` | 342.819 | - |
| optimized `toObject` | 238.868 | legacy 대비 1.44x |

warm string cache는 87.401ms에서 66.753ms로 1.31배 개선됐다. ASCII prefix
filter는 object string 경로 29.413ms, zctf byte 경로 22.293ms로 byte 경로가
1.32배 빨랐다.

## Compact buffer

| 1,000,000 packages | mutable | compact | 변화 |
|---|---:|---:|---:|
| buffer bytes | 112,000,176 | 51,789,002 | 53.8% 감소 |
| Rust 반환 + view | 98.700 | 98.264 | 0.4% 개선 |
| bulk `sumSizes` | 8.055 | 8.090 | 사실상 동일 |

compact mode는 package/string capacity와 heap을 초기 데이터에 맞게 잡는다.
기존 mutable mode와 `makeReportBuffer()` 호환 export는 그대로 유지했다.

## 레이아웃 실험

M6의 세 변형은 별도 read-only buffer와 view로 구현했다. 수치는 같은
1,000,000 package workload다.

| layout | bytes | `sumSizes` | name byte lengths | materialize names |
|---|---:|---:|---:|---:|
| compact AoS | 51,789,002 | 8.090 | 9.067 | 108.965 |
| direct StringRef | 43,788,922 | 10.380 | 10.329 | 107.243 |
| SoA | 29,888,922 | 10.114 | 10.371 | - |
| AoS + sidecar | 59,789,002 | 10.318 | - | - |

direct StringRef는 compact AoS보다 15.4% 작고 name materialization이 1.6%
빨랐지만 다른 순회는 느렸다. SoA는 가장 작지만 numeric scan 이득이 없었고,
sidecar도 중복 저장 비용만 늘었다. 따라서 세 변형은 실험 구현으로 유지하되
기본 layout은 compact AoS로 둔다.

## 적용 범위

| milestone | 적용 내용 |
|---|---|
| M1 | `FixedListView` length/capacity cache, push 시 cache 갱신, local length benchmark |
| M2 | `sumSizes`, `sumDependencyCounts`, `sumNameByteLengths`, `materializeNames`, `toObjectArray` |
| M3 | `rangeUnchecked`, `getUnchecked`, `byteLengthUnchecked`, string cache 비교 |
| M4 | get-loop/cursor/raw/bulk, decode/metadata/materialize/prefix, legacy/optimized 분리 |
| M5 | mutable/compact Rust 생성기와 N-API export, 크기·반환·순회 비교 |
| M6 | direct StringRef, SoA, AoS+sidecar buffer/view 및 레이아웃 비교 |

`forEachRaw`, ephemeral cursor, byte prefix filter도 추가했다. N-API Buffer의
base offset이 4-byte aligned가 아닐 수 있으므로 SoA/sidecar column은 가능한
경우 `Uint32Array`, 그 외에는 zero-copy DataView column을 사용한다.

## 검증

다음 검증을 통과했다.

```sh
cargo test --workspace
node --test packages/*/test/*.test.js
node --max-old-space-size=4096 --expose-gc packages/bench/src/napi.bench.js
```

Rust workspace test 4개와 JS test 10개가 모두 통과했다. JS test는 lazy/raw/
cursor/bulk 결과 동일성, cached length push, compact 크기, 네 layout의
numeric/string 결과 동일성을 포함한다.
