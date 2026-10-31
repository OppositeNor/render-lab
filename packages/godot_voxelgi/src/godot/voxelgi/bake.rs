//! The bake, on the CPU, as Godot's `Voxelizer` runs it (scene/3d/voxelizer.cpp,
//! called by `VoxelGI::bake`, scene/3d/voxel_gi.cpp): `begin_bake` makes the
//! box a power-of-two cube of cells, `plot_mesh` sends every triangle down the
//! octree from the root (`_plot_face`: a child only where the triangle
//! overlaps it, `Geometry3D::triangle_box_overlap`), the leaves sample the
//! triangle on a 4 x 4 grid of columns, `_sort` orders the cells by level then
//! position, and `_fixup_plot` averages the leaves and empties the parents.
//! `get_voxel_gi_octree_cells` and `get_voxel_gi_data_cells` pack the records
//! the renderer uploads (storage.rs decodes them).
//!
//! Where the lab differs: the room's materials are constant colours (no
//! textures), given as the linear colour `srgb_to_linear` would make; meshes
//! have no UVs, so the material cache's 128 x 128 table is one colour.
//!
//! Ported from Godot Engine (MIT; Copyright (c) 2014-present Godot Engine
//! contributors, Copyright (c) 2007-2014 Juan Linietsky, Ariel Manzur).

use glam::{Mat3, Mat4, Vec3};

/// `CHILD_EMPTY` (voxelizer.h): no child in that octant.
pub const CHILD_EMPTY: u32 = 0xFFFF_FFFF;
/// `color_scan_cell_width` (voxelizer.h): a leaf samples its triangle on 4 x 4 columns.
pub const COLOR_SCAN_CELL_WIDTH: usize = 4;
/// `CMP_EPSILON` (core/math/math_defs.h).
const CMP_EPSILON: f32 = 0.00001;

/// A `Voxelizer::Cell`: its children (indices into the cell array), the sums
/// a leaf accumulates (averaged by `_fixup_plot`), and its place.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cell {
    pub children: [u32; 8],
    pub albedo: [f32; 3],
    pub emission: [f32; 3],
    pub normal: [f32; 3],
    pub alpha: f32,
    pub x: u16,
    pub y: u16,
    pub z: u16,
    pub level: u16,
}

impl Default for Cell {
    fn default() -> Cell {
        Cell { children: [CHILD_EMPTY; 8], albedo: [0.0; 3], emission: [0.0; 3], normal: [0.0; 3], alpha: 0.0, x: 0, y: 0, z: 0, level: 0 }
    }
}

/// An axis-aligned box (Godot's `AABB`): its minimum corner and size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    pub position: Vec3,
    pub size: Vec3,
}

impl Aabb {
    pub fn center(&self) -> Vec3 {
        self.position + self.size * 0.5
    }
}

/// One triangle as `plot_mesh` hands it to `_plot_face`: vertices in the
/// VoxelGI's local space, and their normals.
#[derive(Clone, Copy, Debug)]
pub struct Triangle {
    pub vtx: [Vec3; 3],
    pub normal: [Vec3; 3],
}

/// A mesh the bake plots: its triangles (already transformed) and its
/// material's colours (the `MaterialCache` of a material without textures:
/// every texel the albedo, and the emission times its energy).
pub struct BakeMesh {
    pub object: usize,
    pub triangles: Vec<Triangle>,
    pub albedo: Vec3,
    pub emission: Vec3,
}

/// The `Voxelizer`'s state from `begin_bake` to `end_bake`.
#[derive(Clone)]
pub struct Voxelizer {
    pub cell_subdiv: i32,
    pub original_bounds: Aabb,
    pub po2_bounds: Aabb,
    pub axis_cell_size: [i32; 3],
    /// `to_cell_space`: local position to cells, a uniform scale and an offset.
    pub to_cell_scale: f32,
    pub to_cell_origin: Vec3,
    pub cell_size: f32,
    pub cells: Vec<Cell>,
    pub leaf_voxel_count: usize,
    pub sorted: bool,
}

