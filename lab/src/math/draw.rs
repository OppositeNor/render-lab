//! Drawing a math scene's mobs on the canvas, with the UI's painter.

use super::{Canvas, Mob, Part, Scene, rest_mask, typeset_near};
use crate::formula::Formula;
use crate::text::Ink;
use egui::epaint::{Mesh, PathShape, Stroke};
use egui::{Align2, Color32, CornerRadius, FontFamily, Pos2, Rect, StrokeKind, pos2, vec2};

/// Draws `scene` over `rect` (the frame's content area), its frame fitted to the rectangle's height.
pub fn draw(ink: &Ink, rect: Rect, scene: &Scene) {
    let (center, height) = scene.frame();
    let canvas = Canvas::new(rect, center, height);
    let painter = ink.painter.with_clip_rect(rect);
    let clipped = Ink {
        painter: &painter,
        min_size: std::cell::Cell::new(ink.min_size.get()),
    };
    for mob in &scene.mobs {
        draw_mob(&clipped, &canvas, mob);
    }
    // The smallest text drawn on the canvas counts toward the frame's (the selftest's text sizes).
    ink.min_size.set(clipped.min_size.get());
}

fn draw_mob(ink: &Ink, canvas: &Canvas, mob: &Mob) {
    match mob {
        Mob::Tex { .. } => draw_tex(ink, canvas, mob),
        Mob::Text {
            text,
            at,
            size,
            color,
        } => {
            ink.text(
                canvas.to_screen(*at),
                Align2::CENTER_CENTER,
                text,
                size * canvas.scale,
                FontFamily::Proportional,
                *color,
            );
        }
        Mob::Rect {
            lo,
            hi,
            fill,
            stroke,
            width,
            radius,
        } => {
            let r = Rect::from_two_pos(canvas.to_screen(*lo), canvas.to_screen(*hi));
            let corner = CornerRadius::same((radius * canvas.scale).clamp(0.0, 255.0) as u8);
            ink.painter.rect_filled(r, corner, *fill);
            if *width > 0.0 && stroke.a() > 0 {
                ink.painter.rect_stroke(
                    r,
                    corner,
                    Stroke::new(*width, *stroke),
                    StrokeKind::Middle,
                );
            }
        }
        Mob::Path {
            points,
            color,
            width,
            drawn,
            arrow,
            dashed,
        } => {
            let screen: Vec<Pos2> = points.iter().map(|p| canvas.to_screen(*p)).collect();
            draw_path(ink, &cut(&screen, *drawn), *color, *width, *arrow, *dashed);
        }
        Mob::Band { top, bottom, color } => {
            let mut mesh = Mesh::default();
            for (t, b) in top.iter().zip(bottom) {
                mesh.colored_vertex(canvas.to_screen(*t), *color);
                mesh.colored_vertex(canvas.to_screen(*b), *color);
            }
            for i in 0..top.len().min(bottom.len()).saturating_sub(1) as u32 {
                // Each column between two samples is two triangles.
                let k = 2 * i;
                mesh.add_triangle(k, k + 1, k + 2);
                mesh.add_triangle(k + 1, k + 3, k + 2);
            }
            ink.painter.add(mesh);
        }
        Mob::Dot { at, radius, color } => {
            ink.painter
                .circle_filled(canvas.to_screen(*at), radius * canvas.scale, *color);
        }
        Mob::Frame { .. } => {}
    }
}

