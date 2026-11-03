// Godot's SSR passes (SSEffects::screen_space_reflection, ss_effects.cpp:1467-1722),
// ported to WGSL as the lab program's passes (gpu.rs), in the frame's order:
// the half-size downsample of depth and normal-roughness, the Hi-Z pyramid, the
// trace, the roughness filter, and the resolve; then, after the scene pass, the
// copy of the frame that the next frame's trace reads (copy_internal_texture_to_last_frame,
// ss_effects.cpp:396-420). The best-fit normal table the pre-pass reads is made once
// (best_fit_normal.glsl).
//
// Ported from Godot Engine (MIT; Copyright (c) 2014-present Godot Engine
// contributors, Copyright (c) 2007-2014 Juan Linietsky, Ariel Manzur;
// THIRD_PARTY.md). src/godot/ssr/trace.rs mirrors each pass on the CPU.
//
// Each pass binds group 0 (the frame), group 1 (the shading), its command's 16
// words at group 2, and its uses at group 3 (lab/src/program.rs). Where Godot
// compiles a variant (MODE_ODD_WIDTH, MODE_ODD_HEIGHT), the port reads a flag
// from the parameters; the arithmetic is the same.

@group(2) @binding(0) var<uniform> gSsrParams: array<vec4<u32>, 4>;

fn ssrParam(k: u32) -> u32 {
    return gSsrParams[k / 4u][k % 4u];
}

fn ssrParamF(k: u32) -> f32 {
    return bitcast<f32>(ssrParam(k));
}

fn ssrParamSize() -> vec2<i32> {
    return vec2<i32>(i32(ssrParam(0u)), i32(ssrParam(1u)));
}

// ---------------------------------------------------------------------------
// The best-fit normal table (best_fit_normal.glsl, run once at start-up,
// render_forward_clustered.cpp:5160-5198): for direction (x, y, 1), the length
// whose 8-bit quantization errs least.

@group(3) @binding(1) var oBestFit: texture_storage_2d<r8unorm, write>;

fn ssrQuantize(c: vec3<f32>) -> vec3<f32> {
    return round(clamp(c * 0.5 + vec3<f32>(0.5), vec3<f32>(0.0), vec3<f32>(1.0)) * 255.0) * (1.0 / 255.0) * 2.0 - vec3<f32>(1.0);
}

fn ssrFindMinimumError(normal: vec3<f32>) -> f32 {
    var minError = 100000.0;
    var tBest = 0.0;
    for (var nstep = 1.5; nstep < 127.5; nstep += 1.0) {
        let t = nstep / 127.5;
        let vp = normal * t;
        let q = ssrQuantize(vp);
        let d = (q - vp) / t;
        let e = max(abs(d.x), max(abs(d.y), abs(d.z)));
        if (e < minError) {
            minError = e;
            tBest = t;
        }
    }
    return tBest;
}

@compute @workgroup_size(8, 8, 1)
fn ssrBestFitCS(@builtin(global_invocation_id) id: vec3<u32>) {
    var uv = vec2<f32>(id.xy) * (1.0 / 1024.0) + vec2<f32>(0.5 / 1024.0);
    uv.y = uv.y * uv.x;
    textureStore(oBestFit, vec2<i32>(id.xy), vec4<f32>(ssrFindMinimumError(vec3<f32>(uv.x, uv.y, 1.0)), 1.0, 1.0, 1.0));
}

// ---------------------------------------------------------------------------
// "SSR Downsample" (screen_space_reflection_downsample.glsl): each half-size
// texel keeps the nearest (largest, reversed Z) of its 2 x 2 full-size depths
// (3 wide along an odd side) and the normal-roughness texel of that winner.
// Parameters: the half size (0, 1), odd width (2), odd height (3).

