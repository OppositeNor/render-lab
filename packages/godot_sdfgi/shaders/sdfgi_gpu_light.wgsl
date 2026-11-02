// Godot's SDFGI on the GPU (src/godot/sdfgi/gpu.rs): the voxelizer's three
// raster passes (`_render_sdfgi`, render_forward_clustered.cpp:3099-3177, and
// the MODE_RENDER_SDF block of scene_forward_clustered.glsl:2906-2998) and
// the light injection (sdfgi_direct_light.glsl, MODE_PROCESS_DYNAMIC and
// MODE_PROCESS_STATIC), at group 3 as gpu.rs declares them. Composed after
// sdfgi_gpu_common.wgsl (params, CascadeData, Light, ProcessVoxel) and
// sdfgi_passes.wgsl (attenuation, the LTC integral, `sdfgiLightAt`).
//
// Where the lab differs: the voxelizer draws the lab's meshes with each
// object's albedo and emission (no material code); the bounce reads the
// probe maps' E5B9G9R9 words and filters them by hand, and the occlusion's
// R4G4B4A4 nibbles likewise (wgpu has no reinterpreting views); the area
// light has no projector texture (texture_color stays 1).
//
// Ported from Godot Engine (MIT; Copyright (c) 2014-present Godot Engine
// contributors, Copyright (c) 2007-2014 Juan Linietsky, Ariel Manzur;
// THIRD_PARTY.md).

// ---------------------------------------------------------------------------
// The voxelizer (pass sdfgiVoxelize). Params: [0] axis, [1..3] the region's
// offset in cells, [4..6] its size in cells, [7..9] its world box's corner,
// [10..12] its size (real metres, gi.cpp:1463-1504).

@group(3) @binding(1) var gVoxAlbedo: texture_storage_3d<r16uint, write>;
@group(3) @binding(2) var gVoxEmission: texture_storage_3d<r32uint, write>;
@group(3) @binding(3) var gVoxEmissionAniso: texture_storage_3d<r32uint, write>;
@group(3) @binding(4) var gVoxFacing: texture_storage_3d<r32uint, atomic>;

struct SdfgiVoxelVaryings {
    @builtin(position) clip: vec4<f32>,
    @location(0) worldPosition: vec3<f32>,
    @location(1) worldNormal: vec3<f32>,
    @location(2) @interpolate(flat) objectIndex: u32,
}

fn sdfgiAxisOf(v: vec3<f32>, a: u32) -> f32 {
    switch a {
        case 0u: { return v.x; }
        case 1u: { return v.y; }
        default: { return v.z; }
    }
}

// The vertex stage of one axis's pass: Godot's orthographic camera on the
// region's +axis face, looking along -axis (render_forward_clustered.cpp:3131-3154):
// x over the face's right axis, y over its up axis, z over the box's depth,
// reversed (1 at the near face, 0 at the far one), so the frustum is the box.
@vertex
fn sdfgiVoxelizeVS(input: SceneVertex) -> SdfgiVoxelVaryings {
    let o = gObjects[input.objectIndex];
    var out: SdfgiVoxelVaryings;
    out.worldPosition = transformPoint(o.localToWorld, input.position);
    out.worldNormal = normalToWorld(o, input.normal);
    out.objectIndex = input.objectIndex;
    let axis = sdfgiParam(0u);
    let right = (axis + 1u) % 3u;
    let up = (axis + 2u) % 3u;
    let bmin = vec3<f32>(sdfgiParamF(7u), sdfgiParamF(8u), sdfgiParamF(9u));
    let bsize = vec3<f32>(sdfgiParamF(10u), sdfgiParamF(11u), sdfgiParamF(12u));
    let halfSize = bsize * 0.5;
    let center = bmin + halfSize;
    let rel = out.worldPosition - center;
    // view = inverse(camera at center + axis * half[axis], basis (right, up, axis)).
    let vx = sdfgiAxisOf(rel, right);
    let vy = sdfgiAxisOf(rel, up);
    let vz = sdfgiAxisOf(rel, axis) - sdfgiAxisOf(halfSize, axis);
    // set_orthogonal(-h, h, -v, v, 0, d) with Godot's depth correction (reversed, remapped): z = 1 + vz / d.
    let d = 2.0 * sdfgiAxisOf(halfSize, axis);
    out.clip = vec4<f32>(vx / sdfgiAxisOf(halfSize, right), vy / sdfgiAxisOf(halfSize, up), 1.0 + vz / d, 1.0);
    // The ceiling panel is the area light, not geometry (sim.rs voxelized_objects); hidden objects as sceneVS hides them.
    if (o.shapeKind == SHAPE_PANEL || (input.objectIndex < 32u && (gFrame.counts.w & (1u << input.objectIndex)) != 0u)) {
        out.clip = vec4<f32>(2.0, 2.0, 2.0, 1.0);
    }
    return out;
}

