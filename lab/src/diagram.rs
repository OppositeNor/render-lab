//! A story's 2D diagrams (src/graph_view.rs): a beat's `diagram` (the
//! package's `video/diagrams/<name>.toml`, which the timeline carries as a
//! `graph_view::Spec`) or the frame graph's scene (`graph`, src/graph/
//! `FrameGraph::scene_graph`), fitted in the beat's panel with its `focus`
//! lit: a named box, or the row of a named member. The frame graph's lit
//! boxes hold pictures of what they make (src/renderer/ captures them).
//!
//! A diagram moves rather than fades: a beat after one of the same diagram
//! glides from that beat's view (its boxes' places and sizes, its framing,
//! its lit boxes) to its own, its new pictures fading in as they grow; the
//! first beat of a diagram builds it up, layer by layer, each edge drawn
//! along its path once its source has appeared (`Transition`). Both depend
//! on the frame's time in its beat alone, so a frame is the same every time.

use crate::graph_view::{self, Graph, Look, NodeLook, Options, Transform};
use crate::palette;
use crate::text::Ink;
use egui::{CornerRadius, FontFamily, Rect, Stroke, StrokeKind};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// The largest size of a diagram's labels (px at 1080p): a small diagram is
/// not blown up beyond it.
const MAX_FONT: f32 = 33.75;
/// The margin around a diagram inside its panel (px at 1080p).
const MARGIN: f32 = 35.0;

/// What a 2D beat shows of its diagram: the graph, its lit names, and the
/// frame graph's scene it was made from (its lit boxes hold pictures).
#[derive(Clone, Copy, Debug)]
pub struct BeatDiagram<'a> {
    pub graph: &'a Graph,
    pub focus: &'a [String],
    pub scene: Option<&'a crate::graph::GraphScene>,
    pub transition: Transition<'a>,
}

/// How a diagram comes on: from the focus of the beat before (of the same
/// diagram), or built up (`built` false); `seconds` into its beat.
#[derive(Clone, Copy, Debug)]
pub struct Transition<'a> {
    pub from: Option<&'a [String]>,
    pub seconds: f32,
    pub built: bool,
}

impl Default for Transition<'_> {
    /// Settled: shown as it is.
    fn default() -> Self {
        Transition {
            from: None,
            seconds: 0.0,
            built: true,
        }
    }
}

/// How long a move takes, a box takes to appear, and the delay from the
/// first layer's appearing to the last's (seconds).
const MOVE: f32 = 0.9;
const APPEAR: f32 = 0.35;
const BUILD: f32 = 1.0;

fn ease(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// A lit box's picture: the section, its texture, and the texture's size.
pub type Picture = (String, egui::TextureId, [u32; 2]);

impl BeatDiagram<'_> {
    /// The frame graph scene's lit sections (named, or one of their
    /// members) and what each one's picture shows: its `picture`
    /// (`<node>.<resource>`), else its last member (a pass or a resource).
    pub fn lit_groups(&self) -> Vec<(String, String)> {
        lit_sections(self.scene, self.focus)
    }
}

fn lit_sections(
    scene: Option<&crate::graph::GraphScene>,
    focus: &[String],
) -> Vec<(String, String)> {
    let Some(scene) = scene else {
        return Vec::new();
    };
    scene
        .sections
        .iter()
        .filter(|s| !s.outside && !(s.passes.is_empty() && s.resources.is_empty()))
        .filter(|s| {
            focus.contains(&s.id)
                || s.passes
                    .iter()
                    .chain(&s.resources)
                    .any(|m| focus.contains(m))
        })
        .filter_map(|s| {
            Some((
                s.id.clone(),
                s.picture
                    .clone()
                    .or_else(|| s.resources.last().or(s.passes.last()).cloned())?,
            ))
        })
        .collect()
}

/// A diagram's layout, once per graph and set of pictured nodes.
fn laid_out(
    graph: &Graph,
    width: &dyn Fn(&str) -> f32,
    pictured: &[usize],
    aspect: f32,
) -> &'static graph_view::Laid {
    use std::hash::{Hash, Hasher};
    static CACHE: OnceLock<Mutex<HashMap<u64, &'static graph_view::Laid>>> = OnceLock::new();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    format!("{graph:?}").hash(&mut h);
    pictured.hash(&mut h);
    aspect.to_bits().hash(&mut h);
    let mut cache = CACHE.get_or_init(Default::default).lock().unwrap();
    cache.entry(h.finish()).or_insert_with(|| {
        Box::leak(Box::new(graph_view::lay_out(
            graph,
            width,
            pictured,
            aspect,
            &Options::default(),
        )))
    })
}

/// A paused timeline's diagram in the viewer (src/graph_view.rs `view`):
/// the wheel zooms it (from fitted to `MAX_ZOOM` times), a drag pans it or
/// moves a box, a double click fits it again; `key` names its beat, and
/// `preset` is a first view (the zoom, and the point at the centre as
/// fractions of the diagram across and down).
#[derive(Clone, Copy)]
pub struct Paused<'a> {
    pub ctx: &'a egui::Context,
    pub key: u64,
    pub preset: Option<(f32, egui::Vec2)>,
}

