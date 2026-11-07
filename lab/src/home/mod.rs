//! The homepage: the window `render-lab` opens without a command, styled like
//! a game's title screen. Behind it, the current investigation's scene renders
//! full-screen under frosted glass (src/frost.rs).
//!
//! - While the scene behind it loads, the window shows the loading page
//!   (src/loading.rs); the title page follows.
//! - The title page: "Render Lab", Play, Config, and Quit, the language (a
//!   globe) in the top-right corner, which opens the Config page at the
//!   Language option, and the copyright, license, and version in the
//!   bottom-left corner. A photosensitivity warning covers it at start, and
//!   again after a saved setting changes the language, until Continue.
//! - The Config page edits the settings file (src/config.rs), a tab per
//!   category: the interface (the language of the homepage, and of the
//!   narrated edition it offers first, and the theme, src/theme.rs), the
//!   render settings (full screen), and the audio (the volumes of the music
//!   and the narration). Save and Apply applies and writes the interface's
//!   and the audio's at once; a change to the render settings is tried for
//!   ten seconds behind a popup: Apply keeps it and writes the file;
//!   Restore, or letting the time run out, puts the previous one back.
//!   Nothing is written before Save and Apply, except the narration's volume
//!   as the viewer's dock leaves it.
//! - Play opens the selection page: the engines the lab investigates, the
//!   chosen engine's features, and the chosen feature's narrated timeline
//!   (from the start or from a chapter) and demonstrations. Esc or Back
//!   returns to the title.
//!
//! F11 saves a screenshot of the window, here and in the viewer
//! (build/screenshots). Opening a timeline or the demonstrations runs the viewer on the same event
//! loop; Esc in the viewer comes back to the selection page, and closing a
//! window quits. The page is drawn by `Home::draw`, which the window and the
//! offscreen `--snapshot` and `--autotest` modes share, so the page can be
//! checked without opening a window.

use crate::investigations;
use crate::config::{Category, Config};
use crate::egui_pass::{PagePainter, PageTarget};
use crate::frost::Frost;
use crate::gpu::{self, Gpu, FRAME_FORMAT};
use crate::i18n::{self, args, tr, tr_with, Locale};
use crate::investigation::{self, Investigation, TimelineInfo};
use crate::loading::{self, Loading, Progress};
use crate::palette;
use crate::popup::{self, Popup, Popups};
use crate::resources;
use crate::theme;
use crate::readback::{copy_to_readback, read_rgba, readback_buffer};
use crate::renderer::Renderer;
use crate::text::{self as ui, canvas_for};
use crate::audio::AudioPlayer;
use crate::viewer::{self, ViewerExit, ViewerOptions};
use crate::volume::{self, Volume};
use crate::window::fullscreen_mode;
use anyhow::{Context, Result};
use egui::{pos2, vec2, Align2, Color32, CornerRadius, FontFamily, FontId, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Vec2};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::platform::run_on_demand::EventLoopExtRunOnDemand;
use winit::window::{Window, WindowId};

mod widgets;
mod pages;
mod painter;
mod snapshot;
mod app;
use widgets::*;
use painter::*;
pub use snapshot::*;
pub use app::*;

/// The bottom-left line of the title page.
fn credits() -> String {
    format!("© 2026 OppositeNor · {} · v{}", tr("home-mit_license"), env!("CARGO_PKG_VERSION"))
}

pub struct HomeOptions {
    pub manifest_dir: PathBuf,
    pub width: u32,
    pub height: u32,
    /// Size of the text and panels relative to the video's (as in the viewer).
    pub ui_scale: f32,
    /// The settings file (`Config::default_path`); None keeps settings for the session only.
    pub config_path: Option<PathBuf>,
}

/// How long tried render settings stay before they are put back unless kept.
const TRIAL_SECONDS: f32 = 10.0;

/// How long the Language option stays outlined after the title's globe opened the page.
const FLASH_SECONDS: f32 = 2.5;

/// Render settings being tried: what to put back, and since when (page seconds).
#[derive(Clone, Debug)]
struct Trial {
    started: f32,
    /// The session's settings before the tried ones were applied.
    before: Config,
}

/// The Config page's state.
struct Settings {
    path: Option<PathBuf>,
    /// What the file holds (the defaults when there is none).
    saved: Config,
    /// What the page shows and edits.
    pending: Config,
    trial: Option<Trial>,
    /// The last outcome (kept, restored, or an error), shown on the page.
    message: Option<String>,
    /// When the title's globe opened the page (page seconds): the Language
    /// option is outlined for a moment.
    flash: Option<f32>,
    /// The tab shown.
    category: Category,
    /// The themes offered (read when the page opens), and why a file was left out.
    themes: Vec<theme::Entry>,
    theme_errors: Vec<String>,
}

