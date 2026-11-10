//! Chapters 4 and 5: the light pass at a worked cell, the second bounce's cones, and the mips.

use super::*;

// ---------------------------------------------------------------------------
// Chapter 4: the light pass.

pub(super) fn light_demo(state: &ShotState, w: &World, content: &mut FrameContent) {
    let probe = &w.probe;
    let lights = shot_lights(state, w);
    let params = world::params(probe, &light_settings(state));
    light_marks(state, w, content);
    let l = &w.scene.lighting.light;
    if state.p("workedLight") < 0.5 {
        light_pass_readout(state, w, lights.len(), content);
        return;
    }
    let Some((cell, pos)) = nearest_leaf(probe, picked_cell(state, leaf_of(probe, LIGHT_POINT)))
    else {
        return;
    };
    let d = probe.data[cell];
    let level = depth(probe);
    let (min, size) = cell_box(probe, level, pos);
    let center = min + size * 0.5;
    wire_box(content, min, size, palette::SELECTED, 1.0, 2.5);
    content.points.push(("light.cell".into(), center));
    mark(content, state, center, 0.08);
    let step = state.p("lightStep").round() as i32;
    let cpos = pos.as_vec3() + Vec3::splat(0.5);
    let normal = d.normal();
    let albedo = d.albedo().truncate();
    if normal.length() > 0.2 {
        content
            .draw
            .segment(center, center + normal * 0.3, palette::NORMAL, 1.0, 2.5, 0);
    }
    let mut rows = vec![
        row(
            tr("demos-worked_cell"),
            format!("({}, {}, {})", pos.x, pos.y, pos.z),
            palette::SELECTED,
        ),
        row(tr("demos-normal_word"), v3(normal), palette::NORMAL),
    ];
    let mut total = Vec3::ZERO;
    for (k, lt) in lights.iter().enumerate() {
        let name = if lt.kind == light::LIGHT_TYPE_AREA {
            tr("demos-panel")
        } else {
            tr("demos-omni")
        };
        if lt.kind == light::LIGHT_TYPE_AREA {
            let a = light::area_light(&params, lt, cpos, normal);
            let v = a.map(|x| x * albedo).unwrap_or(Vec3::ZERO);
            total += v;
            if step >= 1 {
                content
                    .draw
                    .segment(center, l.center, palette::INCIDENT, 0.6, 1.5, 0);
            }
            if step >= 4 {
                rows.push(row(tr("demos-panel_no_shadow"), v3(v), palette::INCIDENT));
            }
            continue;
        }
        let (v, omni_rows) = omni_term(
            state,
            (probe, &params),
            (lt, name),
            (cpos, normal, albedo, center),
            step,
            content,
        );
        total += v;
        rows.extend(omni_rows);
        let _ = k;
    }
    if step >= 4 {
        rows.push(row(
            tr("demos-cell_light"),
            v3(total + d.emission()),
            palette::SELECTED,
        ));
        let dr = state.p("dynamicRange");
        let stored = (total / dr).extend(d.albedo().w);
        rows.push(row(
            tr_with("demos-stored", lab::args!(range = format!("{dr:.1}"))),
            v4(super::super::storage::unorm8(stored)
                .map(|b| b as f32 / 255.0)
                .into()),
            palette::SELECTED,
        ));
    }
    content.readout = Some(readout(
        tr("demos-light_pass"),
        tr("demos-light_pass_subtitle"),
        lit_rows(state, rows),
    ));
}

/// The light demonstration's lights: the panel's rectangle, and the extra omni light when it is on.
pub(super) fn light_marks(state: &ShotState, w: &World, content: &mut FrameContent) {
    // The panel's rectangle.
    let l = &w.scene.lighting.light;
    let pts: Vec<Vec3> = [
        (-1.0, -1.0),
        (1.0, -1.0),
        (1.0, 1.0),
        (-1.0, 1.0),
        (-1.0, -1.0),
    ]
    .iter()
    .map(|&(a, b)| l.center + l.axis_u * a + l.axis_v * b - Vec3::Y * 0.01)
    .collect();
    content.draw.polyline(&pts, palette::INCIDENT, 0.9, 2.0);
    if state.p("extraLight") > 0.5 {
        let o = omni_position(state);
        ball(content, o, 0.05, palette::INCIDENT, 1.0);
        label(
            content,
            state,
            o + Vec3::Y * 0.1,
            tr("demos-omni").into(),
            palette::INCIDENT,
        );
    }
}

