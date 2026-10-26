// Educational GPU records for the Render Lab.
//
// These layouts are written for teaching. They are not an engine's exact GPU
// structures; an investigation's source index says which structure each one
// stands in for.
// The video and the viewer's struct panel display the declarations below,
// extracted from this file, with byte offsets that build.rs exports from
// naga's layout of this module.
//
// WGSL layout rules: vec3 is 12 bytes aligned to 16, so a following scalar
// fills its last 4 bytes; matrices are column-major with a 16-byte column
// stride (mat3x3<f32> is 48 bytes, 12 of them padding); a struct's size is
// rounded up to its alignment. `m * v` treats v as a column vector, exactly
// like glam's `Mat4 * Vec4` in Rust. `render-lab selftest` checks every offset
// against the Rust structs and reads the records back on the GPU.

// One object: its transforms and bounds, and its distance field's volume.
struct ObjectData {
    localToWorld: mat4x4<f32>,
    worldToLocal: mat4x4<f32>,
    localCenter: vec3<f32>,
    boundingRadius: f32,
    localExtent: vec3<f32>,
    objectIndex: u32,
    worldCenter: vec3<f32>,
    shapeKind: u32,
    worldExtent: vec3<f32>,
    albedoRoughness: vec4<f32>,
    emissive: vec4<f32>,
}

// One sample of a mesh distance field, for display. A renderer keeps only
// the 8-bit encoded value inside a brick.
struct VoxelSample {
    localPosition: vec3<f32>,
    signedDistance: f32,
    voxel: vec3<u32>,
    encoded: u32,
}

// One entry of a distance field's indirection table (a brick index, or the
// invalid index for an empty cell), with its mip's decode values. The
// coordinates are explicit here so they can be shown.
struct BrickEntry {
    virtualBrick: vec3<u32>,
    mipLevel: u32,
    physicalBrick: vec3<u32>,
    physicalIndex: u32,
    distanceScaleBias: vec2<f32>,
    voxelSize: f32,
    flags: u32,
}

// One surface-cache card, unpacked: a renderer packs it tighter; this record
// keeps named fields.
struct CardData {
    center: vec3<f32>,
    objectIndex: u32,
    tangentFrame: mat3x3<f32>,   // columns: tangent, bitangent, normal
    halfExtents: vec3<f32>,
    pageIndex: u32,
    sizeInPages: vec2<u32>,
    resLevel: u32,
    flags: u32,
}

// One page-table entry of the surface cache: where a card's page sits in the
// physical atlas.
struct PageEntry {
    physicalPage: vec2<u32>,
    cardIndex: u32,
    mipLevel: u32,
    cardUVRect: vec4<f32>,
    flags: u32,
}

// Debug record for one reflection query. Real reflection passes keep similar
// quantities in textures and registers rather than one struct.
struct RayDebugData {
    origin: vec3<f32>,
    roughness: f32,
    incidentDir: vec3<f32>,
    hitDistance: f32,
    hitPosition: vec3<f32>,
    hitObject: u32,
    normal: vec3<f32>,
    envBRDF: f32,                     // EnvBRDFApprox(F0, roughness, N.V)
    viewDir: vec3<f32>,
    ggxAlpha: f32,                    // roughness^2, the VNDF lobe width
    reflectedDir: vec3<f32>,
    reflectedHitDistance: f32,
    reflectedHitPosition: vec3<f32>,
    reflectedHitObject: u32,
}

// The camera: a small part of a renderer's view constants.
struct CameraData {
    worldToView: mat4x4<f32>,
    viewToClip: mat4x4<f32>,
    worldToClip: mat4x4<f32>,
    position: vec3<f32>,
    verticalFov: f32,
    viewportSize: vec2<f32>,
    nearPlane: f32,
    farPlane: f32,
}

// One screen probe of a screen-probe gather, which a renderer keeps as depth,
// normal, and an octahedral radiance tile in textures.
struct ProbeRecord {
    position: vec3<f32>,
    firstSample: u32,
    normal: vec3<f32>,
    sampleCount: u32,
    screenPixel: vec2<u32>,
    historyWeight: f32,
    framesAccumulated: u32,
    irradiance: vec4<f32>,
}

// One traced direction of a probe: the radiance that came back.
struct RadianceSample {
    direction: vec3<f32>,
    hitDistance: f32,
    radiance: vec3<f32>,
    hitObject: u32,
}

// Per-frame constants (uniform buffer). Not shown in the struct panel.
struct FrameUniforms {
    camera: CameraData,
    lightCenterRadiance: vec4<f32>,   // panel centre, radiance scale
    lightAxisU: vec4<f32>,            // panel half axes; normal = cross(V, U)
    lightAxisV: vec4<f32>,
    lightColor: vec4<f32>,
    ambientColor: vec4<f32>,
    floorParams: vec4<f32>,           // roughness, F0, reflection strength, GGX samples per pixel
    overlayParams: vec4<f32>,         // exposure, pixel scale, time, card spread (display offset along card normals)
    cutPlane: vec4<f32>,              // keep local samples with dot(xyz, p) <= w
    deformParams: vec4<f32>,          // squash amount, deformed object, ghost opacity, unused
    sdfVolume: vec4<f32>,             // local volume minimum, distance-field object
    atlasDisplay: vec4<f32>,          // world origin of the brick-atlas display, slot size
    counts: vec4<u32>,                // objects, floor object, voxel samples, raster-hidden mask
    selection: vec4<u32>,             // object, card, page, brick
    selection2: vec4<u32>,            // voxel sample, probe, ray, unused
    counts2: vec4<u32>,               // Surface Cache cards, pages, objects fading out of the frame (bit mask), unused
    cacheView: vec4<f32>,             // radiosity frames shown (6 = finished), indirect light scale, presence of the fading objects, unused
}

// The Utah teapot's triangles, in its local space, under a bounding-volume
// hierarchy (src/lab/mesh.rs builds it). Shadow and reflection rays traverse it
// in intersectMesh (common.wgsl). Not shown in the struct panel.
struct MeshNode {
    boundsMin: vec3<f32>,
    firstOrLeft: u32,                 // leaf: first triangle; inner: left child (right child follows it)
    boundsMax: vec3<f32>,
    triangleCount: u32,               // 0 for inner nodes
}

struct MeshTriangle {
    p0: vec3<f32>,
    p1: vec3<f32>,
    p2: vec3<f32>,
    n0: vec3<f32>,                    // vertex normals, interpolated at the hit
    n1: vec3<f32>,
    n2: vec3<f32>,
}
