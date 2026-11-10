//! Chapters 8 to 10: the followed probe, its rays and march, its history and maps; a pixel and its probes.

use super::*;

/// Chapters 8 and 9: the followed probe, its rays, one ray's march, its SH
/// history, and its octahedral maps.
pub(super) fn probes_demo(
    state: &ShotState,
    world: &World,
    content: &mut FrameContent,
    maps: bool,
) {
    let field = world.field();
    let y_mult = world.cascades.settings.y_mult();
    let unsquash = |p: Vec3| Vec3::new(p.x, p.y / y_mult, p.z);
    let probe_pos = probes::probe_position(&field, 0, FOLLOWED_PROBE);
    let real = unsquash(probe_pos);
    ball(content, real, 0.08, palette::PROBE_SELECTED, 1.0);
    probe_rays(state, world, content);
    // One ray's march: spheres of the stored distance.
    march_spheres(state, world, content);
    // The history: mean coefficients after `frames` frames.
    let k = state
        .p("frames")
        .round()
        .clamp(0.0, sim::CONVERGED_FRAMES as f32) as usize;
    let mean = if k == 0 {
        [Vec3::ZERO; SH_SIZE]
    } else {
        world.trace[k - 1]
    };
    // The followed probe's irradiance from its mean coefficients, up and toward the left wall.
    let irradiance = |n: Vec3| -> Vec3 {
        let c = probes::sh_basis(n);
        (0..SH_SIZE)
            .map(|i| mean[i] * c[i] * 4.0 * probes::L_MULT[i])
            .sum()
    };
    let up = irradiance(Vec3::Y);
    let side = irradiance(Vec3::NEG_X);
    let mut rows = vec![
        row(
            tr("demos-probe"),
            format!(
                "{:?} ({:.2}, {:.2}, {:.2}) m",
                FOLLOWED_PROBE.to_array(),
                real.x,
                real.y,
                real.z
            ),
            palette::PROBE_SELECTED,
        ),
        row(
            tr("demos-rays_per_frame"),
            format!(
                "{} ({} {})",
                rays_setting(state, world).ray_count,
                world.integrate.history_size * rays_setting(state, world).ray_count,
                tr("demos-over_history")
            ),
            palette::INCIDENT,
        ),
        row(
            tr("demos-frames_run"),
            format!("{k} / {}", world.integrate.history_size),
            palette::THEME_ACCENT,
        ),
        row(
            "L0",
            format!("{:.3} {:.3} {:.3}", mean[0].x, mean[0].y, mean[0].z),
            palette::text(),
        ),
        row(
            tr("demos-irradiance_up"),
            format!("{:.3} {:.3} {:.3}", up.x, up.y, up.z),
            palette::NORMAL,
        ),
        row(
            tr("demos-irradiance_wall"),
            format!("{:.3} {:.3} {:.3}", side.x, side.y, side.z),
            Color32::from_rgb(230, 110, 90),
        ),
    ];
    if state.p("historyChart") > 0.5 {
        history_chart(world, k, content);
    }
    if maps {
        maps_inset(state, world, content);
        texel_directions(state, real, content);
        rows.extend(gain_rows(state));
    }
    content.readout = Some(Readout {
        title: if maps {
            tr("demos-maps")
        } else {
            tr("demos-probes")
        }
        .into(),
        subtitle: tr("demos-probes_subtitle").into(),
        formula: None,
        formula_lit: None,
        rows,
    });
}

/// The probe settings with the ray count the shot's `giRayCount` picks from the code's table (gi.cpp
/// `update_probes`): index 1, the editor's "16", is 8 rays.
fn rays_setting(state: &ShotState, world: &World) -> probes::Integrate {
    let index = (state.p("giRayCount").round().max(0.0) as usize).min(sim::RAY_COUNTS.len() - 1);
    probes::Integrate {
        ray_count: sim::RAY_COUNTS[index],
        ..world.integrate
    }
}