/// How far a paused diagram zooms in.
pub const MAX_ZOOM: f32 = 8.0;

/// The id a paused diagram's view is kept under (its "Reset nodes" button asks it).
pub fn paused_id() -> egui::Id {
    egui::Id::unique("paused-diagram")
}

/// Draws `beat`'s diagram in its panel `area`, fitted (its labels at most
/// `MAX_FONT` high), or paused in the viewer (`paused`: zoomed, panned, its
/// boxes moved). A lit box with one of `pictures` holds it under its rows
/// (`plan`). Coming from the beat before, the diagram moves from that beat's
/// plan to its own; a first beat builds it up. Labels go through `ink`,
/// whose smallest size the recorder checks against the lab's minimum.
/// Returns the zoom (1: fitted).
pub fn draw(
    ink: &Ink,
    area: Rect,
    paused: Option<Paused>,
    beat: BeatDiagram,
    pictures: &[Picture],
) -> f32 {
    let painter = ink.painter;
    painter.rect_filled(area, CornerRadius::same(12), palette::panel());
    let graph = beat.graph;
    let width = |s: &str| ink.measure(s, 100.0, FontFamily::Proportional).x / 100.0;
    let aspect = pictures
        .first()
        .map_or(9.0 / 16.0, |p| p.2[1] as f32 / p.2[0].max(1) as f32)
        .clamp(0.3, 1.5);
    let now: Vec<usize> = pictures
        .iter()
        .filter_map(|p| graph.nodes.iter().position(|n| n.id == p.0))
        .collect();
    let current = plan(area, graph, &width, &now, aspect);
    let tr = beat.transition;
    // On the way from the beat before: its plan (its lit boxes' pictures had their room).
    let moving = tr.from.filter(|_| tr.seconds < MOVE);
    let e = ease(tr.seconds / MOVE);
    let before: Vec<usize> = moving
        .map(|f| {
            lit_sections(beat.scene, f)
                .iter()
                .filter_map(|(id, _)| graph.nodes.iter().position(|n| &n.id == id))
                .collect()
        })
        .unwrap_or_default();
    let (laid, base) = match moving {
        Some(_) => {
            let from = plan(area, graph, &width, &before, aspect);
            let t = Transform {
                scale: from.1.scale + (current.1.scale - from.1.scale) * e,
                origin: from.1.origin + (current.1.origin - from.1.origin) * e,
            };
            (graph_view::blend(from.0, current.0, e), t)
        }
        None => (current.0.clone(), current.1),
    };
    let shown = &current.2;
    // Paused: the viewer's view of it, from the fitted one.
    let (t, moved) = paused_view(paused, area, base, &laid);
    let zoom_now = t.scale / base.scale;
    let zoom = zoom_now.min(2.0);
    let look = |focus: &[String], i: usize| -> NodeLook { focus_look(graph, focus, i, zoom) };
    // Building up: a box appears when its layer's turn comes (by its place along the flow).
    let b = laid.bounds;
    let turn = |i: usize| {
        if b.width() > 1e-3 {
            (laid.rects[i].min.x - b.min.x) / b.width() * BUILD
        } else {
            0.0
        }
    };
    let appear = |i: usize| {
        if tr.built {
            1.0
        } else {
            ease((tr.seconds - turn(i)) / APPEAR)
        }
    };
    let nodes = (0..graph.nodes.len())
        .map(|i| {
            let mut l = match moving {
                Some(f) => look(f, i).blend(&look(beat.focus, i), e),
                None => look(beat.focus, i),
            };
            l.picture = pictures
                .iter()
                .find(|p| p.0 == graph.nodes[i].id)
                .filter(|_| shown.contains(&i))
                .map(|p| (p.1, p.2));
            // An image node: its PNG, decoded once.
            if let Some((key, _)) = &graph.nodes[i].image {
                l.picture = crate::utility::inset::resource_texture(painter.ctx(), key)
                    .map(|t| (t.id(), [t.size()[0] as u32, t.size()[1] as u32]));
            }
            // A picture new to this beat fades in as its room grows.
            if moving.is_some() && !before.contains(&i) {
                l.tint = egui::Color32::WHITE.gamma_multiply(e);
            }
            l.alpha = appear(i);
            l
        })
        .collect();
    let edges = diagram_edges(graph, &laid, tr, zoom);
    graph_view::draw(
        ink,
        area,
        &t,
        graph,
        &laid,
        &Look {
            nodes,
            edges,
            clusters: Vec::new(),
            moved: &moved,
            edge_labels: true,
            tracked: true,
        },
    );
    painter.rect_stroke(
        area,
        CornerRadius::same(12),
        Stroke::new(1.0, palette::border()),
        StrokeKind::Inside,
    );
    zoom_now
}

