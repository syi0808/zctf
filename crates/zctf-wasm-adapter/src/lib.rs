#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZctfWasmBuffer {
    pub ptr: u32,
    pub len: u32,
    pub cap: u32,
}

pub fn to_wasm_buffer<T: zctf::ZctfDocument>(value: &T) -> zctf::Result<ZctfWasmBuffer> {
    let mut bytes = zctf::encode_owned(value)?;
    let pointer = bytes.as_mut_ptr() as usize;
    if pointer > u32::MAX as usize {
        return Err(zctf::Error::Overflow);
    }
    let output = ZctfWasmBuffer {
        ptr: pointer as u32,
        len: bytes.len() as u32,
        cap: bytes.capacity() as u32,
    };
    std::mem::forget(bytes);
    Ok(output)
}

/// # Safety
/// `buffer` must have been returned by [`to_wasm_buffer`] and not already freed.
pub unsafe fn free_wasm_buffer(buffer: ZctfWasmBuffer) {
    unsafe {
        drop(Vec::from_raw_parts(
            buffer.ptr as usize as *mut u8,
            buffer.len as usize,
            buffer.cap as usize,
        ));
    }
}
