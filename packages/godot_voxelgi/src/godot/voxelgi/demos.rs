//! What each demonstration of the VoxelGI investigation shows: the room
//! shaded with the WGSL port of the screen pass, and the structures of the CPU
//! model (bake.rs, storage.rs, sdf.rs, light.rs, dynamic.rs, cone.rs) drawn over
//! it. Everything a frame shows is computed from the shot's values and the
//! world (world.rs), built once, and its variants, kept once computed.

mod lighting;
mod records;
use lighting::*;
use records::*;

use super::bake::{self, Aabb, CHILD_EMPTY, Triangle};
use super::cone::{self, Cones};
use super::light::{self, ConeStep, LightTrace};
use super::sdf;
use super::shot::Demo;
use super::storage::Probe;
use super::world::{self, LightSettings, NODE, World};
use crate::godot::voxelgi::text::{tr, tr_with};
use egui::Color32;
use glam::{IVec3, Vec3, Vec4};
use lab::frame::{FrameContent, Readout, Statics};
use lab::layout::FrameUniforms;
use lab::palette;
use lab::scene::{Scene, ids};
use lab::shot::ShotState;
use lab::utility::draw::{ball, circle, label, row, wire_box};
use lab::utility::inset::{Corner, Marker, Picture, PictureInset};
use std::sync::{Arc, OnceLock};

// ---------------------------------------------------------------------------
// Drawing helpers.

/// Points of `r` metres, each dimmed to a ghost where the scene's objects
/// hide it from the camera (the overlay has no depth test of its own).
fn balls(content: &mut FrameContent, points: &[(Vec3, f32, Color32)], alpha: f32) {
    let eye = eye(content);
    let objects = &content.objects;
    let seen = light::par_map(0..points.len(), |i| {
        // A ball hidden behind the room's surfaces from the eye is drawn faint.
        let (p, r, _) = points[i];
        let d = p - eye;
        let dist = d.length();
        let hit = lab::scene::trace_scene(
            objects,
            eye,
            d / dist.max(1e-6),
            (dist - 2.5 * r).max(0.0),
            u32::MAX,
        );
        if hit.is_hit() {
            0.12
        } else {
            1.0
        }
    });
    for (&(p, r, c), a) in points.iter().zip(seen) {
        ball(content, p, r, c, alpha * a);
    }
}

/// The largest circle drawn, metres: a cone's farther samples (or a march's
/// longer steps) are drawn as dots, so they do not cover the room.
const MAX_CIRCLE: f32 = 0.8;

/// The rows' colours, the lit row (`focusRow`) in the selection colour.
fn lit_rows(
    state: &ShotState,
    rows: Vec<(String, String, Color32)>,
) -> Vec<(String, String, Color32)> {
    let focus = state.p("focusRow").round() as i32;
    rows.into_iter()
        .enumerate()
        .map(|(i, (a, b, c))| {
            if i as i32 == focus {
                // The focus row is lit.
                (a, b, palette::SELECTED)
            } else {
                (a, b, c)
            }
        })
        .collect()
}

fn readout(title: &str, subtitle: &str, rows: Vec<(String, String, Color32)>) -> Readout {
    Readout {
        title: title.into(),
        subtitle: subtitle.into(),
        formula: None,
        formula_lit: None,
        rows,
    }
}

/// A radiance as a display colour (the lab's tone map).
fn light_color(light: Vec3) -> Color32 {
    let [r, g, b] = lab::scene::display_rgb(light, 1.0);
    Color32::from_rgb(r, g, b)
}

fn unit_color(c: Vec3) -> Color32 {
    let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0) as u8;
    Color32::from_rgb(q(c.x), q(c.y), q(c.z))
}

/// Coverage (alpha) as the story's blue ramp.
fn alpha_color(a: f32) -> Color32 {
    let a = a.clamp(0.0, 1.0);
    unit_color(Vec3::new(0.08, 0.12, 0.25).lerp(Vec3::new(0.35, 0.75, 1.0), a))
}

fn v3(v: Vec3) -> String {
    format!("({:.2}, {:.2}, {:.2})", v.x, v.y, v.z)
}

fn v4(v: Vec4) -> String {
    format!("({:.2}, {:.2}, {:.2}, {:.2})", v.x, v.y, v.z, v.w)
}

// ---------------------------------------------------------------------------
// The probe's geometry in the world.

/// The depth (the leaf level).
pub fn depth(probe: &Probe) -> usize {
    probe.leaf_level()
}

/// A cell's centre in the world: level `level`, position `p` in that level's cells.
pub fn cell_center(probe: &Probe, level: usize, p: IVec3) -> Vec3 {
    let scale = (1 << (depth(probe) - level)) as f32;
    probe.from_cell((p.as_vec3() + Vec3::splat(0.5)) * scale) + NODE
}

/// A cell's minimum corner and size in the world.
pub fn cell_box(probe: &Probe, level: usize, p: IVec3) -> (Vec3, Vec3) {
    let scale = (1 << (depth(probe) - level)) as f32;
    let size = Vec3::splat(scale / probe.to_cell_scale);
    (probe.from_cell(p.as_vec3() * scale) + NODE, size)
}

/// The leaf cell a world point falls in.
pub fn leaf_of(probe: &Probe, p: Vec3) -> IVec3 {
    probe.to_cell(p - NODE).floor().as_ivec3()
}

/// The settings of the light the drawn cells and slices use.
fn light_settings(state: &ShotState) -> LightSettings {
    LightSettings {
        dynamic_range: state.p("dynamicRange"),
        propagation: state.p("propagation"),
        two_bounces: state.p("bounces") > 1.5,
    }
}

// ---------------------------------------------------------------------------
// Worked elements.

/// The teapot's record (with the dynamic offset in the dynamic demonstration).
fn teapot_record(scene: &Scene) -> lab::layout::ObjectData {
    scene.records()[ids::TEAPOT]
}

