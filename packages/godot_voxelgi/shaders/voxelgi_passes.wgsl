// Godot's VoxelGI light passes (voxel_gi.glsl: MODE_COMPUTE_LIGHT's
// compute_light_at_pos, raymarch, and compute_area_light, with
// area_lights_inc.glsl's ltc_evaluate_diff; MODE_SECOND_BOUNCE's cone;
// MODE_UPDATE_MIPMAPS; MODE_WRITE_TEXTURE), ported to WGSL: the update
// passes the renderer runs when the light settings change (update.rs), and
// the selftest's kernels, which run the per-cell functions against their CPU
// mirror (light.rs). Composed after voxelgi_shading.wgsl. Positions are in
// cells.
//
// Ported from Godot Engine (MIT; Copyright (c) 2014-present Godot Engine
// contributors, Copyright (c) 2007-2014 Juan Linietsky, Ariel Manzur;
// THIRD_PARTY.md).

// texture(sampler3D(texture_sdf, texture_sampler), uvw).r: the R8 field, linear, clamped to the edge (its one
// mip is level 0).
fn voxelgiSampleSdf(uvw: vec3<f32>) -> f32 {
    return textureSampleLevel(gVoxelgiSdf, gVolumeSampler, uvw, 0.0).r;
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

// ---------------------------------------------------------------------------
// The light passes (update.rs's plan, the program's passes): one dispatch per pass
// and level, 64 cells to a workgroup, as gi.cpp:3105-3171 dispatches them.

// Each dispatch's parameters (voxel_gi.glsl's push constant, with where the
// records start in gShadingData, in vec4s): the program's group 2.
struct VoxelgiUpdate {
    // The level's first cell and its count.
    first: u32,
    count: u32,
    mip: u32,
    cells: u32,
    children: u32,
    lights: u32,
    lightCount: u32,
    pad0: u32,
    propagation: f32,
    dynamicRange: f32,
    // Metres per cell.
    cellSize: f32,
    pad1: f32,
    pad2: vec4<f32>,
}
@group(2) @binding(0) var<uniform> gVoxelgiUpdate: VoxelgiUpdate;
// The program's own (shading.toml's uses, after the scene's at bindings 0 to 2): write_buffer, each cell's float
// light (rgb), from the light pass up the levels; the lights, six vec4s each (world.rs `light_words`); the light
// texture's mip a dispatch writes (MODE_WRITE_TEXTURE); the distance field, written once from the bake's bytes.
@group(3) @binding(3) var<storage, read_write> gVoxelgiOutputs: array<vec4<f32>>;
// The same buffer, as the write pass reads it.
@group(3) @binding(3) var<storage, read> gVoxelgiOutputsIn: array<vec4<f32>>;
@group(3) @binding(4) var<storage, read> gVoxelgiLights: array<vec4<f32>>;
@group(3) @binding(5) var oVoxelgiLight: texture_storage_3d<rgba8unorm, write>;
@group(3) @binding(6) var oVoxelgiSdf: texture_storage_3d<r8unorm, write>;

// CellData: position (11, 10, 11 bits), albedo RGBA8, emission RGBE9995, normal SNORM8.
struct VoxelgiCell {
    position: vec3<f32>,
    texel: vec3<i32>,
    albedo: vec4<f32>,
    emission: vec3<f32>,
    normal: vec3<f32>,
}

fn voxelgiCell(cell: u32) -> VoxelgiCell {
    let d = bitcast<vec4<u32>>(gShadingData[gVoxelgiUpdate.cells + cell]);
    let posu = vec3<u32>(d.x & 0x7FFu, (d.x >> 11u) & 0x3FFu, d.x >> 21u);
    var c: VoxelgiCell;
    c.texel = vec3<i32>(posu);
    c.position = vec3<f32>(posu) + vec3<f32>(0.5);
    c.albedo = unpack4x8unorm(d.y);
    let m = exp2(f32(d.z >> 27u) - 15.0 - 9.0);
    c.emission = vec3<f32>(f32(d.z & 0x1FFu), f32((d.z >> 9u) & 0x1FFu), f32((d.z >> 18u) & 0x1FFu)) * m;
    c.normal = unpack4x8snorm(d.w).xyz;
    return c;
}

fn voxelgiLoadLight(i: u32) -> VoxelgiLight {
    let b = i * 6u;
    let t0 = gVoxelgiLights[b];
    let t1 = gVoxelgiLights[b + 1u];
    let t2 = gVoxelgiLights[b + 2u];
    let t3 = gVoxelgiLights[b + 3u];
    return VoxelgiLight(u32(t0.x), t0.y, t0.z, t0.w, t1.xyz, t1.w, t2.xyz, t2.w, t3.xyz, t3.w > 0.5, gVoxelgiLights[b + 4u].xyz, gVoxelgiLights[b + 5u].xyz);
}

// The shown bake's distance field (its bytes in gShadingData from the word `first`, four a word) into its R8
// texture, one texel per invocation: the upload `voxel_gi_allocate_data` makes (gi.cpp:104-112).
@compute @workgroup_size(4, 4, 4)
fn voxelgiSdfUploadCS(@builtin(global_invocation_id) id: vec3<u32>) {
    let s = vec3<u32>(gShading.bounds);
    if (any(id >= s)) {
        return;
    }
    let i = id.x + id.y * s.x + id.z * s.x * s.y;
    let word = voxelgiWord(gVoxelgiUpdate.first + i / 4u);
    textureStore(oVoxelgiSdf, id, vec4<f32>(f32((word >> ((i % 4u) * 8u)) & 0xFFu) / 255.0, 0.0, 0.0, 0.0));
}

// MODE_COMPUTE_LIGHT: every light's contribution at a leaf, times its albedo,
// plus its emission, into write_buffer.
@compute @workgroup_size(64)
fn voxelgiLightCS(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= gVoxelgiUpdate.count) {
        return;
    }
    let cell = gVoxelgiUpdate.first + id.x;
    let c = voxelgiCell(cell);
    var accum = vec3<f32>(0.0);
    for (var i = 0u; i < gVoxelgiUpdate.lightCount; i++) {
        accum += voxelgiCellLight(voxelgiLoadLight(i), c.position, c.normal, gVoxelgiUpdate.cellSize) * c.albedo.rgb;
    }
    gVoxelgiOutputs[cell] = vec4<f32>(accum + c.emission, 0.0);
}

