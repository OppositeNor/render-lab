//! What each demonstration of the SSR investigation shows: the room drawn with
//! Godot's SSR (the program's passes, gpu.rs, and the opaque pass,
//! shaders/ssr_shading.wgsl), its materials set from the shot's values.

use super::shot::Demo;
use super::trace::{self, Exit, Image};
use super::{cpu, gpu};
use crate::godot::ssr::text::{tr, tr_with};
use egui::Color32;
use glam::{Vec2, Vec3};
use lab::fetch::Source;
use lab::frame::{self, FrameContent, Readout, Statics};
use lab::layout::FrameUniforms;
use lab::palette;
use lab::scene::ids;
use lab::shot::ShotState;
use lab::utility::height_field::{self, HeightField, Placement};
use lab::utility::inset::{Link, Marker, Outline, Picture, PictureInset};
use std::sync::Arc;

mod stepping;
use stepping::*;

/// The scene's records with the shot's materials: the glossy floor and teapot.
pub fn objects_for(state: &ShotState, statics: &Statics) -> Vec<lab::layout::ObjectData> {
    let mut objects = statics.scene.records();
    objects[ids::FLOOR].albedo_roughness[3] = state.p("floorRoughness");
    objects[ids::TEAPOT].albedo_roughness[3] = state.p("teapotRoughness");
    objects
}

/// The views SSR stacks over the main one (lab/src/view.rs): the depth map as
/// a height field (the pyramid and the trace), and the trace as a
/// cross-section along its ray. Each has its own camera and its own values
/// (`view.field.…`, `view.profile.…`), and draws the main view's data.
pub const FIELD: &str = "field";
pub const PROFILE: &str = "profile";

/// How far a picture view's plane is from its camera at rest; the camera orbits the plane's centre.
const REST: f32 = 2.0;

/// A picture view's camera at rest: facing its plane from `REST` away, as wide
/// as the main camera, so the flat picture covers the view as the main frame does.
pub fn rest_camera(main: &lab::camera::OrbitCamera) -> lab::camera::OrbitCamera {
    lab::camera::OrbitCamera {
        target: [0.0; 3],
        yaw: 0.0,
        pitch: 0.0,
        distance: REST,
        fov: main.fov,
    }
}

/// The views over the main one for `state`, back to front: each shown as far as its `…Show` value.
pub fn views(state: &ShotState) -> Vec<lab::investigation::OverView> {
    let demo = Demo::of(state.demo);
    let mut out = Vec::new();
    for (name, show, demos) in [
        (FIELD, "fieldShow", &[Demo::Pyramid, Demo::Trace][..]),
        (PROFILE, "profileShow", &[Demo::Trace][..]),
    ] {
        let alpha = state.view_state(name).p(show).clamp(0.0, 1.0);
        if alpha > 0.0 && demos.contains(&demo) {
            out.push(lab::investigation::OverView {
                name,
                state: state.clone(),
                camera: state.own_camera(name, rest_camera(&state.camera)),
                alpha,
            });
        }
    }
    out
}

/// How far the views over the main one cover it (its own pictures fade as they do).
fn covered(state: &ShotState) -> f32 {
    views(state).iter().map(|v| v.alpha).fold(0.0, f32::max)
}

/// The study room's records (where the teapot is, for the default P), read once.
fn room_objects() -> &'static [lab::layout::ObjectData] {
    static ROOM: std::sync::OnceLock<Vec<lab::layout::ObjectData>> = std::sync::OnceLock::new();
    ROOM.get_or_init(|| {
        lab::frame::Statics::prepare(lab::scene::SceneId::STUDY_ROOM)
            .expect("the study room")
            .scene
            .records()
    })
}

