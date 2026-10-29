//! Light in the voxels on the CPU: sdfgi_direct_light.glsl, its dynamic pass
//! (`MODE_PROCESS_DYNAMIC`, which writes the cascade's light textures and
//! fills the cell's neighbours) and its static pass (`MODE_PROCESS_STATIC`,
//! which writes the light back into the solid cell), with the area light's
//! diffuse integral of area_lights_inc.glsl (`ltc_evaluate_diff`).
//!
//! Positions are in the squashed space the shader works in (world y times
//! `y_mult`). The light-probe bounce reads the probe maps through
//! `ProbeLookup`; use_occlusion is off (its default), so the bounce has no
//! occlusion term here.
//!
//! Where the lab differs: textures are sampled with full-precision weights
//! (hardware filtering keeps 8 bits of fraction).
//!
//! Ported from Godot Engine (MIT; Copyright (c) 2014-present Godot Engine
//! contributors, Copyright (c) 2007-2014 Juan Linietsky, Ariel Manzur).

use super::model::{Cascades, CASCADE_SIZE, PROBE_AXIS_COUNT};
use super::voxels::{decode_rgbe8985, index, inside, shared_exponent, SolidCell, ANISO_DIR, CELLS, N};
use glam::{IVec3, Vec3};

pub const LIGHT_TYPE_DIRECTIONAL: u32 = 0;
pub const LIGHT_TYPE_OMNI: u32 = 1;
pub const LIGHT_TYPE_SPOT: u32 = 2;
pub const LIGHT_TYPE_AREA: u32 = 3;

/// `SDFGIShader::Light` as the shader reads it (sdfgi_direct_light.glsl
/// `struct Light`), in the squashed space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Light {
    pub kind: u32,
    pub color: Vec3,
    pub energy: f32,
    pub direction: Vec3,
    pub position: Vec3,
    pub attenuation: f32,
    pub radius: f32,
    pub cos_spot_angle: f32,
    pub inv_spot_attenuation: f32,
    pub area_width: Vec3,
    pub area_height: Vec3,
}

/// One cascade's light textures: `light_data` (R32_UINT, read through its
/// E5B9G9R9 view), `light_aniso_0` (RGBA8_UNORM), `light_aniso_1` (RG8_UNORM).
#[derive(Clone)]
pub struct LightTextures {
    pub light: Vec<u32>,
    pub aniso0: Vec<[u8; 4]>,
    pub aniso1: Vec<[u8; 2]>,
}

impl LightTextures {
    /// `render_region` clears them after STORE.
    pub fn cleared() -> LightTextures {
        LightTextures { light: vec![0; CELLS], aniso0: vec![[0; 4]; CELLS], aniso1: vec![[0; 2]; CELLS] }
    }

    pub fn light_at(&self, p: IVec3) -> Vec3 {
        decode_e5b9g9r9(self.light[index(p)])
    }

    /// The six face weights at a texel (+x, +y, +z, -x, -y, -z).
    pub fn aniso_at(&self, p: IVec3) -> [f32; 6] {
        let a = self.aniso0[index(p)];
        let b = self.aniso1[index(p)];
        [a[0], a[1], a[2], a[3], b[0], b[1]].map(|v| v as f32 / 255.0)
    }
}

/// The E5B9G9R9_UFLOAT word: R bits 0-8, G 9-17, B 18-26, E 27-31.
pub fn encode_e5b9g9r9(light: Vec3) -> u32 {
    let (s, exps) = shared_exponent(light);
    (s[0] & 0x1FF) | ((s[1] & 0x1FF) << 9) | ((s[2] & 0x1FF) << 18) | ((exps & 0x1F) << 27)
}

pub fn decode_e5b9g9r9(word: u32) -> Vec3 {
    let m = 2f32.powi(((word >> 27) & 0x1F) as i32 - 15 - 9);
    Vec3::new((word & 0x1FF) as f32, ((word >> 9) & 0x1FF) as f32, ((word >> 18) & 0x1FF) as f32) * m
}

fn unorm8(v: f32) -> u8 {
    // NaN (a cell with no light: 0 / 0) is stored as 0, as GPUs do.
    if v.is_nan() { 0 } else { (v.clamp(0.0, 1.0) * 255.0).round() as u8 }
}

