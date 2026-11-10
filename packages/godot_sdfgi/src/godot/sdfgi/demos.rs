//! What each demonstration of the SDFGI investigation shows: the room shaded
//! with the WGSL screen pass, and the structures of the CPU model (model.rs,
//! voxels.rs, light.rs, probes.rs, screen.rs) drawn over it. Everything a
//! frame shows is computed from the shot's values and the converged world
//! (sim::converged), which is built once.

mod field;
mod light_views;
mod probe_views;
use field::*;
use light_views::*;
use probe_views::*;

use super::light;
use super::model::{self, CASCADE_SIZE, Cascades, Dirty, PROBE_AXIS_COUNT, Settings};
use super::probes::{self, SH_SIZE};
use super::shot::Demo;
use super::sim::{self, World};
use super::voxels::{self, CELLS, index, inside};
use crate::godot::sdfgi::text::{tr, tr_with};
use egui::Color32;
use glam::{IVec3, Vec3};
use lab::frame::{FrameContent, Readout, Statics};
use lab::layout::FrameUniforms;
use lab::palette;
use lab::scene::{Scene, ids};
use lab::shot::ShotState;
use lab::utility::draw::{ball, circle, label, row, wire_box};
use lab::utility::inset::{Corner, Link, Marker, Outline, Picture, PictureInset};
use std::sync::{Arc, OnceLock};

/// Where the lab creates SDFGI, and where its camera stays: the room's
/// centre, at standing height.
pub const ORIGIN: Vec3 = Vec3::new(0.0, 1.6, 0.0);

/// The probe the probe demonstrations follow: cascade 0's probe at
/// (-1.6, 1.07, 0.0) m, between the sphere and the red wall.
pub const FOLLOWED_PROBE: IVec3 = IVec3::new(7, 7, 8);

/// How far the camera has walked from where SDFGI was created: `cameraX` along x, `walkZ` along z.
pub fn walk_offset(state: &ShotState) -> Vec3 {
    Vec3::new(state.p("cameraX"), 0.0, state.p("walkZ"))
}

/// The cascades for a shot: created at `ORIGIN`, then updated once per
/// 0.2 m step of the walk to `cameraX` and `walkZ`, as frames would while the
/// camera walks there, with the shot's y scale.
pub fn cascades(state: &ShotState) -> Cascades {
    let walk = walk_offset(state);
    let steps = if state.p("cameraJump") > 0.5 {
        // A jump moves the camera in a single step.
        1
    } else {
        // A walk: one step per fifth of a metre, so each cascade scrolls as it would.
        (walk.length() / 0.2).ceil() as usize
    };
    let path: Vec<Vec3> = (1..=steps)
        .map(|k| ORIGIN + walk * (k as f32 / steps as f32))
        .collect();
    let settings = Settings {
        y_scale_mode: state.p("yScale").round().clamp(0.0, 2.0) as usize,
        min_cell_size: state.p("minCellSize"),
        ..Settings::default()
    };
    model::follow(settings, ORIGIN, &path)
}

/// The investigation's far plane: the lab's camera stops at 60 m, and the
/// cascades reach 205 m, so its frames draw out to 1 km.
pub const FAR_PLANE: f32 = 1000.0;

/// `camera` with its projection rebuilt for `FAR_PLANE` (the lab's reversed
/// depth, far then near, as lab/src/camera.rs builds it).
fn far_camera(mut camera: lab::layout::CameraData) -> lab::layout::CameraData {
    // The same camera with a far plane at FAR_PLANE, so the farthest cascades fit in its depth.
    let aspect = camera.viewport_size[0] / camera.viewport_size[1].max(1.0);
    let proj = glam::camera::rh::proj::directx::perspective(
        camera.vertical_fov,
        aspect,
        FAR_PLANE,
        camera.near_plane,
    );
    let view = glam::Mat4::from_cols_array_2d(&camera.world_to_view);
    camera.view_to_clip = proj.to_cols_array_2d();
    camera.world_to_clip = (proj * view).to_cols_array_2d();
    camera.far_plane = FAR_PLANE;
    camera
}

const CASCADE_COLORS: [Color32; 4] = [
    palette::AXIS_Y,
    palette::AABB,
    palette::OBB,
    palette::REFLECTED,
];

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
    ((s - field.offset(cascade)) * field.to_cell(cascade))
        .floor()
        .as_ivec3()
}

