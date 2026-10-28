//! The lab's environment: Poly Haven's "Tree Lined Driveway" HDRI
//! (assets/environments, CC0), as the resource pipeline makes it
//! (pipeline/radiance.rs, declared in src/lab/resources.toml): its sun is
//! clipped, since no investigation has a light of its own for it, and it is
//! scaled to a mean radiance of one over the sphere; each investigation sets
//! its sky's level (`Lighting::ambient`, the frame's `ambientColor`). Level 0
//! is the image, 4096 x 2048; each level after it averages 2 x 2 texels.
//! Texels are kept in RGB9E5, the format of the GPU texture (gEnvironment), so
//! `radiance` returns what common.wgsl's `environment` reads.

use crate::lab::resources;
use glam::Vec3;
use std::sync::OnceLock;

/// Levels kept (the resource's `levels`): the backdrop reads level 0, lighting
/// reads `LIGHTING_LEVEL`.
pub const LEVELS: usize = 3;
/// The level that lighting reads (1024 x 512, about the detail of a 256-texel
/// cubemap face), so few rays do not alias on the photograph's detail.
pub const LIGHTING_LEVEL: usize = 2;
/// The image's centre column faces this many turns from -Z towards +X.
pub const TURN: f32 = 0.0;

pub struct Environment {
    /// Width and height of each level.
    pub sizes: [[usize; 2]; LEVELS],
    /// RGB9E5 texels of each level, row-major from the top row.
    pub levels: [Vec<u32>; LEVELS],
}

impl Environment {
    pub fn get() -> &'static Environment {
        static ENVIRONMENT: OnceLock<Environment> = OnceLock::new();
        ENVIRONMENT.get_or_init(|| Environment::load(&resources::ENVIRONMENT))
    }

    fn load(r: &resources::Resource) -> Environment {
        assert_eq!(r.word(0) as usize, LEVELS, "{}: levels", r.id);
        let sizes: [[usize; 2]; LEVELS] = std::array::from_fn(|i| [r.word(1 + 2 * i) as usize, r.word(2 + 2 * i) as usize]);
        let mut offset = 4 * (1 + 2 * LEVELS);
        let levels = std::array::from_fn(|i| {
            let count = sizes[i][0] * sizes[i][1];
            let level = r.array::<u32>(offset, count);
            offset += 4 * count;
            level
        });
        Environment { sizes, levels }
    }

    pub fn texel(&self, level: usize, x: usize, y: usize) -> Vec3 {
        decode_rgb9e5(self.levels[level][y * self.sizes[level][0] + x])
    }

    /// Bilinear between texel centres, wrapping around the vertical axis and
    /// clamped at the poles (common.wgsl's environment).
    pub fn radiance(&self, dir: Vec3, level: usize) -> Vec3 {
        let [w, h] = self.sizes[level];
        let d = dir.normalize();
        let u = (0.5 + d.x.atan2(-d.z) / (2.0 * std::f32::consts::PI) + TURN) * w as f32 - 0.5;
        let v = (d.y.clamp(-1.0, 1.0).acos() / std::f32::consts::PI * h as f32 - 0.5).clamp(0.0, (h - 1) as f32);
        let (x, y) = (u.floor(), v.floor());
        let (fx, fy) = (u - x, v - y);
        let x0 = (x as i32).rem_euclid(w as i32) as usize;
        let x1 = (x0 + 1) % w;
        let y0 = y as usize;
        let y1 = (y0 + 1).min(h - 1);
        let top = self.texel(level, x0, y0).lerp(self.texel(level, x1, y0), fx);
        let bottom = self.texel(level, x0, y1).lerp(self.texel(level, x1, y1), fx);
        top.lerp(bottom, fy)
    }
}

/// The environment's radiance along `dir`, at the level lighting reads.
pub fn radiance(dir: Vec3) -> Vec3 {
    Environment::get().radiance(dir, LIGHTING_LEVEL)
}

pub fn decode_rgb9e5(t: u32) -> Vec3 {
    let step = 2f32.powi((t >> 27) as i32 - 24);
    Vec3::new((t & 511) as f32, (t >> 9 & 511) as f32, (t >> 18 & 511) as f32) * step
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lab::rgb9e5::encode_rgb9e5;

    fn luminance(c: Vec3) -> f32 {
        c.dot(Vec3::new(0.2126, 0.7152, 0.0722))
    }

    #[test]
    fn rgb9e5_round_trip() {
        for c in [Vec3::new(1.0, 0.5, 0.25), Vec3::new(0.0123, 3.5, 100.0), Vec3::splat(511.0 / 512.0), Vec3::ZERO] {
            let back = decode_rgb9e5(encode_rgb9e5(c));
            assert!((back - c).abs().max_element() <= c.max_element() / 256.0, "{c} -> {back}");
        }
    }

    #[test]
    fn environment_is_normalised_and_sunless() {
        let e = Environment::get();
        assert_eq!(e.sizes[0], [4096, 2048]);
        let [w, h] = e.sizes[LIGHTING_LEVEL];
        let (mut sum, mut weight, mut peak) = (0.0f64, 0.0f64, 0.0f32);
        for y in 0..h {
            let s = (std::f64::consts::PI * (y as f64 + 0.5) / h as f64).sin();
            for x in 0..w {
                let l = luminance(e.texel(LIGHTING_LEVEL, x, y));
                sum += s * l as f64;
                weight += s;
                peak = peak.max(l);
            }
        }
        assert!((sum / weight - 1.0).abs() < 0.01, "mean {}", sum / weight);
        assert!(peak < 50.0, "peak {peak}");
    }
}
