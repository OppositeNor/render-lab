// ReSTIR (Bitterli et al. 2020, "Spatiotemporal reservoir resampling for real-time
// ray tracing with dynamic direct lighting", ACM TOG 39(4), 148): what its passes
// share, and the scene pass that shows their result. Composed after the scene
// module (lab/src/shaders.rs), so SceneVaryings, SceneOutput, gFrame, gObjects,
// and common.wgsl are in scope; restir_passes.wgsl holds the passes of Alg. 5.
//
// Written for the lab from the paper's equations and algorithms (MIT, the lab's
// own code); src/paper/restir/cpu.rs mirrors it on the CPU, and `selftest`
// compares the two.

// gShading: the method's parameters (the `///` tags; the lab packs them from
// the shot, lab/src/shader_data.rs). The passes read them here too.
struct RestirShading {
    /// M: the candidates each pixel draws from the lights (Alg. 3; the paper's results use 32, §5).
    /// {@ key=restirCandidates default=32 range=1,64 unit=index label=restir text=controls-candidates help=help-restirCandidates}
    candidates: f32,
    /// Whether each pixel's reservoir is combined with last frame's (temporal reuse, Alg. 5 lines 11-14).
    /// {@ key=restirTemporal default=1 range=0,1 label=restir text=controls-temporal help=help-restirTemporal}
    temporal: f32,
    /// n: the rounds of spatial reuse (Alg. 5 lines 16-20; the paper's biased results use 2, its unbiased 1, §5).
    /// {@ key=restirSpatialPasses default=2 range=0,4 unit=index label=restir text=controls-spatial_passes help=help-restirSpatialPasses}
    spatialPasses: f32,
    /// k: the neighbours each round combines (5 biased, 3 unbiased in the paper, §5).
    /// {@ key=restirNeighbours default=5 range=1,16 unit=index label=restir text=controls-neighbours help=help-restirNeighbours}
    neighbours: f32,
    /// The radius the neighbours are drawn in, in pixels (30 in the paper, §5).
    /// {@ key=restirRadius default=30 range=1,100 unit=index label=restir text=controls-radius help=help-restirRadius}
    radius: f32,
    /// Whether each pixel's selected sample is tested for visibility before reuse (visibility reuse, §3.2).
    /// {@ key=restirVisibilityReuse default=1 range=0,1 label=restir text=controls-visibility_reuse help=help-restirVisibilityReuse}
    visibilityReuse: f32,
    /// Whether reservoirs are combined with the unbiased weight of Alg. 6 (1) or the biased one of Alg. 4 (0).
    /// {@ key=restirUnbiased default=0 range=0,1 label=restir text=controls-unbiased help=help-restirUnbiased}
    unbiased: f32,
    /// Whether a neighbour whose depth differs by more than 10% or whose normal turns more than 25 degrees is skipped (§5).
    /// {@ key=restirGeometryTest default=1 range=0,1 label=restir text=controls-geometry_test help=help-restirGeometryTest}
    geometryTest: f32,
    /// The most last frame's M may be, as a multiple of this frame's (20 in the paper, §5).
    /// {@ key=restirTemporalClamp default=20 range=1,100 unit=index label=restir text=controls-temporal_clamp help=help-restirTemporalClamp}
    temporalClamp: f32,
    /// What the scene shows: 0 the lit image, 1 the light each pixel's sample is on, 2 its M (the candidates its
    /// reservoir stands for), 3 its W, 4 its visibility.
    /// {@ key=restirView default=0 range=0,4 unit=index label=view text=controls-view help=help-restirView}
    view: f32,
    /// Whether frames are averaged while nothing moves (a converged reference, as the paper's figures compare with).
    /// {@ key=restirAccumulate default=0 range=0,1 label=view text=controls-accumulate help=help-restirAccumulate}
    accumulate: f32,
    /// Which reservoirs the scene shows: 0 after RIS (the candidates alone), 1 after visibility, 2 after temporal
    /// reuse, 3 after spatial reuse (the frame's).
    /// {@ key=restirStage default=3 range=0,3 unit=index label=view text=controls-stage help=help-restirStage}
    stage: f32,
}
/// {@ shader_data}
@group(1) @binding(0) var<uniform> gShading: RestirShading;

// gShadingFrame (src/paper/restir/lights.rs `pack`): [0] the light count, the total power, the first light's
// object index, 0; then per light two vectors: its centre and radius, its radiance and the share of the total
// power of the lights up to and including it (the CDF lights are picked from).
@group(1) @binding(2) var<storage, read> gShadingFrame: array<vec4<f32>>;

const RESTIR_NONE: u32 = 0xffffffffu;

struct Light {
    center: vec3<f32>,
    radius: f32,
    radiance: vec3<f32>,
    cdf: f32,
}

fn restirLightCount() -> u32 {
    return u32(gShadingFrame[0].x);
}

fn restirFirstLightObject() -> u32 {
    return u32(gShadingFrame[0].z);
}