@group(3) @binding(1) var dDepth: texture_depth_multisampled_2d;
@group(3) @binding(2) var dNormalRoughness: texture_multisampled_2d<f32>;
@group(3) @binding(3) var oDownDepth: texture_storage_2d<r32float, write>;
@group(3) @binding(4) var oDownNormal: texture_storage_2d<rgba8unorm, write>;

fn ssrDownSample(p: vec2<i32>, depth: ptr<function, f32>, winner: ptr<function, vec2<i32>>) {
    // The lab's depth is multisampled: its first sample (Godot's is resolved to one).
    let d = textureLoad(dDepth, p, 0);
    if (*depth < d) {
        *depth = d;
        *winner = p;
    }
}

@compute @workgroup_size(8, 8, 1)
fn ssrDownsampleCS(@builtin(global_invocation_id) id: vec3<u32>) {
    let pixel = vec2<i32>(id.xy);
    if (any(pixel >= ssrParamSize())) {
        return;
    }
    var winner = pixel * 2;
    var depth = textureLoad(dDepth, winner, 0);
    ssrDownSample(pixel * 2 + vec2<i32>(1, 0), &depth, &winner);
    ssrDownSample(pixel * 2 + vec2<i32>(0, 1), &depth, &winner);
    ssrDownSample(pixel * 2 + vec2<i32>(1, 1), &depth, &winner);
    let oddW = ssrParam(2u) != 0u;
    let oddH = ssrParam(3u) != 0u;
    if (oddW) {
        ssrDownSample(pixel * 2 + vec2<i32>(2, 0), &depth, &winner);
        ssrDownSample(pixel * 2 + vec2<i32>(2, 1), &depth, &winner);
    }
    if (oddH) {
        ssrDownSample(pixel * 2 + vec2<i32>(0, 2), &depth, &winner);
        ssrDownSample(pixel * 2 + vec2<i32>(1, 2), &depth, &winner);
    }
    if (oddW && oddH) {
        ssrDownSample(pixel * 2 + vec2<i32>(2, 2), &depth, &winner);
    }
    textureStore(oDownDepth, pixel, vec4<f32>(depth, 0.0, 0.0, 0.0));
    textureStore(oDownNormal, pixel, textureLoad(dNormalRoughness, winner, 0));
}

// ---------------------------------------------------------------------------
// "SSR HI-Z" (screen_space_reflection_hiz.glsl): level m from level m - 1, the
// max (nearest) of 2 x 2 (3 wide along an odd side of the parent).
// Parameters: level m's size (0, 1), the parent's odd width (2) and height (3).

@group(3) @binding(1) var hSource: texture_2d<f32>;
@group(3) @binding(2) var hDest: texture_storage_2d<r32float, write>;

@compute @workgroup_size(8, 8, 1)
fn ssrHizCS(@builtin(global_invocation_id) id: vec3<u32>) {
    let pixel = vec2<i32>(id.xy);
    if (any(pixel >= ssrParamSize())) {
        return;
    }
    let p = pixel * 2;
    var depth = textureLoad(hSource, p, 0).x;
    depth = max(depth, textureLoad(hSource, p + vec2<i32>(1, 0), 0).x);
    depth = max(depth, textureLoad(hSource, p + vec2<i32>(0, 1), 0).x);
    depth = max(depth, textureLoad(hSource, p + vec2<i32>(1, 1), 0).x);
    let oddW = ssrParam(2u) != 0u;
    let oddH = ssrParam(3u) != 0u;
    if (oddW) {
        depth = max(depth, textureLoad(hSource, p + vec2<i32>(2, 0), 0).x);
        depth = max(depth, textureLoad(hSource, p + vec2<i32>(2, 1), 0).x);
    }
    if (oddH) {
        depth = max(depth, textureLoad(hSource, p + vec2<i32>(0, 2), 0).x);
        depth = max(depth, textureLoad(hSource, p + vec2<i32>(1, 2), 0).x);
    }
    if (oddW && oddH) {
        depth = max(depth, textureLoad(hSource, p + vec2<i32>(2, 2), 0).x);
    }
    textureStore(hDest, pixel, vec4<f32>(depth, 0.0, 0.0, 0.0));
}

