use rayon::prelude::*;
use zctf_core::{read_u32, write_f64, write_u32, write_u64};

const MAGIC_REPORT: u32 = 0x4654_435a;
const VERSION: u32 = 1;
const HEADER_SIZE: usize = 64;
const ROOT_OFFSET: usize = HEADER_SIZE;
const REPORT_ROOT_SIZE: usize = 32;
const LIST_HEADER_SIZE: usize = 16;
const PACKAGE_SIZE: usize = 16;
const DIRECT_MAGIC: u32 = 0x4452_435a;
const SOA_MAGIC: u32 = 0x5341_435a;

const STRING_SLOTS_PER_PACKAGE: usize = 3;
const HEAP_BYTES_PER_PACKAGE: usize = 48;
const PARALLEL_THRESHOLD: usize = 100_000;
const PARALLEL_CHUNK_SIZE: usize = 32_768;

pub fn make_bench_report(count: u32) -> Vec<u8> {
    make_bench_report_with_capacity(count, false)
}

pub fn make_bench_report_compact(count: u32) -> Vec<u8> {
    if count as usize >= PARALLEL_THRESHOLD {
        make_bench_report_compact_parallel(count)
    } else {
        make_bench_report_compact_sequential(count)
    }
}

fn digit_sum(count: usize) -> usize {
    if count == 0 {
        return 0;
    }
    let mut total = 1; // zero
    let mut start = 1;
    let mut digits = 1;
    while start < count {
        let end = start.saturating_mul(10).min(count);
        total += (end - start) * digits;
        start = end;
        digits += 1;
    }
    total
}

fn package_names_heap_len(count: usize) -> usize {
    count * 8 + digit_sum(count)
}

fn exact_heap_len(count: usize) -> usize {
    12 * count + digit_sum(count) + (count / 100) * 190 + digit_sum(count % 100)
}

fn decimal_len(value: usize) -> usize {
    if value == 0 {
        1
    } else {
        value.ilog10() as usize + 1
    }
}