/// A trilinear `texture()` read of an `N`^3 image at `uvw` (0..1), clamped to
/// the edge, of the values `at` gives per texel.
pub fn sample3(uvw: Vec3, at: impl Fn(IVec3) -> f32) -> f32 {
    let t = uvw * N as f32 - Vec3::splat(0.5);
    let base = t.floor();
    let f = t - base;
    let b = base.as_ivec3();
    let mut sum = 0.0;
    for i in 0..8 {
        let o = IVec3::new(i & 1, (i >> 1) & 1, (i >> 2) & 1);
        let p = (b + o).clamp(IVec3::ZERO, IVec3::splat(CASCADE_SIZE - 1));
        let w = Vec3::new(if o.x == 1 { f.x } else { 1.0 - f.x }, if o.y == 1 { f.y } else { 1.0 - f.y }, if o.z == 1 { f.z } else { 1.0 - f.z });
        sum += w.x * w.y * w.z * at(p);
    }
    sum
}

/// The SDF read of the marches: `texture(sdf).r` (the UNORM byte over 255).
pub fn sample_sdf(sdf: &[u8], uvw: Vec3) -> f32 {
    sample3(uvw, |p| sdf[index(p)] as f32 / 255.0)
}

/// `get_omni_attenuation`: (1 - (d/r)^4)^2 * d^-decay.
pub fn omni_attenuation(distance: f32, inv_range: f32, decay: f32) -> f32 {
    let mut nd = distance * inv_range;
    nd *= nd;
    nd *= nd;
    nd = (1.0 - nd).max(0.0);
    nd *= nd;
    nd * distance.max(0.0001).powf(-decay)
}

/// The probe maps the bounce reads: the light-probe texture's irradiance layer
/// of a cascade, sampled at a texel position (bilinear, as `textureLod`).
pub trait ProbeLookup {
    fn irradiance(&self, cascade: usize, pos_uvw: Vec3) -> Vec3;
}

/// What the light pass reads besides the cell: every cascade's offset,
/// `to_cell`, and distance field.
pub struct Field<'a> {
    pub cascades: &'a Cascades,
    pub sdf: Vec<&'a [u8]>,
}

impl Field<'_> {
    /// `cascades.data[j].offset`: cell (0,0,0)'s corner, squashed space.
    pub fn offset(&self, j: usize) -> Vec3 {
        let c = &self.cascades.cascades[j];
        (IVec3::splat(-(CASCADE_SIZE >> 1)) + c.position).as_vec3() * c.cell_size
    }

    pub fn to_cell(&self, j: usize) -> f32 {
        1.0 / self.cascades.cascades[j].cell_size
    }
}

/// The march of one shadow ray (sdfgi_direct_light.glsl): from the cell's
/// centre, biased toward the light, through this cascade and the coarser
/// ones, stepping by the field. True when a solid sample (below 0.001 cell)
/// is reached before the light.
pub fn shadow_ray(field: &Field, cascade: usize, position: Vec3, direction: Vec3, light_distance: f32, steps: Option<&mut Vec<(usize, Vec3, f32)>>) -> bool {
    let grid = CASCADE_SIZE as f32;
    let inv_dir = Vec3::ONE / direction;
    let cell_size = 1.0 / field.to_cell(cascade);
    let mut ray_pos = position + direction.signum() * cell_size * 0.48;
    ray_pos += direction * 0.4 * cell_size;
    let mut light_distance = light_distance;
    let mut steps = steps;
    for j in cascade..field.cascades.cascades.len() {
        let mut pos = (ray_pos - field.offset(j)) * field.to_cell(j);
        let local_distance = light_distance * field.to_cell(j);
        if pos.cmplt(Vec3::ZERO).any() || pos.cmpge(Vec3::splat(grid)).any() {
            continue;
        }
        let t0 = -pos * inv_dir;
        let t1 = (Vec3::splat(grid) - pos) * inv_dir;
        let tmax = t0.max(t1);
        let max_advance = tmax.min_element().min(local_distance);
        let mut advance = 0.0;
        while advance < max_advance {
            let uvw = (pos + direction * advance) / grid;
            let distance = sample_sdf(field.sdf[j], uvw) * 255.0 - 1.0;
            if let Some(s) = steps.as_deref_mut() {
                s.push((j, pos + direction * advance, distance));
            }
            if distance < 0.001 {
                return true;
            }
            advance += distance;
        }
        if advance >= local_distance {
            break;
        }
        pos += direction * max_advance;
        pos /= field.to_cell(j);
        pos += field.offset(j);
        light_distance -= max_advance / field.to_cell(j);
        ray_pos = pos;
    }
    false
}