// ---------------------------------------------------------------------------
// "SSR Main" (screen_space_reflection.glsl): the trace, its validation, the
// colour from last frame, and the blur level.
// Parameters (ScreenSpaceReflectionPushConstant, ss_effects.cpp:1620-1630): the
// SSR size (0, 1), mipmaps (2), num_steps (3), distance_fade (4), curve_fade_in
// (5), depth_tolerance (6), orthogonal (7), view_index (8).

// The SSR UBO (ScreenSpaceReflectionSceneData, ss_effects.cpp:1469-1498), one view.
struct SsrSceneData {
    projection: mat4x4<f32>,
    invProjection: mat4x4<f32>,
    reprojection: mat4x4<f32>,
    eyeOffset: vec4<f32>,
}

@group(3) @binding(1) var tLastFrame: texture_2d<f32>;
@group(3) @binding(2) var tHiz: texture_2d<f32>;
@group(3) @binding(3) var tNormalRoughness: texture_2d<f32>;
@group(3) @binding(4) var oColor: texture_storage_2d<rgba16float, write>;
@group(3) @binding(5) var oMipLevel: texture_storage_2d<r8unorm, write>;
@group(3) @binding(6) var<uniform> uScene: SsrSceneData;
@group(3) @binding(7) var sLinear: sampler;

const SSR_PI: f32 = 3.14159265359;

struct SsrTraceParams {
    screenSize: vec2<i32>,
    mipmaps: i32,
    numSteps: i32,
    distanceFade: f32,
    curveFadeIn: f32,
    depthTolerance: f32,
    orthogonal: bool,
}

fn ssrTraceParams() -> SsrTraceParams {
    return SsrTraceParams(ssrParamSize(), i32(ssrParam(2u)), i32(ssrParam(3u)), ssrParamF(4u), ssrParamF(5u), ssrParamF(6u), ssrParam(7u) != 0u);
}

fn ssrCellCount(size: vec2<i32>, level: i32) -> vec2<f32> {
    return vec2<f32>(f32(max(1, size.x >> u32(level))), f32(max(1, size.y >> u32(level))));
}

fn ssrLinearizeDepth(depth: f32) -> f32 {
    let pos = uScene.invProjection * vec4<f32>(0.0, 0.0, depth, 1.0);
    return pos.z / pos.w;
}

fn ssrViewPos(screenPos: vec3<f32>) -> vec3<f32> {
    let pos = uScene.invProjection * vec4<f32>(screenPos.xy * 2.0 - vec2<f32>(1.0), screenPos.z, 1.0);
    return pos.xyz / pos.w;
}

fn ssrScreenPos(pos: vec3<f32>) -> vec3<f32> {
    var s = uScene.projection * vec4<f32>(pos, 1.0);
    let p = s.xyz / s.w;
    return vec3<f32>(p.xy * 0.5 + vec2<f32>(0.5), p.z);
}

fn ssrHizAt(p: vec2<i32>, level: i32) -> f32 {
    return textureLoad(tHiz, p, level).x;
}

