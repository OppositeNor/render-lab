// Scene intersection, lighting, and reflection math shared by the raster
// passes and the verification kernels. lab/src/scene.rs mirrors these functions
// on the CPU; `render-lab selftest` compares the two.

const PI: f32 = 3.14159265358979;
const SHAPE_PLANE: u32 = 0u;   // rectangle in the local XZ plane, normal +Y
const SHAPE_BOX: u32 = 1u;     // box localCenter +/- localExtent
const SHAPE_SPHERE: u32 = 2u;  // sphere at localCenter, radius localExtent.x
const SHAPE_PANEL: u32 = 3u;   // emissive rectangle, local XZ plane
const SHAPE_MESH: u32 = 4u;    // the teapot's triangles (gMeshNodes, gMeshTriangles)
const SHAPE_TERRAIN: u32 = 5u; // the lab's terrain heightfield (gTerrain)
const TERRAIN_SELF_SHADOW_BIAS: f32 = 0.03; // metres; scene.rs's TERRAIN_SELF_SHADOW_BIAS
const MESH_STACK: u32 = 32u;   // lab/src/mesh.rs asserts the BVH is at most 30 levels deep
const NO_HIT: u32 = 0xffffffffu;

struct SceneHit {
    t: f32,
    object: u32,
    normal: vec3<f32>,
}

struct ObjectHit {
    hit: bool,
    t: f32,
    normal: vec3<f32>,
}

fn transformPoint(m: mat4x4<f32>, p: vec3<f32>) -> vec3<f32> {
    return (m * vec4<f32>(p, 1.0)).xyz;
}

fn transformVector(m: mat4x4<f32>, v: vec3<f32>) -> vec3<f32> {
    return (m * vec4<f32>(v, 0.0)).xyz;
}

fn upper3x3(m: mat4x4<f32>) -> mat3x3<f32> {
    return mat3x3<f32>(m[0].xyz, m[1].xyz, m[2].xyz);
}

// Local normals reach world space through the inverse transpose of the
// local-to-world rotation and scale: n_world = transpose(worldToLocal) * n.
fn normalToWorld(o: ObjectData, nLocal: vec3<f32>) -> vec3<f32> {
    return normalize(transpose(upper3x3(o.worldToLocal)) * nLocal);
}

fn occludesLight(o: ObjectData) -> bool {
    return o.shapeKind == SHAPE_BOX || o.shapeKind == SHAPE_SPHERE || o.shapeKind == SHAPE_MESH || o.shapeKind == SHAPE_TERRAIN;
}

// The terrain (lab/src/terrain.rs): gTerrain holds the vertices per side
// (as bits), the half size, the lowest and highest heights, three unused
// words, then the heights row by row along +Z.
fn terrainResolution() -> u32 {
    return bitcast<u32>(gTerrain[0]);
}

fn terrainHalfSize() -> vec2<f32> {
    return vec2<f32>(gTerrain[1], gTerrain[2]);
}

fn terrainHeight(i: u32, j: u32) -> f32 {
    return gTerrain[8u + j * terrainResolution() + i];
}

fn terrainVertex(i: u32, j: u32) -> vec3<f32> {
    let cell = 2.0 * terrainHalfSize() / f32(terrainResolution() - 1u);
    return vec3<f32>(-terrainHalfSize().x + cell.x * f32(i), terrainHeight(i, j), -terrainHalfSize().y + cell.y * f32(j));
}

// Central differences, one-sided at the edges (Terrain::vertex_normal).
fn terrainVertexNormal(i: u32, j: u32) -> vec3<f32> {
    let last = terrainResolution() - 1u;
    let i0 = select(i - 1u, 0u, i == 0u);
    let i1 = min(i + 1u, last);
    let j0 = select(j - 1u, 0u, j == 0u);
    let j1 = min(j + 1u, last);
    let cell = 2.0 * terrainHalfSize() / f32(last);
    let dx = (terrainHeight(i1, j) - terrainHeight(i0, j)) / (cell.x * f32(i1 - i0));
    let dz = (terrainHeight(i, j1) - terrainHeight(i, j0)) / (cell.y * f32(j1 - j0));
    return normalize(vec3<f32>(-dx, 1.0, -dz));
}

