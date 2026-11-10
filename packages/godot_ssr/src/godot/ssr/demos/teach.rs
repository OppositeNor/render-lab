//! Single steps the story explains, each shown on P as a value steps through it: the geometric normal from the
//! depths around P, the normal packed into three bytes and the roughness into the fourth, P's depth back through
//! the projection, P's group of threads and their iterations, and the charts of the confidence and of the
//! compression beside P's validity.

use super::{chart, plot};
use crate::godot::ssr::cpu;
use crate::godot::ssr::text::{tr, tr_with};
use crate::godot::ssr::trace::{self, Exit, Image};
use egui::Color32;
use glam::{Vec3, Vec4};
use lab::frame::{FLAG_ARROW, FLAG_DASHED, FrameContent};
use lab::palette;
use lab::shot::ShotState;
use lab::utility::inset::{Corner, Outline, Picture, PictureInset};
use std::sync::Arc;

/// A discarded side's grey.
const DIM: Color32 = Color32::from_rgb(70, 80, 95);

/// A straight line of dots between two chart points.
fn line(rgba: &mut [u8], (w, h): (usize, usize), a: (f32, f32), b: (f32, f32), c: Color32) {
    let n = ((b.0 - a.0).abs().max((b.1 - a.1).abs()) as i32).max(1);
    for i in 0..=n {
        let f = i as f32 / n as f32;
        let (x, y) = (a.0 + (b.0 - a.0) * f, a.1 + (b.1 - a.1) * f);
        plot(rgba, w, h, x as i32, y as i32, c);
    }
}

/// A world-space direction of a view-space one.
fn to_world(screen: &cpu::View, v: Vec3) -> Vec3 {
    screen.world_to_view.inverse().transform_vector3(v)
}

/// The geometric normal at P, built as far as `geoStep` (screen_space_reflection.glsl
/// compute_geometric_normal): a chart per axis of the depths two pixels to each side and each side's prediction
/// of P's depth, the side kept, then the tangents and their cross product drawn at P.
pub(super) fn geometric(
    state: &ShotState,
    content: &mut FrameContent,
    screen: &cpu::Screen,
    pixel: [i32; 2],
    world: Vec3,
) {
    let step = state.p("geoStep").round() as i32;
    if step <= 0 {
        return;
    }
    let last = Image::new([1, 1], 4);
    let g = screen.inputs(&last, 64, 0.15, 2.0, 0.5).geometric_at(pixel);
    for (axis, depths, sign, corner) in [
        (0, g.h, g.sign[0], Corner::TopLeft),
        (1, g.v, g.sign[1], Corner::TopRight),
    ] {
        content
            .insets
            .push(axis_chart(axis, depths, g.depth_c, sign, step, corner));
    }
    // The tangents, each along the kept side, and their cross product, drawn at P a few tens of centimetres long.
    let colors = [palette::AXIS_X, palette::AXIS_Z];
    let names = [
        tr("demos-tangent_x").to_string(),
        tr("demos-tangent_y").to_string(),
    ];
    if step >= 4 {
        for ((t, c), name) in g.tangents.iter().zip(colors).zip(names) {
            let d = to_world(screen, *t).normalize_or_zero() * 0.35;
            content
                .draw
                .segment(world, world + d, c, 1.0, 4.0, FLAG_ARROW);
            content.draw.text(world + d * 1.15, name, c, [0.0, 0.5]);
        }
    }
    if step >= 5 {
        let n = to_world(screen, g.normal).normalize_or_zero() * 0.5;
        content
            .draw
            .segment(world, world + n, palette::SELECTED, 1.0, 5.0, FLAG_ARROW);
        // Beside the arrow's middle, clear of the charts above it.
        content.draw.text(
            world + n * 0.6,
            tr("demos-geo_normal"),
            palette::SELECTED,
            [-0.05, 0.5],
        );
    }
}

