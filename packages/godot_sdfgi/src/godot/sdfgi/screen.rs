//! The screen pass on the CPU: gi.glsl's `sdfgi_process` and
//! `sdfvoxel_gi_process`, as shaders/sdfgi_shading.wgsl ports them, reading
//! the probe maps of a `World` (sim.rs). The selftest compares the two, and
//! the demonstrations read the weights and cascades it computes.
//!
//! Ported from Godot Engine (MIT; Copyright (c) 2014-present Godot Engine
//! contributors, Copyright (c) 2007-2014 Juan Linietsky, Ariel Manzur).

use super::light::octahedron_encode;
use super::model::PROBE_AXIS_COUNT;
use super::probes::{ProbeMaps, OCT_SIZE};
use glam::{IVec3, Vec3, Vec4};

/// What the screen pass reads per cascade (SDFGIData::ProbeCascadeData).
#[derive(Clone, Copy, Debug)]
pub struct ProbeCascade {
    /// The cascade's corner relative to SDFGI's camera, squashed space.
    pub position: Vec3,
    pub to_probe: f32,
}

pub struct Inputs<'a> {
    pub cascades: Vec<ProbeCascade>,
    pub maps: Vec<&'a ProbeMaps>,
    pub y_mult: f32,
    pub normal_bias: f32,
    pub energy: f32,
}

/// One of the eight probes a lookup blends, for showing it.
#[derive(Clone, Copy, Debug)]
pub struct ProbeWeight {
    pub probe: IVec3,
    pub weight: f32,
}

pub struct VoxelGi {
    pub diffuse: Vec3,
    pub specular: Vec3,
    pub probes: Vec<ProbeWeight>,
    /// The point in probe units after the normal bias.
    pub biased: Vec3,
}

/// `sdfvoxel_gi_process`.
pub fn voxel_gi(inputs: &Inputs, cascade: usize, cascade_pos: Vec3, cam_normal: Vec3, specular_normal: Vec3, roughness: f32) -> VoxelGi {
    let cascade_pos = cascade_pos + cam_normal * (inputs.normal_bias / 128.0 * 16.0);
    let base = cascade_pos.floor().as_ivec3();
    let diffuse_oct = octahedron_encode(cam_normal) * OCT_SIZE as f32;
    let specular_oct = octahedron_encode(specular_normal) * OCT_SIZE as f32;
    let maps = inputs.maps[cascade];
    let mut diffuse_accum = Vec4::ZERO;
    let mut specular_accum = Vec3::ZERO;
    let mut probes = Vec::with_capacity(8);
    for j in 0..8 {
        let offset = IVec3::new(j & 1, (j >> 1) & 1, (j >> 2) & 1);
        let probe = base + offset;
        let probe_to_pos = cascade_pos - probe.as_vec3();
        let probe_dir = (-probe_to_pos).normalize();
        let trilinear = Vec3::ONE - probe_to_pos.abs();
        let weight = trilinear.x * trilinear.y * trilinear.z * cam_normal.dot(probe_dir).max(0.005);
        let tex = glam::Vec2::new(((probe.x + probe.z * PROBE_AXIS_COUNT) * (OCT_SIZE + 2) + 1) as f32, (probe.y * (OCT_SIZE + 2) + 1) as f32);
        let d = tex + diffuse_oct;
        let diffuse = ProbeMaps::bilinear(&maps.irradiance, d.x, d.y);
        diffuse_accum += (diffuse * weight).extend(weight);
        let s = tex + specular_oct;
        let mut specular = Vec3::ZERO;
        if roughness < 0.99 {
            specular = ProbeMaps::bilinear(&maps.radiance, s.x, s.y);
        }
        if roughness > 0.2 {
            specular = specular.lerp(ProbeMaps::bilinear(&maps.irradiance, s.x, s.y), (roughness - 0.2) * 1.25);
        }
        specular_accum += specular * weight;
        probes.push(ProbeWeight { probe, weight });
    }
    let mut out = VoxelGi { diffuse: diffuse_accum.truncate(), specular: specular_accum, probes, biased: cascade_pos };
    if diffuse_accum.w > 0.0 {
        out.diffuse /= diffuse_accum.w;
        out.specular /= diffuse_accum.w;
    }
    out
}

pub struct Result {
    pub ambient: Vec4,
    pub reflection: Vec4,
    /// The cascade it read, and the blend toward the next (0 to 1).
    pub cascade: Option<usize>,
    pub blend: f32,
}

/// `sdfgi_process`, without the specular march (as the WGSL port).
pub fn process(inputs: &Inputs, vertex: Vec3, normal: Vec3, reflection: Vec3, roughness: f32) -> Result {
    let y = inputs.y_mult;
    let vertex = Vec3::new(vertex.x, vertex.y * y, vertex.z);
    let cam_normal = Vec3::new(normal.x, normal.y * y, normal.z).normalize();
    let reflection = Vec3::new(reflection.x, reflection.y * y, reflection.z).normalize();
    let mut out = Result { ambient: Vec4::ZERO, reflection: Vec4::ZERO, cascade: None, blend: 0.0 };
    let found = inputs.cascades.iter().enumerate().find_map(|(i, c)| {
        let p = (vertex - c.position) * c.to_probe;
        (p.cmpge(Vec3::ZERO).all() && p.cmplt(Vec3::splat(16.0)).all()).then_some((i, p))
    });
    let Some((cascade, cascade_pos)) = found else { return out };
    out.ambient.w = 1.0;
    out.reflection.w = 1.0;
    let mut gi = voxel_gi(inputs, cascade, cascade_pos, cam_normal, reflection, roughness);
    let blend_from = (PROBE_AXIS_COUNT - 1) as f32 / 2.0 - 2.5;
    let blend_to = blend_from + 2.0;
    let inner = vertex * inputs.cascades[cascade].to_probe;
    let len = inner.abs().max_element();
    let blend = if len >= blend_from { smoothstep(blend_from, blend_to, len) } else { 0.0 };
    if blend > 0.0 {
        if cascade == inputs.cascades.len() - 1 {
            out.ambient.w = 1.0 - blend;
            out.reflection.w = 1.0 - blend;
        } else {
            let c = inputs.cascades[cascade + 1];
            let gi2 = voxel_gi(inputs, cascade + 1, (vertex - c.position) * c.to_probe, cam_normal, reflection, roughness);
            gi.diffuse = gi.diffuse.lerp(gi2.diffuse, blend);
            gi.specular = gi.specular.lerp(gi2.specular, blend);
        }
    }
    out.ambient = (gi.diffuse * inputs.energy).extend(out.ambient.w);
    out.reflection = (gi.specular * inputs.energy).extend(out.reflection.w);
    out.cascade = Some(cascade);
    out.blend = blend;
    out
}

pub fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