/// `ltc_evaluate_diff` with no projector texture: the integral over the
/// light's quad (clipped to the horizon of `normal`) of the cosine to
/// `normal`, not divided by 2 pi (area_lights_inc.glsl: "no division by
/// (2.0 * M_PI) for GI calculations").
pub fn ltc_diffuse(normal: Vec3, points: [Vec3; 4]) -> f32 {
    let eye_vec = if normal.z.abs() < 0.7 { Vec3::new(0.0, 0.0, -1.0) } else { Vec3::X };
    let z = -(eye_vec - normal * eye_vec.dot(normal)).normalize();
    let x = normal.cross(z);
    // M_inv = transpose(mat3(x, normal, z)): rows x, normal, z.
    let rot = |p: Vec3| Vec3::new(x.dot(p), normal.dot(p), z.dot(p));
    let mut l = [rot(points[0]), rot(points[1]), rot(points[2]), rot(points[3]), Vec3::ZERO];
    let n = clip_quad_to_horizon(&mut l);
    if n == 0 {
        return 0.0;
    }
    let proj = l.map(|v| v.normalize_or_zero());
    let pnorm = (proj[0] - proj[1]).cross(proj[2] - proj[1]).normalize_or_zero();
    if pnorm.dot(proj[0]).abs() < 1e-10 {
        return 0.0;
    }
    let mut i = integrate_edge(proj[0], proj[1], l[0], l[1]);
    i += integrate_edge(proj[1], proj[2], l[1], l[2]);
    i += integrate_edge(proj[2], proj[3], l[2], l[3]);
    if n >= 4 {
        i += integrate_edge(proj[3], proj[4], l[3], l[4]);
    }
    if n == 5 {
        i += integrate_edge(proj[4], proj[0], l[4], l[0]);
    }
    i.abs()
}

/// `integrate_edge_hill`'s y component: Hill and Heitz's fit of the edge
/// integral of the clamped cosine.
fn edge_hill(p0: Vec3, p1: Vec3) -> f32 {
    let x = p0.dot(p1);
    let y = x.abs();
    let a = 5.42031 + (3.12829 + 0.0902326 * y) * y;
    let b = 3.45068 + (4.18814 + y) * y;
    let mut theta_sintheta = a / b;
    if x < 0.0 {
        theta_sintheta = std::f32::consts::PI / (1.0 - x * x).sqrt() - theta_sintheta;
    }
    theta_sintheta * p0.cross(p1).y
}

fn integrate_edge(p_proj0: Vec3, p_proj1: Vec3, p0: Vec3, p1: Vec3) -> f32 {
    let epsilon = 0.00001;
    if p_proj0.dot(p_proj1) < -1.0 + epsilon {
        let half_point_t = p0 + (p1 - p0).normalize() * p0.dot((p0 - p1).normalize());
        let half_point = half_point_t.normalize();
        edge_hill(p_proj0, half_point) + edge_hill(half_point, p_proj1)
    } else {
        edge_hill(p_proj0, p_proj1)
    }
}

