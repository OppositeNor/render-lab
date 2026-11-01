//! A story's diagrams: a small subset of Mermaid's flowchart syntax, laid out
//! (src/layered.rs) and drawn by the lab in a 2D scene's panel, with the beat's
//! `focus` nodes lit.
//!
//! The subset: a first line `flowchart LR` (or `graph LR`); nodes `ID`,
//! `ID["label"]` (also `[…]`, `(…)`, `{…}`, all drawn as boxes; `<br/>` breaks
//! a line); chains of edges `-->`, `==>`, dotted `-.->`, each with an optional
//! label (`-->|text|`, `-- text -->`, `-. text .->`); `style ID
//! stroke-dasharray: …` (a dashed box); `%%` comments. An edge that closes a
//! cycle is drawn as an arc back under (or over) the columns.

use crate::palette;
use crate::text::Ink;
use egui::{pos2, vec2, Align2, CornerRadius, FontFamily, Pos2, Rect, Shape, Stroke, StrokeKind};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// The largest size of a diagram's labels (px at 1080p): a small diagram is
/// not blown up beyond it.
const MAX_FONT: f32 = 33.75;
/// The margin around a diagram inside its panel (px at 1080p).
const MARGIN: f32 = 35.0;
/// A label line's height, a box's padding across and down (font units).
const LINE: f32 = 1.32;
const PAD_X: f32 = 1.2;
const PAD_Y: f32 = 0.8;

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub id: String,
    pub lines: Vec<String>,
    pub dashed: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Edge {
    pub from: usize,
    pub to: usize,
    pub dotted: bool,
    pub label: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Diagram {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

/// What a 2D beat shows of its diagram: the source and its lit nodes.
#[derive(Clone, Copy, Debug)]
pub struct BeatDiagram<'a> {
    pub source: &'a str,
    pub focus: &'a [String],
}

fn label_lines(text: &str) -> Vec<String> {
    let text = text.trim();
    let text = text.strip_prefix('"').and_then(|t| t.strip_suffix('"')).unwrap_or(text);
    text.replace("<br/>", "\n").replace("<br />", "\n").replace("<br>", "\n").lines().map(|l| l.trim().to_string()).collect()
}

struct Cursor<'a> {
    rest: &'a str,
}

impl<'a> Cursor<'a> {
    fn skip_space(&mut self) {
        self.rest = self.rest.trim_start();
    }

    fn eat(&mut self, token: &str) -> bool {
        if let Some(r) = self.rest.strip_prefix(token) {
            self.rest = r;
            true
        } else {
            false
        }
    }

    /// The text up to `end` (consumed with it).
    fn until(&mut self, end: &str) -> Option<&'a str> {
        let at = self.rest.find(end)?;
        let text = &self.rest[..at];
        self.rest = &self.rest[at + end.len()..];
        Some(text)
    }

    fn id(&mut self) -> Option<&'a str> {
        let len = self.rest.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).unwrap_or(self.rest.len());
        (len > 0).then(|| {
            let id = &self.rest[..len];
            self.rest = &self.rest[len..];
            id
        })
    }

    /// A node: its id and its label, if one is given here.
    fn node(&mut self) -> Result<(&'a str, Option<Vec<String>>), String> {
        self.skip_space();
        let id = self.id().ok_or_else(|| format!("expected a node id at `{}`", self.rest))?;
        for (open, close) in [("[", "]"), ("(", ")"), ("{", "}")] {
            if self.eat(open) {
                let text = if self.rest.starts_with('"') {
                    self.eat("\"");
                    let t = self.until("\"").ok_or("an unclosed quote")?;
                    if !self.eat(close) {
                        return Err(format!("expected `{close}` after the label of {id}"));
                    }
                    t
                } else {
                    self.until(close).ok_or_else(|| format!("an unclosed `{open}` after {id}"))?
                };
                return Ok((id, Some(label_lines(text))));
            }
        }
        Ok((id, None))
    }

    /// An edge: dotted, and its label.
    fn edge(&mut self) -> Result<(bool, Option<String>), String> {
        self.skip_space();
        let (dotted, mut label) = if self.eat("-->") || self.eat("==>") {
            (false, None)
        } else if self.eat("-.->") {
            (true, None)
        } else if self.eat("-.") {
            (true, Some(self.until(".->").ok_or("a `-.` edge without its `.->`")?))
        } else if self.eat("--") {
            (false, Some(self.until("-->").ok_or("a `--` edge without its `-->`")?))
        } else {
            return Err(format!("expected an edge at `{}`", self.rest));
        };
        if label.is_none() && self.eat("|") {
            label = Some(self.until("|").ok_or("an unclosed `|` label")?);
        }
        Ok((dotted, label.map(|l| label_lines(l).join(" ")).filter(|l| !l.is_empty())))
    }
}