// MODE_SECOND_BOUNCE: six cones through the texture pass 0 wrote, in the
// leaf's tangent frame, added to its direct light.
@compute @workgroup_size(64)
fn voxelgiBounceCS(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= gVoxelgiUpdate.count) {
        return;
    }
    let cell = gVoxelgiUpdate.first + id.x;
    let c = voxelgiCell(cell);
    var accum = gVoxelgiOutputs[cell].rgb;
    if (length(c.normal) > 0.2) {
        var v0 = vec3<f32>(0.0, 1.0, 0.0);
        if (abs(c.normal.z) < 0.999) {
            v0 = vec3<f32>(0.0, 0.0, 1.0);
        }
        let tangent = normalize(cross(v0, c.normal));
        let bitangent = normalize(cross(tangent, c.normal));
        var dirs = array<vec3<f32>, 6>(vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(0.866025, 0.0, 0.5), vec3<f32>(0.267617, 0.823639, 0.5), vec3<f32>(-0.700629, 0.509037, 0.5), vec3<f32>(-0.700629, -0.509037, 0.5), vec3<f32>(0.267617, -0.823639, 0.5));
        var weights = array<f32, 6>(0.25, 0.15, 0.15, 0.15, 0.15, 0.15);
        for (var i = 0; i < 6; i++) {
            let d = dirs[i];
            let direction = tangent * d.x + bitangent * d.y + c.normal * d.z;
            let color = voxelgiBounceCone(c.position, direction) * weights[i] * vec4<f32>(c.albedo.rgb, 1.0) * gVoxelgiUpdate.dynamicRange;
            accum += color.rgb;
        }
    }
    gVoxelgiOutputs[cell] = vec4<f32>(accum, 0.0);
}

