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

use super::light::{ConeStep, tangent_frame};
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

#[allow(clippy::wrong_self_convention)] // maps between a frame and the world, named as the engine names them
impl Instance {
    pub fn to_cell(&self, p: Vec3) -> Vec3 {
        (p - self.to_cell_origin) * self.to_cell_scale
    }
    pub fn from_cell(&self, c: Vec3) -> Vec3 {
        c / self.to_cell_scale + self.to_cell_origin
    }
}

/// The Low quality's four cones, 45 degrees from the normal (tangent frame, z the normal).
#[allow(clippy::approx_constant)] // the engine's literals
pub const LOW_CONE_DIRS: [Vec3; 4] = [
    Vec3::new(0.707107, 0.0, 0.707107),
    Vec3::new(0.0, 0.707107, 0.707107),
    Vec3::new(-0.707107, 0.0, 0.707107),
    Vec3::new(0.0, -0.707107, 0.707107),
];
/// High's six: one along the normal, five at 60 degrees (the bounce's own set).
pub const HIGH_CONE_DIRS: [Vec3; 6] = super::light::BOUNCE_CONE_DIRS;
pub const HIGH_CONE_WEIGHTS: [f32; 6] = super::light::BOUNCE_CONE_WEIGHTS;
pub const HIGH_CONE_TAN: f32 = 0.577;
/// gi.glsl's `M_PI`.
#[allow(clippy::approx_constant)] // the engine's literal
pub const M_PI: f32 = 3.141592;

/// A cone the cone traces follow (`voxel_cone_trace`'s and
/// `voxel_cone_trace_45_degrees`'s `pos`, `direction`, `max_distance`, and
/// `p_bias`): from `pos` along `direction` (cells), from `bias` out to
/// `max_distance`.
#[derive(Clone, Copy, Debug)]
pub struct Cone {
    pub pos: Vec3,
    pub direction: Vec3,
    pub max_distance: f32,
    pub bias: f32,
}

/// `voxel_cone_trace`: from `bias`, the diameter `2 tan dist` (at least a
/// cell), its mip `log2(diameter)`, front to back until the alpha reaches 1,
/// the distance the box's diagonal, or the cone leaves the box.
pub fn cone_trace(
    tex: &LightTexture,
    cell_size: Vec3,
    cone: Cone,
    tan_half_angle: f32,
    mut steps: Option<&mut Vec<ConeStep>>,
) -> Vec4 {
    let Cone {
        pos,
        direction,
        max_distance,
        bias,
    } = cone;
    let mut dist = bias;
    let mut color = Vec4::ZERO;
    while dist < max_distance && color.w < 1.0 {
        let diameter = (2.0 * tan_half_angle * dist).max(1.0);
        let uvw = (pos + dist * direction) * cell_size;
        let half_diameter = diameter * 0.5;
        if ((uvw - Vec3::splat(0.5)).abs() - (Vec3::splat(0.5) + half_diameter * cell_size))
            .max_element()
            > 0.0
        {
            break;
        }
        let lod = diameter.log2();
        let scolor = tex.sample_lod(uvw, lod);
        let a = 1.0 - color.w;
        color += a * scolor;
        if let Some(s) = steps.as_deref_mut() {
            s.push(ConeStep {
                dist,
                diameter,
                lod,
                sample: scolor,
            });
        }
        dist += half_diameter;
    }
    color
}

