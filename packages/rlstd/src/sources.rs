//! A package's source index (its `source-index.json`, a resource): every
//! engine file, symbol, and line the package cites, parsed once.

use crate::resources;
use crate::sources::SourceIndex;
use std::sync::OnceLock;

/// The source index of resource `key`, e.g. `static INDEX: PackageIndex = PackageIndex::new("<package>.source-index");`.
pub struct PackageIndex {
    key: &'static str,
    index: OnceLock<SourceIndex>,
}

impl PackageIndex {
    pub const fn new(key: &'static str) -> PackageIndex {
        PackageIndex { key, index: OnceLock::new() }
    }

    /// The index as its JSON reads.
    pub fn json(&self) -> &'static str {
        resources::get(self.key).text()
    }

    pub fn get(&self) -> &SourceIndex {
        self.index.get_or_init(|| SourceIndex::parse(self.json()).unwrap_or_else(|e| panic!("{} is not a valid source index: {e:#}", self.key)))
    }
}