// compute_geometric_normal (after Snailwithtea, "Как я нормали реконструировал",
// Habr 2023, cited at screen_space_reflection.glsl:66): on each axis the side whose
// two depths extrapolate best to the centre gives the tangent.
fn ssrGeometricNormal(size: vec2<i32>, pixel: vec2<i32>, depthC: f32, viewC: vec3<f32>, pixelOffset: f32) -> vec3<f32> {
    let H = vec4<f32>(ssrHizAt(pixel + vec2<i32>(-1, 0), 0), ssrHizAt(pixel + vec2<i32>(-2, 0), 0), ssrHizAt(pixel + vec2<i32>(1, 0), 0), ssrHizAt(pixel + vec2<i32>(2, 0), 0));
    let V = vec4<f32>(ssrHizAt(pixel + vec2<i32>(0, -1), 0), ssrHizAt(pixel + vec2<i32>(0, -2), 0), ssrHizAt(pixel + vec2<i32>(0, 1), 0), ssrHizAt(pixel + vec2<i32>(0, 2), 0));
    let he = abs((2.0 * H.xz - H.yw) - vec2<f32>(depthC));
    let ve = abs((2.0 * V.xz - V.yw) - vec2<f32>(depthC));
    let hSign = select(1, -1, he.x < he.y);
    let vSign = select(1, -1, ve.x < ve.y);
    let fs = vec2<f32>(size);
    let viewH = ssrViewPos(vec3<f32>((vec2<f32>(pixel) + vec2<f32>(f32(hSign), 0.0) + vec2<f32>(pixelOffset)) / fs, H[1 + hSign]));
    let viewV = ssrViewPos(vec3<f32>((vec2<f32>(pixel) + vec2<f32>(0.0, f32(vSign)) + vec2<f32>(pixelOffset)) / fs, V[1 + vSign]));
    let hDer = f32(hSign) * (viewH - viewC);
    let vDer = f32(vSign) * (viewV - viewC);
    return cross(vDer, hDer);
}

// The roughness of a normal-roughness texel: the dynamic flag folded away, the 127/255 undone.
fn ssrDecodeRoughness(w: f32) -> f32 {
    var r = w;
    if (r > 0.5) {
        r = 1.0 - r;
    }
    return r / (127.0 / 255.0);
}

