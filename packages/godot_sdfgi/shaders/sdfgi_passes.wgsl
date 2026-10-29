// Godot's SDFGI light and probe passes (sdfgi_direct_light.glsl,
// sdfgi_integrate.glsl, area_lights_inc.glsl), ported to WGSL for the
// selftest, which runs them against their CPU mirrors (light.rs, probes.rs)
// through the kernels at the end. Composed after sdfgi_shading.wgsl.
//
// Ported from Godot Engine (MIT; Copyright (c) 2014-present Godot Engine
// contributors, Copyright (c) 2007-2014 Juan Linietsky, Ariel Manzur;
// THIRD_PARTY.md).
//
// The distance field they march is cascade 0's, in gShadingData after the
// probe maps (sim::shading_data): 128^3 bytes, four per word, four words per
// vec4, x fastest.

const SDFGI_GRID: f32 = 128.0;
const SDFGI_SDF_OFFSET: u32 = 628864u; // 4 cascades x 2 layers x 2312 x 136 words / 4 (sim::SDF_OFFSET)

fn sdfgiSdfByte(p: vec3<i32>) -> f32 {
    let c = clamp(p, vec3<i32>(0), vec3<i32>(127));
    let i = u32(c.x + c.y * 128 + c.z * 128 * 128);
    let word = bitcast<u32>(gShadingData[SDFGI_SDF_OFFSET + i / 16u][(i / 4u) % 4u]);
    return f32((word >> ((i % 4u) * 8u)) & 0xFFu) / 255.0;
}

// texture(sdf_cascades[0], linear_sampler) at uvw: trilinear, clamped to the edge.
fn sdfgiSampleSdf(uvw: vec3<f32>) -> f32 {
    let t = uvw * SDFGI_GRID - vec3<f32>(0.5);
    let b = floor(t);
    let f = t - b;
    let bi = vec3<i32>(b);
    var sum = 0.0;
    for (var i = 0; i < 8; i++) {
        let o = vec3<i32>(i & 1, (i >> 1u) & 1, (i >> 2u) & 1);
        let w = select(vec3<f32>(1.0) - f, f, o == vec3<i32>(1));
        sum += w.x * w.y * w.z * sdfgiSdfByte(bi + o);
    }
    return sum;
}

// get_omni_attenuation.
fn sdfgiOmniAttenuation(distance: f32, invRange: f32, decay: f32) -> f32 {
    var nd = distance * invRange;
    nd *= nd;
    nd *= nd;
    nd = max(1.0 - nd, 0.0);
    nd *= nd;
    return nd * pow(max(distance, 0.0001), -decay);
}

// integrate_edge_hill(...).y, as integrate_edge inlines it.
fn sdfgiEdgeHill(p0: vec3<f32>, p1: vec3<f32>) -> f32 {
    let x = dot(p0, p1);
    let y = abs(x);
    let a = 5.42031 + (3.12829 + 0.0902326 * y) * y;
    let b = 3.45068 + (4.18814 + y) * y;
    var thetaSintheta = a / b;
    if (x < 0.0) {
        thetaSintheta = 3.14159265359 * inverseSqrt(1.0 - x * x) - thetaSintheta;
    }
    return (thetaSintheta * cross(p0, p1)).y;
}

fn sdfgiIntegrateEdge(pProj0: vec3<f32>, pProj1: vec3<f32>, p0: vec3<f32>, p1: vec3<f32>) -> f32 {
    if (dot(pProj0, pProj1) < -1.0 + 0.00001) {
        let halfPoint = normalize(p0 + normalize(p1 - p0) * dot(p0, normalize(p0 - p1)));
        return sdfgiEdgeHill(pProj0, halfPoint) + sdfgiEdgeHill(halfPoint, pProj1);
    }
    return sdfgiEdgeHill(pProj0, pProj1);
}

fn sdfgiClipLerp(a: vec3<f32>, b: vec3<f32>) -> vec3<f32> {
    return -a.y * b + b.y * a;
}

struct SdfgiQuad {
    l: array<vec3<f32>, 5>,
    n: i32,
}