/// `Math::is_equal_approx` for two floats.
fn approx(a: f32, b: f32) -> bool {
    if a == b {
        return true;
    }
    let tolerance = (CMP_EPSILON * a.abs()).max(CMP_EPSILON);
    (a - b).abs() < tolerance
}

fn approx3(a: Vec3, b: Vec3) -> bool {
    approx(a.x, b.x) && approx(a.y, b.y) && approx(a.z, b.z)
}

/// `get_uv_and_normal` without UVs: the interpolated, normalised normal at
/// `pos` (a vertex's own when `pos` is one).
fn normal_at(pos: Vec3, vtx: &[Vec3; 3], normal: &[Vec3; 3]) -> Vec3 {
    for k in 0..3 {
        if approx3(pos, vtx[k]) {
            return normal[k];
        }
    }
    let v0 = vtx[1] - vtx[0];
    let v1 = vtx[2] - vtx[0];
    let v2 = pos - vtx[0];
    let d00 = v0.dot(v0);
    let d01 = v0.dot(v1);
    let d11 = v1.dot(v1);
    let d20 = v2.dot(v0);
    let d21 = v2.dot(v1);
    let denom = d00 * d11 - d01 * d01;
    if denom == 0.0 {
        return normal[0];
    }
    let v = (d11 * d20 - d01 * d21) / denom;
    let w = (d00 * d21 - d01 * d20) / denom;
    let u = 1.0 - v - w;
    normalized(normal[0] * u + normal[1] * v + normal[2] * w)
}

/// `Vector3::normalized`: zero stays zero.
pub fn normalized(v: Vec3) -> Vec3 {
    let l = v.length_squared();
    if l == 0.0 { Vec3::ZERO } else { v / l.sqrt() }
}

/// `planeBoxOverlap` (core/math/geometry_3d.h).
fn plane_box_overlap(normal: Vec3, d: f32, maxbox: Vec3) -> bool {
    let mut vmin = Vec3::ZERO;
    let mut vmax = Vec3::ZERO;
    for q in 0..3 {
        if normal[q] > 0.0 {
            vmin[q] = -maxbox[q];
            vmax[q] = maxbox[q];
        } else {
            vmin[q] = maxbox[q];
            vmax[q] = -maxbox[q];
        }
    }
    if normal.dot(vmin) + d > 0.0 {
        return false;
    }
    normal.dot(vmax) + d >= 0.0
}

/// How far `triangle_box_overlap` got before it returned: the stage a demonstration shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SatStage {
    /// One of the nine edge x axis tests separated them (its index, 0 to 8).
    EdgeAxis(usize),
    /// The box's own axis (0 x, 1 y, 2 z) separated them.
    BoxAxis(usize),
    /// The triangle's plane misses the box.
    Plane,
    /// No axis separates them: they overlap.
    Overlap,
}

/// `Geometry3D::triangle_box_overlap`: the separating-axis test, in the
/// engine's order (the nine edge tests, the three box axes, the plane).
pub fn triangle_box_overlap(boxcenter: Vec3, boxhalfsize: Vec3, triverts: &[Vec3; 3]) -> bool {
    triangle_box_stage(boxcenter, boxhalfsize, triverts) == SatStage::Overlap
}

