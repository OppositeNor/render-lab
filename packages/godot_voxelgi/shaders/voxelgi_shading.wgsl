// Godot's VoxelGI screen pass (gi.glsl: voxel_cone_trace,
// voxel_cone_trace_45_degrees, voxel_gi_compute, process_gi) and the opaque
// pass's use of its buffers (scene_forward_clustered.glsl), ported to WGSL as
// the lab's pass chain: a depth pre-pass, the scene pass, the GI pass, the
// composite. Composed after the scene module (lab/src/shaders.rs,
// `SceneShading` in lab/src/investigation.rs), so SceneVaryings, gFrame,
// gObjects, and common.wgsl are in scope.
//
// Ported from Godot Engine (MIT; Copyright (c) 2014-present Godot Engine
// contributors, Copyright (c) 2007-2014 Juan Linietsky, Ariel Manzur;
// THIRD_PARTY.md). src/godot/voxelgi/cone.rs mirrors it on the CPU, and
// `selftest` compares the two.
//
// Where the lab differs: Godot's depth pre-pass also writes the
// normal-roughness and tag buffers, process_gi reads them, then the opaque
// pass reads its ambient and reflection buffers. The lab's pre-pass writes
// depth alone; the scene pass after it writes the normal (and its own direct
// light), the GI pass reads them, and a compute pass does the opaque pass's
// mix, for the one VoxelGI around the room. The RGBA8
// 3D texture is a buffer here, and its trilinear, mip-blended sampling
// (textureLod with the LINEAR_WITH_MIPMAPS sampler, clamped to the edge) is
// done by hand. The environment's ambient and reflection are the lab's sky.

// gShading: the investigation's values (the `///` tags; the lab packs them
// from the shot, lab/src/shader_data.rs) and what demos.rs derives.
struct VoxelgiShading {
    /// How much of VoxelGI's light the room shows (0: the environment's alone).
    /// {@ key=voxelgiMix default=1 range=0,1 label=gi text=controls.voxelgi_mix}
    voxelgiMix: f32,
    /// rendering/global_illumination/voxel_gi/quality: 0 Low (4 cones), 1 High (6 cones).
    /// {@ key=quality default=0 range=0,1 label=gi text=controls.quality}
    quality: f32,
    /// VoxelGIData.interior: 1 ignores the environment (the alpha forced to 1).
    /// {@ key=interior default=0 range=0,1 label=gi text=controls.interior}
    interior: f32,
    /// What the scene shows: 0 the lit result, 1 the ambient buffer, 2 the reflection buffer, 3 the ambient's alpha.
    /// {@ key=viewMode default=0 range=0,3 label=gi text=controls.view_mode}
    viewMode: f32,
    /// World position of cell (0, 0, 0) (to_cell_xform's origin).
    /// {@ derived}
    toCellOrigin: vec3<f32>,
    /// Cells per metre.
    /// {@ derived}
    toCellScale: f32,
    /// The octree's size in cells (VoxelGIData.bounds).
    /// {@ derived}
    bounds: vec3<f32>,
    /// dynamic_range * energy.
    /// {@ derived}
    dynamicRange: f32,
    /// VoxelGIData.bias and normal_bias, cells.
    /// {@ derived}
    bias: f32,
    /// {@ derived}
    normalBias: f32,
    /// The texture's mips (one per octree level).
    /// {@ derived}
    mipCount: f32,
    /// The distance field's first word in gShadingData.
    /// {@ derived}
    sdfOffset: f32,
}
/// {@ shader_data}
@group(1) @binding(0) var<uniform> gShading: VoxelgiShading;
// gShadingData (world::shading_data): every mip of the light texture, one
// RGBA8 word per texel, x fastest, mip 0 first; then the distance field, four
// bytes a word; four words a vec4 (bitcast).
@group(1) @binding(1) var<storage, read> gShadingData: array<vec4<f32>>;
@group(1) @binding(2) var<storage, read> gShadingFrame: array<vec4<f32>>;

// gi.glsl's M_PI.
const VOXELGI_M_PI: f32 = 3.141592;

fn voxelgiWord(i: u32) -> u32 {
    return bitcast<u32>(gShadingData[i / 4u][i % 4u]);
}

fn voxelgiMipSize(mip: i32) -> vec3<i32> {
    return max(vec3<i32>(gShading.bounds) >> vec3<u32>(u32(mip)), vec3<i32>(1));
}

