//! What the bake hands the renderer, and how the renderer keeps it: the two
//! per-cell records (`get_voxel_gi_octree_cells`, `get_voxel_gi_data_cells`,
//! scene/3d/voxelizer.cpp), the level counts, and the per-instance light
//! texture's layout (`GI::voxel_gi_allocate_data` and
//! `GI::VoxelGIInstance::update`, servers/rendering/renderer_rd/environment/gi.cpp):
//! an RGBA8 3D texture of the octree's size with one mip per level, mip 0 the
//! leaves and the last mip the root.
//!
//! Ported from Godot Engine (MIT; Copyright (c) 2014-present Godot Engine
//! contributors, Copyright (c) 2007-2014 Juan Linietsky, Ariel Manzur).

use super::bake::{Cell, Voxelizer};
use glam::{IVec3, Vec3, Vec4};

/// `CellChildren`: eight cell indices, 32 bytes.
pub type CellChildren = [u32; 8];

/// `CellData` (voxel_gi.glsl): position, albedo, emission, normal, 16 bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CellData {
    pub position: u32,
    pub albedo: u32,
    pub emission: u32,
    pub normal: u32,
}

impl CellData {
    /// The shader's decode: `posu = (p & 0x7FF, (p >> 11) & 0x3FF, p >> 21)`.
    pub fn pos(&self) -> IVec3 {
        IVec3::new((self.position & 0x7FF) as i32, ((self.position >> 11) & 0x3FF) as i32, (self.position >> 21) as i32)
    }
    pub fn albedo(&self) -> Vec4 {
        unpack_unorm4x8(self.albedo)
    }
    pub fn emission(&self) -> Vec3 {
        decode_rgbe9995(self.emission)
    }
    pub fn normal(&self) -> Vec3 {
        unpack_snorm4x8(self.normal).truncate()
    }
}

/// `Color::to_rgbe9995` (core/math/color.h), bit for bit: the channels
/// clamped to [0, 0x1FF << 7], the shared exponent from the largest, and nine
/// rounded bits each.
pub fn to_rgbe9995(c: Vec3) -> u32 {
    let k_max = (0x1FF << 7) as f32;
    let k_min = 1.0 / (1u32 << 16) as f32;
    let r = c.x.clamp(0.0, k_max);
    let g = c.y.clamp(0.0, k_max);
    let b = c.z.clamp(0.0, k_max);
    let max_channel = r.max(g).max(b.max(k_min));
    let mut e = max_channel.to_bits();
    e = e.wrapping_add(0x0780_4000);
    e &= 0x7F80_0000;
    let ef = f32::from_bits(e);
    let ri = (r + ef).to_bits();
    let gi = (g + ef).to_bits();
    let bi = (b + ef).to_bits();
    e <<= 4;
    e = e.wrapping_add(0x1000_0000);
    e | (bi << 18) | (gi << 9) | (ri & 511)
}

/// The shaders' decode: three 9-bit mantissas times `2^(e - 15 - 9)`.
pub fn decode_rgbe9995(e: u32) -> Vec3 {
    let m = 2f32.powf((e >> 27) as f32 - 15.0 - 9.0);
    Vec3::new((e & 0x1FF) as f32, ((e >> 9) & 0x1FF) as f32, ((e >> 18) & 0x1FF) as f32) * m
}

pub fn unpack_unorm4x8(v: u32) -> Vec4 {
    Vec4::new((v & 0xFF) as f32, ((v >> 8) & 0xFF) as f32, ((v >> 16) & 0xFF) as f32, (v >> 24) as f32) / 255.0
}

pub fn unpack_snorm4x8(v: u32) -> Vec4 {
    let s = |k: u32| (((v >> (8 * k)) & 0xFF) as u8 as i8 as f32 / 127.0).clamp(-1.0, 1.0);
    Vec4::new(s(0), s(1), s(2), s(3))
}

/// `get_voxel_gi_octree_cells`: the children of every cell, in order.
pub fn octree_cells(cells: &[Cell]) -> Vec<CellChildren> {
    cells.iter().map(|c| c.children).collect()
}

/// `get_voxel_gi_data_cells`: one `CellData` per cell.
pub fn data_cells(cells: &[Cell]) -> Vec<CellData> {
    cells.iter().map(data_cell).collect()
}