/// The bake's worked point: where a ray from the room's +x side meets the
/// teapot's spout side, at the teapot's mid height.
pub fn bake_point(scene: &Scene) -> Vec3 {
    let t = teapot_record(scene);
    // Where a ray from beside the teapot, low and toward its centre, meets its surface.
    let c = Vec3::from(t.world_center);
    let ro = c + Vec3::new(2.0, 0.02, 0.4);
    let rd = (c - ro).normalize();
    lab::scene::intersect_object(&t, ro, rd, 10.0)
        .map(|(d, _)| ro + rd * d)
        .unwrap_or(c)
}

/// The worked triangle: the teapot's triangle nearest the worked point, in the node's space.
pub fn worked_triangle(w: &World) -> Triangle {
    static TRI: OnceLock<Triangle> = OnceLock::new();
    *TRI.get_or_init(|| {
        let p = bake_point(&w.scene) - NODE;
        // The teapot's triangle nearest that point.
        let mesh = w
            .meshes
            .iter()
            .find(|m| m.object == ids::TEAPOT)
            .expect("the teapot is baked");
        *mesh
            .triangles
            .iter()
            .min_by(|a, b| {
                let da = (bake::closest_point_on_triangle(&a.vtx, p) - p).length();
                let db = (bake::closest_point_on_triangle(&b.vtx, p) - p).length();
                da.total_cmp(&db)
            })
            .expect("the teapot has triangles")
    })
}

/// The worked triangle's descent from the root, alone in an empty octree.
pub fn descent(w: &World) -> &'static [bake::Descent] {
    static D: OnceLock<Vec<bake::Descent>> = OnceLock::new();
    D.get_or_init(|| {
        // The worked triangle plotted alone, recording every child box it tests on the way down.
        let mut v = bake::Voxelizer::begin_bake(world::SUBDIV, world::bounds());
        let mut trace = Vec::new();
        let tri = worked_triangle(w);
        let po2 = v.po2_bounds;
        v.plot_face(
            0,
            0,
            [0, 0, 0],
            &bake::Face {
                tri: &tri,
                albedo: Vec3::ONE,
                emission: Vec3::ZERO,
            },
            po2,
            &mut Some(&mut trace),
        );
        trace
    })
}

/// The worked leaf: the leaf of the worked triangle's descent nearest the worked point.
pub fn worked_leaf(w: &World) -> (Aabb, IVec3) {
    let p = bake_point(&w.scene) - NODE;
    let d = descent(w);
    // The leaf the triangle reaches nearest the worked point.
    let leaf = d
        .iter()
        .filter(|s| s.overlaps && s.level == world::SUBDIV)
        .min_by(|a, b| {
            (a.aabb.center() - p)
                .length()
                .total_cmp(&(b.aabb.center() - p).length())
        })
        .expect("the triangle reaches a leaf");
    let cell = w.probe.to_cell(leaf.aabb.center()).floor().as_ivec3();
    (leaf.aabb, cell)
}

/// The picked leaf (`cellX`, `cellY`, `cellZ`), or the demonstration's own.
pub fn picked_cell(state: &ShotState, fallback: IVec3) -> IVec3 {
    let p = IVec3::new(
        state.p("cellX").round() as i32,
        state.p("cellY").round() as i32,
        state.p("cellZ").round() as i32,
    );
    if p.min_element() < 0 {
        // No cell picked: the demonstration's own.
        fallback
    } else {
        p
    }
}

/// A leaf cell at or next to `p` (the nearest the octree has, searching a few cells).
pub fn nearest_leaf(probe: &Probe, p: IVec3) -> Option<(usize, IVec3)> {
    let mut best: Option<(i32, usize, IVec3)> = None;
    for dz in -3..=3 {
        for dy in -3..=3 {
            for dx in -3..=3 {
                // Search a 7 x 7 x 7 neighbourhood for the nearest point that falls in a leaf.
                let q = p + IVec3::new(dx, dy, dz);
                if q.min_element() < 0 || q.cmpge(probe.octree_size).any() {
                    continue;
                }
                if let Some(c) = probe.find_leaf(q) {
                    let d = dx * dx + dy * dy + dz * dz;
                    if best.is_none_or(|b| d < b.0) {
                        best = Some((d, c, q));
                    }
                }
            }
        }
    }
    best.map(|b| (b.1, b.2))
}

/// The light demonstration's worked floor cell: in the sphere's shadow from the default omni light.
pub const LIGHT_POINT: Vec3 = Vec3::new(-2.3, 0.0, -0.66);
/// The bounce's worked floor cell, near the red wall.
pub const BOUNCE_POINT: Vec3 = Vec3::new(-2.45, 0.0, 0.9);

/// The extra omni light's position.
pub fn omni_position(state: &ShotState) -> Vec3 {
    Vec3::new(state.p("lightX"), state.p("lightHeight"), state.p("lightZ"))
}

/// The lights of a shot: the panel, and the omni light when it is on.
pub fn shot_lights(state: &ShotState, w: &World) -> Vec<light::Light> {
    let mut l = vec![w.lights[0]];
    if state.p("extraLight") > 0.5 {
        l.push(world::omni_light(&w.probe, omni_position(state)));
    }
    l
}

/// The worked pixel: a point on the sphere, `pixelAzimuth` degrees round from
/// the direction of the red wall (-x) toward +z, `pixelElevation` up. At the
/// default, 60, it faces the sphere camera (demos/presets/sphere-camera.toml),
/// still turned toward the red wall; at 0 it would face away from it.
pub fn pixel_point(state: &ShotState, scene: &Scene) -> (Vec3, Vec3) {
    let s = scene.records()[ids::SPHERE];
    let c = Vec3::from(s.world_center);
    let r = s.world_extent[0];
    let az = state.p("pixelAzimuth").to_radians();
    let el = state.p("pixelElevation").to_radians();
    // The pixel's normal from its azimuth and elevation, on the sphere's surface.
    let n = Vec3::new(-az.cos() * el.cos(), el.sin(), az.sin() * el.cos());
    (c + n * r, n)
}

/// The camera the worked pixel is seen from: the shot's.
fn eye(content: &FrameContent) -> Vec3 {
    Vec3::from(content.uniforms.camera.position)
}

