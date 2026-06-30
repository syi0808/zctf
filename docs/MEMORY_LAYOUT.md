# Memory layout

모든 수치는 little-endian이다. report header는 64 bytes다.

| Header offset | Type | 의미 |
|---:|---|---|
| 0 | u32 | magic `0x4654435a` |
| 4 | u32 | version |
| 20 | u32 | root offset |
| 28 | u32 | list region offset |
| 32 | u32 | string table offset |
| 36 | u32 | string heap offset |
| 40 | u32 | absolute heap cursor |
| 44 | u32 | heap capacity |
| 48 | u32 | epoch (reserved) |
| 52 | u32 | string count |
| 56 | u32 | string capacity |
| 60 | u32 | total buffer length |

`BenchReport` root는 offset 64에서 시작한다.

| 상대 offset | Type | field |
|---:|---|---|
| 0 | u32 | package count |
| 8 | u64 | total size |
| 16 | f64 | duration ms |
| 24 | u32 | package list ref |

package record는 16 bytes이며 `name StringRef`, `version StringRef`, `size u32`,
`dependency_count u32` 순서다. StringRef는 string table의 ID이고 각 table entry는
heap-relative `offset u32 + len u32`다.

FixedList header는 `len`, `capacity`, `item_size`, `items_offset` 네 u32다.
report는 초기 count의 두 배를 record capacity로 잡는다. string table은 초기 문자열
2개와 name mutation 또는 추가 package push에 필요한 slot을 미리 잡는다.

config buffer v2는 별도 magic `0x4346435a`를 사용한다. 32-byte header 뒤
`TransformConfig` root는 다음 hot/cold split을 사용한다.

| 상대 offset | Type | field |
|---:|---|---|
| 0 | u32 | presence bitmap |
| 4 | u32 | packed bool flags |
| 8 | f64 | flattened `svgoConfig.floatPrecision` |
| 16 | u32 | plugin token list ref |
| 20 | u32 | specialized SVGO plugin list ref |
| 24 | u8 | JSX runtime enum ID |
| 25 | u8 | export type enum ID |

`multipass`는 root flags로 flatten된다. plugin record는 name token, packed
active/currentColor flags, known plugin kind를 담는 고정 8 bytes다. known name token은
u32 high bit와 ID를 사용하고 unknown name만 string table/heap에 기록한다. Rust는
header, list, token, string range를 한 번 검증한 뒤 fixed offset reader를 사용한다.
