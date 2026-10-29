// Godot's SDFGI screen pass (gi.glsl: sdfgi_process, sdfvoxel_gi_process),
// ported to WGSL for the lab's scene pass. Composed after the scene module
// (lab/src/shaders.rs, `SceneShading` in lab/src/investigation.rs), so
// SceneVaryings, SceneOutput, gFrame, gObjects, and common.wgsl are in scope.
//
// Ported from Godot Engine (MIT; Copyright (c) 2014-present Godot Engine
// contributors, Copyright (c) 2007-2014 Juan Linietsky, Ariel Manzur;
// THIRD_PARTY.md). src/godot/sdfgi/screen.rs mirrors it on the CPU, and
// `selftest` compares the two.
//
// Where the lab differs: Godot runs this as a compute pass over the depth and
// normal-roughness buffers (GI::process_gi); the lab evaluates the same
// function per fragment of the scene pass, with the fragment's position and
// normal. The specular march below roughness 0.2 is not run here (the room's
// materials are rougher); the probes' radiance is used at every roughness.
// SDFGI follows a camera that stays at the room's centre (sim.rs), so
// "camera-relative" positions are relative to that point, not to the view.

// gShading: the investigation's values (the `///` tags; the lab packs them
// from the shot, lab/src/shader_data.rs) and what demos.rs derives.
struct SdfgiShading {
    /// How much of SDFGI's ambient and reflection the room shows (0: direct light only).
    /// {@ key=sdfgiMix default=1 range=0,1 label=gi text=controls.sdfgi_mix}
    sdfgiMix: f32,
    /// The environment's sdfgi_energy (SDFGIData.energy).
    /// {@ key=sdfgiEnergy default=1 range=0,4 label=gi text=controls.sdfgi_energy}
    energy: f32,
    /// The environment's sdfgi_normal_bias, in cells.
    /// {@ key=normalBias default=1.1 range=0,4 label=gi text=controls.normal_bias}
    normalBias: f32,
    /// y_mult (y_scale[sdfgi_y_scale]).
    /// {@ derived}
    yMult: f32,
    /// The number of cascades.
    /// {@ derived}
    cascadeCount: f32,
    /// Where SDFGI's camera stands (the lab's sim.rs ORIGIN).
    /// {@ derived}
    sdfgiCamera: vec3<f32>,
}
/// {@ shader_data}
@group(1) @binding(0) var<uniform> gShading: SdfgiShading;
// gShadingData (sim::shading_data): per cascade, the light-probe texture's
// irradiance layer then its radiance layer, MAP_WIDTH x MAP_HEIGHT
// E5B9G9R9 words, row-major, four words per vec4 (bitcast).
@group(1) @binding(1) var<storage, read> gShadingData: array<vec4<f32>>;
// gShadingFrame (demos.rs), per frame: two vectors per cascade, as
// SDFGIData::ProbeCascadeData holds it: (position relative to SDFGI's
// camera, to_probe), (probe_world_offset, to_cell).
@group(1) @binding(2) var<storage, read> gShadingFrame: array<vec4<f32>>;

const SDFGI_OCT_SIZE: i32 = 6;
const SDFGI_PROBE_AXIS_SIZE: i32 = 17;
const SDFGI_MAP_WIDTH: i32 = 2312; // 17 * 17 * 8
const SDFGI_MAP_HEIGHT: i32 = 136; // 17 * 8
const SDFGI_CASCADE_PROBE_SIZE: f32 = 16.0;

fn sdfgiDecodeE5b9g9r9(w: u32) -> vec3<f32> {
    let m = exp2(f32(i32((w >> 27u) & 0x1Fu) - 15 - 9));
    return vec3<f32>(f32(w & 0x1FFu), f32((w >> 9u) & 0x1FFu), f32((w >> 18u) & 0x1FFu)) * m;
}