/// One axis's chart: the depths at -2..2 pixels from P (reversed depth, up is nearer), and from step 2 each
/// side's straight line through its two depths to P's column, the kept side lit from step 3.
fn axis_chart(
    axis: usize,
    d: Vec4,
    centre: f32,
    sign: i32,
    step: i32,
    corner: Corner,
) -> PictureInset {
    let (w, h) = (320usize, 200usize);
    let mut rgba = chart(w, h);
    // The depths at -2, -1, 0, 1, 2, and each side's prediction of the centre: twice the near minus the far.
    let values = [d.y, d.x, centre, d.z, d.w];
    let preds = [2.0 * d.x - d.y, 2.0 * d.z - d.w];
    let all = values.iter().chain(preds.iter());
    let lo = all.clone().fold(f32::MAX, |a, &b| a.min(b));
    let hi = all.fold(f32::MIN, |a, &b| a.max(b));
    let pad = ((hi - lo) * 0.2).max(1e-6);
    let at = |o: f32, v: f32| {
        (
            24.0 + (o + 2.0) / 4.0 * (w as f32 - 48.0),
            h as f32 - 16.0 - (v - lo + pad) / (hi - lo + 2.0 * pad) * (h as f32 - 32.0),
        )
    };
    let side_colors = [palette::REFLECTED, palette::INCIDENT];
    if step >= 2 {
        for (i, far) in [at(-2.0, d.y), at(2.0, d.w)].into_iter().enumerate() {
            // Each side's line runs from its far depth through its near one to P's column; the discarded side greys.
            let kept = (sign < 0) == (i == 0);
            let c = if step >= 3 && !kept {
                DIM
            } else {
                side_colors[i]
            };
            let p = at(0.0, preds[i]);
            for o in [-1.0, 0.0, 1.0] {
                line(&mut rgba, (w, h), (far.0, far.1 + o), (p.0, p.1 + o), c);
            }
            // The prediction: a square ring at P's column.
            for k in -6..=6 {
                for (dx, dy) in [(k, -6), (k, 6), (-6, k), (6, k)] {
                    plot(&mut rgba, w, h, p.0 as i32 + dx, p.1 as i32 + dy, c);
                }
            }
        }
    }
    for (i, v) in values.iter().enumerate() {
        // The five depths as dots, P's in its own colour.
        let (x, y) = at(i as f32 - 2.0, *v);
        let c = if i == 2 {
            palette::SELECTED
        } else {
            palette::THEME_ACCENT
        };
        for dy in -2..=2 {
            for dx in -2..=2 {
                plot(&mut rgba, w, h, x as i32 + dx * 2, y as i32 + dy * 2, c);
            }
        }
    }
    let mut lines = Vec::new();
    if step >= 2 {
        let miss = |p: f32| format!("{:.1e}", (p - centre).abs());
        let (first, second) = (miss(preds[0]), miss(preds[1]));
        let text = if axis == 0 {
            tr_with(
                "demos-geo_miss_x",
                lab::args!(first = first, second = second),
            )
        } else {
            tr_with(
                "demos-geo_miss_y",
                lab::args!(first = first, second = second),
            )
        };
        lines.push((text, palette::THEME_ACCENT));
    }
    if step >= 3 {
        let text = match (axis, sign < 0) {
            (0, true) => tr("demos-geo_kept_left"),
            (0, false) => tr("demos-geo_kept_right"),
            (_, true) => tr("demos-geo_kept_up"),
            (_, false) => tr("demos-geo_kept_down"),
        };
        lines.push((text.to_string(), side_colors[usize::from(sign > 0)]));
    }
    let title = if axis == 0 {
        tr("demos-geo_chart_x")
    } else {
        tr("demos-geo_chart_y")
    };
    PictureInset {
        corner,
        x_axis: tr("demos-geo_offset").to_string(),
        y_axis: tr("demos-depth_short").to_string(),
        lines,
        ..PictureInset::new(
            title,
            Picture::Pixels {
                key: format!("ssr-geo-{axis}"),
                size: [w, h],
                rgba: Arc::new(rgba),
            },
            [320.0, 200.0],
        )
    }
}