// clip_quad_to_horizon.
fn sdfgiClipQuad(q: SdfgiQuad) -> SdfgiQuad {
    var l = q.l;
    var config = 0;
    if (l[0].y > 0.0) { config += 1; }
    if (l[1].y > 0.0) { config += 2; }
    if (l[2].y > 0.0) { config += 4; }
    if (l[3].y > 0.0) { config += 8; }
    var n = 0;
    switch (config) {
        case 1: { n = 3; l[1] = sdfgiClipLerp(l[1], l[0]); l[2] = sdfgiClipLerp(l[3], l[0]); }
        case 2: { n = 3; l[0] = sdfgiClipLerp(l[0], l[1]); l[2] = sdfgiClipLerp(l[2], l[1]); }
        case 3: { n = 4; l[2] = sdfgiClipLerp(l[2], l[1]); l[3] = sdfgiClipLerp(l[3], l[0]); }
        case 4: { n = 3; l[0] = sdfgiClipLerp(l[3], l[2]); l[1] = sdfgiClipLerp(l[1], l[2]); }
        case 6: { n = 4; l[0] = sdfgiClipLerp(l[0], l[1]); l[3] = sdfgiClipLerp(l[3], l[2]); }
        case 7: { n = 5; l[4] = sdfgiClipLerp(l[3], l[0]); l[3] = sdfgiClipLerp(l[3], l[2]); }
        case 8: { n = 3; l[0] = sdfgiClipLerp(l[0], l[3]); l[1] = sdfgiClipLerp(l[2], l[3]); l[2] = l[3]; }
        case 9: { n = 4; l[1] = sdfgiClipLerp(l[1], l[0]); l[2] = sdfgiClipLerp(l[2], l[3]); }
        case 11: { n = 5; l[4] = l[3]; l[3] = sdfgiClipLerp(l[2], l[3]); l[2] = sdfgiClipLerp(l[2], l[1]); }
        case 12: { n = 4; l[1] = sdfgiClipLerp(l[1], l[2]); l[0] = sdfgiClipLerp(l[0], l[3]); }
        case 13: { n = 5; l[4] = l[3]; l[3] = l[2]; l[2] = sdfgiClipLerp(l[1], l[2]); l[1] = sdfgiClipLerp(l[1], l[0]); }
        case 14: { n = 5; l[4] = sdfgiClipLerp(l[0], l[3]); l[0] = sdfgiClipLerp(l[0], l[1]); }
        case 15: { n = 4; }
        default: { n = 0; }
    }
    if (n == 3) { l[3] = l[0]; }
    if (n == 4) { l[4] = l[0]; }
    return SdfgiQuad(l, n);
}

fn sdfgiSafeNormalize(v: vec3<f32>) -> vec3<f32> {
    let len = length(v);
    return select(vec3<f32>(0.0), v / len, len > 0.0);
}

// ltc_evaluate_diff without a projector texture; not divided by 2 pi.
fn sdfgiLtcDiffuse(normal: vec3<f32>, points: array<vec3<f32>, 4>) -> f32 {
    let eyeVec = select(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 0.0, -1.0), abs(normal.z) < 0.7);
    let z = -normalize(eyeVec - normal * dot(eyeVec, normal));
    let x = cross(normal, z);
    var q: SdfgiQuad;
    for (var i = 0; i < 4; i++) {
        q.l[i] = vec3<f32>(dot(x, points[i]), dot(normal, points[i]), dot(z, points[i]));
    }
    q.l[4] = vec3<f32>(0.0);
    q = sdfgiClipQuad(q);
    if (q.n == 0) {
        return 0.0;
    }
    var proj: array<vec3<f32>, 5>;
    for (var i = 0; i < 5; i++) {
        proj[i] = sdfgiSafeNormalize(q.l[i]);
    }
    let pnorm = sdfgiSafeNormalize(cross(proj[0] - proj[1], proj[2] - proj[1]));
    if (abs(dot(pnorm, proj[0])) < 1e-10) {
        return 0.0;
    }
    var integral = sdfgiIntegrateEdge(proj[0], proj[1], q.l[0], q.l[1]);
    integral += sdfgiIntegrateEdge(proj[1], proj[2], q.l[1], q.l[2]);
    integral += sdfgiIntegrateEdge(proj[2], proj[3], q.l[2], q.l[3]);
    if (q.n >= 4) {
        integral += sdfgiIntegrateEdge(proj[3], proj[4], q.l[3], q.l[4]);
    }
    if (q.n == 5) {
        integral += sdfgiIntegrateEdge(proj[4], proj[0], q.l[4], q.l[0]);
    }
    return abs(integral);
}

