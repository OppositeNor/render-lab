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
// mix, for the one VoxelGI around the room. The RGBA8 3D texture is a 3D
// texture, as Godot's: the light passes (voxelgi_passes.wgsl) write it, and
// it is sampled with a trilinear, mip-blended sampler clamped to the edge
// (Godot's LINEAR_WITH_MIPMAPS). The environment's ambient and reflection
// are the lab's sky.

// gShading: the investigation's values (the `///` tags; the lab packs them
// from the shot, lab/src/shader_data.rs) and what demos.rs derives.
struct VoxelgiShading {
    /// How much of VoxelGI's light the room shows (0: the environment's alone).
    /// {@ key=voxelgiMix default=1 range=0,1 label=gi text=controls-voxelgi_mix help=help-voxelgiMix}
    voxelgiMix: f32,
    /// rendering/global_illumination/voxel_gi/quality: 0 Low (4 cones), 1 High (6 cones).
    /// {@ key=quality default=0 range=0,1 label=gi text=controls-quality engine=ProjectSettings.rendering/global_illumination/voxel_gi/quality help=help-quality}
    quality: f32,
    /// VoxelGIData.interior: 1 ignores the environment (the alpha forced to 1).
    /// {@ key=interior default=0 range=0,1 label=gi text=controls-interior engine=VoxelGIData.interior help=help-interior}
    interior: f32,
    /// What the scene shows: 0 the lit result, 1 the ambient buffer, 2 the reflection buffer, 3 the ambient's alpha.
    /// {@ key=viewMode default=0 range=0,3 label=gi text=controls-view_mode help=help-viewMode}
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
    /// dynamic_range * energy (giDynamicRange, giEnergy).
    /// {@ derived}
    dynamicRange: f32,
    /// VoxelGIData's exposure_normalization: 1 without physical light units (gi.cpp:3897-3902), as in the lab.
    /// {@ derived}
    exposureNormalization: f32,
    /// VoxelGIData.bias and normal_bias, cells.
    /// {@ derived}
    bias: f32,
    /// {@ derived}
    normalBias: f32,
    /// The texture's mips (one per octree level).
    /// {@ derived}
    mipCount: f32,
    /// rendering/global_illumination/gi/use_half_resolution (giHalfResolution): 1 traces one pixel of each 2 x 2 block.
    /// {@ derived}
    halfResolution: f32,
    /// The extra omni light (extraLight; world.rs `omni_light`): its world position, and 1 when it is on.
    /// {@ derived}
    omniPosition: vec3<f32>,
    /// {@ derived}
    omniOn: f32,
    /// Its colour times its energy, and its range in metres.
    /// {@ derived}
    omniColor: vec3<f32>,
    /// {@ derived}
    omniRange: f32,
    /// Its attenuation (decay).
    /// {@ derived}
    omniAttenuation: f32,
    /// The object with GI mode Dynamic, plus one (0: none): the teapot in the dynamic demonstration.
    /// {@ derived}
    dynamicObject: f32,
}
/// {@ shader_data}
@group(1) @binding(0) var<uniform> gShading: VoxelgiShading;
// gShadingData (world::shading_data): the distance field, four bytes a
// word, then the light passes' cells, children, and lights; four words a vec4
// (bitcast).
@group(1) @binding(1) var<storage, read> gShadingData: array<vec4<f32>>;
@group(1) @binding(2) var<storage, read> gShadingFrame: array<vec4<f32>>;

// The program's images the GI pass reads (shading.toml's scene-uses; update.rs
// writes them): the light texture, RGBA8, one mip per octree level, and the
// sampler gi.glsl reads it with (SAMPLER_LINEAR_WITH_MIPMAPS_CLAMP); and the
// distance field (R8, one mip), which the light passes march.
@group(3) @binding(0) var gVolumeSampler: sampler;
@group(3) @binding(1) var gVoxelgiLight: texture_3d<f32>;
@group(3) @binding(2) var gVoxelgiSdf: texture_3d<f32>;

// gi.glsl's M_PI.
const VOXELGI_M_PI: f32 = 3.141592;

fn voxelgiWord(i: u32) -> u32 {
    return bitcast<u32>(gShadingData[i / 4u][i % 4u]);
}

// textureLod(sampler3D(voxel_gi_textures[index], linear_sampler_with_mipmaps), uvw, lod).
fn voxelgiSampleLod(uvw: vec3<f32>, lod: f32) -> vec4<f32> {
    return textureSampleLevel(gVoxelgiLight, gVolumeSampler, uvw, clamp(lod, 0.0, gShading.mipCount - 1.0));
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
    light = vec4<f32>(light.rgb * gShading.dynamicRange * gShading.exposureNormalization, select(1.0, light.a, blendAmbient));
    var irr = voxelgiConeTrace(cellSize, position, refVec, tan(roughness * 0.5 * VOXELGI_M_PI * 0.99), maxDistance, gShading.bias);
    irr = vec4<f32>(irr.rgb * gShading.dynamicRange * gShading.exposureNormalization, select(1.0, irr.a, blendAmbient));
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
    // Godot's normal-roughness buffer, the GI pass's input.
    @location(2) normalRoughness: vec4<f32>,
    @builtin(sample_mask) coverage: u32,
}