@compute @workgroup_size(8, 8, 1)
fn ssrTraceCS(@builtin(global_invocation_id) id: vec3<u32>) {
    let params = ssrTraceParams();
    let pixel = vec2<i32>(id.xy);
    if (any(pixel >= params.screenSize)) {
        return;
    }
    let fs = vec2<f32>(params.screenSize);
    var color = vec4<f32>(0.0);
    var mipLevel = 0.0;
    var screenPos = vec3<f32>((vec2<f32>(pixel) + vec2<f32>(0.5)) / fs, ssrHizAt(pixel, 0));
    let shouldTrace = screenPos.z != 0.0;
    if (shouldTrace) {
        var pos = ssrViewPos(screenPos);
        let nr = textureLoad(tNormalRoughness, pixel, 0);
        let normal = normalize(nr.xyz * 2.0 - vec3<f32>(1.0));
        let roughness = ssrDecodeRoughness(nr.w);
        // Do not compute SSR for rough materials (screen_space_reflection.glsl:123-129).
        if (roughness >= 0.7) {
            textureStore(oColor, pixel, vec4<f32>(0.0));
            textureStore(oMipLevel, pixel, vec4<f32>(0.0));
            return;
        }
        let geomNormal = normalize(ssrGeometricNormal(params.screenSize, pixel, screenPos.z, pos, 0.5));
        // A small bias towards the geometry normal, against self intersections.
        pos += geomNormal * (1.0 - pow(clamp(dot(normal, geomNormal), 0.0, 1.0), 8.0));
        screenPos = ssrScreenPos(pos);
        var viewDir = normalize(pos + uScene.eyeOffset.xyz);
        if (params.orthogonal) {
            viewDir = vec3<f32>(0.0, 0.0, -1.0);
        }
        var rayDir = normalize(reflect(viewDir, normal));
        // The ray immediately intersecting with itself: bounce.
        if (dot(rayDir, geomNormal) < 0.0) {
            rayDir = normalize(reflect(rayDir, geomNormal));
        }
        var endPos = pos + rayDir;
        // "Clip to near plane": to the camera's plane, z = -0.00001.
        if (endPos.z > 0.0) {
            endPos -= rayDir / rayDir.z * (endPos.z + 0.00001);
        }
        let screenEndPos = ssrScreenPos(endPos);
        // Z normalized to -1 or +1: parametric t tracing (Supnik, "A Tip for HiZ
        // SSR - Parametric 't' Tracing", Hacks of Life, 2020).
        var rayScreen = screenEndPos - screenPos;
        rayScreen = rayScreen / abs(rayScreen.z);
        let facingCamera = rayScreen.z >= 0.0;
        // Where the ray leaves the screen.
        let t0 = (vec2<f32>(0.0) - screenPos.xy) / rayScreen.xy;
        let t1 = (vec2<f32>(1.0) - screenPos.xy) / rayScreen.xy;
        let t2 = max(t0, t1);
        let tMax = min(t2.x, t2.y);
        let cellStep = vec2<f32>(select(1.0, -1.0, rayScreen.x < 0.0), select(1.0, -1.0, rayScreen.y < 0.0));
        var curLevel = 0;
        var curIteration = params.numSteps;
        // The start, advanced to the next cell (against immediate self intersection).
        var t: f32;
        {
            let cellIndex = floor(screenPos.xy * fs);
            let newCellIndex = cellIndex + clamp(cellStep, vec2<f32>(0.0), vec2<f32>(1.0));
            let newCellPos = newCellIndex / fs + cellStep * 0.000001;
            let posT = (newCellPos - screenPos.xy) / rayScreen.xy;
            t = min(posT.x, posT.y);
        }
        while (curLevel >= 0 && curIteration > 0 && t < tMax) {
            let curScreenPos = screenPos + rayScreen * t;
            let cellCount = ssrCellCount(params.screenSize, curLevel);
            let cellIndex = floor(curScreenPos.xy * cellCount);
            let cellDepth = ssrHizAt(vec2<i32>(cellIndex), curLevel);
            // Z is -1 or +1, so no divide.
            let depthT = (cellDepth - screenPos.z) * rayScreen.z;
            let newCellIndex = cellIndex + clamp(cellStep, vec2<f32>(0.0), vec2<f32>(1.0));
            let newCellPos = newCellIndex / cellCount + cellStep * 0.000001;
            let posT = (newCellPos - screenPos.xy) / rayScreen.xy;
            let edgeT = min(posT.x, posT.y);
            var hit = select(depthT <= edgeT, t <= depthT, facingCamera);
            var mipOffset = select(1, -1, hit);
            if (curLevel == 0) {
                let z0 = ssrLinearizeDepth(cellDepth);
                let z1 = ssrLinearizeDepth(curScreenPos.z);
                if ((z0 - z1) > params.depthTolerance) {
                    hit = false;
                    // Keep the level, against going down and up in repeat.
                    mipOffset = 0;
                }
            }
            if (hit) {
                if (!facingCamera) {
                    t = max(t, depthT);
                }
            } else {
                t = edgeT;
            }
            curLevel = min(curLevel + mipOffset, params.mipmaps - 1);
            curIteration -= 1;
        }
        let curScreenPos = screenPos + rayScreen * t;
        var reprojected = uScene.reprojection * vec4<f32>(curScreenPos.xy * 2.0 - vec2<f32>(1.0), curScreenPos.z, 1.0);
        let reprojectedUv = reprojected.xy / reprojected.w * 0.5 + vec2<f32>(0.5);
        // Validity instead of a hard rejection (hit validation after FidelityFX SSSR).
        var validity = 1.0;
        let curPixel = vec2<i32>(curScreenPos.xy * fs);
        let hitDepth = ssrHizAt(curPixel, 0);
        if (t >= tMax || hitDepth == 0.0) {
            validity = 0.0;
        }
        if (all(abs(rayScreen.xy * t) < vec2<f32>(2.0) / fs)) {
            let hitNormal = textureLoad(tNormalRoughness, curPixel, 0).xyz * 2.0 - vec3<f32>(1.0);
            if (dot(rayDir, hitNormal) >= 0.0) {
                validity = 0.0;
            }
        }
        let curPos = ssrViewPos(curScreenPos);
        let hitPos = ssrViewPos(vec3<f32>(curScreenPos.xy, hitDepth));
        let delta = length(curPos - hitPos);
        let confidence = 1.0 - smoothstep(0.0, params.depthTolerance, delta);
        validity *= clamp(confidence * confidence, 0.0, 1.0);
        let reprojectedPixel = reprojectedUv * fs;
        let margin = vec2<f32>((fs.x + fs.y) * 0.05);
        let marginGrad = select(fs - reprojectedPixel, reprojectedPixel, reprojectedPixel < fs * 0.5);
        let marginBlend = smoothstep(0.0, margin.x * margin.y, marginGrad.x * marginGrad.y);
        let rayLen = length(rayScreen.xy * t);
        let fadeIn = select(pow(clamp(rayLen, 0.0, 1.0), params.curveFadeIn), 1.0, params.curveFadeIn == 0.0);
        let fadeOut = select(pow(clamp(1.0 - rayLen, 0.0, 1.0), params.distanceFade), 1.0, params.distanceFade == 0.0);
        var fade = fadeIn * fadeOut;
        if (fade > 0.999) {
            fade = 1.0;
        }
        validity *= fade * marginBlend;
        if (validity > 0.0) {
            color = vec4<f32>(textureSampleLevel(tLastFrame, sLinear, reprojectedUv, 0.0).xyz, 1.0) * validity;
            // Compressed for a smoother filter across samples of different luminance.
            color = vec4<f32>(color.rgb / (1.0 + dot(color.rgb, vec3<f32>(0.2126, 0.7152, 0.0722))), color.a);
        }
        if (roughness > 0.001) {
            let coneAngle = min(roughness, 0.999) * SSR_PI * 0.5;
            let coneLen = rayLen;
            let opLen = 2.0 * tan(coneAngle) * coneLen;
            // The sphere inside the cone, ending at its end.
            let a = opLen;
            let h = coneLen;
            let blurRadius = (a * (sqrt(a * a + 4.0 * h * h) - a)) / (4.0 * h);
            mipLevel = clamp(log2(blurRadius * max(fs.x, fs.y) / 16.0 + 1.0), 0.0, f32(params.mipmaps - 1));
        }
        mipLevel *= pow(clamp(1.25 - rayLen, 0.0, 1.0), 0.2);
    }
    textureStore(oColor, pixel, color);
    textureStore(oMipLevel, pixel, vec4<f32>(mipLevel / 14.0, 0.0, 0.0, 0.0));
}