fn write_decimal_usize(out: &mut [u8], cursor: &mut usize, mut value: usize) {
    let len = decimal_len(value);
    let end = *cursor + len;
    let mut position = end;
    loop {
        position -= 1;
        out[position] = b'0' + (value % 10) as u8;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    *cursor = end;
}

fn write_name_bytes(out: &mut [u8], cursor: &mut usize, index: usize) {
    out[*cursor..*cursor + 8].copy_from_slice(b"package-");
    *cursor += 8;
    write_decimal_usize(out, cursor, index);
}

fn write_version_bytes(out: &mut [u8], cursor: &mut usize, index: usize) {
    out[*cursor..*cursor + 2].copy_from_slice(b"1.");
    *cursor += 2;
    write_decimal_usize(out, cursor, index % 100);
    out[*cursor] = b'.';
    *cursor += 1;
    write_decimal_usize(out, cursor, index % 10);
}

fn write_table_name(
    out: &mut [u8],
    entry: usize,
    heap_offset: usize,
    cursor: &mut usize,
    index: usize,
) {
    let start = *cursor;
    write_name_bytes(out, cursor, index);
    write_u32(out, entry, (start - heap_offset) as u32).unwrap();
    write_u32(out, entry + 4, (*cursor - start) as u32).unwrap();
}

fn write_table_version(
    out: &mut [u8],
    entry: usize,
    heap_offset: usize,
    cursor: &mut usize,
    index: usize,
) {
    let start = *cursor;
    write_version_bytes(out, cursor, index);
    write_u32(out, entry, (start - heap_offset) as u32).unwrap();
    write_u32(out, entry + 4, (*cursor - start) as u32).unwrap();
}

fn write_absolute_name(out: &mut [u8], entry: usize, cursor: &mut usize, index: usize) {
    let start = *cursor;
    write_name_bytes(out, cursor, index);
    write_u32(out, entry, start as u32).unwrap();
    write_u32(out, entry + 4, (*cursor - start) as u32).unwrap();
}

fn write_absolute_version(out: &mut [u8], entry: usize, cursor: &mut usize, index: usize) {
    let start = *cursor;
    write_version_bytes(out, cursor, index);
    write_u32(out, entry, start as u32).unwrap();
    write_u32(out, entry + 4, (*cursor - start) as u32).unwrap();
}

fn initialize_compact_report(count: usize) -> (Vec<u8>, usize, usize, usize) {
    let list_offset = ROOT_OFFSET + REPORT_ROOT_SIZE;
    let items_offset = list_offset + LIST_HEADER_SIZE;
    let table_offset = items_offset + count * PACKAGE_SIZE;
    let heap_offset = table_offset + count * 2 * 8;
    let heap_capacity = exact_heap_len(count);
    let total_len = heap_offset + heap_capacity;
    let mut out = vec![0u8; total_len];

    write_u32(&mut out, 0, MAGIC_REPORT).unwrap();
    write_u32(&mut out, 4, VERSION).unwrap();
    write_u32(&mut out, 20, ROOT_OFFSET as u32).unwrap();
    write_u32(&mut out, 24, ROOT_OFFSET as u32).unwrap();
    write_u32(&mut out, 28, list_offset as u32).unwrap();
    write_u32(&mut out, 32, table_offset as u32).unwrap();
    write_u32(&mut out, 36, heap_offset as u32).unwrap();
    write_u32(&mut out, 40, total_len as u32).unwrap();
    write_u32(&mut out, 44, heap_capacity as u32).unwrap();
    write_u32(&mut out, 52, (count * 2) as u32).unwrap();
    write_u32(&mut out, 56, (count * 2) as u32).unwrap();
    write_u32(&mut out, 60, total_len as u32).unwrap();

    write_u32(&mut out, ROOT_OFFSET, count as u32).unwrap();
    write_u64(&mut out, ROOT_OFFSET + 8, (count as u64) * 1024).unwrap();
    write_f64(&mut out, ROOT_OFFSET + 16, count as f64 / 100.0).unwrap();
    write_u32(&mut out, ROOT_OFFSET + 24, list_offset as u32).unwrap();

    write_u32(&mut out, list_offset, count as u32).unwrap();
    write_u32(&mut out, list_offset + 4, count as u32).unwrap();
    write_u32(&mut out, list_offset + 8, PACKAGE_SIZE as u32).unwrap();
    write_u32(&mut out, list_offset + 12, items_offset as u32).unwrap();
    (out, items_offset, table_offset, heap_offset)
}

pub fn make_bench_report_compact_sequential(count: u32) -> Vec<u8> {
    make_bench_report_with_capacity(count, true)
}

fn make_bench_report_with_capacity(count: u32, compact: bool) -> Vec<u8> {
    let count = count as usize;
    let list_offset = ROOT_OFFSET + REPORT_ROOT_SIZE;
    let items_offset = list_offset + LIST_HEADER_SIZE;
    let package_capacity = if compact {
        count
    } else {
        count.saturating_mul(2)
    };
    let table_offset = items_offset + package_capacity * PACKAGE_SIZE;
    // Initial records use two strings. Remaining slots support either one name
    // mutation per record or appending `count` records with two strings each.
    let string_capacity = if compact {
        count * 2
    } else {
        count * (STRING_SLOTS_PER_PACKAGE + 1)
    };
    let heap_offset = table_offset + string_capacity * 8;
    let heap_capacity = if compact {
        exact_heap_len(count)
    } else {
        count * HEAP_BYTES_PER_PACKAGE + 64
    };
    let total_len = heap_offset + heap_capacity;
    let mut out = vec![0u8; total_len];

    write_u32(&mut out, 0, MAGIC_REPORT).unwrap();
    write_u32(&mut out, 4, VERSION).unwrap();
    write_u32(&mut out, 20, ROOT_OFFSET as u32).unwrap();
    write_u32(&mut out, 24, ROOT_OFFSET as u32).unwrap();
    write_u32(&mut out, 28, list_offset as u32).unwrap();
    write_u32(&mut out, 32, table_offset as u32).unwrap();
    write_u32(&mut out, 36, heap_offset as u32).unwrap();
    write_u32(&mut out, 40, heap_offset as u32).unwrap();
    write_u32(&mut out, 44, heap_capacity as u32).unwrap();
    write_u32(&mut out, 52, (count * 2) as u32).unwrap();
    write_u32(&mut out, 56, string_capacity as u32).unwrap();
    write_u32(&mut out, 60, total_len as u32).unwrap();

    write_u32(&mut out, ROOT_OFFSET, count as u32).unwrap();
    write_u64(&mut out, ROOT_OFFSET + 8, (count as u64) * 1024).unwrap();
    write_f64(&mut out, ROOT_OFFSET + 16, count as f64 / 100.0).unwrap();
    write_u32(&mut out, ROOT_OFFSET + 24, list_offset as u32).unwrap();

    write_u32(&mut out, list_offset, count as u32).unwrap();
    write_u32(&mut out, list_offset + 4, package_capacity as u32).unwrap();
    write_u32(&mut out, list_offset + 8, PACKAGE_SIZE as u32).unwrap();
    write_u32(&mut out, list_offset + 12, items_offset as u32).unwrap();

    let mut cursor = heap_offset;
    for i in 0..count {
        write_table_name(&mut out, table_offset + i * 16, heap_offset, &mut cursor, i);
        write_table_version(
            &mut out,
            table_offset + i * 16 + 8,
            heap_offset,
            &mut cursor,
            i,
        );
        let item = items_offset + i * PACKAGE_SIZE;
        write_u32(&mut out, item, (i * 2) as u32).unwrap();
        write_u32(&mut out, item + 4, (i * 2 + 1) as u32).unwrap();
        write_u32(&mut out, item + 8, (i as u32).wrapping_mul(17)).unwrap();
        write_u32(&mut out, item + 12, (i % 32) as u32).unwrap();
    }
    write_u32(&mut out, 40, cursor as u32).unwrap();
    out
}

/// Read-only experimental layout with string offsets and lengths embedded in
/// each record. Record: name offset/length, version offset/length, size, deps.
pub fn make_bench_report_direct_string_ref(count: u32) -> Vec<u8> {
    if count as usize >= PARALLEL_THRESHOLD {
        make_bench_report_direct_string_ref_parallel(count)
    } else {
        make_bench_report_direct_string_ref_sequential(count)
    }
}

pub fn make_bench_report_direct_string_ref_sequential(count: u32) -> Vec<u8> {
    const HEADER: usize = 32;
    const RECORD: usize = 24;
    let count = count as usize;
    let items_offset = HEADER;
    let heap_offset = items_offset + count * RECORD;
    let mut out = vec![0u8; heap_offset + exact_heap_len(count)];
    write_u32(&mut out, 0, DIRECT_MAGIC).unwrap();
    write_u32(&mut out, 4, count as u32).unwrap();
    write_u32(&mut out, 8, items_offset as u32).unwrap();
    write_u32(&mut out, 12, heap_offset as u32).unwrap();
    let total = out.len() as u32;
    write_u32(&mut out, 16, total).unwrap();

    let mut cursor = heap_offset;
    for i in 0..count {
        let item = items_offset + i * RECORD;
        write_absolute_name(&mut out, item, &mut cursor, i);
        write_absolute_version(&mut out, item + 8, &mut cursor, i);
        write_u32(&mut out, item + 16, (i as u32).wrapping_mul(17)).unwrap();
        write_u32(&mut out, item + 20, (i % 32) as u32).unwrap();
    }
    out
}

/// Read-only struct-of-arrays experiment. Names are an offset/length column;
/// numeric fields are contiguous u32 columns.
pub fn make_bench_report_soa(count: u32) -> Vec<u8> {
    if count as usize >= PARALLEL_THRESHOLD {
        make_bench_report_soa_parallel(count)
    } else {
        make_bench_report_soa_sequential(count)
    }
}

pub fn make_bench_report_soa_sequential(count: u32) -> Vec<u8> {
    const HEADER: usize = 32;
    let count = count as usize;
    let names_offset = HEADER;
    let sizes_offset = names_offset + count * 8;
    let deps_offset = sizes_offset + count * 4;
    let heap_offset = deps_offset + count * 4;
    let names_heap_len = package_names_heap_len(count);
    let mut out = vec![0u8; heap_offset + names_heap_len];
    write_u32(&mut out, 0, SOA_MAGIC).unwrap();
    write_u32(&mut out, 4, count as u32).unwrap();
    write_u32(&mut out, 8, names_offset as u32).unwrap();
    write_u32(&mut out, 12, sizes_offset as u32).unwrap();
    write_u32(&mut out, 16, deps_offset as u32).unwrap();
    write_u32(&mut out, 20, heap_offset as u32).unwrap();
    let total = out.len() as u32;
    write_u32(&mut out, 24, total).unwrap();

    let mut cursor = heap_offset;
    for i in 0..count {
        write_absolute_name(&mut out, names_offset + i * 8, &mut cursor, i);
        write_u32(&mut out, sizes_offset + i * 4, (i as u32).wrapping_mul(17)).unwrap();
        write_u32(&mut out, deps_offset + i * 4, (i % 32) as u32).unwrap();
    }
    out
}

#[derive(Clone, Copy)]
struct ChunkPlan {
    start: usize,
    end: usize,
    heap_base: usize,
    heap_len: usize,
}

fn plan_chunks(count: usize, heap_len_at: fn(usize) -> usize) -> Vec<ChunkPlan> {
    (0..count)
        .step_by(PARALLEL_CHUNK_SIZE)
        .map(|start| {
            let end = (start + PARALLEL_CHUNK_SIZE).min(count);
            let heap_base = heap_len_at(start);
            ChunkPlan {
                start,
                end,
                heap_base,
                heap_len: heap_len_at(end) - heap_base,
            }
        })
        .collect()
}

struct CompactChunk {
    plan: ChunkPlan,
    records: Vec<u8>,
    table: Vec<u8>,
    heap: Vec<u8>,
}

pub fn make_bench_report_compact_parallel(count: u32) -> Vec<u8> {
    let count = count as usize;
    let plans = plan_chunks(count, exact_heap_len);
    let chunks: Vec<_> = plans
        .par_iter()
        .map(|&plan| {
            let len = plan.end - plan.start;
            let mut records = vec![0u8; len * PACKAGE_SIZE];
            let mut table = vec![0u8; len * 16];
            let mut heap = vec![0u8; plan.heap_len];
            let mut cursor = 0;
            for (local, i) in (plan.start..plan.end).enumerate() {
                let name_start = cursor;
                write_name_bytes(&mut heap, &mut cursor, i);
                write_u32(&mut table, local * 16, (plan.heap_base + name_start) as u32).unwrap();
                write_u32(&mut table, local * 16 + 4, (cursor - name_start) as u32).unwrap();

                let version_start = cursor;
                write_version_bytes(&mut heap, &mut cursor, i);
                write_u32(
                    &mut table,
                    local * 16 + 8,
                    (plan.heap_base + version_start) as u32,
                )
                .unwrap();
                write_u32(&mut table, local * 16 + 12, (cursor - version_start) as u32).unwrap();

                let item = local * PACKAGE_SIZE;
                write_u32(&mut records, item, (i * 2) as u32).unwrap();
                write_u32(&mut records, item + 4, (i * 2 + 1) as u32).unwrap();
                write_u32(&mut records, item + 8, (i as u32).wrapping_mul(17)).unwrap();
                write_u32(&mut records, item + 12, (i % 32) as u32).unwrap();
            }
            debug_assert_eq!(cursor, plan.heap_len);
            CompactChunk {
                plan,
                records,
                table,
                heap,
            }
        })
        .collect();

    let (mut out, items_offset, table_offset, heap_offset) = initialize_compact_report(count);
    for chunk in chunks {
        let record_start = items_offset + chunk.plan.start * PACKAGE_SIZE;
        out[record_start..record_start + chunk.records.len()].copy_from_slice(&chunk.records);
        let table_start = table_offset + chunk.plan.start * 16;
        out[table_start..table_start + chunk.table.len()].copy_from_slice(&chunk.table);
        let heap_start = heap_offset + chunk.plan.heap_base;
        out[heap_start..heap_start + chunk.heap.len()].copy_from_slice(&chunk.heap);
    }
    out
}

struct DirectChunk {
    plan: ChunkPlan,
    records: Vec<u8>,
    heap: Vec<u8>,
}

pub fn make_bench_report_direct_string_ref_parallel(count: u32) -> Vec<u8> {
    const HEADER: usize = 32;
    const RECORD: usize = 24;
    let count = count as usize;
    let heap_offset = HEADER + count * RECORD;
    let plans = plan_chunks(count, exact_heap_len);
    let chunks: Vec<_> = plans
        .par_iter()
        .map(|&plan| {
            let len = plan.end - plan.start;
            let mut records = vec![0u8; len * RECORD];
            let mut heap = vec![0u8; plan.heap_len];
            let mut cursor = 0;
            for (local, i) in (plan.start..plan.end).enumerate() {
                let item = local * RECORD;
                let name_start = cursor;
                write_name_bytes(&mut heap, &mut cursor, i);
                write_u32(
                    &mut records,
                    item,
                    (heap_offset + plan.heap_base + name_start) as u32,
                )
                .unwrap();
                write_u32(&mut records, item + 4, (cursor - name_start) as u32).unwrap();

                let version_start = cursor;
                write_version_bytes(&mut heap, &mut cursor, i);
                write_u32(
                    &mut records,
                    item + 8,
                    (heap_offset + plan.heap_base + version_start) as u32,
                )
                .unwrap();
                write_u32(&mut records, item + 12, (cursor - version_start) as u32).unwrap();
                write_u32(&mut records, item + 16, (i as u32).wrapping_mul(17)).unwrap();
                write_u32(&mut records, item + 20, (i % 32) as u32).unwrap();
            }
            debug_assert_eq!(cursor, plan.heap_len);
            DirectChunk {
                plan,
                records,
                heap,
            }
        })
        .collect();

    let mut out = vec![0u8; heap_offset + exact_heap_len(count)];
    write_u32(&mut out, 0, DIRECT_MAGIC).unwrap();
    write_u32(&mut out, 4, count as u32).unwrap();
    write_u32(&mut out, 8, HEADER as u32).unwrap();
    write_u32(&mut out, 12, heap_offset as u32).unwrap();
    let total = out.len() as u32;
    write_u32(&mut out, 16, total).unwrap();
    for chunk in chunks {
        let record_start = HEADER + chunk.plan.start * RECORD;
        out[record_start..record_start + chunk.records.len()].copy_from_slice(&chunk.records);
        let heap_start = heap_offset + chunk.plan.heap_base;
        out[heap_start..heap_start + chunk.heap.len()].copy_from_slice(&chunk.heap);
    }
    out
}

struct SoaChunk {
    plan: ChunkPlan,
    names: Vec<u8>,
    sizes: Vec<u8>,
    dependencies: Vec<u8>,
    heap: Vec<u8>,
}

pub fn make_bench_report_soa_parallel(count: u32) -> Vec<u8> {
    const HEADER: usize = 32;
    let count = count as usize;
    let names_offset = HEADER;
    let sizes_offset = names_offset + count * 8;
    let deps_offset = sizes_offset + count * 4;
    let heap_offset = deps_offset + count * 4;
    let plans = plan_chunks(count, package_names_heap_len);
    let chunks: Vec<_> = plans
        .par_iter()
        .map(|&plan| {
            let len = plan.end - plan.start;
            let mut names = vec![0u8; len * 8];
            let mut sizes = vec![0u8; len * 4];
            let mut dependencies = vec![0u8; len * 4];
            let mut heap = vec![0u8; plan.heap_len];
            let mut cursor = 0;
            for (local, i) in (plan.start..plan.end).enumerate() {
                let name_start = cursor;
                write_name_bytes(&mut heap, &mut cursor, i);
                write_u32(
                    &mut names,
                    local * 8,
                    (heap_offset + plan.heap_base + name_start) as u32,
                )
                .unwrap();
                write_u32(&mut names, local * 8 + 4, (cursor - name_start) as u32).unwrap();
                write_u32(&mut sizes, local * 4, (i as u32).wrapping_mul(17)).unwrap();
                write_u32(&mut dependencies, local * 4, (i % 32) as u32).unwrap();
            }
            debug_assert_eq!(cursor, plan.heap_len);
            SoaChunk {
                plan,
                names,
                sizes,
                dependencies,
                heap,
            }
        })
        .collect();

    let mut out = vec![0u8; heap_offset + package_names_heap_len(count)];
    write_u32(&mut out, 0, SOA_MAGIC).unwrap();
    write_u32(&mut out, 4, count as u32).unwrap();
    write_u32(&mut out, 8, names_offset as u32).unwrap();
    write_u32(&mut out, 12, sizes_offset as u32).unwrap();
    write_u32(&mut out, 16, deps_offset as u32).unwrap();
    write_u32(&mut out, 20, heap_offset as u32).unwrap();
    let total = out.len() as u32;
    write_u32(&mut out, 24, total).unwrap();
    for chunk in chunks {
        let names_start = names_offset + chunk.plan.start * 8;
        out[names_start..names_start + chunk.names.len()].copy_from_slice(&chunk.names);
        let sizes_start = sizes_offset + chunk.plan.start * 4;
        out[sizes_start..sizes_start + chunk.sizes.len()].copy_from_slice(&chunk.sizes);
        let deps_start = deps_offset + chunk.plan.start * 4;
        out[deps_start..deps_start + chunk.dependencies.len()].copy_from_slice(&chunk.dependencies);
        let heap_start = heap_offset + chunk.plan.heap_base;
        out[heap_start..heap_start + chunk.heap.len()].copy_from_slice(&chunk.heap);
    }
    out
}

/// Hybrid experiment: the compact AoS report plus duplicated numeric sidecar
/// columns. Header fields 8 and 12 point at the size and dependency columns.
pub fn make_bench_report_sidecar(count: u32) -> Vec<u8> {
    let mut out = make_bench_report_compact(count);
    let count = count as usize;
    let sizes_offset = out.len();
    out.resize(sizes_offset + count * 8, 0);
    let deps_offset = sizes_offset + count * 4;
    write_u32(&mut out, 8, sizes_offset as u32).unwrap();
    write_u32(&mut out, 12, deps_offset as u32).unwrap();
    let total = out.len() as u32;
    write_u32(&mut out, 60, total).unwrap();
    for i in 0..count {
        write_u32(&mut out, sizes_offset + i * 4, (i as u32).wrapping_mul(17)).unwrap();
        write_u32(&mut out, deps_offset + i * 4, (i % 32) as u32).unwrap();
    }
    out
}

struct ReportView<'a> {
    bytes: &'a [u8],
    len: usize,
    items: usize,
    table: usize,
    heap: usize,
    heap_end: usize,
    string_count: usize,
}

