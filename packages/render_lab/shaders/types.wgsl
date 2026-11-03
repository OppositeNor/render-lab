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

// One object: its transforms and bounds, and its material.
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

// Per-frame constants (uniform buffer). Not shown in the struct panel.
struct FrameUniforms {
    camera: CameraData,
    lightCenterRadiance: vec4<f32>,   // panel centre, radiance scale
    lightAxisU: vec4<f32>,            // panel half axes; normal = cross(V, U)
    lightAxisV: vec4<f32>,
    lightColor: vec4<f32>,
    ambientColor: vec4<f32>,
    floorParams: vec4<f32>,           // roughness, F0, reflection strength, GGX samples per pixel
    overlayParams: vec4<f32>,         // exposure, pixel scale, time, unused
    counts: vec4<u32>,                // objects, floor object, objects fading out of the frame (bit mask), raster-hidden mask
    viewParams: vec4<f32>,            // presence of the fading objects, indirect light scale, ghost opacity, 1 + frames the view held still (<= 255) + 256 x frames the scene held still (0: no history; FrameRequest::history)
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