pub fn triangle_box_stage(boxcenter: Vec3, h: Vec3, triverts: &[Vec3; 3]) -> SatStage {
    let v0 = triverts[0] - boxcenter;
    let v1 = triverts[1] - boxcenter;
    let v2 = triverts[2] - boxcenter;
    let e0 = v1 - v0;
    let e1 = v2 - v1;
    let e2 = v0 - v2;
    // The AXISTEST_* macros: p = a * v.(second) - b * v.(third), and the box's radius on that axis.
    let range = |p: f32, q: f32, rad: f32| {
        let (min, max) = if p < q { (p, q) } else { (q, p) };
        !(min > rad || max < -rad)
    };
    let x_test = |a: f32, b: f32, fa: f32, fb: f32, s: Vec3, t: Vec3| range(a * s.y - b * s.z, a * t.y - b * t.z, fa * h.y + fb * h.z);
    let y_test = |a: f32, b: f32, fa: f32, fb: f32, s: Vec3, t: Vec3| range(-a * s.x + b * s.z, -a * t.x + b * t.z, fa * h.x + fb * h.z);
    let z_test = |a: f32, b: f32, fa: f32, fb: f32, s: Vec3, t: Vec3| range(a * s.x - b * s.y, a * t.x - b * t.y, fa * h.x + fb * h.y);
    let tests: [bool; 9] = {
        let f = e0.abs();
        let r0 = x_test(e0.z, e0.y, f.z, f.y, v0, v2); // AXISTEST_X01
        let r1 = y_test(e0.z, e0.x, f.z, f.x, v0, v2); // AXISTEST_Y02
        let r2 = z_test(e0.y, e0.x, f.y, f.x, v1, v2); // AXISTEST_Z12
        let f = e1.abs();
        let r3 = x_test(e1.z, e1.y, f.z, f.y, v0, v2); // AXISTEST_X01
        let r4 = y_test(e1.z, e1.x, f.z, f.x, v0, v2); // AXISTEST_Y02
        let r5 = z_test(e1.y, e1.x, f.y, f.x, v0, v1); // AXISTEST_Z0
        let f = e2.abs();
        let r6 = x_test(e2.z, e2.y, f.z, f.y, v0, v1); // AXISTEST_X2
        let r7 = y_test(e2.z, e2.x, f.z, f.x, v0, v1); // AXISTEST_Y1
        let r8 = z_test(e2.y, e2.x, f.y, f.x, v1, v2); // AXISTEST_Z12
        [r0, r1, r2, r3, r4, r5, r6, r7, r8]
    };
    if let Some(k) = tests.iter().position(|&t| !t) {
        return SatStage::EdgeAxis(k);
    }
    for axis in 0..3 {
        let (a, b, c) = (v0[axis], v1[axis], v2[axis]);
        let min = a.min(b).min(c);
        let max = a.max(b).max(c);
        if min > h[axis] || max < -h[axis] {
            return SatStage::BoxAxis(axis);
        }
    }
    let normal = e0.cross(e1);
    let d = -normal.dot(v0);
    if plane_box_overlap(normal, d, h) { SatStage::Overlap } else { SatStage::Plane }
}

/// `Face3::get_closest_point_to`.
pub fn closest_point_on_triangle(vertex: &[Vec3; 3], p: Vec3) -> Vec3 {
    let edge0 = vertex[1] - vertex[0];
    let edge1 = vertex[2] - vertex[0];
    let v0 = vertex[0] - p;
    let a = edge0.dot(edge0);
    let b = edge0.dot(edge1);
    let c = edge1.dot(edge1);
    let d = edge0.dot(v0);
    let e = edge1.dot(v0);
    let det = a * c - b * b;
    let mut s = b * e - c * d;
    let mut t = b * d - a * e;
    if s + t < det {
        if s < 0.0 {
            if t < 0.0 {
                if d < 0.0 {
                    s = (-d / a).clamp(0.0, 1.0);
                    t = 0.0;
                } else {
                    s = 0.0;
                    t = (-e / c).clamp(0.0, 1.0);
                }
            } else {
                s = 0.0;
                t = (-e / c).clamp(0.0, 1.0);
            }
        } else if t < 0.0 {
            s = (-d / a).clamp(0.0, 1.0);
            t = 0.0;
        } else {
            let inv_det = 1.0 / det;
            s *= inv_det;
            t *= inv_det;
        }
    } else if s < 0.0 {
        let tmp0 = b + d;
        let tmp1 = c + e;
        if tmp1 > tmp0 {
            let numer = tmp1 - tmp0;
            let denom = a - 2.0 * b + c;
            s = (numer / denom).clamp(0.0, 1.0);
            t = 1.0 - s;
        } else {
            t = (-e / c).clamp(0.0, 1.0);
            s = 0.0;
        }
    } else if t < 0.0 {
        if a + d > b + e {
            let numer = c + e - b - d;
            let denom = a - 2.0 * b + c;
            s = (numer / denom).clamp(0.0, 1.0);
            t = 1.0 - s;
        } else {
            s = (-d / a).clamp(0.0, 1.0);
            t = 0.0;
        }
    } else {
        let numer = c + e - b - d;
        let denom = a - 2.0 * b + c;
        s = (numer / denom).clamp(0.0, 1.0);
        t = 1.0 - s;
    }
    vertex[0] + s * edge0 + t * edge1
}