/// The followed probe's rays over `rayFrames` frames: even slots point to -z, odd ones to +z (a frame's eight
/// rays share a half), the last frame's strongest.
pub(super) fn probe_rays(state: &ShotState, world: &World, content: &mut FrameContent) {
    let field = world.field();
    let y_mult = world.cascades.settings.y_mult();
    let unsquash = |p: Vec3| Vec3::new(p.x, p.y / y_mult, p.z);
    let probe_pos = probes::probe_position(&field, 0, FOLLOWED_PROBE);
    let light_refs: Vec<&light::LightTextures> = world.light.iter().collect();
    let lighting = Scene::study_room().lighting;
    let sky_fn = move |d: Vec3| lab::scene::environment_radiance(&lighting, d);
    let sky = probes::Sky::Radiance(&sky_fn);
    let real = unsquash(probe_pos);
    let frames = state.p("rayFrames").round().clamp(0.0, 30.0) as u32;
    // The rays a frame as the scene's ray-count setting gives them (its index into the code's table).
    let integrate = rays_setting(state, world);
    let mut traced = Vec::new();
    if state.p("probeRays") > 0.5 {
        for h in 0..frames {
            // Each of the first `rayFrames` frames' rays, to its hit or a short way into the sky, the newest brightest.
            for ray in
                probes::trace_rays(&field, &light_refs, 0, FOLLOWED_PROBE, h, &integrate, &sky)
            {
                let end = match ray.hit {
                    Some((j, p)) => unsquash(p / field.to_cell(j) + field.offset(j)),
                    None => real + unsquash(ray.direction) * 1.2,
                };
                // Even slots point to -z, odd ones to +z: a frame's eight rays share a half.
                let color = if h % 2 == 0 {
                    palette::INCIDENT
                } else {
                    palette::VIEW
                };
                content.draw.segment(
                    real,
                    end,
                    color,
                    // Under the SH glyph the rays fade, so the glyph reads.
                    if state.p("shGlyph") > 0.5 {
                        0.12
                    } else if h + 1 == frames {
                        1.0
                    } else {
                        0.35
                    },
                    if ray.hit.is_some() {
                        2.0
                    } else {
                        1.0
                    },
                    0,
                );
                if ray.hit.is_some() && state.p("hitNormals") > 0.5 {
                    // The hit's normal from the field's gradient, unsquashed to the room's heights; the first labelled.
                    let n =
                        Vec3::new(ray.normal.x, ray.normal.y * y_mult, ray.normal.z).normalize();
                    content.draw.segment(
                        end,
                        end + n * 0.25,
                        palette::NORMAL,
                        1.0,
                        3.0,
                        lab::frame::FLAG_ARROW,
                    );
                    if traced.iter().all(|r: &probes::Ray| r.hit.is_none()) {
                        label(
                            content,
                            state,
                            end + n * 0.3,
                            tr("demos-hit_normal").into(),
                            palette::NORMAL,
                        );
                    }
                }
                traced.push(ray);
            }
        }
    }
    if state.p("shGlyph") > 0.5 && !traced.is_empty() {
        sh_glyph(&traced, real, content);
    }
}

/// The light the drawn rays give, projected onto the 16 SH bases as the probe pass adds each ray in
/// (`frame_coefficients`), and read back in every direction: points on a small sphere around the probe, each in
/// the colour the coefficients give for its direction.
fn sh_glyph(rays: &[probes::Ray], probe: Vec3, content: &mut FrameContent) {
    let mut sum = [Vec3::ZERO; SH_SIZE];
    for r in rays {
        let y = probes::sh_basis(r.direction);
        for i in 0..SH_SIZE {
            sum[i] += r.light * y[i];
        }
    }
    let coefficients = sum.map(|c| c * 4.0 / rays.len() as f32);
    let n = 320;
    let golden = std::f32::consts::PI * (3.0 - 5f32.sqrt());
    for k in 0..n {
        // Directions spread evenly over the sphere (a Fibonacci spiral), each read from the coefficients.
        let y = 1.0 - 2.0 * (k as f32 + 0.5) / n as f32;
        let r = (1.0 - y * y).sqrt();
        let d = Vec3::new(
            (golden * k as f32).cos() * r,
            y,
            (golden * k as f32).sin() * r,
        );
        let basis = probes::sh_basis(d);
        let light: Vec3 = (0..SH_SIZE).map(|i| coefficients[i] * basis[i]).sum();
        // Positive light in its colour; where the 16 coefficients read negative (their ringing), blue.
        let tm = |v: f32| ((v.max(0.0) * 3.0 / (1.0 + v.max(0.0) * 3.0)).sqrt() * 255.0) as u8;
        let colour = if light.max_element() < 0.0 {
            Color32::from_rgb(60, 110, 230)
        } else {
            Color32::from_rgb(tm(light.x), tm(light.y), tm(light.z))
        };
        ball(content, probe + d * 0.5, 0.04, colour, 1.0);
    }
}

