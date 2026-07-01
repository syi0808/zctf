# Direct StringRef layout 적용 결과

## 적용 범위

- normal layout은 기존 16-byte `StringId` record와 string table을 유지한다.
- string-heavy layout은 24-byte record에 name/version의 absolute offset과 length를
  직접 저장한다.
- direct layout의 `name`/`version` getter는 record에서 range를 읽고
  `Buffer.toString("latin1", start, end)`를 호출한다.
- `materializeNames()`와 name/version 전체 scan인 `materializeStrings()`도 같은
  direct decode 경로를 사용한다.
- direct layout은 read-only ASCII/Latin-1 snapshot 용도다. UTF-8 일반 문자열,
  mutation, string deduplication이 필요한 경우 normal layout을 사용한다.

## 측정 조건

```text
platform: darwin-arm64
Node.js: v24.11.0
단위: ms
통계: 5회 warm-up 후 5~7회 측정 median
```

원본 결과는
`benchmark-results/napi-2026-07-01-direct-string-ref.json`에 저장했다.
TextDecoder 기준선과 Buffer.toString 경로는 같은 프로세스에서 연속 측정했다.

## 결과

### name 전체 materialization

| packages | direct TextDecoder | direct Buffer.toString | 개선 | normal StringId | direct vs normal |
|---:|---:|---:|---:|---:|---:|
| 1,000 | 0.170 | 0.102 | 40.0% | 0.055 | -85.5% |
| 10,000 | 0.737 | 0.345 | 53.2% | 0.610 | 43.4% |
| 100,000 | 8.932 | 4.368 | 51.1% | 5.576 | 21.7% |
| 1,000,000 | 96.283 | 39.650 | 58.8% | 49.253 | 19.5% |

1,000건에서는 고정 비용과 sub-millisecond 편차가 지배적이다. 10,000건 이상에서는
direct range와 `Buffer.toString` 조합이 normal StringId 경로보다 빠르다.

### getter 기반 name scan

| packages | normal StringId | DirectStringRef | 개선 |
|---:|---:|---:|---:|
| 1,000 | 0.185 | 0.095 | 48.6% |
| 10,000 | 1.450 | 0.514 | 64.6% |
| 100,000 | 10.243 | 4.744 | 53.7% |
| 1,000,000 | 104.288 | 41.196 | 60.5% |

이 케이스가 record getter에서 string id read, table lookup, entry offset 계산을 제거한
효과를 직접 측정한다.

### name + version 전체 materialization

| packages | normal StringId | DirectStringRef | 개선 |
|---:|---:|---:|---:|
| 1,000 | 0.185 | 0.190 | -2.7% |
| 10,000 | 1.085 | 0.727 | 33.0% |
| 100,000 | 11.118 | 7.984 | 28.2% |
| 1,000,000 | 100.546 | 73.737 | 26.7% |

## 저장 크기

| packages | compact normal | direct | direct 변화 |
|---:|---:|---:|---:|
| 1,000 | 48,902 | 40,822 | -16.5% |
| 10,000 | 498,002 | 417,922 | -16.1% |
| 100,000 | 5,079,002 | 4,278,922 | -15.8% |
| 1,000,000 | 51,789,002 | 43,788,922 | -15.4% |

이 fixture는 모든 name/version이 고유하므로 normal layout의 string table이 package당
16 bytes를 추가한다. 따라서 direct record 자체는 8 bytes 크지만 전체 snapshot은 더
작다. 문자열 중복률이 높으면 normal layout의 deduplication 결과를 별도로 측정해야 한다.

## 결론

DirectStringRef는 ASCII/Latin-1 문자열을 실제 JS string으로 대량 반환하는 read-only
snapshot에 유효하다. 1M 기준 getter scan은 60.5%, name materialization은 normal
layout 대비 19.5%, name+version materialization은 26.7% 감소했다.

기본 포맷을 교체하지는 않는다. normal layout은 UTF-8, mutation, deduplication,
partial/lazy access에 사용하고, direct layout은 명시적인 string-heavy export 경로로
선택한다.
