//! Frame composition with egui: header, the struct panel, 3D labels,
//! insets, captions or the viewer's source panel, source cards, diagrams
//! (src/diagram.rs), and the footer. Every text call goes through `Ink`, which records
//! the smallest size used so the recorder can prove nothing is below 24 px
//! at 1080p.

use crate::formula;
use crate::frame::{FrameContent, Statics};
use crate::i18n::{args, tr, tr_with};
use crate::investigation;
use crate::layout::{FieldKind, RustField};
use crate::palette::{self, field_colors};
use crate::rich_text;
use crate::shot::{SceneType, DemoId, PanelMode, ShotState, Timeline};
use crate::sources;
use egui::{pos2, vec2, Align2, Color32, CornerRadius, FontFamily, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2};

pub use crate::text::{bold, install_fonts, restyle, Ink, CODE_PANEL_WIDTH, MIN_TEXT_PT, REFERENCE};
use crate::text::mono_bold;

pub use crate::config::ViewerLayout;

/// The least a dragged panel, dock, and viewport keep, in points.
pub const MIN_PANEL: f32 = 320.0;
pub const MIN_DOCK: f32 = 150.0;
pub const MIN_VIEWPORT: Vec2 = vec2(480.0, 300.0);

/// The edges of the viewer's layout a drag moves (`ViewerAction::ResizeLayout`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutPart {
    /// The struct panel's width.
    Panel,
    /// The pseudo-code panel's width.
    CodePanel,
    /// The dock's height.
    Dock,
}

/// Screen layout in logical points: a 1920 x 1080 canvas scaled uniformly to
/// the target, widened (or heightened) to fill other aspect ratios.
#[derive(Clone, Copy, Debug)]
pub struct Layout {
    pub scale: f32,
    pub canvas: Vec2,
    pub viewport: Rect,
    pub panel: Rect,
    pub content: Rect,
    pub captions: Rect,
    pub footer: Rect,
}

impl Layout {
    pub fn new(width: u32, height: u32) -> Layout {
        Layout::with_ui_scale(width, height, 1.0)
    }

    /// The layout with everything drawn `ui_scale` times its size: the canvas
    /// grows by 1 / `ui_scale`, so text, panels, and labels shrink together.
    /// The recorder uses 1; the viewer may use less, for smaller text.
    pub fn with_ui_scale(width: u32, height: u32, ui_scale: f32) -> Layout {
        let scale = (width as f32 / REFERENCE.x).min(height as f32 / REFERENCE.y) * ui_scale;
        let canvas = vec2(width as f32 / scale, height as f32 / scale);
        let margin = 40.0;
        let content_top = 112.0;
        let content_bottom = canvas.y - 240.0;
        let panel_width = 590.0;
        let panel = Rect::from_min_max(pos2(canvas.x - margin - panel_width, content_top), pos2(canvas.x - margin, content_bottom));
        let viewport = Rect::from_min_max(pos2(margin, content_top), pos2(panel.min.x - 24.0, content_bottom));
        let content = Rect::from_min_max(viewport.min, panel.max);
        let captions = Rect::from_min_max(pos2(margin, canvas.y - 228.0), pos2(canvas.x - margin, canvas.y - 66.0));
        let footer = Rect::from_min_max(pos2(margin, canvas.y - 58.0), pos2(canvas.x - margin, canvas.y - 8.0));
        Layout { scale, canvas, viewport, panel, content, captions, footer }
    }

    /// The interactive viewer's layout: the viewport and the struct panel
    /// end higher, and the captions band becomes the taller slider dock.
    pub fn with_dock(mut self) -> Layout {
        let bottom = self.canvas.y - 290.0;
        self.viewport.max.y = bottom;
        self.panel.max.y = bottom;
        self.content.max.y = bottom;
        self.captions.min.y = bottom + 12.0;
        self
    }

    /// A wider panel for pseudo-code listings; the viewport gives up the width.
    pub fn with_code_panel(self) -> Layout {
        self.with_panel_width(CODE_PANEL_WIDTH)
    }

    /// The panel `width` points wide (kept between `MIN_PANEL` and what leaves
    /// the viewport `MIN_VIEWPORT` wide); the viewport takes the rest.
    pub fn with_panel_width(mut self, width: f32) -> Layout {
        let most = (self.panel.max.x - self.content.min.x - 24.0 - MIN_VIEWPORT.x).max(MIN_PANEL);
        self.panel.min.x = self.panel.max.x - width.clamp(MIN_PANEL, most);
        self.viewport.max.x = self.panel.min.x - 24.0;
        self
    }

    /// The dock `height` points tall (kept between `MIN_DOCK` and what leaves
    /// the viewport `MIN_VIEWPORT` tall); the viewport and the panel end above it.
    pub fn with_dock_height(mut self, height: f32) -> Layout {
        let most = (self.captions.max.y - 12.0 - self.content.min.y - MIN_VIEWPORT.y).max(MIN_DOCK);
        let bottom = self.captions.max.y - height.clamp(MIN_DOCK, most) - 12.0;
        self.viewport.max.y = bottom;
        self.panel.max.y = bottom;
        self.content.max.y = bottom;
        self.captions.min.y = bottom + 12.0;
        self
    }

    /// The viewer's layout with the sizes the user dragged (`ViewerLayout`):
    /// the dock's height (with `dock`), and the panel's or (`code`) the
    /// pseudo-code panel's width.
    pub fn with_user(self, user: &ViewerLayout, dock: bool, code: bool) -> Layout {
        let mut layout = self;
        if let (true, Some(h)) = (dock, user.dock_height) {
            layout = layout.with_dock_height(h as f32);
        }
        match (code, if code { user.code_panel_width } else { user.panel_width }) {
            (_, Some(w)) => layout.with_panel_width(w as f32),
            (true, None) => layout.with_code_panel(),
            (false, None) => layout,
        }
    }

    /// The same layout with the 3D viewport widened over the panel's area
    /// when the struct panel is hidden.
    pub fn for_panel(mut self, panel_visible: bool) -> Layout {
        if !panel_visible {
            self.viewport = self.content;
        }
        self
    }

    /// The 3D viewport in target pixels: x, y, width, height.
    pub fn viewport_px(&self) -> [u32; 4] {
        let x0 = (self.viewport.min.x * self.scale).round() as u32;
        let y0 = (self.viewport.min.y * self.scale).round() as u32;
        let x1 = (self.viewport.max.x * self.scale).round() as u32;
        let y1 = (self.viewport.max.y * self.scale).round() as u32;
        [x0, y0, (x1 - x0).max(1), (y1 - y0).max(1)]
    }

    /// The viewport cut into `n` views side by side, 24 points apart, each
    /// the same whole number of target pixels wide (so they share one set of
    /// viewport targets).
    pub fn split(&self, n: usize) -> Vec<Rect> {
        let v = self.viewport_px();
        let n = n.max(1) as u32;
        let gap = (24.0 * self.scale).round() as u32;
        let width = (v[2].saturating_sub(gap * (n - 1)) / n).max(1);
        let point = |px: u32| px as f32 / self.scale;
        (0..n).map(|k| {
            let x = v[0] + k * (width + gap);
            Rect::from_min_max(pos2(point(x), point(v[1])), pos2(point(x + width), point(v[1] + v[3])))
        }).collect()
    }

    pub fn viewport_size_px(&self) -> [u32; 2] {
        let v = self.viewport_px();
        [v[2], v[3]]
    }

    /// Canvas point of a viewport pixel.
    pub fn viewport_pixel_to_canvas(&self, px: glam::Vec2) -> Pos2 {
        let v = self.viewport_px();
        pos2((v[0] as f32 + px.x) / self.scale, (v[1] as f32 + px.y) / self.scale)
    }