/// One ray's march (`raySteps` of it): spheres of the stored distance.
pub(super) fn march_spheres(state: &ShotState, world: &World, content: &mut FrameContent) {
    let field = world.field();
    let y_mult = world.cascades.settings.y_mult();
    let unsquash = |p: Vec3| Vec3::new(p.x, p.y / y_mult, p.z);
    let probe_pos = probes::probe_position(&field, 0, FOLLOWED_PROBE);
    let light_refs: Vec<&light::LightTextures> = world.light.iter().collect();
    let lighting = Scene::study_room().lighting;
    let sky_fn = move |d: Vec3| lab::scene::environment_radiance(&lighting, d);
    let sky = probes::Sky::Radiance(&sky_fn);
    let steps = state.p("raySteps").round() as usize;
    if steps > 0 {
        let rays = probes::trace_rays(
            &field,
            &light_refs,
            0,
            FOLLOWED_PROBE,
            0,
            &world.integrate,
            &sky,
        );
        let dir = rays[1].direction;
        // The second ray of frame 0, from its biased start, a sphere of the stored distance at each step.
        let start =
            probe_pos + dir / dir.abs().max_element() * world.integrate.ray_bias / field.to_cell(0);
        let mut advance = 0.0;
        let pos = (start - field.offset(0)) * field.to_cell(0);
        for _ in 0..steps {
            let d = light::sample_sdf(
                &world.data[0].stored.sdf,
                (pos + dir * advance) / CASCADE_SIZE as f32,
            ) * 255.0
                - 1.0;
            let p = unsquash((pos + dir * advance) / field.to_cell(0) + field.offset(0));
            circle(
                content,
                p,
                (d.max(0.0) / field.to_cell(0)).min(3.0),
                palette::OUTSIDE,
                0.5,
            );
            ball(content, p, 0.025, palette::OUTSIDE, 1.0);
            if d < 0.05 {
                break;
            }
            advance += d;
        }
    }
}

/// Irradiance (up, green channel) after each frame since creation, the first `k` lit: a straight rise for 30
/// frames.
pub(super) fn history_chart(world: &World, k: usize, content: &mut FrameContent) {
    // Irradiance (up, green channel) after each frame since creation: a straight rise for 30 frames.
    let n = 110usize;
    let values: Vec<f32> = (0..=sim::CONVERGED_FRAMES as usize)
        .map(|f| {
            // The probe's upward irradiance (green) after each frame.
            let m = if f == 0 {
                [Vec3::ZERO; SH_SIZE]
            } else {
                world.trace[f - 1]
            };
            let c = probes::sh_basis(Vec3::Y);
            (0..SH_SIZE)
                .map(|i| m[i].y * c[i] * 4.0 * probes::L_MULT[i])
                .sum::<f32>()
        })
        .collect();
    let top = values.iter().copied().fold(1e-6, f32::max) * 1.1;
    let mut rgba = vec![12u8; n * n * 4];
    for px in rgba.chunks_mut(4) {
        px.copy_from_slice(&[12, 20, 32, 255]);
    }
    for (f, v) in values.iter().enumerate() {
        // Each frame a bar up to its value, the frames so far lit.
        let x = f * (n - 1) / values.len().max(2).saturating_sub(1).max(1);
        let y = n - 1 - ((v / top) * (n - 1) as f32) as usize;
        let lit = f <= k;
        for yy in y..n {
            let i = (yy * n + x) * 4;
            rgba[i..i + 4].copy_from_slice(&if lit {
                [90, 200, 130, 255]
            } else {
                [40, 70, 60, 255]
            });
        }
    }
    let marker = Some((
        [
            k as f32 / sim::CONVERGED_FRAMES as f32,
            values[k.min(values.len() - 1)] / top,
        ],
        palette::SELECTED,
    ));
    content.insets.push(picture(
        Corner::TopRight,
        Caption {
            title: tr("demos-history_chart").into(),
            key: "history-chart".into(),
            lines: vec![],
        },
        n,
        rgba,
        [
            tr("demos-frames_axis").into(),
            tr("demos-irradiance_axis").into(),
        ],
        marker,
    ));
}

