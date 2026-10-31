//! The light passes over the baked octree, on the CPU, as
//! `GI::VoxelGIInstance::update` runs them (servers/rendering/renderer_rd/
//! environment/gi.cpp) with voxel_gi.glsl's static modes: `MODE_COMPUTE_LIGHT`
//! on the leaf cells (each light's attenuation, the back-face test, a shadow
//! march through the baked distance field, the area light's LTC integral,
//! times albedo, plus emission), `MODE_UPDATE_MIPMAPS` up the levels,
//! `MODE_WRITE_TEXTURE` into the RGBA8 3D texture, and, with two bounces,
//! `MODE_SECOND_BOUNCE`: six cones per leaf through that texture, then the
//! mips and the texture again.
//!
//! Everything is in cell units, as the shader's: positions are cells,
//! `params.cell_size` metres per cell.
//!
//! Ported from Godot Engine (MIT; Copyright (c) 2014-present Godot Engine
//! contributors, Copyright (c) 2007-2014 Juan Linietsky, Ariel Manzur).

use super::bake::CHILD_EMPTY;
use super::storage::{LightTexture, Probe};
use glam::{IVec3, Vec3, Vec4};

pub const LIGHT_TYPE_DIRECTIONAL: u32 = 0;
pub const LIGHT_TYPE_OMNI: u32 = 1;
pub const LIGHT_TYPE_SPOT: u32 = 2;
pub const LIGHT_TYPE_AREA: u32 = 3;

/// `voxel_gi_max_lights` (gi.h): the light buffer holds 32.
pub const MAX_LIGHTS: usize = 32;

/// `VoxelGILight` as `update` fills it: positions, directions, the range,
/// and the area's sides in the probe's cell space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Light {
    pub kind: u32,
    pub energy: f32,
    pub radius: f32,
    pub attenuation: f32,
    pub color: Vec3,
    pub cos_spot_angle: f32,
    pub position: Vec3,
    pub inv_spot_attenuation: f32,
    pub direction: Vec3,
    pub has_shadow: bool,
    pub area_width: Vec3,
    pub area_height: Vec3,
}

/// The push constant's values the passes read (`VoxelGIPushConstant`).
#[derive(Clone, Copy, Debug)]
pub struct Params {
    pub limits: IVec3,
    pub propagation: f32,
    pub dynamic_range: f32,
    /// Metres per cell: `1 / max(octree_size / bounds.size)`.
    pub cell_size: f32,
}

/// `texture(sampler3D(texture_sdf, texture_sampler), uvw).r`: the R8_UNORM
/// field, trilinear, clamped to the edge.
pub fn sample_sdf(probe: &Probe, uvw: Vec3) -> f32 {
    let s = probe.octree_size.as_vec3();
    let t = uvw * s - Vec3::splat(0.5);
    let b = t.floor();
    let f = t - b;
    let bi = b.as_ivec3();
    let mut sum = 0.0;
    for i in 0..8 {
        let o = IVec3::new(i & 1, (i >> 1) & 1, (i >> 2) & 1);
        let w = Vec3::select(o.cmpeq(IVec3::ONE), f, Vec3::ONE - f);
        sum += w.x * w.y * w.z * probe.sdf_at(bi + o) as f32 / 255.0;
    }
    sum
}

/// One step of `raymarch`, for a demonstration: where it read, the advance
/// it read there.
#[derive(Clone, Copy, Debug)]
pub struct MarchStep {
    pub from: Vec3,
    pub advance: f32,
}

/// `raymarch`: from `from` toward the cell along `direction`, `distance`
/// cells, reading the field's distance at each step (`* 255 - 1`); a
/// negative one is solid (occluded); steps are whole multiples of
/// `distance_adv`, at least one.
pub fn raymarch(probe: &Probe, mut distance: f32, distance_adv: f32, mut from: Vec3, direction: Vec3, mut steps: Option<&mut Vec<MarchStep>>) -> f32 {
    let cell_size = Vec3::ONE / probe.octree_size.as_vec3();
    let mut occlusion = 1.0f32;
    while distance > 0.5 {
        let mut advance = sample_sdf(probe, from * cell_size) * 255.0 - 1.0;
        if advance < 0.0 {
            occlusion = 0.0;
            if let Some(s) = steps.as_deref_mut() {
                s.push(MarchStep { from, advance });
            }
            break;
        }
        occlusion = advance.min(occlusion);
        if let Some(s) = steps.as_deref_mut() {
            s.push(MarchStep { from, advance });
        }
        advance = distance_adv.max(advance - glsl_mod(advance, distance_adv));
        from += direction * advance;
        distance -= advance;
    }
    occlusion
}