// MODE_UPDATE_MIPMAPS: a parent's light, its children's summed and divided
// by mix(8, their count, propagation).
@compute @workgroup_size(64)
fn voxelgiMipmapsCS(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= gVoxelgiUpdate.count) {
        return;
    }
    let cell = gVoxelgiUpdate.first + id.x;
    let b = gVoxelgiUpdate.children + cell * 2u;
    let lo = bitcast<vec4<u32>>(gShadingData[b]);
    let hi = bitcast<vec4<u32>>(gShadingData[b + 1u]);
    let children = array<u32, 8>(lo.x, lo.y, lo.z, lo.w, hi.x, hi.y, hi.z, hi.w);
    var accum = vec3<f32>(0.0);
    var count = 0u;
    for (var i = 0; i < 8; i++) {
        let child = children[i];
        if (child == 0xFFFFFFFFu) {
            continue;
        }
        accum += gVoxelgiOutputs[child].rgb;
        count += 1u;
    }
    let divisor = mix(8.0, f32(count), gVoxelgiUpdate.propagation);
    gVoxelgiOutputs[cell] = vec4<f32>(accum / divisor, 0.0);
}

// MODE_WRITE_TEXTURE: a cell's light over the dynamic range, and its
// coverage, into its texel of its level's mip; texels without a cell stay 0.
@compute @workgroup_size(64)
fn voxelgiWriteCS(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= gVoxelgiUpdate.count) {
        return;
    }
    let cell = gVoxelgiUpdate.first + id.x;
    let c = voxelgiCell(cell);
    textureStore(oVoxelgiLight, c.texel, vec4<f32>(gVoxelgiOutputsIn[cell].rgb / gVoxelgiUpdate.dynamicRange, c.albedo.a));
}

// Input, one vector per test: (uvw, lod). Output, 4 words: the light
// texture's sample there (the sampler storage.rs's sample_lod mirrors).
@compute @workgroup_size(1)
fn voxelgiSampleTests(@builtin(workgroup_id) id: vec3<u32>) {
    let t = gVoxelgiTests[id.x];
    voxelgiStore(id.x * 4u, voxelgiSampleLod(t.xyz, t.w));
}

// ---------------------------------------------------------------------------
// Dynamic objects (dynamic_gpu.rs; gi.cpp:3177-3394): each view of an object
// drawn with its material into the first dynamic map, lit
// (MODE_DYNAMIC_LIGHTING), then shrunk map by map and plotted into the light
// texture (MODE_DYNAMIC_SHRINK). The maps are the mips of voxelgiDynLight and
// voxelgiDynDepth (Godot's dynamic_maps, one texture each).

// A draw's parameters (dynamic_gpu.rs `ViewPlan::draw`): the object, the eye
// (oversampled cells), the view's basis columns, the rectangle's width and
// height, and the box's depth along the view.
@group(2) @binding(0) var<uniform> gVoxelgiDraw: array<vec4<f32>, 4>;

// The first map's material, as `_render_material` writes it (the program's
// raster targets), and each map's parameters.
@group(3) @binding(7) var gDynAlbedo: texture_storage_2d<rgba8unorm, read>;
@group(3) @binding(8) var gDynNormal: texture_storage_2d<rgba8unorm, read>;
@group(3) @binding(9) var oDynEmission: texture_storage_2d<rgba16float, read_write>;
@group(3) @binding(10) var oDynFirstDepth: texture_storage_2d<r32float, read_write>;
@group(3) @binding(11) var<storage, read> gDynRecords: array<vec4<u32>>;
// A shrink's source map and the map it writes (one mip of the chain each), and
// the light texture's mip it plots into.
@group(3) @binding(12) var gDynSourceLight: texture_storage_2d<rgba16float, read>;
@group(3) @binding(13) var gDynSourceDepth: texture_storage_2d<r32float, read>;
@group(3) @binding(14) var oDynLight: texture_storage_2d<rgba16float, write>;
@group(3) @binding(15) var oDynDepth: texture_storage_2d<r32float, write>;
@group(3) @binding(16) var oVoxelgiPlot: texture_storage_3d<rgba8unorm, read_write>;