// ---------------------------------------------------------------------------
// "SSR Roughness Filter" (screen_space_reflection_filter.glsl): level m from m - 1,
// a 2 x 2 average (bilinear at the corner of four parent texels) then a 7-tap
// Gaussian across and down in workgroup memory; invalid texels weigh 0.2 a level.
// Parameters: level m's size (0, 1), m (2).

@group(3) @binding(1) var fSource: texture_2d<f32>;
@group(3) @binding(2) var fDest: texture_storage_2d<rgba16float, write>;
@group(3) @binding(3) var fLinear: sampler;

const SSR_WEIGHTS = array<f32, 7>(0.07130343198685299, 0.1315141208431224, 0.18987923288883812, 0.21460642856237303, 0.18987923288883812, 0.1315141208431224, 0.07130343198685299);

var<workgroup> fCache: array<array<vec4<f32>, 16>, 16>;
var<workgroup> fTemp: array<array<vec4<f32>, 16>, 8>;

fn ssrFilterWeight(c: vec4<f32>) -> f32 {
    return mix(clamp(f32(ssrParam(2u)) * 0.2, 0.0, 1.0), 1.0, c.a);
}

fn ssrFilterSample(p: vec2<i32>) -> vec4<f32> {
    return textureSampleLevel(fSource, fLinear, (vec2<f32>(p) + vec2<f32>(0.5)) / vec2<f32>(ssrParamSize()), 0.0);
}

