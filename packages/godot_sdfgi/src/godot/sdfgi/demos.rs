//! What each demonstration of the SDFGI investigation shows: the room shaded
//! with the WGSL screen pass, and the structures of the CPU model (model.rs,
//! voxels.rs, light.rs, probes.rs, screen.rs) drawn over it. Everything a
//! frame shows is computed from the shot's values and the converged world
//! (sim::converged), which is built once.

use super::light;
use super::model::{self, Cascades, Dirty, Settings, CASCADE_SIZE, PROBE_AXIS_COUNT};
use super::probes::{self, SH_SIZE};
use super::shot::Demo;
use super::sim::{self, World};
use super::voxels::{self, index, inside, CELLS};
use crate::godot::sdfgi::text::{tr, tr_with};
use egui::Color32;
use glam::{IVec3, Vec3};
use lab::frame::{FrameContent, Label, Readout, Statics};
use lab::utility::inset::{Corner, Marker, Picture, PictureInset};
use lab::layout::FrameUniforms;
use lab::palette;
use lab::scene::{ids, Scene};
use lab::shot::ShotState;
use std::sync::{Arc, OnceLock};

/// Where the lab creates SDFGI, and where its camera stays: the room's
/// centre, at standing height.
pub const ORIGIN: Vec3 = Vec3::new(0.0, 1.6, 0.0);

/// The probe the probe demonstrations follow: cascade 0's probe at
/// (-1.6, 1.07, 0.0) m, between the sphere and the red wall.
pub const FOLLOWED_PROBE: IVec3 = IVec3::new(7, 7, 8);

/// The cascades for a shot: created at `ORIGIN`, then updated once per
/// 0.2 m step of the walk to `cameraX`, as frames would while the camera
/// walks there, with the shot's y scale.
pub fn cascades(state: &ShotState) -> Cascades {
    let walk = state.p("cameraX");
    let steps = if state.p("cameraJump") > 0.5 { 1 } else { (walk.abs() / 0.2).ceil() as usize };
    let path: Vec<Vec3> = (1..=steps).map(|k| ORIGIN + Vec3::X * walk * (k as f32 / steps as f32)).collect();
    let settings = Settings { y_scale_mode: state.p("yScale").round().clamp(0.0, 2.0) as usize, ..Settings::default() };
    model::follow(settings, ORIGIN, &path)
}

fn wire_box(content: &mut FrameContent, min: Vec3, size: Vec3, color: Color32, alpha: f32, width: f32) {
    let c = |i: u32| min + size * Vec3::new((i & 1) as f32, ((i >> 1) & 1) as f32, ((i >> 2) & 1) as f32);
    for (a, b) in [(0, 1), (2, 3), (4, 5), (6, 7), (0, 2), (1, 3), (4, 6), (5, 7), (0, 4), (1, 5), (2, 6), (3, 7)] {
        content.draw.segment(c(a), c(b), color, alpha, width, 0);
    }
}

/// The investigation's far plane: the lab's camera stops at 60 m, and the
/// cascades reach 205 m, so its frames draw out to 1 km.
pub const FAR_PLANE: f32 = 1000.0;

/// `camera` with its projection rebuilt for `FAR_PLANE` (the lab's reversed
/// depth, far then near, as lab/src/camera.rs builds it).
fn far_camera(mut camera: lab::layout::CameraData) -> lab::layout::CameraData {
    let aspect = camera.viewport_size[0] / camera.viewport_size[1].max(1.0);
    let proj = glam::camera::rh::proj::directx::perspective(camera.vertical_fov, aspect, FAR_PLANE, camera.near_plane);
    let view = glam::Mat4::from_cols_array_2d(&camera.world_to_view);
    camera.view_to_clip = proj.to_cols_array_2d();
    camera.world_to_clip = (proj * view).to_cols_array_2d();
    camera.far_plane = FAR_PLANE;
    camera
}

/// The size in pixels of `r` metres at `p`, for the frame's camera (points
/// are drawn as discs of a radius in pixels).
fn px(content: &FrameContent, p: Vec3, r: f32) -> f32 {
    let c = &content.uniforms.camera;
    let d = (p - Vec3::from(c.position)).length().max(1e-3);
    r / (d * (c.vertical_fov * 0.5).tan()) * c.viewport_size[1] * 0.5
}

/// A point of `r` metres.
fn ball(content: &mut FrameContent, p: Vec3, r: f32, color: Color32, alpha: f32) {
    let radius = px(content, p, r).max(1.5);
    content.draw.point(p, radius, color, alpha);
}

/// A circle of `r` metres about `p`, facing the camera (a march's sphere).
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

const CASCADE_COLORS: [Color32; 4] = [palette::AXIS_Y, palette::AABB, palette::OBB, palette::REFLECTED];

/// A cell of cascade 0 of the converged world, in real metres (its centre).
pub fn cell_world(world: &World, cascade: usize, p: IVec3) -> Vec3 {
    let field = world.field();
    let s = (p.as_vec3() + Vec3::splat(0.5)) / field.to_cell(cascade) + field.offset(cascade);
    Vec3::new(s.x, s.y / world.cascades.settings.y_mult(), s.z)
}

/// The cell of cascade 0 a real-space point falls in.
pub fn world_cell(world: &World, cascade: usize, p: Vec3) -> IVec3 {
    let field = world.field();
    let s = Vec3::new(p.x, p.y * world.cascades.settings.y_mult(), p.z);
    ((s - field.offset(cascade)) * field.to_cell(cascade)).floor().as_ivec3()
}

fn objects_for(state: &ShotState, statics: &Statics) -> Vec<lab::layout::ObjectData> {
    let mut objects = statics.scene.records();
    objects[ids::SPHERE].albedo_roughness[3] = state.p("sphereRoughness");
    objects
}

