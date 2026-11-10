//! The frame's SSR inputs on the CPU, for the demonstrations that step one
//! pixel's trace (trace.rs): the GPU's own half-size depth pyramid (`hiz`) and
//! normal-roughness (`halfNormal`), read back after the frame's passes
//! (lab::fetch: in the viewer a frame or two late while the view moves; a
//! recording or a snapshot reads its own frame's). A demonstration that shows
//! one pixel reads only that pixel; the trace reads the whole pyramid.

use super::gpu;
use super::trace::{self, Image};
use glam::{Mat4, Vec2, Vec3, Vec4};
use lab::fetch::{Fetched, Region, Source};
use lab::frame::FrameContent;
use lab::layout::CameraData;
use std::ops::Deref;
use std::sync::{Arc, Mutex};

/// The frame's SSR pixels as the camera sees them: their size and projection.
#[derive(Clone, Copy)]
pub struct View {
    pub size: [u32; 2],
    pub projection: Mat4,
    pub world_to_view: Mat4,
}

impl View {
    /// The SSR pixels of a frame drawn by `camera`.
    pub fn of(camera: &CameraData) -> View {
        let viewport = [
            camera.viewport_size[0].max(1.0) as u32,
            camera.viewport_size[1].max(1.0) as u32,
        ];
        View {
            size: gpu::ssr_size(viewport),
            projection: gpu::godot_projection(Mat4::from_cols_array_2d(&camera.view_to_clip)),
            world_to_view: Mat4::from_cols_array_2d(&camera.world_to_view),
        }
    }

    /// The world point of a screen position (u, v, reversed depth) of this frame.
    pub fn world_of(&self, s: Vec3) -> Vec3 {
        // Undo the projection to view space, then the camera to the world.
        let p = self.projection.inverse() * Vec4::new(s.x * 2.0 - 1.0, s.y * 2.0 - 1.0, s.z, 1.0);
        let view = p.truncate() / p.w;
        self.world_to_view.inverse().transform_point3(view)
    }

    /// The world point of SSR pixel `p`'s centre at reversed depth `depth`.
    pub fn world_at(&self, p: [i32; 2], depth: f32) -> Vec3 {
        let uv = Vec2::new(
            (p[0] as f32 + 0.5) / self.size[0] as f32,
            (p[1] as f32 + 0.5) / self.size[1] as f32,
        );
        self.world_of(uv.extend(depth))
    }

    /// The world point of a view-space point.
    pub fn view_to_world(&self, v: Vec3) -> Vec3 {
        self.world_to_view.inverse().transform_point3(v)
    }

    /// The SSR pixel a world point falls in.
    pub fn pixel_of(&self, p: Vec3) -> Option<[i32; 2]> {
        let c = self.projection * self.world_to_view * p.extend(1.0);
        if c.w <= 1e-4 {
            // Behind the camera: no pixel.
            return None;
        }
        let uv = Vec2::new(c.x / c.w, c.y / c.w) * 0.5 + Vec2::splat(0.5);
        let px = uv * Vec2::new(self.size[0] as f32, self.size[1] as f32);
        let q = [px.x.floor() as i32, px.y.floor() as i32];
        (q[0] >= 0 && q[1] >= 0 && q[0] < self.size[0] as i32 && q[1] < self.size[1] as i32)
            .then_some(q)
    }
}

/// The SSR inputs of one frame: its pyramid and normal-roughness, as the GPU left them.
pub struct Screen {
    pub view: View,
    pub hiz: Vec<Image>,
    pub normal_roughness: Image,
}

impl Deref for Screen {
    type Target = View;
    fn deref(&self) -> &View {
        &self.view
    }
}

impl Screen {
    /// The frame's inputs from images read back (each level of `hiz`, and `normal_roughness`).
    pub fn new(view: View, hiz: Vec<Image>, normal_roughness: Image) -> Screen {
        Screen {
            view,
            hiz,
            normal_roughness,
        }
    }

    /// The world point of SSR pixel `p`'s surface (its depth at level 0).
    pub fn surface(&self, p: [i32; 2]) -> Vec3 {
        self.world_at(p, self.hiz[0].load(p).x)
    }

    /// The trace's inputs over this frame (last frame black: the stepper shows geometry, not colour).
    pub fn inputs<'a>(
        &'a self,
        last: &'a Image,
        steps: i32,
        fade_in: f32,
        fade_out: f32,
        tolerance: f32,
    ) -> trace::Inputs<'a> {
        trace::Inputs {
            hiz: &self.hiz,
            normal_roughness: &self.normal_roughness,
            last_frame: last,
            projection: self.projection,
            inv_projection: self.projection.inverse(),
            reprojection: Mat4::IDENTITY,
            num_steps: steps,
            distance_fade: fade_out,
            curve_fade_in: fade_in,
            depth_tolerance: tolerance,
        }
    }
}

/// A whole image read back, as trace.rs's image.
pub fn image_of(f: &Fetched) -> Option<Image> {
    let size = [f.extent[0], f.extent[1]];
    (f.region.origin == [0, 0, 0] && f.region.size == [size[0], size[1], 1] && f.channels > 0).then(
        || Image {
            size,
            channels: f.channels,
            data: f.floats.clone(),
        },
    )
}