@compute @workgroup_size(8, 8, 1)
fn ssrFilterCS(@builtin(global_invocation_id) gid: vec3<u32>, @builtin(local_invocation_id) lid: vec3<u32>) {
    let g = vec2<i32>(gid.xy);
    let l = vec2<i32>(lid.xy);
    fCache[l.x * 2 + 0][l.y * 2 + 0] = ssrFilterSample(g + l - vec2<i32>(4) + vec2<i32>(0, 0));
    fCache[l.x * 2 + 1][l.y * 2 + 0] = ssrFilterSample(g + l - vec2<i32>(4) + vec2<i32>(1, 0));
    fCache[l.x * 2 + 0][l.y * 2 + 1] = ssrFilterSample(g + l - vec2<i32>(4) + vec2<i32>(0, 1));
    fCache[l.x * 2 + 1][l.y * 2 + 1] = ssrFilterSample(g + l - vec2<i32>(4) + vec2<i32>(1, 1));
    workgroupBarrier();
    for (var row = 0; row < 2; row += 1) {
        let at = vec2<i32>(l.x + 4, l.y * 2 + row);
        var c = vec4<f32>(0.0);
        var w = 0.0;
        for (var k = 0; k < 7; k += 1) {
            let s = fCache[at.x + k - 3][at.y];
            let wk = SSR_WEIGHTS[k] * ssrFilterWeight(s);
            c += s * wk;
            w += wk;
        }
        fTemp[l.x][l.y * 2 + row] = select(vec4<f32>(0.0), c / w, w > 0.0);
    }
    workgroupBarrier();
    if (any(g >= ssrParamSize())) {
        return;
    }
    let at = l + vec2<i32>(0, 4);
    var c = vec4<f32>(0.0);
    var w = 0.0;
    for (var k = 0; k < 7; k += 1) {
        let s = fTemp[at.x][at.y + k - 3];
        let wk = SSR_WEIGHTS[k] * ssrFilterWeight(s);
        c += s * wk;
        w += wk;
    }
    textureStore(fDest, g, select(vec4<f32>(0.0), c / w, w > 0.0));
}

// ---------------------------------------------------------------------------
// "SSR Resolve" (screen_space_reflection_resolve.glsl): back to the internal size,
// each of the four half-size taps at its own blur level, un-compressed, weighted
// by depth, normal, and roughness similarity and by bilinear weights.
// Parameters: the internal size (0, 1).

@group(3) @binding(1) var rDepth: texture_depth_multisampled_2d;
@group(3) @binding(2) var rNormalRoughness: texture_multisampled_2d<f32>;
@group(3) @binding(3) var rDepthHalf: texture_2d<f32>;
@group(3) @binding(4) var rNormalHalf: texture_2d<f32>;
@group(3) @binding(5) var rColor: texture_2d<f32>;
@group(3) @binding(6) var rMipLevel: texture_2d<f32>;
@group(3) @binding(7) var oFinal: texture_storage_2d<rgba16float, write>;
@group(3) @binding(8) var rTrilinear: sampler;

struct SsrTap {
    color: vec4<f32>,
    weight: f32,
}

fn ssrResolveTap(depth: f32, normal: vec3<f32>, roughness: f32, p: vec2<i32>) -> SsrTap {
    let sampleDepth = textureLoad(rDepthHalf, p, 0).x;
    let snr = textureLoad(rNormalHalf, p, 0);
    let sampleNormal = normalize(snr.xyz * 2.0 - vec3<f32>(1.0));
    let sampleRoughness = ssrDecodeRoughness(snr.w);
    let uv = (vec2<f32>(p) + vec2<f32>(0.5)) / (vec2<f32>(ssrParamSize()) * 0.5);
    let mip = textureLoad(rMipLevel, p, 0).x * 14.0;
    var color = textureSampleLevel(rColor, rTrilinear, uv, mip);
    // The trace pass's tone compression, undone.
    color = vec4<f32>(color.rgb / (1.0 - dot(color.rgb, vec3<f32>(0.2126, 0.7152, 0.0722))), color.a);
    let wDepth = exp(-abs(depth - sampleDepth) * 2048.0);
    let wNormal = exp(-clamp(1.0 - dot(normal, sampleNormal), 0.0, 1.0) * 32.0);
    let wRoughness = exp(-abs(roughness - sampleRoughness) * 16.0);
    return SsrTap(color, wDepth * wNormal * wRoughness);
}

