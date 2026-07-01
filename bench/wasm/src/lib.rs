use zctf_product_bench_model::transform_result;

const OUTPUT_HEADER_SIZE: usize = 8;

#[unsafe(no_mangle)]
pub extern "C" fn zctf_alloc(len: u32) -> u32 {
    let bytes = vec![0_u8; len as usize].into_boxed_slice();
    Box::into_raw(bytes) as *mut u8 as usize as u32
}

/// # Safety
/// `ptr` and `len` must identify a live allocation returned by [`zctf_alloc`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zctf_free_input(ptr: u32, len: u32) {
    if len == 0 {
        return;
    }
    let slice = std::ptr::slice_from_raw_parts_mut(ptr as usize as *mut u8, len as usize);
    unsafe {
        drop(Box::from_raw(slice));
    }
}

fn output_bytes(bytes: &[u8]) -> u32 {
    let Some(total_len) = OUTPUT_HEADER_SIZE.checked_add(bytes.len()) else {
        return 0;
    };
    let Ok(total_len_u32) = u32::try_from(total_len) else {
        return 0;
    };
    let Ok(bytes_len_u32) = u32::try_from(bytes.len()) else {
        return 0;
    };
    let mut output = vec![0_u8; total_len].into_boxed_slice();
    output[0..4].copy_from_slice(&total_len_u32.to_le_bytes());
    output[4..8].copy_from_slice(&bytes_len_u32.to_le_bytes());
    output[OUTPUT_HEADER_SIZE..].copy_from_slice(bytes);
    Box::into_raw(output) as *mut u8 as usize as u32
}

/// Returns a pointer to `[allocation_len: u32, document_len: u32, document bytes...]`.
#[unsafe(no_mangle)]
pub extern "C" fn transform_zctf(source_ptr: u32, source_len: u32, warning_count: u32) -> u32 {
    let source = unsafe {
        std::slice::from_raw_parts(source_ptr as usize as *const u8, source_len as usize)
    };
    let Ok(source) = std::str::from_utf8(source) else {
        return 0;
    };
    let Ok(document) = zctf::encode_owned(&transform_result(source, warning_count)) else {
        return 0;
    };
    output_bytes(&document)
}

/// Returns a pointer to `[allocation_len: u32, json_len: u32, JSON UTF-8 bytes...]`.
#[unsafe(no_mangle)]
pub extern "C" fn transform_json(source_ptr: u32, source_len: u32, warning_count: u32) -> u32 {
    let source = unsafe {
        std::slice::from_raw_parts(source_ptr as usize as *const u8, source_len as usize)
    };
    let Ok(source) = std::str::from_utf8(source) else {
        return 0;
    };
    let Ok(json) = serde_json::to_vec(&transform_result(source, warning_count)) else {
        return 0;
    };
    output_bytes(&json)
}

/// # Safety
/// `ptr` must identify a live allocation returned by [`transform_zctf`] or [`transform_json`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zctf_free_output(ptr: u32) {
    if ptr == 0 {
        return;
    }
    let pointer = ptr as usize as *mut u8;
    let total_len = unsafe { std::ptr::read_unaligned(pointer.cast::<u32>()) } as usize;
    let slice = std::ptr::slice_from_raw_parts_mut(pointer, total_len);
    unsafe {
        drop(Box::from_raw(slice));
    }
}
