//! Reusable primitives for validated, little-endian, zero-copy binary documents.
//! Domain schemas, generated views, transports, and benchmark data live outside
//! this crate.
#![forbid(unsafe_code)]

mod document;
mod layout;
mod writer;

pub use document::{Document, Error, FixedList, Format, Result, StringTable};
pub use layout::{
    read_f64, read_u16, read_u32, read_u64, write_f64, write_u16, write_u32, write_u64,
};
pub use writer::{
    DOCUMENT_HEADER_SIZE, DOCUMENT_MAGIC, FORMAT_VERSION, NULL_REF, SCHEMA_HASH_OFFSET,
    ZctfDirectField, ZctfDocument, ZctfField, ZctfHeader, ZctfRecord, ZctfSchemaType, ZctfWriter,
    align_up, encode_owned, schema_hash_bytes, schema_hash_str, schema_hash_u64, validate_zctf,
};
