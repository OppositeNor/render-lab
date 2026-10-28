//! The lighting investigation's source index (source-index.json): every O3DE
//! file, symbol, and line the viewer cites, written by source_index.py from
//! the checkout. `selftest` checks each line's hash against the checkout
//! when it is present.

use crate::lab::resources;
use crate::lab::sources::SourceIndex;
use std::sync::OnceLock;

/// The index as its JSON reads.
pub fn index_json() -> &'static str {
    resources::get("o3de/lighting/source-index").text()
}

pub fn index() -> &'static SourceIndex {
    static INDEX: OnceLock<SourceIndex> = OnceLock::new();
    INDEX.get_or_init(|| SourceIndex::parse(index_json()).expect("src/o3de/lighting/source-index.json is valid"))
}
