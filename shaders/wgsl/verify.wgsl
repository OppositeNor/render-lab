// Verification kernels for `render-lab selftest`.
//
// The dump kernels read each record through the WGSL declarations in
// types.wgsl and write every field, in declaration order, as raw 32-bit
// words. Rust compares them with the bytes it uploaded: a wrong offset,
// stride, or matrix layout shows up as a mismatch. Matrices are written row
// by row in the mathematical sense; WGSL indexes columns, so element
// (row r, column c) is m[c][r].
//
// The remaining kernels rerun the shared scene math from common.wgsl so the
// CPU mirror in src/lab/scene.rs can be compared with the GPU.

@group(1) @binding(0) var<storage, read_write> gOut: array<u32>;
@group(1) @binding(1) var<storage, read> gTests: array<vec4<f32>>;

var<private> cursor: u32;

fn putF(v: f32) {
    gOut[cursor] = bitcast<u32>(v);
    cursor += 1u;
}
fn putU(v: u32) {
    gOut[cursor] = v;
    cursor += 1u;
}
fn putF2(v: vec2<f32>) { putF(v.x); putF(v.y); }
fn putF3(v: vec3<f32>) { putF(v.x); putF(v.y); putF(v.z); }
fn putF4(v: vec4<f32>) { putF(v.x); putF(v.y); putF(v.z); putF(v.w); }
fn putU2(v: vec2<u32>) { putU(v.x); putU(v.y); }
fn putU3(v: vec3<u32>) { putU(v.x); putU(v.y); putU(v.z); }
fn putU4(v: vec4<u32>) { putU(v.x); putU(v.y); putU(v.z); putU(v.w); }
fn putM3(value: mat3x3<f32>) {
    var m = value;
    for (var r = 0; r < 3; r++) {
        for (var c = 0; c < 3; c++) {
            putF(m[c][r]);
        }
    }
}
fn putM4(value: mat4x4<f32>) {
    var m = value;
    for (var r = 0; r < 4; r++) {
        for (var c = 0; c < 4; c++) {
            putF(m[c][r]);
        }
    }
}
fn putCamera(c: CameraData) {
    putM4(c.worldToView); putM4(c.viewToClip); putM4(c.worldToClip);
    putF3(c.position); putF(c.verticalFov);
    putF2(c.viewportSize); putF(c.nearPlane); putF(c.farPlane);
}

@compute @workgroup_size(1)
fn dumpObjects(@builtin(global_invocation_id) id: vec3<u32>) {
    let o = gObjects[id.x];
    cursor = id.x * 55u;
    putM4(o.localToWorld); putM4(o.worldToLocal);
    putF3(o.localCenter); putF(o.boundingRadius);
    putF3(o.localExtent); putU(o.objectIndex);
    putF3(o.worldCenter); putU(o.shapeKind);
    putF3(o.worldExtent);
    putF4(o.albedoRoughness); putF4(o.emissive);
}

@compute @workgroup_size(1)
fn dumpVoxels(@builtin(global_invocation_id) id: vec3<u32>) {
    let s = gVoxels[id.x];
    cursor = id.x * 8u;
    putF3(s.localPosition); putF(s.signedDistance); putU3(s.voxel); putU(s.encoded);
}

@compute @workgroup_size(1)
fn dumpBricks(@builtin(global_invocation_id) id: vec3<u32>) {
    let b = gBricks[id.x];
    cursor = id.x * 12u;
    putU3(b.virtualBrick); putU(b.mipLevel);
    putU3(b.physicalBrick); putU(b.physicalIndex);
    putF2(b.distanceScaleBias); putF(b.voxelSize); putU(b.flags);
}

@compute @workgroup_size(1)
fn dumpCards(@builtin(global_invocation_id) id: vec3<u32>) {
    let c = gCards[id.x];
    cursor = id.x * 21u;
    putF3(c.center); putU(c.objectIndex);
    putM3(c.tangentFrame);
    putF3(c.halfExtents); putU(c.pageIndex);
    putU2(c.sizeInPages); putU(c.resLevel); putU(c.flags);
}

@compute @workgroup_size(1)
fn dumpPages(@builtin(global_invocation_id) id: vec3<u32>) {
    let p = gPages[id.x];
    cursor = id.x * 9u;
    putU2(p.physicalPage); putU(p.cardIndex); putU(p.mipLevel); putF4(p.cardUVRect); putU(p.flags);
}

@compute @workgroup_size(1)
fn dumpRays(@builtin(global_invocation_id) id: vec3<u32>) {
    let r = gRays[id.x];
    cursor = id.x * 28u;
    putF3(r.origin); putF(r.roughness);
    putF3(r.incidentDir); putF(r.hitDistance);
    putF3(r.hitPosition); putU(r.hitObject);
    putF3(r.normal); putF(r.envBRDF);
    putF3(r.viewDir); putF(r.ggxAlpha);
    putF3(r.reflectedDir); putF(r.reflectedHitDistance);
    putF3(r.reflectedHitPosition); putU(r.reflectedHitObject);
}

