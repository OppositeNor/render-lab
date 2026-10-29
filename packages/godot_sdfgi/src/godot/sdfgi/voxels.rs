//! One cascade's voxels and distance field on the CPU, by the engine's rules:
//! the three orthographic passes of `RenderForwardClustered::_render_sdfgi`
//! (render_forward_clustered.cpp) and the `MODE_RENDER_SDF` fragment code
//! (scene_forward_clustered.glsl) that write the voxel images, then the
//! half-size jump flood and `MODE_STORE` of sdfgi_preprocess.glsl, scheduled
//! as `GI::SDFGI::render_region` dispatches them (gi.cpp).
//!
//! Where the lab differs: the engine rasterizes triangles; the lab finds the
//! same fragments by tracing each pixel's ray through the room's shapes
//! (every surface the ray crosses, both faces, as with culling disabled), with
//! the shapes' exact normals in place of interpolated vertex normals. Writes
//! that the GPU orders by scheduling (last write wins, the solid-cell list's
//! order) follow the lab's loop order.
//!
//! Ported from Godot Engine (MIT; Copyright (c) 2014-present Godot Engine
//! contributors, Copyright (c) 2007-2014 Juan Linietsky, Ariel Manzur).

use super::model::{Cascades, Region, CASCADE_SIZE};
use glam::{IVec3, Vec3};
use lab::layout::ObjectData;

pub const N: usize = CASCADE_SIZE as usize;
pub const CELLS: usize = N * N * N;

/// The index of cell `p` in a 128^3 image.
pub fn index(p: IVec3) -> usize {
    p.x as usize + p.y as usize * N + p.z as usize * N * N
}

pub fn inside(p: IVec3) -> bool {
    p.cmpge(IVec3::ZERO).all() && p.cmplt(IVec3::splat(CASCADE_SIZE)).all()
}

/// `aniso_dir[6]`: +X, +Y, +Z, -X, -Y, -Z.
pub const ANISO_DIR: [Vec3; 6] = [Vec3::X, Vec3::Y, Vec3::Z, Vec3::NEG_X, Vec3::NEG_Y, Vec3::NEG_Z];

/// The render images one cascade's regions are voxelized into
/// (`render_albedo` R16_UINT, `render_geom_facing` R32_UINT,
/// `render_emission` and `render_emission_aniso` R32_UINT).
#[derive(Clone)]
pub struct RenderVoxels {
    pub albedo: Vec<u16>,
    pub facing: Vec<u32>,
    pub emission: Vec<u32>,
    pub emission_aniso: Vec<u32>,
}

impl RenderVoxels {
    /// `render_region` clears the four images before a cascade's first region.
    pub fn cleared() -> RenderVoxels {
        RenderVoxels { albedo: vec![0; CELLS], facing: vec![0; CELLS], emission: vec![0; CELLS], emission_aniso: vec![0; CELLS] }
    }

    pub fn solid(&self, p: IVec3) -> bool {
        self.albedo[index(p)] & 1 != 0
    }
}

/// The fragment code's RGBE "8985" encode of a light (scene_forward_clustered.glsl,
/// `MODE_RENDER_SDF`; the same code packs the static light in
/// sdfgi_direct_light.glsl): a shared-exponent RGB9E5 encode (N = 9, B = 15)
/// stored as R's top 8 bits, G's 9, B's top 8, and E's 5.
pub fn encode_rgbe8985(light: Vec3) -> u32 {
    let (s, exps) = shared_exponent(light);
    ((s[0] & 0x1FF) >> 1) | ((s[1] & 0x1FF) << 8) | (((s[2] & 0x1FF) >> 1) << 17) | ((exps & 0x1F) << 25)
}

