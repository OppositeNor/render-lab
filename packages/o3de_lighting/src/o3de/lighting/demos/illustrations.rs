//! The lab's illustrations of what the port does not run: a quad light's linearly transformed cosine, the local
//! lights' shadow atlas, and a sphere light's six shadow frusta. Each is drawn from the shot's values; the
//! atlas's placement ports ShadowmapAtlas::Finalize and GetOrigin (Copyright (c) Contributors to the Open 3D
//! Engine Project, Apache-2.0 OR MIT).

use super::*;

/// The quad's half extents (metres) and its height above the inspected point.
const QUAD_HALF: [f32; 2] = [0.6, 0.35];
const QUAD_HEIGHT: f32 = 1.0;
/// The hemisphere's radius around the point, metres.
const HEMISPHERE: f32 = 0.55;

/// Two unit vectors spanning the plane at right angles to `n`.
fn tangents(n: Vec3) -> (Vec3, Vec3) {
    let t = if n.y.abs() < 0.9 {
        Vec3::Y
    } else {
        Vec3::X
    }
    .cross(n)
    .normalize();
    (t, n.cross(t))
}

/// The quad's corners, above the point `p` with normal `n`, in winding order.
pub fn quad_corners(p: Vec3, n: Vec3) -> [Vec3; 4] {
    let (t, b) = tangents(n);
    let c = p + n * QUAD_HEIGHT + t * 0.3;
    let [x, y] = QUAD_HALF;
    [
        c - t * x - b * y,
        c + t * x - b * y,
        c + t * x + b * y,
        c - t * x + b * y,
    ]
}

/// The clamped cosine's integral over the quad, seen from `p` with normal `n`: the polygon's closed form
/// (Lambert's), one arc term per edge, over pi. It is the irradiance a unit-radiance quad gives, over pi.
pub fn cosine_integral(p: Vec3, n: Vec3, corners: &[Vec3; 4]) -> f32 {
    let v: Vec<Vec3> = corners.iter().map(|q| (*q - p).normalize()).collect();
    let sum: f32 = (0..4)
        .map(|i| {
            let (a, b) = (v[i], v[(i + 1) % 4]);
            a.dot(b).clamp(-1.0, 1.0).acos() * n.dot(a.cross(b).normalize_or_zero())
        })
        .sum();
    (sum / (2.0 * std::f32::consts::PI)).abs()
}

/// A lobe about `axis` in the plane it shares with `side`: a cosine raised to `power`, `length` at its peak.
fn lobe(p: Vec3, (axis, side): (Vec3, Vec3), power: f32, length: f32) -> Vec<Vec3> {
    (0..=48)
        .map(|k| {
            let theta = (k as f32 / 48.0 - 0.5) * std::f32::consts::PI;
            p + (axis * theta.cos() + side * theta.sin())
                * length
                * theta.cos().max(0.0).powf(power)
        })
        .collect()
}

