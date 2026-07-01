# 1차 제품 성능 개선 결과

측정 환경: Apple arm64, Node.js v24.11.0, Vitest 4.1.9. 정식 결과는
[`latest.json`](latest.json)에 있다.

## 적용 내용

- `encode_owned()`의 measure/write two-pass와 정확한 단일 output 할당
- 문자열별 `Vec<u8>` 제거
- `string(direct)` absolute offset/length wire layout 구현
- JS document 생성 시 string table 전체 순회 제거
- generated `readObject()`와 list bulk materialization
- object baseline의 불필요한 중간 `TransformResult`/`Vec` 제거
- return/view/full-materialization 단계 분리

## End-to-end 결과

| warnings | lazy object ops/s | lazy zctf ops/s | zctf 개선 | full object ops/s | zctf `toObject` ops/s | zctf 개선 |
|---:|---:|---:|---:|---:|---:|---:|
| 0 | 2,109,399 | 1,114,472 | 0.53x | 2,078,948 | 1,130,330 | 0.54x |
| 3 | 656,101 | 902,030 | 1.37x | 658,696 | 742,358 | 1.13x |
| 20 | 140,186 | 467,877 | 3.34x | 139,241 | 260,728 | 1.87x |
| 100 | 31,317 | 153,269 | 4.89x | 31,176 | 68,834 | 2.21x |
| 1,000 | 3,178 | 16,942 | 5.33x | 3,066 | 7,018 | 2.29x |
| 10,000 | 314 | 1,772 | 5.64x | 313 | 704 | 2.25x |

0-warning payload에서는 fixed document/Buffer 비용 때문에 object가 빠르다.
3개부터 lazy zctf가 앞서고, 100개 이상에서 4.5~5.6배 차이가 난다. 모든
문자열과 JS object를 생성하는 `toObject()`도 20개 이상에서 1.8~2.3배 빠르다.

## 개선 전 대비

동일한 20-warning 정식 benchmark 기준:

| 경로 | 개선 전 | 개선 후 | 변화 |
|---|---:|---:|---:|
| macro/view | 292,114 ops/s | 467,877 ops/s | +60.2% |
| `toObject()` | 190,021 ops/s | 260,728 ops/s | +37.2% |
| object 대비 lazy | 2.04x | 3.34x | +63.7% |
| object 대비 full | 1.43x | 1.87x | +30.8% |

prebuilt 20-warning document의 view 생성은 약 0.60µs에서 0.12µs로 줄었다.
string entry 검증은 각 string 접근 시점에 수행한다.