/// The followed probe's two maps, 8 x 8 texels each with their border, side by side in a square, nearest-filtered.
/// The picked texel (`mapTexel`, row by row over the 6 x 6), in tile coordinates; None when none is picked.
fn picked_texel(state: &ShotState) -> Option<glam::IVec2> {
    let i = state.p("mapTexel").round();
    (i >= 0.0).then(|| {
        let i = (i as i32).min(probes::OCT_SIZE * probes::OCT_SIZE - 1);
        glam::IVec2::new(i % probes::OCT_SIZE, i / probes::OCT_SIZE)
    })
}

/// The picked texel's direction from the probe, as STORE computes it from the texel's corner (`local / 6`), and,
/// fainter, the direction of the texel's centre, where a lookup takes its value.
fn texel_directions(state: &ShotState, probe: Vec3, content: &mut FrameContent) {
    let Some(local) = picked_texel(state) else {
        return;
    };
    let size = probes::OCT_SIZE as f32;
    let corner = probes::octahedron_decode(local.as_vec2() / size);
    let centre = probes::octahedron_decode((local.as_vec2() + glam::Vec2::splat(0.5)) / size);
    for (d, alpha) in [(centre, 0.45), (corner, 1.0)] {
        let tip = probe + d * 0.9;
        content
            .draw
            .segment(probe, tip, palette::INCIDENT, alpha, 3.0, 0);
        ball(content, tip, 0.04, palette::INCIDENT, alpha);
    }
}

/// The maps' gain rows: what a uniform sky of 1 stores (with `uniformSky`), and the extra 4/pi (with `showGain`).
fn gain_rows(state: &ShotState) -> Vec<(String, String, Color32)> {
    let mut rows = Vec::new();
    if let Some(v) = uniform_sky(state) {
        rows.push(row(
            tr("demos-uniform_sky_texel"),
            format!("{v:.3}"),
            palette::MISMATCH,
        ));
    }
    if state.p("showGain") > 0.5 {
        // With the gain shown: what the maps store for a uniform sky of 1, 4/π.
        let gain = 4.0 / std::f32::consts::PI;
        rows.push(row(
            tr("demos-sky_gain"),
            format!("4/pi = {gain:.3}"),
            palette::MISMATCH,
        ));
    }
    rows
}

/// With `uniformSky`, what a texel of a probe lit by a uniform sky of 1 stores: the gain times the square of L0's
/// basis (only L0 is non-zero), the gain going from the code's 4 x 4 = 16 to the 4 pi the sphere needs
/// (`gainFix` 0 to 1); None without it.
fn uniform_sky(state: &ShotState) -> Option<f32> {
    (state.p("uniformSky") > 0.5).then(|| {
        let y0 = probes::sh_basis(Vec3::Y)[0];
        let gain = 16.0 + (4.0 * std::f32::consts::PI - 16.0) * state.p("gainFix").clamp(0.0, 1.0);
        gain * y0 * y0
    })
}

