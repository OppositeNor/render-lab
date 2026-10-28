//! Terrain: a heightfield on a regular grid in its object's local space,
//! its XZ rectangle centred on the origin and heights along +Y. The raster
//! pass draws the grid's triangles (two per cell, split along the cell's
//! diagonal from its low corner); intersectTerrain in common.wgsl and
//! `Terrain::intersect` below walk the same cells for shadow, reflection,
//! probe, and card rays.
//!
//! The lab has one terrain (`terrain()`), as it has one teapot mesh; the
//! objects of shape SHAPE_TERRAIN in a scene are instances of it. Its heights
//! come from a `HeightSource`: given samples, or (opt-in, internal) fractal
//! Perlin noise with a fixed seed, which makes the same terrain on every run.

use glam::{Vec2, Vec3};
use std::sync::OnceLock;

/// What a terrain's heights are made from.
pub enum HeightSource {
    /// Row-major heights, `resolution` × `resolution`, rows along +Z.
    #[allow(dead_code)] // the lab's terrain is generated; a heightmap is the other way in
    Samples(Vec<f32>),
    /// Fractal Perlin noise (`perlin::fbm`), scaled into `TerrainDesc::height`.
    Perlin { seed: u64, octaves: u32, frequency: f32 },
}

pub struct TerrainDesc {
    /// Vertices per side.
    pub resolution: u32,
    /// Half size of the XZ rectangle, metres.
    pub half_size: Vec2,
    /// Heights span 0 ..= this, metres.
    pub height: f32,
    pub source: HeightSource,
}

/// The lab's terrain: the terrain scene's `[terrain]` (packages/render_lab/scenes/terrain.toml).
pub fn terrain() -> &'static Terrain {
    static TERRAIN: OnceLock<Terrain> = OnceLock::new();
    TERRAIN.get_or_init(|| Terrain::build(crate::scene::terrain_table(crate::scene::SceneId::TERRAIN).unwrap_or_else(|e| panic!("{e:#}"))))
}

pub struct Terrain {
    pub resolution: u32,
    pub half_size: Vec2,
    /// Row-major, `resolution` × `resolution`: `heights[j * resolution + i]`
    /// is the vertex at column `i` (+X) and row `j` (+Z).
    pub heights: Vec<f32>,
    pub min_height: f32,
    pub max_height: f32,
}

