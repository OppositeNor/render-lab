//! The probes' occlusion on the CPU: sdfgi_preprocess.glsl's `MODE_OCCLUSION`
//! as `render_region` dispatches it (gi.cpp: eight dispatches, one per probe
//! parity class) and the packing `MODE_STORE` does.
//!
//! For each probe, a 16^3 window of voxels centred on it: visibility spreads
//! out from the probe shell by shell (cells of equal |x| + |y| + |z| from
//! it), a solid voxel stopping it; then solid voxels take the average of
//! their free neighbours on the probe's side, and free voxels behind a
//! surface that does not face them are dimmed. Every write goes through the
//! R8_UNORM image, so later steps read 8-bit values, as the shader's do.
//!
//! Ported from Godot Engine (MIT; Copyright (c) 2014-present Godot Engine
//! contributors, Copyright (c) 2007-2014 Juan Linietsky, Ariel Manzur).

use super::model::{CASCADE_SIZE, PROBE_CELLS};
use super::voxels::{index, inside, RenderVoxels, CELLS};
use glam::IVec3;

/// `OCCLUSION_SIZE` (gi.cpp: `cascade_size / PROBE_DIVISOR`).
pub const OCCLUSION_SIZE: i32 = PROBE_CELLS;
const OCC_STEPS: i32 = OCCLUSION_SIZE * 3 - 2;

/// `occlusion_shift[8]` of STORE.
pub const OCCLUSION_SHIFT: [u32; 8] = [12, 8, 4, 0, 28, 24, 20, 16];

fn store8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn load8(v: u8) -> f32 {
    v as f32 / 255.0
}

/// One parity class's region: a 16^3 window with its facing bits.
struct Window<'a> {
    voxels: &'a RenderVoxels,
    region_offset: IVec3,
}

impl Window<'_> {
    /// `get_facing(local)`: the facing bits of a window cell (0 outside the grid).
    fn facing(&self, local: IVec3) -> u32 {
        let p = self.region_offset + local;
        if inside(p) && local.cmpge(IVec3::ZERO).all() && local.cmplt(IVec3::splat(OCCLUSION_SIZE * 2)).all() { self.voxels.facing[index(p)] & 0xFF } else { 0 }
    }
}

