//! `mesh`: a Wavefront OBJ of `v/vt/vn` triangles, scaled by `scale` and
//! arranged in a bounding-volume hierarchy of at most `leaf-triangles`
//! triangles a leaf and `max-depth` levels (src/mesh.rs reads it; the
//! raster pass draws its triangles, and intersectMesh in common.wgsl and
//! `TriangleMesh` traverse its nodes).
//!
//! Output, little-endian: the node count, triangle count, and depth, then the
//! nodes and the triangles in leaf order, laid out as types.wgsl's MeshNode
//! (32 bytes) and MeshTriangle (96 bytes).

use super::{put_f32, put_u32, Kind, Params, Ty};
use glam::Vec3;

pub const KIND: Kind = Kind {
    name: "mesh",
    params: &[("scale", Ty::Float), ("leaf-triangles", Ty::Int), ("max-depth", Ty::Int)],
    code: &[include_str!("mesh.rs")],
    process,
};

#[derive(Clone, Copy, Default)]
struct Node {
    bounds_min: Vec3,
    first_or_left: u32,
    bounds_max: Vec3,
    triangle_count: u32,
}

fn process(file: &[u8], params: &Params) -> Result<Vec<u8>, String> {
    let text = std::str::from_utf8(file).map_err(|_| "the OBJ is not text")?;
    let triangles = parse_obj(text, params.float("scale"))?;
    if triangles.is_empty() {
        return Err("the OBJ has no triangles".into());
    }
    let (leaf, max_depth) = (params.int("leaf-triangles").max(1), params.int("max-depth"));
    let (nodes, order, depth) = build(&triangles, leaf);
    if depth > max_depth {
        return Err(format!("the BVH is {depth} levels deep (max-depth is {max_depth})"));
    }
    let mut out = Vec::new();
    put_u32(&mut out, &[nodes.len() as u32, order.len() as u32, depth as u32]);
    for n in &nodes {
        put_f32(&mut out, &n.bounds_min.to_array());
        put_u32(&mut out, &[n.first_or_left]);
        put_f32(&mut out, &n.bounds_max.to_array());
        put_u32(&mut out, &[n.triangle_count]);
    }
    for &i in &order {
        let (p, n) = triangles[i as usize];
        for v in p.iter().chain(&n) {
            put_f32(&mut out, &[v.x, v.y, v.z, 0.0]);
        }
    }
    Ok(out)
}

/// Triangles as (positions, vertex normals).
fn parse_obj(text: &str, scale: f32) -> Result<Vec<([Vec3; 3], [Vec3; 3])>, String> {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut triangles = Vec::new();
    for (number, line) in text.lines().enumerate() {
        let fail = |what: &str| format!("OBJ line {}: {what}", number + 1);
        let mut parts = line.split_whitespace();
        let vec3 = |parts: &mut std::str::SplitWhitespace| -> Result<Vec3, String> {
            let mut v = [0.0f32; 3];
            for c in &mut v {
                *c = parts.next().and_then(|x| x.parse().ok()).ok_or_else(|| fail("three numbers"))?;
            }
            Ok(Vec3::from(v))
        };
        match parts.next() {
            Some("v") => positions.push(vec3(&mut parts)? * scale),
            Some("vn") => normals.push(vec3(&mut parts)?.normalize_or_zero()),
            Some("f") => {
                let corners = parts
                    .map(|c| {
                        let mut ids = c.split('/');
                        let v: usize = ids.next().and_then(|x| x.parse().ok()).ok_or_else(|| fail("a vertex index"))?;
                        let n: usize = ids.nth(1).and_then(|x| x.parse().ok()).ok_or_else(|| fail("a normal index (faces are v/vt/vn)"))?;
                        match (positions.get(v.wrapping_sub(1)), normals.get(n.wrapping_sub(1))) {
                            (Some(&p), Some(&n)) => Ok((p, n)),
                            _ => Err(fail("an index past the vertices or normals declared before it")),
                        }
                    })
                    .collect::<Result<Vec<(Vec3, Vec3)>, String>>()?;
                let [a, b, c] = corners[..] else { return Err(fail("only triangles are supported")) };
                triangles.push(([a.0, b.0, c.0], [a.1, b.1, c.1]));
            }
            _ => {}
        }
    }
    Ok(triangles)
}

/// Median split on the longest axis of the centroid bounds. Ties keep the
/// file order, so the tree (and every traversal) is the same on each build.
/// Returns the nodes, the triangles' order, and the depth.
fn build(triangles: &[([Vec3; 3], [Vec3; 3])], leaf: usize) -> (Vec<Node>, Vec<u32>, usize) {
    let centroids: Vec<Vec3> = triangles.iter().map(|(p, _)| (p[0] + p[1] + p[2]) / 3.0).collect();
    let mut order: Vec<u32> = (0..triangles.len() as u32).collect();
    let mut nodes = vec![Node::default()];
    let mut depth = 0;
    // (node, start, end, depth)
    let mut work = vec![(0usize, 0usize, triangles.len(), 1usize)];
    while let Some((node, start, end, level)) = work.pop() {
        depth = depth.max(level);
        let (mut lo, mut hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
        let (mut clo, mut chi) = (lo, hi);
        for &i in &order[start..end] {
            for p in triangles[i as usize].0 {
                lo = lo.min(p);
                hi = hi.max(p);
            }
            clo = clo.min(centroids[i as usize]);
            chi = chi.max(centroids[i as usize]);
        }
        nodes[node].bounds_min = lo;
        nodes[node].bounds_max = hi;
        if end - start <= leaf {
            nodes[node].first_or_left = start as u32;
            nodes[node].triangle_count = (end - start) as u32;
            continue;
        }
        let span = chi - clo;
        let axis = if span.x >= span.y && span.x >= span.z { 0 } else if span.y >= span.z { 1 } else { 2 };
        order[start..end].sort_by(|&a, &b| centroids[a as usize][axis].total_cmp(&centroids[b as usize][axis]).then(a.cmp(&b)));
        let mid = (start + end) / 2;
        let left = nodes.len();
        nodes.push(Node::default());
        nodes.push(Node::default());
        nodes[node].first_or_left = left as u32;
        nodes[node].triangle_count = 0;
        work.push((left + 1, mid, end, level + 1));
        work.push((left, start, mid, level + 1));
    }
    (nodes, order, depth)
}