/// The quad light: the quad, its corners projected onto the point's hemisphere, the hemisphere's rim, and two
/// lobes side by side, the plain cosine about the normal and the same cosine warped by `warp` toward the
/// reflection of the view, with the cosine's closed-form integral over the quad.
pub(super) fn quad_light(
    content: &mut FrameContent,
    (p, n): (Vec3, Vec3),
    eye: Vec3,
    (alpha, warp): (f32, f32),
) {
    let corners = quad_corners(p, n);
    let draw = &mut content.draw;
    let mut ring = corners.to_vec();
    ring.push(corners[0]);
    draw.polyline(&ring, LIGHT, alpha, 4.0);
    for (i, q) in corners.iter().enumerate() {
        // Each edge of the quad, carried onto the hemisphere: the spherical polygon the integral runs over.
        let next = corners[(i + 1) % 4];
        let arc: Vec<Vec3> = (0..=24)
            .map(|k| p + (q.lerp(next, k as f32 / 24.0) - p).normalize() * HEMISPHERE)
            .collect();
        draw.polyline(&arc, LIGHT, alpha, 3.0);
        draw.segment(p, *q, LIGHT, alpha * 0.5, 1.5, FLAG_DASHED);
    }
    let (t, b) = tangents(n);
    let rim: Vec<Vec3> = (0..=64)
        .map(|k| {
            let a = k as f32 / 64.0 * std::f32::consts::TAU;
            p + (t * a.cos() + b * a.sin()) * HEMISPHERE
        })
        .collect();
    draw.polyline(&rim, palette::text_dim(), alpha, 2.0);
    let v = (eye - p).normalize();
    let r = (2.0 * n.dot(v) * n - v).normalize();
    // The lobes are drawn in the plane of the normal and the picture's horizontal, so they face the viewer; the
    // warped lobe leans toward the reflection's side of that plane as it narrows.
    let side = n.cross(v).try_normalize().unwrap_or(t);
    let lean = if r.dot(side) >= 0.0 {
        1.0
    } else {
        -1.0
    };
    let axis = (n * (warp * 0.7).cos() + side * lean * (warp * 0.7).sin()).normalize();
    let warped_side = side - axis * side.dot(axis);
    let warped_side = warped_side.try_normalize().unwrap_or(side);
    let cosine = lobe(p, (n, side), 1.0, HEMISPHERE * 1.2);
    let warped = lobe(p, (axis, warped_side), 1.0 + 6.0 * warp, HEMISPHERE * 1.4);
    draw.polyline(&cosine, palette::text(), alpha, 3.0);
    draw.polyline(&warped, SPECULAR, alpha, 3.5);
    let labels = [
        (
            p + n * HEMISPHERE * 1.25,
            tr("demos-label_cosine_lobe").to_string(),
            palette::text(),
        ),
        (
            p + axis * HEMISPHERE * 1.45,
            tr("demos-label_warped_lobe").to_string(),
            SPECULAR,
        ),
        (
            corners[1],
            format!(
                "{} = {:.3}",
                tr("demos-quad_integral"),
                cosine_integral(p, n, &corners)
            ),
            LIGHT,
        ),
    ];
    for (anchor, text, color) in labels {
        content.labels.push(Label {
            anchor,
            text,
            color,
        });
    }
}

/// The example set of local-light shadow maps the atlas illustration places: a spot light's 1024, a sphere
/// light's six cube faces at 512, and two disk lights' 256s (the engine's sizes run from 256 to 2048).
const ATLAS_SET: [u32; 9] = [1024, 512, 512, 512, 512, 512, 512, 256, 256];

/// Where ShadowmapAtlas::Finalize puts each map: largest first, each size one digit deeper in a base-4 location
/// whose first digit is the array slice; GetOrigin turns a location into the slice and its texel origin.
pub fn atlas_origins(sizes: &[u32]) -> Vec<(u32, [u32; 2])> {
    let base = sizes.iter().copied().max().unwrap_or(256);
    let mut origins = vec![(0, [0, 0]); sizes.len()];
    let mut location: Vec<u32> = Vec::new();
    let mut size = base;
    while size >= 256 {
        // Each halving of the size extends the location by one digit.
        location.push(0);
        for (i, _) in sizes.iter().enumerate().filter(|(_, s)| **s == size) {
            let mut diff = base;
            let mut origin = [0, 0];
            for digit in &location[1..] {
                // Each digit picks a quarter of the square above it: bit 0 the right half, bit 1 the lower.
                diff /= 2;
                origin[0] += if digit & 1 != 0 {
                    diff
                } else {
                    0
                };
                origin[1] += if digit & 2 != 0 {
                    diff
                } else {
                    0
                };
            }
            origins[i] = (location[0], origin);
            succeed(&mut location);
        }
        size /= 2;
    }
    origins
}

/// ShadowmapAtlas::SucceedLocation: the next location, counting in base 4 after the slice digit, carrying into a
/// new slice.
fn succeed(location: &mut [u32]) {
    for digit in location.iter_mut().skip(1).rev() {
        if *digit < 3 {
            *digit += 1;
            return;
        }
        *digit = 0;
    }
    location[0] += 1;
}

/// The six cube faces' colours, shared by the frusta and their maps in the atlas: +X, -X, +Y, -Y, +Z, -Z.
const FACE_COLORS: [Color32; 6] = [
    palette::rgb(0xff5d5d),
    palette::rgb(0xff9a3c),
    palette::rgb(0x6be36b),
    palette::rgb(0x3cd6c8),
    palette::rgb(0x5d8bff),
    palette::rgb(0xc77dff),
];

