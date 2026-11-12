// ReSTIR's passes (Bitterli et al. 2020, Alg. 5), as the lab program's passes
// (src/paper/restir/gpu.rs), in the paper's order: streaming RIS of each pixel's
// candidates (Alg. 3), visibility (Alg. 5 lines 7-9), temporal reuse (lines
// 11-14), n rounds of spatial reuse (lines 16-20; Alg. 4, or Alg. 6 unbiased),
// then the shading of each pixel's sample (Alg. 3, shadePixel) and the copy of
// the frame's reservoirs that the next frame reuses.
//
// Each pass binds group 0 (the frame), group 1 (the shading), its command's 16
// words at group 2, and its uses at group 3 (lab/src/program.rs). Words 0 and 1
// are the viewport, word 2 the frame's seed; the rest are the pass's own.

@group(2) @binding(0) var<uniform> gRestirParams: array<vec4<u32>, 4>;

fn restirParam(k: u32) -> u32 {
    return gRestirParams[k / 4u][k % 4u];
}

fn restirSize() -> vec2<u32> {
    return vec2<u32>(restirParam(0u), restirParam(1u));
}

fn restirSeed() -> u32 {
    return restirParam(2u);
}

// The cameras (gpu.rs `SCENE_WORDS`): this frame's clip-to-world, last frame's world-to-clip.
struct RestirScene {
    clipToWorld: mat4x4<f32>,
    lastWorldToClip: mat4x4<f32>,
    lastEye: vec4<f32>,
}

// The surface a pixel stores (restirCandidatesCS): its position and camera distance, its normal and object (-1
// where no surface is lit: the background, a light), its albedo and roughness.
fn restirSurface(p: vec4<f32>, m: vec4<f32>, a: vec4<f32>) -> Surface {
    return Surface(p.xyz, p.w, m.xyz, a.w, a.rgb, u32(max(m.w, 0.0)), m.w >= 0.0);
}

fn restirSurfaceAt(tp: texture_2d<f32>, tm: texture_2d<f32>, ta: texture_2d<f32>, q: vec2<i32>) -> Surface {
    return restirSurface(textureLoad(tp, q, 0), textureLoad(tm, q, 0), textureLoad(ta, q, 0));
}

// W from a stream's weight sum (Eq. 6, Alg. 3 line 8; Alg. 4 line 6): the sum over p̂ and over M.
fn restirFinish(r: ptr<function, Reservoir>, pHat: f32, count: f32) {
    if (pHat > 0.0 && count > 0.0) {
        (*r).W = (*r).wsum / (pHat * count);
    } else {
        (*r).W = 0.0;
    }
}

// The biased test for a neighbour (§5, "Neighbor selection"): its camera distance within 10% of this pixel's,
// and its normal within 25 degrees.
fn restirSimilar(s: Surface, n: Surface) -> bool {
    if (!n.valid) {
        return false;
    }
    if (gShading.geometryTest < 0.5) {
        return true;
    }
    return abs(n.distance - s.distance) <= 0.1 * s.distance && dot(n.normal, s.normal) >= cos(radians(25.0));
}

// p̂ of a sample at a surface as Alg. 6 needs it (§4.4): zero where the target is, and, with visibility reuse,
// where the sample is hidden from that surface.
fn restirReachable(s: Surface, x: Sample) -> bool {
    if (restirTarget(s, x) <= 0.0) {
        return false;
    }
    return gShading.visibilityReuse < 0.5 || restirVisible(s, x);
}

// ---------------------------------------------------------------------------
// Streaming RIS (Alg. 3): each pixel's surface from the pre-pass, then M
// candidates, each a light picked by power and a uniform point on it, streamed
// through one reservoir with weight p̂ / p.

@group(3) @binding(1) var cDepth: texture_depth_multisampled_2d;
@group(3) @binding(2) var cNormal: texture_multisampled_2d<f32>;
@group(3) @binding(3) var cMaterial: texture_multisampled_2d<f32>;
@group(3) @binding(4) var<uniform> cScene: RestirScene;
@group(3) @binding(5) var oSurfP: texture_storage_2d<rgba32float, write>;
@group(3) @binding(6) var oSurfM: texture_storage_2d<rgba32float, write>;
@group(3) @binding(7) var oSurfA: texture_storage_2d<rgba32float, write>;
@group(3) @binding(8) var oInitial: texture_storage_2d<rgba32uint, write>;