// One texel of a light-probe layer (clamped to the layer).
fn sdfgiTexel(layer: u32, x: i32, y: i32) -> vec3<f32> {
    let cx = clamp(x, 0, SDFGI_MAP_WIDTH - 1);
    let cy = clamp(y, 0, SDFGI_MAP_HEIGHT - 1);
    let word = layer * u32(SDFGI_MAP_WIDTH * SDFGI_MAP_HEIGHT) + u32(cx + cy * SDFGI_MAP_WIDTH);
    let v = gShadingData[word / 4u];
    return sdfgiDecodeE5b9g9r9(bitcast<u32>(v[word % 4u]));
}

// textureLod with a linear sampler at a texel position (centres at k + 0.5).
fn sdfgiBilinear(layer: u32, pos: vec2<f32>) -> vec3<f32> {
    let t = pos - vec2<f32>(0.5);
    let b = floor(t);
    let f = t - b;
    let x0 = i32(b.x);
    let y0 = i32(b.y);
    let a = mix(sdfgiTexel(layer, x0, y0), sdfgiTexel(layer, x0 + 1, y0), f.x);
    let c = mix(sdfgiTexel(layer, x0, y0 + 1), sdfgiTexel(layer, x0 + 1, y0 + 1), f.x);
    return mix(a, c, f.y);
}

fn sdfgiOctahedronEncode(nIn: vec3<f32>) -> vec2<f32> {
    var n = nIn / (abs(nIn.x) + abs(nIn.y) + abs(nIn.z));
    if (n.z < 0.0) {
        let s = vec2<f32>(select(-1.0, 1.0, n.x >= 0.0), select(-1.0, 1.0, n.y >= 0.0));
        n = vec3<f32>((vec2<f32>(1.0) - abs(n.yx)) * s, n.z);
    }
    return n.xy * 0.5 + vec2<f32>(0.5);
}

fn sdfgiCascadePosition(c: u32) -> vec3<f32> {
    return gShadingFrame[c * 2u].xyz;
}

fn sdfgiToProbe(c: u32) -> f32 {
    return gShadingFrame[c * 2u].w;
}

struct SdfgiGi {
    diffuse: vec3<f32>,
    specular: vec3<f32>,
}

// sdfvoxel_gi_process: the eight probes around the biased point, weighted by
// the trilinear tent and by facing, their irradiance at the normal and their
// radiance (and, above roughness 0.2, irradiance) at the reflection.
fn sdfgiVoxelGi(cascade: u32, cascadePosIn: vec3<f32>, camNormal: vec3<f32>, specularNormal: vec3<f32>, roughness: f32) -> SdfgiGi {
    // normal_bias in probe units: normal_bias / cascade_size * PROBE_DIVISOR.
    let cascadePos = cascadePosIn + camNormal * (gShading.normalBias / 128.0 * 16.0);
    let base = floor(cascadePos);
    let probeBase = vec3<i32>(base);
    let diffuseOct = sdfgiOctahedronEncode(camNormal) * f32(SDFGI_OCT_SIZE);
    let specularOct = sdfgiOctahedronEncode(specularNormal) * f32(SDFGI_OCT_SIZE);
    let irradianceLayer = cascade * 2u;
    let radianceLayer = cascade * 2u + 1u;
    var diffuseAccum = vec4<f32>(0.0);
    var specularAccum = vec3<f32>(0.0);
    for (var j = 0u; j < 8u; j++) {
        let offset = vec3<i32>(vec3<u32>(j, j >> 1u, j >> 2u) & vec3<u32>(1u));
        let probe = probeBase + offset;
        let probeToPos = cascadePos - vec3<f32>(probe);
        let probeDir = normalize(-probeToPos);
        let trilinear = vec3<f32>(1.0) - abs(probeToPos);
        let weight = trilinear.x * trilinear.y * trilinear.z * max(0.005, dot(camNormal, probeDir));
        let texBase = vec2<f32>(f32((probe.x + probe.z * SDFGI_PROBE_AXIS_SIZE) * (SDFGI_OCT_SIZE + 2) + 1), f32(probe.y * (SDFGI_OCT_SIZE + 2) + 1));
        let diffuse = sdfgiBilinear(irradianceLayer, texBase + diffuseOct);
        diffuseAccum += vec4<f32>(diffuse * weight, weight);
        var specular = vec3<f32>(0.0);
        if (roughness < 0.99) {
            specular = sdfgiBilinear(radianceLayer, texBase + specularOct);
        }
        if (roughness > 0.2) {
            specular = mix(specular, sdfgiBilinear(irradianceLayer, texBase + specularOct), (roughness - 0.2) * 1.25);
        }
        specularAccum += specular * weight;
    }
    var out: SdfgiGi;
    out.diffuse = diffuseAccum.rgb;
    out.specular = specularAccum;
    if (diffuseAccum.a > 0.0) {
        out.diffuse /= diffuseAccum.a;
        out.specular /= diffuseAccum.a;
    }
    return out;
}