impl Terrain {
    pub fn build(desc: TerrainDesc) -> Terrain {
        let n = desc.resolution as usize;
        assert!(n >= 2, "a terrain needs at least 2 × 2 vertices");
        let heights = match desc.source {
            HeightSource::Samples(samples) => {
                assert_eq!(samples.len(), n * n, "terrain samples");
                samples
            }
            HeightSource::Perlin { seed, octaves, frequency } => {
                let noise = perlin::Perlin::new(seed);
                let raw: Vec<f64> = (0..n * n)
                    .map(|k| {
                        let (i, j) = (k % n, k / n);
                        let x = -desc.half_size.x as f64 + 2.0 * desc.half_size.x as f64 * i as f64 / (n - 1) as f64;
                        let z = -desc.half_size.y as f64 + 2.0 * desc.half_size.y as f64 * j as f64 / (n - 1) as f64;
                        noise.fbm(x * frequency as f64, z * frequency as f64, octaves)
                    })
                    .collect();
                let (lo, hi) = raw.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &v| (lo.min(v), hi.max(v)));
                raw.iter().map(|&v| ((v - lo) / (hi - lo).max(1e-9) * desc.height as f64) as f32).collect()
            }
        };
        let min_height = heights.iter().copied().fold(f32::INFINITY, f32::min);
        let max_height = heights.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        Terrain { resolution: desc.resolution, half_size: desc.half_size, heights, min_height, max_height }
    }

    /// The centre and half extent of its local bounds.
    pub fn local_center(&self) -> Vec3 {
        Vec3::new(0.0, 0.5 * (self.min_height + self.max_height), 0.0)
    }

    pub fn local_extent(&self) -> Vec3 {
        Vec3::new(self.half_size.x, 0.5 * (self.max_height - self.min_height), self.half_size.y)
    }

    pub fn cells(&self) -> u32 {
        self.resolution - 1
    }

    /// Width and depth of one cell.
    pub fn cell_size(&self) -> Vec2 {
        2.0 * self.half_size / self.cells() as f32
    }

    fn h(&self, i: u32, j: u32) -> f32 {
        self.heights[(j * self.resolution + i) as usize]
    }

    pub fn vertex(&self, i: u32, j: u32) -> Vec3 {
        let c = self.cell_size();
        Vec3::new(-self.half_size.x + c.x * i as f32, self.h(i, j), -self.half_size.y + c.y * j as f32)
    }

    /// The vertex normal: central differences, one-sided at the edges
    /// (terrainVertexNormal in common.wgsl).
    pub fn vertex_normal(&self, i: u32, j: u32) -> Vec3 {
        let last = self.cells();
        let (i0, i1) = (i.saturating_sub(1), (i + 1).min(last));
        let (j0, j1) = (j.saturating_sub(1), (j + 1).min(last));
        let c = self.cell_size();
        let dx = (self.h(i1, j) - self.h(i0, j)) / (c.x * (i1 - i0) as f32);
        let dz = (self.h(i, j1) - self.h(i, j0)) / (c.y * (j1 - j0) as f32);
        Vec3::new(-dx, 1.0, -dz).normalize()
    }

    /// The two triangles of cell (`i`, `j`), as vertex coordinates.
    pub fn cell_triangles(i: u32, j: u32) -> [[(u32, u32); 3]; 2] {
        [[(i, j), (i, j + 1), (i + 1, j + 1)], [(i, j), (i + 1, j + 1), (i + 1, j)]]
    }

    /// The surface height under a local XZ point (clamped to the grid), on the raster triangles.
    pub fn height_at(&self, x: f32, z: f32) -> f32 {
        let c = self.cell_size();
        let fx = ((x + self.half_size.x) / c.x).clamp(0.0, self.cells() as f32);
        let fz = ((z + self.half_size.y) / c.y).clamp(0.0, self.cells() as f32);
        let (i, j) = ((fx as u32).min(self.cells() - 1), (fz as u32).min(self.cells() - 1));
        let (u, v) = (fx - i as f32, fz - j as f32);
        let (h00, h10, h01, h11) = (self.h(i, j), self.h(i + 1, j), self.h(i, j + 1), self.h(i + 1, j + 1));
        if v >= u { h00 + (h11 - h01) * u + (h01 - h00) * v } else { h00 + (h10 - h00) * u + (h11 - h10) * v }
    }

    /// The vertex normals under a local XZ point (clamped to the grid), blended bilinearly.
    pub fn normal_at(&self, x: f32, z: f32) -> Vec3 {
        let c = self.cell_size();
        let fx = ((x + self.half_size.x) / c.x).clamp(0.0, self.cells() as f32);
        let fz = ((z + self.half_size.y) / c.y).clamp(0.0, self.cells() as f32);
        let (i, j) = ((fx as u32).min(self.cells() - 1), (fz as u32).min(self.cells() - 1));
        let (u, v) = (fx - i as f32, fz - j as f32);
        let n = |a: u32, b: u32| self.vertex_normal(a, b);
        (n(i, j) * (1.0 - u) * (1.0 - v) + n(i + 1, j) * u * (1.0 - v) + n(i, j + 1) * (1.0 - u) * v + n(i + 1, j + 1) * u * v).normalize()
    }

    /// Closest hit of a local-space ray: the cells under the ray in order
    /// (Amanatides & Woo, "A Fast Voxel Traversal Algorithm", 1987), each
    /// skipped when the ray passes above or below its corners, then its two
    /// triangles (two-sided Möller–Trumbore). Returns t and the interpolated
    /// vertex normal. The same walk as intersectTerrain in common.wgsl.
    pub fn intersect(&self, lo: Vec3, ld: Vec3, t_max: f32) -> Option<(f32, Vec3)> {
        let bmin = Vec3::new(-self.half_size.x, self.min_height, -self.half_size.y);
        let bmax = Vec3::new(self.half_size.x, self.max_height, self.half_size.y);
        let safe = Vec3::select(ld.abs().cmplt(Vec3::splat(1e-9)), Vec3::splat(1e-9), ld);
        let inv = Vec3::ONE / safe;
        let (t0, t1) = ((bmin - lo) * inv, (bmax - lo) * inv);
        let t_enter = t0.min(t1).max_element().max(0.0);
        let t_exit = t0.max(t1).min_element().min(t_max);
        if t_exit < t_enter {
            return None;
        }
        let c = self.cell_size();
        let last = self.cells() as i32 - 1;
        let p = lo + ld * t_enter;
        let mut i = (((p.x + self.half_size.x) / c.x).floor() as i32).clamp(0, last);
        let mut j = (((p.z + self.half_size.y) / c.y).floor() as i32).clamp(0, last);
        let (si, sj) = (if ld.x >= 0.0 { 1 } else { -1 }, if ld.z >= 0.0 { 1 } else { -1 });
        let next_x = -self.half_size.x + c.x * (i + if si > 0 { 1 } else { 0 }) as f32;
        let next_z = -self.half_size.y + c.y * (j + if sj > 0 { 1 } else { 0 }) as f32;
        let mut tx = (next_x - lo.x) * inv.x;
        let mut tz = (next_z - lo.z) * inv.z;
        let (dtx, dtz) = ((c.x * inv.x).abs(), (c.y * inv.z).abs());
        let mut t_cell = t_enter;
        for _ in 0..2 * (last + 1) + 2 {
            let t_out = tx.min(tz).min(t_exit);
            let (ya, yb) = (lo.y + ld.y * t_cell, lo.y + ld.y * t_out);
            let (ci, cj) = (i as u32, j as u32);
            let corners = [self.h(ci, cj), self.h(ci + 1, cj), self.h(ci, cj + 1), self.h(ci + 1, cj + 1)];
            let lo_h = corners.iter().copied().fold(f32::INFINITY, f32::min);
            let hi_h = corners.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            if ya.min(yb) <= hi_h + 1e-4 && ya.max(yb) >= lo_h - 1e-4 {
                let mut best: Option<(f32, Vec3)> = None;
                for tri in Self::cell_triangles(ci, cj) {
                    let [a, b, d] = tri.map(|(x, z)| self.vertex(x, z));
                    if let Some((t, u, v)) = moller_trumbore(lo, ld, a, b, d) {
                        if t > 1e-4 && t < best.map_or(t_max, |h| h.0) {
                            let [na, nb, nd] = tri.map(|(x, z)| self.vertex_normal(x, z));
                            best = Some((t, (na * (1.0 - u - v) + nb * u + nd * v).normalize()));
                        }
                    }
                }
                if best.is_some() {
                    return best;
                }
            }
            if t_out >= t_exit {
                return None;
            }
            if tx < tz {
                i += si;
                t_cell = tx;
                tx += dtx;
            } else {
                j += sj;
                t_cell = tz;
                tz += dtz;
            }
            if i < 0 || j < 0 || i > last || j > last {
                return None;
            }
        }
        None
    }

    /// The data intersectTerrain reads (gTerrain): the resolution's bits,
    /// the half size, the lowest and highest heights, three unused words,
    /// then the heights.
    pub fn gpu_words(&self) -> Vec<f32> {
        let mut words = vec![f32::from_bits(self.resolution), self.half_size.x, self.half_size.y, self.min_height, self.max_height, 0.0, 0.0, 0.0];
        words.extend_from_slice(&self.heights);
        words
    }
}

