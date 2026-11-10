//! Three rules the story explains, each shown on P as a value sweeps it: the best-fit length that packs a normal
//! into three bytes (the table, mirrored on the CPU), the back-face test that only looks two SSR pixels from the
//! start, and the reprojection of P's hit into last frame's camera, where its colour is read.

use super::{chart, plot};
use crate::godot::ssr::cpu;
use crate::godot::ssr::text::{tr, tr_with};
use crate::godot::ssr::trace::{self, Exit, Image};
use egui::Color32;
use glam::{Mat4, Vec2, Vec3, Vec4};
use lab::fetch::Source;
use lab::frame::{FLAG_DASHED, FrameContent};
use lab::palette;
use lab::shot::ShotState;
use lab::utility::inset::{Corner, Link, Marker, Outline, Picture, PictureInset};
use std::sync::Arc;

/// A discarded or unchecked part's grey.
const DIM: Color32 = Color32::from_rgb(70, 80, 95);

/// An 8-bit round trip of a normal stored as `c * 0.5 + 0.5` (best_fit_normal.glsl quantize).
fn quantize(c: Vec3) -> Vec3 {
    ((c * 0.5 + Vec3::splat(0.5)).clamp(Vec3::ZERO, Vec3::ONE) * 255.0).map(f32::round_ties_even)
        / 255.0
        * 2.0
        - Vec3::ONE
}

/// The error the table minimises at length `t` for `normal` (find_minimum_error): the largest component of the
/// quantization error, per unit of length.
fn fit_error(normal: Vec3, t: f32) -> f32 {
    let vp = normal * t;
    ((quantize(vp) - vp) / t).abs().max_element()
}

/// The length the table stores for `normal` (in its (x, y, 1) form): the one of 126 that errs least.
fn find_minimum_error(normal: Vec3) -> f32 {
    let (mut best, mut t_best) = (f32::MAX, 0.0);
    for step in 0..126 {
        // nstep runs 1.5, 2.5, … 126.5, each length nstep / 127.5.
        let t = (1.5 + step as f32) / 127.5;
        let e = fit_error(normal, t);
        if e < best {
            best = e;
            t_best = t;
        }
    }
    t_best
}

/// The table's entry at `texel` of its 1024 x 1024 grid (best_fit_normal.glsl main): the direction at the texel's
/// centre, (u, v u, 1), and its best length, as the R8 table stores it.
pub(crate) fn table_entry(texel: [u32; 2]) -> f32 {
    let uv = (Vec2::new(texel[0] as f32, texel[1] as f32) + Vec2::splat(0.5)) / 1024.0;
    (find_minimum_error(Vec3::new(uv.x, uv.y * uv.x, 1.0)) * 255.0).round() / 255.0
}

/// encode24's scale for `v`, as the pre-pass reads it from the 1024 x 1024 R8 table (ssr_shading.wgsl
/// ssrEncode24): the collapsed direction's texel, its entry computed at the texel's centre and stored in 8 bits.
pub(super) fn best_fit_scale(v: Vec3) -> f32 {
    let a = v.abs();
    let m = a.max_element();
    let tc = if a.z < m {
        if a.y < m {
            Vec2::new(a.y, a.z)
        } else {
            Vec2::new(a.x, a.z)
        }
    } else {
        Vec2::new(a.x, a.y)
    } / m;
    let tc = if tc.x < tc.y {
        Vec2::new(tc.y, tc.x)
    } else {
        tc
    };
    let tc = Vec2::new(tc.x, tc.y / tc.x);
    let texel = (tc * 1024.0).floor().clamp(Vec2::ZERO, Vec2::splat(1023.0));
    let uv = (texel + Vec2::splat(0.5)) / 1024.0;
    (find_minimum_error(Vec3::new(uv.x, uv.y * uv.x, 1.0)) * 255.0).round() / 255.0
}

/// The swept direction of the best-fit demonstration (view space): from facing the camera (0°) toward its right
/// and a little up, `fitAngle` degrees.
pub(super) fn fit_direction(deg: f32) -> Vec3 {
    let a = deg.to_radians();
    Vec3::new(a.sin(), 0.35 * a.sin(), a.cos()).normalize()
}

/// The angle in degrees between `n` and its 8-bit round trip at length `t`.
fn error_degrees(n: Vec3, t: f32) -> f32 {
    quantize(n * t)
        .normalize()
        .dot(n)
        .clamp(-1.0, 1.0)
        .acos()
        .to_degrees()
}