/// Each example map's colour: the spot light's 1024 orange-white, the six cube faces in their face colours,
/// the disks' 256s grey-green.
fn atlas_color(i: usize) -> Color32 {
    match i {
        0 => palette::rgb(0xffd98a),
        1..=6 => FACE_COLORS[i - 1],
        _ => palette::rgb(0xb8d8a8),
    }
}

/// The atlas on the back wall: one square per array slice, labelled below it, and the first `count` maps of the
/// example set placed in it, each outlined and hatched in its light's colour (a cube face in its face's).
pub(super) fn shadow_atlas(content: &mut FrameContent, count: f32) {
    let origins = atlas_origins(&ATLAS_SET);
    let base = ATLAS_SET[0] as f32;
    let slices = origins.iter().map(|o| o.0).max().unwrap_or(0) + 1;
    let side = 1.1;
    let left = -(slices as f32 * (side + 0.15) - 0.15) / 2.0;
    let (wall, top) = (-2.96, 2.6);
    // A rectangle on the wall, from slice `s`'s texel origin and size, top down: its corners, then its hatching.
    let rect = |s: u32, o: [u32; 2], size: f32, inset: f32| {
        let x0 = left + s as f32 * (side + 0.15) + o[0] as f32 / base * side + inset;
        let y0 = top - o[1] as f32 / base * side - inset;
        (x0, y0, size / base * side - 2.0 * inset)
    };
    let alpha = count.clamp(0.0, 1.0);
    for s in 0..slices {
        let (x0, y0, w) = rect(s, [0, 0], base, 0.0);
        let square = [(0.0, 0.0), (w, 0.0), (w, -w), (0.0, -w), (0.0, 0.0)]
            .map(|(a, b)| Vec3::new(x0 + a, y0 + b, wall));
        content
            .draw
            .polyline(&square, palette::text_dim(), alpha, 2.0);
        content.labels.push(Label {
            anchor: Vec3::new(x0 + w / 2.0, y0 + 0.12, wall),
            text: format!("{} {s}", tr("demos-atlas_slice")),
            color: palette::text(),
        });
    }
    for (i, ((s, o), size)) in origins.iter().zip(ATLAS_SET).enumerate() {
        // Each map placed so far, the next one fading in: its outline and a hatching across it.
        let shown = (count - i as f32).clamp(0.0, 1.0);
        let (x0, y0, w) = rect(*s, *o, size as f32, 0.02);
        let color = atlas_color(i);
        let outline = [(0.0, 0.0), (w, 0.0), (w, -w), (0.0, -w), (0.0, 0.0)]
            .map(|(a, b)| Vec3::new(x0 + a, y0 + b, wall));
        content.draw.polyline(&outline, color, shown, 4.0);
        for k in 1..8 {
            let f = k as f32 / 8.0 * w;
            content.draw.segment(
                Vec3::new(x0 + f, y0, wall),
                Vec3::new(x0, y0 - f, wall),
                color,
                shown * 0.6,
                2.0,
                0,
            );
            content.draw.segment(
                Vec3::new(x0 + w, y0 - f, wall),
                Vec3::new(x0 + f, y0 - w, wall),
                color,
                shown * 0.6,
                2.0,
                0,
            );
        }
    }
}

/// A sphere light's six shadow frusta, one per cube face, each with a 91 degree field of view, so neighbouring
/// faces overlap at their seams; each in its face's colour, the colour of its map in the atlas.
pub(super) fn point_frusta(draw: &mut DrawLists, light: Vec3, (alpha, picked): (f32, i32)) {
    let spread = (45.5f32).to_radians().tan();
    let length = 0.6;
    let axes = [Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z];
    for (face, (axis, color)) in axes.iter().zip(FACE_COLORS).enumerate() {
        // A picked cube-face map (atlas maps 1 to 6) stands out; the other faces fade back.
        let (alpha, width) = match picked {
            1..=6 if face as i32 == picked - 1 => (alpha, 6.0),
            1..=6 => (alpha * 0.2, 2.0),
            _ => (alpha, 3.5),
        };
        let (u, v) = tangents(*axis);
        let far: Vec<Vec3> = [
            (-1.0, -1.0),
            (1.0, -1.0),
            (1.0, 1.0),
            (-1.0, 1.0),
            (-1.0, -1.0),
        ]
        .iter()
        .map(|(a, b)| light + (*axis + (u * *a + v * *b) * spread) * length)
        .collect();
        draw.polyline(&far, color, alpha, width);
        for q in &far[..4] {
            draw.segment(light, *q, color, alpha * 0.8, 2.0, 0);
        }
    }
}

