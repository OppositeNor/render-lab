//! The probes on the CPU: sdfgi_integrate.glsl's `MODE_PROCESS` (each probe
//! traces its rays through the distance fields, projects what they find onto
//! 16 spherical-harmonic coefficients, and keeps them in a ring of
//! `history_size` frames in fixed point) and `MODE_STORE` (the summed history
//! becomes two 6 x 6 octahedral maps per probe, irradiance and radiance, with
//! a one-texel border, in E5B9G9R9).
//!
//! Where the lab differs: the per-probe rotation hashes through `sin` of a
//! large argument (`hashf3`), which the lab evaluates in f32 on the CPU; a
//! GPU's `sin` there differs, so the spiral's rotation of a given probe can
//! too. Textures are sampled with full-precision weights.
//!
//! Ported from Godot Engine (MIT; Copyright (c) 2014-present Godot Engine
//! contributors, Copyright (c) 2007-2014 Juan Linietsky, Ariel Manzur).

use super::light::{decode_e5b9g9r9, encode_e5b9g9r9, sample3, sample_sdf, Field, LightTextures, ProbeLookup};
use super::model::{CASCADE_SIZE, PROBE_AXIS_COUNT};
use glam::{IVec3, Vec2, Vec3};

pub const SH_SIZE: usize = 16;
pub const OCT_SIZE: i32 = 6;
pub const HISTORY_BITS: i32 = 10;
/// Probes per cascade.
pub const PROBES: usize = (PROBE_AXIS_COUNT * PROBE_AXIS_COUNT * PROBE_AXIS_COUNT) as usize;
/// The light-probe texture: 17 * 17 * 8 by 17 * 8 texels per layer.
pub const MAP_WIDTH: i32 = PROBE_AXIS_COUNT * PROBE_AXIS_COUNT * (OCT_SIZE + 2);
pub const MAP_HEIGHT: i32 = PROBE_AXIS_COUNT * (OCT_SIZE + 2);

const PI: f32 = 3.14159265;

/// `vogel_hemisphere`: point `index` of a Vogel spiral of `count`, its polar
/// angle linear in `r`, the sign of z by the index's parity.
pub fn vogel(p_index: u32, count: u32, offset: f32) -> Vec3 {
    let golden_angle = PI * (3.0 - 5f32.sqrt());
    let r = (p_index as f32 + 0.5).sqrt() / (count as f32).sqrt();
    let theta = p_index as f32 * golden_angle + offset;
    let y = (r * PI * 0.5).cos();
    let l = (r * PI * 0.5).sin();
    Vec3::new(l * theta.cos(), l * theta.sin(), y * ((p_index & 1) as f32 * 2.0 - 1.0))
}

fn hash3(x: [u32; 3]) -> [u32; 3] {
    x.map(|mut v| {
        v = ((v >> 16) ^ v).wrapping_mul(0x45d9f3b);
        v = ((v >> 16) ^ v).wrapping_mul(0x45d9f3b);
        (v >> 16) ^ v
    })
}

fn hashf3(co: Vec3) -> f32 {
    let v = (co.dot(Vec3::new(12.9898, 78.233, 137.13451)).sin()) * 43758.5453;
    v - v.floor()
}

/// The fixed rotation of a probe's spiral: `hashf3(hash3(world_offset +
/// probe_cell) & 0xFFFFF)`.
pub fn ray_offset(world_probe: IVec3) -> f32 {
    let h = hash3([world_probe.x as u32, world_probe.y as u32, world_probe.z as u32]);
    hashf3(Vec3::new((h[0] & 0xFFFFF) as f32, (h[1] & 0xFFFFF) as f32, (h[2] & 0xFFFFF) as f32))
}

/// The 16 real spherical-harmonic basis values the shader uses, in its order.
pub fn sh_basis(d: Vec3) -> [f32; SH_SIZE] {
    let d2 = d * d;
    [
        0.282095,
        0.488603 * d.y,
        0.488603 * d.z,
        0.488603 * d.x,
        1.092548 * d.x * d.y,
        1.092548 * d.y * d.z,
        0.315392 * (3.0 * d2.z - 1.0),
        1.092548 * d.x * d.z,
        0.546274 * (d2.x - d2.y),
        0.590043 * d.y * (3.0 * d2.x - d2.y),
        2.890611 * d.y * d.x * d.z,
        0.646360 * d.y * (-1.0 + 5.0 * d2.z),
        0.373176 * (5.0 * d2.z * d.z - 3.0 * d.z),
        0.457045 * d.x * (-1.0 + 5.0 * d2.z),
        1.445305 * (d2.x - d2.y) * d.z,
        0.590043 * d.x * (d2.x - 3.0 * d2.y),
    ]
}