/// Godot's `Plane(p1, p2, p3)` (clockwise): its normal and `d`.
fn plane_of(v: &[Vec3; 3]) -> (Vec3, f32) {
    let n = normalized((v[0] - v[2]).cross(v[0] - v[1]));
    (n, n.dot(v[0]))
}

/// `Plane::intersects_segment`.
fn intersects_segment(n: Vec3, d: f32, begin: Vec3, end: Vec3) -> Option<Vec3> {
    let segment = begin - end;
    let den = n.dot(segment);
    if den.abs() < CMP_EPSILON {
        return None;
    }
    let dist = (n.dot(begin) - d) / den;
    if dist < -CMP_EPSILON || dist > 1.0 + CMP_EPSILON {
        return None;
    }
    Some(begin + segment * -dist)
}

/// One column of a leaf's 4 x 4 scan, for a demonstration: its box, whether
/// the triangle crosses it, and the point it sampled.
#[derive(Clone, Copy, Debug)]
pub struct Column {
    pub from: Vec3,
    pub to: Vec3,
    pub hit: Option<Vec3>,
}

/// What one leaf's `_plot_face` added: the 16 columns, and the sums.
#[derive(Clone, Debug)]
pub struct LeafSample {
    pub closest_axis: usize,
    pub columns: Vec<Column>,
    pub albedo: Vec3,
    pub emission: Vec3,
    pub normal: Vec3,
    pub alpha: f32,
}

/// One step of a triangle's descent, for a demonstration: the child box a
/// level tested, and whether the triangle overlapped it.
#[derive(Clone, Copy, Debug)]
pub struct Descent {
    pub level: i32,
    pub aabb: Aabb,
    pub overlaps: bool,
}

impl Voxelizer {
    /// `begin_bake`: the power-of-two cube, each axis's cell count, and `to_cell_space`.
    pub fn begin_bake(subdiv: i32, bounds: Aabb) -> Voxelizer {
        let mut po2 = bounds;
        let longest = longest_axis(bounds.size);
        let mut axis_cell_size = [0i32; 3];
        axis_cell_size[longest] = 1 << subdiv;
        for i in 0..3 {
            if i == longest {
                continue;
            }
            axis_cell_size[i] = axis_cell_size[longest];
            let mut axis_size = po2.size[longest];
            while axis_size / 2.0 >= po2.size[i] {
                axis_size /= 2.0;
                axis_cell_size[i] >>= 1;
            }
            po2.size[i] = po2.size[longest];
        }
        let scale = axis_cell_size[longest] as f32 / po2.size[longest];
        Voxelizer {
            cell_subdiv: subdiv,
            original_bounds: bounds,
            po2_bounds: po2,
            axis_cell_size,
            to_cell_scale: scale,
            to_cell_origin: po2.position,
            cell_size: po2.size[longest] / axis_cell_size[longest] as f32,
            cells: vec![Cell::default()],
            leaf_voxel_count: 0,
            sorted: false,
        }
    }