    /// Viewport pixel of a target pixel (e.g. a mouse position).
    pub fn target_to_viewport_pixel(&self, x: f32, y: f32) -> Option<glam::Vec2> {
        let v = self.viewport_px();
        let p = glam::Vec2::new(x - v[0] as f32, y - v[1] as f32);
        (p.x >= 0.0 && p.y >= 0.0 && p.x < v[2] as f32 && p.y < v[3] as f32).then_some(p)
    }
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct PlacedLabel {
    pub text: String,
    pub anchor: [f32; 2],
    pub rect: [f32; 4],
}

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct UiReport {
    pub min_text_pt: f32,
    pub labels: Vec<PlacedLabel>,
    pub label_overlaps: usize,
    pub labels_outside: usize,
    /// (field, displayed value lines) for the highlighted fields.
    pub panel_values: Vec<(String, Vec<String>)>,
    pub panel_struct: Option<String>,
    /// The pseudo-code listing drawn, and how many of its lines did not fit.
    pub panel_code: Option<String>,
    pub code_overflow: usize,
    /// The readout's formula: the scale it was drawn at (1 is full size), and
    /// how many readout rows and notes did not fit below it.
    pub formula_scale: Option<f32>,
    pub readout_overflow: usize,
    /// The insets drawn: where each is, where its picture is, and its name
    /// for clicks (`PictureInset::click`), in canvas points.
    #[serde(skip)]
    pub insets: Vec<InsetArea>,
}

#[derive(Clone, Debug)]
pub struct InsetArea {
    pub frame: Rect,
    pub picture: Rect,
    pub click: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ViewerAction {
    SelectDemo(DemoId),
    /// Play or pause; in a demonstration, play or stop its animation.
    TogglePlay,
    Step,
    /// Everything back to the demonstration's start, camera included.
    Reset,
    ToggleTimeline,
    SetParam(&'static str, f32),
    SetCamera(crate::camera::OrbitCamera),
    SetRecord(crate::shot::RecordRef),
    SetHighlight(Vec<String>),
    SetPanel(PanelMode),
    /// The dock's values back to the demonstration's defaults.
    ResetParameters,
    SelectTab(DockTab),
    /// Show a pseudo-code listing (the Code tab).
    SetCode(&'static str),
    /// Move the timeline to this share of its length.
    Seek(f32),
    /// Close the viewer and return to the homepage.
    Home,
    /// The narration's volume; `true` when the change is finished (src/volume.rs).
    SetNarration(crate::volume::Volume, bool),
    /// One value (a channel) back to the demonstration's default (the Experiments tab).
    ResetParam(&'static str),
    /// The live state becomes the reference the experiment compares against (src/experiment.rs).
    SetReference,
    ClearReference,
    /// The reference and the live state trade places.
    SwapReference,
    SetCompare(crate::experiment::Compare),
    /// The wipe's line, as a share of the scene area's width.
    SetSplit(f32),
    /// The difference view's gain.
    SetGain(f32),
    /// Measure the live image against the reference's (src/metrics.rs).
    Measure,
    /// Save the experiment under this name among the user's.
    SavePreset(String),
    /// Load one of the experiments the tab lists (`ExperimentView::presets`).
    LoadPreset(usize),
    /// A layout edge dragged to this size in points, or (None) back to its default.
    ResizeLayout(LayoutPart, Option<f32>),
    /// A drag of a layout edge ended: the layout is written to the settings.
    SaveLayout,
    /// The whole layout back to its defaults (and written).
    ResetLayout,
    /// Open or close the skill graph full screen (src/skills.rs).
    SetSkillsOverlay(bool),
}

/// What the viewer's dock shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DockTab {
    #[default]
    Parameters,
    /// The demonstration's pseudo-code in the panel; the dock picks a listing and lights its steps.
    Code,
    Sources,
    /// A paused timeline's caption, to read again (only with a timeline).
    Captions,
    /// Every value of the investigation, and the experiment's tools (src/experiment.rs).
    Experiments,
    /// What the audience should know before the chapter, from the skill
    /// graph (src/skills.rs); the panel shows the graph.
    Prerequisites,
}

/// The skill graph's context in the viewer (src/skills.rs): the chapters the
/// Prerequisites tab and the skills panel are about (none, or none the graph
/// declares: the whole investigation), and a skill to focus once (a snapshot's).
#[derive(Clone, Debug, Default)]
pub struct SkillsChrome {
    pub chapters: Vec<u32>,
    pub focus: Option<String>,
    /// The skill graph full screen, over the viewer.
    pub overlay: bool,
}

/// What the Experiments tab shows of the viewer's experiment (src/experiment.rs).
#[derive(Clone, Debug, Default)]
pub struct ExperimentView {
    /// A reference is set.
    pub reference: bool,
    pub compare: crate::experiment::Compare,
    pub split: f32,
    pub gain: f32,
    pub measurement: Option<crate::metrics::Measurement>,
    pub message: Option<String>,
    pub presets: Vec<crate::experiment::Preset>,
    /// GPU time per pass (src/timing.rs): name, mean and 95th percentile in milliseconds.
    pub timings: Vec<(String, f64, f64)>,
    /// The viewer's frame time: mean and 95th percentile in milliseconds.
    pub frame_ms: (f64, f64),
    /// Seconds the scene data has been rebuilding (`Investigation::scene_data`).
    pub rebuilding: Option<f32>,
}

/// One beat of the timeline, as the bar's hover preview names it.
#[derive(Clone, Debug)]
pub struct BarSection {
    /// Where the beat starts, as a share of the timeline.
    pub start: f32,
    pub chapter: u32,
    pub chapter_title: String,
    pub headline: String,
}

/// How a paused timeline's diagram is shown in its panel: the zoom (1 fits the
/// panel), and the point of the picture (0..1 across, 0..1 down) at the panel's centre.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DiagramView {
    pub zoom: f32,
    pub center: Vec2,
}

impl Default for DiagramView {
    fn default() -> DiagramView {
        DiagramView { zoom: 1.0, center: vec2(0.5, 0.5) }
    }
}

impl DiagramView {
    pub const MAX_ZOOM: f32 = 8.0;

    /// The picture's rectangle for a fitted rectangle `fit`.
    pub fn rect(&self, fit: Rect) -> Rect {
        let size = fit.size() * self.zoom;
        Rect::from_min_size(fit.center() - vec2(self.center.x * size.x, self.center.y * size.y), size)
    }
}

pub struct ViewerChrome {
    pub playing: bool,
    pub timeline: Option<(u64, u64)>,
    pub timeline_mode: bool,
    /// Where each chapter starts, as a share of the timeline.
    pub chapters: Vec<f32>,
    /// Every beat, in order, for the bar's hover preview; and the timeline's rate.
    pub sections: Vec<BarSection>,
    pub fps: u32,
    /// Output: the preview shown under the pointer this frame, if any.
    pub hover: Option<String>,
    /// The caption on screen while the timeline plays; paused, the one at or
    /// last before the frame (the Captions tab), with the one before it.
    pub caption: Option<String>,
    pub previous_caption: Option<String>,
    /// The seconds the timeline just skipped (Left and Right), and the notice's opacity.
    pub skip: Option<(i64, f32)>,
    pub status: String,
    pub tab: DockTab,
    pub actions: Vec<ViewerAction>,
    /// True while the pointer is over an egui widget (input belongs to egui).
    pub ui_has_pointer: bool,
    /// Canvas rectangle of each dock slider, by parameter (for the autotest).
    pub widgets: Vec<(String, Rect)>,
    /// Messages over the viewer, above everything else (src/popup.rs).
    pub popups: crate::popup::Popups,
    /// The narration's volume, which the dock and the timeline bar set.
    pub narration: crate::volume::Volume,
    /// Paused on a diagram beat: the diagram panel's zoom and pan.
    pub diagram: Option<DiagramView>,
    /// The Experiments tab's view of the experiment.
    pub experiment: ExperimentView,
    /// The Prerequisites tab's and the skills panel's chapters.
    pub skills: SkillsChrome,
}

pub enum Chrome<'a> {
    Recording { timeline: &'a Timeline, frame: u64 },
    Viewer(&'a mut ViewerChrome),
}

pub struct UiInput<'a> {
    pub layout: Layout,
    pub state: &'a ShotState,
    pub content: Option<&'a FrameContent>,
    pub statics: &'a Statics,
    /// The GPU textures an inset's picture can show (packages/rlstd/src/inset.rs).
    pub textures: &'a crate::utility::inset::Textures,
    pub scene_type: SceneType,
    /// The diagram a 2D scene shows (src/diagram.rs).
    pub diagram: Option<crate::diagram::BeatDiagram<'a>>,
    pub beat_sources: &'a [String],
    /// A 2D scene of the legal notices, not the sources card.
    pub notices: bool,
    pub chapter_number: u32,
    pub fade: f32,
    pub stamp: Option<u64>,
    /// The views beside the main one (`Investigation::panes`), left to right:
    /// each drawn with its own border, title, and labels.
    pub panes: &'a [PaneView<'a>],
    /// The main view's title, when the scene area is split.
    pub title: Option<&'a str>,
    /// An experiment's comparison drawn with the live view (src/experiment.rs).
    pub compare: Option<CompareOverlay>,
    /// What the performance panel shows (`PanelMode::Performance`).
    pub performance: Option<&'a PerformanceView>,
}

/// The frame's cost as the performance panel shows it (src/timing.rs): the
/// GPU, each pass's mean, 95th percentile, and last time in milliseconds,
/// and the recent frames' GPU times and frame times, oldest first.
#[derive(Clone, Debug, Default)]
pub struct PerformanceView {
    pub gpu: String,
    /// The device has timestamp queries.
    pub timestamps: bool,
    pub passes: Vec<(String, f64, f64, f64)>,
    pub totals: Vec<f64>,
    pub frames: Vec<f64>,
}

/// How an experiment's comparison shows (src/experiment.rs): its mode, the
/// reference's view (beside the live one, or the same), the wipe's line, the
/// difference's gain.
#[derive(Clone, Copy, Debug)]
pub struct CompareOverlay {
    pub mode: crate::experiment::Compare,
    pub reference: Rect,
    pub split: f32,
    pub gain: f32,
}

/// A view beside the main one, as the UI draws it.
#[derive(Clone, Copy)]
pub struct PaneView<'a> {
    pub rect: Rect,
    pub content: &'a FrameContent,
    pub title: Option<&'a str>,
}

pub fn rust_fields(name: &str) -> Vec<RustField> {
    use crate::layout::*;
    match name {
        "ObjectData" => ObjectData::fields(),
        "CameraData" => CameraData::fields(),
        "FrameUniforms" => FrameUniforms::fields(),
        _ => vec![],
    }
}

fn read_f32(bytes: &[u8], offset: usize) -> f32 {
    f32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}
fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn fmt_f(v: f32) -> String {
    let v = if v.abs() < 5e-4 { 0.0 } else { v };
    format!("{v:.3}")
}

/// Vectors of three or four floats use two decimals so they fit the panel.
fn fmt_f2(v: f32) -> String {
    let v = if v.abs() < 5e-3 { 0.0 } else { v };
    format!("{v:.2}")
}

fn fmt_u(v: u32) -> String {
    if v == u32::MAX { "INVALID".into() } else { v.to_string() }
}

/// Displayed value lines of one field, read from the record bytes.
pub fn format_field(bytes: &[u8], field: &RustField) -> Vec<String> {
    match field.kind {
        FieldKind::F32(1) => vec![fmt_f(read_f32(bytes, field.offset))],
        FieldKind::F32(n) => {
            let f = if n >= 3 { fmt_f2 } else { fmt_f };
            vec![format!("({})", (0..n as usize).map(|i| f(read_f32(bytes, field.offset + 4 * i))).collect::<Vec<_>>().join(", "))]
        }
        FieldKind::U32(1) => vec![fmt_u(read_u32(bytes, field.offset))],
        FieldKind::U32(n) => vec![format!("({})", (0..n as usize).map(|i| fmt_u(read_u32(bytes, field.offset + 4 * i))).collect::<Vec<_>>().join(", "))],
        FieldKind::Mat3 | FieldKind::Mat4 => {
            let n = if field.kind == FieldKind::Mat3 { 3 } else { 4 };
            (0..n)
                .map(|r| (0..n).map(|c| format!("{:>7}", fmt_f(read_f32(bytes, field.offset + 16 * c + 4 * r)))).collect::<Vec<_>>().join(""))
                .collect()
        }
        FieldKind::Nested => vec!["…".into()],
    }
}

pub fn draw(ctx: &egui::Context, input: &UiInput, mut chrome: Chrome) -> UiReport {
    let painter = ctx.layer_painter(egui::LayerId::background());
    let ink = Ink::new(&painter);
    let l = &input.layout;
    let mut report = UiReport::default();

    draw_header(&ink, input, &mut chrome, ctx);
    let scene_beat = input.scene_type == SceneType::ThreeD;
    // The Experiments tab puts its values in the panel's place.
    // The performance panel, when asked for, takes the panel's place over the values.
    let value_panel = scene_beat && !matches!(input.state.panel, PanelMode::Performance | PanelMode::Skills) && matches!(&chrome, Chrome::Viewer(v) if v.tab == DockTab::Experiments && !(v.timeline_mode && v.playing));
    match input.scene_type {
        SceneType::ThreeD => {
            painter.rect_stroke(l.viewport.expand(1.0), CornerRadius::same(10), Stroke::new(1.5, palette::border()), StrokeKind::Outside);
            // The other views: each its border, its title, and its labels.
            for pane in input.panes {
                painter.rect_stroke(pane.rect.expand(1.0), CornerRadius::same(10), Stroke::new(1.5, palette::border()), StrokeKind::Outside);
                let at = UiInput { layout: Layout { viewport: pane.rect, ..*l }, content: Some(pane.content), ..*input };
                let title = pane.title.map(|t| draw_pane_title(&ink, pane.rect, t, &[])).into_iter().collect::<Vec<_>>();
                draw_labels(&ink, &at, pane.content, &title, &mut report);
            }
            if let Some(content) = input.content {
                // Insets and the callout first; labels then avoid them.
                let mut obstacles = Vec::new();
                // The investigation's insets, then the image the story shows.
                let package = investigation::current().package().unwrap_or("rlstd");
                let story: Vec<_> = input.state.images.iter().filter_map(|image| crate::utility::inset::story(image, &content.images, package)).collect();
                // Insets sharing a corner stack in a row away from it, and in a
                // new row once the next would leave the viewport.
                let mut rows: Vec<(crate::utility::inset::Corner, egui::Vec2, f32)> = Vec::new();
                for inset in content.insets.iter().chain(&story).filter(|i| i.alpha > 0.0) {
                    let size = crate::utility::inset::frame_size(ctx, &ink, l, input.textures, inset);
                    let offset = match rows.iter_mut().find(|(corner, _, _)| *corner == inset.corner) {
                        None => {
                            rows.push((inset.corner, vec2(size.x + 12.0, 0.0), size.y));
                            Vec2::ZERO
                        }
                        Some((_, at, tallest)) => {
                            if at.x + size.x > l.viewport.width() - 32.0 {
                                *at = vec2(0.0, at.y + *tallest + 12.0);
                                *tallest = 0.0;
                            }
                            let offset = *at;
                            at.x += size.x + 12.0;
                            *tallest = tallest.max(size.y);
                            offset
                        }
                    };
                    let (frame, picture) = crate::utility::inset::draw(ctx, &ink, l, &content.uniforms.camera, input.textures, inset, offset);
                    obstacles.push(frame);
                    report.insets.push(InsetArea { frame, picture, click: inset.click.clone() });
                }
                if let Some(title) = input.title {
                    let insets = obstacles.clone();
                    obstacles.push(draw_pane_title(&ink, l.viewport, title, &insets));
                }
                if let Some(compare) = input.compare {
                    let insets = obstacles.clone();
                    obstacles.extend(draw_compare(&ink, l.viewport, compare, &insets));
                }
                if let Some(key) = &input.state.callout {
                    let insets = obstacles.clone();
                    obstacles.extend(draw_callout(&ink, l, key, &insets));
                }
                if let Some(tag) = &input.state.formula {
                    let insets = obstacles.clone();
                    obstacles.extend(draw_formula_card(&ink, l, tag, input.state.formula_term, input.state.formula_lit, &insets));
                }
                draw_labels(&ink, input, content, &obstacles, &mut report);
                if input.state.panel == PanelMode::Skills {
                    let skills = match &chrome {
                        Chrome::Viewer(v) => v.skills.clone(),
                        Chrome::Recording { .. } => SkillsChrome::default(),
                    };
                    if draw_skills_panel(ctx, &ink, l.panel, &skills) {
                        if let Chrome::Viewer(v) = &mut chrome {
                            v.actions.push(ViewerAction::SetSkillsOverlay(true));
                        }
                    }
                } else if !value_panel {
                    draw_panel(&ink, input, content, &mut report);
                }
            } else if let Some(key) = &input.state.callout {
                draw_callout(&ink, l, key, &[]);
            }
        }
        // A 2D scene: the beat's diagram, its sources card, or the notices.
        SceneType::TwoD if input.diagram.is_none() && input.notices => draw_notices_card(&ink, input),
        SceneType::TwoD if input.diagram.is_none() => draw_source_card(&ink, input),
        SceneType::TwoD => {
            if let Some(diagram) = input.diagram {
                let area = l.content;
                let view = match &chrome {
                    Chrome::Viewer(v) => v.diagram,
                    Chrome::Recording { .. } => None,
                };
                match view {
                    None => crate::diagram::draw(&ink, area, None, diagram),
                    Some(v) => {
                        // The diagram panel of a paused timeline: zoomed and panned, clipped to the panel.
                        crate::diagram::draw(&ink, area, Some(v.rect(area)), diagram);
                        let hint = tr_with("viewer.diagram_hint", args!(zoom = format!("{:.1}", v.zoom)));
                        let at = pos2(area.min.x + 14.0, area.max.y - 12.0);
                        let galley = painter.layout_no_wrap(hint.clone(), egui::FontId::proportional(24.0), palette::text_dim());
                        painter.rect_filled(Rect::from_min_size(at - vec2(8.0, galley.size().y + 4.0), galley.size() + vec2(16.0, 8.0)), CornerRadius::same(6), Color32::from_rgba_unmultiplied(7, 18, 31, 220));
                        ink.text(at, Align2::LEFT_BOTTOM, hint, 24.0, FontFamily::Proportional, palette::text_dim());
                    }
                }
            }
        }
    }
    if input.fade > 0.0 {
        let a = (input.fade.clamp(0.0, 1.0) * 255.0) as u8;
        let bg = palette::background();
        painter.rect_filled(l.content.expand(4.0), CornerRadius::ZERO, Color32::from_rgba_unmultiplied(bg.r(), bg.g(), bg.b(), a));
    }
    match &mut chrome {
        Chrome::Recording { timeline, frame } => {
            draw_captions(&ink, l, timeline, *frame);
            draw_footer(&ink, l, Some((timeline, *frame)));
        }
        Chrome::Viewer(viewer) => {
            if viewer.timeline_mode && viewer.playing {
                draw_caption_text(&ink, l, viewer.caption.as_deref());
            } else {
                crate::controls::draw_dock(ctx, &ink, input, viewer);
                if value_panel {
                    crate::controls::draw_experiment_panel(ctx, &ink, input, viewer);
                }
            }
            if let Some((seconds, opacity)) = viewer.skip {
                draw_skip_notice(&ink, l, seconds, opacity);
            }
            if viewer.timeline_mode {
                draw_timeline_bar(ctx, &ink, l, viewer);
            } else {
                draw_footer(&ink, l, None);
            }
            // The skill graph full screen, over everything but the popups.
            if viewer.skills.overlay && draw_skills_overlay(ctx, l.canvas, &viewer.skills.clone()) {
                viewer.actions.push(ViewerAction::SetSkillsOverlay(false));
            }
            viewer.popups.draw(ctx, l.canvas);
            viewer.ui_has_pointer |= ctx.egui_wants_pointer_input() || viewer.skills.overlay;
        }
    }
    let _ = scene_beat;
    if let Some(index) = input.stamp {
        // One 8x8 block per bit at the top-left corner: red = 1, green = 0.
        for bit in 0..24u32 {
            let on = (index >> bit) & 1 == 1;
            let s = 8.0 / l.scale;
            let r = Rect::from_min_size(pos2(bit as f32 * s, 0.0), vec2(s, s));
            painter.rect_filled(r, CornerRadius::ZERO, if on { Color32::from_rgb(255, 0, 0) } else { Color32::from_rgb(0, 255, 0) });
        }
    }
    report.min_text_pt = ink.min_size.get();
    report
}

fn draw_header(ink: &Ink, input: &UiInput, chrome: &mut Chrome, ctx: &egui::Context) {
    let l = &input.layout;
    let engine = sources::index();
    // The engine's version and commit, then its package's copyright and license
    // line (`credit-text`), fitted beside what the left of the header holds.
    let mut right = format!("{} {} · {}", investigation::current().wording().engine_tag, engine.engine.version, &engine.engine.commit[..8]);
    if let Some(credit) = crate::resources::credit(investigation::current().package().unwrap_or("rlstd")) {
        right = format!("{right} · {credit}");
    }
    let measure = |t: &str| rich_text::measure(ink, t, 24.0, FontFamily::Proportional).x;
    let left = match chrome {
        // The chapter's tag, in bold as it is drawn below.
        Chrome::Recording { timeline, frame } => {
            let chapter = timeline.chapter_at(*frame);
            rich_text::measure(ink, &format!("{:02} / {:02}   {}", chapter.number, timeline.chapters.len(), chapter.title.to_uppercase()), 24.0, bold()).x
        }
        Chrome::Viewer(_) => {
            let investigation = investigation::current();
            let title = measure(&format!("RENDER LAB · {}", input.state.demo.title().to_uppercase()));
            let scenes = investigation.scenes();
            title + if scenes.len() > 1 { 28.0 + scenes.iter().map(|s| measure(s.title()) + 24.0).sum::<f32>() } else { 0.0 }
        }
    };
    let right = ink.fit_right(&right, (l.canvas.x - 80.0 - left - 24.0).max(200.0), 24.0, FontFamily::Proportional);
    ink.text(pos2(l.canvas.x - 40.0, 26.0), Align2::RIGHT_TOP, &right, 24.0, FontFamily::Proportional, palette::text_faint());
    match chrome {
        Chrome::Recording { timeline, frame } => {
            let chapter = timeline.chapter_at(*frame);
            let (_, beat) = timeline.beat_at(*frame);
            let tag = format!("{:02} / {:02}   {}", chapter.number, timeline.chapters.len(), chapter.title.to_uppercase());
            ink.text(pos2(40.0, 20.0), Align2::LEFT_TOP, &tag, 24.0, bold(), palette::accent());
            let headline = ink.fit_right(&beat.headline, l.canvas.x - 80.0, 38.0, bold());
            ink.text(pos2(38.0, 52.0), Align2::LEFT_TOP, headline, 38.0, bold(), palette::text());
        }
        Chrome::Viewer(viewer) => {
            let title = ink.text(pos2(40.0, 14.0), Align2::LEFT_TOP, format!("RENDER LAB · {}", input.state.demo.title().to_uppercase()), 24.0, bold(), palette::accent());
            ink.font(24.0, FontFamily::Proportional);
            // The scene tabs, on the title's line, when the demonstrations show more than one scene.
            let investigation = investigation::current();
            let shown = investigation.demo_scene(input.state.demo);
            let scenes = investigation.scenes();
            if scenes.len() > 1 {
                egui::Area::new(egui::Id::unique("viewer-scenes")).fixed_pos(pos2(title.max.x + 28.0, 8.0)).show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        for scene in scenes {
                            if ui.selectable_label(scene == shown, egui::RichText::new(scene.title()).strong()).clicked() && scene != shown {
                                viewer.actions.push(ViewerAction::SelectDemo(investigation.scene_demos(scene)[0]));
                            }
                        }
                    });
                });
            }
            let response = egui::Area::new(egui::Id::unique("viewer-controls")).fixed_pos(pos2(36.0, 50.0)).show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // The shown scene's demonstrations.
                    for (i, demo) in investigation.scene_demos(shown).iter().enumerate() {
                        let selected = input.state.demo == *demo && !viewer.timeline_mode;
                        if ui.selectable_label(selected, format!("{} {}", i + 1, investigation.demo_tab(*demo))).clicked() {
                            viewer.actions.push(ViewerAction::SelectDemo(*demo));
                        }
                    }
                    ui.separator();
                    if ui.button(if viewer.playing { tr("ui.pause") } else { tr("ui.play") }).clicked() {
                        viewer.actions.push(ViewerAction::TogglePlay);
                    }
                    if ui.button(tr("ui.step")).clicked() {
                        viewer.actions.push(ViewerAction::Step);
                    }
                    if ui.button(tr("ui.reset")).clicked() {
                        viewer.actions.push(ViewerAction::Reset);
                    }
                    if viewer.timeline.is_some() && ui.selectable_label(viewer.timeline_mode, tr("ui.timeline")).clicked() {
                        viewer.actions.push(ViewerAction::ToggleTimeline);
                    }
                    ui.separator();
                    if ui.button(tr("ui.home")).on_hover_text(tr("ui.home_hint")).clicked() {
                        viewer.actions.push(ViewerAction::Home);
                    }
                });
            });
            if input.scene_type == SceneType::ThreeD && !(viewer.timeline_mode && viewer.playing) {
                draw_layout_handles(ctx, &ink, input, viewer);
            }
            viewer.ui_has_pointer = ctx.egui_wants_pointer_input() || response.response.hovered();
            // The status fills what the controls leave of the row.
            let room = l.canvas.x - 40.0 - response.response.rect.max.x - 24.0;
            if room > 60.0 {
                let status = ink.fit_right(&viewer.status, room, 24.0, FontFamily::Proportional);
                ink.text(pos2(l.canvas.x - 40.0, 62.0), Align2::RIGHT_TOP, status, 24.0, FontFamily::Proportional, palette::text_dim());
            }
        }
    }
}