fn restirLight(i: u32) -> Light {
    let a = gShadingFrame[1u + 2u * i];
    let b = gShadingFrame[2u + 2u * i];
    return Light(a.xyz, a.w, b.xyz, b.w);
}

fn restirLuminance(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

// ---------------------------------------------------------------------------
// Random numbers: the PCG hash (Jarzynski and Olano, "Hash Functions for GPU
// Rendering", JCGT 2020), seeded per pixel, frame, and pass; cpu.rs `Rng`
// draws the same numbers.

fn restirHash(x: u32) -> u32 {
    let state = x * 747796405u + 2891336453u;
    let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

struct Rng {
    state: u32,
}

fn restirRng(pixel: vec2<u32>, frame: u32, stream: u32) -> Rng {
    return Rng(restirHash(pixel.x ^ restirHash(pixel.y ^ restirHash(frame * 16u + stream))));
}

fn restirNext(rng: ptr<function, Rng>) -> u32 {
    (*rng).state = restirHash((*rng).state);
    return (*rng).state;
}

// A float in [0, 1) from the top 24 bits.
fn restirRand(rng: ptr<function, Rng>) -> f32 {
    return f32(restirNext(rng) >> 8u) * (1.0 / 16777216.0);
}

// ---------------------------------------------------------------------------
// A light sample: a light and a point on it, the point kept as two 16-bit
// coordinates on the sphere (z and the angle), so the point read back is the
// point the weights were computed for.

struct Sample {
    light: u32,
    uv: u32,
}

fn restirSampleDir(uv: u32) -> vec3<f32> {
    let u = (f32(uv & 0xffffu) + 0.5) / 65536.0;
    let v = (f32(uv >> 16u) + 0.5) / 65536.0;
    let z = 1.0 - 2.0 * u;
    let r = sqrt(max(0.0, 1.0 - z * z));
    let phi = 2.0 * PI * v;
    return vec3<f32>(r * cos(phi), z, r * sin(phi));
}

fn restirSamplePoint(s: Sample) -> vec3<f32> {
    let l = restirLight(s.light);
    return l.center + l.radius * restirSampleDir(s.uv);
}

// The light picked by power: the first whose CDF passes u (a binary search).
fn restirPickLight(u: f32) -> u32 {
    var lo = 0u;
    var hi = restirLightCount() - 1u;
    while (lo < hi) {
        let mid = (lo + hi) / 2u;
        if (restirLight(mid).cdf > u) {
            hi = mid;
        } else {
            lo = mid + 1u;
        }
    }
    return lo;
}

// p(x) of a candidate (§5, "Candidate Generation"): the light's share of the power, then a uniform point on its
// surface, so per unit area (Le ∝ p, as the paper's p ∝ Le).
fn restirSourcePdf(light: u32) -> f32 {
    let l = restirLight(light);
    let before = select(0.0, restirLight(light - 1u).cdf, light > 0u);
    return (l.cdf - before) / (4.0 * PI * l.radius * l.radius);
}

// ---------------------------------------------------------------------------
// The surface at a pixel, as the candidate pass stores it (restir_passes.wgsl),
// and its BRDF: a GGX dielectric layer over a Lambertian base, as the paper's
// unified material (§5, "Target PDF"), the lab's albedo and roughness.

struct Surface {
    position: vec3<f32>,
    distance: f32,
    normal: vec3<f32>,
    roughness: f32,
    albedo: vec3<f32>,
    object: u32,
    valid: bool,
}

fn restirBrdf(s: Surface, l: vec3<f32>, v: vec3<f32>) -> vec3<f32> {
    let n = s.normal;
    let noL = dot(n, l);
    let noV = max(dot(n, v), 1e-4);
    if (noL <= 0.0) {
        return vec3<f32>(0.0);
    }
    let h = normalize(l + v);
    let noH = max(dot(n, h), 0.0);
    let voH = max(dot(v, h), 0.0);
    let a = max(s.roughness * s.roughness, 1e-3);
    let a2 = a * a;
    let d = a2 / (PI * pow(noH * noH * (a2 - 1.0) + 1.0, 2.0));
    let k = a * 0.5;
    let g = (noL / (noL * (1.0 - k) + k)) * (noV / (noV * (1.0 - k) + k));
    let f = 0.04 + 0.96 * pow(1.0 - voH, 5.0);
    let specular = d * g * f / (4.0 * noL * noV);
    return s.albedo * (1.0 - f) / PI + vec3<f32>(specular);
}

// The unshadowed contribution ρ · Le · G of a sample at a surface (the paper's
// target, §5), and p̂, its luminance.
fn restirContribution(s: Surface, x: Sample) -> vec3<f32> {
    if (!s.valid || x.light == RESTIR_NONE) {
        return vec3<f32>(0.0);
    }
    let l = restirLight(x.light);
    let dir = restirSampleDir(x.uv);
    let p = l.center + l.radius * dir;
    let toLight = p - s.position;
    let d2 = max(dot(toLight, toLight), 1e-8);
    let wi = toLight * inverseSqrt(d2);
    let cosSurface = dot(s.normal, wi);
    let cosLight = dot(dir, -wi);
    if (cosSurface <= 0.0 || cosLight <= 0.0) {
        return vec3<f32>(0.0);
    }
    let v = normalize(gFrame.camera.position - s.position);
    return restirBrdf(s, wi, v) * l.radiance * cosSurface * cosLight / d2;
}

fn restirTarget(s: Surface, x: Sample) -> f32 {
    return restirLuminance(restirContribution(s, x));
}

// V: whether the segment from the surface to the sample is clear. The lights
// do not block one another (the lab's choice: they are small), the room's
// objects do (shadowVisibility, common.wgsl, skips the surface itself).
fn restirVisible(s: Surface, x: Sample) -> bool {
    let p = restirSamplePoint(x);
    let toLight = p - s.position;
    let dist = length(toLight);
    let dir = toLight / max(dist, 1e-6);
    let origin = s.position + s.normal * 1e-3;
    return restirShadowClear(origin, dir, dist * 0.999, s.object);
}

fn restirShadowClear(p: vec3<f32>, dir: vec3<f32>, dist: f32, selfObject: u32) -> bool {
    let first = restirFirstLightObject();
    for (var i = 0u; i < first; i++) {
        let o = gObjects[i];
        if (i == selfObject || !occludesLight(o)) {
            continue;
        }
        var blocked = false;
        if (o.shapeKind == SHAPE_MESH) {
            blocked = intersectMesh(transformPoint(o.worldToLocal, p), transformVector(o.worldToLocal, dir), dist, true).hit;
        } else {
            blocked = intersectObject(o, p, dir, dist).hit;
        }
        if (blocked) {
            return false;
        }
    }
    return true;
}

// ---------------------------------------------------------------------------
// A reservoir (Alg. 2 and §5, "Reservoir storage"): the selected sample y, the
// probabilistic weight W, and M, the candidates it stands for, kept per pixel
// as four words (light, point, W, M).

struct Reservoir {
    y: Sample,
    wsum: f32,
    M: f32,
    W: f32,
}

fn restirEmpty() -> Reservoir {
    return Reservoir(Sample(RESTIR_NONE, 0u), 0.0, 0.0, 0.0);
}

fn restirPack(r: Reservoir) -> vec4<u32> {
    return vec4<u32>(r.y.light, r.y.uv, bitcast<u32>(r.W), bitcast<u32>(r.M));
}

fn restirUnpack(v: vec4<u32>) -> Reservoir {
    return Reservoir(Sample(v.x, v.y), 0.0, bitcast<f32>(v.w), bitcast<f32>(v.z));
}

// Alg. 2, update: add a candidate of weight w, keeping it with probability w / wsum.
fn restirUpdate(r: ptr<function, Reservoir>, x: Sample, w: f32, rng: ptr<function, Rng>) {
    (*r).wsum += w;
    (*r).M += 1.0;
    let u = restirRand(rng);
    if (w > 0.0 && u * (*r).wsum < w) {
        (*r).y = x;
    }
}

// ---------------------------------------------------------------------------
// The scene pass: the reservoirs' result (restirShade's image) on the room's
// surfaces, the lights drawn with their radiance. A view mode shows the
// reservoirs themselves instead.

@group(3) @binding(1) var gRestirDirect: texture_2d<f32>;
@group(3) @binding(2) var gRestirAccumR: texture_2d<f32>;
@group(3) @binding(3) var gRestirAccumG: texture_2d<f32>;
@group(3) @binding(4) var gRestirAccumB: texture_2d<f32>;

struct RestirPrepassOutput {
    @location(0) normal: vec4<f32>,
    @location(1) material: vec4<f32>,
}

// The pre-pass: each pixel's normal (turned to the eye) and object, and its albedo and roughness, for the
// candidate pass to read.
@fragment
fn restirPrepassFS(input: SceneVaryings) -> RestirPrepassOutput {
    let n = eyeSideNormal(input.worldNormal, input.worldPosition, gFrame.camera.position);
    let o = gObjects[input.objectIndex];
    return RestirPrepassOutput(vec4<f32>(n, f32(input.objectIndex)), o.albedoRoughness);
}

@fragment
fn restirForwardFS(input: SceneVaryings) -> SceneOutput {
    let pixel = vec2<i32>(input.clip.xy);
    let o = gObjects[input.objectIndex];
    var color = vec3<f32>(0.0);
    if (input.objectIndex >= restirFirstLightObject()) {
        // A light: its radiance.
        color = o.emissive.rgb * o.emissive.w;
    } else if (gShading.accumulate > 0.5 && u32(gShading.view) == 0u) {
        color = vec3<f32>(textureLoad(gRestirAccumR, pixel, 0).x, textureLoad(gRestirAccumG, pixel, 0).x, textureLoad(gRestirAccumB, pixel, 0).x);
    } else {
        color = textureLoad(gRestirDirect, pixel, 0).rgb;
    }
    return SceneOutput(vec4<f32>(color, 1.0), 0xffffffffu);
}