// Two-sided Moller-Trumbore: (t, u, v), t < 0 on a miss.
fn terrainTriangle(o: vec3<f32>, d: vec3<f32>, p0: vec3<f32>, p1: vec3<f32>, p2: vec3<f32>) -> vec3<f32> {
    let e1 = p1 - p0;
    let e2 = p2 - p0;
    let pv = cross(d, e2);
    let det = dot(e1, pv);
    if (abs(det) < 1e-12) {
        return vec3<f32>(-1.0, 0.0, 0.0);
    }
    let inv = 1.0 / det;
    let tv = o - p0;
    let u = dot(tv, pv) * inv;
    if (u < 0.0 || u > 1.0) {
        return vec3<f32>(-1.0, 0.0, 0.0);
    }
    let qv = cross(tv, e1);
    let v = dot(d, qv) * inv;
    if (v < 0.0 || u + v > 1.0) {
        return vec3<f32>(-1.0, 0.0, 0.0);
    }
    return vec3<f32>(dot(e2, qv) * inv, u, v);
}

// Closest hit of a local-space ray with the terrain: the cells under the ray
// in order (Amanatides & Woo 1987), each skipped when the ray passes above
// or below its corners, then its two triangles. The same walk as
// Terrain::intersect in lab/src/terrain.rs; the normal is the interpolated
// vertex normal, in local space.
fn intersectTerrain(lo: vec3<f32>, ld: vec3<f32>, tMax: f32) -> ObjectHit {
    var result = ObjectHit(false, tMax, vec3<f32>(0.0, 1.0, 0.0));
    let half = terrainHalfSize();
    let bmin = vec3<f32>(-half.x, gTerrain[3], -half.y);
    let bmax = vec3<f32>(half.x, gTerrain[4], half.y);
    let inv = 1.0 / select(ld, vec3<f32>(1e-9), abs(ld) < vec3<f32>(1e-9));
    let t0 = (bmin - lo) * inv;
    let t1 = (bmax - lo) * inv;
    let tn3 = min(t0, t1);
    let tf3 = max(t0, t1);
    let tEnter = max(max(max(tn3.x, tn3.y), tn3.z), 0.0);
    let tExit = min(min(min(tf3.x, tf3.y), tf3.z), tMax);
    if (tExit < tEnter) {
        return result;
    }
    let cells = i32(terrainResolution()) - 1;
    let last = cells - 1;
    let cell = 2.0 * half / f32(cells);
    let p = lo + ld * tEnter;
    var i = clamp(i32(floor((p.x + half.x) / cell.x)), 0, last);
    var j = clamp(i32(floor((p.z + half.y) / cell.y)), 0, last);
    let si = select(-1, 1, ld.x >= 0.0);
    let sj = select(-1, 1, ld.z >= 0.0);
    let nextX = -half.x + cell.x * f32(i + select(0, 1, si > 0));
    let nextZ = -half.y + cell.y * f32(j + select(0, 1, sj > 0));
    var tx = (nextX - lo.x) * inv.x;
    var tz = (nextZ - lo.z) * inv.z;
    let dtx = abs(cell.x * inv.x);
    let dtz = abs(cell.y * inv.z);
    var tCell = tEnter;
    for (var step = 0; step < 2 * cells + 2; step++) {
        let tOut = min(min(tx, tz), tExit);
        let ya = lo.y + ld.y * tCell;
        let yb = lo.y + ld.y * tOut;
        let ci = u32(i);
        let cj = u32(j);
        let h00 = terrainHeight(ci, cj);
        let h10 = terrainHeight(ci + 1u, cj);
        let h01 = terrainHeight(ci, cj + 1u);
        let h11 = terrainHeight(ci + 1u, cj + 1u);
        let loH = min(min(h00, h10), min(h01, h11));
        let hiH = max(max(h00, h10), max(h01, h11));
        if (min(ya, yb) <= hiH + 1e-4 && max(ya, yb) >= loH - 1e-4) {
            // The cell's triangles: (i,j) (i,j+1) (i+1,j+1), then (i,j) (i+1,j+1) (i+1,j).
            let a = terrainVertex(ci, cj);
            let c = terrainVertex(ci + 1u, cj + 1u);
            let first = terrainTriangle(lo, ld, a, terrainVertex(ci, cj + 1u), c);
            let second = terrainTriangle(lo, ld, a, c, terrainVertex(ci + 1u, cj));
            var best = tMax;
            if (first.x > 1e-4 && first.x < best) {
                best = first.x;
                result.hit = true;
                result.normal = normalize(terrainVertexNormal(ci, cj) * (1.0 - first.y - first.z) + terrainVertexNormal(ci, cj + 1u) * first.y + terrainVertexNormal(ci + 1u, cj + 1u) * first.z);
            }
            if (second.x > 1e-4 && second.x < best) {
                best = second.x;
                result.hit = true;
                result.normal = normalize(terrainVertexNormal(ci, cj) * (1.0 - second.y - second.z) + terrainVertexNormal(ci + 1u, cj + 1u) * second.y + terrainVertexNormal(ci + 1u, cj) * second.z);
            }
            if (result.hit) {
                result.t = best;
                return result;
            }
        }
        if (tOut >= tExit) {
            return result;
        }
        if (tx < tz) {
            i += si;
            tCell = tx;
            tx += dtx;
        } else {
            j += sj;
            tCell = tz;
            tz += dtz;
        }
        if (i < 0 || j < 0 || i > last || j > last) {
            return result;
        }
    }
    return result;
}