pub fn build_frame(state: &ShotState, statics: &Statics, viewport: [f32; 2], pixel_scale: f32, time: f32) -> FrameContent {
    let demo = Demo::of(state.demo);
    let objects = objects_for(state, statics);
    let lighting = statics.scene.lighting;
    let camera = far_camera(state.camera.camera_data(viewport));
    let uniforms = FrameUniforms {
        camera,
        light_center_radiance: lighting.light.center.extend(lighting.light.radiance).to_array(),
        light_axis_u: lighting.light.axis_u.extend(0.0).to_array(),
        light_axis_v: lighting.light.axis_v.extend(0.0).to_array(),
        light_color: lighting.light.color.extend(1.0).to_array(),
        ambient_color: lighting.ambient.extend(1.0).to_array(),
        floor_params: [objects[ids::FLOOR].albedo_roughness[3], lighting.f0, 0.0, 1.0],
        overlay_params: [lighting.exposure, pixel_scale, time, 0.0],
        // The panel would hide the room from the top camera of the slices.
        counts: [objects.len() as u32, ids::FLOOR as u32, 0, if matches!(demo, Demo::Field | Demo::Occlusion | Demo::Cascades | Demo::Regions) { 1 << ids::PANEL } else { 0 }],
        view_params: [1.0, 0.0, 0.0, 0.0],
    };
    let mut content = FrameContent::new(objects, uniforms);
    let world = sim::converged();
    content.shading_frame = sim::frame_cascades(&world.cascades, ORIGIN);
    match demo {
        Demo::Room => room(state, &mut content),
        Demo::Cascades => cascades_demo(state, &mut content),
        Demo::Regions => regions_demo(state, &mut content),
        Demo::Voxels => voxels_demo(state, world, &mut content),
        Demo::Field => field_demo(state, world, &mut content),
        Demo::Occlusion => occlusion_demo(state, world, &mut content),
        Demo::Light => light_demo(state, &statics.scene, world, &mut content),
        Demo::Probes | Demo::Maps => probes_demo(state, world, &mut content, demo == Demo::Maps),
        Demo::Pixel => pixel_demo(state, world, &mut content),
    }
    shared_overlays(state, world, &mut content);
    content
}

/// What any demonstration can turn on: the cascades' boxes and cascade 0's probes.
fn shared_overlays(state: &ShotState, world: &World, content: &mut FrameContent) {
    let cascades = cascades(state);
    let shown = state.p("cascadesShown").clamp(0.0, cascades.cascades.len() as f32);
    for (i, color) in CASCADE_COLORS.iter().enumerate().take(cascades.cascades.len()) {
        let alpha = (shown - i as f32).clamp(0.0, 1.0);
        if alpha <= 0.0 {
            continue;
        }
        let (min, size) = cascades.bounds(i);
        wire_box(content, min, size, *color, alpha, 2.0);
        let width = CASCADE_SIZE as f32 * cascades.cascades[i].cell_size;
        label(content, state, min + size * Vec3::new(0.5, 1.0, 0.0), tr_with("demos.cascade_width", lab::args!(index = i, width = format!("{width:.1}"))), *color);
    }
    if state.p("probes") > 0.5 {
        for p in 0..probes::PROBES {
            let probe = probes::probe_of(p);
            let pos = cascades.probe_position(0, probe);
            let in_room = Scene::inside_room(pos, 0.0);
            let followed = probe == FOLLOWED_PROBE && cascades.cascades[0].position == world.cascades.cascades[0].position;
            let color = if followed { palette::PROBE_SELECTED } else { palette::PROBE };
            ball(content, pos, if in_room { 0.06 } else { 0.035 }, color, if in_room { 1.0 } else { 0.3 });
        }
    }
}

/// Chapter 1: the room, SDFGI's settings and their defaults.
fn room(state: &ShotState, content: &mut FrameContent) {
    let settings = Settings::default();
    content.readout = Some(Readout {
        title: tr("demos.settings").into(),
        subtitle: tr("demos.settings_subtitle").into(),
        formula: None,
        formula_lit: None,
        rows: {
            let mut rows = vec![
                row(tr("demos.sdfgi_shown"), format!("{:.2}", state.p("sdfgiMix")), palette::text()),
                row("cascades", settings.cascades.to_string(), palette::THEME_ACCENT),
                row("min_cell_size", format!("{} m", settings.min_cell_size), palette::THEME_ACCENT),
                row("y_scale", "75 % (y_mult 1.5)", palette::THEME_ACCENT),
                row("ray_count", tr("demos.ray_count_value"), palette::INCIDENT),
                row(tr("demos.ray_labels"), "8, 16, 32, 64, 96, 128", palette::INCIDENT),
                row(tr("demos.ray_table"), "4, 8, 16, 32, 64, 96, 128", palette::INCIDENT),
                row("converge", tr("demos.converge_value"), palette::INCIDENT),
                row("update_lights", tr("demos.update_lights_value"), palette::INCIDENT),
                row("depth_prepass", tr("demos.prepass_value"), palette::VIEW),
            ];
            let focus = state.p("focusRow").round() as i32;
            if focus >= 0 {
                for (i, r) in rows.iter_mut().enumerate() {
                    if i as i32 != focus {
                        r.2 = palette::text_faint();
                    }
                }
            }
            rows
        },
    });
}

/// Chapter 2: the cascades around the camera, its walk, and the scroll.
fn cascades_demo(state: &ShotState, content: &mut FrameContent) {
    let cascades = cascades(state);
    let walk = state.p("cameraX");
    let camera = ORIGIN + Vec3::X * walk;
    ball(content, camera, 0.15, palette::VIEW, 1.0);
    label(content, state, camera + Vec3::Y * 0.3, tr("demos.sdfgi_camera").into(), palette::VIEW);
    let c0 = cascades.cascades[0];
    let y_mult = cascades.settings.y_mult();
    let cell = (Vec3::new(camera.x, camera.y * y_mult, camera.z) / c0.cell_size).as_ivec3();
    let dirty = match c0.dirty {
        Dirty::All => "DIRTY_ALL".to_string(),
        Dirty::Regions(d) => format!("({}, {}, {})", d.x, d.y, d.z),
    };
    // The dirty slab of cascade 0, when the last step moved it.
    if state.p("regions") > 0.5 {
        for r in cascades.pending_regions().iter().filter(|r| r.cascade == 0) {
            wire_box(content, r.bounds_min, r.bounds_size, palette::INSIDE, 1.0, 3.0);
        }
    }
    // A top view of one row of cells along x: the camera's cell and the cascade's centre.
    if state.p("scrollView") > 0.5 {
        let y = 0.02;
        let from = c0.position.x - 20;
        for k in 0..=40 {
            let x = (from + k) as f32 * c0.cell_size;
            let strong = (from + k - c0.position.x).abs() <= 4;
            content.draw.segment(Vec3::new(x, y, -0.3), Vec3::new(x, y, 0.3), if strong { palette::THEME_ACCENT } else { palette::BRICK_EMPTY }, 1.0, if strong { 2.0 } else { 1.0 }, 0);
        }
        let centre = c0.position.x as f32 * c0.cell_size;
        content.draw.segment(Vec3::new(centre, y, -0.6), Vec3::new(centre, y, 0.6), palette::AXIS_Y, 1.0, 4.0, 0);
        label(content, state, Vec3::new(centre, 0.1, 0.7), tr("demos.cascade_centre").into(), palette::AXIS_Y);
        let cam = cell.x as f32 * c0.cell_size + c0.cell_size * 0.5;
        ball(content, Vec3::new(cam, y, 0.0), 0.08, palette::VIEW, 1.0);
    }
    let (min, size) = cascades.bounds(0);
    content.readout = Some(Readout {
        title: tr("demos.cascade_zero").into(),
        subtitle: tr("demos.cascade_zero_subtitle").into(),
        formula: None,
        formula_lit: None,
        rows: vec![
            row(tr("demos.camera_x"), format!("{walk:.2} m"), palette::VIEW),
            row(tr("demos.camera_cell"), format!("int({:.2} / 0.2) = {}", camera.x, cell.x), palette::VIEW),
            row(tr("demos.cascade_position"), format!("{} {}", c0.position.x, tr("demos.cells")), palette::AXIS_Y),
            row("drag_margin", tr("demos.drag_margin_value"), palette::THEME_ACCENT),
            row("dirty_regions", dirty, palette::INSIDE),
            row(tr("demos.box"), format!("{:.2} .. {:.2} m", min.x, min.x + size.x), palette::AXIS_Y),
            row(tr("demos.height"), format!("{:.2} m (y_mult {y_mult})", size.y), palette::AXIS_Y),
        ],
    });
}