/// A picture view's frame: its canvas scene (`statics`), its own camera, and a dark room around it.
fn canvas_content(
    state: &ShotState,
    statics: &Statics,
    camera: &lab::camera::OrbitCamera,
    viewport: [f32; 2],
    pixel_scale: f32,
    time: f32,
) -> FrameContent {
    let objects = statics.scene.records();
    let lighting = statics.scene.lighting;
    let uniforms = FrameUniforms {
        camera: camera.camera_data(viewport),
        light_center_radiance: lighting
            .light
            .center
            .extend(lighting.light.radiance)
            .to_array(),
        light_axis_u: lighting.light.axis_u.extend(0.0).to_array(),
        light_axis_v: lighting.light.axis_v.extend(0.0).to_array(),
        light_color: lighting.light.color.extend(1.0).to_array(),
        ambient_color: lighting.ambient.extend(1.0).to_array(),
        floor_params: [objects[0].albedo_roughness[3], lighting.f0, 0.0, 1.0],
        overlay_params: [lighting.exposure, pixel_scale, time, 0.0],
        counts: [objects.len() as u32, 0, 0, 0],
        view_params: [1.0, 1.0, 0.0, 0.0],
    };
    let _ = state;
    let mut content = FrameContent::new(objects, uniforms);
    // A dark room around the picture, so the sky never shows behind it, from any side.
    content.draw.cuboid(
        Vec3::ZERO,
        [Vec3::X * 60.0, Vec3::Y * 60.0, Vec3::Z * 60.0],
        Color32::from_rgb(8, 11, 16),
        1.0,
    );
    content
}

/// The main view's trace of P, on the main view's data (for a picture view).
fn main_trace(state: &ShotState, screen: &cpu::Screen) -> ([i32; 2], trace::Trace) {
    let pixel = picked(state, screen, room_objects());
    let last = Image::new([1, 1], 4);
    let inputs = screen.inputs(
        &last,
        state.p("ssrMaxSteps").round() as i32,
        state.p("ssrFadeIn"),
        state.p("ssrFadeOut"),
        state.p("ssrDepthTolerance"),
    );
    (pixel, inputs.trace(pixel))
}

