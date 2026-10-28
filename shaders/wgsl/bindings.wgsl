// Resources shared by every renderer shader (bind group 0).
// src/lab/gpu.rs creates the matching bind group layout.

@group(0) @binding(0) var<uniform> gFrame: FrameUniforms;
@group(0) @binding(1) var<storage, read> gObjects: array<ObjectData>;
@group(0) @binding(2) var<storage, read> gVoxels: array<VoxelSample>;
@group(0) @binding(3) var<storage, read> gBricks: array<BrickEntry>;
@group(0) @binding(4) var<storage, read> gCards: array<CardData>;
@group(0) @binding(5) var<storage, read> gPages: array<PageEntry>;
@group(0) @binding(6) var<storage, read> gRays: array<RayDebugData>;
@group(0) @binding(7) var<storage, read> gProbes: array<ProbeRecord>;
@group(0) @binding(8) var<storage, read> gRadiance: array<RadianceSample>;
// The physical brick atlas: 8x8x8 one-byte samples per brick, like PF_G8.
@group(0) @binding(9) var gBrickAtlas: texture_3d<f32>;
// The picture of the Surface Cache atlases that the card quads show (sRGB,
// premultiplied), drawn each frame by displayCS in surface_cache.wgsl.
@group(0) @binding(10) var gSurfaceDisplay: texture_2d<f32>;
@group(0) @binding(11) var gLinearSampler: sampler;
// The teapot's BVH and triangles, in its local space (static).
@group(0) @binding(12) var<storage, read> gMeshNodes: array<MeshNode>;
@group(0) @binding(13) var<storage, read> gMeshTriangles: array<MeshTriangle>;
// The Surface Cache atlases as stored: one u32 per texel and layer, in
// the engine's encodings (see SURFACE_LAYER_* in common.wgsl).
@group(0) @binding(14) var gSurfaceCache: texture_2d_array<u32>;
// The lab's environment (src/lab/environment.rs): an equirectangular RGB9E5
// image with three levels, read with textureLoad.
@group(0) @binding(15) var gEnvironment: texture_2d<f32>;
// The lab's terrain heightfield (src/lab/terrain.rs, static): see terrainHeight in common.wgsl.
@group(0) @binding(16) var<storage, read> gTerrain: array<f32>;