@compute @workgroup_size(1)
fn dumpProbes(@builtin(global_invocation_id) id: vec3<u32>) {
    let p = gProbes[id.x];
    cursor = id.x * 16u;
    putF3(p.position); putU(p.firstSample);
    putF3(p.normal); putU(p.sampleCount);
    putU2(p.screenPixel); putF(p.historyWeight); putU(p.framesAccumulated);
    putF4(p.irradiance);
}

@compute @workgroup_size(1)
fn dumpRadiance(@builtin(global_invocation_id) id: vec3<u32>) {
    let s = gRadiance[id.x];
    cursor = id.x * 8u;
    putF3(s.direction); putF(s.hitDistance); putF3(s.radiance); putU(s.hitObject);
}

@compute @workgroup_size(1)
fn dumpFrame(@builtin(global_invocation_id) id: vec3<u32>) {
    let u = gFrame;
    cursor = 0u;
    putCamera(u.camera);
    putF4(u.lightCenterRadiance); putF4(u.lightAxisU); putF4(u.lightAxisV);
    putF4(u.lightColor); putF4(u.ambientColor); putF4(u.floorParams);
    putF4(u.overlayParams); putF4(u.cutPlane); putF4(u.deformParams);
    putF4(u.sdfVolume); putF4(u.atlasDisplay);
    putU4(u.counts); putU4(u.selection); putU4(u.selection2); putU4(u.counts2);
    putF4(u.cacheView);
}

// Input: two vec4 per ray (origin.xyz, tMax) and (direction.xyz, skipObject).
// Output: t, object, normal, reflect(direction, normal), EnvBRDF at roughness 0.3.
@compute @workgroup_size(1)
fn traceTests(@builtin(global_invocation_id) id: vec3<u32>) {
    let a = gTests[id.x * 2u];
    let b = gTests[id.x * 2u + 1u];
    let hit = traceScene(a.xyz, b.xyz, a.w, u32(b.w));
    cursor = id.x * 9u;
    putF(hit.t); putU(hit.object); putF3(hit.normal);
    putF3(reflect(b.xyz, hit.normal));
    putF(envBRDFApprox(gFrame.floorParams.y, 0.3, saturate(dot(hit.normal, -b.xyz))));
}

// Input: one vec4 per world point. Output: clip xyzw, then the pixel position
// inside the viewport (origin top left, +Y down).
@compute @workgroup_size(1)
fn projectTests(@builtin(global_invocation_id) id: vec3<u32>) {
    let clip = gFrame.camera.worldToClip * vec4<f32>(gTests[id.x].xyz, 1.0);
    let ndc = clip.xy / clip.w;
    let pixel = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5) * gFrame.camera.viewportSize;
    cursor = id.x * 6u;
    putF4(clip); putF2(pixel);
}

// Input: two vec4 per query (position.xyz, object) and (normal.xyz, bias).
// Output: sampleSurfaceCache's final lighting, indirect irradiance, weight sum, valid.
@compute @workgroup_size(1)
fn surfaceCacheTests(@builtin(global_invocation_id) id: vec3<u32>) {
    let a = gTests[id.x * 2u];
    let b = gTests[id.x * 2u + 1u];
    let s = sampleSurfaceCache(u32(a.w), a.xyz, b.xyz, b.w);
    cursor = id.x * 8u;
    putF3(s.finalLighting); putF3(s.indirect); putF(s.weight); putU(select(0u, 1u, s.valid));
}

// Input: two vec4 per query (incident.xyz, roughness) and (normal.xyz, k).
// Output: reflectionRayDirection for the k-th of 16 Hammersley points.
@compute @workgroup_size(1)
fn ggxTests(@builtin(global_invocation_id) id: vec3<u32>) {
    let a = gTests[id.x * 2u];
    let b = gTests[id.x * 2u + 1u];
    cursor = id.x * 3u;
    putF3(reflectionRayDirection(a.xyz, b.xyz, a.w, hammersley(u32(b.w), 16u)));
}

// Input: two vec4 per query (position.xyz, object) and (normal.xyz, 0).
// Output: shadeSurface radiance, then the mirror reflection seen from the
// camera position (a single cone sample at roughness 0).
@compute @workgroup_size(1)
fn lightingTests(@builtin(global_invocation_id) id: vec3<u32>) {
    let a = gTests[id.x * 2u];
    let b = gTests[id.x * 2u + 1u];
    let object = u32(a.w);
    cursor = id.x * 6u;
    putF3(shadeSurface(a.xyz, b.xyz, object));
    let incident = normalize(a.xyz - gFrame.camera.position);
    putF3(reflectedRadiance(a.xyz, b.xyz, incident, object, 0.0, vec2<f32>(0.0)));
}