/// The mantissas and exponent the shaders' RGBE encode computes (float math
/// as in the shader: `floor(log(cMax) / LN2)`, `pow(2, e - B - N)`).
pub fn shared_exponent(light: Vec3) -> ([u32; 3], u32) {
    const POW2TO9: f32 = 512.0;
    const B: f32 = 15.0;
    const NB: f32 = 9.0;
    const LN2: f32 = 0.693_147_2;
    let c = light.clamp(Vec3::ZERO, Vec3::splat(65408.0));
    let c_max = c.max_element();
    let expp = (-B - 1.0).max((c_max.ln() / LN2).floor()) + 1.0 + B;
    let s_max = (c_max / 2f32.powf(expp - B - NB) + 0.5).floor();
    let exps = if (0.0..POW2TO9).contains(&s_max) { expp } else { expp + 1.0 };
    let scale = 2f32.powf(exps - B - NB);
    let s = [(c.x / scale + 0.5).floor() as u32, (c.y / scale + 0.5).floor() as u32, (c.z / scale + 0.5).floor() as u32];
    (s, exps as u32)
}

/// Decodes an RGBE8985 word (sdfgi_direct_light.glsl: `m = pow(2, e - 15 - 9)`).
pub fn decode_rgbe8985(rgbe: u32) -> Vec3 {
    let r = ((rgbe & 0xff) << 1) as f32;
    let g = ((rgbe >> 8) & 0x1ff) as f32;
    let b = (((rgbe >> 17) & 0xff) << 1) as f32;
    let e = ((rgbe >> 25) & 0x1F) as f32;
    Vec3::new(r, g, b) * 2f32.powf(e - 15.0 - 9.0)
}

/// One fragment of the SDF pass: what the fragment shader writes at the voxel
/// its interpolated position falls in.
fn write_fragment(voxels: &mut RenderVoxels, grid_pos: IVec3, albedo: Vec3, normal: Vec3, emission: Vec3) {
    // The shader does not clamp grid_pos; an image store outside the image is discarded.
    if !inside(grid_pos) {
        return;
    }
    let i = index(grid_pos);
    let q = |c: f32| ((c * 31.0) as u32).min(31);
    let albedo16 = 0x1 | (q(albedo.x) << 11) | (q(albedo.y) << 6) | (q(albedo.z) << 1);
    voxels.albedo[i] = albedo16 as u16;
    let mut closest = -1e20;
    let mut facing_bits = 0;
    for (k, dir) in ANISO_DIR.iter().enumerate() {
        let d = normal.dot(*dir);
        if d > closest {
            closest = d;
            facing_bits = 1 << k;
        }
    }
    voxels.facing[i] |= facing_bits;
    if emission.length() > 0.001 {
        let mut lumas = [0.0f32; 6];
        let mut total = Vec3::ZERO;
        for (k, dir) in ANISO_DIR.iter().enumerate() {
            let light = emission * normal.dot(*dir).max(0.0);
            total += light;
            lumas[k] = light.max_element();
        }
        let luma_total = total.max_element();
        let mut aniso = 0;
        for (k, l) in lumas.iter().enumerate() {
            aniso |= (((l / luma_total) * 31.0) as u32).min(31) << (k * 5);
        }
        voxels.emission[i] = encode_rgbe8985(total);
        voxels.emission_aniso[i] = aniso;
    }
}

/// Every surface a ray crosses, nearest first, within `t_max`: the fragments
/// one pixel of an orthographic pass produces with culling disabled.
fn crossings(objects: &[ObjectData], ro: Vec3, rd: Vec3, t_max: f32) -> Vec<(f32, usize, Vec3)> {
    let mut hits = Vec::new();
    for (k, o) in objects.iter().enumerate() {
        let mut t0 = 0.0;
        while let Some((t, n)) = lab::scene::intersect_object(o, ro + rd * t0, rd, t_max - t0) {
            hits.push((t0 + t, k, n));
            t0 += t + 1e-4;
            if t0 >= t_max {
                break;
            }
        }
    }
    hits.sort_by(|a, b| a.0.total_cmp(&b.0));
    hits
}

/// `_render_sdfgi` for one region: three passes, camera at the region box's
/// +axis face looking along -axis, one pixel per cell of the face; each
/// fragment's voxel is `sdf_offset + ivec3(local_pos * sdf_size)`, where
/// `local_pos` is its position in the box scaled to 0..1.
pub fn voxelize(objects: &[ObjectData], region: &Region, voxels: &mut RenderVoxels) {
    voxelize_axes(objects, region, voxels, 3);
}