/// One dispatch (one parity class `occlusion_index`, the probe offset
/// already flipped by the cascade's parity) into `dst`, the class's R8 image.
fn occlusion_class(voxels: &RenderVoxels, probe_offset: IVec3, dst: &mut [u8]) {
    let groups = IVec3::splat(PROBE_CELLS + 1) - probe_offset;
    let grid = IVec3::splat(CASCADE_SIZE);
    for rz in 0..groups.z {
        for ry in 0..groups.y {
            for rx in 0..groups.x {
                let region = IVec3::new(rx, ry, rz);
                let region_offset = -IVec3::splat(OCCLUSION_SIZE) + region * OCCLUSION_SIZE * 2 + probe_offset * OCCLUSION_SIZE;
                let w = Window { voxels, region_offset };
                let in_grid = |p: IVec3| p.cmpge(IVec3::ZERO).all() && p.cmplt(grid).all();
                // Phase 2: shells outward from the probe, in all eight octants.
                for step in 0..OCC_STEPS {
                    for ax in 0..OCCLUSION_SIZE {
                        for ay in 0..OCCLUSION_SIZE {
                            let az = step - ax - ay;
                            if !(0..OCCLUSION_SIZE).contains(&az) {
                                continue;
                            }
                            let proc_abs = IVec3::new(ax, ay, az);
                            for i in 0..8 {
                                let bits = IVec3::new(i & 1, (i >> 1) & 1, (i >> 2) & 1);
                                let proc_sign = bits * 2 - IVec3::ONE;
                                let local = IVec3::splat(OCCLUSION_SIZE) + proc_abs * proc_sign - (IVec3::ONE - bits);
                                let offset = local + region_offset;
                                if !in_grid(offset) {
                                    continue;
                                }
                                let occ = if w.facing(local) != 0 {
                                    0.0
                                } else if step == 0 {
                                    1.0
                                } else {
                                    let read_dir = -proc_sign;
                                    let major = if proc_abs.x < proc_abs.y {
                                        if proc_abs.z < proc_abs.y { IVec3::Y } else { IVec3::Z }
                                    } else if proc_abs.z < proc_abs.x {
                                        IVec3::X
                                    } else {
                                        IVec3::Z
                                    };
                                    let mut avg = 0.0;
                                    let mut occ = 0.0;
                                    for axis in 0..3 {
                                        let mut step_dir = IVec3::ZERO;
                                        step_dir[axis] = read_dir[axis];
                                        let read = offset + step_dir + if proc_abs[axis] == 0 { major * read_dir } else { IVec3::ZERO };
                                        if w.facing(read - region_offset) == 0 {
                                            if in_grid(read) {
                                                occ += load8(dst[index(read)]);
                                                avg += 1.0;
                                            }
                                        } else if proc_abs[axis] != 0 {
                                            avg += 1.0;
                                        }
                                    }
                                    if avg > 0.0 { occ / avg } else { occ }
                                };
                                dst[index(offset)] = store8(occ);
                            }
                        }
                    }
                }
                // Phase 3: bias solid voxels away (they read only free cells, which this phase does not write).
                let mut solid_writes = Vec::new();
                for lz in 0..OCCLUSION_SIZE * 2 {
                    for ly in 0..OCCLUSION_SIZE * 2 {
                        for lx in 0..OCCLUSION_SIZE * 2 {
                            let local = IVec3::new(lx, ly, lz);
                            let offset = region_offset + local;
                            if !in_grid(offset) {
                                continue;
                            }
                            let facing = w.facing(local);
                            if facing == 0 {
                                continue;
                            }
                            let mut proc_pos = local - IVec3::splat(OCCLUSION_SIZE);
                            proc_pos += IVec3::select(proc_pos.cmpge(IVec3::ZERO), IVec3::ONE, IVec3::ZERO);
                            let read_dir = -proc_pos.signum();
                            let facing_pos = IVec3::new((facing & 1) as i32, ((facing >> 1) & 1) as i32, ((facing >> 2) & 1) as i32);
                            let facing_neg = IVec3::new(((facing >> 3) & 1) as i32, ((facing >> 4) & 1) as i32, ((facing >> 5) & 1) as i32);
                            let valid = IVec3::select(read_dir.cmpgt(IVec3::ZERO), facing_pos, facing_neg).cmpne(IVec3::ZERO);
                            let (vx, vy, vz) = (valid.test(0), valid.test(1), valid.test(2));
                            let dx = IVec3::new(read_dir.x, 0, 0);
                            let dy = IVec3::new(0, read_dir.y, 0);
                            let dz = IVec3::new(0, 0, read_dir.z);
                            let mut occ = 0.0;
                            let mut avg = 0.0;
                            let mut read = |d: IVec3| {
                                let r = local + d;
                                if w.facing(r) == 0 {
                                    let g = r + region_offset;
                                    if in_grid(g) {
                                        occ += load8(dst[index(g)]);
                                        avg += 1.0;
                                    }
                                }
                            };
                            if vx {
                                read(dx);
                            }
                            if vy {
                                read(dy);
                            }
                            if vz {
                                read(dz);
                            }
                            if vy && vz {
                                read(dy + dz);
                            }
                            if vx && vz {
                                read(dx + dz);
                            }
                            if vx && vy {
                                read(dx + dy);
                            }
                            if vx && vy && vz {
                                read(read_dir);
                            }
                            solid_writes.push((index(offset), if avg > 0.0 { occ / avg } else { occ }));
                        }
                    }
                }
                for (i, v) in solid_writes {
                    dst[i] = store8(v);
                }
                // Phase 4: free voxels behind surfaces that do not face them.
                for lz in 0..OCCLUSION_SIZE * 2 {
                    for ly in 0..OCCLUSION_SIZE * 2 {
                        for lx in 0..OCCLUSION_SIZE * 2 {
                            let local = IVec3::new(lx, ly, lz);
                            let offset = region_offset + local;
                            if !in_grid(offset) || w.facing(local) != 0 {
                                continue;
                            }
                            let mut proc_pos = local - IVec3::splat(OCCLUSION_SIZE);
                            proc_pos += IVec3::select(proc_pos.cmpge(IVec3::ZERO), IVec3::ONE, IVec3::ZERO);
                            let proc_abs = proc_pos.abs();
                            let read_dir = proc_pos.signum();
                            // block_mask is set to zero at both uses: only read_mask matters.
                            let read_mask = |toward: bool, axis: usize| -> u32 { if toward { [8, 16, 32][axis] } else { [1, 2, 4][axis] } };
                            let mut visible = 0.0;
                            let mut total = 0.0;
                            for axis in 0..3 {
                                if proc_abs[axis] < OCCLUSION_SIZE {
                                    let mut d = IVec3::ZERO;
                                    d[axis] = read_dir[axis];
                                    let mask = w.facing(local + d);
                                    if mask != 0 && in_grid(local + d + region_offset) {
                                        total += 1.0;
                                        if mask & read_mask(read_dir[axis] > 0, axis) != 0 {
                                            visible += 1.0;
                                        }
                                    }
                                }
                            }
                            for axis in 0..3 {
                                if proc_abs[axis] == 1 {
                                    let mut d = IVec3::ZERO;
                                    d[axis] = read_dir[axis];
                                    let mask = w.facing(local - d);
                                    if mask != 0 && in_grid(local - d + region_offset) {
                                        total += 1.0;
                                        if mask & read_mask(read_dir[axis] < 0, axis) != 0 {
                                            visible += 1.0;
                                        }
                                    }
                                }
                            }
                            if total > 0.0 {
                                let occ = load8(dst[index(offset)]) * visible / total;
                                dst[index(offset)] = store8(occ);
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The eight classes' R8 images (`render_occlusion[8]`) for a cascade whose
/// centre is at `position` (cells).
pub fn occlusion(voxels: &RenderVoxels, position: IVec3) -> Vec<Vec<u8>> {
    let probe_global = position / PROBE_CELLS;
    let mut images = vec![vec![0u8; CELLS]; 8];
    std::thread::scope(|s| {
        for (i, image) in images.iter_mut().enumerate() {
            let i = i as i32;
            let mut offset = IVec3::new(i & 1, (i >> 1) & 1, (i >> 2) & 1);
            for axis in 0..3 {
                if probe_global[axis] & 1 != 0 {
                    offset[axis] = (offset[axis] + 1) & 1;
                }
            }
            s.spawn(move || occlusion_class(voxels, offset, image));
        }
    });
    images
}

/// STORE's packing: four bits per class, `uint(clamp(occ * 15, 0, 15))`,
/// shifted by `OCCLUSION_SHIFT` (the low 16 bits go to `occlusion_data` at
/// x, the high 16 at x + 128).
pub fn pack(images: &[Vec<u8>]) -> Vec<u32> {
    (0..CELLS).map(|c| {
        let mut word = 0u32;
        for (i, image) in images.iter().enumerate() {
            let occ = load8(image[c]);
            word |= ((occ * 15.0).clamp(0.0, 15.0) as u32) << OCCLUSION_SHIFT[i];
        }
        word
    }).collect()
}

/// A class's value at a cell, as the readers decode it (4 bits over 15).
pub fn class_value(packed: u32, class: usize) -> f32 {
    ((packed >> OCCLUSION_SHIFT[class]) & 0xF) as f32 / 15.0
}