/// P's normal packed as far as `encodeStep` (the depth pre-pass's encode24): the unit normal and the cube from
/// minus one to one, the normal scaled to touch the cube, the stored (shortened) vector; then the alpha byte.
pub(super) fn encode(
    state: &ShotState,
    content: &mut FrameContent,
    screen: &cpu::View,
    world: Vec3,
    nr: Vec4,
) {
    let step = state.p("encodeStep").round() as i32;
    if step <= 0 {
        return;
    }
    // The cube and the vectors drawn this many metres per unit.
    let s = 0.3;
    // P's stored normal, or the direction the best-fit sweep turns through, packed as the pre-pass packs it.
    let (unit, stored) = if state.p("fitAngle") >= 0.0 {
        let n = super::rules::fit_direction(state.p("fitAngle"));
        (
            n,
            n / n.abs().max_element() * super::rules::best_fit_scale(n),
        )
    } else {
        let stored = nr.truncate() * 2.0 - Vec3::ONE;
        (stored.normalize_or_zero(), stored)
    };
    let axes = [Vec3::X, Vec3::Y, Vec3::Z].map(|a| to_world(screen, a) * s);
    for (i, a) in axes.iter().enumerate() {
        // The cube's twelve edges: the four along each of its axes.
        let (b, c) = (axes[(i + 1) % 3], axes[(i + 2) % 3]);
        for (sb, sc) in [(-1.0, -1.0), (-1.0, 1.0), (1.0, -1.0), (1.0, 1.0)] {
            let base = world + b * sb + c * sc;
            content
                .draw
                .segment(base - *a, base + *a, palette::THEME_ACCENT, 0.6, 1.5, 0);
        }
    }
    // Each arrow is labelled only while it is the newest one, so its label stands alone.
    let arrow = |content: &mut FrameContent, v: Vec3, c: Color32, label: Option<String>| {
        let d = to_world(screen, v) * s;
        content
            .draw
            .segment(world, world + d, c, 1.0, 4.0, FLAG_ARROW);
        if let Some(label) = label {
            content.draw.text(world + d * 1.08, label, c, [0.0, 0.5]);
        }
    };
    let label = (step == 1).then(|| tr("demos-label_normal").to_string());
    arrow(content, unit, palette::AXIS_Y, label);
    if step >= 2 {
        let touch = unit / unit.abs().max_element().max(1e-6);
        let label = (step == 2).then(|| tr("demos-label_cube").to_string());
        arrow(content, touch, palette::SELECTED, label);
    }
    if step >= 3 {
        let label = (step == 3).then(|| tr("demos-label_stored").to_string());
        arrow(content, stored, palette::INCIDENT, label);
    }
    if step >= 4 {
        content.insets.push(alpha_bar(state, nr.w));
    }
}

/// The alpha byte as a bar from 0 to 255, its middle marked, and P's value on it: the scaled roughness in the
/// lower half, or as a moving object would store it (`alphaDynamic`), one minus that, in the upper half.
fn alpha_bar(state: &ShotState, alpha: f32) -> PictureInset {
    let (w, h) = (400usize, 60usize);
    let mut rgba = chart(w, h);
    let still = if alpha > 0.5 {
        1.0 - alpha
    } else {
        alpha
    };
    let a = still + (1.0 - 2.0 * still) * state.p("alphaDynamic").clamp(0.0, 1.0);
    let x = |v: f32| 8.0 + v * (w as f32 - 16.0);
    line(&mut rgba, (w, h), (x(0.0), 30.0), (x(1.0), 30.0), DIM);
    line(
        &mut rgba,
        (w, h),
        (x(0.5), 8.0),
        (x(0.5), 52.0),
        palette::THEME_ACCENT,
    );
    for dx in -1..=1 {
        line(
            &mut rgba,
            (w, h),
            (x(a) + dx as f32, 4.0),
            (x(a) + dx as f32, 56.0),
            palette::INCIDENT,
        );
    }
    let byte = (a * 255.0).round() as u32;
    let half = if a > 0.5 {
        tr("demos-alpha_moving")
    } else {
        tr("demos-alpha_still")
    };
    PictureInset {
        corner: Corner::TopRight,
        x_axis: tr("demos-alpha_axis").to_string(),
        lines: vec![
            (
                tr_with(
                    "demos-alpha_byte",
                    lab::args!(
                        byte = byte,
                        roughness = format!("{:.2}", trace::decode_roughness(a))
                    ),
                ),
                palette::INCIDENT,
            ),
            (half.to_string(), palette::THEME_ACCENT),
        ],
        ..PictureInset::new(
            tr("demos-alpha_title"),
            Picture::Pixels {
                key: "ssr-alpha-bar".into(),
                size: [w, h],
                rgba: Arc::new(rgba),
            },
            [400.0, 60.0],
        )
    }
}