/// The first `passes` of `voxelize`'s three views (x, then y, then z).
pub fn voxelize_axes(objects: &[ObjectData], region: &Region, voxels: &mut RenderVoxels, passes: usize) {
    let size = region.local_size;
    let min = region.bounds_min;
    let extent = region.bounds_size;
    for axis in 0..passes.min(3) {
        let right = (axis + 1) % 3;
        let up = (axis + 2) % 3;
        let (w, h) = (size[right], size[up]);
        for py in 0..h {
            for px in 0..w {
                // The pixel's centre on the +axis face, the ray along -axis.
                let mut ro = min;
                ro[right] += (px as f32 + 0.5) / w as f32 * extent[right];
                ro[up] += (py as f32 + 0.5) / h as f32 * extent[up];
                ro[axis] += extent[axis];
                let mut rd = Vec3::ZERO;
                rd[axis] = -1.0;
                for (t, k, n) in crossings(objects, ro, rd, extent[axis]) {
                    let o = &objects[k];
                    let p = ro + rd * t;
                    let local = (p - min) / extent;
                    let grid_pos = region.local_offset + (local * size.as_vec3()).as_ivec3();
                    let albedo = Vec3::new(o.albedo_roughness[0], o.albedo_roughness[1], o.albedo_roughness[2]);
                    let emission = Vec3::new(o.emissive[0], o.emissive[1], o.emissive[2]) * o.emissive[3];
                    write_fragment(voxels, grid_pos, albedo, n.normalize(), emission);
                }
            }
        }
    }
}

/// A jump-flood seed (`render_sdf` RGBA8_UINT): the full-size cell of the
/// nearest solid voxel found, `w = 255` when set, 0 when unused.
pub type Seed = [u8; 4];

const UNUSED: Seed = [0; 4];

fn seed_at(p: IVec3) -> Seed {
    [p.x as u8, p.y as u8, p.z as u8, 255]
}

fn seed_cell(s: Seed) -> IVec3 {
    IVec3::new(s[0] as i32, s[1] as i32, s[2] as i32)
}

/// The 26 offsets of `MODE_JUMPFLOOD`, in its order.
fn offsets26() -> Vec<IVec3> {
    let mut v = Vec::with_capacity(26);
    for x in -1..=1 {
        for y in -1..=1 {
            for z in -1..=1 {
                if (x, y, z) != (0, 0, 0) {
                    v.push(IVec3::new(x, y, z));
                }
            }
        }
    }
    v
}

/// The 27 offsets of `MODE_JUMPFLOOD_OPTIMIZED`, in its order (with the cell itself).
fn offsets27() -> Vec<IVec3> {
    let mut v = Vec::with_capacity(27);
    for x in -1..=1 {
        for y in -1..=1 {
            for z in -1..=1 {
                v.push(IVec3::new(x, y, z));
            }
        }
    }
    v
}

fn idx(p: IVec3, n: i32) -> usize {
    (p.x + p.y * n + p.z * n * n) as usize
}

fn dist(a: Vec3, seed: Seed) -> f32 {
    a.distance(seed_cell(seed).as_vec3())
}

/// One jump-flood pass over an `n`^3 grid of seeds, `MODE_JUMPFLOOD`
/// (regular) or `MODE_JUMPFLOOD_OPTIMIZED`: each cell keeps the nearest of
/// the seeds at `step` cells around it. The two differ only in where the
/// search starts (the regular pass from the cell's own seed, which a
/// full-size solid cell keeps outright; the optimized one from the first
/// valid of 27, the cell included), which decides ties.
pub fn jump_flood_pass(src: &[Seed], n: i32, step: i32, half_size: bool, optimized: bool) -> Vec<Seed> {
    let off26 = offsets26();
    let off27 = offsets27();
    let mut dst = vec![UNUSED; src.len()];
    for z in 0..n {
        for y in 0..n {
            for x in 0..n {
                let pos = IVec3::new(x, y, z);
                let mut posf = pos.as_vec3();
                if half_size {
                    posf = posf * 2.0 + 0.5;
                }
                let load = |q: IVec3| if q.cmpge(IVec3::ZERO).all() && q.cmplt(IVec3::splat(n)).all() { Some(src[idx(q, n)]) } else { None };
                let out = if optimized {
                    let mut closest = UNUSED;
                    let mut closest_dist = 0.0;
                    for o in &off27 {
                        let point = load(pos + *o * step).unwrap_or(UNUSED);
                        if point[3] == 0 {
                            continue;
                        }
                        let d = dist(posf, point);
                        if closest[3] == 0 || d < closest_dist {
                            closest = point;
                            closest_dist = d;
                        }
                    }
                    closest
                } else {
                    let mut p = src[idx(pos, n)];
                    if !half_size && p == seed_at(pos) {
                        p
                    } else {
                        let mut p_dist = if p[3] != 0 { dist(posf, p) } else { 0.0 };
                        for o in &off26 {
                            let Some(q) = load(pos + *o * step) else { continue };
                            if q[3] == 0 {
                                continue;
                            }
                            let q_dist = dist(posf, q);
                            if p[3] == 0 || q_dist < p_dist {
                                p = q;
                                p_dist = q_dist;
                            }
                        }
                        p
                    }
                };
                dst[idx(pos, n)] = out;
            }
        }
    }
    dst
}

