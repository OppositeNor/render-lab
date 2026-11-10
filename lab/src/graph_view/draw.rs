//! Drawing a laid-out graph: its boxes, edges, and nodes, as a look says.

use super::*;

/// How a laid-out graph is drawn: each node, edge, and boxed cluster (its
/// strength: faded when false), the nodes moved by hand (font units), the
/// labels of edges, the corner radius of boxes, and whether text goes
/// through `Ink` (whose smallest size the recorder checks) or straight to the
/// painter (a zoomable view, whose text may be small).
pub struct Look<'a> {
    pub nodes: Vec<NodeLook>,
    pub edges: Vec<EdgeLook>,
    pub clusters: Vec<bool>,
    pub moved: &'a [(usize, Vec2)],
    pub edge_labels: bool,
    pub tracked: bool,
}

/// A node's rectangle in graph units, moved by hand.
pub fn placed(laid: &Laid, moved: &[(usize, Vec2)], i: usize) -> Rect {
    laid.rects[i].translate(moved.iter().find(|m| m.0 == i).map_or(Vec2::ZERO, |m| m.1))
}

/// A line ending in an arrowhead along its last segment, solid or dashed.
pub(super) fn arrow(
    painter: &egui::Painter,
    points: Vec<Pos2>,
    dotted: bool,
    stroke: Stroke,
    tip: f32,
) {
    let n = points.len();
    if n < 2 {
        return;
    }
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
    painter.add(Shape::convex_polygon(
        vec![end, base + normal * tip * 0.8, base - normal * tip * 0.8],
        stroke.color,
        Stroke::NONE,
    ));
}

/// Draws `graph`, laid out as `laid`, through `t`, clipped to `clip`, as `look` says.
pub fn draw(ink: &Ink, clip: Rect, t: &Transform, graph: &Graph, laid: &Laid, look: &Look) {
    let painter = ink.painter.with_clip_rect(clip);
    let draw_ink = Ink::new(&painter);
    let p = Painting {
        painter: &painter,
        ink: &draw_ink,
        clip,
        t,
        graph,
        laid,
        look,
        scale: t.scale,
    };
    p.boxes();
    p.edges();
    for i in 0..graph.nodes.len() {
        p.node(i);
    }
    ink.min_size
        .set(ink.min_size.get().min(draw_ink.min_size.get()));
}

/// What `draw` paints with: the painter clipped to `clip`, the ink on it, and the graph, its layout, and its look
/// through `t` (`scale` pixels per unit).
pub(super) struct Painting<'a> {
    painter: &'a egui::Painter,
    ink: &'a Ink<'a>,
    clip: Rect,
    t: &'a Transform,
    graph: &'a Graph,
    laid: &'a Laid,
    look: &'a Look<'a>,
    scale: f32,
}