struct SdfgiResult {
    ambient: vec4<f32>,
    reflection: vec4<f32>,
}

// sdfgi_process: the finest cascade that holds the point, blended toward the
// next one over its last two probes (or toward the sky at the last).
fn sdfgiProcess(vertexIn: vec3<f32>, normalIn: vec3<f32>, reflectionIn: vec3<f32>, roughness: f32) -> SdfgiResult {
    let yMult = gShading.yMult;
    let vertex = vec3<f32>(vertexIn.x, vertexIn.y * yMult, vertexIn.z);
    let camNormal = normalize(vec3<f32>(normalIn.x, normalIn.y * yMult, normalIn.z));
    let reflection = normalize(vec3<f32>(reflectionIn.x, reflectionIn.y * yMult, reflectionIn.z));
    var out: SdfgiResult;
    out.ambient = vec4<f32>(0.0);
    out.reflection = vec4<f32>(0.0);
    let count = u32(gShading.cascadeCount);
    var cascade = 0xFFFFFFFFu;
    var cascadePos = vec3<f32>(0.0);
    for (var i = 0u; i < count; i++) {
        cascadePos = (vertex - sdfgiCascadePosition(i)) * sdfgiToProbe(i);
        if (any(cascadePos < vec3<f32>(0.0)) || any(cascadePos >= vec3<f32>(SDFGI_CASCADE_PROBE_SIZE))) {
            continue;
        }
        cascade = i;
        break;
    }
    if (cascade == 0xFFFFFFFFu) {
        return out;
    }
    out.ambient = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    out.reflection = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    var gi = sdfgiVoxelGi(cascade, cascadePos, camNormal, reflection, roughness);
    let blendFrom = f32(SDFGI_PROBE_AXIS_SIZE - 1) / 2.0 - 2.5;
    let blendTo = blendFrom + 2.0;
    let inner = vertex * sdfgiToProbe(cascade);
    let len = max(abs(inner.x), max(abs(inner.y), abs(inner.z)));
    var blend = 0.0;
    if (len >= blendFrom) {
        blend = smoothstep(blendFrom, blendTo, len);
    }
    if (blend > 0.0) {
        if (cascade == count - 1u) {
            out.ambient.a = 1.0 - blend;
            out.reflection.a = 1.0 - blend;
        } else {
            let pos2 = (vertex - sdfgiCascadePosition(cascade + 1u)) * sdfgiToProbe(cascade + 1u);
            let gi2 = sdfgiVoxelGi(cascade + 1u, pos2, camNormal, reflection, roughness);
            gi.diffuse = mix(gi.diffuse, gi2.diffuse, blend);
            gi.specular = mix(gi.specular, gi2.specular, blend);
        }
    }
    out.ambient = vec4<f32>(gi.diffuse * gShading.energy, out.ambient.a);
    out.reflection = vec4<f32>(gi.specular * gShading.energy, out.reflection.a);
    return out;
}

