use crate::{read_f64, read_u16, read_u32, read_u64};
use std::fmt;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    OutOfBounds { offset: usize, size: usize },
    Overflow,
    InvalidMagic { expected: u32, actual: u32 },
    UnsupportedVersion(u32),
    InvalidTotalLength(usize),
    InvalidList,
    InvalidStringTable,
    InvalidUtf8,
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfBounds { offset, size } => {
                write!(
                    formatter,
                    "read at {offset} with size {size} is out of bounds"
                )
            }
            Self::Overflow => formatter.write_str("offset arithmetic overflow"),
            Self::InvalidMagic { expected, actual } => {
                write!(
                    formatter,
                    "invalid magic 0x{actual:08x}; expected 0x{expected:08x}"
                )
            }
            Self::UnsupportedVersion(version) => write!(formatter, "unsupported version {version}"),
            Self::InvalidTotalLength(length) => write!(formatter, "invalid total length {length}"),
            Self::InvalidList => formatter.write_str("invalid fixed list"),
            Self::InvalidStringTable => formatter.write_str("invalid string table"),
            Self::InvalidUtf8 => formatter.write_str("string is not valid UTF-8"),
        }
    }
}

impl std::error::Error for Error {}

#[derive(Debug, Clone, Copy)]
pub struct Format<'a> {
    pub magic: u32,
    pub versions: &'a [u32],
    pub minimum_size: usize,
    pub total_length_offset: Option<usize>,
}

#[derive(Debug, Clone, Copy)]
pub struct Document<'a> {
    bytes: &'a [u8],
    version: u32,
}

impl<'a> Document<'a> {
    pub fn parse(bytes: &'a [u8], format: Format<'_>) -> Result<Self> {
        if bytes.len() < format.minimum_size {
            return Err(Error::OutOfBounds {
                offset: 0,
                size: format.minimum_size,
            });
        }
        let actual = read_u32(bytes, 0)?;
        if actual != format.magic {
            return Err(Error::InvalidMagic {
                expected: format.magic,
                actual,
            });
        }
        let version = read_u32(bytes, 4)?;
        if !format.versions.contains(&version) {
            return Err(Error::UnsupportedVersion(version));
        }
        let bytes = if let Some(offset) = format.total_length_offset {
            let total = read_u32(bytes, offset)? as usize;
            if total < format.minimum_size || total > bytes.len() {
                return Err(Error::InvalidTotalLength(total));
            }
            &bytes[..total]
        } else {
            bytes
        };
        Ok(Self { bytes, version })
    }

    pub fn bytes(&self) -> &'a [u8] {
        self.bytes
    }

    pub fn version(&self) -> u32 {
        self.version
    }

    pub fn u16(&self, offset: usize) -> Result<u16> {
        read_u16(self.bytes, offset)
    }

    pub fn u32(&self, offset: usize) -> Result<u32> {
        read_u32(self.bytes, offset)
    }

    pub fn u64(&self, offset: usize) -> Result<u64> {
        read_u64(self.bytes, offset)
    }

    pub fn f64(&self, offset: usize) -> Result<f64> {
        read_f64(self.bytes, offset)
    }

    pub fn fixed_list(&self, offset: usize, expected_stride: usize) -> Result<FixedList<'a>> {
        FixedList::parse(*self, offset, expected_stride)
    }

    pub fn string_table(
        &self,
        table_offset: usize,
        heap_offset: usize,
        count: usize,
    ) -> Result<StringTable<'a>> {
        StringTable::parse(*self, table_offset, heap_offset, count)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FixedList<'a> {
    document: Document<'a>,
    len: usize,
    capacity: usize,
    stride: usize,
    items_offset: usize,
}

impl<'a> FixedList<'a> {
    pub const HEADER_SIZE: usize = 16;

    fn parse(document: Document<'a>, offset: usize, expected_stride: usize) -> Result<Self> {
        let len = document.u32(offset)? as usize;
        let capacity = document.u32(offset.checked_add(4).ok_or(Error::Overflow)?)? as usize;
        let stride = document.u32(offset.checked_add(8).ok_or(Error::Overflow)?)? as usize;
        let items_offset = document.u32(offset.checked_add(12).ok_or(Error::Overflow)?)? as usize;
        let byte_len = capacity.checked_mul(stride).ok_or(Error::Overflow)?;
        let end = items_offset.checked_add(byte_len).ok_or(Error::Overflow)?;
        if len > capacity || stride != expected_stride || end > document.bytes().len() {
            return Err(Error::InvalidList);
        }
        Ok(Self {
            document,
            len,
            capacity,
            stride,
            items_offset,
        })
    }

    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub fn capacity(&self) -> usize {
        self.capacity
    }
    pub fn stride(&self) -> usize {
        self.stride
    }