/// The line from the camera through P's place on screen, drawn as far as `unproject` of the way to P: the inverse
/// projection taking P's screen position and depth back to its point in the room.
pub(super) fn unproject(state: &ShotState, content: &mut FrameContent, world: Vec3, depth: f32) {
    let u = state.p("unproject").clamp(0.0, 1.0);
    if u <= 0.0 {
        return;
    }
    let eye = Vec3::from(content.uniforms.camera.position);
    let end = eye + (world - eye) * u;
    content
        .draw
        .segment(eye, end, palette::SELECTED, 1.0, 3.0, FLAG_DASHED);
    if u >= 1.0 && state.p("mirror") <= 0.0 {
        content.draw.text(
            world + Vec3::Y * 0.12,
            tr_with(
                "demos-label_depth",
                lab::args!(depth = format!("{depth:.5}")),
            ),
            palette::SELECTED,
            [1.1, 0.5],
        );
    }
}

/// The view ray arriving at P and its mirror direction about P's stored normal (`normal`, world space), drawn
/// as far as `mirror`: where a reflected ray leaves the surface.
pub(super) fn mirror(state: &ShotState, content: &mut FrameContent, world: Vec3, normal: Vec3) {
    let u = state.p("mirror").clamp(0.0, 1.0);
    if u <= 0.0 {
        return;
    }
    let eye = Vec3::from(content.uniforms.camera.position);
    let d = (world - eye).normalize_or_zero();
    let r = d - 2.0 * d.dot(normal) * normal;
    content.draw.segment(
        world - d * 0.6 * u,
        world,
        palette::THEME_ACCENT,
        1.0,
        3.0,
        FLAG_DASHED,
    );
    content.draw.segment(
        world,
        world + r * 0.8 * u,
        palette::REFLECTED,
        1.0,
        4.0,
        FLAG_ARROW,
    );
    if u >= 1.0 {
        content.draw.text(
            world + r * 0.85,
            tr("demos-label_mirror"),
            palette::REFLECTED,
            [0.0, 0.5],
        );
    }
}

/// The iterations of each thread of the group at `origin` (0 where it skips the trace: sky, rough, or off the
/// screen), row by row: the CPU mirror on the frame's own pyramid.
fn lane_iterations(state: &ShotState, screen: &cpu::Screen, origin: [i32; 2]) -> [usize; 64] {
    let last = Image::new([1, 1], 4);
    let inputs = screen.inputs(
        &last,
        state.p("ssrMaxSteps").round() as i32,
        state.p("ssrFadeIn"),
        state.p("ssrFadeOut"),
        state.p("ssrDepthTolerance"),
    );
    let mut iters = [0usize; 64];
    for (i, n) in iters.iter_mut().enumerate() {
        let p = [origin[0] + (i % 8) as i32, origin[1] + (i / 8) as i32];
        if p[0] < screen.size[0] as i32 && p[1] < screen.size[1] as i32 {
            let t = inputs.trace(p);
            *n = if t.exit == Exit::NotTraced {
                0
            } else {
                t.steps.len()
            };
        }
    }
    iters
}