    /// `plot_mesh` for one mesh: each triangle that touches the original
    /// bounds goes down from the root.
    pub fn plot_mesh(&mut self, mesh: &BakeMesh) {
        let ob = self.original_bounds;
        for tri in &mesh.triangles {
            if !triangle_box_overlap(ob.center(), ob.size * 0.5, &tri.vtx) {
                continue;
            }
            let po2 = self.po2_bounds;
            self.plot_face(0, 0, [0, 0, 0], tri, mesh.albedo, mesh.emission, po2, &mut None);
        }
    }

    /// `_plot_face`, with an optional record of the descent.
    #[allow(clippy::too_many_arguments)]
    pub fn plot_face(&mut self, idx: u32, level: i32, p: [i32; 3], tri: &Triangle, albedo: Vec3, emission: Vec3, aabb: Aabb, trace: &mut Option<&mut Vec<Descent>>) {
        if level == self.cell_subdiv {
            let s = leaf_sample(tri, albedo, emission, aabb);
            let c = &mut self.cells[idx as usize];
            for k in 0..3 {
                c.albedo[k] += s.albedo[k];
                c.emission[k] += s.emission[k];
                c.normal[k] += s.normal[k];
            }
            c.alpha += s.alpha;
            return;
        }
        let half = (1 << self.cell_subdiv) >> (level + 1);
        for i in 0..8 {
            let mut child = aabb;
            child.size *= 0.5;
            let mut n = p;
            if i & 1 != 0 {
                child.position.x += child.size.x;
                n[0] += half;
            }
            if i & 2 != 0 {
                child.position.y += child.size.y;
                n[1] += half;
            }
            if i & 4 != 0 {
                child.position.z += child.size.z;
                n[2] += half;
            }
            if (0..3).any(|k| n[k] < 0 || n[k] >= self.axis_cell_size[k]) {
                continue;
            }
            let qsize = child.size * 0.5;
            let overlaps = triangle_box_overlap(child.position + qsize, qsize, &tri.vtx);
            if !overlaps {
                if let Some(t) = trace.as_deref_mut() {
                    t.push(Descent { level: level + 1, aabb: child, overlaps: false });
                }
                continue;
            }
            if self.cells[idx as usize].children[i] == CHILD_EMPTY {
                let child_idx = self.cells.len() as u32;
                self.cells[idx as usize].children[i] = child_idx;
                self.cells.push(Cell { level: (level + 1) as u16, x: (n[0] / half) as u16, y: (n[1] / half) as u16, z: (n[2] / half) as u16, ..Cell::default() });
            }
            let next = self.cells[idx as usize].children[i];
            if let Some(t) = trace.as_deref_mut() {
                t.push(Descent { level: level + 1, aabb: child, overlaps: true });
            }
            self.plot_face(next, level + 1, n, tri, albedo, emission, child, trace);
        }
    }

    /// `_sort`: by level, then x, y, z (the `CellSort` key, level in its top
    /// bits); children remapped.
    pub fn sort(&mut self) {
        let mut order: Vec<usize> = (0..self.cells.len()).collect();
        let key = |c: &Cell| ((c.level as u64) << 48) | ((c.x as u64) << 32) | ((c.y as u64) << 16) | c.z as u64;
        order.sort_by_key(|&i| key(&self.cells[i]));
        assert_eq!(self.cells[order[0]].level, 0, "the sorted cells start with the root");
        let mut reverse = vec![0u32; self.cells.len()];
        for (new, &old) in order.iter().enumerate() {
            reverse[old] = new as u32;
        }
        let mut sorted: Vec<Cell> = order.iter().map(|&i| self.cells[i]).collect();
        for c in &mut sorted {
            for ch in &mut c.children {
                if *ch != CHILD_EMPTY {
                    *ch = reverse[*ch as usize];
                }
            }
        }
        self.cells = sorted;
        self.sorted = true;
    }

