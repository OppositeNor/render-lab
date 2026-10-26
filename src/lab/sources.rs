//! An investigation's machine-readable source index: the engine checkout it
//! was read from, every cited file, symbol, and line, the reading per
//! chapter, the entries per demonstration, and how the lab's records map to
//! the engine's. Shared by the video, the viewer, the narrated script, and
//! the source-reading guide. Each investigation embeds its own.

use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineInfo {
    pub name: String,
    pub version: String,
    pub commit: String,
}

// These structs mirror the whole JSON schema; the viewer reads only some fields.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct Detail {
    pub line: u32,
    pub note: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct SourceEntry {
    pub path: String,
    pub line: u32,
    pub symbol: String,
    pub inspect: String,
    /// Chinese version of `inspect`, for the zh-CN edition.
    #[serde(default, rename = "inspectZh")]
    pub inspect_zh: Option<String>,
    #[serde(default)]
    pub details: Vec<Detail>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct ChapterReading {
    pub number: u32,
    pub title: String,
    pub reading: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mapping {
    pub lab: String,
    /// The engine structure, short (the struct panel shows it) and in full.
    pub engine_short: String,
    pub engine_name: String,
    pub relationship: String,
    pub sources: Vec<String>,
    pub exact: String,
    pub lab_layout: String,
}

impl Mapping {
    pub fn engine_short(&self) -> &str {
        &self.engine_short
    }
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct SourceIndex {
    pub engine: EngineInfo,
    pub entries: BTreeMap<String, SourceEntry>,
    pub chapters: Vec<ChapterReading>,
    pub demos: BTreeMap<String, Vec<String>>,
    pub mappings: Vec<Mapping>,
}

impl SourceEntry {
    /// What to inspect, in the current locale.
    pub fn inspect_text(&self) -> &str {
        match &self.inspect_zh {
            Some(zh) if crate::lab::i18n::zh() => zh,
            _ => &self.inspect,
        }
    }

    pub fn file_name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(&self.path)
    }

    /// Short symbol for on-screen callouts: drops template parameters.
    pub fn short_symbol(&self) -> String {
        let mut out = String::new();
        let mut depth = 0;
        for ch in self.symbol.chars() {
            match ch {
                '<' => depth += 1,
                '>' => depth -= 1,
                _ if depth == 0 => out.push(ch),
                _ => {}
            }
        }
        out
    }
}

impl SourceIndex {
    pub fn entry(&self, key: &str) -> &SourceEntry {
        self.entries.get(key).unwrap_or_else(|| panic!("unknown source key {key}"))
    }

    pub fn mapping(&self, record: &str) -> Option<&Mapping> {
        self.mappings.iter().find(|m| m.lab == record || m.lab.split(" / ").any(|part| part == record))
    }
}

impl SourceIndex {
    pub fn parse(json: &str) -> anyhow::Result<SourceIndex> {
        Ok(serde_json::from_str(json)?)
    }
}

/// The current investigation's source index.
pub fn index() -> &'static SourceIndex {
    crate::lab::investigation::current().sources()
}
