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

mod viewport;
mod graph_panel;
mod knowledge_panel;
mod panel;
mod code;
mod timeline_bar;
pub mod windows;
mod window_frame;
mod scene_window;
use scene_window::*;
use viewport::*;
use window_frame::*;
pub use graph_panel::*;
pub use knowledge_panel::*;
pub use panel::*;
pub use code::*;
pub use timeline_bar::*;

/// A recording's layout (`Layout::for_video`): the margin of its panel card and 2D page, and the bands its
/// tag (above) and its subtitles and watermark (below) keep over the viewport, in points.
pub const VIDEO_MARGIN: f32 = 24.0;
pub const VIDEO_TOP: f32 = 76.0;
pub const VIDEO_BOTTOM: f32 = 132.0;

/// The least a dragged panel, dock, and viewport keep, in points.
pub const MIN_PANEL: f32 = 320.0;
pub const MIN_DOCK: f32 = 150.0;
pub const MIN_VIEWPORT: Vec2 = vec2(480.0, 300.0);

/// What a window's title bar, edge, chevron, or the Windows menu does to it (`ViewerAction::Window`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WindowChange {
    /// Moved by this many points (a docked one floats from where it is).
    Move(Vec2),
    /// Its title bar let go: docked to this edge, or (`Dock::Float`) left floating.
    Drop(windows::Dock),
    /// Its edge or corner dragged to here (canvas points): a docked one's inner edge, a floating one's corner.
    Resize(Pos2),
    Collapse(bool),
    Open(bool),
    /// Back to its default place.
    Default,
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
    /// The panel is drawn; when not, the viewport has its room (`for_panel`).
    pub panel_shown: bool,
    /// The interactive viewer's windows (src/ui/windows.rs), by `WindowId::index`; none in a recording.
    pub windows: [Option<windows::WindowFrame>; 3],
    /// The room the windows dock in, between the header and the footer.
    pub area: Rect,
    /// The places and the windows the viewer laid out (`with_windows`), for `for_panel`.
    pub windowed: Option<(ViewerLayout, windows::Shown)>,
    /// A recording's layout (`for_video`): the viewport to the frame's edges, the tag, the subtitles, and the
    /// watermark over it.
    pub video: bool,
}

impl Layout {
    pub fn new(width: u32, height: u32) -> Layout {
        Layout::with_ui_scale(width, height, 1.0)
    }

    /// The layout with everything drawn `ui_scale` times its size: the canvas
    /// grows by 1 / `ui_scale`, so text, panels, and labels shrink together.
    /// The recorder uses 1; the viewer may use less, for smaller text.
    pub fn with_ui_scale(width: u32, height: u32, ui_scale: f32) -> Layout {
        let (scale, canvas) = crate::text::canvas_for(width, height, ui_scale);
        let margin = 40.0;
        let content_top = 112.0;
        let content_bottom = canvas.y - 240.0;
        let panel_width = 590.0;
        let panel = Rect::from_min_max(pos2(canvas.x - margin - panel_width, content_top), pos2(canvas.x - margin, content_bottom));
        let viewport = Rect::from_min_max(pos2(margin, content_top), pos2(panel.min.x - 24.0, content_bottom));
        let content = Rect::from_min_max(viewport.min, panel.max);
        let captions = Rect::from_min_max(pos2(margin, canvas.y - 228.0), pos2(canvas.x - margin, canvas.y - 66.0));
        let footer = Rect::from_min_max(pos2(margin, canvas.y - 58.0), pos2(canvas.x - margin, canvas.y - 8.0));
        Layout { scale, canvas, viewport, panel, content, captions, footer, panel_shown: true, windows: [None; 3], area: content, windowed: None, video: false }
    }

