use crate::layout::{MAGIC_CONFIG, get_f64, get_u32, get_u64};
use std::hint::black_box;

const VERSION_SPECIALIZED: u32 = 2;
const ROOT_SIZE: usize = 32;
const LIST_HEADER_SIZE: usize = 16;
const PLUGIN_SIZE: usize = 8;
const STRING_ENTRY_SIZE: usize = 8;
const KNOWN_NAME_BIT: u32 = 0x8000_0000;
const KNOWN_NAME_MAX: u32 = 5;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConfigSummary {
    pub checksum: u64,
    pub plugin_count: u32,
}

pub fn consume_compiled_config(bytes: &[u8]) -> Result<ConfigSummary, &'static str> {
    if bytes.len() < 8 || get_u32(bytes, 0) != MAGIC_CONFIG {
        return Err("invalid zctf config");
    }
    match get_u32(bytes, 4) {
        1 => consume_v1(bytes),
        VERSION_SPECIALIZED => ConfigView::parse(bytes).map(|view| view.summary()),
        _ => Err("unsupported zctf config version"),
    }
}

pub fn consume_compiled_config_repeated(
    bytes: &[u8],
    reads: u32,
    promote: bool,
) -> Result<u64, &'static str> {
    let view = ConfigView::parse(bytes)?;
    let mut checksum = 0u64;
    if promote {
        let local = TransformConfigLocal::from_view(&view);
        for _ in 0..reads {
            checksum = checksum.wrapping_add(black_box(local.hot_checksum()));
        }
    } else {
        for _ in 0..reads {
            checksum = checksum.wrapping_add(black_box(view.hot_checksum()));
        }
    }
    Ok(checksum)
}

struct ConfigView<'a> {
    bytes: &'a [u8],
    root: usize,
    plugins: List,
    svgo_plugins: List,
}

#[derive(Clone, Copy)]
struct List {
    len: usize,
    items: usize,
}