    pub fn item_offset(&self, index: usize) -> Result<usize> {
        if index >= self.len {
            return Err(Error::OutOfBounds {
                offset: index,
                size: 1,
            });
        }
        self.items_offset
            .checked_add(index.checked_mul(self.stride).ok_or(Error::Overflow)?)
            .ok_or(Error::Overflow)
    }

    pub fn document(&self) -> Document<'a> {
        self.document
    }
}

#[derive(Debug, Clone, Copy)]
pub struct StringTable<'a> {
    document: Document<'a>,
    table_offset: usize,
    heap_offset: usize,
    count: usize,
}

impl<'a> StringTable<'a> {
    pub const ENTRY_SIZE: usize = 8;

    fn parse(
        document: Document<'a>,
        table_offset: usize,
        heap_offset: usize,
        count: usize,
    ) -> Result<Self> {
        let table_end = table_offset
            .checked_add(count.checked_mul(Self::ENTRY_SIZE).ok_or(Error::Overflow)?)
            .ok_or(Error::Overflow)?;
        if table_end > heap_offset || heap_offset > document.bytes().len() {
            return Err(Error::InvalidStringTable);
        }
        let table = Self {
            document,
            table_offset,
            heap_offset,
            count,
        };
        for id in 0..count {
            table.range(id)?;
        }
        Ok(table)
    }

    pub fn len(&self) -> usize {
        self.count
    }
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    fn range(&self, id: usize) -> Result<std::ops::Range<usize>> {
        if id >= self.count {
            return Err(Error::InvalidStringTable);
        }
        let entry = self.table_offset + id * Self::ENTRY_SIZE;
        let start = self
            .heap_offset
            .checked_add(self.document.u32(entry)? as usize)
            .ok_or(Error::Overflow)?;
        let end = start
            .checked_add(self.document.u32(entry + 4)? as usize)
            .ok_or(Error::Overflow)?;
        if end > self.document.bytes().len() {
            return Err(Error::InvalidStringTable);
        }
        Ok(start..end)
    }

    pub fn bytes(&self, id: usize) -> Result<&'a [u8]> {
        Ok(&self.document.bytes()[self.range(id)?])
    }

    pub fn get(&self, id: usize) -> Result<&'a str> {
        std::str::from_utf8(self.bytes(id)?).map_err(|_| Error::InvalidUtf8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::write_u32;

    #[test]
    fn rejects_bad_headers_and_lists_without_panicking() {
        let format = Format {
            magic: 0x1234,
            versions: &[1],
            minimum_size: 24,
            total_length_offset: Some(8),
        };
        assert!(Document::parse(&[0; 4], format).is_err());
        let mut bytes = vec![0; 48];
        write_u32(&mut bytes, 0, 0x1234).unwrap();
        write_u32(&mut bytes, 4, 1).unwrap();
        write_u32(&mut bytes, 8, 48).unwrap();
        write_u32(&mut bytes, 24, 2).unwrap();
        write_u32(&mut bytes, 28, 1).unwrap();
        write_u32(&mut bytes, 32, 4).unwrap();
        write_u32(&mut bytes, 36, 40).unwrap();
        assert!(matches!(
            Document::parse(&bytes, format).unwrap().fixed_list(24, 4),
            Err(Error::InvalidList)
        ));
    }

    #[test]
    fn reads_schema_independent_lists_and_strings() {
        let format = Format {
            magic: 0x1234,
            versions: &[1],
            minimum_size: 24,
            total_length_offset: Some(8),
        };
        let mut bytes = vec![0; 96];
        write_u32(&mut bytes, 0, 0x1234).unwrap();
        write_u32(&mut bytes, 4, 1).unwrap();
        write_u32(&mut bytes, 8, 96).unwrap();
        write_u32(&mut bytes, 24, 1).unwrap();
        write_u32(&mut bytes, 28, 2).unwrap();
        write_u32(&mut bytes, 32, 4).unwrap();
        write_u32(&mut bytes, 36, 40).unwrap();
        write_u32(&mut bytes, 40, 7).unwrap();
        write_u32(&mut bytes, 48, 0).unwrap();
        write_u32(&mut bytes, 52, 5).unwrap();
        bytes[56..61].copy_from_slice(b"hello");

        let document = Document::parse(&bytes, format).unwrap();
        let list = document.fixed_list(24, 4).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(document.u32(list.item_offset(0).unwrap()).unwrap(), 7);
        assert_eq!(
            document.string_table(48, 56, 1).unwrap().get(0).unwrap(),
            "hello"
        );
    }
}
