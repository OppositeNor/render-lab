// Godot's VoxelGI light passes (voxel_gi.glsl: MODE_COMPUTE_LIGHT's
// compute_light_at_pos, raymarch, and compute_area_light, with
// area_lights_inc.glsl's ltc_evaluate_diff; MODE_SECOND_BOUNCE's cone),
// ported to WGSL for the selftest, which runs them against their CPU mirror
// (light.rs) through the kernels at the end. Composed after
// voxelgi_shading.wgsl. Positions are in cells.
//
// Ported from Godot Engine (MIT; Copyright (c) 2014-present Godot Engine
// contributors, Copyright (c) 2007-2014 Juan Linietsky, Ariel Manzur;
// THIRD_PARTY.md).

fn voxelgiSdfByte(p: vec3<i32>) -> f32 {
    let s = vec3<i32>(gShading.bounds);
    let c = clamp(p, vec3<i32>(0), s - vec3<i32>(1));
    let i = u32(c.x + c.y * s.x + c.z * s.x * s.y);
    let word = voxelgiWord(u32(gShading.sdfOffset) + i / 4u);
    return f32((word >> ((i % 4u) * 8u)) & 0xFFu) / 255.0;
}

// texture(sampler3D(texture_sdf, texture_sampler), uvw).r: trilinear, clamped.
fn voxelgiSampleSdf(uvw: vec3<f32>) -> f32 {
    let t = uvw * gShading.bounds - vec3<f32>(0.5);
    let b = floor(t);
    let f = t - b;
    let bi = vec3<i32>(b);
    var sum = 0.0;
    for (var i = 0; i < 8; i++) {
        let o = vec3<i32>(i & 1, (i >> 1u) & 1, (i >> 2u) & 1);
        let w = select(vec3<f32>(1.0) - f, f, o == vec3<i32>(1));
        sum += w.x * w.y * w.z * voxelgiSdfByte(bi + o);
    }
    return sum;
}

fn voxelgiMod(x: f32, y: f32) -> f32 {
    return x - y * floor(x / y);
}

// raymarch.
fn voxelgiRaymarch(distanceIn: f32, distanceAdv: f32, fromIn: vec3<f32>, direction: vec3<f32>) -> f32 {
    let cellSize = vec3<f32>(1.0) / gShading.bounds;
    var occlusion = 1.0;
    var distance = distanceIn;
    var at = fromIn;
    while (distance > 0.5) {
        var advance = voxelgiSampleSdf(at * cellSize) * 255.0 - 1.0;
        if (advance < 0.0) {
            occlusion = 0.0;
            break;
        }
        occlusion = min(advance, occlusion);
        advance = max(distanceAdv, advance - voxelgiMod(advance, distanceAdv));
        at += direction * advance;
        distance -= advance;
    }
    return occlusion;
}

// get_omni_attenuation.
fn voxelgiOmniAttenuation(distance: f32, invRange: f32, decay: f32) -> f32 {
    var nd = distance * invRange;
    nd *= nd;
    nd *= nd;
    nd = max(1.0 - nd, 0.0);
    nd *= nd;
    return nd * pow(max(distance, 0.0001), -decay);
}

struct VoxelgiLight {
    kind: u32,
    energy: f32,
    radius: f32,
    attenuation: f32,
    color: vec3<f32>,
    cosSpotAngle: f32,
    position: vec3<f32>,
    invSpotAttenuation: f32,
    direction: vec3<f32>,
    hasShadow: bool,
    areaWidth: vec3<f32>,
    areaHeight: vec3<f32>,
}

// get_normal_advance.
fn voxelgiNormalAdvance(normal: vec3<f32>) -> f32 {
    let u = abs(normal);
    var axis = vec3<f32>(1.0, 0.0, 0.0);
    if (u.x >= u.y && u.x >= u.z) {
        axis = vec3<f32>(select(-1.0, 1.0, normal.x > 0.0), 0.0, 0.0);
    } else if (u.y > u.x && u.y >= u.z) {
        axis = vec3<f32>(0.0, select(-1.0, 1.0, normal.y > 0.0), 0.0);
    } else if (u.z > u.x && u.z > u.y) {
        axis = vec3<f32>(0.0, 0.0, select(-1.0, 1.0, normal.z > 0.0));
    }
    return 1.0 / dot(normal, axis);
}

// clip_segment.
fn voxelgiClipSegment(plane: vec4<f32>, begin: vec3<f32>, end: vec3<f32>) -> vec3<f32> {
    let segment = begin - end;
    let den = dot(plane.xyz, segment);
    if (den < 0.0001) {
        return end;
    }
    let dist = (dot(plane.xyz, begin) - plane.w) / den;
    if (dist < 0.0001 || dist > 1.0001) {
        return end;
    }
    return begin + segment * -dist;
}