/// `l_mult`: the clamped-cosine convolution per band over pi (1, 2/3, 1/4),
/// and 0 for band 3 (the shader's comment calls it l4).
pub const L_MULT: [f32; SH_SIZE] = [1.0, 2.0 / 3.0, 2.0 / 3.0, 2.0 / 3.0, 0.25, 0.25, 0.25, 0.25, 0.25, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];

/// What a ray that escapes every cascade reads (`sky_flags`): the sky's
/// radiance in a direction (the lab's environment), or a flat colour (black
/// when the environment reads no sky light).
pub enum Sky<'a> {
    Radiance(&'a dyn Fn(Vec3) -> Vec3),
    Color(Vec3),
}

/// One traced ray: its direction, where it hit (cascade, cell-space point),
/// and the light it brought back.
#[derive(Clone, Copy, Debug)]
pub struct Ray {
    pub direction: Vec3,
    pub hit: Option<(usize, Vec3)>,
    pub light: Vec3,
}

/// The settings of `update_probes` that the rays depend on.
#[derive(Clone, Copy, Debug)]
pub struct Integrate {
    /// `history_size`: 30 by default (frames_to_converge index 5).
    pub history_size: u32,
    /// `ray_count`: 8 by default (probe_ray_count index 1).
    pub ray_count: u32,
    /// `ray_bias` = the environment's probe bias, 1.1 by default.
    pub ray_bias: f32,
    pub y_mult: f32,
}

impl Default for Integrate {
    fn default() -> Integrate {
        Integrate { history_size: 30, ray_count: 8, ray_bias: 1.1, y_mult: 1.5 }
    }
}

/// The world position of a probe (squashed space): `offset + probe_cell *
/// probe_cell_size`.
pub fn probe_position(field: &Field, cascade: usize, probe: IVec3) -> Vec3 {
    let probe_cell_size = (CASCADE_SIZE as f32 / (PROBE_AXIS_COUNT - 1) as f32) / field.to_cell(cascade);
    field.offset(cascade) + probe.as_vec3() * probe_cell_size
}

/// One cascade's part of a probe ray's march, in its cell units from `pos`:
/// step by the stored distance, a hit below 0.05 cell, until the box's exit.
#[derive(Clone, Copy, Debug)]
pub struct March {
    pub hit: bool,
    pub steps: u32,
    pub advance: f32,
    pub max_advance: f32,
}

pub fn march_cascade(sdf: &[u8], pos: Vec3, dir: Vec3) -> March {
    let grid = CASCADE_SIZE as f32;
    let inv_dir = Vec3::ONE / dir;
    let t0 = -pos * inv_dir;
    let t1 = (Vec3::splat(grid) - pos) * inv_dir;
    let max_advance = t0.max(t1).min_element();
    let mut advance = 0.0;
    let mut steps = 0;
    while advance < max_advance {
        let distance = sample_sdf(sdf, (pos + dir * advance) / grid) * 255.0 - 1.0;
        steps += 1;
        if distance < 0.05 {
            return March { hit: true, steps, advance, max_advance };
        }
        advance += distance;
    }
    March { hit: false, steps, advance, max_advance }
}

