use crate::{Error, Result, write_f64, write_u16, write_u32, write_u64};

pub const DOCUMENT_MAGIC: u32 = u32::from_le_bytes(*b"ZCTF");
pub const FORMAT_VERSION: u16 = 1;
pub const DOCUMENT_HEADER_SIZE: usize = 36;
pub const NULL_REF: u32 = u32::MAX;
pub const SCHEMA_HASH_OFFSET: u64 = 0xcbf29ce484222325;
const SCHEMA_HASH_PRIME: u64 = 0x100000001b3;

pub const fn schema_hash_bytes(mut hash: u64, bytes: &[u8]) -> u64 {
    let mut index = 0;
    while index < bytes.len() {
        hash ^= bytes[index] as u64;
        hash = hash.wrapping_mul(SCHEMA_HASH_PRIME);
        index += 1;
    }
    hash
}

pub const fn schema_hash_str(hash: u64, value: &str) -> u64 {
    schema_hash_bytes(hash, value.as_bytes())
}

pub const fn schema_hash_u64(mut hash: u64, value: u64) -> u64 {
    let bytes = value.to_le_bytes();
    let mut index = 0;
    while index < bytes.len() {
        hash ^= bytes[index] as u64;
        hash = hash.wrapping_mul(SCHEMA_HASH_PRIME);
        index += 1;
    }
    hash
}

pub trait ZctfSchemaType {
    const TYPE_ID: u64;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZctfHeader {
    pub schema_id: u64,
    pub layout_version: u32,
    pub root_offset: usize,
    pub string_table_offset: usize,
    pub string_heap_offset: usize,
    pub document_len: usize,
}

pub fn validate_zctf(
    bytes: &[u8],
    expected_schema_id: u64,
    expected_layout_version: u32,
) -> Result<ZctfHeader> {
    if bytes.len() < DOCUMENT_HEADER_SIZE {
        return Err(Error::OutOfBounds {
            offset: 0,
            size: DOCUMENT_HEADER_SIZE,
        });
    }
    let magic = crate::read_u32(bytes, 0)?;
    if magic != DOCUMENT_MAGIC {
        return Err(Error::InvalidMagic {
            expected: DOCUMENT_MAGIC,
            actual: magic,
        });
    }
    let format_version = crate::read_u16(bytes, 4)?;
    if format_version != FORMAT_VERSION {
        return Err(Error::UnsupportedVersion(u32::from(format_version)));
    }
    if bytes[6] != 1 {
        return Err(Error::UnsupportedEndian(bytes[6]));
    }
    let schema_id = crate::read_u64(bytes, 8)?;
    if schema_id != expected_schema_id {
        return Err(Error::InvalidSchema {
            expected: expected_schema_id,
            actual: schema_id,
        });
    }
    let layout_version = crate::read_u32(bytes, 16)?;
    if layout_version != expected_layout_version {
        return Err(Error::InvalidLayoutVersion {
            expected: expected_layout_version,
            actual: layout_version,
        });
    }
    let header = ZctfHeader {
        schema_id,
        layout_version,
        root_offset: crate::read_u32(bytes, 20)? as usize,
        string_table_offset: crate::read_u32(bytes, 24)? as usize,
        string_heap_offset: crate::read_u32(bytes, 28)? as usize,
        document_len: crate::read_u32(bytes, 32)? as usize,
    };
    if header.document_len != bytes.len()
        || header.root_offset < DOCUMENT_HEADER_SIZE
        || header.root_offset > header.string_table_offset
        || header.string_table_offset > header.string_heap_offset
        || header.string_heap_offset > header.document_len
        || !(header.string_heap_offset - header.string_table_offset).is_multiple_of(8)
    {
        return Err(Error::InvalidTotalLength(header.document_len));
    }
    let count = (header.string_heap_offset - header.string_table_offset) / 8;
    for id in 0..count {
        let entry = header.string_table_offset + id * 8;
        let start = header
            .string_heap_offset
            .checked_add(crate::read_u32(bytes, entry)? as usize)
            .ok_or(Error::Overflow)?;
        let end = start
            .checked_add(crate::read_u32(bytes, entry + 4)? as usize)
            .ok_or(Error::Overflow)?;
        if end > header.document_len {
            return Err(Error::InvalidStringTable);
        }
    }
    Ok(header)
}

pub const fn align_up(value: usize, alignment: usize) -> usize {
    (value + alignment - 1) & !(alignment - 1)
}

pub trait ZctfDocument {
    const SCHEMA_ID: u64;
    const LAYOUT_VERSION: u32;

