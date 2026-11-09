//! One view for every graph the lab draws: a story's diagrams and the frame
//! graph's scene (src/diagram.rs), the frame graph's DAG panel (src/ui/),
//! and the knowledge graph (src/knowledge.rs).
//!
//! A `Graph` is boxes (`Node`: title lines, member rows, a dashed outline, or
//! a fixed-size card), edges, and clusters of nodes (boxed groups, or loose
//! sets laid out together). `lay_out` places them with the layered layout
//! (src/layered.rs; `clustered` when there are clusters), left to right or
//! top to bottom, routes the edges (an edge that closes a cycle as an arc
//! back), and reserves room for pictures. `draw` paints a laid-out graph as
//! its `Look` says (colours, lit rows, pictures, badges); `interact` reads
//! the pointer for the views that pan, zoom, and move their nodes.

use crate::layered;
use crate::palette;
use crate::text::Ink;
use egui::{
    Align2, Color32, CornerRadius, FontFamily, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2, pos2,
    vec2,
};

mod draw;
pub use draw::*;

/// A label line's height, a box's padding across and down, a member row's
/// height, the room around a box's rule, and a picture's greatest height
/// (font units: a label's font is 1 high).
pub const LINE: f32 = 1.32;
pub const PAD_X: f32 = 1.2;
pub const PAD_Y: f32 = 0.8;
pub const ROW: f32 = 1.12;
pub const RULE: f32 = 0.4;
pub const PICTURE: f32 = 4.2;
/// An image node's least width and greatest height (font units).
pub const IMAGE: f32 = 14.0;

/// A texture shown in a node, and its size in texels.
pub type Picture = (egui::TextureId, [u32; 2]);

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    /// Sets of nodes laid out together, in the order of their tracks; when
    /// any, every node is in one.
    pub clusters: Vec<Cluster>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Node {
    pub id: String,
    pub title: Vec<String>,
    /// What it holds, a row each (a section's passes), under its title.
    pub rows: Vec<Row>,
    pub dashed: bool,
    /// A card of this size (font units): its title on one line over its picture.
    pub card: Option<(f32, f32)>,
    /// An image under its title: a PNG resource's key and its shape (height over width).
    pub image: Option<(String, f32)>,
}

/// The shape (height over width) of the PNG resource `key`, from its header.
pub fn png_aspect(key: &str) -> Result<f32, String> {
    let bytes = crate::resources::find(key)
        .ok_or_else(|| format!("no resource `{key}`"))?
        .bytes();
    if bytes.len() < 24 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" {
        return Err(format!("`{key}` is not a PNG"));
    }
    let word = |at: usize| {
        u32::from_be_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]) as f32
    };
    Ok(word(20) / word(16).max(1.0))
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Row {
    pub id: String,
    pub label: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Edge {
    pub from: usize,
    pub to: usize,
    pub dotted: bool,
    pub label: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Cluster {
    pub title: Option<String>,
    pub members: Vec<usize>,
    pub boxed: bool,
}

/// A graph as data (a story's diagram in a timeline; tools/video/diagrams.py
/// writes it from the package's `video/diagrams/<name>.toml`): nodes and
/// rows by id, edges between ids.
#[derive(Clone, Debug, Default, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Spec {
    pub nodes: Vec<SpecNode>,
    #[serde(default)]
    pub edges: Vec<SpecEdge>,
}

#[derive(Clone, Debug, Default, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct SpecNode {
    pub id: String,
    pub title: Vec<String>,
    #[serde(default)]
    pub rows: Vec<SpecRow>,
    #[serde(default)]
    pub dashed: bool,
    /// An image under its title: a PNG resource's key (`<package>.<id>`).
    #[serde(default)]
    pub image: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct SpecRow {
    pub id: String,
    pub label: String,
}

#[derive(Clone, Debug, Default, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct SpecEdge {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub dotted: bool,
    #[serde(default)]
    pub label: Option<String>,
}