/// The rays probe `probe` of `cascade` traces on history slot
/// `history_index` (`MODE_PROCESS`).
pub fn trace_rays(field: &Field, light: &[&LightTextures], cascade: usize, probe: IVec3, history_index: u32, settings: &Integrate, sky: &Sky) -> Vec<Ray> {
    let grid = CASCADE_SIZE as f32;
    let world_offset = field.cascades.cascades[cascade].position / (CASCADE_SIZE / (PROBE_AXIS_COUNT - 1));
    let offset = ray_offset(world_offset + probe);
    let probe_pos = probe_position(field, cascade, probe);
    let ray_total = settings.history_size * settings.ray_count;
    let mut rays = Vec::with_capacity(settings.ray_count as usize);
    for i in 0..settings.ray_count {
        let index = history_index + i * settings.history_size;
        let mut dir = vogel(index, ray_total, offset);
        dir.y *= settings.y_mult;
        dir = dir.normalize();
        let abs = dir.abs();
        let mut ray_pos = probe_pos + dir * 1.0 / abs.max_element() * settings.ray_bias / field.to_cell(cascade);
        let mut hit = None;
        let mut uvw = Vec3::ZERO;
        for j in cascade..field.cascades.cascades.len() {
            let mut pos = (ray_pos - field.offset(j)) * field.to_cell(j);
            if pos.cmplt(Vec3::ZERO).any() || pos.cmpge(Vec3::splat(grid)).any() {
                continue;
            }
            let march = march_cascade(field.sdf[j], pos, dir);
            let max_advance = march.max_advance;
            if march.hit {
                uvw = (pos + dir * march.advance) / grid;
                hit = Some((j, pos + dir * march.advance));
                break;
            }
            pos += dir * max_advance;
            pos /= field.to_cell(j);
            pos += field.offset(j);
            ray_pos = pos;
        }
        let light_rgb = match hit {
            Some((hc, _)) => {
                const EPSILON: f32 = 0.001;
                let s = |o: Vec3| sample_sdf(field.sdf[hc], uvw + o);
                let n = Vec3::new(s(Vec3::X * EPSILON) - s(-Vec3::X * EPSILON), s(Vec3::Y * EPSILON) - s(-Vec3::Y * EPSILON), s(Vec3::Z * EPSILON) - s(-Vec3::Z * EPSILON)).normalize();
                let tex = light[hc];
                let hit_light = Vec3::new(
                    sample3(uvw, |p| tex.light_at(p).x),
                    sample3(uvw, |p| tex.light_at(p).y),
                    sample3(uvw, |p| tex.light_at(p).z),
                );
                let a = |k: usize| sample3(uvw, |p| tex.aniso_at(p)[k]);
                let aniso0 = Vec3::new(a(0), a(1), a(2));
                let aniso1 = Vec3::new(a(3), a(4), a(5));
                hit_light * ((n * aniso0).max(Vec3::ZERO).dot(Vec3::ONE) + (-n * aniso1).max(Vec3::ZERO).dot(Vec3::ONE))
            }
            None => match sky {
                Sky::Radiance(f) => f(dir),
                Sky::Color(c) => *c,
            },
        };
        rays.push(Ray { direction: dir, hit, light: light_rgb });
    }
    rays
}

/// One probe's 16 coefficients as one frame stores them in the history:
/// `ivec4(sum(L * Y) * 4 / ray_count * 1024)`, clamped to 16 bits (rgb, and
/// a = the ray weight).
pub fn frame_coefficients(rays: &[Ray], ray_count: u32) -> [[i32; 4]; SH_SIZE] {
    let mut sum = [Vec3::ZERO; SH_SIZE];
    for r in rays {
        let y = sh_basis(r.direction);
        for i in 0..SH_SIZE {
            sum[i] += r.light * y[i];
        }
    }
    let scale = 4.0 / ray_count as f32 * (1 << HISTORY_BITS) as f32;
    sum.map(|s| {
        let v = [s.x * scale, s.y * scale, s.z * scale, scale];
        v.map(|c| (c as i32).clamp(-32768, 32767))
    })
}

/// One cascade's probe history: `history_size` slots of 16 coefficients per
/// probe (`lightprobe_history_texture`, R16G16B16A16_SINT) and their running
/// sum (`lightprobe_average_texture`, R32G32B32A32_SINT).
#[derive(Clone)]
pub struct History {
    pub history_size: u32,
    pub layers: Vec<Vec<[[i32; 4]; SH_SIZE]>>,
    pub average: Vec<[[i32; 4]; SH_SIZE]>,
}

pub fn probe_index(p: IVec3) -> usize {
    (p.x + p.y * PROBE_AXIS_COUNT + p.z * PROBE_AXIS_COUNT * PROBE_AXIS_COUNT) as usize
}

pub fn probe_of(i: usize) -> IVec3 {
    let a = PROBE_AXIS_COUNT as usize;
    IVec3::new((i % a) as i32, ((i / a) % a) as i32, (i / (a * a)) as i32)
}

impl History {
    /// Cleared at creation.
    pub fn new(history_size: u32) -> History {
        History { history_size, layers: vec![vec![[[0; 4]; SH_SIZE]; PROBES]; history_size as usize], average: vec![[[0; 4]; SH_SIZE]; PROBES] }
    }

    /// The ring's update for one probe: subtract the slot's old value from
    /// the sum, add the new one, store it.
    pub fn push(&mut self, probe: usize, history_index: u32, value: [[i32; 4]; SH_SIZE]) {
        let slot = &mut self.layers[history_index as usize][probe];
        for i in 0..SH_SIZE {
            for c in 0..4 {
                self.average[probe][i][c] = self.average[probe][i][c] - slot[i][c] + value[i][c];
            }
        }
        *slot = value;
    }

    /// The mean coefficients STORE reads: `average / history_size / 1024`.
    pub fn mean(&self, probe: usize) -> [Vec3; SH_SIZE] {
        let k = self.history_size as f32 * (1 << HISTORY_BITS) as f32;
        self.average[probe].map(|c| Vec3::new(c[0] as f32, c[1] as f32, c[2] as f32) / k)
    }
}

