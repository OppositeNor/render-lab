// Godot's SDFGI probe passes on the GPU (sdfgi_integrate.glsl; gi.cpp
// update_probes, store_probes, and the scroll of render_region), as the lab
// program's passes (src/godot/sdfgi/gpu.rs): MODE_PROCESS
// (sdfgiProbeProcessCS), MODE_STORE (sdfgiProbeStoreCS), MODE_SCROLL
// (sdfgiProbeScrollCS), and MODE_SCROLL_STORE (sdfgiProbeScrollStoreCS), all
// 8 x 8 x 1 as the engine's (sdfgi_integrate.glsl:9). The push constant's
// fields are the command's parameters (sdfgi_gpu_common.wgsl): [0]
// max_cascades, [1] cascade, [2] history_index, [3] history_size, [4]
// ray_count, [5] ray_bias, [6..7] image_size, [8..10] world_offset, [11] sky
// flags, [12..14] scroll, [15] y_mult; grid_size (128) and probe_axis_size
// (17) are constants. probes.rs is the CPU mirror.
//
// Where the lab differs: the voxel light (R32_UINT words) is decoded and
// filtered here, eight words per sample, where the engine samples its
// E5B9G9R9 view with the linear sampler; a ray that misses reads the lab's
// sky (`environmentRadiance`, as the CPU mirror does) where the engine reads
// the sky's radiance map at mip 2 (in 4.7.2 through the non-array
// `texture2D` path, research.md Finding 25), so sky_flags is 1 (sky) or 0
// (black); the ambient texture for volumetric fog (store_ambient_texture) is
// not written (the lab has no fog).
//
// Ported from Godot Engine (MIT; Copyright (c) 2014-present Godot Engine
// contributors, Copyright (c) 2007-2014 Juan Linietsky, Ariel Manzur;
// THIRD_PARTY.md).

const SDFGI_SH_SIZE: u32 = 16u;

// ---------------------------------------------------------------------------
// MODE_PROCESS (sdfgi_integrate.glsl:164-348).

@group(3) @binding(1) var gProbeProcessSdf: binding_array<texture_3d<f32>, 4>;
@group(3) @binding(2) var gProbeProcessLight: binding_array<texture_3d<u32>, 4>;
@group(3) @binding(3) var gProbeProcessAniso0: binding_array<texture_3d<f32>, 4>;
@group(3) @binding(4) var gProbeProcessAniso1: binding_array<texture_3d<f32>, 4>;
@group(3) @binding(6) var gProbeProcessSampler: sampler;
@group(3) @binding(7) var<storage, read> gProbeProcessCascades: SdfgiCascades;
@group(3) @binding(9) var gProbeProcessHistory: texture_storage_2d_array<rgba16sint, read_write>;
@group(3) @binding(10) var gProbeProcessAverage: texture_storage_2d<rgba32sint, read_write>;

// `shared SH sh_accum[64]` (sdfgi_integrate.glsl:146-154): 48 floats per
// thread of the 8 x 8 group, each thread its own slice.
var<workgroup> gProbeShAccum: array<array<f32, 48>, 64>;

// texture(sampler3D(light_cascades[j], linear_sampler), uvw).rgb: the
// E5B9G9R9 words decoded, then filtered trilinearly, clamped to the edge.
fn sdfgiProbeLight(j: u32, uvw: vec3<f32>) -> vec3<f32> {
    let t = uvw * f32(SDFGI_CASCADE_SIZE) - vec3<f32>(0.5);
    let b = floor(t);
    let f = t - b;
    let bi = vec3<i32>(b);
    var sum = vec3<f32>(0.0);
    for (var i = 0; i < 8; i++) {
        let o = vec3<i32>(i & 1, (i >> 1u) & 1, (i >> 2u) & 1);
        let w = select(vec3<f32>(1.0) - f, f, o == vec3<i32>(1));
        let c = clamp(bi + o, vec3<i32>(0), vec3<i32>(SDFGI_CASCADE_SIZE - 1));
        sum += w.x * w.y * w.z * sdfgiRgb9e5(textureLoad(gProbeProcessLight[j], c, 0).r);
    }
    return sum;
}