// VoxelGIDynamicPushConstant, and the dispatch's mode (dynamic_gpu.rs `Record`).
struct VoxelgiDynamicRecord {
    limits: vec3<i32>,
    lightCount: u32,
    xDir: vec3<i32>,
    zBase: f32,
    yDir: vec3<i32>,
    zSign: f32,
    zDir: vec3<i32>,
    posMultiplier: f32,
    rectPos: vec2<i32>,
    rectSize: vec2<i32>,
    prevRectOfs: vec2<i32>,
    prevRectSize: vec2<i32>,
    flipX: bool,
    flipY: bool,
    dynamicRange: f32,
    onMipmap: bool,
    propagation: f32,
    cellSize: f32,
    mode: u32,
}

// The record the dispatch names (its first parameter word).
fn voxelgiDynamicRecord() -> VoxelgiDynamicRecord {
    let b = gVoxelgiUpdate.first * 8u;
    let v0 = gDynRecords[b];
    let v1 = gDynRecords[b + 1u];
    let v2 = gDynRecords[b + 2u];
    let v3 = gDynRecords[b + 3u];
    let v4 = gDynRecords[b + 4u];
    let v5 = gDynRecords[b + 5u];
    let v6 = gDynRecords[b + 6u];
    let v7 = gDynRecords[b + 7u];
    return VoxelgiDynamicRecord(
        bitcast<vec3<i32>>(v0.xyz), v0.w,
        bitcast<vec3<i32>>(v1.xyz), bitcast<f32>(v1.w),
        bitcast<vec3<i32>>(v2.xyz), bitcast<f32>(v2.w),
        bitcast<vec3<i32>>(v3.xyz), bitcast<f32>(v3.w),
        bitcast<vec2<i32>>(v4.xy), bitcast<vec2<i32>>(v4.zw),
        bitcast<vec2<i32>>(v5.xy), bitcast<vec2<i32>>(v5.zw),
        v6.x != 0u, v6.y != 0u, bitcast<f32>(v6.z), v6.w != 0u,
        bitcast<f32>(v7.x), bitcast<f32>(v7.y), v7.z);
}

struct VoxelgiDynamicVaryings {
    @builtin(position) clip: vec4<f32>,
    @location(0) viewNormal: vec3<f32>,
    @location(1) viewDepth: f32,
    @location(2) @interpolate(flat) objectIndex: u32,
}

// The view's camera (`xform.set_look_at(center - size / 2 * render_dir,
// center, up)` in oversampled cells) and `cm.set_orthogonal(-w / 2, w / 2,
// -h / 2, h / 2, 0.0001, depth)`, reversed as the lab's depth is: a texel's
// column runs along the view, x to the right and y down the rows as the view's
// x and y grow (so the lighting pass's flips find each column's texel). Only
// the dynamic object is drawn (`cull_argument`).
@vertex
fn voxelgiDynamicVS(input: SceneVertex) -> VoxelgiDynamicVaryings {
    let o = gObjects[input.objectIndex];
    let d0 = gVoxelgiDraw[0];
    let d1 = gVoxelgiDraw[1];
    let d2 = gVoxelgiDraw[2];
    let d3 = gVoxelgiDraw[3];
    let c0 = vec3<f32>(d1.x, d1.y, d1.z);
    let c1 = vec3<f32>(d1.w, d2.x, d2.y);
    let c2 = vec3<f32>(d2.z, d2.w, d3.x);
    let world = transformPoint(o.localToWorld, input.position);
    let over = (world - gShading.toCellOrigin) * gShading.toCellScale * 16.0;
    let rel = over - d0.yzw;
    let v = vec3<f32>(dot(c0, rel), dot(c1, rel), dot(c2, rel));
    let n = normalToWorld(o, input.normal);
    var out: VoxelgiDynamicVaryings;
    out.viewNormal = vec3<f32>(dot(c0, n), dot(c1, n), dot(c2, n));
    out.viewDepth = -v.z;
    out.objectIndex = input.objectIndex;
    let near = 0.0001;
    out.clip = vec4<f32>(v.x / (d3.y * 0.5), -v.y / (d3.z * 0.5), 1.0 - (-v.z - near) / (d3.w - near), 1.0);
    if (input.objectIndex != bitcast<u32>(d0.x)) {
        out.clip = vec4<f32>(2.0, 2.0, 2.0, 1.0);
    }
    return out;
}