/// The viewer's draggable edges: between the viewport and the panel (its
/// width), and above the dock (its height). A drag sets the size, its end
/// writes the layout, and a double-click puts that size back to its default.
fn draw_layout_handles(ctx: &egui::Context, ink: &Ink, input: &UiInput, viewer: &mut ViewerChrome) {
    let l = &input.layout;
    let code = matches!(input.state.panel, PanelMode::Code | PanelMode::Skills);
    let mut handles: Vec<(LayoutPart, Rect, egui::CursorIcon)> = Vec::new();
    // The panel's edge, when the panel shows beside the viewport.
    if l.viewport.max.x + 1.0 < l.panel.min.x {
        let part = if code { LayoutPart::CodePanel } else { LayoutPart::Panel };
        handles.push((part, Rect::from_min_max(pos2(l.viewport.max.x, l.content.min.y), pos2(l.panel.min.x, l.content.max.y)), egui::CursorIcon::ResizeHorizontal));
    }
    handles.push((LayoutPart::Dock, Rect::from_min_max(pos2(l.captions.min.x, l.content.max.y - 2.0), pos2(l.captions.max.x, l.captions.min.y + 2.0)), egui::CursorIcon::ResizeVertical));
    for (part, rect, cursor) in handles {
        let id = egui::Id::unique(match part {
            LayoutPart::Panel => "layout-panel-edge",
            LayoutPart::CodePanel => "layout-code-edge",
            LayoutPart::Dock => "layout-dock-edge",
        });
        egui::Area::new(id).fixed_pos(rect.min).order(egui::Order::Background).show(ctx, |ui| {
            let (_, response) = ui.allocate_exact_size(rect.size(), egui::Sense::click_and_drag());
            let response = response.on_hover_text(tr("ui.resize_hint"));
            if response.hovered() || response.dragged() {
                ctx.set_cursor_icon(cursor);
                let stroke = Stroke::new(3.0, if response.dragged() { palette::accent() } else { palette::border() });
                match part {
                    LayoutPart::Dock => ink.painter.hline(rect.x_range().shrink(24.0), rect.center().y, stroke),
                    _ => ink.painter.vline(rect.center().x, rect.y_range().shrink(24.0), stroke),
                };
            }
            if response.double_clicked() {
                viewer.actions.push(ViewerAction::ResizeLayout(part, None));
                viewer.actions.push(ViewerAction::SaveLayout);
            } else if response.dragged() {
                if let Some(p) = response.interact_pointer_pos() {
                    let size = match part {
                        LayoutPart::Dock => l.captions.max.y - p.y - 6.0,
                        _ => l.panel.max.x - p.x - 12.0,
                    };
                    viewer.actions.push(ViewerAction::ResizeLayout(part, Some(size)));
                }
            }
            if response.drag_stopped() {
                viewer.actions.push(ViewerAction::SaveLayout);
            }
        });
    }
}

fn draw_labels(ink: &Ink, input: &UiInput, content: &FrameContent, obstacles: &[Rect], report: &mut UiReport) {
    let l = &input.layout;
    let projector = crate::camera::Projector::new(&content.uniforms.camera);
    let bounds = l.viewport.shrink(6.0);
    let mut placed: Vec<Rect> = Vec::new();
    let blocked = |r: &Rect| obstacles.iter().any(|o| o.intersects(*r));
    for label in &content.labels {
        let Some((px, _)) = projector.project(label.anchor) else { continue };
        let anchor = l.viewport_pixel_to_canvas(px);
        if !l.viewport.contains(anchor) {
            continue;
        }
        let size = rich_text::measure(ink, &label.text, 24.0, FontFamily::Proportional) + vec2(22.0, 10.0);
        let near = [vec2(20.0, -size.y - 14.0), vec2(20.0, 14.0), vec2(-size.x - 20.0, -size.y - 14.0), vec2(-size.x - 20.0, 14.0), vec2(-size.x * 0.5, -size.y - 30.0), vec2(-size.x * 0.5, 30.0), vec2(36.0, -size.y * 0.5), vec2(-size.x - 36.0, -size.y * 0.5), vec2(20.0, -size.y - 60.0), vec2(20.0, 60.0)];
        // Farther rings for crowded anchors; the leader line keeps the link visible.
        let mut candidates: Vec<Rect> = near.iter().map(|o| Rect::from_min_size(anchor + *o, size)).collect();
        for ring in [2.0f32, 3.2] {
            for k in 0..12 {
                let angle = k as f32 / 12.0 * std::f32::consts::TAU;
                let d = vec2(angle.cos(), angle.sin()) * (size.y + 28.0) * ring;
                candidates.push(Rect::from_center_size(anchor + d + vec2(angle.cos() * size.x * 0.5, 0.0), size));
            }
        }
        if obstacles.iter().any(|o| o.contains(anchor)) {
            continue;
        }
        let score = |r: &Rect| {
            let overlap: f32 = placed.iter().chain(obstacles).map(|p| p.intersect(*r)).filter(|i| i.is_positive()).map(|i| i.area()).sum();
            let outside = if bounds.contains_rect(*r) { 0.0 } else { 1e6 };
            overlap + outside
        };
        let best = candidates.iter().copied().min_by(|a, b| score(a).total_cmp(&score(b))).unwrap();
        let rect = translate_inside(best, bounds);
        if placed.iter().any(|p| p.intersects(rect)) || blocked(&rect) {
            report.label_overlaps += 1;
        }
        if !bounds.contains_rect(rect) {
            report.labels_outside += 1;
        }
        placed.push(rect);
        let painter = ink.painter;
        let edge = rect.clamp(anchor);
        painter.line_segment([anchor, edge], Stroke::new(2.0, label.color));
        painter.circle_filled(anchor, 4.0, label.color);
        painter.rect_filled(rect, CornerRadius::same(6), Color32::from_rgba_premultiplied(9, 22, 36, 235));
        painter.rect_filled(Rect::from_min_max(rect.min, pos2(rect.min.x + 5.0, rect.max.y)), CornerRadius::same(2), label.color);
        painter.rect_stroke(rect, CornerRadius::same(6), Stroke::new(1.0, palette::border()), StrokeKind::Inside);
        rich_text::text(ink, pos2(rect.min.x + 13.0, rect.center().y), Align2::LEFT_CENTER, &label.text, 24.0, FontFamily::Proportional, palette::text());
        report.labels.push(PlacedLabel { text: label.text.clone(), anchor: [anchor.x, anchor.y], rect: [rect.min.x, rect.min.y, rect.max.x, rect.max.y] });
    }
}

fn translate_inside(r: Rect, bounds: Rect) -> Rect {
    let mut d = vec2(0.0, 0.0);
    if r.min.x < bounds.min.x {
        d.x = bounds.min.x - r.min.x;
    } else if r.max.x > bounds.max.x {
        d.x = bounds.max.x - r.max.x;
    }
    if r.min.y < bounds.min.y {
        d.y = bounds.min.y - r.min.y;
    } else if r.max.y > bounds.max.y {
        d.y = bounds.max.y - r.max.y;
    }
    r.translate(d)
}

