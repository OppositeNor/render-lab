// What common.wgsl, overlay.wgsl, and verify.wgsl call when no investigation
// supplies a Surface Cache lookup or reflection-ray choice: composed after
// common.wgsl in every shader module instead of
// src/commercial/shaders/surface_lookup.wgsl when src/commercial is absent
// (build.rs, src/lab/shaders.rs). There are then no cards: every lookup
// misses, so indirect light falls back to the flat ambient term, and the
// floor reflects only along the mirror direction.

// A card UV mapped straight onto its pages, with no page table.
fn surfaceCacheCoord(card: CardData, cardUV: vec2<f32>) -> vec2<f32> {
    return clamp(cardUV, vec2<f32>(0.0), vec2<f32>(1.0)) * vec2<f32>(card.sizeInPages) * PHYSICAL_PAGE_SIZE;
}

struct SurfaceCacheSample {
    finalLighting: vec3<f32>,
    indirect: vec3<f32>,
    weight: f32,
    valid: bool,
}

fn sampleSurfaceCache(objectIndex: u32, p: vec3<f32>, n: vec3<f32>, bias: f32) -> SurfaceCacheSample {
    return SurfaceCacheSample(vec3<f32>(0.0), vec3<f32>(0.0), 0.0, false);
}

// No clamp on a ray's radiance.
const REFLECTION_MAX_RAY_INTENSITY: f32 = 3.0e38;

// Every reflection is traced, at any roughness.
fn tracedReflectionAlpha(roughness: f32) -> f32 {
    return 1.0;
}

fn reflectionRayDirection(incident: vec3<f32>, n: vec3<f32>, roughness: f32, e: vec2<f32>) -> vec3<f32> {
    return reflect(incident, n);
}
