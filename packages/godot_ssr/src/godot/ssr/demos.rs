//! What each demonstration of the SSR investigation shows: the room drawn with
//! Godot's SSR (the program's passes, gpu.rs, and the opaque pass,
//! shaders/ssr_shading.wgsl), its materials set from the shot's values.

use super::{cpu, gpu};
use lab::fetch::Source;
use lab::utility::height_field::{self, HeightField, Placement};
use super::shot::Demo;
use super::trace::{self, Exit, Image};
use crate::godot::ssr::text::{tr, tr_with};
use egui::Color32;
use glam::{Vec2, Vec3};
use lab::frame::{self, FrameContent, Readout, Statics};
use lab::layout::FrameUniforms;
use lab::palette;
use lab::scene::ids;
use lab::shot::ShotState;
use lab::utility::inset::{Link, Marker, Outline, Picture, PictureInset};
use std::sync::Arc;

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
    lab::camera::OrbitCamera { target: [0.0; 3], yaw: 0.0, pitch: 0.0, distance: REST, fov: main.fov }
}

/// The views over the main one for `state`, back to front: each shown as far as its `…Show` value.
pub fn views(state: &ShotState) -> Vec<lab::investigation::OverView> {
    let demo = Demo::of(state.demo);
    let mut out = Vec::new();
    for (name, show, demos) in [(FIELD, "fieldShow", &[Demo::Pyramid, Demo::Trace][..]), (PROFILE, "profileShow", &[Demo::Trace][..])] {
        let alpha = state.view_state(name).p(show).clamp(0.0, 1.0);
        if alpha > 0.0 && demos.contains(&demo) {
            out.push(lab::investigation::OverView { name, state: state.clone(), camera: state.own_camera(name, rest_camera(&state.camera)), alpha });
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
    ROOM.get_or_init(|| lab::frame::Statics::prepare(lab::scene::SceneId::STUDY_ROOM).expect("the study room").scene.records())
}

/// A picture view's frame: its canvas scene (`statics`), its own camera, and a dark room around it.
fn canvas_content(state: &ShotState, statics: &Statics, camera: &lab::camera::OrbitCamera, viewport: [f32; 2], pixel_scale: f32, time: f32) -> FrameContent {
    let objects = statics.scene.records();
    let lighting = statics.scene.lighting;
    let uniforms = FrameUniforms {
        camera: camera.camera_data(viewport),
        light_center_radiance: lighting.light.center.extend(lighting.light.radiance).to_array(),
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
    content.draw.cuboid(Vec3::ZERO, [Vec3::X * 60.0, Vec3::Y * 60.0, Vec3::Z * 60.0], Color32::from_rgb(8, 11, 16), 1.0);
    content
}

/// The main view's trace of P, on the main view's data (for a picture view).
fn main_trace(state: &ShotState, screen: &cpu::Screen) -> ([i32; 2], trace::Trace) {
    let pixel = picked(state, screen, room_objects());
    let last = Image::new([1, 1], 4);
    let inputs = screen.inputs(&last, state.p("ssrMaxSteps").round() as i32, state.p("ssrFadeIn"), state.p("ssrFadeOut"), state.p("ssrDepthTolerance"));
    (pixel, inputs.trace(pixel))
}

/// The field view: the main view's depth map as a height field, from its own camera.
fn field_view(state: &ShotState, statics: &Statics, viewport: [f32; 2], pixel_scale: f32, time: f32) -> FrameContent {
    let own = state.view_state(FIELD);
    let camera = state.own_camera(FIELD, rest_camera(&state.camera));
    let mut content = canvas_content(&own, statics, &camera, viewport, pixel_scale, time);
    let rest = rest_camera(&state.camera).camera_data(viewport);
    // The main view's data (a data link), seen as the main camera saw it.
    let Some(screen) = cpu::screen_in(lab::view::ViewId::MAIN, &state.camera.camera_data(viewport)) else {
        return content;
    };
    if Demo::of(state.demo) == Demo::Trace {
        let (pixel, t) = main_trace(state, &screen);
        let stepper = Stepper::new(&t, state.p("traceStep"));
        let level = stepper.level();
        // The columns of the level shown, the cell tested lit (blocking: the hit colour), the ray over them.
        let lit: Vec<([i32; 2], Color32)> = stepper.tested().map(|(_, s)| field_cells(s, level).into_iter().map(|c| (c, if s.hit { palette::INCIDENT } else { palette::SELECTED })).collect()).unwrap_or_default();
        let (w, h) = (screen.size[0] as f32, screen.size[1] as f32);
        let ends = [t.screen_pos, t.screen_pos + t.screen_ray * t.steps.last().map_or(0.0, |s| s.next_t)];
        let focus = [ends[0].x.min(ends[1].x) * w, ends[0].y.min(ends[1].y) * h, ends[0].x.max(ends[1].x) * w, ends[0].y.max(ends[1].y) * h];
        let f = field(&own, &mut content, &rest, &screen, &FieldView { pixel, focus, level, lit: &lit });
        let top = field_top(&screen, pixel);
        let over = |tt: f32| {
            let s = t.screen_pos + t.screen_ray * tt;
            f.point_uv(Vec2::new(s.x, s.y), s.z / top)
        };
        let ray: Vec<Vec3> = std::iter::once(0.0).chain(stepper.path()).map(over).collect();
        if ray.len() > 1 {
            content.draw.polyline(&ray, palette::REFLECTED, 1.0, 3.0);
            content.draw.point(*ray.last().expect("a point"), 6.0, palette::REFLECTED, 1.0);
        }
    } else {
        let pixel = picked(state, &screen, room_objects());
        let shown = own.p("fieldLevel");
        let at = (shown.round().max(0.0) as usize).min(screen.hiz.len() - 1);
        let cell = [pixel[0] >> at, pixel[1] >> at];
        let focus = [pixel[0] as f32 - 16.0, pixel[1] as f32 - 16.0, pixel[0] as f32 + 16.0, pixel[1] as f32 + 16.0];
        field(&own, &mut content, &rest, &screen, &FieldView { pixel, focus, level: shown, lit: &[(cell, palette::SELECTED)] });
    }
    content
}

/// The cross-section view: the main view's trace along its ray, from its own camera.
fn profile_view(state: &ShotState, statics: &Statics, viewport: [f32; 2], pixel_scale: f32, time: f32) -> FrameContent {
    let own = state.view_state(PROFILE);
    let camera = state.own_camera(PROFILE, rest_camera(&state.camera));
    let mut content = canvas_content(&own, statics, &camera, viewport, pixel_scale, time);
    let rest = rest_camera(&state.camera).camera_data(viewport);
    let Some(screen) = cpu::screen_in(lab::view::ViewId::MAIN, &state.camera.camera_data(viewport)) else {
        return content;
    };
    let (_, t) = main_trace(state, &screen);
    profile(&mut content, &rest, &screen, &t, &Stepper::new(&t, state.p("traceStep")));
    content
}

pub fn build_frame(state: &ShotState, statics: &Statics, viewport: [f32; 2], pixel_scale: f32, time: f32) -> FrameContent {
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
        light_center_radiance: lighting.light.center.extend(lighting.light.radiance).to_array(),
        light_axis_u: lighting.light.axis_u.extend(0.0).to_array(),
        light_axis_v: lighting.light.axis_v.extend(0.0).to_array(),
        light_color: lighting.light.color.extend(1.0).to_array(),
        ambient_color: lighting.ambient.extend(1.0).to_array(),
        floor_params: [objects[ids::FLOOR].albedo_roughness[3], lighting.f0, 0.0, 1.0],
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
pub fn picked(state: &ShotState, screen: &cpu::View, objects: &[lab::layout::ObjectData]) -> [i32; 2] {
    let (x, y) = (state.p("pickX"), state.p("pickY"));
    if x >= 0.0 && y >= 0.0 {
        let (w, h) = (screen.size[0] as f32, screen.size[1] as f32);
        let (rw, rh) = (state.p("pickWidth"), state.p("pickHeight"));
        // Chosen at another size: the same direction, at this one (the same pixel at that size).
        let (x, y) = if rw > 0.0 && rh > 0.0 && (rw != w || rh != h) {
            let ndc = ((x + 0.5) / rw * 2.0 - 1.0) * (rw / rh) / (w / h);
            (((ndc + 1.0) * 0.5 * w).floor().max(0.0), ((y + 0.5) / rh * h).floor())
        } else {
            (x, y)
        };
        return [(x as i32).min(screen.size[0] as i32 - 1), (y as i32).min(screen.size[1] as i32 - 1)];
    }
    let t = Vec3::from(objects[ids::TEAPOT].world_center);
    screen.pixel_of(Vec3::new(t.x, 0.0, t.z + 0.9)).unwrap_or([screen.size[0] as i32 / 2, screen.size[1] as i32 * 4 / 5])
}

fn grey(v: f32) -> [u8; 4] {
    let g = (v.clamp(0.0, 1.0) * 255.0) as u8;
    [g, g, g, 255]
}

fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Where the trace stepper is: `traceStep` = i + f, the iterations before i
/// done and iteration i f of the way, in fifths, one thing at a time (so a
/// story can say each in a sentence of its own): its cell lights as the last
/// iteration's marks fade (f 0 to 0.2); where the ray leaves the cell is
/// marked (0.2 to 0.4); where it reaches the cell's depth is marked (0.4 to
/// 0.6); the ray moves to where the iteration sends it (0.6 to 0.8); and the
/// level shown climbs or descends (0.8 to 1). A whole value shows the
/// iteration before it complete.
struct Stepper<'a> {
    steps: &'a [trace::Step],
    /// The iteration in progress (`steps.len()` once all are done).
    i: usize,
    /// Each fifth's progress, 0 to 1: the cell, its leaving mark, its depth mark, the ray's move, the level's change.
    appear: f32,
    leave: f32,
    reach: f32,
    travel: f32,
    climb: f32,
}

impl<'a> Stepper<'a> {
    fn new(t: &'a trace::Trace, step: f32) -> Stepper<'a> {
        let len = t.steps.len();
        let x = step.max(0.0);
        let i = (x.floor() as usize).min(len);
        let f = if i < len { x - i as f32 } else { 0.0 };
        let fifth = |k: f32| smooth(f * 5.0 - k);
        Stepper { steps: &t.steps, i, appear: fifth(0.0), leave: fifth(1.0), reach: fifth(2.0), travel: fifth(3.0), climb: fifth(4.0) }
    }

    fn current(&self) -> Option<&'a trace::Step> {
        self.steps.get(self.i)
    }

    fn previous(&self) -> Option<&'a trace::Step> {
        self.i.checked_sub(1).and_then(|k| self.steps.get(k))
    }

    /// The iterations whose comparison is on screen, each with its index and how strongly it shows.
    fn fading(&self) -> Vec<(usize, &'a trace::Step, f32)> {
        let mut out = Vec::new();
        if let Some(s) = self.previous() {
            let a = if self.current().is_some() { 1.0 - self.appear } else { 1.0 };
            if a > 0.0 {
                out.push((self.i - 1, s, a));
            }
        }
        if let Some(s) = self.current() && self.appear > 0.0 {
            out.push((self.i, s, self.appear));
        }
        out
    }

    /// The iteration a still names (the readout's, the lit cell's): the stronger of the two fading.
    fn tested(&self) -> Option<(usize, &'a trace::Step)> {
        self.fading().into_iter().max_by(|a, b| a.2.total_cmp(&b.2)).map(|(k, s, _)| (k, s))
    }

    /// Where along the ray (its t) the trace has got to.
    /// Where the chart's window is anchored along the ray: the start of the
    /// iteration in progress, moving on to the next start only while the level
    /// changes (so the ray visibly moves on a still chart, then the chart follows).
    fn anchor(&self) -> f32 {
        match (self.current(), self.previous()) {
            (Some(s), _) => s.t + (s.next_t - s.t) * self.climb,
            (None, Some(s)) => s.next_t,
            (None, None) => 0.0,
        }
    }

    fn tip(&self) -> f32 {
        match (self.current(), self.previous()) {
            (Some(s), _) => s.t + (s.next_t - s.t) * self.travel,
            (None, Some(s)) => s.next_t,
            (None, None) => 0.0,
        }
    }

    /// How strongly iteration `k`'s marks show (`j` 0 the leaving mark, 1 the
    /// depth mark): the iteration in progress's as their fifths come, the one
    /// before it as it fades.
    fn mark_of(&self, k: usize, j: usize, a: f32) -> f32 {
        if k == self.i { a.min(if j == 0 { self.leave } else { self.reach }) } else { a }
    }

    /// How strongly iteration `k`'s decision shows (its arrow): the iteration in progress's once the ray moves.
    fn move_of(&self, k: usize, a: f32) -> f32 {
        if k == self.i { a.min(self.travel) } else { a }
    }

    /// Whether iteration `k`'s result can be named: both its marks are up (or it is the one before).
    fn decided(&self, k: usize) -> bool {
        k != self.i || self.reach > 0.9
    }

    /// The pyramid level shown: between two levels while the ray climbs or descends (never below zero).
    fn level(&self) -> f32 {
        match (self.current(), self.previous()) {
            (Some(s), _) => {
                let (a, b) = (s.level.max(0) as f32, s.next_level.max(0) as f32);
                a + (b - a) * self.climb
            }
            (None, Some(s)) => s.next_level.max(0) as f32,
            (None, None) => 0.0,
        }
    }

    /// The ray's path as values of t: where each finished iteration sent it, then the tip.
    fn path(&self) -> Vec<f32> {
        let mut out: Vec<f32> = self.steps[..self.i].iter().map(|s| s.next_t).collect();
        if self.current().is_some() && self.travel > 0.0 {
            out.push(self.tip());
        }
        out
    }
}

/// The cells of the level the field shows (`level`, between two levels while it
/// changes) that stand for an iteration's cell: the cell itself on its own
/// level, or the four it splits into while the ray descends from it.
fn field_cells(s: &trace::Step, level: f32) -> Vec<[i32; 2]> {
    let l = level.max(0.0).floor() as i32;
    let own = s.level.max(0);
    if own <= l {
        return vec![[s.cell[0] >> (l - own), s.cell[1] >> (l - own)]];
    }
    let k = own - l;
    let n = 1 << k;
    (0..n * n).map(|j| [(s.cell[0] << k) + j % n, (s.cell[1] << k) + j / n]).collect()
}

/// What the depth map's height field shows: P (`pixel`), around which it
/// zooms (`fieldZoom`: a window framing `focus`, a texel box x0, y0, x1, y1);
/// its columns, of pyramid level `level` (between two levels, each 2 x 2
/// rises to the tallest of its parent and merges); and `lit`, cells of that
/// level coloured (texel coordinates of the level, a colour).
#[derive(Clone, Copy)]
struct FieldView<'a> {
    pixel: [i32; 2],
    focus: [f32; 4],
    level: f32,
    lit: &'a [([i32; 2], Color32)],
}

/// The depth map drawn as a height field (lab::utility::height_field), over a
/// dark ground that hides the room: each SSR texel a column as tall as its
/// reversed depth (near is tall), fading in flat, as the screen shows it
/// (`fieldShow`), then tilting back as the columns rise (`fieldMorph`).
/// What it shows is `shown`'s (`FieldView`).
fn field(state: &ShotState, content: &mut FrameContent, rest: &lab::layout::CameraData, screen: &cpu::Screen, shown: &FieldView) -> HeightField {
    let FieldView { pixel, focus, level, lit } = *shown;
    let show = 1.0;
    let morph = smooth(state.p("fieldMorph"));
    // On the plane the view's camera faces at rest; a little height even when flat, so the
    // tiles stand clear of the slab under them.
    let tilt = state.p("fieldTilt").clamp(0.0, 85.0);
    let placement = Placement::facing(rest, REST, (0.002 + 0.07 * morph) * REST / 0.25, tilt * morph, state.p("fieldTurn") * morph, 1.0 - 0.4 * morph);
    let hiz = &screen.hiz;
    let size = hiz[0].size;
    let (w, h) = (size[0] as f32, size[1] as f32);
    // The window: the whole image, or `focus` with a margin, at the image's shape (at least 40 texels across).
    let zoom = smooth(state.p("fieldZoom"));
    let across = ((focus[2] - focus[0]) * 1.3).max((focus[3] - focus[1]) * 1.3 * w / h).clamp(40.0, w);
    let close = [across, across * h / w];
    let cx = ((focus[0] + focus[2]) * 0.5).clamp(close[0] * 0.5, w - close[0] * 0.5);
    let cy = ((focus[1] + focus[3]) * 0.5).clamp(close[1] * 0.5, h - close[1] * 0.5);
    let near = [cx - close[0] * 0.5, cy - close[1] * 0.5, cx + close[0] * 0.5, cy + close[1] * 0.5];
    let window = [near[0] * zoom, near[1] * zoom, w + (near[2] - w) * zoom, h + (near[3] - h) * zoom];
    let field = HeightField { size, window, placement };
    field.base(&mut content.draw, Color32::from_rgb(20, 24, 30), show);
    let top = field_top(screen, pixel);
    // Coloured on the depth map's ramp (its inset's), so the flat field is that picture.
    let ramp = hiz[0].data.iter().copied().fold(0.0f32, f32::max).max(1e-6);
    let last = hiz.len() - 1;
    let l = (level.max(0.0).floor() as usize).min(last);
    let f = if l < last { level - l as f32 } else { 0.0 };
    let f = smooth(f.clamp(0.0, 1.0));
    let img = &hiz[l];
    let (sx, sy) = (w / img.size[0] as f32, h / img.size[1] as f32);
    for y in 0..img.size[1] as i32 {
        for x in 0..img.size[0] as i32 {
            let texels = [x as f32 * sx, y as f32 * sy, (x + 1) as f32 * sx, (y + 1) as f32 * sy];
            if !field.shows(texels) {
                continue;
            }
            let d = img.load([x, y]).x;
            // Between levels: rising to the parent's tallest.
            let d = if f > 0.0 { d + (hiz[l + 1].load([(x >> 1).min(hiz[l + 1].size[0] as i32 - 1), (y >> 1).min(hiz[l + 1].size[1] as i32 - 1)]).x - d) * f } else { d };
            if d <= 0.0 {
                continue;
            }
            let color = lit.iter().find(|(c, _)| *c == [x, y]).map_or_else(|| height_field::grey(d, ramp), |(_, c)| *c);
            field.column(&mut content.draw, texels, (d / top).min(1.0), color, show);
        }
    }
    // P, on its column.
    let p = Vec2::new(pixel[0] as f32 + 0.5, pixel[1] as f32 + 0.5);
    content.draw.point(field.point(p, (hiz[0].load(pixel).x / top).min(1.0)), 6.0, palette::SELECTED, show);
    field
}

/// A cell the ray crosses: the cell, the t where the ray enters and leaves
/// it, and its depth.
type Crossing = ([i32; 2], f32, f32, f32);

/// The cells of `level` the ray crosses from t = 0 to `end`, in order.
fn cells_along(screen: &cpu::Screen, t: &trace::Trace, level: usize, end: f32) -> Vec<Crossing> {
    let img = &screen.hiz[level];
    let size = Vec2::new(img.size[0] as f32, img.size[1] as f32);
    let mut out: Vec<Crossing> = Vec::new();
    const SAMPLES: usize = 3000;
    for k in 0..=SAMPLES {
        let tt = end * k as f32 / SAMPLES as f32;
        let p = (t.screen_pos + t.screen_ray * tt).truncate() * size;
        let cell = [p.x.floor() as i32, p.y.floor() as i32];
        if cell[0] < 0 || cell[1] < 0 || cell[0] >= img.size[0] as i32 || cell[1] >= img.size[1] as i32 {
            break;
        }
        match out.last_mut() {
            Some(last) if last.0 == cell => last.2 = tt,
            _ => out.push((cell, tt, tt, img.load(cell).x)),
        }
    }
    out
}

/// The trace as a cross-section along the ray (the usual picture of a Hi-Z
/// march), over the view (`profileShow`): across, the ray's t from P; up,
/// reversed depth (nearer is higher). The cells of the level shown stand as
/// bars as tall as their nearest depth (two levels cross-fading while the ray
/// climbs or descends), the true surface (level 0) drawn thin over them; the
/// ray is a straight line. For the iteration tested: its cell lit, where the
/// ray leaves the cell and where it reaches the cell's depth marked, whichever
/// comes first deciding (a miss climbs a level, a hit descends), and an arrow
/// to where the next iteration starts. The axes span the whole trace, so they
/// stay still while it is stepped.
fn profile(content: &mut FrameContent, rest: &lab::layout::CameraData, screen: &cpu::Screen, t: &trace::Trace, stepper: &Stepper) {
    let alpha = 1.0;
    if t.steps.is_empty() {
        return;
    }
    // On the plane the view's camera faces at rest.
    let plane = Placement::facing(rest, REST, 0.0, 0.0, 0.0, 1.0);
    let lift = REST / 0.25;
    let last_level = screen.hiz.len() - 1;
    // The whole march, with a little room after where it ends.
    let reach = t.steps.iter().map(|s| s.edge_t.max(s.next_t)).fold(0.0f32, f32::max).min(t.t_max.max(1e-6));
    let end = (reach.max(t.steps.last().map_or(0.0, |s| s.next_t)) * 1.08).max(1e-6);
    // Every level the trace visits, once: the bars of each, and the range of depths over all of them.
    let mut levels: Vec<usize> = t.steps.iter().map(|s| (s.level.max(0) as usize).min(last_level)).collect();
    levels.sort_unstable();
    levels.dedup();
    let bars_of: Vec<(usize, Vec<Crossing>)> = levels.iter().map(|&l| (l, cells_along(screen, t, l, end))).collect();
    let bars_at = |l: usize| bars_of.iter().find(|(k, _)| *k == l).map(|(_, b)| b.as_slice()).unwrap_or(&[]);
    let surface = cells_along(screen, t, 0, end);
    let ray = |tt: f32| (t.screen_pos + t.screen_ray * tt).z;
    let mut lo = ray(0.0).min(ray(end));
    let mut hi = ray(0.0).max(ray(end));
    for &(_, _, _, d) in bars_of.iter().flat_map(|(_, b)| b).chain(&surface) {
        if d > 0.0 {
            lo = lo.min(d);
            hi = hi.max(d);
        }
    }
    let pad = (hi - lo).max(1e-6) * 0.15;
    let (lo, hi) = (lo - pad, hi + pad);
    let (u0, u1, v0, v1) = (0.07, 0.93, 0.2, 0.86);
    // Across, a window around the ray's tip, eight cells of the level shown wide: it follows the tip and widens
    // as the ray climbs (both move smoothly), so a step on any level reads at the same size.
    let size = Vec2::new(screen.size[0] as f32, screen.size[1] as f32);
    let per_texel = 1.0 / (t.screen_ray.x.abs() * size.x).max(t.screen_ray.y.abs() * size.y).max(1e-9);
    let width = (8.0 * per_texel * 2f32.powf(stepper.level())).min(end);
    let t0 = (stepper.anchor() - 0.3 * width).clamp(0.0, end - width);
    let at = |tt: f32, d: f32| Vec2::new(u0 + ((tt - t0) / width).clamp(0.0, 1.0) * (u1 - u0), v1 - ((d - lo) / (hi - lo)).clamp(0.0, 1.0) * (v1 - v0));
    let inside = |tt: f32| tt >= t0 && tt <= t0 + width;
    let on = |uv: Vec2| plane.on(uv, 2e-3 * lift);
    // The chart's axes.
    plane.rect(&mut content.draw, Vec2::new(u0, v0), Vec2::new(u1, v1), -2e-3 * lift, Color32::from_rgb(16, 21, 28), alpha);
    content.draw.segment(on(Vec2::new(u0, v1)), on(Vec2::new(u1, v1)), palette::text_dim(), alpha, 1.5, frame::FLAG_ARROW);
    content.draw.segment(on(Vec2::new(u0, v1)), on(Vec2::new(u0, v0)), palette::text_dim(), alpha, 1.5, frame::FLAG_ARROW);
    // The axes' names where the axes end: pinned, so they never move.
    content.draw.text(on(Vec2::new(u1, v1 + 0.015)), tr("demos-profile_t"), palette::text_dim(), [1.0, 0.0]);
    content.draw.text(on(Vec2::new(u0, v0 - 0.012)), tr("demos-profile_depth"), palette::text_dim(), [0.0, 1.0]);
    // The level's cells along the ray: between two levels, the lower fading into the upper.
    let level = stepper.level();
    let below = (level.floor() as usize).min(last_level);
    let above = (below + 1).min(last_level);
    let mix = if above > below { level - below as f32 } else { 0.0 };
    for (l, a) in [(below, 1.0 - mix), (above, mix)] {
        if a <= 0.0 || (l == above && above == below) {
            continue;
        }
        for &(_, x0, x1, d) in bars_at(l) {
            plane.rect(&mut content.draw, at(x0, lo), at(x1, d.max(lo)), 0.0, Color32::from_rgb(92, 100, 112), alpha * 0.85 * a);
        }
    }
    // The cell each iteration on screen tests, lit on its own level, as strongly as it shows.
    for &(_, s, a) in &stepper.fading() {
        let l = (s.level.max(0) as usize).min(last_level);
        let color = if s.hit { palette::INCIDENT } else { palette::SELECTED };
        if let Some(&(_, x0, x1, d)) = bars_at(l).iter().find(|(c, ..)| *c == s.cell) {
            plane.rect(&mut content.draw, at(x0, lo), at(x1, d.max(lo)), 1e-3 * lift, color, alpha * a);
        }
    }
    // The true surface, thin, over them: only while the bars are of level 0, where they are the surface's own.
    let surface_alpha = (1.0 - level).clamp(0.0, 1.0) * 0.7;
    if surface_alpha > 0.0 {
        for &(_, a, b, d) in &surface {
            content.draw.segment(on(at(a, d)), on(at(b, d)), palette::SURFACE, alpha * surface_alpha, 1.5, 0);
        }
    }
    // The ray as far as the trace has marched it, and where it is.
    let reached = stepper.tip();
    content.draw.segment(on(at(0.0, ray(0.0))), on(at(reached, ray(reached))), palette::REFLECTED, alpha, 3.0, 0);
    content.draw.point(on(at(reached, ray(reached))), 6.0, palette::REFLECTED, alpha);
    if inside(0.0) {
        content.draw.text(on(at(0.0, ray(0.0)) + Vec2::new(0.008, -0.01)), "P", palette::SELECTED, [0.0, 1.0]);
    }
    // Each iteration's comparison, fading: where the ray leaves the cell and where it reaches the cell's depth.
    let named = stepper.tested().map(|(k, _)| k);
    for &(k, s, a) in &stepper.fading() {
        // The leaving mark, then the depth mark, each in its own fifth (one thing at a time).
        let marks = [(s.edge_t, palette::AXIS_Z, tr("demos-profile_leaves"), stepper.mark_of(k, 0, a)), (s.depth_t, palette::INCIDENT, tr("demos-profile_reaches"), stepper.mark_of(k, 1, a))];
        for (j, (tt, color, text, a)) in marks.into_iter().enumerate() {
            if a <= 0.0 {
                continue;
            }
            // Past the window's edge (a far wall's depth, or one already behind the ray): an arrow at that edge
            // pointing to it, in the mark's own row, so the comparison still reads.
            if !inside(tt) {
                let right = tt > t0 + width;
                let (edge, back) = if right { (u1, u1 - 0.05) } else { (u0, u0 + 0.05) };
                let y = v0 + 0.075 + 0.05 * j as f32;
                content.draw.segment(on(Vec2::new(back, y)), on(Vec2::new(edge, y)), color, alpha * 0.9 * a, 2.0, frame::FLAG_ARROW);
                if named == Some(k) {
                    let x = if right { back - 0.006 } else { back + 0.006 };
                    // Said to be off the chart, so the arrow does not read as pointing at nothing.
                    let text = tr_with("demos-profile_off", lab::args!(mark = text));
                    content.draw.text(on(Vec2::new(x, y - 0.015)), text, color, [if right { 1.0 } else { 0.0 }, 0.0]);
                }
                continue;
            }
            let x = at(tt, lo).x;
            content.draw.segment(on(Vec2::new(x, v1)), on(Vec2::new(x, v0)), color, alpha * 0.9 * a, 2.0, frame::FLAG_DASHED);
            content.draw.point(on(at(tt, ray(tt))), 7.0, color, alpha * a);
            // Names do not fade: only the iteration named carries them.
            if named == Some(k) {
                // Under the axis, clear of the depth axis's label above.
                // At the top of its line, the two in rows of their own so they never overlap.
                // Right of its line, or left of it in the chart's right half, so it stays inside.
                let left = x > 0.55;
                content.draw.text(on(Vec2::new(if left { x - 0.006 } else { x + 0.006 }, v0 + 0.06 + 0.05 * j as f32)), text, color, [if left { 1.0 } else { 0.0 }, 0.0]);
            }
        }
        let moving = stepper.move_of(k, a);
        if s.next_t > s.t && moving > 0.0 {
            content.draw.segment(on(at(s.t, ray(s.t))), on(at(s.next_t, ray(s.next_t))), palette::SELECTED, alpha * moving, 3.0, frame::FLAG_ARROW);
        }
    }
    // The result, once both marks are up.
    let Some((k, s)) = stepper.tested() else { return };
    if !stepper.decided(k) {
        return;
    }
    let result = if s.held { tr("demos-held") } else if s.hit { tr("demos-hit_down") } else { tr("demos-miss_up") };
    let text = tr_with("demos-profile_level", lab::args!(from = s.level, to = s.next_level, result = result));
    content.draw.text(on(Vec2::new(0.5, v0 - 0.075)), text, if s.hit { palette::INCIDENT } else { palette::SELECTED }, [0.5, 1.0]);
}

/// The reversed depth a field column of full height stands for: P's, twice
/// over, so P's column is half the height. Reversed depth grows
/// fast near the camera; the floor nearer than P is capped there (the ray
/// climbs away from it, over farther texels).
fn field_top(screen: &cpu::Screen, pixel: [i32; 2]) -> f32 {
    let max = screen.hiz[0].data.iter().copied().fold(0.0f32, f32::max).max(1e-6);
    (screen.hiz[0].load(pixel).x * 2.0).clamp(1e-6, max)
}

/// A Hi-Z level as a picture: its depths on a ramp over the room's range.
fn level_picture(level: &Image, key: String) -> Picture {
    let max = level.data.iter().copied().fold(0.0f32, f32::max).max(1e-6);
    let rgba: Vec<u8> = level.data.iter().flat_map(|d| grey((d / max).powf(0.5))).collect();
    Picture::Pixels { key, size: [level.size[0] as usize, level.size[1] as usize], rgba: Arc::new(rgba) }
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
    let inputs = screen.inputs(&last, state.p("ssrMaxSteps").round() as i32, state.p("ssrFadeIn"), state.p("ssrFadeOut"), state.p("ssrDepthTolerance"));
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
        content.draw.point(*path.last().expect("a point"), 5.0, palette::REFLECTED, room);
    }
    // The level of the iteration tested, the cells visited on it, the tested one lit.
    let tested = stepper.tested();
    let current = tested.map(|(_, s)| s);
    let shown = tested.map_or(0, |(k, _)| k + 1);
    let level = current.map_or(0, |s| s.level.max(0)) as usize;
    let img = &screen.hiz[level];
    let mut outlines = Vec::new();
    for (k, s) in t.steps.iter().take(shown).enumerate() {
        if s.level.max(0) as usize != level {
            continue;
        }
        let (w, h) = (img.size[0] as f32, img.size[1] as f32);
        let rect = [s.cell[0] as f32 / w, s.cell[1] as f32 / h, (s.cell[0] + 1) as f32 / w, (s.cell[1] + 1) as f32 / h];
        let lit = k + 1 == shown;
        outlines.push(Outline { rect, color: if lit { palette::SELECTED } else { palette::REFLECTED }, width: if lit { 3.0 } else { 1.5 }, outside: false });
    }
    let size = Vec2::new(screen.size[0] as f32, screen.size[1] as f32);
    let marker = Marker { at: [(pixel[0] as f32 + 0.5) / size.x, (pixel[1] as f32 + 0.5) / size.y], color: palette::SELECTED, label: "P".into() };
    let link = (!outlines.is_empty()).then(|| Link { from: if path.len() > 1 { at(stepper.tip()) } else { start }, outline: outlines.len() - 1 });
    let title = tr_with("demos-trace_inset", lab::args!(level = level, width = img.size[0], height = img.size[1]));
    // The level as a picture; the height field, when shown, is that picture risen.
    if room > 0.5 {
        content.insets.push(PictureInset { marker: Some(marker), outlines, link, ..PictureInset::new(title, level_picture(img, format!("ssr-trace-level-{level}")), [420.0, 420.0 * img.size[1] as f32 / img.size[0] as f32]) });
    }
    // The iteration shown, as the loop computes it.
    let mut rows: Vec<(String, String, Color32)> = Vec::new();
    let row = |rows: &mut Vec<(String, String, Color32)>, name: &str, v: String, c: Color32| rows.push((name.to_string(), v, c));
    if let Some(s) = current {
        row(&mut rows, tr("demos-step"), format!("{shown} / {}", t.steps.len()), palette::SELECTED);
        row(&mut rows, tr("demos-level"), format!("{}", s.level), palette::SELECTED);
        row(&mut rows, tr("demos-cell"), format!("({}, {})", s.cell[0], s.cell[1]), palette::SELECTED);
        row(&mut rows, tr("demos-cell_depth"), format!("{:.5}", s.cell_depth), palette::THEME_ACCENT);
        row(&mut rows, tr("demos-ray_depth"), format!("{:.5}", (t.screen_pos + t.screen_ray * s.t).z), palette::REFLECTED);
        row(&mut rows, tr("demos-depth_t"), format!("{:.5}", s.depth_t), palette::INCIDENT);
        row(&mut rows, tr("demos-edge_t"), format!("{:.5}", s.edge_t), palette::AXIS_Z);
        let result = if s.held { tr("demos-held") } else if s.hit { tr("demos-hit_down") } else { tr("demos-miss_up") };
        row(&mut rows, tr("demos-result"), result.to_string(), palette::SELECTED);
    }
    if stepper.i == t.steps.len() {
        let exit = match t.exit {
            Exit::Hit => tr("demos-exit_hit"),
            Exit::Steps => tr("demos-exit_steps"),
            Exit::Screen => tr("demos-exit_screen"),
            Exit::NotTraced => tr("demos-exit_none"),
        };
        row(&mut rows, tr("demos-exit"), exit.to_string(), palette::REFLECTED);
        row(&mut rows, tr("demos-validity"), format!("{:.3}", t.validity), palette::REFLECTED);
    }
    content.readout = Some(Readout {
        title: tr_with("demos-trace_title", lab::args!(x = pixel[0], y = pixel[1])),
        subtitle: tr("demos-trace_subtitle").into(),
        formula: None,
        formula_lit: None,
        rows,
    });
}

