//! The Utah teapot (packages/rlstd/assets/models/utah_teapot.obj, Cem Yuksel's 2026
//! triangle mesh), as the resource pipeline makes it (src/pipeline/mesh.rs,
//! declared in src/resources.toml): scaled into the room's metres and
//! arranged in a bounding-volume hierarchy. The raster pass draws its
//! triangles; intersectMesh in common.wgsl and `TriangleMesh::intersect` below
//! traverse the same nodes for shadow, reflection, probe, and card rays.

use crate::layout::{MeshNode, MeshTriangle};
use crate::resources::{self, Loaded};
use glam::Vec3;
use std::sync::OnceLock;

/// Traversal stack size in intersectMesh; the resource's `max-depth`, which
/// the pipeline checks the tree against.
pub const MAX_DEPTH: usize = 30;

pub struct TriangleMesh {
    pub nodes: Vec<MeshNode>,
    /// In leaf order: node `firstOrLeft .. firstOrLeft + triangleCount`.
    pub triangles: Vec<MeshTriangle>,
    pub bounds_min: Vec3,
    pub bounds_max: Vec3,
    pub depth: usize,
}

pub fn teapot() -> &'static TriangleMesh {
    static TEAPOT: OnceLock<TriangleMesh> = OnceLock::new();
    TEAPOT.get_or_init(|| TriangleMesh::load(&resources::get("rlstd.teapot").load()))
}

fn v3(a: [f32; 3]) -> Vec3 {
    Vec3::from(a)
}

impl TriangleMesh {
    /// A `mesh` resource: its header (node count, triangle count, depth), then
    /// the nodes and the triangles.
    fn load(r: &Loaded) -> TriangleMesh {
        let (node_count, triangle_count, depth) = (r.word(0) as usize, r.word(1) as usize, r.word(2) as usize);
        assert!(depth <= MAX_DEPTH, "{}: the BVH is {depth} levels deep (stack holds {MAX_DEPTH})", r.id());
        let nodes: Vec<MeshNode> = r.array(12, node_count);
        let triangles: Vec<MeshTriangle> = r.array(12 + node_count * size_of::<MeshNode>(), triangle_count);
        assert_eq!(r.bytes.len(), 12 + node_count * size_of::<MeshNode>() + triangle_count * size_of::<MeshTriangle>(), "{}: size", r.id());
        let root = nodes[0];
        TriangleMesh { nodes, triangles, bounds_min: v3(root.bounds_min), bounds_max: v3(root.bounds_max), depth }
    }

    pub fn local_center(&self) -> Vec3 {
        (self.bounds_min + self.bounds_max) * 0.5
    }

    pub fn local_extent(&self) -> Vec3 {
        (self.bounds_max - self.bounds_min) * 0.5
    }

    /// Closest hit in local space: `(t, interpolated normal)`. `rd` need not
    /// be normalised, so `t` stays valid along a transformed world ray.
    pub fn intersect(&self, ro: Vec3, rd: Vec3, t_max: f32) -> Option<(f32, Vec3)> {
        self.traverse(ro, rd, t_max, false)
    }

    /// Whether any triangle lies on the ray before `t_max` (shadow rays).
    pub fn occludes(&self, ro: Vec3, rd: Vec3, t_max: f32) -> bool {
        self.traverse(ro, rd, t_max, true).is_some()
    }