fn sdfgiProbeSdf(j: u32, uvw: vec3<f32>) -> f32 {
    return textureSampleLevel(gProbeProcessSdf[j], gProbeProcessSampler, uvw, 0.0).r;
}

@compute @workgroup_size(8, 8, 1)
fn sdfgiProbeProcessCS(@builtin(global_invocation_id) gid: vec3<u32>, @builtin(local_invocation_id) lid: vec3<u32>) {
    let maxCascades = sdfgiParam(0u);
    let cascade = sdfgiParam(1u);
    let historyIndex = sdfgiParam(2u);
    let historySize = sdfgiParam(3u);
    let rayCount = sdfgiParam(4u);
    let rayBias = sdfgiParamF(5u);
    let imageSize = vec2<i32>(sdfgiParamI(6u), sdfgiParamI(7u));
    let worldOffset = sdfgiParamI3(8u);
    let skyFlags = sdfgiParam(11u);
    let yMult = sdfgiParamF(15u);
    let gridSize = vec3<f32>(f32(SDFGI_CASCADE_SIZE));

    let pos = vec2<i32>(gid.xy);
    if (any(pos >= imageSize)) { // sdfgi_integrate.glsl:158
        return;
    }
    let probeIndex = lid.x + lid.y * 8u; // :162

    // :166-174
    let probeCellSize = f32(gridSize.x / f32(SDFGI_PROBE_AXIS - 1)) / gProbeProcessCascades.data[cascade].toCell;
    var probeCell: vec3<i32>;
    probeCell.x = pos.x % SDFGI_PROBE_AXIS;
    probeCell.y = pos.y;
    probeCell.z = pos.x / SDFGI_PROBE_AXIS;
    let probePos = gProbeProcessCascades.data[cascade].offset + vec3<f32>(probeCell) * probeCellSize;
    let posToUvw = vec3<f32>(1.0) / gridSize;

    for (var i = 0u; i < SDFGI_SH_SIZE * 3u; i++) { // :176-178
        gProbeShAccum[probeIndex][i] = 0.0;
    }

    // :181-187
    let h3 = sdfgiHash3(vec3<u32>(worldOffset + probeCell));
    let offset = sdfgiHashf3(vec3<f32>(h3 & vec3<u32>(0xFFFFFu)));
    let rayOffset = historyIndex;
    let rayMult = historySize;
    let rayTotal = rayMult * rayCount;

    for (var i = 0u; i < rayCount; i++) { // :189
        var rayDir = sdfgiVogel(rayOffset + i * rayMult, rayTotal, offset);
        rayDir.y *= yMult;
        rayDir = normalize(rayDir);

        var rayPos = probePos; // :195
        let invDir = vec3<f32>(1.0) / rayDir;
        var hit = false;
        var hitCascade = 0u;
        let bias = rayBias;
        let absRayDir = abs(rayDir);
        rayPos += rayDir * 1.0 / max(absRayDir.x, max(absRayDir.y, absRayDir.z)) * bias / gProbeProcessCascades.data[cascade].toCell; // :203
        var uvw = vec3<f32>(0.0);

        for (var j = cascade; j < maxCascades; j++) { // :206
            var p = rayPos - gProbeProcessCascades.data[j].offset;
            p *= gProbeProcessCascades.data[j].toCell;
            if (any(p < vec3<f32>(0.0)) || any(p >= gridSize)) {
                continue; // :212
            }
            // :216-219
            let t0 = -p * invDir;
            let t1 = (gridSize - p) * invDir;
            let tmax = max(t0, t1);
            let maxAdvance = min(tmax.x, min(tmax.y, tmax.z));
            var advance = 0.0;
            while (advance < maxAdvance) { // :223
                uvw = (p + rayDir * advance) * posToUvw;
                let distance = sdfgiProbeSdf(j, uvw) * 255.0 - 1.0;
                if (distance < 0.05) {
                    hit = true;
                    break;
                }
                advance += distance;
            }
            if (hit) {
                hitCascade = j;
                break;
            }
            // :243-246
            p += rayDir * maxAdvance;
            p /= gProbeProcessCascades.data[j].toCell;
            p += gProbeProcessCascades.data[j].offset;
            rayPos = p;
        }

        var light = vec4<f32>(0.0);
        if (hit) {
            // :252 "avoid reading different texture from different threads"
            for (var j = cascade; j < maxCascades; j++) {
                if (j == hitCascade) {
                    let EPSILON = 0.001;
                    let hitNormal = normalize(vec3<f32>(
                        sdfgiProbeSdf(hitCascade, uvw + vec3<f32>(EPSILON, 0.0, 0.0)) - sdfgiProbeSdf(hitCascade, uvw - vec3<f32>(EPSILON, 0.0, 0.0)),
                        sdfgiProbeSdf(hitCascade, uvw + vec3<f32>(0.0, EPSILON, 0.0)) - sdfgiProbeSdf(hitCascade, uvw - vec3<f32>(0.0, EPSILON, 0.0)),
                        sdfgiProbeSdf(hitCascade, uvw + vec3<f32>(0.0, 0.0, EPSILON)) - sdfgiProbeSdf(hitCascade, uvw - vec3<f32>(0.0, 0.0, EPSILON))));
                    let hitLight = sdfgiProbeLight(hitCascade, uvw); // :260
                    let aniso0 = textureSampleLevel(gProbeProcessAniso0[hitCascade], gProbeProcessSampler, uvw, 0.0);
                    let hitAniso0 = aniso0.rgb;
                    let hitAniso1 = vec3<f32>(aniso0.a, textureSampleLevel(gProbeProcessAniso1[hitCascade], gProbeProcessSampler, uvw, 0.0).rg);
                    // :266 "one liner magic"
                    light = vec4<f32>(hitLight * (dot(max(vec3<f32>(0.0), hitNormal * hitAniso0), vec3<f32>(1.0)) + dot(max(vec3<f32>(0.0), -hitNormal * hitAniso1), vec3<f32>(1.0))), 1.0);
                }
            }
        } else if ((skyFlags & 1u) != 0u) {
            // :271-283, the lab's sky in place of the radiance map (see the header).
            light = vec4<f32>(environmentRadiance(rayDir), 0.0);
        } else {
            light = vec4<f32>(0.0); // :290
        }

        // :295-319, SH_ACCUM over the 16 coefficients.
        for (var k = 0u; k < SDFGI_SH_SIZE; k++) {
            let l = light.rgb * sdfgiShBasis(rayDir, k);
            gProbeShAccum[probeIndex][k * 3u + 0u] += l.r;
            gProbeShAccum[probeIndex][k * 3u + 1u] += l.g;
            gProbeShAccum[probeIndex][k * 3u + 2u] += l.b;
        }
    }

    for (var i = 0u; i < SDFGI_SH_SIZE; i++) { // :323-347
        let prevPos = vec3<i32>(pos.x, pos.y * i32(SDFGI_SH_SIZE) + i32(i), i32(historyIndex));
        let averagePos = prevPos.xy;
        let value = vec4<f32>(gProbeShAccum[probeIndex][i * 3u + 0u], gProbeShAccum[probeIndex][i * 3u + 1u], gProbeShAccum[probeIndex][i * 3u + 2u], 1.0) * 4.0 / f32(rayCount);
        // :330 ivec4() truncates toward zero; clamped to 16 bits.
        let ivalue = clamp(vec4<i32>(value * f32(1 << u32(SDFGI_HISTORY_BITS))), vec4<i32>(-32768), vec4<i32>(32767));
        let prevValue = textureLoad(gProbeProcessHistory, prevPos.xy, prevPos.z);
        var average = textureLoad(gProbeProcessAverage, averagePos);
        average -= prevValue;
        average += ivalue;
        textureStore(gProbeProcessHistory, prevPos.xy, prevPos.z, ivalue);
        textureStore(gProbeProcessAverage, averagePos, average);
    }
}

