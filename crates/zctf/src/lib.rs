pub use zctf_core::*;
pub use zctf_macros::{document, enum_repr, record};

pub mod prelude {
    pub use crate::{
        ZctfDocument, ZctfRecord, ZctfWriter, document, encode_owned, enum_repr, record,
    };
}
