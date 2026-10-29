//! The SDFGI investigation's source index (source-index.json): every Godot
//! file, symbol, and line the viewer cites, written by source_index.py from
//! the checkout. `selftest` checks each line's hash against the checkout
//! when it is present.

use lab::resources;
use lab::sources::SourceIndex;
use std::sync::OnceLock;

/// The index as its JSON reads.
pub fn index_json() -> &'static str {
    resources::get("godot_sdfgi.source-index").text()
}

pub fn index() -> &'static SourceIndex {
    static INDEX: OnceLock<SourceIndex> = OnceLock::new();
    INDEX.get_or_init(|| SourceIndex::parse(index_json()).expect("packages/godot_sdfgi/source-index.json is valid"))
}