/// Chapter 3: the regions of the walk, the geometry inside them, and the light lists.
fn regions_demo(state: &ShotState, content: &mut FrameContent) {
    let cascades = cascades(state);
    let regions = cascades.pending_regions();
    for r in &regions {
        wire_box(content, r.bounds_min, r.bounds_size, palette::INSIDE, 1.0, 4.0);
        label(content, state, r.bounds_min + r.bounds_size * Vec3::new(0.5, 1.0, 0.5), tr_with("demos.region_label", lab::args!(cascade = r.cascade)), palette::INSIDE);
    }
    let static_light = sim::omni(Vec3::new(state.p("lightX"), state.p("lightHeight"), state.p("lightZ")), 15.0, 1.0, cascades.settings.y_mult());
    let lights = if state.p("lightBake") > 0.5 { vec![static_light] } else { Vec::new() };
    let cull = sim::cull_static(&cascades, &lights);
    let given = sim::static_pass_lights(&cascades, &cull, &lights);
    if state.p("extraLight") > 0.5 {
        ball(content, Vec3::new(state.p("lightX"), state.p("lightHeight"), state.p("lightZ")), 0.1, palette::INCIDENT, 1.0);
    }
    let mut rows = vec![row(tr("demos.regions"), regions.len().to_string(), palette::INSIDE)];
    for r in regions.iter().take(4) {
        rows.push(row(format!("{} {}", tr("demos.cascade"), r.cascade), format!("{:?} + {:?} {}", r.local_offset.to_array(), r.local_size.to_array(), tr("demos.cells")), CASCADE_COLORS[r.cascade.min(3)]));
    }
    rows.push(row("cascade_light_index", format!("{:?}", cull.cascade_light_index), palette::INCIDENT));
    rows.push(row(tr("demos.stored_under"), format!("{:?}", (0..8).filter(|&c| !cull.lists[c].is_empty()).collect::<Vec<_>>()), palette::INCIDENT));
    rows.push(row(tr("demos.static_pass_gets"), format!("{:?}", given.iter().map(|(c, l)| (*c, l.len())).collect::<Vec<_>>()), palette::INCIDENT));
    content.readout = Some(Readout { title: tr("demos.culling").into(), subtitle: tr("demos.culling_subtitle").into(), formula: None, formula_lit: None, rows });
}

/// The solid voxel the voxel and field readouts describe: the top of the sphere.
fn worked_voxel(world: &World) -> IVec3 {
    let top = Vec3::new(-1.05, 1.24, -0.55);
    let target = world_cell(world, 0, top);
    world.data[0].stored.cells.iter().map(|c| c.cell()).min_by_key(|c| (*c - target).length_squared()).unwrap_or(target)
}

/// Chapter 4: the solid voxels of cascade 0 and one voxel's bits.
fn voxels_demo(state: &ShotState, world: &World, content: &mut FrameContent) {
    let data = &world.data[0];
    let passes = state.p("voxelPasses");
    let cut = state.p("cutHeight");
    if state.p("voxels") > 0.5 {
        let voxels = voxel_passes(passes.round().clamp(0.0, 3.0) as usize);
        let size = world.cascades.cascades[0].cell_size * 0.45;
        for c in &data.stored.cells {
            let p = c.cell();
            if voxels.albedo[index(p)] & 1 == 0 {
                continue;
            }
            let w = cell_world(world, 0, p);
            if w.y > cut {
                continue;
            }
            let color = if state.p("voxelFacing") > 0.5 {
                // One colour per axis, the +/- faces in two shades: the bits a voxel's fragments set.
                const FACE: [Color32; 6] = [palette::AXIS_X, palette::AXIS_Y, palette::AXIS_Z, Color32::from_rgb(140, 40, 40), Color32::from_rgb(40, 120, 50), Color32::from_rgb(40, 70, 150)];
                let f = c.facing();
                let (mut r, mut g, mut b, mut n) = (0u32, 0u32, 0u32, 0u32);
                for (k, col) in FACE.iter().enumerate() {
                    if f & (1 << k) != 0 {
                        r += col.r() as u32;
                        g += col.g() as u32;
                        b += col.b() as u32;
                        n += 1;
                    }
                }
                let n = n.max(1);
                Color32::from_rgb((r / n) as u8, (g / n) as u8, (b / n) as u8)
            } else {
                let a = c.albedo_rgb();
                Color32::from_rgb((a.x.sqrt() * 255.0) as u8, (a.y.sqrt() * 255.0) as u8, (a.z.sqrt() * 255.0) as u8)
            };
            ball(content, w, size, color, 1.0);
        }
    }
    let p = worked_voxel(world);
    let w = cell_world(world, 0, p);
    ball(content, w, 0.08, palette::SELECTED, 1.0);
    for (k, dir) in voxels::ANISO_DIR.iter().enumerate() {
        let on = data.voxels.facing[index(p)] & (1 << k) != 0;
        content.draw.segment(w, w + *dir * 0.25, if on { palette::NORMAL } else { palette::BRICK_EMPTY }, if on { 1.0 } else { 0.4 }, if on { 3.0 } else { 1.0 }, lab::frame::FLAG_ARROW);
    }
    let albedo = data.voxels.albedo[index(p)];
    let facing = data.voxels.facing[index(p)];
    content.readout = Some(Readout {
        title: tr("demos.voxel").into(),
        subtitle: tr_with("demos.voxel_subtitle", lab::args!(x = p.x, y = p.y, z = p.z)),
        formula: None,
        formula_lit: None,
        rows: vec![
            row(tr("demos.albedo_word"), format!("{albedo:#06x} (R16_UINT)"), palette::text()),
            row(tr("demos.solid_bit"), format!("bit 0 = {}", albedo & 1), palette::INSIDE),
            row("R (bits 11-15)", format!("{} / 31", (albedo >> 11) & 31), Color32::from_rgb(255, 120, 120)),
            row("G (bits 6-10)", format!("{} / 31", (albedo >> 6) & 31), Color32::from_rgb(120, 230, 120)),
            row("B (bits 1-5)", format!("{} / 31", (albedo >> 1) & 31), Color32::from_rgb(130, 160, 255)),
            row(tr("demos.facing_bits"), format!("{:06b}", facing.reverse_bits() >> 26), palette::NORMAL),
            row(tr("demos.emission"), format!("{:#010x}", data.voxels.emission[index(p)]), palette::text_dim()),
            row(tr("demos.passes_run"), format!("{}", passes.round()), palette::THEME_ACCENT),
        ],
    });
}

