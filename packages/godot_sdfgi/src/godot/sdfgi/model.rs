//! Godot's SDFGI on the CPU, at the engine's sizes: the cascades and how they
//! follow the camera (`GI::SDFGI::create`, `update`, `get_pending_region_data`,
//! `update_cascades` in servers/rendering/renderer_rd/environment/gi.cpp).
//! Each function follows the engine's code line by line, with its integer
//! arithmetic and its truncations, so the demonstrations and the selftest can
//! show the engine's numbers.
//!
//! Ported from Godot Engine (MIT; Copyright (c) 2014-present Godot Engine
//! contributors, Copyright (c) 2007-2014 Juan Linietsky, Ariel Manzur).

use glam::{IVec3, Vec3};

/// `SDFGI::CASCADE_SIZE`: cells per axis of every cascade (gi.h).
pub const CASCADE_SIZE: i32 = 128;
/// `SDFGI::PROBE_DIVISOR`: probe intervals per axis (gi.h).
pub const PROBE_DIVISOR: i32 = 16;
/// `probe_axis_count = PROBE_DIVISOR + 1` (gi.cpp, `create`).
pub const PROBE_AXIS_COUNT: i32 = PROBE_DIVISOR + 1;
/// Cells between two probes, `cascade_size / PROBE_DIVISOR`.
pub const PROBE_CELLS: i32 = CASCADE_SIZE / PROBE_DIVISOR;
/// `y_scale[3] = { 2.0, 1.5, 1.0 }`, indexed by the environment's y scale
/// (50 %, 75 %, 100 %).
pub const Y_SCALE: [f32; 3] = [2.0, 1.5, 1.0];

/// The environment's settings the cascades depend on (Environment's
/// `sdfgi_*` properties, with their defaults).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    /// `sdfgi_cascades`, 1 to 8 (default 4).
    pub cascades: usize,
    /// `sdfgi_min_cell_size`, metres (default 0.2).
    pub min_cell_size: f32,
    /// `sdfgi_y_scale`: 0 = 50 %, 1 = 75 % (default), 2 = 100 %.
    pub y_scale_mode: usize,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings { cascades: 4, min_cell_size: 0.2, y_scale_mode: 1 }
    }
}

impl Settings {
    pub fn y_mult(&self) -> f32 {
        Y_SCALE[self.y_scale_mode.min(2)]
    }
}

/// `SDFGI::Cascade::dirty_regions`: per axis, the cells to rebuild (positive:
/// from the start of the texture, negative: from its end), or all of them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dirty {
    Regions(IVec3),
    /// `DIRTY_ALL`.
    All,
}