impl Painting<'_> {
    /// A text, through the ink when the look is tracked; none below 8 pixels.
    fn text(
        &self,
        at: Pos2,
        align: Align2,
        s: &str,
        size: f32,
        family: FontFamily,
        color: Color32,
    ) {
        if size < 8.0 {
            return;
        }
        if self.look.tracked {
            self.ink.text(at, align, s, size, family, color);
        } else {
            self.painter
                .text(at, align, s, egui::FontId::new(size, family), color);
        }
    }

    /// The clusters' boxes, their titles in their top bands.
    fn boxes(&self) {
        let (painter, scale) = (self.painter, self.scale);
        for &(c, b) in &self.laid.boxes {
            // A cluster's box is drawn where it meets the clip, faded when the look does not light its cluster.
            let b = self.t.rect(b);
            if !b.intersects(self.clip) {
                continue;
            }
            let strong = self.look.clusters.get(c).copied().unwrap_or(true);
            let fade = |c: Color32| {
                if strong {
                    c
                } else {
                    c.gamma_multiply(0.3)
                }
            };
            let radius = CornerRadius::same((scale * 0.6).min(255.0) as u8);
            painter.rect_filled(b, radius, fade(palette::background()).gamma_multiply(0.6));
            painter.rect_stroke(
                b,
                radius,
                Stroke::new(1.5, fade(palette::border())),
                StrokeKind::Inside,
            );
            if let Some(title) = &self.graph.clusters[c].title {
                // A view whose text is checked keeps it at full size; a zoomable one a little smaller.
                let size = if self.look.tracked {
                    scale
                } else {
                    scale * 0.84
                };
                self.text(
                    b.min + vec2(scale * 0.6, scale * 0.2),
                    Align2::LEFT_TOP,
                    title,
                    size,
                    FontFamily::Proportional,
                    fade(palette::text_dim()),
                );
            }
        }
    }

    /// The edges, as far as each is drawn, and their labels.
    fn edges(&self) {
        let (look, laid, t, scale) = (self.look, self.laid, self.t, self.scale);
        let tip = (scale * 0.3).clamp(5.0, 14.0);
        for (e, edge) in self.graph.edges.iter().enumerate() {
            // An edge is skipped while hidden, not yet drawn, or joining a hidden node; else drawn as far as `drawn`.
            let l = look.edges[e];
            if l.hidden
                || l.drawn <= 0.0
                || look.nodes[edge.from].hidden
                || look.nodes[edge.to].hidden
            {
                continue;
            }
            let route = &laid.routes[e];
            let moved = look
                .moved
                .iter()
                .any(|m| m.0 == edge.from || m.0 == edge.to);
            // A moved node's edges run straight to it, as curves.
            let points: Vec<Pos2> = if moved && !route.back {
                let (a, b) = ends(
                    placed(laid, look.moved, edge.from),
                    placed(laid, look.moved, edge.to),
                    laid.flow,
                );
                curve(a, b, laid.flow)
                    .into_iter()
                    .map(|p| t.at(p))
                    .collect()
            } else {
                route.points.iter().map(|&p| t.at(p)).collect()
            };
            let label_at = t.at(route.label_at);
            arrow(
                self.painter,
                prefix(&points, l.drawn),
                edge.dotted,
                Stroke::new(l.width, l.color),
                tip,
            );
            if let (true, Some(label), true) = (look.edge_labels, &edge.label, l.drawn >= 1.0) {
                // A fully drawn edge's label sits on a chip at its label point.
                let w = self.ink.measure(label, scale, FontFamily::Proportional);
                self.painter.rect_filled(
                    Rect::from_center_size(label_at, w + vec2(scale * 0.5, scale * 0.2)),
                    CornerRadius::same(4),
                    palette::panel(),
                );
                self.text(
                    label_at,
                    Align2::CENTER_CENTER,
                    label,
                    scale,
                    FontFamily::Proportional,
                    palette::text_dim(),
                );
            }
        }
    }

    /// Node `i`: its fill, its contents (a card, a title alone, or a group), and its outline.
    fn node(&self, i: usize) {
        let (painter, scale) = (self.painter, self.scale);
        let node = &self.graph.nodes[i];
        let faded;
        let mut l = &self.look.nodes[i];
        let r = self.t.rect(placed(self.laid, self.look.moved, i));
        if l.hidden || l.alpha <= 0.0 || !r.intersects(self.clip) {
            return;
        }
        if l.alpha < 1.0 {
            // Partly shown: every colour of it at that strength.
            let a = l.alpha;
            faded = NodeLook {
                fill: l.fill.gamma_multiply(a),
                outline: Stroke::new(l.outline.width, l.outline.color.gamma_multiply(a)),
                text: l.text.gamma_multiply(a),
                rows: l
                    .rows
                    .iter()
                    .map(|(c, lit)| (c.gamma_multiply(a), *lit))
                    .collect(),
                tint: l.tint.gamma_multiply(a),
                ..l.clone()
            };
            l = &faded;
        }
        let under = if l.alpha < 1.0 {
            palette::panel_raised().gamma_multiply(l.alpha)
        } else {
            palette::panel_raised()
        };
        let radius = CornerRadius::same((scale * 0.3).min(255.0) as u8);
        painter.rect_filled(r, radius, under);
        painter.rect_filled(r, radius, l.fill);
        if node.card.is_some() {
            self.card(node, l, r);
        } else if node.rows.is_empty() && self.laid.pictures[i] == 0.0 {
            // A node with neither rows nor a picture: its title lines, centred in the box.
            let line = scale * LINE;
            let top = r.center().y - line * node.title.len() as f32 / 2.0;
            for (k, title) in node.title.iter().enumerate() {
                self.text(
                    pos2(r.center().x, top + line * (k as f32 + 0.5)),
                    Align2::CENTER_CENTER,
                    title,
                    scale,
                    FontFamily::Proportional,
                    l.text,
                );
            }
        } else {
            self.group(node, l, r, self.laid.pictures[i]);
        }
        if node.dashed {
            // A dashed node's outline is dashes around its four sides.
            let corners = [
                r.left_top(),
                r.right_top(),
                r.right_bottom(),
                r.left_bottom(),
                r.left_top(),
            ];
            painter.extend(Shape::dashed_line(
                &corners,
                l.outline,
                scale * 0.25,
                scale * 0.2,
            ));
        } else {
            painter.rect_stroke(r, radius, l.outline, StrokeKind::Inside);
        }
    }

    /// A card in `r`: its title on a line, its picture under it, its badge in the picture's corner.
    fn card(&self, node: &Node, l: &NodeLook, r: Rect) {
        let scale = self.scale;
        let label = scale * 1.33;
        let inner = r.shrink2(vec2(scale * 0.33, 0.0));
        self.text(
            pos2(inner.min.x, r.min.y + label / 2.0),
            Align2::LEFT_CENTER,
            &self.ink.fit_right(
                &node.title.join(" "),
                inner.width(),
                scale,
                FontFamily::Proportional,
            ),
            scale,
            FontFamily::Proportional,
            l.text,
        );
        // Its rows under the picture, a line each.
        let rows = node.rows.len() as f32 * ROW * scale;
        let room = Rect::from_min_max(
            pos2(r.min.x + scale * 0.17, r.min.y + label),
            pos2(r.max.x - scale * 0.17, r.max.y - scale * 0.17 - rows),
        );
        for (k, row) in node.rows.iter().enumerate() {
            // The card's rows sit along its bottom, each in its look's colour.
            let (color, _) = l
                .rows
                .get(k)
                .copied()
                .unwrap_or((palette::text_dim(), false));
            let at = pos2(
                inner.min.x,
                r.max.y - rows + (k as f32 + 0.5) * ROW * scale - scale * 0.08,
            );
            self.text(
                at,
                Align2::LEFT_CENTER,
                &self
                    .ink
                    .fit_right(&row.label, inner.width(), scale, FontFamily::Proportional),
                scale,
                FontFamily::Proportional,
                color,
            );
        }
        match l.picture {
            Some(p) => picture(self.painter, room, p, l.tint, false),
            None => self.text(
                room.center(),
                Align2::CENTER_CENTER,
                l.note.as_deref().unwrap_or("–"),
                scale,
                FontFamily::Proportional,
                palette::text_faint(),
            ),
        }
        if let Some(badge) = &l.badge {
            // The badge goes in the picture's bottom-right corner, if it fits in 90% of its width.
            let size = self.ink.measure(badge, scale, FontFamily::Monospace)
                + vec2(scale * 0.4, scale * 0.08);
            let tag = Rect::from_min_size(room.max - size, size);
            if tag.width() < room.width() * 0.9 {
                self.painter.rect_filled(
                    tag,
                    CornerRadius::same(4),
                    palette::panel().gamma_multiply(0.85),
                );
                self.text(
                    tag.center(),
                    Align2::CENTER_CENTER,
                    badge,
                    scale,
                    FontFamily::Monospace,
                    palette::text_dim(),
                );
            }
        }
    }

    /// A group in `r`: its title in bold, a rule, its rows, its picture (`picture` units high; 0: none).
    fn group(&self, node: &Node, l: &NodeLook, r: Rect, picture_height: f32) {
        let (painter, scale) = (self.painter, self.scale);
        let line = scale * LINE;
        let mut y = r.min.y + PAD_Y / 2.0 * scale;
        for title in &node.title {
            self.text(
                pos2(r.center().x, y + line / 2.0),
                Align2::CENTER_CENTER,
                title,
                scale,
                crate::text::bold(),
                l.text,
            );
            y += line;
        }
        if !node.rows.is_empty() {
            let rule = y + RULE / 2.0 * scale;
            painter.line_segment(
                [
                    pos2(r.min.x + scale * 0.4, rule),
                    pos2(r.max.x - scale * 0.4, rule),
                ],
                Stroke::new(1.0, palette::text_faint()),
            );
            y += RULE * scale;
        }
        for (k, row) in node.rows.iter().enumerate() {
            // Each row is a band; a lit row is filled with the accent behind its text.
            let band = Rect::from_min_max(
                pos2(r.min.x + scale * 0.25, y),
                pos2(r.max.x - scale * 0.25, y + ROW * scale),
            );
            let (color, lit) = l
                .rows
                .get(k)
                .copied()
                .unwrap_or((palette::text_dim(), false));
            if lit {
                painter.rect_filled(
                    band,
                    CornerRadius::same((scale * 0.2) as u8),
                    palette::accent().gamma_multiply(0.45),
                );
            }
            self.text(
                band.center(),
                Align2::CENTER_CENTER,
                &row.label,
                scale,
                FontFamily::Proportional,
                color,
            );
            y += ROW * scale;
        }
        if let (Some(p), true) = (l.picture, picture_height > 0.0) {
            y += RULE * scale;
            let room = Rect::from_min_size(
                pos2(r.min.x + PAD_X / 2.0 * scale, y),
                vec2(r.width() - PAD_X * scale, picture_height * scale),
            );
            picture(painter, room, p, l.tint, true);
        }
    }
}

/// A picture fitted in `room`, its shape kept; framed when `frame`.
pub(super) fn picture(painter: &egui::Painter, room: Rect, p: Picture, tint: Color32, frame: bool) {
    // A picture taller than the room fits its height; a wider one fits its width.
    let shape = p.1[1] as f32 / p.1[0].max(1) as f32;
    let fitted = if shape > room.height() / room.width() {
        Rect::from_center_size(room.center(), vec2(room.height() / shape, room.height()))
    } else {
        Rect::from_center_size(room.center(), vec2(room.width(), room.width() * shape))
    };
    painter.image(
        p.0,
        fitted,
        Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
        tint,
    );
    if frame {
        painter.rect_stroke(
            fitted,
            CornerRadius::ZERO,
            Stroke::new(1.0, palette::text_faint()),
            StrokeKind::Outside,
        );
    }
}