/// `MODE_INITIALIZE_JUMP_FLOOD_HALF`: each half-size cell takes one of the
/// solid voxels of its 2 x 2 x 2 block, chosen by the cell's parity so that
/// neighbours pick different ones.
pub fn initialize_half(voxels: &RenderVoxels) -> Vec<Seed> {
    let h = (N / 2) as i32;
    let mut dst = vec![UNUSED; (h * h * h) as usize];
    for z in 0..h {
        for y in 0..h {
            for x in 0..h {
                let pos = IVec3::new(x, y, z);
                let base = pos * 2;
                let mut closest = Vec::with_capacity(8);
                for i in 0..8 {
                    let src = base + IVec3::new(i & 1, (i >> 1) & 1, (i >> 2) & 1);
                    if voxels.solid(src) {
                        closest.push(seed_at(src));
                    }
                }
                if !closest.is_empty() {
                    let indexv = (pos & IVec3::ONE) * IVec3::new(1, 2, 4);
                    let k = (indexv.x | indexv.y | indexv.z) as usize % closest.len();
                    dst[idx(pos, h)] = closest[k];
                }
            }
        }
    }
    dst
}

/// `MODE_UPSCALE_JUMP_FLOOD`: a solid voxel seeds itself; any other takes the
/// half-size cell's seed and looks for a nearer solid voxel in that seed's
/// 2 x 2 x 2 block.
pub fn upscale(voxels: &RenderVoxels, half: &[Seed]) -> Vec<Seed> {
    let h = (N / 2) as i32;
    let n = N as i32;
    let mut dst = vec![UNUSED; CELLS];
    for z in 0..n {
        for y in 0..n {
            for x in 0..n {
                let pos = IVec3::new(x, y, z);
                let v = if voxels.solid(pos) {
                    seed_at(pos)
                } else {
                    let mut v = half[idx(pos >> 1, h)];
                    let seed = seed_cell(v);
                    let mut d = (seed - pos).as_vec3().length();
                    let vbase = seed - (seed & IVec3::ONE);
                    for i in 0..8 {
                        let p = vbase + IVec3::new(i & 1, (i >> 1) & 1, (i >> 2) & 1);
                        let d2 = (p - pos).as_vec3().length();
                        if d2 < d && voxels.solid(p) {
                            v = [p.x as u8, p.y as u8, p.z as u8, v[3]];
                            d = d2;
                        }
                    }
                    v
                };
                dst[idx(pos, n)] = v;
            }
        }
    }
    dst
}

/// The steps of the half-size jump flood as `render_region` runs them:
/// `(step, optimized)`, regular passes while `half / (s / 2) < 8`.
pub fn half_schedule() -> Vec<(i32, bool)> {
    let half = (N / 2) as i32;
    let mut schedule = Vec::new();
    let mut s = half;
    while s > 1 {
        s /= 2;
        schedule.push((s, false));
        if half / (s / 2) >= 8 {
            break;
        }
    }
    while s > 1 {
        s /= 2;
        schedule.push((s, true));
    }
    schedule
}

