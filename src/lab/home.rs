//! The homepage: the window `render-lab` opens without a command, styled like
//! a game's title screen. Behind it, the current investigation's scene renders
//! full-screen under frosted glass (src/lab/frost.rs).
//!
//! - The title page: "Render Lab", Play, Config, and Quit, and the copyright,
//!   license, and version in the bottom-left corner.
//! - The Config page edits the settings file (src/lab/config.rs). Save and
//!   Apply tries the edited settings for ten seconds behind a popup: Apply
//!   keeps them and writes the file; Restore, or letting the time run out,
//!   puts the previous ones back. Nothing is written before Apply.
//! - Play opens the selection page: the engines the lab investigates, the
//!   chosen engine's features, and the chosen feature's narrated timeline
//!   (from the start or from a chapter) and demonstrations. Esc or Back
//!   returns to the title.
//!
//! F11 toggles borderless fullscreen here and in the viewer, and it carries
//! over between them; it is for the session only and never saved. Opening a timeline or the demonstrations runs the viewer on the same event
//! loop; Esc in the viewer comes back to the selection page, and closing a
//! window quits. The page is drawn by `Home::draw`, which the window and the
//! offscreen `--snapshot` and `--autotest` modes share, so the page can be
//! checked without opening a window.

use crate::investigations;
use crate::lab::config::Config;
use crate::lab::frame::Statics;
use crate::lab::frost::Frost;
use crate::lab::gpu::{self, Gpu, FRAME_FORMAT};
use crate::lab::i18n::{args, tr, tr_with};
use crate::lab::investigation::{self, Investigation, TimelineInfo};
use crate::lab::palette;
use crate::lab::readback::{padded_row_bytes, read_buffer, readback_buffer, unpad_rows};
use crate::lab::renderer::Renderer;
use crate::lab::text::{self as ui, REFERENCE};
use crate::lab::audio::AudioPlayer;
use crate::lab::viewer::{self, ViewerExit, ViewerOptions};
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

/// How long tried settings stay before they are put back unless kept.
const TRIAL_SECONDS: f32 = 10.0;

/// Settings being tried: what to put back, and since when (page seconds).
#[derive(Clone, Copy, Debug)]
struct Trial {
    started: f32,
    /// The session's fullscreen before the settings were applied.
    fullscreen_before: bool,
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
}

