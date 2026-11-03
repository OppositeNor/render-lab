//! What each demonstration of the SSR investigation shows: the room drawn with
//! Godot's SSR (the program's passes, gpu.rs, and the opaque pass,
//! shaders/ssr_shading.wgsl), its materials set from the shot's values.

use super::{cpu, gpu};
use lab::fetch::Source;
use super::shot::Demo;
use super::trace::{self, Exit, Image};
use crate::godot::ssr::text::{tr, tr_with};
use egui::Color32;
use glam::{Vec2, Vec3};
use lab::frame::{FrameContent, Readout, Statics};
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

pub fn build_frame(state: &ShotState, statics: &Statics, viewport: [f32; 2], pixel_scale: f32, time: f32) -> FrameContent {
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
        return [(x as i32).min(screen.size[0] as i32 - 1), (y as i32).min(screen.size[1] as i32 - 1)];
    }
    let t = Vec3::from(objects[ids::TEAPOT].world_center);
    screen.pixel_of(Vec3::new(t.x, 0.0, t.z + 0.9)).unwrap_or([screen.size[0] as i32 / 2, screen.size[1] as i32 * 4 / 5])
}

fn grey(v: f32) -> [u8; 4] {
    let g = (v.clamp(0.0, 1.0) * 255.0) as u8;
    [g, g, g, 255]
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
    let shown = (state.p("traceStep").round().max(0.0) as usize).min(t.steps.len());
    let start = screen.surface(pixel);
    content.draw.point(start, 6.0, palette::SELECTED, 1.0);
    // The ray in the room: the screen-space ray at each iteration's t, back in the world.
    let at = |tt: f32| screen.world_of(t.screen_pos + t.screen_ray * tt);
    let mut path: Vec<Vec3> = vec![screen.view_to_world(t.biased)];
    path.extend(t.steps.iter().take(shown).map(|s| at(s.next_t)));
    if path.len() > 1 {
        content.draw.polyline(&path, palette::REFLECTED, 1.0, 3.0);
        content.draw.point(*path.last().expect("a point"), 5.0, palette::REFLECTED, 1.0);
    }
    // The level of the iteration shown, the cells visited on it, the current one lit.
    let current = if shown == 0 { None } else { t.steps.get(shown - 1) };
    let level = current.map_or(0, |s| s.level.max(0)) as usize;
    let img = &screen.hiz[level];
    let mut outlines = Vec::new();
    for (k, s) in t.steps.iter().take(shown).enumerate() {
        if s.level as usize != level {
            continue;
        }
        let (w, h) = (img.size[0] as f32, img.size[1] as f32);
        let rect = [s.cell[0] as f32 / w, s.cell[1] as f32 / h, (s.cell[0] + 1) as f32 / w, (s.cell[1] + 1) as f32 / h];
        let lit = k + 1 == shown;
        outlines.push(Outline { rect, color: if lit { palette::SELECTED } else { palette::REFLECTED }, width: if lit { 3.0 } else { 1.5 }, outside: false });
    }
    let size = Vec2::new(screen.size[0] as f32, screen.size[1] as f32);
    let marker = Marker { at: [(pixel[0] as f32 + 0.5) / size.x, (pixel[1] as f32 + 0.5) / size.y], color: palette::SELECTED, label: "P".into() };
    let link = (!outlines.is_empty()).then(|| Link { from: current.map_or(start, |s| at(s.next_t)), outline: outlines.len() - 1 });
    let title = tr_with("demos.trace_inset", lab::args!(level = level, width = img.size[0], height = img.size[1]));
    content.insets.push(PictureInset { marker: Some(marker), outlines, link, ..PictureInset::new(title, level_picture(img, format!("ssr-trace-level-{level}")), [420.0, 420.0 * img.size[1] as f32 / img.size[0] as f32]) });
    // The iteration shown, as the loop computes it.
    let mut rows: Vec<(String, String, Color32)> = Vec::new();
    let row = |rows: &mut Vec<(String, String, Color32)>, name: &str, v: String, c: Color32| rows.push((name.to_string(), v, c));
    if let Some(s) = current {
        row(&mut rows, tr("demos.step"), format!("{shown} / {}", t.steps.len()), palette::SELECTED);
        row(&mut rows, tr("demos.level"), format!("{}", s.level), palette::SELECTED);
        row(&mut rows, tr("demos.cell"), format!("({}, {})", s.cell[0], s.cell[1]), palette::SELECTED);
        row(&mut rows, tr("demos.cell_depth"), format!("{:.5}", s.cell_depth), palette::THEME_ACCENT);
        row(&mut rows, tr("demos.ray_depth"), format!("{:.5}", (t.screen_pos + t.screen_ray * s.t).z), palette::REFLECTED);
        row(&mut rows, tr("demos.depth_t"), format!("{:.5}", s.depth_t), palette::THEME_ACCENT);
        row(&mut rows, tr("demos.edge_t"), format!("{:.5}", s.edge_t), palette::THEME_ACCENT);
        let result = if s.held { tr("demos.held") } else if s.hit { tr("demos.hit_down") } else { tr("demos.miss_up") };
        row(&mut rows, tr("demos.result"), result.to_string(), palette::SELECTED);
    }
    if shown == t.steps.len() {
        let exit = match t.exit {
            Exit::Hit => tr("demos.exit_hit"),
            Exit::Steps => tr("demos.exit_steps"),
            Exit::Screen => tr("demos.exit_screen"),
            Exit::NotTraced => tr("demos.exit_none"),
        };
        row(&mut rows, tr("demos.exit"), exit.to_string(), palette::REFLECTED);
        row(&mut rows, tr("demos.validity"), format!("{:.3}", t.validity), palette::REFLECTED);
    }
    content.readout = Some(Readout {
        title: tr_with("demos.trace_title", lab::args!(x = pixel[0], y = pixel[1])),
        subtitle: tr("demos.trace_subtitle").into(),
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
        (tr("demos.ndc").to_string(), format!("({:.4}, {:.4})", uv.x, uv.y), palette::SELECTED),
        (tr("demos.depth").to_string(), format!("{depth:.5}"), palette::THEME_ACCENT),
        (tr("demos.view_pos").to_string(), v3(view), palette::SELECTED),
        (tr("demos.world_pos").to_string(), v3(world), palette::SELECTED),
        (tr("demos.view_normal").to_string(), v3(normal), palette::AXIS_Y),
        (tr("demos.roughness_byte").to_string(), format!("{}", (nr.w * 255.0).round() as u32), palette::INCIDENT),
        (tr("demos.roughness").to_string(), format!("{:.3}", trace::decode_roughness(nr.w)), palette::INCIDENT),
    ];
    content.readout = Some(rows_readout(tr_with("demos.inputs_title", lab::args!(x = pixel[0], y = pixel[1])), tr("demos.inputs_subtitle"), rows));
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
    let title = tr_with("demos.pyramid_title", lab::args!(level = level, width = img.size[0], height = img.size[1]));
    let marker = Marker { at: [(pixel[0] as f32 + 0.5) / screen.size[0] as f32, (pixel[1] as f32 + 0.5) / screen.size[1] as f32], color: palette::SELECTED, label: "P".into() };
    content.insets.push(PictureInset { marker: Some(marker), outlines: vec![outline], link: Some(Link { from: start, outline: 0 }), ..PictureInset::new(title, level_picture(img, format!("ssr-pyramid-{level}")), [480.0, 480.0 * h / w]) });
    content.draw.point(start, 6.0, palette::SELECTED, 1.0);
    let side = 1i32 << level;
    let rows = vec![
        (tr("demos.p_cell").to_string(), format!("({}, {})", cell[0], cell[1]), palette::SELECTED),
        (tr("demos.cell_depth").to_string(), format!("{:.5}", img.load(cell).x), palette::THEME_ACCENT),
        (tr("demos.p_depth").to_string(), format!("{p_depth:.5}"), palette::SELECTED),
        (tr("demos.footprint").to_string(), format!("{side} x {side}"), palette::THEME_ACCENT),
    ];
    content.readout = Some(rows_readout(tr_with("demos.pyramid_title", lab::args!(level = level, width = img.size[0], height = img.size[1])), tr("demos.pyramid_subtitle"), rows));
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
    let names = [tr("demos.term_edge"), tr("demos.term_back"), tr("demos.term_confidence"), tr("demos.term_margin"), tr("demos.term_fade")];
    let mut rows: Vec<(String, String, Color32)> = names.iter().zip(t.terms).map(|(n, v)| (n.to_string(), format!("{v:.3}"), palette::THEME_ACCENT)).collect();
    rows.push((tr("demos.ray_len").to_string(), format!("{:.3}", t.ray_len), palette::REFLECTED));
    rows.push((tr("demos.validity").to_string(), format!("{:.3}", t.validity), palette::SELECTED));
    content.readout = Some(rows_readout(tr_with("demos.hit_title", lab::args!(x = pixel[0], y = pixel[1])), tr("demos.hit_subtitle"), rows));
    // The fade curve over the ray's screen length, 0 to 1, its peak and P's length marked.
    let (w, h) = (360usize, 180usize);
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
    let title = tr_with("demos.fade_chart", lab::args!(peak = format!("{peak:.2}"), at = format!("{at:.2}")));
    let marker = Marker { at: [at, 1.0 - peak], color: palette::SELECTED, label: format!("{peak:.2}") };
    content.insets.push(PictureInset { marker: Some(marker), ..PictureInset::new(title, Picture::Pixels { key: "ssr-fade-chart".into(), size: [w, h], rgba: Arc::new(rgba) }, [360.0, 180.0]) });
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
        (tr("demos.roughness").to_string(), format!("{:.3}", t.roughness), palette::INCIDENT),
        (tr("demos.cone_angle").to_string(), format!("{:.1}°", angle.to_degrees()), palette::INCIDENT),
        (tr("demos.ray_len").to_string(), format!("{:.3}", t.ray_len), palette::REFLECTED),
        (tr("demos.blur_radius").to_string(), format!("{:.1} px", radius * largest), palette::THEME_ACCENT),
        (tr("demos.mip_level").to_string(), format!("{:.2}", t.mip_level), palette::SELECTED),
    ];
    content.readout = Some(rows_readout(tr_with("demos.blur_title", lab::args!(x = pixel[0], y = pixel[1])), tr("demos.blur_subtitle"), rows));
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
    content.insets.push(PictureInset::new(tr("demos.cone_chart").to_string(), Picture::Pixels { key: "ssr-cone-chart".into(), size: [w, h], rgba: Arc::new(rgba) }, [360.0, 220.0]));
}