// ---------------------------------------------------------------------------
// MODE_STORE (sdfgi_integrate.glsl:350-473): one thread per interior
// octahedral texel, into both layers, and the border texels it mirrors to.

@group(3) @binding(8) var gProbeStoreData: texture_storage_2d_array<r32uint, write>;
@group(3) @binding(10) var gProbeStoreAverage: texture_2d<i32>;

@compute @workgroup_size(8, 8, 1)
fn sdfgiProbeStoreCS(@builtin(global_invocation_id) gid: vec3<u32>) {
    let maxCascades = sdfgiParam(0u);
    let cascade = sdfgiParam(1u);
    let historySize = sdfgiParam(3u);
    let imageSize = vec2<i32>(sdfgiParamI(6u), sdfgiParamI(7u));
    let pos = vec2<i32>(gid.xy);
    if (any(pos >= imageSize)) {
        return;
    }
    // :356-361
    let shPos = (pos / SDFGI_OCT) * vec2<i32>(1, i32(SDFGI_SH_SIZE));
    let octPos = (pos / SDFGI_OCT) * (SDFGI_OCT + 2) + vec2<i32>(1);
    let localPos = pos % SDFGI_OCT;
    let normal = sdfgiOctahedronDecode(vec2<f32>(localPos) / f32(SDFGI_OCT));

    // :412-426
    var irradiance = vec3<f32>(0.0);
    var radiance = vec3<f32>(0.0);
    for (var i = 0u; i < SDFGI_SH_SIZE; i++) {
        let averagePos = shPos + vec2<i32>(0, i32(i));
        let average = textureLoad(gProbeStoreAverage, averagePos, 0);
        let sh = (vec4<f32>(average) / f32(historySize)) / f32(1 << u32(SDFGI_HISTORY_BITS));
        let m = sh.rgb * sdfgiShBasis(normal, i) * 4.0;
        irradiance += m * sdfgiLMult(i);
        radiance += m;
    }
    let irradianceRgbe = sdfgiRgbeEncode(irradiance); // :430
    let radianceRgbe = sdfgiRgbeEncode(radiance);

    // :435-463, the texel and its mirror copies in the 1-texel border.
    let o = SDFGI_OCT;
    let none = vec2<i32>(-2, -2);
    var copyTo = array<vec2<i32>, 4>(octPos + localPos, none, none, none);
    if (all(localPos == vec2<i32>(0, 0))) {
        copyTo[1] = octPos + vec2<i32>(o - 1, -1);
        copyTo[2] = octPos + vec2<i32>(-1, o - 1);
        copyTo[3] = octPos + vec2<i32>(o, o);
    } else if (all(localPos == vec2<i32>(o - 1, 0))) {
        copyTo[1] = octPos + vec2<i32>(0, -1);
        copyTo[2] = octPos + vec2<i32>(o, o - 1);
        copyTo[3] = octPos + vec2<i32>(-1, o);
    } else if (all(localPos == vec2<i32>(0, o - 1))) {
        copyTo[1] = octPos + vec2<i32>(-1, 0);
        copyTo[2] = octPos + vec2<i32>(o - 1, o);
        copyTo[3] = octPos + vec2<i32>(o, -1);
    } else if (all(localPos == vec2<i32>(o - 1, o - 1))) {
        copyTo[1] = octPos + vec2<i32>(0, o);
        copyTo[2] = octPos + vec2<i32>(o, 0);
        copyTo[3] = octPos + vec2<i32>(-1, -1);
    } else if (localPos.y == 0) {
        copyTo[1] = octPos + vec2<i32>(o - localPos.x - 1, localPos.y - 1);
    } else if (localPos.x == 0) {
        copyTo[1] = octPos + vec2<i32>(localPos.x - 1, o - localPos.y - 1);
    } else if (localPos.y == o - 1) {
        copyTo[1] = octPos + vec2<i32>(o - localPos.x - 1, localPos.y + 1);
    } else if (localPos.x == o - 1) {
        copyTo[1] = octPos + vec2<i32>(localPos.x + 1, o - localPos.y - 1);
    }
    // copy_to was a texture position (oct_pos, cascade); the sentinel is (-2, -2, -2) there.
    for (var i = 0; i < 4; i++) { // :465-471
        if (all(copyTo[i] == none)) {
            continue;
        }
        textureStore(gProbeStoreData, copyTo[i], i32(cascade), vec4<u32>(irradianceRgbe));
        textureStore(gProbeStoreData, copyTo[i], i32(cascade + maxCascades), vec4<u32>(radianceRgbe));
    }
}