    /// Encodes the same document deterministically on every invocation.
    ///
    /// [`encode_owned`] invokes this method once to measure the exact layout and
    /// once to write into the final allocation. Implementations must not consume
    /// external state or produce different field/list/string counts between passes.
    fn encode_zctf(&self, writer: &mut ZctfWriter) -> Result<()>;
}

pub trait ZctfRecord {
    const SIZE: usize;
    const ALIGN: usize;
    fn encode_record(&self, writer: &mut ZctfWriter, offset: usize) -> Result<()>;
}

pub trait ZctfField {
    const SIZE: usize;
    const ALIGN: usize;
    fn write_field(&self, writer: &mut ZctfWriter, offset: usize) -> Result<()>;
}

pub trait ZctfDirectField {
    const SIZE: usize;
    const ALIGN: usize;
    fn write_direct_field(&self, writer: &mut ZctfWriter, offset: usize) -> Result<()>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WriterMode {
    Buffered,
    Measuring,
    Planned,
}

#[derive(Debug, Clone, Copy)]
struct StringEntry {
    offset: u32,
    len: u32,
}

#[derive(Debug, Clone, Copy)]
struct DirectPatch {
    field_offset: usize,
    heap_offset: u32,
    len: u32,
}

#[derive(Debug, Clone, Copy)]
struct EncodePlan {
    schema_id: u64,
    layout_version: u32,
    records_end: usize,
    string_count: usize,
    string_bytes: usize,
}

pub struct ZctfWriter {
    mode: WriterMode,
    schema_id: u64,
    layout_version: u32,
    root_offset: usize,
    body: Vec<u8>,
    body_len: usize,
    string_entries: Vec<StringEntry>,
    string_heap: Vec<u8>,
    direct_patches: Vec<DirectPatch>,
    measured_string_count: usize,
    measured_string_bytes: usize,
    records_end: usize,
    string_table_offset: usize,
    string_heap_offset: usize,
    next_string: usize,
    heap_cursor: usize,
    started: bool,
}

impl Default for ZctfWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl ZctfWriter {
    pub fn new() -> Self {
        Self::with_capacity(256)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            mode: WriterMode::Buffered,
            schema_id: 0,
            layout_version: 0,
            root_offset: align_up(DOCUMENT_HEADER_SIZE, 8),
            body: Vec::with_capacity(capacity),
            body_len: 0,
            string_entries: Vec::new(),
            string_heap: Vec::new(),
            direct_patches: Vec::new(),
            measured_string_count: 0,
            measured_string_bytes: 0,
            records_end: 0,
            string_table_offset: 0,
            string_heap_offset: 0,
            next_string: 0,
            heap_cursor: 0,
            started: false,
        }
    }

    fn measuring() -> Self {
        let mut writer = Self::with_capacity(0);
        writer.mode = WriterMode::Measuring;
        writer
    }