pub(super) fn maps_inset(state: &ShotState, world: &World, content: &mut FrameContent) {
    // Both maps, 8 x 8 texels each with their border, side by side in a square, nearest-filtered.
    let maps = &world.maps[0];
    let n = (probes::OCT_SIZE + 2) as usize;
    let k = 24usize;
    let size = 2 * n * k + k;
    let origin = glam::IVec2::new(
        (FOLLOWED_PROBE.x + FOLLOWED_PROBE.z * PROBE_AXIS_COUNT) * (probes::OCT_SIZE + 2),
        FOLLOWED_PROBE.y * (probes::OCT_SIZE + 2),
    );
    let mut rgba = vec![0u8; size * size * 4];
    for px in rgba.chunks_mut(4) {
        px.copy_from_slice(&[12, 20, 32, 255]);
    }
    let top = (size - n * k) / 2;
    for (m, layer) in [&maps.irradiance, &maps.radiance].into_iter().enumerate() {
        // Each texel of the probe's tile, border included, tone-mapped and drawn k x k, the border darker.
        for y in 0..n {
            for x in 0..n {
                let t = match uniform_sky(state) {
                    // A uniform sky of 1: every texel holds the same value, the gain times L0's basis squared.
                    Some(v) => Vec3::splat(v),
                    None => {
                        probes::ProbeMaps::texel(layer, origin.x + x as i32, origin.y + y as i32)
                    }
                };
                let tm = |v: f32| ((v / (1.0 + v)).sqrt() * 255.0) as u8;
                let border = x == 0 || x == n - 1 || y == 0 || y == n - 1;
                let mut px = [tm(t.x * 3.0), tm(t.y * 3.0), tm(t.z * 3.0), 255];
                if border {
                    px = [px[0] / 2, px[1] / 2, px[2] / 2, 255];
                }
                for dy in 0..k {
                    for dx in 0..k {
                        let edge = dx == 0 || dy == 0;
                        let i = ((top + y * k + dy) * size + m * (n * k + k) + x * k + dx) * 4;
                        rgba[i..i + 4].copy_from_slice(if edge {
                            &[20, 28, 40, 255]
                        } else {
                            &px
                        });
                    }
                }
            }
        }
    }
    // The picked texel outlined in both maps (bright), and the border texels STORE copies it into (dim).
    if let Some(local) = picked_texel(state) {
        let marks = std::iter::once((local, [255u8, 209, 102, 255])).chain(
            probes::border_copies(local)
                .into_iter()
                .map(|b| (b, [120, 200, 255, 255])),
        );
        for (t, colour) in marks {
            let (x, y) = ((t.x + 1) as usize, (t.y + 1) as usize);
            for m in 0..2 {
                for d in 0..k {
                    // The texel's square's four edges, two pixels in from its grid line.
                    for (dx, dy) in [(d, 1), (d, k - 1), (1, d), (k - 1, d)] {
                        let i = ((top + y * k + dy) * size + m * (n * k + k) + x * k + dx) * 4;
                        rgba[i..i + 4].copy_from_slice(&colour);
                    }
                }
            }
        }
    }
    // A click on a texel picks it (script/logic.lam `pickMapTexel`); with the uniform sky, the gain is said in numbers.
    let mut lines = vec![(tr("demos-oct_border").to_string(), palette::text_dim())];
    if state.p("uniformSky") > 0.5 {
        let gain = 16.0 + (4.0 * std::f32::consts::PI - 16.0) * state.p("gainFix").clamp(0.0, 1.0);
        lines.insert(
            0,
            (
                tr_with("demos-gain_line", lab::args!(gain = format!("{gain:.2}"))),
                palette::MISMATCH,
            ),
        );
    }
    content.insets.push(PictureInset {
        click: Some("pickMapTexel".into()),
        size: [420.0, 420.0],
        ..picture(
            Corner::TopLeft,
            Caption {
                title: tr("demos-oct_maps").into(),
                key: "oct-maps".into(),
                lines,
            },
            size,
            rgba,
            [tr("demos-oct_axes").into(), "v".into()],
            None,
        )
    });
}