impl<'a> ConfigView<'a> {
    fn parse(bytes: &'a [u8]) -> Result<Self, &'static str> {
        if bytes.len() < 64
            || get_u32(bytes, 0) != MAGIC_CONFIG
            || get_u32(bytes, 4) != VERSION_SPECIALIZED
        {
            return Err("invalid specialized config");
        }
        let total = get_u32(bytes, 20) as usize;
        if total < 64 || total > bytes.len() {
            return Err("truncated config");
        }
        let bytes = &bytes[..total];
        let root = get_u32(bytes, 8) as usize;
        if root
            .checked_add(ROOT_SIZE)
            .is_none_or(|end| end > bytes.len())
        {
            return Err("truncated config root");
        }

        let table = get_u32(bytes, 12) as usize;
        let heap = get_u32(bytes, 16) as usize;
        let string_count = get_u32(bytes, 24) as usize;
        let table_end = table
            .checked_add(string_count.saturating_mul(STRING_ENTRY_SIZE))
            .ok_or("invalid string table")?;
        if table_end > heap || heap > bytes.len() {
            return Err("invalid string regions");
        }

        let plugins = validate_list(bytes, get_u32(bytes, root + 16) as usize, 4)?;
        let svgo_plugins = validate_list(bytes, get_u32(bytes, root + 20) as usize, PLUGIN_SIZE)?;
        for index in 0..plugins.len {
            validate_name(
                bytes,
                get_u32(bytes, plugins.items + index * 4),
                table,
                heap,
                string_count,
            )?;
        }
        for index in 0..svgo_plugins.len {
            validate_name(
                bytes,
                get_u32(bytes, svgo_plugins.items + index * PLUGIN_SIZE),
                table,
                heap,
                string_count,
            )?;
        }
        Ok(Self {
            bytes,
            root,
            plugins,
            svgo_plugins,
        })
    }

    // Parsing validates every fixed offset and referenced list once. Hot reads
    // can then avoid repeated bounds checks.
    #[inline(always)]
    fn u8_unchecked(&self, offset: usize) -> u8 {
        unsafe { *self.bytes.get_unchecked(offset) }
    }

    #[inline(always)]
    fn u16_unchecked(&self, offset: usize) -> u16 {
        unsafe {
            u16::from_le_bytes([
                *self.bytes.get_unchecked(offset),
                *self.bytes.get_unchecked(offset + 1),
            ])
        }
    }

    #[inline(always)]
    fn u32_unchecked(&self, offset: usize) -> u32 {
        unsafe {
            u32::from_le_bytes([
                *self.bytes.get_unchecked(offset),
                *self.bytes.get_unchecked(offset + 1),
                *self.bytes.get_unchecked(offset + 2),
                *self.bytes.get_unchecked(offset + 3),
            ])
        }
    }

    #[inline(always)]
    fn f64_unchecked(&self, offset: usize) -> f64 {
        unsafe {
            f64::from_le_bytes([
                *self.bytes.get_unchecked(offset),
                *self.bytes.get_unchecked(offset + 1),
                *self.bytes.get_unchecked(offset + 2),
                *self.bytes.get_unchecked(offset + 3),
                *self.bytes.get_unchecked(offset + 4),
                *self.bytes.get_unchecked(offset + 5),
                *self.bytes.get_unchecked(offset + 6),
                *self.bytes.get_unchecked(offset + 7),
            ])
        }
    }

    #[inline(always)]
    fn hot_checksum(&self) -> u64 {
        let root = self.root;
        (self.u32_unchecked(root) as u64)
            .wrapping_add(self.u32_unchecked(root + 4) as u64)
            .wrapping_add(self.f64_unchecked(root + 8).to_bits())
            .wrapping_add(self.jsx_runtime() as u64)
            .wrapping_add(self.export_type() as u64)
    }

    #[inline(always)]
    fn jsx_runtime(&self) -> u8 {
        match self.u8_unchecked(self.root + 24) {
            2 => 2,
            _ => 1,
        }
    }

    #[inline(always)]
    fn export_type(&self) -> u8 {
        match self.u8_unchecked(self.root + 25) {
            2 => 2,
            _ => 1,
        }
    }

    fn summary(&self) -> ConfigSummary {
        let mut checksum = self.hot_checksum();
        for index in 0..self.plugins.len {
            checksum =
                checksum.wrapping_add(self.u32_unchecked(self.plugins.items + index * 4) as u64);
        }
        for index in 0..self.svgo_plugins.len {
            let item = self.svgo_plugins.items + index * PLUGIN_SIZE;
            checksum = checksum
                .wrapping_add(self.u32_unchecked(item) as u64)
                .wrapping_add(self.u16_unchecked(item + 4) as u64)
                .wrapping_add(self.u16_unchecked(item + 6) as u64);
        }
        ConfigSummary {
            checksum,
            plugin_count: (self.plugins.len + self.svgo_plugins.len) as u32,
        }
    }
}

#[derive(Clone, Copy)]
struct TransformConfigLocal {
    presence: u32,
    flags: u32,
    float_precision: f64,
    jsx_runtime: u8,
    export_type: u8,
}

impl TransformConfigLocal {
    #[inline]
    fn from_view(view: &ConfigView<'_>) -> Self {
        let root = view.root;
        Self {
            presence: view.u32_unchecked(root),
            flags: view.u32_unchecked(root + 4),
            float_precision: view.f64_unchecked(root + 8),
            jsx_runtime: view.jsx_runtime(),
            export_type: view.export_type(),
        }
    }

    #[inline(always)]
    fn hot_checksum(&self) -> u64 {
        (self.presence as u64)
            .wrapping_add(self.flags as u64)
            .wrapping_add(self.float_precision.to_bits())
            .wrapping_add(self.jsx_runtime as u64)
            .wrapping_add(self.export_type as u64)
    }
}

fn validate_list(bytes: &[u8], offset: usize, stride: usize) -> Result<List, &'static str> {
    if offset == 0 {
        return Ok(List { len: 0, items: 0 });
    }
    if offset
        .checked_add(LIST_HEADER_SIZE)
        .is_none_or(|end| end > bytes.len())
    {
        return Err("truncated list");
    }
    let len = get_u32(bytes, offset) as usize;
    let encoded_stride = get_u32(bytes, offset + 8) as usize;
    let items = get_u32(bytes, offset + 12) as usize;
    if encoded_stride != stride
        || items
            .checked_add(len.saturating_mul(stride))
            .is_none_or(|end| end > bytes.len())
    {
        return Err("invalid list");
    }
    Ok(List { len, items })
}