/// P's group of eight by eight threads as far as `laneStep`: the group and P's thread, each thread's iterations
/// of the trace (the CPU mirror on the frame's own pyramid), and the longest, which the group waits for.
pub(super) fn lanes(
    state: &ShotState,
    content: &mut FrameContent,
    screen: &cpu::Screen,
    pixel: [i32; 2],
) {
    let step = state.p("laneStep").round() as i32;
    if step <= 0 {
        return;
    }
    let origin = [pixel[0] / 8 * 8, pixel[1] / 8 * 8];
    let iters = lane_iterations(state, screen, origin);
    let max = *iters.iter().max().unwrap_or(&0);
    let longest = iters.iter().position(|&n| n == max).unwrap_or(0);
    let mine = ((pixel[1] - origin[1]) * 8 + pixel[0] - origin[0]) as usize;
    let (cell, side) = (32usize, 256usize);
    let mut rgba = vec![0u8; side * side * 4];
    for (i, n) in iters.iter().enumerate() {
        // A thread's cell is as bright as its share of the longest trace; a skipped thread stays dark.
        let c = if *n == 0 {
            [25, 28, 34]
        } else {
            let f = *n as f32 / max.max(1) as f32;
            let a = palette::THEME_ACCENT;
            [a.r(), a.g(), a.b()].map(|ch| (30.0 + (ch as f32 - 30.0) * f) as u8)
        };
        let (cx, cy) = (i % 8 * cell, i / 8 * cell);
        for y in cy..cy + cell {
            for x in cx..cx + cell {
                let k = (y * side + x) * 4;
                rgba[k..k + 4].copy_from_slice(&[c[0], c[1], c[2], 255]);
            }
        }
    }
    let rect = |i: usize| {
        let (x, y) = ((i % 8) as f32 / 8.0, (i / 8) as f32 / 8.0);
        [x, y, x + 0.125, y + 0.125]
    };
    let mut outlines = vec![Outline {
        rect: rect(mine),
        color: palette::SELECTED,
        width: 3.0,
        outside: false,
    }];
    let mut lines = vec![(tr("demos-lanes_key").to_string(), palette::THEME_ACCENT)];
    if step >= 2 {
        lines.push((
            tr_with("demos-lanes_p", lab::args!(n = iters[mine])),
            palette::SELECTED,
        ));
    }
    if step >= 3 && max > 0 {
        outlines.push(Outline {
            rect: rect(longest),
            color: palette::INCIDENT,
            width: 3.0,
            outside: false,
        });
        let busy: usize = iters.iter().sum();
        let idle = 100.0 * (1.0 - busy as f32 / (64 * max.max(1)) as f32);
        lines.push((
            tr_with(
                "demos-lanes_max",
                lab::args!(max = max, idle = format!("{idle:.0}")),
            ),
            palette::INCIDENT,
        ));
    }
    // The group's pixels in the room, each at its surface, so the inset's square is seen where it lies.
    for i in 0..64 {
        let p = [origin[0] + i % 8, origin[1] + i / 8];
        if p[0] < screen.size[0] as i32
            && p[1] < screen.size[1] as i32
            && screen.hiz[0].load(p).x > 0.0
        {
            content
                .draw
                .point(screen.surface(p), 2.5, palette::THEME_ACCENT, 0.9);
        }
    }
    // The group, for the inset's click (script/logic.lam `pickLane`): its first pixel and the SSR size.
    content.script = Some(Arc::new(super::super::script::LaneGroup {
        x: origin[0] as i64,
        y: origin[1] as i64,
        w: screen.size[0] as i64,
        h: screen.size[1] as i64,
    }));
    content.insets.push(PictureInset {
        corner: Corner::BottomRight,
        grid: Some([8, 8]),
        outlines,
        click: Some("pickLane".into()),
        lines,
        ..PictureInset::new(
            tr("demos-lanes_title"),
            Picture::Pixels {
                key: format!("ssr-lanes-{step}-{}-{}", origin[0], origin[1]),
                size: [side, side],
                rgba: Arc::new(rgba),
            },
            [240.0, 240.0],
        )
    });
}