/// Chapter 10: a pixel on the red wall, its cascade, its eight probes.
pub(super) fn pixel_demo(state: &ShotState, world: &World, content: &mut FrameContent) {
    fog_probes(state, world, content);
    // A pixel on the sphere (its roughness), or on the left wall (fully rough).
    let on_sphere = state.p("spherePixel") > 0.5;
    let sphere_centre = Vec3::new(-1.05, 0.62, -0.55);
    let n = if on_sphere {
        Vec3::new(0.35, 0.5, 0.8).normalize()
    } else {
        Vec3::X
    };
    let p = if on_sphere {
        sphere_centre + n * 0.621
    } else {
        Vec3::new(-2.999, state.p("pixelY"), state.p("pixelZ"))
    };
    let inputs = sim::screen_inputs(world);
    let eye = Vec3::from(content.uniforms.camera.position);
    let v = (eye - p).normalize();
    // The view reflected about the normal.
    let r = (-v) - 2.0 * (-v).dot(n) * n;
    let roughness = if on_sphere {
        state.p("sphereRoughness")
    } else {
        1.0
    };
    let result = super::super::screen::process(&inputs, p - ORIGIN, n, r, roughness);
    pixel_marks(content, (p, n, r), on_sphere);
    let mut rows = vec![
        row(
            tr("demos-cascade"),
            result
                .cascade
                .map(|c| c.to_string())
                .unwrap_or_else(|| "-".into()),
            palette::AXIS_Y,
        ),
        row(
            tr("demos-blend"),
            format!("{:.3}", result.blend),
            palette::AXIS_Y,
        ),
    ];
    rows.extend(blend_point(&inputs, state.p("blendPoint"), content));
    if on_sphere {
        rows.extend(roughness_rows(roughness));
    }
    if let Some(c) = result.cascade {
        // In a cascade: the pixel's probe cell, its eight probes, and the light they give.
        let y = inputs.y_mult;
        let vertex = Vec3::new(p.x - ORIGIN.x, (p.y - ORIGIN.y) * y, p.z - ORIGIN.z);
        let cascade_pos = (vertex - inputs.cascades[c].position) * inputs.cascades[c].to_probe;
        let gi = super::super::screen::voxel_gi(&inputs, c, cascade_pos, n, r, roughness);
        if state.p("pixelProbes") > 0.5 {
            pixel_probes(state, world, (c, p), &gi.probes, content);
        }
        rows.push(row(
            tr("demos-probe_cell"),
            format!("{:.2} {:.2} {:.2}", gi.biased.x, gi.biased.y, gi.biased.z),
            palette::PROBE_SELECTED,
        ));
        rows.push(row(
            tr("demos-diffuse"),
            format!(
                "{:.3} {:.3} {:.3}",
                result.ambient.x, result.ambient.y, result.ambient.z
            ),
            palette::text(),
        ));
        rows.push(row(
            tr("demos-reflection"),
            format!(
                "{:.3} {:.3} {:.3}",
                result.reflection.x, result.reflection.y, result.reflection.z
            ),
            palette::VIEW,
        ));
    }
    content.readout = Some(Readout {
        title: tr("demos-pixel").into(),
        subtitle: tr("demos-pixel_subtitle").into(),
        formula: None,
        formula_lit: None,
        rows,
    });
}

/// The pixel's marks: its point, its normal, the normal bias, and (on the sphere) its reflected direction.
fn pixel_marks(content: &mut FrameContent, (p, n, r): (Vec3, Vec3, Vec3), on_sphere: bool) {
    ball(content, p, 0.07, palette::SELECTED, 1.0);
    content.draw.segment(
        p,
        p + n * 0.4,
        palette::NORMAL,
        1.0,
        3.0,
        lab::frame::FLAG_ARROW,
    );
    // The normal bias: the point moved off the surface before its probes are chosen (1.1 cells).
    let bias = p + n * (1.1 * 0.2);
    content
        .draw
        .segment(p, bias, palette::INCIDENT, 1.0, 4.0, 0);
    if on_sphere {
        content.draw.segment(
            p,
            p + r * 0.8,
            palette::REFLECTED,
            1.0,
            3.0,
            lab::frame::FLAG_ARROW,
        );
    }
}

/// A point `d` m along SDFGI's camera's +x axis, when `d` is above 0: drawn, and its row (which cascade holds it
/// and how far it has blended to the next).
pub(super) fn blend_point(
    inputs: &super::super::screen::Inputs,
    d: f32,
    content: &mut FrameContent,
) -> Option<(String, String, Color32)> {
    if d <= 0.0 {
        return None;
    }
    let q = ORIGIN + Vec3::X * d;
    let qr = super::super::screen::process(inputs, q - ORIGIN, Vec3::Y, Vec3::Y, 1.0);
    ball(content, q, 0.25, palette::AXIS_Y, 1.0);
    content
        .draw
        .segment(ORIGIN, q, palette::AXIS_Y, 1.0, 2.0, 0);
    Some(row(
        tr("demos-test_point"),
        format!(
            "{d:.1} m: {} {}, blend {:.2}",
            tr("demos-cascade"),
            qr.cascade
                .map(|c| c.to_string())
                .unwrap_or_else(|| "-".into()),
            qr.blend
        ),
        palette::AXIS_Y,
    ))
}