impl Diagram {
    pub fn parse(source: &str) -> Result<Diagram, String> {
        let mut d = Diagram::default();
        let mut index: HashMap<String, usize> = HashMap::new();
        let mut header = false;
        let mut node = |d: &mut Diagram, id: &str, lines: Option<Vec<String>>| -> usize {
            let i = *index.entry(id.to_string()).or_insert_with(|| {
                d.nodes.push(Node { id: id.to_string(), lines: vec![id.to_string()], dashed: false });
                d.nodes.len() - 1
            });
            if let Some(lines) = lines {
                d.nodes[i].lines = lines;
            }
            i
        };
        for (n, raw) in source.lines().enumerate() {
            let line = raw.trim();
            let at = |e: String| format!("line {}: {e}", n + 1);
            if line.is_empty() || line.starts_with("%%") {
                continue;
            }
            if !header {
                let words: Vec<&str> = line.split_whitespace().collect();
                if !matches!(words.as_slice(), ["flowchart" | "graph", "LR"]) {
                    return Err(at(format!("a diagram starts with `flowchart LR`, not `{line}`")));
                }
                header = true;
                continue;
            }
            if let Some(rest) = line.strip_prefix("style ") {
                let (id, style) = rest.trim().split_once(char::is_whitespace).ok_or_else(|| at("a style without properties".into()))?;
                if !style.contains("stroke-dasharray") {
                    return Err(at(format!("unsupported style `{style}` (only stroke-dasharray)")));
                }
                let i = node(&mut d, id, None);
                d.nodes[i].dashed = true;
                continue;
            }
            let mut c = Cursor { rest: line };
            let (id, lines) = c.node().map_err(at)?;
            let mut from = node(&mut d, id, lines);
            loop {
                c.skip_space();
                if c.rest.is_empty() || c.eat(";") {
                    break;
                }
                let (dotted, label) = c.edge().map_err(at)?;
                let (id, lines) = c.node().map_err(at)?;
                let to = node(&mut d, id, lines);
                d.edges.push(Edge { from, to, dotted, label });
                from = to;
            }
        }
        if d.nodes.is_empty() {
            return Err("a diagram without nodes".into());
        }
        Ok(d)
    }

    /// Problems with a beat's `focus`: names no node has.
    pub fn check_focus(&self, focus: &[String]) -> Result<(), String> {
        match focus.iter().find(|f| !self.nodes.iter().any(|n| &n.id == *f)) {
            Some(f) => Err(format!("unknown focus node {f}")),
            None => Ok(()),
        }
    }

    /// Per edge, whether it closes a cycle with the edges written before it
    /// (the edge back, as written): such an edge is laid out reversed and
    /// drawn as an arc back.
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

/// A diagram laid out in font units (a label's font is 1 high).
pub struct Laid {
    pub diagram: Diagram,
    layout: crate::layered::Layout,
    back: Vec<bool>,
    /// Each back edge's arc: below the columns (or above), and its depth.
    arcs: Vec<(bool, f32)>,
    /// Everything drawn: min and max corners.
    bounds: (Pos2, Pos2),
}

/// A diagram's layout, computed once per source. `width` measures a line of
/// text in font units.
pub fn laid_out(source: &str, width: &dyn Fn(&str) -> f32) -> Result<&'static Laid, String> {
    static CACHE: OnceLock<Mutex<HashMap<String, Result<&'static Laid, String>>>> = OnceLock::new();
    let mut cache = CACHE.get_or_init(Default::default).lock().unwrap();
    cache
        .entry(source.to_string())
        .or_insert_with(|| {
            let diagram = Diagram::parse(source)?;
            Ok(Box::leak(Box::new(lay_out(diagram, width))))
        })
        .clone()
}

