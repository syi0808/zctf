#[repr(C)]
#[derive(Debug)]
pub struct ZctfFfiBuffer {
    pub ptr: *mut u8,
    pub len: usize,
    pub cap: usize,
}

pub fn to_ffi_buffer<T: zctf::ZctfDocument>(value: &T) -> zctf::Result<ZctfFfiBuffer> {
    let mut bytes = zctf::encode_owned(value)?;
    let output = ZctfFfiBuffer {
        ptr: bytes.as_mut_ptr(),
        len: bytes.len(),
        cap: bytes.capacity(),
    };
    std::mem::forget(bytes);
    Ok(output)
}

/// # Safety
/// `buffer` must have been returned by [`to_ffi_buffer`] and not already freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zctf_free_buffer(buffer: ZctfFfiBuffer) {
    if !buffer.ptr.is_null() {
        unsafe {
            drop(Vec::from_raw_parts(buffer.ptr, buffer.len, buffer.cap));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Empty;
    impl zctf::ZctfDocument for Empty {
        const SCHEMA_ID: u64 = 1;
        const LAYOUT_VERSION: u32 = 1;
        fn encode_zctf(&self, writer: &mut zctf::ZctfWriter) -> zctf::Result<()> {
            writer.begin_document(1, 1, 0).map(|_| ())
        }
    }
    #[test]
    fn owns_and_frees_bytes() {
        let buffer = to_ffi_buffer(&Empty).unwrap();
        assert!(buffer.len >= zctf::DOCUMENT_HEADER_SIZE);
        unsafe {
            zctf_free_buffer(buffer);
        }
    }
}