// Entry distance of a ray into a BVH node's box, or -1 if it misses or
// enters at or after tMax. Mirrors box_entry in lab/src/mesh.rs.
fn meshBoxEntry(node: MeshNode, ro: vec3<f32>, invD: vec3<f32>, tMax: f32) -> f32 {
    let t0 = (node.boundsMin - ro) * invD;
    let t1 = (node.boundsMax - ro) * invD;
    let tn3 = min(t0, t1);
    let tf3 = max(t0, t1);
    let tn = max(max(max(tn3.x, tn3.y), tn3.z), 0.0);
    let tf = min(min(tf3.x, tf3.y), tf3.z);
    if (tf >= tn && tn < tMax) {
        return tn;
    }
    return -1.0;
}

// Closest hit of a local-space ray with the teapot: a nearest-child-first
// walk of the BVH and two-sided Moller-Trumbore triangle tests. The same
// traversal as TriangleMesh::intersect in lab/src/mesh.rs. With anyHit (shadow
// rays) it stops at the first triangle in range.
fn intersectMesh(lo: vec3<f32>, ld: vec3<f32>, tMax: f32, anyHit: bool) -> ObjectHit {
    var result = ObjectHit(false, tMax, vec3<f32>(0.0, 1.0, 0.0));
    let invD = 1.0 / select(ld, vec3<f32>(1e-9), abs(ld) < vec3<f32>(1e-9));
    var best = tMax;
    var bestTriangle = 0u;
    var bestUV = vec2<f32>(0.0);
    var stack: array<u32, MESH_STACK>;
    var sp = 1u;
    stack[0] = 0u;
    while (sp > 0u) {
        sp -= 1u;
        let node = gMeshNodes[stack[sp]];
        if (meshBoxEntry(node, lo, invD, best) < 0.0) {
            continue;
        }
        if (node.triangleCount > 0u) {
            for (var i = node.firstOrLeft; i < node.firstOrLeft + node.triangleCount; i++) {
                let tri = gMeshTriangles[i];
                let e1 = tri.p1 - tri.p0;
                let e2 = tri.p2 - tri.p0;
                let pv = cross(ld, e2);
                let det = dot(e1, pv);
                if (abs(det) < 1e-12) {
                    continue;
                }
                let inv = 1.0 / det;
                let tv = lo - tri.p0;
                let u = dot(tv, pv) * inv;
                if (u < 0.0 || u > 1.0) {
                    continue;
                }
                let qv = cross(tv, e1);
                let v = dot(ld, qv) * inv;
                if (v < 0.0 || u + v > 1.0) {
                    continue;
                }
                let t = dot(e2, qv) * inv;
                if (t > 1e-4 && t < best) {
                    best = t;
                    bestTriangle = i;
                    bestUV = vec2<f32>(u, v);
                    result.hit = true;
                    if (anyHit) {
                        result.t = t;
                        return result;
                    }
                }
            }
            continue;
        }
        let a = node.firstOrLeft;
        let b = a + 1u;
        var ta = meshBoxEntry(gMeshNodes[a], lo, invD, best);
        var tb = meshBoxEntry(gMeshNodes[b], lo, invD, best);
        var near = a;
        var far = b;
        // Visit the nearer child first: push it last.
        if (tb >= 0.0 && (ta < 0.0 || tb < ta)) {
            near = b;
            far = a;
            let swap = ta;
            ta = tb;
            tb = swap;
        }
        if (tb >= 0.0) {
            stack[sp] = far;
            sp += 1u;
        }
        if (ta >= 0.0) {
            stack[sp] = near;
            sp += 1u;
        }
    }
    if (result.hit) {
        let tri = gMeshTriangles[bestTriangle];
        var n = tri.n0 * (1.0 - bestUV.x - bestUV.y) + tri.n1 * bestUV.x + tri.n2 * bestUV.y;
        if (dot(n, n) <= 1e-12) {
            n = cross(tri.p1 - tri.p0, tri.p2 - tri.p0);
        }
        result.t = best;
        result.normal = normalize(n);
    }
    return result;
}