/// The source callout in the viewport's bottom-left corner, raised above any
/// inset it would overlap.
/// The narration's formula card (`ShotState::formula`) in the viewport's
/// top-right corner: the formula, with term `term` lit to `lit` in its tag's
/// colour for it.
fn draw_formula_card(ink: &Ink, l: &Layout, tag: &str, term: usize, lit: f32, insets: &[Rect]) -> Option<Rect> {
    let (latex, display, colors) = rich_text::first_formula(tag)?;
    let f = formula::get(&latex, formula::FORMULA_PX, display).map_err(|e| eprintln!("formula card {latex:?}: {e}")).ok()?;
    let pad = formula::PAD as f32;
    let ink_size = vec2(f.width as f32, f.height as f32) - Vec2::splat(2.0 * pad);
    let size = ink_size + vec2(36.0, 28.0);
    let mut rect = Rect::from_min_size(pos2(l.viewport.max.x - 16.0 - size.x, l.viewport.min.y + 16.0), size);
    // Below whatever already sits in that corner (an inset, the callout).
    while let Some(inset) = insets.iter().find(|i| i.intersects(rect)) {
        rect = rect.translate(vec2(0.0, inset.max.y + 8.0 - rect.min.y));
    }
    ink.painter.rect_filled(rect, CornerRadius::same(10), Color32::from_rgba_premultiplied(7, 18, 31, 235));
    ink.painter.rect_stroke(rect, CornerRadius::same(10), Stroke::new(1.5, palette::border()), StrokeKind::Inside);
    let lit = (term > 0 && lit > 0.0).then(|| (rich_text::Lit { term, weight: lit, color: colors.get(term - 1).copied().unwrap_or(palette::accent()) }, palette::text_dim()));
    rich_text::draw_formula(ink, rect.min + vec2(18.0 - pad, 14.0 - pad), &latex, formula::FORMULA_PX, display, &f, 1.0, palette::text(), lit);
    Some(rect.expand(4.0))
}

/// A view's title at the middle of its top edge (the corners hold the insets,
/// the formula card, and the callout); beside an inset it would cover when
/// the top edge has room, else below it. Returns the area it covers.
/// An experiment's comparison over the views: the wipe's line, the
/// reference's border beside the live view, and which view is which.
fn draw_compare(ink: &Ink, view: Rect, compare: CompareOverlay, insets: &[Rect]) -> Vec<Rect> {
    use crate::experiment::Compare;
    let (a, b) = (tr("experiments.label_reference"), tr("experiments.label_live"));
    match compare.mode {
        Compare::Off => Vec::new(),
        Compare::Wipe => {
            let x = view.min.x + view.width() * compare.split.clamp(0.0, 1.0);
            ink.painter.vline(x, view.y_range(), Stroke::new(2.0, palette::accent()));
            let left = Rect::from_min_max(view.min, pos2(x, view.max.y));
            let right = Rect::from_min_max(pos2(x, view.min.y), view.max);
            let mut out = Vec::new();
            if left.width() > 120.0 {
                out.push(draw_pane_title(ink, left, a, insets));
            }
            if right.width() > 120.0 {
                let mut avoid = insets.to_vec();
                avoid.extend(out.iter().copied());
                out.push(draw_pane_title(ink, right, b, &avoid));
            }
            out
        }
        Compare::SideBySide => {
            ink.painter.rect_stroke(compare.reference.expand(1.0), CornerRadius::same(10), Stroke::new(1.5, palette::border()), StrokeKind::Outside);
            vec![draw_pane_title(ink, compare.reference, a, &[]), draw_pane_title(ink, view, b, insets)]
        }
        Compare::Difference => vec![draw_pane_title(ink, view, &tr_with("experiments.label_difference", args!(gain = format!("{:.0}", compare.gain))), insets)],
        Compare::Reference => vec![draw_pane_title(ink, view, a, insets)],
    }
}

fn draw_pane_title(ink: &Ink, view: Rect, title: &str, insets: &[Rect]) -> Rect {
    let size = ink.measure(title, 24.0, FontFamily::Proportional) + vec2(28.0, 14.0);
    let mut rect = Rect::from_min_size(pos2(view.center().x - size.x * 0.5, view.min.y + 16.0), size);
    while let Some(inset) = insets.iter().find(|i| i.intersects(rect)) {
        let beside = Rect::from_min_size(pos2(inset.max.x + 12.0, rect.min.y), size);
        rect = if beside.max.x <= view.max.x - 16.0 && !insets.iter().any(|i| i.intersects(beside)) { beside } else { rect.translate(vec2(0.0, inset.max.y + 8.0 - rect.min.y)) };
    }
    ink.painter.rect_filled(rect, CornerRadius::same(8), Color32::from_rgba_premultiplied(7, 18, 31, 235));
    ink.painter.rect_stroke(rect, CornerRadius::same(8), Stroke::new(1.5, palette::border()), StrokeKind::Inside);
    ink.text(rect.left_center() + vec2(14.0, 0.0), Align2::LEFT_CENTER, title, 24.0, FontFamily::Proportional, palette::text());
    rect.expand(4.0)
}

fn draw_callout(ink: &Ink, l: &Layout, key: &str, insets: &[Rect]) -> Option<Rect> {
    let index = sources::index();
    let entry = index.entries.get(key)?;
    // Within the viewport: the symbol alone when the file name does not fit beside it.
    let room = l.viewport.width() - 32.0 - 28.0;
    let mut text = format!("{} · {}", entry.file_name(), entry.short_symbol());
    if ink.measure(&text, 24.0, FontFamily::Monospace).x > room {
        text = ink.fit_right(&entry.short_symbol(), room, 24.0, FontFamily::Monospace);
    }
    let size = ink.measure(&text, 24.0, FontFamily::Monospace) + vec2(28.0, 14.0);
    let mut rect = Rect::from_min_size(pos2(l.viewport.min.x + 16.0, l.viewport.max.y - 16.0 - size.y), size);
    for inset in insets {
        if rect.intersects(*inset) {
            rect = rect.translate(vec2(0.0, inset.min.y - 8.0 - rect.max.y));
        }
    }
    ink.painter.rect_filled(rect, CornerRadius::same(8), Color32::from_rgba_premultiplied(7, 18, 31, 235));
    ink.painter.rect_stroke(rect, CornerRadius::same(8), Stroke::new(1.5, palette::accent()), StrokeKind::Inside);
    ink.text(rect.left_center() + vec2(14.0, 0.0), Align2::LEFT_CENTER, text, 24.0, FontFamily::Monospace, palette::accent());
    Some(rect.expand(4.0))
}

/// The performance panel: the GPU, the frame's GPU time and frame time,
/// their recent history against the frame-rate budgets, and a row per pass
/// (its share of the GPU time as a bar, its mean time), the most expensive
/// first. Passes named in `highlight` are called out (always shown), the
/// others dimmed.
fn draw_performance(ink: &Ink, rect: Rect, view: &PerformanceView, highlight: &[String]) {
    let painter = ink.painter;
    let x = rect.min.x + 22.0;
    let right = rect.max.x - 22.0;
    let width = right - x;
    let mut y = rect.min.y + 16.0;
    let mean = |xs: &[f64]| if xs.is_empty() { f64::NAN } else { xs.iter().sum::<f64>() / xs.len() as f64 };
    let ms = |v: f64| if v.is_finite() { format!("{v:.2} ms") } else { "–".into() };
    ink.text(pos2(x, y), Align2::LEFT_TOP, tr("perf.title"), 30.0, bold(), palette::text());
    y += 42.0;
    let gpu = ink.fit_right(&view.gpu, width, 24.0, FontFamily::Proportional);
    ink.text(pos2(x, y), Align2::LEFT_TOP, gpu, 24.0, FontFamily::Proportional, palette::accent());
    y += 36.0;
    // The headline numbers: the GPU's time for the frame, and the time between frames.
    let recent = |xs: &[f64]| mean(&xs[xs.len().saturating_sub(30)..]);
    let (gpu_ms, frame_ms) = (recent(&view.totals), recent(&view.frames));
    let half = width / 2.0;
    ink.text(pos2(x, y), Align2::LEFT_TOP, tr("perf.gpu"), 24.0, FontFamily::Proportional, palette::text_dim());
    ink.text(pos2(x + half, y), Align2::LEFT_TOP, tr("perf.frame"), 24.0, FontFamily::Proportional, palette::text_dim());
    y += 30.0;
    ink.text(pos2(x, y), Align2::LEFT_TOP, ms(gpu_ms), 28.0, mono_bold(), palette::text());
    let fps = if frame_ms > 0.0 { format!("{} · {:.0} fps", ms(frame_ms).replace(" ms", ""), 1000.0 / frame_ms) } else { ms(frame_ms) };
    ink.text(pos2(x + half, y), Align2::LEFT_TOP, ink.fit_right(&fps, half, 28.0, mono_bold()), 28.0, mono_bold(), palette::text());
    y += 42.0;
    // The history: GPU time (accent) and frame time (faint), against the
    // budgets that fit its scale; smaller when the panel is short.
    let graph_height = ((rect.max.y - y) * 0.3).clamp(60.0, 160.0);
    let graph = Rect::from_min_size(pos2(x, y), vec2(width, graph_height));
    painter.rect_filled(graph, CornerRadius::same(6), palette::background());
    // The scale: the 95th percentile, so one slow frame (a scene loading) does not flatten the rest.
    let mut all: Vec<f64> = view.totals.iter().chain(&view.frames).copied().filter(|v| v.is_finite()).collect();
    all.sort_by(|a, b| a.total_cmp(b));
    let p95 = all.get(((all.len() as f64 - 1.0) * 0.95).round().max(0.0) as usize).copied().unwrap_or(0.0);
    let top = p95.max(1000.0 / 120.0) * 1.25;
    let to_y = |v: f64| graph.max.y - (v / top).clamp(0.0, 1.0) as f32 * graph.height();
    let mut last_label: Option<f32> = None;
    for (budget, label) in [(1000.0 / 120.0, "120 fps"), (1000.0 / 60.0, "60 fps"), (1000.0 / 30.0, "30 fps")] {
        let gy = to_y(budget);
        if budget > top || gy < graph.min.y + 24.0 || last_label.is_some_and(|y| y - gy < 26.0) {
            continue;
        }
        last_label = Some(gy);
        painter.hline(graph.x_range(), gy, Stroke::new(1.0, palette::border()));
        ink.text(pos2(graph.max.x - 6.0, gy - 2.0), Align2::RIGHT_BOTTOM, label, 24.0, FontFamily::Proportional, palette::text_faint());
    }
    for (series, color, thickness) in [(&view.frames, palette::text_faint(), 1.5), (&view.totals, palette::accent(), 2.5)] {
        if series.len() < 2 {
            continue;
        }
        let n = series.len() - 1;
        let points: Vec<Pos2> = series.iter().enumerate().map(|(i, v)| pos2(graph.min.x + graph.width() * i as f32 / n as f32, to_y(*v))).collect();
        painter.add(egui::Shape::line(points, Stroke::new(thickness, color)));
    }
    y = graph.max.y + 8.0;
    let caption = tr_with("perf.history", args!(frames = view.totals.len().max(view.frames.len())));
    ink.text(pos2(x, y), Align2::LEFT_TOP, ink.fit_right(&caption, width, 24.0, FontFamily::Proportional), 24.0, FontFamily::Proportional, palette::text_faint());
    y += 34.0;
    painter.line_segment([pos2(x, y), pos2(right, y)], Stroke::new(1.0, palette::border()));
    y += 10.0;
    if !view.timestamps {
        ink.text(pos2(x, y), Align2::LEFT_TOP, ink.fit_right(tr("perf.no_timestamps"), width, 24.0, FontFamily::Proportional), 24.0, FontFamily::Proportional, palette::text_dim());
        return;
    }
    if view.passes.is_empty() {
        ink.text(pos2(x, y), Align2::LEFT_TOP, tr("perf.measuring"), 24.0, FontFamily::Proportional, palette::text_dim());
        return;
    }
    // One row per pass, the most expensive first, as many as fit: its name,
    // a bar of its share of the frame's GPU time, and its mean. The passes
    // called out come first among equals: they always show.
    let total: f64 = view.passes.iter().map(|p| p.1).sum::<f64>().max(1e-6);
    let any_lit = view.passes.iter().any(|p| highlight.contains(&p.0));
    let mut order: Vec<&(String, f64, f64, f64)> = view.passes.iter().collect();
    order.sort_by(|a, b| highlight.contains(&b.0).cmp(&highlight.contains(&a.0)).then(b.1.total_cmp(&a.1)));
    let row = 32.0;
    let room = ((rect.max.y - 12.0 - y) / row).floor().max(1.0) as usize;
    let shown = if order.len() > room { room.saturating_sub(1).max(1) } else { order.len() };
    // Called-out passes first, then by cost: the rows drawn are re-sorted by cost.
    let mut rows: Vec<&(String, f64, f64, f64)> = order[..shown].to_vec();
    rows.sort_by(|a, b| b.1.total_cmp(&a.1));
    let name_width = width * 0.45;
    let bar_x = x + name_width + 10.0;
    let bar_width = width * 0.28;
    for (name, mean_ms, _, _) in rows {
        let lit = highlight.contains(name);
        if lit {
            painter.rect_filled(Rect::from_min_size(pos2(x - 10.0, y - 2.0), vec2(width + 20.0, row)), CornerRadius::same(5), palette::highlight_row());
        }
        let text_color = if lit || !any_lit { palette::text() } else { palette::text_faint() };
        ink.text(pos2(x, y), Align2::LEFT_TOP, ink.fit_right(pass_label(name), name_width, 24.0, FontFamily::Monospace), 24.0, FontFamily::Monospace, text_color);
        let track = Rect::from_min_size(pos2(bar_x, y + 9.0), vec2(bar_width, 11.0));
        painter.rect_filled(track, CornerRadius::same(3), palette::background());
        let share = (*mean_ms / total).clamp(0.0, 1.0) as f32;
        let bar_color = if lit || !any_lit { palette::accent() } else { palette::border() };
        painter.rect_filled(Rect::from_min_size(track.min, vec2((track.width() * share).max(2.0), track.height())), CornerRadius::same(3), bar_color);
        let value = ms(*mean_ms);
        ink.text(pos2(right, y), Align2::RIGHT_TOP, value, 24.0, FontFamily::Monospace, text_color);
        y += row;
    }
    if shown < order.len() {
        let rest: f64 = order[shown..].iter().map(|p| p.1).sum();
        let more = tr_with("perf.more", args!(count = order.len() - shown, time = format!("{rest:.2}")));
        ink.text(pos2(x, y), Align2::LEFT_TOP, ink.fit_right(&more, width, 24.0, FontFamily::Proportional), 24.0, FontFamily::Proportional, palette::text_faint());
    }
}