fn voxelgiMipOffset(mip: i32) -> u32 {
    var o = 0u;
    for (var m = 0; m < mip; m++) {
        let s = voxelgiMipSize(m);
        o += u32(s.x * s.y * s.z);
    }
    return o;
}

// texelFetch of the RGBA8 texture, clamped to the edge.
fn voxelgiTexel(mip: i32, offset: u32, p: vec3<i32>) -> vec4<f32> {
    let s = voxelgiMipSize(mip);
    let c = clamp(p, vec3<i32>(0), s - vec3<i32>(1));
    return unpack4x8unorm(voxelgiWord(offset + u32(c.x + c.y * s.x + c.z * s.x * s.y)));
}

// Trilinear inside one mip.
fn voxelgiSampleMip(mip: i32, uvw: vec3<f32>) -> vec4<f32> {
    let offset = voxelgiMipOffset(mip);
    let t = uvw * vec3<f32>(voxelgiMipSize(mip)) - vec3<f32>(0.5);
    let b = floor(t);
    let f = t - b;
    let bi = vec3<i32>(b);
    var sum = vec4<f32>(0.0);
    for (var i = 0; i < 8; i++) {
        let o = vec3<i32>(i & 1, (i >> 1u) & 1, (i >> 2u) & 1);
        let w = select(vec3<f32>(1.0) - f, f, o == vec3<i32>(1));
        sum += voxelgiTexel(mip, offset, bi + o) * (w.x * w.y * w.z);
    }
    return sum;
}

// textureLod: the level clamped to the mips, the two nearest blended.
fn voxelgiSampleLod(uvw: vec3<f32>, lodIn: f32) -> vec4<f32> {
    let lod = clamp(lodIn, 0.0, gShading.mipCount - 1.0);
    let m0 = i32(floor(lod));
    let m1 = min(m0 + 1, i32(gShading.mipCount) - 1);
    let f = lod - f32(m0);
    let a = voxelgiSampleMip(m0, uvw);
    if (f == 0.0 || m0 == m1) {
        return a;
    }
    return a * (1.0 - f) + voxelgiSampleMip(m1, uvw) * f;
}

// voxel_cone_trace.
fn voxelgiConeTrace(cellSize: vec3<f32>, pos: vec3<f32>, direction: vec3<f32>, tanHalfAngle: f32, maxDistance: f32, bias: f32) -> vec4<f32> {
    var dist = bias;
    var color = vec4<f32>(0.0);
    while (dist < maxDistance && color.a < 1.0) {
        let diameter = max(1.0, 2.0 * tanHalfAngle * dist);
        let uvw = (pos + dist * direction) * cellSize;
        let halfDiameter = diameter * 0.5;
        if (any(abs(uvw - vec3<f32>(0.5)) > vec3<f32>(0.5) + halfDiameter * cellSize)) {
            break;
        }
        let scolor = voxelgiSampleLod(uvw, log2(diameter));
        let a = 1.0 - color.a;
        color += a * scolor;
        dist += halfDiameter;
    }
    return color;
}

// voxel_cone_trace_45_degrees.
fn voxelgiConeTrace45(cellSize: vec3<f32>, pos: vec3<f32>, direction: vec3<f32>, maxDistance: f32, bias: f32) -> vec4<f32> {
    var dist = bias;
    var color = vec4<f32>(0.0);
    var radius = max(0.5, dist);
    var lodLevel = log2(radius * 2.0);
    while (dist < maxDistance && color.a < 1.0) {
        let uvw = (pos + dist * direction) * cellSize;
        if (any(abs(uvw - vec3<f32>(0.5)) > vec3<f32>(0.5) + radius * cellSize)) {
            break;
        }
        var scolor = voxelgiSampleLod(uvw, lodLevel);
        lodLevel += 1.0;
        let a = 1.0 - color.a;
        scolor *= a;
        color += scolor;
        dist += radius;
        radius = max(0.5, dist);
    }
    return color;
}

struct VoxelgiResult {
    ambient: vec4<f32>,
    reflection: vec4<f32>,
}