struct SdfgiLight {
    kind: u32,
    position: vec3<f32>,
    direction: vec3<f32>,
    radius: f32,
    areaWidth: vec3<f32>,
    attenuation: f32,
    areaHeight: vec3<f32>,
    invSpotAttenuation: f32,
    cosSpotAngle: f32,
}

struct SdfgiLightAt {
    valid: bool,
    direction: vec3<f32>,
    distance: f32,
    attenuation: f32,
}

// The light switch of sdfgi_direct_light.glsl's main, for one light.
fn sdfgiLightAt(light: SdfgiLight, position: vec3<f32>) -> SdfgiLightAt {
    var out = SdfgiLightAt(false, vec3<f32>(0.0), 1e20, 1.0);
    if (light.kind == 0u) {
        out.direction = -light.direction;
    } else if (light.kind == 1u || light.kind == 2u) {
        let rel = light.position - position;
        out.direction = normalize(rel);
        out.distance = length(rel);
        out.attenuation = sdfgiOmniAttenuation(out.distance, 1.0 / light.radius, light.attenuation);
        if (light.kind == 2u) {
            let cosAngle = dot(-out.direction, light.direction);
            if (cosAngle < light.cosSpotAngle) {
                return out;
            }
            let scos = max(cosAngle, light.cosSpotAngle);
            let spotRim = max(0.0001, (1.0 - scos) / (1.0 - light.cosSpotAngle));
            out.attenuation *= 1.0 - pow(spotRim, light.invSpotAttenuation);
        }
    } else {
        if (dot(light.areaWidth, light.areaWidth) < 1e-7 || dot(light.areaHeight, light.areaHeight) < 1e-7) {
            return out;
        }
        if (dot(light.direction, position - light.position) <= 0.0) {
            return out;
        }
        // compute_area_light.
        let aLen = length(light.areaWidth);
        let bLen = length(light.areaHeight);
        let wn = normalize(light.areaWidth);
        let hn = normalize(light.areaHeight);
        let toVert = position - light.position;
        let local = vec3<f32>(dot(toVert, wn), dot(toVert, hn), dot(toVert, -light.direction));
        let closest = light.position + clamp(local.x, -aLen / 2.0, aLen / 2.0) * wn + clamp(local.y, -bLen / 2.0, bLen / 2.0) * hn;
        let rel = closest - position;
        out.distance = length(rel);
        out.direction = rel / out.distance;
        if (out.distance < 1e-4) {
            out.direction = light.direction;
        }
        if (out.distance * light.invSpotAttenuation >= 1.0) {
            out.attenuation = 0.0;
        } else {
            let hw = light.areaWidth / 2.0;
            let hh = light.areaHeight / 2.0;
            let points = array<vec3<f32>, 4>(light.position - hw - hh - position, light.position + hw - hh - position, light.position + hw + hh - position, light.position - hw + hh - position);
            out.attenuation = sdfgiOmniAttenuation(out.distance, 1.0 / light.radius, light.attenuation - 2.0) * sdfgiLtcDiffuse(out.direction, points);
        }
    }
    out.valid = out.attenuation >= 0.001;
    return out;
}

struct SdfgiMarch {
    hit: bool,
    steps: u32,
    advance: f32,
}