// The candidates of one pixel at its surface, streamed through a reservoir (cpu.rs `candidates` mirrors it).
fn restirCandidates(s: Surface, rng: ptr<function, Rng>) -> Reservoir {
    var r = restirEmpty();
    let m = u32(gShading.candidates);
    for (var i = 0u; i < m; i++) {
        // Generate x_i ~ p: a light by power, then a point on it; its weight p̂ / p.
        let light = restirPickLight(restirRand(rng));
        let uv = (restirNext(rng) >> 16u) | (restirNext(rng) & 0xffff0000u);
        let x = Sample(light, uv);
        restirUpdate(&r, x, restirTarget(s, x) / restirSourcePdf(light), rng);
    }
    restirFinish(&r, restirTarget(s, r.y), r.M);
    return r;
}

@compute @workgroup_size(8, 8, 1)
fn restirCandidatesCS(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = restirSize();
    if (id.x >= size.x || id.y >= size.y) {
        return;
    }
    let q = vec2<i32>(id.xy);
    let depth = textureLoad(cDepth, q, 0);
    let normal = textureLoad(cNormal, q, 0);
    let material = textureLoad(cMaterial, q, 0);
    // The pixel's point, back through the projection (reversed Z: the background is at depth 0).
    let ndc = vec2<f32>((f32(q.x) + 0.5) / f32(size.x) * 2.0 - 1.0, 1.0 - (f32(q.y) + 0.5) / f32(size.y) * 2.0);
    let world = cScene.clipToWorld * vec4<f32>(ndc, depth, 1.0);
    let position = world.xyz / world.w;
    let object = u32(normal.w + 0.5);
    let lit = depth > 0.0 && object < restirFirstLightObject();
    let m = select(-1.0, f32(object), lit);
    let p = vec4<f32>(position, distance(position, gFrame.camera.position));
    textureStore(oSurfP, q, p);
    textureStore(oSurfM, q, vec4<f32>(normalize(normal.xyz), m));
    textureStore(oSurfA, q, material);
    var r = restirEmpty();
    if (lit) {
        var rng = restirRng(id.xy, restirSeed(), 0u);
        r = restirCandidates(restirSurface(p, vec4<f32>(normalize(normal.xyz), m), material), &rng);
    }
    textureStore(oInitial, q, restirPack(r));
}

// ---------------------------------------------------------------------------
// Visibility reuse (§3.2; Alg. 5 lines 7-9): a shadow ray to each pixel's
// selected sample; an occluded one keeps its M, and its W becomes 0.

@group(3) @binding(1) var vIn: texture_2d<u32>;
@group(3) @binding(2) var vSurfP: texture_2d<f32>;
@group(3) @binding(3) var vSurfM: texture_2d<f32>;
@group(3) @binding(4) var vSurfA: texture_2d<f32>;
@group(3) @binding(5) var vOut: texture_storage_2d<rgba32uint, write>;

@compute @workgroup_size(8, 8, 1)
fn restirVisibilityCS(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = restirSize();
    if (id.x >= size.x || id.y >= size.y) {
        return;
    }
    let q = vec2<i32>(id.xy);
    var r = restirUnpack(textureLoad(vIn, q, 0));
    let s = restirSurfaceAt(vSurfP, vSurfM, vSurfA, q);
    if (gShading.visibilityReuse > 0.5 && s.valid && r.y.light != RESTIR_NONE && !restirVisible(s, r.y)) {
        r.W = 0.0;
    }
    textureStore(vOut, q, restirPack(r));
}

// ---------------------------------------------------------------------------
// Combining reservoirs (Alg. 4, biased; Alg. 6, unbiased with uniform weights):
// the pixel's reservoir and others are streamed into one, each weighted
// p̂_q(r.y) · r.W · r.M; M is the sum of theirs; W is wsum over p̂_q(y) times
// the sum of M (Alg. 4) or times Z, the M of the sources at whose surface the
// sample's p̂ is not zero (Alg. 6, m = 1 / Z). Entry 0 is the pixel's own.

const RESTIR_MAX_SOURCES: u32 = 17u;

struct Sources {
    reservoirs: array<Reservoir, 17>,
    surfaces: array<Surface, 17>,
    count: u32,
}

