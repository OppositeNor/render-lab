//! The homepage: the window `render-lab` opens without a command, styled like
//! a game's title screen. Behind it, the current investigation's scene renders
//! full-screen under frosted glass (src/frost.rs).
//!
//! - While the scene behind it loads, the window shows the loading page
//!   (src/loading.rs); the title page follows.
//! - The title page: "Render Lab", Play, Config, and Quit, the language (a
//!   globe) in the top-right corner, which opens the Config page at the
//!   Language option, and the copyright, license, and version in the
//!   bottom-left corner.
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
use crate::frost::Frost;
use crate::gpu::{self, Gpu, FRAME_FORMAT};
use crate::i18n::{self, args, tr, tr_with, Locale};
use crate::investigation::{self, Investigation, TimelineInfo};
use crate::loading::{self, Loading, Progress};
use crate::palette;
use crate::popup::{self, Popup, Popups};
use crate::resources;
use crate::theme;
use crate::readback::{padded_row_bytes, read_buffer, readback_buffer, unpad_rows};
use crate::renderer::Renderer;
use crate::text::{self as ui, REFERENCE};
use crate::audio::AudioPlayer;
use crate::viewer::{self, ViewerExit, ViewerOptions};
use crate::volume::{self, Volume};
use anyhow::{Context, Result};
use egui::{pos2, vec2, Align2, Color32, CornerRadius, FontFamily, FontId, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Vec2};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::platform::run_on_demand::EventLoopExtRunOnDemand;
use winit::window::{Window, WindowId};

/// Borderless fullscreen on the window's current monitor, or windowed.
pub fn fullscreen_mode(on: bool) -> Option<winit::window::Fullscreen> {
    on.then_some(winit::window::Fullscreen::Borderless(None))
}

/// The bottom-left line of the title page.
fn credits() -> String {
    format!("© 2026 OppositeNor · {} · v{}", tr("home.mit_license"), env!("CARGO_PKG_VERSION"))
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
    Quit,
}

impl Choice {
    /// For reports: `watch engine/feature timeline @ 1234`, `explore engine/feature`, `quit`.
    pub fn describe(&self) -> String {
        match self {
            Choice::Watch { investigation, timeline, start_frame } => format!("watch {} {} @ {start_frame}", investigation.path(), timeline.id),
            Choice::Explore { investigation } => format!("explore {}", investigation.path()),
            Choice::Quit => "quit".into(),
        }
    }
}

/// What the homepage shows of a timeline, read from its JSON.
struct TimelineSummary {
    title: String,
    fps: u32,
    total_frames: u64,
    /// Number, title, and first frame of each chapter.
    chapters: Vec<(u32, String, u64)>,
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

/// A timeline as the selection page lists it.
struct TimelineRow {
    info: &'static TimelineInfo,
    /// Title, length, chapters (number, "clock  title", first frame), and
    /// whether the narration is built; or why the timeline cannot be read.
    summary: Result<(String, String, Vec<(u32, String, u64)>, bool), String>,
    /// Its language, once it has been read.
    locale: Option<Locale>,
}

/// The edition a feature offers first: the first in `language`, else the
/// first in English, else the first.
fn preferred_edition(rows: &[TimelineRow], language: Locale) -> usize {
    let first = |locale: Locale| rows.iter().position(|r| r.locale == Some(locale));
    first(language).or_else(|| first(Locale::En)).unwrap_or(0)
}

/// An option's value as the Config page names it.
fn on_off(on: bool) -> &'static str {
    if on { tr("home.on") } else { tr("home.off") }
}

/// Translucent white: the glass the page's panels and buttons are made of.
fn glass(alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(255, 255, 255, alpha)
}

/// A frosted-glass panel; `smoked` darkens it, for panels full of text.
fn glass_panel(painter: &egui::Painter, rect: Rect, radius: u8, smoked: bool) {
    let fill = if smoked { palette::theme().smoke } else { glass(14) };
    painter.rect_filled(rect, CornerRadius::same(radius), fill);
    painter.rect_stroke(rect, CornerRadius::same(radius), Stroke::new(1.5, glass(48)), StrokeKind::Inside);
}