/// GLSL's `mod`: `x - y * floor(x / y)`.
pub fn glsl_mod(x: f32, y: f32) -> f32 {
    x - y * (x / y).floor()
}

/// `get_omni_attenuation`.
pub fn omni_attenuation(distance: f32, inv_range: f32, decay: f32) -> f32 {
    let mut nd = distance * inv_range;
    nd *= nd;
    nd *= nd;
    nd = (1.0 - nd).max(0.0);
    nd *= nd;
    nd * distance.max(0.0001).powf(-decay)
}

/// `compute_light_vector`: the attenuation at `pos` and the light's position
/// (a directional light's is far up its direction).
pub fn light_vector(light: &Light, params: &Params, pos: Vec3) -> Option<(f32, Vec3)> {
    if light.kind == LIGHT_TYPE_DIRECTIONAL {
        return Some((1.0, pos - light.direction * params.limits.as_vec3().length()));
    }
    let light_pos = light.position;
    let distance = (pos - light_pos).length();
    if distance >= light.radius {
        return None;
    }
    let mut attenuation = omni_attenuation(distance * params.cell_size, 1.0 / (light.radius * params.cell_size), light.attenuation);
    if light.kind == LIGHT_TYPE_SPOT {
        let rel = (pos - light_pos).normalize();
        let cos_angle = rel.dot(light.direction);
        if cos_angle < light.cos_spot_angle {
            return None;
        }
        let scos = cos_angle.max(light.cos_spot_angle);
        let spot_rim = ((1.0 - scos) / (1.0 - light.cos_spot_angle)).max(0.0001);
        attenuation *= 1.0 - spot_rim.powf(light.inv_spot_attenuation);
    }
    Some((attenuation, light_pos))
}

/// `get_normal_advance`: the path length of one cell along the dominant axis.
pub fn normal_advance(normal: Vec3) -> f32 {
    let u = normal.abs();
    let axis = if u.x >= u.y && u.x >= u.z {
        Vec3::new(normal.x.signum_or_plus(), 0.0, 0.0)
    } else if u.y > u.x && u.y >= u.z {
        Vec3::new(0.0, normal.y.signum_or_plus(), 0.0)
    } else if u.z > u.x && u.z > u.y {
        Vec3::new(0.0, 0.0, normal.z.signum_or_plus())
    } else {
        Vec3::X
    };
    1.0 / normal.dot(axis)
}

/// `v > 0 ? 1 : -1`, as the shader picks the axis's sign.
trait SignumOrPlus {
    fn signum_or_plus(self) -> f32;
}

impl SignumOrPlus for f32 {
    fn signum_or_plus(self) -> f32 {
        if self > 0.0 { 1.0 } else { -1.0 }
    }
}

/// `clip_segment`.
fn clip_segment(plane: Vec4, begin: Vec3, end: &mut Vec3) {
    let segment = begin - *end;
    let den = plane.truncate().dot(segment);
    if den < 0.0001 {
        return;
    }
    let dist = (plane.truncate().dot(begin) - plane.w) / den;
    if !(0.0001..=1.0001).contains(&dist) {
        return;
    }
    *end = begin + segment * -dist;
}

/// What `compute_light_at_pos` worked out, for a demonstration.
#[derive(Clone, Debug, Default)]
pub struct LightTrace {
    pub attenuation: f32,
    pub light_dir: Vec3,
    /// The shadow ray: where the march started (toward the cell) and its steps.
    pub march_start: Option<Vec3>,
    pub march_end: Option<Vec3>,
    pub steps: Vec<MarchStep>,
    pub occlusion: f32,
}