/// `clip_quad_to_horizon`: clips the quad to y > 0, returning its vertex count.
fn clip_quad_to_horizon(l: &mut [Vec3; 5]) -> usize {
    let mut config = 0;
    for (k, bit) in [1, 2, 4, 8].iter().enumerate() {
        if l[k].y > 0.0 {
            config += bit;
        }
    }
    let lerp = |a: Vec3, b: Vec3| -a.y * b + b.y * a;
    let n = match config {
        1 => {
            l[1] = lerp(l[1], l[0]);
            l[2] = lerp(l[3], l[0]);
            3
        }
        2 => {
            l[0] = lerp(l[0], l[1]);
            l[2] = lerp(l[2], l[1]);
            3
        }
        3 => {
            l[2] = lerp(l[2], l[1]);
            l[3] = lerp(l[3], l[0]);
            4
        }
        4 => {
            l[0] = lerp(l[3], l[2]);
            l[1] = lerp(l[1], l[2]);
            3
        }
        6 => {
            l[0] = lerp(l[0], l[1]);
            l[3] = lerp(l[3], l[2]);
            4
        }
        7 => {
            l[4] = lerp(l[3], l[0]);
            l[3] = lerp(l[3], l[2]);
            5
        }
        8 => {
            l[0] = lerp(l[0], l[3]);
            l[1] = lerp(l[2], l[3]);
            l[2] = l[3];
            3
        }
        9 => {
            l[1] = lerp(l[1], l[0]);
            l[2] = lerp(l[2], l[3]);
            4
        }
        11 => {
            l[4] = l[3];
            l[3] = lerp(l[2], l[3]);
            l[2] = lerp(l[2], l[1]);
            5
        }
        12 => {
            l[1] = lerp(l[1], l[2]);
            l[0] = lerp(l[0], l[3]);
            4
        }
        13 => {
            l[4] = l[3];
            l[3] = l[2];
            l[2] = lerp(l[1], l[2]);
            l[1] = lerp(l[1], l[0]);
            5
        }
        14 => {
            l[4] = lerp(l[0], l[3]);
            l[0] = lerp(l[0], l[1]);
            5
        }
        15 => 4,
        _ => 0,
    };
    if n == 3 {
        l[3] = l[0];
    }
    if n == 4 {
        l[4] = l[0];
    }
    n
}

/// `compute_area_light`: the closest point of the light's rectangle, the
/// window of its range, and the diffuse integral seen from the cell toward it.
/// Returns (attenuation, direction to the light, distance).
pub fn area_light(light: &Light, position: Vec3) -> (f32, Vec3, f32) {
    let a_len = light.area_width.length();
    let b_len = light.area_height.length();
    let wn = light.area_width.normalize();
    let hn = light.area_height.normalize();
    let to_vert = position - light.position;
    let local = Vec3::new(to_vert.dot(wn), to_vert.dot(hn), to_vert.dot(-light.direction));
    let closest_local = Vec3::new(local.x.clamp(-a_len / 2.0, a_len / 2.0), local.y.clamp(-b_len / 2.0, b_len / 2.0), 0.0);
    // inv_spot_attenuation carries 1 / (radius + |area_size| / 2) for area lights.
    let inv_center_range = light.inv_spot_attenuation;
    let closest = light.position + closest_local.x * wn + closest_local.y * hn;
    let rel = closest - position;
    let distance = rel.length();
    let mut direction = rel / distance;
    if distance < 1e-4 {
        direction = light.direction;
    }
    if distance * inv_center_range >= 1.0 {
        return (0.0, direction, distance);
    }
    let hw = light.area_width / 2.0;
    let hh = light.area_height / 2.0;
    let points = [light.position - hw - hh - position, light.position + hw - hh - position, light.position + hw + hh - position, light.position - hw + hh - position];
    let mut attenuation = omni_attenuation(distance, 1.0 / light.radius, light.attenuation - 2.0);
    attenuation *= ltc_diffuse(direction, points);
    (attenuation, direction, distance)
}

/// A light's direction, distance, and attenuation at a cell, or `None` when
/// the shader skips it (outside a spot's cone, behind an area light, below
/// 0.001).
pub fn light_at(light: &Light, position: Vec3) -> Option<(Vec3, f32, f32)> {
    let (direction, distance, attenuation) = match light.kind {
        LIGHT_TYPE_DIRECTIONAL => (-light.direction, 1e20, 1.0),
        LIGHT_TYPE_OMNI | LIGHT_TYPE_SPOT => {
            let rel = light.position - position;
            let direction = rel.normalize();
            let distance = rel.length();
            let mut attenuation = omni_attenuation(distance, 1.0 / light.radius, light.attenuation);
            if light.kind == LIGHT_TYPE_SPOT {
                let cos_angle = (-direction).dot(light.direction);
                if cos_angle < light.cos_spot_angle {
                    return None;
                }
                let scos = cos_angle.max(light.cos_spot_angle);
                let spot_rim = ((1.0 - scos) / (1.0 - light.cos_spot_angle)).max(0.0001);
                attenuation *= 1.0 - spot_rim.powf(light.inv_spot_attenuation);
            }
            (direction, distance, attenuation)
        }
        _ => {
            if light.area_width.length_squared() < 1e-7 || light.area_height.length_squared() < 1e-7 {
                return None;
            }
            if light.direction.dot(position - light.position) <= 0.0 {
                return None;
            }
            let (attenuation, direction, distance) = area_light(light, position);
            (direction, distance, attenuation)
        }
    };
    if attenuation < 0.001 { None } else { Some((direction, distance, attenuation)) }
}