// ---------------------------------------------------------------------------
// The frame.

/// The investigation's far plane.
pub const FAR_PLANE: f32 = 200.0;

fn objects_for(state: &ShotState, statics: &Statics, demo: Demo) -> Vec<lab::layout::ObjectData> {
    let mut objects = statics.scene.records();
    objects[ids::SPHERE].albedo_roughness[3] = state.p("sphereRoughness");
    if demo == Demo::Dynamic {
        objects[ids::TEAPOT] = world::teapot_at(state.p("dynamicX"));
    }
    objects
}

pub fn build_frame(
    state: &ShotState,
    statics: &Statics,
    viewport: [f32; 2],
    pixel_scale: f32,
    time: f32,
) -> FrameContent {
    let demo = Demo::of(state.demo);
    let objects = objects_for(state, statics, demo);
    let lighting = statics.scene.lighting;
    let camera = state.camera.camera_data(viewport);
    let uniforms = FrameUniforms {
        camera,
        light_center_radiance: lighting
            .light
            .center
            .extend(lighting.light.radiance)
            .to_array(),
        light_axis_u: lighting.light.axis_u.extend(0.0).to_array(),
        light_axis_v: lighting.light.axis_v.extend(0.0).to_array(),
        light_color: lighting.light.color.extend(1.0).to_array(),
        ambient_color: lighting.ambient.extend(1.0).to_array(),
        floor_params: [
            objects[ids::FLOOR].albedo_roughness[3],
            lighting.f0,
            0.0,
            1.0,
        ],
        overlay_params: [lighting.exposure, pixel_scale, time, 0.0],
        // The panel would hide the room from a camera above it (the slices).
        counts: [
            objects.len() as u32,
            ids::FLOOR as u32,
            0,
            if state.view_camera().pitch > 60.0 || state.p("octreeLevel") >= 0.0 {
                // A camera from above (or the octree's levels) hides the panel, which would cover the room.
                1 << ids::PANEL
            } else {
                0
            },
        ],
        view_params: [1.0, 0.0, 0.0, 0.0],
    };
    let _ = FAR_PLANE;
    let mut content = FrameContent::new(objects, uniforms);
    let w = world::world();
    match demo {
        Demo::Room => room_demo(state, &mut content),
        Demo::Bounds => bounds_demo(state, w, &mut content),
        Demo::Octree => octree_demo(state, w, &mut content),
        Demo::Cells => cells_demo(state, w, &mut content),
        Demo::Field => field_demo(state, w, &mut content),
        Demo::Light => light_demo(state, w, &mut content),
        Demo::Bounce => bounce_demo(state, w, &mut content),
        Demo::Mips => mips_demo(state, w, &mut content),
        Demo::Dynamic => dynamic_demo(state, &mut content),
        Demo::Pixel => pixel_demo(state, w, &mut content),
        Demo::Cost => cost_demo(state, viewport, &mut content),
        Demo::Compose => compose_demo(state, w, &mut content),
    }
    if demo != Demo::Dynamic {
        shared_overlays(state, w, &mut content);
    }
    content
}

/// What any demonstration can turn on: the box and the cells of a level.
fn shared_overlays(state: &ShotState, w: &World, content: &mut FrameContent) {
    let show = state.p("showBox");
    if show > 0.0 {
        // The probe's box, then (as boundsStep grows) the power-of-two cube and the cells per axis.
        let b = world::bounds();
        wire_box(content, b.position + NODE, b.size, palette::AABB, show, 2.0);
        let step = state.p("boundsStep");
        if step > 0.0 {
            let po2 = w.voxelizer.po2_bounds;
            wire_box(
                content,
                po2.position + NODE,
                po2.size,
                palette::OBB,
                (show * step).min(1.0) * 0.8,
                1.5,
            );
            label(
                content,
                state,
                po2.position + NODE + po2.size * Vec3::new(0.5, 1.0, 0.0),
                tr_with(
                    "demos-po2_cube",
                    lab::args!(side = format!("{:.1}", po2.size.x)),
                ),
                palette::OBB,
            );
        }
        if step > 1.0 {
            let s = w.probe.octree_size;
            label(
                content,
                state,
                b.position + NODE + b.size * Vec3::new(0.5, 0.0, 1.0),
                tr_with(
                    "demos-cells_per_axis",
                    lab::args!(
                        x = s.x,
                        y = s.y,
                        z = s.z,
                        size = format!("{:.0}", 100.0 / w.probe.to_cell_scale)
                    ),
                ),
                palette::AABB,
            );
        }
    }
    let level = state.p("octreeLevel").round() as i32;
    if level >= 0 {
        draw_level(state, w, content, level as usize);
    }
}

/// The cells of one level as points sized to their cells, coloured by `cellView`.
fn draw_level(state: &ShotState, w: &World, content: &mut FrameContent, level: usize) {
    let probe = &w.probe;
    let view = state.p("cellView").round() as i32;
    let cut = state.p("cutHeight");
    let settings = light_settings(state);
    let lit = if view == 4 || view == 5 {
        Some(world::lit_for(&settings))
    } else {
        None
    };
    let omni = if view == 5 && state.p("extraLight") > 0.5 {
        Some(world::direct_with_omni(omni_position(state)))
    } else {
        None
    };
    let half = 0.5 * (1 << (depth(probe) - level)) as f32 / probe.to_cell_scale;
    // Cells wider than 12 cm are drawn as wire boxes, so the coarse levels do not hide the room.
    let boxes = 2.0 * half > 0.12;
    let mut points = Vec::new();
    for cell in probe.level_range(level) {
        // Each cell of the level below the cut, coloured by the view chosen.
        let d = probe.data[cell];
        let p = cell_center(probe, level, d.pos());
        if p.y > cut {
            continue;
        }
        let color = match view {
            // Views: coverage, normal, emission, the light texture, direct light; albedo by default.
            1 => alpha_color(d.albedo().w),
            2 => unit_color(d.normal() * 0.5 + Vec3::splat(0.5)),
            3 => light_color(d.emission()),
            4 => {
                let lit = lit.as_ref().unwrap();
                let mip = probe.mip_count() - 1 - level;
                light_color(lit.texture.texel(mip, d.pos()).truncate() * settings.dynamic_range)
            }
            5 => light_color(match &omni {
                Some(o) if level == depth(probe) => o[cell],
                _ => lit.as_ref().unwrap().direct[cell],
            }),
            _ => unit_color(d.albedo().truncate()),
        };
        if boxes {
            // Coarse cells as wire boxes (albedo shown as coverage), fine ones as balls.
            let (min, size) = cell_box(probe, level, d.pos());
            let c = if view == 0 {
                alpha_color(d.albedo().w)
            } else {
                color
            };
            wire_box(content, min, size, c, 0.75, 1.2);
        } else {
            points.push((p, half * 0.9, color));
        }
    }
    balls(content, &points, 1.0);
}