// ---------------------------------------------------------------------------
// MODE_SCROLL (sdfgi_integrate.glsl:475-598): the kept probes' history and
// average copied to the scroll textures at their new places, a new probe
// seeded from the parent cascade's eight nearest probes.

@group(3) @binding(7) var<storage, read> gProbeScrollCascades: SdfgiCascades;
@group(3) @binding(9) var gProbeScrollHistory: texture_2d_array<i32>;
@group(3) @binding(10) var gProbeScrollAverage: texture_2d<i32>;
@group(3) @binding(11) var gProbeScrollHistoryOut: texture_storage_2d_array<rgba16sint, write>;
@group(3) @binding(12) var gProbeScrollAverageOut: texture_storage_2d<rgba32sint, write>;
@group(3) @binding(13) var gProbeScrollParent: texture_2d<i32>;

// imageLoad of the parent's average: outside the image it reads zero (as the CPU mirror does).
fn sdfgiProbeParentAverage(p: vec2<i32>) -> vec4<i32> {
    let size = vec2<i32>(textureDimensions(gProbeScrollParent));
    if (any(p < vec2<i32>(0)) || any(p >= size)) {
        return vec4<i32>(0);
    }
    return textureLoad(gProbeScrollParent, p, 0);
}

@compute @workgroup_size(8, 8, 1)
fn sdfgiProbeScrollCS(@builtin(global_invocation_id) gid: vec3<u32>) {
    let maxCascades = sdfgiParam(0u);
    let cascade = sdfgiParam(1u);
    let historySize = sdfgiParam(3u);
    let imageSize = vec2<i32>(sdfgiParamI(6u), sdfgiParamI(7u));
    let scroll = sdfgiParamI3(12u);
    let pos = vec2<i32>(gid.xy);
    if (any(pos >= imageSize)) {
        return;
    }
    let sh = i32(SDFGI_SH_SIZE);
    // :477-482
    var probeCell: vec3<i32>;
    probeCell.x = pos.x % SDFGI_PROBE_AXIS;
    probeCell.y = pos.y;
    probeCell.z = pos.x / SDFGI_PROBE_AXIS;
    let readProbe = probeCell - scroll;

    if (all(readProbe >= vec3<i32>(0)) && all(readProbe < vec3<i32>(SDFGI_PROBE_AXIS))) { // :484
        var texPos = readProbe.xy;
        texPos.x += readProbe.z * SDFGI_PROBE_AXIS;
        for (var j = 0u; j < historySize; j++) { // :491-499
            for (var i = 0; i < sh; i++) {
                let src = vec2<i32>(texPos.x, texPos.y * sh + i);
                let dst = vec2<i32>(pos.x, pos.y * sh + i);
                textureStore(gProbeScrollHistoryOut, dst, i32(j), textureLoad(gProbeScrollHistory, src, i32(j), 0));
            }
        }
        for (var i = 0; i < sh; i++) { // :501-507
            let src = vec2<i32>(texPos.x, texPos.y * sh + i);
            let dst = vec2<i32>(pos.x, pos.y * sh + i);
            textureStore(gProbeScrollAverageOut, dst, textureLoad(gProbeScrollAverage, src, 0));
        }
    } else if (cascade < maxCascades - 1u) { // :508
        // :512-522, to the parent's probe coordinates (the cascade UBO as it stands: research.md Finding 10).
        let cellToProbe = f32(f32(SDFGI_CASCADE_SIZE) / f32(SDFGI_PROBE_AXIS - 1));
        let probeCellSize = cellToProbe / gProbeScrollCascades.data[cascade].toCell;
        var probePos = gProbeScrollCascades.data[cascade].offset + vec3<f32>(probeCell) * probeCellSize;
        let probeCellSizeNext = cellToProbe / gProbeScrollCascades.data[cascade + 1u].toCell;
        probePos -= gProbeScrollCascades.data[cascade + 1u].offset;
        probePos /= probeCellSizeNext;
        let probePosi = vec3<i32>(probePos);

        var averageLight: array<vec4<f32>, 16>;
        for (var j = 0; j < sh; j++) {
            averageLight[j] = vec4<f32>(0.0);
        }
        var totalWeight = 0.0;
        for (var i = 0; i < 8; i++) { // :533-552
            let offset = probePosi + ((vec3<i32>(i) >> vec3<u32>(0u, 1u, 2u)) & vec3<i32>(1, 1, 1));
            let trilinear = vec3<f32>(1.0) - abs(probePos - vec3<f32>(offset));
            let weight = trilinear.x * trilinear.y * trilinear.z;
            var texPos = offset.xy;
            texPos.x += offset.z * SDFGI_PROBE_AXIS;
            for (var j = 0; j < sh; j++) {
                let average = sdfgiProbeParentAverage(vec2<i32>(texPos.x, texPos.y * sh + j));
                let value = (vec4<f32>(average) / f32(historySize)) / f32(1 << u32(SDFGI_HISTORY_BITS));
                averageLight[j] += value * weight;
            }
            totalWeight += weight;
        }
        if (totalWeight > 0.0) {
            totalWeight = 1.0 / totalWeight;
        }
        for (var i = 0; i < sh; i++) { // :559-570
            var ivalue = clamp(vec4<i32>(averageLight[i] * totalWeight * f32(1 << u32(SDFGI_HISTORY_BITS))), vec4<i32>(-32768), vec4<i32>(32767));
            let dst = vec2<i32>(pos.x, pos.y * sh + i);
            for (var j = 0u; j < historySize; j++) {
                textureStore(gProbeScrollHistoryOut, dst, i32(j), ivalue);
            }
            ivalue *= i32(historySize);
            textureStore(gProbeScrollAverageOut, dst, ivalue);
        }
    } else {
        // :572-596, the last cascade copies what is there (its load reads dst_pos, as the engine's does).
        for (var j = 0u; j < historySize; j++) {
            for (var i = 0; i < sh; i++) {
                let dst = vec2<i32>(pos.x, pos.y * sh + i);
                textureStore(gProbeScrollHistoryOut, dst, i32(j), textureLoad(gProbeScrollHistory, dst, i32(j), 0));
            }
        }
        for (var i = 0; i < sh; i++) {
            let spos = vec2<i32>(pos.x, pos.y * sh + i);
            textureStore(gProbeScrollAverageOut, spos, textureLoad(gProbeScrollAverage, spos, 0));
        }
    }
}

