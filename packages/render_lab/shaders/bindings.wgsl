// Resources shared by every renderer shader (bind group 0).
// lab/src/pipelines.rs creates the matching bind group layout.

@group(0) @binding(0) var<uniform> gFrame: FrameUniforms;
@group(0) @binding(1) var<storage, read> gObjects: array<ObjectData>;
@group(0) @binding(2) var gLinearSampler: sampler;
// The teapot's BVH and triangles, in its local space (static).
@group(0) @binding(3) var<storage, read> gMeshNodes: array<MeshNode>;
@group(0) @binding(4) var<storage, read> gMeshTriangles: array<MeshTriangle>;
// The lab's environment (lab/src/environment.rs): an equirectangular RGB9E5
// image with three levels, read with textureLoad.
@group(0) @binding(5) var gEnvironment: texture_2d<f32>;
// The lab's terrain heightfield (lab/src/terrain.rs, static): see terrainHeight in common.wgsl.
@group(0) @binding(6) var<storage, read> gTerrain: array<f32>;