    fn planned(plan: EncodePlan) -> Result<Self> {
        let string_table_offset = align_up(plan.records_end, 4);
        let table_len = plan.string_count.checked_mul(8).ok_or(Error::Overflow)?;
        let string_heap_offset = string_table_offset
            .checked_add(table_len)
            .ok_or(Error::Overflow)?;
        let document_len = string_heap_offset
            .checked_add(plan.string_bytes)
            .ok_or(Error::Overflow)?;
        if document_len > u32::MAX as usize {
            return Err(Error::Overflow);
        }
        Ok(Self {
            mode: WriterMode::Planned,
            schema_id: plan.schema_id,
            layout_version: plan.layout_version,
            root_offset: align_up(DOCUMENT_HEADER_SIZE, 8),
            body: vec![0; document_len],
            body_len: 0,
            string_entries: Vec::new(),
            string_heap: Vec::new(),
            direct_patches: Vec::new(),
            measured_string_count: plan.string_count,
            measured_string_bytes: plan.string_bytes,
            records_end: plan.records_end,
            string_table_offset,
            string_heap_offset,
            next_string: 0,
            heap_cursor: string_heap_offset,
            started: false,
        })
    }

    fn plan(&self) -> Result<EncodePlan> {
        if self.mode != WriterMode::Measuring || !self.started {
            return Err(Error::InvalidTotalLength(self.body_len));
        }
        Ok(EncodePlan {
            schema_id: self.schema_id,
            layout_version: self.layout_version,
            records_end: self.body_len,
            string_count: self.measured_string_count,
            string_bytes: self.measured_string_bytes,
        })
    }

    fn grow_body(&mut self, end: usize) -> Result<()> {
        if end > u32::MAX as usize {
            return Err(Error::Overflow);
        }
        match self.mode {
            WriterMode::Buffered => self.body.resize(end, 0),
            WriterMode::Measuring => {}
            WriterMode::Planned => {
                if end > self.records_end {
                    return Err(Error::OutOfBounds {
                        offset: self.body_len,
                        size: end - self.body_len,
                    });
                }
            }
        }
        self.body_len = end;
        Ok(())
    }

    pub fn begin_document(
        &mut self,
        schema_id: u64,
        layout_version: u32,
        root_size: usize,
    ) -> Result<usize> {
        if self.started {
            return Err(Error::InvalidTotalLength(self.body.len()));
        }
        self.started = true;
        if self.mode == WriterMode::Planned
            && (schema_id != self.schema_id || layout_version != self.layout_version)
        {
            return Err(Error::InvalidSchema {
                expected: self.schema_id,
                actual: schema_id,
            });
        }
        self.schema_id = schema_id;
        self.layout_version = layout_version;
        self.grow_body(
            self.root_offset
                .checked_add(root_size)
                .ok_or(Error::Overflow)?,
        )?;
        Ok(self.root_offset)
    }

    pub fn reserve_aligned(&mut self, size: usize, alignment: usize) -> Result<usize> {
        let offset = align_up(self.body_len, alignment);
        let end = offset.checked_add(size).ok_or(Error::Overflow)?;
        self.grow_body(end)?;
        Ok(offset)
    }