fn restirCombine(s: Surface, sources: ptr<function, Sources>, rng: ptr<function, Rng>) -> Reservoir {
    var out = restirEmpty();
    var m = 0.0;
    for (var i = 0u; i < (*sources).count; i++) {
        let r = (*sources).reservoirs[i];
        restirUpdate(&out, r.y, restirTarget(s, r.y) * r.W * r.M, rng);
        m += r.M;
    }
    out.M = m;
    if (gShading.unbiased < 0.5) {
        restirFinish(&out, restirTarget(s, out.y), out.M);
        return out;
    }
    var z = 0.0;
    for (var i = 0u; i < (*sources).count; i++) {
        // Alg. 6 lines 7-9: only the sources that could have produced the sample count.
        if (restirReachable((*sources).surfaces[i], out.y)) {
            z += (*sources).reservoirs[i].M;
        }
    }
    restirFinish(&out, restirTarget(s, out.y), z);
    return out;
}

// ---------------------------------------------------------------------------
// Temporal reuse (Alg. 5 lines 11-14; §5): the pixel's reservoir combined
// with last frame's at the point's place in last frame's view, its M clamped
// to temporalClamp times this one's. Word 3 is 1 when there is a last frame.

@group(3) @binding(1) var tIn: texture_2d<u32>;
@group(3) @binding(2) var tPrev: texture_2d<u32>;
@group(3) @binding(3) var tSurfP: texture_2d<f32>;
@group(3) @binding(4) var tSurfM: texture_2d<f32>;
@group(3) @binding(5) var tSurfA: texture_2d<f32>;
@group(3) @binding(6) var tPrevP: texture_2d<f32>;
@group(3) @binding(7) var tPrevM: texture_2d<f32>;
@group(3) @binding(8) var tPrevA: texture_2d<f32>;
@group(3) @binding(9) var<uniform> tScene: RestirScene;
@group(3) @binding(10) var tOut: texture_storage_2d<rgba32uint, write>;

// Where a point was in last frame's view, or (-1, -1) off it.
fn restirReproject(position: vec3<f32>, size: vec2<u32>) -> vec2<i32> {
    let clip = tScene.lastWorldToClip * vec4<f32>(position, 1.0);
    if (clip.w <= 0.0) {
        return vec2<i32>(-1);
    }
    let ndc = clip.xy / clip.w;
    let pixel = vec2<f32>((ndc.x * 0.5 + 0.5) * f32(size.x), (0.5 - ndc.y * 0.5) * f32(size.y));
    if (pixel.x < 0.0 || pixel.y < 0.0 || pixel.x >= f32(size.x) || pixel.y >= f32(size.y)) {
        return vec2<i32>(-1);
    }
    return vec2<i32>(pixel);
}

@compute @workgroup_size(8, 8, 1)
fn restirTemporalCS(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = restirSize();
    if (id.x >= size.x || id.y >= size.y) {
        return;
    }
    let q = vec2<i32>(id.xy);
    let current = restirUnpack(textureLoad(tIn, q, 0));
    let s = restirSurfaceAt(tSurfP, tSurfM, tSurfA, q);
    let qPrev = restirReproject(s.position, size);
    var out = current;
    if (gShading.temporal > 0.5 && restirParam(3u) != 0u && s.valid && qPrev.x >= 0) {
        let sPrev = restirSurfaceAt(tPrevP, tPrevM, tPrevA, qPrev);
        if (restirSimilar(s, sPrev)) {
            // Last frame's reservoir there, its M at most temporalClamp times this one's (§5).
            var previous = restirUnpack(textureLoad(tPrev, qPrev, 0));
            previous.M = min(previous.M, gShading.temporalClamp * current.M);
            var sources: Sources;
            sources.reservoirs[0] = current;
            sources.surfaces[0] = s;
            sources.reservoirs[1] = previous;
            sources.surfaces[1] = sPrev;
            sources.count = 2u;
            var rng = restirRng(id.xy, restirSeed(), 1u);
            out = restirCombine(s, &sources, &rng);
        }
    }
    textureStore(tOut, q, restirPack(out));
}