/// What one cell of the light pass computes, before it is stored.
#[derive(Clone, Debug, Default)]
pub struct CellLight {
    /// Light per face (+x, +y, +z, -x, -y, -z).
    pub accum: [Vec3; 6],
    /// The bounce's part of it (dynamic pass only).
    pub bounce: [Vec3; 6],
    /// The stored light's part (emission, and the static lights).
    pub stored: [Vec3; 6],
}

/// The light pass for one solid cell (`main` of sdfgi_direct_light.glsl),
/// up to the per-face sums.
pub fn cell_light(field: &Field, cascade: usize, cell: &SolidCell, lights: &[Light], bounce_feedback: f32, probes: Option<&dyn ProbeLookup>) -> CellLight {
    let positioni = cell.cell();
    let position = (positioni.as_vec3() + Vec3::splat(0.5)) / field.to_cell(cascade) + field.offset(cascade);
    let albedo = cell.albedo_rgb();
    let valid_aniso = cell.facing();
    let mut out = CellLight::default();
    // The bounce from the probes (dynamic pass, bounce_feedback above 0.001).
    if let Some(probes) = probes
        && bounce_feedback > 0.001
    {
        let feedback = if bounce_feedback < 1.0 { albedo * bounce_feedback } else { albedo.lerp(Vec3::ONE, bounce_feedback - 1.0) };
        let pos = (positioni.as_vec3() + Vec3::splat(0.5)) * (PROBE_AXIS_COUNT - 1) as f32 / CASCADE_SIZE as f32;
        let base = pos.as_ivec3();
        let mut weight_accum = [0.0f32; 6];
        for j in 0..8 {
            let offset = IVec3::new(j & 1, (j >> 1) & 1, (j >> 2) & 1);
            let probe = base + offset;
            let probe_to_pos = pos - probe.as_vec3();
            let probe_dir = (-probe_to_pos).normalize();
            let trilinear = Vec3::ONE - probe_to_pos.abs();
            for (k, n) in ANISO_DIR.iter().enumerate() {
                if valid_aniso & (1 << k) == 0 {
                    continue;
                }
                let weight = trilinear.x * trilinear.y * trilinear.z * n.dot(probe_dir).max(0.0);
                if weight > 0.0 {
                    // Texel position of the probe's octahedral map at n (lightprobe_texture, layer = cascade).
                    let oct = octahedron_encode(*n) * 6.0;
                    let tex = Vec3::new(((probe.x + probe.z * PROBE_AXIS_COUNT) * 8 + 1) as f32 + oct.x, (probe.y * 8 + 1) as f32 + oct.y, cascade as f32);
                    let indirect = probes.irradiance(cascade, tex);
                    out.bounce[k] += indirect * weight;
                    weight_accum[k] += weight;
                }
            }
        }
        for k in 0..6 {
            if weight_accum[k] > 0.0 {
                out.bounce[k] /= weight_accum[k];
                out.bounce[k] *= feedback;
            }
        }
        out.accum = out.bounce;
    }
    // The stored light (emission, and any static light written back).
    let l = decode_rgbe8985(cell.light & 0x3FFF_FFFF);
    for (i, s) in out.stored.iter_mut().enumerate() {
        let strength = ((cell.light_aniso >> (i * 5)) & 0x1F) as f32 / 31.0;
        *s = l * strength;
        out.accum[i] += *s;
    }
    // The lights.
    for light in lights {
        let Some((direction, distance, attenuation)) = light_at(light, position) else { continue };
        if shadow_ray(field, cascade, position, direction, distance, None) {
            continue;
        }
        let light = albedo * light.color * light.energy * attenuation;
        for (j, dir) in ANISO_DIR.iter().enumerate() {
            if valid_aniso & (1 << j) != 0 {
                out.accum[j] += dir.dot(direction).max(0.0) * light;
            }
        }
    }
    out
}