fn validate_name(
    bytes: &[u8],
    token: u32,
    table: usize,
    heap: usize,
    string_count: usize,
) -> Result<(), &'static str> {
    if token & KNOWN_NAME_BIT != 0 {
        return if token & !KNOWN_NAME_BIT <= KNOWN_NAME_MAX {
            Ok(())
        } else {
            Err("invalid known name")
        };
    }
    let id = token as usize;
    if id >= string_count {
        return Err("invalid string id");
    }
    let entry = table + id * STRING_ENTRY_SIZE;
    let start = heap
        .checked_add(get_u32(bytes, entry) as usize)
        .ok_or("invalid string")?;
    let end = start
        .checked_add(get_u32(bytes, entry + 4) as usize)
        .ok_or("invalid string")?;
    if end > bytes.len() {
        return Err("truncated string");
    }
    Ok(())
}

fn consume_v1(bytes: &[u8]) -> Result<ConfigSummary, &'static str> {
    if bytes.len() < 56 {
        return Err("invalid legacy config");
    }
    let root = get_u32(bytes, 8) as usize;
    if root + 24 > bytes.len() {
        return Err("truncated config root");
    }
    let presence = get_u64(bytes, root);
    let plugins_offset = get_u32(bytes, root + 12) as usize;
    let svgo_offset = get_u32(bytes, root + 16) as usize;
    let mut checksum = presence
        .wrapping_add(bytes[root + 8] as u64)
        .wrapping_add(bytes[root + 9] as u64)
        .wrapping_add(bytes[root + 10] as u64)
        .wrapping_add(bytes[root + 11] as u64);
    let mut plugin_count = 0;
    if plugins_offset != 0 {
        plugin_count += consume_v1_string_list(bytes, plugins_offset, &mut checksum)?;
    }
    if svgo_offset != 0 {
        if svgo_offset + 24 > bytes.len() {
            return Err("truncated svgo config");
        }
        checksum = checksum
            .wrapping_add(get_u64(bytes, svgo_offset))
            .wrapping_add(bytes[svgo_offset + 8] as u64)
            .wrapping_add(get_f64(bytes, svgo_offset + 16).to_bits());
        let nested = get_u32(bytes, svgo_offset + 12) as usize;
        if nested != 0 {
            let list = validate_list(bytes, nested, 16)?;
            plugin_count += list.len as u32;
            for index in 0..list.len {
                let item = list.items + index * 16;
                checksum = checksum
                    .wrapping_add(get_u64(bytes, item))
                    .wrapping_add(get_u32(bytes, item + 8) as u64)
                    .wrapping_add(bytes[item + 12] as u64)
                    .wrapping_add(bytes[item + 13] as u64);
            }
        }
    }
    Ok(ConfigSummary {
        checksum,
        plugin_count,
    })
}

fn consume_v1_string_list(
    bytes: &[u8],
    offset: usize,
    checksum: &mut u64,
) -> Result<u32, &'static str> {
    let list = validate_list(bytes, offset, 4)?;
    for index in 0..list.len {
        *checksum = checksum.wrapping_add(get_u32(bytes, list.items + index * 4) as u64);
    }
    Ok(list.len as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_specialized_offsets() {
        let mut bytes = vec![0; 64];
        bytes[0..4].copy_from_slice(&MAGIC_CONFIG.to_le_bytes());
        bytes[4..8].copy_from_slice(&VERSION_SPECIALIZED.to_le_bytes());
        bytes[8..12].copy_from_slice(&32u32.to_le_bytes());
        bytes[12..16].copy_from_slice(&64u32.to_le_bytes());
        bytes[16..20].copy_from_slice(&64u32.to_le_bytes());
        bytes[20..24].copy_from_slice(&64u32.to_le_bytes());
        assert!(ConfigView::parse(&bytes).is_ok());
        bytes[20..24].copy_from_slice(&65u32.to_le_bytes());
        assert_eq!(ConfigView::parse(&bytes).err(), Some("truncated config"));
    }
}
