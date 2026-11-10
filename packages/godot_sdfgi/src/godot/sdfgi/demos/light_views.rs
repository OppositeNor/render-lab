//! Chapter 7's added views: the omni light's falloff as a chart, where the squashed distance puts the light, and
//! the worked cell's per-face light.

use super::*;

/// With `falloffChart`, the omni light's falloff over distance (`get_omni_attenuation`, range 5 m): the engine's
/// default attenuation of 1 (at 1 and up), and beside it the inverse square (at 2), the worked cell's distance marked.
pub(super) fn falloff_chart(state: &ShotState, cell_distance: f32, content: &mut FrameContent) {
    let shown = state.p("falloffChart");
    if shown < 0.5 {
        return;
    }
    let (n, range) = (120usize, 5.0f32);
    let mut rgba = vec![0u8; n * n * 4];
    for px in rgba.chunks_mut(4) {
        px.copy_from_slice(&[12, 20, 32, 255]);
    }
    let mut curves = vec![(1.0f32, [255u8, 200, 90])];
    if shown >= 1.5 {
        curves.push((2.0, [120, 170, 240]));
    }
    for (decay, colour) in curves {
        for x in 1..n {
            // The falloff at this distance, clipped at 1 near the light so the tail stays readable.
            let d = x as f32 / (n - 1) as f32 * range;
            let v = light::omni_attenuation(d, 1.0 / range, decay).min(1.0);
            let y = n - 1 - (v * (n - 1) as f32) as usize;
            for dy in 0..2 {
                let i = ((y.saturating_sub(dy)) * n + x) * 4;
                rgba[i..i + 4].copy_from_slice(&[colour[0], colour[1], colour[2], 255]);
            }
        }
    }
    let marker = Some((
        [(cell_distance / range).clamp(0.0, 1.0), 0.05],
        palette::SELECTED,
    ));
    content.insets.push(picture(
        Corner::TopLeft,
        Caption {
            title: tr("demos-falloff_chart").into(),
            key: format!("falloff-{}", shown.round()),
            lines: vec![(tr("demos-falloff_note").into(), palette::text_dim())],
        },
        n,
        rgba,
        [
            tr("demos-distance_axis").into(),
            tr("demos-falloff_axis").into(),
        ],
        marker,
    ));
}

/// With `squashedLight`, where the light pass's distance puts the light: every height times y_mult, so the light
/// sits that much higher, faint, joined to the real one.
pub(super) fn squashed_light(
    state: &ShotState,
    light_pos: Vec3,
    y_mult: f32,
    content: &mut FrameContent,
) {
    if state.p("squashedLight") <= 0.5 {
        return;
    }
    let up = Vec3::new(light_pos.x, light_pos.y * y_mult, light_pos.z);
    content.draw.segment(
        light_pos,
        up,
        palette::INCIDENT,
        0.6,
        2.0,
        lab::frame::FLAG_ARROW,
    );
    ball(content, up, 0.09, palette::INCIDENT, 0.45);
}

/// With `faceLight`, the worked cell's six faces as arrows; with `faceWeights` each arrow takes the share of the
/// cell's light its face stores (the anisotropy weights), the stored total read off the light texture.
pub(super) fn face_light(
    state: &ShotState,
    world: &World,
    cell: IVec3,
    content: &mut FrameContent,
) {
    let shown = state.p("faceLight").clamp(0.0, 1.0);
    if shown <= 0.0 {
        return;
    }
    let weights = world.light[0].aniso_at(cell);
    let weighted = state.p("faceWeights").clamp(0.0, 1.0);
    let at = cell_world(world, 0, cell);
    const FACES: [&str; 6] = ["+X", "+Y", "+Z", "-X", "-Y", "-Z"];
    for (k, dir) in voxels::ANISO_DIR.iter().enumerate() {
        // Each face: an even arrow, growing or shrinking to its weight as `faceWeights` rises, named with its weight.
        let length = 0.3 * (1.0 - weighted) + (0.15 + 0.45 * weights[k]) * weighted;
        let tip = at + *dir * length.max(0.02);
        let text = if weighted > 0.5 {
            format!("{} {:.2}", FACES[k], weights[k])
        } else {
            FACES[k].to_string()
        };
        content
            .draw
            .text(tip + *dir * 0.05, text, palette::INCIDENT, [0.5, 0.5]);
        content.draw.segment(
            at,
            at + *dir * length.max(0.02),
            palette::INCIDENT,
            shown,
            3.0,
            lab::frame::FLAG_ARROW,
        );
    }
}