/// A pass's name on screen: the lab's own passes (src/renderer.rs) in the
/// interface's language; an investigation's chain passes by their entry point.
/// A story's `highlight` names them by `name`, as measured.
pub fn pass_label(name: &str) -> &str {
    match name {
        "depth views" => tr("perf.pass_depth_views"),
        "depth pre-pass" => tr("perf.pass_prepass"),
        "scene pass" => tr("perf.pass_scene"),
        "overlay" => tr("perf.pass_overlay"),
        "composite" => tr("perf.pass_composite"),
        other => other,
    }
}

fn swatch(painter: &Painter, rect: Rect, colors: &[Color32]) {
    let n = colors.len().max(1) as f32;
    for (i, c) in colors.iter().enumerate() {
        let x0 = rect.min.x + rect.width() * i as f32 / n;
        let x1 = rect.min.x + rect.width() * (i + 1) as f32 / n;
        painter.rect_filled(Rect::from_min_max(pos2(x0, rect.min.y), pos2(x1, rect.max.y)), CornerRadius::ZERO, *c);
    }
}

/// The skills panel's view, kept in egui's memory between frames: the scale
/// (points per graph unit; 0 until fitted), the graph point at the panel's
/// centre, the selected skill, and the chapters it was last fitted to.
#[derive(Clone, Debug, Default)]
struct SkillsView {
    scale: f32,
    center: Vec2,
    selected: Option<usize>,
    fitted: Option<Vec<u32>>,
    /// The chrome's focus last applied (`SkillsChrome::focus`).
    focused: Option<String>,
    /// A skill to centre on the next frame (`focus_skill`).
    centre_on: Option<usize>,
}

/// Where the skill graph is drawn: the side panel, or full screen over everything.
#[derive(Clone, Copy, PartialEq)]
enum SkillsPlace {
    Panel,
    Overlay,
}

impl SkillsPlace {
    fn view_id(self) -> egui::Id {
        egui::Id::unique(if self == SkillsPlace::Panel { "skills-panel-view" } else { "skills-overlay-view" })
    }

    /// The order of its input areas: the overlay's above everything else.
    fn order(self) -> egui::Order {
        if self == SkillsPlace::Panel { egui::Order::Background } else { egui::Order::Tooltip }
    }
}

/// Select skill `node` in the skills panel and the full-screen graph, and
/// centre it there (a click in the Prerequisites tab).
pub fn focus_skill(ctx: &egui::Context, node: usize) {
    ctx.data_mut(|d| {
        for place in [SkillsPlace::Panel, SkillsPlace::Overlay] {
            let view = d.get_temp_mut_or_default::<SkillsView>(place.view_id());
            view.selected = Some(node);
            view.centre_on = Some(node);
        }
    });
}

/// The skill graph full screen, over the viewer or the homepage, for
/// `chrome`'s chapters (none: the whole story). Returns true when it is
/// closed (its button; Esc is the caller's).
pub fn draw_skills_overlay(ctx: &egui::Context, canvas: Vec2, chrome: &SkillsChrome) -> bool {
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("skills-overlay")));
    let ink = Ink::new(&painter);
    let screen = Rect::from_min_size(Pos2::ZERO, canvas);
    painter.rect_filled(screen, CornerRadius::ZERO, palette::theme().dim);
    // What is under the overlay takes no pointer input.
    egui::Area::new(egui::Id::unique("skills-overlay-block")).fixed_pos(Pos2::ZERO).order(egui::Order::Foreground).fade_in(false).show(ctx, |ui| {
        ui.allocate_rect(screen, egui::Sense::click_and_drag());
    });
    draw_skill_graph(ctx, &ink, screen.shrink(40.0), chrome, SkillsPlace::Overlay)
}

/// A skill's role for the audience, as the panel colours it.
fn skill_color(p: &crate::skills::Partition, i: usize) -> Color32 {
    if p.is_known(i) {
        palette::code_string()
    } else if p.level.contains(&i) {
        palette::accent()
    } else if p.teach.contains(&i) {
        palette::code_number()
    } else {
        palette::text_faint()
    }
}

/// The chapters the viewer's context names that the graph declares (none: every chapter).
pub fn skill_chapters(graph: &crate::skills::Graph, chrome: &SkillsChrome) -> Vec<u32> {
    chrome.chapters.iter().copied().filter(|c| graph.chapters.iter().any(|g| g.number == *c)).collect()
}

/// The skill graph in the panel (`PanelMode::Skills`). Returns true when
/// its full-screen button is pressed.
fn draw_skills_panel(ctx: &egui::Context, ink: &Ink, rect: Rect, chrome: &SkillsChrome) -> bool {
    draw_skill_graph(ctx, ink, rect, chrome, SkillsPlace::Panel)
}

/// The skill graph in `rect`: every skill, coloured by what the
/// investigation's audience knows (src/skills.rs), the context's chapters at
/// full strength; dragged to pan, scrolled to zoom, a click selects a skill
/// (its card below), a double click fits the context again. Its button at
/// the top right (full screen in the panel, close in the overlay) returns
/// true when pressed. Node titles follow the zoom, so they are drawn with
/// the painter, not `Ink` (whose smallest-size record is the recorder's check).
fn draw_skill_graph(ctx: &egui::Context, ink: &Ink, rect: Rect, chrome: &SkillsChrome, place: SkillsPlace) -> bool {
    let painter = ink.painter;
    painter.rect_filled(rect, CornerRadius::same(12), palette::panel());
    painter.rect_stroke(rect, CornerRadius::same(12), Stroke::new(1.0, palette::border()), StrokeKind::Inside);
    let x = rect.min.x + 22.0;
    let mut y = rect.min.y + 16.0;
    ink.text(pos2(x, y), Align2::LEFT_TOP, tr("skills.title"), 30.0, bold(), palette::text());
    let label = if place == SkillsPlace::Panel { tr("skills.full_screen") } else { tr("skills.close") };
    let button_size = vec2(ink.measure(label, 24.0, FontFamily::Proportional).x + 32.0, 40.0);
    let mut pressed = false;
    let button_order = if place == SkillsPlace::Panel { egui::Order::Middle } else { egui::Order::Tooltip };
    egui::Area::new(egui::Id::unique(if place == SkillsPlace::Panel { "skills-panel-button" } else { "skills-overlay-button" })).fixed_pos(pos2(rect.max.x - 16.0 - button_size.x, rect.min.y + 12.0)).order(button_order).fade_in(false).show(ctx, |ui| {
        pressed = ui.add(egui::Button::new(egui::RichText::new(label).size(24.0)).min_size(button_size)).clicked();
    });
    y += 42.0;
    // The legend: one swatch per role.
    let mut lx = x;
    for (label, color) in [(tr("skills.known"), palette::code_string()), (tr("skills.level"), palette::accent()), (tr("skills.teach"), palette::code_number()), (tr("skills.outside"), palette::text_faint())] {
        painter.rect_filled(Rect::from_min_size(pos2(lx, y + 6.0), vec2(16.0, 16.0)), CornerRadius::same(3), color);
        let r = ink.text(pos2(lx + 24.0, y), Align2::LEFT_TOP, label, 24.0, FontFamily::Proportional, palette::text_dim());
        lx = r.max.x + 22.0;
    }
    y += 34.0;
    let measure = |t: &str| ink.measure(t, 20.0, FontFamily::Proportional).x / 20.0 * 0.5;
    let Some((graph, layout)) = crate::skills::current_layout(&measure) else {
        ink.text(pos2(x, y), Align2::LEFT_TOP, tr("skills.none"), 24.0, FontFamily::Proportional, palette::text_dim());
        return pressed;
    };
    let chapters = skill_chapters(graph, chrome);
    let p = graph.audience();
    let context = graph.context(&chapters);
    let card_height = 150.0;
    let area = Rect::from_min_max(pos2(rect.min.x + 8.0, y), pos2(rect.max.x - 8.0, rect.max.y - card_height - 8.0));
    let hint = ink.fit_right(tr("skills.hint"), area.width() - 28.0, 24.0, FontFamily::Proportional);
    ink.text(pos2(x, area.max.y - 4.0), Align2::LEFT_BOTTOM, hint, 24.0, FontFamily::Proportional, palette::text_faint());
    let graph_area = Rect::from_min_max(area.min, pos2(area.max.x, area.max.y - 34.0));
    let id = place.view_id();
    let mut view = ctx.data(|d| d.get_temp::<SkillsView>(id)).unwrap_or_default();
    // The bounding box of a set of nodes, in graph units.
    let bounds = |nodes: &mut dyn Iterator<Item = usize>| -> Option<Rect> {
        nodes.map(|i| Rect::from_center_size(pos2(layout.positions[i].0, layout.positions[i].1), vec2(layout.widths[i], 1.0))).reduce(|a, b| a.union(b))
    };
    let fit = |b: Rect, view: &mut SkillsView| {
        let b = b.expand(1.0);
        // Fitted, but no smaller than titles stay readable at (the rest is a drag away).
        view.scale = (graph_area.width() / b.width()).min(graph_area.height() / b.height()).clamp(34.0, 60.0);
        view.center = b.center().to_vec2();
    };
    let whole = bounds(&mut (0..graph.nodes.len())).unwrap_or(Rect::from_min_size(Pos2::ZERO, vec2(1.0, 1.0)));
    let min_scale = ((graph_area.width() / (whole.width() + 2.0)).min(graph_area.height() / (whole.height() + 2.0)) * 0.8).min(3.0);
    if let Some(key) = &chrome.focus {
        if view.focused.as_ref() != Some(key) {
            if let Some(node) = graph.find(key) {
                view.selected = Some(node);
                view.center = vec2(layout.positions[node].0, layout.positions[node].1);
                view.scale = 36.0;
            }
            view.focused = Some(key.clone());
            view.fitted = Some(chapters.clone());
        }
    }
    if view.scale == 0.0 || view.fitted.as_ref() != Some(&chapters) {
        let b = bounds(&mut (0..graph.nodes.len()).filter(|&i| context[i])).unwrap_or(whole);
        fit(b, &mut view);
        view.fitted = Some(chapters.clone());
    }
    // A skill focused from the dock: centred, at a readable scale.
    if let Some(node) = view.centre_on.take().filter(|&n| n < graph.nodes.len()) {
        view.center = vec2(layout.positions[node].0, layout.positions[node].1);
        view.scale = view.scale.max(36.0);
    }
    // Input: an interactive area over the graph (egui then has the pointer, not the camera).
    let to_screen = |view: &SkillsView, g: (f32, f32)| graph_area.center() + (vec2(g.0, g.1) - view.center) * view.scale;
    let mut hovered = None;
    let input_id = egui::Id::unique(if place == SkillsPlace::Panel { "skills-panel-input" } else { "skills-overlay-input" });
    egui::Area::new(input_id).fixed_pos(graph_area.min).order(place.order()).fade_in(false).show(ctx, |ui| {
        let response = ui.allocate_rect(Rect::from_min_size(graph_area.min, graph_area.size()), egui::Sense::click_and_drag());
        if response.dragged() {
            view.center -= response.drag_delta() / view.scale;
        }
        let node_at = |view: &SkillsView, at: Pos2| (0..graph.nodes.len()).find(|&i| Rect::from_center_size(to_screen(view, layout.positions[i]), vec2(layout.widths[i], 1.0) * view.scale).contains(at));
        if let Some(at) = response.hover_pos() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                // Zoom about the point under the pointer, which stays where it is.
                let under = view.center + (at - graph_area.center()) / view.scale;
                view.scale = (view.scale * (scroll / 120.0).exp()).clamp(min_scale, 80.0);
                view.center = under - (at - graph_area.center()) / view.scale;
            }
            hovered = node_at(&view, at);
        }
        if response.double_clicked() {
            let b = bounds(&mut (0..graph.nodes.len()).filter(|&i| context[i])).unwrap_or(whole);
            fit(b, &mut view);
        } else if response.clicked() {
            view.selected = response.interact_pointer_pos().and_then(|at| node_at(&view, at));
        }
        if let Some(i) = hovered {
            response.on_hover_ui_at_pointer(|ui| {
                ui.set_max_width(420.0);
                ui.label(egui::RichText::new(&graph.nodes[i].title).strong().color(skill_color(&p, i)));
                ui.label(&graph.nodes[i].summary);
            });
        }
    });
    view.center = view.center.clamp(whole.min.to_vec2(), whole.max.to_vec2());
    ctx.data_mut(|d| d.insert_temp(id, view.clone()));
    // The selected skill's requirements and what requires it, lit.
    let requires = graph.requires();
    let lit: Vec<bool> = match view.selected {
        Some(s) => {
            let below = crate::skills::context(&requires, &[s]);
            (0..graph.nodes.len()).map(|i| below[i] || crate::skills::context(&requires, &[i])[s]).collect()
        }
        None => vec![false; graph.nodes.len()],
    };
    let clipped = painter.with_clip_rect(graph_area);
    let fade = |c: Color32, strong: bool| if strong { c } else { c.gamma_multiply(0.3) };
    for (r, i, bends) in &layout.bends {
        let (r, i) = (*r, *i);
        let from = to_screen(&view, layout.positions[r]) + vec2(layout.widths[r] / 2.0 * view.scale, 0.0);
        let to = to_screen(&view, layout.positions[i]) - vec2(layout.widths[i] / 2.0 * view.scale, 0.0);
        let mut points = vec![from];
        points.extend(bends.iter().map(|&b| to_screen(&view, b)));
        points.push(to);
        let on_path = view.selected.is_some() && lit[r] && lit[i];
        let color = if on_path { palette::SELECTED } else { fade(palette::border(), context[r] && context[i]) };
        let width = if on_path { 2.5 } else { 1.2 };
        clipped.add(egui::Shape::line(points, Stroke::new(width, color)));
        let tip = (view.scale * 0.25).clamp(3.0, 9.0);
        clipped.add(egui::Shape::convex_polygon(vec![to, to - vec2(tip * 1.6, tip * 0.8), to - vec2(tip * 1.6, -tip * 0.8)], color, Stroke::NONE));
    }
    let font = view.scale * 0.5;
    for i in 0..graph.nodes.len() {
        let strong = context[i] && (view.selected.is_none() || lit[i]);
        let color = fade(skill_color(&p, i), strong);
        let box_rect = Rect::from_center_size(to_screen(&view, layout.positions[i]), vec2(layout.widths[i], 1.0) * view.scale);
        if !box_rect.intersects(graph_area) {
            continue;
        }
        clipped.rect_filled(box_rect, CornerRadius::same((view.scale * 0.2) as u8), palette::panel_raised());
        let outline = if view.selected == Some(i) || hovered == Some(i) { Stroke::new(3.0, palette::SELECTED) } else { Stroke::new(if graph.nodes[i].base { 3.0 } else { 1.5 }, color) };
        clipped.rect_stroke(box_rect, CornerRadius::same((view.scale * 0.2) as u8), outline, StrokeKind::Inside);
        if font >= 8.0 {
            clipped.text(box_rect.center(), Align2::CENTER_CENTER, &graph.nodes[i].title, egui::FontId::proportional(font), fade(palette::text(), strong));
        }
    }
    // The selected skill's card under the graph.
    let card = Rect::from_min_max(pos2(rect.min.x + 8.0, rect.max.y - card_height), pos2(rect.max.x - 8.0, rect.max.y - 8.0));
    painter.line_segment([pos2(card.min.x + 14.0, card.min.y), pos2(card.max.x - 14.0, card.min.y)], Stroke::new(1.0, palette::border()));
    let cx = card.min.x + 14.0;
    let width = card.width() - 28.0;
    let Some(s) = view.selected else {
        let text = if chapters.is_empty() { tr("skills.for_all").to_string() } else { tr_with("skills.for_chapters", args!(chapters = chapters.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(", "))) };
        ink.wrapped(pos2(cx, card.min.y + 10.0), width, &text, 24.0, FontFamily::Proportional, palette::text_dim());
        return pressed;
    };
    let node = &graph.nodes[s];
    let role = if p.is_known(s) { tr("skills.known") } else if p.level.contains(&s) { tr("skills.level") } else if p.teach.contains(&s) { tr("skills.teach") } else { tr("skills.outside") };
    let title = ink.fit_right(&format!("{} · {role}", node.title), width, 24.0, bold());
    ink.text(pos2(cx, card.min.y + 8.0), Align2::LEFT_TOP, title, 24.0, bold(), skill_color(&p, s));
    let bottom = ink.wrapped(pos2(cx, card.min.y + 40.0), width, &node.summary, 24.0, FontFamily::Proportional, palette::text());
    let used = graph.chapters_using(s);
    let used = if used.is_empty() { tr("skills.no_chapter").to_string() } else { tr_with("skills.used_by", args!(chapters = used.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(", "))) };
    let used = ink.fit_right(&used, width, 24.0, FontFamily::Proportional);
    ink.text(pos2(cx, (bottom + 4.0).min(card.max.y - 30.0)), Align2::LEFT_TOP, used, 24.0, FontFamily::Proportional, palette::text_dim());
    pressed
}