/// Which page the homepage shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Title,
    Select,
    Config,
}

impl Page {
    pub fn parse(name: &str) -> Result<Page> {
        match name {
            "title" => Ok(Page::Title),
            "select" => Ok(Page::Select),
            "config" => Ok(Page::Config),
            _ => anyhow::bail!("unknown homepage page `{name}` (title, select, or config)"),
        }
    }
}

/// What the homepage opens.
#[derive(Clone, Copy)]
pub enum Choice {
    /// Play a narrated timeline, from `start_frame`.
    Watch { investigation: &'static dyn Investigation, timeline: &'static TimelineInfo, start_frame: u64 },
    /// The free demonstrations.
    Explore { investigation: &'static dyn Investigation },
    /// The demonstrations on a feature's tab (src/feature/), as its homepage button opens them.
    Feature { investigation: &'static dyn Investigation, feature: &'static dyn crate::feature::Feature },
    Quit,
}

impl Choice {
    /// For reports: `watch engine/feature timeline @ 1234`, `explore engine/feature`, `quit`.
    pub fn describe(&self) -> String {
        match self {
            Choice::Watch { investigation, timeline, start_frame } => format!("watch {} {} @ {start_frame}", investigation.path(), timeline.id),
            Choice::Explore { investigation } => format!("explore {}", investigation.path()),
            Choice::Feature { investigation, feature } => format!("{} {}", feature.id(), investigation.path()),
            Choice::Quit => "quit".into(),
        }
    }
}

/// What the homepage shows of a timeline, read from its JSON.
struct TimelineSummary {
    title: String,
    fps: u32,
    total_frames: u64,
    chapters: Vec<Chapter>,
    narrated: bool,
    locale: Locale,
}

impl TimelineSummary {
    fn load(manifest_dir: &Path, info: &TimelineInfo) -> Result<TimelineSummary> {
        let path = manifest_dir.join(&info.path);
        let text = std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        let json: serde_json::Value = serde_json::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
        let chapters = json["chapters"]
            .as_array()
            .map(|cs| cs.iter().map(|c| (c["number"].as_u64().unwrap_or(0) as u32, c["title"].as_str().unwrap_or("").to_string(), c["startFrame"].as_u64().unwrap_or(0))).collect())
            .unwrap_or_default();
        Ok(TimelineSummary {
            title: json["title"].as_str().unwrap_or(&info.title).to_string(),
            fps: json["fps"].as_u64().unwrap_or(30).max(1) as u32,
            total_frames: json["totalFrames"].as_u64().unwrap_or(0),
            chapters,
            narrated: manifest_dir.join(&info.narration).is_file(),
            locale: serde_json::from_value(json["locale"].clone()).unwrap_or_default(),
        })
    }