struct VoxelgiDynamicMaterial {
    @location(0) albedo: vec4<f32>,
    @location(1) normal: vec4<f32>,
    @location(2) orm: vec4<f32>,
    @location(3) emission: vec4<f32>,
    @location(4) depth: vec4<f32>,
}

// MODE_RENDER_MATERIAL's outputs (scene_forward_clustered.glsl:3001-3017):
// albedo and alpha; the view's normal, made to touch the unit cube (encode24
// without its best-fit scale), mapped to 0..1; ambient occlusion, roughness,
// metallic, subsurface strength; emission; the depth along the view.
@fragment
fn voxelgiDynamicFS(input: VoxelgiDynamicVaryings) -> VoxelgiDynamicMaterial {
    let o = gObjects[input.objectIndex];
    let n = normalize(input.viewNormal);
    let cube = n / max(abs(n.z), max(abs(n.x), abs(n.y)));
    var out: VoxelgiDynamicMaterial;
    out.albedo = vec4<f32>(o.albedoRoughness.rgb, 1.0);
    out.normal = vec4<f32>(cube * 0.5 + 0.5, 0.0);
    out.orm = vec4<f32>(1.0, o.albedoRoughness.a, 0.0, 0.0);
    out.emission = vec4<f32>(o.emissive.rgb * o.emissive.w, 0.0);
    out.depth = vec4<f32>(input.viewDepth, 0.0, 0.0, 0.0);
    return out;
}

// The texel of the map that holds column `posXY` (flip_x, flip_y).
fn voxelgiDynamicUv(r: VoxelgiDynamicRecord, posXY: vec2<i32>, size: vec2<i32>) -> vec2<i32> {
    var uv = posXY;
    if (r.flipX) {
        uv.x = size.x - posXY.x - 1;
    }
    if (r.flipY) {
        uv.y = size.y - posXY.y - 1;
    }
    return uv;
}

// MODE_DYNAMIC_LIGHTING: each texel of the first map placed in the grid by its
// depth and lit like a cell (shadows from the distance field), plus its
// emission, into the emission map; its depth replaced by its place along the view.
@compute @workgroup_size(8, 8, 1)
fn voxelgiDynamicLightingCS(@builtin(global_invocation_id) id: vec3<u32>) {
    let r = voxelgiDynamicRecord();
    let posXY = vec2<i32>(id.xy);
    if (any(posXY >= r.rectSize)) {
        return;
    }
    let uv = voxelgiDynamicUv(r, posXY, r.rectSize);
    let z = r.zBase + textureLoad(oDynFirstDepth, uv).x * r.zSign;
    let pos = r.xDir * (r.rectPos.x + posXY.x) + r.yDir * (r.rectPos.y + posXY.y) + abs(r.zDir) * i32(z);
    var normal = normalize(textureLoad(gDynNormal, uv).xyz * 2.0 - 1.0);
    normal = vec3<f32>(r.xDir) * normal.x * select(1.0, -1.0, r.flipX) + vec3<f32>(r.yDir) * normal.y * select(1.0, -1.0, r.flipY) - vec3<f32>(r.zDir) * normal.z;
    let albedo = textureLoad(gDynAlbedo, uv);
    var accum = vec3<f32>(0.0);
    for (var i = 0u; i < r.lightCount; i++) {
        let l = voxelgiLoadLight(i);
        let p = vec3<f32>(pos) * r.posMultiplier;
        if (l.kind != 3u) {
            let a = voxelgiLightAtPos(l, p, normal, r.cellSize);
            if (!a.valid) {
                continue;
            }
            accum += max(0.0, dot(normal, -a.lightDir)) * a.light * albedo.rgb;
        } else {
            let a = voxelgiAreaLight(l, p, normal, r.cellSize);
            if (!a.valid) {
                continue;
            }
            accum += a.light * albedo.rgb;
        }
    }
    accum += textureLoad(oDynEmission, uv).xyz;
    textureStore(oDynEmission, uv, vec4<f32>(accum, albedo.a));
    textureStore(oDynFirstDepth, uv, vec4<f32>(z, 0.0, 0.0, 0.0));
}

