//! What each demonstration of the VoxelGI investigation shows: the room
//! shaded with the WGSL port of the screen pass, and the structures of the CPU
//! model (bake.rs, storage.rs, sdf.rs, light.rs, dynamic.rs, cone.rs) drawn over
//! it. Everything a frame shows is computed from the shot's values and the
//! world (world.rs), built once, and its variants, kept once computed.

use super::bake::{self, Aabb, Triangle, CHILD_EMPTY};
use super::cone::{self, Cones};
use super::light::{self, ConeStep, LightTrace};
use super::sdf;
use super::shot::Demo;
use super::storage::Probe;
use super::world::{self, LightSettings, World, NODE};
use crate::godot::voxelgi::text::{tr, tr_with};
use egui::Color32;
use glam::{IVec3, Vec3, Vec4};
use lab::frame::{FrameContent, Label, Readout, Statics};
use lab::layout::FrameUniforms;
use lab::palette;
use lab::scene::{ids, Scene};
use lab::shot::ShotState;
use lab::utility::inset::{Corner, Marker, Picture, PictureInset};
use std::sync::{Arc, OnceLock};

// ---------------------------------------------------------------------------
// Drawing helpers.

fn wire_box(content: &mut FrameContent, min: Vec3, size: Vec3, color: Color32, alpha: f32, width: f32) {
    let c = |i: u32| min + size * Vec3::new((i & 1) as f32, ((i >> 1) & 1) as f32, ((i >> 2) & 1) as f32);
    for (a, b) in [(0, 1), (2, 3), (4, 5), (6, 7), (0, 2), (1, 3), (4, 6), (5, 7), (0, 4), (1, 5), (2, 6), (3, 7)] {
        content.draw.segment(c(a), c(b), color, alpha, width, 0);
    }
}

/// The size in pixels of `r` metres at `p`, for the frame's camera.
fn px(content: &FrameContent, p: Vec3, r: f32) -> f32 {
    let c = &content.uniforms.camera;
    let d = (p - Vec3::from(c.position)).length().max(1e-3);
    r / (d * (c.vertical_fov * 0.5).tan()) * c.viewport_size[1] * 0.5
}

fn ball(content: &mut FrameContent, p: Vec3, r: f32, color: Color32, alpha: f32) {
    let radius = px(content, p, r).max(1.5);
    content.draw.point(p, radius, color, alpha);
}

/// Points of `r` metres, each dimmed to a ghost where the scene's objects
/// hide it from the camera (the overlay has no depth test of its own).
fn balls(content: &mut FrameContent, points: &[(Vec3, f32, Color32)], alpha: f32) {
    let eye = eye(content);
    let objects = &content.objects;
    let seen = light::par_map(0..points.len(), |i| {
        let (p, r, _) = points[i];
        let d = p - eye;
        let dist = d.length();
        let hit = lab::scene::trace_scene(objects, eye, d / dist.max(1e-6), (dist - 2.5 * r).max(0.0), u32::MAX);
        if hit.is_hit() { 0.12 } else { 1.0 }
    });
    for (&(p, r, c), a) in points.iter().zip(seen) {
        ball(content, p, r, c, alpha * a);
    }
}

/// The largest circle drawn, metres: a cone's farther samples (or a march's
/// longer steps) are drawn as dots, so they do not cover the room.
const MAX_CIRCLE: f32 = 0.8;

/// A circle of `r` metres about `p`, facing the camera (a cone's sample).
fn circle(content: &mut FrameContent, p: Vec3, r: f32, color: Color32, alpha: f32) {
    let eye = Vec3::from(content.uniforms.camera.position);
    let n = (eye - p).normalize_or_zero();
    let u = n.any_orthonormal_vector();
    let v = n.cross(u);
    let pts: Vec<Vec3> = (0..=32).map(|k| {
        let a = k as f32 / 32.0 * std::f32::consts::TAU;
        p + (u * a.cos() + v * a.sin()) * r
    }).collect();
    content.draw.polyline(&pts, color, alpha, 1.5);
}

fn label(content: &mut FrameContent, state: &ShotState, anchor: Vec3, text: String, color: Color32) {
    if state.p("labels") > 0.5 {
        content.labels.push(Label { anchor, text, color });
    }
}

fn row(name: impl Into<String>, value: impl Into<String>, color: Color32) -> (String, String, Color32) {
    (name.into(), value.into(), color)
}

/// The rows' colours, the lit row (`focusRow`) in the selection colour.
fn lit_rows(state: &ShotState, rows: Vec<(String, String, Color32)>) -> Vec<(String, String, Color32)> {
    let focus = state.p("focusRow").round() as i32;
    rows.into_iter().enumerate().map(|(i, (a, b, c))| if i as i32 == focus { (a, b, palette::SELECTED) } else { (a, b, c) }).collect()
}

fn readout(title: &str, subtitle: &str, rows: Vec<(String, String, Color32)>) -> Readout {
    Readout { title: title.into(), subtitle: subtitle.into(), formula: None, formula_lit: None, rows }
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
    LightSettings { dynamic_range: state.p("dynamicRange"), propagation: state.p("propagation"), two_bounces: state.p("bounces") > 1.5 }
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
    let c = Vec3::from(t.world_center);
    let ro = c + Vec3::new(2.0, 0.02, 0.4);
    let rd = (c - ro).normalize();
    lab::scene::intersect_object(&t, ro, rd, 10.0).map(|(d, _)| ro + rd * d).unwrap_or(c)
}

/// The worked triangle: the teapot's triangle nearest the worked point, in the node's space.
pub fn worked_triangle(w: &World) -> Triangle {
    static TRI: OnceLock<Triangle> = OnceLock::new();
    *TRI.get_or_init(|| {
        let p = bake_point(&w.scene) - NODE;
        let mesh = w.meshes.iter().find(|m| m.object == ids::TEAPOT).expect("the teapot is baked");
        *mesh.triangles.iter().min_by(|a, b| {
            let da = (bake::closest_point_on_triangle(&a.vtx, p) - p).length();
            let db = (bake::closest_point_on_triangle(&b.vtx, p) - p).length();
            da.total_cmp(&db)
        }).expect("the teapot has triangles")
    })
}

/// The worked triangle's descent from the root, alone in an empty octree.
pub fn descent(w: &World) -> &'static [bake::Descent] {
    static D: OnceLock<Vec<bake::Descent>> = OnceLock::new();
    D.get_or_init(|| {
        let mut v = bake::Voxelizer::begin_bake(world::SUBDIV, world::bounds());
        let mut trace = Vec::new();
        let tri = worked_triangle(w);
        let po2 = v.po2_bounds;
        v.plot_face(0, 0, [0, 0, 0], &tri, Vec3::ONE, Vec3::ZERO, po2, &mut Some(&mut trace));
        trace
    })
}

/// The worked leaf: the leaf of the worked triangle's descent nearest the worked point.
pub fn worked_leaf(w: &World) -> (Aabb, IVec3) {
    let p = bake_point(&w.scene) - NODE;
    let d = descent(w);
    let leaf = d.iter().filter(|s| s.overlaps && s.level == world::SUBDIV).min_by(|a, b| (a.aabb.center() - p).length().total_cmp(&(b.aabb.center() - p).length())).expect("the triangle reaches a leaf");
    let cell = w.probe.to_cell(leaf.aabb.center()).floor().as_ivec3();
    (leaf.aabb, cell)
}