fn lay_out(diagram: Diagram, width: &dyn Fn(&str) -> f32) -> Laid {
    let back = diagram.back_edges();
    let sizes: Vec<(f32, f32)> = diagram.nodes.iter().map(|n| (n.lines.iter().map(|l| width(l)).fold(0.0, f32::max) + PAD_X, n.lines.len() as f32 * LINE + PAD_Y)).collect();
    let edges: Vec<(usize, usize)> = diagram.edges.iter().zip(&back).map(|(e, &b)| if b { (e.to, e.from) } else { (e.from, e.to) }).collect();
    // A forward edge's label sits between its columns.
    let label_room = diagram.edges.iter().zip(&back).filter(|(_, b)| !**b).filter_map(|(e, _)| e.label.as_deref()).map(|l| width(l) + 1.0).fold(0.0, f32::max);
    let order: Vec<usize> = (0..diagram.nodes.len()).collect();
    let layout = crate::layered::layout(&sizes, &edges, &order, crate::layered::Spacing { column: label_room.max(1.7), row: 0.7, dummy: 0.5 });
    let node_rect = |i: usize| Rect::from_center_size(pos2(layout.positions[i].0, layout.positions[i].1), vec2(layout.widths[i], layout.heights[i]));
    let mut min = Pos2::ZERO;
    let mut max = pos2(layout.size.0, layout.size.1);
    let mut arcs = vec![(true, 0.0); diagram.edges.len()];
    for (e, edge) in diagram.edges.iter().enumerate().filter(|(e, _)| back[*e]) {
        let (a, b) = (node_rect(edge.from), node_rect(edge.to));
        // Toward the nearer side of the columns.
        let below = (a.center().y + b.center().y) / 2.0 >= layout.size.1 / 2.0;
        let depth = 1.6;
        let label = edge.label.as_deref().map(|l| width(l)).unwrap_or(0.0);
        let x = (a.center().x + b.center().x) / 2.0;
        if below {
            let y = a.max.y.max(b.max.y) + depth * 0.75;
            max.y = max.y.max(y + if label > 0.0 { LINE } else { 0.2 });
        } else {
            let y = a.min.y.min(b.min.y) - depth * 0.75;
            min.y = min.y.min(y - if label > 0.0 { LINE } else { 0.2 });
        }
        min.x = min.x.min(x - label / 2.0 - 0.4);
        max.x = max.x.max(x + label / 2.0 + 0.4);
        arcs[e] = (below, depth);
    }
    Laid { diagram, layout, back, arcs, bounds: (min, max) }
}

/// The points of a cubic Bézier curve.
fn cubic(p: [Pos2; 4], out: &mut Vec<Pos2>) {
    for k in 1..=16 {
        let t = k as f32 / 16.0;
        let u = 1.0 - t;
        out.push((p[0].to_vec2() * (u * u * u) + p[1].to_vec2() * (3.0 * u * u * t) + p[2].to_vec2() * (3.0 * u * t * t) + p[3].to_vec2() * (t * t * t)).to_pos2());
    }
}

/// A line, solid or dashed, ending in an arrowhead along its last segment.
fn arrow(painter: &egui::Painter, points: Vec<Pos2>, dotted: bool, stroke: Stroke, tip: f32) {
    let n = points.len();
    let (end, before) = (points[n - 1], points[n.saturating_sub(4)]);
    let dir = (end - before).normalized();
    let normal = vec2(-dir.y, dir.x);
    let base = end - dir * tip * 1.6;
    let mut line = points;
    *line.last_mut().unwrap() = base;
    if dotted {
        painter.extend(Shape::dashed_line(&line, stroke, tip * 0.9, tip * 0.7));
    } else {
        painter.add(Shape::line(line, stroke));
    }
    painter.add(Shape::convex_polygon(vec![end, base + normal * tip * 0.8, base - normal * tip * 0.8], stroke.color, Stroke::NONE));
}

