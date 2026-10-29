//! The user's settings, kept in `$XDG_CONFIG_HOME/render-lab/config.toml`
//! (`~/.config/render-lab/config.toml` by default) and edited on the
//! homepage's Config page. The file is written only when a tried setting is
//! kept (see `Settings` in src/home.rs); a missing file means the
//! defaults.
//!
//! The settings come in categories (`Category`): the interface's (the
//! language and the theme) and the audio's apply when they are saved; the
//! render settings (full screen) are tried first, behind a popup.

use crate::i18n::Locale;
use crate::theme;
use crate::volume::Volume;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// What the Config page groups the settings by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    /// The language and the theme.
    Interface,
    /// How the lab renders: full screen. A change is tried before it is kept.
    Render,
    /// The volumes.
    Audio,
}

impl Category {
    pub const ALL: [Category; 3] = [Category::Interface, Category::Render, Category::Audio];

    pub fn key(self) -> &'static str {
        match self {
            Category::Interface => "interface",
            Category::Render => "render",
            Category::Audio => "audio",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    /// Interface: the homepage's language, and the edition of a narrated
    /// timeline it offers first (English when a feature has none in this language).
    pub language: Locale,
    /// Interface: the theme (src/theme.rs), by its file's stem.
    pub theme: String,
    /// Render: open the homepage and the viewer in borderless fullscreen.
    pub fullscreen: bool,
    /// Audio: the homepage's music.
    pub music: Volume,
    /// Audio: the narration of a timeline in the viewer, which its dock also sets.
    pub narration: Volume,
}

impl Default for Config {
    fn default() -> Config {
        Config { language: Locale::En, theme: theme::DEFAULT.into(), fullscreen: false, music: Volume::new(50), narration: Volume::new(100) }
    }
}

impl Config {
    /// Whether `self` and `other` differ in `category`'s settings.
    pub fn differs_in(&self, other: &Config, category: Category) -> bool {
        match category {
            Category::Interface => self.language != other.language || self.theme != other.theme,
            Category::Render => self.fullscreen != other.fullscreen,
            Category::Audio => self.music != other.music || self.narration != other.narration,
        }
    }

    /// These settings with `other`'s render settings.
    pub fn with_render_of(&self, other: &Config) -> Config {
        Config { fullscreen: other.fullscreen, ..self.clone() }
    }

    /// `$XDG_CONFIG_HOME/render-lab/config.toml`, else `~/.config/render-lab/config.toml`.
    pub fn default_path() -> Option<PathBuf> {
        let base = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).filter(|p| p.is_absolute()).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
        Some(base.join("render-lab").join("config.toml"))
    }

    /// The settings in `path`; the defaults when the file does not exist.
    /// Keys the lab does not know are ignored.
    pub fn load(path: &Path) -> Result<Config> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Config::default()),
            Err(e) => return Err(e).with_context(|| format!("read {}", path.display())),
        };
        let document = toml_edit::Document::parse(text).with_context(|| format!("parse {}", path.display()))?;
        let mut config = Config::default();
        if let Some(item) = document.get("fullscreen") {
            config.fullscreen = item.as_bool().with_context(|| format!("{}: `fullscreen` must be true or false", path.display()))?;
        }
        if let Some(item) = document.get("language") {
            let code = item.as_str().with_context(|| format!("{}: `language` must be a string", path.display()))?;
            let codes = Locale::ALL.map(Locale::code).join(", ");
            config.language = Locale::from_code(code).with_context(|| format!("{}: unknown `language` \"{code}\" (one of {codes})", path.display()))?;
        }
        if let Some(item) = document.get("theme") {
            config.theme = item.as_str().with_context(|| format!("{}: `theme` must be a string", path.display()))?.to_string();
        }
        for (name, volume) in [("music", &mut config.music), ("narration", &mut config.narration)] {
            let key = format!("{name}_volume");
            if let Some(item) = document.get(&key) {
                let percent = item.as_integer().filter(|p| (0..=100).contains(p)).with_context(|| format!("{}: `{key}` must be a whole number from 0 to 100", path.display()))?;
                volume.percent = percent as u8;
            }
            let key = format!("{name}_muted");
            if let Some(item) = document.get(&key) {
                volume.muted = item.as_bool().with_context(|| format!("{}: `{key}` must be true or false", path.display()))?;
            }
        }
        Ok(config)
    }

    pub fn to_toml(&self) -> String {
        let mut text = format!(
            "# Render Lab settings, written by the homepage's Config page.\n\n# Interface: the homepage's language and the narrated edition it offers first ({}).\nlanguage = \"{}\"\n\n# Interface: the theme, a file's stem in a themes/ folder beside the binary or beside this file.\ntheme = \"{}\"\n\n# Render: open the homepage and the viewer in borderless fullscreen.\nfullscreen = {}\n",
            Locale::ALL.map(Locale::code).join(", "),
            self.language.code(),
            self.theme.replace('\\', "\\\\").replace('"', "\\\""),
            self.fullscreen
        );
        for (name, what, volume) in [("music", "Audio: the homepage's music", self.music), ("narration", "Audio: the narration", self.narration)] {
            text.push_str(&format!("\n# {what}: its volume (0 to 100) and whether it is muted.\n{name}_volume = {}\n{name}_muted = {}\n", volume.percent, volume.muted));
        }
        text
    }

    /// Write the file, through a temporary file renamed over it, so a failed
    /// write never leaves a half-written file behind.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
        }
        let temporary = path.with_extension("toml.tmp");
        std::fs::write(&temporary, self.to_toml()).with_context(|| format!("write {}", temporary.display()))?;
        std::fs::rename(&temporary, path).with_context(|| format!("replace {}", path.display()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let dir = std::env::temp_dir().join(format!("render-lab-config-test-{}", std::process::id()));
        let path = dir.join("config.toml");
        let config = Config { fullscreen: true, language: Locale::ZhCn, theme: "tokyo-night".into(), music: Volume { percent: 30, muted: true }, narration: Volume::new(75) };
        config.save(&path).unwrap();
        assert_eq!(Config::load(&path).unwrap(), config);
        // A file of an older lab: what it does not name keeps its default.
        std::fs::write(&path, "fullscreen = true\n").unwrap();
        assert_eq!(Config::load(&path).unwrap(), Config { fullscreen: true, ..Config::default() });
        std::fs::write(&path, "music_volume = 101\n").unwrap();
        assert!(Config::load(&path).is_err());
        std::fs::write(&path, "language = \"fr\"\n").unwrap();
        assert!(Config::load(&path).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