// The shared exponent of rgbe_encode (B = 15, N = 9): the three 9-bit
// mantissas and the exponent (scene_forward_clustered.glsl:2967-2987,
// sdfgi_direct_light.glsl:464-487).
fn sdfgiSharedExponent(color: vec3<f32>) -> vec4<u32> {
    let pow2to9 = 512.0;
    let B = 15.0;
    let N = 9.0;
    let LN2 = 0.6931471805599453094172321215;
    let cRed = clamp(color.r, 0.0, 65408.0);
    let cGreen = clamp(color.g, 0.0, 65408.0);
    let cBlue = clamp(color.b, 0.0, 65408.0);
    let cMax = max(cRed, max(cGreen, cBlue));
    let expp = max(-B - 1.0, floor(log(cMax) / LN2)) + 1.0 + B;
    let sMax = floor((cMax / pow(2.0, expp - B - N)) + 0.5);
    var exps = expp + 1.0;
    if (0.0 <= sMax && sMax < pow2to9) {
        exps = expp;
    }
    let sRed = floor((cRed / pow(2.0, exps - B - N)) + 0.5);
    let sGreen = floor((cGreen / pow(2.0, exps - B - N)) + 0.5);
    let sBlue = floor((cBlue / pow(2.0, exps - B - N)) + 0.5);
    return vec4<u32>(u32(sRed), u32(sGreen), u32(sBlue), u32(exps));
}

// RGBE8985, the 30-bit word with two spare bits (scene_forward_clustered.glsl:2989, sdfgi_direct_light.glsl:490).
fn sdfgiPack8985(s: vec4<u32>) -> u32 {
    return ((s.x & 0x1FFu) >> 1u) | ((s.y & 0x1FFu) << 8u) | (((s.z & 0x1FFu) >> 1u) << 17u) | ((s.w & 0x1Fu) << 25u);
}

// E5B9G9R9 (sdfgi_direct_light.glsl:493).
fn sdfgiPack9995(s: vec4<u32>) -> u32 {
    return (s.x & 0x1FFu) | ((s.y & 0x1FFu) << 9u) | ((s.z & 0x1FFu) << 18u) | ((s.w & 0x1Fu) << 27u);
}

@fragment
fn sdfgiVoxelizeFS(input: SdfgiVoxelVaryings) -> @location(0) vec4<f32> {
    let o = gObjects[input.objectIndex];
    let albedo = o.albedoRoughness.rgb;
    let emission = o.emissive.rgb * o.emissive.w;
    let offset = sdfgiParamI3(1u);
    let size = sdfgiParamI3(4u);
    let bmin = vec3<f32>(sdfgiParamF(7u), sdfgiParamF(8u), sdfgiParamF(9u));
    let bsize = vec3<f32>(sdfgiParamF(10u), sdfgiParamF(11u), sdfgiParamF(12u));
    // scene_forward_clustered.glsl:2909-2910: sdf_to_bounds * view-space vertex is the
    // position in the region's box, 0..1; truncated to a cell, unclamped.
    let localPos = (input.worldPosition - bmin) / bsize;
    let gridPos = offset + vec3<i32>(localPos * vec3<f32>(size));

    // :2912-2917, the solid flag and RGB555.
    var albedo16 = 0x1u;
    albedo16 |= min(u32(albedo.r * 31.0), 31u) << 11u;
    albedo16 |= min(u32(albedo.g * 31.0), 31u) << 6u;
    albedo16 |= min(u32(albedo.b * 31.0), 31u) << 1u;
    textureStore(gVoxAlbedo, gridPos, vec4<u32>(albedo16));

    // :2919-2943, the face the normal points most along, OR-ed in.
    var facingBits = 0u;
    let camNormal = normalize(input.worldNormal);
    var closestDist = -1e20;
    for (var i = 0u; i < 6u; i++) {
        let d = dot(camNormal, sdfgiAnisoDir(i));
        if (d > closestDist) {
            closestDist = d;
            facingBits = 1u << i;
        }
    }
    textureAtomicOr(gVoxFacing, gridPos, facingBits);

    // :2946-2995, emission split over the six faces, RGBE8985.
    if (length(emission) > 0.001) {
        var lumas: array<f32, 6>;
        var lightTotal = vec3<f32>(0.0);
        for (var i = 0u; i < 6u; i++) {
            let strength = max(0.0, dot(camNormal, sdfgiAnisoDir(i)));
            let light = emission * strength;
            lightTotal += light;
            lumas[i] = max(light.r, max(light.g, light.b));
        }
        let lumaTotal = max(lightTotal.r, max(lightTotal.g, lightTotal.b));
        var lightAniso = 0u;
        for (var i = 0u; i < 6u; i++) {
            lightAniso |= min(31u, u32((lumas[i] / lumaTotal) * 31.0)) << (i * 5u);
        }
        let lightRgbe = sdfgiPack8985(sdfgiSharedExponent(lightTotal));
        textureStore(gVoxEmission, gridPos, vec4<u32>(lightRgbe));
        textureStore(gVoxEmissionAniso, gridPos, vec4<u32>(lightAniso));
    }
    return vec4<f32>(0.0);
}