// ---------------------------------------------------------------------------
// MODE_SCROLL_STORE (sdfgi_integrate.glsl:600-620): the scroll textures copied back.

@group(3) @binding(9) var gProbeScrollStoreHistory: texture_storage_2d_array<rgba16sint, write>;
@group(3) @binding(10) var gProbeScrollStoreAverage: texture_storage_2d<rgba32sint, write>;
@group(3) @binding(11) var gProbeScrollStoreHistoryIn: texture_2d_array<i32>;
@group(3) @binding(12) var gProbeScrollStoreAverageIn: texture_2d<i32>;

@compute @workgroup_size(8, 8, 1)
fn sdfgiProbeScrollStoreCS(@builtin(global_invocation_id) gid: vec3<u32>) {
    let historySize = sdfgiParam(3u);
    let imageSize = vec2<i32>(sdfgiParamI(6u), sdfgiParamI(7u));
    let pos = vec2<i32>(gid.xy);
    if (any(pos >= imageSize)) {
        return;
    }
    let sh = i32(SDFGI_SH_SIZE);
    for (var j = 0u; j < historySize; j++) {
        for (var i = 0; i < sh; i++) {
            let spos = vec2<i32>(pos.x, pos.y * sh + i);
            textureStore(gProbeScrollStoreHistory, spos, i32(j), textureLoad(gProbeScrollStoreHistoryIn, spos, i32(j), 0));
        }
    }
    for (var i = 0; i < sh; i++) {
        let spos = vec2<i32>(pos.x, pos.y * sh + i);
        textureStore(gProbeScrollStoreAverage, spos, textureLoad(gProbeScrollStoreAverageIn, spos, 0));
    }
}