fn rows_readout(title: String, subtitle: &str, rows: Vec<(String, String, Color32)>) -> Readout {
    Readout { title, subtitle: subtitle.into(), formula: None, formula_lit: None, rows }
}

fn v3(v: Vec3) -> String {
    format!("({:.3}, {:.3}, {:.3})", v.x, v.y, v.z)
}

/// What the pre-pass stored at P, decoded, and its position rebuilt from its depth.
fn inputs_demo(state: &ShotState, content: &mut FrameContent) {
    // P alone: its depth and normal-roughness, read back from the GPU.
    let screen = cpu::View::of(&content.uniforms.camera);
    let pixel = picked(state, &screen, &content.objects);
    let depth = cpu::texel(content, "godot_ssr.inputs.depth", Source::image("hiz", 0), pixel);
    let nr = cpu::texel(content, "godot_ssr.inputs.normal", Source::image("halfNormal", 0), pixel);
    let (Some((pixel, depth)), Some((_, nr))) = (depth, nr) else {
        return;
    };
    let depth = depth.x;
    let uv = Vec2::new((pixel[0] as f32 + 0.5) / screen.size[0] as f32, (pixel[1] as f32 + 0.5) / screen.size[1] as f32);
    let world = screen.world_of(uv.extend(depth));
    content.draw.point(world, 7.0, palette::SELECTED, 1.0);
    let normal = (nr.truncate() * 2.0 - Vec3::ONE).normalize_or_zero();
    let world_normal = screen.world_to_view.inverse().transform_vector3(normal);
    content.draw.segment(world, world + world_normal * 0.5, palette::AXIS_Y, 1.0, 3.0, 0);
    let view = screen.world_to_view.transform_point3(world);
    let rows = vec![
        (tr("demos-ndc").to_string(), format!("({:.4}, {:.4})", uv.x, uv.y), palette::SELECTED),
        (tr("demos-depth").to_string(), format!("{depth:.5}"), palette::THEME_ACCENT),
        (tr("demos-view_pos").to_string(), v3(view), palette::SELECTED),
        (tr("demos-world_pos").to_string(), v3(world), palette::SELECTED),
        (tr("demos-view_normal").to_string(), v3(normal), palette::AXIS_Y),
        (tr("demos-roughness_byte").to_string(), format!("{}", (nr.w * 255.0).round() as u32), palette::INCIDENT),
        (tr("demos-roughness").to_string(), format!("{:.3}", trace::decode_roughness(nr.w)), palette::INCIDENT),
    ];
    content.readout = Some(rows_readout(tr_with("demos-inputs_title", lab::args!(x = pixel[0], y = pixel[1])), tr("demos-inputs_subtitle"), rows));
}