/// A formula's part: the whole bitmap placed about its centre, its mask tinted, cut at the wipe across the part's
/// box (the last twentieth of the written width fading in).
fn draw_tex(ink: &Ink, canvas: &Canvas, mob: &Mob) {
    let &Mob::Tex {
        ref latex,
        part,
        at,
        size,
        color,
        wipe,
        lo,
        hi,
    } = mob
    else {
        return;
    };
    if wipe <= 0.0 || color.a() == 0 {
        return;
    }
    let px = size * canvas.scale;
    let Ok((f, near)) = typeset_near(latex, px) else {
        return;
    };
    let scale = px / near;
    let center = canvas.to_screen(at);
    let half = vec2(f.width as f32, f.height as f32) * 0.5 * scale;
    let full = Rect::from_center_size(center, half * 2.0);
    let texture = texture(ink.painter.ctx(), latex, near, &f, part);
    // The wipe's edge, from the part's box on the canvas to the bitmap's u.
    let edge_x = canvas
        .to_screen(at + glam::Vec2::new(lo.x + wipe.min(1.0) * (hi.x - lo.x), 0.0))
        .x;
    let u = if wipe >= 1.0 {
        1.0
    } else {
        ((edge_x - full.min.x) / full.width()).clamp(0.0, 1.0)
    };
    let soft = if wipe >= 1.0 {
        0.0
    } else {
        (0.05 * (hi.x - lo.x) * canvas.scale / full.width()).min(u)
    };
    let solid = Rect::from_min_max(
        full.min,
        pos2(full.min.x + (u - soft) * full.width(), full.max.y),
    );
    let uv = |a: f32, b: f32| Rect::from_min_max(pos2(a, 0.0), pos2(b, 1.0));
    ink.painter
        .image(texture.id(), solid, uv(0.0, u - soft), color);
    if soft > 0.0 {
        let band = Rect::from_min_max(
            pos2(solid.max.x, full.min.y),
            pos2(full.min.x + u * full.width(), full.max.y),
        );
        ink.painter.image(
            texture.id(),
            band,
            uv(u - soft, u),
            color.gamma_multiply(0.45),
        );
    }
    ink.min_size
        .set(ink.min_size.get().min(f.min_glyph_px * scale));
}

/// The texture of a formula's part (its mask as white with coverage alpha), made once per size.
fn texture(
    ctx: &egui::Context,
    latex: &str,
    size: f32,
    f: &Formula,
    part: Part,
) -> egui::TextureHandle {
    let id = egui::Id::unique(("math tex", latex, size.to_bits(), part));
    if let Some(t) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return t;
    }
    let rest;
    let mask: &[u8] = match part {
        Part::Whole => &f.coverage,
        Part::Rest => {
            rest = rest_mask(f);
            &rest
        }
        Part::Term(n) => f.terms.get(n - 1).map_or(&[][..], Vec::as_slice),
    };
    let rgba: Vec<u8> = if mask.is_empty() {
        vec![0; f.coverage.len() * 4]
    } else {
        mask.iter().flat_map(|&a| [a, a, a, a]).collect()
    };
    let image =
        egui::ColorImage::from_rgba_premultiplied([f.width as usize, f.height as usize], &rgba);
    let t = ctx.load_texture(
        format!("math {part:?}: {latex}"),
        image,
        egui::TextureOptions::LINEAR,
    );
    ctx.data_mut(|d| d.insert_temp(id, t.clone()));
    t
}

/// The first `drawn` fraction (by length) of the polyline `points`.
fn cut(points: &[Pos2], drawn: f32) -> Vec<Pos2> {
    if drawn >= 1.0 || points.len() < 2 {
        return points.to_vec();
    }
    let total: f32 = points.windows(2).map(|w| w[0].distance(w[1])).sum();
    let mut left = total * drawn.max(0.0);
    let mut out = vec![points[0]];
    for w in points.windows(2) {
        // Each segment is kept whole while the length left covers it, and cut where it runs out.
        let d = w[0].distance(w[1]);
        if d >= left {
            out.push(w[0] + (w[1] - w[0]) * (left / d.max(1e-6)));
            break;
        }
        out.push(w[1]);
        left -= d;
    }
    out
}

/// A polyline `width` points wide, dashed or solid, with an arrowhead at its end.
fn draw_path(ink: &Ink, points: &[Pos2], color: Color32, width: f32, arrow: bool, dashed: bool) {
    if points.len() < 2 || color.a() == 0 {
        return;
    }
    let stroke = Stroke::new(width, color);
    let head = (width * 5.0).max(14.0);
    let mut line = points.to_vec();
    let end = *points.last().unwrap();
    let dir = (end - points[points.len() - 2]).normalized();
    if arrow {
        // The line stops at the arrowhead's base, so the tip stays sharp.
        if let Some(last) = line.last_mut() {
            *last = end - dir * head * 0.8;
        }
    }
    if dashed {
        ink.painter
            .extend(egui::Shape::dashed_line(&line, stroke, 12.0, 8.0));
    } else {
        ink.painter.add(PathShape::line(line, stroke));
    }
    if arrow {
        let side = vec2(-dir.y, dir.x) * head * 0.45;
        let base = end - dir * head;
        ink.painter.add(PathShape::convex_polygon(
            vec![end, base + side, base - side],
            color,
            Stroke::NONE,
        ));
    }
}