impl<'a> ReportView<'a> {
    fn parse(bytes: &'a [u8]) -> Result<Self, &'static str> {
        if bytes.len() < HEADER_SIZE || read_u32(bytes, 0).ok() != Some(MAGIC_REPORT) {
            return Err("invalid zctf report");
        }
        let list = read_u32(bytes, ROOT_OFFSET + 24).map_err(|_| "truncated report")? as usize;
        let len = read_u32(bytes, list).map_err(|_| "truncated package list")? as usize;
        let items = read_u32(bytes, list + 12).map_err(|_| "truncated package list")? as usize;
        let table = read_u32(bytes, 32).map_err(|_| "truncated report")? as usize;
        let heap = read_u32(bytes, 36).map_err(|_| "truncated report")? as usize;
        let heap_end = read_u32(bytes, 40).map_err(|_| "truncated report")? as usize;
        let string_count = read_u32(bytes, 52).map_err(|_| "truncated report")? as usize;
        if items
            .checked_add(len.checked_mul(PACKAGE_SIZE).ok_or("report too large")?)
            .is_none_or(|end| end > bytes.len())
            || table
                .checked_add(
                    string_count
                        .checked_mul(8)
                        .ok_or("string table too large")?,
                )
                .is_none_or(|end| end > bytes.len())
            || heap > heap_end
            || heap_end > bytes.len()
        {
            return Err("truncated report");
        }
        Ok(Self {
            bytes,
            len,
            items,
            table,
            heap,
            heap_end,
            string_count,
        })
    }

    fn string_bytes(&self, id: usize) -> Result<&'a [u8], &'static str> {
        if id >= self.string_count {
            return Err("invalid string id");
        }
        let entry = self.table + id * 8;
        let relative = read_u32(self.bytes, entry).map_err(|_| "truncated string table")? as usize;
        let len = read_u32(self.bytes, entry + 4).map_err(|_| "truncated string table")? as usize;
        let start = self
            .heap
            .checked_add(relative)
            .ok_or("invalid string range")?;
        let end = start.checked_add(len).ok_or("invalid string range")?;
        if end > self.heap_end {
            return Err("invalid string range");
        }
        Ok(&self.bytes[start..end])
    }
}

pub fn sum_report_sizes(bytes: &[u8]) -> Result<u64, &'static str> {
    let report = ReportView::parse(bytes)?;
    let mut sum = 0u64;
    for i in 0..report.len {
        sum += read_u32(report.bytes, report.items + i * PACKAGE_SIZE + 8)
            .map_err(|_| "truncated package")? as u64;
    }
    Ok(sum)
}