// MODE_DYNAMIC_SHRINK: each texel the average of the 2 x 2 covered texels of
// the map before (a light mip divides as the mip pass does); written while a
// map follows (WRITE), and from the first light mip on plotted at its depth,
// split over two voxels and blended over the light texture (PLOT).
@compute @workgroup_size(8, 8, 1)
fn voxelgiDynamicShrinkCS(@builtin(global_invocation_id) id: vec3<u32>) {
    let r = voxelgiDynamicRecord();
    let posXY = vec2<i32>(id.xy);
    if (any(posXY >= r.rectSize)) {
        return;
    }
    let uv = voxelgiDynamicUv(r, posXY, r.rectSize);
    var accum = vec4<f32>(0.0);
    var accumZ = 0.0;
    var count = 0.0;
    for (var i = 0; i < 4; i++) {
        var ofs = posXY * 2 + vec2<i32>(i & 1, i >> 1u) - r.prevRectOfs;
        if (any(ofs < vec2<i32>(0)) || any(ofs >= r.prevRectSize)) {
            continue;
        }
        ofs = voxelgiDynamicUv(r, ofs, r.prevRectSize);
        let light = textureLoad(gDynSourceLight, ofs);
        if (light.a == 0.0) {
            continue;
        }
        accum += light;
        accumZ += textureLoad(gDynSourceDepth, ofs).x * 0.5;
        count += 1.0;
    }
    if (r.onMipmap) {
        accum = vec4<f32>(accum.rgb / mix(8.0, count, r.propagation), accum.a / 8.0);
    } else {
        accum /= 4.0;
    }
    if (count == 0.0) {
        accumZ = 0.0;
    } else {
        accumZ /= count;
    }
    if (r.mode != 3u) {
        textureStore(oDynLight, uv, accum);
        textureStore(oDynDepth, uv, vec4<f32>(accumZ, 0.0, 0.0, 0.0));
    }
    if (r.mode == 1u || accum.a < 0.001) {
        return;
    }
    let pos = r.xDir * (r.rectPos.x + posXY.x) + r.yDir * (r.rectPos.y + posXY.y) + abs(r.zDir) * i32(accumZ);
    let zFrac = fract(accumZ);
    for (var i = 0; i < 2; i++) {
        let pos3d = pos + abs(r.zDir) * i;
        if (any(pos3d < vec3<i32>(0)) || any(pos3d >= r.limits)) {
            continue;
        }
        let blit = accum * select(zFrac, 1.0 - zFrac, i == 0);
        var color = textureLoad(oVoxelgiPlot, pos3d);
        color = vec4<f32>(color.rgb * r.dynamicRange, color.a);
        let sa = 1.0 - blit.a;
        let a = color.a * sa + blit.a;
        if (a != 0.0) {
            color = vec4<f32>((color.rgb * color.a * sa + blit.rgb * blit.a) / a, a);
        }
        color = vec4<f32>(color.rgb / r.dynamicRange, color.a);
        textureStore(oVoxelgiPlot, pos3d, color);
    }
}