impl TimelineSummary {
    fn load(manifest_dir: &Path, info: &TimelineInfo) -> Result<TimelineSummary> {
        let path = manifest_dir.join(info.path);
        let text = std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        let json: serde_json::Value = serde_json::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
        let chapters = json["chapters"]
            .as_array()
            .map(|cs| cs.iter().map(|c| (c["number"].as_u64().unwrap_or(0) as u32, c["title"].as_str().unwrap_or("").to_string(), c["startFrame"].as_u64().unwrap_or(0))).collect())
            .unwrap_or_default();
        Ok(TimelineSummary {
            title: json["title"].as_str().unwrap_or(info.title).to_string(),
            fps: json["fps"].as_u64().unwrap_or(30).max(1) as u32,
            total_frames: json["totalFrames"].as_u64().unwrap_or(0),
            chapters,
            narrated: manifest_dir.join(info.narration).is_file(),
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
    let fill = if smoked { Color32::from_rgba_unmultiplied(6, 14, 24, 120) } else { glass(14) };
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
    painter.rect_stroke(rect, CornerRadius::same(16), Stroke::new(if hot { 2.0 } else { 1.5 }, if hot { palette::ACCENT } else { glass(70) }), StrokeKind::Inside);
    let color = if enabled { palette::TEXT } else { palette::TEXT_FAINT };
    painter.text(rect.center(), Align2::CENTER_CENTER, label, FontId::new(font, ui::bold()), color);
    response
}

/// The homepage's state: the page, the selection, and what the chosen
/// feature's timelines hold.
pub struct Home {
    manifest_dir: PathBuf,
    pub page: Page,
    /// Fullscreen in this session: from the settings, then F11 and tried settings.
    pub fullscreen: bool,
    settings: Settings,
    engine: usize,
    feature: usize,
    /// Per investigation (by `investigations::all()` index), per timeline.
    timelines: Vec<Option<Vec<Result<TimelineSummary, String>>>>,
    /// The timeline (edition) the selection page offers, by index.
    edition: usize,
    /// An error from the last viewer run, shown until the next choice.
    pub error: Option<String>,
    /// Canvas rectangle of every control, by name (for the autotest).
    pub widgets: Vec<(String, Rect)>,
    /// The smallest text size drawn (points at the 1080p reference).
    pub min_text: f32,
    /// Whether a control was clicked since the window last played a click.
    pub clicked: bool,
}

impl Home {
    pub fn new(manifest_dir: PathBuf, config_path: Option<PathBuf>) -> Home {
        // A file that cannot be read leaves the defaults, and the Config page says why.
        let (saved, message) = match config_path.as_deref().map(Config::load).transpose() {
            Ok(config) => (config.unwrap_or_default(), None),
            Err(e) => (Config::default(), Some(format!("{e:#}"))),
        };
        let settings = Settings { path: config_path, saved, pending: saved, trial: None, message };
        let mut home = Home {
            manifest_dir,
            page: Page::Title,
            fullscreen: saved.fullscreen,
            settings,
            engine: 0,
            feature: 0,
            timelines: investigations::all().iter().map(|_| None).collect(),
            edition: 0,
            error: None,
            widgets: Vec::new(),
            min_text: f32::INFINITY,
            clicked: false,
        };
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

    /// F11: fullscreen for this session only; the settings are untouched.
    pub fn toggle_fullscreen(&mut self) {
        self.fullscreen = !self.fullscreen;
    }

    /// Save and Apply: try the edited settings until they are kept or put back.
    fn try_settings(&mut self, time: f32) {
        self.settings.trial = Some(Trial { started: time, fullscreen_before: self.fullscreen });
        self.fullscreen = self.settings.pending.fullscreen;
        self.settings.message = None;
    }

    /// The popup's Apply: keep the tried settings and write them to the file.
    fn keep_settings(&mut self) {
        if self.settings.trial.take().is_none() {
            return;
        }
        let pending = self.settings.pending;
        self.settings.message = Some(match &self.settings.path {
            Some(path) => match pending.save(path) {
                Ok(()) => {
                    self.settings.saved = pending;
                    format!("{} {}", tr("home.saved_to"), path.display())
                }
                Err(e) => format!("{} {e:#}", tr("home.applied_but_not_saved")),
            },
            None => {
                self.settings.saved = pending;
                tr("home.applied_for_session").to_string()
            }
        });
    }

    /// The popup's Restore, or the time running out: put the previous settings back.
    fn restore_settings(&mut self) {
        if let Some(trial) = self.settings.trial.take() {
            self.fullscreen = trial.fullscreen_before;
            self.settings.pending = self.settings.saved;
            self.settings.message = Some(tr("home.restored_the_previous_settings").to_string());
        }
    }

    /// Discard: forget the edits and leave the page.
    fn discard_settings(&mut self) {
        self.settings.pending = self.settings.saved;
        self.settings.message = None;
        self.page = Page::Title;
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
            Page::Title => self.draw_title(ctx, &ink, canvas),
            Page::Select => self.draw_select(ctx, &ink, canvas),
            Page::Config => {
                self.draw_config(ctx, &ink, canvas, time);
                None
            }
        };
        self.min_text = self.min_text.min(ink.min_size.get());
        if choice.is_some() {
            self.error = None;
        }
        choice
    }

    /// The title page: the name, Play and Quit on a glass card, and the credits.
    fn draw_title(&mut self, ctx: &egui::Context, ink: &ui::Ink, canvas: Vec2) -> Option<Choice> {
        let center = canvas.x / 2.0;
        let card = Rect::from_center_size(pos2(center, canvas.y * 0.5), vec2(900.0, 710.0));
        glass_panel(ink.painter, card, 28, false);
        ink.text(pos2(center, card.min.y + 64.0), Align2::CENTER_TOP, "Render Lab", 128.0, ui::bold(), palette::TEXT);
        let rule = card.min.y + 230.0;
        ink.painter.line_segment([pos2(center - 150.0, rule), pos2(center + 150.0, rule)], Stroke::new(2.0, palette::ACCENT));
        ink.text(pos2(center, rule + 22.0), Align2::CENTER_TOP, tr("home.subtitle"), 28.0, FontFamily::Proportional, palette::TEXT_DIM);
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
                self.page = Page::Config;
            }
            let quit = glass_button(ui, button, tr("home.quit"), 40.0, true);
            self.widgets.push(("quit".into(), quit.rect));
            if quit.clicked() {
                choice = Some(Choice::Quit);
            }
        });
        ink.text(pos2(40.0, canvas.y - 52.0), Align2::LEFT_TOP, credits(), 24.0, FontFamily::Proportional, palette::TEXT_FAINT);
        choice
    }