fn draw_panel(ink: &Ink, input: &UiInput, content: &FrameContent, report: &mut UiReport) {
    let l = &input.layout;
    let state = input.state;
    if state.panel == PanelMode::Hidden {
        return;
    }
    let painter = ink.painter;
    let rect = l.panel;
    painter.rect_filled(rect, CornerRadius::same(12), palette::panel());
    painter.rect_stroke(rect, CornerRadius::same(12), Stroke::new(1.0, palette::border()), StrokeKind::Inside);
    if state.panel == PanelMode::Code {
        if let Some(listing) = state.code.as_deref().and_then(|c| investigation::current().listing(c)) {
            draw_code(ink, rect, listing, &state.highlight, report);
        }
        return;
    }
    if state.panel == PanelMode::Performance {
        if let Some(view) = input.performance {
            draw_performance(ink, rect, view, &state.highlight);
        }
        return;
    }
    let Some(record) = &content.panel_record else {
        if let Some(readout) = &content.readout {
            draw_readout(ink, rect, readout, &content.notes, report);
        }
        return;
    };
    let Some(layout) = input.statics.layouts.get(record.struct_name) else { return };
    let Some(decl) = input.statics.declarations.get(record.struct_name) else { return };
    report.panel_struct = Some(record.struct_name.to_string());
    let x = rect.min.x + 22.0;
    let width = rect.width() - 44.0;
    let mut y = rect.min.y + 16.0;
    ink.text(pos2(x, y), Align2::LEFT_TOP, record.struct_name, 30.0, mono_bold(), palette::text());
    ink.text(pos2(rect.max.x - 22.0, y + 4.0), Align2::RIGHT_TOP, format!("{} B / {}", layout.size, tr("ui.element")), 24.0, FontFamily::Proportional, palette::text_dim());
    y += 42.0;
    let title = ink.fit_right(&record.title, width, 24.0, FontFamily::Proportional);
    ink.text(pos2(x, y), Align2::LEFT_TOP, title, 24.0, FontFamily::Proportional, palette::accent());
    y += 32.0;
    if let Some(mapping) = sources::index().mapping(record.struct_name) {
        let text = ink.fit_right(&format!("≈ {}: {}", investigation::current().wording().engine_short, mapping.engine_short()), width, 24.0, FontFamily::Proportional);
        ink.text(pos2(x, y), Align2::LEFT_TOP, text, 24.0, FontFamily::Proportional, palette::text_faint());
        y += 34.0;
    }
    painter.line_segment([pos2(x, y), pos2(rect.max.x - 22.0, y)], Stroke::new(1.0, palette::border()));
    y += 10.0;
    let rust = rust_fields(record.struct_name);
    let highlighted = |name: &str| state.highlight.iter().any(|h| h == name);
    if state.panel == PanelMode::Memory {
        draw_memory(ink, input, record, layout, &rust, x, width, y, rect.max.y - 16.0);
        return;
    }
    // Declaration rows with byte offsets and padding.
    let line = 27.0;
    ink.text(pos2(x, y), Align2::LEFT_TOP, format!("struct {} {{", record.struct_name), 24.0, FontFamily::Monospace, palette::text_dim());
    y += line;
    let pads = layout.padding();
    let mut cursor = 0usize;
    for (_, field_name, code) in &decl.fields {
        let Some(f) = layout.field(field_name) else { continue };
        if f.offset > cursor {
            ink.text(pos2(x + 8.0, y), Align2::LEFT_TOP, format!("{:>3}  ·· {} B padding", cursor, f.offset - cursor), 24.0, FontFamily::Monospace, palette::padding());
            y += line;
        }
        let row = Rect::from_min_size(pos2(x - 8.0, y - 2.0), vec2(width + 16.0, line));
        if highlighted(field_name) {
            painter.rect_filled(row, CornerRadius::same(4), palette::highlight_row());
            swatch(painter, Rect::from_min_size(pos2(x - 6.0, y + 2.0), vec2(6.0, line - 8.0)), &field_colors(record.struct_name, field_name));
        }
        let color = if highlighted(field_name) { palette::text() } else { palette::text_dim() };
        let text = ink.fit_right(&format!("{:>3}  {code}", f.offset), width - 8.0, 24.0, FontFamily::Monospace);
        ink.text(pos2(x + 8.0, y), Align2::LEFT_TOP, text, 24.0, FontFamily::Monospace, color);
        y += line;
        cursor = f.offset + f.size;
    }
    if layout.size > cursor {
        ink.text(pos2(x + 8.0, y), Align2::LEFT_TOP, format!("{:>3}  ·· {} B padding", cursor, layout.size - cursor), 24.0, FontFamily::Monospace, palette::padding());
        y += line;
    }
    ink.text(pos2(x, y), Align2::LEFT_TOP, "}", 24.0, FontFamily::Monospace, palette::text_dim());
    y += line + 4.0;
    let _ = pads;
    if state.panel != PanelMode::Values {
        return;
    }
    // Values of the highlighted fields, read from the uploaded bytes.
    let bottom = rect.max.y - 14.0;
    for name in &state.highlight {
        let Some(field) = rust.iter().find(|f| f.name == name.as_str()) else { continue };
        let lines = format_field(&record.bytes, field);
        report.panel_values.push((name.clone(), lines.clone()));
        let needed = line * (lines.len() as f32 + if lines.len() > 1 { 1.0 } else { 0.0 });
        if y + needed > bottom {
            break;
        }
        if lines.len() == 1 {
            let one_line = format!("{name} = {}", lines[0]);
            if ink.measure(&one_line, 24.0, FontFamily::Monospace).x <= width {
                ink.text(pos2(x, y), Align2::LEFT_TOP, one_line, 24.0, FontFamily::Monospace, palette::text());
                y += line;
            } else {
                if y + 2.0 * line > bottom {
                    break;
                }
                ink.text(pos2(x, y), Align2::LEFT_TOP, format!("{name} ="), 24.0, FontFamily::Monospace, palette::text());
                y += line;
                let value = ink.fit_right(&format!("  {}", lines[0]), width, 24.0, FontFamily::Monospace);
                ink.text(pos2(x, y), Align2::LEFT_TOP, value, 24.0, FontFamily::Monospace, palette::text());
                y += line;
            }
        } else {
            let storage = if matches!(field.kind, FieldKind::Mat3) { tr("ui.3_cols_16_b") } else { tr("ui.4_cols_16_b") };
            ink.text(pos2(x, y), Align2::LEFT_TOP, format!("{name}  ({storage})"), 24.0, FontFamily::Monospace, palette::text());
            y += line;
            for row in &lines {
                ink.text(pos2(x, y), Align2::LEFT_TOP, row, 24.0, FontFamily::Monospace, palette::text());
                y += line;
            }
        }
    }
    for (note, color) in &content.notes {
        if y + line > bottom {
            break;
        }
        fitted_line(ink, pos2(x, y), note, width, *color);
        y += line;
    }
}

/// An investigation's readout (`FrameContent::readout`): its formula, names
/// on the left and values on the right, then the frame's notes.
fn draw_readout(ink: &Ink, rect: Rect, readout: &crate::frame::Readout, notes: &[(String, Color32)], report: &mut UiReport) {
    let x = rect.min.x + 22.0;
    let width = rect.width() - 44.0;
    let mut y = rect.min.y + 16.0;
    let title = ink.fit_right(&readout.title, width, 30.0, bold());
    ink.text(pos2(x, y), Align2::LEFT_TOP, title, 30.0, bold(), palette::text());
    y += 42.0;
    if !readout.subtitle.is_empty() {
        let subtitle = ink.fit_right(&readout.subtitle, width, 24.0, FontFamily::Proportional);
        ink.text(pos2(x, y), Align2::LEFT_TOP, subtitle, 24.0, FontFamily::Proportional, palette::accent());
        y += 32.0;
    }
    ink.painter.line_segment([pos2(x, y), pos2(rect.max.x - 22.0, y)], Stroke::new(1.0, palette::border()));
    y += 10.0;
    let line = 29.0;
    let bottom = rect.max.y - 14.0;
    if let Some((latex, display, colors)) = readout.formula.as_deref().and_then(rich_text::first_formula) {
        match formula::get(&latex, formula::FORMULA_PX, display) {
            Ok(f) => {
                // Full size, unless the panel is too short for it and every row (the
                // viewer's panel is shorter than the recorder's). The padding around
                // the ink overlaps the panel's margin.
                // The ink with 4 px around it, as the readouts have always had.
                let pad = formula::PAD as f32 - 4.0;
                let size = vec2(f.width as f32, f.height as f32) - Vec2::splat(2.0 * pad);
                let rows: f32 = readout.rows.iter().map(|(n, v, _)| if n.is_empty() && v.is_empty() { 0.5 } else { 1.0 }).sum::<f32>() + notes.len() as f32;
                let room = bottom - y - 12.0 - 6.0 - rows * line;
                let scale = (width / size.x).min(room / size.y).clamp(0.5, 1.0);
                let lit = readout.formula_lit.map(|(term, weight)| (rich_text::Lit { term, weight, color: colors.get(term.wrapping_sub(1)).copied().unwrap_or(palette::accent()) }, palette::text_dim()));
                rich_text::draw_formula(ink, pos2(x, y) - Vec2::splat(pad * scale), &latex, formula::FORMULA_PX, display, &f, scale, palette::text(), lit);
                report.formula_scale = Some(scale);
                y += size.y * scale + 12.0;
            }
            Err(e) => {
                ink.text(pos2(x, y), Align2::LEFT_TOP, ink.fit_right(&format!("formula: {e}"), width, 24.0, FontFamily::Proportional), 24.0, FontFamily::Proportional, palette::MISMATCH);
                y += line;
            }
        }
    }
    for (i, (name, value, color)) in readout.rows.iter().enumerate() {
        if y + line > bottom {
            report.readout_overflow = readout.rows.len() - i + notes.len();
            return;
        }
        if name.is_empty() && value.is_empty() {
            y += line * 0.5;
            continue;
        }
        let value_width = ink.measure(value, 24.0, FontFamily::Monospace).x;
        fitted_line(ink, pos2(x, y), name, (width - value_width - 16.0).max(40.0), palette::text_dim());
        ink.text(pos2(rect.max.x - 22.0, y), Align2::RIGHT_TOP, value, 24.0, FontFamily::Monospace, *color);
        y += line;
    }
    y += 6.0;
    for (i, (note, color)) in notes.iter().enumerate() {
        if y + line > bottom {
            report.readout_overflow = notes.len() - i;
            return;
        }
        fitted_line(ink, pos2(x, y), note, width, *color);
        y += line;
    }
}