    fn clock(&self, frame: u64) -> String {
        let s = frame / self.fps as u64;
        format!("{}:{:02}", s / 60, s % 60)
    }
}

/// A chapter as a timeline lists it: its number, its title (on the
/// selection page, its clock and title), and its first frame.
type Chapter = (u32, String, u64);

/// A timeline as the selection page lists it.
struct TimelineRow {
    info: &'static TimelineInfo,
    /// Title, length, chapters (number, "clock  title", first frame), and
    /// whether the narration is built; or why the timeline cannot be read.
    summary: Result<(String, String, Vec<Chapter>, bool), String>,
    /// Its language, once it has been read.
    locale: Option<Locale>,
}

/// The edition a feature offers first: the first in `language`, else the
/// first in English, else the first.
fn preferred_edition(rows: &[TimelineRow], language: Locale) -> usize {
    let first = |locale: Locale| rows.iter().position(|r| r.locale == Some(locale));
    first(language).or_else(|| first(Locale::EN)).unwrap_or(0)
}

/// An entry of the selection page's Feature column: an investigation, or a
/// series standing for its parts (src/packages.rs, `Series`).
#[derive(Clone, Copy)]
enum Feature {
    One(&'static dyn Investigation),
    Series(&'static crate::packages::Series),
}

/// A series' parts that are installed, in order.
fn parts_of(series: &crate::packages::Series) -> Vec<&'static dyn Investigation> {
    series.parts.iter().filter_map(|p| investigations::all().iter().copied().find(|i| i.package() == Some(p.as_str()))).collect()
}

/// The homepage's state: the page, the selection, and what the chosen
/// feature's timelines hold.
pub struct Home {
    manifest_dir: PathBuf,
    pub page: Page,
    /// The settings in use this session: the saved ones, then tried ones.
    pub applied: Config,
    settings: Settings,
    engine: usize,
    feature: usize,
    /// The part chosen when the feature is a series.
    part: usize,
    /// Per investigation (by `investigations::all()` index), per timeline.
    timelines: Vec<Option<Vec<Result<TimelineSummary, String>>>>,
    /// The timeline (edition) picked on the selection page, by index; until
    /// one is, the one in the language (`preferred_edition`).
    edition: Option<usize>,
    /// The title's globe icon, once loaded (None inside if it could not be).
    globe: Option<Option<egui::TextureHandle>>,
    /// An error from the last viewer run, shown until the next choice.
    pub error: Option<String>,
    /// Canvas rectangle of every control, by name (for the autotest).
    pub widgets: Vec<(String, Rect)>,
    /// The smallest text size drawn (points at the 1080p reference).
    pub min_text: f32,
    /// Whether a control was clicked since the window last played a click.
    pub clicked: bool,
    /// Messages over the page (src/popup.rs), such as where a screenshot went.
    pub popups: Popups,
    /// The photosensitivity warning is up: at start, and again when a saved
    /// setting changes the language, over the page until Continue.
    pub pse: bool,
    /// The chosen feature's knowledge graph is up, full screen (src/knowledge.rs).
    pub knowledge: bool,
}

impl Home {
    pub fn new(manifest_dir: PathBuf, config_path: Option<PathBuf>) -> Home {
        // A file that cannot be read leaves the defaults, and the Config page says why.
        let (saved, message) = match config_path.as_deref().map(Config::load).transpose() {
            Ok(config) => (config.unwrap_or_default(), None),
            Err(e) => (Config::default(), Some(format!("{e:#}"))),
        };
        let (themes, theme_errors) = theme::list();
        let settings = Settings { path: config_path, saved: saved.clone(), pending: saved.clone(), trial: None, message, flash: None, category: Category::Interface, themes, theme_errors };
        i18n::set(saved.language);
        crate::pseudocode::set_style(saved.listing_style);
        let message = theme::apply(&saved.theme).err().map(|e| format!("{e:#}"));
        let mut home = Home {
            manifest_dir,
            page: Page::Title,
            applied: saved,
            settings,
            engine: 0,
            feature: 0,
            part: 0,
            timelines: investigations::all().iter().map(|_| None).collect(),
            edition: None,
            globe: None,
            error: None,
            widgets: Vec::new(),
            min_text: f32::INFINITY,
            clicked: false,
            popups: Popups::default(),
            pse: true,
            knowledge: false,
        };
        if message.is_some() {
            home.settings.message = message;
        }
        // Start on the investigation chosen on the command line.
        let current = investigation::current().path();
        if let Some(chosen) = investigations::find(&current) {
            home.select(chosen);
        }
        home
    }

    /// The chosen engine's Feature column: its investigations, a series in
    /// place of its parts (where the first of them would be).
    fn features(&self) -> Vec<Feature> {
        let engine = investigations::engines()[self.engine].id;
        let mut features: Vec<Feature> = Vec::new();
        for i in investigations::all().iter().copied().filter(|i| i.engine().id == engine) {
            match i.package().and_then(crate::resources::part_of) {
                Some((series, _)) => {
                    if !features.iter().any(|f| matches!(f, Feature::Series(s) if std::ptr::eq(*s, series))) {
                        features.push(Feature::Series(series));
                    }
                }
                None => features.push(Feature::One(i)),
            }
        }
        features
    }

    fn selected(&self) -> &'static dyn Investigation {
        let features = self.features();
        match features[self.feature.min(features.len() - 1)] {
            Feature::One(i) => i,
            Feature::Series(series) => {
                let parts = parts_of(series);
                parts[self.part.min(parts.len() - 1)]
            }
        }
    }

    fn timeline_rows(&mut self, chosen: &'static dyn Investigation) -> Vec<TimelineRow> {
        let index = investigations::all().iter().position(|i| i.path() == chosen.path()).unwrap_or(0);
        let dir = self.manifest_dir.clone();
        let loaded = self.timelines[index].get_or_insert_with(|| chosen.timelines().iter().map(|info| TimelineSummary::load(&dir, info).map_err(|e| format!("{e:#}"))).collect());
        chosen
            .timelines()
            .iter()
            .zip(loaded.iter())
            .map(|(info, s)| TimelineRow {
                info,
                locale: s.as_ref().ok().map(|s| s.locale),
                summary: s
                    .as_ref()
                    .map(|s| (s.title.clone(), s.clock(s.total_frames), s.chapters.iter().map(|(n, title, f)| (*n, format!("{}  {title}", s.clock(*f)), *f)).collect(), s.narrated))
                    .map_err(|e| e.clone()),
            })
            .collect()
    }

