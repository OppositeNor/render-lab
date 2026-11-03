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
// CPU mirror in lab/src/scene.rs can be compared with the GPU.

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
fn putU4(v: vec4<u32>) { putU(v.x); putU(v.y); putU(v.z); putU(v.w); }
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
fn dumpFrame(@builtin(global_invocation_id) id: vec3<u32>) {
    let u = gFrame;
    cursor = 0u;
    putCamera(u.camera);
    putF4(u.lightCenterRadiance); putF4(u.lightAxisU); putF4(u.lightAxisV);
    putF4(u.lightColor); putF4(u.ambientColor); putF4(u.floorParams);
    putF4(u.overlayParams); putU4(u.counts); putF4(u.viewParams);
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

// Input: two vec4 per query (position.xyz, object) and (normal.xyz, 0).
// Output: shadeSurface radiance, then the mirror reflection seen from the
// camera position.
@compute @workgroup_size(1)
fn lightingTests(@builtin(global_invocation_id) id: vec3<u32>) {
    let a = gTests[id.x * 2u];
    let b = gTests[id.x * 2u + 1u];
    let object = u32(a.w);
    cursor = id.x * 6u;
    putF3(shadeSurface(a.xyz, b.xyz, object));
    let incident = normalize(a.xyz - gFrame.camera.position);
    putF3(reflectedRadiance(a.xyz, b.xyz, incident, object));
}