/// The frame's whole pyramid and normal-roughness (asked for in `content`):
/// the latest that have landed, all of one frame; None until they have. While
/// the view changes size they are of the size before it, and the screen's
/// pixels are theirs.
pub fn screen(content: &mut FrameContent) -> Option<Arc<Screen>> {
    let view = View::of(&content.uniforms.camera);
    let mips = gpu::mipmaps(view.size);
    let levels: Vec<Option<Arc<Fetched>>> = (0..mips)
        .map(|m| {
            content.fetch(
                &format!("godot_ssr.hiz/{m}"),
                Source::image("hiz", m),
                Region::ALL,
            )
        })
        .collect();
    let normal = content.fetch(
        "godot_ssr.halfNormal",
        Source::image("halfNormal", 0),
        Region::ALL,
    );
    assemble(view, levels, normal)
}

/// The screen of view `of` (lab/src/view.rs), drawn by `camera`: what it read
/// back for itself (a picture view drawing the main view's data; nothing is
/// asked for here, `of` asks). None until it has.
pub fn screen_in(of: lab::view::ViewId, camera: &CameraData) -> Option<Arc<Screen>> {
    let view = View::of(camera);
    let mips = gpu::mipmaps(view.size);
    let levels: Vec<Option<Arc<Fetched>>> = (0..mips)
        .map(|m| lab::fetch::get_in(of, &format!("godot_ssr.hiz/{m}")))
        .collect();
    let normal = lab::fetch::get_in(of, "godot_ssr.halfNormal");
    assemble(view, levels, normal)
}

/// The screen from a pyramid's levels and the normals read back, all of one frame (kept for the current view).
fn assemble(
    mut view: View,
    levels: Vec<Option<Arc<Fetched>>>,
    normal: Option<Arc<Fetched>>,
) -> Option<Arc<Screen>> {
    // Every level and the normals must have landed.
    let (levels, normal): (Vec<Arc<Fetched>>, Arc<Fetched>) =
        (levels.into_iter().collect::<Option<_>>()?, normal?);
    // All of one frame: the levels and the normals read after the same passes.
    let frames: Vec<u64> = levels.iter().chain([&normal]).map(|f| f.frame).collect();
    if frames.iter().any(|&f| f != frames[0])
        || levels.iter().chain([&normal]).any(|f| f.tag != normal.tag)
    {
        // Images from two frames would not match: show the view's last whole screen instead.
        return kept(&view, None);
    }
    // The screen's pixels are the images', which lag a resize by a frame or two.
    view.size = [normal.extent[0], normal.extent[1]];
    // A screen already built from these images for this camera is reused.
    let key = (normal.tag, normal.frame, view_bits(&view));
    if let Some(s) = kept(&view, Some(key)) {
        return Some(s);
    }
    let hiz: Vec<Image> = levels.iter().map(|f| image_of(f)).collect::<Option<_>>()?;
    if hiz
        .iter()
        .enumerate()
        .any(|(m, l)| l.size != gpu::level_size(view.size, m as u32))
    {
        // A level of the wrong size is from another resolution: keep the last screen.
        return kept(&view, None);
    }
    let screen = Arc::new(Screen::new(view, hiz, image_of(&normal)?));
    // Keep it as this view's screen, replacing the one before.
    let mut kept = KEPT.lock().expect("the kept screens");
    kept.retain(|(v, _, _)| *v != lab::view::current());
    kept.push((lab::view::current(), key, screen.clone()));
    Some(screen)
}

type Key = (u64, u64, [u32; 48]);

/// The screen kept for each view (lab::view): views never evict or read each other's.
static KEPT: Mutex<Vec<(lab::view::ViewId, Key, Arc<Screen>)>> = Mutex::new(Vec::new());

/// The current view's screen kept: the one of `key`, or with no key its last one of this view's size.
fn kept(view: &View, key: Option<Key>) -> Option<Arc<Screen>> {
    let kept = KEPT.lock().expect("the kept screens");
    let (_, k, s) = kept.iter().find(|(v, _, _)| *v == lab::view::current())?;
    match key {
        // With a key, only the screen of exactly those images and camera.
        Some(key) => (*k == key).then(|| s.clone()),
        // Without one, the last screen, if it is still the view's size.
        None => (s.size == view.size).then(|| s.clone()),
    }
}

fn view_bits(view: &View) -> [u32; 48] {
    let mut bits = [0u32; 48];
    for (i, v) in view
        .projection
        .to_cols_array()
        .iter()
        .chain(view.world_to_view.to_cols_array().iter())
        .enumerate()
    {
        // The two matrices' bits, then the size, so any change to the camera changes the key.
        bits[i] = v.to_bits();
    }
    bits[32] = view.size[0];
    bits[33] = view.size[1];
    bits
}

/// Pixel `p` of a program image, read back alone (key `key`): the latest read,
/// its pixel (`p`, or while P moves the one a frame or two before) and texel.
pub fn texel(
    content: &mut FrameContent,
    key: &str,
    source: Source,
    p: [i32; 2],
) -> Option<([i32; 2], Vec4)> {
    let f = content.fetch(key, source, Region::around(p, 0))?;
    let at = [f.region.origin[0] as i32, f.region.origin[1] as i32];
    Some((at, f.texel(at)?))
}

/// One whole level of a program image (key `key`), as trace.rs's image: the latest read.
pub fn level(content: &mut FrameContent, key: &str, source: Source) -> Option<Image> {
    let f = content.fetch(key, source, Region::ALL)?;
    image_of(&f)
}