    /// `_fixup_plot` from the root.
    pub fn fixup(&mut self, idx: usize, level: i32) {
        if level == self.cell_subdiv {
            self.leaf_voxel_count += 1;
            let c = &mut self.cells[idx];
            let alpha = c.alpha;
            for k in 0..3 {
                c.albedo[k] /= alpha;
                c.emission[k] /= alpha;
                c.normal[k] /= alpha;
            }
            let n = Vec3::from(c.normal);
            c.normal = if n.length() < 0.01 { [0.0; 3] } else { n.normalize().to_array() };
            c.alpha = 1.0;
            return;
        }
        {
            let c = &mut self.cells[idx];
            c.emission = [0.0; 3];
            c.normal = [0.0; 3];
            c.albedo = [0.0; 3];
        }
        let mut alpha_average = 0.0;
        for i in 0..8 {
            let child = self.cells[idx].children[i];
            if child == CHILD_EMPTY {
                continue;
            }
            self.fixup(child as usize, level + 1);
            alpha_average += self.cells[child as usize].alpha;
        }
        self.cells[idx].alpha = alpha_average / 8.0;
    }

    /// `end_bake`.
    pub fn end_bake(&mut self) {
        if !self.sorted {
            self.sort();
        }
        self.fixup(0, 0);
    }

    /// `get_voxel_gi_octree_size`.
    pub fn octree_size(&self) -> [i32; 3] {
        self.axis_cell_size
    }

    /// `get_voxel_gi_level_cell_count`: cells per level, `cell_subdiv + 1` levels.
    pub fn level_counts(&self) -> Vec<u32> {
        let mut w = vec![0u32; self.cell_subdiv as usize + 1];
        for c in &self.cells {
            w[c.level as usize] += 1;
        }
        w
    }
}

/// `AABB::get_longest_axis_index`: x on a tie, then y.
pub fn longest_axis(size: Vec3) -> usize {
    let mut axis = 0;
    let mut max = size.x;
    if size.y > max {
        axis = 1;
        max = size.y;
    }
    if size.z > max {
        axis = 2;
    }
    axis
}

