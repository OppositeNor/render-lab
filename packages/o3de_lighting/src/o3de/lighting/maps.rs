//! The material textures the lab gives its objects in place of O3DE assets:
//! an occlusion map for the sphere, on the latitude-longitude UVs a UV
//! sphere's mesh carries, and two decal textures (a base map with alpha and
//! a normal map each). Stored as O3DE stores an occlusion texture (one
//! 8-bit channel) and appended to the shading data (`ibl::shading_data`);
//! read with a bilinear, wrapping lookup (O3DE's material sampler wraps and
//! filters trilinearly and anisotropically; the lab reads mip 0).

use glam::{Vec2, Vec3};
use std::sync::OnceLock;

pub const OCCLUSION_WIDTH: usize = 256;
pub const OCCLUSION_HEIGHT: usize = 128;

/// The occlusion map, row-major from v = 0: panels 16 around and 8 from pole
/// to pole, darkened to 0.2 in the grooves between them, rounded to 8 bits.
pub fn occlusion() -> &'static [f32] {
    static MAP: OnceLock<Vec<f32>> = OnceLock::new();
    MAP.get_or_init(|| {
        let smoothstep = |a: f32, b: f32, x: f32| {
            let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        };
        (0..OCCLUSION_HEIGHT)
            .flat_map(|y| (0..OCCLUSION_WIDTH).map(move |x| ((x as f32 + 0.5) / OCCLUSION_WIDTH as f32, (y as f32 + 0.5) / OCCLUSION_HEIGHT as f32)))
            .map(|(u, v)| {
                let (gu, gv) = ((u * 16.0).fract(), (v * 8.0).fract());
                let edge = gu.min(1.0 - gu).min(gv.min(1.0 - gv));
                let value = 1.0 - 0.8 * (1.0 - smoothstep(0.0, 0.08, edge));
                (value * 255.0).round() / 255.0
            })
            .collect()
    })
}

/// A bilinear lookup that wraps in both directions, as the material sampler does.
pub fn sample_occlusion(uv: Vec2) -> f32 {
    let map = occlusion();
    let (w, h) = (OCCLUSION_WIDTH as i32, OCCLUSION_HEIGHT as i32);
    let p = uv * Vec2::new(w as f32, h as f32) - 0.5;
    let base = p.floor();
    let t = p - base;
    let texel = |x: i32, y: i32| map[(y.rem_euclid(h) * w + x.rem_euclid(w)) as usize];
    let (x, y) = (base.x as i32, base.y as i32);
    let top = texel(x, y) * (1.0 - t.x) + texel(x + 1, y) * t.x;
    let bottom = texel(x, y + 1) * (1.0 - t.x) + texel(x + 1, y + 1) * t.x;
    top * (1.0 - t.y) + bottom * t.y
}

/// The sphere's UV at a direction from its centre (its local frame):
/// longitude across, latitude from the top pole down.
pub fn sphere_uv(local_direction: Vec3) -> Vec2 {
    let d = local_direction.normalize_or(Vec3::Y);
    Vec2::new(d.z.atan2(d.x) / std::f32::consts::TAU + 0.5, d.y.clamp(-1.0, 1.0).acos() / std::f32::consts::PI)
}

/// GetOcclusionInput (OcclusionInput.azsli): 1 without a texture, otherwise
/// the texture's red channel raised to the factor.
pub fn occlusion_input(uv: Vec2, factor: Option<f32>) -> f32 {
    factor.map_or(1.0, |f| sample_occlusion(uv).powf(f))
}

pub const DECAL_SIZE: usize = 128;
pub const DECAL_TEXTURES: usize = 2;

/// The decal textures: for each, DECAL_SIZE^2 base-map texels (RGB, alpha),
/// then as many normal-map texels (the two signed channels O3DE's decal
/// normal maps keep, before GetTangentSpaceNormal swaps them). Texture 0 is
/// a paint splat with a ragged edge; texture 1 a grid of panels whose lines
/// are dark and grooved.
pub fn decals() -> &'static [[f32; 4]] {
    static DATA: OnceLock<Vec<[f32; 4]>> = OnceLock::new();
    DATA.get_or_init(|| {
        let n = DECAL_SIZE as f32;
        let mut out = Vec::with_capacity(DECAL_TEXTURES * 2 * DECAL_SIZE * DECAL_SIZE);
        let texel = |x: usize, y: usize| Vec2::new((x as f32 + 0.5) / n, (y as f32 + 0.5) / n);
        let smooth = |a: f32, b: f32, x: f32| {
            let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        };
        // 0: the splat.
        for y in 0..DECAL_SIZE {
            for x in 0..DECAL_SIZE {
                let p = texel(x, y) * 2.0 - Vec2::ONE;
                let angle = p.y.atan2(p.x);
                let edge = 0.72 + 0.12 * (5.0 * angle).sin() + 0.06 * (13.0 * angle).cos();
                let alpha = 1.0 - smooth(edge - 0.08, edge, p.length());
                out.push([0.82, 0.28, 0.12, alpha]);
            }
        }
        for _ in 0..DECAL_SIZE * DECAL_SIZE {
            out.push([0.0, 0.0, 0.0, 0.0]);
        }
        // 1: the panel grid, 4 x 4 panels.
        let line = |u: f32| {
            let f = (u * 4.0).fract();
            f.min(1.0 - f)
        };
        for y in 0..DECAL_SIZE {
            for x in 0..DECAL_SIZE {
                let uv = texel(x, y);
                let d = line(uv.x).min(line(uv.y));
                let alpha = 1.0 - smooth(0.02, 0.06, d);
                out.push([0.12, 0.13, 0.15, alpha]);
            }
        }
        for y in 0..DECAL_SIZE {
            for x in 0..DECAL_SIZE {
                let uv = texel(x, y);
                // The groove's slope across each line, in the normal map's two channels.
                let slope = |u: f32| {
                    let f = (u * 4.0).fract();
                    let d = f.min(1.0 - f);
                    let side = if f < 0.5 { -1.0 } else { 1.0 };
                    side * 0.6 * (1.0 - smooth(0.0, 0.06, d))
                };
                out.push([slope(uv.y), slope(uv.x), 0.0, 0.0]);
            }
        }
        out
    })
}

/// A bilinear, clamped lookup of decal texture `index`'s base map (`normal`
/// false) or normal map, as PassSrg::LinearSampler reads them.
pub fn sample_decal(index: usize, normal: bool, uv: Vec2) -> [f32; 4] {
    let data = decals();
    let size = DECAL_SIZE as i32;
    let offset = (2 * index + usize::from(normal)) * DECAL_SIZE * DECAL_SIZE;
    let p = uv * size as f32 - 0.5;
    let base = p.floor();
    let t = p - base;
    let texel = |x: i32, y: i32| data[offset + (y.clamp(0, size - 1) * size + x.clamp(0, size - 1)) as usize];
    let (x, y) = (base.x as i32, base.y as i32);
    let mix = |a: [f32; 4], b: [f32; 4], t: f32| [0, 1, 2, 3].map(|k| a[k] * (1.0 - t) + b[k] * t);
    mix(mix(texel(x, y), texel(x + 1, y), t.x), mix(texel(x, y + 1), texel(x + 1, y + 1), t.x), t.y)
}