@compute @workgroup_size(8, 8, 1)
fn ssrResolveCS(@builtin(global_invocation_id) id: vec3<u32>) {
    let pixel = vec2<i32>(id.xy);
    if (any(pixel >= ssrParamSize())) {
        return;
    }
    let depth = textureLoad(rDepth, pixel, 0);
    let nr = textureLoad(rNormalRoughness, pixel, 0);
    let normal = normalize(nr.xyz * 2.0 - vec3<f32>(1.0));
    let roughness = ssrDecodeRoughness(nr.w);
    let bilinear = fract((vec2<f32>(pixel) + vec2<f32>(0.5)) * 0.5);
    let base = (pixel - vec2<i32>(1)) / 2;
    let s0 = ssrResolveTap(depth, normal, roughness, base + vec2<i32>(0, 0));
    let s1 = ssrResolveTap(depth, normal, roughness, base + vec2<i32>(1, 0));
    let s2 = ssrResolveTap(depth, normal, roughness, base + vec2<i32>(0, 1));
    let s3 = ssrResolveTap(depth, normal, roughness, base + vec2<i32>(1, 1));
    let w0 = s0.weight * bilinear.x * bilinear.y;
    let w1 = s1.weight * (1.0 - bilinear.x) * bilinear.y;
    let w2 = s2.weight * bilinear.x * (1.0 - bilinear.y);
    let w3 = s3.weight * (1.0 - bilinear.x) * (1.0 - bilinear.y);
    let weight = w0 + w1 + w2 + w3;
    var result = vec4<f32>(0.0);
    if (weight > 0.0) {
        result = (s0.color * w0 + s1.color * w1 + s2.color * w2 + s3.color * w3) / weight;
    }
    textureStore(oFinal, pixel, result);
}

// ---------------------------------------------------------------------------
// "Copy Framebuffer for SSIL/SSR" (copy_internal_texture_to_last_frame,
// ss_effects.cpp:396-420; make_mipmap, copy.glsl:82-94): after the scene pass,
// this frame's HDR colour into the half-size last frame, a 2 x 2 box average,
// NaN and Inf replaced by 100. Parameters: the half size (0, 1).

@group(3) @binding(1) var cHdr: texture_2d<f32>;
@group(3) @binding(2) var oLastFrame: texture_storage_2d<rgba16float, write>;

fn ssrSanitize(c: vec4<f32>) -> vec4<f32> {
    let bad = any(c != c) || any(abs(c) > vec4<f32>(3.0e38));
    return select(c, vec4<f32>(100.0, 100.0, 100.0, 1.0), bad);
}

@compute @workgroup_size(8, 8, 1)
fn ssrCopyLastFrameCS(@builtin(global_invocation_id) id: vec3<u32>) {
    let pixel = vec2<i32>(id.xy);
    if (any(pixel >= ssrParamSize())) {
        return;
    }
    let p = pixel * 2;
    var c = ssrSanitize(textureLoad(cHdr, p, 0));
    c += ssrSanitize(textureLoad(cHdr, p + vec2<i32>(1, 0), 0));
    c += ssrSanitize(textureLoad(cHdr, p + vec2<i32>(0, 1), 0));
    c += ssrSanitize(textureLoad(cHdr, p + vec2<i32>(1, 1), 0));
    textureStore(oLastFrame, pixel, c * 0.25);
}