/// A level of the pyramid, P's cell on it, and that cell's footprint at level 0.
fn pyramid_demo(state: &ShotState, content: &mut FrameContent) {
    // The level shown, whole, and P's depth, read back from the GPU.
    let screen = cpu::View::of(&content.uniforms.camera);
    let pixel = picked(state, &screen, &content.objects);
    let level = (state.p("hizLevel").round().max(0.0) as usize).min(gpu::mipmaps(screen.size) as usize - 1);
    let p_depth = cpu::texel(content, "godot_ssr.pyramid.depth", Source::image("hiz", 0), pixel);
    let img = cpu::level(content, "godot_ssr.pyramid.level", Source::image("hiz", level as u32));
    let (Some((pixel, p_depth)), Some(img)) = (p_depth, img) else {
        return;
    };
    let (p_depth, img) = (p_depth.x, &img);
    let cell = [(pixel[0] >> level).min(img.size[0] as i32 - 1), (pixel[1] >> level).min(img.size[1] as i32 - 1)];
    let (w, h) = (img.size[0] as f32, img.size[1] as f32);
    let outline = Outline { rect: [cell[0] as f32 / w, cell[1] as f32 / h, (cell[0] + 1) as f32 / w, (cell[1] + 1) as f32 / h], color: palette::SELECTED, width: 3.0, outside: false };
    let start = screen.world_at(pixel, p_depth);
    // The field view over this one draws this view's whole pyramid: read it back for it (a data link, lab::fetch).
    let room = 1.0 - covered(state);
    if room < 1.0 {
        cpu::screen(content);
    }
    let title = tr_with("demos-pyramid_title", lab::args!(level = level, width = img.size[0], height = img.size[1]));
    let marker = Marker { at: [(pixel[0] as f32 + 0.5) / screen.size[0] as f32, (pixel[1] as f32 + 0.5) / screen.size[1] as f32], color: palette::SELECTED, label: "P".into() };
    if room > 0.5 {
        content.insets.push(PictureInset { marker: Some(marker), outlines: vec![outline], link: Some(Link { from: start, outline: 0 }), ..PictureInset::new(title, level_picture(img, format!("ssr-pyramid-{level}")), [480.0, 480.0 * h / w]) });
    }
    content.draw.point(start, 6.0, palette::SELECTED, room);
    let side = 1i32 << level;
    let rows = vec![
        (tr("demos-p_cell").to_string(), format!("({}, {})", cell[0], cell[1]), palette::SELECTED),
        (tr("demos-cell_depth").to_string(), format!("{:.5}", img.load(cell).x), palette::THEME_ACCENT),
        (tr("demos-p_depth").to_string(), format!("{p_depth:.5}"), palette::SELECTED),
        (tr("demos-footprint").to_string(), format!("{side} x {side}"), palette::THEME_ACCENT),
    ];
    content.readout = Some(rows_readout(tr_with("demos-pyramid_title", lab::args!(level = level, width = img.size[0], height = img.size[1])), tr("demos-pyramid_subtitle"), rows));
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
    let t = screen.inputs(&last, state.p("ssrMaxSteps").round() as i32, fade_in, fade_out, state.p("ssrDepthTolerance")).trace(pixel);
    let names = [tr("demos-term_edge"), tr("demos-term_back"), tr("demos-term_confidence"), tr("demos-term_margin"), tr("demos-term_fade")];
    let mut rows: Vec<(String, String, Color32)> = names.iter().zip(t.terms).map(|(n, v)| (n.to_string(), format!("{v:.3}"), palette::THEME_ACCENT)).collect();
    rows.push((tr("demos-ray_len").to_string(), format!("{:.3}", t.ray_len), palette::REFLECTED));
    rows.push((tr("demos-validity").to_string(), format!("{:.3}", t.validity), palette::SELECTED));
    content.readout = Some(rows_readout(tr_with("demos-hit_title", lab::args!(x = pixel[0], y = pixel[1])), tr("demos-hit_subtitle"), rows));
    // The fade curve over the ray's screen length, 0 to 1, its peak and P's length marked.
    let (w, h) = (360usize, 130usize);
    let mut rgba = chart(w, h);
    let (mut peak, mut at) = (0.0f32, 0.0f32);
    for x in 0..w {
        let l = x as f32 / (w - 1) as f32;
        let f = trace::fade(l, fade_in, fade_out);
        if f > peak {
            peak = f;
            at = l;
        }
        plot(&mut rgba, w, h, x as i32, ((1.0 - f) * (h - 1) as f32) as i32, palette::THEME_ACCENT);
    }
    let mx = (t.ray_len.clamp(0.0, 1.0) * (w - 1) as f32) as i32;
    for y in 0..h as i32 {
        plot(&mut rgba, w, h, mx, y, palette::REFLECTED);
    }
    // The circle and the line told apart in the inset's own lines, in their colours (a marker's label would hide the curve).
    let marker = Marker { at: [at, peak], color: palette::SELECTED, label: String::new() };
    let lines = vec![
        (tr_with("demos-fade_peak", lab::args!(peak = format!("{peak:.2}"), at = format!("{at:.2}"))), palette::SELECTED),
        (tr_with("demos-fade_p", lab::args!(len = format!("{:.2}", t.ray_len), fade = format!("{:.2}", trace::fade(t.ray_len.clamp(0.0, 1.0), fade_in, fade_out)))), palette::REFLECTED),
    ];
    let picture = Picture::Pixels { key: "ssr-fade-chart".into(), size: [w, h], rgba: Arc::new(rgba) };
    content.insets.push(PictureInset { marker: Some(marker), x_axis: tr("demos-ray_len").to_string(), y_axis: tr("demos-fade_axis").to_string(), lines, ..PictureInset::new(tr("demos-fade_chart"), picture, [360.0, 130.0]) });
}

