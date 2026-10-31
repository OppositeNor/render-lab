//! The bake's distance field, as `Voxelizer::get_sdf_3d_image` computes it
//! (scene/3d/voxelizer.cpp): every leaf cell a seed of squared distance 0,
//! every other texel 1e20, then the exact one-dimensional distance transform
//! `edt` ("based on https://prideout.net/blog/distance_fields/") along z, then
//! y, then x, and one byte per texel, 0 for solid and else the truncated
//! distance in cells, at most 254, plus one. The renderer uploads it as an
//! R8_UNORM 3D texture (gi.cpp); a GPU version of the same pass is compiled
//! out (`#if 0`).
//!
//! Ported from Godot Engine (MIT; Copyright (c) 2014-present Godot Engine
//! contributors, Copyright (c) 2007-2014 Juan Linietsky, Ariel Manzur).

use super::bake::{Cell, Voxelizer};

/// `BIG_VAL`.
pub const BIG_VAL: f32 = 1e20;

/// `edt`: the lower envelope of the parabolas through `f` (squared
/// distances), written back in place, for `n` values `stride` apart.
pub fn edt(f: &mut [f32], start: usize, stride: usize, n: usize) {
    let at = |f: &[f32], q: usize| f[start + q * stride];
    let sq = |v: f32| v * v;
    let mut d = vec![0.0f32; n];
    let mut v = vec![0usize; n];
    let mut z = vec![0.0f32; n + 1];
    let mut k = 0usize;
    v[0] = 0;
    z[0] = -BIG_VAL;
    z[1] = BIG_VAL;
    for q in 1..n {
        let mut s = ((at(f, q) + sq(q as f32)) - (at(f, v[k]) + sq(v[k] as f32))) / (2.0 * q as f32 - 2.0 * v[k] as f32);
        while s <= z[k] {
            k -= 1;
            s = ((at(f, q) + sq(q as f32)) - (at(f, v[k]) + sq(v[k] as f32))) / (2.0 * q as f32 - 2.0 * v[k] as f32);
        }
        k += 1;
        v[k] = q;
        z[k] = s;
        z[k + 1] = BIG_VAL;
    }
    k = 0;
    for q in 0..n {
        while z[k + 1] < q as f32 {
            k += 1;
        }
        d[q] = sq(q as f32 - v[k] as f32) + at(f, v[k]);
    }
    for (i, &di) in d.iter().enumerate() {
        f[start + i * stride] = di;
    }
}

/// The squared distances after `passes` of the three (0 none, 1 along z, 2 then y, 3 then x).
pub fn squared_distances(v: &Voxelizer, passes: usize) -> Vec<f32> {
    let s = v.octree_size();
    let (sx, sy, sz) = (s[0] as usize, s[1] as usize, s[2] as usize);
    let y_mult = sx;
    let z_mult = sx * sy;
    let mut work = vec![BIG_VAL; sx * sy * sz];
    for c in v.cells.iter().filter(|c: &&Cell| c.level as i32 == v.cell_subdiv) {
        work[c.x as usize + c.y as usize * y_mult + c.z as usize * z_mult] = 0.0;
    }
    if passes >= 1 {
        for i in 0..sx {
            for j in 0..sy {
                edt(&mut work, i + j * y_mult, z_mult, sz);
            }
        }
    }
    if passes >= 2 {
        for i in 0..sx {
            for j in 0..sz {
                edt(&mut work, i + j * z_mult, y_mult, sy);
            }
        }
    }
    if passes >= 3 {
        for i in 0..sy {
            for j in 0..sz {
                edt(&mut work, i * y_mult + j * z_mult, 1, sx);
            }
        }
    }
    work
}

/// The byte `get_sdf_3d_image` stores for a squared distance.
pub fn encode(squared: f32) -> u8 {
    let d = squared.sqrt() as u32;
    if d == 0 { 0 } else { (d.min(254) + 1) as u8 }
}

/// `get_sdf_3d_image`.
pub fn sdf_image(v: &Voxelizer) -> Vec<u8> {
    squared_distances(v, 3).iter().map(|&s| encode(s)).collect()
}