// ---------------------------------------------------------------------------
// The light passes (sdfgiDynamicLight, sdfgiStaticLight). Params: [0]
// max_cascades, [1] cascade, [2] light_count, [3] process_offset, [4]
// process_increment, [5] bounce_feedback, [6] y_mult, [7] use_occlusion
// (sdfgi_direct_light.glsl:94-108; grid_size 128, probe_axis_size 17).

@group(3) @binding(1) var gDlSdf: binding_array<texture_3d<f32>, 4>;
@group(3) @binding(2) var gDlSampler: sampler;
@group(3) @binding(3) var<storage, read> gDlDispatch: SdfgiDispatchData;
@group(3) @binding(4) var<storage, read> gDlCells: array<SdfgiProcessVoxel>;
@group(3) @binding(4) var<storage, read_write> gSlCells: array<SdfgiProcessVoxel>;
@group(3) @binding(5) var gDlLight: texture_storage_3d<r32uint, write>;
@group(3) @binding(6) var gDlAniso0: texture_storage_3d<rgba8unorm, write>;
@group(3) @binding(7) var gDlAniso1: texture_storage_3d<rg8unorm, write>;
@group(3) @binding(8) var<storage, read> gDlCascades: SdfgiCascades;
@group(3) @binding(9) var<storage, read> gDlLights: array<SdfgiGpuLight>;
@group(3) @binding(10) var gDlProbes: texture_2d_array<u32>;
@group(3) @binding(11) var gDlOcclusion: texture_3d<u32>;

const SDFGI_DL_GRID: f32 = 128.0;

// The light-probe texture's E5B9G9R9 word at a texel, clamped to the layer.
fn sdfgiDlProbeTexel(x: i32, y: i32, layer: i32) -> vec3<f32> {
    let size = vec2<i32>(textureDimensions(gDlProbes));
    let p = clamp(vec2<i32>(x, y), vec2<i32>(0), size - vec2<i32>(1));
    return sdfgiRgb9e5(textureLoad(gDlProbes, p, layer, 0).r);
}

// textureLod(sampler2DArray(lightprobe_texture, linear_sampler), uvw, 0.0).rgb,
// filtered by hand (sdfgi_direct_light.glsl:275).
fn sdfgiDlProbeBilinear(uvw: vec3<f32>) -> vec3<f32> {
    let size = vec2<f32>(textureDimensions(gDlProbes));
    let t = uvw.xy * size - vec2<f32>(0.5);
    let b = floor(t);
    let f = t - b;
    let x0 = i32(b.x);
    let y0 = i32(b.y);
    let layer = i32(round(uvw.z));
    let a = mix(sdfgiDlProbeTexel(x0, y0, layer), sdfgiDlProbeTexel(x0 + 1, y0, layer), f.x);
    let c = mix(sdfgiDlProbeTexel(x0, y0 + 1, layer), sdfgiDlProbeTexel(x0 + 1, y0 + 1, layer), f.x);
    return mix(a, c, f.y);
}