fn moller_trumbore(o: Vec3, d: Vec3, p0: Vec3, p1: Vec3, p2: Vec3) -> Option<(f32, f32, f32)> {
    let (e1, e2) = (p1 - p0, p2 - p0);
    let pv = d.cross(e2);
    let det = e1.dot(pv);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let tv = o - p0;
    let u = tv.dot(pv) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let qv = tv.cross(e1);
    let v = d.dot(qv) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    Some((e2.dot(qv) * inv, u, v))
}

/// Ken Perlin's improved noise ("Improving Noise", SIGGRAPH 2002) in two
/// dimensions, with a permutation shuffled from a seed instead of his fixed
/// table, summed over octaves. Computed in f64 so the terrain is the same
/// everywhere.
mod perlin {
    pub struct Perlin {
        perm: [u8; 512],
    }

    impl Perlin {
        pub fn new(seed: u64) -> Perlin {
            let mut table: [u8; 256] = std::array::from_fn(|i| i as u8);
            // SplitMix64 drives a Fisher–Yates shuffle.
            let mut state = seed;
            let mut next = || {
                state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
                let mut z = state;
                z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
                z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
                z ^ (z >> 31)
            };
            for i in (1..256).rev() {
                table.swap(i, (next() % (i as u64 + 1)) as usize);
            }
            Perlin { perm: std::array::from_fn(|i| table[i % 256]) }
        }

        fn grad(hash: u8, x: f64, y: f64) -> f64 {
            match hash & 7 {
                0 => x + y,
                1 => -x + y,
                2 => x - y,
                3 => -x - y,
                4 => x,
                5 => -x,
                6 => y,
                _ => -y,
            }
        }

        pub fn noise(&self, x: f64, y: f64) -> f64 {
            let fade = |t: f64| t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
            let lerp = |t: f64, a: f64, b: f64| a + t * (b - a);
            let (xf, yf) = (x.floor(), y.floor());
            let (xi, yi) = ((xf as i64 & 255) as usize, (yf as i64 & 255) as usize);
            let (x, y) = (x - xf, y - yf);
            let (u, v) = (fade(x), fade(y));
            let p = &self.perm;
            let (a, b) = (p[xi] as usize + yi, p[xi + 1] as usize + yi);
            lerp(
                v,
                lerp(u, Self::grad(p[a], x, y), Self::grad(p[b], x - 1.0, y)),
                lerp(u, Self::grad(p[a + 1], x, y - 1.0), Self::grad(p[b + 1], x - 1.0, y - 1.0)),
            )
        }

        /// Octaves at doubling frequency and halving amplitude.
        pub fn fbm(&self, x: f64, y: f64, octaves: u32) -> f64 {
            (0..octaves).map(|o| self.noise(x * f64::from(1u32 << o), y * f64::from(1u32 << o)) / f64::from(1u32 << o)).sum()
        }
    }
}
