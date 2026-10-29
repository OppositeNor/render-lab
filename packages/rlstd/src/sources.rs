//! A package's source index (its `source-index.json`, a resource): every
//! engine file, symbol, and line the package cites, parsed once, and its
//! cited lines checked against the engine's checkout.

use crate::resources;
use crate::sources::SourceIndex;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::Path;
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

    /// Every line the index cites (an entry's line and its details' lines),
    /// hashed again from the file in `checkout` as the index hashes it (the
    /// first 8 bytes of the trimmed line's SHA-256): how many lines it cites,
    /// and each one that differs, as `{key, path, line}`.
    pub fn changed_lines(&self, checkout: &Path) -> anyhow::Result<(usize, Vec<Value>)> {
        let index: Value = serde_json::from_str(self.json())?;
        let mut changed = Vec::new();
        let mut cited = 0;
        for (key, entry) in index["entries"].as_object().into_iter().flatten() {
            let path = entry["path"].as_str().unwrap_or_default();
            let text = std::fs::read_to_string(checkout.join(path)).unwrap_or_default();
            let lines: Vec<&str> = text.lines().collect();
            let details = entry["details"].as_array().into_iter().flatten();
            for cite in std::iter::once(entry).chain(details) {
                let (line, hash) = (cite["line"].as_u64().unwrap_or(0), cite["lineHash"].as_str().unwrap_or_default());
                cited += 1;
                let actual = lines.get(line.saturating_sub(1) as usize).map(|l| Sha256::digest(l.trim().as_bytes()).iter().take(8).map(|b| format!("{b:02x}")).collect::<String>());
                if actual.as_deref() != Some(hash) {
                    changed.push(json!({"key": key, "path": path, "line": line}));
                }
            }
        }
        Ok((cited, changed))
    }
}