/// Burley's normalised diffusion profile (Christensen and Burley 2015), the curve subsurface scattering blurs
/// the diffuse light with: R(r) = (exp(-r / d) + exp(-r / 3d)) / (8 pi d r), d the scatter distance.
pub fn burley(r: f32, d: f32) -> f32 {
    ((-r / d).exp() + (-r / (3.0 * d)).exp()) / (8.0 * std::f32::consts::PI * d * r.max(1e-4))
}

/// The scatter distance per channel the narration gives as EnhancedPBR's default, millimetres.
const SCATTER_MM: [f32; 3] = [8.0, 2.16, 1.04];

/// The profile drawn as three curves above the teapot, red, green, and blue, against the distance from where
/// light entered (0 to 20 mm along 0.6 m), each curve's height r R(r) scaled to its peak, on a dark backing, with
/// its axis and channels labelled; `scale` multiplies the scatter distances.
pub(super) fn sss_profile(content: &mut FrameContent, alpha: f32, scale: f32) {
    let origin = Vec3::new(-0.6, 0.62, -1.9);
    let (span, height, max_mm) = (0.6, 0.32, 20.0);
    for k in 0..24 {
        // A dark backing behind the chart, drawn as close strokes.
        let y = -0.06 + k as f32 / 23.0 * (height + 0.1);
        content.draw.segment(
            origin + Vec3::new(-0.05, y, -0.01),
            origin + Vec3::new(span + 0.05, y, -0.01),
            palette::rgb(0x0b1020),
            alpha * 0.85,
            9.0,
            0,
        );
    }
    content.draw.segment(
        origin,
        origin + Vec3::X * span,
        palette::text(),
        alpha,
        2.0,
        FLAG_ARROW,
    );
    content.draw.segment(
        origin,
        origin + Vec3::Y * height,
        palette::text(),
        alpha,
        2.0,
        0,
    );
    content.labels.push(Label {
        anchor: origin + Vec3::X * span * 0.5 - Vec3::Y * 0.05,
        text: tr("demos-label_distance_mm").into(),
        color: palette::text(),
    });
    let colors = [palette::AXIS_X, palette::AXIS_Y, palette::AXIS_Z];
    for ((d, color), name) in SCATTER_MM.iter().zip(colors).zip(["R", "G", "B"]) {
        // r R(r): the share of light leaving at radius r, which the blur's footprint follows.
        let d = d * scale;
        let samples: Vec<(f32, f32)> = (1..=96)
            .map(|k| {
                let r = k as f32 / 96.0 * max_mm;
                (r, r * burley(r, d))
            })
            .collect();
        let peak = samples.iter().map(|s| s.1).fold(0.0, f32::max).max(1e-6);
        let curve: Vec<Vec3> = samples
            .iter()
            .map(|(r, v)| origin + Vec3::X * (r / max_mm * span) + Vec3::Y * (v / peak * height))
            .collect();
        content.draw.polyline(&curve, color, alpha, 3.0);
        let top = curve.iter().copied().fold(origin, |a, b| {
            if b.y > a.y {
                b
            } else {
                a
            }
        });
        content.labels.push(Label {
            anchor: top,
            text: name.into(),
            color,
        });
    }
}

/// GaussianNormalized (Atom/RPI/Math.azsli): a normal distribution of mean `mean` and standard deviation `sd`.
fn gaussian(x: f32, mean: f32, sd: f32) -> f32 {
    (-(x - mean) * (x - mean) / (2.0 * sd * sd)).exp() / (sd * std::f32::consts::TAU.sqrt())
}