    /// A recording's layout: the viewport reaches the frame's edges (beside the panel, a card at the right
    /// edge, when the panel shows), and what the video says lies over it: a small tag at the top (the
    /// chapter and the beat), the caption as a one-line subtitle near the bottom, the engine's credit as a
    /// watermark in the lower left, and the progress as a thin line along the bottom edge.
    pub fn for_video(mut self) -> Layout {
        let c = self.canvas;
        self.video = true;
        self.panel = Rect::from_min_max(pos2(c.x - VIDEO_MARGIN - 590.0, VIDEO_MARGIN), pos2(c.x - VIDEO_MARGIN, c.y - VIDEO_MARGIN));
        self.viewport = Rect::from_min_max(Pos2::ZERO, pos2(self.panel.min.x - VIDEO_MARGIN, c.y));
        // A 2D page: the frame between the tag and the subtitles.
        self.content = Rect::from_min_max(pos2(VIDEO_MARGIN, VIDEO_TOP), pos2(c.x - VIDEO_MARGIN, c.y - VIDEO_BOTTOM));
        self.footer = Rect::from_min_max(pos2(0.0, c.y - 4.0), c.to_pos2());
        self.video_captions()
    }

    /// The subtitles' band: under the viewport's picture (the frame's, on a 2D page).
    fn video_captions(mut self) -> Layout {
        let c = self.canvas;
        let (left, right) = if self.panel_shown { (self.viewport.min.x, self.viewport.max.x) } else { (0.0, c.x) };
        self.captions = Rect::from_min_max(pos2(left, c.y - VIDEO_BOTTOM), pos2(right, c.y - 52.0));
        self
    }

    /// Where the viewport's own cards go (insets, the callout, the formula card): in a recording, clear of the
    /// tag above and the subtitles and the watermark below.
    pub fn safe(&self) -> Rect {
        if self.video { Rect::from_min_max(self.viewport.min + vec2(0.0, VIDEO_TOP - 16.0), self.viewport.max - vec2(0.0, VIDEO_BOTTOM - 8.0)) } else { self.viewport }
    }

    /// The interactive viewer's layout: its windows (src/ui/windows.rs) where the user left them, the
    /// viewport in what the docked ones leave, the panel the Panel window's body and the captions band the
    /// Controls window's. Without the Controls window (a playing timeline) the captions keep their band
    /// below, as in a recording.
    pub fn with_windows(self, user: &ViewerLayout, shown: windows::Shown) -> Layout {
        let bottom = if shown.controls { self.footer.min.y - 8.0 } else { self.content.max.y };
        let area = Rect::from_min_max(self.content.min, pos2(self.content.max.x, bottom));
        self.place_windows(area, user, shown)
    }

    /// The windows placed in `area` (`with_windows`).
    fn place_windows(mut self, area: Rect, user: &ViewerLayout, shown: windows::Shown) -> Layout {
        use windows::WindowId;
        let (frames, viewport) = windows::arrange(area, &user.windows, shown);
        self.area = area;
        self.windows = frames;
        self.windowed = Some((*user, shown));
        self.viewport = viewport;
        let panel = frames[WindowId::Panel.index()];
        self.panel_shown = panel.is_some_and(|f| !f.collapsed);
        self.panel = panel.map_or(Rect::from_min_size(viewport.right_top(), Vec2::ZERO), |f| f.body);
        // A 2D page takes the viewport and a panel docked beside it.
        self.content = match panel {
            Some(f) if f.dock == windows::Dock::Right && !f.collapsed => viewport.union(f.frame),
            _ => viewport,
        };
        if let Some(controls) = frames[WindowId::Controls.index()] {
            self.captions = controls.body;
        }
        self
    }

    /// The places of the viewer's windows, as the user left them (none in a recording).
    pub fn user_windows(&self) -> Option<&ViewerLayout> {
        self.windowed.as_ref().map(|(user, _)| user)
    }

    /// The viewer's window `id` as this layout draws it.
    pub fn window(&self, id: windows::WindowId) -> Option<windows::WindowFrame> {
        self.windows[id.index()]
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
        if self.video {
            return self.video_captions();
        }
        self
    }

