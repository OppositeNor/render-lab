// Godot's SDFGI on the GPU (src/godot/sdfgi/gpu.rs, a lab program): what
// its passes share. Each pass binds its resources at group 3 (gpu.rs lists
// them, binding by binding); group 2 binding 0 is the command's 16 words of
// parameters, which stand in for the engine's push constants. Composed after
// sdfgi_shading.wgsl and sdfgi_passes.wgsl, whose pure helpers (attenuation,
// the LTC integral, the Vogel spiral, the SH basis, the RGBE encodings) the
// passes reuse.
//
// Ported from Godot Engine (MIT; Copyright (c) 2014-present Godot Engine
// contributors, Copyright (c) 2007-2014 Juan Linietsky, Ariel Manzur;
// THIRD_PARTY.md).

enable wgpu_binding_array;

@group(2) @binding(0) var<uniform> gSdfgiParams: array<vec4<u32>, 4>;

// Word k of the command's parameters.
fn sdfgiParam(k: u32) -> u32 {
    return gSdfgiParams[k / 4u][k % 4u];
}

fn sdfgiParamI(k: u32) -> i32 {
    return bitcast<i32>(sdfgiParam(k));
}

fn sdfgiParamF(k: u32) -> f32 {
    return bitcast<f32>(sdfgiParam(k));
}

fn sdfgiParamI3(k: u32) -> vec3<i32> {
    return vec3<i32>(sdfgiParamI(k), sdfgiParamI(k + 1u), sdfgiParamI(k + 2u));
}

// `cascades_ubo` (gi.h:572-578, std140): CascadeData { vec3 offset; float
// to_cell; ivec3 probe_world_offset; uint pad; vec4 pad2; }, 8 of them.
struct SdfgiCascadeData {
    offset: vec3<f32>,
    toCell: f32,
    probeWorldOffset: vec3<i32>,
    pad: u32,
    pad2: vec4<f32>,
}

struct SdfgiCascades {
    data: array<SdfgiCascadeData, 8>,
}

// `SDFGIShader::Light` (gi.h:351-367; sdfgi_direct_light.glsl `struct Light`, std140, 112 bytes).
struct SdfgiGpuLight {
    color: vec3<f32>,
    energy: f32,
    direction: vec3<f32>,
    hasShadow: u32,
    position: vec3<f32>,
    attenuation: f32,
    kind: u32,
    cosSpotAngle: f32,
    invSpotAttenuation: f32,
    radius: f32,
    areaWidth: vec4<f32>,
    areaHeight: vec4<f32>,
    areaProjectorRect: vec4<f32>,
}

// `ProcessVoxel` (sdfgi_preprocess.glsl:100-109, the solid-cell list's record).
struct SdfgiProcessVoxel {
    position: u32,
    albedo: u32,
    light: u32,
    lightAniso: u32,
}

// `DispatchData` (sdfgi_preprocess.glsl): the STORE pass's counters, then the indirect dispatch's arguments.
struct SdfgiDispatchData {
    x: u32,
    y: u32,
    z: u32,
    totalCount: u32,
}

struct SdfgiDispatchDataAtomic {
    x: atomic<u32>,
    y: atomic<u32>,
    z: atomic<u32>,
    totalCount: atomic<u32>,
}

const SDFGI_CASCADE_SIZE: i32 = 128;
const SDFGI_PROBE_AXIS: i32 = 17;
const SDFGI_OCT: i32 = 6;
const SDFGI_HISTORY_BITS: i32 = 10;

// `aniso_dir` (+X, +Y, +Z, -X, -Y, -Z), the order of every six-face table.
fn sdfgiAnisoDir(i: u32) -> vec3<f32> {
    switch i {
        case 0u: { return vec3<f32>(1.0, 0.0, 0.0); }
        case 1u: { return vec3<f32>(0.0, 1.0, 0.0); }
        case 2u: { return vec3<f32>(0.0, 0.0, 1.0); }
        case 3u: { return vec3<f32>(-1.0, 0.0, 0.0); }
        case 4u: { return vec3<f32>(0.0, -1.0, 0.0); }
        default: { return vec3<f32>(0.0, 0.0, -1.0); }
    }
}

// A light or probe word (E5B9G9R9, `rgbe_encode`'s layout) as the RGB9E5
// view would decode it: the readers decode the words themselves (wgpu has no
// reinterpreting views; the lab's difference, gpu.rs).
fn sdfgiRgb9e5(w: u32) -> vec3<f32> {
    let m = exp2(f32(i32((w >> 27u) & 0x1Fu) - 15 - 9));
    return vec3<f32>(f32(w & 0x1FFu), f32((w >> 9u) & 0x1FFu), f32((w >> 18u) & 0x1FFu)) * m;
}