// Ray/object test in the object's local space. The direction is not
// renormalised, so the returned t is valid along the world-space ray.
fn intersectObject(o: ObjectData, ro: vec3<f32>, rd: vec3<f32>, tMax: f32) -> ObjectHit {
    var result = ObjectHit(false, tMax, vec3<f32>(0.0, 1.0, 0.0));
    let lo = transformPoint(o.worldToLocal, ro);
    let ld = transformVector(o.worldToLocal, rd);
    let c = o.localCenter;
    let e = o.localExtent;
    if (o.shapeKind == SHAPE_SPHERE) {
        let oc = lo - c;
        let a = dot(ld, ld);
        let b = dot(oc, ld);
        let k = dot(oc, oc) - e.x * e.x;
        let disc = b * b - a * k;
        if (disc < 0.0) {
            return result;
        }
        let s = sqrt(disc);
        var t = (-b - s) / a;
        if (t < 1e-4) {
            t = (-b + s) / a;
        }
        if (t < 1e-4 || t >= tMax) {
            return result;
        }
        result.hit = true;
        result.t = t;
        result.normal = normalToWorld(o, (lo + ld * t - c) / e.x);
        return result;
    }
    if (o.shapeKind == SHAPE_BOX) {
        let safeDir = select(ld, vec3<f32>(1e-9), abs(ld) < vec3<f32>(1e-9));
        let inv = 1.0 / safeDir;
        let t0 = (c - e - lo) * inv;
        let t1 = (c + e - lo) * inv;
        let tn3 = min(t0, t1);
        let tf3 = max(t0, t1);
        let tn = max(max(tn3.x, tn3.y), tn3.z);
        let tf = min(min(tf3.x, tf3.y), tf3.z);
        if (tf < tn) {
            return result;
        }
        let t = select(tf, tn, tn > 1e-4);
        if (t < 1e-4 || t >= tMax) {
            return result;
        }
        let p = (lo + ld * t - c) / e;
        let a = abs(p);
        var nLocal = vec3<f32>(0.0, 0.0, sign(p.z));
        if (a.x >= a.y && a.x >= a.z) {
            nLocal = vec3<f32>(sign(p.x), 0.0, 0.0);
        } else if (a.y >= a.z) {
            nLocal = vec3<f32>(0.0, sign(p.y), 0.0);
        }
        result.hit = true;
        result.t = t;
        result.normal = normalToWorld(o, nLocal);
        return result;
    }
    if (o.shapeKind == SHAPE_MESH) {
        result = intersectMesh(lo, ld, tMax, false);
        if (result.hit) {
            // The mesh is open (lid gap, spout): face the normal toward the ray.
            result.normal = normalToWorld(o, result.normal);
            if (dot(result.normal, rd) > 0.0) {
                result.normal = -result.normal;
            }
        }
        return result;
    }
    if (o.shapeKind == SHAPE_TERRAIN) {
        result = intersectTerrain(lo, ld, tMax);
        if (result.hit) {
            result.normal = normalToWorld(o, result.normal);
            if (dot(result.normal, rd) > 0.0) {
                result.normal = -result.normal;
            }
        }
        return result;
    }
    // Planes and the emissive panel are rectangles in the local XZ plane.
    if (abs(ld.y) < 1e-9) {
        return result;
    }
    let t = (c.y - lo.y) / ld.y;
    if (t < 1e-4 || t >= tMax) {
        return result;
    }
    let p = lo + ld * t;
    if (abs(p.x - c.x) > e.x || abs(p.z - c.z) > e.z) {
        return result;
    }
    var n = normalToWorld(o, vec3<f32>(0.0, 1.0, 0.0));
    if (dot(n, rd) > 0.0) {
        n = -n;
    }
    result.hit = true;
    result.t = t;
    result.normal = n;
    return result;
}

