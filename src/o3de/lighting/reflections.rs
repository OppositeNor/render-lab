//! Reflection probes (Reflections.pass and ReflectionProbeFeatureProcessor):
//! the two probes the lab places in the room, their cubemaps baked on the CPU,
//! and the CPU mirror of the shader's reflectionSample, which runs the probe
//! stencil, blend weight, global, outer, inner, and composite steps for one
//! sample in O3DE's order.

use super::brdf::{multi_scatter_compensation, saturate, BrdfMap, Shading};
use super::ibl::{Cubemap, SPECULAR_MIPS, SPECULAR_SIZE};
use crate::lab::layout::ObjectData;
use crate::lab::shot::ShotState;
use glam::{Mat3, Quat, Vec2, Vec3};
use std::sync::OnceLock;

/// RenderCommon.h StencilRefs, and the reflection passes' 0x3F read mask.
pub const STENCIL_USE_IBL_SPECULAR_PASS: u32 = 0x3;
pub const STENCIL_USE_DIFFUSE_GI_PASS: u32 = 0x80;
pub const STENCIL_REFLECTION_MASK: u32 = 0x3F;
/// The reflection target's clear value (ReflectionGlobalFullscreen.pass).
pub const REFLECTION_CLEAR: Vec3 = Vec3::splat(0.4);

/// A probe as its ObjectSrg holds it.
#[derive(Clone, Copy, Debug)]
pub struct Probe {
    pub position: Vec3,
    pub rotation: Quat,
    pub outer_half: Vec3,
    pub inner_half: Vec3,
    pub parallax: bool,
    /// m_renderExposure, in stops.
    pub exposure: f32,
    /// Which baked cubemap (the probe's index before sorting).
    pub cubemap: usize,
}

/// The probes' fixed placements, which their cubemaps are baked from:
/// (position, outer extents, inner extents), full sizes as the component sets them.
const PLACEMENTS: [(Vec3, Vec3, Vec3); 2] = [
    (Vec3::new(-1.3, 1.2, -1.3), Vec3::new(3.4, 2.8, 3.4), Vec3::new(2.0, 2.6, 2.0)),
    (Vec3::new(1.4, 1.2, 0.3), Vec3::new(3.0, 2.8, 3.6), Vec3::new(1.6, 2.6, 2.2)),
];

/// The scene's probes when `probes` is on, sorted as the feature processor
/// sorts them: by descending inner volume, so smaller ones render last.
pub fn scene_probes(state: &ShotState) -> Vec<Probe> {
    if state.p("probes") < 0.5 {
        return Vec::new();
    }
    let inner_scale = state.p("probeInnerScale").clamp(0.0, 1.0);
    let mut probes: Vec<Probe> = PLACEMENTS
        .iter()
        .enumerate()
        .map(|(i, &(position, outer, inner))| Probe {
            position,
            rotation: Quat::IDENTITY,
            outer_half: outer / 2.0,
            // The component clamps the inner extents to the outer ones.
            inner_half: (inner * inner_scale).min(outer) / 2.0,
            parallax: state.p("probeParallax") > 0.5,
            exposure: state.p("probeExposure"),
            cubemap: i,
        })
        .collect();
    probes.sort_by(|a, b| {
        let volume = |p: &Probe| p.inner_half.x * p.inner_half.z * p.inner_half.y;
        volume(b).total_cmp(&volume(a))
    });
    probes
}

pub const MAX_PROBES: usize = 4;

/// gShadingFrame from FRAME_PROBES: the count, then four vectors per probe.
pub fn frame_records(probes: &[Probe]) -> Vec<[f32; 4]> {
    assert!(probes.len() <= MAX_PROBES);
    let mut out = vec![[probes.len() as f32, 0.0, 0.0, 0.0]];
    for p in probes {
        out.push(p.position.extend(p.exposure).to_array());
        out.push(p.rotation.to_array());
        out.push(p.outer_half.extend(if p.parallax { 1.0 } else { 0.0 }).to_array());
        out.push(p.inner_half.extend(p.cubemap as f32).to_array());
    }
    out
}

impl Probe {
    fn local_point(&self, p: Vec3) -> Vec3 {
        Mat3::from_quat(self.rotation).transpose() * (p - self.position)
    }

    /// ObbContainsPoint: |p| < halfExtents in the probe's frame.
    pub fn contains(&self, half: Vec3, p: Vec3) -> bool {
        let q = self.local_point(p).abs();
        q.x < half.x && q.y < half.y && q.z < half.z
    }