struct VoxelgiLightAt {
    valid: bool,
    light: vec3<f32>,
    lightDir: vec3<f32>,
}

// compute_light_vector and compute_light_at_pos.
fn voxelgiLightAtPos(l: VoxelgiLight, pos: vec3<f32>, normal: vec3<f32>, cellSize: f32) -> VoxelgiLightAt {
    var out: VoxelgiLightAt;
    out.valid = false;
    var attenuation = 1.0;
    var lightPos = pos - l.direction * length(gShading.bounds);
    if (l.kind != 0u) {
        lightPos = l.position;
        let distance = length(pos - lightPos);
        if (distance >= l.radius) {
            return out;
        }
        attenuation = voxelgiOmniAttenuation(distance * cellSize, 1.0 / (l.radius * cellSize), l.attenuation);
        if (l.kind == 2u) {
            let rel = normalize(pos - lightPos);
            let cosAngle = dot(rel, l.direction);
            if (cosAngle < l.cosSpotAngle) {
                return out;
            }
            let scos = max(cosAngle, l.cosSpotAngle);
            let spotRim = max(0.0001, (1.0 - scos) / (1.0 - l.cosSpotAngle));
            attenuation *= 1.0 - pow(spotRim, l.invSpotAttenuation);
        }
    }
    let lightDir = normalize(pos - lightPos);
    if (attenuation < 0.01 || (length(normal) > 0.2 && dot(normal, lightDir) >= 0.0)) {
        return out;
    }
    if (l.hasShadow) {
        let distanceAdv = voxelgiNormalAdvance(lightDir);
        var to = pos;
        if (length(normal) > 0.2) {
            to += normal * distanceAdv * 0.51;
        } else {
            to -= sign(lightDir) * 0.45;
        }
        let limits = gShading.bounds;
        lightPos = voxelgiClipSegment(select(vec4<f32>(-1.0, 0.0, 0.0, 0.0), vec4<f32>(1.0, 0.0, 0.0, limits.x - 1.0), lightDir.x < 0.0), to, lightPos);
        lightPos = voxelgiClipSegment(select(vec4<f32>(0.0, -1.0, 0.0, 0.0), vec4<f32>(0.0, 1.0, 0.0, limits.y - 1.0), lightDir.y < 0.0), to, lightPos);
        lightPos = voxelgiClipSegment(select(vec4<f32>(0.0, 0.0, -1.0, 0.0), vec4<f32>(0.0, 0.0, 1.0, limits.z - 1.0), lightDir.z < 0.0), to, lightPos);
        var distance = length(to - lightPos);
        if (distance < 0.1) {
            return out;
        }
        distance += distanceAdv - voxelgiMod(distance, distanceAdv);
        lightPos = to - lightDir * distance;
        let occlusion = voxelgiRaymarch(distance, distanceAdv, lightPos, lightDir);
        if (occlusion == 0.0) {
            return out;
        }
        attenuation *= occlusion;
    }
    out.valid = true;
    out.light = l.color * attenuation * l.energy;
    out.lightDir = lightDir;
    return out;
}