// MODE_RENDER_NORMAL_ROUGHNESS's output (scene_forward_clustered.glsl:3019-3028):
// the normal made to touch the unit cube (encode24 without its best-fit
// scale, which the GI pass's normalisation undoes), mapped to 0..1; the
// roughness times 127 / 255, mirrored above one half for a dynamic object.
fn voxelgiNormalRoughness(n: vec3<f32>, roughness: f32, objectIndex: u32) -> vec4<f32> {
    let cube = n / max(abs(n.z), max(abs(n.x), abs(n.y)));
    var w = roughness * (127.0 / 255.0);
    if (f32(objectIndex) + 1.0 == gShading.dynamicObject) {
        w = 1.0 - w;
    }
    return vec4<f32>(cube * 0.5 + 0.5, w);
}

// The lab's direct light from the extra omni light, when it is on: Godot's
// omni falloff (get_omni_attenuation) times the cosine, shadowed by the lab's
// ray-traced visibility (the panel's direct light is the lab's own as well).
fn voxelgiOmniIrradiance(p: vec3<f32>, n: vec3<f32>, selfObject: u32) -> vec3<f32> {
    if (gShading.omniOn < 0.5) {
        return vec3<f32>(0.0);
    }
    let origin = p + n * 2e-3;
    let w = gShading.omniPosition - origin;
    let d = length(w);
    let cosS = dot(n, w / d);
    if (cosS <= 0.0 || d >= gShading.omniRange) {
        return vec3<f32>(0.0);
    }
    let visible = shadowVisibility(origin, w / d, d - 1e-3, selfObject);
    return gShading.omniColor * voxelgiOmniAttenuation(d, 1.0 / gShading.omniRange, gShading.omniAttenuation) * cosS * visible;
}

// The scene pass, against the pre-pass's depth: the lab's direct light, the
// normal the composite reads, and the normal-roughness buffer the GI pass reads.
@fragment
fn voxelgiForwardFS(input: SceneVaryings) -> VoxelgiForwardOutput {
    let v = normalize(gFrame.camera.position - input.worldPosition);
    let n = eyeSideNormal(input.worldNormal, input.worldPosition, gFrame.camera.position);
    let o = gObjects[input.objectIndex];
    let irradiance = directIrradiance(input.worldPosition, n, input.objectIndex) + voxelgiOmniIrradiance(input.worldPosition, n, input.objectIndex);
    let direct = o.albedoRoughness.rgb * irradiance / PI + o.emissive.rgb * o.emissive.w;
    let nr = voxelgiNormalRoughness(n, o.albedoRoughness.w, input.objectIndex);
    return VoxelgiForwardOutput(vec4<f32>(direct, f32(input.objectIndex) + 1.0), vec4<f32>(n, 0.0), nr, voxelgiCoverage(input.objectIndex, input.clip.xy));
}

// The chain's targets (SceneShading in src/godot/voxelgi/mod.rs): target i
// read at binding 2 i, written at 2 i + 1; then the depth and the HDR target.
@group(2) @binding(0) var tDirect: texture_multisampled_2d<f32>;
@group(2) @binding(2) var tNormal: texture_multisampled_2d<f32>;
@group(2) @binding(4) var tGiAmbient: texture_2d<f32>;
@group(2) @binding(5) var oGiAmbient: texture_storage_2d<rgba16float, write>;
@group(2) @binding(6) var tGiReflection: texture_2d<f32>;
@group(2) @binding(7) var oGiReflection: texture_storage_2d<rgba16float, write>;
@group(2) @binding(8) var tNormalRoughness: texture_multisampled_2d<f32>;
@group(2) @binding(10) var tDepth: texture_depth_multisampled_2d;
@group(2) @binding(11) var oHdr: texture_storage_2d<rgba16float, write>;

// The world position at a point of the screen (in pixels) and a reversed
// depth: the view-space z the projection maps to that depth, along the ray
// through that point. The GI pass reconstructs a pixel at its top-left
// corner, as gi.glsl's reconstruct_position does (`vec2(screen_pos)`,
// gi.glsl:161-190); the composite, standing for the opaque pass's fragment,
// at its centre.
fn voxelgiWorldPosition(pixel: vec2<f32>, size: vec2<u32>, depth: f32) -> vec3<f32> {
    let c = gFrame.camera;
    let uv = pixel / vec2<f32>(size);
    let z = -c.viewToClip[3].z / (depth + c.viewToClip[2].z);
    let p = vec3<f32>((uv.x * 2.0 - 1.0) / c.viewToClip[0][0], (1.0 - uv.y * 2.0) / c.viewToClip[1][1], -1.0) * -z;
    let r = c.worldToView;
    return c.position + vec3<f32>(dot(r[0].xyz, p), dot(r[1].xyz, p), dot(r[2].xyz, p));
}