    /// The same layout with the 3D viewport widened over the panel's area
    /// when the struct panel is hidden, or has nothing to show.
    pub fn for_panel(mut self, panel_visible: bool) -> Layout {
        if let (false, Some((user, shown))) = (panel_visible, self.windowed) {
            return self.place_windows(self.area, &user, windows::Shown { panel: false, ..shown });
        }
        if !panel_visible {
            self.viewport = if self.video { Rect::from_min_size(Pos2::ZERO, self.canvas) } else { self.content };
            self.panel_shown = false;
            if self.video {
                return self.video_captions();
            }
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
    /// Move the timeline to this share of its length.
    Seek(f32),
    /// Close the viewer and return to the homepage.
    Home,
    /// The narration's volume; `true` when the change is finished (src/volume.rs).
    SetNarration(crate::volume::Volume, bool),
    /// The timeline's and the narration's speed (one of `SPEEDS`).
    SetSpeed(f32),
    /// A window moved, docked, resized, collapsed, opened or closed (src/ui/windows.rs).
    Window(windows::WindowId, WindowChange),
    /// A window's drag ended: the layout is written to the settings.
    SaveLayout,
    /// The whole layout back to its defaults (and written).
    ResetLayout,
    /// Look through the free camera, or the main camera again (src/camera.rs `FreeCamera`).
    ToggleFreeCamera,
    /// Select an entity in the Scene window (src/entity.rs `Entity::id`), or nothing.
    Select(Option<String>),
    /// What the selected entity's gizmo does (src/gizmo.rs).
    SetGizmo(crate::gizmo::Mode),
}

/// The selected entity's gizmo, as the UI draws it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GizmoView {
    pub mode: crate::gizmo::Mode,
    /// The handle under the pointer, and the one being dragged.
    pub hover: Option<crate::gizmo::Handle>,
    pub active: Option<crate::gizmo::Handle>,
}

/// What the viewer's dock shows: a tab, by its feature's id (src/feature/).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DockTab(pub &'static str);

impl Default for DockTab {
    /// The Parameters tab, which the viewer opens on.
    fn default() -> DockTab {
        crate::feature::parameters::TAB
    }
}

/// The knowledge graph's context (src/knowledge.rs): the chapters the
/// Prerequisites tab and the knowledge panel are about (none, or none the graph
/// declares: the whole investigation), and a topic to focus once (a snapshot's).
#[derive(Clone, Debug, Default)]
pub struct KnowledgeContext {
    pub chapters: Vec<u32>,
    pub focus: Option<String>,
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

/// A paused timeline's diagram page in the viewer: its beat (the view is
/// set up afresh for another), and a first view (src/diagram.rs `Paused`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DiagramPage {
    pub key: u64,
    pub preset: Option<(f32, Vec2)>,
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
    /// The caption on screen while the timeline plays.
    pub caption: Option<String>,
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
    /// The timeline's speed, which the dock and the timeline bar set.
    pub speed: f32,
    /// Paused on a diagram beat: the diagram panel's zoom and pan.
    pub diagram: Option<DiagramPage>,
    /// The viewer's features (src/feature/), lent for the frame: the current tab's draws its dock and its
    /// panel, each its story panels and what it shows full screen.
    pub features: crate::feature::Features,
    /// The last live reload of the shaders: what it says, and whether it failed (src/resources.rs `reload_changed`).
    pub shader_reload: Option<(String, bool)>,
    /// The focused view's place, when several views are on screen (src/view.rs): drawn with a lit border.
    pub focus_rect: Option<Rect>,
    /// The entity selected in the Scene window (src/entity.rs `Entity::id`), and its gizmo.
    pub selected: Option<String>,
    pub gizmo: GizmoView,
}

impl ViewerChrome {
    /// The tab is a feature's that draws its own panel (`FeatureLayout::panel`): its layout.
    pub fn feature_panel(&self) -> Option<crate::feature::FeatureLayout> {
        self.features.feature(self.tab.0).map(|f| f.layout()).filter(|l| l.panel)
    }
}

pub enum Chrome<'a> {
    Recording { timeline: &'a Timeline, frame: u64 },
    Viewer(&'a mut ViewerChrome),
}

pub struct UiInput<'a> {
    pub layout: Layout,
    pub state: &'a ShotState,
    /// The focused view's state when it is not the main view (src/view.rs): the dock's.
    pub focus: Option<&'a ShotState>,
    pub content: Option<&'a FrameContent>,
    pub statics: &'a Statics,
    /// The GPU textures an inset's picture can show (packages/render_lab/src/inset.rs).
    pub textures: &'a crate::utility::inset::Textures,
    pub scene_type: SceneType,
    /// The diagram a 2D scene shows (src/diagram.rs).
    pub diagram: Option<crate::diagram::BeatDiagram<'a>>,
    /// The frame graph scene's lit boxes' pictures (src/renderer/ captures them).
    pub diagram_pictures: &'a [crate::diagram::Picture],
    pub beat_sources: &'a [String],
    /// A 2D scene of the legal notices, not the sources card.
    pub notices: bool,
    pub chapter_number: u32,
    pub fade: f32,
    pub stamp: Option<u64>,
    /// The views beside the main one (`Investigation::panes`), left to right:
    /// each drawn with its own border, title, and labels.
    pub panes: &'a [PaneView<'a>],
    /// The views stacked over the main one (src/view.rs), their labels drawn over it.
    pub overs: &'a [PaneView<'a>],
    /// The main view's title, when the scene area is split.
    pub title: Option<&'a str>,
    /// While the free camera is on (src/free_view.rs): where the main
    /// camera's own view is drawn, a corner of the viewport (`content` is
    /// then the free view's).
    pub main_view: Option<egui::Rect>,
    /// An experiment's comparison drawn with the live view (src/experiment.rs).
    pub compare: Option<CompareOverlay>,
    /// What the performance panel shows (`PanelMode::Performance`).
    pub performance: Option<&'a PerformanceView>,
    /// What the graph panel shows (`PanelMode::Graph`).
    pub graph: Option<&'a GraphView>,
}