// Objects faded out of the frame for a clear view (gFrame.counts.z, one
// bit per object) are present to the degree gFrame.viewParams.x: at 0 they
// are not drawn, not hit, and cast no shadow; in between they fade.
fn objectPresence(i: u32) -> f32 {
    return select(1.0, gFrame.viewParams.x, i < 32u && (gFrame.counts.z & (1u << i)) != 0u);
}

fn traceScene(ro: vec3<f32>, rd: vec3<f32>, tMax: f32, skipObject: u32) -> SceneHit {
    return traceSceneSkipping(ro, rd, tMax, skipObject, NO_HIT);
}

fn traceSceneSkipping(ro: vec3<f32>, rd: vec3<f32>, tMax: f32, skipObject: u32, alsoSkip: u32) -> SceneHit {
    var hit = SceneHit(tMax, NO_HIT, vec3<f32>(0.0, 1.0, 0.0));
    let count = gFrame.counts.x;
    for (var i = 0u; i < count; i++) {
        if (i == skipObject || i == alsoSkip || objectPresence(i) <= 0.0) {
            continue;
        }
        let h = intersectObject(gObjects[i], ro, rd, hit.t);
        if (h.hit) {
            hit = SceneHit(h.t, i, h.normal);
        }
    }
    return hit;
}

// The share of the light a shadow ray lets through: none past an occluder,
// some past an object that is fading out of the frame.
fn shadowVisibility(p: vec3<f32>, dir: vec3<f32>, dist: f32, selfObject: u32) -> f32 {
    var visibility = 1.0;
    let count = gFrame.counts.x;
    for (var i = 0u; i < count; i++) {
        let presence = objectPresence(i);
        let o = gObjects[i];
        // A surface does not shadow itself, except the terrain, whose hills do.
        if ((i == selfObject && o.shapeKind != SHAPE_TERRAIN) || presence <= 0.0) {
            continue;
        }
        if (!occludesLight(o)) {
            continue;
        }
        var blocked = false;
        if (o.shapeKind == SHAPE_MESH) {
            // Any triangle in range blocks the light; no need for the closest.
            blocked = intersectMesh(transformPoint(o.worldToLocal, p), transformVector(o.worldToLocal, dir), dist, true).hit;
        } else if (i == selfObject) {
            // The terrain shadowing itself: start a little along the ray, past
            // the neighbouring triangles its smooth normal leans over.
            blocked = intersectObject(o, p + dir * TERRAIN_SELF_SHADOW_BIAS, dir, dist - TERRAIN_SELF_SHADOW_BIAS).hit;
        } else {
            blocked = intersectObject(o, p, dir, dist).hit;
        }
        if (blocked) {
            visibility *= 1.0 - presence;
            if (visibility <= 0.0) {
                return 0.0;
            }
        }
    }
    return visibility;
}

fn lightNormal() -> vec3<f32> {
    return normalize(cross(gFrame.lightAxisV.xyz, gFrame.lightAxisU.xyz));
}