// The R4G4B4A4_UNORM_PACK16 view of one occlusion texel (r bits 12-15, g 8-11, b 4-7, a 0-3), clamped.
fn sdfgiDlOcclusionTexel(p: vec3<i32>) -> vec4<f32> {
    let size = vec3<i32>(textureDimensions(gDlOcclusion));
    let w = textureLoad(gDlOcclusion, clamp(p, vec3<i32>(0), size - vec3<i32>(1)), 0).r;
    return vec4<f32>(f32((w >> 12u) & 0xFu), f32((w >> 8u) & 0xFu), f32((w >> 4u) & 0xFu), f32(w & 0xFu)) / 15.0;
}

// textureLod(sampler3D(occlusion_texture, linear_sampler), uvw, 0.0), filtered by hand (:263).
fn sdfgiDlOcclusionTrilinear(uvw: vec3<f32>) -> vec4<f32> {
    let size = vec3<f32>(textureDimensions(gDlOcclusion));
    let t = uvw * size - vec3<f32>(0.5);
    let b = floor(t);
    let f = t - b;
    let bi = vec3<i32>(b);
    var sum = vec4<f32>(0.0);
    for (var i = 0; i < 8; i++) {
        let o = vec3<i32>(i & 1, (i >> 1u) & 1, (i >> 2u) & 1);
        let w = select(vec3<f32>(1.0) - f, f, o == vec3<i32>(1));
        sum += w.x * w.y * w.z * sdfgiDlOcclusionTexel(bi + o);
    }
    return sum;
}

// octahedron_encode (sdfgi_direct_light.glsl:117-123).
fn sdfgiDlOctahedronEncode(nIn: vec3<f32>) -> vec2<f32> {
    var n = nIn / (abs(nIn.x) + abs(nIn.y) + abs(nIn.z));
    if (n.z < 0.0) {
        let s = vec2<f32>(select(-1.0, 1.0, n.x >= 0.0), select(-1.0, 1.0, n.y >= 0.0));
        n = vec3<f32>((vec2<f32>(1.0) - abs(n.yx)) * s, n.z);
    }
    return n.xy * 0.5 + vec2<f32>(0.5);
}

// The 26 neighbours in the order of the bits (sdfgi_direct_light.glsl:520-546).
fn sdfgiNeighbour(i: u32) -> vec3<i32> {
    // x, y, z run -1..1 in that nesting order, skipping (0, 0, 0) (index 13 of 27).
    let k = select(i, i + 1u, i >= 13u);
    return vec3<i32>(i32(k / 9u) - 1, i32((k / 3u) % 3u) - 1, i32(k % 3u) - 1);
}

// What both passes share: the cell's light from the stored light and every
// light, its shadow ray marched through the cascades (sdfgi_direct_light.glsl:179-445).
struct SdfgiDlCell {
    index: u32,
    valid: bool,
    positioni: vec3<i32>,
    position: u32,
    albedoWord: u32,
    light: u32,
    lightAniso: u32,
    accum: array<vec3<f32>, 6>,
}

fn sdfgiDlLightOf(i: u32) -> SdfgiLight {
    let g = gDlLights[i];
    var l: SdfgiLight;
    l.kind = g.kind;
    l.position = g.position;
    l.direction = g.direction;
    l.radius = g.radius;
    l.areaWidth = g.areaWidth.xyz;
    l.attenuation = g.attenuation;
    l.areaHeight = g.areaHeight.xyz;
    l.invSpotAttenuation = g.invSpotAttenuation;
    l.cosSpotAngle = g.cosSpotAngle;
    return l;
}