pub fn data_cell(c: &Cell) -> CellData {
    let q = |v: f32| (v * 255.0).clamp(0.0, 255.0) as u32;
    let position = c.x as u32 | ((c.y as u32) << 11) | ((c.z as u32) << 21);
    let albedo = (q(c.alpha) << 24) | (q(c.albedo[2]) << 16) | (q(c.albedo[1]) << 8) | q(c.albedo[0]);
    let emission = to_rgbe9995(Vec3::from(c.emission));
    let n = super::bake::normalized(Vec3::from(c.normal));
    let s = |v: f32| (v * 127.0).clamp(-128.0, 127.0) as i8 as u8 as u32;
    let normal = s(n.x) | (s(n.y) << 8) | (s(n.z) << 16);
    CellData { position, albedo, emission, normal }
}

/// The uploaded VoxelGI data (`GI::VoxelGI`): the records, the level counts,
/// the octree's size, the transform to cells, and the distance field.
#[derive(Clone)]
pub struct Probe {
    pub octree: Vec<CellChildren>,
    pub data: Vec<CellData>,
    pub level_counts: Vec<u32>,
    pub octree_size: IVec3,
    /// `to_cell_xform`: local position to cells.
    pub to_cell_scale: f32,
    pub to_cell_origin: Vec3,
    pub sdf: Vec<u8>,
}

impl Probe {
    pub fn from_bake(v: &Voxelizer, sdf: Vec<u8>) -> Probe {
        let s = v.octree_size();
        Probe {
            octree: octree_cells(&v.cells),
            data: data_cells(&v.cells),
            level_counts: v.level_counts(),
            octree_size: IVec3::new(s[0], s[1], s[2]),
            to_cell_scale: v.to_cell_scale,
            to_cell_origin: v.to_cell_origin,
            sdf,
        }
    }

    /// The mip count of the light texture: one per level.
    pub fn mip_count(&self) -> usize {
        self.level_counts.len()
    }

    /// The level a mip holds (`mipmap.level = levels.size() - i - 1`).
    pub fn mip_level(&self, mip: usize) -> usize {
        self.mip_count() - mip - 1
    }

    /// The first cell of a level (`cell_offset`, the counts of the coarser levels).
    pub fn level_offset(&self, level: usize) -> usize {
        self.level_counts[..level].iter().map(|&c| c as usize).sum()
    }

    /// The cells of a level, as a range of indices.
    pub fn level_range(&self, level: usize) -> std::ops::Range<usize> {
        let o = self.level_offset(level);
        o..o + self.level_counts[level] as usize
    }

    pub fn leaf_level(&self) -> usize {
        self.mip_count() - 1
    }

    /// A mip's size: the octree's halved `mip` times, at least 1.
    pub fn mip_size(&self, mip: usize) -> IVec3 {
        IVec3::new(self.octree_size.x >> mip, self.octree_size.y >> mip, self.octree_size.z >> mip).max(IVec3::ONE)
    }

    pub fn to_cell(&self, local: Vec3) -> Vec3 {
        (local - self.to_cell_origin) * self.to_cell_scale
    }

    pub fn from_cell(&self, cell: Vec3) -> Vec3 {
        cell / self.to_cell_scale + self.to_cell_origin
    }

    /// The SDF byte of a leaf cell (x fastest, then y, then z).
    pub fn sdf_at(&self, p: IVec3) -> u8 {
        let s = self.octree_size;
        let c = p.clamp(IVec3::ZERO, s - IVec3::ONE);
        self.sdf[(c.x + c.y * s.x + c.z * s.x * s.y) as usize]
    }

    /// The leaf cell at `p`, if the octree has one: walk down from the root.
    pub fn find_leaf(&self, p: IVec3) -> Option<usize> {
        let depth = self.leaf_level() as i32;
        let mut idx = 0usize;
        for level in 0..depth {
            let half = 1 << (depth - level - 1);
            let octant = ((p.x / half) & 1) | (((p.y / half) & 1) << 1) | (((p.z / half) & 1) << 2);
            let child = self.octree[idx][octant as usize];
            if child == super::bake::CHILD_EMPTY {
                return None;
            }
            idx = child as usize;
        }
        Some(idx)
    }