pub fn sum_report_dependency_counts(bytes: &[u8]) -> Result<u64, &'static str> {
    let report = ReportView::parse(bytes)?;
    let mut sum = 0u64;
    for i in 0..report.len {
        sum += read_u32(report.bytes, report.items + i * PACKAGE_SIZE + 12)
            .map_err(|_| "truncated package")? as u64;
    }
    Ok(sum)
}

pub fn sum_report_name_byte_lengths(bytes: &[u8]) -> Result<u64, &'static str> {
    let report = ReportView::parse(bytes)?;
    let mut sum = 0u64;
    for i in 0..report.len {
        let id = read_u32(report.bytes, report.items + i * PACKAGE_SIZE)
            .map_err(|_| "truncated package")? as usize;
        sum += report.string_bytes(id)?.len() as u64;
    }
    Ok(sum)
}

pub fn count_report_name_prefix(bytes: &[u8], prefix: &[u8]) -> Result<u32, &'static str> {
    let report = ReportView::parse(bytes)?;
    let mut count = 0u32;
    for i in 0..report.len {
        let id = read_u32(report.bytes, report.items + i * PACKAGE_SIZE)
            .map_err(|_| "truncated package")? as usize;
        count += report.string_bytes(id)?.starts_with(prefix) as u32;
    }
    Ok(count)
}