/// The table's error over the length, for the swept direction (`fitAngle`): the curve the start-up search
/// walks, its minimum (the stored length) and length one (the normal touching the cube) marked, with each
/// one's 8-bit angular error.
pub(super) fn fit_chart(state: &ShotState) -> Option<PictureInset> {
    let deg = state.p("fitAngle");
    if deg < 0.0 {
        return None;
    }
    let n = fit_direction(deg);
    let cube = n / n.abs().max_element();
    let (w, h) = (360usize, 170usize);
    let mut rgba = chart(w, h);
    let lengths: Vec<f32> = (0..w)
        .map(|x| 0.05 + 0.95 * x as f32 / (w - 1) as f32)
        .collect();
    let errors: Vec<f32> = lengths.iter().map(|&t| fit_error(cube, t)).collect();
    let top = errors.iter().cloned().fold(1e-6, f32::max);
    for (x, e) in errors.iter().enumerate() {
        let y = (h as f32 - 6.0) - e / top * (h as f32 - 12.0);
        plot(&mut rgba, w, h, x as i32, y as i32, palette::THEME_ACCENT);
    }
    let best = best_fit_scale(n);
    let col = |t: f32| ((t - 0.05) / 0.95 * (w - 1) as f32) as i32;
    let chosen = state.p("fitLen");
    for (t, c) in [
        (1.0, DIM),
        (best, palette::SELECTED),
        (chosen, palette::REFLECTED),
    ] {
        // Grey: length one; yellow: the table's; pink: a length clicked on the chart.
        if t > 0.0 {
            for y in 0..h as i32 {
                plot(&mut rgba, w, h, col(t), y, c);
            }
        }
    }
    let mut lines = vec![
        (
            tr_with(
                "demos-fit_cube",
                lab::args!(
                    err = format!(
                        "{:.3}",
                        error_degrees(cube.normalize(), 1.0 * cube.length())
                    )
                ),
            ),
            palette::THEME_ACCENT,
        ),
        (
            tr_with(
                "demos-fit_best",
                lab::args!(
                    t = format!("{best:.2}"),
                    err = format!("{:.3}", error_degrees(n, best * cube.length()))
                ),
            ),
            palette::SELECTED,
        ),
    ];
    if chosen > 0.0 {
        lines.push((
            tr_with(
                "demos-fit_chosen",
                lab::args!(
                    t = format!("{chosen:.2}"),
                    err = format!("{:.3}", error_degrees(n, chosen * cube.length()))
                ),
            ),
            palette::REFLECTED,
        ));
    }
    Some(PictureInset {
        corner: Corner::BottomLeft,
        click: Some("pickFitLength".into()),
        x_axis: tr("demos-fit_axis_x").to_string(),
        y_axis: tr("demos-fit_axis_y").to_string(),
        lines,
        ..PictureInset::new(
            tr_with("demos-fit_title", lab::args!(deg = format!("{deg:.0}"))),
            Picture::Pixels {
                key: "ssr-fit-chart".into(),
                size: [w, h],
                rgba: Arc::new(rgba),
            },
            [360.0, 170.0],
        )
    })
}

/// The back-face rule along P's ray (screen_space_reflection.glsl:245-250): a hit `backSweep` SSR pixels from
/// the start, taken to face away from the ray, is rejected inside the two-pixel box around the start and never
/// tested beyond it. The point is drawn on the ray, and a strip shows the box and where the point lies.
pub(super) fn back_face(
    state: &ShotState,
    content: &mut FrameContent,
    screen: &cpu::Screen,
    t: &trace::Trace,
) {
    let s = state.p("backSweep");
    if s < 0.0 || t.exit == Exit::NotTraced {
        return;
    }
    let fs = Vec2::new(screen.size[0] as f32, screen.size[1] as f32);
    let ray = t.screen_ray.truncate();
    // The t that is `s` SSR pixels along the ray on screen, and the shader's box test at it.
    let per_t = (ray * fs).length().max(1e-6);
    let tt = s / per_t;
    let rule = state.p("backRule") >= 0.5;
    let checked = rule && (ray * tt).abs().cmplt(Vec2::splat(2.0) / fs).all();
    let at = screen.world_of(t.screen_pos + t.screen_ray * tt);
    let start = screen.world_of(t.screen_pos);
    let c = if checked {
        palette::REFLECTED
    } else {
        palette::AXIS_Y
    };
    content.draw.segment(start, at, c, 1.0, 3.0, 0);
    content.draw.point(at, 7.0, c, 1.0);
    let (w, h) = (360usize, 50usize);
    let mut rgba = chart(w, h);
    let max_px = 8.0;
    for x in 0..w {
        let px = x as f32 / (w - 1) as f32 * max_px;
        let inside = (ray * (px / per_t))
            .abs()
            .cmplt(Vec2::splat(2.0) / fs)
            .all();
        let col = if inside && rule {
            palette::REFLECTED
        } else {
            DIM
        };
        for y in 18..32 {
            plot(&mut rgba, w, h, x as i32, y, col);
        }
    }
    let mx = (s.min(max_px) / max_px * (w - 1) as f32) as i32;
    for y in 4..46 {
        plot(&mut rgba, w, h, mx, y, palette::SELECTED);
    }
    let verdict = if !rule {
        tr_with("demos-back_off", lab::args!(px = format!("{s:.1}")))
    } else if checked {
        tr_with("demos-back_rejected", lab::args!(px = format!("{s:.1}")))
    } else {
        tr_with("demos-back_unchecked", lab::args!(px = format!("{s:.1}")))
    };
    content.insets.push(PictureInset {
        corner: Corner::BottomRight,
        x_axis: tr("demos-back_axis").to_string(),
        lines: vec![(verdict, c)],
        ..PictureInset::new(
            tr("demos-back_title"),
            Picture::Pixels {
                key: "ssr-back-strip".into(),
                size: [w, h],
                rgba: Arc::new(rgba),
            },
            [360.0, 50.0],
        )
    });
}