impl Spec {
    /// The graph it describes; an edge to an unknown id, or two nodes or rows of one id, is an error.
    pub fn graph(&self) -> Result<Graph, String> {
        let mut seen = std::collections::BTreeSet::new();
        for id in self
            .nodes
            .iter()
            .flat_map(|n| std::iter::once(&n.id).chain(n.rows.iter().map(|r| &r.id)))
        {
            if !seen.insert(id.as_str()) {
                return Err(format!("two nodes or rows are named `{id}`"));
            }
        }
        if self.nodes.is_empty() {
            return Err("a diagram without nodes".into());
        }
        let index = |id: &str| {
            self.nodes
                .iter()
                .position(|n| n.id == id)
                .ok_or_else(|| format!("an edge names `{id}`, which is no node"))
        };
        let edges = self
            .edges
            .iter()
            .map(|e| {
                Ok(Edge {
                    from: index(&e.from)?,
                    to: index(&e.to)?,
                    dotted: e.dotted,
                    label: e.label.clone().filter(|l| !l.is_empty()),
                })
            })
            .collect::<Result<_, String>>()?;
        let nodes = self
            .nodes
            .iter()
            .map(|n| {
                let image = n
                    .image
                    .as_ref()
                    .map(|key| png_aspect(key).map(|a| (key.clone(), a)))
                    .transpose()
                    .map_err(|e| format!("{}: {e}", n.id))?;
                Ok(Node {
                    id: n.id.clone(),
                    title: n.title.clone(),
                    rows: n
                        .rows
                        .iter()
                        .map(|r| Row {
                            id: r.id.clone(),
                            label: r.label.clone(),
                        })
                        .collect(),
                    dashed: n.dashed,
                    card: None,
                    image,
                })
            })
            .collect::<Result<_, String>>()?;
        Ok(Graph {
            nodes,
            edges,
            clusters: Vec::new(),
        })
    }
}

impl Graph {
    /// The node a name means: a node by its id, or the node a row of that id is in.
    pub fn named(&self, name: &str) -> Option<usize> {
        self.nodes.iter().position(|n| n.id == name).or_else(|| {
            self.nodes
                .iter()
                .position(|n| n.rows.iter().any(|r| r.id == name))
        })
    }

    /// Problems with a beat's `focus`: names no node or row has.
    pub fn check_focus(&self, focus: &[String]) -> Result<(), String> {
        match focus.iter().find(|f| self.named(f).is_none()) {
            Some(f) => Err(format!("unknown focus node {f}")),
            None => Ok(()),
        }
    }

    /// Per edge, whether it closes a cycle with the edges written before it:
    /// such an edge is laid out reversed and drawn as an arc back.
    pub fn back_edges(&self) -> Vec<bool> {
        let mut after: Vec<Vec<usize>> = vec![Vec::new(); self.nodes.len()];
        let reaches = |after: &[Vec<usize>], from: usize, to: usize| {
            let mut seen = vec![false; after.len()];
            let mut stack = vec![from];
            while let Some(i) = stack.pop() {
                if i == to {
                    return true;
                }
                if !std::mem::replace(&mut seen[i], true) {
                    stack.extend(&after[i]);
                }
            }
            false
        };
        self.edges
            .iter()
            .map(|e| {
                let back = reaches(&after, e.to, e.from);
                if !back {
                    after[e.from].push(e.to);
                }
                back
            })
            .collect()
    }
}

/// Which way the layers run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Flow {
    #[default]
    Right,
    Down,
}

/// How `lay_out` spaces a graph (font units): its flow; the space between
/// layers, between the nodes of a layer, and a skipping edge's slot (none:
/// the layers leave room for the edges' labels); and a cluster box's
/// padding, its title's room, and the gap between tracks.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub flow: Flow,
    pub spacing: Option<layered::Spacing>,
    pub pad: f32,
    pub title: f32,
    pub gap: f32,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            flow: Flow::Right,
            spacing: None,
            pad: 0.7,
            title: 1.5,
            gap: 1.8,
        }
    }
}

/// An edge as laid out: its points, its label's place, and whether it is an arc back.
#[derive(Clone, Debug)]
pub struct Route {
    pub points: Vec<Pos2>,
    pub label_at: Pos2,
    pub back: bool,
}

/// A graph laid out (font units).
#[derive(Clone, Debug)]
pub struct Laid {
    pub rects: Vec<Rect>,
    /// Each node's picture height (0: none).
    pub pictures: Vec<f32>,
    pub routes: Vec<Route>,
    /// The boxed clusters' rectangles, by cluster.
    pub boxes: Vec<(usize, Rect)>,
    pub flow: Flow,
    /// Everything drawn.
    pub bounds: Rect,
}