/// The worked element's mark: a ring about it and its label.
fn mark(content: &mut FrameContent, state: &ShotState, p: Vec3, r: f32) {
    circle(content, p, r, palette::SELECTED, 1.0);
    label(
        content,
        state,
        p + Vec3::Y * (r + 0.05),
        tr("demos-worked_label").into(),
        palette::SELECTED,
    );
}

// ---------------------------------------------------------------------------
// Chapter 1: the room and the settings.

fn room_demo(state: &ShotState, content: &mut FrameContent) {
    let rows = vec![
        row(
            tr("demos-voxelgi_shown"),
            format!("{:.2}", state.p("voxelgiMix")),
            palette::text(),
        ),
        row(
            tr("demos-renderer"),
            tr("demos-forward_plus"),
            palette::THEME_ACCENT,
        ),
        row("size", "20 x 20 x 20 m", palette::AABB),
        row(tr("demos-lab_box"), "6.4 x 3.2 x 6.4 m", palette::AABB),
        row("subdiv", "128", palette::AABB),
        row("dynamic_range", "2.0", palette::INCIDENT),
        row("energy", "1.0", palette::INCIDENT),
        row("bias / normal_bias", "1.5 / 0.0", palette::VIEW),
        row("propagation", "0.5", palette::INCIDENT),
        row("use_two_bounces", "true", palette::INCIDENT),
        row("interior", "false", palette::VIEW),
        row(
            "voxel_gi/quality",
            if state.p("quality") > 0.5 {
                tr("demos-quality_high")
            } else {
                tr("demos-quality_low")
            },
            palette::VIEW,
        ),
        row(
            "gi/use_half_resolution",
            if state.p("giHalfResolution") > 0.5 {
                "true"
            } else {
                "false"
            },
            palette::VIEW,
        ),
    ];
    content.readout = Some(readout(
        tr("demos-settings"),
        tr("demos-settings_subtitle"),
        lit_rows(state, rows),
    ));
}

// ---------------------------------------------------------------------------
// Chapter 8: what it costs, and what Godot does about it.

/// The room with what its VoxelGI costs, counted (cost.rs): an update's
/// dispatches over the cells, and a frame's pixels and cone steps.
fn cost_demo(state: &ShotState, viewport: [f32; 2], content: &mut FrameContent) {
    let c = super::cost::costs(state);
    let half = state.p("giHalfResolution") > 0.5;
    let (w, h) = if half {
        (
            (viewport[0] as u32 / 2).max(1),
            (viewport[1] as u32 / 2).max(1),
        )
    } else {
        (viewport[0] as u32, viewport[1] as u32)
    };
    let high = state.p("quality") > 0.5;
    let (cones, steps) = if high {
        (6, c.high_steps)
    } else {
        (4, c.low_steps)
    };
    let every = state.p("giRelightEachFrame") > 0.5;
    let rows = vec![
        row(
            tr("demos-cost_light_passes"),
            if every {
                tr("demos-cost_every_frame")
            } else {
                tr("demos-cost_on_change")
            },
            palette::INCIDENT,
        ),
        row(
            tr("demos-cost_dispatches"),
            tr_with(
                "demos-cost_dispatches_value",
                lab::args!(count = c.dispatches, passes = c.passes),
            ),
            palette::INCIDENT,
        ),
        row(
            tr("demos-cost_workgroups"),
            format!("{}", c.workgroups),
            palette::INCIDENT,
        ),
        row(
            tr("demos-cost_leaves"),
            format!("{}", c.leaves),
            palette::AABB,
        ),
        row(
            tr("demos-cost_cells"),
            format!("{} / {}", c.cells, c.texels),
            palette::AABB,
        ),
        row(tr("demos-cost_pixels"), format!("{w} x {h}"), palette::VIEW),
        row(
            tr("demos-cost_cones"),
            format!("{cones} x {steps} = {}", cones * steps),
            palette::VIEW,
        ),
        row(
            tr("demos-cost_spec"),
            format!(
                "{} / {} / {}",
                c.spec_steps[0], c.spec_steps[1], c.spec_steps[2]
            ),
            palette::VIEW,
        ),
    ];
    content.readout = Some(readout(
        tr("demos-cost"),
        tr("demos-cost_subtitle"),
        lit_rows(state, rows),
    ));
    // The sorted cells, each level one run (the first leaf marked), for the story.
    if state.shows_image("cellArray") {
        let probe = &world::world().probe;
        content.images.push((
            "cellArray".into(),
            cell_array_inset(probe, probe.level_offset(probe.leaf_level())),
        ));
    }
}

/// With `costSplit`, the cost demonstration's two views from one camera: the
/// frame at full resolution on the left, as the values set it on the right.
pub fn panes(state: &ShotState) -> Vec<lab::investigation::Pane> {
    if Demo::of(state.demo) != Demo::Cost || state.p("costSplit") < 0.5 {
        return Vec::new();
    }
    let mut left = state.clone();
    left.set("giHalfResolution", 0.0);
    let right = if state.p("giHalfResolution") > 0.5 {
        tr("demos-cost_half")
    } else {
        tr("demos-cost_full")
    };
    vec![
        lab::investigation::Pane {
            state: left,
            title: Some(tr("demos-cost_full").to_string()),
        },
        lab::investigation::Pane {
            state: state.clone(),
            title: Some(right.to_string()),
        },
    ]
}