pub fn consume_bench_report(bytes: &[u8]) -> Result<u64, &'static str> {
    let report = ReportView::parse(bytes)?;
    let mut checksum = 0u64;
    for i in 0..report.len {
        let item = report.items + i * PACKAGE_SIZE;
        checksum = checksum
            .wrapping_add(read_u32(report.bytes, item + 8).map_err(|_| "truncated package")? as u64)
            .wrapping_add(
                read_u32(report.bytes, item + 12).map_err(|_| "truncated package")? as u64,
            );
    }
    Ok(checksum)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_round_trip() {
        let report = make_bench_report(100);
        let compact = make_bench_report_compact(100);
        assert_eq!(read_u32(&report, ROOT_OFFSET).unwrap(), 100);
        assert!(consume_bench_report(&report).unwrap() > 0);
        assert!(consume_bench_report(&compact).unwrap() > 0);
        assert!(compact.len() < report.len());
        assert_eq!(
            read_u32(&make_bench_report_direct_string_ref(100), 0).unwrap(),
            DIRECT_MAGIC
        );
        assert_eq!(read_u32(&make_bench_report_soa(100), 0).unwrap(), SOA_MAGIC);
        assert!(consume_bench_report(&make_bench_report_sidecar(100)).unwrap() > 0);
    }

    #[test]
    fn direct_writers_and_parallel_builders_match_sequential_bytes() {
        for count in [0, 1, 10, 1_001] {
            assert_eq!(
                make_bench_report_compact_sequential(count),
                make_bench_report_compact_parallel(count)
            );
            assert_eq!(
                make_bench_report_direct_string_ref_sequential(count),
                make_bench_report_direct_string_ref_parallel(count)
            );
            assert_eq!(
                make_bench_report_soa_sequential(count),
                make_bench_report_soa_parallel(count)
            );
        }
    }

    #[test]
    fn native_aggregates_match_expected_values() {
        let count = 100u32;
        let report = make_bench_report_compact_sequential(count);
        assert_eq!(sum_report_sizes(&report).unwrap(), 17 * 99 * 100 / 2);
        assert_eq!(sum_report_dependency_counts(&report).unwrap(), 1_494);
        assert_eq!(sum_report_name_byte_lengths(&report).unwrap(), 990);
        assert_eq!(count_report_name_prefix(&report, b"package-9").unwrap(), 11);
    }
}