/// The longitudinal terms of HairLightingEquations.azsli (M_R, M_TT, M_TRT), ported: each a normalised Gaussian
/// of the half angle Lh = (Lr + Li) / 2, its mean the cuticle tilt times 1, 1, and 1.5, its standard deviation
/// roughness A squared times 0.5, 0.5, and 1 (roughness A is the linear roughness squared, HairSurface.azsli).
pub fn hair_longitudinal(lh: f32, tilt: f32, roughness: f32) -> [f32; 3] {
    let a2 = roughness.powi(4);
    [
        gaussian(lh, tilt, 0.5 * a2),
        gaussian(lh, tilt, 0.5 * a2),
        gaussian(lh, 1.5 * tilt, a2),
    ]
}

/// A hair strand through `at` along `strand`, lit from `angle` degrees off the vertical in the picture's plane,
/// and its three lobes as the longitudinal terms shape them: each drawn over the outgoing longitudinal angle Lr,
/// in the plane of the strand and the light (R and TRT on the light's side, TT through the strand to the far
/// side), its length the term at Lh = (Lr + Li) / 2 over R's peak, its peak direction an arrow, each labelled.
/// The azimuthal terms (N_R, N_TT, N_TRT) are left out of the drawing.
pub(super) fn hair_lobes(
    content: &mut FrameContent,
    (at, strand): (Vec3, Vec3),
    angle: f32,
    (alpha, tilt, roughness): (f32, f32, f32),
) {
    let strand = strand.normalize();
    let a = angle.to_radians();
    let to_light = (Vec3::new(a.sin(), a.cos(), 0.15)).normalize();
    let draw = &mut content.draw;
    draw.segment(
        at - strand * 1.1,
        at + strand * 1.1,
        palette::text(),
        alpha,
        5.0,
        0,
    );
    draw.segment(
        at + to_light * 1.1,
        at,
        LIGHT,
        alpha,
        3.0,
        FLAG_ARROW | FLAG_DASHED,
    );
    // Li, the light's angle to the strand's normal plane, and the light's side within that plane.
    let li = to_light.dot(strand).clamp(-1.0, 1.0).asin();
    let side = (to_light - strand * to_light.dot(strand))
        .try_normalize()
        .unwrap_or(Vec3::Y);
    let peak = gaussian(0.0, 0.0, 0.5 * roughness.powi(4));
    let lobes = [
        ("R", SPECULAR),
        ("TT", palette::rgb(0x5d8bff)),
        ("TRT", DIFFUSE),
    ];
    let mut tips = Vec::new();
    for (lobe, (_, color)) in lobes.iter().enumerate() {
        // TT leaves through the strand, to the side away from the light.
        let w = if lobe == 1 {
            -side
        } else {
            side
        };
        let curve: Vec<(Vec3, f32)> = (0..=144)
            .map(|k| {
                let lr = (k as f32 / 144.0 - 0.5) * std::f32::consts::PI;
                let m = hair_longitudinal((lr + li) * 0.5, tilt, roughness)[lobe] / peak;
                let d = w * lr.cos() + strand * lr.sin();
                (at + d * m.min(1.5) * 1.1, m)
            })
            .collect();
        let points: Vec<Vec3> = curve.iter().map(|c| c.0).collect();
        content.draw.polyline(&points, *color, alpha, 4.0);
        let tip = curve
            .iter()
            .fold((at, 0.0), |a, b| {
                if b.1 > a.1 {
                    *b
                } else {
                    a
                }
            })
            .0;
        content
            .draw
            .segment(at, tip, *color, alpha, 2.5, FLAG_ARROW);
        tips.push(tip);
    }
    let mut labels = vec![
        (
            at + strand * 1.1,
            tr("demos-label_strand").to_string(),
            palette::text(),
        ),
        (
            at + to_light * 1.1,
            tr("demos-label_light").to_string(),
            LIGHT,
        ),
    ];
    for ((name, color), tip) in lobes.iter().zip(tips) {
        labels.push((tip, (*name).to_string(), *color));
    }
    for (anchor, text, color) in labels {
        content.labels.push(Label {
            anchor,
            text,
            color,
        });
    }
}