// The scene pass: the lab's direct light from the panel, plus SDFGI's
// ambient times albedo (scene_forward_clustered.glsl: ambient_light *= albedo)
// and its reflection times the environment BRDF.
@fragment
fn sdfgiForwardFS(input: SceneVaryings) -> SceneOutput {
    var n = normalize(input.worldNormal);
    let eye = gFrame.camera.position;
    let v = normalize(eye - input.worldPosition);
    if (dot(n, v) < 0.0) {
        n = -n;
    }
    let o = gObjects[input.objectIndex];
    let albedo = o.albedoRoughness.rgb;
    let roughness = o.albedoRoughness.w;
    var color = albedo * directIrradiance(input.worldPosition, n, input.objectIndex) / PI + o.emissive.rgb * o.emissive.w;
    let gi = sdfgiProcess(input.worldPosition - gShading.sdfgiCamera, n, reflect(-v, n), roughness);
    let ambient = gi.ambient.rgb * gi.ambient.a;
    let nv = max(dot(n, v), 1e-4);
    let env = envBrdfApprox(0.04, roughness, nv);
    let reflection = gi.reflection.rgb * gi.reflection.a * env;
    color += gShading.sdfgiMix * (albedo * ambient + reflection);
    var coverage = 0xffffffffu;
    let presence = objectPresence(input.objectIndex);
    if (presence < 1.0) {
        coverage = (1u << u32(clamp(floor(presence * 4.0 + interleavedGradientNoise(input.clip.xy)), 0.0, 4.0))) - 1u;
    }
    return SceneOutput(vec4<f32>(color, 1.0), coverage);
}

// The environment BRDF fit the lab's floor uses (Karis, "Physically Based
// Shading on Mobile", 2014), here for the probes' reflection.
fn envBrdfApprox(f0: f32, roughness: f32, nv: f32) -> f32 {
    let c0 = vec4<f32>(-1.0, -0.0275, -0.572, 0.022);
    let c1 = vec4<f32>(1.0, 0.0425, 1.04, -0.04);
    let r = roughness * c0 + c1;
    let a004 = min(r.x * r.x, exp2(-9.28 * nv)) * r.x + r.y;
    let ab = vec2<f32>(-1.04, 1.04) * a004 + r.zw;
    return f0 * ab.x + ab.y;
}

// ---------------------------------------------------------------------------
// Verification kernel (selftest; screen.rs is the CPU mirror).
@group(2) @binding(0) var<storage, read_write> gSdfgiOut: array<u32>;
@group(2) @binding(1) var<storage, read> gSdfgiTests: array<vec4<f32>>;

// Input, three vectors per test: (vertex relative to SDFGI's camera,
// roughness), (normal, unused), (reflection, unused). Output, 8 words: the
// ambient and the reflection, rgba.
@compute @workgroup_size(1)
fn sdfgiProcessTests(@builtin(workgroup_id) id: vec3<u32>) {
    let a = gSdfgiTests[id.x * 3u];
    let b = gSdfgiTests[id.x * 3u + 1u];
    let c = gSdfgiTests[id.x * 3u + 2u];
    let r = sdfgiProcess(a.xyz, b.xyz, c.xyz, a.w);
    let o = id.x * 8u;
    gSdfgiOut[o] = bitcast<u32>(r.ambient.x);
    gSdfgiOut[o + 1u] = bitcast<u32>(r.ambient.y);
    gSdfgiOut[o + 2u] = bitcast<u32>(r.ambient.z);
    gSdfgiOut[o + 3u] = bitcast<u32>(r.ambient.w);
    gSdfgiOut[o + 4u] = bitcast<u32>(r.reflection.x);
    gSdfgiOut[o + 5u] = bitcast<u32>(r.reflection.y);
    gSdfgiOut[o + 6u] = bitcast<u32>(r.reflection.z);
    gSdfgiOut[o + 7u] = bitcast<u32>(r.reflection.w);
}