/// A glass button with its label centred; it brightens under the pointer.
fn glass_button(ui: &mut egui::Ui, size: Vec2, label: &str, font: f32, enabled: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(size, if enabled { Sense::click() } else { Sense::hover() });
    let hot = enabled && response.hovered();
    let fill = if enabled && response.is_pointer_button_down_on() {
        52
    } else if hot {
        38
    } else {
        20
    };
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(16), glass(fill));
    painter.rect_stroke(rect, CornerRadius::same(16), Stroke::new(if hot { 2.0 } else { 1.5 }, if hot { palette::accent() } else { glass(70) }), StrokeKind::Inside);
    let color = if enabled { palette::text() } else { palette::text_faint() };
    painter.text(rect.center(), Align2::CENTER_CENTER, label, FontId::new(font, ui::bold()), color);
    response
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
        let message = theme::apply(&saved.theme).err().map(|e| format!("{e:#}"));
        let mut home = Home {
            manifest_dir,
            page: Page::Title,
            applied: saved,
            settings,
            engine: 0,
            feature: 0,
            timelines: investigations::all().iter().map(|_| None).collect(),
            edition: None,
            globe: None,
            error: None,
            widgets: Vec::new(),
            min_text: f32::INFINITY,
            clicked: false,
            popups: Popups::default(),
        };
        if message.is_some() {
            home.settings.message = message;
        }
        // Start on the investigation chosen on the command line.
        let current = investigation::current().path();
        if let Some(chosen) = investigations::find(&current) {
            home.engine = investigations::engines().iter().position(|e| e.id == chosen.engine().id).unwrap_or(0);
            home.feature = home.features().iter().position(|f| f.path() == current).unwrap_or(0);
        }
        home
    }

    fn features(&self) -> Vec<&'static dyn Investigation> {
        let engine = investigations::engines()[self.engine].id;
        investigations::all().iter().copied().filter(|i| i.engine().id == engine).collect()
    }

    fn selected(&self) -> &'static dyn Investigation {
        let features = self.features();
        features[self.feature.min(features.len() - 1)]
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
        i18n::set(config.language);
        if config.theme != self.applied.theme {
            if let Err(e) = theme::apply(&config.theme) {
                self.settings.message = Some(format!("{e:#}"));
            }
        }
        self.applied = config;
    }

    /// Write `config` to the settings file; the message says where, or why not.
    fn save(&mut self, config: Config) {
        self.settings.message = Some(match &self.settings.path {
            Some(path) => match config.save(path) {
                Ok(()) => {
                    self.settings.saved = config;
                    format!("{} {}", tr("home.saved_to"), path.display())
                }
                Err(e) => format!("{} {e:#}", tr("home.applied_but_not_saved")),
            },
            None => {
                self.settings.saved = config;
                tr("home.applied_for_session").to_string()
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
            self.settings.message = Some(tr("home.restored_the_previous_settings").to_string());
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
    fn keep_narration(&mut self, narration: Volume) {
        if narration == self.applied.narration {
            return;
        }
        self.applied.narration = narration;
        self.settings.saved.narration = narration;
        self.settings.pending.narration = narration;
        if let Some(path) = &self.settings.path {
            if let Err(e) = self.settings.saved.save(path) {
                self.settings.message = Some(format!("{} {e:#}", tr("home.applied_but_not_saved")));
            }
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

    /// Draw the page on a canvas of `canvas` points; returns the choice made.
    /// `time`: the page's clock in seconds (the Config popup counts down on it).
    pub fn draw(&mut self, ctx: &egui::Context, canvas: Vec2, time: f32) -> Option<Choice> {
        self.widgets.clear();
        // Any enabled control: the glass buttons, the lists, the checkbox. Only set here
        // (a frame may take more than one pass, and only the first sees the click).
        self.clicked |= ctx.viewport(|v| v.interact_widgets.clicked.is_some());
        let painter = ctx.layer_painter(egui::LayerId::background());
        let ink = ui::Ink::new(&painter);
        let choice = match self.page {
            Page::Title => self.draw_title(ctx, &ink, canvas, time),
            Page::Select => self.draw_select(ctx, &ink, canvas),
            Page::Config => {
                self.draw_config(ctx, &ink, canvas, time);
                None
            }
        };
        self.popups.draw(ctx, canvas);
        self.min_text = self.min_text.min(ink.min_size.get());
        if choice.is_some() {
            self.error = None;
        }
        choice
    }

    /// The title page: the name, Play and Quit on a glass card, the language, and the credits.
    fn draw_title(&mut self, ctx: &egui::Context, ink: &ui::Ink, canvas: Vec2, time: f32) -> Option<Choice> {
        let center = canvas.x / 2.0;
        let card = Rect::from_center_size(pos2(center, canvas.y * 0.5), vec2(900.0, 710.0));
        glass_panel(ink.painter, card, 28, false);
        ink.text(pos2(center, card.min.y + 64.0), Align2::CENTER_TOP, "Render Lab", 128.0, ui::bold(), palette::text());
        let rule = card.min.y + 230.0;
        ink.painter.line_segment([pos2(center - 150.0, rule), pos2(center + 150.0, rule)], Stroke::new(2.0, palette::accent()));
        ink.text(pos2(center, rule + 22.0), Align2::CENTER_TOP, tr("home.subtitle"), 28.0, FontFamily::Proportional, palette::text_dim());
        let button = vec2(460.0, 84.0);
        let mut choice = None;
        egui::Area::new(egui::Id::unique("home-title")).fixed_pos(pos2(center - button.x / 2.0, rule + 96.0)).fade_in(false).constrain(false).show(ctx, |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 22.0);
            let play = glass_button(ui, button, tr("home.play"), 40.0, true);
            self.widgets.push(("play".into(), play.rect));
            if play.clicked() {
                self.page = Page::Select;
            }
            let config = glass_button(ui, button, tr("home.config"), 40.0, true);
            self.widgets.push(("config".into(), config.rect));
            if config.clicked() {
                self.open_config(Category::Interface);
            }
            let quit = glass_button(ui, button, tr("home.quit"), 40.0, true);
            self.widgets.push(("quit".into(), quit.rect));
            if quit.clicked() {
                choice = Some(Choice::Quit);
            }
        });
        ink.text(pos2(40.0, canvas.y - 52.0), Align2::LEFT_TOP, credits(), 24.0, FontFamily::Proportional, palette::text_faint());

        // The language, at the top right: the globe and the language's name. It opens the
        // Config page with the Language option outlined.
        let globe = self.globe(ctx);
        let name = self.applied.language.name();
        let icon = 44.0;
        let size = vec2(20.0 + icon + 14.0 + ink.measure(name, 28.0, ui::bold()).x + 24.0, 64.0);
        egui::Area::new(egui::Id::unique("home-language")).fixed_pos(pos2(canvas.x - 40.0 - size.x, 30.0)).fade_in(false).constrain(false).show(ctx, |ui| {
            let r = glass_button(ui, size, "", 28.0, true);
            let color = if r.hovered() { palette::accent() } else { palette::text() };
            let icon_rect = Rect::from_min_size(pos2(r.rect.min.x + 20.0, r.rect.center().y - icon / 2.0), vec2(icon, icon));
            if let Some(globe) = globe {
                ui.painter().image(globe, icon_rect, Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)), color);
            }
            ui.painter().text(pos2(icon_rect.max.x + 14.0, r.rect.center().y), Align2::LEFT_CENTER, name, FontId::new(28.0, ui::bold()), color);
            self.widgets.push(("language".into(), r.rect));
            if r.on_hover_text(tr("home.language_hint")).clicked() {
                self.open_config(Category::Interface);
                self.settings.flash = Some(time);
            }
        });
        choice
    }

    /// The selection page: engines, the chosen engine's features, and the
    /// chosen feature's timelines and demonstrations, on glass panels.
    fn draw_select(&mut self, ctx: &egui::Context, ink: &ui::Ink, canvas: Vec2) -> Option<Choice> {
        let mut choice = None;
        let chosen = self.selected();
        // The chrome's strings follow the chosen feature.
        investigation::set(chosen);
        let painter = ink.painter;

        ink.text(pos2(40.0, 20.0), Align2::LEFT_TOP, "RENDER LAB", 24.0, ui::bold(), palette::accent());
        ink.text(pos2(38.0, 52.0), Align2::LEFT_TOP, tr("home.choose"), 38.0, ui::bold(), palette::text());
        let back_size = vec2(200.0, 60.0);
        egui::Area::new(egui::Id::unique("home-back")).fixed_pos(pos2(canvas.x - 40.0 - back_size.x, 30.0)).fade_in(false).constrain(false).show(ctx, |ui| {
            let back = glass_button(ui, back_size, tr("home.back"), 28.0, true);
            self.widgets.push(("back".into(), back.rect));
            if back.clicked() {
                self.page = Page::Title;
            }
        });
        let top = 130.0;
        let bottom = canvas.y - 90.0;
        let column = |x0: f32, x1: f32| Rect::from_min_max(pos2(x0, top), pos2(x1, bottom));
        let engines_rect = column(40.0, 420.0);
        let features_rect = column(444.0, 824.0);
        let detail_rect = column(848.0, canvas.x - 40.0);
        for r in [engines_rect, features_rect, detail_rect] {
            glass_panel(painter, r, 18, true);
        }

        // Engines.
        let engines = investigations::engines();
        let mut new_engine = None;
        self.column_area(ctx, "home-engines", engines_rect, tr("home.engine"), |home, ui| {
            for (i, engine) in engines.iter().enumerate() {
                let r = ui.add(egui::Button::selectable(i == home.engine, RichText::new(engine.name).font(FontId::new(30.0, ui::bold()))).min_size(vec2(ui.available_width(), 56.0)));
                home.widgets.push((format!("engine:{}", engine.id), r.rect));
                if r.clicked() {
                    new_engine = Some(i);
                }
                if i == home.engine {
                    ui.add_space(4.0);
                    ui.add(egui::Label::new(RichText::new(engine.summary).color(palette::text_dim())).wrap());
                    ui.add_space(12.0);
                }
            }
        });

        // Features of the chosen engine, with the version each was read from.
        let features = self.features();
        let mut new_feature = None;
        self.column_area(ctx, "home-features", features_rect, tr("home.feature"), |home, ui| {
            for (i, feature) in features.iter().enumerate() {
                let r = ui.add(egui::Button::selectable(i == home.feature, RichText::new(feature.title()).font(FontId::new(30.0, ui::bold()))).min_size(vec2(ui.available_width(), 56.0)));
                home.widgets.push((format!("feature:{}", feature.path()), r.rect));
                if r.clicked() {
                    new_feature = Some(i);
                }
                let engine = &feature.sources().engine;
                let commit = engine.commit.get(..8).unwrap_or(&engine.commit);
                ui.label(RichText::new(format!("{} {} · {commit}", engine.name, engine.version)).color(palette::text_faint()));
                ui.add_space(12.0);
            }
        });

        // The chosen feature: what it covers, how to open it, its chapters and demonstrations.
        let rows = self.timeline_rows(chosen);
        let edition = self.edition.unwrap_or_else(|| preferred_edition(&rows, self.applied.language)).min(rows.len().saturating_sub(1));
        let mut new_edition = None;
        let error = self.error.clone();
        self.column_area(ctx, "home-detail", detail_rect, chosen.title(), |home, ui| {
            ui.add(egui::Label::new(RichText::new(chosen.summary()).color(palette::text())).wrap());
            ui.add_space(14.0);
            if let Some(e) = &error {
                ui.add(egui::Label::new(RichText::new(e).color(palette::HISTORY_REJECT)).wrap());
                ui.add_space(10.0);
            }
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 16.0;
                let first = rows.get(edition).filter(|r| r.summary.is_ok());
                let button = vec2((ui.available_width() - 16.0) / 2.0, 64.0);
                let watch = glass_button(ui, button, tr("home.watch_the_narrated_video"), 28.0, first.is_some());
                home.widgets.push(("watch".into(), watch.rect));
                if let (true, Some(row)) = (watch.clicked(), first) {
                    choice = Some(Choice::Watch { investigation: chosen, timeline: row.info, start_frame: 0 });
                }
                let explore = glass_button(ui, button, tr("home.explore_the_demonstrations"), 28.0, true);
                home.widgets.push(("explore".into(), explore.rect));
                if explore.clicked() {
                    choice = Some(Choice::Explore { investigation: chosen });
                }
            });
            ui.add_space(18.0);
            let width = ui.available_width();
            ui.horizontal_top(|ui| {
                // The narrated timelines and their chapters.
                ui.vertical(|ui| {
                    ui.set_width(width * 0.58);
                    // With several editions, a picker; the chapters and Watch follow it.
                    if rows.len() > 1 {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(tr("home.edition")).color(palette::text_dim()));
                            for (i, row) in rows.iter().enumerate() {
                                let r = ui.selectable_label(i == edition, RichText::new(&row.info.edition).font(FontId::new(24.0, ui::bold())));
                                home.widgets.push((format!("edition:{}", row.info.id), r.rect));
                                if r.clicked() {
                                    new_edition = Some(i);
                                }
                            }
                        });
                        ui.add_space(6.0);
                    }
                    if let Some(row) = rows.get(edition) {
                        match &row.summary {
                            Ok((title, length, chapters, narrated)) => {
                                ui.label(RichText::new(format!("{title} · {length}")).font(FontId::new(24.0, ui::bold())).color(palette::accent()));
                                if !narrated {
                                    ui.label(RichText::new(tr("home.no_narration")).color(palette::text_faint()));
                                }
                                ui.label(RichText::new(tr("home.start_from_a_chapter")).color(palette::text_dim()));
                                for (number, label, frame) in chapters {
                                    let r = ui.add(egui::Button::new(RichText::new(format!("{number:02}  {label}"))).frame(false).truncate());
                                    home.widgets.push((format!("chapter:{number}"), r.rect));
                                    if r.clicked() {
                                        choice = Some(Choice::Watch { investigation: chosen, timeline: row.info, start_frame: *frame });
                                    }
                                }
                            }
                            Err(e) => {
                                ui.label(RichText::new(&row.info.title).font(FontId::new(24.0, ui::bold())).color(palette::accent()));
                                ui.add(egui::Label::new(RichText::new(format!("{} {e}", tr("home.not_built_yet"))).color(palette::text_faint())).wrap());
                            }
                        }
                        ui.add_space(10.0);
                    }
                });
                // The demonstrations, with their keys in the viewer.
                ui.vertical(|ui| {
                    ui.label(RichText::new(tr("home.demonstrations")).font(FontId::new(24.0, ui::bold())).color(palette::accent()));
                    const KEYS: [&str; 12] = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "−", "="];
                    for (i, demo) in chosen.demos().iter().enumerate() {
                        ui.add(egui::Label::new(RichText::new(format!("{}  {}", KEYS.get(i).unwrap_or(&" "), chosen.demo_title(*demo))).color(palette::text_dim())).truncate());
                    }
                });
            });
        });
        let footer = tr("home.esc_hint");
        ink.text(pos2(40.0, canvas.y - 58.0), Align2::LEFT_TOP, footer, 24.0, FontFamily::Proportional, palette::text_faint());

        if let Some(e) = new_engine {
            self.engine = e;
            self.feature = 0;
            self.edition = None;
        }
        if let Some(f) = new_feature {
            self.feature = f;
            self.edition = None;
        }
        if let Some(e) = new_edition {
            self.edition = Some(e);
        }
        choice
    }

    /// The Config page: the settings, Save and Apply, Discard, and Restore
    /// Defaults; while settings are tried, a popup counts down over it.
    fn draw_config(&mut self, ctx: &egui::Context, ink: &ui::Ink, canvas: Vec2, time: f32) {
        if self.settings.trial.as_ref().is_some_and(|trial| time - trial.started >= TRIAL_SECONDS) {
            self.restore_settings();
        }
        let trying = self.settings.trial.is_some();
        ink.text(pos2(40.0, 20.0), Align2::LEFT_TOP, "RENDER LAB", 24.0, ui::bold(), palette::accent());
        ink.text(pos2(38.0, 52.0), Align2::LEFT_TOP, tr("home.config"), 38.0, ui::bold(), palette::text());
        // The card's top; it grows to fit what it holds (the path at its foot may wrap).
        let card = Rect::from_min_size(pos2(canvas.x / 2.0 - 620.0, canvas.y * 0.53 - 410.0), vec2(1240.0, 820.0));
        let inner = card.shrink2(vec2(48.0, 34.0));
        let flash = self.settings.flash.map(|start| 1.0 - (time - start) / FLASH_SECONDS).filter(|a| *a > 0.0);
        let area = egui::Area::new(egui::Id::unique("home-config")).fixed_pos(inner.min).fade_in(false).constrain(false).show(ctx, |ui| {
            ui.set_width(inner.width());
            // A checkbox as large as the text beside it.
            let spacing = ui.spacing_mut();
            spacing.icon_width = 30.0;
            spacing.icon_width_inner = 18.0;
            spacing.icon_spacing = 14.0;
            ui.add_enabled_ui(!trying, |ui| {
                let defaults = Config::default();
                let Home { settings, widgets, .. } = self;
                let heading = |ui: &mut egui::Ui, text: &str| {
                    ui.label(RichText::new(text).font(FontId::new(24.0, ui::bold())).color(palette::accent()));
                    ui.add_space(6.0);
                };
                let hint = |ui: &mut egui::Ui, text: &str| {
                    ui.add(egui::Label::new(RichText::new(text).color(palette::text_dim())).wrap());
                    ui.add_space(24.0);
                };

                // A tab per category, on the left; a tab whose settings were edited is marked.
                let tabs = 250.0;
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 36.0;
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 14.0;
                        for category in Category::ALL {
                            let edited = settings.pending.differs_in(&settings.saved, category);
                            let r = tab_button(ui, vec2(tabs, 64.0), category_name(category), settings.category == category, edited, !trying);
                            widgets.push((format!("category:{}", category.key()), r.rect));
                            if r.clicked() {
                                settings.category = category;
                            }
                        }
                    });
                    ui.vertical(|ui| {
                        ui.set_width(inner.width() - tabs - 36.0);
                        ui.set_min_height(420.0);
                        let pending = &mut settings.pending;
                        match settings.category {
                            Category::Interface => {
                                heading(ui, tr("home.language"));
                                let (row, restore) = option_row(ui, widgets, "language", pending.language != defaults.language, defaults.language.name(), !trying, |ui, widgets| {
                                    ui.spacing_mut().item_spacing.x = 12.0;
                                    for locale in Locale::ALL {
                                        let r = ui.add(egui::Button::selectable(pending.language == locale, RichText::new(locale.name()).font(FontId::new(30.0, ui::bold()))).min_size(vec2(180.0, 48.0)));
                                        widgets.push((format!("option:language:{}", locale.code()), r.rect));
                                        if r.clicked() {
                                            pending.language = locale;
                                        }
                                    }
                                });
                                if restore {
                                    pending.language = defaults.language;
                                }
                                if let Some(alpha) = flash {
                                    ui.painter().rect_stroke(row.expand2(vec2(28.0, 8.0)), CornerRadius::same(12), Stroke::new(3.0, palette::accent().gamma_multiply(alpha)), StrokeKind::Outside);
                                }
                                hint(ui, tr("home.language_hint"));

                                heading(ui, tr("home.theme"));
                                let themes = &settings.themes;
                                let name = |id: &str| themes.iter().find(|t| t.id == id).map_or(id.to_string(), |t| t.name.clone());
                                let (_, restore) = option_row(ui, widgets, "theme", pending.theme != defaults.theme, &name(&defaults.theme), !trying, |ui, widgets| {
                                    // A list, one to a row, so the default and Restore fit beside it.
                                    ui.vertical(|ui| {
                                        ui.spacing_mut().item_spacing.y = 8.0;
                                        for entry in themes {
                                            let r = ui.add(egui::Button::selectable(pending.theme == entry.id, RichText::new(&entry.name).font(FontId::new(26.0, ui::bold()))).min_size(vec2(280.0, 44.0)));
                                            widgets.push((format!("option:theme:{}", entry.id), r.rect));
                                            if r.clicked() {
                                                pending.theme = entry.id.clone();
                                            }
                                        }
                                    });
                                });
                                if restore {
                                    pending.theme = defaults.theme.clone();
                                }
                                ui.add(egui::Label::new(RichText::new(tr("home.theme_hint")).color(palette::text_dim())).wrap());
                                let folders = theme::dirs().iter().map(|d| d.display().to_string()).collect::<Vec<_>>().join("\n");
                                ui.add(egui::Label::new(RichText::new(folders).color(palette::text_faint())).wrap());
                                ui.add_space(24.0);
                                for error in &settings.theme_errors {
                                    ui.add(egui::Label::new(RichText::new(error).color(palette::text_faint())).wrap());
                                }
                            }
                            Category::Render => {
                                heading(ui, tr("home.display"));
                                let (_, restore) = option_row(ui, widgets, "fullscreen", pending.fullscreen != defaults.fullscreen, on_off(defaults.fullscreen), !trying, |ui, widgets| {
                                    let r = ui.checkbox(&mut pending.fullscreen, RichText::new(tr("home.full_screen")).font(FontId::new(30.0, ui::bold())));
                                    widgets.push(("option:fullscreen".into(), r.rect));
                                });
                                if restore {
                                    pending.fullscreen = defaults.fullscreen;
                                }
                                hint(ui, tr("home.full_screen_hint"));
                                hint(ui, tr("home.render_hint"));
                            }
                            Category::Audio => {
                                heading(ui, tr("home.volume"));
                                for (name, label, volume, default) in [("music", tr("home.music"), &mut pending.music, defaults.music), ("narration", tr("home.narration"), &mut pending.narration, defaults.narration)] {
                                    let (_, restore) = option_row(ui, widgets, name, *volume != default, &format!("{} %", default.percent), !trying, |ui, widgets| {
                                        let (rect, _) = ui.allocate_exact_size(vec2(240.0, 48.0), Sense::hover());
                                        ui.painter().text(rect.left_center(), Align2::LEFT_CENTER, label, FontId::new(30.0, ui::bold()), palette::text());
                                        let r = volume::slider(ui, volume, 40.0, 360.0);
                                        widgets.push((format!("option:{name}"), r.rail));
                                        widgets.push((format!("mute:{name}"), r.mute));
                                    });
                                    if restore {
                                        *volume = default;
                                    }
                                }
                                hint(ui, tr("home.audio_hint"));
                            }
                        }
                    });
                });

                ui.add_space(12.0);
                let changed = settings.pending != settings.saved;
                let [first, discard, reset] = ui
                    .horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 18.0;
                        let size = vec2((inner.width() - 36.0) / 3.0, 72.0);
                        // Save and Apply only when something changed; otherwise Return.
                        let first = if changed { ("save", tr("home.save_and_apply")) } else { ("return", tr("home.return")) };
                        [first, ("discard", tr("home.discard")), ("defaults", tr("home.restore_defaults"))].map(|(name, label)| {
                            let r = glass_button(ui, size, label, 30.0, !trying);
                            widgets.push((name.into(), r.rect));
                            r.clicked()
                        })
                    })
                    .inner;
                if first && changed {
                    self.try_settings(time);
                }
                if first && !changed {
                    self.page = Page::Title;
                    self.settings.message = None;
                    self.settings.flash = None;
                }
                if discard {
                    self.discard_settings();
                }
                if reset {
                    self.settings.pending = Config::default();
                    self.settings.message = Some(tr("home.defaults_shown").to_string());
                }
                ui.add_space(24.0);
                if self.settings.pending != self.settings.saved {
                    ui.label(RichText::new(tr("home.unsaved_changes")).color(palette::accent()));
                }
                if let Some(message) = &self.settings.message {
                    ui.add(egui::Label::new(RichText::new(message).color(palette::text_dim())).wrap());
                }
                let stored = match &self.settings.path {
                    Some(path) => format!("{} {}", tr("home.stored_in"), path.display()),
                    None => tr("home.no_config_dir").to_string(),
                };
                ui.add(egui::Label::new(RichText::new(stored).color(palette::text_faint())).wrap());
            });
        });
        // The background layer is drawn under every area, so the card can follow its content.
        glass_panel(ink.painter, Rect::from_min_max(card.min, pos2(card.max.x, area.response.rect.max.y + 34.0)), 24, true);
        let footer = if trying { tr("home.esc_restores") } else { tr("home.esc_discards") };
        ink.text(pos2(40.0, canvas.y - 58.0), Align2::LEFT_TOP, footer, 24.0, FontFamily::Proportional, palette::text_faint());

        // The countdown popup, above everything else on the page.
        let Some(started) = self.settings.trial.as_ref().map(|t| t.started) else { return };
        let left = (TRIAL_SECONDS - (time - started)).ceil().max(1.0) as u32;
        egui::Area::new(egui::Id::unique("home-config-popup")).order(egui::Order::Foreground).fixed_pos(Pos2::ZERO).fade_in(false).constrain(false).show(ctx, |ui| {
            let painter = ui.painter();
            painter.rect_filled(Rect::from_min_size(Pos2::ZERO, canvas), CornerRadius::ZERO, palette::theme().dim);
            let popup = Rect::from_center_size(pos2(canvas.x / 2.0, canvas.y * 0.5), vec2(820.0, 360.0));
            painter.rect_filled(popup, CornerRadius::same(22), palette::theme().popup);
            painter.rect_stroke(popup, CornerRadius::same(22), Stroke::new(2.0, palette::accent()), StrokeKind::Inside);
            painter.text(pos2(popup.center().x, popup.min.y + 44.0), Align2::CENTER_TOP, tr("home.keep_these_settings"), FontId::new(38.0, ui::bold()), palette::text());
            let countdown = tr_with("home.countdown", args!(seconds = left));
            painter.text(pos2(popup.center().x, popup.min.y + 110.0), Align2::CENTER_TOP, countdown, FontId::new(28.0, FontFamily::Proportional), palette::text_dim());
            let size = vec2(300.0, 72.0);
            let row = Rect::from_center_size(pos2(popup.center().x, popup.max.y - 80.0), vec2(size.x * 2.0 + 24.0, size.y));
            let mut keep = false;
            let mut restore = false;
            ui.scope_builder(egui::UiBuilder::new().max_rect(row).layout(egui::Layout::left_to_right(egui::Align::Center)), |ui| {
                ui.spacing_mut().item_spacing.x = 24.0;
                let apply = glass_button(ui, size, tr("home.apply"), 32.0, true);
                self.widgets.push(("trial:apply".into(), apply.rect));
                keep = apply.clicked();
                let back = glass_button(ui, size, tr("home.restore"), 32.0, true);
                self.widgets.push(("trial:restore".into(), back.rect));
                restore = back.clicked();
            });
            if keep {
                self.keep_settings();
            } else if restore {
                self.restore_settings();
            }
        });
    }

    /// A column: its heading, then `add` in an area clipped to the column.
    fn column_area(&mut self, ctx: &egui::Context, id: &str, rect: Rect, heading: &str, add: impl FnOnce(&mut Self, &mut egui::Ui)) {
        let inner = rect.shrink2(vec2(22.0, 18.0));
        // No fade-in (a snapshot shows the page as it is) and no shifting to fit the screen.
        egui::Area::new(egui::Id::unique(id)).fixed_pos(inner.min).fade_in(false).constrain(false).show(ctx, |ui| {
            ui.set_width(inner.width());
            ui.set_max_width(inner.width());
            ui.set_clip_rect(inner);
            ui.label(RichText::new(heading).font(FontId::new(24.0, ui::bold())).color(palette::accent()));
            ui.add_space(8.0);
            add(self, ui);
        });
    }
}