/// The lab's illustration of the diffuse probe grid: a grid of probes through the room joined to their
/// neighbours, and rays traced from one of them to the surfaces they hit (the gem's tracing is not in the port).
pub(super) fn probe_grid(content: &mut FrameContent, alpha: f32, cage: Option<Vec3>) {
    let spacing = 1.5;
    let at = |i: i32, j: i32, k: i32| {
        Vec3::new(
            -2.25 + i as f32 * spacing,
            0.75 + j as f32 * spacing,
            -2.25 + k as f32 * spacing,
        )
    };
    for i in 0..4 {
        for j in 0..2 {
            for k in 0..4 {
                // Each probe, and the grid's edges to its neighbours along +x, +y, and +z.
                let p = at(i, j, k);
                content.draw.point(p, 10.0, LIGHT, alpha);
                for (di, dj, dk) in [(1, 0, 0), (0, 1, 0), (0, 0, 1)] {
                    if i + di < 4 && j + dj < 2 && k + dk < 4 {
                        content.draw.segment(
                            p,
                            at(i + di, j + dj, k + dk),
                            LIGHT,
                            alpha * 0.45,
                            1.5,
                            0,
                        );
                    }
                }
            }
        }
    }
    if let Some(p) = cage {
        // The cell around the picked point: its eight probes, joined along the cell's twelve edges.
        let [i, j, k] = probe_cell(p);
        for (a, b) in CAGE_EDGES {
            let (pa, pb) = (at(i + a.0, j + a.1, k + a.2), at(i + b.0, j + b.1, k + b.2));
            content
                .draw
                .segment(pa, pb, palette::SELECTED, alpha, 4.0, 0);
        }
    }
    let from = at(1, 0, 1);
    let mut first_hit = None;
    for r in 0..12 {
        // A spiral of directions over the sphere, each traced to its first surface.
        let t = (r as f32 + 0.5) / 12.0;
        let (z, phi) = (1.0 - 2.0 * t, r as f32 * 2.399_963);
        let sz = (1.0 - z * z).sqrt();
        let d = Vec3::new(sz * phi.cos(), z, sz * phi.sin());
        let hit = lab::scene::trace_scene(
            &content.objects,
            from,
            d,
            10.0,
            lab::scene::ids::PANEL as u32,
        );
        let end = from
            + d * if hit.is_hit() {
                hit.t
            } else {
                3.0
            };
        content
            .draw
            .segment(from, end, palette::text(), alpha * 0.8, 2.0, 0);
        content.draw.point(end, 6.0, palette::text(), alpha);
        first_hit.get_or_insert(end);
    }
    content.draw.point(from, 16.0, palette::text(), alpha);
    content.labels.push(Label {
        anchor: from,
        text: tr("demos-label_probe").into(),
        color: LIGHT,
    });
    if let Some(end) = first_hit {
        content.labels.push(Label {
            anchor: end,
            text: tr("demos-label_hit").into(),
            color: palette::text(),
        });
    }
}

/// A double-sided leaf, a vertical quad beside the teapot, with its mesh (vertex) normal and its shading normal,
/// which the forward pass turns toward the viewer on a back face while the vertex normal stays as authored.
pub(super) fn leaf_normals(content: &mut FrameContent, eye: Vec3, alpha: f32) {
    let (centre, normal) = (Vec3::new(0.3, 0.45, -1.75), Vec3::X);
    let (u, v) = (Vec3::Z * 0.34, Vec3::Y * 0.28);
    let corners = [
        centre - u - v,
        centre + u - v,
        centre + u + v,
        centre - u + v,
        centre - u - v,
    ];
    content.draw.polyline(&corners, DIFFUSE, alpha, 3.5);
    let shading = if normal.dot(eye - centre) >= 0.0 {
        normal
    } else {
        -normal
    };
    let (vertex_tip, shading_tip) = (
        centre + normal * 0.32 + Vec3::Y * 0.06,
        centre + shading * 0.3 - Vec3::Y * 0.06,
    );
    content.draw.segment(
        centre + Vec3::Y * 0.05,
        vertex_tip,
        palette::text_dim(),
        alpha,
        4.0,
        FLAG_ARROW,
    );
    content.draw.segment(
        centre - Vec3::Y * 0.05,
        shading_tip,
        palette::NORMAL,
        alpha,
        4.0,
        FLAG_ARROW,
    );
    let labels = [
        (centre + v, tr("demos-label_leaf"), DIFFUSE),
        (
            vertex_tip,
            tr("demos-label_vertex_normal"),
            palette::text_dim(),
        ),
        (
            shading_tip,
            tr("demos-label_shading_normal"),
            palette::NORMAL,
        ),
    ];
    for (anchor, text, color) in labels {
        content.labels.push(Label {
            anchor,
            text: text.into(),
            color,
        });
    }
}