/// The points of a cubic Bézier curve (after its first).
fn cubic(p: [Pos2; 4], out: &mut Vec<Pos2>) {
    for k in 1..=16 {
        let t = k as f32 / 16.0;
        let u = 1.0 - t;
        out.push(
            (p[0].to_vec2() * (u * u * u)
                + p[1].to_vec2() * (3.0 * u * u * t)
                + p[2].to_vec2() * (3.0 * u * t * t)
                + p[3].to_vec2() * (t * t * t))
                .to_pos2(),
        );
    }
}

/// A curve from `a` to `b` leaving and entering along the flow.
fn curve(a: Pos2, b: Pos2, flow: Flow) -> Vec<Pos2> {
    let mut points = vec![a];
    match flow {
        Flow::Right => {
            let dx = ((b.x - a.x) / 2.0).max(0.8);
            cubic([a, a + vec2(dx, 0.0), b - vec2(dx, 0.0), b], &mut points);
        }
        Flow::Down => {
            let dy = ((b.y - a.y) / 2.0).max(0.8);
            cubic([a, a + vec2(0.0, dy), b - vec2(0.0, dy), b], &mut points);
        }
    }
    points
}

/// Where an edge leaves `a` and enters `b` along the flow.
pub fn ends(a: Rect, b: Rect, flow: Flow) -> (Pos2, Pos2) {
    match flow {
        Flow::Right => (a.right_center(), b.left_center()),
        Flow::Down => (a.center_bottom(), b.center_top()),
    }
}

/// A node's size (font units): a card's own; else its title (bold over
/// rows), its rows, the rule between, and room for a picture of `aspect`
/// (height over width) when `pictured`.
fn size(
    node: &Node,
    width: &dyn Fn(&str) -> f32,
    pictured: bool,
    aspect: f32,
) -> ((f32, f32), f32) {
    if let Some(card) = node.card {
        return (card, 0.0);
    }
    // An image: at least `IMAGE` wide, its own shape, at most `IMAGE` high.
    let (pictured, aspect) = match &node.image {
        Some((_, a)) => (true, *a),
        None => (pictured, aspect),
    };
    let grouped = !node.rows.is_empty() || pictured;
    let bold = if grouped {
        1.08
    } else {
        1.0
    };
    let w = node
        .title
        .iter()
        .map(|l| width(l) * bold)
        .chain(node.rows.iter().map(|r| width(&r.label)))
        .fold(0.0, f32::max);
    let rule = if node.rows.is_empty() {
        0.0
    } else {
        RULE
    };
    let w = if node.image.is_some() {
        w.max(IMAGE)
    } else {
        w
    };
    let most = if node.image.is_some() {
        IMAGE
    } else {
        PICTURE
    };
    let picture = if pictured {
        (w * aspect).min(most)
    } else {
        0.0
    };
    let extra = if pictured {
        picture + RULE
    } else {
        0.0
    };
    (
        (
            w + PAD_X,
            node.title.len() as f32 * LINE + node.rows.len() as f32 * ROW + rule + extra + PAD_Y,
        ),
        picture,
    )
}

/// The layered layout of `graph`, left to right (`sizes` already turned for its flow), with its edges `edges`
/// (the back edges reversed): the layout and the boxed clusters' rectangles.
fn layered_layout(
    graph: &Graph,
    sizes: &[(f32, f32)],
    edges: &[(usize, usize)],
    back: &[bool],
    width: &dyn Fn(&str) -> f32,
    options: &Options,
) -> layered::Clustered {
    let flow = options.flow;
    let label_room = graph
        .edges
        .iter()
        .zip(back)
        .filter(|(_, b)| !**b)
        .filter_map(|(e, _)| e.label.as_deref())
        .map(|l| {
            if flow == Flow::Down {
                LINE + 0.4
            } else {
                width(l) + 1.0
            }
        })
        .fold(0.0, f32::max);
    let spacing = options.spacing.unwrap_or(layered::Spacing {
        column: label_room.max(1.7),
        row: 0.7,
        dummy: 0.5,
    });
    if graph.clusters.is_empty() {
        let order: Vec<usize> = (0..graph.nodes.len()).collect();
        return layered::Clustered {
            layout: layered::layout(sizes, edges, &order, spacing),
            boxes: Vec::new(),
        };
    }
    let clusters: Vec<layered::Cluster> = graph
        .clusters
        .iter()
        .map(|c| layered::Cluster {
            members: c.members.clone(),
            boxed: c.boxed,
        })
        .collect();
    // A box's title goes above it once turned: on its side, no room is kept beside it.
    let title = if flow == Flow::Down {
        0.0
    } else {
        options.title
    };
    layered::clustered(
        sizes,
        edges,
        &clusters,
        spacing,
        options.pad,
        title,
        options.gap,
    )
}