impl Dirty {
    pub fn is_clean(self) -> bool {
        self == Dirty::Regions(IVec3::ZERO)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cascade {
    /// Metres per cell (`cell_size`).
    pub cell_size: f32,
    /// The cascade's centre in cells of this cascade, from the world origin,
    /// with y in the squashed space (`position`).
    pub position: IVec3,
    pub dirty: Dirty,
}

/// One region `get_pending_region_data` hands the culling: the cascade, its
/// cells, and its world box (real metres).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Region {
    pub cascade: usize,
    pub local_offset: IVec3,
    pub local_size: IVec3,
    pub bounds_min: Vec3,
    pub bounds_size: Vec3,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Cascades {
    pub settings: Settings,
    pub cascades: Vec<Cascade>,
}

impl Cascades {
    /// `SDFGI::create`: each cascade centred on the probe nearest the camera,
    /// twice the cell size of the last, and all of it dirty.
    pub fn create(settings: Settings, world_position: Vec3) -> Cascades {
        let y_mult = settings.y_mult();
        let mut base_cell_size = settings.min_cell_size;
        let mut cascades = Vec::with_capacity(settings.cascades);
        for _ in 0..settings.cascades {
            let cell_size = base_cell_size;
            let world = Vec3::new(world_position.x, world_position.y * y_mult, world_position.z);
            let probe_size = Vec3::splat(cell_size * PROBE_CELLS as f32);
            let probe_pos = (world / probe_size + Vec3::splat(0.5)).floor().as_ivec3();
            cascades.push(Cascade { cell_size, position: probe_pos * PROBE_CELLS, dirty: Dirty::All });
            base_cell_size *= 2.0;
        }
        Cascades { settings, cascades }
    }

    /// `SDFGI::update`: each cascade jumps `2 * drag_margin` cells whenever
    /// the camera's cell is more than `drag_margin` from its centre, and
    /// records the cells that jump uncovered.
    pub fn update(&mut self, world_position: Vec3) {
        let y_mult = self.settings.y_mult();
        let drag_margin = (CASCADE_SIZE / PROBE_DIVISOR) / 2;
        for cascade in &mut self.cascades {
            let mut dirty = IVec3::ZERO;
            // probe_half_size is computed, then set to zero.
            let world = Vec3::new(world_position.x, world_position.y * y_mult, world_position.z);
            // Vector3i(Vector3) truncates toward zero.
            let pos_in_cascade = (world / cascade.cell_size).as_ivec3();
            let mut all = false;
            for j in 0..3 {
                if pos_in_cascade[j] < cascade.position[j] {
                    while pos_in_cascade[j] < cascade.position[j] - drag_margin {
                        cascade.position[j] -= drag_margin * 2;
                        dirty[j] += drag_margin * 2;
                    }
                } else if pos_in_cascade[j] > cascade.position[j] {
                    while pos_in_cascade[j] > cascade.position[j] + drag_margin {
                        cascade.position[j] += drag_margin * 2;
                        dirty[j] -= drag_margin * 2;
                    }
                }
                if dirty[j] == 0 {
                    continue;
                } else if dirty[j].unsigned_abs() >= CASCADE_SIZE as u32 {
                    // Moved too much: the later axes are not visited this frame.
                    all = true;
                    break;
                }
            }
            cascade.dirty = if all { Dirty::All } else { Dirty::Regions(dirty) };
            if let Dirty::Regions(d) = cascade.dirty
                && d != IVec3::ZERO
            {
                let size = CASCADE_SIZE as u32;
                let total_volume = size * size * size;
                let safe_volume: u32 = (0..3).map(|j| size - d[j].unsigned_abs()).product();
                let dirty_volume = total_volume - safe_volume;
                if dirty_volume > safe_volume / 2 {
                    cascade.dirty = Dirty::All;
                }
            }
        }
    }

    /// `get_pending_region_data` for every region in its order: cascades
    /// ascending, then x, y, z, each later axis's region chipped by the
    /// earlier ones.
    pub fn pending_regions(&self) -> Vec<Region> {
        let y_mult = self.settings.y_mult();
        let squash = Vec3::new(1.0, 1.0 / y_mult, 1.0);
        let half = IVec3::splat(-(CASCADE_SIZE >> 1));
        let mut regions = Vec::new();
        for (i, c) in self.cascades.iter().enumerate() {
            match c.dirty {
                Dirty::All => {
                    let size = IVec3::splat(CASCADE_SIZE);
                    regions.push(Region { cascade: i, local_offset: IVec3::ZERO, local_size: size, bounds_min: (half + c.position).as_vec3() * c.cell_size * squash, bounds_size: size.as_vec3() * c.cell_size * squash });
                }
                Dirty::Regions(d) => {
                    for j in 0..3 {
                        if d[j] == 0 {
                            continue;
                        }
                        let mut from = IVec3::ZERO;
                        let mut to = IVec3::splat(CASCADE_SIZE);
                        if d[j] > 0 {
                            to[j] = d[j];
                        } else {
                            from[j] = to[j] + d[j];
                        }
                        for k in 0..j {
                            if d[k] > 0 {
                                from[k] += d[k];
                            } else if d[k] < 0 {
                                to[k] += d[k];
                            }
                        }
                        let size = to - from;
                        regions.push(Region { cascade: i, local_offset: from, local_size: size, bounds_min: (from + half + c.position).as_vec3() * c.cell_size * squash, bounds_size: size.as_vec3() * c.cell_size * squash });
                    }
                }
            }
        }
        regions
    }

    /// A cascade's box in real metres (`update_cascades`' offset, with y
    /// divided by `y_mult`): its minimum corner and size.
    pub fn bounds(&self, i: usize) -> (Vec3, Vec3) {
        let c = &self.cascades[i];
        let squash = Vec3::new(1.0, 1.0 / self.settings.y_mult(), 1.0);
        let min = (IVec3::splat(-(CASCADE_SIZE >> 1)) + c.position).as_vec3() * c.cell_size * squash;
        (min, Vec3::splat(CASCADE_SIZE as f32 * c.cell_size) * squash)
    }

    /// The world position (real metres) of probe `p` (0 to 16 per axis) of
    /// cascade `i`: probes sit every `PROBE_CELLS` cells from the box's
    /// minimum corner (sdfgi_integrate.glsl, `probe_pos`).
    pub fn probe_position(&self, i: usize, p: IVec3) -> Vec3 {
        let c = &self.cascades[i];
        let squash = Vec3::new(1.0, 1.0 / self.settings.y_mult(), 1.0);
        let offset = (IVec3::splat(-(CASCADE_SIZE >> 1)) + c.position).as_vec3() * c.cell_size;
        (offset + p.as_vec3() * (PROBE_CELLS as f32 * c.cell_size)) * squash
    }
}

/// The cascades after the camera moved from `start` along `path` (one
/// `update` per point): what the lab shows for a camera position, since the
/// engine's state depends on where the camera has been.
pub fn follow(settings: Settings, start: Vec3, path: &[Vec3]) -> Cascades {
    let mut cascades = Cascades::create(settings, start);
    for &p in path {
        cascades.update(p);
    }
    cascades
}
