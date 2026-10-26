//! The lighting investigation's source index (source-index.json): every O3DE
//! file, symbol, and line the viewer cites, written by source_index.py from
//! the checkout. `selftest` checks each line's hash against the checkout
//! when it is present.

use crate::lab::sources::SourceIndex;
use std::sync::OnceLock;

pub const INDEX_JSON: &str = include_str!("source-index.json");

pub fn index() -> &'static SourceIndex {
    static INDEX: OnceLock<SourceIndex> = OnceLock::new();
    INDEX.get_or_init(|| SourceIndex::parse(INDEX_JSON).expect("src/o3de/lighting/source-index.json is valid"))
}