/// A rough pixel's rows: its roughness, and how much of its reflection is from radiance, from irradiance, and
/// from Godot's own march.
pub(super) fn roughness_rows(roughness: f32) -> Vec<(String, String, Color32)> {
    let mut rows = Vec::new();
    let radiance = if roughness < 0.99 {
        // The radiance map's share falls from roughness 0.2 to 0.99, the irradiance map taking over.
        1.0 - ((roughness - 0.2) * 1.25).clamp(0.0, 1.0)
    } else {
        0.0
    };
    rows.push(row(
        tr("demos-roughness"),
        format!("{roughness:.2}"),
        palette::VIEW,
    ));
    rows.push(row(
        tr("demos-from_radiance"),
        format!("{:.0} %", radiance * 100.0),
        palette::REFLECTED,
    ));
    rows.push(row(
        tr("demos-from_irradiance"),
        format!("{:.0} %", (1.0 - radiance) * 100.0),
        palette::NORMAL,
    ));
    rows.push(row(
        tr("demos-godot_march"),
        if roughness < 0.2 {
            format!("{:.0} %", (1.0 - (roughness * 5.0).min(1.0)) * 100.0)
        } else {
            "0 %".to_string()
        },
        palette::text_dim(),
    ));
    rows
}

/// The pixel `p`'s eight probes in cascade `c`, each as large and as strong as its share of the weight.
pub(super) fn pixel_probes(
    state: &ShotState,
    world: &World,
    (c, p): (usize, Vec3),
    weights: &[super::super::screen::ProbeWeight],
    content: &mut FrameContent,
) {
    let y = world.cascades.settings.y_mult();
    let total: f32 = weights.iter().map(|w| w.weight).sum();
    let field = world.field();
    for w in weights {
        // Each probe sized and lit by its share of the weight, a line to the pixel.
        let pos = probes::probe_position(&field, c, w.probe);
        let real = Vec3::new(pos.x, pos.y / y, pos.z);
        let share = w.weight / total.max(1e-9);
        ball(
            content,
            real,
            0.04 + 0.12 * share,
            palette::PROBE_SELECTED,
            0.4 + 0.6 * share,
        );
        content.draw.segment(
            p,
            real,
            palette::PROBE,
            0.2 + 0.8 * share,
            1.0 + 2.0 * share,
            0,
        );
        label(
            content,
            state,
            real + Vec3::Y * 0.15,
            format!("{:.0} %", share * 100.0),
            palette::PROBE_SELECTED,
        );
    }
}

/// A fog sample at `fogHeight` (none below 0), at x = 0.5, z = 0 in the room: the probe the screen pass's lookup
/// takes there, with the height scaled by y_mult (blue), and the one the fog takes, with the height left unscaled
/// (orange), each linked to the sample.
fn fog_probes(state: &ShotState, world: &World, content: &mut FrameContent) {
    let height = state.p("fogHeight");
    if height < 0.0 {
        return;
    }
    let field = world.field();
    let y_mult = world.cascades.settings.y_mult();
    let sample = Vec3::new(0.5, height, 0.0);
    let probe_cells = (CASCADE_SIZE / (PROBE_AXIS_COUNT - 1)) as f32;
    let nearest = |p: Vec3| -> IVec3 {
        ((p - field.offset(0)) * field.to_cell(0) / probe_cells)
            .round()
            .as_ivec3()
    };
    let unsquash = |p: Vec3| Vec3::new(p.x, p.y / y_mult, p.z);
    // The sample on a vertical line up from the floor, its height said.
    ball(content, sample, 0.1, palette::SELECTED, 1.0);
    content.draw.segment(
        Vec3::new(sample.x, 0.0, sample.z),
        sample,
        palette::SELECTED,
        0.8,
        2.0,
        0,
    );
    label(
        content,
        state,
        sample + Vec3::X * 0.15,
        format!("{height:.2} m"),
        palette::SELECTED,
    );
    for (p, colour) in [
        (
            Vec3::new(sample.x, sample.y * y_mult, sample.z),
            palette::VIEW,
        ),
        (sample, palette::MISMATCH),
    ] {
        // The probe nearest the sample in the probes' grid, for each way of placing the sample in it.
        let probe = unsquash(probes::probe_position(&field, 0, nearest(p)));
        content
            .draw
            .segment(sample, probe, colour, 1.0, 3.0, lab::frame::FLAG_ARROW);
        ball(content, probe, 0.12, colour, 1.0);
        label(
            content,
            state,
            probe - Vec3::X * 0.2,
            format!("{:.2} m", probe.y),
            colour,
        );
    }
}