/// The jump flood's intermediate states, for showing it step by step:
/// the half-size initialization, after each half-size pass, the upscale, and
/// the final full-size pass.
pub struct JumpFlood {
    pub half_init: Vec<Seed>,
    pub half_passes: Vec<(i32, bool, Vec<Seed>)>,
    pub upscaled: Vec<Seed>,
    pub seeds: Vec<Seed>,
}

pub fn jump_flood(voxels: &RenderVoxels) -> JumpFlood {
    let h = (N / 2) as i32;
    let half_init = initialize_half(voxels);
    let mut current = half_init.clone();
    let mut half_passes = Vec::new();
    for (step, optimized) in half_schedule() {
        current = jump_flood_pass(&current, h, step, true, optimized);
        half_passes.push((step, optimized, current.clone()));
    }
    let upscaled = upscale(voxels, &current);
    let seeds = jump_flood_pass(&upscaled, N as i32, 1, false, true);
    JumpFlood { half_init, half_passes, upscaled, seeds }
}

/// One entry of the solid-cell list (`ProcessVoxel`, 16 bytes): position
/// 7-7-7 with neighbour bits 11-21 above it; albedo 5-5-5, six facing bits,
/// neighbour bits 0-10; light (RGBE8985) and light anisotropy (6 x 5 bits),
/// each with two more neighbour bits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SolidCell {
    pub position: u32,
    pub albedo: u32,
    pub light: u32,
    pub light_aniso: u32,
}

impl SolidCell {
    pub fn cell(&self) -> IVec3 {
        IVec3::new((self.position & 0x7F) as i32, ((self.position >> 7) & 0x7F) as i32, ((self.position >> 14) & 0x7F) as i32)
    }

    /// The 26 neighbour bits, reassembled.
    pub fn neighbours(&self) -> u32 {
        (self.albedo >> 21) | ((self.position >> 21) << 11) | ((self.light >> 30) << 22) | ((self.light_aniso >> 30) << 24)
    }

    pub fn facing(&self) -> u32 {
        (self.albedo >> 15) & 0x3F
    }

    /// Albedo as the light pass decodes it (5-5-5 over 31).
    pub fn albedo_rgb(&self) -> Vec3 {
        Vec3::new(((self.albedo >> 10) & 0x1F) as f32, ((self.albedo >> 5) & 0x1F) as f32, (self.albedo & 0x1F) as f32) / 31.0
    }
}

/// What `MODE_STORE` writes for a cascade: the distance field
/// (`sdf_tex`, R8_UNORM: 0 at a solid voxel, else 1 + the distance to the
/// seed, over 255) and the solid-cell list.
pub struct Stored {
    /// The R8_UNORM bytes.
    pub sdf: Vec<u8>,
    pub cells: Vec<SolidCell>,
}


/// `MODE_STORE` over the whole cascade, in the dispatch's 4 x 4 x 4 groups
/// (the list's order follows the groups; the GPU's depends on scheduling).
/// `light` and `light_aniso` are the render images it copies into each cell:
/// the voxelized emission, or, after a scroll, what the scroll wrote there.
pub fn store(voxels: &RenderVoxels, seeds: &[Seed]) -> Stored {
    let mut sdf = vec![0u8; CELLS];
    let mut cells = Vec::new();
    let g = (N / 4) as i32;
    for gz in 0..g {
        for gy in 0..g {
            for gx in 0..g {
                for lz in 0..4 {
                    for ly in 0..4 {
                        for lx in 0..4 {
                            let pos = IVec3::new(gx * 4 + lx, gy * 4 + ly, gz * 4 + lz);
                            let seed = seed_cell(seeds[index(pos)]);
                            let solid = seed == pos;
                            let d = if solid { 0.0 } else { 1.0 + (seed - pos).as_vec3().length() };
                            // UNORM: d / 255 stored, rounded to the nearest of 256 levels.
                            sdf[index(pos)] = ((d / 255.0).clamp(0.0, 1.0) * 255.0).round() as u8;
                            if !solid {
                                continue;
                            }
                            let mut bit = 0;
                            let mut neighbours = 0u32;
                            for i in -1..=1 {
                                for j in -1..=1 {
                                    for k in -1..=1 {
                                        if (i, j, k) == (0, 0, 0) {
                                            continue;
                                        }
                                        let np = pos + IVec3::new(i, j, k);
                                        if inside(np) {
                                            if seed_cell(seeds[index(np)]) == pos {
                                                neighbours |= 1 << bit;
                                            }
                                        }
                                        bit += 1;
                                    }
                                }
                            }
                            let i = index(pos);
                            let mut cell = SolidCell { position: (pos.x | (pos.y << 7) | (pos.z << 14)) as u32, ..SolidCell::default() };
                            cell.albedo = (voxels.albedo[i] as u32) >> 1;
                            cell.albedo |= (voxels.facing[i] & 0x3F) << 15;
                            cell.albedo |= neighbours << 21;
                            cell.position |= (neighbours >> 11) << 21;
                            cell.light = voxels.emission[i];
                            cell.light_aniso = voxels.emission_aniso[i];
                            cell.light |= (neighbours >> 22) << 30;
                            cell.light_aniso |= (neighbours >> 24) << 30;
                            cells.push(cell);
                        }
                    }
                }
            }
        }
    }
    Stored { sdf, cells }
}