/// An omni light's term (`name` in its rows) at the worked cell (`cpos`, its normal and albedo, drawn at `center`), shown to `step`:
/// its line, attenuation, cosine, and shadow march; the light it adds, and its rows.
pub(super) fn omni_term(
    state: &ShotState,
    (probe, params): (&Probe, &light::Params),
    (lt, name): (&light::Light, &str),
    (cpos, normal, albedo, center): (Vec3, Vec3, Vec3, Vec3),
    step: i32,
    content: &mut FrameContent,
) -> (Vec3, Vec<(String, String, Color32)>) {
    let mut rows = Vec::new();
    let mut trace = LightTrace::default();
    let res = light::light_at_pos(probe, params, lt, cpos, normal, Some(&mut trace));
    let lp = probe.from_cell(lt.position) + NODE;
    if step >= 1 {
        content
            .draw
            .segment(center, lp, palette::INCIDENT, 0.9, 2.0, 0);
    }
    if step >= 2 {
        rows.push(row(
            tr("demos-attenuation"),
            format!("{:.3}", trace.attenuation),
            palette::INCIDENT,
        ));
    }
    if step >= 3 {
        let cos = normal.dot(-trace.light_dir).max(0.0);
        rows.push(row(
            tr("demos-cosine"),
            format!("{cos:.3}"),
            palette::NORMAL,
        ));
    }
    rows.extend(march_marks(state, probe, &trace, content));
    let v = res
        .map(|(li, dir)| {
            if normal.length() > 0.2 {
                normal.dot(-dir).max(0.0) * li * albedo
            } else {
                li * albedo
            }
        })
        .unwrap_or(Vec3::ZERO);
    if step >= 4 {
        rows.push(row(
            tr_with("demos-light_term", lab::args!(name = name)),
            v3(v),
            palette::INCIDENT,
        ));
    }
    (v, rows)
}

/// The light pass's readout without a worked cell: the lights, the dynamic range, and how many leaves it clips.
pub(super) fn light_pass_readout(
    state: &ShotState,
    w: &World,
    lights: usize,
    content: &mut FrameContent,
) {
    // How many leaves the write pass clips at this dynamic range (light / range above 1 in a channel).
    let settings = light_settings(state);
    let lit = world::lit_for(&settings);
    let probe = &w.probe;
    let leaves = probe.level_range(probe.leaf_level());
    let clipped = lit.outputs[leaves.clone()]
        .iter()
        .filter(|v| v.max_element() > settings.dynamic_range)
        .count();
    let brightest = lit.outputs[leaves]
        .iter()
        .map(|v| v.max_element())
        .fold(0.0f32, f32::max);
    let rows = vec![
        row(tr("demos-lights"), format!("{lights}"), palette::INCIDENT),
        row(
            tr("demos-max_lights"),
            format!("{}", light::MAX_LIGHTS),
            palette::INCIDENT,
        ),
        row(
            "dynamic_range",
            format!("{:.2}", settings.dynamic_range),
            palette::INCIDENT,
        ),
        row(
            tr("demos-brightest_leaf"),
            format!("{brightest:.2}"),
            palette::INCIDENT,
        ),
        row(
            tr("demos-leaves_clipped"),
            format!("{clipped}"),
            palette::SELECTED,
        ),
        row(
            tr("demos-light_seconds"),
            format!("{:.2} s", w.light_seconds),
            palette::text_dim(),
        ),
    ];
    content.readout = Some(readout(
        tr("demos-light_pass"),
        tr("demos-light_pass_subtitle"),
        lit_rows(state, rows),
    ));
}

/// The omni light's shadow march, as far as `marchSteps`: a circle per step, of the distance the field gave; and
/// its row.
pub(super) fn march_marks(
    state: &ShotState,
    probe: &Probe,
    trace: &LightTrace,
    content: &mut FrameContent,
) -> Vec<(String, String, Color32)> {
    let mut rows = Vec::new();
    // The shadow march: a circle per step, of the distance the field gave.
    let shown = state.p("marchSteps").round() as usize;
    for (i, s) in trace.steps.iter().take(shown).enumerate() {
        let p = probe.from_cell(s.from) + NODE;
        let r = (s.advance.max(0.0) + 0.5) / probe.to_cell_scale;
        let color = if s.advance < 0.0 {
            palette::MISMATCH
        } else {
            palette::OUTSIDE
        };
        if r <= MAX_CIRCLE {
            circle(content, p, r, color, 0.8);
        }
        ball(content, p, 0.012, color, 1.0);
        if i + 1 == shown.min(trace.steps.len()) {
            content.points.push(("light.march".into(), p));
        }
    }
    if !trace.steps.is_empty() {
        rows.push(row(
            tr("demos-march"),
            tr_with(
                "demos-march_value",
                lab::args!(
                    steps = trace.steps.len(),
                    occlusion = format!("{:.2}", trace.occlusion)
                ),
            ),
            palette::OUTSIDE,
        ));
    }
    rows
}