// :294-445 for one cell: the stored light, then every light that reaches it unshadowed.
fn sdfgiDlTrace(cellIn: SdfgiDlCell) -> SdfgiDlCell {
    var cell = cellIn;
    let cascade = sdfgiParam(1u);
    let maxCascades = sdfgiParam(0u);
    let lightCount = sdfgiParam(2u);
    var position = vec3<f32>(cell.positioni) + vec3<f32>(0.5);
    position /= gDlCascades.data[cascade].toCell;
    position += gDlCascades.data[cascade].offset;
    let albedo = vec3<f32>(vec3<u32>(cell.albedoWord >> 10u, cell.albedoWord >> 5u, cell.albedoWord) & vec3<u32>(0x1Fu)) / 31.0;
    let validAniso = (cell.albedoWord >> 15u) & 0x3Fu;
    {
        // :295-311, the stored RGBE8985 light (emission, and the static light).
        let rgbe = cell.light;
        let r = f32((rgbe & 0xFFu) << 1u);
        let g = f32((rgbe >> 8u) & 0x1FFu);
        let b = f32(((rgbe >> 17u) & 0xFFu) << 1u);
        let e = f32((rgbe >> 25u) & 0x1Fu);
        let m = pow(2.0, e - 15.0 - 9.0);
        let l = vec3<f32>(r, g, b) * m;
        let aniso = cell.lightAniso;
        for (var i = 0u; i < 6u; i++) {
            let strength = f32((aniso >> (i * 5u)) & 0x1Fu) / 31.0;
            cell.accum[i] += l * strength;
        }
    }
    let posToUvw = vec3<f32>(1.0 / SDFGI_DL_GRID);
    for (var i = 0u; i < lightCount; i++) {
        // :324-370, the light's direction, distance, attenuation (sdfgi_passes.wgsl's switch).
        let at = sdfgiLightAt(sdfgiDlLightOf(i), position);
        if (!at.valid) {
            continue;
        }
        var attenuation = at.attenuation;
        let direction = at.direction;
        var lightDistance = at.distance;
        var hit = false;
        var rayPos = position;
        let rayDir = direction;
        let invDir = vec3<f32>(1.0) / rayDir;
        // :379-381, the bias out of the cell.
        let cellSize = 1.0 / gDlCascades.data[cascade].toCell;
        rayPos += sign(direction) * cellSize * 0.48;
        rayPos += rayDir * 0.4 * cellSize;
        for (var j = cascade; j < maxCascades; j++) {
            var pos = rayPos - gDlCascades.data[j].offset;
            pos *= gDlCascades.data[j].toCell;
            let localDistance = lightDistance * gDlCascades.data[j].toCell;
            if (any(pos < vec3<f32>(0.0)) || any(pos >= vec3<f32>(SDFGI_DL_GRID))) {
                continue;
            }
            let t0 = -pos * invDir;
            let t1 = (vec3<f32>(SDFGI_DL_GRID) - pos) * invDir;
            let tmax = max(t0, t1);
            var maxAdvance = min(tmax.x, min(tmax.y, tmax.z));
            maxAdvance = min(localDistance, maxAdvance);
            var advance = 0.0;
            var occlusion = 1.0;
            while (advance < maxAdvance) {
                let uvw = (pos + rayDir * advance) * posToUvw;
                let distance = textureSampleLevel(gDlSdf[j], gDlSampler, uvw, 0.0).r * 255.0 - 1.0;
                if (distance < 0.001) {
                    hit = true;
                    break;
                }
                occlusion = min(occlusion, distance);
                advance += distance;
            }
            if (hit) {
                attenuation *= occlusion;
                break;
            }
            if (advance >= localDistance) {
                break;
            }
            pos += rayDir * maxAdvance;
            pos /= gDlCascades.data[j].toCell;
            pos += gDlCascades.data[j].offset;
            lightDistance -= maxAdvance / gDlCascades.data[j].toCell;
            rayPos = pos;
        }
        if (!hit) {
            let g = gDlLights[i];
            let light = albedo * g.color * vec3<f32>(1.0) * g.energy * attenuation;
            for (var k = 0u; k < 6u; k++) {
                if ((validAniso & (1u << k)) != 0u) {
                    cell.accum[k] += max(0.0, dot(sdfgiAnisoDir(k), direction)) * light;
                }
            }
        }
    }
    return cell;
}

// :180-205, the cell this invocation lights this frame.
fn sdfgiDlCellIndex(gid: u32) -> u32 {
    var voxelIndex = gid;
    let increment = sdfgiParam(4u);
    if (increment > 1u) {
        voxelIndex *= increment;
        voxelIndex += sdfgiParam(3u);
    }
    return voxelIndex;
}

fn sdfgiDlStart(index: u32, v: SdfgiProcessVoxel) -> SdfgiDlCell {
    var cell: SdfgiDlCell;
    cell.index = index;
    cell.valid = true;
    cell.position = v.position;
    cell.positioni = vec3<i32>((vec3<u32>(v.position) >> vec3<u32>(0u, 7u, 14u)) & vec3<u32>(0x7Fu));
    cell.albedoWord = v.albedo;
    cell.light = v.light;
    cell.lightAniso = v.lightAniso;
    for (var i = 0u; i < 6u; i++) {
        cell.accum[i] = vec3<f32>(0.0);
    }
    return cell;
}

