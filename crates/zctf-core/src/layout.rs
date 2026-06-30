use crate::{Error, Result};

#[inline]
pub fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(
        bytes
            .get(offset..offset.checked_add(2).ok_or(Error::Overflow)?)
            .ok_or(Error::OutOfBounds { offset, size: 2 })?
            .try_into()
            .unwrap(),
    ))
}

#[inline]
pub fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..offset.checked_add(4).ok_or(Error::Overflow)?)
            .ok_or(Error::OutOfBounds { offset, size: 4 })?
            .try_into()
            .unwrap(),
    ))
}

#[inline]
pub fn read_u64(bytes: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(
        bytes
            .get(offset..offset.checked_add(8).ok_or(Error::Overflow)?)
            .ok_or(Error::OutOfBounds { offset, size: 8 })?
            .try_into()
            .unwrap(),
    ))
}

#[inline]
pub fn read_f64(bytes: &[u8], offset: usize) -> Result<f64> {
    Ok(f64::from_le_bytes(
        bytes
            .get(offset..offset.checked_add(8).ok_or(Error::Overflow)?)
            .ok_or(Error::OutOfBounds { offset, size: 8 })?
            .try_into()
            .unwrap(),
    ))
}

#[inline]
pub fn write_u16(bytes: &mut [u8], offset: usize, value: u16) -> Result<()> {
    bytes
        .get_mut(offset..offset.checked_add(2).ok_or(Error::Overflow)?)
        .ok_or(Error::OutOfBounds { offset, size: 2 })?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

#[inline]
pub fn write_u32(bytes: &mut [u8], offset: usize, value: u32) -> Result<()> {
    bytes
        .get_mut(offset..offset.checked_add(4).ok_or(Error::Overflow)?)
        .ok_or(Error::OutOfBounds { offset, size: 4 })?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

#[inline]
pub fn write_u64(bytes: &mut [u8], offset: usize, value: u64) -> Result<()> {
    bytes
        .get_mut(offset..offset.checked_add(8).ok_or(Error::Overflow)?)
        .ok_or(Error::OutOfBounds { offset, size: 8 })?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

#[inline]
pub fn write_f64(bytes: &mut [u8], offset: usize, value: f64) -> Result<()> {
    bytes
        .get_mut(offset..offset.checked_add(8).ok_or(Error::Overflow)?)
        .ok_or(Error::OutOfBounds { offset, size: 8 })?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}
