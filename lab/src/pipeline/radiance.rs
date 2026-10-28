//! `radiance`: an equirectangular Radiance (RGBE) image, made into the levels
//! the lab's environment samples (src/environment.rs). The sun is clipped
//! at `sun-cap` luminance and the image is scaled to a mean radiance of one
//! over the sphere; level 0 is the image, and each of the `levels` after it
//! averages 2 x 2 texels of the one before.
//!
//! Output, little-endian: the number of levels, each level's width and
//! height, then each level's RGB9E5 texels, row-major from the top row.

use crate::rgb9e5::encode_rgb9e5;
use super::{put_u32, Kind, Params, Ty};
use glam::Vec3;

pub const KIND: Kind = Kind {
    name: "radiance",
    params: &[("sun-cap", Ty::Float), ("levels", Ty::Int)],
    code: &[include_str!("radiance.rs"), include_str!("../rgb9e5.rs")],
    process,
};

fn process(file: &[u8], params: &Params) -> Result<Vec<u8>, String> {
    let (sun_cap, count) = (params.float("sun-cap"), params.int("levels"));
    let (width, height, mut texels) = read_radiance(file)?;
    if count == 0 || width % (1 << (count - 1)) != 0 || height % (1 << (count - 1)) != 0 {
        return Err(format!("{width} x {height} does not halve into {count} levels"));
    }
    for t in &mut texels {
        let luminance = luminance(*t);
        if luminance > sun_cap {
            *t *= sun_cap / luminance;
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
    let mut out = Vec::new();
    put_u32(&mut out, &[count as u32]);
    for i in 0..count {
        put_u32(&mut out, &[(width >> i) as u32, (height >> i) as u32]);
    }
    for i in 0..count {
        if i > 0 {
            let w = width >> (i - 1);
            level = (0..level.len() / 4)
                .map(|k| {
                    let (x, y) = (k % (w / 2) * 2, k / (w / 2) * 2);
                    (level[y * w + x] + level[y * w + x + 1] + level[(y + 1) * w + x] + level[(y + 1) * w + x + 1]) * 0.25
                })
                .collect();
        }
        put_u32(&mut out, &level.iter().map(|&t| encode_rgb9e5(t)).collect::<Vec<u32>>());
    }
    Ok(out)
}

fn luminance(c: Vec3) -> f32 {
    c.dot(Vec3::new(0.2126, 0.7152, 0.0722))
}

/// A Radiance (RGBE) image with a `-Y height +X width` header: its size and
/// texels, row-major from the top row. Scanlines are run-length encoded per
/// channel or flat.
fn read_radiance(file: &[u8]) -> Result<(usize, usize, Vec<Vec3>), String> {
    let mut at = 0;
    let mut line = || -> Result<&str, String> {
        let end = at + file[at..].iter().position(|&b| b == b'\n').ok_or("the Radiance header does not end")?;
        let text = std::str::from_utf8(&file[at..end]).map_err(|_| "the Radiance header is not text")?;
        at = end + 1;
        Ok(text)
    };
    if !line()?.starts_with("#?") {
        return Err("not a Radiance image".into());
    }
    loop {
        let l = line()?;
        if l.is_empty() {
            break;
        }
        if l.starts_with("FORMAT=") && l != "FORMAT=32-bit_rle_rgbe" {
            return Err(format!("unsupported Radiance format {l}"));
        }
    }
    let size: Vec<&str> = line()?.split_whitespace().collect();
    if size.len() != 4 || size[0] != "-Y" || size[2] != "+X" {
        return Err("unsupported Radiance orientation".into());
    }
    let (height, width): (usize, usize) = (size[1].parse().map_err(|_| "Radiance height")?, size[3].parse().map_err(|_| "Radiance width")?);
    let mut data = &file[at..];
    let mut texels = Vec::with_capacity(width * height);
    let mut scanline = vec![[0u8; 4]; width];
    let truncated = || "the Radiance image is truncated".to_string();
    for _ in 0..height {
        if data.len() < 4 {
            return Err(truncated());
        }
        if (8..0x8000).contains(&width) && data[0] == 2 && data[1] == 2 && data[2] & 0x80 == 0 {
            if ((data[2] as usize) << 8) | data[3] as usize != width {
                return Err("a Radiance scanline's width is not the image's".into());
            }
            data = &data[4..];
            for c in 0..4 {
                let mut x = 0;
                while x < width {
                    let count = *data.first().ok_or_else(truncated)? as usize;
                    if count > 128 {
                        let value = *data.get(1).ok_or_else(truncated)?;
                        for texel in scanline.get_mut(x..x + count - 128).ok_or("a Radiance run overflows its scanline")? {
                            texel[c] = value;
                        }
                        x += count - 128;
                        data = &data[2..];
                    } else {
                        let run = data.get(1..1 + count).ok_or_else(truncated)?;
                        for (texel, &value) in scanline.get_mut(x..x + count).ok_or("a Radiance run overflows its scanline")?.iter_mut().zip(run) {
                            texel[c] = value;
                        }
                        x += count;
                        data = &data[1 + count..];
                    }
                }
            }
        } else {
            let flat = data.get(..4 * width).ok_or_else(truncated)?;
            for (texel, bytes) in scanline.iter_mut().zip(flat.chunks_exact(4)) {
                texel.copy_from_slice(bytes);
            }
            data = &data[4 * width..];
        }
        texels.extend(scanline.iter().map(|&[r, g, b, e]| if e == 0 { Vec3::ZERO } else { Vec3::new(r as f32, g as f32, b as f32) * 2f32.powi(e as i32 - 136) }));
    }
    Ok((width, height, texels))
}