// process_gi's VoxelGI part for the lab's one instance: roughness squared,
// voxel_gi_compute, and the sums divided by the blend.
fn voxelgiProcess(positionWorld: vec3<f32>, normalWorld: vec3<f32>, refWorld: vec3<f32>, roughnessIn: f32, highQuality: bool, blendAmbient: bool) -> VoxelgiResult {
    var out: VoxelgiResult;
    out.ambient = vec4<f32>(0.0);
    out.reflection = vec4<f32>(0.0);
    let roughness = roughnessIn * roughnessIn;
    let normal = normalize(normalWorld);
    let refVec = normalize(refWorld);
    var position = (positionWorld - gShading.toCellOrigin) * gShading.toCellScale;
    position += normal * gShading.normalBias;
    if (any(position < vec3<f32>(0.0)) || any(position > gShading.bounds)) {
        return out;
    }
    // The tangent frame process_gi builds (v0 = |n.z| < 0.999 ? +z : +y).
    var v0 = vec3<f32>(0.0, 1.0, 0.0);
    if (abs(normal.z) < 0.999) {
        v0 = vec3<f32>(0.0, 0.0, 1.0);
    }
    let tangent = normalize(cross(v0, normal));
    let bitangent = normalize(cross(tangent, normal));
    let blendv = abs(position / gShading.bounds * 2.0 - vec3<f32>(1.0));
    let blend = clamp(1.0 - max(blendv.x, max(blendv.y, blendv.z)), 0.0, 1.0);
    let maxDistance = length(gShading.bounds);
    let cellSize = vec3<f32>(1.0) / gShading.bounds;
    var light = vec4<f32>(0.0);
    if (highQuality) {
        var dirs = array<vec3<f32>, 6>(vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(0.866025, 0.0, 0.5), vec3<f32>(0.267617, 0.823639, 0.5), vec3<f32>(-0.700629, 0.509037, 0.5), vec3<f32>(-0.700629, -0.509037, 0.5), vec3<f32>(0.267617, -0.823639, 0.5));
        var weights = array<f32, 6>(0.25, 0.15, 0.15, 0.15, 0.15, 0.15);
        for (var i = 0; i < 6; i++) {
            let d = dirs[i];
            let dir = normalize(tangent * d.x + bitangent * d.y + normal * d.z);
            light += weights[i] * voxelgiConeTrace(cellSize, position, dir, 0.577, maxDistance, gShading.bias);
        }
    } else {
        var dirs = array<vec3<f32>, 4>(vec3<f32>(0.707107, 0.0, 0.707107), vec3<f32>(0.0, 0.707107, 0.707107), vec3<f32>(-0.707107, 0.0, 0.707107), vec3<f32>(0.0, -0.707107, 0.707107));
        for (var i = 0; i < 4; i++) {
            let d = dirs[i];
            let dir = normalize(tangent * d.x + bitangent * d.y + normal * d.z);
            light += 0.25 * voxelgiConeTrace45(cellSize, position, dir, maxDistance, gShading.bias);
        }
    }
    light = vec4<f32>(light.rgb * gShading.dynamicRange, select(1.0, light.a, blendAmbient));
    var irr = voxelgiConeTrace(cellSize, position, refVec, tan(roughness * 0.5 * VOXELGI_M_PI * 0.99), maxDistance, gShading.bias);
    irr = vec4<f32>(irr.rgb * gShading.dynamicRange, select(1.0, irr.a, blendAmbient));
    if (blend > 0.0) {
        out.ambient = light * blend / blend;
        out.reflection = irr * blend / blend;
    }
    return out;
}

// The sky's reflection at a roughness: the lab's environment (its lighting
// level) averaged over nine directions spread by the roughness, where Godot
// reads its radiance map at the roughness's mip. demos::sky_reflection mirrors it.
fn voxelgiSkyReflection(r: vec3<f32>, roughness: f32) -> vec3<f32> {
    var v0 = vec3<f32>(0.0, 1.0, 0.0);
    if (abs(r.y) > 0.999) {
        v0 = vec3<f32>(1.0, 0.0, 0.0);
    }
    let t = normalize(cross(v0, r));
    let b = cross(r, t);
    var sum = vec3<f32>(0.0);
    for (var i = -1; i <= 1; i++) {
        for (var j = -1; j <= 1; j++) {
            sum += environmentRadiance(normalize(r + (t * f32(i) + b * f32(j)) * roughness));
        }
    }
    return sum / 9.0;
}

// The objects fading out of the frame cover that share of a pixel's MSAA
// samples, dithered from pixel to pixel (the pre-pass and the scene pass alike).
fn voxelgiCoverage(objectIndex: u32, pixel: vec2<f32>) -> u32 {
    let presence = objectPresence(objectIndex);
    if (presence < 1.0) {
        return (1u << u32(clamp(floor(presence * 4.0 + interleavedGradientNoise(pixel)), 0.0, 4.0))) - 1u;
    }
    return 0xffffffffu;
}