/// Cascade 0's render images after `passes` of the three views (cached).
fn voxel_passes(passes: usize) -> &'static voxels::RenderVoxels {
    static PASSES: OnceLock<Vec<voxels::RenderVoxels>> = OnceLock::new();
    &PASSES.get_or_init(|| {
        let scene = Scene::study_room();
        let objects = sim::voxelized_objects(&scene.records());
        let cascades = model::Cascades::create(Settings::default(), ORIGIN);
        let region = cascades.pending_regions()[0];
        (0..=3).map(|n| {
            let mut v = voxels::RenderVoxels::cleared();
            voxels::voxelize_axes(&objects, &region, &mut v, n);
            v
        }).collect()
    })[passes]
}

/// A horizontal slice of cascade 0 around the room, as an inset: 48 x 48 cells.
const SLICE_CELLS: i32 = 34;

fn slice_inset(state: &ShotState, world: &World, title: String, key: String, pixel: impl Fn(IVec3) -> [u8; 4], marker: Option<IVec3>, lines: Vec<(String, Color32)>) -> PictureInset {
    let min = world_cell(world, 0, Vec3::new(-SLICE_CELLS as f32 * 0.1, 0.0, -SLICE_CELLS as f32 * 0.1));
    slice_window(state, world, title, key, pixel, marker, lines, min, SLICE_CELLS)
}

/// A slice inset of `cells` x `cells` from cell `min` (x and z).
#[allow(clippy::too_many_arguments)]
fn slice_window(state: &ShotState, world: &World, title: String, key: String, pixel: impl Fn(IVec3) -> [u8; 4], marker: Option<IVec3>, lines: Vec<(String, Color32)>, min: IVec3, cells: i32) -> PictureInset {
    let y = world_cell(world, 0, Vec3::new(0.0, state.p("sliceHeight"), 0.0)).y;
    let n = cells as usize;
    // Each cell drawn as a block of k x k pixels, so the inset's filtering keeps cells sharp.
    let k = (220 / n).max(1);
    let size = n * k;
    let mut rgba = vec![0u8; size * size * 4];
    for row in 0..n {
        for col in 0..n {
            // Rows from the top: +z at the bottom, as the top camera sees the room.
            let p = IVec3::new(min.x + col as i32, y, min.z + row as i32);
            let c = if inside(p) { pixel(p) } else { [0, 0, 0, 255] };
            for dy in 0..k {
                for dx in 0..k {
                    let i = ((row * k + dy) * size + col * k + dx) * 4;
                    rgba[i..i + 4].copy_from_slice(&c);
                }
            }
        }
    }
    let marker = marker.map(|m| ([((m.x - min.x) as f32 + 0.5) / n as f32, 1.0 - ((m.z - min.z) as f32 + 0.5) / n as f32], palette::SELECTED));
    picture(Corner::TopRight, title, format!("{key}-{y}"), size, rgba, "x".into(), "z".into(), marker, lines)
}

/// A square picture of `size` pixels in an inset, 220 points a side, with its
/// axes and a marked point.
#[allow(clippy::too_many_arguments)]
fn picture(corner: Corner, title: String, key: String, size: usize, rgba: Vec<u8>, x_axis: String, y_axis: String, marker: Option<([f32; 2], Color32)>, lines: Vec<(String, Color32)>) -> PictureInset {
    let marker = marker.map(|(at, color)| Marker { at, color, label: String::new() });
    PictureInset { corner, x_axis, y_axis, marker, lines, ..PictureInset::new(title, Picture::Pixels { key, size: [size, size], rgba: Arc::new(rgba) }, [220.0, 220.0]) }
}

fn grey(v: f32) -> [u8; 4] {
    let g = (v.clamp(0.0, 1.0) * 255.0) as u8;
    [g, g, g, 255]
}