    /// ComputeLerpBetweenInnerOuterOBBs: 1 on the inner box, 0 on the outer one.
    pub fn lerp_inner_outer(&self, position: Vec3) -> f32 {
        let p = self.local_point(position).abs();
        let closest = p.clamp(-self.inner_half, self.inner_half);
        let dir = (p - closest).normalize();
        let distance = (self.outer_half - p).abs() / dir;
        let shortest = distance.min_element();
        let total = (p + dir * shortest - closest).length();
        if total > 0.0 { saturate(shortest / total) } else { 1.0 }
    }

    /// The blend weight inside the outer box (ReflectionProbeBlendWeight).
    pub fn blend_weight(&self, p: Vec3) -> f32 {
        if self.contains(self.inner_half, p) { 1.0 } else { self.lerp_inner_outer(p) }
    }

    /// ApplyParallaxCorrectionOBB.
    pub fn parallax_correction(&self, position: Vec3, reflect_dir: Vec3) -> Vec3 {
        let rotation = Mat3::from_quat(self.rotation);
        let p = rotation.transpose() * (position - self.position);
        let r = rotation.transpose() * reflect_dir;
        let a = (self.outer_half - p) / r;
        let b = (-self.outer_half - p) / r;
        let distance = a.max(b).min_element();
        (rotation * (r * distance + p)).normalize()
    }
}

/// What the reflection passes read at one sample.
#[derive(Clone, Copy, Debug)]
pub struct ReflectionInputs {
    pub position: Vec3,
    pub normal: Vec3,
    pub enable_ibl: bool,
    pub multiscatter: bool,
    pub specular_f0: Vec3,
    pub roughness: f32,
    pub specular_occlusion: f32,
    pub stencil: u32,
}

/// One sample after the reflection passes.
#[derive(Clone, Copy, Debug)]
pub struct ReflectionSample {
    pub stencil: u32,
    pub blend_weight: f32,
    /// The global pass's weight, 1 - min(blend weight, 1), where it ran.
    pub global_weight: f32,
    pub reflection: Vec3,
    pub composited: Vec3,
}

/// ComputeProbeSpecular; none where it discards.
fn probe_specular(probe: &Probe, half: Vec3, input: &ReflectionInputs, eye: Vec3, map: &BrdfMap) -> Option<Vec3> {
    if !probe.contains(half, input.position) || !input.enable_ibl {
        return None;
    }
    let dir_to_camera = (eye - input.position).normalize();
    let n_dot_v = input.normal.dot(dir_to_camera).max(0.01);
    let brdf = map.sample(Vec2::new(saturate(n_dot_v), 1.0 - input.roughness));
    let reflect_dir = crate::lab::scene::reflect(-dir_to_camera, input.normal);
    let dir = if probe.parallax { probe.parallax_correction(input.position, reflect_dir) } else { reflect_dir };
    let probe_specular = sample_specular(&cubemaps()[probe.cubemap], dir, input.roughness * 5.0);
    let compensation = multi_scatter_compensation(input.specular_f0, brdf, input.multiscatter);
    Some(probe_specular * compensation * (input.specular_f0 * brdf.x + Vec3::splat(brdf.y)))
}

/// reflectionSample: the probe stencil, the blend weight, the global pass, the
/// outer and inner probe passes, and the composite's test, for one sample.
pub fn reflection_sample(probes: &[Probe], input: &ReflectionInputs, eye: Vec3, ibl_exposure: f32, map: &BrdfMap) -> ReflectionSample {
    let mut stencil = input.stencil;
    if stencil & STENCIL_REFLECTION_MASK > 0 {
        for probe in probes {
            if probe.contains(probe.inner_half - Vec3::splat(0.05), input.position) {
                stencil = (stencil & !STENCIL_REFLECTION_MASK) | ((stencil + 1).min(255) & STENCIL_REFLECTION_MASK);
            }
        }
    }
    let masked = stencil & STENCIL_REFLECTION_MASK;
    let outside_inner = masked == STENCIL_USE_IBL_SPECULAR_PASS;
    let total: f32 = if outside_inner { probes.iter().filter(|p| p.contains(p.outer_half, input.position)).map(|p| p.blend_weight(input.position)).sum() } else { 0.0 };
    let mut reflection = REFLECTION_CLEAR;
    let mut global_weight = 0.0;
    if outside_inner {
        reflection = Vec3::ZERO;
        if input.enable_ibl {
            let dir_to_camera = (eye - input.position).normalize();
            let reflect_dir = crate::lab::scene::reflect(-dir_to_camera, input.normal);
            let global = super::ibl::Ibl::get().sample_specular(reflect_dir, input.roughness * 5.0);
            let n_dot_v = saturate(input.normal.dot(dir_to_camera)).max(0.01);
            let brdf = map.sample(Vec2::new(n_dot_v, 1.0 - input.roughness));
            let compensation = multi_scatter_compensation(input.specular_f0, brdf, input.multiscatter);
            global_weight = 1.0 - total.min(1.0);
            reflection = global_weight * global * compensation * (input.specular_f0 * brdf.x + Vec3::splat(brdf.y)) * 2.0f32.powf(ibl_exposure) * input.specular_occlusion;
        }
        for probe in probes {
            if let Some(specular) = probe_specular(probe, probe.outer_half, input, eye, map) {
                reflection += specular * 2.0f32.powf(probe.exposure) * probe.blend_weight(input.position) / total.max(1.0);
            }
        }
    }
    if masked >= STENCIL_USE_IBL_SPECULAR_PASS {
        for probe in probes {
            if let Some(specular) = probe_specular(probe, probe.inner_half, input, eye, map) {
                reflection = specular * 2.0f32.powf(probe.exposure);
            }
        }
    }
    ReflectionSample { stencil, blend_weight: total, global_weight, reflection, composited: if masked >= 1 { reflection } else { Vec3::ZERO } }
}