    fn traverse(&self, ro: Vec3, rd: Vec3, t_max: f32, any_hit: bool) -> Option<(f32, Vec3)> {
        let inv = safe_inverse(rd);
        let mut best: Option<(f32, u32, f32, f32)> = None;
        let mut t_best = t_max;
        let mut stack = [0u32; MAX_DEPTH + 2];
        let mut sp = 1;
        while sp > 0 {
            sp -= 1;
            let node = &self.nodes[stack[sp] as usize];
            if box_entry(node, ro, inv, t_best).is_none() {
                continue;
            }
            if node.triangle_count > 0 {
                for i in node.first_or_left..node.first_or_left + node.triangle_count {
                    if let Some((t, u, v)) = intersect_triangle(&self.triangles[i as usize], ro, rd, t_best) {
                        t_best = t;
                        best = Some((t, i, u, v));
                        if any_hit {
                            return Some((t, self.triangle_normal(i as usize, u, v)));
                        }
                    }
                }
                continue;
            }
            // Visit the nearer child first: push it last.
            let (a, b) = (node.first_or_left, node.first_or_left + 1);
            let ta = box_entry(&self.nodes[a as usize], ro, inv, t_best);
            let tb = box_entry(&self.nodes[b as usize], ro, inv, t_best);
            let (near, far, t_near, t_far) = if tb.unwrap_or(f32::INFINITY) < ta.unwrap_or(f32::INFINITY) { (b, a, tb, ta) } else { (a, b, ta, tb) };
            if t_far.is_some() {
                stack[sp] = far;
                sp += 1;
            }
            if t_near.is_some() {
                stack[sp] = near;
                sp += 1;
            }
        }
        let (t, i, u, v) = best?;
        Some((t, self.triangle_normal(i as usize, u, v)))
    }

    /// Reference for the selftest: every triangle, no hierarchy.
    pub fn intersect_brute(&self, ro: Vec3, rd: Vec3, t_max: f32) -> Option<(f32, Vec3)> {
        let mut best: Option<(f32, usize, f32, f32)> = None;
        let mut t_best = t_max;
        for (i, tri) in self.triangles.iter().enumerate() {
            if let Some((t, u, v)) = intersect_triangle(tri, ro, rd, t_best) {
                t_best = t;
                best = Some((t, i, u, v));
            }
        }
        let (t, i, u, v) = best?;
        Some((t, self.triangle_normal(i, u, v)))
    }

    /// The nearest hit with `t_min < t < t_max`, skipping triangle `skip`:
    /// `(t, interpolated normal, triangle)`. Rays that continue through the
    /// mesh (card generation's surfel rays) step past each hit this way.
    pub fn intersect_after(&self, ro: Vec3, rd: Vec3, t_min: f32, t_max: f32, skip: Option<u32>) -> Option<(f32, Vec3, u32)> {
        let inv = safe_inverse(rd);
        let mut best: Option<(f32, u32, f32, f32)> = None;
        let mut t_best = t_max;
        let mut stack = [0u32; MAX_DEPTH + 2];
        let mut sp = 1;
        while sp > 0 {
            sp -= 1;
            let node = &self.nodes[stack[sp] as usize];
            if box_entry(node, ro, inv, t_best).is_none() {
                continue;
            }
            if node.triangle_count > 0 {
                for i in node.first_or_left..node.first_or_left + node.triangle_count {
                    if Some(i) == skip {
                        continue;
                    }
                    if let Some((t, u, v)) = intersect_triangle(&self.triangles[i as usize], ro, rd, t_best) {
                        if t > t_min {
                            t_best = t;
                            best = Some((t, i, u, v));
                        }
                    }
                }
                continue;
            }
            for child in [node.first_or_left, node.first_or_left + 1] {
                stack[sp] = child;
                sp += 1;
            }
        }
        let (t, i, u, v) = best?;
        Some((t, self.triangle_normal(i as usize, u, v), i))
    }