/// One line of 24 px text cut to `width`, or, holding a formula, drawn whole
/// (src/rich_text.rs).
fn fitted_line(ink: &Ink, pos: Pos2, text: &str, width: f32, color: Color32) {
    if rich_text::has_tags(text) {
        rich_text::text(ink, pos, Align2::LEFT_TOP, text, 24.0, FontFamily::Proportional, color);
    } else {
        let text = ink.fit_right(text, width, 24.0, FontFamily::Proportional);
        ink.text(pos, Align2::LEFT_TOP, text, 24.0, FontFamily::Proportional, color);
    }
}

/// A Python pseudo-code listing: the engine function it follows, then the
/// code with the highlighted tags' lines lit.
fn draw_code(ink: &Ink, rect: Rect, listing: &crate::pseudocode::Listing, highlight: &[String], report: &mut UiReport) {
    use crate::pseudocode::Token;
    let painter = ink.painter;
    let x = rect.min.x + 22.0;
    let width = rect.width() - 44.0;
    let mut y = rect.min.y + 16.0;
    report.panel_code = Some(listing.name.to_string());
    ink.text(pos2(x, y), Align2::LEFT_TOP, tr("ui.python_pseudo_code"), 30.0, bold(), palette::text());
    y += 42.0;
    let title = ink.fit_right(listing.title(), width, 24.0, FontFamily::Proportional);
    ink.text(pos2(x, y), Align2::LEFT_TOP, title, 24.0, FontFamily::Proportional, palette::accent());
    y += 32.0;
    if let Some(entry) = sources::index().entries.get(&listing.source) {
        let engine = investigation::current().wording().engine_short;
        let one_line = format!("≈ {engine}: {} · {}", entry.short_symbol(), entry.file_name());
        let lines = if ink.measure(&one_line, 24.0, FontFamily::Proportional).x <= width {
            vec![one_line]
        } else {
            vec![format!("≈ {engine}: {}", entry.short_symbol()), format!("    {}:{}", entry.file_name(), entry.line)]
        };
        for text in lines {
            let text = ink.fit_right(&text, width, 24.0, FontFamily::Proportional);
            ink.text(pos2(x, y), Align2::LEFT_TOP, text, 24.0, FontFamily::Proportional, palette::text_faint());
            y += 30.0;
        }
        y += 4.0;
    }
    painter.line_segment([pos2(x, y), pos2(rect.max.x - 22.0, y)], Stroke::new(1.0, palette::border()));
    y += 12.0;
    let line = 28.0;
    let column = ink.measure("0", 24.0, FontFamily::Monospace).x;
    let any_lit = listing.lines.iter().any(|l| listing.lit(l, highlight));
    let columns = ((width / column).floor() as usize).max(8);
    for code_line in &listing.lines {
        let rows = wrap_code(&code_line.text, columns);
        let height = line * rows.len() as f32;
        if y + height > rect.max.y - 12.0 {
            report.code_overflow += 1;
            continue;
        }
        let lit = listing.lit(code_line, highlight);
        if lit {
            painter.rect_filled(Rect::from_min_size(pos2(x - 10.0, y - 2.0), vec2(width + 20.0, height)), CornerRadius::same(4), palette::highlight_row());
            painter.rect_filled(Rect::from_min_size(pos2(x - 10.0, y + 1.0), vec2(4.0, height - 6.0)), CornerRadius::same(2), palette::accent());
        }
        // Unlit lines dim while some lines are lit.
        let dim = any_lit && !lit;
        let mut col = 0usize;
        for (kind, text) in crate::pseudocode::tokens(&code_line.text) {
            let color = match kind {
                Token::Keyword => palette::code_keyword(),
                Token::Name => palette::code_name(),
                Token::Number => palette::code_number(),
                Token::Text => palette::code_string(),
                Token::Comment => palette::text_faint(),
                Token::Plain => palette::text(),
            };
            let color = if dim { color.gamma_multiply(0.55) } else { color };
            let end = col + text.chars().count();
            // The token's part on each row it reaches.
            for (row, &(start, stop, indent)) in rows.iter().enumerate() {
                let (from, to) = (col.max(start), end.min(stop));
                if from >= to {
                    continue;
                }
                let part: String = text.chars().skip(from - col).take(to - from).collect();
                if !part.trim().is_empty() {
                    let at = pos2(x + (indent + from - start) as f32 * column, y + row as f32 * line);
                    ink.text(at, Align2::LEFT_TOP, part, 24.0, FontFamily::Monospace, color);
                }
            }
            col = end;
        }
        y += height;
    }
}

/// A code line's rows within `columns`: each row's character range and the
/// column it starts at. A long line breaks after its last space or comma that
/// fits, and continues four columns deeper than its indentation.
fn wrap_code(text: &str, columns: usize) -> Vec<(usize, usize, usize)> {
    let chars: Vec<char> = text.chars().collect();
    let deeper = chars.iter().take_while(|c| **c == ' ').count() + 4;
    let mut rows = Vec::new();
    let (mut start, mut indent) = (0, 0);
    while chars.len() - start > columns - indent {
        let limit = start + (columns - indent).max(1);
        let stop = (start + 1..=limit).rev().find(|&b| matches!(chars[b - 1], ' ' | ',')).unwrap_or(limit);
        rows.push((start, stop, indent));
        start = stop;
        while start < chars.len() && chars[start] == ' ' {
            start += 1;
        }
        indent = deeper.min(columns / 2);
    }
    rows.push((start, chars.len(), indent));
    rows
}

fn draw_memory(ink: &Ink, input: &UiInput, record: &crate::frame::PanelRecord, layout: &crate::layout::ShaderStruct, rust: &[RustField], x: f32, width: f32, mut y: f32, bottom: f32) {
    let painter = ink.painter;
    let state = input.state;
    let rows = layout.size / 16;
    let cell = 15.0;
    let row_h = 28.0;
    let label_x = x + 68.0 + 16.0 * cell + 14.0;
    let pads = layout.padding();
    let is_pad = |byte: usize| pads.iter().any(|(o, s, _)| byte >= *o && byte < o + s);
    let field_at = |byte: usize| layout.fields.iter().position(|f| byte >= f.offset && byte < f.offset + f.size);
    ink.text(pos2(x, y), Align2::LEFT_TOP, tr("ui.byte_0_15"), 24.0, FontFamily::Monospace, palette::text_faint());
    y += row_h;
    for r in 0..rows {
        if y + row_h > bottom - 3.0 * row_h {
            break;
        }
        ink.text(pos2(x, y), Align2::LEFT_TOP, format!("{:>4}", r * 16), 24.0, FontFamily::Monospace, palette::text_dim());
        let mut label = String::new();
        for b in 0..16 {
            let byte = r * 16 + b;
            let cell_rect = Rect::from_min_size(pos2(x + 68.0 + b as f32 * cell, y + 3.0), vec2(cell - 2.0, row_h - 8.0));
            let color = if is_pad(byte) {
                palette::padding()
            } else if let Some(fi) = field_at(byte) {
                let f = &layout.fields[fi];
                if label.is_empty() {
                    label = match f.matrix {
                        Some(_) => format!("{} c{}", f.name, (byte - f.offset) / 16),
                        None => f.name.clone(),
                    };
                }
                if state.highlight.iter().any(|h| h == &f.name) {
                    let colors = field_colors(record.struct_name, &f.name);
                    let column = f.matrix.map(|_| (byte - f.offset) / 16).unwrap_or(0);
                    colors[column.min(colors.len() - 1)]
                } else if fi % 2 == 0 {
                    palette::theme().field_even
                } else {
                    palette::theme().field_odd
                }
            } else {
                palette::padding()
            };
            painter.rect_filled(cell_rect, CornerRadius::same(2), color);
        }
        let text = ink.fit_right(&label, x + width - label_x, 24.0, FontFamily::Monospace);
        ink.text(pos2(label_x, y), Align2::LEFT_TOP, text, 24.0, FontFamily::Monospace, palette::text_dim());
        y += row_h;
    }
    y += 8.0;
    let pad_total: usize = pads.iter().map(|p| p.1).sum();
    ink.text(pos2(x, y), Align2::LEFT_TOP, format!("{} B {} + {} B padding", layout.size - pad_total, tr("ui.data"), pad_total), 24.0, FontFamily::Proportional, palette::text());
    y += row_h + 2.0;
    for name in &state.highlight {
        if let (Some(f), Some(rf)) = (layout.field(name), rust.iter().find(|r| r.name == name.as_str())) {
            if y + row_h > bottom {
                break;
            }
            let text = match rf.kind {
                FieldKind::Mat3 => format!("{name}: {} B = {}", f.size, tr("ui.3_columns_16_b")),
                FieldKind::Mat4 => format!("{name}: {} B = {}", f.size, tr("ui.4_columns_16_b")),
                _ => format!("{name}: offset {}, {} B", f.offset, f.size),
            };
            let text = ink.fit_right(&text, width, 24.0, FontFamily::Proportional);
            ink.text(pos2(x, y), Align2::LEFT_TOP, text, 24.0, FontFamily::Proportional, field_colors(record.struct_name, name)[0]);
            y += row_h;
        }
    }
}

fn draw_source_card(ink: &Ink, input: &UiInput) {
    let l = &input.layout;
    let painter = ink.painter;
    let index = sources::index();
    let rect = l.content;
    painter.rect_filled(rect, CornerRadius::same(14), palette::panel());
    painter.rect_stroke(rect, CornerRadius::same(14), Stroke::new(1.0, palette::border()), StrokeKind::Inside);
    let x = rect.min.x + 40.0;
    let width = rect.width() - 80.0;
    let mut y = rect.min.y + 28.0;
    let heading = tr_with("ui.read_it_in", args!(engine = index.engine.name, version = index.engine.version, chapter = input.chapter_number));
    ink.text(pos2(x, y), Align2::LEFT_TOP, heading, 36.0, bold(), palette::text());
    y += 50.0;
    ink.text(pos2(x, y), Align2::LEFT_TOP, format!("{} {} · commit {}", tr("ui.paths_are_relative_to"), investigation::current().wording().checkout_dir, &index.engine.commit[..12]), 24.0, FontFamily::Proportional, palette::text_dim());
    y += 46.0;
    for key in input.beat_sources {
        let Some(entry) = index.entries.get(key) else { continue };
        if y > rect.max.y - 100.0 {
            break;
        }
        ink.text(pos2(x, y), Align2::LEFT_TOP, &entry.symbol, 28.0, mono_bold(), palette::accent());
        y += 38.0;
        let path = ink.fit_left(&format!("{}:{}", entry.path, entry.line), width, 24.0, FontFamily::Monospace);
        ink.text(pos2(x, y), Align2::LEFT_TOP, path, 24.0, FontFamily::Monospace, palette::text());
        y += 32.0;
        y = ink.wrapped(pos2(x, y), width, entry.inspect_text(), 24.0, FontFamily::Proportional, palette::text_dim()) + 22.0;
    }
}

/// The legal notices of the investigation's package (`[[notice]]`): each
/// notice, and the file of its terms, which comes with the package.
fn draw_notices_card(ink: &Ink, input: &UiInput) {
    let l = &input.layout;
    let painter = ink.painter;
    let rect = l.content;
    painter.rect_filled(rect, CornerRadius::same(14), palette::panel());
    painter.rect_stroke(rect, CornerRadius::same(14), Stroke::new(1.0, palette::border()), StrokeKind::Inside);
    let x = rect.min.x + 40.0;
    let width = rect.width() - 80.0;
    let mut y = rect.min.y + 28.0;
    ink.text(pos2(x, y), Align2::LEFT_TOP, tr("ui.notices_title"), 36.0, bold(), palette::text());
    y += 64.0;
    let package = investigation::current().package().unwrap_or("rlstd");
    for (text, license) in crate::resources::notices(package) {
        if y > rect.max.y - 90.0 {
            break;
        }
        y = ink.wrapped(pos2(x, y), width, &text, 28.0, FontFamily::Proportional, palette::text()) + 8.0;
        if let Some(file) = license.as_ref().and_then(|f| f.file_name()).map(|f| f.to_string_lossy().into_owned()) {
            ink.text(pos2(x, y), Align2::LEFT_TOP, tr_with("ui.notice_terms", args!(file = file)), 24.0, FontFamily::Proportional, palette::text_dim());
            y += 32.0;
        }
        y += 24.0;
    }
}

fn draw_captions(ink: &Ink, l: &Layout, timeline: &Timeline, frame: u64) {
    let (_, beat) = timeline.beat_at(frame);
    let Some(caption) = beat.captions.iter().find(|c| c.start <= frame && frame < c.end) else { return };
    draw_caption_lines(ink, l, &caption.text);
}