/// A view beside the main one, as the UI draws it.
#[derive(Clone, Copy)]
pub struct PaneView<'a> {
    pub rect: Rect,
    pub content: &'a FrameContent,
    pub title: Option<&'a str>,
}

/// The frame's UI. `recording`: the features a recording's story panels are drawn by (src/feature/; the
/// viewer's are its chrome's).
pub fn draw(ctx: &egui::Context, input: &UiInput, mut chrome: Chrome, recording: &mut crate::feature::Features) -> UiReport {
    let painter = ctx.layer_painter(egui::LayerId::background());
    let ink = Ink::new(&painter);
    let l = &input.layout;
    let mut report = UiReport::default();

    draw_header(&ink, input, &mut chrome, ctx);
    let scene_beat = input.scene_type == SceneType::ThreeD;
    // A feature's tab may put its own panel in the panel's place (the Experiments tab its tools, the Quiz tab its
    // question); one that yields gives way to the panels that show the frame, when asked for.
    let frame_panel = input.state.panel.analysis();
    let feature_panel = scene_beat && matches!(&chrome, Chrome::Viewer(v) if v.feature_panel().is_some_and(|l| !(l.yields && frame_panel)) && !(v.timeline_mode && v.playing));
    match input.scene_type {
        SceneType::ThreeD => {
            // A recording's picture reaches the frame's edges: no border.
            if !l.video {
                painter.rect_stroke(l.viewport.expand(1.0), CornerRadius::same(10), Stroke::new(1.5, palette::border()), StrokeKind::Outside);
            }
            // The other views: each its border, its title, and its labels.
            for pane in input.panes {
                painter.rect_stroke(pane.rect.expand(1.0), CornerRadius::same(10), Stroke::new(1.5, palette::border()), StrokeKind::Outside);
                let at = UiInput { layout: Layout { viewport: pane.rect, ..*l }, content: Some(pane.content), ..*input };
                let title = pane.title.map(|t| draw_pane_title(&ink, pane.rect, t, &[])).into_iter().collect::<Vec<_>>();
                draw_labels(&ink, &at, pane.content, &title, &mut report);
            }
            // The views stacked over the main one: their labels (their pictures are in the frame).
            for over in input.overs {
                let at = UiInput { layout: Layout { viewport: over.rect, ..*l }, content: Some(over.content), ..*input };
                draw_labels(&ink, &at, over.content, &[], &mut report);
            }
            // The focused view (the dock's), when there are several.
            if let Chrome::Viewer(v) = &chrome && let Some(r) = v.focus_rect {
                painter.rect_stroke(r.expand(2.0), CornerRadius::same(10), Stroke::new(2.5, palette::accent()), StrokeKind::Outside);
            }
            if let Some(content) = input.content {
                // A view stacked over the main one (src/view.rs) covers it: the main view's tags (its insets, its
                // callout, its labels) belong to it, so they are not drawn over the other view's picture.
                let covered = !input.overs.is_empty();
                // Insets and the callout first; labels then avoid them, and a recording's tag and subtitles.
                let mut obstacles = Vec::new();
                if l.video {
                    obstacles.push(Rect::from_min_max(l.viewport.min, pos2(l.viewport.max.x, l.safe().min.y)));
                    obstacles.push(Rect::from_min_max(pos2(l.viewport.min.x, l.safe().max.y), l.viewport.max));
                }
                // The investigation's insets, then the image the story shows.
                let package = investigation::current().package().unwrap_or("render_lab");
                let story: Vec<_> = input.state.images.iter().filter_map(|image| crate::utility::inset::story(image, &content.images, package)).collect();
                // Insets sharing a corner stack in a row away from it, and in a
                // new row once the next would leave the viewport.
                let mut rows: Vec<(crate::utility::inset::Corner, egui::Vec2, f32)> = Vec::new();
                for inset in content.insets.iter().chain(&story).filter(|i| i.alpha > 0.0 && !covered) {
                    let size = crate::utility::inset::frame_size(ctx, &ink, l, input.textures, inset);
                    let offset = match rows.iter_mut().find(|(corner, _, _)| *corner == inset.corner) {
                        None => {
                            rows.push((inset.corner, vec2(size.x + 12.0, 0.0), size.y));
                            Vec2::ZERO
                        }
                        Some((_, at, tallest)) => {
                            if at.x + size.x > l.safe().width() - 32.0 {
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
                // The main camera's own view, framed and named.
                if let Some(rect) = input.main_view {
                    painter.rect_stroke(rect.expand(1.0), CornerRadius::same(6), Stroke::new(2.0, palette::VIEW), StrokeKind::Outside);
                    obstacles.push(rect);
                    obstacles.push(draw_pane_title(&ink, rect, tr("ui-main_view"), &[]));
                }
                if let Some(compare) = input.compare {
                    let insets = obstacles.clone();
                    obstacles.extend(draw_compare(&ink, l.viewport, compare, &insets));
                }
                if let (Some(key), false) = (&input.state.callout, covered) {
                    let insets = obstacles.clone();
                    obstacles.extend(draw_callout(&ink, l, key, &insets));
                }
                if let Some(tag) = &input.state.formula {
                    let insets = obstacles.clone();
                    obstacles.extend(draw_formula_card(&ink, l, tag, input.state.formula_term, input.state.formula_lit, &insets));
                }
                if !covered {
                    draw_labels(&ink, input, content, &obstacles, &mut report);
                }
                // The selected entity's gizmo (src/gizmo.rs), over the labels, under the windows.
                if let Chrome::Viewer(v) = &chrome {
                    draw_gizmo(&ink, input, content, v);
                }
                if !feature_panel {
                    draw_window_card(&ink, l, windows::WindowId::Panel);
                    // A story panel a feature draws (src/feature/: the knowledge graph, the frame graph, the
                    // code, the performance), else the demonstration's.
                    let interactive = matches!(&chrome, Chrome::Viewer(_));
                    let shader_error = match &chrome {
                        Chrome::Viewer(v) => v.shader_reload.clone().filter(|r| r.1).map(|r| r.0),
                        Chrome::Recording { .. } => None,
                    };
                    let mut panel = crate::feature::StoryPanel { ctx, ink: &ink, input, interactive, shader_error: shader_error.as_deref(), report: &mut report, actions: Vec::new(), pointer: false };
                    let features = match &mut chrome {
                        Chrome::Viewer(v) => &mut v.features,
                        Chrome::Recording { .. } => &mut *recording,
                    };
                    match features.panel_mut(input.state.panel) {
                        Some(owner) => owner.story_panel(&mut panel),
                        None => draw_panel(&ink, input, content, panel.report),
                    }
                    let (actions, pointer) = (panel.actions, panel.pointer);
                    if let Chrome::Viewer(v) = &mut chrome {
                        v.actions.extend(actions);
                        v.ui_has_pointer |= pointer;
                    }
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
                let page = match &chrome {
                    Chrome::Viewer(v) => v.diagram,
                    Chrome::Recording { .. } => None,
                };
                match page {
                    None => {
                        crate::diagram::draw(&ink, area, None, diagram, input.diagram_pictures);
                    }
                    Some(page) => {
                        // The diagram panel of a paused timeline: zoomed, panned, its boxes moved, clipped to the panel.
                        let paused = crate::diagram::Paused { ctx, key: page.key, preset: page.preset };
                        let zoom = crate::diagram::draw(&ink, area, Some(paused), diagram, input.diagram_pictures);
                        if let Chrome::Viewer(v) = &mut chrome {
                            v.ui_has_pointer |= ctx.pointer_hover_pos().is_some_and(|p| area.contains(p));
                        }
                        let hint = tr_with("viewer-diagram_hint", args!(zoom = format!("{zoom:.1}")));
                        let at = pos2(area.min.x + 14.0, area.max.y - 12.0);
                        let galley = painter.layout_no_wrap(hint.clone(), egui::FontId::proportional(24.0), palette::text_dim());
                        painter.rect_filled(Rect::from_min_size(at - vec2(8.0, galley.size().y + 4.0), galley.size() + vec2(16.0, 8.0)), CornerRadius::same(6), Color32::from_rgba_unmultiplied(7, 18, 31, 220));
                        ink.text(at, Align2::LEFT_BOTTOM, hint, 24.0, FontFamily::Proportional, palette::text_dim());
                        // Beside the hint: put the moved boxes back (and the view).
                        let label = tr("graph-reset");
                        let size = vec2(ink.measure(label, 24.0, FontFamily::Proportional).x + 32.0, 40.0);
                        egui::Area::new(egui::Id::unique("paused-diagram-reset")).fixed_pos(pos2(area.min.x + 14.0 + galley.size().x + 24.0, area.max.y - 12.0 - size.y)).fade_in(false).show(ctx, |ui| {
                            if ui.add(egui::Button::new(egui::RichText::new(label).size(24.0)).min_size(size)).clicked() {
                                crate::graph_view::ask_reset(ctx, crate::diagram::paused_id());
                            }
                        });
                    }
                }
            }
        }
    }
    if input.fade > 0.0 {
        let a = (input.fade.clamp(0.0, 1.0) * 255.0) as u8;
        let bg = palette::background();
        let faded = if l.video { Rect::from_min_size(Pos2::ZERO, l.canvas) } else { l.content.expand(4.0) };
        painter.rect_filled(faded, CornerRadius::ZERO, Color32::from_rgba_unmultiplied(bg.r(), bg.g(), bg.b(), a));
    }
    match &mut chrome {
        Chrome::Recording { timeline, frame } => {
            draw_video_tag(&ink, l, timeline, *frame);
            draw_captions(&ink, l, timeline, *frame);
            draw_video_foot(&ink, l, timeline, *frame);
        }
        Chrome::Viewer(viewer) => {
            if viewer.timeline_mode && viewer.playing {
                draw_caption_text(&ink, l, viewer.caption.as_deref());
            } else {
                draw_window_card(&ink, l, windows::WindowId::Controls);
                crate::controls::draw_dock(ctx, &ink, input, viewer);
                if feature_panel && l.panel_shown {
                    draw_window_card(&ink, l, windows::WindowId::Panel);
                    let mut dock = crate::controls::Dock::new(input, viewer);
                    if let Some(state) = viewer.features.state_mut(viewer.tab.0) {
                        state.panel(&mut crate::feature::PanelUi { ctx, ink: &ink, input, dock: &mut dock });
                    }
                    viewer.actions.extend(dock.actions);
                    viewer.widgets.extend(dock.widgets);
                }
            }
            if !(viewer.timeline_mode && viewer.playing) && input.scene_type == SceneType::ThreeD {
                draw_scene_window(ctx, &ink, input, viewer);
            }
            draw_window_titles(ctx, &ink, input, viewer);
            if let Some((seconds, opacity)) = viewer.skip {
                draw_skip_notice(&ink, l, seconds, opacity);
            }
            if let Some((text, failed)) = &viewer.shader_reload {
                draw_reload_notice(&ink, l, text, *failed);
            }
            if viewer.timeline_mode {
                draw_timeline_bar(ctx, &ink, l, viewer);
            } else {
                draw_footer(&ink, l, None);
            }
            // What the features show full screen (the knowledge graph, the DAG), over everything but the popups.
            let shader_error = viewer.shader_reload.clone().filter(|r| r.1).map(|r| r.0);
            let mut overlay = crate::feature::StoryPanel { ctx, ink: &ink, input, interactive: true, shader_error: shader_error.as_deref(), report: &mut report, actions: Vec::new(), pointer: false };
            for state in viewer.features.states_mut() {
                state.overlay(&mut overlay);
            }
            viewer.actions.extend(overlay.actions);
            viewer.ui_has_pointer |= overlay.pointer;
            viewer.popups.draw(ctx, l.canvas);
            viewer.ui_has_pointer |= ctx.egui_wants_pointer_input();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_lay_out_the_viewer() {
        use windows::{Dock, Shown, WindowId};
        let base = Layout::new(1920, 1080);
        let all = Shown { scene: false, controls: true, panel: true, wide: false };
        // The default: the controls docked below, the panel on the right, the viewport between.
        let l = base.with_windows(&ViewerLayout::default(), all);
        let (panel, controls) = (l.window(WindowId::Panel).unwrap(), l.window(WindowId::Controls).unwrap());
        assert_eq!((l.panel, l.captions), (panel.body, controls.body));
        assert_eq!(l.viewport.max.x, panel.frame.min.x - windows::GAP);
        assert_eq!(l.viewport.max.y, controls.frame.min.y - windows::GAP);
        assert!(l.panel_shown && l.content.contains_rect(panel.frame) && !l.content.intersects(controls.frame));
        // Without the panel (nothing to show), the viewport takes its room; a floating panel takes none.
        let hidden = l.for_panel(false);
        assert!(!hidden.panel_shown && hidden.window(WindowId::Panel).is_none() && hidden.viewport.max.x == l.area.max.x);
        let mut user = ViewerLayout::default();
        user.window_mut(WindowId::Panel).dock = Dock::Float;
        let floating = base.with_windows(&user, all);
        assert_eq!(floating.viewport.max.x, floating.area.max.x);
        assert!(floating.viewport.contains_rect(floating.window(WindowId::Panel).unwrap().frame));
        // A playing timeline: no controls; the captions keep their band, as in a recording.
        let playing = base.with_windows(&ViewerLayout::default(), Shown { controls: false, ..all });
        assert_eq!(playing.captions, base.captions);
        assert!(playing.viewport.max.y <= base.captions.min.y);
    }
}