/// An option's row on the Config page: `add` draws its control, naming its
/// widgets. An option that differs from its default is marked, names the
/// default, and has a Restore; returns the row's rectangle and whether
/// Restore was clicked.
fn option_row(ui: &mut egui::Ui, widgets: &mut Vec<(String, Rect)>, name: &str, changed: bool, default: &str, enabled: bool, add: impl FnOnce(&mut egui::Ui, &mut Vec<(String, Rect)>)) -> (Rect, bool) {
    let mut restore = false;
    let row = ui.horizontal(|ui| {
        ui.set_min_height(48.0);
        add(ui, widgets);
        if changed {
            ui.add_space(12.0);
            ui.label(RichText::new(format!("{} {default}", tr("home.default"))).color(palette::accent()));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let r = glass_button(ui, vec2(190.0, 48.0), tr("home.restore"), 26.0, enabled);
                widgets.push((format!("restore:{name}"), r.rect));
                restore = r.clicked();
            });
        }
    });
    let r = row.response.rect;
    if changed {
        ui.painter().rect_filled(Rect::from_min_max(pos2(r.min.x - 20.0, r.min.y + 4.0), pos2(r.min.x - 14.0, r.max.y - 4.0)), CornerRadius::same(3), palette::accent());
    }
    (r, restore)
}