fn objects_for(state: &ShotState, statics: &Statics) -> Vec<lab::layout::ObjectData> {
    let mut objects = statics.scene.records();
    objects[ids::SPHERE].albedo_roughness[3] = state.p("sphereRoughness");
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
    let objects = objects_for(state, statics);
    let lighting = statics.scene.lighting;
    let camera = far_camera(state.camera.camera_data(viewport));
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
        // The panel would hide the room from the top camera of the slices.
        counts: [
            objects.len() as u32,
            ids::FLOOR as u32,
            0,
            if matches!(
                demo,
                Demo::Field | Demo::Occlusion | Demo::Cascades | Demo::Regions
            ) {
                1 << ids::PANEL
            } else {
                0
            },
        ],
        view_params: [1.0, 0.0, 0.0, 0.0],
    };
    let mut content = FrameContent::new(objects, uniforms);
    // A teapot whose GI mode is Dynamic or Disabled is left out of the voxels: every demonstration reads that world.
    let world = if state.p("teapotGiMode") > 0.5 {
        sim::converged_dynamic_teapot()
    } else {
        sim::converged()
    };
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
    let shown = state
        .p("cascadesShown")
        .clamp(0.0, cascades.cascades.len() as f32);
    for (i, color) in CASCADE_COLORS
        .iter()
        .enumerate()
        .take(cascades.cascades.len())
    {
        // Each cascade's box fades in as `cascadesShown` passes its index, labelled with its width.
        let alpha = (shown - i as f32).clamp(0.0, 1.0);
        if alpha <= 0.0 {
            continue;
        }
        let (min, size) = cascades.bounds(i);
        wire_box(content, min, size, *color, alpha, 2.0);
        let width = CASCADE_SIZE as f32 * cascades.cascades[i].cell_size;
        label(
            content,
            state,
            min + size * Vec3::new(0.5, 1.0, 0.0),
            tr_with(
                "demos-cascade_width",
                lab::args!(index = i, width = format!("{width:.1}")),
            ),
            *color,
        );
    }
    let row = state
        .p("probeRow")
        .round()
        .clamp(0.0, PROBE_AXIS_COUNT as f32) as i32;
    for x in 0..row {
        // One row of cascade 0's probes along x, counted from the cascade's edge, each with its index.
        let pos = cascades.probe_position(0, IVec3::new(x, 8, 8));
        ball(content, pos, 0.09, palette::PROBE_SELECTED, 1.0);
        if x % 8 == 0 {
            // The ends and the middle numbered: 0, 8, and 16.
            content
                .draw
                .text(pos, x.to_string(), palette::text(), [0.5, 1.4]);
        }
        if x > 0 {
            let prev = cascades.probe_position(0, IVec3::new(x - 1, 8, 8));
            content
                .draw
                .segment(prev, pos, palette::PROBE_SELECTED, 0.8, 2.0, 0);
        }
    }
    if state.p("probes") > 0.5 {
        for p in 0..probes::PROBES {
            // Cascade 0's probes: those outside the room smaller and faint, the followed one lit.
            let probe = probes::probe_of(p);
            let pos = cascades.probe_position(0, probe);
            let in_room = Scene::inside_room(pos, 0.0);
            let followed = probe == FOLLOWED_PROBE
                && cascades.cascades[0].position == world.cascades.cascades[0].position;
            let color = if followed {
                palette::PROBE_SELECTED
            } else {
                palette::PROBE
            };
            ball(
                content,
                pos,
                if in_room {
                    0.06
                } else {
                    0.035
                },
                color,
                if in_room {
                    1.0
                } else {
                    0.3
                },
            );
        }
    }
}