    /// The selection page: engines, the chosen engine's features, and the
    /// chosen feature's timelines and demonstrations, on glass panels.
    fn draw_select(&mut self, ctx: &egui::Context, ink: &ui::Ink, canvas: Vec2) -> Option<Choice> {
        let mut choice = None;
        let chosen = self.selected();
        // The chrome's strings and the backdrop follow the chosen feature.
        investigation::set(chosen);
        let painter = ink.painter;

        ink.text(pos2(40.0, 20.0), Align2::LEFT_TOP, "RENDER LAB", 24.0, ui::bold(), palette::ACCENT);
        ink.text(pos2(38.0, 52.0), Align2::LEFT_TOP, tr("home.choose"), 38.0, ui::bold(), palette::TEXT);
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
                    ui.add(egui::Label::new(RichText::new(engine.summary).color(palette::TEXT_DIM)).wrap());
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
                ui.label(RichText::new(format!("{} {} · {commit}", engine.name, engine.version)).color(palette::TEXT_FAINT));
                ui.add_space(12.0);
            }
        });

        // The chosen feature: what it covers, how to open it, its chapters and demonstrations.
        let rows = self.timeline_rows(chosen);
        let edition = self.edition.min(rows.len().saturating_sub(1));
        let mut new_edition = None;
        let error = self.error.clone();
        self.column_area(ctx, "home-detail", detail_rect, chosen.title(), |home, ui| {
            ui.add(egui::Label::new(RichText::new(chosen.summary()).color(palette::TEXT)).wrap());
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
                            ui.label(RichText::new(tr("home.edition")).color(palette::TEXT_DIM));
                            for (i, row) in rows.iter().enumerate() {
                                let r = ui.selectable_label(i == edition, RichText::new(row.info.edition).font(FontId::new(24.0, ui::bold())));
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
                                ui.label(RichText::new(format!("{title} · {length}")).font(FontId::new(24.0, ui::bold())).color(palette::ACCENT));
                                if !narrated {
                                    ui.label(RichText::new(tr("home.no_narration")).color(palette::TEXT_FAINT));
                                }
                                ui.label(RichText::new(tr("home.start_from_a_chapter")).color(palette::TEXT_DIM));
                                for (number, label, frame) in chapters {
                                    let r = ui.add(egui::Button::new(RichText::new(format!("{number:02}  {label}"))).frame(false).truncate());
                                    home.widgets.push((format!("chapter:{number}"), r.rect));
                                    if r.clicked() {
                                        choice = Some(Choice::Watch { investigation: chosen, timeline: row.info, start_frame: *frame });
                                    }
                                }
                            }
                            Err(e) => {
                                ui.label(RichText::new(row.info.title).font(FontId::new(24.0, ui::bold())).color(palette::ACCENT));
                                ui.add(egui::Label::new(RichText::new(format!("{} {e}", tr("home.not_built_yet"))).color(palette::TEXT_FAINT)).wrap());
                            }
                        }
                        ui.add_space(10.0);
                    }
                });
                // The demonstrations, with their keys in the viewer.
                ui.vertical(|ui| {
                    ui.label(RichText::new(tr("home.demonstrations")).font(FontId::new(24.0, ui::bold())).color(palette::ACCENT));
                    const KEYS: [&str; 12] = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "−", "="];
                    for (i, demo) in chosen.demos().iter().enumerate() {
                        ui.add(egui::Label::new(RichText::new(format!("{}  {}", KEYS.get(i).unwrap_or(&" "), chosen.demo_title(*demo))).color(palette::TEXT_DIM)).truncate());
                    }
                });
            });
        });
        let footer = tr("home.esc_hint");
        ink.text(pos2(40.0, canvas.y - 58.0), Align2::LEFT_TOP, footer, 24.0, FontFamily::Proportional, palette::TEXT_FAINT);

        if let Some(e) = new_engine {
            self.engine = e;
            self.feature = 0;
            self.edition = 0;
        }
        if let Some(f) = new_feature {
            self.feature = f;
            self.edition = 0;
        }
        if let Some(e) = new_edition {
            self.edition = e;
        }
        choice
    }

    /// The Config page: the settings, Save and Apply, Discard, and Restore
    /// Defaults; while settings are tried, a popup counts down over it.
    fn draw_config(&mut self, ctx: &egui::Context, ink: &ui::Ink, canvas: Vec2, time: f32) {
        if self.settings.trial.is_some_and(|trial| time - trial.started >= TRIAL_SECONDS) {
            self.restore_settings();
        }
        let trying = self.settings.trial.is_some();
        ink.text(pos2(40.0, 20.0), Align2::LEFT_TOP, "RENDER LAB", 24.0, ui::bold(), palette::ACCENT);
        ink.text(pos2(38.0, 52.0), Align2::LEFT_TOP, tr("home.config"), 38.0, ui::bold(), palette::TEXT);
        let card = Rect::from_center_size(pos2(canvas.x / 2.0, canvas.y * 0.5), vec2(1240.0, 470.0));
        glass_panel(ink.painter, card, 24, true);
        let inner = card.shrink2(vec2(48.0, 40.0));
        egui::Area::new(egui::Id::unique("home-config")).fixed_pos(inner.min).fade_in(false).constrain(false).show(ctx, |ui| {
            ui.set_width(inner.width());
            // A checkbox as large as the text beside it.
            let spacing = ui.spacing_mut();
            spacing.icon_width = 30.0;
            spacing.icon_width_inner = 18.0;
            spacing.icon_spacing = 14.0;
            ui.add_enabled_ui(!trying, |ui| {
                ui.label(RichText::new(tr("home.display")).font(FontId::new(24.0, ui::bold())).color(palette::ACCENT));
                ui.add_space(10.0);
                let defaults = Config::default();
                // An option that differs from its default is marked, and its Restore puts the default back.
                let changed = self.settings.pending.fullscreen != defaults.fullscreen;
                let row = ui.horizontal(|ui| {
                    let mut fullscreen = self.settings.pending.fullscreen;
                    let r = ui.checkbox(&mut fullscreen, RichText::new(tr("home.full_screen")).font(FontId::new(30.0, ui::bold())));
                    self.widgets.push(("option:fullscreen".into(), r.rect));
                    self.settings.pending.fullscreen = fullscreen;
                    if changed {
                        ui.add_space(12.0);
                        ui.label(RichText::new(format!("{} {}", tr("home.default"), on_off(defaults.fullscreen))).color(palette::ACCENT));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let restore = glass_button(ui, vec2(190.0, 48.0), tr("home.restore"), 26.0, !trying);
                            self.widgets.push(("restore:fullscreen".into(), restore.rect));
                            if restore.clicked() {
                                self.settings.pending.fullscreen = defaults.fullscreen;
                            }
                        });
                    }
                });
                if changed {
                    let r = row.response.rect;
                    ui.painter().rect_filled(Rect::from_min_max(pos2(r.min.x - 20.0, r.min.y + 4.0), pos2(r.min.x - 14.0, r.max.y - 4.0)), CornerRadius::same(3), palette::ACCENT);
                }
                ui.add(egui::Label::new(RichText::new(tr("home.full_screen_hint")).color(palette::TEXT_DIM)).wrap());
                ui.add_space(36.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 18.0;
                    let size = vec2((inner.width() - 36.0) / 3.0, 72.0);
                    // Save and Apply only when something changed; otherwise Return.
                    if self.settings.pending != self.settings.saved {
                        let save = glass_button(ui, size, tr("home.save_and_apply"), 30.0, !trying);
                        self.widgets.push(("save".into(), save.rect));
                        if save.clicked() {
                            self.try_settings(time);
                        }
                    } else {
                        let back = glass_button(ui, size, tr("home.return"), 30.0, !trying);
                        self.widgets.push(("return".into(), back.rect));
                        if back.clicked() {
                            self.page = Page::Title;
                            self.settings.message = None;
                        }
                    }
                    let discard = glass_button(ui, size, tr("home.discard"), 30.0, !trying);
                    self.widgets.push(("discard".into(), discard.rect));
                    if discard.clicked() {
                        self.discard_settings();
                    }
                    let defaults = glass_button(ui, size, tr("home.restore_defaults"), 30.0, !trying);
                    self.widgets.push(("defaults".into(), defaults.rect));
                    if defaults.clicked() {
                        self.settings.pending = Config::default();
                        self.settings.message = Some(tr("home.defaults_shown").to_string());
                    }
                });
                ui.add_space(24.0);
                if self.settings.pending != self.settings.saved {
                    ui.label(RichText::new(tr("home.unsaved_changes")).color(palette::ACCENT));
                }
                if let Some(message) = &self.settings.message {
                    ui.add(egui::Label::new(RichText::new(message).color(palette::TEXT_DIM)).wrap());
                }
                let stored = match &self.settings.path {
                    Some(path) => format!("{} {}", tr("home.stored_in"), path.display()),
                    None => tr("home.no_config_dir").to_string(),
                };
                ui.add(egui::Label::new(RichText::new(stored).color(palette::TEXT_FAINT)).wrap());
            });
        });
        let footer = if trying { tr("home.esc_restores") } else { tr("home.esc_discards") };
        ink.text(pos2(40.0, canvas.y - 58.0), Align2::LEFT_TOP, footer, 24.0, FontFamily::Proportional, palette::TEXT_FAINT);

        // The countdown popup, above everything else on the page.
        let Some(trial) = self.settings.trial else { return };
        let left = (TRIAL_SECONDS - (time - trial.started)).ceil().max(1.0) as u32;
        egui::Area::new(egui::Id::unique("home-config-popup")).order(egui::Order::Foreground).fixed_pos(Pos2::ZERO).fade_in(false).constrain(false).show(ctx, |ui| {
            let painter = ui.painter();
            painter.rect_filled(Rect::from_min_size(Pos2::ZERO, canvas), CornerRadius::ZERO, Color32::from_rgba_unmultiplied(2, 6, 12, 150));
            let popup = Rect::from_center_size(pos2(canvas.x / 2.0, canvas.y * 0.5), vec2(820.0, 360.0));
            painter.rect_filled(popup, CornerRadius::same(22), Color32::from_rgba_unmultiplied(10, 22, 36, 235));
            painter.rect_stroke(popup, CornerRadius::same(22), Stroke::new(2.0, palette::ACCENT), StrokeKind::Inside);
            painter.text(pos2(popup.center().x, popup.min.y + 44.0), Align2::CENTER_TOP, tr("home.keep_these_settings"), FontId::new(38.0, ui::bold()), palette::TEXT);
            let countdown = tr_with("home.countdown", args!(seconds = left));
            painter.text(pos2(popup.center().x, popup.min.y + 110.0), Align2::CENTER_TOP, countdown, FontId::new(28.0, FontFamily::Proportional), palette::TEXT_DIM);
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
            ui.label(RichText::new(heading).font(FontId::new(24.0, ui::bold())).color(palette::ACCENT));
            ui.add_space(8.0);
            add(self, ui);
        });
    }
}