    pub fn write_bytes(&mut self, offset: usize, bytes: &[u8]) -> Result<()> {
        let end = offset.checked_add(bytes.len()).ok_or(Error::Overflow)?;
        if self.mode == WriterMode::Measuring {
            if end > self.body_len {
                return Err(Error::OutOfBounds {
                    offset,
                    size: bytes.len(),
                });
            }
            return Ok(());
        }
        self.body
            .get_mut(offset..end)
            .ok_or(Error::OutOfBounds {
                offset,
                size: bytes.len(),
            })?
            .copy_from_slice(bytes);
        Ok(())
    }
    pub fn set_u8(&mut self, offset: usize, value: u8) -> Result<()> {
        if self.mode == WriterMode::Measuring {
            return self.write_bytes(offset, &[value]);
        }
        *self
            .body
            .get_mut(offset)
            .ok_or(Error::OutOfBounds { offset, size: 1 })? = value;
        Ok(())
    }
    pub fn set_u16(&mut self, offset: usize, value: u16) -> Result<()> {
        if self.mode == WriterMode::Measuring {
            return self.write_bytes(offset, &value.to_le_bytes());
        }
        write_u16(&mut self.body, offset, value)
    }
    pub fn set_u32(&mut self, offset: usize, value: u32) -> Result<()> {
        if self.mode == WriterMode::Measuring {
            return self.write_bytes(offset, &value.to_le_bytes());
        }
        write_u32(&mut self.body, offset, value)
    }
    pub fn set_u64(&mut self, offset: usize, value: u64) -> Result<()> {
        if self.mode == WriterMode::Measuring {
            return self.write_bytes(offset, &value.to_le_bytes());
        }
        write_u64(&mut self.body, offset, value)
    }
    pub fn set_f32(&mut self, offset: usize, value: f32) -> Result<()> {
        self.write_bytes(offset, &value.to_le_bytes())
    }
    pub fn set_f64(&mut self, offset: usize, value: f64) -> Result<()> {
        if self.mode == WriterMode::Measuring {
            return self.write_bytes(offset, &value.to_le_bytes());
        }
        write_f64(&mut self.body, offset, value)
    }
    pub fn set_ref(&mut self, offset: usize, target: usize) -> Result<()> {
        self.set_u32(offset, u32::try_from(target).map_err(|_| Error::Overflow)?)
    }
    pub fn set_string(&mut self, offset: usize, value: &str) -> Result<()> {
        match self.mode {
            WriterMode::Measuring => {
                let id = u32::try_from(self.measured_string_count).map_err(|_| Error::Overflow)?;
                self.measured_string_count += 1;
                self.measured_string_bytes = self
                    .measured_string_bytes
                    .checked_add(value.len())
                    .ok_or(Error::Overflow)?;
                self.set_u32(offset, id)
            }
            WriterMode::Planned => {
                let id = u32::try_from(self.next_string).map_err(|_| Error::Overflow)?;
                let start = self.heap_cursor;
                let end = start.checked_add(value.len()).ok_or(Error::Overflow)?;
                if self.next_string >= self.measured_string_count
                    || end > self.body.len()
                    || end - self.string_heap_offset > self.measured_string_bytes
                {
                    return Err(Error::Overflow);
                }
                self.set_u32(offset, id)?;
                let entry = self.string_table_offset + self.next_string * 8;
                self.set_u32(entry, (start - self.string_heap_offset) as u32)?;
                self.set_u32(entry + 4, value.len() as u32)?;
                self.write_bytes(start, value.as_bytes())?;
                self.next_string += 1;
                self.heap_cursor = end;
                Ok(())
            }
            WriterMode::Buffered => {
                let id = u32::try_from(self.string_entries.len()).map_err(|_| Error::Overflow)?;
                let relative =
                    u32::try_from(self.string_heap.len()).map_err(|_| Error::Overflow)?;
                let len = u32::try_from(value.len()).map_err(|_| Error::Overflow)?;
                self.string_entries.push(StringEntry {
                    offset: relative,
                    len,
                });
                self.string_heap.extend_from_slice(value.as_bytes());
                self.set_u32(offset, id)
            }
        }
    }

    pub fn set_direct_string(&mut self, offset: usize, value: &str) -> Result<()> {
        match self.mode {
            WriterMode::Measuring => {
                self.measured_string_bytes = self
                    .measured_string_bytes
                    .checked_add(value.len())
                    .ok_or(Error::Overflow)?;
                self.set_u32(offset, 0)?;
                self.set_u32(offset + 4, value.len() as u32)
            }
            WriterMode::Planned => {
                let start = self.heap_cursor;
                let end = start.checked_add(value.len()).ok_or(Error::Overflow)?;
                if end > self.body.len()
                    || end - self.string_heap_offset > self.measured_string_bytes
                {
                    return Err(Error::Overflow);
                }
                self.set_u32(offset, u32::try_from(start).map_err(|_| Error::Overflow)?)?;
                self.set_u32(
                    offset + 4,
                    u32::try_from(value.len()).map_err(|_| Error::Overflow)?,
                )?;
                self.write_bytes(start, value.as_bytes())?;
                self.heap_cursor = end;
                Ok(())
            }
            WriterMode::Buffered => {
                let relative =
                    u32::try_from(self.string_heap.len()).map_err(|_| Error::Overflow)?;
                let len = u32::try_from(value.len()).map_err(|_| Error::Overflow)?;
                self.string_heap.extend_from_slice(value.as_bytes());
                self.direct_patches.push(DirectPatch {
                    field_offset: offset,
                    heap_offset: relative,
                    len,
                });
                self.set_u32(offset, relative)?;
                self.set_u32(offset + 4, len)
            }
        }
    }