/// Draws `beat`'s diagram in its panel `area`: fitted (its labels at most
/// `MAX_FONT` high), or, with `picture`, scaled into that rectangle (the
/// panel's area zoomed and panned). Labels go through `ink`, whose smallest
/// size the recorder checks against the lab's minimum.
pub fn draw(ink: &Ink, area: Rect, picture: Option<Rect>, beat: BeatDiagram) {
    let painter = ink.painter;
    painter.rect_filled(area, CornerRadius::same(12), palette::panel());
    let width = |s: &str| ink.measure(s, 100.0, FontFamily::Proportional).x / 100.0;
    let laid = match laid_out(beat.source, &width) {
        Ok(l) => l,
        Err(e) => {
            ink.text(area.center(), Align2::CENTER_CENTER, format!("diagram: {e}"), 24.0, FontFamily::Proportional, palette::MISMATCH);
            return;
        }
    };
    let (lo, hi) = laid.bounds;
    let size = hi - lo;
    let fit = ((area.width() - 2.0 * MARGIN) / size.x).min((area.height() - 2.0 * MARGIN) / size.y).min(MAX_FONT);
    let picture = picture.unwrap_or(area);
    let zoom = picture.width() / area.width();
    let scale = fit * zoom;
    let origin = picture.center() - (lo.to_vec2() + size / 2.0) * scale;
    let at = |p: (f32, f32)| origin + vec2(p.0, p.1) * scale;
    let clipped = painter.with_clip_rect(area);
    let d = &laid.diagram;
    let l = &laid.layout;
    let rect = |i: usize| Rect::from_center_size(at(l.positions[i]), vec2(l.widths[i], l.heights[i]) * scale);
    // A column's width, from the nodes centred on it (a column of only edges: none).
    let column_width = |x: f32| (0..d.nodes.len()).filter(|&i| (l.positions[i].0 - x).abs() < 1e-3).map(|i| l.widths[i]).fold(0.0, f32::max);
    let edge_color = palette::text_dim();
    let stroke = Stroke::new(1.8 * zoom.min(2.0), edge_color);
    let tip = (scale * 0.3).clamp(5.0, 14.0);
    let draw_ink = Ink::new(&clipped);
    let edge_label = |text: &str, centre: Pos2| {
        let font = scale;
        let w = draw_ink.measure(text, font, FontFamily::Proportional);
        clipped.rect_filled(Rect::from_center_size(centre, w + vec2(font * 0.5, font * 0.2)), CornerRadius::same(4), palette::panel());
        draw_ink.text(centre, Align2::CENTER_CENTER, text, font, FontFamily::Proportional, palette::text_dim());
    };
    for (e, edge) in d.edges.iter().enumerate() {
        let (a, b) = (rect(edge.from), rect(edge.to));
        let mut points = Vec::new();
        let label_at;
        if laid.back[e] {
            let (below, depth) = laid.arcs[e];
            let (p0, p3, push) = if below { (a.center_bottom(), b.center_bottom(), depth) } else { (a.center_top(), b.center_top(), -depth) };
            let y = if below { p0.y.max(p3.y) } else { p0.y.min(p3.y) } + push * scale;
            points.push(p0);
            cubic([p0, pos2(p0.x, y), pos2(p3.x, y), p3], &mut points);
            label_at = pos2((p0.x + p3.x) / 2.0, if below { p0.y.max(p3.y) } else { p0.y.min(p3.y) } + push * 0.75 * scale + if below { 0.55 } else { -0.55 } * scale);
        } else {
            // Through a column it skips, the edge runs straight across the column.
            let mut stops = vec![a.right_center()];
            for &p in l.bends.iter().find(|x| x.0 == edge.from && x.1 == edge.to).map(|x| x.2.as_slice()).unwrap_or_default() {
                let half = column_width(p.0) / 2.0;
                stops.push(at((p.0 - half, p.1)));
                stops.push(at((p.0 + half, p.1)));
            }
            stops.push(b.left_center());
            points.push(stops[0]);
            for (k, w) in stops.windows(2).enumerate() {
                if k % 2 == 1 {
                    points.push(w[1]);
                    continue;
                }
                let dx = (w[1].x - w[0].x) / 2.0;
                cubic([w[0], w[0] + vec2(dx, 0.0), w[1] - vec2(dx, 0.0), w[1]], &mut points);
            }
            label_at = points[points.len() / 2];
        }
        arrow(&clipped, points, edge.dotted, stroke, tip);
        if let Some(text) = &edge.label {
            edge_label(text, label_at);
        }
    }
    let line = scale * LINE;
    for (i, node) in d.nodes.iter().enumerate() {
        let r = rect(i);
        let lit = beat.focus.iter().any(|f| *f == node.id);
        let radius = CornerRadius::same((scale * 0.3).min(255.0) as u8);
        let (fill, outline) = if lit {
            (palette::accent().gamma_multiply(0.35), Stroke::new(4.0 * zoom.min(2.0), palette::accent()))
        } else {
            (palette::panel_raised(), Stroke::new(1.5 * zoom.min(2.0), palette::text_faint()))
        };
        clipped.rect_filled(r, radius, palette::panel_raised());
        clipped.rect_filled(r, radius, fill);
        if node.dashed && !lit {
            let corners = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
            clipped.extend(Shape::dashed_line(&corners, outline, scale * 0.25, scale * 0.2));
        } else {
            clipped.rect_stroke(r, radius, outline, StrokeKind::Inside);
        }
        let top = r.center().y - line * node.lines.len() as f32 / 2.0;
        for (k, text_line) in node.lines.iter().enumerate() {
            draw_ink.text(pos2(r.center().x, top + line * (k as f32 + 0.5)), Align2::CENTER_CENTER, text_line, scale, FontFamily::Proportional, palette::text());
        }
    }
    ink.min_size.set(ink.min_size.get().min(draw_ink.min_size.get()));
    painter.rect_stroke(area, CornerRadius::same(12), Stroke::new(1.0, palette::border()), StrokeKind::Inside);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_subset() {
        let d = Diagram::parse(
            "flowchart LR\n  A[\"One<br/>two\"] --> B[Three]\n  %% a comment\n  B -. \"back\" .-> A\n  B -->|\"on\"| C\n  C -- across --> D\n  B -.-> D\n  E[\"Alone\"]\n  style E stroke-dasharray: 5 5\n",
        )
        .unwrap();
        assert_eq!(d.nodes.iter().map(|n| n.id.as_str()).collect::<Vec<_>>(), ["A", "B", "C", "D", "E"]);
        assert_eq!(d.nodes[0].lines, ["One", "two"]);
        assert_eq!(d.nodes[2].lines, ["C"]);
        assert!(d.nodes[4].dashed);
        let labels: Vec<_> = d.edges.iter().map(|e| (e.from, e.to, e.dotted, e.label.as_deref())).collect();
        assert_eq!(labels, [(0, 1, false, None), (1, 0, true, Some("back")), (1, 2, false, Some("on")), (2, 3, false, Some("across")), (1, 3, true, None)]);
        assert_eq!(d.back_edges(), [false, true, false, false, false]);
        assert!(d.check_focus(&["A".into(), "E".into()]).is_ok());
        assert!(d.check_focus(&["Z".into()]).is_err());
    }

    #[test]
    fn rejects_what_it_does_not_draw() {
        assert!(Diagram::parse("flowchart TD\nA --> B").is_err());
        assert!(Diagram::parse("flowchart LR\nA --> ").is_err());
        assert!(Diagram::parse("flowchart LR\nA[\"open --> B").is_err());
        assert!(Diagram::parse("flowchart LR\nstyle A fill:#fff").is_err());
    }

    #[test]
    fn lays_out_a_cycle_left_to_right() {
        let laid = lay_out(Diagram::parse("flowchart LR\nA --> B\nB --> C\nC -->|again| B").unwrap(), &|s| s.len() as f32 * 0.6);
        let x = |i: usize| laid.layout.positions[i].0;
        assert!(x(0) < x(1) && x(1) < x(2));
        assert_eq!(laid.back, [false, false, true]);
        // The edge written last closes the cycle, wherever a walk would meet it.
        let d = Diagram::parse("flowchart LR\nS --> T\nC --> L\nL --> T\nT -->|r| L").unwrap();
        assert_eq!(d.back_edges(), [false, false, false, true]);
    }
}