// ---------------------------------------------------------------------------
// Spatial reuse (Alg. 5 lines 16-20; §5): k neighbours drawn in a disc of the
// radius around the pixel, from a low-discrepancy sequence (the R2 sequence,
// Roberts 2018, turned per pixel: the lab's choice of sequence), each skipped
// if it fails the geometry test; then the combination. Word 3 is the round.

@group(3) @binding(1) var sIn: texture_2d<u32>;
@group(3) @binding(2) var sSurfP: texture_2d<f32>;
@group(3) @binding(3) var sSurfM: texture_2d<f32>;
@group(3) @binding(4) var sSurfA: texture_2d<f32>;
@group(3) @binding(5) var sOut: texture_storage_2d<rgba32uint, write>;

// Neighbour j of pixel q in round `round` (cpu.rs `neighbour` mirrors it).
fn restirNeighbour(q: vec2<i32>, j: u32, rotation: vec2<f32>, size: vec2<u32>) -> vec2<i32> {
    let e = fract(rotation + f32(j + 1u) * vec2<f32>(0.7548776662, 0.5698402910));
    let r = gShading.radius * sqrt(e.x);
    let a = 2.0 * PI * e.y;
    let p = vec2<f32>(q) + vec2<f32>(r * cos(a), r * sin(a));
    return clamp(vec2<i32>(round(p)), vec2<i32>(0), vec2<i32>(size) - vec2<i32>(1));
}

@compute @workgroup_size(8, 8, 1)
fn restirSpatialCS(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = restirSize();
    if (id.x >= size.x || id.y >= size.y) {
        return;
    }
    let q = vec2<i32>(id.xy);
    let current = restirUnpack(textureLoad(sIn, q, 0));
    let s = restirSurfaceAt(sSurfP, sSurfM, sSurfA, q);
    if (!s.valid) {
        textureStore(sOut, q, restirPack(current));
        return;
    }
    var rng = restirRng(id.xy, restirSeed(), 2u + restirParam(3u));
    let rotation = vec2<f32>(restirRand(&rng), restirRand(&rng));
    var sources: Sources;
    sources.reservoirs[0] = current;
    sources.surfaces[0] = s;
    sources.count = 1u;
    let k = min(u32(gShading.neighbours), RESTIR_MAX_SOURCES - 1u);
    for (var j = 0u; j < k; j++) {
        // Each neighbour that passes the geometry test brings its reservoir and its surface.
        let qn = restirNeighbour(q, j, rotation, size);
        let sn = restirSurfaceAt(sSurfP, sSurfM, sSurfA, qn);
        if (all(qn == q) || !restirSimilar(s, sn)) {
            continue;
        }
        sources.reservoirs[sources.count] = restirUnpack(textureLoad(sIn, qn, 0));
        sources.surfaces[sources.count] = sn;
        sources.count += 1u;
    }
    textureStore(sOut, q, restirPack(restirCombine(s, &sources, &rng)));
}

// ---------------------------------------------------------------------------
// The frame's reservoirs kept for the next frame (Alg. 5 line 24), with the
// surfaces they were made at. Word 3 says which image holds them: 0 the
// temporal pass's, 1 resA, 2 resB.

@group(3) @binding(1) var kT: texture_2d<u32>;
@group(3) @binding(2) var kA: texture_2d<u32>;
@group(3) @binding(3) var kB: texture_2d<u32>;
@group(3) @binding(4) var kSurfP: texture_2d<f32>;
@group(3) @binding(5) var kSurfM: texture_2d<f32>;
@group(3) @binding(6) var kSurfA: texture_2d<f32>;
@group(3) @binding(7) var kPrev: texture_storage_2d<rgba32uint, write>;
@group(3) @binding(8) var kPrevP: texture_storage_2d<rgba32float, write>;
@group(3) @binding(9) var kPrevM: texture_storage_2d<rgba32float, write>;
@group(3) @binding(10) var kPrevA: texture_storage_2d<rgba32float, write>;

@compute @workgroup_size(8, 8, 1)
fn restirKeepCS(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = restirSize();
    if (id.x >= size.x || id.y >= size.y) {
        return;
    }
    let q = vec2<i32>(id.xy);
    var r: vec4<u32>;
    switch restirParam(3u) {
        case 1u: { r = textureLoad(kA, q, 0); }
        case 2u: { r = textureLoad(kB, q, 0); }
        default: { r = textureLoad(kT, q, 0); }
    }
    textureStore(kPrev, q, r);
    textureStore(kPrevP, q, textureLoad(kSurfP, q, 0));
    textureStore(kPrevM, q, textureLoad(kSurfM, q, 0));
    textureStore(kPrevA, q, textureLoad(kSurfA, q, 0));
}