    pub fn write_fixed_list<T: ZctfRecord>(&mut self, offset: usize, values: &[T]) -> Result<()> {
        let header = self.reserve_aligned(16, 4)?;
        let items = self.reserve_aligned(
            values.len().checked_mul(T::SIZE).ok_or(Error::Overflow)?,
            T::ALIGN,
        )?;
        self.set_u32(
            header,
            u32::try_from(values.len()).map_err(|_| Error::Overflow)?,
        )?;
        self.set_u32(
            header + 4,
            u32::try_from(T::SIZE).map_err(|_| Error::Overflow)?,
        )?;
        self.set_u32(
            header + 8,
            u32::try_from(items).map_err(|_| Error::Overflow)?,
        )?;
        self.set_u32(header + 12, 0)?;
        self.set_ref(offset, header)?;
        for (index, value) in values.iter().enumerate() {
            value.encode_record(self, items + index * T::SIZE)?;
        }
        Ok(())
    }

    pub fn finish(mut self) -> Result<Vec<u8>> {
        if !self.started {
            return Err(Error::InvalidTotalLength(0));
        }
        if self.mode == WriterMode::Measuring {
            return Err(Error::InvalidTotalLength(self.body_len));
        }
        let (string_table_offset, string_heap_offset) = if self.mode == WriterMode::Planned {
            if self.body_len != self.records_end
                || self.next_string != self.measured_string_count
                || self.heap_cursor != self.body.len()
            {
                return Err(Error::InvalidTotalLength(self.body_len));
            }
            (self.string_table_offset, self.string_heap_offset)
        } else {
            let string_table_offset = align_up(self.body_len, 4);
            self.body.resize(string_table_offset, 0);
            let entries = std::mem::take(&mut self.string_entries);
            let table_len = entries.len().checked_mul(8).ok_or(Error::Overflow)?;
            let string_heap_offset = string_table_offset
                .checked_add(table_len)
                .ok_or(Error::Overflow)?;
            self.body.resize(string_heap_offset, 0);
            for (index, entry) in entries.into_iter().enumerate() {
                self.set_u32(string_table_offset + index * 8, entry.offset)?;
                self.set_u32(string_table_offset + index * 8 + 4, entry.len)?;
            }
            self.body.append(&mut self.string_heap);
            let patches = std::mem::take(&mut self.direct_patches);
            for patch in patches {
                let absolute = string_heap_offset
                    .checked_add(patch.heap_offset as usize)
                    .ok_or(Error::Overflow)?;
                self.set_u32(
                    patch.field_offset,
                    u32::try_from(absolute).map_err(|_| Error::Overflow)?,
                )?;
                self.set_u32(patch.field_offset + 4, patch.len)?;
            }
            (string_table_offset, string_heap_offset)
        };
        let document_len = self.body.len();
        if document_len > u32::MAX as usize {
            return Err(Error::Overflow);
        }
        self.write_bytes(0, &DOCUMENT_MAGIC.to_le_bytes())?;
        self.set_u16(4, FORMAT_VERSION)?;
        self.set_u8(6, 1)?;
        self.set_u8(7, 0)?;
        self.set_u64(8, self.schema_id)?;
        self.set_u32(16, self.layout_version)?;
        self.set_u32(20, self.root_offset as u32)?;
        self.set_u32(24, string_table_offset as u32)?;
        self.set_u32(28, string_heap_offset as u32)?;
        self.set_u32(32, document_len as u32)?;
        Ok(self.body)
    }
}

pub fn encode_owned<T: ZctfDocument>(value: &T) -> Result<Vec<u8>> {
    let mut measure = ZctfWriter::measuring();
    value.encode_zctf(&mut measure)?;
    let plan = measure.plan()?;
    let mut writer = ZctfWriter::planned(plan)?;
    value.encode_zctf(&mut writer)?;
    writer.finish()
}

macro_rules! impl_integer_field {
    ($ty:ty, $size:expr, $setter:ident, $name:literal) => {
        impl ZctfSchemaType for $ty {
            const TYPE_ID: u64 = schema_hash_str(SCHEMA_HASH_OFFSET, $name);
        }
        impl ZctfField for $ty {
            const SIZE: usize = $size;
            const ALIGN: usize = $size;
            fn write_field(&self, writer: &mut ZctfWriter, offset: usize) -> Result<()> {
                writer.$setter(offset, *self as _)
            }
        }
    };
}
impl_integer_field!(u8, 1, set_u8, "u8");
impl_integer_field!(i8, 1, set_u8, "i8");
impl_integer_field!(u16, 2, set_u16, "u16");
impl_integer_field!(i16, 2, set_u16, "i16");
impl_integer_field!(u32, 4, set_u32, "u32");
impl_integer_field!(i32, 4, set_u32, "i32");
impl_integer_field!(u64, 8, set_u64, "u64");
impl_integer_field!(i64, 8, set_u64, "i64");

impl ZctfSchemaType for bool {
    const TYPE_ID: u64 = schema_hash_str(SCHEMA_HASH_OFFSET, "bool");
}
impl ZctfField for bool {
    const SIZE: usize = 1;
    const ALIGN: usize = 1;
    fn write_field(&self, writer: &mut ZctfWriter, offset: usize) -> Result<()> {
        writer.set_u8(offset, u8::from(*self))
    }
}
impl ZctfSchemaType for f32 {
    const TYPE_ID: u64 = schema_hash_str(SCHEMA_HASH_OFFSET, "f32");
}
impl ZctfField for f32 {
    const SIZE: usize = 4;
    const ALIGN: usize = 4;
    fn write_field(&self, writer: &mut ZctfWriter, offset: usize) -> Result<()> {
        writer.set_f32(offset, *self)
    }
}
impl ZctfSchemaType for f64 {
    const TYPE_ID: u64 = schema_hash_str(SCHEMA_HASH_OFFSET, "f64");
}
impl ZctfField for f64 {
    const SIZE: usize = 8;
    const ALIGN: usize = 8;
    fn write_field(&self, writer: &mut ZctfWriter, offset: usize) -> Result<()> {
        writer.set_f64(offset, *self)
    }
}
impl ZctfSchemaType for String {
    const TYPE_ID: u64 = schema_hash_str(SCHEMA_HASH_OFFSET, "string:utf8:false");
}
impl ZctfField for String {
    const SIZE: usize = 4;
    const ALIGN: usize = 4;
    fn write_field(&self, writer: &mut ZctfWriter, offset: usize) -> Result<()> {
        writer.set_string(offset, self)
    }
}
impl ZctfDirectField for String {
    const SIZE: usize = 8;
    const ALIGN: usize = 4;
    fn write_direct_field(&self, writer: &mut ZctfWriter, offset: usize) -> Result<()> {
        writer.set_direct_string(offset, self)
    }
}
impl<T: ZctfSchemaType> ZctfSchemaType for Vec<T> {
    const TYPE_ID: u64 = schema_hash_u64(schema_hash_str(SCHEMA_HASH_OFFSET, "list"), T::TYPE_ID);
}
impl<T: ZctfRecord> ZctfField for Vec<T> {
    const SIZE: usize = 4;
    const ALIGN: usize = 4;
    fn write_field(&self, writer: &mut ZctfWriter, offset: usize) -> Result<()> {
        writer.write_fixed_list(offset, self)
    }
}
impl<T: ZctfSchemaType> ZctfSchemaType for Option<T> {
    const TYPE_ID: u64 = schema_hash_u64(schema_hash_str(SCHEMA_HASH_OFFSET, "option"), T::TYPE_ID);
}
impl<T: ZctfField> ZctfField for Option<T> {
    const SIZE: usize = align_up(1, T::ALIGN) + T::SIZE;
    const ALIGN: usize = T::ALIGN;
    fn write_field(&self, writer: &mut ZctfWriter, offset: usize) -> Result<()> {
        match self {
            Some(value) => {
                writer.set_u8(offset, 1)?;
                value.write_field(writer, offset + align_up(1, T::ALIGN))
            }
            None => writer.set_u8(offset, 0),
        }
    }
}
impl<T: ZctfDirectField> ZctfDirectField for Option<T> {
    const SIZE: usize = align_up(1, T::ALIGN) + T::SIZE;
    const ALIGN: usize = T::ALIGN;
    fn write_direct_field(&self, writer: &mut ZctfWriter, offset: usize) -> Result<()> {
        match self {
            Some(value) => {
                writer.set_u8(offset, 1)?;
                value.write_direct_field(writer, offset + align_up(1, T::ALIGN))
            }
            None => writer.set_u8(offset, 0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Foo {
        name: String,
        size: u32,
    }
    impl ZctfDocument for Foo {
        const SCHEMA_ID: u64 = 7;
        const LAYOUT_VERSION: u32 = 1;
        fn encode_zctf(&self, writer: &mut ZctfWriter) -> Result<()> {
            let root = writer.begin_document(Self::SCHEMA_ID, 1, 8)?;
            writer.set_string(root, &self.name)?;
            writer.set_u32(root + 4, self.size)
        }
    }
    #[test]
    fn writes_product_document() {
        let value = Foo {
            name: "hello".into(),
            size: 3,
        };
        let bytes = encode_owned(&value).unwrap();
        let mut buffered = ZctfWriter::new();
        value.encode_zctf(&mut buffered).unwrap();
        assert_eq!(bytes, buffered.finish().unwrap());
        assert_eq!(&bytes[..4], b"ZCTF");
        assert_eq!(crate::read_u64(&bytes, 8).unwrap(), 7);
        assert_eq!(crate::read_u32(&bytes, 32).unwrap() as usize, bytes.len());
        assert_eq!(&bytes[bytes.len() - 5..], b"hello");
        let header = validate_zctf(&bytes, 7, 1).unwrap();
        assert_eq!(header.document_len, bytes.len());
        let mut corrupt = bytes;
        corrupt[8] ^= 1;
        assert!(matches!(
            validate_zctf(&corrupt, 7, 1),
            Err(Error::InvalidSchema { .. })
        ));
    }

    struct DirectFoo {
        name: String,
    }
    impl ZctfDocument for DirectFoo {
        const SCHEMA_ID: u64 = 8;
        const LAYOUT_VERSION: u32 = 1;
        fn encode_zctf(&self, writer: &mut ZctfWriter) -> Result<()> {
            let root = writer.begin_document(Self::SCHEMA_ID, 1, 8)?;
            writer.set_direct_string(root, &self.name)
        }
    }

    #[test]
    fn planned_and_buffered_direct_strings_match() {
        let value = DirectFoo {
            name: "direct".into(),
        };
        let planned = encode_owned(&value).unwrap();
        let mut buffered = ZctfWriter::new();
        value.encode_zctf(&mut buffered).unwrap();
        assert_eq!(planned, buffered.finish().unwrap());
        let root = crate::read_u32(&planned, 20).unwrap() as usize;
        let table = crate::read_u32(&planned, 24).unwrap();
        let heap = crate::read_u32(&planned, 28).unwrap();
        assert_eq!(table, heap);
        let start = crate::read_u32(&planned, root).unwrap() as usize;
        let len = crate::read_u32(&planned, root + 4).unwrap() as usize;
        assert_eq!(&planned[start..start + len], b"direct");
    }
}