/// `octahedron_encode` of the integrate shader: 0..1 on the map to a direction.
pub fn octahedron_decode(f: Vec2) -> Vec3 {
    let f = f * 2.0 - Vec2::ONE;
    let mut n = Vec3::new(f.x, f.y, 1.0 - f.x.abs() - f.y.abs());
    let t = (-n.z).clamp(0.0, 1.0);
    n.x += if n.x >= 0.0 { -t } else { t };
    n.y += if n.y >= 0.0 { -t } else { t };
    n.normalize()
}

/// STORE's two values for one texel of a probe's map: irradiance
/// (`sum(sh * c * 4 * l_mult)`) and radiance (`sum(sh * c * 4)`).
pub fn store_texel(mean: &[Vec3; SH_SIZE], local: glam::IVec2) -> (Vec3, Vec3) {
    let normal = octahedron_decode(local.as_vec2() / OCT_SIZE as f32);
    let c = sh_basis(normal);
    let mut irradiance = Vec3::ZERO;
    let mut radiance = Vec3::ZERO;
    for i in 0..SH_SIZE {
        let m = mean[i] * c[i] * 4.0;
        irradiance += m * L_MULT[i];
        radiance += m;
    }
    (irradiance, radiance)
}

/// One cascade's two light-probe layers (irradiance, radiance) as E5B9G9R9
/// words, `MAP_WIDTH` x `MAP_HEIGHT`.
#[derive(Clone)]
pub struct ProbeMaps {
    pub irradiance: Vec<u32>,
    pub radiance: Vec<u32>,
}

impl ProbeMaps {
    pub fn empty() -> ProbeMaps {
        ProbeMaps { irradiance: vec![0; (MAP_WIDTH * MAP_HEIGHT) as usize], radiance: vec![0; (MAP_WIDTH * MAP_HEIGHT) as usize] }
    }

    pub fn texel(layer: &[u32], x: i32, y: i32) -> Vec3 {
        decode_e5b9g9r9(layer[(x.clamp(0, MAP_WIDTH - 1) + y.clamp(0, MAP_HEIGHT - 1) * MAP_WIDTH) as usize])
    }

    /// A bilinear read at a texel position (texel centres at k + 0.5), as
    /// `textureLod` with a linear sampler reads it.
    pub fn bilinear(layer: &[u32], x: f32, y: f32) -> Vec3 {
        let tx = x - 0.5;
        let ty = y - 0.5;
        let (x0, y0) = (tx.floor(), ty.floor());
        let (fx, fy) = (tx - x0, ty - y0);
        let (x0, y0) = (x0 as i32, y0 as i32);
        let a = ProbeMaps::texel(layer, x0, y0).lerp(ProbeMaps::texel(layer, x0 + 1, y0), fx);
        let b = ProbeMaps::texel(layer, x0, y0 + 1).lerp(ProbeMaps::texel(layer, x0 + 1, y0 + 1), fx);
        a.lerp(b, fy)
    }
}

/// `MODE_STORE` for every probe of a cascade, with the border copies.
pub fn store(history: &History) -> ProbeMaps {
    let mut maps = ProbeMaps::empty();
    for p in 0..PROBES {
        let probe = probe_of(p);
        let mean = history.mean(p);
        let oct_pos = glam::IVec2::new((probe.x + probe.z * PROBE_AXIS_COUNT) * (OCT_SIZE + 2) + 1, probe.y * (OCT_SIZE + 2) + 1);
        for ly in 0..OCT_SIZE {
            for lx in 0..OCT_SIZE {
                let local = glam::IVec2::new(lx, ly);
                let (irradiance, radiance) = store_texel(&mean, local);
                let (wi, wr) = (encode_e5b9g9r9(irradiance), encode_e5b9g9r9(radiance));
                let n = OCT_SIZE;
                let mut copy_to = vec![local];
                if local == glam::IVec2::new(0, 0) {
                    copy_to.extend([glam::IVec2::new(n - 1, -1), glam::IVec2::new(-1, n - 1), glam::IVec2::new(n, n)]);
                } else if local == glam::IVec2::new(n - 1, 0) {
                    copy_to.extend([glam::IVec2::new(0, -1), glam::IVec2::new(n, n - 1), glam::IVec2::new(-1, n)]);
                } else if local == glam::IVec2::new(0, n - 1) {
                    copy_to.extend([glam::IVec2::new(-1, 0), glam::IVec2::new(n - 1, n), glam::IVec2::new(n, -1)]);
                } else if local == glam::IVec2::new(n - 1, n - 1) {
                    copy_to.extend([glam::IVec2::new(0, n), glam::IVec2::new(n, 0), glam::IVec2::new(-1, -1)]);
                } else if ly == 0 {
                    copy_to.push(glam::IVec2::new(n - lx - 1, ly - 1));
                } else if lx == 0 {
                    copy_to.push(glam::IVec2::new(lx - 1, n - ly - 1));
                } else if ly == n - 1 {
                    copy_to.push(glam::IVec2::new(n - lx - 1, ly + 1));
                } else if lx == n - 1 {
                    copy_to.push(glam::IVec2::new(lx + 1, n - ly - 1));
                }
                for t in copy_to {
                    let q = oct_pos + t;
                    let i = (q.x + q.y * MAP_WIDTH) as usize;
                    maps.irradiance[i] = wi;
                    maps.radiance[i] = wr;
                }
            }
        }
    }
    maps
}

