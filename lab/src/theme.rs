//! The interface's themes: TOML files, one per theme, named by their file
//! stem (`tokyo-night.toml` is the theme `tokyo-night`). The lab reads them
//! at run time from `themes/` beside the binary (where the build puts the
//! themes it offers, from the checkout's `themes/`) and from the settings
//! directory's `themes/` (`~/.config/render-lab/themes/` by default), whose
//! files take the place of the binary's of the same name. The lab's own
//! colours (`Theme::CLASSIC`, src/palette.rs) are the built-in theme
//! `render-lab`, which no file replaces.
//!
//! A theme file names the theme and sets every colour of the interface:
//!
//! ```toml
//! name = "Tokyo Night"
//!
//! [colors]
//! background = "#1a1b26"   # "#rrggbb", or "#rrggbbaa" with alpha
//! # ... every field of `Theme`
//! ```

use crate::config::Config;
use crate::packages;
use crate::palette::{self, Theme};
use anyhow::{bail, Context, Result};
use egui::Color32;
use std::path::{Path, PathBuf};

/// The built-in theme: the lab's own colours, which the recorder keeps.
pub const BUILT_IN: &str = "render-lab";
/// The theme a new settings file uses.
pub const DEFAULT: &str = BUILT_IN;

/// A theme the Config page offers.
#[derive(Clone, Debug)]
pub struct Entry {
    /// The file stem, which the settings file names.
    pub id: String,
    /// What the Config page shows.
    pub name: String,
    pub theme: Theme,
}

/// The theme folders, in order: a later folder's file replaces an earlier one's.
pub fn dirs() -> Vec<PathBuf> {
    let mut dirs = vec![packages::bin_dir().join("themes")];
    if let Some(dir) = Config::default_path().as_deref().and_then(Path::parent) {
        dirs.push(dir.join("themes"));
    }
    dirs
}

/// Every theme: the built-in one first, then the files' by name; and why
/// any file was left out.
pub fn list() -> (Vec<Entry>, Vec<String>) {
    let mut files: Vec<(String, PathBuf)> = Vec::new();
    for dir in dirs() {
        let Ok(read) = std::fs::read_dir(&dir) else { continue };
        let mut paths: Vec<PathBuf> = read.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|e| e == "toml")).collect();
        paths.sort();
        for path in paths {
            let Some(id) = path.file_stem().and_then(|s| s.to_str()).map(String::from) else { continue };
            if id == BUILT_IN {
                continue;
            }
            files.retain(|(other, _)| *other != id);
            files.push((id, path));
        }
    }
    let mut entries = vec![built_in()];
    let mut errors = Vec::new();
    for (id, path) in files {
        match read(&path) {
            Ok((name, theme)) => entries.push(Entry { id, name, theme }),
            Err(e) => errors.push(format!("{e:#}")),
        }
    }
    entries[1..].sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    (entries, errors)
}

fn built_in() -> Entry {
    Entry { id: BUILT_IN.into(), name: "Render Lab".into(), theme: Theme::CLASSIC }
}

/// The theme named `id`, from the last folder that has it.
pub fn find(id: &str) -> Result<Entry> {
    if id == BUILT_IN {
        return Ok(built_in());
    }
    let path = dirs().into_iter().rev().map(|d| d.join(format!("{id}.toml"))).find(|p| p.is_file());
    let Some(path) = path else {
        let looked = dirs().iter().map(|d| d.display().to_string()).collect::<Vec<_>>().join(", ");
        bail!("there is no theme `{id}` (looked in {looked})");
    };
    let (name, theme) = read(&path)?;
    Ok(Entry { id: id.into(), name, theme })
}

/// Use the theme named `id`; when it cannot be read, the built-in one, and why.
pub fn apply(id: &str) -> Result<()> {
    match find(id) {
        Ok(entry) => {
            palette::set_theme(entry.theme);
            Ok(())
        }
        Err(e) => {
            palette::set_theme(Theme::CLASSIC);
            Err(e)
        }
    }
}

fn read(path: &Path) -> Result<(String, Theme)> {
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    parse(&text).with_context(|| format!("theme {}", path.display()))
}

/// A theme file's name and colours.
pub fn parse(text: &str) -> Result<(String, Theme)> {
    let document = toml_edit::Document::parse(text).context("parse")?;
    let name = document.get("name").and_then(|n| n.as_str()).context("`name` must be a string")?.to_string();
    let colors = document.get("colors").and_then(|c| c.as_table_like()).context("no `[colors]` table")?;
    let mut theme = Theme::CLASSIC;
    let mut fields = fields(&mut theme);
    for (key, item) in colors.iter() {
        let Some((_, field)) = fields.iter_mut().find(|(k, _)| *k == key) else {
            bail!("unknown colour `{key}`");
        };
        let value = item.as_str().with_context(|| format!("`{key}` must be a string"))?;
        **field = color(value).with_context(|| format!("`{key}`"))?;
    }
    let missing: Vec<&str> = fields.iter().map(|(k, _)| *k).filter(|k| !colors.contains_key(k)).collect();
    if !missing.is_empty() {
        bail!("no colour for {}", missing.join(", "));
    }
    Ok((name, theme))
}

/// `#rrggbb` or `#rrggbbaa` (alpha not premultiplied).
fn color(value: &str) -> Result<Color32> {
    let hex = value.strip_prefix('#').filter(|h| (h.len() == 6 || h.len() == 8) && h.chars().all(|c| c.is_ascii_hexdigit()));
    let Some(hex) = hex else { bail!("\"{value}\" is not #rrggbb or #rrggbbaa") };
    let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).unwrap();
    let alpha = if hex.len() == 8 { byte(6) } else { 255 };
    Ok(Color32::from_rgba_unmultiplied(byte(0), byte(2), byte(4), alpha))
}

/// Every colour of a theme, by its key in a theme file.
fn fields(t: &mut Theme) -> [(&'static str, &mut Color32); 24] {
    [
        ("background", &mut t.background),
        ("panel", &mut t.panel),
        ("panel_raised", &mut t.panel_raised),
        ("border", &mut t.border),
        ("text", &mut t.text),
        ("text_dim", &mut t.text_dim),
        ("text_faint", &mut t.text_faint),
        ("accent", &mut t.accent),
        ("highlight_row", &mut t.highlight_row),
        ("padding", &mut t.padding),
        ("hover", &mut t.hover),
        ("selection", &mut t.selection),
        ("field_even", &mut t.field_even),
        ("field_odd", &mut t.field_odd),
        ("track", &mut t.track),
        ("card", &mut t.card),
        ("smoke", &mut t.smoke),
        ("popup", &mut t.popup),
        ("dim", &mut t.dim),
        ("shadow", &mut t.shadow),
        ("code_keyword", &mut t.code_keyword),
        ("code_name", &mut t.code_name),
        ("code_number", &mut t.code_number),
        ("code_string", &mut t.code_string),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offered_themes_parse() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../themes");
        let mut found = Vec::new();
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            read(&path).unwrap();
            found.push(path.file_stem().unwrap().to_string_lossy().into_owned());
        }
        assert!(found.len() >= 3 && !found.iter().any(|f| f == BUILT_IN), "{found:?}");
    }

    #[test]
    fn rejects_bad_files() {
        assert!(parse("name = \"x\"\n[colors]\nbackground = \"#000000\"\n").is_err());
        assert!(parse("name = \"x\"\n[colors]\nnope = \"#000000\"\n").is_err());
        assert_eq!(color("#ff800080").unwrap(), Color32::from_rgba_unmultiplied(255, 128, 0, 128));
        assert!(color("ff8000").is_err());
    }
}