// ---------------------------------------------------------------------------
// Shading (Alg. 3, shadePixel): f_q(y) · W, the sample's contribution with its
// visibility, from the reservoirs of the stage shown (gShading.stage); or a
// view of those reservoirs. While nothing moves, frames are averaged into the
// accumulation (word 3: the frames already in it). Word 4 is the image
// holding the frame's reservoirs, as restirKeepCS's word 3.

@group(3) @binding(1) var hInitial: texture_2d<u32>;
@group(3) @binding(2) var hVisible: texture_2d<u32>;
@group(3) @binding(3) var hTemporal: texture_2d<u32>;
@group(3) @binding(4) var hFinal: texture_2d<u32>;
@group(3) @binding(5) var hSurfP: texture_2d<f32>;
@group(3) @binding(6) var hSurfM: texture_2d<f32>;
@group(3) @binding(7) var hSurfA: texture_2d<f32>;
@group(3) @binding(8) var hDirect: texture_storage_2d<rgba16float, write>;
@group(3) @binding(9) var hAccumR: texture_storage_2d<r32float, read_write>;
@group(3) @binding(10) var hAccumG: texture_storage_2d<r32float, read_write>;
@group(3) @binding(11) var hAccumB: texture_storage_2d<r32float, read_write>;

// A colour for a light's index, for the view of which light each pixel samples.
fn restirLightColor(light: u32) -> vec3<f32> {
    let h = restirHash(light * 2654435761u + 1u);
    return vec3<f32>(f32(h & 255u), f32((h >> 8u) & 255u), f32((h >> 16u) & 255u)) / 255.0;
}

@compute @workgroup_size(8, 8, 1)
fn restirShadeCS(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = restirSize();
    if (id.x >= size.x || id.y >= size.y) {
        return;
    }
    let q = vec2<i32>(id.xy);
    var packed: vec4<u32>;
    switch u32(gShading.stage) {
        case 0u: { packed = textureLoad(hInitial, q, 0); }
        case 1u: { packed = textureLoad(hVisible, q, 0); }
        case 2u: { packed = textureLoad(hTemporal, q, 0); }
        default: { packed = textureLoad(hFinal, q, 0); }
    }
    let r = restirUnpack(packed);
    let s = restirSurfaceAt(hSurfP, hSurfM, hSurfA, q);
    var color = vec3<f32>(0.0);
    var visible = false;
    if (s.valid && r.y.light != RESTIR_NONE) {
        visible = restirVisible(s, r.y);
    }
    switch u32(gShading.view) {
        case 1u: {
            if (s.valid && r.y.light != RESTIR_NONE) {
                color = restirLightColor(r.y.light);
            }
        }
        case 2u: {
            // M on a logarithmic ramp: 1 dark, 1000 or more bright.
            color = distanceColor(saturate(log2(max(r.M, 1.0)) / 10.0));
        }
        case 3u: {
            color = vec3<f32>(saturate(log2(max(r.W, 1e-6)) / 20.0 + 0.5));
        }
        case 4u: {
            color = select(vec3<f32>(0.9, 0.2, 0.1), vec3<f32>(0.2, 0.9, 0.3), visible) * select(0.0, 1.0, s.valid && r.y.light != RESTIR_NONE);
        }
        default: {
            if (visible) {
                color = restirContribution(s, r.y) * r.W;
            }
        }
    }
    textureStore(hDirect, q, vec4<f32>(color, 1.0));
    // The running mean: frame n of the accumulation weighs 1 / (n + 1).
    let n = f32(restirParam(3u));
    let keep = n / (n + 1.0);
    let add = 1.0 / (n + 1.0);
    let old = vec3<f32>(textureLoad(hAccumR, q).x, textureLoad(hAccumG, q).x, textureLoad(hAccumB, q).x);
    let mean = old * keep + color * add;
    textureStore(hAccumR, q, vec4<f32>(mean.r));
    textureStore(hAccumG, q, vec4<f32>(mean.g));
    textureStore(hAccumB, q, vec4<f32>(mean.b));
}