/// Chapter 1: the room, SDFGI's settings and their defaults.
fn room(state: &ShotState, content: &mut FrameContent) {
    let settings = Settings::default();
    content.readout = Some(Readout {
        title: tr("demos-settings").into(),
        subtitle: tr("demos-settings_subtitle").into(),
        formula: None,
        formula_lit: None,
        rows: {
            let mut rows = vec![
                row(
                    tr("demos-sdfgi_shown"),
                    format!("{:.2}", state.p("sdfgiMix")),
                    palette::text(),
                ),
                row(
                    "cascades",
                    settings.cascades.to_string(),
                    palette::THEME_ACCENT,
                ),
                row(
                    "min_cell_size",
                    format!("{} m", settings.min_cell_size),
                    palette::THEME_ACCENT,
                ),
                row("y_scale", "75 % (y_mult 1.5)", palette::THEME_ACCENT),
                row("ray_count", tr("demos-ray_count_value"), palette::INCIDENT),
                row(
                    tr("demos-ray_labels"),
                    "8, 16, 32, 64, 96, 128",
                    palette::INCIDENT,
                ),
                row(
                    tr("demos-ray_table"),
                    "4, 8, 16, 32, 64, 96, 128",
                    palette::INCIDENT,
                ),
                row("converge", tr("demos-converge_value"), palette::INCIDENT),
                row(
                    "update_lights",
                    tr("demos-update_lights_value"),
                    palette::INCIDENT,
                ),
                row("depth_prepass", tr("demos-prepass_value"), palette::VIEW),
            ];
            let focus = state.p("focusRow").round() as i32;
            // With a focus row, the other rows dim.
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
    let camera = ORIGIN + walk_offset(state);
    ball(content, camera, 0.15, palette::VIEW, 1.0);
    // The camera SDFGI follows, drawn as the lab draws a camera: SDFGI reads only its position, so it is drawn
    // looking along its walk (+X), with the main camera's field of view.
    let walker = lab::camera::OrbitCamera {
        target: (camera + Vec3::X).to_array(),
        yaw: -90.0,
        pitch: 0.0,
        distance: 1.0,
        fov: state.camera.fov,
    };
    let walker = walker.camera_data(content.uniforms.camera.viewport_size);
    lab::free_view::mark_camera(
        content,
        &walker,
        6.0,
        (state.p("labels") > 0.5).then(|| tr("demos-sdfgi_camera")),
    );
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
            wire_box(
                content,
                r.bounds_min,
                r.bounds_size,
                palette::INSIDE,
                1.0,
                3.0,
            );
        }
    }
    // A top view of one row of cells along x: the camera's cell and the cascade's centre.
    scroll_row(state, &c0, cell, content);
    let (min, size) = cascades.bounds(0);
    content.readout = Some(Readout {
        title: tr("demos-cascade_zero").into(),
        subtitle: tr("demos-cascade_zero_subtitle").into(),
        formula: None,
        formula_lit: None,
        rows: vec![
            row(
                tr("demos-camera_x"),
                format!("{:.2} m", camera.x),
                palette::VIEW,
            ),
            row(
                tr("demos-camera_cell"),
                format!("int({:.2} / 0.2) = {}", camera.x, cell.x),
                palette::VIEW,
            ),
            row(
                tr("demos-cascade_position"),
                format!("{} {}", c0.position.x, tr("demos-cells")),
                palette::AXIS_Y,
            ),
            row(
                "drag_margin",
                tr("demos-drag_margin_value"),
                palette::THEME_ACCENT,
            ),
            row("dirty_regions", dirty, palette::INSIDE),
            row(
                tr("demos-box"),
                format!("{:.2} .. {:.2} m", min.x, min.x + size.x),
                palette::AXIS_Y,
            ),
            row(
                tr("demos-height"),
                format!("{:.2} m (y_mult {y_mult})", size.y),
                palette::AXIS_Y,
            ),
        ],
    });
}

/// A top view of one row of cascade 0's cells along x, when the scroll view is on: the camera's cell and the
/// cascade's centre.
fn scroll_row(state: &ShotState, c0: &model::Cascade, cell: IVec3, content: &mut FrameContent) {
    if state.p("scrollView") > 0.5 {
        // Forty-one cell edges on the floor, those near the centre stronger, then the centre and the camera's cell.
        let y = 0.02;
        let from = c0.position.x - 20;
        for k in 0..=40 {
            let x = (from + k) as f32 * c0.cell_size;
            let strong = (from + k - c0.position.x).abs() <= 4;
            content.draw.segment(
                Vec3::new(x, y, -0.3),
                Vec3::new(x, y, 0.3),
                if strong {
                    palette::THEME_ACCENT
                } else {
                    palette::BRICK_EMPTY
                },
                1.0,
                if strong {
                    2.0
                } else {
                    1.0
                },
                0,
            );
        }
        let centre = c0.position.x as f32 * c0.cell_size;
        content.draw.segment(
            Vec3::new(centre, y, -0.6),
            Vec3::new(centre, y, 0.6),
            palette::AXIS_Y,
            1.0,
            4.0,
            0,
        );
        label(
            content,
            state,
            Vec3::new(centre, 0.1, 0.7),
            tr("demos-cascade_centre").into(),
            palette::AXIS_Y,
        );
        let cam = cell.x as f32 * c0.cell_size + c0.cell_size * 0.5;
        ball(content, Vec3::new(cam, y, 0.0), 0.08, palette::VIEW, 1.0);
    }
}

/// Chapter 3: the regions of the walk, the geometry inside them, and the light lists.
fn regions_demo(state: &ShotState, content: &mut FrameContent) {
    let cascades = cascades(state);
    let regions = cascades.pending_regions();
    for r in &regions {
        wire_box(
            content,
            r.bounds_min,
            r.bounds_size,
            palette::INSIDE,
            1.0,
            4.0,
        );
        label(
            content,
            state,
            r.bounds_min + r.bounds_size * Vec3::new(0.5, 0.0, 0.5),
            tr_with("demos-region_label", lab::args!(cascade = r.cascade)),
            palette::INSIDE,
        );
    }
    let static_light = sim::omni(
        Vec3::new(state.p("lightX"), state.p("lightHeight"), state.p("lightZ")),
        15.0,
        1.0,
        cascades.settings.y_mult(),
    );
    let lights = if state.p("lightBake") > 0.5 {
        // Baking the light: it goes through the static cull, as Godot gives it to the cascades' regions.
        vec![static_light]
    } else {
        Vec::new()
    };
    let cull = sim::cull_static(&cascades, &lights);
    let given = sim::static_pass_lights(&cascades, &cull, &lights);
    if state.p("extraLight") > 0.5 {
        ball(
            content,
            Vec3::new(state.p("lightX"), state.p("lightHeight"), state.p("lightZ")),
            0.1,
            palette::INCIDENT,
            1.0,
        );
    }
    let mut rows = vec![row(
        tr("demos-regions"),
        regions.len().to_string(),
        palette::INSIDE,
    )];
    for r in regions.iter().take(4) {
        rows.push(row(
            format!("{} {}", tr("demos-cascade"), r.cascade),
            format!(
                "({}, {}, {}) + ({}, {}, {})",
                r.local_offset.x,
                r.local_offset.y,
                r.local_offset.z,
                r.local_size.x,
                r.local_size.y,
                r.local_size.z
            ),
            CASCADE_COLORS[r.cascade.min(3)],
        ));
    }
    rows.push(row(
        "cascade_light_index",
        format!("{:?}", cull.cascade_light_index),
        palette::INCIDENT,
    ));
    rows.push(row(
        tr("demos-stored_under"),
        format!(
            "{:?}",
            (0..8)
                .filter(|&c| !cull.lists[c].is_empty())
                .collect::<Vec<_>>()
        ),
        palette::INCIDENT,
    ));
    rows.push(row(
        tr("demos-static_pass_gets"),
        format!(
            "{:?}",
            given.iter().map(|(c, l)| (*c, l.len())).collect::<Vec<_>>()
        ),
        palette::INCIDENT,
    ));
    content.readout = Some(Readout {
        title: tr("demos-culling").into(),
        subtitle: tr("demos-culling_subtitle").into(),
        formula: None,
        formula_lit: None,
        rows,
    });
}

/// The solid voxel the voxel and field readouts describe: the top of the sphere.
fn worked_voxel(world: &World) -> IVec3 {
    let top = Vec3::new(-1.05, 1.24, -0.55);
    let target = world_cell(world, 0, top);
    world.data[0]
        .stored
        .cells
        .iter()
        .map(|c| c.cell())
        .min_by_key(|c| (*c - target).length_squared())
        .unwrap_or(target)
}

/// Chapter 6: one probe's occlusion around it.
fn occlusion_demo(state: &ShotState, world: &World, content: &mut FrameContent) {
    let data = &world.data[0];
    let probe_cell = FOLLOWED_PROBE * 8;
    // The followed probe's occlusion class: the parity of its world position on each axis.
    let world_probe = world.cascades.cascades[0].position / 8 + FOLLOWED_PROBE;
    let bits = world_probe & IVec3::ONE;
    let class = (bits.x | (bits.y << 1) | (bits.z << 2)) as usize;
    let shell = state.p("occlusionShell");
    let inset = slice_window(
        state,
        world,
        Caption {
            title: tr("demos-occlusion_slice").into(),
            key: format!("occ-{}", shell.round()),
            lines: vec![],
        },
        |p| {
            // The cell's offset folded into the probe's octant, as the occlusion pass counts its shells.
            let d = p - probe_cell;
            let local = IVec3::new(
                if d.x >= 0 {
                    d.x
                } else {
                    -d.x - 1
                },
                if d.y >= 0 {
                    d.y
                } else {
                    -d.y - 1
                },
                if d.z >= 0 {
                    d.z
                } else {
                    -d.z - 1
                },
            );
            if local.max_element() >= 8 {
                // Outside the probe's window: dark.
                return [20, 20, 30, 255];
            }
            if (local.x + local.y + local.z) as f32 > shell {
                // A shell the pass has not reached yet.
                return [40, 40, 60, 255];
            }
            if data.voxels.solid(p) {
                // A solid voxel, in orange; free cells are grey by their occlusion.
                return [200, 90, 40, 255];
            }
            grey(super::occlusion::class_value(
                data.occlusion[index(p)],
                class,
            ))
        },
        Some(probe_cell),
        probe_cell - IVec3::new(10, 0, 10),
        20,
    );
    content.insets.push(inset);
    ball(
        content,
        cell_world(world, 0, probe_cell),
        0.1,
        palette::PROBE_SELECTED,
        1.0,
    );
    let packed = data.occlusion[index(probe_cell + IVec3::new(2, -2, 1))];
    cube_probes(state, world, probe_cell + IVec3::new(2, -2, 1), content);
    content.readout = Some(Readout {
        title: tr("demos-occlusion").into(),
        subtitle: tr("demos-occlusion_subtitle").into(),
        formula: None,
        formula_lit: None,
        rows: (0..8)
            .map(|i| {
                row(
                    format!("{} {i}", tr("demos-class")),
                    format!(
                        "{} / 15",
                        (packed >> super::occlusion::OCCLUSION_SHIFT[i]) & 0xF
                    ),
                    if i == class {
                        palette::PROBE_SELECTED
                    } else {
                        palette::text_dim()
                    },
                )
            })
            .chain([row(
                "use_occlusion",
                tr("demos-occlusion_off"),
                palette::text(),
            )])
            .collect(),
    });
}

/// The readout's cell and the eight probes at the corners of its probe cube (`cubeProbes` of them, 0 to 8): a line
/// to each, as bright as the 4-bit visibility the cell keeps for that probe's class, with that value.
fn cube_probes(state: &ShotState, world: &World, cell: IVec3, content: &mut FrameContent) {
    let shown = state.p("cubeProbes").round().clamp(0.0, 8.0) as i32;
    if shown == 0 {
        return;
    }
    let packed = world.data[0].occlusion[index(cell)];
    let at = cell_world(world, 0, cell);
    ball(content, at, 0.05, palette::SELECTED, 1.0);
    let base = IVec3::new(
        cell.x.div_euclid(8),
        cell.y.div_euclid(8),
        cell.z.div_euclid(8),
    );
    for k in 0..shown {
        // Each corner of the probe cube: its probe's class is the parity of its world position on each axis.
        let probe = base + IVec3::new(k & 1, (k >> 1) & 1, (k >> 2) & 1);
        let bits = (world.cascades.cascades[0].position / 8 + probe) & IVec3::ONE;
        let class = (bits.x | (bits.y << 1) | (bits.z << 2)) as usize;
        let value = (packed >> super::occlusion::OCCLUSION_SHIFT[class]) & 0xF;
        let p = cell_world(world, 0, probe * 8);
        content.draw.segment(
            at,
            p,
            palette::PROBE_SELECTED,
            0.15 + 0.85 * value as f32 / 15.0,
            2.0,
            0,
        );
        ball(content, p, 0.06, palette::PROBE_SELECTED, 1.0);
        // The upper corner's value above its probe, the lower one's below, so a top view keeps them apart.
        let align = if k >> 1 & 1 == 1 {
            [0.5, 1.3]
        } else {
            [0.5, -0.3]
        };
        content
            .draw
            .text(p, format!("{value}/15"), palette::text(), align);
    }
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
        // One cell's static light after each of three walks: the scrolls carry its light along.
        let start = world.cascades.cascades[0].position;
        let keep = world_cell(&world, 0, Vec3::new(1.5, 0.05, 1.0));
        let value = |w: &World, cell: IVec3| {
            w.data[0]
                .stored
                .cells
                .iter()
                .find(|c| c.cell() == cell)
                .map(|c| voxels::decode_rgbe8985(c.light & 0x3FFF_FFFF).y)
                .unwrap_or(0.0)
        };
        let mut out = vec![value(&world, keep)];
        for x in [1.0, 2.7, 4.3] {
            world.walk(&objects, ORIGIN + Vec3::X * x);
            world.static_frame(&[light]);
            out.push(value(
                &world,
                keep - (world.cascades.cascades[0].position - start),
            ));
        }
        out
    })
}