/// Chapter 5: the distance field and the jump flood, step by step.
fn field_demo(state: &ShotState, world: &World, content: &mut FrameContent) {
    let data = &world.data[0];
    let step = state.p("floodStep").round().clamp(0.0, 8.0) as usize;
    let worked = world_cell(world, 0, Vec3::new(-0.2, state.p("sliceHeight"), -0.55));
    let seeds_at = |p: IVec3| -> [u8; 4] {
        match step {
            0 => data.flood.half_init[half_index(p)],
            1..=6 => data.flood.half_passes[step - 1].2[half_index(p)],
            7 => data.flood.upscaled[index(p)],
            _ => data.flood.seeds[index(p)],
        }
    };
    // The slices: an inset while `slice` picks one, an image while the story shows it (super::IMAGES).
    let slice = state.p("slice").round() as i32;
    for (n, key) in [(1, "distanceField"), (2, "floodSeeds")] {
        let (inset_on, image_on) = (slice == n, state.shows_image(key));
        if !inset_on && !image_on {
            continue;
        }
        let title = if n == 1 { tr("demos.sdf_slice").to_string() } else { tr_with("demos.seed_slice", lab::args!(step = step)) };
        let inset = slice_inset(state, world, title, format!("sdf-{n}-{step}"), |p| {
            if n == 1 {
                grey(data.stored.sdf[index(p)] as f32 / 24.0)
            } else {
                let s = seeds_at(p);
                let changed = step > 0 && {
                    let prev = match step - 1 {
                        0 => data.flood.half_init[half_index(p)],
                        k @ 1..=6 => data.flood.half_passes[k - 1].2[half_index(p)],
                        7 => data.flood.upscaled[index(p)],
                        _ => s,
                    };
                    prev != s
                };
                if s[3] == 0 {
                    [0, 0, 0, 255]
                } else if changed {
                    // A cell this step gave a nearer seed.
                    [255, 120, 60, 255]
                } else {
                    // The seed's own colour, so cells sharing a seed share a colour.
                    [60 + (s[0] as u32 * 37 % 160) as u8, 60 + (s[2] as u32 * 53 % 160) as u8, 90 + (s[1] as u32 * 29 % 140) as u8, 255]
                }
            }
        }, Some(worked), vec![]);
        if image_on {
            content.images.push((key.to_string(), inset.clone()));
        }
        if inset_on {
            content.insets.push(inset);
        }
    }
    let seed = seeds_at(worked);
    let byte = data.stored.sdf[index(worked)];
    let seed_cell = IVec3::new(seed[0] as i32, seed[1] as i32, seed[2] as i32);
    let w = cell_world(world, 0, worked);
    ball(content, w, 0.06, palette::SELECTED, 1.0);
    // The worked cell and its seed, for a story's arrows and labels (super::ANNOTATIONS).
    content.points.push(("field.worked".into(), w));
    if seed[3] != 0 {
        let s = cell_world(world, 0, seed_cell);
        content.points.push(("field.seed".into(), s));
        content.draw.segment(w, s, palette::OUTSIDE, 1.0, 2.0, lab::frame::FLAG_ARROW);
        ball(content, s, 0.05, palette::INSIDE, 1.0);
    }
    // A sphere trace across the slice: from the worked cell along +x, each step the stored distance.
    let mut march_steps = 0;
    if state.p("fieldMarch") > 0.5 {
        let field = world.field();
        let y_mult = world.cascades.settings.y_mult();
        let start = (worked.as_vec3() + Vec3::splat(0.5)) / field.to_cell(0) + field.offset(0);
        let pos = (start - field.offset(0)) * field.to_cell(0);
        let dir = Vec3::X;
        let mut advance = 0.0;
        for _ in 0..40 {
            let d = light::sample_sdf(&data.stored.sdf, (pos + dir * advance) / CASCADE_SIZE as f32) * 255.0 - 1.0;
            let at = (pos + dir * advance) / field.to_cell(0) + field.offset(0);
            let real = Vec3::new(at.x, at.y / y_mult, at.z);
            circle(content, real, (d.max(0.0) / field.to_cell(0)).max(0.02), palette::OUTSIDE, 0.9);
            ball(content, real, 0.03, palette::OUTSIDE, 1.0);
            march_steps += 1;
            if d < 0.05 {
                break;
            }
            advance += d;
        }
    }
    let schedule = voxels::half_schedule();
    let step_text = match step {
        0 => tr("demos.flood_init").to_string(),
        1..=6 => format!("{} {} ({})", tr("demos.flood_step_size"), schedule[step - 1].0, if schedule[step - 1].1 { tr("demos.optimized") } else { tr("demos.regular") }),
        7 => tr("demos.flood_upscale").to_string(),
        _ => tr("demos.flood_final").to_string(),
    };
    content.readout = Some(Readout {
        title: tr("demos.field").into(),
        subtitle: step_text,
        formula: None,
        formula_lit: None,
        rows: vec![
            row(tr("demos.cell"), format!("{:?}", worked.to_array()), palette::SELECTED),
            row(tr("demos.seed"), if seed[3] == 0 { tr("demos.unused").to_string() } else { format!("{:?}", [seed[0], seed[1], seed[2]]) }, palette::INSIDE),
            row(tr("demos.distance_cells"), format!("{:.2}", (seed_cell - worked).as_vec3().length()), palette::OUTSIDE),
            row(tr("demos.stored_byte"), format!("{byte} = 1 + d"), palette::OUTSIDE),
            row(tr("demos.marches_read"), format!("{byte} - 1 = {}", byte as i32 - 1), palette::text()),
            row(tr("demos.solid_cells"), format!("{} / {}", data.stored.cells.len(), CELLS / 4), palette::INSIDE),
            row(tr("demos.march_steps"), march_steps.to_string(), palette::OUTSIDE),
        ],
    });
}

fn half_index(p: IVec3) -> usize {
    let h: IVec3 = p >> IVec3::ONE;
    (h.x + h.y * 64 + h.z * 64 * 64) as usize
}

/// Chapter 6: one probe's occlusion around it.
fn occlusion_demo(state: &ShotState, world: &World, content: &mut FrameContent) {
    let data = &world.data[0];
    let probe_cell = FOLLOWED_PROBE * 8;
    let world_probe = world.cascades.cascades[0].position / 8 + FOLLOWED_PROBE;
    let bits = world_probe & IVec3::ONE;
    let class = (bits.x | (bits.y << 1) | (bits.z << 2)) as usize;
    let shell = state.p("occlusionShell");
    let inset = slice_window(state, world, tr("demos.occlusion_slice").into(), format!("occ-{}", shell.round()), |p| {
        let d = p - probe_cell;
        let local = IVec3::new(if d.x >= 0 { d.x } else { -d.x - 1 }, if d.y >= 0 { d.y } else { -d.y - 1 }, if d.z >= 0 { d.z } else { -d.z - 1 });
        if local.max_element() >= 8 {
            return [20, 20, 30, 255];
        }
        if (local.x + local.y + local.z) as f32 > shell {
            return [40, 40, 60, 255];
        }
        if data.voxels.solid(p) {
            return [200, 90, 40, 255];
        }
        grey(super::occlusion::class_value(data.occlusion[index(p)], class))
    }, Some(probe_cell), vec![], probe_cell - IVec3::new(10, 0, 10), 20);
    content.insets.push(inset);
    ball(content, cell_world(world, 0, probe_cell), 0.1, palette::PROBE_SELECTED, 1.0);
    let packed = data.occlusion[index(probe_cell + IVec3::new(2, -2, 1))];
    content.readout = Some(Readout {
        title: tr("demos.occlusion").into(),
        subtitle: tr("demos.occlusion_subtitle").into(),
        formula: None,
        formula_lit: None,
        rows: (0..8).map(|i| row(format!("{} {i}", tr("demos.class")), format!("{} / 15", (packed >> super::occlusion::OCCLUSION_SHIFT[i]) & 0xF), if i == class { palette::PROBE_SELECTED } else { palette::text_dim() })).chain([row("use_occlusion", tr("demos.occlusion_off"), palette::text())]).collect(),
    });
}

