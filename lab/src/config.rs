//! The user's settings, kept in `$XDG_CONFIG_HOME/render-lab/config.toml`
//! (`~/.config/render-lab/config.toml` by default) and edited on the
//! homepage's Config page. The file is written only when a tried setting is
//! kept (see `Settings` in src/home/); a missing file means the
//! defaults.
//!
//! The settings come in categories (`Category`): the interface's (the
//! language, the theme, and the listings' style) and the audio's apply when they are saved; the
//! render settings (full screen) are tried first, behind a popup.

use crate::i18n::Locale;
use crate::pseudocode::Style;
use crate::theme;
use crate::volume::Volume;
use crate::ui::windows::{Dock, WindowId, WindowPlace};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// What the Config page groups the settings by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    /// The language, the theme, and the listings' style.
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

/// The viewer's layout as the user left it: where each of its windows is (src/ui/windows.rs), in canvas
/// points. The viewer writes it when a window is moved, docked, resized, collapsed, or closed; recordings
/// never use it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewerLayout {
    /// By `WindowId::index`.
    pub windows: [WindowPlace; 3],
}

// Its numbers are whole points, never NaN: it is equal to itself.
impl Eq for ViewerLayout {}

impl Default for ViewerLayout {
    fn default() -> Self {
        ViewerLayout { windows: WindowId::ALL.map(WindowId::default_place) }
    }
}

impl ViewerLayout {
    pub fn window(&self, id: WindowId) -> &WindowPlace {
        &self.windows[id.index()]
    }

    pub fn window_mut(&mut self, id: WindowId) -> &mut WindowPlace {
        &mut self.windows[id.index()]
    }

    /// The key a window is written under.
    fn key(id: WindowId) -> String {
        format!("viewer_window_{}", id.key())
    }

    /// A window's line in the settings file, an inline table.
    fn entry(id: WindowId, p: &WindowPlace) -> String {
        let [x, y, w, h] = p.float.map(|v| v.round() as i32);
        format!("{} = {{ dock = \"{}\", size = {}, wide = {}, x = {x}, y = {y}, width = {w}, height = {h}, collapsed = {}, open = {} }}", Self::key(id), p.dock.key(), p.size.round() as i32, p.wide.round() as i32, p.collapsed, p.open)
    }