/// The picked leaf (`cellX`, `cellY`, `cellZ`), or the demonstration's own.
pub fn picked_cell(state: &ShotState, fallback: IVec3) -> IVec3 {
    let p = IVec3::new(state.p("cellX").round() as i32, state.p("cellY").round() as i32, state.p("cellZ").round() as i32);
    if p.min_element() < 0 { fallback } else { p }
}

/// A leaf cell at or next to `p` (the nearest the octree has, searching a few cells).
pub fn nearest_leaf(probe: &Probe, p: IVec3) -> Option<(usize, IVec3)> {
    let mut best: Option<(i32, usize, IVec3)> = None;
    for dz in -3..=3 {
        for dy in -3..=3 {
            for dx in -3..=3 {
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
/// the direction of the red wall (-x), `pixelElevation` up.
pub fn pixel_point(state: &ShotState, scene: &Scene) -> (Vec3, Vec3) {
    let s = scene.records()[ids::SPHERE];
    let c = Vec3::from(s.world_center);
    let r = s.world_extent[0];
    let az = state.p("pixelAzimuth").to_radians();
    let el = state.p("pixelElevation").to_radians();
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

pub fn build_frame(state: &ShotState, statics: &Statics, viewport: [f32; 2], pixel_scale: f32, time: f32) -> FrameContent {
    let demo = Demo::of(state.demo);
    let objects = objects_for(state, statics, demo);
    let lighting = statics.scene.lighting;
    let camera = state.camera.camera_data(viewport);
    let uniforms = FrameUniforms {
        camera,
        light_center_radiance: lighting.light.center.extend(lighting.light.radiance).to_array(),
        light_axis_u: lighting.light.axis_u.extend(0.0).to_array(),
        light_axis_v: lighting.light.axis_v.extend(0.0).to_array(),
        light_color: lighting.light.color.extend(1.0).to_array(),
        ambient_color: lighting.ambient.extend(1.0).to_array(),
        floor_params: [objects[ids::FLOOR].albedo_roughness[3], lighting.f0, 0.0, 1.0],
        overlay_params: [lighting.exposure, pixel_scale, time, 0.0],
        // The panel would hide the room from a camera above it (the slices).
        counts: [objects.len() as u32, ids::FLOOR as u32, 0, if state.camera.pitch > 60.0 || state.p("octreeLevel") >= 0.0 { 1 << ids::PANEL } else { 0 }],
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
        let b = world::bounds();
        wire_box(content, b.position + NODE, b.size, palette::AABB, show, 2.0);
        let step = state.p("boundsStep");
        if step > 0.0 {
            let po2 = w.voxelizer.po2_bounds;
            wire_box(content, po2.position + NODE, po2.size, palette::OBB, (show * step).min(1.0) * 0.8, 1.5);
            label(content, state, po2.position + NODE + po2.size * Vec3::new(0.5, 1.0, 0.0), tr_with("demos.po2_cube", lab::args!(side = format!("{:.1}", po2.size.x))), palette::OBB);
        }
        if step > 1.0 {
            let s = w.probe.octree_size;
            label(content, state, b.position + NODE + b.size * Vec3::new(0.5, 0.0, 1.0), tr_with("demos.cells_per_axis", lab::args!(x = s.x, y = s.y, z = s.z, size = format!("{:.0}", 100.0 / w.probe.to_cell_scale))), palette::AABB);
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
    let lit = if view == 4 || view == 5 { Some(world::lit_for(&settings)) } else { None };
    let omni = if view == 5 && state.p("extraLight") > 0.5 { Some(world::direct_with_omni(omni_position(state))) } else { None };
    let half = 0.5 * (1 << (depth(probe) - level)) as f32 / probe.to_cell_scale;
    // Cells wider than 12 cm are drawn as wire boxes, so the coarse levels do not hide the room.
    let boxes = 2.0 * half > 0.12;
    let mut points = Vec::new();
    for cell in probe.level_range(level) {
        let d = probe.data[cell];
        let p = cell_center(probe, level, d.pos());
        if p.y > cut {
            continue;
        }
        let color = match view {
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
            let (min, size) = cell_box(probe, level, d.pos());
            let c = if view == 0 { alpha_color(d.albedo().w) } else { color };
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
    label(content, state, p + Vec3::Y * (r + 0.05), tr("demos.worked_label").into(), palette::SELECTED);
}

// ---------------------------------------------------------------------------
// Chapter 1: the room and the settings.

fn room_demo(state: &ShotState, content: &mut FrameContent) {
    let rows = vec![
        row(tr("demos.voxelgi_shown"), format!("{:.2}", state.p("voxelgiMix")), palette::text()),
        row(tr("demos.renderer"), tr("demos.forward_plus"), palette::THEME_ACCENT),
        row("size", "20 x 20 x 20 m", palette::AABB),
        row(tr("demos.lab_box"), "6.4 x 3.2 x 6.4 m", palette::AABB),
        row("subdiv", "128", palette::AABB),
        row("dynamic_range", "2.0", palette::INCIDENT),
        row("energy", "1.0", palette::INCIDENT),
        row("bias / normal_bias", "1.5 / 0.0", palette::VIEW),
        row("propagation", "0.5", palette::INCIDENT),
        row("use_two_bounces", "true", palette::INCIDENT),
        row("interior", "false", palette::VIEW),
        row("voxel_gi/quality", tr("demos.quality_low"), palette::VIEW),
        row("gi/use_half_resolution", "false", palette::VIEW),
    ];
    content.readout = Some(readout(tr("demos.settings"), tr("demos.settings_subtitle"), lit_rows(state, rows)));
}

// ---------------------------------------------------------------------------
// Chapter 2: the bounds, the octree, the leaves.

fn bounds_demo(state: &ShotState, w: &World, content: &mut FrameContent) {
    let v = &w.voxelizer;
    let s = v.octree_size();
    let rows = vec![
        row("size", format!("{:.1} x {:.1} x {:.1} m", world::SIZE.x, world::SIZE.y, world::SIZE.z), palette::AABB),
        row(tr("demos.longest_axis"), format!("x: 1 << {} = {}", v.cell_subdiv, s[0]), palette::AABB),
        row(tr("demos.shorter_axes"), format!("y {} (3.2 m), z {}", s[1], s[2]), palette::AABB),
        row(tr("demos.po2_bounds"), format!("{:.1} m cube", v.po2_bounds.size.x), palette::OBB),
        row("cell_size", format!("{:.0} cm", v.cell_size * 100.0), palette::AABB),
        row(tr("demos.levels"), format!("{}", v.cell_subdiv + 1), palette::AABB),
        row(tr("demos.dense_cells"), format!("{}", s[0] * s[1] * s[2]), palette::text()),
        row(tr("demos.bake_time"), format!("{:.2} s", w.bake_seconds), palette::text_dim()),
    ];
    content.readout = Some(readout(tr("demos.bounds"), tr("demos.bounds_subtitle"), lit_rows(state, rows)));
}

fn octree_demo(state: &ShotState, w: &World, content: &mut FrameContent) {
    let probe = &w.probe;
    let level = state.p("octreeLevel").round() as i32;
    let mut rows = Vec::new();
    for (l, &c) in probe.level_counts.iter().enumerate() {
        let color = if l as i32 == level { palette::SELECTED } else { palette::AABB };
        rows.push(row(tr_with("demos.level_n", lab::args!(level = l)), format!("{c}"), color));
    }
    // The worked triangle's descent.
    if state.p("workedTriangle") > 0.5 {
        let tri = worked_triangle(w);
        let pts: Vec<Vec3> = [0, 1, 2, 0].iter().map(|&k| tri.vtx[k] + NODE).collect();
        content.draw.polyline(&pts, palette::SELECTED, 1.0, 3.0);
        // The level being entered in full, the one above it fading: the
        // children the triangle overlaps lit, the ones it misses dim.
        let shown = state.p("descentLevel");
        for d in descent(w) {
            let age = shown - d.level as f32;
            let a = if age < 0.0 { (1.0 + age).max(0.0) } else if age < 1.0 { 1.0 } else if age < 2.0 { 0.3 } else { 0.0 };
            if a <= 0.0 {
                continue;
            }
            if d.overlaps {
                wire_box(content, d.aabb.position + NODE, d.aabb.size, palette::SELECTED, a, 2.0);
            } else {
                wire_box(content, d.aabb.position + NODE, d.aabb.size, palette::text_dim(), a * 0.4, 1.0);
            }
        }
        let deepest = descent(w).iter().filter(|d| d.overlaps && d.level as f32 <= shown).map(|d| d.level).max().unwrap_or(0);
        let count = descent(w).iter().filter(|d| d.overlaps && d.level == deepest).count();
        label(content, state, tri.vtx[0] + NODE + Vec3::Y * 0.06, tr_with("demos.descent", lab::args!(level = deepest, count = count)), palette::SELECTED);
        // The separating-axis test on the first child it skipped at the deepest level drawn.
        let step = state.p("satStep").round() as i32;
        if step > 0
            && let Some(sk) = descent(w).iter().find(|d| !d.overlaps && d.level == deepest.max(1))
        {
            let stage = bake::triangle_box_stage(sk.aabb.center(), sk.aabb.size * 0.5, &tri.vtx);
            wire_box(content, sk.aabb.position + NODE, sk.aabb.size, palette::MISMATCH, 1.0, 2.0);
            let text = match stage {
                bake::SatStage::EdgeAxis(k) => tr_with("demos.sat_edge", lab::args!(k = k + 1)),
                bake::SatStage::BoxAxis(a) => tr_with("demos.sat_box", lab::args!(axis = ["x", "y", "z"][a])),
                bake::SatStage::Plane => tr("demos.sat_plane").to_string(),
                bake::SatStage::Overlap => tr("demos.sat_overlap").to_string(),
            };
            rows.push(row(tr("demos.sat"), text, palette::MISMATCH));
        }
    }
    // The worked leaf's columns.
    if state.p("workedLeaf") > 0.5 {
        let (aabb, cell) = worked_leaf(w);
        let tri = worked_triangle(w);
        wire_box(content, aabb.position + NODE, aabb.size, palette::SELECTED, 1.0, 2.0);
        let s = bake::leaf_sample(&tri, Vec3::ONE, Vec3::ZERO, aabb);
        let n = state.p("leafColumns").round().clamp(0.0, 16.0) as usize;
        let mut hits = 0;
        for c in s.columns.iter().take(n) {
            let color = if c.hit.is_some() { palette::VIEW } else { palette::text_dim() };
            wire_box(content, c.from + NODE, c.to - c.from, color, if c.hit.is_some() { 0.9 } else { 0.3 }, 1.0);
            if let Some(h) = c.hit {
                ball(content, h + NODE, 0.003, palette::VIEW, 1.0);
                hits += 1;
            }
        }
        rows.push(row(tr("demos.leaf"), format!("({}, {}, {})", cell.x, cell.y, cell.z), palette::SELECTED));
        rows.push(row(tr("demos.dominant_axis"), ["x", "y", "z"][s.closest_axis].to_string(), palette::SELECTED));
        rows.push(row(tr("demos.columns_hit"), format!("{hits} / {n}"), palette::VIEW));
        rows.push(row("alpha", format!("{:.4}", hits as f32 / 16.0), palette::VIEW));
    }
    content.readout = Some(readout(tr("demos.octree"), tr("demos.octree_subtitle"), lit_rows(state, rows)));
}

// ---------------------------------------------------------------------------
// Chapter 3: the cells' records, the sort, the field, the upload.

/// The cells demonstration's worked cell: the bake's worked leaf, or the picked one.
pub fn cells_cell(state: &ShotState, w: &World) -> Option<(usize, IVec3)> {
    nearest_leaf(&w.probe, picked_cell(state, worked_leaf(w).1))
}

fn cells_demo(state: &ShotState, w: &World, content: &mut FrameContent) {
    let probe = &w.probe;
    let Some((cell, pos)) = cells_cell(state, w) else { return };
    let level = depth(probe);
    let (min, size) = cell_box(probe, level, pos);
    wire_box(content, min, size, palette::SELECTED, 1.0, 2.5);
    content.points.push(("cells.worked".into(), min + size * 0.5));
    mark(content, state, min + size * 0.5, 0.06);
    let d = probe.data[cell];
    // Its normal: the samples' sum before the fixup, the stored normal after.
    let n = if state.p("fixupStep") < 0.5 { super::bake::normalized(Vec3::from(w.sums[cell].normal)) } else { d.normal() };
    content.draw.segment(min + size * 0.5, min + size * 0.5 + n * 0.12, palette::NORMAL, 1.0, 3.0, lab::frame::FLAG_ARROW);
    let mut rows = vec![row(tr("demos.cell_index"), tr_with("demos.cell_index_value", lab::args!(index = cell, level = level)), palette::text())];
    if state.p("fixupStep") < 0.5 {
        let s = &w.sums[cell];
        rows.push(row(tr("demos.albedo_sum"), v3(Vec3::from(s.albedo)), palette::AABB));
        rows.push(row(tr("demos.normal_sum"), v3(Vec3::from(s.normal)), palette::NORMAL));
        rows.push(row(tr("demos.alpha_sum"), format!("{:.4}", s.alpha), palette::VIEW));
    } else {
        let children = probe.octree[cell];
        let none = children.iter().all(|&c| c == CHILD_EMPTY);
        rows.push(row(tr("demos.children"), if none { tr("demos.children_none").to_string() } else { children.iter().map(|c| if *c == CHILD_EMPTY { "-".to_string() } else { c.to_string() }).collect::<Vec<_>>().join(" ") }, palette::text_dim()));
        let word = |name: &str, offset: u32, value: u32| (format!("{name} +{offset}"), format!("0x{value:08X}"));
        let decoded = |v: &[f32]| v.iter().map(|x| format!("{x:.2}")).collect::<Vec<_>>().join(" ");
        let (n, v) = word(tr("demos.position_word"), 0, d.position);
        rows.push(row(n, v, palette::AABB));
        rows.push(row(tr("demos.decoded_xyz"), format!("{} {} {}", pos.x, pos.y, pos.z), palette::AABB));
        let (n, v) = word(tr("demos.albedo_word"), 4, d.albedo);
        rows.push(row(n, v, palette::AABB));
        rows.push(row(tr("demos.decoded_rgba"), decoded(&d.albedo().to_array()), palette::AABB));
        let (n, v) = word(tr("demos.emission_word"), 8, d.emission);
        rows.push(row(n, v, palette::INCIDENT));
        rows.push(row(tr("demos.decoded_rgb"), decoded(&d.emission().to_array()), palette::INCIDENT));
        let (n, v) = word(tr("demos.normal_word"), 12, d.normal);
        rows.push(row(n, v, palette::NORMAL));
        rows.push(row(tr("demos.decoded_xyz"), decoded(&d.normal().to_array()), palette::NORMAL));
        let t = w.lit.texture.texel(0, pos);
        rows.push(row(tr("demos.texel"), decoded(&t.to_array()), palette::INCIDENT));
        rows.push(row(tr("demos.texel_light"), decoded(&(t.truncate() * world::DYNAMIC_RANGE).to_array()), palette::INCIDENT));
        rows.push(row(tr("demos.sdf_byte"), format!("{}", probe.sdf_at(pos)), palette::OUTSIDE));
    }
    content.readout = Some(readout(tr("demos.cell_records"), tr("demos.cell_records_subtitle"), lit_rows(state, rows)));
    if state.p("cellArray") > 0.5 || state.shows_image("cellArray") {
        let inset = cell_array_inset(probe, cell);
        if state.shows_image("cellArray") {
            content.images.push(("cellArray".into(), inset.clone()));
        }
        if state.p("cellArray") > 0.5 {
            content.insets.push(inset);
        }
    }
    slices(state, w, content, Some(pos));
}

const LEVEL_COLORS: [Color32; 8] = [palette::AXIS_X, palette::AXIS_Y, palette::AXIS_Z, palette::OBB, palette::REFLECTED, palette::PROBE, palette::VIEW, palette::AABB];

/// The sorted cell array as a bar: each level's run in its colour (widths
/// as the counts, every level at least a few pixels), the worked cell marked.
fn cell_array_inset(probe: &Probe, cell: usize) -> PictureInset {
    let (wpx, hpx) = (440usize, 40usize);
    let total: f32 = probe.level_counts.iter().map(|&c| (c as f32).max(1.0).ln_1p()).sum();
    let mut edges = vec![0usize];
    let mut acc = 0.0;
    for &c in &probe.level_counts {
        acc += (c as f32).max(1.0).ln_1p();
        edges.push((acc / total * wpx as f32) as usize);
    }
    let mut rgba = vec![0u8; wpx * hpx * 4];
    for x in 0..wpx {
        let l = edges.windows(2).position(|e| x >= e[0] && x < e[1]).unwrap_or(7);
        let c = LEVEL_COLORS[l % 8];
        for y in 0..hpx {
            let i = (y * wpx + x) * 4;
            rgba[i..i + 4].copy_from_slice(&[c.r(), c.g(), c.b(), 255]);
        }
    }
    let level = probe.level_of(cell);
    let within = (cell - probe.level_offset(level)) as f32 / probe.level_counts[level].max(1) as f32;
    let at = (edges[level] as f32 + within * (edges[level + 1] - edges[level]) as f32) / wpx as f32;
    let mut inset = PictureInset::new(tr("demos.cell_array"), Picture::Pixels { key: "cell-array".into(), size: [wpx, hpx], rgba: Arc::new(rgba) }, [440.0, 40.0]);
    inset.corner = Corner::BottomLeft;
    inset.marker = Some(Marker { at: [at, 0.5], color: palette::SELECTED, label: String::new() });
    inset.x_axis = tr("demos.cell_array_axis").into();
    inset.lines = probe.level_counts.iter().enumerate().map(|(l, c)| (format!("{l}: {c}"), LEVEL_COLORS[l % 8])).collect();
    inset
}

/// The distance transform's squared distances after `passes` (cached).
fn edt_passes(w: &World, passes: usize) -> &'static [f32] {
    static P: OnceLock<Vec<Vec<f32>>> = OnceLock::new();
    &P.get_or_init(|| (0..=3).map(|n| sdf::squared_distances(&w.voxelizer, n)).collect())[passes]
}

/// Slice insets: the field or the light texture's mip at `sliceHeight`.
fn slices(state: &ShotState, w: &World, content: &mut FrameContent, mark: Option<IVec3>) {
    let slice = state.p("slice").round() as i32;
    let want_field = slice == 1 || state.shows_image("fieldSlice");
    let want_mip = slice == 2 || state.shows_image("mipSlice");
    let probe = &w.probe;
    let y_leaf = leaf_of(probe, Vec3::new(0.0, state.p("sliceHeight"), 0.0)).y.clamp(0, probe.octree_size.y - 1);
    let s = probe.octree_size;
    if want_field {
        let passes = state.p("edtPass").round().clamp(0.0, 3.0) as usize;
        let sq = edt_passes(w, passes);
        let max = 16.0f32;
        let inset = slice_inset(tr_with("demos.field_slice", lab::args!(passes = passes)), format!("sdf-{passes}-{y_leaf}"), s.x as usize, s.z as usize, |x, z| {
            let v = sq[(x + y_leaf * s.x + z * s.x * s.y) as usize];
            if v == 0.0 {
                return [10, 10, 10, 255];
            }
            if v >= sdf::BIG_VAL * 0.5 {
                return [235, 235, 240, 255];
            }
            // Grey by the distance, a darker band every two cells.
            let d = v.sqrt();
            let band = if (d / 2.0).floor() as i32 % 2 == 0 { 1.0 } else { 0.82 };
            let g = ((70.0 + 170.0 * (d / max).min(1.0)) * band) as u8;
            [g, g, g, 255]
        }, mark.map(|m| [m.x, m.z]), tr("demos.field_legend").into());
        if state.shows_image("fieldSlice") {
            content.images.push(("fieldSlice".into(), inset.clone()));
        }
        if slice == 1 {
            content.insets.push(inset);
        }
    }
    if want_mip {
        let settings = light_settings(state);
        let lit = world::lit_for(&settings);
        let mip = state.p("mipLevel").round().clamp(0.0, (probe.mip_count() - 1) as f32) as usize;
        let ms = probe.mip_size(mip);
        let y = (y_leaf >> mip).min(ms.y - 1);
        let tex = &lit.texture;
        let inset = slice_inset(tr_with("demos.mip_slice", lab::args!(mip = mip, x = ms.x, z = ms.z)), format!("mip-{mip}-{y}-{}-{}-{}", settings.dynamic_range, settings.propagation, settings.two_bounces), ms.x as usize, ms.z as usize, |x, z| {
            let t = tex.texel(mip, IVec3::new(x, y, z));
            if t.w == 0.0 {
                let c = if (x + z) % 2 == 0 { 22 } else { 34 };
                return [c, c, c + 6, 255];
            }
            let c = light_color(t.truncate() * settings.dynamic_range);
            [c.r(), c.g(), c.b(), 255]
        }, mark.map(|m| [m.x >> mip, m.z >> mip]), tr("demos.mip_legend").into());
        if state.shows_image("mipSlice") {
            content.images.push(("mipSlice".into(), inset.clone()));
        }
        if slice == 2 {
            content.insets.push(inset);
        }
    }
}

/// An x-z slice of `nx` x `nz` texels, each a block of pixels, +z at the bottom.
#[allow(clippy::too_many_arguments)]
fn slice_inset(title: String, key: String, nx: usize, nz: usize, pixel: impl Fn(i32, i32) -> [u8; 4], mark: Option<[i32; 2]>, legend: String) -> PictureInset {
    let k = (256 / nx.max(nz)).max(1);
    let (wpx, hpx) = (nx * k, nz * k);
    let mut rgba = vec![0u8; wpx * hpx * 4];
    for z in 0..nz {
        for x in 0..nx {
            let c = pixel(x as i32, z as i32);
            for dy in 0..k {
                for dx in 0..k {
                    let i = ((z * k + dy) * wpx + x * k + dx) * 4;
                    rgba[i..i + 4].copy_from_slice(&c);
                }
            }
        }
    }
    let marker = mark.map(|m| Marker { at: [(m[0] as f32 + 0.5) / nx as f32, (m[1] as f32 + 0.5) / nz as f32], color: palette::SELECTED, label: String::new() });
    let mut inset = PictureInset::new(title, Picture::Pixels { key, size: [wpx, hpx], rgba: Arc::new(rgba) }, [240.0, 240.0 * hpx as f32 / wpx as f32]);
    inset.corner = Corner::TopRight;
    inset.x_axis = "x".into();
    inset.y_axis = "z".into();
    inset.marker = marker;
    inset.lines = vec![(legend, palette::text_dim())];
    inset
}

fn field_demo(state: &ShotState, w: &World, content: &mut FrameContent) {
    let probe = &w.probe;
    let fallback = leaf_of(probe, Vec3::new(-0.35, state.p("sliceHeight"), -0.1));
    let p = picked_cell(state, fallback);
    let passes = state.p("edtPass").round().clamp(0.0, 3.0) as usize;
    let sq = edt_passes(w, passes)[(p.x + p.y * probe.octree_size.x + p.z * probe.octree_size.x * probe.octree_size.y) as usize];
    let (min, size) = cell_box(probe, depth(probe), p);
    wire_box(content, min, size, palette::SELECTED, 1.0, 2.0);
    content.points.push(("field.worked".into(), min + size * 0.5));
    // The slice's plane, outlined in the room.
    let y = state.p("sliceHeight");
    let b = world::bounds();
    let c0 = Vec3::new(b.position.x + NODE.x, y, b.position.z + NODE.z);
    let pts = [c0, c0 + Vec3::X * b.size.x, c0 + Vec3::new(b.size.x, 0.0, b.size.z), c0 + Vec3::Z * b.size.z, c0];
    content.draw.polyline(&pts, palette::OUTSIDE, 0.7, 1.5);
    let byte = sdf::encode(sq);
    let rows = vec![
        row(tr("demos.worked_cell"), format!("({}, {}, {})", p.x, p.y, p.z), palette::SELECTED),
        row(tr("demos.passes"), format!("{passes} / 3"), palette::OUTSIDE),
        row(tr("demos.squared"), if sq >= sdf::BIG_VAL * 0.5 { "1e20".to_string() } else { format!("{sq:.0}") }, palette::OUTSIDE),
        row(tr("demos.distance_cells"), if sq >= sdf::BIG_VAL * 0.5 { "-".to_string() } else { format!("{:.2}", sq.sqrt()) }, palette::OUTSIDE),
        row(tr("demos.byte"), format!("{byte}"), palette::OUTSIDE),
        row(tr("demos.advance"), format!("{} {}", byte as i32 - 1, tr("demos.cells")), palette::OUTSIDE),
    ];
    content.readout = Some(readout(tr("demos.field"), tr("demos.field_subtitle"), lit_rows(state, rows)));
    slices(state, w, content, Some(p));
}

// ---------------------------------------------------------------------------
// Chapter 4: the light pass.

fn light_demo(state: &ShotState, w: &World, content: &mut FrameContent) {
    let probe = &w.probe;
    let lights = shot_lights(state, w);
    let params = world::params(probe, &light_settings(state));
    // The panel's rectangle.
    let l = &w.scene.lighting.light;
    let pts: Vec<Vec3> = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0)].iter().map(|&(a, b)| l.center + l.axis_u * a + l.axis_v * b - Vec3::Y * 0.01).collect();
    content.draw.polyline(&pts, palette::INCIDENT, 0.9, 2.0);
    if state.p("extraLight") > 0.5 {
        let o = omni_position(state);
        ball(content, o, 0.05, palette::INCIDENT, 1.0);
        label(content, state, o + Vec3::Y * 0.1, tr("demos.omni").into(), palette::INCIDENT);
    }
    if state.p("workedLight") < 0.5 {
        // How many leaves the write pass clips at this dynamic range (light / range above 1 in a channel).
        let settings = light_settings(state);
        let lit = world::lit_for(&settings);
        let leaves = probe.level_range(probe.leaf_level());
        let clipped = lit.outputs[leaves.clone()].iter().filter(|v| v.max_element() > settings.dynamic_range).count();
        let brightest = lit.outputs[leaves].iter().map(|v| v.max_element()).fold(0.0f32, f32::max);
        let rows = vec![
            row(tr("demos.lights"), format!("{}", lights.len()), palette::INCIDENT),
            row(tr("demos.max_lights"), format!("{}", light::MAX_LIGHTS), palette::INCIDENT),
            row("dynamic_range", format!("{:.2}", settings.dynamic_range), palette::INCIDENT),
            row(tr("demos.brightest_leaf"), format!("{brightest:.2}"), palette::INCIDENT),
            row(tr("demos.leaves_clipped"), format!("{clipped}"), palette::SELECTED),
            row(tr("demos.light_seconds"), format!("{:.2} s", w.light_seconds), palette::text_dim()),
        ];
        content.readout = Some(readout(tr("demos.light_pass"), tr("demos.light_pass_subtitle"), lit_rows(state, rows)));
        return;
    }
    let Some((cell, pos)) = nearest_leaf(probe, picked_cell(state, leaf_of(probe, LIGHT_POINT))) else { return };
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
        content.draw.segment(center, center + normal * 0.3, palette::NORMAL, 1.0, 2.5, 0);
    }
    let mut rows = vec![row(tr("demos.worked_cell"), format!("({}, {}, {})", pos.x, pos.y, pos.z), palette::SELECTED), row(tr("demos.normal_word"), v3(normal), palette::NORMAL)];
    let mut total = Vec3::ZERO;
    for (k, lt) in lights.iter().enumerate() {
        let name = if lt.kind == light::LIGHT_TYPE_AREA { tr("demos.panel") } else { tr("demos.omni") };
        if lt.kind == light::LIGHT_TYPE_AREA {
            let a = light::area_light(&params, lt, cpos, normal);
            let v = a.map(|x| x * albedo).unwrap_or(Vec3::ZERO);
            total += v;
            if step >= 1 {
                content.draw.segment(center, l.center, palette::INCIDENT, 0.6, 1.5, 0);
            }
            if step >= 4 {
                rows.push(row(tr("demos.panel_no_shadow"), v3(v), palette::INCIDENT));
            }
            continue;
        }
        let mut trace = LightTrace::default();
        let res = light::light_at_pos(probe, &params, lt, cpos, normal, Some(&mut trace));
        let lp = probe.from_cell(lt.position) + NODE;
        if step >= 1 {
            content.draw.segment(center, lp, palette::INCIDENT, 0.9, 2.0, 0);
        }
        if step >= 2 {
            rows.push(row(tr("demos.attenuation"), format!("{:.3}", trace.attenuation), palette::INCIDENT));
        }
        if step >= 3 {
            let cos = normal.dot(-trace.light_dir).max(0.0);
            rows.push(row(tr("demos.cosine"), format!("{cos:.3}"), palette::NORMAL));
        }
        // The shadow march: a circle per step, of the distance the field gave.
        let shown = state.p("marchSteps").round() as usize;
        for (i, s) in trace.steps.iter().take(shown).enumerate() {
            let p = probe.from_cell(s.from) + NODE;
            let r = (s.advance.max(0.0) + 0.5) / probe.to_cell_scale;
            let color = if s.advance < 0.0 { palette::MISMATCH } else { palette::OUTSIDE };
            if r <= MAX_CIRCLE {
                circle(content, p, r, color, 0.8);
            }
            ball(content, p, 0.012, color, 1.0);
            if i + 1 == shown.min(trace.steps.len()) {
                content.points.push(("light.march".into(), p));
            }
        }
        if !trace.steps.is_empty() {
            rows.push(row(tr("demos.march"), tr_with("demos.march_value", lab::args!(steps = trace.steps.len(), occlusion = format!("{:.2}", trace.occlusion))), palette::OUTSIDE));
        }
        let v = res.map(|(li, dir)| if normal.length() > 0.2 { normal.dot(-dir).max(0.0) * li * albedo } else { li * albedo }).unwrap_or(Vec3::ZERO);
        total += v;
        if step >= 4 {
            rows.push(row(tr_with("demos.light_term", lab::args!(name = name)), v3(v), palette::INCIDENT));
        }
        let _ = k;
    }
    if step >= 4 {
        rows.push(row(tr("demos.cell_light"), v3(total + d.emission()), palette::SELECTED));
        let dr = state.p("dynamicRange");
        let stored = (total / dr).extend(d.albedo().w);
        rows.push(row(tr_with("demos.stored", lab::args!(range = format!("{dr:.1}"))), v4(super::storage::unorm8(stored).map(|b| b as f32 / 255.0).into()), palette::SELECTED));
    }
    content.readout = Some(readout(tr("demos.light_pass"), tr("demos.light_pass_subtitle"), lit_rows(state, rows)));
}

// ---------------------------------------------------------------------------
// Chapter 5: the second bounce and the mips.

/// The readout's rows for a cone's last drawn step: its distance, width, and mip, in cells.
fn step_rows(rows: &mut Vec<(String, String, Color32)>, s: &ConeStep) {
    rows.push(row(tr("demos.step_distance"), format!("{:.1}", s.dist), palette::VIEW));
    rows.push(row(tr("demos.step_diameter"), format!("{:.1}", s.diameter), palette::VIEW));
    rows.push(row(tr("demos.step_mip"), format!("{:.2}", s.lod), palette::VIEW));
}

/// A cone drawn as its axis and a circle per sample, coloured by what it read.
fn draw_cone(content: &mut FrameContent, probe: &Probe, start: Vec3, dir_cells: Vec3, steps: &[ConeStep], shown: usize, dynamic_range: f32, alpha: f32) {
    let end_dist = steps.last().map_or(2.0, |s| s.dist + s.diameter * 0.5);
    let shown_steps = &steps[..shown.min(steps.len())];
    let to_world = |dist: f32| probe.from_cell(start + dir_cells * dist) + NODE;
    let reach = shown_steps.last().map_or(0.0, |s| s.dist).min(end_dist);
    content.draw.segment(to_world(0.0), to_world(reach.max(0.5)), palette::VIEW, alpha, 2.0, 0);
    for s in shown_steps {
        let p = to_world(s.dist);
        let r = s.diameter * 0.5 / probe.to_cell_scale;
        let c = if s.sample.w > 0.0 { light_color(s.sample.truncate() / s.sample.w.max(1e-3) * dynamic_range) } else { palette::text_dim() };
        if r <= MAX_CIRCLE {
            circle(content, p, r, c, alpha);
        } else {
            ball(content, p, 0.02, c, alpha);
        }
    }
}

fn bounce_demo(state: &ShotState, w: &World, content: &mut FrameContent) {
    let probe = &w.probe;
    let settings = light_settings(state);
    let lit = world::lit_for(&settings);
    if state.p("workedBounce") < 0.5 {
        let rows = vec![row(tr("demos.passes_run"), format!("{}", lit.passes), palette::INCIDENT)];
        content.readout = Some(readout(tr("demos.bounce"), tr("demos.bounce_subtitle"), lit_rows(state, rows)));
        slices(state, w, content, None);
        return;
    }
    let Some((cell, pos)) = nearest_leaf(probe, picked_cell(state, leaf_of(probe, BOUNCE_POINT))) else { return };
    let d = probe.data[cell];
    let (min, size) = cell_box(probe, depth(probe), pos);
    wire_box(content, min, size, palette::SELECTED, 1.0, 2.5);
    content.points.push(("bounce.cell".into(), min + size * 0.5));
    mark(content, state, min + size * 0.5, 0.08);
    let cpos = pos.as_vec3() + Vec3::splat(0.5);
    let normal = d.normal();
    let albedo = d.albedo();
    let params = world::params(probe, &settings);
    let mut rows = vec![row(tr("demos.worked_cell"), format!("({}, {}, {})", pos.x, pos.y, pos.z), palette::SELECTED), row(tr("demos.direct"), v3(lit.direct[cell]), palette::INCIDENT)];
    let shown = state.p("conesShown").round() as usize;
    let steps_shown = state.p("coneSteps").round() as usize;
    let mut bounce = Vec3::ZERO;
    if normal.length() > 0.2 {
        let [t, b, n] = light::tangent_frame(normal);
        for (i, dir) in light::BOUNCE_CONE_DIRS.iter().enumerate() {
            let direction = t * dir.x + b * dir.y + n * dir.z;
            let mut steps = Vec::new();
            let c = light::bounce_cone(&lit.first_texture, &params, cpos, direction, Some(&mut steps));
            let add = (c * light::BOUNCE_CONE_WEIGHTS[i] * Vec4::new(albedo.x, albedo.y, albedo.z, 1.0) * params.dynamic_range).truncate();
            bounce += add;
            if i < shown {
                draw_cone(content, probe, cpos, direction, &steps, steps_shown, params.dynamic_range, 0.9);
                if i + 1 == shown {
                    rows.push(row(tr_with("demos.cone_n", lab::args!(n = i + 1)), tr_with("demos.cone_value", lab::args!(steps = steps.len(), alpha = format!("{:.2}", c.w), weight = light::BOUNCE_CONE_WEIGHTS[i])), palette::VIEW));
                    if let Some(s) = steps.get(steps_shown.min(steps.len()).saturating_sub(1)) {
                        step_rows(&mut rows, s);
                    }
                }
            }
        }
    }
    rows.push(row(tr("demos.bounce_sum"), v3(bounce), palette::VIEW));
    rows.push(row(tr("demos.after_bounce"), v3(lit.outputs[cell]), palette::SELECTED));
    content.readout = Some(readout(tr("demos.bounce"), tr("demos.bounce_subtitle"), lit_rows(state, rows)));
    slices(state, w, content, Some(pos));
}

fn mips_demo(state: &ShotState, w: &World, content: &mut FrameContent) {
    let probe = &w.probe;
    let settings = light_settings(state);
    let lit = world::lit_for(&settings);
    let params = world::params(probe, &settings);
    let leaf = picked_cell(state, leaf_of(probe, BOUNCE_POINT));
    let mut rows = vec![row("propagation", format!("{:.2}", settings.propagation), palette::INCIDENT), row("dynamic_range", format!("{:.1}", settings.dynamic_range), palette::INCIDENT)];
    if state.p("workedParent") > 0.5 {
        let plevel = depth(probe) - 1;
        if let Some(parent) = probe.find_cell(leaf, plevel) {
            let pd = probe.data[parent];
            let (min, size) = cell_box(probe, plevel, pd.pos());
            wire_box(content, min, size, palette::SELECTED, 1.0, 2.5);
            content.points.push(("mips.parent".into(), min + size * 0.5));
            for &ch in &probe.octree[parent] {
                if ch == CHILD_EMPTY {
                    continue;
                }
                let cd = probe.data[ch as usize];
                let (cmin, csize) = cell_box(probe, plevel + 1, cd.pos());
                ball(content, cmin + csize * 0.5, csize.x * 0.4, light_color(lit.outputs[ch as usize]), 1.0);
            }
            let (value, count) = light::mip_cell(probe, &params, &lit.outputs, parent);
            let divisor = 8.0 + (count as f32 - 8.0) * params.propagation;
            rows.push(row(tr("demos.children_count"), format!("{count} / 8"), palette::AABB));
            rows.push(row(tr("demos.divisor"), format!("mix(8, {count}, {:.2}) = {divisor:.2}", params.propagation), palette::INCIDENT));
            rows.push(row(tr("demos.parent_light"), v3(value), palette::SELECTED));
            rows.push(row(tr("demos.parent_alpha"), format!("{:.3}", pd.albedo().w), palette::VIEW));
        }
    }
    content.readout = Some(readout(tr("demos.mips"), tr("demos.mips_subtitle"), lit_rows(state, rows)));
    slices(state, w, content, Some(leaf));
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
    let mut rows = vec![row(tr("demos.dynamic_box"), format!("{} x {} x {}", size.x, size.y, size.z), palette::SELECTED)];
    // The oversampled box, in the world.
    let m = super::dynamic::MULTIPLIER as f32;
    let bmin = probe.from_cell(plot.begin.as_vec3() / m) + NODE;
    let bmax = probe.from_cell(plot.end.as_vec3() / m) + NODE;
    wire_box(content, bmin, bmax - bmin, palette::SELECTED, 0.8, 1.5);
    if state.p("dynamicVoxels") > 0.5 {
        let mip = state.p("dynamicMip").round().clamp(0.0, (probe.mip_count() - 1) as f32) as usize;
        let level = probe.mip_count() - 1 - mip;
        let half = 0.5 * (1 << mip) as f32 / probe.to_cell_scale;
        // Drawn four times brighter than stored (the room's direct light is dim in 8 bits).
        let points: Vec<(Vec3, f32, Color32)> = plot.plotted[mip].iter().filter_map(|&p| {
            let t = plot.texture.texel(mip, p);
            (t.w > 0.0).then(|| (cell_center(probe, level, p), half * 0.85, light_color(t.truncate() * world::DYNAMIC_RANGE * DYNAMIC_GAIN)))
        }).collect();
        if 2.0 * half > 0.12 {
            for (p, h, c) in &points {
                wire_box(content, *p - Vec3::splat(*h / 0.85), Vec3::splat(2.0 * *h / 0.85), *c, 0.9, 1.5);
            }
        } else {
            balls(content, &points, 1.0);
        }
        rows.push(row(tr("demos.dynamic_gain"), format!("x {DYNAMIC_GAIN}"), palette::text_dim()));
        rows.push(row(tr_with("demos.plotted_mip", lab::args!(mip = mip)), format!("{}", plot.plotted[mip].len()), palette::SELECTED));
    }
    let view = state.p("dynamicView").round() as i32;
    if (0..6).contains(&view) {
        let v = &plot.views[view as usize];
        let level = state.p("shrinkLevel").round().clamp(0.0, (v.maps.len() - 1) as f32) as usize;
        let map = &v.maps[level];
        let dir = ["+x", "+y", "+z", "-x", "-y", "-z"][view as usize];
        rows.push(row(tr("demos.view"), format!("{dir}: {} x {}", v.maps[0].rect_size.x, v.maps[0].rect_size.y), palette::SELECTED));
        rows.push(row(tr("demos.shrink_map"), tr_with("demos.shrink_map_value", lab::args!(level = level, w = map.rect_size.x, h = map.rect_size.y, mip = map.mipmap)), palette::SELECTED));
        // The first map's albedo, and the chosen map's light.
        let first = &v.maps[0];
        let (aw, ah) = (first.rect_size.x as usize, first.rect_size.y as usize);
        let albedo: Vec<u8> = (0..ah).rev().flat_map(|y| (0..aw).map(move |x| (y, x))).flat_map(|(y, x)| {
            let a = v.albedo[y * aw + x];
            if a.w > 0.0 { [(a.x * 255.0) as u8, (a.y * 255.0) as u8, (a.z * 255.0) as u8, 255] } else { [16, 16, 20, 255] }
        }).collect();
        let mut ins = PictureInset::new(tr_with("demos.view_albedo", lab::args!(dir = dir)), Picture::Pixels { key: format!("dyn-albedo-{dx}-{view}"), size: [aw, ah], rgba: Arc::new(albedo) }, [320.0, 320.0 * ah as f32 / aw.max(1) as f32]);
        ins.corner = Corner::TopLeft;
        content.insets.push(ins);
        let (lw, lh) = (map.rect_size.x as usize, map.rect_size.y as usize);
        let light: Vec<u8> = (0..lh).rev().flat_map(|y| (0..lw).map(move |x| (y, x))).flat_map(|(y, x)| {
            let l = map.light[y * lw + x];
            if l.w > 0.0 {
                let c = light_color(l.truncate() / l.w.max(1e-3) * DYNAMIC_GAIN);
                [c.r(), c.g(), c.b(), 255]
            } else {
                [16, 16, 20, 255]
            }
        }).collect();
        let mut ins = PictureInset::new(tr_with("demos.view_light", lab::args!(level = level)), Picture::Pixels { key: format!("dyn-light-{dx}-{view}-{level}"), size: [lw.max(1), lh.max(1)], rgba: Arc::new(light) }, [320.0, 320.0 * lh as f32 / lw.max(1) as f32]);
        ins.corner = Corner::TopRight;
        content.insets.push(ins);
    }
    let maps = super::dynamic::OVERSAMPLED + probe.mip_count();
    rows.push(row(tr("demos.dispatches"), tr_with("demos.dispatches_value", lab::args!(views = 6, per = maps)), palette::text_dim()));
    content.readout = Some(readout(tr("demos.dynamic"), tr("demos.dynamic_subtitle"), lit_rows(state, rows)));
}

// ---------------------------------------------------------------------------
// Chapter 7: a pixel's cones.

/// The worked pixel's cones, as `process` traces them.
pub fn pixel_cones(state: &ShotState, w: &World, eye_pos: Vec3) -> (Vec3, Vec3, Cones, Vec4, Vec4) {
    let (p, n) = pixel_point(state, &w.scene);
    let v = (eye_pos - p).normalize();
    let r = lab::scene::reflect(-v, n);
    let inst = world::instance(&w.probe, &LightSettings::default(), state.p("interior") > 0.5);
    let mut cones = Cones::default();
    let (amb, spec) = cone::process(&inst, &w.lit.texture, p, n, r, state.p("sphereRoughness"), state.p("quality") > 0.5, Some(&mut cones));
    (p, n, cones, amb, spec)
}

fn pixel_demo(state: &ShotState, w: &World, content: &mut FrameContent) {
    let probe = &w.probe;
    let e = eye(content);
    let (p, n, cones, amb, spec) = pixel_cones(state, w, e);
    let inst = world::instance(probe, &LightSettings::default(), false);
    let mut rows = vec![row(tr("demos.tag"), tr("demos.tag_value"), palette::text_dim())];
    if state.p("workedPixel") > 0.5 {
        ball(content, p, 0.02, palette::SELECTED, 1.0);
        content.points.push(("pixel.point".into(), p));
        mark(content, state, p, 0.06);
        content.draw.segment(p, p + n * 0.25, palette::NORMAL, 1.0, 2.0, lab::frame::FLAG_ARROW);
        let shown = state.p("conesShown").round() as usize;
        let steps = state.p("coneSteps").round() as usize;
        for (i, (dir, _w, st, _c)) in cones.diffuse.iter().enumerate().take(shown) {
            draw_cone_world(content, &inst, cones.position, *dir, st, steps, 0.9);
            let _ = i;
        }
        if state.p("specCone") > 0.5
            && let Some((dir, _tan, st, _c)) = &cones.specular
        {
            draw_cone_world(content, &inst, cones.position, *dir, st, steps, 1.0);
        }
        rows.push(row(tr("demos.cell_position"), v3(cones.position), palette::VIEW));
        rows.push(row(tr("demos.edge_blend"), format!("{:.3}", cones.blend), palette::VIEW));
        let last = shown.min(cones.diffuse.len());
        if let Some((_, _, st, c)) = cones.diffuse.get(last.saturating_sub(1)) {
            rows.push(row(tr_with("demos.cone_n", lab::args!(n = last.max(1))), tr_with("demos.cone_alpha", lab::args!(steps = st.len(), alpha = format!("{:.2}", c.w))), palette::VIEW));
            if let Some(s) = st.get(steps.min(st.len()).saturating_sub(1)) {
                step_rows(&mut rows, s);
            }
        }
        if let Some((_, tan, st, _)) = &cones.specular {
            rows.push(row(tr("demos.spec_cone"), tr_with("demos.spec_value", lab::args!(angle = format!("{:.1}", tan.atan().to_degrees()), steps = st.len())), palette::REFLECTED));
        }
    }
    rows.push(row(tr("demos.ambient"), v4(amb), palette::INCIDENT));
    rows.push(row(tr("demos.reflection"), v4(spec), palette::REFLECTED));
    content.readout = Some(readout(tr("demos.pixel"), tr("demos.pixel_subtitle"), lit_rows(state, rows)));
}

/// A cone of the screen pass (positions in the instance's cells).
fn draw_cone_world(content: &mut FrameContent, inst: &cone::Instance, start: Vec3, dir: Vec3, steps: &[ConeStep], shown: usize, alpha: f32) {
    let to_world = |dist: f32| inst.from_cell(start + dir * dist);
    let shown_steps = &steps[..shown.min(steps.len())];
    let reach = shown_steps.last().map_or(1.0, |s| s.dist);
    content.draw.segment(to_world(0.0), to_world(reach), palette::VIEW, alpha, 2.0, 0);
    for s in shown_steps {
        let r = s.diameter * 0.5 / inst.to_cell_scale;
        let c = if s.sample.w > 0.0 { light_color(s.sample.truncate() / s.sample.w.max(1e-3) * inst.dynamic_range) } else { palette::text_dim() };
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
    let env_amb = lab::scene::environment_radiance(&lighting, n);
    let env_ref = sky_reflection(&lighting, r, state.p("sphereRoughness"));
    let mix = state.p("voxelgiMix");
    let ambient = env_amb.lerp(amb.truncate(), amb.w * mix);
    let reflection = env_ref.lerp(spec.truncate(), spec.w * mix);
    let s = w.scene.records()[ids::SPHERE];
    let albedo = Vec3::new(s.albedo_roughness[0], s.albedo_roughness[1], s.albedo_roughness[2]);
    let nv = n.dot(v).max(1e-4);
    let env = lab::scene::env_brdf_approx(0.04, state.p("sphereRoughness"), nv);
    if state.p("workedPixel") > 0.5 {
        ball(content, p, 0.02, palette::SELECTED, 1.0);
        content.points.push(("pixel.point".into(), p));
    }
    let rows = vec![
        row(tr("demos.env_ambient"), v3(env_amb), palette::text_dim()),
        row(tr("demos.voxelgi_ambient"), v4(amb), palette::INCIDENT),
        row(tr("demos.mixed_ambient"), v3(ambient), palette::INCIDENT),
        row(tr("demos.times_albedo"), v3(ambient * albedo), palette::AABB),
        row(tr("demos.env_reflection"), v3(env_ref), palette::text_dim()),
        row(tr("demos.voxelgi_reflection"), v4(spec), palette::REFLECTED),
        row(tr("demos.mixed_reflection"), v3(reflection), palette::REFLECTED),
        row(tr("demos.times_brdf"), format!("{} (x {env:.3})", v3(reflection * env)), palette::REFLECTED),
    ];
    content.readout = Some(readout(tr("demos.compose"), tr("demos.compose_subtitle"), lit_rows(state, rows)));
}

/// voxelgiSkyReflection (shaders/voxelgi_shading.wgsl): the lab's sky
/// averaged over nine directions spread by the roughness.
pub fn sky_reflection(lighting: &lab::scene::Lighting, r: Vec3, roughness: f32) -> Vec3 {
    let v0 = if r.y.abs() > 0.999 { Vec3::X } else { Vec3::Y };
    let t = v0.cross(r).normalize();
    let b = r.cross(t);
    let mut sum = Vec3::ZERO;
    for i in -1..=1 {
        for j in -1..=1 {
            sum += lab::scene::environment_radiance(lighting, (r + (t * i as f32 + b * j as f32) * roughness).normalize());
        }
    }
    sum / 9.0
}

/// The scene shading's derived values (gShading's `{@ derived}` members).
pub fn shader_data(block: &mut lab::shader_data::Block, _state: &ShotState) {
    let w = world::world();
    let inst = world::instance(&w.probe, &LightSettings::default(), false);
    block.set("toCellOrigin", inst.to_cell_origin.to_array());
    block.set("toCellScale", inst.to_cell_scale);
    block.set("bounds", inst.bounds.to_array());
    block.set("dynamicRange", inst.dynamic_range);
    block.set("bias", inst.bias);
    block.set("normalBias", inst.normal_bias);
    block.set("mipCount", w.probe.mip_count() as f32);
    block.set("sdfOffset", world::texture_words(&w.probe) as f32);
}