/// The F1 run: a static omni light, then three scrolls of cascade 0; the
/// stored static light of a kept floor cell after each (cached).
fn static_scrolls() -> &'static Vec<f32> {
    static RUN: OnceLock<Vec<f32>> = OnceLock::new();
    RUN.get_or_init(|| {
        let settings = Settings::default();
        let objects = sim::voxelized_objects(&Scene::study_room().records());
        let light = sim::omni(Vec3::new(1.5, 1.2, 1.0), 15.0, 1.0, settings.y_mult());
        let mut world = World::new(&objects, settings, ORIGIN);
        world.static_frame(&[light]);
        let start = world.cascades.cascades[0].position;
        let keep = world_cell(&world, 0, Vec3::new(1.5, 0.05, 1.0));
        let value = |w: &World, cell: IVec3| w.data[0].stored.cells.iter().find(|c| c.cell() == cell).map(|c| voxels::decode_rgbe8985(c.light & 0x3FFF_FFFF).y).unwrap_or(0.0);
        let mut out = vec![value(&world, keep)];
        for x in [1.0, 2.7, 4.3] {
            world.walk(&objects, ORIGIN + Vec3::X * x);
            world.static_frame(&[light]);
            out.push(value(&world, keep - (world.cascades.cascades[0].position - start)));
        }
        out
    })
}

/// The room's world converged with another bounce feedback (cached per value).
fn bounce_world(bounce: f32) -> &'static World {
    static WORLDS: OnceLock<std::sync::Mutex<Vec<(u32, &'static World)>>> = OnceLock::new();
    let key = (bounce * 100.0).round() as u32;
    let worlds = WORLDS.get_or_init(|| std::sync::Mutex::new(Vec::new()));
    if let Some((_, w)) = worlds.lock().unwrap().iter().find(|(k, _)| *k == key) {
        return w;
    }
    let world: &'static World = Box::leak(Box::new(sim::converged_with(bounce)));
    worlds.lock().unwrap().push((key, world));
    world
}

/// Chapter 7: the cells' light, the extra light's worked cell, its shadow
/// ray, and the static light after scrolls.
fn light_demo(state: &ShotState, scene: &Scene, world: &World, content: &mut FrameContent) {
    let data = &world.data[0];
    let phase = state.p("lightPhase").round() as usize;
    if state.p("cellLight") > 0.5 {
        let size = world.cascades.cascades[0].cell_size * 0.45;
        for (i, c) in data.stored.cells.iter().enumerate() {
            let updated = phase >= 4 || i % sim::FRAMES_TO_UPDATE_LIGHT == phase;
            let l = world.light[0].light_at(c.cell());
            let t = |v: f32| ((v / (1.0 + v)).sqrt() * 255.0) as u8;
            let color = if updated { Color32::from_rgb(t(l.x * 12.0), t(l.y * 12.0), t(l.z * 12.0)) } else { Color32::from_rgb(35, 38, 48) };
            ball(content, cell_world(world, 0, c.cell()), size, color, 1.0);
        }
    }
    let y_mult = world.cascades.settings.y_mult();
    let light_pos = Vec3::new(state.p("lightX"), state.p("lightHeight"), state.p("lightZ"));
    let omni = sim::omni(light_pos, 5.0, 1.0, y_mult).light;
    let at = if state.p("cellX") < -9.0 { Vec3::new(light_pos.x, 0.05, light_pos.z) } else { Vec3::new(state.p("cellX"), 0.05, state.p("cellZ")) };
    let cell = world_cell(world, 0, at);
    let cell_w = cell_world(world, 0, cell);
    let mut rows = Vec::new();
    if state.p("extraLight") > 0.5 {
        ball(content, light_pos, 0.1, palette::INCIDENT, 1.0);
        label(content, state, light_pos + Vec3::Y * 0.2, tr("demos.omni_light").into(), palette::INCIDENT);
        ball(content, cell_w, 0.07, palette::SELECTED, 1.0);
        let field = world.field();
        let squashed = (cell.as_vec3() + Vec3::splat(0.5)) / field.to_cell(0) + field.offset(0);
        if let Some((dir, dist, att)) = light::light_at(&omni, squashed) {
            let mut steps = Vec::new();
            let hit = light::shadow_ray(&field, 0, squashed, dir, dist, Some(&mut steps));
            if state.p("shadowRay") > 0.5 {
                for (j, pos, d) in steps.iter().take(24) {
                    let s = *pos / field.to_cell(*j) + field.offset(*j);
                    let r = Vec3::new(s.x, s.y / y_mult, s.z);
                    circle(content, r, (d.max(0.0) / field.to_cell(*j)).min(1.5).max(0.02), palette::OUTSIDE, 0.9);
                    ball(content, r, 0.03, palette::OUTSIDE, 1.0);
                }
                content.draw.segment(cell_w, light_pos, if hit { palette::MISMATCH } else { palette::HIT }, 1.0, 4.0, 0);
            }
            let real_d = (light_pos - cell_w).length();
            rows.extend([
                row(tr("demos.squashed_distance"), format!("{dist:.2} m ({} {real_d:.2} m)", tr("demos.real")), palette::INCIDENT),
                row(tr("demos.attenuation"), format!("(1 - (d/5)^4)^2 / d = {att:.4}"), palette::INCIDENT),
                row(tr("demos.shadow"), if hit { tr("demos.blocked").to_string() } else { format!("{} ({} {})", tr("demos.lit"), steps.len(), tr("demos.steps")) }, if hit { palette::MISMATCH } else { palette::NORMAL }),
            ]);
        }
    }
    let scrolls = static_scrolls();
    let k = state.p("scrolls").round().clamp(0.0, 3.0) as usize;
    rows.push(row(tr("demos.static_after_scrolls"), format!("{:.3} ({:.1}x)", scrolls[k], scrolls[k] / scrolls[0].max(1e-6)), palette::MISMATCH));
    let bounce = state.p("bounceFeedback");
    let wall = world_cell(world, 0, Vec3::new(-2.95, 1.2, -0.5));
    let wall_light = |w: &World| w.light[0].light_at(wall).max_element();
    let nearest = [0.0f32, 0.5, 1.0, 1.5].into_iter().min_by(|a, b| (a - bounce).abs().total_cmp(&(b - bounce).abs())).unwrap_or(0.5);
    let wall_value = if nearest == 0.5 { wall_light(world) } else { wall_light(bounce_world(nearest)) };
    rows.push(row(tr("demos.wall_cell"), format!("{wall_value:.3} ({} {nearest})", tr("demos.at_bounce")), palette::INCIDENT));
    let lit = data.stored.cells.iter().map(|c| world.light[0].light_at(c.cell()).max_element()).fold(0.0, f32::max);
    rows.push(row(tr("demos.cells_updated"), if phase < 4 { format!("1 / {}: {} {phase}", sim::FRAMES_TO_UPDATE_LIGHT, tr("demos.phase")) } else { tr("demos.all_cells").into() }, palette::THEME_ACCENT));
    rows.push(row(tr("demos.brightest_cell"), format!("{lit:.3}"), palette::text_dim()));
    rows.push(row(tr("demos.panel_light"), format!("AreaLight3D {:.1} x {:.1} m", scene.lighting.light.axis_u.length() * 2.0, scene.lighting.light.axis_v.length() * 2.0), palette::text_dim()));
    content.readout = Some(Readout { title: tr("demos.light").into(), subtitle: tr("demos.light_subtitle").into(), formula: None, formula_lit: None, rows });
}

