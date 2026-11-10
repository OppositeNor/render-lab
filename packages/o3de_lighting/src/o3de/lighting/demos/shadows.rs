//! The sun's cascades: where gShadingFrame holds them, the shadow camera they are fitted to, and their overlay.

use super::*;

/// Where gShadingFrame holds the sun's cascades (shaders/standard_lighting.wgsl FRAME_CASCADES).
pub const FRAME_CASCADES: usize = 240;

/// The camera the sun's cascades cover: the main camera, as O3DE takes the
/// active camera's (DirectionalLightComponentController::
/// GetCameraConfiguration), its far plane the light's shadow far. The free
/// camera (lab/src/free_view.rs) sees them from elsewhere.
pub fn shadow_camera(
    state: &ShotState,
    camera: &lab::layout::CameraData,
) -> super::super::cascades::Observer {
    let view = glam::Mat4::from_cols_array_2d(&camera.world_to_view);
    // The view matrix's third row is the camera's back axis in the world, so forward is its negative.
    let forward = -Vec3::new(view.x_axis.z, view.y_axis.z, view.z_axis.z).normalize();
    let aspect = camera.viewport_size[0] / camera.viewport_size[1].max(1.0);
    super::super::cascades::Observer {
        position: Vec3::from(camera.position),
        forward,
        fov_y: camera.vertical_fov,
        aspect,
        near: camera.near_plane,
        far: state.p("shadowFar").min(camera.far_plane).max(0.2),
    }
}

/// gShadingFrame[FRAME_CASCADES..]: each cascade's world to shadow-map
/// transform (cascades::world_to_shadowmap, by column), then each cascade's
/// (near, far, texel in metres, depth range in metres), then the sun: (its direction of
/// travel, the cascade count), (white times sunLux, on), and
/// (normalShadowBias, shadowBias, shadowBlur, cascadeColors), and (the
/// cascades' camera depth span, the fullscreen blur's const falloff and
/// depth falloff strength, unused).
pub fn cascade_records(state: &ShotState, camera: &lab::layout::CameraData) -> Vec<[f32; 4]> {
    let observer = shadow_camera(state, camera);
    let set = super::super::cascades::cascades(
        observer,
        sun_direction(state),
        state.p("cascadeSplitRatio"),
    );
    let mut out = Vec::new();
    for i in 0..super::super::cascades::CASCADES {
        out.extend(set.world_to_shadowmap(i).to_cols_array_2d());
    }
    for c in &set.cascades {
        out.push([c.near, c.far, c.texel(), c.max_y - c.min_y]);
    }
    let d = sun_direction(state);
    let lux = state.p("sunLux");
    out.push([d.x, d.y, d.z, super::super::cascades::CASCADES as f32]);
    out.push([
        lux,
        lux,
        lux,
        if lux > 0.0 {
            1.0
        } else {
            0.0
        },
    ]);
    out.push([
        state.p("normalShadowBias"),
        state.p("shadowBias"),
        state.p("shadowBlur"),
        state.p("cascadeColors"),
    ]);
    // m_far_minus_near: the camera's depth span the cascades cover, which
    // GetThickness scales a cascade's depth difference by.
    out.push([
        observer.far - observer.near,
        state.p("shadowBlurStrength"),
        state.p("shadowBlurSharpness"),
        0.0,
    ]);
    out
}

/// The directional light's direction, from `sunYaw` and `sunPitch` (below the horizon).
pub fn sun_direction(state: &ShotState) -> Vec3 {
    let (yaw, pitch) = (
        state.p("sunYaw").to_radians(),
        state.p("sunPitch").to_radians(),
    );
    Vec3::new(
        pitch.cos() * yaw.sin(),
        -pitch.sin(),
        pitch.cos() * yaw.cos(),
    )
    .normalize()
}

pub const CASCADE_COLORS: [Color32; 4] = [
    palette::rgb(0x64dfd1),
    palette::rgb(0x8fb8ff),
    palette::rgb(0xffd98a),
    palette::rgb(0xff8fc8),
];