/// Points per pixel and the canvas in points for a target, as in `ui::Layout`.
fn canvas_for(width: u32, height: u32, ui_scale: f32) -> (f32, Vec2) {
    let scale = (width as f32 / REFERENCE.x).min(height as f32 / REFERENCE.y) * ui_scale;
    (scale, vec2(width as f32 / scale, height as f32 / scale))
}

/// The scene behind the page: the renderer draws the current investigation's
/// backdrop full-screen, and the frost blurs it.
struct Backdrop {
    renderer: Renderer,
    frost: Frost,
}

impl Backdrop {
    fn new(gpu: Gpu, size: [u32; 2], output: wgpu::TextureFormat) -> Result<Backdrop> {
        let frost = Frost::new(&gpu.device, output);
        let renderer = Renderer::new(gpu, Statics::prepare()?, size[0].max(64), size[1].max(64))?;
        Ok(Backdrop { renderer, frost })
    }

    fn gpu(&self) -> &Gpu {
        &self.renderer.gpu
    }

    /// Render the scene at `time` seconds into the frame target, at `size`.
    fn render(&mut self, size: [u32; 2], time: f32) {
        let size = [size[0].max(64), size[1].max(64)];
        if [self.renderer.width, self.renderer.height] != size {
            self.renderer.resize(size[0], size[1]);
        }
        let state = investigation::current().backdrop(time);
        self.renderer.render_backdrop(state.as_ref(), time);
    }