/// What the edges' routes are laid out in (the layout's space, left to right): the layout, how wide a line of
/// text is, and the bounds the routes grow.
struct Routing<'a> {
    layout: &'a layered::Layout,
    width: &'a dyn Fn(&str) -> f32,
    lo: Pos2,
    hi: Pos2,
}

impl Routing<'_> {
    /// Node `i`'s rectangle in the layout's space.
    fn rect(&self, i: usize) -> Rect {
        let layout = self.layout;
        Rect::from_center_size(
            pos2(layout.positions[i].0, layout.positions[i].1),
            vec2(layout.widths[i], layout.heights[i]),
        )
    }

    /// The widest node of the column at `x`.
    fn column_width(&self, x: f32) -> f32 {
        let layout = self.layout;
        (0..layout.positions.len())
            .filter(|&i| (layout.positions[i].0 - x).abs() < 1e-3)
            .map(|i| layout.widths[i])
            .fold(0.0, f32::max)
    }

    /// A back edge: an arc under (or over) the columns, toward the nearer side; its points and its label's place.
    fn back(&mut self, edge: &Edge) -> (Vec<Pos2>, Pos2) {
        let (a, b) = (self.rect(edge.from), self.rect(edge.to));
        let below = (a.center().y + b.center().y) / 2.0 >= self.layout.size.1 / 2.0;
        let depth = 1.6;
        let (p0, p3) = if below {
            (a.center_bottom(), b.center_bottom())
        } else {
            (a.center_top(), b.center_top())
        };
        let y = if below {
            p0.y.max(p3.y) + depth
        } else {
            p0.y.min(p3.y) - depth
        };
        let mut points = vec![p0];
        cubic([p0, pos2(p0.x, y), pos2(p3.x, y), p3], &mut points);
        let label_at = pos2(
            (p0.x + p3.x) / 2.0,
            if below {
                y - depth * 0.25 + 0.55
            } else {
                y + depth * 0.25 - 0.55
            },
        );
        let label = edge.label.as_deref().map_or(0.0, self.width);
        let room = if label > 0.0 {
            LINE
        } else {
            0.2
        };
        if below {
            self.hi.y = self.hi.y.max(y + room);
        } else {
            self.lo.y = self.lo.y.min(y - room);
        }
        self.lo.x = self.lo.x.min(label_at.x - label / 2.0 - 0.4);
        self.hi.x = self.hi.x.max(label_at.x + label / 2.0 + 0.4);
        (points, label_at)
    }

    /// A forward edge: through a column it skips, it runs straight across the column; its points and its
    /// label's place.
    fn forward(&self, edge: &Edge) -> (Vec<Pos2>, Pos2) {
        let (a, b) = (self.rect(edge.from), self.rect(edge.to));
        let mut stops = vec![a.right_center()];
        for &p in self
            .layout
            .bends
            .iter()
            .find(|x| x.0 == edge.from && x.1 == edge.to)
            .map(|x| x.2.as_slice())
            .unwrap_or_default()
        {
            let half = self.column_width(p.0) / 2.0;
            stops.push(pos2(p.0 - half, p.1));
            stops.push(pos2(p.0 + half, p.1));
        }
        stops.push(b.left_center());
        let mut points = vec![stops[0]];
        for (k, w) in stops.windows(2).enumerate() {
            if k % 2 == 1 {
                points.push(w[1]);
                continue;
            }
            points.extend(curve(w[0], w[1], Flow::Right).into_iter().skip(1));
        }
        let label_at = points[points.len() / 2];
        (points, label_at)
    }
}