/// Paused: the viewer's view of the diagram, from the fitted one (`base`): zoomed, panned, its boxes moved; else
/// the fitted view.
fn paused_view(
    paused: Option<Paused>,
    area: Rect,
    base: Transform,
    laid: &graph_view::Laid,
) -> (Transform, Vec<(usize, egui::Vec2)>) {
    let (t, moved) = match paused {
        Some(p) => {
            let id = paused_id();
            let fitted = (area.center() - base.origin) / base.scale;
            let setup = |v: &mut graph_view::ViewState, _: bool| {
                v.scale = base.scale;
                v.centre = fitted;
                if let Some((zoom, at)) = p.preset {
                    v.scale = base.scale * zoom;
                    v.centre = laid.bounds.min.to_vec2() + laid.bounds.size() * at;
                }
            };
            let controls = graph_view::Controls {
                zoom: Some((base.scale, base.scale * MAX_ZOOM)),
                move_nodes: true,
            };
            let (mut state, _) =
                graph_view::view(p.ctx, Some(id), area, laid, p.key, &controls, &setup);
            graph_view::keep(&mut state, laid, area);
            p.ctx.data_mut(|d| d.insert_temp(id, state.clone()));
            (state.transform(area), state.moved)
        }
        None => (base, Vec::new()),
    };
    (t, moved)
}

/// Box `i` as the beat's `focus` lights it: named, outlined; one of its rows named, its outline accented and the
/// row lit.
fn focus_look(graph: &Graph, focus: &[String], i: usize, zoom: f32) -> NodeLook {
    let node = &graph.nodes[i];
    let named = focus.contains(&node.id);
    let rows: Vec<bool> = node.rows.iter().map(|r| focus.contains(&r.id)).collect();
    let mut look = NodeLook::plain(node, named, 1.5 * zoom);
    if !named && rows.contains(&true) {
        look.outline = Stroke::new(2.5 * zoom, palette::accent());
    }
    look.rows = rows
        .iter()
        .map(|&lit| {
            (
                if lit {
                    palette::text()
                } else {
                    palette::text_dim()
                },
                lit,
            )
        })
        .collect();
    look
}

/// The diagram's arrows: building up, each drawn once the box it leaves has appeared.
fn diagram_edges(
    graph: &Graph,
    laid: &graph_view::Laid,
    tr: Transition,
    zoom: f32,
) -> Vec<graph_view::EdgeLook> {
    let b = laid.bounds;
    let turn = |i: usize| {
        if b.width() > 1e-3 {
            (laid.rects[i].min.x - b.min.x) / b.width() * BUILD
        } else {
            0.0
        }
    };

    graph
        .edges
        .iter()
        .map(|edge| {
            let drawn = if tr.built {
                1.0
            } else {
                ((tr.seconds - turn(edge.from) - APPEAR * 0.6) / (APPEAR * 1.5)).clamp(0.0, 1.0)
            };
            graph_view::EdgeLook {
                color: palette::text_dim(),
                width: 1.8 * zoom,
                hidden: false,
                drawn,
            }
        })
        .collect()
}

/// A diagram's layout and framing in `area` with `pictured` boxes holding
/// pictures: fitted if the labels stay at the lab's minimum; else at the
/// minimum, the view on the pictured boxes (the rest clipped), if they fit in
/// it; else without pictures. Also which boxes show their pictures.
fn plan(
    area: Rect,
    graph: &Graph,
    width: &dyn Fn(&str) -> f32,
    pictured: &[usize],
    aspect: f32,
) -> (&'static graph_view::Laid, Transform, Vec<usize>) {
    let min = crate::text::MIN_TEXT_PT;
    let fit_of = |laid: &graph_view::Laid| {
        ((area.width() - 2.0 * MARGIN) / laid.bounds.width())
            .min((area.height() - 2.0 * MARGIN) / laid.bounds.height())
            .min(MAX_FONT)
    };
    let plain = laid_out(graph, width, &[], aspect);
    if !pictured.is_empty() {
        let with = laid_out(graph, width, pictured, aspect);
        if fit_of(with) >= min {
            return (
                with,
                Transform::fit(area, with.bounds, MARGIN, MAX_FONT),
                pictured.to_vec(),
            );
        }
        let lit = pictured
            .iter()
            .map(|&i| with.rects[i])
            .reduce(|a, b| a.union(b))
            .expect("a pictured node");
        let room = (area.size() - egui::Vec2::splat(2.0 * MARGIN)) / min;
        if lit.width() <= room.x && lit.height() <= room.y {
            // Centred on them, but not past the diagram's edges where it can help it.
            let b = with.bounds;
            let half = room / 2.0;
            let keep = |c: f32, a: f32, z: f32, h: f32| {
                if z - a <= 2.0 * h {
                    (a + z) / 2.0
                } else {
                    c.clamp(a + h, z - h)
                }
            };
            let centre = egui::pos2(
                keep(lit.center().x, b.min.x, b.max.x, half.x),
                keep(lit.center().y, b.min.y, b.max.y, half.y),
            );
            return (
                with,
                Transform::centred(area, centre, min),
                pictured.to_vec(),
            );
        }
    }
    (
        plain,
        Transform::fit(area, plain.bounds, MARGIN, MAX_FONT),
        Vec::new(),
    )
}