// integrate_edge_hill(...).y, as integrate_edge inlines it.
fn voxelgiEdgeHill(p0: vec3<f32>, p1: vec3<f32>) -> f32 {
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

fn voxelgiIntegrateEdge(pProj0: vec3<f32>, pProj1: vec3<f32>, p0: vec3<f32>, p1: vec3<f32>) -> f32 {
    if (dot(pProj0, pProj1) < -1.0 + 0.00001) {
        let halfPoint = normalize(p0 + normalize(p1 - p0) * dot(p0, normalize(p0 - p1)));
        return voxelgiEdgeHill(pProj0, halfPoint) + voxelgiEdgeHill(halfPoint, pProj1);
    }
    return voxelgiEdgeHill(pProj0, pProj1);
}

fn voxelgiClipLerp(a: vec3<f32>, b: vec3<f32>) -> vec3<f32> {
    return -a.y * b + b.y * a;
}

struct VoxelgiQuad {
    l: array<vec3<f32>, 5>,
    n: i32,
}

// clip_quad_to_horizon.
fn voxelgiClipQuad(q: VoxelgiQuad) -> VoxelgiQuad {
    var l = q.l;
    var config = 0;
    if (l[0].y > 0.0) { config += 1; }
    if (l[1].y > 0.0) { config += 2; }
    if (l[2].y > 0.0) { config += 4; }
    if (l[3].y > 0.0) { config += 8; }
    var n = 0;
    switch (config) {
        case 1: { n = 3; l[1] = voxelgiClipLerp(l[1], l[0]); l[2] = voxelgiClipLerp(l[3], l[0]); }
        case 2: { n = 3; l[0] = voxelgiClipLerp(l[0], l[1]); l[2] = voxelgiClipLerp(l[2], l[1]); }
        case 3: { n = 4; l[2] = voxelgiClipLerp(l[2], l[1]); l[3] = voxelgiClipLerp(l[3], l[0]); }
        case 4: { n = 3; l[0] = voxelgiClipLerp(l[3], l[2]); l[1] = voxelgiClipLerp(l[1], l[2]); }
        case 6: { n = 4; l[0] = voxelgiClipLerp(l[0], l[1]); l[3] = voxelgiClipLerp(l[3], l[2]); }
        case 7: { n = 5; l[4] = voxelgiClipLerp(l[3], l[0]); l[3] = voxelgiClipLerp(l[3], l[2]); }
        case 8: { n = 3; l[0] = voxelgiClipLerp(l[0], l[3]); l[1] = voxelgiClipLerp(l[2], l[3]); l[2] = l[3]; }
        case 9: { n = 4; l[1] = voxelgiClipLerp(l[1], l[0]); l[2] = voxelgiClipLerp(l[2], l[3]); }
        case 11: { n = 5; l[4] = l[3]; l[3] = voxelgiClipLerp(l[2], l[3]); l[2] = voxelgiClipLerp(l[2], l[1]); }
        case 12: { n = 4; l[1] = voxelgiClipLerp(l[1], l[2]); l[0] = voxelgiClipLerp(l[0], l[3]); }
        case 13: { n = 5; l[4] = l[3]; l[3] = l[2]; l[2] = voxelgiClipLerp(l[1], l[2]); l[1] = voxelgiClipLerp(l[1], l[0]); }
        case 14: { n = 5; l[4] = voxelgiClipLerp(l[0], l[3]); l[0] = voxelgiClipLerp(l[0], l[1]); }
        case 15: { n = 4; }
        default: { n = 0; }
    }
    if (n == 3) { l[3] = l[0]; }
    if (n == 4) { l[4] = l[0]; }
    return VoxelgiQuad(l, n);
}

fn voxelgiSafeNormalize(v: vec3<f32>) -> vec3<f32> {
    let len = length(v);
    return select(vec3<f32>(0.0), v / len, len > 0.0);
}

// ltc_evaluate_diff without a projector texture; not divided by 2 pi.
fn voxelgiLtcDiffuse(normal: vec3<f32>, points: array<vec3<f32>, 4>) -> f32 {
    let eyeVec = select(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 0.0, -1.0), abs(normal.z) < 0.7);
    let z = -normalize(eyeVec - normal * dot(eyeVec, normal));
    let x = cross(normal, z);
    var q: VoxelgiQuad;
    for (var i = 0; i < 4; i++) {
        q.l[i] = vec3<f32>(dot(x, points[i]), dot(normal, points[i]), dot(z, points[i]));
    }
    q.l[4] = vec3<f32>(0.0);
    q = voxelgiClipQuad(q);
    if (q.n == 0) {
        return 0.0;
    }
    var proj: array<vec3<f32>, 5>;
    for (var i = 0; i < 5; i++) {
        proj[i] = voxelgiSafeNormalize(q.l[i]);
    }
    let pnorm = voxelgiSafeNormalize(cross(proj[0] - proj[1], proj[2] - proj[1]));
    if (abs(dot(pnorm, proj[0])) < 1e-10) {
        return 0.0;
    }
    var integral = voxelgiIntegrateEdge(proj[0], proj[1], q.l[0], q.l[1]);
    integral += voxelgiIntegrateEdge(proj[1], proj[2], q.l[1], q.l[2]);
    integral += voxelgiIntegrateEdge(proj[2], proj[3], q.l[2], q.l[3]);
    if (q.n >= 4) {
        integral += voxelgiIntegrateEdge(proj[3], proj[4], q.l[3], q.l[4]);
    }
    if (q.n == 5) {
        integral += voxelgiIntegrateEdge(proj[4], proj[0], q.l[4], q.l[0]);
    }
    return abs(integral);
}