// ---------------------------------------------------------------------------
// Chapter 2: the bounds, the octree, the leaves.

fn bounds_demo(state: &ShotState, w: &World, content: &mut FrameContent) {
    let v = &w.voxelizer;
    let s = v.octree_size();
    let rows = vec![
        row(
            "size",
            format!(
                "{:.1} x {:.1} x {:.1} m",
                world::SIZE.x,
                world::SIZE.y,
                world::SIZE.z
            ),
            palette::AABB,
        ),
        row(
            tr("demos-longest_axis"),
            format!("x: 1 << {} = {}", v.cell_subdiv, s[0]),
            palette::AABB,
        ),
        row(
            tr("demos-shorter_axes"),
            format!("y {} (3.2 m), z {}", s[1], s[2]),
            palette::AABB,
        ),
        row(
            tr("demos-po2_bounds"),
            format!("{:.1} m cube", v.po2_bounds.size.x),
            palette::OBB,
        ),
        row(
            "cell_size",
            format!("{:.0} cm", v.cell_size * 100.0),
            palette::AABB,
        ),
        row(
            tr("demos-levels"),
            format!("{}", v.cell_subdiv + 1),
            palette::AABB,
        ),
        row(
            tr("demos-dense_cells"),
            format!("{}", s[0] * s[1] * s[2]),
            palette::text(),
        ),
        row(
            tr("demos-bake_time"),
            format!("{:.2} s", w.bake_seconds),
            palette::text_dim(),
        ),
    ];
    content.readout = Some(readout(
        tr("demos-bounds"),
        tr("demos-bounds_subtitle"),
        lit_rows(state, rows),
    ));
}

fn octree_demo(state: &ShotState, w: &World, content: &mut FrameContent) {
    let probe = &w.probe;
    let level = state.p("octreeLevel").round() as i32;
    let mut rows = Vec::new();
    for (l, &c) in probe.level_counts.iter().enumerate() {
        let color = if l as i32 == level {
            palette::SELECTED
        } else {
            palette::AABB
        };
        rows.push(row(
            tr_with("demos-level_n", lab::args!(level = l)),
            format!("{c}"),
            color,
        ));
    }
    // The worked triangle's descent.
    if state.p("workedTriangle") > 0.5 {
        rows.extend(triangle_descent(state, w, content));
    }
    // The worked leaf's columns.
    if state.p("workedLeaf") > 0.5 {
        rows.extend(leaf_columns(state, w, content));
    }
    content.readout = Some(readout(
        tr("demos-octree"),
        tr("demos-octree_subtitle"),
        lit_rows(state, rows),
    ));
}

/// The worked triangle's descent through the octree to `descentLevel`, and the separating-axis test on the first
/// child it skipped at the deepest level drawn (to `satStep`); its row.
fn triangle_descent(
    state: &ShotState,
    w: &World,
    content: &mut FrameContent,
) -> Vec<(String, String, Color32)> {
    let mut rows = Vec::new();
    let tri = worked_triangle(w);
    let pts: Vec<Vec3> = [0, 1, 2, 0].iter().map(|&k| tri.vtx[k] + NODE).collect();
    content.draw.polyline(&pts, palette::SELECTED, 1.0, 3.0);
    // The level being entered in full, the one above it fading: the
    // children the triangle overlaps lit, the ones it misses dim.
    let shown = state.p("descentLevel");
    for d in descent(w) {
        // A box fades in as its level comes, holds a level, dims the next, then goes.
        let age = shown - d.level as f32;
        let a = if age < 0.0 {
            (1.0 + age).max(0.0)
        } else if age < 1.0 {
            1.0
        } else if age < 2.0 {
            0.3
        } else {
            0.0
        };
        if a <= 0.0 {
            continue;
        }
        if d.overlaps {
            wire_box(
                content,
                d.aabb.position + NODE,
                d.aabb.size,
                palette::SELECTED,
                a,
                2.0,
            );
        } else {
            wire_box(
                content,
                d.aabb.position + NODE,
                d.aabb.size,
                palette::text_dim(),
                a * 0.4,
                1.0,
            );
        }
    }
    let deepest = descent(w)
        .iter()
        .filter(|d| d.overlaps && d.level as f32 <= shown)
        .map(|d| d.level)
        .max()
        .unwrap_or(0);
    let count = descent(w)
        .iter()
        .filter(|d| d.overlaps && d.level == deepest)
        .count();
    label(
        content,
        state,
        tri.vtx[0] + NODE + Vec3::Y * 0.06,
        tr_with("demos-descent", lab::args!(level = deepest, count = count)),
        palette::SELECTED,
    );
    // The separating-axis test on the first child it skipped at the deepest level drawn.
    let step = state.p("satStep").round() as i32;
    if step > 0
        && let Some(sk) = descent(w)
            .iter()
            .find(|d| !d.overlaps && d.level == deepest.max(1))
    {
        // The stage at which the test separates them, named in the readout.
        let stage = bake::triangle_box_stage(sk.aabb.center(), sk.aabb.size * 0.5, &tri.vtx);
        wire_box(
            content,
            sk.aabb.position + NODE,
            sk.aabb.size,
            palette::MISMATCH,
            1.0,
            2.0,
        );
        let text = match stage {
            bake::SatStage::EdgeAxis(k) => tr_with("demos-sat_edge", lab::args!(k = k + 1)),
            bake::SatStage::BoxAxis(a) => {
                tr_with("demos-sat_box", lab::args!(axis = ["x", "y", "z"][a]))
            }
            bake::SatStage::Plane => tr("demos-sat_plane").to_string(),
            bake::SatStage::Overlap => tr("demos-sat_overlap").to_string(),
        };
        rows.push(row(tr("demos-sat"), text, palette::MISMATCH));
    }
    rows
}