/// A category's name on its tab.
fn category_name(category: Category) -> &'static str {
    match category {
        Category::Interface => tr("home.interface"),
        Category::Render => tr("home.render"),
        Category::Audio => tr("home.audio"),
    }
}

/// A Config page tab: a glass button, outlined when it is the tab shown,
/// with a dot when its settings were edited.
fn tab_button(ui: &mut egui::Ui, size: Vec2, label: &str, selected: bool, edited: bool, enabled: bool) -> egui::Response {
    let r = glass_button(ui, size, label, 30.0, enabled);
    let painter = ui.painter();
    if selected {
        painter.rect_filled(r.rect, CornerRadius::same(16), palette::accent().gamma_multiply(0.18));
        painter.rect_stroke(r.rect, CornerRadius::same(16), Stroke::new(2.0, palette::accent()), StrokeKind::Inside);
    }
    if edited {
        painter.circle_filled(pos2(r.rect.max.x - 22.0, r.rect.center().y), 6.0, palette::accent());
    }
    r
}

/// Points per pixel and the canvas in points for a target, as in `ui::Layout`.
fn canvas_for(width: u32, height: u32, ui_scale: f32) -> (f32, Vec2) {
    let scale = (width as f32 / REFERENCE.x).min(height as f32 / REFERENCE.y) * ui_scale;
    (scale, vec2(width as f32 / scale, height as f32 / scale))
}

/// The scene behind the page: the title investigation's backdrop
/// (`investigations::title`), drawn full-screen and blurred by the frost. It
/// loads after the window opens (src/loading.rs), which shows the loading
/// page until then; if it fails, the page has a plain background.
struct Backdrop {
    gpu: Gpu,
    loading: Option<Loading>,
    renderer: Option<Renderer>,
    frost: Frost,
}

impl Backdrop {
    /// Starts loading the scene; `wait` loads it before returning (offscreen
    /// pages, so every frame shows it).
    fn new(gpu: Gpu, size: [u32; 2], output: wgpu::TextureFormat, wait: bool) -> Result<Backdrop> {
        let frost = Frost::new(&gpu.device, output)?;
        let title = investigations::title();
        let scene = investigation::scoped(title, || title.first_scene());
        let loading = Loading::start(title, gpu.clone(), scene, size);
        let (loading, renderer) = if wait { (None, Some(loading.wait()?)) } else { (Some(loading), None) };
        Ok(Backdrop { gpu, loading, renderer, frost })
    }

    fn gpu(&self) -> &Gpu {
        &self.gpu
    }

    /// Take the scene once it has loaded; while it loads, how far it has come.
    fn poll(&mut self) -> Option<Progress> {
        if let Some(result) = self.loading.as_mut().and_then(|l| l.poll()) {
            self.loading = None;
            match result {
                Ok(renderer) => self.renderer = Some(renderer),
                // The page stays on its plain background.
                Err(e) => eprintln!("the homepage's backdrop did not load: {e:#}"),
            }
        }
        self.loading.as_ref().map(Loading::progress)
    }