/// The leaf branch of `_plot_face`: the dominant axis of the triangle's
/// plane, 4 x 4 columns of the cell along it, a sample where the triangle
/// crosses each, and the sums scaled by 1/16 (or the closest point to the
/// cell's centre at 1/16 when no column is crossed).
pub fn leaf_sample(tri: &Triangle, albedo: Vec3, emission: Vec3, aabb: Aabb) -> LeafSample {
    let vtx = &tri.vtx;
    let (normal, d) = plane_of(vtx);
    let mut closest_axis = 0;
    let mut closest_dot = 0.0;
    for i in 0..3 {
        let dot = normal[i].abs();
        if i == 0 || dot > closest_dot {
            closest_axis = i;
            closest_dot = dot;
        }
    }
    let mut axis = Vec3::ZERO;
    axis[closest_axis] = 1.0;
    let mut t1 = Vec3::ZERO;
    t1[(closest_axis + 1) % 3] = 1.0;
    let mut t2 = Vec3::ZERO;
    t2[(closest_axis + 2) % 3] = 1.0;
    let w = COLOR_SCAN_CELL_WIDTH as f32;
    t1 *= aabb.size[(closest_axis + 1) % 3] / w;
    t2 *= aabb.size[(closest_axis + 2) % 3] / w;
    let mut albedo_accum = Vec3::ZERO;
    let mut emission_accum = Vec3::ZERO;
    let mut normal_accum = Vec3::ZERO;
    let mut alpha = 0.0f32;
    let mut columns = Vec::with_capacity(16);
    for i in 0..COLOR_SCAN_CELL_WIDTH {
        let ofs_i = t1 * i as f32;
        for j in 0..COLOR_SCAN_CELL_WIDTH {
            let ofs_j = t2 * j as f32;
            let from = aabb.position + ofs_i + ofs_j;
            let to = from + t1 + t2 + axis * aabb.size[closest_axis];
            let half = (to - from) * 0.5;
            if !triangle_box_overlap(from + half, half, vtx) {
                columns.push(Column { from, to, hit: None });
                continue;
            }
            let mut ray_from = from + (t1 + t2) * 0.5 - axis * aabb.size[closest_axis];
            let mut ray_to = ray_from + axis * aabb.size[closest_axis] * 2.0;
            if normal.dot(ray_from - ray_to) < 0.0 {
                std::mem::swap(&mut ray_from, &mut ray_to);
            }
            let intersection = intersects_segment(normal, d, ray_from, ray_to).unwrap_or_else(|| {
                let dist = |p: Vec3| normal.dot(p) - d;
                if dist(ray_from).abs() < dist(ray_to).abs() { ray_from - normal * dist(ray_from) } else { ray_to - normal * dist(ray_to) }
            });
            let intersection = closest_point_on_triangle(vtx, intersection);
            let mut lnormal = normal_at(intersection, vtx, &tri.normal);
            if lnormal == Vec3::ZERO {
                lnormal = normal;
            }
            albedo_accum += albedo;
            emission_accum += emission;
            normal_accum += lnormal;
            alpha += 1.0;
            columns.push(Column { from, to, hit: Some(intersection) });
        }
    }
    let accdiv = 1.0 / (w * w);
    if alpha == 0.0 {
        let inters = closest_point_on_triangle(vtx, aabb.center());
        // The engine passes `normal` as the output here, so the interpolated
        // normal at the closest point is what `lnormal` falls back to.
        let lnormal = normal_at(inters, vtx, &tri.normal);
        let lnormal = if lnormal == Vec3::ZERO { normal } else { lnormal };
        alpha = accdiv;
        return LeafSample { closest_axis, columns, albedo: albedo * alpha, emission: emission * alpha, normal: lnormal * alpha, alpha };
    }
    alpha *= accdiv;
    LeafSample { closest_axis, columns, albedo: albedo_accum * accdiv, emission: emission_accum * accdiv, normal: normal_accum * accdiv, alpha }
}

/// The room's meshes as `VoxelGI::bake` finds them (`_find_meshes`): every
/// visible GeometryInstance3D with GI mode Static whose bounds touch the box,
/// with its transform into the VoxelGI's local space (`xf`) and its normals
/// through the basis's inverse transpose (`plot_mesh`). `skip` leaves objects
/// out (the ceiling panel, which is the area light's fixture, and a dynamic object).
pub fn room_meshes(scene: &lab::scene::Scene, meshes: &lab::scene::Meshes, node: Mat4, skip: &[usize]) -> Vec<BakeMesh> {
    let to_local = node.inverse();
    let mut out = Vec::new();
    for (k, o) in scene.objects.iter().enumerate() {
        if skip.contains(&k) {
            continue;
        }
        let xf = to_local * o.local_to_world();
        let normal_xform = Mat3::from_mat4(xf).inverse().transpose();
        let range = meshes.ranges[k].clone();
        let idx = &meshes.indices[range.start as usize..range.end as usize];
        let triangles = idx.chunks_exact(3).map(|t| {
            let v = |i: u32| meshes.vertices[i as usize];
            let vtx = [0, 1, 2].map(|j| xf.transform_point3(Vec3::from(v(t[j]).position)));
            let normal = [0, 1, 2].map(|j| normalized(normal_xform * Vec3::from(v(t[j]).normal)));
            Triangle { vtx, normal }
        }).collect();
        out.push(BakeMesh { object: k, triangles, albedo: o.albedo, emission: o.emissive * o.emissive_strength });
    }
    out
}