/// The confidence over the gap between the ray's end and the surface (0 to 1 view unit): one minus a smoothstep
/// to the depth tolerance (dim), and its square (bright); P's gap marked.
pub(super) fn confidence_chart(state: &ShotState, t: &trace::Trace) -> PictureInset {
    let (w, h) = (360usize, 130usize);
    let mut rgba = chart(w, h);
    let tol = state.p("ssrDepthTolerance").max(1e-4);
    let y = |v: f32| (h as f32 - 8.0) - v * (h as f32 - 16.0);
    for x in 0..w {
        // Each column is a gap from 0 to 1 view unit.
        let gap = x as f32 / (w - 1) as f32;
        let f = (gap / tol).clamp(0.0, 1.0);
        let c = 1.0 - f * f * (3.0 - 2.0 * f);
        plot(&mut rgba, w, h, x as i32, y(c) as i32, DIM);
        plot(
            &mut rgba,
            w,
            h,
            x as i32,
            y(c * c) as i32,
            palette::THEME_ACCENT,
        );
    }
    let gx = t.gap.clamp(0.0, 1.0) * (w - 1) as f32;
    line(
        &mut rgba,
        (w, h),
        (gx, 0.0),
        (gx, h as f32 - 1.0),
        palette::REFLECTED,
    );
    let tx = tol.min(1.0) * (w - 1) as f32;
    line(&mut rgba, (w, h), (tx, 0.0), (tx, h as f32 - 1.0), DIM);
    PictureInset {
        x_axis: tr("demos-gap_axis").to_string(),
        y_axis: tr("demos-confidence_axis").to_string(),
        lines: vec![
            (
                tr_with(
                    "demos-confidence_p",
                    lab::args!(
                        gap = format!("{:.3}", t.gap),
                        conf = format!("{:.2}", t.terms[2])
                    ),
                ),
                palette::REFLECTED,
            ),
            (
                tr_with(
                    "demos-confidence_tol",
                    lab::args!(tol = format!("{tol:.2}")),
                ),
                palette::THEME_ACCENT,
            ),
        ],
        ..PictureInset::new(
            tr("demos-confidence_chart"),
            Picture::Pixels {
                key: "ssr-confidence-chart".into(),
                size: [w, h],
                rgba: Arc::new(rgba),
            },
            [360.0, 130.0],
        )
    }
}

/// The compression of a colour by its luminance, L / (1 + L) over L from 0 to 16, with two texels marked (0.5 and
/// `brightTexel`): averaged as they are, and averaged compressed then uncompressed.
pub(super) fn compress_chart(state: &ShotState) -> PictureInset {
    let (w, h) = (360usize, 150usize);
    let mut rgba = chart(w, h);
    let y = |v: f32| (h as f32 - 8.0) - v * (h as f32 - 16.0);
    let x = |l: f32| l / 16.0 * (w - 1) as f32;
    for px in 0..w {
        let l = px as f32 / (w - 1) as f32 * 16.0;
        plot(
            &mut rgba,
            w,
            h,
            px as i32,
            y(l / (1.0 + l)) as i32,
            palette::THEME_ACCENT,
        );
        plot(&mut rgba, w, h, px as i32, y(l / 16.0) as i32, DIM);
    }
    let (a, b) = (0.5f32, state.p("brightTexel").max(0.5));
    for l in [a, b] {
        line(
            &mut rgba,
            (w, h),
            (x(l), y(0.0)),
            (x(l), y(l / (1.0 + l))),
            palette::SELECTED,
        );
    }
    let plain = (a + b) / 2.0;
    let c = (a / (1.0 + a) + b / (1.0 + b)) / 2.0;
    let back = c / (1.0 - c);
    for (l, col) in [(plain, palette::INCIDENT), (back, palette::REFLECTED)] {
        line(&mut rgba, (w, h), (x(l), y(0.0)), (x(l), y(1.0)), col);
    }
    PictureInset {
        x_axis: tr("demos-luminance_axis").to_string(),
        y_axis: tr("demos-compressed_axis").to_string(),
        lines: vec![
            (
                tr_with(
                    "demos-compress_plain",
                    lab::args!(bright = format!("{b:.1}"), avg = format!("{plain:.2}")),
                ),
                palette::INCIDENT,
            ),
            (
                tr_with(
                    "demos-compress_first",
                    lab::args!(avg = format!("{back:.2}")),
                ),
                palette::REFLECTED,
            ),
        ],
        ..PictureInset::new(
            tr("demos-compress_chart"),
            Picture::Pixels {
                key: "ssr-compress-chart".into(),
                size: [w, h],
                rgba: Arc::new(rgba),
            },
            [360.0, 130.0],
        )
    }
}

