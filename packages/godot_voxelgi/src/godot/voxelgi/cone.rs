//! The read side on the CPU, as gi.glsl runs it per pixel (`process_gi`,
//! `voxel_gi_compute`, `voxel_cone_trace`, `voxel_cone_trace_45_degrees`;
//! servers/rendering/renderer_rd/shaders/environment/gi.glsl), the mirror of
//! the WGSL port in shaders/voxelgi_shading.wgsl: the pixel's position and
//! normal into the box's cell space, the edge fade, four 45-degree cones (Low,
//! the default) or six (High) for the ambient light, one cone along the
//! reflection whose width follows the roughness, each scaled back by the
//! dynamic range, and the weighted average over the instances that cover the
//! pixel.
//!
//! Where the lab differs: Godot moves positions relative to the camera first
//! (`to_camera`); the lab's `xform` takes world positions straight to cells,
//! which gives the same cell position.
//!
//! Ported from Godot Engine (MIT; Copyright (c) 2014-present Godot Engine
//! contributors, Copyright (c) 2007-2014 Juan Linietsky, Ariel Manzur).

use super::light::{tangent_frame, ConeStep};
use super::storage::LightTexture;
use glam::{Vec3, Vec4};

/// `VoxelGIData` as `setup_voxel_gi_instances` fills it.
#[derive(Clone, Copy, Debug)]
pub struct Instance {
    /// world position -> cell: `cell = (p - origin) * scale`.
    pub to_cell_scale: f32,
    pub to_cell_origin: Vec3,
    /// The octree's size, in cells.
    pub bounds: Vec3,
    /// `dynamic_range * energy`.
    pub dynamic_range: f32,
    pub bias: f32,
    pub normal_bias: f32,
    /// `!interior`.
    pub blend_ambient: bool,
    pub exposure_normalization: f32,
}

impl Instance {
    pub fn to_cell(&self, p: Vec3) -> Vec3 {
        (p - self.to_cell_origin) * self.to_cell_scale
    }
    pub fn from_cell(&self, c: Vec3) -> Vec3 {
        c / self.to_cell_scale + self.to_cell_origin
    }
}

/// The Low quality's four cones, 45 degrees from the normal (tangent frame, z the normal).
pub const LOW_CONE_DIRS: [Vec3; 4] = [Vec3::new(0.707107, 0.0, 0.707107), Vec3::new(0.0, 0.707107, 0.707107), Vec3::new(-0.707107, 0.0, 0.707107), Vec3::new(0.0, -0.707107, 0.707107)];
/// High's six: one along the normal, five at 60 degrees (the bounce's own set).
pub const HIGH_CONE_DIRS: [Vec3; 6] = super::light::BOUNCE_CONE_DIRS;
pub const HIGH_CONE_WEIGHTS: [f32; 6] = super::light::BOUNCE_CONE_WEIGHTS;
pub const HIGH_CONE_TAN: f32 = 0.577;
/// gi.glsl's `M_PI`.
pub const M_PI: f32 = 3.141592;