/// The worked leaf's 4 x 4 columns along the dominant axis, as far as `leafColumns`, their hits, and its rows.
fn leaf_columns(
    state: &ShotState,
    w: &World,
    content: &mut FrameContent,
) -> Vec<(String, String, Color32)> {
    let mut rows = Vec::new();
    let (aabb, cell) = worked_leaf(w);
    let tri = worked_triangle(w);
    wire_box(
        content,
        aabb.position + NODE,
        aabb.size,
        palette::SELECTED,
        1.0,
        2.0,
    );
    let s = bake::leaf_sample(
        &bake::Face {
            tri: &tri,
            albedo: Vec3::ONE,
            emission: Vec3::ZERO,
        },
        aabb,
    );
    let n = state.p("leafColumns").round().clamp(0.0, 16.0) as usize;
    let mut hits = 0;
    for c in s.columns.iter().take(n) {
        // Each column up to `leafColumns`, bright where the triangle crosses it, its hit marked.
        let color = if c.hit.is_some() {
            palette::VIEW
        } else {
            palette::text_dim()
        };
        wire_box(
            content,
            c.from + NODE,
            c.to - c.from,
            color,
            if c.hit.is_some() {
                0.9
            } else {
                0.3
            },
            1.0,
        );
        if let Some(h) = c.hit {
            ball(content, h + NODE, 0.003, palette::VIEW, 1.0);
            hits += 1;
        }
    }
    rows.push(row(
        tr("demos-leaf"),
        format!("({}, {}, {})", cell.x, cell.y, cell.z),
        palette::SELECTED,
    ));
    rows.push(row(
        tr("demos-dominant_axis"),
        ["x", "y", "z"][s.closest_axis].to_string(),
        palette::SELECTED,
    ));
    rows.push(row(
        tr("demos-columns_hit"),
        format!("{hits} / {n}"),
        palette::VIEW,
    ));
    rows.push(row(
        "alpha",
        format!("{:.4}", hits as f32 / 16.0),
        palette::VIEW,
    ));
    rows
}

// ---------------------------------------------------------------------------
// Chapter 6: a dynamic object.

/// How much brighter the dynamic texels are drawn than stored.
const DYNAMIC_GAIN: f32 = 4.0;

fn dynamic_demo(state: &ShotState, content: &mut FrameContent) {
    let dw = world::dynamic_world();
    let probe = &dw.probe;
    let dx = state.p("dynamicX");
    let plot = world::dynamic_plot(dx);
    let size = plot.end - plot.begin;
    let mut rows = vec![row(
        tr("demos-dynamic_box"),
        format!("{} x {} x {}", size.x, size.y, size.z),
        palette::SELECTED,
    )];
    // The oversampled box, in the world.
    let m = super::dynamic::MULTIPLIER as f32;
    let bmin = probe.from_cell(plot.begin.as_vec3() / m) + NODE;
    let bmax = probe.from_cell(plot.end.as_vec3() / m) + NODE;
    wire_box(content, bmin, bmax - bmin, palette::SELECTED, 0.8, 1.5);
    if state.p("dynamicVoxels") > 0.5 {
        rows.extend(plotted_voxels(state, probe, &plot, content));
    }
    let view = state.p("dynamicView").round() as i32;
    if (0..6).contains(&view) {
        rows.extend(view_maps(state, &plot, (dx, view), content));
    }
    let maps = super::dynamic::OVERSAMPLED + probe.mip_count();
    rows.push(row(
        tr("demos-dispatches"),
        tr_with("demos-dispatches_value", lab::args!(views = 6, per = maps)),
        palette::text_dim(),
    ));
    content.readout = Some(readout(
        tr("demos-dynamic"),
        tr("demos-dynamic_subtitle"),
        lit_rows(state, rows),
    ));
}

/// The voxels the dynamic pass plotted into the chosen mip, four times brighter than stored, and their rows.
fn plotted_voxels(
    state: &ShotState,
    probe: &Probe,
    plot: &super::dynamic::Plot,
    content: &mut FrameContent,
) -> Vec<(String, String, Color32)> {
    let mut rows = Vec::new();
    let mip = state
        .p("dynamicMip")
        .round()
        .clamp(0.0, (probe.mip_count() - 1) as f32) as usize;
    let level = probe.mip_count() - 1 - mip;
    let half = 0.5 * (1 << mip) as f32 / probe.to_cell_scale;
    // Drawn four times brighter than stored (the room's direct light is dim in 8 bits).
    let points: Vec<(Vec3, f32, Color32)> = plot.plotted[mip]
        .iter()
        .filter_map(|&p| {
            let t = plot.texture.texel(mip, p);
            (t.w > 0.0).then(|| {
                (
                    cell_center(probe, level, p),
                    half * 0.85,
                    light_color(t.truncate() * world::DYNAMIC_RANGE * DYNAMIC_GAIN),
                )
            })
        })
        .collect();
    if 2.0 * half > 0.12 {
        // Big voxels as wire boxes, small ones as balls.
        for (p, h, c) in &points {
            wire_box(
                content,
                *p - Vec3::splat(*h / 0.85),
                Vec3::splat(2.0 * *h / 0.85),
                *c,
                0.9,
                1.5,
            );
        }
    } else {
        balls(content, &points, 1.0);
    }
    rows.push(row(
        tr("demos-dynamic_gain"),
        format!("x {DYNAMIC_GAIN}"),
        palette::text_dim(),
    ));
    rows.push(row(
        tr_with("demos-plotted_mip", lab::args!(mip = mip)),
        format!("{}", plot.plotted[mip].len()),
        palette::SELECTED,
    ));
    rows
}