// resolve_gi's choice of an MSAA sample (resolve.glsl:53-221, the branch it
// compiles): with 2 samples the nearer; with 4, the sample whose depth the
// fewest samples share (the first on a tie); with 8, the same within each
// half, then, as the code reads, the second half's pick when the first half's
// smallest count is the smaller; else the first sample.
fn voxelgiResolveGiSample(pixel: vec2<u32>) -> u32 {
    let n = textureNumSamples(tDepth);
    if (n == 2u) {
        return select(0u, 1u, textureLoad(tDepth, pixel, 0) < textureLoad(tDepth, pixel, 1));
    }
    if (n != 4u && n != 8u) {
        return 0u;
    }
    var depths: array<f32, 8>;
    for (var i = 0u; i < n; i++) {
        depths[i] = textureLoad(tDepth, pixel, i);
    }
    var best = array<u32, 2>(0u, 4u);
    var least = array<f32, 2>(1e9, 1e9);
    for (var i = 0u; i < n; i++) {
        // How many samples share this one's depth; the rarest of each half of four is kept.
        var freq = 0.0;
        for (var j = 0u; j < n; j++) {
            freq += select(0.0, 1.0, depths[j] == depths[i]);
        }
        let h = i / 4u;
        if (freq < least[h]) {
            least[h] = freq;
            best[h] = i;
        }
    }
    if (n == 8u && least[0] < least[1]) {
        return best[1];
    }
    return best[0];
}

// The GI pass (gi.cpp's process_gi, gi.glsl): once per pixel, on the sample
// resolve_gi picks (Godot resolves its MSAA buffers to one sample first),
// voxel_gi_compute into the ambient and reflection buffers. At half
// resolution (gi.glsl:705-724) the buffers are half the size and each texel
// traces the top-left pixel of its 2 x 2 block (`pos <<= 1`); a block past
// the screen's edge writes nothing.
@compute @workgroup_size(8, 8)
fn voxelgiGiCS(@builtin(global_invocation_id) id: vec3<u32>) {
    let texel = id.xy;
    if (any(texel >= textureDimensions(oGiAmbient))) {
        return;
    }
    var pixel = texel;
    if (gShading.halfResolution > 0.5) {
        pixel = texel << vec2<u32>(1u);
    }
    let size = textureDimensions(tDepth);
    if (any(pixel >= size)) {
        return;
    }
    var gi: VoxelgiResult;
    let s = voxelgiResolveGiSample(pixel);
    let depth = textureLoad(tDepth, pixel, s);
    let direct = textureLoad(tDirect, pixel, s);
    if (depth > 0.0 && direct.a > 0.5) {
        let p = voxelgiWorldPosition(vec2<f32>(pixel), size, depth);
        // fetch_normal_and_roughness and process_gi's decode (gi.glsl:609-627).
        let nr = textureLoad(tNormalRoughness, pixel, s);
        let n = normalize(nr.xyz * 2.0 - 1.0);
        var roughness = nr.w;
        if (roughness > 0.5) {
            roughness = 1.0 - roughness;
        }
        roughness /= 127.0 / 255.0;
        let r = normalize(reflect(-normalize(gFrame.camera.position - p), n));
        gi = voxelgiProcess(p, n, r, roughness, gShading.quality > 0.5, gShading.interior < 0.5);
    }
    textureStore(oGiAmbient, texel, gi.ambient);
    textureStore(oGiReflection, texel, gi.reflection);
}

// textureLod(sampler2D(buffer, SAMPLER_LINEAR_CLAMP), screen_uv, 0.0) of a
// GI buffer (scene_forward_clustered.glsl:2016-2023): bilinear, clamped to the
// edge, at the pixel's centre. At full size it is the pixel's own texel.
fn voxelgiGiBuffer(t: texture_2d<f32>, pixel: vec2<u32>, size: vec2<u32>) -> vec4<f32> {
    let dims = textureDimensions(t);
    if (all(dims == size)) {
        return textureLoad(t, pixel, 0);
    }
    let uv = (vec2<f32>(pixel) + vec2<f32>(0.5)) / vec2<f32>(size);
    let f = uv * vec2<f32>(dims) - vec2<f32>(0.5);
    let b = floor(f);
    let w = f - b;
    let hi = vec2<i32>(dims) - vec2<i32>(1);
    let p0 = clamp(vec2<i32>(b), vec2<i32>(0), hi);
    let p1 = clamp(vec2<i32>(b) + vec2<i32>(1), vec2<i32>(0), hi);
    let top = mix(textureLoad(t, vec2<i32>(p0.x, p0.y), 0), textureLoad(t, vec2<i32>(p1.x, p0.y), 0), w.x);
    let bottom = mix(textureLoad(t, vec2<i32>(p0.x, p1.y), 0), textureLoad(t, vec2<i32>(p1.x, p1.y), 0), w.x);
    return mix(top, bottom, w.y);
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
    let gi = VoxelgiResult(voxelgiGiBuffer(tGiAmbient, pixel, size), voxelgiGiBuffer(tGiReflection, pixel, size));
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
        let p = voxelgiWorldPosition(vec2<f32>(pixel) + vec2<f32>(0.5), size, depth);
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