    /// Render the scene at `time` seconds into the frame target, at `size`,
    /// once it has loaded.
    fn render(&mut self, size: [u32; 2], time: f32) {
        let Some(renderer) = self.renderer.as_mut() else { return };
        let size = [size[0].max(64), size[1].max(64)];
        if [renderer.width, renderer.height] != size {
            renderer.resize(size[0], size[1]);
        }
        let title = investigations::title();
        investigation::scoped(title, || {
            let state = title.backdrop(time);
            renderer.render_backdrop(state.as_ref(), time);
        });
    }

    /// The device, the rendered frame and its size (once loaded), and the frost.
    fn parts(&mut self) -> (&Gpu, Option<(&wgpu::TextureView, [u32; 2])>, &mut Frost) {
        (&self.gpu, self.renderer.as_ref().map(|r| (&r.frame_view, [r.width, r.height])), &mut self.frost)
    }
}

impl Drop for Backdrop {
    /// A scene still loading is waited for: its worker uses the device.
    fn drop(&mut self) {
        if let Some(loading) = self.loading.take() {
            let _ = loading.wait();
        }
    }
}

/// egui on wgpu: draws the homepage into any colour target.
struct Painter {
    ctx: egui::Context,
    renderer: egui_wgpu::Renderer,
}

impl Painter {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Painter {
        let ctx = egui::Context::default();
        ui::install_fonts(&ctx);
        let renderer = egui_wgpu::Renderer::new(device, format, egui_wgpu::RendererOptions { msaa_samples: 1, depth_stencil_format: None, dithering: false, predictable_texture_filtering: true });
        Painter { ctx, renderer }
    }

    /// One frame of the page into `view` (`size` pixels): the backdrop at
    /// `time`, frosted, then the page; the loading page while the backdrop
    /// loads. Returns the choice made.
    #[allow(clippy::too_many_arguments)]
    fn paint(&mut self, backdrop: &mut Backdrop, view: &wgpu::TextureView, size: [u32; 2], ui_scale: f32, events: Vec<egui::Event>, time: f32, home: &mut Home, readback: Option<(&wgpu::Texture, &wgpu::Buffer)>) -> Option<Choice> {
        let (scale, canvas) = canvas_for(size[0], size[1], ui_scale);
        let mut raw = egui::RawInput { time: Some(time as f64), events, screen_rect: Some(Rect::from_min_size(Pos2::ZERO, canvas)), ..Default::default() };
        raw.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point = Some(scale);
        let mut choice = None;
        let loading = backdrop.poll();
        ui::restyle(&self.ctx);
        let mut output = self.ctx.run_pass(raw, |ctx| match &loading {
            Some(progress) => loading::page(ctx, canvas, "Render Lab", progress),
            None => choice = home.draw(ctx, canvas, time),
        });
        backdrop.render(size, time);
        let jobs = self.ctx.tessellate(output.shapes, output.pixels_per_point);
        let screen = egui_wgpu::ScreenDescriptor { size_in_pixels: size, pixels_per_point: output.pixels_per_point };
        let (gpu, frame, frost) = backdrop.parts();
        // Before the backdrop has loaded, the page is on the plain background.
        let loaded = frame.is_some();
        let bg = palette::background();
        let clear = if loaded { wgpu::Color::BLACK } else { wgpu::Color { r: bg.r() as f64 / 255.0, g: bg.g() as f64 / 255.0, b: bg.b() as f64 / 255.0, a: 1.0 } };
        let (device, queue) = (&gpu.device, &gpu.queue);
        for (id, deltas) in output.textures_delta.set.drain() {
            for delta in &deltas {
                self.renderer.update_texture(device, queue, id, delta);
            }
        }
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("home") });
        let mut commands = self.renderer.update_buffers(device, queue, &mut encoder, &jobs, &screen);
        if let Some((frame, frame_size)) = frame {
            frost.blur(device, &mut encoder, frame, frame_size);
        }
        {
            let mut pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("home"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment { view, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(clear), store: wgpu::StoreOp::Store } })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                })
                .forget_lifetime();
            if loaded {
                frost.draw(&mut pass);
            }
            self.renderer.render(&mut pass, &jobs, &screen);
        }
        if let Some((texture, buffer)) = readback {
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo { texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                wgpu::TexelCopyBufferInfo { buffer, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(padded_row_bytes(size[0])), rows_per_image: Some(size[1]) } },
                wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 },
            );
        }
        commands.push(encoder.finish());
        queue.submit(commands);
        for id in output.textures_delta.free.drain() {
            self.renderer.free_texture(&id);
        }
        choice
    }
}

/// The homepage on an offscreen target, for `--snapshot` and `--autotest`.
struct Offscreen {
    backdrop: Backdrop,
    painter: Painter,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    buffer: wgpu::Buffer,
    size: [u32; 2],
    ui_scale: f32,
    frames: u64,
    /// Controls clicked, and how many of those clicks (and of the hovers before
    /// them) would have played the click sound.
    clicks: usize,
    heard: usize,
    hovers_heard: usize,
}

impl Offscreen {
    /// `wait`: load the backdrop first, so every frame shows the page on it.
    fn new(options: &HomeOptions, wait: bool) -> Result<Offscreen> {
        let gpu = Gpu::new(gpu::instance(None), None)?;
        let size = [options.width.max(64), options.height.max(64)];
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("home"),
            size: wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FRAME_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let buffer = readback_buffer(&gpu.device, size[0], size[1]);
        let painter = Painter::new(&gpu.device, FRAME_FORMAT);
        let backdrop = Backdrop::new(gpu, size, FRAME_FORMAT, wait)?;
        Ok(Offscreen { backdrop, painter, texture, view, buffer, size, ui_scale: options.ui_scale, frames: 0, clicks: 0, heard: 0, hovers_heard: 0 })
    }

    /// The page's clock: deterministic, one sixtieth of a second per frame.
    fn time(&self) -> f32 {
        self.frames as f32 / 60.0
    }

    fn frame(&mut self, home: &mut Home, events: Vec<egui::Event>) -> Option<Choice> {
        self.frames += 1;
        let time = self.time();
        self.painter.paint(&mut self.backdrop, &self.view, self.size, self.ui_scale, events, time, home, None)
    }

    fn capture(&mut self, home: &mut Home) -> Vec<u8> {
        self.frames += 1;
        let time = self.time();
        self.painter.paint(&mut self.backdrop, &self.view, self.size, self.ui_scale, Vec::new(), time, home, Some((&self.texture, &self.buffer)));
        let gpu = self.backdrop.gpu();
        let padded = padded_row_bytes(self.size[0]) as u64;
        unpad_rows(&read_buffer(&gpu.device, &gpu.queue, &self.buffer, 0, padded * self.size[1] as u64), self.size[0], self.size[1])
    }

    /// Press and release the primary button on a control; None if the page
    /// has no such control, else what the click chose.
    fn click(&mut self, home: &mut Home, name: &str) -> Option<Option<Choice>> {
        let at = home.widgets.iter().find(|w| w.0 == name)?.1.center();
        let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        self.frame(home, vec![egui::Event::PointerMoved(at)]);
        self.hovers_heard += std::mem::take(&mut home.clicked) as usize;
        self.frame(home, vec![press(true)]);
        let choice = self.frame(home, vec![press(false)]);
        // One more pass, so the page shows (and lists the controls of) what was selected.
        self.frame(home, vec![egui::Event::PointerGone]);
        self.clicks += 1;
        self.heard += std::mem::take(&mut home.clicked) as usize;
        Some(choice)
    }
}

/// Render the homepage offscreen to a PNG; `trial` shows the Config page's
/// popup, with Full Screen being tried (the window stays as it is).
pub fn snapshot(options: &HomeOptions, page: Page, trial: bool, png: &Path) -> Result<()> {
    let mut home = Home::new(options.manifest_dir.clone(), options.config_path.clone());
    home.page = page;
    if trial {
        home.page = Page::Config;
        home.settings.category = Category::Render;
        home.settings.pending.fullscreen = true;
        home.try_settings(0.0);
    }
    let mut offscreen = Offscreen::new(options, true)?;
    // egui sizes the areas on the first pass.
    offscreen.frame(&mut home, Vec::new());
    let rgba = offscreen.capture(&mut home);
    crate::readback::write_png(png, offscreen.size[0], offscreen.size[1], &rgba)?;
    eprintln!("homepage: {} ({}x{}, smallest text {:.1} pt)", png.display(), offscreen.size[0], offscreen.size[1], home.min_text);
    Ok(())
}

/// Render the homepage's first frame offscreen to a PNG: the loading page,
/// while the title's scene loads.
pub fn snapshot_starting(options: &HomeOptions, png: &Path) -> Result<()> {
    let mut home = Home::new(options.manifest_dir.clone(), options.config_path.clone());
    let mut offscreen = Offscreen::new(options, false)?;
    let rgba = offscreen.capture(&mut home);
    let loading = offscreen.backdrop.loading.is_some();
    crate::readback::write_png(png, offscreen.size[0], offscreen.size[1], &rgba)?;
    eprintln!("homepage while it loads: {} ({}x{}){}", png.display(), offscreen.size[0], offscreen.size[1], if loading { "" } else { "; the scene had already loaded, so this is the title page" });
    Ok(())
}

