//! Reusable primitives for validated, little-endian, zero-copy binary documents.
//! Domain schemas, generated views, transports, and benchmark data live outside
//! this crate.
#![forbid(unsafe_code)]

mod document;
mod layout;

pub use document::{Document, Error, FixedList, Format, Result, StringTable};
pub use layout::{
    read_f64, read_u16, read_u32, read_u64, write_f64, write_u16, write_u32, write_u64,
};