/// `compute_light_at_pos`: the light reaching `pos` from a punctual light
/// (its colour x attenuation x energy), and its direction, or `None` when it
/// is out of range, behind the cell, or shadowed.
pub fn light_at_pos(probe: &Probe, params: &Params, light: &Light, pos: Vec3, normal: Vec3, mut trace: Option<&mut LightTrace>) -> Option<(Vec3, Vec3)> {
    let (mut attenuation, mut light_pos) = light_vector(light, params, pos)?;
    let light_dir = (pos - light_pos).normalize();
    if let Some(t) = trace.as_deref_mut() {
        t.attenuation = attenuation;
        t.light_dir = light_dir;
        t.occlusion = 1.0;
    }
    if attenuation < 0.01 || (normal.length() > 0.2 && normal.dot(light_dir) >= 0.0) {
        return None;
    }
    if light.has_shadow {
        let distance_adv = normal_advance(light_dir);
        let mut to = pos;
        if normal.length() > 0.2 {
            to += normal * distance_adv * 0.51;
        } else {
            to -= Vec3::new(light_dir.x.signum0(), light_dir.y.signum0(), light_dir.z.signum0()) * 0.45;
        }
        let l = params.limits;
        let pick = |neg: bool, a: Vec4, b: Vec4| if neg { b } else { a };
        clip_segment(pick(light_dir.x < 0.0, Vec4::new(-1.0, 0.0, 0.0, 0.0), Vec4::new(1.0, 0.0, 0.0, (l.x - 1) as f32)), to, &mut light_pos);
        clip_segment(pick(light_dir.y < 0.0, Vec4::new(0.0, -1.0, 0.0, 0.0), Vec4::new(0.0, 1.0, 0.0, (l.y - 1) as f32)), to, &mut light_pos);
        clip_segment(pick(light_dir.z < 0.0, Vec4::new(0.0, 0.0, -1.0, 0.0), Vec4::new(0.0, 0.0, 1.0, (l.z - 1) as f32)), to, &mut light_pos);
        let mut distance = (to - light_pos).length();
        if distance < 0.1 {
            return None;
        }
        distance += distance_adv - glsl_mod(distance, distance_adv);
        light_pos = to - light_dir * distance;
        let mut steps = Vec::new();
        let occlusion = raymarch(probe, distance, distance_adv, light_pos, light_dir, trace.is_some().then_some(&mut steps));
        if let Some(t) = trace.as_deref_mut() {
            t.march_start = Some(light_pos);
            t.march_end = Some(to);
            t.steps = steps;
            t.occlusion = occlusion;
        }
        if occlusion == 0.0 {
            return None;
        }
        attenuation *= occlusion;
    }
    Some((light.color * attenuation * light.energy, light_dir))
}

/// GLSL's `sign`: 0 at 0.
trait Signum0 {
    fn signum0(self) -> f32;
}

impl Signum0 for f32 {
    fn signum0(self) -> f32 {
        if self > 0.0 { 1.0 } else if self < 0.0 { -1.0 } else { 0.0 }
    }
}

/// `compute_area_light`: the area light's LTC integral seen from `pos`,
/// windowed by its range; there is no shadow march.
pub fn area_light(params: &Params, light: &Light, pos: Vec3, normal_in: Vec3) -> Option<Vec3> {
    let epsilon = 1e-7;
    let (w, h) = (light.area_width, light.area_height);
    let dir = light.direction;
    if w.dot(w) < epsilon || h.dot(h) < epsilon {
        return None;
    }
    if dir.dot(pos - light.position) <= 0.0 {
        return None;
    }
    let (a_len, b_len) = (w.length(), h.length());
    let (wn, hn) = (w.normalize(), h.normalize());
    let to_vert = pos - light.position;
    let local = Vec3::new(to_vert.dot(wn), to_vert.dot(hn), to_vert.dot(-dir));
    let closest_local = Vec3::new(local.x.clamp(-a_len / 2.0, a_len / 2.0), local.y.clamp(-b_len / 2.0, b_len / 2.0), 0.0);
    let closest = light.position + closest_local.x * wn + closest_local.y * hn;
    let dist = (closest - pos).length();
    let light_length = (1.0 / params.cell_size).max(dist);
    if light_length >= light.radius {
        return None;
    }
    let attenuation = omni_attenuation(light_length * params.cell_size, 1.0 / (light.radius * params.cell_size), light.attenuation) * light_length * light_length * params.cell_size * params.cell_size;
    if attenuation < 0.01 {
        return None;
    }
    let (hw, hh) = (w / 2.0, h / 2.0);
    let c = params.cell_size;
    let points = [(light.position - hw - hh - pos) * c, (light.position + hw - hh - pos) * c, (light.position + hw + hh - pos) * c, (light.position - hw + hh - pos) * c];
    let normal = if normal_in.dot(normal_in) < 0.04 { -dir } else { normal_in };
    let ltc = ltc_diffuse(normal, points);
    Some(light.color * ltc * attenuation * light.energy)
}