/// Render the viewer offscreen to a PNG as the homepage opens it, with the
/// settings: the current investigation's demonstrations, or (`timeline`) its
/// first timeline playing, in the edition the homepage offers first; or the
/// demonstration `demo` (its key).
pub fn snapshot_viewer(options: &HomeOptions, timeline: bool, demo: Option<&str>, png: &Path) -> Result<()> {
    snapshot_viewer_page(options, timeline, demo, None, png)
}

/// `snapshot_viewer`, or with `paused`, its timeline paused on a 2D scene.
pub fn snapshot_viewer_page(options: &HomeOptions, timeline: bool, demo: Option<&str>, paused: Option<viewer::PausedPage>, png: &Path) -> Result<()> {
    let mut home = Home::new(options.manifest_dir.clone(), options.config_path.clone());
    let current = investigation::current();
    let choice = if timeline {
        let rows = home.timeline_rows(current);
        let row = rows.get(preferred_edition(&rows, home.applied.language)).with_context(|| format!("{} has no timeline", current.path()))?;
        Choice::Watch { investigation: current, timeline: row.info, start_frame: 0 }
    } else {
        Choice::Explore { investigation: current }
    };
    let size = PhysicalSize::new(options.width, options.height);
    let viewer_options = viewer_options(choice, &home, options, size).expect("a choice that opens the viewer");
    viewer::snapshot_page(viewer_options, demo, paused, png)
}

/// Click through the homepage offscreen and check what each control opens.
pub fn autotest(options: &HomeOptions, report: &Path) -> Result<()> {
    // A settings file of its own beside the report, so the user's is never touched.
    let config_path = report.with_file_name("home-autotest-config.toml");
    let _ = std::fs::remove_file(&config_path);
    let mut home = Home::new(options.manifest_dir.clone(), Some(config_path.clone()));
    let mut offscreen = Offscreen::new(options, true)?;
    offscreen.frame(&mut home, Vec::new());
    offscreen.frame(&mut home, Vec::new());
    let mut checks = Vec::new();
    let mut check = |name: &str, passed: bool, detail: String| {
        eprintln!("home autotest {name}: {} {detail}", if passed { "ok" } else { "FAILED" });
        checks.push(serde_json::json!({ "name": name, "passed": passed, "detail": detail }));
    };
    let describe = |c: Option<Choice>| c.map(|c| c.describe()).unwrap_or_else(|| "nothing".into());

    // The Config page: render settings are tried, then kept or put back; only Apply writes them.
    let stored = || Config::load(&config_path).ok().filter(|_| config_path.exists());
    let has = |home: &Home, name: &str| home.widgets.iter().any(|w| w.0 == name);
    offscreen.click(&mut home, "config");
    check("the title's Config opens the Config page at the Interface tab", home.page == Page::Config && home.settings.category == Category::Interface && has(&home, "option:language:zh-CN") && !has(&home, "option:fullscreen"), format!("{:?}, {:?}", home.page, home.settings.category));
    check("with nothing changed, the first button is Return, and no option is marked", has(&home, "return") && !has(&home, "save") && !has(&home, "restore:language"), String::new());
    offscreen.click(&mut home, "category:render");
    check("the Render tab shows Full Screen", home.settings.category == Category::Render && has(&home, "option:fullscreen"), format!("{:?}", home.settings.category));
    offscreen.click(&mut home, "option:fullscreen");
    check("a change turns Return into Save and Apply, and marks the option with its Restore", has(&home, "save") && !has(&home, "return") && has(&home, "restore:fullscreen"), String::new());
    check("ticking Full Screen changes nothing until it is applied", home.settings.pending.fullscreen && !home.applied.fullscreen && stored().is_none(), format!("pending {:?}, fullscreen {}", home.settings.pending, home.applied.fullscreen));
    offscreen.click(&mut home, "save");
    let trying = home.settings.trial.is_some() && home.applied.fullscreen;
    check("Save and Apply tries a render setting without writing it", trying && stored().is_none(), format!("trial {}, fullscreen {}, file {:?}", home.settings.trial.is_some(), home.applied.fullscreen, stored()));
    offscreen.click(&mut home, "trial:restore");
    check("the popup's Restore puts the previous settings back", home.settings.trial.is_none() && !home.applied.fullscreen && !home.settings.pending.fullscreen && stored().is_none(), format!("fullscreen {}, pending {:?}", home.applied.fullscreen, home.settings.pending));
    offscreen.click(&mut home, "option:fullscreen");
    offscreen.click(&mut home, "save");
    offscreen.frames += (TRIAL_SECONDS * 60.0) as u64 + 30;
    offscreen.frame(&mut home, Vec::new());
    check("after ten seconds without Apply, the previous settings come back", home.settings.trial.is_none() && !home.applied.fullscreen && stored().is_none(), format!("fullscreen {}, file {:?}", home.applied.fullscreen, stored()));
    offscreen.click(&mut home, "option:fullscreen");
    offscreen.click(&mut home, "save");
    offscreen.click(&mut home, "trial:apply");
    check("the popup's Apply keeps the settings and writes the file", home.applied.fullscreen && stored() == Some(Config { fullscreen: true, ..Config::default() }), format!("fullscreen {}, file {:?}", home.applied.fullscreen, stored()));
    check("right after saving, the first button is Return again", has(&home, "return") && !has(&home, "save"), String::new());
    offscreen.click(&mut home, "restore:fullscreen");
    check("an option's Restore shows its default without applying it", !home.settings.pending.fullscreen && home.applied.fullscreen && has(&home, "save") && !has(&home, "restore:fullscreen") && stored() == Some(Config { fullscreen: true, ..Config::default() }), format!("pending {:?}, fullscreen {}", home.settings.pending, home.applied.fullscreen));
    offscreen.click(&mut home, "option:fullscreen");
    offscreen.click(&mut home, "defaults");
    check("Restore Defaults shows the defaults without applying them", home.settings.pending == Config::default() && home.applied.fullscreen && stored() == Some(Config { fullscreen: true, ..Config::default() }), format!("pending {:?}, fullscreen {}", home.settings.pending, home.applied.fullscreen));
    offscreen.click(&mut home, "discard");
    check("Discard drops the edits and returns to the title", home.page == Page::Title && home.settings.pending == home.settings.saved, format!("{:?}, pending {:?}", home.page, home.settings.pending));
    offscreen.click(&mut home, "config");
    offscreen.click(&mut home, "return");
    check("Return goes back to the title", home.page == Page::Title, format!("{:?}", home.page));

    // The language: the title's globe opens the Config page at it; saved, it changes the text at once.
    offscreen.click(&mut home, "language");
    check("the title's globe opens the Config page with the Language option outlined", home.page == Page::Config && home.settings.flash.is_some() && has(&home, "option:language:zh-CN"), format!("{:?}", home.page));
    offscreen.click(&mut home, "option:language:zh-CN");
    check("picking a language changes nothing until it is saved", home.settings.pending.language == Locale::ZhCn && i18n::current() == Locale::En && has(&home, "restore:language"), format!("pending {:?}, text {:?}", home.settings.pending.language, i18n::current()));
    offscreen.click(&mut home, "save");
    check("a saved language applies and is written at once, without the popup", home.settings.trial.is_none() && i18n::current() == Locale::ZhCn && tr("home.config") != "Config" && stored().is_some_and(|c| c.language == Locale::ZhCn && c.fullscreen), format!("text {:?}: {}, file {:?}", i18n::current(), tr("home.config"), stored()));
    // The edition offered first is in the language, else English.
    for i in investigations::all().iter().copied().filter(|i| !i.timelines().is_empty()) {
        let rows = home.timeline_rows(i);
        let offered = &rows[preferred_edition(&rows, Locale::ZhCn)];
        let wanted = rows.iter().find(|r| r.locale == Some(Locale::ZhCn)).or_else(|| rows.iter().find(|r| r.locale == Some(Locale::En))).unwrap_or(&rows[0]);
        check(&format!("{}: in Chinese, the edition offered first is the Chinese one, else the English one", i.path()), offered.info.id == wanted.info.id, format!("{} ({:?})", offered.info.id, offered.locale));
    }

    // The theme: offered from the themes beside the binary, the default first in use.
    let ids: Vec<String> = home.settings.themes.iter().map(|t| t.id.clone()).collect();
    let offered = home.settings.themes.clone();
    let colors = |id: &str| offered.iter().find(|t| t.id == id).map(|t| t.theme);
    check("the Interface tab offers the built-in theme and the default one, which is in use", has(&home, &format!("option:theme:{}", theme::BUILT_IN)) && has(&home, &format!("option:theme:{}", theme::DEFAULT)) && colors(theme::DEFAULT) == Some(palette::theme()), format!("{ids:?}"));
    let other = ids.iter().find(|id| *id != theme::DEFAULT && *id != theme::BUILT_IN).cloned().unwrap_or_else(|| theme::BUILT_IN.into());
    // A theme and a render setting at once: the theme applies and is written; the render setting is tried.
    offscreen.click(&mut home, &format!("option:theme:{other}"));
    offscreen.click(&mut home, "category:render");
    offscreen.click(&mut home, "option:fullscreen");
    offscreen.click(&mut home, "save");
    check("a saved theme applies and is written at once, while a render change beside it is tried", home.settings.trial.is_some() && colors(&other) == Some(palette::theme()) && stored().is_some_and(|c| c.theme == other && c.fullscreen), format!("trial {}, file {:?}", home.settings.trial.is_some(), stored()));
    offscreen.click(&mut home, "trial:restore");
    check("Restore puts back the render setting and keeps the theme", home.applied.fullscreen && home.applied.theme == other && colors(&other) == Some(palette::theme()) && home.settings.pending == home.settings.saved, format!("applied {:?}", home.applied));

    // The volumes: a slider and its mute button, applied and written at once.
    offscreen.click(&mut home, "category:audio");
    offscreen.click(&mut home, "mute:music");
    check("the music's mute button mutes it, and keeps its level", home.settings.pending.music.muted && home.settings.pending.music.percent == Config::default().music.percent && has(&home, "restore:music"), format!("{:?}", home.settings.pending.music));
    let rail = home.widgets.iter().find(|w| w.0 == "option:narration").map(|w| w.1);
    if let Some(rail) = rail {
        // A click a quarter along the rail.
        let at = pos2(rail.min.x + rail.width() * 0.25, rail.center().y);
        let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        offscreen.frame(&mut home, vec![egui::Event::PointerMoved(at)]);
        offscreen.frame(&mut home, vec![press(true)]);
        offscreen.frame(&mut home, vec![press(false)]);
        offscreen.frame(&mut home, vec![egui::Event::PointerGone]);
        home.clicked = false;
    }
    let narration = home.settings.pending.narration;
    check("a click on the narration's slider sets its level", rail.is_some() && narration.percent < 60 && !narration.muted, format!("{narration:?}"));
    offscreen.click(&mut home, "save");
    check("saved volumes are in use and written at once, without the popup", home.settings.trial.is_none() && home.applied.music.muted && home.applied.narration == narration && stored().is_some_and(|c| c.music.muted && c.narration == narration), format!("music {:?}, file {:?}", home.applied.music, stored()));
    home.keep_narration(Volume::new(80));
    check("the narration's volume as the viewer leaves it is kept and written", home.applied.narration == Volume::new(80) && stored().is_some_and(|c| c.narration == Volume::new(80) && c.language == Locale::ZhCn), format!("file {:?}", stored()));
    offscreen.click(&mut home, "config");
    offscreen.click(&mut home, "defaults");
    offscreen.click(&mut home, "save");
    offscreen.click(&mut home, "trial:apply");
    check("Restore Defaults, applied, puts English, the default theme, and the default volumes back", home.applied == Config::default() && i18n::current() == Locale::En && colors(theme::DEFAULT) == Some(palette::theme()) && stored() == Some(Config::default()), format!("file {:?}", stored()));
    offscreen.click(&mut home, "return");
    let _ = std::fs::remove_file(&config_path);

    // The title page.
    let quit = offscreen.click(&mut home, "quit").flatten();
    check("the title's Quit quits", matches!(quit, Some(Choice::Quit)), describe(quit));
    let play = offscreen.click(&mut home, "play");
    check("the title's Play opens the selection page", play.is_some_and(|c| c.is_none()) && home.page == Page::Select, format!("{:?}", home.page));

    // The selection page.
    let engines: Vec<&str> = investigations::engines().iter().map(|e| e.id).collect();
    let listed = engines.iter().all(|e| home.widgets.iter().any(|w| w.0 == format!("engine:{e}")));
    check("every engine is listed", listed && !engines.is_empty(), engines.join(", "));
    for i in investigations::all() {
        let engine = format!("engine:{}", i.engine().id);
        let feature = format!("feature:{}", i.path());
        let selected = offscreen.click(&mut home, &engine).is_some() && offscreen.click(&mut home, &feature).is_some() && home.selected().path() == i.path();
        check(&format!("{} can be selected", i.path()), selected, feature.clone());
        let explore = offscreen.click(&mut home, "explore").flatten();
        let ok = matches!(explore, Some(Choice::Explore { investigation }) if investigation.path() == i.path());
        check(&format!("{}: Explore opens the demonstrations", i.path()), ok, describe(explore));
        let Some(timeline) = i.timelines().first() else { continue };
        let watch = offscreen.click(&mut home, "watch").flatten();
        let ok = matches!(watch, Some(Choice::Watch { investigation, timeline: t, start_frame: 0 }) if investigation.path() == i.path() && t.id == timeline.id);
        check(&format!("{}: Watch opens the timeline from its start", i.path()), ok, describe(watch));
        match TimelineSummary::load(&options.manifest_dir, timeline) {
            Ok(summary) => {
                if let Some((number, _, frame)) = summary.chapters.last().cloned() {
                    let chosen = offscreen.click(&mut home, &format!("chapter:{number}")).flatten();
                    let ok = matches!(chosen, Some(Choice::Watch { start_frame, .. }) if start_frame == frame);
                    check(&format!("{}: a chapter opens the timeline at its start", i.path()), ok, format!("chapter {number} (frame {frame}) → {}", describe(chosen)));
                }
            }
            Err(e) => check(&format!("{}: its timeline loads", i.path()), false, format!("{e:#}")),
        }
        // Every other edition: picking it makes Watch play that timeline.
        for other in i.timelines().iter().skip(1) {
            let picked = offscreen.click(&mut home, &format!("edition:{}", other.id)).is_some();
            let watch = offscreen.click(&mut home, "watch").flatten();
            let ok = picked && matches!(watch, Some(Choice::Watch { timeline: t, start_frame: 0, .. }) if t.id == other.id);
            check(&format!("{}: the {} edition can be picked and watched", i.path(), other.id), ok, describe(watch));
        }
        if let Some(first) = i.timelines().first() {
            offscreen.click(&mut home, &format!("edition:{}", first.id));
        }
    }
    let back = offscreen.click(&mut home, "back");
    check("Back returns to the title", back.is_some() && home.page == Page::Title, format!("{:?}", home.page));
    check("text is at least 24 pt at 1080p", home.min_text >= ui::MIN_TEXT_PT, format!("{:.1} pt", home.min_text));
    check("every click on a control plays the click sound, and hovering plays none", offscreen.heard == offscreen.clicks && offscreen.hovers_heard == 0,
          format!("{} of {} clicks, {} hovers", offscreen.heard, offscreen.clicks, offscreen.hovers_heard));
    let passed = checks.iter().all(|c| c["passed"] == true);
    std::fs::write(report, serde_json::to_string_pretty(&serde_json::json!({ "passed": passed, "checks": checks }))?)?;
    if !passed {
        anyhow::bail!("homepage autotest failed; see {}", report.display());
    }
    eprintln!("homepage autotest passed; report: {}", report.display());
    Ok(())
}

