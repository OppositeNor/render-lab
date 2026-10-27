//! The lab's environment: Poly Haven's "Tree Lined Driveway" HDRI
//! (assets/environments, CC0), an equirectangular Radiance image decoded once
//! per run. Its sun is clipped, since no investigation has a light of its own
//! for it, and it is scaled to a mean radiance of one over the sphere; each
//! investigation sets its sky's level (`Lighting::ambient`, the frame's
//! `ambientColor`). Level 0 is the image, 4096 x 2048; each level after it
//! averages 2 x 2 texels. Texels are kept in RGB9E5, the format of the GPU
//! texture (gEnvironment), so `radiance` returns what common.wgsl's
//! `environment` reads.

use glam::Vec3;
use std::sync::OnceLock;

static HDR: &[u8] = include_bytes!("../../assets/environments/tree_lined_driveway_4k.hdr");

/// The sun's disc is capped at this luminance, in the photograph's units: the
/// glare one to two degrees from it. The cap removes about two thirds of the
/// image's light, all within three degrees of the sun.
pub const SUN_CAP: f32 = 5.0;
/// Levels kept: the backdrop reads level 0, lighting reads `LIGHTING_LEVEL`.
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
        ENVIRONMENT.get_or_init(|| Environment::decode(HDR))
    }

    fn decode(file: &[u8]) -> Environment {
        let (width, height, mut texels) = read_radiance(file);
        for t in &mut texels {
            let luminance = luminance(*t);
            if luminance > SUN_CAP {
                *t *= SUN_CAP / luminance;
            }
        }
        // Mean luminance, each row weighted by its solid angle.
        let mut sum = 0.0f64;
        let mut weight = 0.0f64;
        for y in 0..height {
            let w = (std::f64::consts::PI * (y as f64 + 0.5) / height as f64).sin();
            sum += w * texels[y * width..(y + 1) * width].iter().map(|&t| luminance(t) as f64).sum::<f64>();
            weight += w * width as f64;
        }
        let scale = (weight / sum) as f32;
        let mut level: Vec<Vec3> = texels.into_iter().map(|t| t * scale).collect();
        let mut sizes = [[0; 2]; LEVELS];
        let levels = std::array::from_fn(|i| {
            if i > 0 {
                let [w, h] = sizes[i - 1];
                level = (0..w * h / 4).map(|k| {
                    let (x, y) = (k % (w / 2) * 2, k / (w / 2) * 2);
                    (level[y * w + x] + level[y * w + x + 1] + level[(y + 1) * w + x] + level[(y + 1) * w + x + 1]) * 0.25
                }).collect();
            }
            sizes[i] = [width >> i, height >> i];
            level.iter().map(|&t| encode_rgb9e5(t)).collect()
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

fn luminance(c: Vec3) -> f32 {
    c.dot(Vec3::new(0.2126, 0.7152, 0.0722))
}

/// A Radiance (RGBE) image with a `-Y height +X width` header: its size and
/// texels, row-major from the top row. Scanlines are run-length encoded per
/// channel or flat.
fn read_radiance(file: &[u8]) -> (usize, usize, Vec<Vec3>) {
    let mut at = 0;
    let mut line = || {
        let end = at + file[at..].iter().position(|&b| b == b'\n').expect("Radiance header ends");
        let text = std::str::from_utf8(&file[at..end]).expect("Radiance header is text");
        at = end + 1;
        text
    };
    assert!(line().starts_with("#?"), "not a Radiance image");
    loop {
        let l = line();
        if l.is_empty() {
            break;
        }
        assert!(!l.starts_with("FORMAT=") || l == "FORMAT=32-bit_rle_rgbe", "unsupported Radiance format {l}");
    }
    let size: Vec<&str> = line().split_whitespace().collect();
    assert!(size.len() == 4 && size[0] == "-Y" && size[2] == "+X", "unsupported Radiance orientation");
    let (height, width): (usize, usize) = (size[1].parse().unwrap(), size[3].parse().unwrap());
    let mut data = &file[at..];
    let mut texels = Vec::with_capacity(width * height);
    let mut scanline = vec![[0u8; 4]; width];
    for _ in 0..height {
        if (8..0x8000).contains(&width) && data[0] == 2 && data[1] == 2 && data[2] & 0x80 == 0 {
            assert_eq!(((data[2] as usize) << 8) | data[3] as usize, width, "Radiance scanline width");
            data = &data[4..];
            for c in 0..4 {
                let mut x = 0;
                while x < width {
                    let count = data[0] as usize;
                    if count > 128 {
                        for texel in &mut scanline[x..x + count - 128] {
                            texel[c] = data[1];
                        }
                        x += count - 128;
                        data = &data[2..];
                    } else {
                        for (k, texel) in scanline[x..x + count].iter_mut().enumerate() {
                            texel[c] = data[1 + k];
                        }
                        x += count;
                        data = &data[1 + count..];
                    }
                }
            }
        } else {
            for (texel, bytes) in scanline.iter_mut().zip(data.chunks_exact(4)) {
                texel.copy_from_slice(bytes);
            }
            data = &data[4 * width..];
        }
        texels.extend(scanline.iter().map(|&[r, g, b, e]| if e == 0 { Vec3::ZERO } else { Vec3::new(r as f32, g as f32, b as f32) * 2f32.powi(e as i32 - 136) }));
    }
    (width, height, texels)
}

/// EXT_texture_shared_exponent's encoding: nine-bit mantissas under one
/// five-bit exponent (bias 15).
pub fn encode_rgb9e5(c: Vec3) -> u32 {
    let c = c.clamp(Vec3::ZERO, Vec3::splat(65408.0));
    let max = c.max_element();
    if max < 2f32.powi(-24) {
        return 0;
    }
    // floor(log2(max)), from the float's exponent bits.
    let log2 = ((max.to_bits() >> 23) & 0xff) as i32 - 127;
    let mut exponent = log2.max(-16) + 16;
    let mut step = 2f32.powi(exponent - 24);
    if (max / step + 0.5).floor() >= 512.0 {
        step *= 2.0;
        exponent += 1;
    }
    let m = (c / step + 0.5).floor();
    m.x as u32 | (m.y as u32) << 9 | (m.z as u32) << 18 | (exponent as u32) << 27
}

pub fn decode_rgb9e5(t: u32) -> Vec3 {
    let step = 2f32.powi((t >> 27) as i32 - 24);
    Vec3::new((t & 511) as f32, (t >> 9 & 511) as f32, (t >> 18 & 511) as f32) * step
}

#[cfg(test)]
mod tests {
    use super::*;

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