/// The ground's height under `(x, z)`, by a ray cast down onto the scene (the terrain's surface), or 0.
fn ground(objects: &[ObjectData], x: f32, z: f32) -> f32 {
    let hit = lab::scene::trace_scene(
        objects,
        Vec3::new(x, 200.0, z),
        -Vec3::Y,
        400.0,
        lab::scene::ids::PANEL as u32,
    );
    if hit.is_hit() {
        200.0 - hit.t
    } else {
        0.0
    }
}

/// A square of half side `half` around `centre`, laid on the ground.
fn ground_square(objects: &[ObjectData], centre: Vec3, half: f32) -> Vec<Vec3> {
    let corners = [
        (-1.0, -1.0),
        (1.0, -1.0),
        (1.0, 1.0),
        (-1.0, 1.0),
        (-1.0, -1.0),
    ];
    corners
        .windows(2)
        .flat_map(|w| {
            (0..16).map(move |k| {
                let t = k as f32 / 16.0;
                (
                    w[0].0 + (w[1].0 - w[0].0) * t,
                    w[0].1 + (w[1].1 - w[0].1) * t,
                )
            })
        })
        .chain(std::iter::once((-1.0, -1.0)))
        .map(|(a, b)| {
            let (x, z) = (centre.x + a * half, centre.z + b * half);
            Vec3::new(x, ground(objects, x, z) + 0.05, z)
        })
        .collect()
}

/// The lab's illustration of the terrain's clipmaps: a stack of squares on the ground around the point below the
/// camera, each half the size of the one before and holding the same texels, so the finest is nearest the camera;
/// each labelled with its level and side.
pub(super) fn clipmap_rings(content: &mut FrameContent, eye: Vec3, alpha: f32) {
    let centre = Vec3::new(eye.x, 0.0, eye.z);
    for level in 0..5 {
        // Level 0 is the widest; each next level halves the side and keeps the texel count.
        let half = 24.0 / 2f32.powi(level);
        let square = ground_square(&content.objects, centre, half);
        let color = [
            palette::AXIS_X,
            LIGHT,
            palette::AXIS_Y,
            palette::AXIS_Z,
            SPECULAR,
        ][level as usize];
        content.draw.polyline(&square, color, alpha, 4.5);
        content.labels.push(Label {
            anchor: square[8],
            text: format!(
                "{} {level} · {:.0} m",
                tr("demos-label_clipmap"),
                2.0 * half
            ),
            color,
        });
    }
}

/// The lab's illustration of where detail materials apply: within a render distance of the camera, fading out
/// over its outer eighth (TerrainDetailMaterialManager's defaults, 512 m and 64 m, scaled to the lab's hills).
pub(super) fn detail_range(draw: &mut DrawLists, objects: &[ObjectData], eye: Vec3, alpha: f32) {
    let radius = 16.0;
    for (r, color) in [
        (radius, DIFFUSE),
        (radius * (1.0 - 64.0 / 512.0), palette::text_dim()),
    ] {
        // The render distance, and where the fade begins.
        let ring: Vec<Vec3> = (0..=96)
            .map(|k| {
                let a = k as f32 / 96.0 * std::f32::consts::TAU;
                let (x, z) = (eye.x + r * a.cos(), eye.z + r * a.sin());
                Vec3::new(x, ground(objects, x, z) + 0.05, z)
            })
            .collect();
        draw.polyline(&ring, color, alpha, 2.5);
    }
}

/// PhotometricValue's EV100 luminance to nits: 12.5 times two to the EV100, over ISO 100 and a one-second
/// shutter (Ev100LightMeterConstantLuminance, Ev100Iso, Ev100ShutterSpeed).
pub fn ev100_to_nits(ev100: f32) -> f32 {
    12.5 * 2f32.powf(ev100) / (100.0 * 1.0)
}