/// The probe maps of every cascade, for the light pass's bounce.
pub struct Maps<'a>(pub Vec<&'a ProbeMaps>);

impl ProbeLookup for Maps<'_> {
    fn irradiance(&self, cascade: usize, pos_uvw: Vec3) -> Vec3 {
        ProbeMaps::bilinear(&self.0[cascade].irradiance, pos_uvw.x, pos_uvw.y)
    }
}

/// `INTEGRATE_MODE_SCROLL` then `INTEGRATE_MODE_SCROLL_STORE` for a cascade
/// that moved by `scroll` probes: a probe whose old position is inside the
/// grid takes its history; a newly exposed one is seeded, in every slot,
/// with the trilinear blend of the coarser cascade's average around it
/// (read through `parent`, its history before this frame's scroll); the last
/// cascade's new probes keep what was there. `ubo` holds the cascades'
/// offsets as the shader reads them: the UBO is rewritten only after the
/// regions are processed (rfc.cpp), so these are the previous frame's.
pub fn scroll_history(history: &History, parent: Option<&History>, scroll: IVec3, ubo: &[(Vec3, f32)], cascade: usize) -> History {
    let mut out = history.clone();
    let axis = PROBE_AXIS_COUNT;
    let h = history.history_size as f32 * (1 << HISTORY_BITS) as f32;
    for p in 0..PROBES {
        let probe = probe_of(p);
        let read = probe - scroll;
        if read.cmpge(IVec3::ZERO).all() && read.cmplt(IVec3::splat(axis)).all() {
            let r = probe_index(read);
            for j in 0..history.history_size as usize {
                out.layers[j][p] = history.layers[j][r];
            }
            out.average[p] = history.average[r];
        } else if let Some(parent) = parent {
            let cell_to_probe = CASCADE_SIZE as f32 / (axis - 1) as f32;
            let probe_cell_size = cell_to_probe / ubo[cascade].1;
            let mut pos = ubo[cascade].0 + probe.as_vec3() * probe_cell_size;
            let probe_cell_size_next = cell_to_probe / ubo[cascade + 1].1;
            pos -= ubo[cascade + 1].0;
            pos /= probe_cell_size_next;
            let posi = pos.as_ivec3();
            let mut average = [glam::Vec4::ZERO; SH_SIZE];
            let mut total = 0.0;
            for i in 0..8 {
                let offset = posi + IVec3::new(i & 1, (i >> 1) & 1, (i >> 2) & 1);
                let t = Vec3::ONE - (pos - offset.as_vec3()).abs();
                let weight = t.x * t.y * t.z;
                // An image load outside the texture reads zero.
                let inside = offset.cmpge(IVec3::ZERO).all() && offset.cmplt(IVec3::splat(axis)).all();
                for j in 0..SH_SIZE {
                    let a = if inside { parent.average[probe_index(offset)][j] } else { [0; 4] };
                    average[j] += glam::Vec4::new(a[0] as f32, a[1] as f32, a[2] as f32, a[3] as f32) / h * weight;
                }
                total += weight;
            }
            if total > 0.0 {
                total = 1.0 / total;
            }
            let mut value = [[0i32; 4]; SH_SIZE];
            for j in 0..SH_SIZE {
                let v = average[j] * total * (1 << HISTORY_BITS) as f32;
                value[j] = [v.x, v.y, v.z, v.w].map(|c| (c as i32).clamp(-32768, 32767));
            }
            for layer in out.layers.iter_mut() {
                layer[p] = value;
            }
            out.average[p] = value.map(|c| c.map(|x| x * history.history_size as i32));
        }
    }
    out
}