    /// The device, the rendered frame and its size, and the frost.
    fn parts(&mut self) -> (&Gpu, &wgpu::TextureView, [u32; 2], &mut Frost) {
        (&self.renderer.gpu, &self.renderer.frame_view, [self.renderer.width, self.renderer.height], &mut self.frost)
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
    /// `time`, frosted, then the page. Returns the choice made.
    #[allow(clippy::too_many_arguments)]
    fn paint(&mut self, backdrop: &mut Backdrop, view: &wgpu::TextureView, size: [u32; 2], ui_scale: f32, events: Vec<egui::Event>, time: f32, home: &mut Home, readback: Option<(&wgpu::Texture, &wgpu::Buffer)>) -> Option<Choice> {
        let (scale, canvas) = canvas_for(size[0], size[1], ui_scale);
        let mut raw = egui::RawInput { time: Some(time as f64), events, screen_rect: Some(Rect::from_min_size(Pos2::ZERO, canvas)), ..Default::default() };
        raw.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point = Some(scale);
        let mut choice = None;
        let mut output = self.ctx.run_pass(raw, |ctx| choice = home.draw(ctx, canvas, time));
        // The backdrop follows the page's investigation (the selection page may have changed it).
        backdrop.render(size, time);
        let jobs = self.ctx.tessellate(output.shapes, output.pixels_per_point);
        let screen = egui_wgpu::ScreenDescriptor { size_in_pixels: size, pixels_per_point: output.pixels_per_point };
        let (gpu, frame, frame_size, frost) = backdrop.parts();
        let (device, queue) = (&gpu.device, &gpu.queue);
        for (id, deltas) in output.textures_delta.set.drain() {
            for delta in &deltas {
                self.renderer.update_texture(device, queue, id, delta);
            }
        }
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("home") });
        let mut commands = self.renderer.update_buffers(device, queue, &mut encoder, &jobs, &screen);
        frost.blur(device, &mut encoder, frame, frame_size);
        {
            let mut pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("home"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment { view, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store } })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                })
                .forget_lifetime();
            frost.draw(&mut pass);
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
    fn new(options: &HomeOptions) -> Result<Offscreen> {
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
        let backdrop = Backdrop::new(gpu, size, FRAME_FORMAT)?;
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
        home.settings.pending.fullscreen = true;
        home.try_settings(0.0);
    }
    let mut offscreen = Offscreen::new(options)?;
    // egui sizes the areas on the first pass.
    offscreen.frame(&mut home, Vec::new());
    let rgba = offscreen.capture(&mut home);
    crate::lab::readback::write_png(png, offscreen.size[0], offscreen.size[1], &rgba)?;
    eprintln!("homepage: {} ({}x{}, smallest text {:.1} pt)", png.display(), offscreen.size[0], offscreen.size[1], home.min_text);
    Ok(())
}