/// The emission's conversion read off as a label above the sphere: the EV100 a material authors, and the nits it
/// becomes.
pub(super) fn ev100_label(content: &mut FrameContent, ev100: f32) {
    content.labels.push(Label {
        anchor: Vec3::new(-1.05, 1.5, -0.55),
        text: format!(
            "EV100 {ev100:.1} → {:.2} {}",
            ev100_to_nits(ev100),
            tr("demos-nits")
        ),
        color: LIGHT,
    });
}

/// The room's simple point lights, the first `cap` of them bright and numbered (the ones a mobile pass counts),
/// the rest small grey dots.
pub(super) fn numbered_lights(
    content: &mut FrameContent,
    shading: &Shading,
    (alpha, cap): (f32, f32),
) {
    let mut positions = vec![shading.light.position];
    positions.extend(shading.extra_lights.iter().map(|l| l.position));
    for (i, p) in positions.iter().enumerate() {
        if (i as f32) < cap.round() {
            // A counted light: its marker and its number in the pass's list.
            content.draw.point(*p, 16.0, palette::text(), alpha);
            content.draw.point(*p, 11.0, LIGHT, alpha);
            content.labels.push(Label {
                anchor: *p,
                text: format!("{}", i + 1),
                color: LIGHT,
            });
        } else {
            content
                .draw
                .point(*p, 6.0, palette::text_dim(), alpha * 0.7);
        }
    }
}

/// A corner of a grid cell, as its offsets from the cell's lowest probe.
type CellCorner = (i32, i32, i32);

/// The twelve edges of a grid cell, as pairs of its corners.
const CAGE_EDGES: [(CellCorner, CellCorner); 12] = [
    ((0, 0, 0), (1, 0, 0)),
    ((0, 1, 0), (1, 1, 0)),
    ((0, 0, 1), (1, 0, 1)),
    ((0, 1, 1), (1, 1, 1)),
    ((0, 0, 0), (0, 1, 0)),
    ((1, 0, 0), (1, 1, 0)),
    ((0, 0, 1), (0, 1, 1)),
    ((1, 0, 1), (1, 1, 1)),
    ((0, 0, 0), (0, 0, 1)),
    ((1, 0, 0), (1, 0, 1)),
    ((0, 1, 0), (0, 1, 1)),
    ((1, 1, 0), (1, 1, 1)),
];

/// The illustrated grid's cell holding `p`: the index of its lowest probe (the grid's 4 x 2 x 4 probes, 1.5 m
/// apart from (-2.25, 0.75, -2.25)), clamped to the grid's cells.
pub fn probe_cell(p: Vec3) -> [i32; 3] {
    let f = ((p - Vec3::new(-2.25, 0.75, -2.25)) / 1.5).floor();
    [
        f.x.clamp(0.0, 2.0) as i32,
        f.y.clamp(0.0, 0.0) as i32,
        f.z.clamp(0.0, 2.0) as i32,
    ]
}

/// The illustrated atlas's layout on the back wall: each map's rectangle, `(left, top, side)` in metres.
pub fn atlas_rects() -> Vec<(f32, f32, f32)> {
    let origins = atlas_origins(&ATLAS_SET);
    let base = ATLAS_SET[0] as f32;
    let slices = origins.iter().map(|o| o.0).max().unwrap_or(0) + 1;
    let side = 1.1;
    let left = -(slices as f32 * (side + 0.15) - 0.15) / 2.0;
    origins
        .iter()
        .zip(ATLAS_SET)
        .map(|((s, o), size)| {
            let x0 = left + *s as f32 * (side + 0.15) + o[0] as f32 / base * side;
            (
                x0,
                2.6 - o[1] as f32 / base * side,
                size as f32 / base * side,
            )
        })
        .collect()
}

/// The atlas map under the back wall's point `(x, y)`, or -1.
pub fn atlas_map_at(x: f32, y: f32) -> f32 {
    atlas_rects()
        .iter()
        .position(|(x0, y0, w)| x >= *x0 && x <= x0 + w && y <= *y0 && y >= y0 - w)
        .map_or(-1.0, |i| i as f32)
}