/// The trace's start at P as far as `startStep`: P's stored normal and the view ray reflected about it; then
/// the true normal beside it, and a chart of the push off the surface, one minus the eighth power of the cosine
/// between the two normals, with P's angle (and `pushTilt`) marked.
pub(super) fn start(
    state: &ShotState,
    content: &mut FrameContent,
    screen: &cpu::Screen,
    t: &trace::Trace,
    world: Vec3,
) {
    let step = state.p("startStep").round() as i32;
    if step <= 0 || t.exit == Exit::NotTraced {
        return;
    }
    // Each label at its arrow's tip, only while its step is the newest (the two normals nearly coincide).
    let arrow =
        |content: &mut FrameContent, v: Vec3, len: f32, c: Color32, label: (String, [f32; 2])| {
            let d = to_world(screen, v).normalize_or_zero() * len;
            content
                .draw
                .segment(world, world + d, c, 1.0, 4.0, FLAG_ARROW);
            if !label.0.is_empty() {
                content.draw.text(world + d * 1.08, label.0, c, label.1);
            }
        };
    let newest = |key_step: i32, text: &str| {
        if step == key_step {
            text.to_string()
        } else {
            String::new()
        }
    };
    arrow(
        content,
        t.normal,
        0.4,
        palette::AXIS_Y,
        (newest(1, tr("demos-label_stored_normal")), [1.05, 1.0]),
    );
    arrow(
        content,
        t.ray_dir,
        0.7,
        palette::REFLECTED,
        (newest(1, tr("demos-label_mirror")), [0.0, 0.0]),
    );
    if step < 2 {
        return;
    }
    arrow(
        content,
        t.geom_normal,
        0.55,
        palette::SELECTED,
        (newest(2, tr("demos-label_true_normal")), [-0.05, 1.0]),
    );
    let own = t
        .normal
        .dot(t.geom_normal)
        .clamp(-1.0, 1.0)
        .acos()
        .to_degrees();
    content.insets.push(push_chart(state.p("pushTilt"), own));
}

/// The push off the surface over the angle between the stored and the true normal (0 to 90°), P's angle `own`
/// marked, and `tilt` when it is above 0.
fn push_chart(tilt: f32, own: f32) -> PictureInset {
    let push = |deg: f32| 1.0 - deg.to_radians().cos().clamp(0.0, 1.0).powf(8.0);
    let (w, h) = (320usize, 170usize);
    let mut rgba = chart(w, h);
    let y = |v: f32| (h as f32 - 8.0) - v * (h as f32 - 16.0);
    let x = |deg: f32| deg / 90.0 * (w - 1) as f32;
    for px in 0..w {
        let deg = px as f32 / (w - 1) as f32 * 90.0;
        plot(
            &mut rgba,
            w,
            h,
            px as i32,
            y(push(deg)) as i32,
            palette::THEME_ACCENT,
        );
    }
    line(
        &mut rgba,
        (w, h),
        (x(own), 0.0),
        (x(own), h as f32 - 1.0),
        palette::REFLECTED,
    );
    let mut lines = vec![(
        tr_with(
            "demos-push_p",
            lab::args!(
                angle = format!("{own:.1}"),
                push = format!("{:.4}", push(own))
            ),
        ),
        palette::REFLECTED,
    )];
    if tilt > 0.0 {
        line(
            &mut rgba,
            (w, h),
            (x(tilt), 0.0),
            (x(tilt), h as f32 - 1.0),
            palette::SELECTED,
        );
        lines.push((
            tr_with(
                "demos-push_tilt",
                lab::args!(
                    angle = format!("{tilt:.0}"),
                    push = format!("{:.2}", push(tilt))
                ),
            ),
            palette::SELECTED,
        ));
    }
    PictureInset {
        corner: Corner::TopLeft,
        x_axis: tr("demos-push_axis_x").to_string(),
        y_axis: tr("demos-push_axis_y").to_string(),
        lines,
        ..PictureInset::new(
            tr("demos-push_chart"),
            Picture::Pixels {
                key: "ssr-push-chart".into(),
                size: [w, h],
                rgba: Arc::new(rgba),
            },
            [320.0, 170.0],
        )
    }
}