/// Lays `graph` out: `width` measures a line of text in font units;
/// `pictured` nodes get room for a picture of `aspect` (height over width).
pub fn lay_out(
    graph: &Graph,
    width: &dyn Fn(&str) -> f32,
    pictured: &[usize],
    aspect: f32,
    options: &Options,
) -> Laid {
    let flow = options.flow;
    let sized: Vec<((f32, f32), f32)> = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| size(n, width, pictured.contains(&i), aspect))
        .collect();
    // The layered layout runs left to right: top to bottom is laid out on its side.
    let across = |s: (f32, f32)| {
        if flow == Flow::Down {
            (s.1, s.0)
        } else {
            s
        }
    };
    let sizes: Vec<(f32, f32)> = sized.iter().map(|s| across(s.0)).collect();
    let back = graph.back_edges();
    let edges: Vec<(usize, usize)> = graph
        .edges
        .iter()
        .zip(&back)
        .map(|(e, &b)| {
            if b {
                (e.to, e.from)
            } else {
                (e.from, e.to)
            }
        })
        .collect();
    let layered::Clustered { layout, boxes } =
        layered_layout(graph, &sizes, &edges, &back, width, options);
    // In the layout's space (left to right), then turned for a top-to-bottom flow.
    let side = |p: Pos2| {
        if flow == Flow::Down {
            pos2(p.y, p.x)
        } else {
            p
        }
    };
    let side_rect = |r: Rect| Rect::from_two_pos(side(r.min), side(r.max));
    let mut routing = Routing {
        layout: &layout,
        width,
        lo: pos2(0.0, 0.0),
        hi: pos2(layout.size.0, layout.size.1),
    };
    let routes: Vec<Route> = graph
        .edges
        .iter()
        .zip(&back)
        .map(|(edge, &back)| {
            let (points, label_at) = if back {
                routing.back(edge)
            } else {
                routing.forward(edge)
            };
            Route {
                points: points.into_iter().map(side).collect(),
                label_at: side(label_at),
                back,
            }
        })
        .collect();
    let rects: Vec<Rect> = (0..graph.nodes.len())
        .map(|i| side_rect(routing.rect(i)))
        .collect();
    let boxes: Vec<(usize, Rect)> = boxes
        .into_iter()
        .map(|(c, (x0, y0, x1, y1))| {
            let mut r = side_rect(Rect::from_min_max(pos2(x0, y0), pos2(x1, y1)));
            if flow == Flow::Down {
                r.min.y -= options.title;
            }
            (c, r)
        })
        .collect();
    let mut bounds = side_rect(Rect::from_min_max(routing.lo, routing.hi));
    for r in rects.iter().chain(boxes.iter().map(|b| &b.1)) {
        bounds = bounds.union(*r);
    }
    Laid {
        rects,
        pictures: sized.iter().map(|s| s.1).collect(),
        routes,
        boxes,
        flow,
        bounds,
    }
}

/// Where graph units land on screen: `origin` + a point × `scale`.
#[derive(Clone, Copy, Debug)]
pub struct Transform {
    pub scale: f32,
    pub origin: Pos2,
}

impl Transform {
    /// `bounds` fitted in `area` less `margin`, at most `max` pixels per unit.
    pub fn fit(area: Rect, bounds: Rect, margin: f32, max: f32) -> Transform {
        let scale = ((area.width() - 2.0 * margin) / bounds.width())
            .min((area.height() - 2.0 * margin) / bounds.height())
            .min(max);
        Transform::centred(area, bounds.center(), scale)
    }

    /// `centre` at `area`'s centre, at `scale`.
    pub fn centred(area: Rect, centre: Pos2, scale: f32) -> Transform {
        Transform {
            scale,
            origin: area.center() - centre.to_vec2() * scale,
        }
    }

    pub fn at(&self, p: Pos2) -> Pos2 {
        self.origin + p.to_vec2() * self.scale
    }

    pub fn rect(&self, r: Rect) -> Rect {
        Rect::from_min_max(self.at(r.min), self.at(r.max))
    }
}

/// How a node is drawn.
#[derive(Clone, Debug)]
pub struct NodeLook {
    pub fill: Color32,
    pub outline: Stroke,
    pub text: Color32,
    /// Each row's text colour and whether it is lit (a band behind it).
    pub rows: Vec<(Color32, bool)>,
    pub picture: Option<Picture>,
    /// A picture's tint (dimmed when the view calls other nodes out).
    pub tint: Color32,
    /// Text in a card's picture corner (a GPU time), or in place of a missing picture.
    pub badge: Option<String>,
    pub note: Option<String>,
    pub hidden: bool,
    /// How much of it shows (a diagram building up: 0 none, 1 all).
    pub alpha: f32,
}