// compute_area_light: no shadow march.
fn voxelgiAreaLight(l: VoxelgiLight, pos: vec3<f32>, normalIn: vec3<f32>, cellSize: f32) -> VoxelgiLightAt {
    var out: VoxelgiLightAt;
    out.valid = false;
    let w = l.areaWidth;
    let h = l.areaHeight;
    if (dot(w, w) < 1e-7 || dot(h, h) < 1e-7) {
        return out;
    }
    if (dot(l.direction, pos - l.position) <= 0.0) {
        return out;
    }
    let aLen = length(w);
    let bLen = length(h);
    let wn = normalize(w);
    let hn = normalize(h);
    let toVert = pos - l.position;
    let local = vec3<f32>(dot(toVert, wn), dot(toVert, hn), dot(toVert, -l.direction));
    let closestLocal = vec3<f32>(clamp(local.x, -aLen / 2.0, aLen / 2.0), clamp(local.y, -bLen / 2.0, bLen / 2.0), 0.0);
    let closest = l.position + closestLocal.x * wn + closestLocal.y * hn;
    let dist = length(closest - pos);
    let lightLength = max(1.0 / cellSize, dist);
    if (lightLength >= l.radius) {
        return out;
    }
    let attenuation = voxelgiOmniAttenuation(lightLength * cellSize, 1.0 / (l.radius * cellSize), l.attenuation) * lightLength * lightLength * cellSize * cellSize;
    if (attenuation < 0.01) {
        return out;
    }
    let hw = w / 2.0;
    let hh = h / 2.0;
    let points = array<vec3<f32>, 4>((l.position - hw - hh - pos) * cellSize, (l.position + hw - hh - pos) * cellSize, (l.position + hw + hh - pos) * cellSize, (l.position - hw + hh - pos) * cellSize);
    var normal = normalIn;
    if (dot(normal, normal) < 0.04) {
        normal = -l.direction;
    }
    out.valid = true;
    out.light = l.color * voxelgiLtcDiffuse(normal, points) * attenuation * l.energy;
    return out;
}

// One light's term of MODE_COMPUTE_LIGHT for a cell of albedo 1.
fn voxelgiCellLight(l: VoxelgiLight, pos: vec3<f32>, normal: vec3<f32>, cellSize: f32) -> vec3<f32> {
    if (l.kind == 3u) {
        let a = voxelgiAreaLight(l, pos, normal, cellSize);
        return select(vec3<f32>(0.0), a.light, a.valid);
    }
    let a = voxelgiLightAtPos(l, pos, normal, cellSize);
    if (!a.valid) {
        return vec3<f32>(0.0);
    }
    if (length(normal) > 0.2) {
        return max(0.0, dot(normal, -a.lightDir)) * a.light;
    }
    return a.light;
}

// MODE_SECOND_BOUNCE's march of one cone.
fn voxelgiBounceCone(pos: vec3<f32>, direction: vec3<f32>) -> vec4<f32> {
    var color = vec4<f32>(0.0);
    var dist = 1.5;
    let maxDistance = length(gShading.bounds);
    let cellSize = vec3<f32>(1.0) / gShading.bounds;
    while (dist < maxDistance && color.a < 0.95) {
        let diameter = max(1.0, 2.0 * 0.577 * dist);
        let uvw = (pos + dist * direction) * cellSize;
        let halfDiameter = diameter * 0.5;
        let scolor = voxelgiSampleLod(uvw, log2(diameter));
        let a = 1.0 - color.a;
        color += a * scolor;
        dist += halfDiameter;
    }
    return color;
}

// Input, eight vectors per test: (kind, energy, radius, attenuation),
// (color, cos_spot_angle), (position, inv_spot_attenuation), (direction,
// has_shadow), (area_width, metres per cell), (area_height, 0), (cell
// position, 0), (normal, 0). Output, 4 words: the light, and 0.
@compute @workgroup_size(1)
fn voxelgiLightTests(@builtin(workgroup_id) id: vec3<u32>) {
    let b = id.x * 8u;
    let t0 = gVoxelgiTests[b];
    let t1 = gVoxelgiTests[b + 1u];
    let t2 = gVoxelgiTests[b + 2u];
    let t3 = gVoxelgiTests[b + 3u];
    let t4 = gVoxelgiTests[b + 4u];
    let t5 = gVoxelgiTests[b + 5u];
    let l = VoxelgiLight(u32(t0.x), t0.y, t0.z, t0.w, t1.xyz, t1.w, t2.xyz, t2.w, t3.xyz, t3.w > 0.5, t4.xyz, t5.xyz);
    let light = voxelgiCellLight(l, gVoxelgiTests[b + 6u].xyz, gVoxelgiTests[b + 7u].xyz, t4.w);
    voxelgiStore(id.x * 4u, vec4<f32>(light, 0.0));
}

// Input, two vectors per test: (cell position, 0), (direction, 0). Output,
// 4 words: the cone's rgba, from the texture in gShadingData.
@compute @workgroup_size(1)
fn voxelgiBounceTests(@builtin(workgroup_id) id: vec3<u32>) {
    let c = voxelgiBounceCone(gVoxelgiTests[id.x * 2u].xyz, gVoxelgiTests[id.x * 2u + 1u].xyz);
    voxelgiStore(id.x * 4u, c);
}
