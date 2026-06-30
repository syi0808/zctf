use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use zctf_core::{consume_bench_report, make_bench_report};

struct Allocation {
    bytes: Box<[u8]>,
}

static ALLOCATIONS: OnceLock<Mutex<HashMap<u64, Allocation>>> = OnceLock::new();
static NEXT_HANDLE: OnceLock<Mutex<u64>> = OnceLock::new();

fn allocations() -> &'static Mutex<HashMap<u64, Allocation>> {
    ALLOCATIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

#[unsafe(no_mangle)]
pub extern "C" fn zctf_make_bench_report(count: u32) -> u64 {
    let bytes = make_bench_report(count).into_boxed_slice();
    let mut next = NEXT_HANDLE.get_or_init(|| Mutex::new(1)).lock().unwrap();
    let handle = *next;
    *next += 1;
    allocations()
        .lock()
        .unwrap()
        .insert(handle, Allocation { bytes });
    handle
}

#[unsafe(no_mangle)]
pub extern "C" fn zctf_buffer_ptr(handle: u64) -> *const u8 {
    allocations()
        .lock()
        .unwrap()
        .get(&handle)
        .map_or(std::ptr::null(), |allocation| allocation.bytes.as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn zctf_buffer_len(handle: u64) -> u32 {
    allocations()
        .lock()
        .unwrap()
        .get(&handle)
        .map_or(0, |allocation| allocation.bytes.len() as u32)
}

#[unsafe(no_mangle)]
pub extern "C" fn zctf_consume_bench_report(handle: u64) -> u64 {
    allocations()
        .lock()
        .unwrap()
        .get(&handle)
        .and_then(|allocation| consume_bench_report(&allocation.bytes).ok())
        .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn zctf_release(handle: u64) {
    allocations().lock().unwrap().remove(&handle);
}