impl NodeLook {
    /// Between `self` (at 0) and `other` (at 1): colours and widths blended.
    pub fn blend(&self, other: &NodeLook, t: f32) -> NodeLook {
        let c = |a: Color32, b: Color32| lerp_color(a, b, t);
        NodeLook {
            fill: c(self.fill, other.fill),
            outline: Stroke::new(
                self.outline.width + (other.outline.width - self.outline.width) * t,
                c(self.outline.color, other.outline.color),
            ),
            text: c(self.text, other.text),
            rows: other
                .rows
                .iter()
                .zip(
                    self.rows
                        .iter()
                        .chain(std::iter::repeat(&(palette::text_dim(), false))),
                )
                .map(|(b, a)| {
                    (
                        c(a.0, b.0),
                        if t >= 0.5 {
                            b.1
                        } else {
                            a.1
                        },
                    )
                })
                .collect(),
            picture: other.picture.or(self.picture),
            tint: c(self.tint, other.tint),
            badge: other.badge.clone(),
            note: other.note.clone(),
            hidden: other.hidden && self.hidden,
            alpha: self.alpha + (other.alpha - self.alpha) * t,
        }
    }

    /// A node as a diagram draws it: dim (`lit` false) or lit, its outline
    /// `width` pixels.
    pub fn plain(node: &Node, lit: bool, width: f32) -> NodeLook {
        let (fill, outline) = if lit {
            (
                palette::accent().gamma_multiply(0.35),
                Stroke::new(width * 2.6, palette::accent()),
            )
        } else {
            (
                palette::panel_raised(),
                Stroke::new(width, palette::text_faint()),
            )
        };
        NodeLook {
            fill,
            outline,
            text: palette::text(),
            rows: node
                .rows
                .iter()
                .map(|_| (palette::text_dim(), false))
                .collect(),
            picture: None,
            tint: Color32::WHITE,
            badge: None,
            note: None,
            hidden: false,
            alpha: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct EdgeLook {
    pub color: Color32,
    pub width: f32,
    pub hidden: bool,
    /// How much of it is drawn, from its start (a diagram building up).
    pub drawn: f32,
}

/// `a` to `b` by `t` (0..1), in each channel.
pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let m = |x: u8, y: u8| {
        (x as f32 + (y as f32 - x as f32) * t)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    Color32::from_rgba_premultiplied(
        m(a.r(), b.r()),
        m(a.g(), b.g()),
        m(a.b(), b.b()),
        m(a.a(), b.a()),
    )
}

/// Between two layouts of one graph (`a` at 0, `b` at 1): the nodes, boxes,
/// and pictures where they are by `t`, the edges re-routed between the
/// nodes (or blended, point by point, where both routes have as many).
pub fn blend(a: &Laid, b: &Laid, t: f32) -> Laid {
    let r = |x: Rect, y: Rect| {
        Rect::from_min_max(x.min + (y.min - x.min) * t, x.max + (y.max - x.max) * t)
    };
    let rects: Vec<Rect> = a
        .rects
        .iter()
        .zip(&b.rects)
        .map(|(x, y)| r(*x, *y))
        .collect();
    let routes = a
        .routes
        .iter()
        .zip(&b.routes)
        .map(|(x, y)| {
            let p = |u: Pos2, v: Pos2| u + (v - u) * t;
            if x.points.len() == y.points.len() {
                Route {
                    points: x
                        .points
                        .iter()
                        .zip(&y.points)
                        .map(|(u, v)| p(*u, *v))
                        .collect(),
                    label_at: p(x.label_at, y.label_at),
                    back: y.back,
                }
            } else {
                y.clone()
            }
        })
        .collect();
    let boxes = b
        .boxes
        .iter()
        .map(|&(c, y)| {
            (
                c,
                a.boxes
                    .iter()
                    .find(|x| x.0 == c)
                    .map_or(y, |&(_, x)| r(x, y)),
            )
        })
        .collect();
    Laid {
        rects,
        pictures: a
            .pictures
            .iter()
            .zip(&b.pictures)
            .map(|(x, y)| x + (y - x) * t)
            .collect(),
        routes,
        boxes,
        flow: b.flow,
        bounds: r(a.bounds, b.bounds),
    }
}

/// The first `fraction` of a line, by length.
fn prefix(points: &[Pos2], fraction: f32) -> Vec<Pos2> {
    if fraction >= 1.0 || points.len() < 2 {
        return points.to_vec();
    }
    let total: f32 = points.windows(2).map(|w| (w[1] - w[0]).length()).sum();
    let mut left = total * fraction.max(0.0);
    let mut out = vec![points[0]];
    for w in points.windows(2) {
        let step = (w[1] - w[0]).length();
        if step >= left {
            out.push(w[0] + (w[1] - w[0]) * (left / step.max(1e-6)));
            return out;
        }
        left -= step;
        out.push(w[1]);
    }
    out
}

/// A view of a graph that pans, zooms, and moves its nodes by hand, kept in
/// egui's memory between frames: its scale (pixels per unit; 0 until set),
/// the graph point at the area's centre, the moved nodes, what a drag holds
/// (the view, or a node), and what it was last set up for.
#[derive(Clone, Debug, Default)]
pub struct ViewState {
    pub scale: f32,
    pub centre: Vec2,
    pub moved: Vec<(usize, Vec2)>,
    drag: Option<Option<usize>>,
    /// What the view was set up for (`view`'s key), and the layout it was of.
    key: Option<u64>,
    laid: usize,
}

impl ViewState {
    pub fn transform(&self, area: Rect) -> Transform {
        Transform::centred(area, self.centre.to_pos2(), self.scale)
    }
}

/// Asks the view kept under `id` to put its nodes back and set itself up
/// afresh on its next frame (a "Reset nodes" button; as a double click does).
pub fn ask_reset(ctx: &egui::Context, id: egui::Id) {
    ctx.data_mut(|d| d.insert_temp(id.with("reset"), true));
}

/// Whether the view kept under `id` was asked to reset (once).
pub fn take_reset(ctx: &egui::Context, id: egui::Id) -> bool {
    ctx.data_mut(|d| d.remove_temp::<bool>(id.with("reset")))
        .unwrap_or(false)
}

/// A view of `laid` in `area` for this frame: in the viewer kept in egui's
/// memory under the id `kept` names (the pointer works then), else made
/// afresh (None). When `key` changes (a
/// new graph, new call-outs) `setup` sets it up again (told whether it is
/// still the same layout: the nodes moved by hand stay then); then the
/// pointer is read (`interact`), and a double click on the background sets
/// it up afresh, its nodes back in place (as `ask_reset` does).
pub fn view(
    ctx: &egui::Context,
    kept: Option<egui::Id>,
    area: Rect,
    laid: &Laid,
    key: u64,
    controls: &Controls,
    setup: &dyn Fn(&mut ViewState, bool),
) -> (ViewState, Picks) {
    let mut state: ViewState = kept
        .and_then(|id| ctx.data(|d| d.get_temp(id)))
        .unwrap_or_default();
    let this = laid as *const Laid as usize;
    if state.key != Some(key) || state.scale <= 0.0 {
        let same = state.scale > 0.0 && state.laid == this;
        if !same {
            state.moved.clear();
        }
        setup(&mut state, same);
        state.key = Some(key);
        state.laid = this;
    }
    let mut picks = if kept.is_some() {
        interact(ctx, area, laid, &mut state, controls, &|_| true)
    } else {
        Picks::default()
    };
    picks.reset |= kept.is_some_and(|id| take_reset(ctx, id));
    if picks.reset {
        state.moved.clear();
        setup(&mut state, false);
    }
    if let Some(id) = kept {
        ctx.data_mut(|d| d.insert_temp(id, state.clone()));
    }
    (state, picks)
}

/// The centre that shows `laid` from its start (its top-left) at `scale` in `area`, centred where it fits.
pub fn start(laid: &Laid, area: Rect, scale: f32) -> Vec2 {
    let half = area.size() / scale / 2.0;
    let b = laid.bounds;
    vec2(
        keep_in(b.min.x, b.min.x, b.max.x, half.x),
        keep_in(b.min.y, b.min.y, b.max.y, half.y),
    )
}

/// The centre that shows `nodes` at `scale` in `area`, as near the middle as the graph's edges allow (none: no nodes).
pub fn centre_on(laid: &Laid, area: Rect, scale: f32, nodes: &[usize]) -> Option<Vec2> {
    let half = area.size() / scale / 2.0;
    let b = laid.bounds;
    let r = nodes
        .iter()
        .map(|&i| laid.rects[i])
        .reduce(|a, r| a.union(r))?;
    Some(vec2(
        keep_in(r.center().x, b.min.x, b.max.x, half.x),
        keep_in(r.center().y, b.min.y, b.max.y, half.y),
    ))
}

/// Keeps a view on its graph: its centre no further out than half the view past the graph's edges.
pub fn keep(state: &mut ViewState, laid: &Laid, area: Rect) {
    let half = area.size() / state.scale.max(1e-3) / 2.0;
    let b = laid.bounds;
    state.centre = vec2(
        keep_in(state.centre.x, b.min.x - half.x, b.max.x + half.x, half.x),
        keep_in(state.centre.y, b.min.y - half.y, b.max.y + half.y, half.y),
    );
}

/// `c` between `a + h` and `z - h`; the middle where those cross.
fn keep_in(c: f32, a: f32, z: f32, h: f32) -> f32 {
    if z - a <= 2.0 * h {
        (a + z) / 2.0
    } else {
        c.clamp(a + h, z - h)
    }
}

/// The scales a panel's view zooms between: out until the whole graph fits
/// `area` (or the readable `scale`, if that is smaller), in to four times `scale`.
pub fn zoom_range(laid: &Laid, area: Rect, scale: f32) -> (f32, f32) {
    let size = laid.bounds.size();
    let fit = (area.width() / size.x.max(1e-3)).min(area.height() / size.y.max(1e-3));
    (fit.min(scale), scale * 4.0)
}

/// What a view lets the pointer do: zoom with the wheel between two scales
/// (else the wheel pans along the flow), and move nodes by dragging them.
#[derive(Clone, Copy, Debug, Default)]
pub struct Controls {
    pub zoom: Option<(f32, f32)>,
    pub move_nodes: bool,
}

/// What the pointer did over a view this frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct Picks {
    pub hovered: Option<usize>,
    /// A click (not a drag) on a node, or on nothing.
    pub clicked: Option<usize>,
    pub clicked_empty: bool,
    /// A double click on the background.
    pub reset: bool,
    pub inside: bool,
}

/// The topmost node under `p` (screen) in the view.
pub fn hit(
    laid: &Laid,
    state: &ViewState,
    area: Rect,
    p: Pos2,
    shown: &dyn Fn(usize) -> bool,
) -> Option<usize> {
    let t = state.transform(area);
    (0..laid.rects.len())
        .rev()
        .filter(|&i| shown(i))
        .find(|&i| t.rect(placed(laid, &state.moved, i)).contains(p))
}

/// Reads the pointer over `area`: the wheel zooms about the pointer (or
/// pans), a drag on the background pans, a drag on a node moves it, and a
/// double click on the background asks for the view to be reset (the
/// adopter does it: it knows the view it wants).
pub fn interact(
    ctx: &egui::Context,
    area: Rect,
    laid: &Laid,
    state: &mut ViewState,
    controls: &Controls,
    shown: &dyn Fn(usize) -> bool,
) -> Picks {
    let (pointer, pressed, down, delta, scroll, clicked, double) = ctx.input(|i| {
        (
            i.pointer.hover_pos(),
            i.pointer.primary_pressed(),
            i.pointer.primary_down(),
            i.pointer.delta(),
            i.smooth_scroll_delta,
            i.pointer.primary_clicked(),
            i.pointer
                .button_double_clicked(egui::PointerButton::Primary),
        )
    });
    let inside = pointer.filter(|p| area.contains(*p));
    let mut picks = Picks {
        inside: inside.is_some(),
        ..Default::default()
    };
    if let Some(p) = inside {
        match controls.zoom {
            Some((lo, hi)) if scroll.y != 0.0 => {
                // About the point under the pointer, which stays where it is.
                let under = state.centre + (p - area.center()) / state.scale;
                state.scale = (state.scale * (scroll.y / 120.0).exp()).clamp(lo, hi);
                state.centre = under - (p - area.center()) / state.scale;
            }
            Some(_) => {}
            None => {
                let along = match laid.flow {
                    Flow::Right => vec2(scroll.x + scroll.y, 0.0),
                    Flow::Down => vec2(scroll.x, scroll.y),
                };
                state.centre -= along / state.scale.max(1e-3);
            }
        }
        picks.hovered = hit(laid, state, area, p, shown);
        if pressed {
            state.drag = Some(if controls.move_nodes {
                picks.hovered
            } else {
                None
            });
        }
        if double && picks.hovered.is_none() {
            picks.reset = true;
        }
        if clicked {
            picks.clicked = picks.hovered;
            picks.clicked_empty = picks.hovered.is_none();
        }
    }
    if !down {
        state.drag = None;
    }
    match state.drag {
        Some(None) => state.centre -= delta / state.scale.max(1e-3),
        Some(Some(i)) => match state.moved.iter_mut().find(|m| m.0 == i) {
            Some(m) => m.1 += delta / state.scale.max(1e-3),
            None => state.moved.push((i, delta / state.scale.max(1e-3))),
        },
        None => {}
    }
    picks
}

#[cfg(test)]
mod tests;