    /// The cell at level `level` that contains leaf position `p`, if any.
    pub fn find_cell(&self, p: IVec3, level: usize) -> Option<usize> {
        let depth = self.leaf_level() as i32;
        let mut idx = 0usize;
        for l in 0..level as i32 {
            let half = 1 << (depth - l - 1);
            let octant = ((p.x / half) & 1) | (((p.y / half) & 1) << 1) | (((p.z / half) & 1) << 2);
            let child = self.octree[idx][octant as usize];
            if child == super::bake::CHILD_EMPTY {
                return None;
            }
            idx = child as usize;
        }
        Some(idx)
    }

    /// The cell's level, from its place in the sorted array.
    pub fn level_of(&self, cell: usize) -> usize {
        let mut o = 0usize;
        for (l, &c) in self.level_counts.iter().enumerate() {
            o += c as usize;
            if cell < o {
                return l;
            }
        }
        self.leaf_level()
    }
}

/// The instance's RGBA8 3D texture, every mip, as the write pass leaves it:
/// texel = (light / dynamic range, coverage), 8 bits each.
#[derive(Clone)]
pub struct LightTexture {
    pub sizes: Vec<IVec3>,
    pub mips: Vec<Vec<[u8; 4]>>,
}

impl LightTexture {
    /// Cleared to (0, 0, 0, 0) over all mips (`texture_clear`).
    pub fn cleared(probe: &Probe) -> LightTexture {
        let sizes: Vec<IVec3> = (0..probe.mip_count()).map(|m| probe.mip_size(m)).collect();
        LightTexture { mips: sizes.iter().map(|s| vec![[0u8; 4]; (s.x * s.y * s.z) as usize]).collect(), sizes }
    }

    pub fn index(&self, mip: usize, p: IVec3) -> usize {
        let s = self.sizes[mip];
        (p.x + p.y * s.x + p.z * s.x * s.y) as usize
    }

    pub fn texel(&self, mip: usize, p: IVec3) -> Vec4 {
        let s = self.sizes[mip];
        let c = p.clamp(IVec3::ZERO, s - IVec3::ONE);
        let t = self.mips[mip][self.index(mip, c)];
        Vec4::new(t[0] as f32, t[1] as f32, t[2] as f32, t[3] as f32) / 255.0
    }

    /// The RGBA8 store of `imageStore`: each channel to the nearest of 256 steps.
    pub fn store(&mut self, mip: usize, p: IVec3, v: Vec4) {
        let i = self.index(mip, p);
        self.mips[mip][i] = unorm8(v);
    }

    /// Trilinear filtering inside one mip, clamped to the edge (the
    /// `LINEAR_WITH_MIPMAPS`, repeat-disabled sampler).
    pub fn sample_mip(&self, mip: usize, uvw: Vec3) -> Vec4 {
        let s = self.sizes[mip].as_vec3();
        let t = uvw * s - Vec3::splat(0.5);
        let b = t.floor();
        let f = t - b;
        let bi = b.as_ivec3();
        let mut sum = Vec4::ZERO;
        for i in 0..8 {
            let o = IVec3::new(i & 1, (i >> 1) & 1, (i >> 2) & 1);
            let w = Vec3::select(o.cmpeq(IVec3::ONE), f, Vec3::ONE - f);
            sum += self.texel(mip, bi + o) * (w.x * w.y * w.z);
        }
        sum
    }

    /// `textureLod`: the level clamped to the mips, the two nearest mips
    /// blended linearly.
    pub fn sample_lod(&self, uvw: Vec3, lod: f32) -> Vec4 {
        let max = (self.mips.len() - 1) as f32;
        let lod = lod.clamp(0.0, max);
        let m0 = lod.floor() as usize;
        let m1 = (m0 + 1).min(self.mips.len() - 1);
        let f = lod - m0 as f32;
        let a = self.sample_mip(m0, uvw);
        if f == 0.0 || m0 == m1 {
            return a;
        }
        a * (1.0 - f) + self.sample_mip(m1, uvw) * f
    }

    /// Every texel of every mip as one RGBA8 word each, x fastest, mip 0
    /// first: what the scene shading's data buffer carries.
    pub fn words(&self) -> Vec<u32> {
        self.mips.iter().flat_map(|m| m.iter().map(|t| u32::from_le_bytes(*t))).collect()
    }
}

/// A float to an 8-bit UNORM, as the GPU stores it: clamped, then rounded.
pub fn unorm8(v: Vec4) -> [u8; 4] {
    let q = |c: f32| (c.clamp(0.0, 1.0) * 255.0).round() as u8;
    [q(v.x), q(v.y), q(v.z), q(v.w)]
}