/// P's hit in the room as far as `hitShow`: P's ray to the point it ended on, then the line from that point to
/// the camera, the way back onto a screen where the trace reads the point's colour.
pub(super) fn hit(
    state: &ShotState,
    content: &mut FrameContent,
    screen: &cpu::Screen,
    t: &trace::Trace,
    pixel: [i32; 2],
) {
    let step = state.p("hitShow").round() as i32;
    if step <= 0 || t.exit == Exit::NotTraced {
        return;
    }
    let from = screen.surface(pixel);
    let at = screen.world_of(t.hit_screen);
    content
        .draw
        .segment(from, at, palette::REFLECTED, 1.0, 3.0, FLAG_ARROW);
    content.draw.point(at, 7.0, palette::REFLECTED, 1.0);
    content.draw.text(
        at + Vec3::Y * 0.06,
        tr("demos-label_hit"),
        palette::REFLECTED,
        [0.5, 1.0],
    );
    if step >= 2 && state.p("lastYaw") <= 0.0 {
        let eye = Vec3::from(content.uniforms.camera.position);
        content
            .draw
            .segment(at, eye, palette::SELECTED, 1.0, 2.5, FLAG_DASHED);
    }
}

/// The fade over the ray's screen length, 0 to 1, its peak and P's length marked, and a length a click on the
/// chart picks (`fadeProbe`) with its fade.
pub(super) fn fade_inset(state: &ShotState, t: &trace::Trace) -> PictureInset {
    let (fade_in, fade_out) = (state.p("ssrFadeIn"), state.p("ssrFadeOut"));
    let (w, h) = (360usize, 130usize);
    let (mut rgba, peak, at) = super::fade_chart((w, h), (fade_in, fade_out), t.ray_len);
    let fade = |l: f32| trace::fade(l.clamp(0.0, 1.0), fade_in, fade_out);
    let mut lines = vec![
        (
            tr_with(
                "demos-fade_peak",
                lab::args!(peak = format!("{peak:.2}"), at = format!("{at:.2}")),
            ),
            palette::SELECTED,
        ),
        (
            tr_with(
                "demos-fade_p",
                lab::args!(
                    len = format!("{:.2}", t.ray_len),
                    fade = format!("{:.2}", fade(t.ray_len))
                ),
            ),
            palette::REFLECTED,
        ),
    ];
    let probe = state.p("fadeProbe");
    if probe >= 0.0 {
        let x = probe * (w - 1) as f32;
        line(
            &mut rgba,
            (w, h),
            (x, 0.0),
            (x, h as f32 - 1.0),
            palette::THEME_ACCENT,
        );
        lines.push((
            tr_with(
                "demos-fade_probe",
                lab::args!(
                    len = format!("{probe:.2}"),
                    fade = format!("{:.2}", fade(probe))
                ),
            ),
            palette::THEME_ACCENT,
        ));
    }
    // The circle and the lines told apart in the inset's own lines (a marker's label would hide the curve).
    PictureInset {
        marker: Some(lab::utility::inset::Marker {
            at: [at, peak],
            color: palette::SELECTED,
            label: String::new(),
        }),
        x_axis: tr("demos-ray_len").to_string(),
        y_axis: tr("demos-fade_axis").to_string(),
        lines,
        click: Some("pickFadeLength".into()),
        ..PictureInset::new(
            tr("demos-fade_chart"),
            Picture::Pixels {
                key: "ssr-fade-chart".into(),
                size: [w, h],
                rgba: Arc::new(rgba),
            },
            [360.0, 130.0],
        )
    }
}