/// The probe stencil view's colours (stencilColor in the shader).
pub fn stencil_color(masked: u32) -> Vec3 {
    if masked < STENCIL_USE_IBL_SPECULAR_PASS {
        return Vec3::splat(0.02);
    }
    super::brdf::heat((masked - STENCIL_USE_IBL_SPECULAR_PASS) as f32 / 3.0)
}

/// A probe's cubemap along `dir` at `mip`.
pub fn sample_probe(probe: &Probe, dir: Vec3, mip: f32) -> Vec3 {
    sample_specular(&cubemaps()[probe.cubemap], dir, mip)
}

/// MeshFeatureProcessor's forward-pass probe for a mesh: of the probes whose
/// outer box (as an AABB) overlaps the mesh's world bounds, the last in the
/// feature processor's order, which is the smallest.
pub fn forward_probe(probes: &[Probe], center: Vec3, extent: Vec3) -> Option<usize> {
    probes.iter().rposition(|p| {
        let rotation = Mat3::from_quat(p.rotation);
        let half = rotation.x_axis.abs() * p.outer_half.x + rotation.y_axis.abs() * p.outer_half.y + rotation.z_axis.abs() * p.outer_half.z;
        ((p.position - center).abs() - (half + extent)).max_element() <= 0.0
    })
}

/// SampleLevel on a probe's cubemap: trilinear between mips.
fn sample_specular(mips: &[Cubemap], dir: Vec3, mip: f32) -> Vec3 {
    let m = mip.clamp(0.0, (SPECULAR_MIPS - 1) as f32);
    let m0 = m.floor() as usize;
    let m1 = (m0 + 1).min(SPECULAR_MIPS - 1);
    mips[m0].sample(dir).lerp(mips[m1].sample(dir), m - m0 as f32)
}

/// The room's objects and shading as the probes see them when baked: the
/// study room at the room demonstration's start.
fn bake_scene() -> (Vec<ObjectData>, Shading) {
    let state = super::shot::for_demo(super::shot::Demo::Room);
    let objects = super::demos::pose_objects(crate::lab::scene::Scene::study_room().records(), &state);
    (objects, super::demos::shading(&state))
}

/// Each probe's specular cubemap: the room shaded by the CPU mirror (the
/// point light and the global skylight) from the probe's centre, 256 texels
/// a face as the Medium bake quality's preset makes them, then prefiltered
/// with the skylight's GGX filter into six mips. Built once per run.
pub fn cubemaps() -> &'static Vec<Vec<Cubemap>> {
    static CUBES: OnceLock<Vec<Vec<Cubemap>>> = OnceLock::new();
    CUBES.get_or_init(|| {
        let (objects, shading) = bake_scene();
        let map = BrdfMap::get();
        PLACEMENTS
            .iter()
            .map(|&(center, _, _)| {
                super::ibl::Ibl::build_specular(
                    |dir| {
                        let hit = crate::lab::scene::trace_scene(&objects, center, dir, 100.0, crate::lab::scene::ids::PANEL as u32);
                        if !hit.is_hit() {
                            return super::ibl::environment(dir);
                        }
                        let p = center + dir * hit.t;
                        let n = if hit.normal.dot(dir) > 0.0 { -hit.normal } else { hit.normal };
                        super::brdf::pixel(&shading, &objects, p, n, center, hit.object, 0.0, map, [Vec3::ZERO; 2]).color
                    },
                    SPECULAR_SIZE,
                    super::ibl::GGX_SAMPLES,
                )
            })
            .collect()
    })
}

/// The probes' cubemaps as gShadingData appends them.
pub fn shading_data() -> Vec<[f32; 4]> {
    cubemaps().iter().flatten().flat_map(|c| c.texels.iter().map(|t| [t.x, t.y, t.z, 0.0])).collect()
}