/// One view of the dynamic pass (`view` of 0..6, the object at `dynamicX` = `dx`): its rows, the first map's
/// albedo, and the chosen shrink map's light, as insets.
fn view_maps(
    state: &ShotState,
    plot: &super::dynamic::Plot,
    (dx, view): (f32, i32),
    content: &mut FrameContent,
) -> Vec<(String, String, Color32)> {
    let mut rows = Vec::new();
    let v = &plot.views[view as usize];
    let level = state
        .p("shrinkLevel")
        .round()
        .clamp(0.0, (v.maps.len() - 1) as f32) as usize;
    let map = &v.maps[level];
    let dir = ["+x", "+y", "+z", "-x", "-y", "-z"][view as usize];
    rows.push(row(
        tr("demos-view"),
        format!(
            "{dir}: {} x {}",
            v.maps[0].rect_size.x, v.maps[0].rect_size.y
        ),
        palette::SELECTED,
    ));
    rows.push(row(
        tr("demos-shrink_map"),
        tr_with(
            "demos-shrink_map_value",
            lab::args!(
                level = level,
                w = map.rect_size.x,
                h = map.rect_size.y,
                mip = map.mipmap
            ),
        ),
        palette::SELECTED,
    ));
    // The first map's albedo, and the chosen map's light.
    let first = &v.maps[0];
    let (aw, ah) = (first.rect_size.x as usize, first.rect_size.y as usize);
    let albedo: Vec<u8> = (0..ah)
        .rev()
        .flat_map(|y| (0..aw).map(move |x| (y, x)))
        .flat_map(|(y, x)| {
            // Rows from the top, so +y is up; an uncovered texel is the dark ground.
            let a = v.albedo[y * aw + x];
            if a.w > 0.0 {
                [
                    (a.x * 255.0) as u8,
                    (a.y * 255.0) as u8,
                    (a.z * 255.0) as u8,
                    255,
                ]
            } else {
                [16, 16, 20, 255]
            }
        })
        .collect();
    let mut ins = PictureInset::new(
        tr_with("demos-view_albedo", lab::args!(dir = dir)),
        Picture::Pixels {
            key: format!("dyn-albedo-{dx}-{view}"),
            size: [aw, ah],
            rgba: Arc::new(albedo),
        },
        [320.0, 320.0 * ah as f32 / aw.max(1) as f32],
    );
    ins.corner = Corner::TopLeft;
    content.insets.push(ins);
    let (lw, lh) = (map.rect_size.x as usize, map.rect_size.y as usize);
    let light: Vec<u8> = (0..lh)
        .rev()
        .flat_map(|y| (0..lw).map(move |x| (y, x)))
        .flat_map(|(y, x)| {
            let l = map.light[y * lw + x];
            // The light per covered texel, its coverage divided out, brightened as the voxels are.
            if l.w > 0.0 {
                let c = light_color(l.truncate() / l.w.max(1e-3) * DYNAMIC_GAIN);
                [c.r(), c.g(), c.b(), 255]
            } else {
                [16, 16, 20, 255]
            }
        })
        .collect();
    let mut ins = PictureInset::new(
        tr_with("demos-view_light", lab::args!(level = level)),
        Picture::Pixels {
            key: format!("dyn-light-{dx}-{view}-{level}"),
            size: [lw.max(1), lh.max(1)],
            rgba: Arc::new(light),
        },
        [320.0, 320.0 * lh as f32 / lw.max(1) as f32],
    );
    ins.corner = Corner::TopRight;
    content.insets.push(ins);
    rows
}

// ---------------------------------------------------------------------------
// Chapter 7: a pixel's cones.

/// The worked pixel's cones, as `process` traces them.
pub fn pixel_cones(state: &ShotState, w: &World, eye_pos: Vec3) -> (Vec3, Vec3, Cones, Vec4, Vec4) {
    let (p, n) = pixel_point(state, &w.scene);
    // The view reflected about the normal, then the pixel's cones through the probe's instance.
    let v = (eye_pos - p).normalize();
    let r = lab::scene::reflect(-v, n);
    let inst = world::instance(
        &w.probe,
        &LightSettings::default(),
        state.p("interior") > 0.5,
    );
    let mut cones = Cones::default();
    let (amb, spec) = cone::process(
        &inst,
        &w.lit.texture,
        &cone::Sample {
            position: p,
            normal: n,
            reflection: r,
            roughness: state.p("sphereRoughness"),
        },
        state.p("quality") > 0.5,
        Some(&mut cones),
    );
    (p, n, cones, amb, spec)
}

fn pixel_demo(state: &ShotState, w: &World, content: &mut FrameContent) {
    let probe = &w.probe;
    let e = eye(content);
    let (p, n, cones, amb, spec) = pixel_cones(state, w, e);
    let inst = world::instance(probe, &LightSettings::default(), false);
    let mut rows = vec![row(
        tr("demos-tag"),
        tr("demos-tag_value"),
        palette::text_dim(),
    )];
    if state.p("workedPixel") > 0.5 {
        ball(content, p, 0.02, palette::SELECTED, 1.0);
        content.points.push(("pixel.point".into(), p));
        mark(content, state, p, 0.06);
        content.draw.segment(
            p,
            p + n * 0.25,
            palette::NORMAL,
            1.0,
            2.0,
            lab::frame::FLAG_ARROW,
        );
        let shown = state.p("conesShown").round() as usize;
        let steps = state.p("coneSteps").round() as usize;
        // The first `conesShown` diffuse cones and the specular one, each to `coneSteps` steps.
        for (i, (dir, _w, st, _c)) in cones.diffuse.iter().enumerate().take(shown) {
            draw_cone_world(content, &inst, cones.position, *dir, st, steps, 0.9);
            let _ = i;
        }
        if state.p("specCone") > 0.5
            && let Some((dir, _tan, st, _c)) = &cones.specular
        {
            draw_cone_world(content, &inst, cones.position, *dir, st, steps, 1.0);
        }
        rows.push(row(
            tr("demos-cell_position"),
            v3(cones.position),
            palette::VIEW,
        ));
        rows.push(row(
            tr("demos-edge_blend"),
            format!("{:.3}", cones.blend),
            palette::VIEW,
        ));
        let last = shown.min(cones.diffuse.len());
        if let Some((_, _, st, c)) = cones.diffuse.get(last.saturating_sub(1)) {
            rows.push(row(
                tr_with("demos-cone_n", lab::args!(n = last.max(1))),
                tr_with(
                    "demos-cone_alpha",
                    lab::args!(steps = st.len(), alpha = format!("{:.2}", c.w)),
                ),
                palette::VIEW,
            ));
            if let Some(s) = st.get(steps.min(st.len()).saturating_sub(1)) {
                step_rows(&mut rows, s);
            }
        }
        if let Some((_, tan, st, _)) = &cones.specular {
            rows.push(row(
                tr("demos-spec_cone"),
                tr_with(
                    "demos-spec_value",
                    lab::args!(
                        angle = format!("{:.1}", tan.atan().to_degrees()),
                        steps = st.len()
                    ),
                ),
                palette::REFLECTED,
            ));
        }
    }
    rows.push(row(tr("demos-ambient"), v4(amb), palette::INCIDENT));
    rows.push(row(tr("demos-reflection"), v4(spec), palette::REFLECTED));
    content.readout = Some(readout(
        tr("demos-pixel"),
        tr("demos-pixel_subtitle"),
        lit_rows(state, rows),
    ));
}