/// A caption's lines with a shadow, 48 px apart, or more where a line holds a taller formula.
fn draw_caption_lines(ink: &Ink, l: &Layout, text: &str) {
    let mut y = l.captions.min.y + 18.0;
    for line in text.lines() {
        rich_text::text(ink, pos2(l.captions.center().x + 1.5, y + 2.0), Align2::CENTER_TOP, line, 34.0, FontFamily::Proportional, palette::theme().shadow);
        let rect = rich_text::text(ink, pos2(l.captions.center().x, y), Align2::CENTER_TOP, line, 34.0, FontFamily::Proportional, palette::text());
        y += 48.0f32.max(rect.height() + 8.0);
    }
}

/// The Captions tab: the paused caption at the size it plays, and the one
/// before it, dimmer and smaller, above it for context.
pub fn draw_caption_tab(ink: &Ink, rect: Rect, previous: Option<&str>, caption: Option<&str>) {
    let painter = ink.painter.with_clip_rect(rect);
    let ink = Ink::new(&painter);
    let Some(caption) = caption else {
        ink.text(rect.center(), Align2::CENTER_CENTER, tr("ui.no_caption"), 24.0, FontFamily::Proportional, palette::text_dim());
        return;
    };
    let mut y = rect.min.y + 14.0;
    for line in previous.into_iter().flat_map(str::lines) {
        let r = rich_text::text(&ink, pos2(rect.center().x, y), Align2::CENTER_TOP, line, 24.0, FontFamily::Proportional, palette::text_dim());
        y += 34.0f32.max(r.height() + 6.0);
    }
    y += 10.0;
    for line in caption.lines() {
        let r = rich_text::text(&ink, pos2(rect.center().x, y), Align2::CENTER_TOP, line, 32.0, FontFamily::Proportional, palette::text());
        y += 44.0f32.max(r.height() + 8.0);
    }
}

fn draw_caption_text(ink: &Ink, l: &Layout, caption: Option<&str>) {
    let Some(caption) = caption else { return };
    draw_caption_lines(ink, l, caption);
}

/// How far the timeline just skipped, in a card on the viewport's left
/// (back) or right (forward) side, fading out.
fn draw_skip_notice(ink: &Ink, l: &Layout, seconds: i64, opacity: f32) {
    let text = if seconds < 0 {
        format!("◀◀  {}", tr_with("ui.skip_back", args!(seconds = -seconds)))
    } else {
        format!("{}  ▶▶", tr_with("ui.skip_forward", args!(seconds = seconds)))
    };
    let size = ink.measure(&text, 34.0, bold()) + vec2(56.0, 28.0);
    let x = if seconds < 0 { l.viewport.min.x + l.viewport.width() * 0.2 } else { l.viewport.max.x - l.viewport.width() * 0.2 };
    let card = Rect::from_center_size(pos2(x, l.viewport.center().y), size);
    ink.painter.rect_filled(card, CornerRadius::same(12), palette::panel().gamma_multiply(0.85 * opacity));
    ink.painter.rect_stroke(card, CornerRadius::same(12), Stroke::new(1.5, palette::accent().gamma_multiply(opacity)), StrokeKind::Inside);
    ink.text(card.center(), Align2::CENTER_CENTER, text, 34.0, bold(), palette::text().gamma_multiply(opacity));
}

/// The narration's volume at the timeline bar's right end: its width.
const BAR_VOLUME: f32 = 300.0;

/// The viewer's timeline: a play/pause button, the time, a bar with the
/// chapters that seeks where it is clicked or dragged, and the narration's volume.
fn draw_timeline_bar(ctx: &egui::Context, ink: &Ink, l: &Layout, viewer: &mut ViewerChrome) {
    let Some((frame, total)) = viewer.timeline else { return };
    let painter = ink.painter;
    let button = Rect::from_min_size(pos2(l.footer.min.x, l.footer.min.y + 4.0), vec2(150.0, 40.0));
    let bar = Rect::from_min_max(pos2(button.max.x + 24.0, l.footer.min.y + 18.0), pos2(l.footer.max.x - BAR_VOLUME - 24.0, l.footer.min.y + 30.0));
    let volume = Rect::from_min_max(pos2(bar.max.x + 24.0, l.footer.min.y + 4.0), pos2(l.footer.max.x, l.footer.min.y + 44.0));
    // In front of the dock, so the bar keeps its hover preview while the timeline is paused.
    // Not kept on screen: its controls are placed at the footer's rectangles, so an area
    // moved up to fit would grow over the dock every frame and take its pointer.
    let response = egui::Area::new(egui::Id::unique("timeline-bar")).order(egui::Order::Foreground).fixed_pos(l.footer.min).constrain(false).show(ctx, |ui| {
        ui.set_min_size(l.footer.size());
        let play = ui.put(button, egui::Button::new(if viewer.playing { tr("ui.bar_pause") } else { tr("ui.bar_play") }));
        if play.clicked() {
            viewer.actions.push(ViewerAction::TogglePlay);
        }
        let hit = ui.allocate_rect(bar.expand2(vec2(0.0, 10.0)), egui::Sense::click_and_drag());
        if hit.clicked() || hit.dragged() {
            if let Some(p) = hit.interact_pointer_pos() {
                viewer.actions.push(ViewerAction::Seek((p.x - bar.min.x) / bar.width()));
            }
        }
        ui.scope_builder(egui::UiBuilder::new().max_rect(volume).layout(egui::Layout::left_to_right(egui::Align::Center)), |ui| {
            let mut narration = viewer.narration;
            let r = crate::volume::slider(ui, &mut narration, 32.0, BAR_VOLUME - 150.0);
            viewer.widgets.push(("bar:narration".into(), r.rail));
            viewer.widgets.push(("bar:narration-mute".into(), r.mute));
            if r.changed || r.committed {
                viewer.actions.push(ViewerAction::SetNarration(narration, r.committed));
            }
        });
        hit.hover_pos().map(|p| ((p.x - bar.min.x) / bar.width()).clamp(0.0, 1.0))
    });
    viewer.ui_has_pointer |= response.response.hovered();
    viewer.widgets.push(("timeline-bar".into(), bar));
    painter.rect_filled(bar, CornerRadius::same(4), palette::theme().track);
    for &c in &viewer.chapters {
        let x = bar.min.x + bar.width() * c;
        painter.rect_filled(Rect::from_min_size(pos2(x, bar.min.y - 6.0), vec2(2.0, bar.height() + 12.0)), CornerRadius::ZERO, palette::text_faint());
    }
    let share = frame as f32 / total.max(1) as f32;
    let x = bar.min.x + bar.width() * share;
    painter.rect_filled(Rect::from_min_max(bar.min, pos2(x, bar.max.y)), CornerRadius::same(4), palette::accent());
    painter.circle_filled(pos2(x, bar.center().y), 10.0, palette::accent());
    if let Some(share) = response.inner {
        draw_bar_preview(ctx, bar, share, total, viewer);
    }
}

/// The section under the pointer on the timeline bar: its chapter, the
/// beat's headline, and the time, in a card above the bar, with the beat's
/// span lit on the bar.
fn draw_bar_preview(ctx: &egui::Context, bar: Rect, share: f32, total: u64, viewer: &mut ViewerChrome) {
    let Some(i) = viewer.sections.iter().rposition(|s| s.start <= share) else { return };
    let section = &viewer.sections[i];
    let end = viewer.sections.get(i + 1).map_or(1.0, |s| s.start);
    // Above every panel and the dock.
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::unique("timeline-preview")));
    let ink = Ink::new(&painter);
    let span = Rect::from_min_max(pos2(bar.min.x + bar.width() * section.start, bar.min.y - 3.0), pos2(bar.min.x + bar.width() * end, bar.max.y + 3.0));
    painter.rect_filled(span, CornerRadius::same(3), Color32::from_rgba_unmultiplied(255, 255, 255, 60));
    let x = bar.min.x + bar.width() * share;
    painter.rect_filled(Rect::from_min_size(pos2(x - 1.0, bar.min.y - 8.0), vec2(2.0, bar.height() + 16.0)), CornerRadius::ZERO, palette::text());

    let fps = viewer.fps.max(1) as u64;
    let clock = |frame: u64| {
        let s = frame / fps;
        format!("{}:{:02}", s / 60, s % 60)
    };
    let at = (share as f64 * total as f64) as u64;
    let chapter = tr_with("ui.bar_chapter", args!(chapter = section.chapter, title = section.chapter_title));
    let width = 720.0f32;
    let chapter_line = ink.fit_right(&chapter, width - 120.0, 24.0, bold());
    let headline = ink.fit_right(&section.headline, width, 26.0, bold());
    let size = vec2(width + 36.0, 108.0);
    let left = (x - size.x / 2.0).clamp(bar.min.x - 150.0, bar.max.x - size.x);
    let card = Rect::from_min_size(pos2(left, bar.min.y - 26.0 - size.y), size);
    painter.rect_filled(card, CornerRadius::same(12), palette::theme().card);
    painter.rect_stroke(card, CornerRadius::same(12), Stroke::new(1.5, palette::accent()), StrokeKind::Inside);
    ink.text(card.min + vec2(18.0, 14.0), Align2::LEFT_TOP, &chapter_line, 24.0, bold(), palette::accent());
    ink.text(pos2(card.max.x - 18.0, card.min.y + 14.0), Align2::RIGHT_TOP, clock(at), 24.0, FontFamily::Monospace, palette::text_dim());
    ink.text(card.min + vec2(18.0, 54.0), Align2::LEFT_TOP, &headline, 26.0, bold(), palette::text());
    viewer.hover = Some(format!("{chapter} | {} | {}", section.headline, clock(at)));
}

/// The engine source entries of a demonstration: symbol, path, and line.
pub fn draw_source_list(ink: &Ink, rect: Rect, demo: DemoId) {
    let index = sources::index();
    let wording = investigation::current().wording();
    let x = rect.min.x + 20.0;
    let mut y = rect.min.y + 10.0;
    let heading = tr_with("ui.source_list_heading", args!(engine = wording.engine_short, demo = demo.title(), checkout = wording.checkout_dir));
    ink.text(pos2(x, y), Align2::LEFT_TOP, heading, 24.0, bold(), palette::accent());
    y += 34.0;
    let keys = index.demos.get(demo.key()).cloned().unwrap_or_default();
    for key in keys.iter().take(4) {
        let entry = index.entry(key);
        let symbol_width = 520.0;
        let symbol = ink.fit_right(&entry.short_symbol(), symbol_width - 16.0, 24.0, mono_bold());
        ink.text(pos2(x, y), Align2::LEFT_TOP, symbol, 24.0, mono_bold(), palette::text());
        let path = ink.fit_left(&format!("{}:{}", entry.path, entry.line), rect.width() - symbol_width - 40.0, 24.0, FontFamily::Monospace);
        ink.text(pos2(x + symbol_width, y), Align2::LEFT_TOP, path, 24.0, FontFamily::Monospace, palette::text_dim());
        y += 29.0;
    }
}

fn draw_footer(ink: &Ink, l: &Layout, timeline: Option<(&&Timeline, u64)>) {
    let painter = ink.painter;
    let bar = Rect::from_min_size(pos2(l.footer.min.x, l.footer.min.y + 2.0), vec2(l.footer.width(), 4.0));
    painter.rect_filled(bar, CornerRadius::same(2), palette::theme().track);
    if let Some((timeline, frame)) = timeline {
        let total = timeline.total_frames.max(1) as f32;
        for c in &timeline.chapters {
            let x = bar.min.x + bar.width() * c.start_frame as f32 / total;
            painter.rect_filled(Rect::from_min_size(pos2(x, bar.min.y - 5.0), vec2(2.0, 14.0)), CornerRadius::ZERO, palette::text_faint());
        }
        let x = bar.min.x + bar.width() * frame as f32 / total;
        painter.rect_filled(Rect::from_min_max(bar.min, pos2(x, bar.max.y)), CornerRadius::same(2), palette::accent());
        painter.circle_filled(pos2(x, bar.center().y), 7.0, palette::accent());
    }
    ink.text(pos2(l.footer.min.x, l.footer.min.y + 16.0), Align2::LEFT_TOP, investigation::current().footer(), 24.0, FontFamily::Proportional, palette::text_faint());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dragged_sizes_apply_within_limits() {
        let base = Layout::new(1920, 1080).with_dock();
        // No dragged sizes: the default layout.
        let same = base.with_user(&ViewerLayout::default(), true, false);
        assert_eq!((same.panel, same.viewport, same.captions), (base.panel, base.viewport, base.captions));
        // A wider panel and a taller dock; the viewport gives up the room.
        let user = ViewerLayout { panel_width: Some(800), code_panel_width: None, dock_height: Some(330) };
        let l = base.with_user(&user, true, false);
        assert_eq!(l.panel.width(), 800.0);
        assert_eq!(l.viewport.max.x, l.panel.min.x - 24.0);
        assert_eq!(l.captions.height(), 330.0);
        assert_eq!((l.viewport.max.y, l.panel.max.y), (l.captions.min.y - 12.0, l.captions.min.y - 12.0));
        // Too small or too large: kept to the limits.
        let extreme = base.with_user(&ViewerLayout { panel_width: Some(1), code_panel_width: None, dock_height: Some(100_000) }, true, false);
        assert_eq!(extreme.panel.width(), MIN_PANEL);
        assert!(extreme.viewport.height() >= MIN_VIEWPORT.y - 0.01);
        // The code panel has its own width, its default the wider one.
        assert_eq!(base.with_user(&user, true, true).panel.width(), CODE_PANEL_WIDTH);
        // Without the dock (a playing timeline), its height is not applied.
        assert_eq!(base.with_user(&user, false, false).captions, base.captions);
    }
}
