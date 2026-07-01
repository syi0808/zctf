use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SchemaFragment {
    pub zctf: String,
    pub name: String,
    pub kind: RecordKind,
    #[serde(default)]
    pub layout_version: u32,
    #[serde(default)]
    pub fields: Vec<Field>,
    #[serde(default)]
    pub repr: Option<String>,
    #[serde(default)]
    pub variants: Vec<EnumVariant>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RecordKind {
    Document,
    Record,
    Enum,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Field {
    pub rust_name: String,
    pub js_name: String,
    #[serde(rename = "type")]
    pub ty: Type,
    #[serde(default)]
    pub skip: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Type {
    Bool,
    U8,
    U16,
    U32,
    U64,
    I8,
    I16,
    I32,
    I64,
    F32,
    F64,
    String {
        #[serde(default = "utf8")]
        encoding: String,
        #[serde(default)]
        direct: bool,
    },
    Option {
        item: Box<Type>,
    },
    List {
        item: Box<Type>,
    },
    Named {
        name: String,
    },
}
fn utf8() -> String {
    "utf8".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EnumVariant {
    pub name: String,
    pub value: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Schema {
    pub zctf: String,
    pub name: String,
    pub schema_id: String,
    pub layout_version: u32,
    pub root: String,
    pub records: BTreeMap<String, SchemaFragment>,
    pub enums: BTreeMap<String, SchemaFragment>,
}

impl Schema {
    pub fn schema_id_u64(&self) -> Result<u64> {
        Ok(u64::from_str_radix(
            self.schema_id.trim_start_matches("0x"),
            16,
        )?)
    }
}

pub fn snake_to_camel(name: &str) -> String {
    let mut out = String::new();
    let mut upper = false;
    for ch in name.chars() {
        if ch == '_' {
            upper = true;
        } else if upper {
            out.extend(ch.to_uppercase());
            upper = false;
        } else {
            out.push(ch);
        }
    }
    out
}

const HASH_OFFSET: u64 = 0xcbf29ce484222325;
const HASH_PRIME: u64 = 0x100000001b3;

fn hash_str(mut hash: u64, value: &str) -> u64 {
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(HASH_PRIME);
    }
    hash
}

fn hash_u64(mut hash: u64, value: u64) -> u64 {
    for byte in value.to_le_bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(HASH_PRIME);
    }
    hash
}

fn type_id(
    ty: &Type,
    all: &BTreeMap<String, SchemaFragment>,
    stack: &mut Vec<String>,
) -> Result<u64> {
    Ok(match ty {
        Type::Bool => hash_str(HASH_OFFSET, "bool"),
        Type::U8 => hash_str(HASH_OFFSET, "u8"),
        Type::U16 => hash_str(HASH_OFFSET, "u16"),
        Type::U32 => hash_str(HASH_OFFSET, "u32"),
        Type::U64 => hash_str(HASH_OFFSET, "u64"),
        Type::I8 => hash_str(HASH_OFFSET, "i8"),
        Type::I16 => hash_str(HASH_OFFSET, "i16"),
        Type::I32 => hash_str(HASH_OFFSET, "i32"),
        Type::I64 => hash_str(HASH_OFFSET, "i64"),
        Type::F32 => hash_str(HASH_OFFSET, "f32"),
        Type::F64 => hash_str(HASH_OFFSET, "f64"),
        Type::String { encoding, direct } => {
            hash_str(HASH_OFFSET, &format!("string:{encoding}:{direct}"))
        }
        Type::Option { item } => {
            hash_u64(hash_str(HASH_OFFSET, "option"), type_id(item, all, stack)?)
        }
        Type::List { item } => hash_u64(hash_str(HASH_OFFSET, "list"), type_id(item, all, stack)?),
        Type::Named { name } => {
            if stack.contains(name) {
                return Err(format!("recursive schema type {name} is unsupported").into());
            }
            let fragment = all
                .get(name)
                .ok_or_else(|| format!("missing schema fragment for {name}"))?;
            stack.push(name.clone());
            let id = fragment_type_id(fragment, all, stack)?;
            stack.pop();
            id
        }
    })
}

fn fragment_type_id(
    fragment: &SchemaFragment,
    all: &BTreeMap<String, SchemaFragment>,
    stack: &mut Vec<String>,
) -> Result<u64> {
    let kind = match fragment.kind {
        RecordKind::Document => "document",
        RecordKind::Record => "record",
        RecordKind::Enum => "enum",
    };
    let mut hash = hash_str(HASH_OFFSET, kind);
    hash = hash_str(hash, &fragment.name);
    if fragment.kind == RecordKind::Enum {
        hash = hash_str(hash, fragment.repr.as_deref().unwrap_or("u8"));
        for variant in &fragment.variants {
            hash = hash_str(hash, &variant.name);
            hash = hash_u64(hash, variant.value as u64);
        }
    } else {
        for field in fragment.fields.iter().filter(|field| !field.skip) {
            hash = hash_str(hash, &field.rust_name);
            hash = hash_u64(hash, type_id(&field.ty, all, stack)?);
        }
    }
    Ok(hash)
}

pub fn assemble(fragments: Vec<SchemaFragment>) -> Result<Vec<Schema>> {
    let all: BTreeMap<_, _> = fragments.into_iter().map(|f| (f.name.clone(), f)).collect();
    let mut output = Vec::new();
    for document in all.values().filter(|f| f.kind == RecordKind::Document) {
        let mut records = BTreeMap::new();
        let mut enums = BTreeMap::new();
        collect_named(document, &all, &mut records, &mut enums)?;
        records.insert(document.name.clone(), document.clone());
        let id = fragment_type_id(document, &all, &mut vec![document.name.clone()])?;
        output.push(Schema {
            zctf: "1".into(),
            name: document.name.clone(),
            schema_id: format!("0x{id:016x}"),
            layout_version: document.layout_version.max(1),
            root: document.name.clone(),
            records,
            enums,
        });
    }
    Ok(output)
}

fn collect_named(
    fragment: &SchemaFragment,
    all: &BTreeMap<String, SchemaFragment>,
    records: &mut BTreeMap<String, SchemaFragment>,
    enums: &mut BTreeMap<String, SchemaFragment>,
) -> Result<()> {
    fn visit(
        ty: &Type,
        all: &BTreeMap<String, SchemaFragment>,
        records: &mut BTreeMap<String, SchemaFragment>,
        enums: &mut BTreeMap<String, SchemaFragment>,
    ) -> Result<()> {
        match ty {
            Type::Option { item } | Type::List { item } => visit(item, all, records, enums),
            Type::Named { name } => {
                let found = all
                    .get(name)
                    .ok_or_else(|| format!("missing schema fragment for {name}"))?;
                match found.kind {
                    RecordKind::Enum => {
                        enums.insert(name.clone(), found.clone());
                    }
                    _ => {
                        if records.insert(name.clone(), found.clone()).is_none() {
                            collect_named(found, all, records, enums)?;
                        }
                    }
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
    for field in &fragment.fields {
        visit(&field.ty, all, records, enums)?;
    }
    Ok(())
}

pub fn load_fragment(path: impl AsRef<Path>) -> Result<SchemaFragment> {
    Ok(serde_json::from_slice(&std::fs::read(path)?)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn camel_case_is_stable() {
        assert_eq!(snake_to_camel("duration_ms"), "durationMs");
    }

    #[test]
    fn schema_id_changes_when_a_nested_record_changes() {
        let document = SchemaFragment {
            zctf: "1".into(),
            name: "Root".into(),
            kind: RecordKind::Document,
            layout_version: 1,
            fields: vec![Field {
                rust_name: "child".into(),
                js_name: "child".into(),
                ty: Type::Named {
                    name: "Child".into(),
                },
                skip: false,
            }],
            repr: None,
            variants: vec![],
        };
        let child = |ty| SchemaFragment {
            zctf: "1".into(),
            name: "Child".into(),
            kind: RecordKind::Record,
            layout_version: 1,
            fields: vec![Field {
                rust_name: "value".into(),
                js_name: "value".into(),
                ty,
                skip: false,
            }],
            repr: None,
            variants: vec![],
        };
        let first = assemble(vec![document.clone(), child(Type::U32)]).unwrap();
        let second = assemble(vec![document, child(Type::U64)]).unwrap();
        assert_ne!(first[0].schema_id, second[0].schema_id);
    }
}
