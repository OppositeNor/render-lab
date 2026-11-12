//! An investigation's machine-readable source index: the engine checkout it
//! was read from (or the paper, for an investigation of a paper's method),
//! every cited file, symbol, and line (or section and page), the reading per
//! chapter, the entries per demonstration, how the lab's records map to the
//! source's, and the works a story cites. Shared by the video, the viewer,
//! the narrated script, and the source-reading guide. Each investigation
//! embeds its own.

use serde::Deserialize;
use std::collections::BTreeMap;

/// The checkout the entries were read from; empty for a paper without its authors' code.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineInfo {
    pub name: String,
    pub version: String,
    pub commit: String,
}

/// The paper an index cites, for an investigation of a paper's method.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaperInfo {
    /// Its key in the index's `references`.
    pub key: String,
    /// How the chrome names it, "Bitterli et al. 2020".
    pub short: String,
    pub title: String,
    /// The journal or proceedings, short: "ACM TOG 39(4)".
    pub venue: String,
    pub doi: String,
    /// The SHA-256 of the PDF the entries were read from.
    pub sha256: String,
}

/// A work a story cites: its key and its reference in APA style.
#[derive(Debug, Deserialize)]
pub struct Reference {
    pub key: String,
    pub apa: String,
}

// These structs mirror the whole JSON schema; the viewer reads only some fields.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct Detail {
    #[serde(default)]
    pub line: u32,
    /// The page, for a paper's entry.
    #[serde(default)]
    pub page: Option<String>,
    pub note: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct SourceEntry {
    /// The file and line in the checkout (an engine's entry).
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub line: u32,
    /// The section and page in the paper (a paper's entry), "4.4" and "148:9".
    #[serde(default)]
    pub section: Option<String>,
    #[serde(default)]
    pub page: Option<String>,
    /// The function or struct, or a paper's equation, algorithm line, or figure.
    pub symbol: String,
    /// What the line does, in English; another language's is the package's
    /// source table's `entry-<key>-inspect` (`i18n::source_text`).
    pub inspect: String,
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
    /// The checkout the entries were read from; an investigation of a paper
    /// has its `paper` instead (and an engine block only for its authors' code).
    #[serde(default)]
    pub engine: EngineInfo,
    #[serde(default)]
    pub paper: Option<PaperInfo>,
    /// The works a story cites, in the order its references card lists them.
    #[serde(default)]
    pub references: Vec<Reference>,
    pub entries: BTreeMap<String, SourceEntry>,
    pub chapters: Vec<ChapterReading>,
    pub demos: BTreeMap<String, Vec<String>>,
    pub mappings: Vec<Mapping>,
    /// The frame graph's nodes (src/graph/) and the engine passes they mirror.
    #[serde(default)]
    pub passes: Vec<PassMapping>,
}

/// A node of the frame's graph (its entry point, or the lab's name) and the
/// engine pass it mirrors, with the entries that cite it.
#[derive(Debug, Deserialize)]
pub struct PassMapping {
    pub lab: String,
    pub engine: String,
    pub sources: Vec<String>,
}

impl SourceEntry {
    /// What to inspect (the entry `key`'s), in the current locale.
    pub fn inspect_text(&self, key: &str) -> &str {
        let package = crate::investigation::current()
            .package()
            .unwrap_or("render_lab");
        crate::i18n::source_text(package, &format!("entry-{key}-inspect")).unwrap_or(&self.inspect)
    }

    pub fn file_name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(&self.path)
    }

    /// Where it is: `path:line` in a checkout, or `§section, p. page` in a paper.
    pub fn location(&self) -> String {
        match (&self.section, &self.page) {
            (Some(section), Some(page)) => format!("§{section}, p. {page}"),
            (None, Some(page)) => format!("p. {page}"),
            _ => format!("{}:{}", self.path, self.line),
        }
    }

    /// Where it is, short, for a callout: the file's name, or the section.
    pub fn short_location(&self) -> String {
        match (&self.section, &self.page) {
            (Some(section), _) => format!("§{section}"),
            (None, Some(page)) => format!("p. {page}"),
            _ => self.file_name().to_string(),
        }
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
        self.entries
            .get(key)
            .unwrap_or_else(|| panic!("unknown source key {key}"))
    }

    pub fn mapping(&self, record: &str) -> Option<&Mapping> {
        self.mappings
            .iter()
            .find(|m| m.lab == record || m.lab.split(" / ").any(|part| part == record))
    }

    /// The engine pass a node of the frame's graph mirrors (a repeated entry's `#2` too).
    pub fn pass(&self, node: &str) -> Option<&PassMapping> {
        let entry = crate::graph::entry_of(node);
        self.passes.iter().find(|p| p.lab == entry)
    }
}

impl SourceIndex {
    pub fn parse(json: &str) -> anyhow::Result<SourceIndex> {
        let index: SourceIndex = serde_json::from_str(json)?;
        anyhow::ensure!(
            !index.engine.name.is_empty() || index.paper.is_some(),
            "a source index names its engine or its paper"
        );
        Ok(index)
    }

    /// The source's name and version: the engine's, or the paper's short name and venue.
    pub fn source_name(&self) -> (&str, &str) {
        match &self.paper {
            Some(paper) => (&paper.short, &paper.venue),
            None => (&self.engine.name, &self.engine.version),
        }
    }

    /// What the entries were read at: the commit's first `n` characters, or the paper's DOI.
    pub fn revision(&self, n: usize) -> &str {
        match &self.paper {
            Some(paper) => &paper.doi,
            None => self.engine.commit.get(..n).unwrap_or(&self.engine.commit),
        }
    }

    /// The source in one line, "Godot 4.7 · 1a2b3c4d" or "Bitterli et al. 2020 ACM TOG 39(4) · <doi>".
    pub fn edition(&self) -> String {
        let (name, version) = self.source_name();
        format!("{name} {version} · {}", self.revision(8))
    }
}

/// The current investigation's source index.
pub fn index() -> &'static SourceIndex {
    crate::investigation::current().sources()
}