/// The room's world converged with another bounce feedback (cached per value).
fn bounce_world(bounce: f32) -> &'static World {
    static WORLDS: OnceLock<std::sync::Mutex<Vec<(u32, &'static World)>>> = OnceLock::new();
    let key = (bounce * 100.0).round() as u32;
    let worlds = WORLDS.get_or_init(|| std::sync::Mutex::new(Vec::new()));
    // Each bounce feedback converges once, then is kept for the life of the program.
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
            // Each cell coloured by its light, the ones this frame's phase did not update dark.
            let updated = phase >= 4 || i % sim::FRAMES_TO_UPDATE_LIGHT == phase;
            let l = world.light[0].light_at(c.cell());
            let t = |v: f32| ((v / (1.0 + v)).sqrt() * 255.0) as u8;
            let color = if updated {
                Color32::from_rgb(t(l.x * 12.0), t(l.y * 12.0), t(l.z * 12.0))
            } else {
                Color32::from_rgb(35, 38, 48)
            };
            ball(content, cell_world(world, 0, c.cell()), size, color, 1.0);
        }
    }
    let light_pos = Vec3::new(state.p("lightX"), state.p("lightHeight"), state.p("lightZ"));
    let at = if state.p("cellX") < -9.0 {
        // No cell picked: the floor under the light.
        Vec3::new(light_pos.x, 0.05, light_pos.z)
    } else {
        Vec3::new(state.p("cellX"), 0.05, state.p("cellZ"))
    };
    let cell = world_cell(world, 0, at);
    face_light(state, world, cell, content);
    neighbour_flags(state, world, cell, content);
    let mut rows = Vec::new();
    if state.p("extraLight") > 0.5 {
        rows.extend(extra_light(state, world, (light_pos, cell), content));
    }
    let scrolls = static_scrolls();
    let k = state.p("scrolls").round().clamp(0.0, 3.0) as usize;
    rows.push(row(
        tr("demos-static_after_scrolls"),
        format!(
            "{:.3} ({:.1}x)",
            scrolls[k],
            scrolls[k] / scrolls[0].max(1e-6)
        ),
        palette::MISMATCH,
    ));
    let bounce = state.p("bounceFeedback");
    let wall = world_cell(world, 0, Vec3::new(-2.95, 1.2, -0.5));
    let wall_light = |w: &World| w.light[0].light_at(wall).max_element();
    // The wall's light at the nearest converged bounce feedback.
    let nearest = [0.0f32, 0.5, 1.0, 1.5]
        .into_iter()
        .min_by(|a, b| (a - bounce).abs().total_cmp(&(b - bounce).abs()))
        .unwrap_or(0.5);
    let wall_value = if nearest == 0.5 {
        wall_light(world)
    } else {
        wall_light(bounce_world(nearest))
    };
    rows.push(row(
        tr("demos-wall_cell"),
        format!("{wall_value:.3} ({} {nearest})", tr("demos-at_bounce")),
        palette::INCIDENT,
    ));
    let lit = data
        .stored
        .cells
        .iter()
        .map(|c| world.light[0].light_at(c.cell()).max_element())
        .fold(0.0, f32::max);
    rows.push(row(
        tr("demos-cells_updated"),
        if phase < 4 {
            format!(
                "1 / {}: {} {phase}",
                sim::FRAMES_TO_UPDATE_LIGHT,
                tr("demos-phase")
            )
        } else {
            tr("demos-all_cells").into()
        },
        palette::THEME_ACCENT,
    ));
    rows.push(row(
        tr("demos-brightest_cell"),
        format!("{lit:.3}"),
        palette::text_dim(),
    ));
    rows.push(row(
        tr("demos-panel_light"),
        format!(
            "AreaLight3D {:.1} x {:.1} m",
            scene.lighting.light.axis_u.length() * 2.0,
            scene.lighting.light.axis_v.length() * 2.0
        ),
        palette::text_dim(),
    ));
    content.readout = Some(Readout {
        title: tr("demos-light").into(),
        subtitle: tr("demos-light_subtitle").into(),
        formula: None,
        formula_lit: None,
        rows,
    });
}