/// Click through the homepage offscreen and check what each control opens.
pub fn autotest(options: &HomeOptions, report: &Path) -> Result<()> {
    // A settings file of its own beside the report, so the user's is never touched.
    let config_path = report.with_file_name("home-autotest-config.toml");
    let _ = std::fs::remove_file(&config_path);
    let mut home = Home::new(options.manifest_dir.clone(), Some(config_path.clone()));
    let mut offscreen = Offscreen::new(options)?;
    offscreen.frame(&mut home, Vec::new());
    offscreen.frame(&mut home, Vec::new());
    let mut checks = Vec::new();
    let mut check = |name: &str, passed: bool, detail: String| {
        eprintln!("home autotest {name}: {} {detail}", if passed { "ok" } else { "FAILED" });
        checks.push(serde_json::json!({ "name": name, "passed": passed, "detail": detail }));
    };
    let describe = |c: Option<Choice>| c.map(|c| c.describe()).unwrap_or_else(|| "nothing".into());

    // The Config page: settings are tried, then kept or put back; only Apply writes the file.
    let stored = || Config::load(&config_path).ok().filter(|_| config_path.exists());
    let has = |home: &Home, name: &str| home.widgets.iter().any(|w| w.0 == name);
    offscreen.click(&mut home, "config");
    check("the title's Config opens the Config page", home.page == Page::Config, format!("{:?}", home.page));
    check("with nothing changed, the first button is Return, and no option is marked", has(&home, "return") && !has(&home, "save") && !has(&home, "restore:fullscreen"), String::new());
    offscreen.click(&mut home, "option:fullscreen");
    check("a change turns Return into Save and Apply, and marks the option with its Restore", has(&home, "save") && !has(&home, "return") && has(&home, "restore:fullscreen"), String::new());
    check("ticking Full Screen changes nothing until it is applied", home.settings.pending.fullscreen && !home.fullscreen && stored().is_none(), format!("pending {:?}, fullscreen {}", home.settings.pending, home.fullscreen));
    offscreen.click(&mut home, "save");
    let trying = home.settings.trial.is_some() && home.fullscreen;
    check("Save and Apply tries the settings without writing them", trying && stored().is_none(), format!("trial {}, fullscreen {}, file {:?}", home.settings.trial.is_some(), home.fullscreen, stored()));
    offscreen.click(&mut home, "trial:restore");
    check("the popup's Restore puts the previous settings back", home.settings.trial.is_none() && !home.fullscreen && !home.settings.pending.fullscreen && stored().is_none(), format!("fullscreen {}, pending {:?}", home.fullscreen, home.settings.pending));
    offscreen.click(&mut home, "option:fullscreen");
    offscreen.click(&mut home, "save");
    offscreen.frames += (TRIAL_SECONDS * 60.0) as u64 + 30;
    offscreen.frame(&mut home, Vec::new());
    check("after ten seconds without Apply, the previous settings come back", home.settings.trial.is_none() && !home.fullscreen && stored().is_none(), format!("fullscreen {}, file {:?}", home.fullscreen, stored()));
    offscreen.click(&mut home, "option:fullscreen");
    offscreen.click(&mut home, "save");
    offscreen.click(&mut home, "trial:apply");
    check("the popup's Apply keeps the settings and writes the file", home.fullscreen && stored() == Some(Config { fullscreen: true }), format!("fullscreen {}, file {:?}", home.fullscreen, stored()));
    check("right after saving, the first button is Return again", has(&home, "return") && !has(&home, "save"), String::new());
    offscreen.click(&mut home, "restore:fullscreen");
    check("an option's Restore shows its default without applying it", !home.settings.pending.fullscreen && home.fullscreen && has(&home, "save") && !has(&home, "restore:fullscreen") && stored() == Some(Config { fullscreen: true }), format!("pending {:?}, fullscreen {}", home.settings.pending, home.fullscreen));
    offscreen.click(&mut home, "option:fullscreen");
    home.toggle_fullscreen();
    check("F11 changes the session only", !home.fullscreen && stored() == Some(Config { fullscreen: true }) && home.settings.pending.fullscreen, format!("fullscreen {}, file {:?}", home.fullscreen, stored()));
    home.toggle_fullscreen();
    offscreen.click(&mut home, "defaults");
    check("Restore Defaults shows the defaults without applying them", home.settings.pending == Config::default() && home.fullscreen && stored() == Some(Config { fullscreen: true }), format!("pending {:?}, fullscreen {}", home.settings.pending, home.fullscreen));
    offscreen.click(&mut home, "discard");
    check("Discard drops the edits and returns to the title", home.page == Page::Title && home.settings.pending == home.settings.saved, format!("{:?}, pending {:?}", home.page, home.settings.pending));
    offscreen.click(&mut home, "config");
    offscreen.click(&mut home, "return");
    check("Return goes back to the title", home.page == Page::Title, format!("{:?}", home.page));
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
    choice: Option<Choice>,
    error: Option<anyhow::Error>,
    /// The sound of a control being clicked.
    click: &'a mut Option<AudioPlayer>,
}

