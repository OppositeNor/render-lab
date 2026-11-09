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
                world.integrate.ray_count,
                world.integrate.history_size * world.integrate.ray_count,
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
        maps_inset(world, content);
        if state.p("showGain") > 0.5 {
            let gain = 4.0 / std::f32::consts::PI;
            rows.push(row(
                tr("demos-sky_gain"),
                format!("4/pi = {gain:.3}"),
                palette::MISMATCH,
            ));
        }
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
    if state.p("probeRays") > 0.5 {
        for h in 0..frames {
            for ray in probes::trace_rays(
                &field,
                &light_refs,
                0,
                FOLLOWED_PROBE,
                h,
                &world.integrate,
                &sky,
            ) {
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
                    if h + 1 == frames {
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
            }
        }
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
pub(super) fn maps_inset(world: &World, content: &mut FrameContent) {
    // Both maps, 8 x 8 texels each with their border, side by side in a square, nearest-filtered.
    let maps = &world.maps[0];
    let n = (probes::OCT_SIZE + 2) as usize;
    let k = 12usize;
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
        for y in 0..n {
            for x in 0..n {
                let t = probes::ProbeMaps::texel(layer, origin.x + x as i32, origin.y + y as i32);
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
    content.insets.push(picture(
        Corner::TopLeft,
        Caption {
            title: tr("demos-oct_maps").into(),
            key: "oct-maps".into(),
            lines: vec![(tr("demos-oct_border").into(), palette::text_dim())],
        },
        size,
        rgba,
        [tr("demos-oct_axes").into(), "v".into()],
        None,
    ));
}

/// Chapter 10: a pixel on the red wall, its cascade, its eight probes.
pub(super) fn pixel_demo(state: &ShotState, world: &World, content: &mut FrameContent) {
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