/// An omni light added at `light_pos` (`extraLight`): the cell it lights, the shadow ray's march to it, and the
/// light's rows: the squashed distance, the attenuation, and whether the cell is lit.
fn extra_light(
    state: &ShotState,
    world: &World,
    (light_pos, cell): (Vec3, IVec3),
    content: &mut FrameContent,
) -> Vec<(String, String, Color32)> {
    let y_mult = world.cascades.settings.y_mult();
    let omni = sim::omni(light_pos, 5.0, 1.0, y_mult).light;
    let cell_w = cell_world(world, 0, cell);
    let mut rows = Vec::new();
    ball(content, light_pos, 0.1, palette::INCIDENT, 1.0);
    squashed_light(state, light_pos, y_mult, content);
    label(
        content,
        state,
        light_pos + Vec3::Y * 0.2,
        tr("demos-omni_light").into(),
        palette::INCIDENT,
    );
    ball(content, cell_w, 0.07, palette::SELECTED, 1.0);
    let field = world.field();
    let squashed = (cell.as_vec3() + Vec3::splat(0.5)) / field.to_cell(0) + field.offset(0);
    if let Some((_, dist, _)) = light::light_at(&omni, squashed) {
        falloff_chart(state, dist, content);
    }
    if let Some((dir, dist, att)) = light::light_at(&omni, squashed) {
        let mut steps = Vec::new();
        let hit = light::shadow_ray(&field, 0, squashed, dir, dist, Some(&mut steps));
        if state.p("shadowRay") > 0.5 {
            let shown = state.p("shadowSteps").round().clamp(0.0, 24.0) as usize;
            for (j, pos, d) in steps.iter().take(shown) {
                // Each march step, back from cells and the squashed height to the world, a circle of its distance.
                let s = *pos / field.to_cell(*j) + field.offset(*j);
                let r = Vec3::new(s.x, s.y / y_mult, s.z);
                circle(
                    content,
                    r,
                    (d.max(0.0) / field.to_cell(*j)).clamp(0.02, 1.5),
                    palette::OUTSIDE,
                    0.9,
                );
                ball(content, r, 0.03, palette::OUTSIDE, 1.0);
            }
            content.draw.segment(
                cell_w,
                light_pos,
                if hit {
                    palette::MISMATCH
                } else {
                    palette::HIT
                },
                1.0,
                4.0,
                0,
            );
        }
        let real_d = (light_pos - cell_w).length();
        rows.extend([
            row(
                tr("demos-squashed_distance"),
                format!("{dist:.2} m ({} {real_d:.2})", tr("demos-real")),
                palette::INCIDENT,
            ),
            row(
                tr("demos-attenuation"),
                format!("{att:.4}"),
                palette::INCIDENT,
            ),
            row(
                tr("demos-shadow"),
                if hit {
                    tr("demos-blocked").to_string()
                } else {
                    format!(
                        "{} ({} {})",
                        tr("demos-lit"),
                        steps.len(),
                        tr("demos-steps")
                    )
                },
                if hit {
                    palette::MISMATCH
                } else {
                    palette::NORMAL
                },
            ),
        ]);
    }
    rows
}

/// gShading's derived values (sdfgi_shading.wgsl).
pub fn shader_data(block: &mut lab::shader_data::Block, _state: &ShotState) {
    let settings = Settings::default();
    block.set("yMult", settings.y_mult());
    block.set("cascadeCount", settings.cascades as f32);
    block.set("sdfgiCamera", ORIGIN.to_array());
}