/// `ltc_evaluate_diff` with no projector texture (area_lights_inc.glsl): the
/// integral over the light's quad, clipped to the horizon of `normal`, of the
/// cosine to `normal`, not divided by 2 pi ("no division by (2.0 * M_PI) for
/// GI calculations").
pub fn ltc_diffuse(normal: Vec3, points: [Vec3; 4]) -> f32 {
    let eye_vec = if normal.z.abs() < 0.7 { Vec3::new(0.0, 0.0, -1.0) } else { Vec3::X };
    let z = -(eye_vec - normal * eye_vec.dot(normal)).normalize();
    let x = normal.cross(z);
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

/// `MODE_COMPUTE_LIGHT` for one cell: every light's contribution times
/// albedo (with the cosine to the cell's normal when it has one), plus the
/// cell's emission.
pub fn compute_light(probe: &Probe, params: &Params, lights: &[Light], cell: usize) -> Vec3 {
    let d = probe.data[cell];
    let pos = d.pos().as_vec3() + Vec3::splat(0.5);
    let albedo = d.albedo();
    let emission = d.emission();
    let normal = d.normal();
    let mut accum = Vec3::ZERO;
    for l in lights {
        if l.kind != LIGHT_TYPE_AREA {
            let Some((light, light_dir)) = light_at_pos(probe, params, l, pos, normal, None) else { continue };
            let light = light * albedo.truncate();
            accum += if normal.length() > 0.2 { normal.dot(-light_dir).max(0.0) * light } else { light };
        } else {
            let Some(light) = area_light(params, l, pos, normal) else { continue };
            accum += light * albedo.truncate();
        }
    }
    accum + emission
}

/// The six cones of `MODE_SECOND_BOUNCE`, in the cell's tangent frame (z the normal).
pub const BOUNCE_CONE_DIRS: [Vec3; 6] = [
    Vec3::new(0.0, 0.0, 1.0),
    Vec3::new(0.866025, 0.0, 0.5),
    Vec3::new(0.267617, 0.823639, 0.5),
    Vec3::new(-0.700629, 0.509037, 0.5),
    Vec3::new(-0.700629, -0.509037, 0.5),
    Vec3::new(0.267617, -0.823639, 0.5),
];
pub const BOUNCE_CONE_WEIGHTS: [f32; 6] = [0.25, 0.15, 0.15, 0.15, 0.15, 0.15];
pub const BOUNCE_TAN_HALF_ANGLE: f32 = 0.577;

/// The tangent frame both the bounce and the screen pass build from a normal:
/// `v0 = |n.z| < 0.999 ? +z : +y`, tangent `v0 x n`, bitangent `tangent x n`.
pub fn tangent_frame(n: Vec3) -> [Vec3; 3] {
    let v0 = if n.z.abs() < 0.999 { Vec3::Z } else { Vec3::Y };
    let tangent = v0.cross(n).normalize();
    let bitangent = tangent.cross(n).normalize();
    [tangent, bitangent, n]
}

/// One sample of a cone's march, for a demonstration.
#[derive(Clone, Copy, Debug)]
pub struct ConeStep {
    pub dist: f32,
    pub diameter: f32,
    pub lod: f32,
    pub sample: Vec4,
}

/// The bounce's march of one cone: from 1.5 cells, the diameter `2 tan dist`
/// (at least one cell), the mip `log2(diameter)`, front to back until the
/// alpha reaches 0.95 (no bounds test: it is commented out).
pub fn bounce_cone(tex: &LightTexture, params: &Params, pos: Vec3, direction: Vec3, mut steps: Option<&mut Vec<ConeStep>>) -> Vec4 {
    let mut color = Vec4::ZERO;
    let mut dist = 1.5f32;
    let max_distance = params.limits.as_vec3().length();
    let cell_size = Vec3::ONE / params.limits.as_vec3();
    while dist < max_distance && color.w < 0.95 {
        let diameter = (2.0 * BOUNCE_TAN_HALF_ANGLE * dist).max(1.0);
        let uvw = (pos + dist * direction) * cell_size;
        let half_diameter = diameter * 0.5;
        let lod = diameter.log2();
        let scolor = tex.sample_lod(uvw, lod);
        let a = 1.0 - color.w;
        color += a * scolor;
        if let Some(s) = steps.as_deref_mut() {
            s.push(ConeStep { dist, diameter, lod, sample: scolor });
        }
        dist += half_diameter;
    }
    color
}

/// `MODE_SECOND_BOUNCE` for one cell: its direct light plus, if it has a
/// normal, each cone's light x weight x albedo x dynamic range.
pub fn second_bounce(probe: &Probe, params: &Params, tex: &LightTexture, cell: usize, direct: Vec3) -> Vec3 {
    let d = probe.data[cell];
    let pos = d.pos().as_vec3() + Vec3::splat(0.5);
    let normal = d.normal();
    let albedo = d.albedo();
    let mut accum = direct;
    if normal.length() > 0.2 {
        let [t, b, n] = tangent_frame(normal);
        for (i, dir) in BOUNCE_CONE_DIRS.iter().enumerate() {
            let direction = t * dir.x + b * dir.y + n * dir.z;
            let color = bounce_cone(tex, params, pos, direction, None);
            let color = color * BOUNCE_CONE_WEIGHTS[i] * Vec4::new(albedo.x, albedo.y, albedo.z, 1.0) * params.dynamic_range;
            accum += color.truncate();
        }
    }
    accum
}

/// `MODE_UPDATE_MIPMAPS` for one parent cell: its children's light summed
/// and divided by `mix(8, count, propagation)`.
pub fn mip_cell(probe: &Probe, params: &Params, outputs: &[Vec3], cell: usize) -> (Vec3, u32) {
    let mut accum = Vec3::ZERO;
    let mut count = 0u32;
    for &child in &probe.octree[cell] {
        if child == CHILD_EMPTY {
            continue;
        }
        accum += outputs[child as usize];
        count += 1;
    }
    let divisor = 8.0 + (count as f32 - 8.0) * params.propagation;
    (accum / divisor, count)
}

/// `MODE_WRITE_TEXTURE` for one cell: its light over the dynamic range, and
/// its coverage, into its texel of its level's mip.
pub fn write_cell(probe: &Probe, params: &Params, outputs: &[Vec3], cell: usize, tex: &mut LightTexture) {
    let d = probe.data[cell];
    let mip = probe.mip_count() - 1 - probe.level_of(cell);
    let a = d.albedo().w;
    tex.store(mip, d.pos(), (outputs[cell] / params.dynamic_range).extend(a));
}

/// What `update` leaves: the float buffer after each pass and the texture.
#[derive(Clone)]
pub struct Lit {
    /// `outputs` (the write buffer) after pass 0: direct light at the leaves, mips above.
    pub direct: Vec<Vec3>,
    /// The texture pass 0 wrote (what the bounce cones read).
    pub first_texture: LightTexture,
    /// `outputs` after the last pass.
    pub outputs: Vec<Vec3>,
    pub texture: LightTexture,
    pub passes: usize,
}

/// Runs `f(cell)` over `range` on all cores, in order.
pub fn par_map<T: Send + Sync + Copy + Default>(range: std::ops::Range<usize>, f: impl Fn(usize) -> T + Sync) -> Vec<T> {
    let n = range.len();
    let threads = std::thread::available_parallelism().map_or(4, |t| t.get()).min(32);
    let chunk = n.div_ceil(threads).max(1);
    let mut out = vec![T::default(); n];
    std::thread::scope(|s| {
        for (k, part) in out.chunks_mut(chunk).enumerate() {
            let f = &f;
            let start = range.start + k * chunk;
            s.spawn(move || {
                for (i, o) in part.iter_mut().enumerate() {
                    *o = f(start + i);
                }
            });
        }
    });
    out
}

/// Pass 0's mips and write, or pass 1's, over `outputs` (leaves already set).
fn mips_and_write(probe: &Probe, params: &Params, outputs: &mut [Vec3]) -> LightTexture {
    for mip in 1..probe.mip_count() {
        let level = probe.mip_level(mip);
        let range = probe.level_range(level);
        let values = par_map(range.clone(), |c| mip_cell(probe, params, outputs, c).0);
        outputs[range].copy_from_slice(&values);
    }
    let mut tex = LightTexture::cleared(probe);
    for cell in 0..probe.data.len() {
        write_cell(probe, params, outputs, cell, &mut tex);
    }
    tex
}

/// `VoxelGIInstance::update` with the lights changed: pass 0 (direct light,
/// mips, write), and with two bounces pass 1 (the bounce at the leaves,
/// mips, write).
pub fn update(probe: &Probe, params: &Params, lights: &[Light], two_bounces: bool) -> Lit {
    let lights = &lights[..lights.len().min(MAX_LIGHTS)];
    let mut outputs = vec![Vec3::ZERO; probe.data.len()];
    let leaves = probe.level_range(probe.leaf_level());
    let direct = par_map(leaves.clone(), |c| compute_light(probe, params, lights, c));
    outputs[leaves.clone()].copy_from_slice(&direct);
    let first_texture = mips_and_write(probe, params, &mut outputs);
    let pass0 = outputs.clone();
    if !two_bounces {
        return Lit { direct: pass0, first_texture: first_texture.clone(), outputs, texture: first_texture, passes: 1 };
    }
    let bounced = par_map(leaves.clone(), |c| second_bounce(probe, params, &first_texture, c, pass0[c]));
    outputs[leaves].copy_from_slice(&bounced);
    let texture = mips_and_write(probe, params, &mut outputs);
    Lit { direct: pass0, first_texture, outputs, texture, passes: 2 }
}