struct Surface {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    backdrop: Backdrop,
    painter: Painter,
}

/// The homepage's window, for one run of the event loop.
struct HomeApp<'a> {
    home: &'a mut Home,
    options: &'a HomeOptions,
    size: PhysicalSize<u32>,
    surface: Option<Surface>,
    events: Vec<egui::Event>,
    cursor: Pos2,
    started: Instant,
    /// Whether the window is fullscreen now; it follows `Home::fullscreen`.
    shown_fullscreen: bool,
    /// F11: save the next presented frame (`readback::screenshot`).
    screenshot: bool,
    choice: Option<Choice>,
    error: Option<anyhow::Error>,
    /// The sound of a control being clicked.
    click: &'a mut Option<AudioPlayer>,
    /// The music, and the volume it plays at now; it follows `Home::applied`.
    music: &'a mut Option<AudioPlayer>,
    heard_music: Volume,
}

impl HomeApp<'_> {
    fn open(&mut self, event_loop: &ActiveEventLoop) {
        if self.surface.is_some() || self.error.is_some() {
            return;
        }
        let attributes = Window::default_attributes().with_title("Render Lab").with_inner_size(self.size).with_fullscreen(fullscreen_mode(self.home.applied.fullscreen));
        let result = (|| -> Result<Surface> {
            let window = Arc::new(event_loop.create_window(attributes)?);
            let instance = gpu::instance(Some(Box::new(event_loop.owned_display_handle())));
            let surface = instance.create_surface(window.clone())?;
            let gpu = Gpu::new(instance, Some(&surface))?;
            let size = window.inner_size();
            let caps = surface.get_capabilities(&gpu.adapter);
            // A UNORM swapchain, as the viewer's: the page holds sRGB-encoded bytes.
            let format = caps.formats.iter().copied().find(|f| !f.is_srgb() && matches!(f, wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Rgba8Unorm)).unwrap_or(caps.formats[0]);
            let mut config = surface.get_default_config(&gpu.adapter, size.width.max(1), size.height.max(1)).context("surface unsupported")?;
            config.format = format;
            config.present_mode = wgpu::PresentMode::Fifo;
            // So F11 can copy the frame it presents.
            if caps.usages.contains(wgpu::TextureUsages::COPY_SRC) {
                config.usage |= wgpu::TextureUsages::COPY_SRC;
            }
            surface.configure(&gpu.device, &config);
            let painter = Painter::new(&gpu.device, format);
            let backdrop = Backdrop::new(gpu, [config.width, config.height], format, false)?;
            Ok(Surface { window, surface, config, backdrop, painter })
        })();
        match result {
            Ok(s) => {
                self.started = Instant::now();
                self.shown_fullscreen = self.home.applied.fullscreen;
                // The new window draws with a new egui context: textures of the last one are gone.
                self.home.globe = None;
                self.surface = Some(s);
            }
            Err(e) => {
                self.error = Some(e);
                event_loop.exit();
            }
        }
    }

    fn scale(&self) -> f32 {
        let size = self.surface.as_ref().map_or(self.size, |s| s.window.inner_size());
        canvas_for(size.width.max(1), size.height.max(1), self.options.ui_scale).0
    }

    fn redraw(&mut self, event_loop: &ActiveEventLoop) {
        let Some(s) = self.surface.as_mut() else { return };
        let frame = match s.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                s.surface.configure(&s.backdrop.gpu().device, &s.config);
                return;
            }
            _ => return,
        };
        let view = frame.texture.create_view(&Default::default());
        let events = std::mem::take(&mut self.events);
        let time = self.started.elapsed().as_secs_f32();
        // The frame a screenshot is taken of shows no popup.
        self.home.popups.hidden = self.screenshot;
        let choice = s.painter.paint(&mut s.backdrop, &view, [s.config.width, s.config.height], self.options.ui_scale, events, time, self.home, None);
        if std::mem::take(&mut self.home.clicked) {
            if let Some(click) = self.click.as_mut() {
                click.play_from(0.0);
            }
        }
        if std::mem::take(&mut self.screenshot) {
            let gpu = s.backdrop.gpu();
            let saved = crate::readback::screenshot(&gpu.device, &gpu.queue, &frame.texture, &self.options.manifest_dir);
            self.home.popups.push(Popup::screenshot(&saved));
        }
        for action in self.home.popups.take_actions() {
            popup::perform(action);
        }
        s.window.pre_present_notify();
        s.backdrop.gpu().queue.present(frame);
        // Settings being tried or put back.
        if self.shown_fullscreen != self.home.applied.fullscreen {
            self.shown_fullscreen = self.home.applied.fullscreen;
            s.window.set_fullscreen(fullscreen_mode(self.shown_fullscreen));
        }
        if self.heard_music != self.home.applied.music {
            self.heard_music = self.home.applied.music;
            // FFplay takes a volume when it starts: the loop starts again at the new one.
            if let Some(m) = self.music.as_mut() {
                m.set_gain(self.heard_music.gain());
                m.play_from(0.0);
            }
        }
        if let Some(c) = choice {
            self.choice = Some(c);
            // The windowed size, for the next window; a fullscreen one fills the screen anyway.
            if !self.shown_fullscreen {
                self.size = s.window.inner_size();
            }
            event_loop.exit();
        }
    }
}