/// The per-face ratios and the total, as both passes pack them.
pub fn pack(accum: &[Vec3; 6]) -> (Vec3, [f32; 6], f32) {
    let total: Vec3 = accum.iter().copied().sum();
    let lumas = accum.map(|l| l.max_element());
    (total, lumas, total.max_element())
}

/// `octahedron_encode`: a direction to 0..1 on the octahedral map.
pub fn octahedron_encode(n: Vec3) -> glam::Vec2 {
    let n = n / (n.x.abs() + n.y.abs() + n.z.abs());
    let xy = if n.z >= 0.0 {
        glam::Vec2::new(n.x, n.y)
    } else {
        let sign = glam::Vec2::new(if n.x >= 0.0 { 1.0 } else { -1.0 }, if n.y >= 0.0 { 1.0 } else { -1.0 });
        (glam::Vec2::ONE - glam::Vec2::new(n.y.abs(), n.x.abs())) * sign
    };
    xy * 0.5 + glam::Vec2::splat(0.5)
}

/// `MODE_PROCESS_DYNAMIC` over the cells this frame updates (every
/// `increment`-th, from `offset`), writing the cascade's light textures and
/// each cell's neighbours.
pub fn dynamic_pass(field: &Field, cascade: usize, cells: &[SolidCell], lights: &[Light], bounce_feedback: f32, probes: Option<&dyn ProbeLookup>, offset: usize, increment: usize, textures: &mut LightTextures) {
    const NEIGHBOURS: [IVec3; 26] = {
        let mut v = [IVec3::ZERO; 26];
        let mut k = 0;
        let mut x = -1;
        while x <= 1 {
            let mut y = -1;
            while y <= 1 {
                let mut z = -1;
                while z <= 1 {
                    if !(x == 0 && y == 0 && z == 0) {
                        v[k] = IVec3::new(x, y, z);
                        k += 1;
                    }
                    z += 1;
                }
                y += 1;
            }
            x += 1;
        }
        v
    };
    let mut i = offset;
    while i < cells.len() {
        let cell = &cells[i];
        let result = cell_light(field, cascade, cell, lights, bounce_feedback, probes);
        let (total, lumas, luma_total) = pack(&result.accum);
        let word = encode_e5b9g9r9(total);
        let a0 = [unorm8(lumas[0] / luma_total), unorm8(lumas[1] / luma_total), unorm8(lumas[2] / luma_total), unorm8(lumas[3] / luma_total)];
        let a1 = [unorm8(lumas[4] / luma_total), unorm8(lumas[5] / luma_total)];
        let p = cell.cell();
        let mut write = |q: IVec3| {
            if inside(q) {
                textures.light[index(q)] = word;
                textures.aniso0[index(q)] = a0;
                textures.aniso1[index(q)] = a1;
            }
        };
        write(p);
        let neighbours = cell.neighbours();
        for (n, offset) in NEIGHBOURS.iter().enumerate() {
            if neighbours & (1 << n) != 0 {
                write(p + *offset);
            }
        }
        i += increment.max(1);
    }
}

/// `MODE_PROCESS_STATIC` over every cell: adds the static lights to the
/// cell's stored light and writes the sum back into the cell (RGBE8985).
pub fn static_pass(field: &Field, cascade: usize, cells: &mut [SolidCell], lights: &[Light]) {
    for i in 0..cells.len() {
        let result = cell_light(field, cascade, &cells[i], lights, 0.0, None);
        let (total, lumas, luma_total) = pack(&result.accum);
        let (s, exps) = shared_exponent(total);
        let rgbe = ((s[0] & 0x1FF) >> 1) | ((s[1] & 0x1FF) << 8) | (((s[2] & 0x1FF) >> 1) << 17) | ((exps & 0x1F) << 25);
        let cell = &mut cells[i];
        cell.light = (cell.light & (3 << 30)) | rgbe;
        let mut aniso = cell.light_aniso & (3 << 30);
        for (k, l) in lumas.iter().enumerate() {
            // uint(NaN) is 0 on the GPUs the lab knows; Rust's `as` saturates NaN to 0 too.
            aniso |= (((l / luma_total) * 31.0) as u32).min(31) << (k * 5);
        }
        cell.light_aniso = aniso;
    }
}