// The shadow march, in one cascade (offset, toCell): from the cell's centre,
// biased toward the light; a hit below 0.001 cell.
fn sdfgiShadowRay(offset: vec3<f32>, toCell: f32, position: vec3<f32>, direction: vec3<f32>, lightDistance: f32) -> SdfgiMarch {
    var out = SdfgiMarch(false, 0u, 0.0);
    let cellSize = 1.0 / toCell;
    var rayPos = position + sign(direction) * cellSize * 0.48;
    rayPos += direction * 0.4 * cellSize;
    let invDir = vec3<f32>(1.0) / direction;
    let pos = (rayPos - offset) * toCell;
    let localDistance = lightDistance * toCell;
    if (any(pos < vec3<f32>(0.0)) || any(pos >= vec3<f32>(SDFGI_GRID))) {
        return out;
    }
    let t0 = -pos * invDir;
    let t1 = (vec3<f32>(SDFGI_GRID) - pos) * invDir;
    let tmax = max(t0, t1);
    let maxAdvance = min(min(tmax.x, min(tmax.y, tmax.z)), localDistance);
    var advance = 0.0;
    // A cap the engine does not have (its loop has none), so a test cannot hang; the selftest checks no ray reaches it.
    while (advance < maxAdvance && out.steps < 4096u) {
        let distance = sdfgiSampleSdf((pos + direction * advance) / SDFGI_GRID) * 255.0 - 1.0;
        out.steps++;
        if (distance < 0.001) {
            out.hit = true;
            break;
        }
        advance += distance;
    }
    out.advance = advance;
    return out;
}

// vogel_hemisphere.
fn sdfgiVogel(index: u32, count: u32, offset: f32) -> vec3<f32> {
    let pi = 3.14159265;
    let goldenAngle = pi * (3.0 - sqrt(5.0));
    let r = sqrt(f32(index) + 0.5) / sqrt(f32(count));
    let theta = f32(index) * goldenAngle + offset;
    let y = cos(r * pi * 0.5);
    let l = sin(r * pi * 0.5);
    return vec3<f32>(l * cos(theta), l * sin(theta), y * (f32(index & 1u) * 2.0 - 1.0));
}

fn sdfgiHash3(xIn: vec3<u32>) -> vec3<u32> {
    var x = xIn;
    x = ((x >> vec3<u32>(16u)) ^ x) * 0x45d9f3bu;
    x = ((x >> vec3<u32>(16u)) ^ x) * 0x45d9f3bu;
    x = (x >> vec3<u32>(16u)) ^ x;
    return x;
}

fn sdfgiHashf3(co: vec3<f32>) -> f32 {
    return fract(sin(dot(co, vec3<f32>(12.9898, 78.233, 137.13451))) * 43758.5453);
}

// The probe march, in one cascade: a hit below 0.05 cell.
fn sdfgiProbeRay(offset: vec3<f32>, toCell: f32, probePos: vec3<f32>, dir: vec3<f32>, bias: f32) -> SdfgiMarch {
    var out = SdfgiMarch(false, 0u, 0.0);
    let absDir = abs(dir);
    let rayPos = probePos + dir * 1.0 / max(absDir.x, max(absDir.y, absDir.z)) * bias / toCell;
    let invDir = vec3<f32>(1.0) / dir;
    let pos = (rayPos - offset) * toCell;
    if (any(pos < vec3<f32>(0.0)) || any(pos >= vec3<f32>(SDFGI_GRID))) {
        return out;
    }
    let t0 = -pos * invDir;
    let t1 = (vec3<f32>(SDFGI_GRID) - pos) * invDir;
    let tmax = max(t0, t1);
    let maxAdvance = min(tmax.x, min(tmax.y, tmax.z));
    var advance = 0.0;
    // A cap the engine does not have (its loop has none), so a test cannot hang; the selftest checks no ray reaches it.
    while (advance < maxAdvance && out.steps < 4096u) {
        let distance = sdfgiSampleSdf((pos + dir * advance) / SDFGI_GRID) * 255.0 - 1.0;
        out.steps++;
        if (distance < 0.05) {
            out.hit = true;
            break;
        }
        advance += distance;
    }
    out.advance = advance;
    return out;
}

