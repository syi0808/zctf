use std::cell::RefCell;
use std::collections::HashMap;
use zctf_bench_fixtures::{consume_bench_report, make_bench_report};

thread_local! {
    static ALLOCATIONS: RefCell<HashMap<u32, Box<[u8]>>> = RefCell::new(HashMap::new());
    static NEXT_HANDLE: RefCell<u32> = const { RefCell::new(1) };
}

#[unsafe(no_mangle)]
pub extern "C" fn zctf_make_bench_report(count: u32) -> u32 {
    let handle = NEXT_HANDLE.with(|next| {
        let handle = *next.borrow();
        *next.borrow_mut() = handle + 1;
        handle
    });
    ALLOCATIONS.with(|allocations| {
        allocations
            .borrow_mut()
            .insert(handle, make_bench_report(count).into_boxed_slice());
    });
    handle
}

#[unsafe(no_mangle)]
pub extern "C" fn zctf_buffer_ptr(handle: u32) -> u32 {
    ALLOCATIONS.with(|allocations| {
        allocations
            .borrow()
            .get(&handle)
            .map_or(0, |bytes| bytes.as_ptr() as u32)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn zctf_buffer_len(handle: u32) -> u32 {
    ALLOCATIONS.with(|allocations| {
        allocations
            .borrow()
            .get(&handle)
            .map_or(0, |bytes| bytes.len() as u32)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn zctf_consume_bench_report(handle: u32) -> u64 {
    ALLOCATIONS.with(|allocations| {
        allocations
            .borrow()
            .get(&handle)
            .and_then(|bytes| consume_bench_report(bytes).ok())
            .unwrap_or(0)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn zctf_release(handle: u32) {
    ALLOCATIONS.with(|allocations| {
        allocations.borrow_mut().remove(&handle);
    });
}