/// A cascade built from nothing (`DIRTY_ALL`): its one region voxelized,
/// flooded, and stored.
pub struct CascadeData {
    pub voxels: RenderVoxels,
    pub flood: JumpFlood,
    pub stored: Stored,
    /// The packed occlusion word per cell (8 classes x 4 bits, `occlusion_data`'s two halves).
    pub occlusion: Vec<u32>,
}

pub fn build_cascade(objects: &[ObjectData], cascades: &Cascades, cascade: usize) -> CascadeData {
    let mut voxels = RenderVoxels::cleared();
    for region in cascades.pending_regions().iter().filter(|r| r.cascade == cascade) {
        voxelize(objects, region, &mut voxels);
    }
    let flood = jump_flood(&voxels);
    let images = super::occlusion::occlusion(&voxels, cascades.cascades[cascade].position);
    let occlusion = super::occlusion::pack(&images);
    let stored = store(&voxels, &flood.seeds);
    CascadeData { voxels, flood, stored, occlusion }
}

/// `PRE_PROCESS_SCROLL`: a cascade that moved by `scroll` cells
/// (`dirty_regions`) re-injects its old solid cells into the cleared render
/// images at `read_pos + scroll`, dropping those that leave the grid; the
/// cell's stored light (emission, and any static light written back) goes
/// into the emission images, from which STORE copies it again.
pub fn scroll_into(previous: &[SolidCell], scroll: IVec3, voxels: &mut RenderVoxels) {
    for cell in previous {
        let write = cell.cell() + scroll;
        if !inside(write) {
            continue;
        }
        let i = index(write);
        voxels.albedo[i] = (((cell.albedo & 0x7FFF) << 1) | 1) as u16;
        voxels.facing[i] = (cell.albedo >> 15) & 0x3F;
        voxels.emission[i] = cell.light & 0x3fff_ffff;
        voxels.emission_aniso[i] = cell.light_aniso & 0x3fff_ffff;
    }
}

/// `render_region` for a cascade that scrolled: the old cells scrolled in,
/// the new slabs voxelized, then the jump flood, occlusion, and STORE over
/// the whole cascade. (The engine recomputes occlusion only in the probe
/// windows a new slab touches and scrolls the rest; the lab recomputes all of
/// it, from the same voxels.)
pub fn scroll_cascade(objects: &[ObjectData], cascades: &Cascades, cascade: usize, previous: &[SolidCell], scroll: IVec3) -> CascadeData {
    let mut voxels = RenderVoxels::cleared();
    scroll_into(previous, scroll, &mut voxels);
    for region in cascades.pending_regions().iter().filter(|r| r.cascade == cascade) {
        voxelize(objects, region, &mut voxels);
    }
    let flood = jump_flood(&voxels);
    let images = super::occlusion::occlusion(&voxels, cascades.cascades[cascade].position);
    let occlusion = super::occlusion::pack(&images);
    let stored = store(&voxels, &flood.seeds);
    CascadeData { voxels, flood, stored, occlusion }
}