/// The main camera's frustum split into its cascades (the first
/// `cascadesShown`, a fraction growing the next), each cascade's bounding
/// sphere and snapped light-space box, the light's direction, and the
/// inspected point's cascade: seen from the free camera, which also draws the
/// main camera itself.
pub(super) fn cascade_overlay(
    draw: &mut DrawLists,
    labels: &mut Vec<Label>,
    state: &ShotState,
    camera: &lab::layout::CameraData,
    p: Vec3,
    n: Vec3,
) {
    use super::super::cascades;
    let alpha = state.p("cascades");
    let observer = shadow_camera(state, camera);
    let dir = sun_direction(state);
    let set = cascades::cascades(observer, dir, state.p("cascadeSplitRatio"));
    let shown = state
        .p("cascadesShown")
        .clamp(0.0, cascades::CASCADES as f32);
    let near = observer.corners(observer.near);
    for (i, c) in set.cascades.iter().enumerate() {
        // A cascade grows in as `cascadesShown` passes its index; the first one not started ends the loop.
        let grown = (shown - i as f32).clamp(0.0, 1.0);
        if grown <= 0.0 {
            break;
        }
        let color = CASCADE_COLORS[i];
        // A growing cascade's far end eases out from its near end.
        let far = c.near + (c.far - c.near) * Ease::Smooth.apply(grown);
        let (a, b) = (observer.corners(c.near), observer.corners(far));
        let ring: Vec<Vec3> = b.iter().chain(std::iter::once(&b[0])).copied().collect();
        draw.polyline(&ring, color, alpha, 3.0);
        for k in 0..4 {
            // The side edges join the near corners to the far ones (the camera's near plane, for the first cascade).
            draw.segment(
                if i == 0 {
                    near[k]
                } else {
                    a[k]
                },
                b[k],
                color,
                alpha * 0.8,
                2.0,
                0,
            );
        }
        if grown >= 1.0 {
            labels.push(Label {
                anchor: (b[2] + b[3]) * 0.5,
                text: format!("{} {i} · {:.2} m", tr("demos-cascade"), c.far),
                color,
            });
        }
        let spheres = state.p("cascadeSpheres") * grown;
        if spheres > 0.0 {
            // The bounding sphere is drawn as three great circles in the light's frame.
            for (u, v) in [
                (set.frame.x, set.frame.y),
                (set.frame.y, set.frame.z),
                (set.frame.x, set.frame.z),
            ] {
                let circle: Vec<Vec3> = (0..=64)
                    .map(|t| {
                        let a = t as f32 / 64.0 * std::f32::consts::TAU;
                        c.center + (u * a.cos() + v * a.sin()) * c.radius
                    })
                    .collect();
                draw.polyline(&circle, color, alpha * spheres * 0.55, 1.5);
            }
            draw.point(c.center, 7.0, color, alpha * spheres);
        }
        let boxes = state.p("cascadeBoxes") * grown;
        if boxes > 0.0 {
            snapped_box(draw, (&set.frame, c), color, alpha * boxes * 0.7);
        }
    }
    // The light, shining along its direction.
    let from = Vec3::new(-2.2, 2.7, -2.2) - dir * 0.9;
    draw.segment(from, from + dir * 0.9, LIGHT, alpha, 4.0, FLAG_ARROW);
    labels.push(Label {
        anchor: from - dir * 0.1,
        text: tr("demos-directional_light").into(),
        color: LIGHT,
    });
    // The inspected point's cascade.
    draw.point(p, 9.0, palette::SELECTED, alpha);
    // The point's cascade is named once that cascade is shown, and a point outside them all says so.
    let text = match set.select(p, n) {
        Some(i) if (i as f32) < shown => format!("{} {i}", tr("demos-cascade")),
        Some(_) => String::new(),
        None => tr("demos-outside_every_cascade").into(),
    };
    if !text.is_empty() {
        labels.push(Label {
            anchor: p + Vec3::Y * 0.1,
            text,
            color: palette::SELECTED,
        });
    }
}

/// Cascade `c`'s snapped light-space box (in the light's `frame`), its twelve edges at `alpha`.
pub(super) fn snapped_box(
    draw: &mut DrawLists,
    (frame, c): (
        &super::super::cascades::LightFrame,
        &super::super::cascades::Cascade,
    ),
    color: Color32,
    alpha: f32,
) {
    let (lo, hi) = (c.snapped_min, c.snapped_max);
    let corner = |x: bool, y: bool, z: bool| {
        frame.to_world(Vec3::new(
            if x {
                hi.x
            } else {
                lo.x
            },
            if y {
                hi.y
            } else {
                lo.y
            },
            if z {
                hi.z
            } else {
                lo.z
            },
        ))
    };
    for (x0, y0, z0, x1, y1, z1) in [
        (0, 0, 0, 1, 0, 0),
        (0, 1, 0, 1, 1, 0),
        (0, 0, 1, 1, 0, 1),
        (0, 1, 1, 1, 1, 1),
        (0, 0, 0, 0, 1, 0),
        (1, 0, 0, 1, 1, 0),
        (0, 0, 1, 0, 1, 1),
        (1, 0, 1, 1, 1, 1),
        (0, 0, 0, 0, 0, 1),
        (1, 0, 0, 1, 0, 1),
        (0, 1, 0, 0, 1, 1),
        (1, 1, 0, 1, 1, 1),
    ] {
        // Each edge joins two corners: four run along x, four along y, and four along z.
        draw.segment(
            corner(x0 == 1, y0 == 1, z0 == 1),
            corner(x1 == 1, y1 == 1, z1 == 1),
            color,
            alpha,
            1.5,
            0,
        );
    }
}
