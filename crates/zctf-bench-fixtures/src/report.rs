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

pub fn make_bench_report(count: u32) -> Vec<u8> {
    make_bench_report_with_capacity(count, false)
}

pub fn make_bench_report_compact(count: u32) -> Vec<u8> {
    make_bench_report_with_capacity(count, true)
}

fn exact_heap_len(count: usize) -> usize {
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
    12 * count + digit_sum(count) + (count / 100) * 190 + digit_sum(count % 100)
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
        let name = format!("package-{i}");
        let version = format!("1.{}.{}", i % 100, i % 10);
        write_string(
            &mut out,
            table_offset,
            heap_offset,
            i * 2,
            &name,
            &mut cursor,
        );
        write_string(
            &mut out,
            table_offset,
            heap_offset,
            i * 2 + 1,
            &version,
            &mut cursor,
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
        let name = format!("package-{i}");
        let version = format!("1.{}.{}", i % 100, i % 10);
        write_direct_string(&mut out, item, &name, &mut cursor);
        write_direct_string(&mut out, item + 8, &version, &mut cursor);
        write_u32(&mut out, item + 16, (i as u32).wrapping_mul(17)).unwrap();
        write_u32(&mut out, item + 20, (i % 32) as u32).unwrap();
    }
    out
}

/// Read-only struct-of-arrays experiment. Names are an offset/length column;
/// numeric fields are contiguous u32 columns.
pub fn make_bench_report_soa(count: u32) -> Vec<u8> {
    const HEADER: usize = 32;
    let count = count as usize;
    let names_offset = HEADER;
    let sizes_offset = names_offset + count * 8;
    let deps_offset = sizes_offset + count * 4;
    let heap_offset = deps_offset + count * 4;
    let names_heap_len: usize = (0..count).map(|i| format!("package-{i}").len()).sum();
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
        let name = format!("package-{i}");
        write_direct_string(&mut out, names_offset + i * 8, &name, &mut cursor);
        write_u32(&mut out, sizes_offset + i * 4, (i as u32).wrapping_mul(17)).unwrap();
        write_u32(&mut out, deps_offset + i * 4, (i % 32) as u32).unwrap();
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

fn write_direct_string(out: &mut [u8], entry: usize, value: &str, cursor: &mut usize) {
    let bytes = value.as_bytes();
    write_u32(out, entry, *cursor as u32).unwrap();
    write_u32(out, entry + 4, bytes.len() as u32).unwrap();
    out[*cursor..*cursor + bytes.len()].copy_from_slice(bytes);
    *cursor += bytes.len();
}

fn write_string(
    out: &mut [u8],
    table_offset: usize,
    heap_offset: usize,
    id: usize,
    value: &str,
    cursor: &mut usize,
) {
    let bytes = value.as_bytes();
    let relative = *cursor - heap_offset;
    write_u32(out, table_offset + id * 8, relative as u32).unwrap();
    write_u32(out, table_offset + id * 8 + 4, bytes.len() as u32).unwrap();
    out[*cursor..*cursor + bytes.len()].copy_from_slice(bytes);
    *cursor += bytes.len();
}

pub fn consume_bench_report(bytes: &[u8]) -> Result<u64, &'static str> {
    if bytes.len() < HEADER_SIZE || read_u32(bytes, 0).ok() != Some(MAGIC_REPORT) {
        return Err("invalid zctf report");
    }
    let list_offset = read_u32(bytes, ROOT_OFFSET + 24).map_err(|_| "truncated report")? as usize;
    let len = read_u32(bytes, list_offset).map_err(|_| "truncated package list")? as usize;
    let items = read_u32(bytes, list_offset + 12).map_err(|_| "truncated package list")? as usize;
    if items + len.saturating_mul(PACKAGE_SIZE) > bytes.len() {
        return Err("truncated package list");
    }
    let mut checksum = 0u64;
    for i in 0..len {
        let item = items + i * PACKAGE_SIZE;
        checksum = checksum
            .wrapping_add(read_u32(bytes, item + 8).map_err(|_| "truncated package")? as u64)
            .wrapping_add(read_u32(bytes, item + 12).map_err(|_| "truncated package")? as u64);
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
}