    /// The unsigned distance from `p` to the mesh and the closest point
    /// (branch and bound over the BVH; the mesh is open, so it has no inside).
    pub fn closest_point(&self, p: Vec3) -> (f32, Vec3) {
        let mut best = (f32::INFINITY, p);
        let mut stack = [0u32; MAX_DEPTH + 2];
        let mut sp = 1;
        while sp > 0 {
            sp -= 1;
            let node = &self.nodes[stack[sp] as usize];
            let outside = (v3(node.bounds_min) - p).max(p - v3(node.bounds_max)).max(Vec3::ZERO);
            if outside.length_squared() >= best.0 * best.0 {
                continue;
            }
            if node.triangle_count > 0 {
                for i in node.first_or_left..node.first_or_left + node.triangle_count {
                    let tri = &self.triangles[i as usize];
                    let q = closest_point_on_triangle(p, v3(tri.p0), v3(tri.p1), v3(tri.p2));
                    let d = (q - p).length();
                    if d < best.0 {
                        best = (d, q);
                    }
                }
                continue;
            }
            for child in [node.first_or_left, node.first_or_left + 1] {
                stack[sp] = child;
                sp += 1;
            }
        }
        best
    }

    fn triangle_normal(&self, i: usize, u: f32, v: f32) -> Vec3 {
        let tri = &self.triangles[i];
        let n = v3(tri.n0) * (1.0 - u - v) + v3(tri.n1) * u + v3(tri.n2) * v;
        if n.length_squared() > 1e-12 { n.normalize() } else { (v3(tri.p1) - v3(tri.p0)).cross(v3(tri.p2) - v3(tri.p0)).normalize_or_zero() }
    }
}

fn safe_inverse(d: Vec3) -> Vec3 {
    let safe = Vec3::select(d.abs().cmplt(Vec3::splat(1e-9)), Vec3::splat(1e-9), d);
    Vec3::ONE / safe
}

/// Entry distance of the ray into a node's box, if it enters before `t_max`.
fn box_entry(node: &MeshNode, ro: Vec3, inv: Vec3, t_max: f32) -> Option<f32> {
    let t0 = (v3(node.bounds_min) - ro) * inv;
    let t1 = (v3(node.bounds_max) - ro) * inv;
    let tn = t0.min(t1).max_element().max(0.0);
    let tf = t0.max(t1).min_element();
    (tf >= tn && tn < t_max).then_some(tn)
}

/// Closest point on triangle abc to p (Ericson, Real-Time Collision Detection 5.1.5).
fn closest_point_on_triangle(p: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
    let (ab, ac, ap) = (b - a, c - a, p - a);
    let (d1, d2) = (ab.dot(ap), ac.dot(ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = p - b;
    let (d3, d4) = (ab.dot(bp), ac.dot(bp));
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return a + ab * (d1 / (d1 - d3));
    }
    let cp = p - c;
    let (d5, d6) = (ab.dot(cp), ac.dot(cp));
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return a + ac * (d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        return b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }
    let denom = 1.0 / (va + vb + vc);
    a + ab * (vb * denom) + ac * (vc * denom)
}

/// Möller–Trumbore, both faces: `(t, u, v)` for hits with 1e-4 < t < t_max.
fn intersect_triangle(tri: &MeshTriangle, ro: Vec3, rd: Vec3, t_max: f32) -> Option<(f32, f32, f32)> {
    let p0 = v3(tri.p0);
    let e1 = v3(tri.p1) - p0;
    let e2 = v3(tri.p2) - p0;
    let pv = rd.cross(e2);
    let det = e1.dot(pv);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let tv = ro - p0;
    let u = tv.dot(pv) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let qv = tv.cross(e1);
    let v = rd.dot(qv) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = e2.dot(qv) * inv;
    (t > 1e-4 && t < t_max).then_some((t, u, v))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn teapot_parses_and_builds() {
        let m = teapot();
        assert_eq!(m.triangles.len(), 35634);
        assert!(m.depth <= MAX_DEPTH);
        let h = m.bounds_max.y - m.bounds_min.y;
        assert!((h - 0.441).abs() < 1e-3, "height {h}");
        // A ray straight down onto the lid hits the top of the teapot.
        let hit = m.intersect(Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, -1.0, 0.0), 10.0).expect("hit");
        assert!((1.0 - hit.0 - m.bounds_max.y).abs() < 0.01);
    }
}