    /// Esc: tried settings are put back; the Config page discards its edits
    /// and, like the selection page, returns to the title. False on the title.
    pub fn back(&mut self) -> bool {
        // The full-screen knowledge graph closes first.
        if self.knowledge {
            self.knowledge = false;
            return true;
        }
        match self.page {
            Page::Title => false,
            Page::Config if self.settings.trial.is_some() => {
                self.restore_settings();
                true
            }
            Page::Config => {
                self.discard_settings();
                true
            }
            Page::Select => {
                self.page = Page::Title;
                true
            }
        }
    }

    /// The Config page, at `category`; the themes are read again.
    fn open_config(&mut self, category: Category) {
        self.page = Page::Config;
        self.settings.category = category;
        (self.settings.themes, self.settings.theme_errors) = theme::list();
    }

    /// Use `config` this session: its language, its theme, and the rest
    /// (which the window and the viewer read from `applied`).
    fn apply(&mut self, config: Config) {
        // The warning again, in the language just chosen.
        self.pse |= config.language != self.applied.language;
        i18n::set(config.language);
        crate::pseudocode::set_style(config.listing_style);
        if config.theme != self.applied.theme && let Err(e) = theme::apply(&config.theme) {
            self.settings.message = Some(format!("{e:#}"));
        }
        self.applied = config;
    }

    /// Write `config` to the settings file; the message says where, or why not.
    fn save(&mut self, config: Config) {
        self.settings.message = Some(match &self.settings.path {
            Some(path) => match config.save(path) {
                Ok(()) => {
                    self.settings.saved = config;
                    format!("{} {}", tr("home-saved_to"), path.display())
                }
                Err(e) => format!("{} {e:#}", tr("home-applied_but_not_saved")),
            },
            None => {
                self.settings.saved = config;
                tr("home-applied_for_session").to_string()
            }
        });
    }

    /// Save and Apply: the interface's and the audio's settings are applied
    /// and written at once; changed render settings are tried until they
    /// are kept or put back.
    fn try_settings(&mut self, time: f32) {
        let pending = self.settings.pending.clone();
        self.settings.message = None;
        self.apply(pending.with_render_of(&self.applied));
        // A theme that did not load says so over where the file went.
        let theme_error = self.settings.message.take();
        let kept = pending.with_render_of(&self.settings.saved);
        if kept != self.settings.saved {
            self.save(kept);
        }
        if pending.differs_in(&self.applied, Category::Render) {
            self.settings.trial = Some(Trial { started: time, before: self.applied.clone() });
            self.apply(pending);
        }
        if theme_error.is_some() {
            self.settings.message = theme_error;
        }
    }

    /// The popup's Apply: keep the tried settings and write them to the file.
    fn keep_settings(&mut self) {
        if self.settings.trial.take().is_some() {
            self.save(self.settings.pending.clone());
        }
    }

    /// The popup's Restore, or the time running out: put the previous render settings back.
    fn restore_settings(&mut self) {
        if let Some(trial) = self.settings.trial.take() {
            self.apply(trial.before);
            self.settings.pending = self.settings.saved.clone();
            self.settings.message = Some(tr("home-restored_the_previous_settings").to_string());
        }
    }

    /// Discard: forget the edits and leave the page.
    fn discard_settings(&mut self) {
        self.settings.pending = self.settings.saved.clone();
        self.settings.message = None;
        self.settings.flash = None;
        self.page = Page::Title;
    }

    /// The narration's volume as the viewer's dock left it: in use, and
    /// written to the settings file (the Config page has no edits then).
    /// The layout the viewer was left with (it wrote the file as each drag ended).
    fn keep_layout(&mut self, layout: crate::config::ViewerLayout) {
        self.applied.layout = layout;
        self.settings.saved.layout = layout;
        self.settings.pending.layout = layout;
    }

    fn keep_narration(&mut self, narration: Volume) {
        if narration == self.applied.narration {
            return;
        }
        self.applied.narration = narration;
        self.settings.saved.narration = narration;
        self.settings.pending.narration = narration;
        if let Some(path) = &self.settings.path && let Err(e) = self.settings.saved.save(path) {
            self.settings.message = Some(format!("{} {e:#}", tr("home-applied_but_not_saved")));
        }
    }

    /// The globe icon's texture, loaded on first use; None if it cannot be read.
    fn globe(&mut self, ctx: &egui::Context) -> Option<egui::TextureId> {
        self.globe
            .get_or_insert_with(|| {
                let path = resources::get(GLOBE).path();
                let image = std::fs::File::open(&path).map_err(anyhow::Error::from).and_then(|f| crate::record::decode_png(std::io::BufReader::new(f)));
                match image {
                    Ok(image) => Some(ctx.load_texture("home-globe", image, egui::TextureOptions::LINEAR)),
                    Err(e) => {
                        eprintln!("the globe icon did not load from {}: {e:#}", path.display());
                        None
                    }
                }
            })
            .as_ref()
            .map(|t| t.id())
    }

}