/// Chapters 8 and 9: the followed probe, its rays, one ray's march, its SH
/// history, and its octahedral maps.
fn probes_demo(state: &ShotState, world: &World, content: &mut FrameContent, maps: bool) {
    let field = world.field();
    let y_mult = world.cascades.settings.y_mult();
    let unsquash = |p: Vec3| Vec3::new(p.x, p.y / y_mult, p.z);
    let probe_pos = probes::probe_position(&field, 0, FOLLOWED_PROBE);
    let real = unsquash(probe_pos);
    ball(content, real, 0.08, palette::PROBE_SELECTED, 1.0);
    let light_refs: Vec<&light::LightTextures> = world.light.iter().collect();
    let lighting = Scene::study_room().lighting;
    let sky_fn = move |d: Vec3| lab::scene::environment_radiance(&lighting, d);
    let sky = probes::Sky::Radiance(&sky_fn);
    let frames = state.p("rayFrames").round().clamp(0.0, 30.0) as u32;
    if state.p("probeRays") > 0.5 {
        for h in 0..frames {
            for ray in probes::trace_rays(&field, &light_refs, 0, FOLLOWED_PROBE, h, &world.integrate, &sky) {
                let end = match ray.hit {
                    Some((j, p)) => unsquash(p / field.to_cell(j) + field.offset(j)),
                    None => real + unsquash(ray.direction) * 1.2,
                };
                // Even slots point to -z, odd ones to +z: a frame's eight rays share a half.
                let color = if h % 2 == 0 { palette::INCIDENT } else { palette::VIEW };
                content.draw.segment(real, end, color, if h + 1 == frames { 1.0 } else { 0.35 }, if ray.hit.is_some() { 2.0 } else { 1.0 }, 0);
            }
        }
    }
    // One ray's march: spheres of the stored distance.
    let steps = state.p("raySteps").round() as usize;
    if steps > 0 {
        let rays = probes::trace_rays(&field, &light_refs, 0, FOLLOWED_PROBE, 0, &world.integrate, &sky);
        let dir = rays[1].direction;
        let start = probe_pos + dir / dir.abs().max_element() * world.integrate.ray_bias / field.to_cell(0);
        let mut advance = 0.0;
        let pos = (start - field.offset(0)) * field.to_cell(0);
        for _ in 0..steps {
            let d = light::sample_sdf(&world.data[0].stored.sdf, (pos + dir * advance) / CASCADE_SIZE as f32) * 255.0 - 1.0;
            let p = unsquash((pos + dir * advance) / field.to_cell(0) + field.offset(0));
            circle(content, p, (d.max(0.0) / field.to_cell(0)).min(3.0), palette::OUTSIDE, 0.5);
            ball(content, p, 0.025, palette::OUTSIDE, 1.0);
            if d < 0.05 {
                break;
            }
            advance += d;
        }
    }
    // The history: mean coefficients after `frames` frames.
    let k = state.p("frames").round().clamp(0.0, sim::CONVERGED_FRAMES as f32) as usize;
    let mean = if k == 0 { [Vec3::ZERO; SH_SIZE] } else { world.trace[k - 1] };
    let irradiance = |n: Vec3| -> Vec3 {
        let c = probes::sh_basis(n);
        (0..SH_SIZE).map(|i| mean[i] * c[i] * 4.0 * probes::L_MULT[i]).sum()
    };
    let up = irradiance(Vec3::Y);
    let side = irradiance(Vec3::NEG_X);
    let mut rows = vec![
        row(tr("demos.probe"), format!("{:?} ({:.2}, {:.2}, {:.2}) m", FOLLOWED_PROBE.to_array(), real.x, real.y, real.z), palette::PROBE_SELECTED),
        row(tr("demos.rays_per_frame"), format!("{} ({} {})", world.integrate.ray_count, world.integrate.history_size * world.integrate.ray_count, tr("demos.over_history")), palette::INCIDENT),
        row(tr("demos.frames_run"), format!("{k} / {}", world.integrate.history_size), palette::THEME_ACCENT),
        row("L0", format!("{:.3} {:.3} {:.3}", mean[0].x, mean[0].y, mean[0].z), palette::text()),
        row(tr("demos.irradiance_up"), format!("{:.3} {:.3} {:.3}", up.x, up.y, up.z), palette::NORMAL),
        row(tr("demos.irradiance_wall"), format!("{:.3} {:.3} {:.3}", side.x, side.y, side.z), Color32::from_rgb(230, 110, 90)),
    ];
    if state.p("historyChart") > 0.5 {
        // Irradiance (up, green channel) after each frame since creation: a straight rise for 30 frames.
        let n = 110usize;
        let values: Vec<f32> = (0..=sim::CONVERGED_FRAMES as usize).map(|f| {
            let m = if f == 0 { [Vec3::ZERO; SH_SIZE] } else { world.trace[f - 1] };
            let c = probes::sh_basis(Vec3::Y);
            (0..SH_SIZE).map(|i| m[i].y * c[i] * 4.0 * probes::L_MULT[i]).sum::<f32>()
        }).collect();
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
                rgba[i..i + 4].copy_from_slice(&if lit { [90, 200, 130, 255] } else { [40, 70, 60, 255] });
            }
        }
        let marker = Some(([k as f32 / sim::CONVERGED_FRAMES as f32, values[k.min(values.len() - 1)] / top], palette::SELECTED));
        content.insets.push(picture(Corner::TopRight, tr("demos.history_chart").into(), "history-chart".into(), n, rgba, tr("demos.frames_axis").into(), tr("demos.irradiance_axis").into(), marker, vec![]));
    }
    if maps {
        // Both maps, 8 x 8 texels each with their border, side by side in a square, nearest-filtered.
        let maps = &world.maps[0];
        let n = (probes::OCT_SIZE + 2) as usize;
        let k = 12usize;
        let size = 2 * n * k + k;
        let origin = glam::IVec2::new((FOLLOWED_PROBE.x + FOLLOWED_PROBE.z * PROBE_AXIS_COUNT) * (probes::OCT_SIZE + 2), FOLLOWED_PROBE.y * (probes::OCT_SIZE + 2));
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
                            rgba[i..i + 4].copy_from_slice(if edge { &[20, 28, 40, 255] } else { &px });
                        }
                    }
                }
            }
        }
        content.insets.push(picture(Corner::TopLeft, tr("demos.oct_maps").into(), "oct-maps".into(), size, rgba, tr("demos.oct_axes").into(), "v".into(), None, vec![(tr("demos.oct_border").into(), palette::text_dim())]));
        if state.p("showGain") > 0.5 {
            let gain = 4.0 / std::f32::consts::PI;
            rows.push(row(tr("demos.sky_gain"), format!("4/pi = {gain:.3}"), palette::MISMATCH));
        }
    }
    content.readout = Some(Readout { title: if maps { tr("demos.maps") } else { tr("demos.probes") }.into(), subtitle: tr("demos.probes_subtitle").into(), formula: None, formula_lit: None, rows });
}

