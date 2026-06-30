pub const MAGIC_REPORT: u32 = 0x4654_435a;
pub const MAGIC_CONFIG: u32 = 0x4346_435a;
pub const VERSION: u32 = 1;
pub const HEADER_SIZE: usize = 64;
pub const ROOT_OFFSET: usize = HEADER_SIZE;
pub const REPORT_ROOT_SIZE: usize = 32;
pub const LIST_HEADER_SIZE: usize = 16;
pub const PACKAGE_SIZE: usize = 16;

#[inline]
pub fn get_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

#[inline]
pub fn get_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

#[inline]
pub fn get_f64(bytes: &[u8], offset: usize) -> f64 {
    f64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

#[inline]
pub fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

#[inline]
pub fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

#[inline]
pub fn put_f64(bytes: &mut [u8], offset: usize, value: f64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