impl ApplicationHandler for HomeApp<'_> {
    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        // A run on a reused event loop starts here; `resumed` may not follow.
        if cause == StartCause::Init {
            self.open(event_loop);
        }
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.open(event_loop);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let scale = self.scale();
        match event {
            WindowEvent::CloseRequested => {
                self.choice = Some(Choice::Quit);
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                if let Some(s) = self.surface.as_mut() {
                    s.config.width = size.width.max(1);
                    s.config.height = size.height.max(1);
                    s.surface.configure(&s.backdrop.gpu().device, &s.config);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = pos2(position.x as f32 / scale, position.y as f32 / scale);
                self.events.push(egui::Event::PointerMoved(self.cursor));
            }
            WindowEvent::CursorLeft { .. } => self.events.push(egui::Event::PointerGone),
            WindowEvent::MouseInput { state, button, .. } => {
                let button = match button {
                    MouseButton::Left => Some(egui::PointerButton::Primary),
                    MouseButton::Right => Some(egui::PointerButton::Secondary),
                    MouseButton::Middle => Some(egui::PointerButton::Middle),
                    _ => None,
                };
                if let Some(button) = button {
                    self.events.push(egui::Event::PointerButton { pos: self.cursor, button, pressed: state == ElementState::Pressed, modifiers: Default::default() });
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (unit, delta) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (egui::MouseWheelUnit::Line, vec2(x, y)),
                    MouseScrollDelta::PixelDelta(PhysicalPosition { x, y }) => (egui::MouseWheelUnit::Point, vec2(x as f32, y as f32) / scale),
                };
                self.events.push(egui::Event::MouseWheel { unit, delta, phase: egui::TouchPhase::Move, source: egui::MouseWheelSource::Unknown, modifiers: Default::default() });
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed && !event.repeat => match event.physical_key {
                // Esc: from the selection page to the title; on the title, quit.
                PhysicalKey::Code(KeyCode::Escape) if !self.home.back() => {
                    self.choice = Some(Choice::Quit);
                    event_loop.exit();
                }
                // Saved when the next frame is presented.
                PhysicalKey::Code(KeyCode::F11) => self.screenshot = true,
                _ => {}
            },
            WindowEvent::RedrawRequested => self.redraw(event_loop),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(s) = &self.surface {
            s.window.request_redraw();
        }
    }
}

/// The homepage's music and the sound of its controls (packages/rlstd/assets/audio/README.md); the
/// lab is silent without them or without FFplay.
const MUSIC: &str = "rlstd.audio/music";
const CLICK: &str = "rlstd.audio/click";
/// The title page's language button.
const GLOBE: &str = "rlstd.icon/globe";

/// The homepage, then whatever it opens, until it quits or a window is closed.
pub fn run(options: HomeOptions) -> Result<()> {
    let mut event_loop = EventLoop::new()?;
    let mut home = Home::new(options.manifest_dir.clone(), options.config_path.clone());
    let mut size = PhysicalSize::new(options.width, options.height);
    // The main theme's music plays while the homepage is shown, not in a demonstration or a video.
    let mut music = AudioPlayer::new(resources::get(MUSIC).path(), &options.manifest_dir).map(|m| m.looping(2.0));
    // So are its clicks. The player outlives each page, so the click that opens the viewer is heard.
    let mut click = AudioPlayer::new(resources::get(CLICK).path(), &options.manifest_dir);
    loop {
        let heard_music = home.applied.music;
        if let Some(m) = music.as_mut() {
            m.set_gain(heard_music.gain());
            m.play_from(0.0);
        }
        event_loop.set_control_flow(ControlFlow::Poll);
        let mut app = HomeApp { home: &mut home, options: &options, size, surface: None, events: Vec::new(), cursor: Pos2::ZERO, started: Instant::now(), shown_fullscreen: false, screenshot: false, choice: None, error: None, click: &mut click, music: &mut music, heard_music };
        event_loop.run_app_on_demand(&mut app)?;
        if let Some(e) = app.error.take() {
            return Err(e);
        }
        let choice = app.choice.unwrap_or(Choice::Quit);
        size = app.size;
        drop(app);
        if let Some(m) = music.as_mut() {
            m.stop();
        }
        if !open(choice, &mut event_loop, &mut home, &options, size) {
            return Ok(());
        }
    }
}

/// The viewer's options for what the homepage chose, with the settings in
/// use (the investigation becomes the current one); None for Quit. A timeline
/// is in its own language; the demonstrations are in the homepage's.
fn viewer_options(choice: Choice, home: &Home, options: &HomeOptions, size: PhysicalSize<u32>) -> Option<ViewerOptions> {
    let (investigation, timeline, audio, start_frame, locale) = match choice {
        Choice::Quit => return None,
        Choice::Watch { investigation, timeline, start_frame } => (investigation, Some(options.manifest_dir.join(&timeline.path)), Some(options.manifest_dir.join(&timeline.narration)), start_frame, None),
        Choice::Explore { investigation } => (investigation, None, None, 0, Some(home.applied.language)),
    };
    investigation::set(investigation);
    Some(ViewerOptions {
        timeline,
        audio,
        manifest_dir: options.manifest_dir.clone(),
        ui_scale: options.ui_scale,
        autotest: None,
        width: size.width,
        height: size.height,
        locale,
        start_frame,
        fullscreen: home.applied.fullscreen,
        narration: home.applied.narration,
    })
}

/// Run the viewer on what the homepage chose; false to quit.
fn open(choice: Choice, event_loop: &mut EventLoop<()>, home: &mut Home, options: &HomeOptions, size: PhysicalSize<u32>) -> bool {
    let Some(viewer_options) = viewer_options(choice, home, options, size) else { return false };
    let exit = viewer::run_on(event_loop, viewer_options);
    // A timeline in another language switched the text; the homepage keeps its own.
    i18n::set(home.applied.language);
    let exit = exit.map(|(exit, narration)| {
        home.keep_narration(narration);
        exit
    });
    match exit {
        Ok(ViewerExit::Back) => {}
        Ok(ViewerExit::Closed) => return false,
        // Show what went wrong on the selection page instead of quitting.
        Err(e) => home.error = Some(format!("{e:#}")),
    }
    true
}