// The depth pre-pass (render_forward_clustered.cpp's "Render Depth Pre-Pass";
// Godot's also writes the normal-roughness and VoxelGI tag buffers, which the
// lab's scene pass writes instead): depth only, so the scene pass after it
// shades each pixel's visible surface once.
@fragment
fn voxelgiPrepassFS(input: SceneVaryings) -> @builtin(sample_mask) u32 {
    return voxelgiCoverage(input.objectIndex, input.clip.xy);
}

struct VoxelgiForwardOutput {
    // The lab's direct light from the panel and the emission; alpha the object
    // index + 1 (0 where nothing was drawn).
    @location(0) direct: vec4<f32>,
    // The normal, turned toward the eye.
    @location(1) normal: vec4<f32>,
    @builtin(sample_mask) coverage: u32,
}

// The scene pass, against the pre-pass's depth: the lab's direct light, and
// the normal the GI pass reads (Godot's normal-roughness buffer; the
// roughness is the object's).
@fragment
fn voxelgiForwardFS(input: SceneVaryings) -> VoxelgiForwardOutput {
    var n = normalize(input.worldNormal);
    let v = normalize(gFrame.camera.position - input.worldPosition);
    if (dot(n, v) < 0.0) {
        n = -n;
    }
    let o = gObjects[input.objectIndex];
    let direct = o.albedoRoughness.rgb * directIrradiance(input.worldPosition, n, input.objectIndex) / PI + o.emissive.rgb * o.emissive.w;
    return VoxelgiForwardOutput(vec4<f32>(direct, f32(input.objectIndex) + 1.0), vec4<f32>(n, 0.0), voxelgiCoverage(input.objectIndex, input.clip.xy));
}

// The chain's targets (SceneShading in src/godot/voxelgi/mod.rs): target i
// read at binding 2 i, written at 2 i + 1; then the depth and the HDR target.
@group(2) @binding(0) var tDirect: texture_multisampled_2d<f32>;
@group(2) @binding(2) var tNormal: texture_multisampled_2d<f32>;
@group(2) @binding(4) var tGiAmbient: texture_2d<f32>;
@group(2) @binding(5) var oGiAmbient: texture_storage_2d<rgba16float, write>;
@group(2) @binding(6) var tGiReflection: texture_2d<f32>;
@group(2) @binding(7) var oGiReflection: texture_storage_2d<rgba16float, write>;
@group(2) @binding(8) var tDepth: texture_depth_multisampled_2d;
@group(2) @binding(9) var oHdr: texture_storage_2d<rgba16float, write>;

// The world position at a pixel's centre and a reversed depth: the view-space
// z the projection maps to that depth, along the pixel's view ray.
fn voxelgiWorldPosition(pixel: vec2<u32>, size: vec2<u32>, depth: f32) -> vec3<f32> {
    let c = gFrame.camera;
    let uv = (vec2<f32>(pixel) + vec2<f32>(0.5)) / vec2<f32>(size);
    let z = -c.viewToClip[3].z / (depth + c.viewToClip[2].z);
    let p = vec3<f32>((uv.x * 2.0 - 1.0) / c.viewToClip[0][0], (1.0 - uv.y * 2.0) / c.viewToClip[1][1], -1.0) * -z;
    let r = c.worldToView;
    return c.position + vec3<f32>(dot(r[0].xyz, p), dot(r[1].xyz, p), dot(r[2].xyz, p));
}

// The GI pass (gi.cpp's process_gi, gi.glsl): once per pixel, on its first
// sample (Godot resolves its MSAA buffers to one sample first, resolve_gi),
// voxel_gi_compute into the ambient and reflection buffers.
@compute @workgroup_size(8, 8)
fn voxelgiGiCS(@builtin(global_invocation_id) id: vec3<u32>) {
    let pixel = id.xy;
    let size = textureDimensions(oGiAmbient);
    if (any(pixel >= size)) {
        return;
    }
    var gi: VoxelgiResult;
    let depth = textureLoad(tDepth, pixel, 0);
    let direct = textureLoad(tDirect, pixel, 0);
    if (depth > 0.0 && direct.a > 0.5) {
        let p = voxelgiWorldPosition(pixel, size, depth);
        let n = textureLoad(tNormal, pixel, 0).xyz;
        let roughness = gObjects[u32(direct.a - 0.5)].albedoRoughness.w;
        let r = reflect(-normalize(gFrame.camera.position - p), n);
        gi = voxelgiProcess(p, n, r, roughness, gShading.quality > 0.5, gShading.interior < 0.5);
    }
    textureStore(oGiAmbient, pixel, gi.ambient);
    textureStore(oGiReflection, pixel, gi.reflection);
}