// Irradiance from the emissive panel, sampled on a fixed 4x4 grid.
fn directIrradiance(p: vec3<f32>, n: vec3<f32>, selfObject: u32) -> vec3<f32> {
    let center = gFrame.lightCenterRadiance.xyz;
    let u = gFrame.lightAxisU.xyz;
    let v = gFrame.lightAxisV.xyz;
    let nl = lightNormal();
    let area = 4.0 * length(u) * length(v);
    let origin = p + n * 2e-3;
    var sum = 0.0;
    for (var j = 0; j < 4; j++) {
        for (var i = 0; i < 4; i++) {
            let s = center + u * ((f32(i) + 0.5) / 2.0 - 1.0) + v * ((f32(j) + 0.5) / 2.0 - 1.0);
            var w = s - origin;
            let d2 = dot(w, w);
            let d = sqrt(d2);
            w /= d;
            let cosS = dot(n, w);
            let cosL = dot(nl, -w);
            if (cosS <= 0.0 || cosL <= 0.0) {
                continue;
            }
            let visible = shadowVisibility(origin, w, d - 1e-3, selfObject);
            if (visible <= 0.0) {
                continue;
            }
            sum += cosS * cosL / d2 * visible;
        }
    }
    return gFrame.lightColor.rgb * gFrame.lightCenterRadiance.w * sum * area / 16.0;
}

// Cheap analytic occlusion from the sphere and boxes, for contact shading.
fn ambientVisibility(p: vec3<f32>, n: vec3<f32>, selfObject: u32) -> f32 {
    var visibility = 1.0;
    let count = gFrame.counts.x;
    for (var i = 0u; i < count; i++) {
        if (i == selfObject) {
            continue;
        }
        let o = gObjects[i];
        if (!occludesLight(o) || o.shapeKind == SHAPE_TERRAIN) {
            continue;
        }
        let toC = o.worldCenter - p;
        let d2 = dot(toC, toC);
        var r = 0.8 * min(min(o.worldExtent.x, o.worldExtent.y), o.worldExtent.z);
        if (o.shapeKind == SHAPE_SPHERE) {
            r = o.boundingRadius;
        }
        let cosA = dot(n, toC) * inverseSqrt(max(d2, 1e-6));
        visibility *= 1.0 - saturate(cosA) * saturate(r * r / max(d2, 1e-6)) * 0.85;
    }
    return visibility;
}

// The lab's environment (lab/src/environment.rs) along `dir` at one level:
// bilinear between texel centres, wrapping around the vertical axis and
// clamped at the poles, filtered by hand so it matches the CPU's lookup.
// The constants are environment.rs's TURN and LIGHTING_LEVEL.
const ENVIRONMENT_TURN: f32 = 0.0;
const ENVIRONMENT_LIGHTING_LEVEL: i32 = 2;

fn environment(dir: vec3<f32>, level: i32) -> vec3<f32> {
    let size = vec2<i32>(textureDimensions(gEnvironment, level));
    let d = normalize(dir);
    let u = (0.5 + atan2(d.x, -d.z) / (2.0 * PI) + ENVIRONMENT_TURN) * f32(size.x) - 0.5;
    let v = clamp(acos(clamp(d.y, -1.0, 1.0)) / PI * f32(size.y) - 0.5, 0.0, f32(size.y - 1));
    let x = floor(u);
    let y = floor(v);
    let f = vec2<f32>(u - x, v - y);
    let x0 = ((i32(x) % size.x) + size.x) % size.x;
    let x1 = (x0 + 1) % size.x;
    let y0 = i32(y);
    let y1 = min(y0 + 1, size.y - 1);
    let top = mix(textureLoad(gEnvironment, vec2<i32>(x0, y0), level).rgb, textureLoad(gEnvironment, vec2<i32>(x1, y0), level).rgb, f.x);
    let bottom = mix(textureLoad(gEnvironment, vec2<i32>(x0, y1), level).rgb, textureLoad(gEnvironment, vec2<i32>(x1, y1), level).rgb, f.x);
    return mix(top, bottom, f.y);
}

// The sky as lighting sees it (scene::environment_radiance).
fn environmentRadiance(dir: vec3<f32>) -> vec3<f32> {
    return gFrame.ambientColor.rgb * environment(dir, ENVIRONMENT_LIGHTING_LEVEL);
}