// The 16 real SH basis values of the integrate shader.
fn sdfgiShBasis(d: vec3<f32>, i: u32) -> f32 {
    let d2 = d * d;
    switch (i) {
        case 0u: { return 0.282095; }
        case 1u: { return 0.488603 * d.y; }
        case 2u: { return 0.488603 * d.z; }
        case 3u: { return 0.488603 * d.x; }
        case 4u: { return 1.092548 * d.x * d.y; }
        case 5u: { return 1.092548 * d.y * d.z; }
        case 6u: { return 0.315392 * (3.0 * d2.z - 1.0); }
        case 7u: { return 1.092548 * d.x * d.z; }
        case 8u: { return 0.546274 * (d2.x - d2.y); }
        case 9u: { return 0.590043 * d.y * (3.0 * d2.x - d2.y); }
        case 10u: { return 2.890611 * d.y * d.x * d.z; }
        case 11u: { return 0.646360 * d.y * (-1.0 + 5.0 * d2.z); }
        case 12u: { return 0.373176 * (5.0 * d2.z * d.z - 3.0 * d.z); }
        case 13u: { return 0.457045 * d.x * (-1.0 + 5.0 * d2.z); }
        case 14u: { return 1.445305 * (d2.x - d2.y) * d.z; }
        default: { return 0.590043 * d.x * (d2.x - 3.0 * d2.y); }
    }
}

fn sdfgiLMult(i: u32) -> f32 {
    if (i == 0u) { return 1.0; }
    if (i < 4u) { return 2.0 / 3.0; }
    if (i < 9u) { return 0.25; }
    return 0.0;
}

// The integrate shader's octahedron_encode (map 0..1 to a direction).
fn sdfgiOctahedronDecode(fIn: vec2<f32>) -> vec3<f32> {
    let f = fIn * 2.0 - vec2<f32>(1.0);
    var n = vec3<f32>(f.x, f.y, 1.0 - abs(f.x) - abs(f.y));
    let t = clamp(-n.z, 0.0, 1.0);
    n.x += select(t, -t, n.x >= 0.0);
    n.y += select(t, -t, n.y >= 0.0);
    return normalize(n);
}

// rgbe_encode (E5B9G9R9).
fn sdfgiRgbeEncode(color: vec3<f32>) -> u32 {
    let c = clamp(color, vec3<f32>(0.0), vec3<f32>(65408.0));
    let cMax = max(c.r, max(c.g, c.b));
    let expp = max(-15.0 - 1.0, floor(log(cMax) / 0.6931471805599453)) + 1.0 + 15.0;
    let sMax = floor(cMax / pow(2.0, expp - 15.0 - 9.0) + 0.5);
    var exps = expp + 1.0;
    if (0.0 <= sMax && sMax < 512.0) {
        exps = expp;
    }
    let s = floor(c / pow(2.0, exps - 15.0 - 9.0) + vec3<f32>(0.5));
    return (u32(s.r) & 0x1FFu) | ((u32(s.g) & 0x1FFu) << 9u) | ((u32(s.b) & 0x1FFu) << 18u) | ((u32(exps) & 0x1Fu) << 27u);
}

// ---------------------------------------------------------------------------
// Verification kernels (selftest). Output words in gSdfgiOut.

// Input, seven vectors per test: (cell position, kind), (light position,
// radius), (light direction, attenuation), (area width, inverse spot
// attenuation), (area height, cos spot angle), (cascade offset, to_cell),
// unused. Output, 8 words: attenuation, direction, distance, valid, the
// shadow march's hit, its steps.
@compute @workgroup_size(1)
fn sdfgiLightTests(@builtin(workgroup_id) id: vec3<u32>) {
    let base = id.x * 7u;
    let a = gSdfgiTests[base];
    let b = gSdfgiTests[base + 1u];
    let c = gSdfgiTests[base + 2u];
    let d = gSdfgiTests[base + 3u];
    let e = gSdfgiTests[base + 4u];
    let f = gSdfgiTests[base + 5u];
    let light = SdfgiLight(u32(a.w), b.xyz, c.xyz, b.w, d.xyz, c.w, e.xyz, d.w, e.w);
    let at = sdfgiLightAt(light, a.xyz);
    var march = SdfgiMarch(false, 0u, 0.0);
    if (at.valid) {
        march = sdfgiShadowRay(f.xyz, f.w, a.xyz, at.direction, at.distance);
    }
    let o = id.x * 8u;
    gSdfgiOut[o] = bitcast<u32>(at.attenuation);
    gSdfgiOut[o + 1u] = bitcast<u32>(at.direction.x);
    gSdfgiOut[o + 2u] = bitcast<u32>(at.direction.y);
    gSdfgiOut[o + 3u] = bitcast<u32>(at.direction.z);
    gSdfgiOut[o + 4u] = bitcast<u32>(at.distance);
    gSdfgiOut[o + 5u] = u32(at.valid);
    gSdfgiOut[o + 6u] = u32(march.hit);
    gSdfgiOut[o + 7u] = march.steps;
}

