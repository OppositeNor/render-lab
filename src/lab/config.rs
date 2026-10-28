//! The user's settings, kept in `$XDG_CONFIG_HOME/render-lab/config.toml`
//! (`~/.config/render-lab/config.toml` by default) and edited on the
//! homepage's Config page. The file is written only when a tried setting is
//! kept (see `Settings` in src/lab/home.rs); a missing file means the
//! defaults.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config {
    /// Open the homepage and the viewer in borderless fullscreen.
    pub fullscreen: bool,
}

impl Default for Config {
    fn default() -> Config {
        Config { fullscreen: false }
    }
}

impl Config {
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
        Ok(config)
    }

    pub fn to_toml(&self) -> String {
        format!("# Render Lab settings, written by the homepage's Config page.\n\n# Open the homepage and the viewer in borderless fullscreen.\nfullscreen = {}\n", self.fullscreen)
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