// The sky as the backdrop shows it: the environment's full detail.
fn skyRadiance(dir: vec3<f32>) -> vec3<f32> {
    return gFrame.ambientColor.rgb * environment(dir, 0);
}

// The world direction through a point of the viewport (uv 0..1, y down).
fn viewRay(uv: vec2<f32>) -> vec3<f32> {
    let c = gFrame.camera;
    let d = vec3<f32>((uv.x * 2.0 - 1.0) / c.viewToClip[0][0], (1.0 - uv.y * 2.0) / c.viewToClip[1][1], -1.0);
    // The view rotation's transpose.
    let r = c.worldToView;
    return normalize(vec3<f32>(dot(r[0].xyz, d), dot(r[1].xyz, d), dot(r[2].xyz, d)));
}

// ---------------------------------------------------------------------------
// The default study-room shading, for an investigation without its own
// (SceneShading). Indirect light is a flat ambient term, scaled by
// gFrame.viewParams.y.
fn indirectIrradiance(n: vec3<f32>) -> vec3<f32> {
    return gFrame.ambientColor.rgb * (0.55 + 0.45 * n.y) * PI * gFrame.viewParams.y;
}

// Radiance leaving a non-reflective surface: direct light from the panel,
// the ambient term, and emission.
fn shadeSurface(p: vec3<f32>, n: vec3<f32>, objectIndex: u32) -> vec3<f32> {
    let o = gObjects[objectIndex];
    let albedo = o.albedoRoughness.rgb;
    return albedo * (directIrradiance(p, n, objectIndex) + indirectIrradiance(n)) / PI + o.emissive.rgb * o.emissive.w;
}

// ---------------------------------------------------------------------------
// Reflections. The default shading traces the floor's mirror direction only.

// The half vector of a visible GGX normal, sampled with spherical caps
// (Dupuy & Benyoub, "Sampling Visible GGX Normals with Spherical Caps", 2023)
// over the bounded range of Eto & Tokuyoshi ("Bounded VNDF Sampling for
// Smith-GGX Reflections", 2023), which keeps most reflections above the
// horizon. V and H are in tangent space, +z along the normal.
fn importanceSampleVisibleGGX(e: vec2<f32>, alpha: f32, v: vec3<f32>) -> vec3<f32> {
    let vh = normalize(vec3<f32>(alpha * v.xy, v.z));
    let phi = 2.0 * PI * e.x;
    let a = saturate(alpha);
    let s = 1.0 + length(v.xy);
    let a2 = a * a;
    let s2 = s * s;
    let k = (s2 - a2 * s2) / (s2 + a2 * v.z * v.z);
    let z = mix(1.0, -k * vh.z, e.y);
    let sinTheta = sqrt(saturate(1.0 - z * z));
    let h = vec3<f32>(sinTheta * cos(phi), sinTheta * sin(phi), z) + vh;
    return normalize(vec3<f32>(alpha * h.xy, max(0.0, h.z)));
}

fn tangentBasis(n: vec3<f32>) -> mat3x3<f32> {
    let up = select(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 1.0, 0.0), abs(n.y) < 0.99);
    let t = normalize(cross(up, n));
    return mat3x3<f32>(t, cross(n, t), n);
}

// Interleaved gradient noise (Jimenez, "Next Generation Post Processing in Call of Duty: Advanced Warfare", 2014).
fn interleavedGradientNoise(pixel: vec2<f32>) -> f32 {
    return fract(52.9829189 * fract(dot(pixel, vec2<f32>(0.06711056, 0.00583715))));
}

fn hammersley(k: u32, count: u32) -> vec2<f32> {
    return vec2<f32>((f32(k) + 0.5) / f32(count), f32(reverseBits(k)) * 2.3283064365386963e-10);
}

// Sample k of `count`: a Hammersley point, rotated per pixel.
fn sampleSequence(k: u32, count: u32, pixel: vec2<f32>) -> vec2<f32> {
    let h = hammersley(k, count);
    let rotation = vec2<f32>(interleavedGradientNoise(pixel), interleavedGradientNoise(pixel + vec2<f32>(37.0, 17.0)));
    return fract(h + rotation);
}