@compute @workgroup_size(64, 1, 1)
fn sdfgiDynamicLightCS(@builtin(global_invocation_id) gid: vec3<u32>) {
    let voxelIndex = sdfgiDlCellIndex(gid.x);
    if (voxelIndex >= gDlDispatch.totalCount) {
        return;
    }
    var cell = sdfgiDlStart(voxelIndex, gDlCells[voxelIndex]);
    let cascade = sdfgiParam(1u);
    let bounceFeedback = sdfgiParamF(5u);
    let useOcclusion = sdfgiParam(7u) != 0u;
    let maxCascades = sdfgiParam(0u);
    let albedo = vec3<f32>(vec3<u32>(cell.albedoWord >> 10u, cell.albedoWord >> 5u, cell.albedoWord) & vec3<u32>(0x1Fu)) / 31.0;
    let validAniso = (cell.albedoWord >> 15u) & 0x3Fu;
    // :216-290, the bounce: last frame's probe irradiance around the cell, per face.
    if (bounceFeedback > 0.001) {
        let feedback = select(mix(albedo, vec3<f32>(1.0), bounceFeedback - 1.0), albedo * bounceFeedback, bounceFeedback < 1.0);
        let pos = (vec3<f32>(cell.positioni) + vec3<f32>(0.5)) * f32(SDFGI_PROBE_AXIS - 1) / SDFGI_DL_GRID;
        let probeBasePos = vec3<i32>(pos);
        var weightAccum = array<f32, 6>(0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        var texPos = vec3<i32>(probeBasePos.xy, i32(cascade));
        texPos.x += probeBasePos.z * SDFGI_PROBE_AXIS;
        texPos = vec3<i32>(texPos.xy * (SDFGI_OCT + 2) + vec2<i32>(1), texPos.z);
        let baseTexPosf = vec3<f32>(texPos);
        let texPixelSize = vec2<f32>(1.0) / vec2<f32>(vec2<i32>((SDFGI_OCT + 2) * SDFGI_PROBE_AXIS * SDFGI_PROBE_AXIS, (SDFGI_OCT + 2) * SDFGI_PROBE_AXIS));
        let probeUvOffset = vec3<f32>(vec3<i32>(SDFGI_OCT + 2, SDFGI_OCT + 2, (SDFGI_OCT + 2) * SDFGI_PROBE_AXIS)) * texPixelSize.xyx;
        for (var j = 0u; j < 8u; j++) {
            let offset = (vec3<i32>(i32(j)) >> vec3<u32>(0u, 1u, 2u)) & vec3<i32>(1);
            let probePosi = probeBasePos + offset;
            let probePos = vec3<f32>(probePosi);
            let probeToPos = pos - probePos;
            let probeDir = normalize(-probeToPos);
            let trilinear = vec3<f32>(1.0) - abs(probeToPos);
            for (var k = 0u; k < 6u; k++) {
                if ((validAniso & (1u << k)) != 0u) {
                    let n = sdfgiAnisoDir(k);
                    var weight = trilinear.x * trilinear.y * trilinear.z * max(0.0, dot(n, probeDir));
                    if (weight > 0.0 && useOcclusion) {
                        // :254-265, the 4-bit visibility of this probe's parity class at the face's neighbour.
                        let occIndexv = abs((gDlCascades.data[cascade].probeWorldOffset + probePosi) & vec3<i32>(1)) * vec3<i32>(1, 2, 4);
                        let occMask = select(vec4<f32>(0.0), vec4<f32>(1.0), vec4<i32>(occIndexv.x | occIndexv.y) == vec4<i32>(0, 1, 2, 3));
                        var occPos = (vec3<f32>(cell.positioni) + n + vec3<f32>(0.5)) / SDFGI_DL_GRID;
                        occPos.z += f32(cascade);
                        if (occIndexv.z != 0) {
                            occPos.x += 1.0;
                        }
                        occPos *= vec3<f32>(0.5, 1.0, 1.0 / f32(maxCascades));
                        let occlusion = dot(sdfgiDlOcclusionTrilinear(occPos), occMask);
                        weight *= occlusion;
                    }
                    if (weight > 0.0) {
                        var texPosf = baseTexPosf + vec3<f32>(sdfgiDlOctahedronEncode(n) * f32(SDFGI_OCT), 0.0);
                        texPosf = vec3<f32>(texPosf.xy * texPixelSize, texPosf.z);
                        var posUvw = texPosf;
                        posUvw = vec3<f32>(posUvw.xy + vec2<f32>(offset.xy) * probeUvOffset.xy, posUvw.z);
                        posUvw.x += f32(offset.z) * probeUvOffset.z;
                        let indirectLight = sdfgiDlProbeBilinear(posUvw);
                        cell.accum[k] += indirectLight * weight;
                        weightAccum[k] += weight;
                    }
                }
            }
        }
        for (var k = 0u; k < 6u; k++) {
            if (weightAccum[k] > 0.0) {
                cell.accum[k] /= weightAccum[k];
                cell.accum[k] *= feedback;
            }
        }
    }
    cell = sdfgiDlTrace(cell);
    // :449-512, the six faces' total in E5B9G9R9, and their share of it.
    var lumas: array<f32, 6>;
    var lightTotal = vec3<f32>(0.0);
    for (var i = 0u; i < 6u; i++) {
        lightTotal += cell.accum[i];
        lumas[i] = max(cell.accum[i].r, max(cell.accum[i].g, cell.accum[i].b));
    }
    let lumaTotal = max(lightTotal.r, max(lightTotal.g, lightTotal.b));
    let lightTotalRgbe = sdfgiPack9995(sdfgiSharedExponent(lightTotal));
    let aniso0 = vec4<f32>(lumas[0] / lumaTotal, lumas[1] / lumaTotal, lumas[2] / lumaTotal, lumas[3] / lumaTotal);
    let aniso1 = vec2<f32>(lumas[4] / lumaTotal, lumas[5] / lumaTotal);
    textureStore(gDlAniso0, cell.positioni, aniso0);
    textureStore(gDlAniso1, cell.positioni, vec4<f32>(aniso1, 0.0, 0.0));
    textureStore(gDlLight, cell.positioni, vec4<u32>(lightTotalRgbe));
    // :517-555, the empty neighbours whose nearest solid cell this is, for filtering.
    let neighbors = (cell.albedoWord >> 21u) | ((cell.position >> 21u) << 11u) | ((cell.light >> 30u) << 22u) | ((cell.lightAniso >> 30u) << 24u);
    for (var i = 0u; i < 26u; i++) {
        if ((neighbors & (1u << i)) != 0u) {
            let neighbourPos = cell.positioni + sdfgiNeighbour(i);
            textureStore(gDlLight, neighbourPos, vec4<u32>(lightTotalRgbe));
            textureStore(gDlAniso0, neighbourPos, aniso0);
            textureStore(gDlAniso1, neighbourPos, vec4<f32>(aniso1, 0.0, 0.0));
        }
    }
}

@compute @workgroup_size(64, 1, 1)
fn sdfgiStaticLightCS(@builtin(global_invocation_id) gid: vec3<u32>) {
    let voxelIndex = sdfgiDlCellIndex(gid.x);
    if (voxelIndex >= gDlDispatch.totalCount) {
        return;
    }
    var cell = sdfgiDlStart(voxelIndex, gSlCells[voxelIndex]);
    cell = sdfgiDlTrace(cell);
    var lumas: array<f32, 6>;
    var lightTotal = vec3<f32>(0.0);
    for (var i = 0u; i < 6u; i++) {
        lightTotal += cell.accum[i];
        lumas[i] = max(cell.accum[i].r, max(cell.accum[i].g, cell.accum[i].b));
    }
    let lumaTotal = max(lightTotal.r, max(lightTotal.g, lightTotal.b));
    // :488-490, RGBE8985 ("since it's self-save"), then :559-572, written back into the cell.
    let lightTotalRgbe = sdfgiPack8985(sdfgiSharedExponent(lightTotal));
    var light = gSlCells[voxelIndex].light & (3u << 30u);
    light |= lightTotalRgbe;
    gSlCells[voxelIndex].light = light;
    var lightAniso = gSlCells[voxelIndex].lightAniso & (3u << 30u);
    for (var i = 0u; i < 6u; i++) {
        lightAniso |= min(31u, u32((lumas[i] / lumaTotal) * 31.0)) << (i * 5u);
    }
    gSlCells[voxelIndex].lightAniso = lightAniso;
}