/// A cone of the screen pass (positions in the instance's cells).
fn draw_cone_world(
    content: &mut FrameContent,
    inst: &cone::Instance,
    start: Vec3,
    dir: Vec3,
    steps: &[ConeStep],
    shown: usize,
    alpha: f32,
) {
    let to_world = |dist: f32| inst.from_cell(start + dir * dist);
    let shown_steps = &steps[..shown.min(steps.len())];
    let reach = shown_steps.last().map_or(1.0, |s| s.dist);
    content
        .draw
        .segment(to_world(0.0), to_world(reach), palette::VIEW, alpha, 2.0, 0);
    for s in shown_steps {
        // Each step a circle of the cone's width there, coloured by what it sampled (a dot when too wide).
        let r = s.diameter * 0.5 / inst.to_cell_scale;
        let c = if s.sample.w > 0.0 {
            light_color(s.sample.truncate() / s.sample.w.max(1e-3) * inst.dynamic_range)
        } else {
            palette::text_dim()
        };
        if r <= MAX_CIRCLE {
            circle(content, to_world(s.dist), r, c, alpha);
        } else {
            ball(content, to_world(s.dist), 0.02, c, alpha);
        }
    }
}

// ---------------------------------------------------------------------------
// Chapter 8: the composite.

fn compose_demo(state: &ShotState, w: &World, content: &mut FrameContent) {
    let e = eye(content);
    let (p, n, _cones, amb, spec) = pixel_cones(state, w, e);
    let lighting = w.scene.lighting;
    let v = (e - p).normalize();
    let r = lab::scene::reflect(-v, n);
    // The environment's light, replaced by VoxelGI's by its coverage times the mix.
    let env_amb = lab::scene::environment_radiance(&lighting, n);
    let env_ref = sky_reflection(&lighting, r, state.p("sphereRoughness"));
    let mix = state.p("voxelgiMix");
    let ambient = env_amb.lerp(amb.truncate(), amb.w * mix);
    let reflection = env_ref.lerp(spec.truncate(), spec.w * mix);
    let s = w.scene.records()[ids::SPHERE];
    let albedo = Vec3::new(
        s.albedo_roughness[0],
        s.albedo_roughness[1],
        s.albedo_roughness[2],
    );
    let nv = n.dot(v).max(1e-4);
    let env = lab::scene::env_brdf_approx(0.04, state.p("sphereRoughness"), nv);
    if state.p("workedPixel") > 0.5 {
        ball(content, p, 0.02, palette::SELECTED, 1.0);
        content.points.push(("pixel.point".into(), p));
    }
    let rows = vec![
        row(tr("demos-env_ambient"), v3(env_amb), palette::text_dim()),
        row(tr("demos-voxelgi_ambient"), v4(amb), palette::INCIDENT),
        row(tr("demos-mixed_ambient"), v3(ambient), palette::INCIDENT),
        row(
            tr("demos-times_albedo"),
            v3(ambient * albedo),
            palette::AABB,
        ),
        row(tr("demos-env_reflection"), v3(env_ref), palette::text_dim()),
        row(tr("demos-voxelgi_reflection"), v4(spec), palette::REFLECTED),
        row(
            tr("demos-mixed_reflection"),
            v3(reflection),
            palette::REFLECTED,
        ),
        row(
            tr("demos-times_brdf"),
            format!("{} (x {env:.3})", v3(reflection * env)),
            palette::REFLECTED,
        ),
    ];
    content.readout = Some(readout(
        tr("demos-compose"),
        tr("demos-compose_subtitle"),
        lit_rows(state, rows),
    ));
}

/// voxelgiSkyReflection (shaders/voxelgi_shading.wgsl): the lab's sky
/// averaged over nine directions spread by the roughness.
pub fn sky_reflection(lighting: &lab::scene::Lighting, r: Vec3, roughness: f32) -> Vec3 {
    let v0 = if r.y.abs() > 0.999 {
        Vec3::X
    } else {
        Vec3::Y
    };
    // The sky around the reflection, a 3 x 3 spread widened by roughness, averaged.
    let t = v0.cross(r).normalize();
    let b = r.cross(t);
    let mut sum = Vec3::ZERO;
    for i in -1..=1 {
        for j in -1..=1 {
            sum += lab::scene::environment_radiance(
                lighting,
                (r + (t * i as f32 + b * j as f32) * roughness).normalize(),
            );
        }
    }
    sum / 9.0
}

/// The scene shading's derived values (gShading's `{@ derived}` members).
pub fn shader_data(block: &mut lab::shader_data::Block, state: &ShotState) {
    let w = world::world();
    let inst = world::instance(&w.probe, &LightSettings::default(), false);
    block.set("toCellOrigin", inst.to_cell_origin.to_array());
    block.set("toCellScale", inst.to_cell_scale);
    block.set("bounds", inst.bounds.to_array());
    // The scene node's settings (at their defaults, the instance's).
    block.set(
        "dynamicRange",
        state.p("giDynamicRange") * state.p("giEnergy"),
    );
    block.set("bias", state.p("giBias"));
    block.set("normalBias", state.p("giNormalBias"));
    block.set("mipCount", w.probe.mip_count() as f32);
    block.set("sdfOffset", 0.0);
    block.set("halfResolution", state.p("giHalfResolution"));
}