/// P's roughness as a cone, the circle inside it where the ray ends, and the blur level.
fn blur_demo(state: &ShotState, content: &mut FrameContent) {
    let Some(screen) = cpu::screen(content) else {
        return;
    };
    let pixel = picked(state, &screen, &content.objects);
    let last = Image::new([1, 1], 4);
    let t = screen.inputs(&last, state.p("ssrMaxSteps").round() as i32, state.p("ssrFadeIn"), state.p("ssrFadeOut"), state.p("ssrDepthTolerance")).trace(pixel);
    let largest = screen.size[0].max(screen.size[1]) as f32;
    let angle = t.roughness.min(0.999) * std::f32::consts::PI * 0.5;
    let a = 2.0 * angle.tan() * t.ray_len;
    let radius = if t.ray_len > 0.0 { (a * ((a * a + 4.0 * t.ray_len * t.ray_len).sqrt() - a)) / (4.0 * t.ray_len) } else { 0.0 };
    let rows = vec![
        (tr("demos-roughness").to_string(), format!("{:.3}", t.roughness), palette::INCIDENT),
        (tr("demos-cone_angle").to_string(), format!("{:.1}°", angle.to_degrees()), palette::INCIDENT),
        (tr("demos-ray_len").to_string(), format!("{:.3}", t.ray_len), palette::REFLECTED),
        (tr("demos-blur_radius").to_string(), format!("{:.1} px", radius * largest), palette::THEME_ACCENT),
        (tr("demos-mip_level").to_string(), format!("{:.2}", t.mip_level), palette::SELECTED),
    ];
    content.readout = Some(rows_readout(tr_with("demos-blur_title", lab::args!(x = pixel[0], y = pixel[1])), tr("demos-blur_subtitle"), rows));
    // The cone from its apex (left) along the ray (right), the circle inside it at its end.
    let (w, h) = (360usize, 220usize);
    let mut rgba = chart(w, h);
    // One scale for the cone and the circle: the cone's length across the chart, its end inside it.
    let room = (w - 40) as f32;
    let scale = (room / t.ray_len.max(1e-6)).min((h as f32 * 0.5 - 6.0) / (angle.tan() * t.ray_len).max(1e-6));
    let len = t.ray_len * scale;
    let half = angle.tan() * len;
    let (ax, ay) = (20.0f32, h as f32 * 0.5);
    for k in 0..400 {
        let f = k as f32 / 399.0;
        plot(&mut rgba, w, h, (ax + f * len) as i32, (ay - f * half) as i32, palette::INCIDENT);
        plot(&mut rgba, w, h, (ax + f * len) as i32, (ay + f * half) as i32, palette::INCIDENT);
    }
    let r = radius * scale;
    for k in 0..720 {
        let a = k as f32 / 720.0 * std::f32::consts::TAU;
        plot(&mut rgba, w, h, (ax + len - r + r * a.cos()) as i32, (ay + r * a.sin()) as i32, palette::SELECTED);
    }
    content.insets.push(PictureInset::new(tr("demos-cone_chart").to_string(), Picture::Pixels { key: "ssr-cone-chart".into(), size: [w, h], rgba: Arc::new(rgba) }, [360.0, 220.0]));
}