impl HomeApp<'_> {
    fn open(&mut self, event_loop: &ActiveEventLoop) {
        if self.surface.is_some() || self.error.is_some() {
            return;
        }
        let attributes = Window::default_attributes().with_title("Render Lab").with_inner_size(self.size).with_fullscreen(fullscreen_mode(self.home.fullscreen));
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
            surface.configure(&gpu.device, &config);
            let painter = Painter::new(&gpu.device, format);
            let backdrop = Backdrop::new(gpu, [config.width, config.height], format)?;
            Ok(Surface { window, surface, config, backdrop, painter })
        })();
        match result {
            Ok(s) => {
                self.started = Instant::now();
                self.shown_fullscreen = self.home.fullscreen;
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
        let choice = s.painter.paint(&mut s.backdrop, &view, [s.config.width, s.config.height], self.options.ui_scale, events, time, self.home, None);
        if std::mem::take(&mut self.home.clicked) {
            if let Some(click) = self.click.as_mut() {
                click.play_from(0.0);
            }
        }
        s.window.pre_present_notify();
        s.backdrop.gpu().queue.present(frame);
        // F11, or settings being tried or put back.
        if self.shown_fullscreen != self.home.fullscreen {
            self.shown_fullscreen = self.home.fullscreen;
            s.window.set_fullscreen(fullscreen_mode(self.shown_fullscreen));
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
                // The window follows on the next redraw.
                PhysicalKey::Code(KeyCode::F11) => self.home.toggle_fullscreen(),
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

/// The homepage's music and the sound of its controls (assets/audio/README.md); the
/// lab is silent without them or without FFplay.
const MUSIC: &str = "assets/audio/music/soundza-bright-choir-loop-in-em.ogg";
const CLICK: &str = "assets/audio/ui/soundza-ui-button-click.wav";

/// The homepage, then whatever it opens, until it quits or a window is closed.
pub fn run(options: HomeOptions) -> Result<()> {
    let mut event_loop = EventLoop::new()?;
    let mut home = Home::new(options.manifest_dir.clone(), options.config_path.clone());
    let mut size = PhysicalSize::new(options.width, options.height);
    // The main theme's music plays while the homepage is shown, not in a demonstration or a video.
    let mut music = AudioPlayer::new(options.manifest_dir.join(MUSIC), &options.manifest_dir).map(|m| m.looping(0.5, 2.0));
    // So are its clicks. The player outlives each page, so the click that opens the viewer is heard.
    let mut click = AudioPlayer::new(options.manifest_dir.join(CLICK), &options.manifest_dir);
    loop {
        if let Some(m) = music.as_mut() {
            m.play_from(0.0);
        }
        event_loop.set_control_flow(ControlFlow::Poll);
        let mut app = HomeApp { home: &mut home, options: &options, size, surface: None, events: Vec::new(), cursor: Pos2::ZERO, started: Instant::now(), shown_fullscreen: false, choice: None, error: None, click: &mut click };
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

/// Run the viewer on what the homepage chose; false to quit.
fn open(choice: Choice, event_loop: &mut EventLoop<()>, home: &mut Home, options: &HomeOptions, size: PhysicalSize<u32>) -> bool {
    let (investigation, timeline, audio, start_frame) = match choice {
        Choice::Quit => return false,
        Choice::Watch { investigation, timeline, start_frame } => (investigation, Some(options.manifest_dir.join(timeline.path)), Some(options.manifest_dir.join(timeline.narration)), start_frame),
        Choice::Explore { investigation } => (investigation, None, None, 0),
    };
    investigation::set(investigation);
    let locale = crate::lab::i18n::current();
    let viewer_options = ViewerOptions {
        timeline,
        audio,
        manifest_dir: options.manifest_dir.clone(),
        ui_scale: options.ui_scale,
        autotest: None,
        width: size.width,
        height: size.height,
        locale: None,
        start_frame,
        fullscreen: home.fullscreen,
    };
    let exit = viewer::run_on(event_loop, viewer_options);
    // A timeline in another language switched the text; the homepage keeps its own.
    crate::lab::i18n::set(locale);
    match exit {
        Ok((ViewerExit::Back, still_fullscreen)) => home.fullscreen = still_fullscreen,
        Ok((ViewerExit::Closed, _)) => return false,
        // Show what went wrong on the selection page instead of quitting.
        Err(e) => home.error = Some(format!("{e:#}")),
    }
    true
}