// Input, three vectors per test: (probe position, ray index), (cascade
// offset, to_cell), (ray offset, ray total, y_mult, bias); and the probe's
// world cell in a fourth (for the hash). Output, 8 words: the ray's
// direction, hit, steps, advance, and the hash's offset.
@compute @workgroup_size(1)
fn sdfgiProbeTests(@builtin(workgroup_id) id: vec3<u32>) {
    let base = id.x * 4u;
    let a = gSdfgiTests[base];
    let b = gSdfgiTests[base + 1u];
    let c = gSdfgiTests[base + 2u];
    let w = gSdfgiTests[base + 3u];
    let h3 = sdfgiHash3(vec3<u32>(vec3<i32>(w.xyz)));
    let hashed = sdfgiHashf3(vec3<f32>(h3 & vec3<u32>(0xFFFFFu)));
    var dir = sdfgiVogel(u32(a.w), u32(c.y), c.x);
    dir.y *= c.z;
    dir = normalize(dir);
    let march = sdfgiProbeRay(b.xyz, b.w, a.xyz, dir, c.w);
    let o = id.x * 8u;
    gSdfgiOut[o] = bitcast<u32>(dir.x);
    gSdfgiOut[o + 1u] = bitcast<u32>(dir.y);
    gSdfgiOut[o + 2u] = bitcast<u32>(dir.z);
    gSdfgiOut[o + 3u] = u32(march.hit);
    gSdfgiOut[o + 4u] = march.steps;
    gSdfgiOut[o + 5u] = bitcast<u32>(march.advance);
    gSdfgiOut[o + 6u] = bitcast<u32>(hashed);
    gSdfgiOut[o + 7u] = 0u;
}

// Input, 17 vectors per test: the 16 mean SH coefficients (rgb), then
// (local texel x, y). Output, 8 words: irradiance, radiance, and their
// E5B9G9R9 words.
@compute @workgroup_size(1)
fn sdfgiStoreTests(@builtin(workgroup_id) id: vec3<u32>) {
    let base = id.x * 17u;
    let local = gSdfgiTests[base + 16u].xy;
    let normal = sdfgiOctahedronDecode(local / 6.0);
    var irradiance = vec3<f32>(0.0);
    var radiance = vec3<f32>(0.0);
    for (var i = 0u; i < 16u; i++) {
        let m = gSdfgiTests[base + i].xyz * sdfgiShBasis(normal, i) * 4.0;
        irradiance += m * sdfgiLMult(i);
        radiance += m;
    }
    let o = id.x * 8u;
    gSdfgiOut[o] = bitcast<u32>(irradiance.x);
    gSdfgiOut[o + 1u] = bitcast<u32>(irradiance.y);
    gSdfgiOut[o + 2u] = bitcast<u32>(irradiance.z);
    gSdfgiOut[o + 3u] = bitcast<u32>(radiance.x);
    gSdfgiOut[o + 4u] = bitcast<u32>(radiance.y);
    gSdfgiOut[o + 5u] = bitcast<u32>(radiance.z);
    gSdfgiOut[o + 6u] = sdfgiRgbeEncode(irradiance);
    gSdfgiOut[o + 7u] = sdfgiRgbeEncode(radiance);
}