// ---------------------------------------------------------------------------
// Chapter 5: the second bounce and the mips.

/// The readout's rows for a cone's last drawn step: its distance, width, and mip, in cells.
pub(super) fn step_rows(rows: &mut Vec<(String, String, Color32)>, s: &ConeStep) {
    rows.push(row(
        tr("demos-step_distance"),
        format!("{:.1}", s.dist),
        palette::VIEW,
    ));
    rows.push(row(
        tr("demos-step_diameter"),
        format!("{:.1}", s.diameter),
        palette::VIEW,
    ));
    rows.push(row(
        tr("demos-step_mip"),
        format!("{:.2}", s.lod),
        palette::VIEW,
    ));
}

/// A traced cone to draw: its start and direction (cells), the steps it
/// took, and how many of them are shown.
pub(super) struct TracedCone<'a> {
    start: Vec3,
    dir_cells: Vec3,
    steps: &'a [ConeStep],
    shown: usize,
}

/// A cone drawn as its axis and a circle per sample, coloured by what it read.
pub(super) fn draw_cone(
    content: &mut FrameContent,
    probe: &Probe,
    cone: &TracedCone,
    dynamic_range: f32,
    alpha: f32,
) {
    let TracedCone {
        start,
        dir_cells,
        steps,
        shown,
    } = *cone;
    let end_dist = steps.last().map_or(2.0, |s| s.dist + s.diameter * 0.5);
    let shown_steps = &steps[..shown.min(steps.len())];
    let to_world = |dist: f32| probe.from_cell(start + dir_cells * dist) + NODE;
    let reach = shown_steps.last().map_or(0.0, |s| s.dist).min(end_dist);
    content.draw.segment(
        to_world(0.0),
        to_world(reach.max(0.5)),
        palette::VIEW,
        alpha,
        2.0,
        0,
    );
    for s in shown_steps {
        let p = to_world(s.dist);
        let r = s.diameter * 0.5 / probe.to_cell_scale;
        let c = if s.sample.w > 0.0 {
            light_color(s.sample.truncate() / s.sample.w.max(1e-3) * dynamic_range)
        } else {
            palette::text_dim()
        };
        if r <= MAX_CIRCLE {
            circle(content, p, r, c, alpha);
        } else {
            ball(content, p, 0.02, c, alpha);
        }
    }
}

pub(super) fn bounce_demo(state: &ShotState, w: &World, content: &mut FrameContent) {
    let probe = &w.probe;
    let settings = light_settings(state);
    let lit = world::lit_for(&settings);
    if state.p("workedBounce") < 0.5 {
        bounce_pass_readout(state, w, lit.passes, content);
        return;
    }
    let Some((cell, pos)) = nearest_leaf(probe, picked_cell(state, leaf_of(probe, BOUNCE_POINT)))
    else {
        return;
    };
    let d = probe.data[cell];
    let (min, size) = cell_box(probe, depth(probe), pos);
    wire_box(content, min, size, palette::SELECTED, 1.0, 2.5);
    content
        .points
        .push(("bounce.cell".into(), min + size * 0.5));
    mark(content, state, min + size * 0.5, 0.08);
    let cpos = pos.as_vec3() + Vec3::splat(0.5);
    let normal = d.normal();
    let albedo = d.albedo();
    let params = world::params(probe, &settings);
    let mut rows = vec![
        row(
            tr("demos-worked_cell"),
            format!("({}, {}, {})", pos.x, pos.y, pos.z),
            palette::SELECTED,
        ),
        row(tr("demos-direct"), v3(lit.direct[cell]), palette::INCIDENT),
    ];
    let shown = state.p("conesShown").round() as usize;
    let steps_shown = state.p("coneSteps").round() as usize;
    let mut bounce = Vec3::ZERO;
    if normal.length() > 0.2 {
        let [t, b, n] = light::tangent_frame(normal);
        for (i, dir) in light::BOUNCE_CONE_DIRS.iter().enumerate() {
            let direction = t * dir.x + b * dir.y + n * dir.z;
            let mut steps = Vec::new();
            let c = light::bounce_cone(
                &lit.first_texture,
                &params,
                cpos,
                direction,
                Some(&mut steps),
            );
            let add = (c
                * light::BOUNCE_CONE_WEIGHTS[i]
                * Vec4::new(albedo.x, albedo.y, albedo.z, 1.0)
                * params.dynamic_range)
                .truncate();
            bounce += add;
            if i < shown {
                draw_cone(
                    content,
                    probe,
                    &TracedCone {
                        start: cpos,
                        dir_cells: direction,
                        steps: &steps,
                        shown: steps_shown,
                    },
                    params.dynamic_range,
                    0.9,
                );
                if i + 1 == shown {
                    rows.push(row(
                        tr_with("demos-cone_n", lab::args!(n = i + 1)),
                        tr_with(
                            "demos-cone_value",
                            lab::args!(
                                steps = steps.len(),
                                alpha = format!("{:.2}", c.w),
                                weight = light::BOUNCE_CONE_WEIGHTS[i]
                            ),
                        ),
                        palette::VIEW,
                    ));
                    if let Some(s) = steps.get(steps_shown.min(steps.len()).saturating_sub(1)) {
                        step_rows(&mut rows, s);
                    }
                }
            }
        }
    }
    rows.push(row(tr("demos-bounce_sum"), v3(bounce), palette::VIEW));
    rows.push(row(
        tr("demos-after_bounce"),
        v3(lit.outputs[cell]),
        palette::SELECTED,
    ));
    content.readout = Some(readout(
        tr("demos-bounce"),
        tr("demos-bounce_subtitle"),
        lit_rows(state, rows),
    ));
    slices(state, w, content, Some(pos));
}