    /// A window's place as `document` has it (its default for what it leaves out).
    fn read(id: WindowId, document: &toml_edit::Document<String>, path: &Path) -> Result<WindowPlace> {
        let key = Self::key(id);
        let mut place = id.default_place();
        let Some(item) = document.get(&key) else { return Ok(place) };
        let table = item.as_inline_table().map(|t| t.iter().map(|(k, v)| (k.to_string(), v.clone())).collect::<Vec<_>>()).or_else(|| item.as_table().map(|t| t.iter().filter_map(|(k, v)| v.as_value().map(|v| (k.to_string(), v.clone()))).collect())).with_context(|| format!("{}: `{key}` must be a table", path.display()))?;
        let bad = |field: &str| anyhow::anyhow!("{}: `{key}.{field}` is not valid", path.display());
        for (field, value) in table {
            let number = || value.as_integer().filter(|n| (-100_000..=100_000).contains(n)).map(|n| n as f32).ok_or_else(|| bad(&field));
            match field.as_str() {
                "dock" => place.dock = value.as_str().and_then(Dock::parse).ok_or_else(|| bad(&field))?,
                "size" => place.size = number()?,
                "wide" => place.wide = number()?,
                "x" => place.float[0] = number()?,
                "y" => place.float[1] = number()?,
                "width" => place.float[2] = number()?,
                "height" => place.float[3] = number()?,
                "collapsed" => place.collapsed = value.as_bool().ok_or_else(|| bad(&field))?,
                "open" => place.open = value.as_bool().ok_or_else(|| bad(&field))?,
                _ => {}
            }
        }
        Ok(place)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    /// Interface: the homepage's language, and the edition of a narrated
    /// timeline it offers first (English when a feature has none in this language).
    pub language: Locale,
    /// Interface: the theme (src/theme.rs), by its file's stem.
    pub theme: String,
    /// Interface: how the code panel draws pseudo-code listings (src/pseudocode.rs).
    pub listing_style: Style,
    /// Render: open the homepage and the viewer in borderless fullscreen.
    pub fullscreen: bool,
    /// Audio: the homepage's music.
    pub music: Volume,
    /// Audio: the narration of a timeline in the viewer, which its dock also sets.
    pub narration: Volume,
    /// The viewer's layout, which dragging its edges sets (Restore Defaults puts it back too).
    pub layout: ViewerLayout,
}

impl Default for Config {
    fn default() -> Config {
        Config { language: Locale::EN, theme: theme::DEFAULT.into(), listing_style: Style::Blocks, fullscreen: false, music: Volume::new(50), narration: Volume::new(100), layout: ViewerLayout::default() }
    }
}

impl Config {
    /// Whether `self` and `other` differ in `category`'s settings.
    pub fn differs_in(&self, other: &Config, category: Category) -> bool {
        match category {
            Category::Interface => self.language != other.language || self.theme != other.theme || self.listing_style != other.listing_style,
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
            let codes = Locale::all().into_iter().map(Locale::code).collect::<Vec<_>>().join(", ");
            config.language = Locale::from_code(code).with_context(|| format!("{}: unknown `language` \"{code}\" (one of {codes})", path.display()))?;
        }
        if let Some(item) = document.get("theme") {
            config.theme = item.as_str().with_context(|| format!("{}: `theme` must be a string", path.display()))?.to_string();
        }
        if let Some(item) = document.get("listing_style") {
            let key = item.as_str().with_context(|| format!("{}: `listing_style` must be a string", path.display()))?;
            config.listing_style = Style::from_key(key).with_context(|| format!("{}: unknown `listing_style` \"{key}\" (text or blocks)", path.display()))?;
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
        let mut layout = ViewerLayout::default();
        for id in WindowId::ALL {
            layout.windows[id.index()] = ViewerLayout::read(id, &document, path)?;
        }
        config.layout = layout;
        Ok(config)
    }

    pub fn to_toml(&self) -> String {
        let mut text = format!(
            "# Render Lab settings, written by the homepage's Config page.\n\n# Interface: the homepage's language and the narrated edition it offers first ({}).\nlanguage = \"{}\"\n\n# Interface: the theme, a file's stem in a themes/ folder beside the binary or beside this file.\ntheme = \"{}\"\n\n# Interface: how the code panel draws pseudo-code listings: blocks, or text.\nlisting_style = \"{}\"\n\n# Render: open the homepage and the viewer in borderless fullscreen.\nfullscreen = {}\n",
            Locale::all().into_iter().map(Locale::code).collect::<Vec<_>>().join(", "),
            self.language.code(),
            self.theme.replace('\\', "\\\\").replace('"', "\\\""),
            self.listing_style.key(),
            self.fullscreen
        );
        for (name, what, volume) in [("music", "Audio: the homepage's music", self.music), ("narration", "Audio: the narration", self.narration)] {
            text.push_str(&format!("\n# {what}: its volume (0 to 100) and whether it is muted.\n{name}_volume = {}\n{name}_muted = {}\n", volume.percent, volume.muted));
        }
        let windows: Vec<String> = WindowId::ALL.into_iter().filter(|id| self.layout.window(*id) != &id.default_place()).map(|id| ViewerLayout::entry(id, self.layout.window(id))).collect();
        if !windows.is_empty() {
            text.push_str(&format!("\n# The viewer's windows as they were left: docked to an edge (left, right, bottom; its size) or floating (x, y, width, height), in points of the 1920 x 1080 canvas (absent: the default).\n{}\n", windows.join("\n")));
        }
        text
    }

    /// Write `layout` into the settings in `path`, keeping the others as the file has them.
    pub fn save_layout(path: &Path, layout: ViewerLayout) -> Result<()> {
        Config { layout, ..Config::load(path)? }.save(path)
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
        let config = Config { fullscreen: true, language: Locale::from_code("zh-CN").unwrap(), theme: "tokyo-night".into(), listing_style: Style::Text, music: Volume { percent: 30, muted: true }, narration: Volume::new(75), layout: {
            let mut layout = ViewerLayout::default();
            *layout.window_mut(WindowId::Panel) = WindowPlace { dock: Dock::Float, float: [300.0, 200.0, 700.0, 500.0], collapsed: true, ..WindowId::Panel.default_place() };
            layout.window_mut(WindowId::Controls).size = 300.0;
            layout
        } };
        config.save(&path).unwrap();
        assert_eq!(Config::load(&path).unwrap(), config);
        // A file of an older lab: what it does not name keeps its default.
        std::fs::write(&path, "fullscreen = true\n").unwrap();
        assert_eq!(Config::load(&path).unwrap(), Config { fullscreen: true, ..Config::default() });
        std::fs::write(&path, "music_volume = 101\n").unwrap();
        assert!(Config::load(&path).is_err());
        std::fs::write(&path, "language = \"fr\"\n").unwrap();
        assert!(Config::load(&path).is_err());
        std::fs::write(&path, "viewer_window_panel = { dock = \"top\" }\n").unwrap();
        assert!(Config::load(&path).is_err());
        std::fs::write(&path, "listing_style = \"scratch\"\n").unwrap();
        assert!(Config::load(&path).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