/// The field view: the main view's depth map as a height field, from its own camera.
fn field_view(
    state: &ShotState,
    statics: &Statics,
    viewport: [f32; 2],
    pixel_scale: f32,
    time: f32,
) -> FrameContent {
    let own = state.view_state(FIELD);
    let camera = state.own_camera(FIELD, rest_camera(&state.camera));
    let mut content = canvas_content(&own, statics, &camera, viewport, pixel_scale, time);
    let rest = rest_camera(&state.camera).camera_data(viewport);
    // The main view's data (a data link), seen as the main camera saw it.
    let Some(screen) = cpu::screen_in(lab::view::ViewId::MAIN, &state.camera.camera_data(viewport))
    else {
        return content;
    };
    if Demo::of(state.demo) == Demo::Trace {
        let (pixel, t) = main_trace(state, &screen);
        let stepper = Stepper::new(&t, state.p("traceStep"));
        let level = stepper.level();
        // The columns of the level shown, the cell tested lit (blocking: the hit colour), the ray over them.
        let lit: Vec<([i32; 2], Color32)> = stepper
            .tested()
            .map(|(_, s)| {
                field_cells(s, level)
                    .into_iter()
                    .map(|c| {
                        (
                            c,
                            if s.hit {
                                palette::INCIDENT
                            } else {
                                palette::SELECTED
                            },
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        let (w, h) = (screen.size[0] as f32, screen.size[1] as f32);
        let ends = [
            t.screen_pos,
            t.screen_pos + t.screen_ray * t.steps.last().map_or(0.0, |s| s.next_t),
        ];
        let focus = [
            ends[0].x.min(ends[1].x) * w,
            ends[0].y.min(ends[1].y) * h,
            ends[0].x.max(ends[1].x) * w,
            ends[0].y.max(ends[1].y) * h,
        ];
        let f = field(
            &own,
            &mut content,
            &rest,
            &screen,
            &FieldView {
                pixel,
                focus,
                level,
                lit: &lit,
            },
        );
        let top = field_top(&screen, pixel);
        let over = |tt: f32| {
            let s = t.screen_pos + t.screen_ray * tt;
            f.point_uv(Vec2::new(s.x, s.y), s.z / top)
        };
        let ray: Vec<Vec3> = std::iter::once(0.0)
            .chain(stepper.path())
            .map(over)
            .collect();
        if ray.len() > 1 {
            content.draw.polyline(&ray, palette::REFLECTED, 1.0, 3.0);
            content
                .draw
                .point(*ray.last().expect("a point"), 6.0, palette::REFLECTED, 1.0);
        }
    } else {
        let pixel = picked(state, &screen, room_objects());
        let shown = own.p("fieldLevel");
        let at = (shown.round().max(0.0) as usize).min(screen.hiz.len() - 1);
        let cell = [pixel[0] >> at, pixel[1] >> at];
        let focus = [
            pixel[0] as f32 - 16.0,
            pixel[1] as f32 - 16.0,
            pixel[0] as f32 + 16.0,
            pixel[1] as f32 + 16.0,
        ];
        field(
            &own,
            &mut content,
            &rest,
            &screen,
            &FieldView {
                pixel,
                focus,
                level: shown,
                lit: &[(cell, palette::SELECTED)],
            },
        );
    }
    content
}

/// The cross-section view: the main view's trace along its ray, from its own camera.
fn profile_view(
    state: &ShotState,
    statics: &Statics,
    viewport: [f32; 2],
    pixel_scale: f32,
    time: f32,
) -> FrameContent {
    let own = state.view_state(PROFILE);
    let camera = state.own_camera(PROFILE, rest_camera(&state.camera));
    let mut content = canvas_content(&own, statics, &camera, viewport, pixel_scale, time);
    let rest = rest_camera(&state.camera).camera_data(viewport);
    let Some(screen) = cpu::screen_in(lab::view::ViewId::MAIN, &state.camera.camera_data(viewport))
    else {
        return content;
    };
    let (_, t) = main_trace(state, &screen);
    profile(
        &mut content,
        &rest,
        &screen,
        &t,
        &Stepper::new(&t, state.p("traceStep")),
    );
    content
}

pub fn build_frame(
    state: &ShotState,
    statics: &Statics,
    viewport: [f32; 2],
    pixel_scale: f32,
    time: f32,
) -> FrameContent {
    // A view stacked over the main one builds its own picture (lab/src/view.rs).
    match lab::view::name() {
        Some(FIELD) => return field_view(state, statics, viewport, pixel_scale, time),
        Some(PROFILE) => return profile_view(state, statics, viewport, pixel_scale, time),
        _ => {}
    }
    let demo = Demo::of(state.demo);
    let objects = objects_for(state, statics);
    let lighting = statics.scene.lighting;
    let uniforms = FrameUniforms {
        camera: state.camera.camera_data(viewport),
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
        counts: [objects.len() as u32, ids::FLOOR as u32, 0, 0],
        // The lab's ambient term on: it stands in for Godot's ambient light (README "Simplified").
        view_params: [1.0, 1.0, 0.0, 0.0],
    };
    let mut content = FrameContent::new(objects, uniforms);
    match demo {
        Demo::Room | Demo::Compose => {}
        Demo::Trace => trace_demo(state, &mut content),
        Demo::Inputs => inputs_demo(state, &mut content),
        Demo::Pyramid => pyramid_demo(state, &mut content),
        Demo::Hit => hit_demo(state, &mut content),
        Demo::Blur => blur_demo(state, &mut content),
    }
    content
}

/// The pixel P a demonstration follows: the picked one, or the floor in front of the teapot.
pub fn picked(
    state: &ShotState,
    screen: &cpu::View,
    objects: &[lab::layout::ObjectData],
) -> [i32; 2] {
    let (x, y) = (state.p("pickX"), state.p("pickY"));
    if x >= 0.0 && y >= 0.0 {
        let (w, h) = (screen.size[0] as f32, screen.size[1] as f32);
        let (rw, rh) = (state.p("pickWidth"), state.p("pickHeight"));
        // Chosen at another size: the same direction, at this one (the same pixel at that size).
        let (x, y) = if rw > 0.0 && rh > 0.0 && (rw != w || rh != h) {
            let ndc = ((x + 0.5) / rw * 2.0 - 1.0) * (rw / rh) / (w / h);
            (
                ((ndc + 1.0) * 0.5 * w).floor().max(0.0),
                ((y + 0.5) / rh * h).floor(),
            )
        } else {
            (x, y)
        };
        return [
            (x as i32).min(screen.size[0] as i32 - 1),
            (y as i32).min(screen.size[1] as i32 - 1),
        ];
    }
    let t = Vec3::from(objects[ids::TEAPOT].world_center);
    screen
        .pixel_of(Vec3::new(t.x, 0.0, t.z + 0.9))
        .unwrap_or([screen.size[0] as i32 / 2, screen.size[1] as i32 * 4 / 5])
}

/// One pixel's trace, stepped (screen_space_reflection.glsl's loop): the ray in
/// the room up to the iteration shown, the level it is on with the cells it
/// visited, and the iteration's values.
fn trace_demo(state: &ShotState, content: &mut FrameContent) {
    let Some(screen) = cpu::screen(content) else {
        return;
    };
    let pixel = picked(state, &screen, &content.objects);
    let last = Image::new([1, 1], 4);
    let inputs = screen.inputs(
        &last,
        state.p("ssrMaxSteps").round() as i32,
        state.p("ssrFadeIn"),
        state.p("ssrFadeOut"),
        state.p("ssrDepthTolerance"),
    );
    let t = inputs.trace(pixel);
    let stepper = Stepper::new(&t, state.p("traceStep"));
    // The views over this one (the field, the cross-section) fade its own pictures as they cover it.
    let room = 1.0 - covered(state);
    let start = screen.surface(pixel);
    content.draw.point(start, 6.0, palette::SELECTED, room);
    // The ray in the room: the screen-space ray at each iteration's t, back in the world.
    let at = |tt: f32| screen.world_of(t.screen_pos + t.screen_ray * tt);
    let mut path: Vec<Vec3> = vec![screen.view_to_world(t.biased)];
    path.extend(stepper.path().into_iter().map(at));
    if path.len() > 1 {
        content.draw.polyline(&path, palette::REFLECTED, room, 3.0);
        content.draw.point(
            *path.last().expect("a point"),
            5.0,
            palette::REFLECTED,
            room,
        );
    }
    // The level of the iteration tested, the cells visited on it, the tested one lit.
    let tested = stepper.tested();
    let current = tested.map(|(_, s)| s);
    let shown = tested.map_or(0, |(k, _)| k + 1);
    let level = current.map_or(0, |s| s.level.max(0)) as usize;
    let img = &screen.hiz[level];
    let outlines = visited_cells(&t, level, img, shown);
    let size = Vec2::new(screen.size[0] as f32, screen.size[1] as f32);
    // The marker is y up (inset.rs); SSR pixel rows run down.
    let marker = Marker {
        at: [
            (pixel[0] as f32 + 0.5) / size.x,
            1.0 - (pixel[1] as f32 + 0.5) / size.y,
        ],
        color: palette::SELECTED,
        label: "P".into(),
    };
    let link = (!outlines.is_empty()).then(|| Link {
        from: if path.len() > 1 {
            at(stepper.tip())
        } else {
            start
        },
        outline: outlines.len() - 1,
    });
    let title = tr_with(
        "demos-trace_inset",
        lab::args!(level = level, width = img.size[0], height = img.size[1]),
    );
    // The level as a picture; the height field, when shown, is that picture risen.
    if room > 0.5 {
        content.insets.push(PictureInset {
            marker: Some(marker),
            outlines,
            link,
            ..PictureInset::new(
                title,
                level_picture(img, format!("ssr-trace-level-{level}")),
                [420.0, 420.0 * img.size[1] as f32 / img.size[0] as f32],
            )
        });
    }
    // The iteration shown, as the loop computes it.
    let rows = trace_rows(&t, &stepper);
    content.readout = Some(Readout {
        title: tr_with("demos-trace_title", lab::args!(x = pixel[0], y = pixel[1])),
        subtitle: tr("demos-trace_subtitle").into(),
        formula: None,
        formula_lit: None,
        rows,
    });
}

/// The cells the first `shown` iterations visited on `level` (`img`), the last one lit.
fn visited_cells(t: &trace::Trace, level: usize, img: &Image, shown: usize) -> Vec<Outline> {
    let mut outlines = Vec::new();
    for (k, s) in t.steps.iter().take(shown).enumerate() {
        if s.level.max(0) as usize != level {
            continue;
        }
        let (w, h) = (img.size[0] as f32, img.size[1] as f32);
        let rect = [
            s.cell[0] as f32 / w,
            s.cell[1] as f32 / h,
            (s.cell[0] + 1) as f32 / w,
            (s.cell[1] + 1) as f32 / h,
        ];
        let lit = k + 1 == shown;
        outlines.push(Outline {
            rect,
            color: if lit {
                palette::SELECTED
            } else {
                palette::REFLECTED
            },
            width: if lit {
                3.0
            } else {
                1.5
            },
            outside: false,
        });
    }
    outlines
}

/// The iteration shown, as the loop computes it: its step, level, cell, the cell's depth and the ray's, where it
/// reaches the depth and leaves the cell, and its result; at the end, why the trace ended and its validity.
fn trace_rows(t: &trace::Trace, stepper: &Stepper) -> Vec<(String, String, Color32)> {
    let tested = stepper.tested();
    let current = tested.map(|(_, s)| s);
    let shown = tested.map_or(0, |(k, _)| k + 1);
    let mut rows: Vec<(String, String, Color32)> = Vec::new();
    let row = |rows: &mut Vec<(String, String, Color32)>, name: &str, v: String, c: Color32| {
        rows.push((name.to_string(), v, c))
    };
    if let Some(s) = current {
        row(
            &mut rows,
            tr("demos-step"),
            format!("{shown} / {}", t.steps.len()),
            palette::SELECTED,
        );
        row(
            &mut rows,
            tr("demos-level"),
            format!("{}", s.level),
            palette::SELECTED,
        );
        row(
            &mut rows,
            tr("demos-cell"),
            format!("({}, {})", s.cell[0], s.cell[1]),
            palette::SELECTED,
        );
        row(
            &mut rows,
            tr("demos-cell_depth"),
            format!("{:.5}", s.cell_depth),
            palette::THEME_ACCENT,
        );
        row(
            &mut rows,
            tr("demos-ray_depth"),
            format!("{:.5}", (t.screen_pos + t.screen_ray * s.t).z),
            palette::REFLECTED,
        );
        row(
            &mut rows,
            tr("demos-depth_t"),
            format!("{:.5}", s.depth_t),
            palette::INCIDENT,
        );
        row(
            &mut rows,
            tr("demos-edge_t"),
            format!("{:.5}", s.edge_t),
            palette::AXIS_Z,
        );
        let result = if s.held {
            tr("demos-held")
        } else if s.hit {
            tr("demos-hit_down")
        } else {
            tr("demos-miss_up")
        };
        row(
            &mut rows,
            tr("demos-result"),
            result.to_string(),
            palette::SELECTED,
        );
    }
    if stepper.i == t.steps.len() {
        let exit = match t.exit {
            Exit::Hit => tr("demos-exit_hit"),
            Exit::Steps => tr("demos-exit_steps"),
            Exit::Screen => tr("demos-exit_screen"),
            Exit::NotTraced => tr("demos-exit_none"),
        };
        row(
            &mut rows,
            tr("demos-exit"),
            exit.to_string(),
            palette::REFLECTED,
        );
        row(
            &mut rows,
            tr("demos-validity"),
            format!("{:.3}", t.validity),
            palette::REFLECTED,
        );
    }
    rows
}

fn rows_readout(title: String, subtitle: &str, rows: Vec<(String, String, Color32)>) -> Readout {
    Readout {
        title,
        subtitle: subtitle.into(),
        formula: None,
        formula_lit: None,
        rows,
    }
}

fn v3(v: Vec3) -> String {
    format!("({:.3}, {:.3}, {:.3})", v.x, v.y, v.z)
}

/// What the pre-pass stored at P, decoded, and its position rebuilt from its depth.
fn inputs_demo(state: &ShotState, content: &mut FrameContent) {
    // P alone: its depth and normal-roughness, read back from the GPU.
    let screen = cpu::View::of(&content.uniforms.camera);
    let pixel = picked(state, &screen, &content.objects);
    let depth = cpu::texel(
        content,
        "godot_ssr.inputs.depth",
        Source::image("hiz", 0),
        pixel,
    );
    let nr = cpu::texel(
        content,
        "godot_ssr.inputs.normal",
        Source::image("halfNormal", 0),
        pixel,
    );
    let (Some((pixel, depth)), Some((_, nr))) = (depth, nr) else {
        return;
    };
    let depth = depth.x;
    let uv = Vec2::new(
        (pixel[0] as f32 + 0.5) / screen.size[0] as f32,
        (pixel[1] as f32 + 0.5) / screen.size[1] as f32,
    );
    let world = screen.world_of(uv.extend(depth));
    content.draw.point(world, 7.0, palette::SELECTED, 1.0);
    let normal = (nr.truncate() * 2.0 - Vec3::ONE).normalize_or_zero();
    let world_normal = screen.world_to_view.inverse().transform_vector3(normal);
    content.draw.segment(
        world,
        world + world_normal * 0.5,
        palette::AXIS_Y,
        1.0,
        3.0,
        0,
    );
    let view = screen.world_to_view.transform_point3(world);
    let rows = vec![
        (
            tr("demos-ndc").to_string(),
            format!("({:.4}, {:.4})", uv.x, uv.y),
            palette::SELECTED,
        ),
        (
            tr("demos-depth").to_string(),
            format!("{depth:.5}"),
            palette::THEME_ACCENT,
        ),
        (
            tr("demos-view_pos").to_string(),
            v3(view),
            palette::SELECTED,
        ),
        (
            tr("demos-world_pos").to_string(),
            v3(world),
            palette::SELECTED,
        ),
        (
            tr("demos-view_normal").to_string(),
            v3(normal),
            palette::AXIS_Y,
        ),
        (
            tr("demos-roughness_byte").to_string(),
            format!("{}", (nr.w * 255.0).round() as u32),
            palette::INCIDENT,
        ),
        (
            tr("demos-roughness").to_string(),
            format!("{:.3}", trace::decode_roughness(nr.w)),
            palette::INCIDENT,
        ),
    ];
    content.readout = Some(rows_readout(
        tr_with("demos-inputs_title", lab::args!(x = pixel[0], y = pixel[1])),
        tr("demos-inputs_subtitle"),
        rows,
    ));
}

/// A level of the pyramid, P's cell on it, and that cell's footprint at level 0.
fn pyramid_demo(state: &ShotState, content: &mut FrameContent) {
    // The level shown, whole, and P's depth, read back from the GPU.
    let screen = cpu::View::of(&content.uniforms.camera);
    let pixel = picked(state, &screen, &content.objects);
    let level =
        (state.p("hizLevel").round().max(0.0) as usize).min(gpu::mipmaps(screen.size) as usize - 1);
    let p_depth = cpu::texel(
        content,
        "godot_ssr.pyramid.depth",
        Source::image("hiz", 0),
        pixel,
    );
    let img = cpu::level(
        content,
        "godot_ssr.pyramid.level",
        Source::image("hiz", level as u32),
    );
    let (Some((pixel, p_depth)), Some(img)) = (p_depth, img) else {
        return;
    };
    let (p_depth, img) = (p_depth.x, &img);
    let cell = [
        (pixel[0] >> level).min(img.size[0] as i32 - 1),
        (pixel[1] >> level).min(img.size[1] as i32 - 1),
    ];
    let (w, h) = (img.size[0] as f32, img.size[1] as f32);
    let outline = Outline {
        rect: [
            cell[0] as f32 / w,
            cell[1] as f32 / h,
            (cell[0] + 1) as f32 / w,
            (cell[1] + 1) as f32 / h,
        ],
        color: palette::SELECTED,
        width: 3.0,
        outside: false,
    };
    let start = screen.world_at(pixel, p_depth);
    // The field view over this one draws this view's whole pyramid: read it back for it (a data link, lab::fetch).
    let room = 1.0 - covered(state);
    if room < 1.0 {
        cpu::screen(content);
    }
    let title = tr_with(
        "demos-pyramid_title",
        lab::args!(level = level, width = img.size[0], height = img.size[1]),
    );
    // The marker is y up (inset.rs); SSR pixel rows run down.
    let marker = Marker {
        at: [
            (pixel[0] as f32 + 0.5) / screen.size[0] as f32,
            1.0 - (pixel[1] as f32 + 0.5) / screen.size[1] as f32,
        ],
        color: palette::SELECTED,
        label: "P".into(),
    };
    if room > 0.5 {
        content.insets.push(PictureInset {
            marker: Some(marker),
            outlines: vec![outline],
            link: Some(Link {
                from: start,
                outline: 0,
            }),
            ..PictureInset::new(
                title,
                level_picture(img, format!("ssr-pyramid-{level}")),
                [480.0, 480.0 * h / w],
            )
        });
    }
    content.draw.point(start, 6.0, palette::SELECTED, room);
    let side = 1i32 << level;
    let rows = vec![
        (
            tr("demos-p_cell").to_string(),
            format!("({}, {})", cell[0], cell[1]),
            palette::SELECTED,
        ),
        (
            tr("demos-cell_depth").to_string(),
            format!("{:.5}", img.load(cell).x),
            palette::THEME_ACCENT,
        ),
        (
            tr("demos-p_depth").to_string(),
            format!("{p_depth:.5}"),
            palette::SELECTED,
        ),
        (
            tr("demos-footprint").to_string(),
            format!("{side} x {side}"),
            palette::THEME_ACCENT,
        ),
    ];
    content.readout = Some(rows_readout(
        tr_with(
            "demos-pyramid_title",
            lab::args!(level = level, width = img.size[0], height = img.size[1]),
        ),
        tr("demos-pyramid_subtitle"),
        rows,
    ));
}

/// A chart as pixels: `w` x `h`, a dark ground.
fn chart(w: usize, h: usize) -> Vec<u8> {
    let mut rgba = vec![0u8; w * h * 4];
    for px in rgba.chunks_mut(4) {
        px.copy_from_slice(&[18, 24, 34, 255]);
    }
    rgba
}

fn plot(rgba: &mut [u8], w: usize, h: usize, x: i32, y: i32, c: Color32) {
    for dy in -1..=1 {
        for dx in -1..=1 {
            let (px, py) = (x + dx, y + dy);
            if px >= 0 && py >= 0 && (px as usize) < w && (py as usize) < h {
                let i = (py as usize * w + px as usize) * 4;
                rgba[i..i + 4].copy_from_slice(&[c.r(), c.g(), c.b(), 255]);
            }
        }
    }
}

/// P's validity, term by term, and the fade curve with P's ray length on it.
fn hit_demo(state: &ShotState, content: &mut FrameContent) {
    let Some(screen) = cpu::screen(content) else {
        return;
    };
    let pixel = picked(state, &screen, &content.objects);
    let last = Image::new([1, 1], 4);
    let (fade_in, fade_out) = (state.p("ssrFadeIn"), state.p("ssrFadeOut"));
    let t = screen
        .inputs(
            &last,
            state.p("ssrMaxSteps").round() as i32,
            fade_in,
            fade_out,
            state.p("ssrDepthTolerance"),
        )
        .trace(pixel);
    let names = [
        tr("demos-term_edge"),
        tr("demos-term_back"),
        tr("demos-term_confidence"),
        tr("demos-term_margin"),
        tr("demos-term_fade"),
    ];
    let mut rows: Vec<(String, String, Color32)> = names
        .iter()
        .zip(t.terms)
        .map(|(n, v)| (n.to_string(), format!("{v:.3}"), palette::THEME_ACCENT))
        .collect();
    rows.push((
        tr("demos-ray_len").to_string(),
        format!("{:.3}", t.ray_len),
        palette::REFLECTED,
    ));
    rows.push((
        tr("demos-validity").to_string(),
        format!("{:.3}", t.validity),
        palette::SELECTED,
    ));
    content.readout = Some(rows_readout(
        tr_with("demos-hit_title", lab::args!(x = pixel[0], y = pixel[1])),
        tr("demos-hit_subtitle"),
        rows,
    ));
    // The fade curve over the ray's screen length, 0 to 1, its peak and P's length marked.
    let (w, h) = (360usize, 130usize);
    let (rgba, peak, at) = fade_chart((w, h), (fade_in, fade_out), t.ray_len);
    // The circle and the line told apart in the inset's own lines, in their colours (a marker's label would hide the curve).
    let marker = Marker {
        at: [at, peak],
        color: palette::SELECTED,
        label: String::new(),
    };
    let lines = vec![
        (
            tr_with(
                "demos-fade_peak",
                lab::args!(peak = format!("{peak:.2}"), at = format!("{at:.2}")),
            ),
            palette::SELECTED,
        ),
        (
            tr_with(
                "demos-fade_p",
                lab::args!(
                    len = format!("{:.2}", t.ray_len),
                    fade = format!(
                        "{:.2}",
                        trace::fade(t.ray_len.clamp(0.0, 1.0), fade_in, fade_out)
                    )
                ),
            ),
            palette::REFLECTED,
        ),
    ];
    let picture = Picture::Pixels {
        key: "ssr-fade-chart".into(),
        size: [w, h],
        rgba: Arc::new(rgba),
    };
    content.insets.push(PictureInset {
        marker: Some(marker),
        x_axis: tr("demos-ray_len").to_string(),
        y_axis: tr("demos-fade_axis").to_string(),
        lines,
        ..PictureInset::new(tr("demos-fade_chart"), picture, [360.0, 130.0])
    });
}

/// The fade curve over the ray's screen length, 0 to 1, and P's length (`ray_len`) marked: the chart, and where
/// the curve peaks (its value and its length).
fn fade_chart(
    (w, h): (usize, usize),
    (fade_in, fade_out): (f32, f32),
    ray_len: f32,
) -> (Vec<u8>, f32, f32) {
    let mut rgba = chart(w, h);
    let (mut peak, mut at) = (0.0f32, 0.0f32);
    for x in 0..w {
        let l = x as f32 / (w - 1) as f32;
        let f = trace::fade(l, fade_in, fade_out);
        if f > peak {
            peak = f;
            at = l;
        }
        plot(
            &mut rgba,
            w,
            h,
            x as i32,
            ((1.0 - f) * (h - 1) as f32) as i32,
            palette::THEME_ACCENT,
        );
    }
    let mx = (ray_len.clamp(0.0, 1.0) * (w - 1) as f32) as i32;
    for y in 0..h as i32 {
        plot(&mut rgba, w, h, mx, y, palette::REFLECTED);
    }
    (rgba, peak, at)
}

/// P's roughness as a cone, the circle inside it where the ray ends, and the blur level.
fn blur_demo(state: &ShotState, content: &mut FrameContent) {
    let Some(screen) = cpu::screen(content) else {
        return;
    };
    let pixel = picked(state, &screen, &content.objects);
    let last = Image::new([1, 1], 4);
    let t = screen
        .inputs(
            &last,
            state.p("ssrMaxSteps").round() as i32,
            state.p("ssrFadeIn"),
            state.p("ssrFadeOut"),
            state.p("ssrDepthTolerance"),
        )
        .trace(pixel);
    let largest = screen.size[0].max(screen.size[1]) as f32;
    let angle = t.roughness.min(0.999) * std::f32::consts::PI * 0.5;
    let a = 2.0 * angle.tan() * t.ray_len;
    let radius = if t.ray_len > 0.0 {
        (a * ((a * a + 4.0 * t.ray_len * t.ray_len).sqrt() - a)) / (4.0 * t.ray_len)
    } else {
        0.0
    };
    let rows = vec![
        (
            tr("demos-roughness").to_string(),
            format!("{:.3}", t.roughness),
            palette::INCIDENT,
        ),
        (
            tr("demos-cone_angle").to_string(),
            format!("{:.1}°", angle.to_degrees()),
            palette::INCIDENT,
        ),
        (
            tr("demos-ray_len").to_string(),
            format!("{:.3}", t.ray_len),
            palette::REFLECTED,
        ),
        (
            tr("demos-blur_radius").to_string(),
            format!("{:.1} px", radius * largest),
            palette::THEME_ACCENT,
        ),
        (
            tr("demos-mip_level").to_string(),
            format!("{:.2}", t.mip_level),
            palette::SELECTED,
        ),
    ];
    content.readout = Some(rows_readout(
        tr_with("demos-blur_title", lab::args!(x = pixel[0], y = pixel[1])),
        tr("demos-blur_subtitle"),
        rows,
    ));
    // The cone from its apex (left) along the ray (right), the circle inside it at its end.
    let (w, h) = (360usize, 220usize);
    let rgba = cone_chart((w, h), angle, (t.ray_len, radius));
    content.insets.push(PictureInset::new(
        tr("demos-cone_chart").to_string(),
        Picture::Pixels {
            key: "ssr-cone-chart".into(),
            size: [w, h],
            rgba: Arc::new(rgba),
        },
        [360.0, 220.0],
    ));
}

/// The cone from its apex (left) along the ray (right), the circle inside it at its end: one scale for both, the
/// cone's length across the chart, its end inside it.
fn cone_chart((w, h): (usize, usize), angle: f32, (ray_len, radius): (f32, f32)) -> Vec<u8> {
    let mut rgba = chart(w, h);
    // One scale for the cone and the circle: the cone's length across the chart, its end inside it.
    let room = (w - 40) as f32;
    let scale =
        (room / ray_len.max(1e-6)).min((h as f32 * 0.5 - 6.0) / (angle.tan() * ray_len).max(1e-6));
    let len = ray_len * scale;
    let half = angle.tan() * len;
    let (ax, ay) = (20.0f32, h as f32 * 0.5);
    for k in 0..400 {
        let f = k as f32 / 399.0;
        plot(
            &mut rgba,
            w,
            h,
            (ax + f * len) as i32,
            (ay - f * half) as i32,
            palette::INCIDENT,
        );
        plot(
            &mut rgba,
            w,
            h,
            (ax + f * len) as i32,
            (ay + f * half) as i32,
            palette::INCIDENT,
        );
    }
    let r = radius * scale;
    for k in 0..720 {
        let a = k as f32 / 720.0 * std::f32::consts::TAU;
        plot(
            &mut rgba,
            w,
            h,
            (ax + len - r + r * a.cos()) as i32,
            (ay + r * a.sin()) as i32,
            palette::SELECTED,
        );
    }
    rgba
}