// The opaque pass's use of the GI buffers (scene_forward_clustered.glsl),
// sample by sample, averaged into the HDR target: the sky where nothing was
// drawn; else the direct light, then the environment's ambient and
// reflection mixed toward VoxelGI's by its alpha, ambient times albedo, the
// reflection times the environment BRDF.
@compute @workgroup_size(8, 8)
fn voxelgiComposeCS(@builtin(global_invocation_id) id: vec3<u32>) {
    let pixel = id.xy;
    let size = textureDimensions(oHdr);
    if (any(pixel >= size)) {
        return;
    }
    let gi = VoxelgiResult(textureLoad(tGiAmbient, pixel, 0), textureLoad(tGiReflection, pixel, 0));
    let mode = i32(round(gShading.viewMode));
    let samples = textureNumSamples(tDirect);
    var sum = vec3<f32>(0.0);
    for (var i = 0u; i < samples; i++) {
        let depth = textureLoad(tDepth, pixel, i);
        let direct = textureLoad(tDirect, pixel, i);
        if (depth <= 0.0 || direct.a < 0.5) {
            sum += skyRadiance(viewRay((vec2<f32>(pixel) + vec2<f32>(0.5)) / vec2<f32>(size)));
            continue;
        }
        let o = gObjects[u32(direct.a - 0.5)];
        let albedo = o.albedoRoughness.rgb;
        let roughness = o.albedoRoughness.w;
        let p = voxelgiWorldPosition(pixel, size, depth);
        let n = textureLoad(tNormal, pixel, i).xyz;
        let v = normalize(gFrame.camera.position - p);
        let r = reflect(-v, n);
        let ambient = mix(environmentRadiance(n), gi.ambient.rgb, gi.ambient.a * gShading.voxelgiMix);
        let reflection = mix(voxelgiSkyReflection(r, roughness), gi.reflection.rgb, gi.reflection.a * gShading.voxelgiMix);
        let nv = max(dot(n, v), 1e-4);
        var color = direct.rgb + albedo * ambient + reflection * envBRDFApprox(0.04, roughness, nv);
        if (mode == 1) {
            color = gi.ambient.rgb;
        } else if (mode == 2) {
            color = gi.reflection.rgb;
        } else if (mode == 3) {
            color = vec3<f32>(gi.ambient.a);
        }
        sum += color;
    }
    textureStore(oHdr, pixel, vec4<f32>(sum / f32(samples), 1.0));
}

// ---------------------------------------------------------------------------
// Verification kernels (selftest; cone.rs and light.rs are the CPU mirrors).
@group(2) @binding(0) var<storage, read_write> gVoxelgiOut: array<u32>;
@group(2) @binding(1) var<storage, read> gVoxelgiTests: array<vec4<f32>>;

fn voxelgiStore(o: u32, v: vec4<f32>) {
    gVoxelgiOut[o] = bitcast<u32>(v.x);
    gVoxelgiOut[o + 1u] = bitcast<u32>(v.y);
    gVoxelgiOut[o + 2u] = bitcast<u32>(v.z);
    gVoxelgiOut[o + 3u] = bitcast<u32>(v.w);
}

// Input, three vectors per test: (world position, roughness), (normal,
// quality), (reflection, blend_ambient). Output, 8 words: the ambient and
// the reflection, rgba.
@compute @workgroup_size(1)
fn voxelgiProcessTests(@builtin(workgroup_id) id: vec3<u32>) {
    let a = gVoxelgiTests[id.x * 3u];
    let b = gVoxelgiTests[id.x * 3u + 1u];
    let c = gVoxelgiTests[id.x * 3u + 2u];
    let r = voxelgiProcess(a.xyz, b.xyz, c.xyz, a.w, b.w > 0.5, c.w > 0.5);
    voxelgiStore(id.x * 8u, r.ambient);
    voxelgiStore(id.x * 8u + 4u, r.reflection);
}