// Radiance along one reflection ray: the sky on a miss. The default shading
// keeps no radiance for the surfaces a ray hits, so a hit is black.
fn hitRadiance(hit: SceneHit, dir: vec3<f32>) -> vec3<f32> {
    if (hit.object == NO_HIT) {
        return environmentRadiance(dir);
    }
    return vec3<f32>(0.0);
}

fn reflectionRayRadiance(origin: vec3<f32>, dir: vec3<f32>, selfObject: u32) -> vec3<f32> {
    let hit = traceScene(origin, dir, 100.0, selfObject);
    var radiance = hitRadiance(hit, dir);
    // An object fading out of the frame is seen partly through.
    if (hit.object != NO_HIT && objectPresence(hit.object) < 1.0) {
        let behind = traceSceneSkipping(origin, dir, 100.0, selfObject, hit.object);
        radiance = mix(hitRadiance(behind, dir), radiance, objectPresence(hit.object));
    }
    return radiance;
}

// The floor's reflection: the radiance along the mirror direction.
fn reflectedRadiance(p: vec3<f32>, n: vec3<f32>, incident: vec3<f32>, selfObject: u32) -> vec3<f32> {
    let dir = reflect(incident, n);
    if (dot(dir, n) <= 0.0) {
        return vec3<f32>(0.0);
    }
    return reflectionRayRadiance(p + n * 2e-3, dir, selfObject);
}

// The analytic fit of the pre-integrated split-sum environment BRDF that
// Karis published ("Physically Based Shading on Mobile", 2014).
fn envBRDFApprox(f0: f32, roughness: f32, noV: f32) -> f32 {
    let c0 = vec4<f32>(-1.0, -0.0275, -0.572, 0.022);
    let c1 = vec4<f32>(1.0, 0.0425, 1.04, -0.04);
    let r = roughness * c0 + c1;
    let a004 = min(r.x * r.x, exp2(-9.28 * noV)) * r.x + r.y;
    let ab = vec2<f32>(-1.04, 1.04) * a004 + r.zw;
    return f0 * ab.x + saturate(50.0 * f0) * ab.y;
}

// The floor: diffuse lighting, plus its mirror reflection weighted by the
// environment BRDF at the floor's roughness.
fn shadeFloor(p: vec3<f32>, n: vec3<f32>, eye: vec3<f32>, floorIndex: u32) -> vec3<f32> {
    let incident = normalize(p - eye);
    let roughness = gFrame.floorParams.x;
    let o = gObjects[floorIndex];
    let diffuse = o.albedoRoughness.rgb * (directIrradiance(p, n, floorIndex) + indirectIrradiance(n)) / PI;
    let specular = reflectedRadiance(p, n, incident, floorIndex);
    let envBRDF = envBRDFApprox(gFrame.floorParams.y, roughness, saturate(dot(n, -incident)));
    return diffuse + specular * envBRDF * gFrame.floorParams.z;
}

fn linearToSrgb(c: vec3<f32>) -> vec3<f32> {
    let x = saturate(c);
    return select(1.055 * pow(x, vec3<f32>(1.0 / 2.4)) - 0.055, x * 12.92, x <= vec3<f32>(0.0031308));
}

fn srgbToLinear(c: vec3<f32>) -> vec3<f32> {
    return select(pow((c + 0.055) / 1.055, vec3<f32>(2.4)), c / 12.92, c <= vec3<f32>(0.04045));
}

// Narkowicz's ACES fit.
fn tonemap(hdr: vec3<f32>) -> vec3<f32> {
    let x = hdr * gFrame.overlayParams.x;
    return saturate((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14));
}

// Diverging colormap for signed distances: warm inside, cool outside.
// Matches palette::INSIDE, SURFACE, and OUTSIDE in lab/src/palette.rs.
fn distanceColor(normalizedDistance: f32) -> vec3<f32> {
    let s = clamp(normalizedDistance, -1.0, 1.0);
    let inside = vec3<f32>(0.980, 0.420, 0.180);
    let surface = vec3<f32>(0.969, 0.949, 0.878);
    let outside = vec3<f32>(0.200, 0.549, 0.980);
    if (s < 0.0) {
        return mix(surface, inside, pow(-s, 0.6));
    }
    return mix(surface, outside, pow(s, 0.6));
}