/// Chapter 10: a pixel on the red wall, its cascade, its eight probes.
fn pixel_demo(state: &ShotState, world: &World, content: &mut FrameContent) {
    let on_sphere = state.p("spherePixel") > 0.5;
    let sphere_centre = Vec3::new(-1.05, 0.62, -0.55);
    let n = if on_sphere { Vec3::new(0.35, 0.5, 0.8).normalize() } else { Vec3::X };
    let p = if on_sphere { sphere_centre + n * 0.621 } else { Vec3::new(-2.999, state.p("pixelY"), state.p("pixelZ")) };
    let inputs = sim::screen_inputs(world);
    let eye = Vec3::from(content.uniforms.camera.position);
    let v = (eye - p).normalize();
    let r = (-v) - 2.0 * (-v).dot(n) * n;
    let roughness = if on_sphere { state.p("sphereRoughness") } else { 1.0 };
    let result = super::screen::process(&inputs, p - ORIGIN, n, r, roughness);
    ball(content, p, 0.07, palette::SELECTED, 1.0);
    content.draw.segment(p, p + n * 0.4, palette::NORMAL, 1.0, 3.0, lab::frame::FLAG_ARROW);
    // The normal bias: the point moved off the surface before its probes are chosen (1.1 cells).
    let bias = p + n * (1.1 * 0.2);
    content.draw.segment(p, bias, palette::INCIDENT, 1.0, 4.0, 0);
    if on_sphere {
        content.draw.segment(p, p + r * 0.8, palette::REFLECTED, 1.0, 3.0, lab::frame::FLAG_ARROW);
    }
    let mut rows = vec![row(tr("demos.cascade"), result.cascade.map(|c| c.to_string()).unwrap_or_else(|| "-".into()), palette::AXIS_Y), row(tr("demos.blend"), format!("{:.3}", result.blend), palette::AXIS_Y)];
    let d = state.p("blendPoint");
    if d > 0.0 {
        // A point on SDFGI's camera's +x axis: which cascade holds it and how far it has blended to the next.
        let q = ORIGIN + Vec3::X * d;
        let qr = super::screen::process(&inputs, q - ORIGIN, Vec3::Y, Vec3::Y, 1.0);
        ball(content, q, 0.25, palette::AXIS_Y, 1.0);
        content.draw.segment(ORIGIN, q, palette::AXIS_Y, 1.0, 2.0, 0);
        rows.push(row(tr("demos.test_point"), format!("{d:.1} m: {} {}, blend {:.2}", tr("demos.cascade"), qr.cascade.map(|c| c.to_string()).unwrap_or_else(|| "-".into()), qr.blend), palette::AXIS_Y));
    }
    if on_sphere {
        let radiance = if roughness < 0.99 { 1.0 - ((roughness - 0.2) * 1.25).clamp(0.0, 1.0) } else { 0.0 };
        rows.push(row(tr("demos.roughness"), format!("{roughness:.2}"), palette::VIEW));
        rows.push(row(tr("demos.from_radiance"), format!("{:.0} %", radiance * 100.0), palette::REFLECTED));
        rows.push(row(tr("demos.from_irradiance"), format!("{:.0} %", (1.0 - radiance) * 100.0), palette::NORMAL));
        rows.push(row(tr("demos.godot_march"), if roughness < 0.2 { format!("{:.0} %", (1.0 - (roughness * 5.0).min(1.0)) * 100.0) } else { "0 %".to_string() }, palette::text_dim()));
    }
    if let Some(c) = result.cascade {
        let y = inputs.y_mult;
        let vertex = Vec3::new(p.x - ORIGIN.x, (p.y - ORIGIN.y) * y, p.z - ORIGIN.z);
        let cascade_pos = (vertex - inputs.cascades[c].position) * inputs.cascades[c].to_probe;
        let gi = super::screen::voxel_gi(&inputs, c, cascade_pos, n, r, roughness);
        if state.p("pixelProbes") > 0.5 {
            let total: f32 = gi.probes.iter().map(|w| w.weight).sum();
            let field = world.field();
            for w in &gi.probes {
                let pos = probes::probe_position(&field, c, w.probe);
                let real = Vec3::new(pos.x, pos.y / y, pos.z);
                let share = w.weight / total.max(1e-9);
                ball(content, real, 0.04 + 0.12 * share, palette::PROBE_SELECTED, 0.4 + 0.6 * share);
                content.draw.segment(p, real, palette::PROBE, 0.2 + 0.8 * share, 1.0 + 2.0 * share, 0);
                label(content, state, real + Vec3::Y * 0.15, format!("{:.0} %", share * 100.0), palette::PROBE_SELECTED);
            }
        }
        rows.push(row(tr("demos.probe_cell"), format!("{:.2} {:.2} {:.2}", gi.biased.x, gi.biased.y, gi.biased.z), palette::PROBE_SELECTED));
        rows.push(row(tr("demos.diffuse"), format!("{:.3} {:.3} {:.3}", result.ambient.x, result.ambient.y, result.ambient.z), palette::text()));
        rows.push(row(tr("demos.reflection"), format!("{:.3} {:.3} {:.3}", result.reflection.x, result.reflection.y, result.reflection.z), palette::VIEW));
    }
    content.readout = Some(Readout { title: tr("demos.pixel").into(), subtitle: tr("demos.pixel_subtitle").into(), formula: None, formula_lit: None, rows });
}

/// gShading's derived values (sdfgi_shading.wgsl).
pub fn shader_data(block: &mut lab::shader_data::Block, _state: &ShotState) {
    let settings = Settings::default();
    block.set("yMult", settings.y_mult());
    block.set("cascadeCount", settings.cascades as f32);
    block.set("sdfgiCamera", ORIGIN.to_array());
}