/// Where `p` (world) lands on a camera's screen, as a uv with y down; None behind it.
fn uv_of(world_to_clip: Mat4, p: Vec3) -> Option<Vec2> {
    let c = world_to_clip * p.extend(1.0);
    (c.w > 0.0).then(|| Vec2::new(c.x / c.w * 0.5 + 0.5, 0.5 - c.y / c.w * 0.5))
}

/// P's hit reprojected into last frame's camera (screen_space_reflection.glsl:224-229): last frame's camera is
/// this one turned back by `lastYaw` degrees about its target (the lab's stand-in for one frame of motion, made
/// large enough to see). Its eye and its line to the hit are drawn, and last frame's image is shown with the
/// texel the trace reads there and where the hit is now.
pub(super) fn reproject(
    state: &ShotState,
    content: &mut FrameContent,
    screen: &cpu::Screen,
    t: &trace::Trace,
) {
    let d = state.p("lastYaw");
    if d <= 0.0 || t.exit == Exit::NotTraced {
        return;
    }
    let viewport = content.uniforms.camera.viewport_size;
    let mut last_cam = state.camera;
    last_cam.yaw -= d;
    let last = last_cam.camera_data(viewport);
    let now = &content.uniforms.camera;
    let hit = screen.world_of(t.hit_screen);
    let (Some(uv_last), Some(uv_now)) = (
        uv_of(Mat4::from_cols_array_2d(&last.world_to_clip), hit),
        uv_of(Mat4::from_cols_array_2d(&now.world_to_clip), hit),
    ) else {
        return;
    };
    let eye = Vec3::from(last.position);
    content.draw.point(eye, 8.0, palette::INCIDENT, 1.0);
    content
        .draw
        .segment(eye, hit, palette::INCIDENT, 1.0, 2.5, FLAG_DASHED);
    // The eye is often off screen: the line's label sits a quarter of the way from the hit.
    content.draw.text(
        hit + (eye - hit) * 0.45,
        tr("demos-label_last_camera"),
        palette::INCIDENT,
        [0.5, 1.3],
    );
    let Some(img) = cpu::level(
        content,
        "godot_ssr.lastFrame",
        Source::image("lastFrame", 0),
    ) else {
        return;
    };
    content
        .insets
        .push(history_inset(&img, uv_last, uv_now, hit, d));
}

/// Last frame's image (tone-mapped for the inset), the texel read at `uv_last` outlined and linked to the hit,
/// and where the hit is now.
fn history_inset(img: &Image, uv_last: Vec2, uv_now: Vec2, hit: Vec3, d: f32) -> PictureInset {
    let (w, h) = (img.size[0] as usize, img.size[1] as usize);
    let mut rgba = vec![0u8; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            // Each texel tone-mapped (x / (1 + x)) and gamma-encoded for the inset.
            let c: Vec4 = img.load([x as i32, y as i32]);
            let k = (y * w + x) * 4;
            for i in 0..3 {
                let v = c[i].max(0.0);
                rgba[k + i] = ((v / (1.0 + v)).powf(1.0 / 2.2) * 255.0) as u8;
            }
            rgba[k + 3] = 255;
        }
    }
    let texel = (uv_last * Vec2::new(w as f32, h as f32)).floor();
    let cell = |uv: Vec2| {
        let s = Vec2::new(6.0 / w as f32, 6.0 / h as f32);
        [uv.x - s.x, uv.y - s.y, uv.x + s.x, uv.y + s.y]
    };
    PictureInset {
        corner: Corner::BottomRight,
        outlines: vec![
            Outline {
                rect: cell(uv_last),
                color: palette::SELECTED,
                width: 3.0,
                outside: false,
            },
            Outline {
                rect: cell(uv_now),
                color: DIM,
                width: 2.0,
                outside: false,
            },
        ],
        marker: Some(Marker {
            at: [uv_last.x, 1.0 - uv_last.y],
            color: palette::SELECTED,
            label: String::new(),
        }),
        link: Some(Link {
            from: hit,
            outline: 0,
        }),
        lines: vec![
            (
                tr_with(
                    "demos-reproject_moved",
                    lab::args!(
                        deg = format!("{d:.0}"),
                        px = format!(
                            "{:.0}",
                            ((uv_last - uv_now) * Vec2::new(w as f32, h as f32)).length()
                        )
                    ),
                ),
                palette::INCIDENT,
            ),
            (
                tr_with(
                    "demos-reproject_texel",
                    lab::args!(x = texel.x as i32, y = texel.y as i32,),
                ),
                palette::SELECTED,
            ),
        ],
        ..PictureInset::new(
            tr("views-last_frame"),
            Picture::Pixels {
                key: "ssr-history".into(),
                size: [w, h],
                rgba: Arc::new(rgba),
            },
            [320.0, 320.0 * h as f32 / w as f32],
        )
    }
}
