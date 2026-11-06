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
use viewport::*;
pub use graph_panel::*;
pub use knowledge_panel::*;
pub use panel::*;
pub use code::*;
pub use timeline_bar::*;

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
    /// The panel is drawn; when not, the viewport has its room (`for_panel`).
    pub panel_shown: bool,
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
        Layout { scale, canvas, viewport, panel, content, captions, footer, panel_shown: true }
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
    /// when the struct panel is hidden, or has nothing to show.
    pub fn for_panel(mut self, panel_visible: bool) -> Layout {
        if !panel_visible {
            self.viewport = self.content;
            self.panel_shown = false;
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
    /// Show a pseudo-code listing (the Code tab).
    SetCode(&'static str),
    /// Move the timeline to this share of its length.
    Seek(f32),
    /// Close the viewer and return to the homepage.
    Home,
    /// The narration's volume; `true` when the change is finished (src/volume.rs).
    SetNarration(crate::volume::Volume, bool),
    /// The timeline's and the narration's speed (one of `SPEEDS`).
    SetSpeed(f32),
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
    /// Look through the free camera, or the main camera again (src/camera.rs `FreeCamera`).
    ToggleFreeCamera,
    /// Open or close the knowledge graph full screen (src/knowledge.rs).
    SetKnowledgeOverlay(bool),
    /// Open or close the frame graph's DAG full screen.
    SetDagOverlay(bool),
    /// Show (or hide) the selected pass's WGSL beside the full-screen DAG.
    ShowSource(bool),
    /// Show a resource of the frame's graph as it stood after a node
    /// (`<node>.<resource>`, the inspector), or none.
    Inspect(Option<String>),
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
    /// What the audience should know before the chapter, from the knowledge
    /// graph (src/knowledge.rs); the panel shows the graph.
    Prerequisites,
    /// The frame graph (src/graph/) in the panel (its DAG, grid, or
    /// resources); the dock describes the hovered or selected pass.
    Graph,
}

/// The knowledge graph's context in the viewer (src/knowledge.rs): the chapters the
/// Prerequisites tab and the knowledge panel are about (none, or none the graph
/// declares: the whole investigation), and a topic to focus once (a snapshot's).
#[derive(Clone, Debug, Default)]
pub struct KnowledgeChrome {
    pub chapters: Vec<u32>,
    pub focus: Option<String>,
    /// The knowledge graph full screen, over the viewer.
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
    /// The timeline's speed, which the dock and the timeline bar set.
    pub speed: f32,
    /// Paused on a diagram beat: the diagram panel's zoom and pan.
    pub diagram: Option<DiagramPage>,
    /// The Experiments tab's view of the experiment.
    pub experiment: ExperimentView,
    /// The Prerequisites tab's and the knowledge panel's chapters.
    pub knowledge: KnowledgeChrome,
    /// The frame graph's DAG is shown full screen (`ViewerAction::SetDagOverlay`).
    pub dag_overlay: bool,
    /// The full-screen DAG shares the screen with the selected pass's WGSL (`ViewerAction::ShowSource`).
    pub graph_source: bool,
    /// The last live reload of the shaders: what it says, and whether it failed (src/resources.rs `reload_changed`).
    pub shader_reload: Option<(String, bool)>,
    /// The focused view's place, when several views are on screen (src/view.rs): drawn with a lit border.
    pub focus_rect: Option<Rect>,
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

pub fn draw(ctx: &egui::Context, input: &UiInput, mut chrome: Chrome) -> UiReport {
    let painter = ctx.layer_painter(egui::LayerId::background());
    let ink = Ink::new(&painter);
    let l = &input.layout;
    let mut report = UiReport::default();

    draw_header(&ink, input, &mut chrome, ctx);
    let scene_beat = input.scene_type == SceneType::ThreeD;
    // The Experiments tab puts its values in the panel's place.
    // The performance panel, when asked for, takes the panel's place over the values.
    let value_panel = scene_beat && !matches!(input.state.panel, PanelMode::Performance | PanelMode::Knowledge | PanelMode::Graph | PanelMode::Dag | PanelMode::Resources) && matches!(&chrome, Chrome::Viewer(v) if v.tab == DockTab::Experiments && !(v.timeline_mode && v.playing));
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
                // Insets and the callout first; labels then avoid them.
                let mut obstacles = Vec::new();
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
                if input.state.panel == PanelMode::Knowledge {
                    let topics = match &chrome {
                        Chrome::Viewer(v) => v.knowledge.clone(),
                        Chrome::Recording { .. } => KnowledgeChrome::default(),
                    };
                    let pressed = draw_knowledge_panel(ctx, &ink, l.panel, &topics);
                    if let Chrome::Viewer(v) = &mut chrome {
                        if pressed {
                            v.actions.push(ViewerAction::SetKnowledgeOverlay(true));
                        }
                        // Its input area is in the background layer, which egui does not
                        // count as over the UI: the wheel zooms the graph, not the camera.
                        v.ui_has_pointer |= ctx.pointer_hover_pos().is_some_and(|p| l.panel.contains(p));
                    }
                } else if matches!(input.state.panel, PanelMode::Graph | PanelMode::Dag | PanelMode::Resources) && l.panel_shown {
                    let view = input.graph.cloned().unwrap_or_default();
                    let interactive = matches!(&chrome, Chrome::Viewer(_));
                    let panel = GraphPanel { ctx, ink: &ink, rect: l.panel, view: &view, highlight: &input.state.highlight, full: false, interactive, source: None };
                    let pick = match input.state.panel {
                        PanelMode::Graph => draw_graph_panel(&panel),
                        PanelMode::Dag => draw_dag_panel(&panel),
                        _ => draw_resource_panel(&panel),
                    };
                    if let Chrome::Viewer(v) = &mut chrome {
                        // Its input is read by hand in the background layer: the wheel scrolls it, not the camera.
                        v.ui_has_pointer |= ctx.pointer_hover_pos().is_some_and(|p| l.panel.contains(p));
                        if let Some(pick) = pick {
                            v.actions.push(graph_action(pick, &view));
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
            if let Some((text, failed)) = &viewer.shader_reload {
                draw_reload_notice(&ink, l, text, *failed);
            }
            if viewer.timeline_mode {
                draw_timeline_bar(ctx, &ink, l, viewer);
            } else {
                draw_footer(&ink, l, None);
            }
            // The knowledge graph full screen, over everything but the popups.
            // The DAG full screen, over everything (Esc closes it).
            if viewer.dag_overlay && matches!(input.state.panel, PanelMode::Dag | PanelMode::Resources) && scene_beat {
                let view = input.graph.cloned().unwrap_or_default();
                let canvas = Rect::from_min_size(Pos2::ZERO, l.canvas);
                egui::Area::new(egui::Id::unique("frame-graph-overlay")).order(egui::Order::Foreground).fixed_pos(Pos2::ZERO).constrain(false).show(ctx, |ui| {
                    // Covers the viewer, so nothing under it takes the pointer.
                    ui.allocate_rect(canvas, egui::Sense::click_and_drag());
                    let overlay_ink = Ink::new(ui.painter());
                    ui.painter().rect_filled(canvas, CornerRadius::ZERO, palette::background().gamma_multiply(0.92));
                    let area = canvas.shrink(24.0);
                    let source = viewer.graph_source && input.state.panel == PanelMode::Dag;
                    let pick = if input.state.panel == PanelMode::Dag {
                        // With the source: the graph on the left, the selected pass's WGSL on the right.
                        let graph_area = if source { Rect::from_min_max(area.min, pos2(area.min.x + area.width() * 0.5 - 8.0, area.max.y)) } else { area };
                        let pick = draw_dag_panel(&GraphPanel { ctx, ink: &overlay_ink, rect: graph_area, view: &view, highlight: &input.state.highlight, full: true, interactive: true, source: Some(source) });
                        if source {
                            let pane = Rect::from_min_max(pos2(graph_area.max.x + 16.0, area.min.y), area.max);
                            draw_source_pane(ctx, &overlay_ink, pane, &view, viewer.shader_reload.as_ref().filter(|r| r.1).map(|r| r.0.as_str()));
                        }
                        pick
                    } else {
                        draw_resource_panel(&GraphPanel { ctx, ink: &overlay_ink, rect: area, view: &view, highlight: &input.state.highlight, full: true, interactive: true, source: None })
                    };
                    if let Some(pick) = pick {
                        viewer.actions.push(graph_action(pick, &view));
                    }
                    ink.min_size.set(ink.min_size.get().min(overlay_ink.min_size.get()));
                });
                viewer.ui_has_pointer = true;
            }
            if viewer.knowledge.overlay && draw_knowledge_overlay(ctx, l.canvas, &viewer.knowledge.clone()) {
                viewer.actions.push(ViewerAction::SetKnowledgeOverlay(false));
            }
            viewer.popups.draw(ctx, l.canvas);
            viewer.ui_has_pointer |= ctx.egui_wants_pointer_input() || viewer.knowledge.overlay;
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