/// The bounce's readout without a worked cell (the passes run), over the slices.
pub(super) fn bounce_pass_readout(
    state: &ShotState,
    w: &World,
    passes: usize,
    content: &mut FrameContent,
) {
    let rows = vec![row(
        tr("demos-passes_run"),
        format!("{passes}"),
        palette::INCIDENT,
    )];
    content.readout = Some(readout(
        tr("demos-bounce"),
        tr("demos-bounce_subtitle"),
        lit_rows(state, rows),
    ));
    slices(state, w, content, None);
}

pub(super) fn mips_demo(state: &ShotState, w: &World, content: &mut FrameContent) {
    let probe = &w.probe;
    let settings = light_settings(state);
    let lit = world::lit_for(&settings);
    let params = world::params(probe, &settings);
    let leaf = picked_cell(state, leaf_of(probe, BOUNCE_POINT));
    let mut rows = vec![
        row(
            "propagation",
            format!("{:.2}", settings.propagation),
            palette::INCIDENT,
        ),
        row(
            "dynamic_range",
            format!("{:.1}", settings.dynamic_range),
            palette::INCIDENT,
        ),
    ];
    if state.p("workedParent") > 0.5 {
        let plevel = depth(probe) - 1;
        if let Some(parent) = probe.find_cell(leaf, plevel) {
            let pd = probe.data[parent];
            let (min, size) = cell_box(probe, plevel, pd.pos());
            wire_box(content, min, size, palette::SELECTED, 1.0, 2.5);
            content
                .points
                .push(("mips.parent".into(), min + size * 0.5));
            for &ch in &probe.octree[parent] {
                if ch == CHILD_EMPTY {
                    continue;
                }
                let cd = probe.data[ch as usize];
                let (cmin, csize) = cell_box(probe, plevel + 1, cd.pos());
                ball(
                    content,
                    cmin + csize * 0.5,
                    csize.x * 0.4,
                    light_color(lit.outputs[ch as usize]),
                    1.0,
                );
            }
            let (value, count) = light::mip_cell(probe, &params, &lit.outputs, parent);
            let divisor = 8.0 + (count as f32 - 8.0) * params.propagation;
            rows.push(row(
                tr("demos-children_count"),
                format!("{count} / 8"),
                palette::AABB,
            ));
            rows.push(row(
                tr("demos-divisor"),
                format!("mix(8, {count}, {:.2}) = {divisor:.2}", params.propagation),
                palette::INCIDENT,
            ));
            rows.push(row(tr("demos-parent_light"), v3(value), palette::SELECTED));
            rows.push(row(
                tr("demos-parent_alpha"),
                format!("{:.3}", pd.albedo().w),
                palette::VIEW,
            ));
        }
    }
    content.readout = Some(readout(
        tr("demos-mips"),
        tr("demos-mips_subtitle"),
        lit_rows(state, rows),
    ));
    slices(state, w, content, Some(leaf));
}