/// `voxel_cone_trace`: from `bias`, the diameter `2 tan dist` (at least a
/// cell), its mip `log2(diameter)`, front to back until the alpha reaches 1,
/// the distance the box's diagonal, or the cone leaves the box.
#[allow(clippy::too_many_arguments)]
pub fn cone_trace(tex: &LightTexture, cell_size: Vec3, pos: Vec3, direction: Vec3, tan_half_angle: f32, max_distance: f32, bias: f32, mut steps: Option<&mut Vec<ConeStep>>) -> Vec4 {
    let mut dist = bias;
    let mut color = Vec4::ZERO;
    while dist < max_distance && color.w < 1.0 {
        let diameter = (2.0 * tan_half_angle * dist).max(1.0);
        let uvw = (pos + dist * direction) * cell_size;
        let half_diameter = diameter * 0.5;
        if ((uvw - Vec3::splat(0.5)).abs() - (Vec3::splat(0.5) + half_diameter * cell_size)).max_element() > 0.0 {
            break;
        }
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

/// `voxel_cone_trace_45_degrees`: the radius is the distance (at least half a
/// cell), so each step doubles the distance and climbs one mip.
pub fn cone_trace_45(tex: &LightTexture, cell_size: Vec3, pos: Vec3, direction: Vec3, max_distance: f32, bias: f32, mut steps: Option<&mut Vec<ConeStep>>) -> Vec4 {
    let mut dist = bias;
    let mut color = Vec4::ZERO;
    let mut radius = dist.max(0.5);
    let mut lod_level = (radius * 2.0).log2();
    while dist < max_distance && color.w < 1.0 {
        let uvw = (pos + dist * direction) * cell_size;
        if ((uvw - Vec3::splat(0.5)).abs() - (Vec3::splat(0.5) + radius * cell_size)).max_element() > 0.0 {
            break;
        }
        let mut scolor = tex.sample_lod(uvw, lod_level);
        let sample = scolor;
        let lod = lod_level;
        lod_level += 1.0;
        let a = 1.0 - color.w;
        scolor *= a;
        color += scolor;
        if let Some(s) = steps.as_deref_mut() {
            s.push(ConeStep { dist, diameter: radius * 2.0, lod, sample });
        }
        dist += radius;
        radius = dist.max(0.5);
    }
    color
}

/// Each cone `voxel_gi_compute` traced, for a demonstration.
#[derive(Clone, Debug, Default)]
pub struct Cones {
    /// The pixel's position in cells (after the normal bias), and its edge fade.
    pub position: Vec3,
    pub blend: f32,
    pub inside: bool,
    /// The diffuse cones: direction (cells), weight, steps, result.
    pub diffuse: Vec<(Vec3, f32, Vec<ConeStep>, Vec4)>,
    /// The specular cone: direction, its tan, steps, result.
    pub specular: Option<(Vec3, f32, Vec<ConeStep>, Vec4)>,
    pub light: Vec4,
    pub reflection: Vec4,
}

/// `voxel_gi_compute` for one instance: adds its diffuse and specular light,
/// weighted by the edge fade, and the fade itself.
#[allow(clippy::too_many_arguments)]
pub fn voxel_gi_compute(inst: &Instance, tex: &LightTexture, position_world: Vec3, normal_world: Vec3, ref_world: Vec3, roughness: f32, high_quality: bool, out_spec: &mut Vec4, out_diff: &mut Vec4, out_blend: &mut f32, mut trace: Option<&mut Cones>) {
    let mut position = inst.to_cell(position_world);
    // mat3(xform) is a uniform scale here, so normalize() takes it out.
    let ref_vec = ref_world.normalize();
    let normal = normal_world.normalize();
    position += normal * inst.normal_bias;
    let inside = !(position.cmplt(Vec3::ZERO).any() || position.cmpgt(inst.bounds).any());
    if let Some(t) = trace.as_deref_mut() {
        t.position = position;
        t.inside = inside;
    }
    if !inside {
        return;
    }
    let [tangent, bitangent, n] = tangent_frame(normal);
    let blendv = (position / inst.bounds * 2.0 - Vec3::ONE).abs();
    let blend = (1.0 - blendv.max_element()).clamp(0.0, 1.0);
    let max_distance = inst.bounds.length();
    let cell_size = Vec3::ONE / inst.bounds;
    let mut light = Vec4::ZERO;
    let frame = |d: Vec3| (tangent * d.x + bitangent * d.y + n * d.z).normalize();
    if high_quality {
        for (i, d) in HIGH_CONE_DIRS.iter().enumerate() {
            let dir = frame(*d);
            let mut steps = Vec::new();
            let c = cone_trace(tex, cell_size, position, dir, HIGH_CONE_TAN, max_distance, inst.bias, trace.is_some().then_some(&mut steps));
            light += HIGH_CONE_WEIGHTS[i] * c;
            if let Some(t) = trace.as_deref_mut() {
                t.diffuse.push((dir, HIGH_CONE_WEIGHTS[i], steps, c));
            }
        }
    } else {
        for d in LOW_CONE_DIRS {
            let dir = frame(d);
            let mut steps = Vec::new();
            let c = cone_trace_45(tex, cell_size, position, dir, max_distance, inst.bias, trace.is_some().then_some(&mut steps));
            light += 0.25 * c;
            if let Some(t) = trace.as_deref_mut() {
                t.diffuse.push((dir, 0.25, steps, c));
            }
        }
    }
    let scale = inst.dynamic_range * inst.exposure_normalization;
    light = (light.truncate() * scale).extend(if inst.blend_ambient { light.w } else { 1.0 });
    *out_diff += light * blend;
    let tan = (roughness * 0.5 * M_PI * 0.99).tan();
    let mut steps = Vec::new();
    let irr = cone_trace(tex, cell_size, position, ref_vec, tan, max_distance, inst.bias, trace.is_some().then_some(&mut steps));
    let irr = (irr.truncate() * scale).extend(if inst.blend_ambient { irr.w } else { 1.0 });
    *out_spec += irr * blend;
    *out_blend += blend;
    if let Some(t) = trace {
        t.blend = blend;
        t.specular = Some((ref_vec, tan, steps, irr));
        t.light = light;
        t.reflection = irr;
    }
}

/// The ambient and reflection `process_gi` stores for a pixel covered by
/// one VoxelGI (roughness squared first, the sums divided by the blend).
pub fn process(inst: &Instance, tex: &LightTexture, position: Vec3, normal: Vec3, reflection: Vec3, roughness: f32, high_quality: bool, trace: Option<&mut Cones>) -> (Vec4, Vec4) {
    let roughness = roughness * roughness;
    let mut amb = Vec4::ZERO;
    let mut spec = Vec4::ZERO;
    let mut blend = 0.0;
    voxel_gi_compute(inst, tex, position, normal, reflection, roughness, high_quality, &mut spec, &mut amb, &mut blend, trace);
    if blend > 0.0 {
        amb /= blend;
        spec /= blend;
    }
    (amb, spec)
}