/// `voxel_cone_trace_45_degrees`: the radius is the distance (at least half a
/// cell), so each step doubles the distance and climbs one mip.
pub fn cone_trace_45(
    tex: &LightTexture,
    cell_size: Vec3,
    cone: Cone,
    mut steps: Option<&mut Vec<ConeStep>>,
) -> Vec4 {
    let Cone {
        pos,
        direction,
        max_distance,
        bias,
    } = cone;
    let mut dist = bias;
    let mut color = Vec4::ZERO;
    let mut radius = dist.max(0.5);
    let mut lod_level = (radius * 2.0).log2();
    while dist < max_distance && color.w < 1.0 {
        let uvw = (pos + dist * direction) * cell_size;
        if ((uvw - Vec3::splat(0.5)).abs() - (Vec3::splat(0.5) + radius * cell_size)).max_element()
            > 0.0
        {
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
            s.push(ConeStep {
                dist,
                diameter: radius * 2.0,
                lod,
                sample,
            });
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

/// A pixel's surface in the world, as `voxel_gi_compute` takes it: its
/// position, normal, reflection vector, and roughness.
#[derive(Clone, Copy, Debug)]
pub struct Sample {
    pub position: Vec3,
    pub normal: Vec3,
    pub reflection: Vec3,
    pub roughness: f32,
}

/// The sums `voxel_gi_compute` adds to (its `out_spec`, `out_diff`, and
/// `out_blend`).
#[derive(Clone, Copy, Debug, Default)]
pub struct GiSums {
    pub spec: Vec4,
    pub diff: Vec4,
    pub blend: f32,
}

/// `voxel_gi_compute` for one instance: adds its diffuse and specular light,
/// weighted by the edge fade, and the fade itself, to `sums`.
pub fn voxel_gi_compute(
    inst: &Instance,
    tex: &LightTexture,
    sample: &Sample,
    high_quality: bool,
    sums: &mut GiSums,
    mut trace: Option<&mut Cones>,
) {
    let Sample {
        position: position_world,
        normal: normal_world,
        reflection: ref_world,
        roughness,
    } = *sample;
    let GiSums {
        spec: out_spec,
        diff: out_diff,
        blend: out_blend,
    } = sums;
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
    let mut light = diffuse_cones(
        (inst, tex),
        (position, cell_size, max_distance),
        [tangent, bitangent, n],
        high_quality,
        trace.as_deref_mut(),
    );
    let scale = inst.dynamic_range * inst.exposure_normalization;
    light = (light.truncate() * scale).extend(if inst.blend_ambient {
        light.w
    } else {
        1.0
    });
    *out_diff += light * blend;
    let tan = (roughness * 0.5 * M_PI * 0.99).tan();
    let mut steps = Vec::new();
    let irr = cone_trace(
        tex,
        cell_size,
        Cone {
            pos: position,
            direction: ref_vec,
            max_distance,
            bias: inst.bias,
        },
        tan,
        trace.is_some().then_some(&mut steps),
    );
    let irr = (irr.truncate() * scale).extend(if inst.blend_ambient {
        irr.w
    } else {
        1.0
    });
    *out_spec += irr * blend;
    *out_blend += blend;
    if let Some(t) = trace {
        t.blend = blend;
        t.specular = Some((ref_vec, tan, steps, irr));
        t.light = light;
        t.reflection = irr;
    }
}

/// The diffuse cones of `voxel_gi_compute` from `position` (in cells, of size `cell_size` in uvw, as far as
/// `max_distance`), in the frame `[tangent, bitangent, n]`: the six weighted cones at high quality, four of 45
/// degrees at low; their weighted light, each cone recorded in `trace`.
fn diffuse_cones(
    (inst, tex): (&Instance, &LightTexture),
    (position, cell_size, max_distance): (Vec3, Vec3, f32),
    [tangent, bitangent, n]: [Vec3; 3],
    high_quality: bool,
    mut trace: Option<&mut Cones>,
) -> Vec4 {
    let mut light = Vec4::ZERO;
    let frame = |d: Vec3| (tangent * d.x + bitangent * d.y + n * d.z).normalize();
    if high_quality {
        for (i, d) in HIGH_CONE_DIRS.iter().enumerate() {
            let dir = frame(*d);
            let mut steps = Vec::new();
            let c = cone_trace(
                tex,
                cell_size,
                Cone {
                    pos: position,
                    direction: dir,
                    max_distance,
                    bias: inst.bias,
                },
                HIGH_CONE_TAN,
                trace.is_some().then_some(&mut steps),
            );
            light += HIGH_CONE_WEIGHTS[i] * c;
            if let Some(t) = trace.as_deref_mut() {
                t.diffuse.push((dir, HIGH_CONE_WEIGHTS[i], steps, c));
            }
        }
    } else {
        for d in LOW_CONE_DIRS {
            let dir = frame(d);
            let mut steps = Vec::new();
            let c = cone_trace_45(
                tex,
                cell_size,
                Cone {
                    pos: position,
                    direction: dir,
                    max_distance,
                    bias: inst.bias,
                },
                trace.is_some().then_some(&mut steps),
            );
            light += 0.25 * c;
            if let Some(t) = trace.as_deref_mut() {
                t.diffuse.push((dir, 0.25, steps, c));
            }
        }
    }
    light
}

/// The ambient and reflection `process_gi` stores for a pixel covered by
/// one VoxelGI (roughness squared first, the sums divided by the blend).
pub fn process(
    inst: &Instance,
    tex: &LightTexture,
    sample: &Sample,
    high_quality: bool,
    trace: Option<&mut Cones>,
) -> (Vec4, Vec4) {
    let sample = Sample {
        roughness: sample.roughness * sample.roughness,
        ..*sample
    };
    let mut sums = GiSums::default();
    voxel_gi_compute(inst, tex, &sample, high_quality, &mut sums, trace);
    let GiSums {
        mut spec,
        diff: mut amb,
        blend,
    } = sums;
    if blend > 0.0 {
        amb /= blend;
        spec /= blend;
    }
    (amb, spec)
}
