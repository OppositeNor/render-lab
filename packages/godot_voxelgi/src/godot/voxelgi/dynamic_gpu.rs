//! Dynamic objects on the GPU: the commands `GI::VoxelGIInstance::update`
//! records each frame for a GeometryInstance3D with GI mode Dynamic
//! (servers/rendering/renderer_rd/environment/gi.cpp:3177-3394), for the
//! program's plan (update.rs). For each of the six views of the object's
//! oversampled box: the object drawn with its material into the first
//! dynamic map (`_render_material`: albedo, normal, ORM, emission, depth),
//! `MODE_DYNAMIC_LIGHTING` over that map, then one `MODE_DYNAMIC_SHRINK`
//! dispatch per map after it (write, write and plot, plot). Each map's
//! parameters are Godot's `VoxelGIDynamicPushConstant`, computed here on the
//! CPU as the C++ computes them, uploaded as one record per map (the lab's
//! dispatch parameters hold sixteen words) and named by the dispatch.
//! dynamic.rs is the CPU mirror.
//!
//! Where the lab differs: the twelve dynamic maps (Godot's separate
//! textures, `dynamic_maps`) are the twelve mips of one light image and one
//! depth image, each mip the size of its map (16x to 2x oversampled, then one
//! per light mip); the normal is written without Godot's best-fit scale
//! (`encode24` scales it along itself, which the lighting pass's
//! normalisation undoes).
//!
//! Ported from Godot Engine (MIT; Copyright (c) 2014-present Godot Engine
//! contributors, Copyright (c) 2007-2014 Juan Linietsky, Ariel Manzur).

use super::dynamic::{self, MULTIPLIER, OVERSAMPLED, ViewFrame};
use super::storage::Probe;
use super::world::{self, LightSettings};
use glam::{IVec2, IVec3, Vec3};
use lab::investigation::{Element, ProgramCommand};
use lab::layout::ObjectData;

/// Words of one map's record (`VoxelgiDynamicRecord` in voxelgi_passes.wgsl).
pub const RECORD_WORDS: usize = 32;
/// The dispatch modes a record names: Godot's four dynamic pipelines.
pub const MODE_LIGHTING: u32 = 0;
pub const MODE_SHRINK_WRITE: u32 = 1;
pub const MODE_SHRINK_WRITE_PLOT: u32 = 2;
pub const MODE_SHRINK_PLOT: u32 = 3;

/// `VoxelGIDynamicPushConstant` (gi.h), with the dispatch's mode in its first pad word.
#[derive(Clone, Copy, Debug, Default)]
pub struct Record {
    pub limits: IVec3,
    pub light_count: u32,
    pub x_dir: IVec3,
    pub z_base: f32,
    pub y_dir: IVec3,
    pub z_sign: f32,
    pub z_dir: IVec3,
    pub pos_multiplier: f32,
    pub rect_pos: IVec2,
    pub rect_size: IVec2,
    pub prev_rect_ofs: IVec2,
    pub prev_rect_size: IVec2,
    pub flip_x: bool,
    pub flip_y: bool,
    pub dynamic_range: f32,
    pub on_mipmap: bool,
    pub propagation: f32,
    pub cell_size: f32,
    pub mode: u32,
}

impl Record {
    /// Its words, in the push constant's order.
    pub fn words(&self) -> [u32; RECORD_WORDS] {
        let i = |v: i32| v as u32;
        let f = f32::to_bits;
        [
            i(self.limits.x),
            i(self.limits.y),
            i(self.limits.z),
            self.light_count,
            i(self.x_dir.x),
            i(self.x_dir.y),
            i(self.x_dir.z),
            f(self.z_base),
            i(self.y_dir.x),
            i(self.y_dir.y),
            i(self.y_dir.z),
            f(self.z_sign),
            i(self.z_dir.x),
            i(self.z_dir.y),
            i(self.z_dir.z),
            f(self.pos_multiplier),
            i(self.rect_pos.x),
            i(self.rect_pos.y),
            i(self.rect_size.x),
            i(self.rect_size.y),
            i(self.prev_rect_ofs.x),
            i(self.prev_rect_ofs.y),
            i(self.prev_rect_size.x),
            i(self.prev_rect_size.y),
            self.flip_x as u32,
            self.flip_y as u32,
            f(self.dynamic_range),
            self.on_mipmap as u32,
            f(self.propagation),
            f(self.cell_size),
            self.mode,
            0,
        ]
    }
}

/// One view of one object: its draw's parameters (the object, the eye, the
/// basis, the rectangle's size, the box's depth), its draw's size, and its
/// maps' records (the first map's lighting, then each shrink).
pub struct ViewPlan {
    pub draw: [u32; 16],
    pub size: [u32; 2],
    pub records: Vec<Record>,
}

/// The maps of a view's chain: the oversampled ones, then one per light mip.
pub fn map_count(probe: &Probe) -> usize {
    OVERSAMPLED + probe.mip_count()
}

/// The six views of `object` (its world record, object `index`) in `probe`,
/// lit by `light_count` lights at `settings` (gi.cpp:3204-3392).
pub fn views(
    probe: &Probe,
    (object, index): (&ObjectData, u32),
    light_count: u32,
    settings: &LightSettings,
) -> Vec<ViewPlan> {
    let (begin, end) = dynamic::oversampled_box(probe, object, world::NODE);
    let size = end - begin;
    let params = world::params(probe, settings);
    (0..6)
        .map(|j| {
            // The view's camera, its first map's record, then each shrink's.
            let (_, basis, eye) = dynamic::view_axes(j, begin, end);
            let frame = ViewFrame::new(j, begin, end);
            let first = Record {
                limits: probe.octree_size,
                light_count,
                x_dir: frame.dirs[0],
                z_base: frame.z_base,
                y_dir: frame.dirs[1],
                z_sign: frame.z_sign,
                z_dir: frame.dirs[2],
                pos_multiplier: 1.0 / MULTIPLIER as f32,
                rect_pos: frame.rect_pos,
                rect_size: frame.rect_size,
                flip_x: basis[0].dot(Vec3::ONE) < 0.0,
                flip_y: basis[1].dot(Vec3::ONE) < 0.0,
                dynamic_range: params.dynamic_range,
                propagation: params.propagation,
                cell_size: params.cell_size,
                mode: MODE_LIGHTING,
                ..Default::default()
            };
            let f = f32::to_bits;
            let draw = [
                index,
                f(eye.x),
                f(eye.y),
                f(eye.z),
                f(basis[0].x),
                f(basis[0].y),
                f(basis[0].z),
                f(basis[1].x),
                f(basis[1].y),
                f(basis[1].z),
                f(basis[2].x),
                f(basis[2].y),
                f(basis[2].z),
                f(frame.rect_size.x as f32),
                f(frame.rect_size.y as f32),
                f(size[frame.axes[2]] as f32),
            ];
            ViewPlan {
                draw,
                size: [frame.rect_size.x as u32, frame.rect_size.y as u32],
                records: chain(probe, first),
            }
        })
        .collect()
}

/// A view's records: its first map's, then each shrink's, the rectangle halved
/// (an odd start keeps the texel before it, an odd size rounds up) and the
/// limits halved once a map is a light mip after the first (gi.cpp:3326-3386).
fn chain(probe: &Probe, first: Record) -> Vec<Record> {
    let maps = map_count(probe);
    let mut records = vec![first];
    let mut r = first;
    for k in 1..maps {
        let mipmap = k as i32 - OVERSAMPLED as i32;
        let mut ofs = IVec2::ZERO;
        let (mut pos, mut size) = (r.rect_pos, r.rect_size);
        for a in 0..2 {
            // Each axis of the rectangle halved, keeping every texel of the bigger map.
            if pos[a] & 1 != 0 {
                size[a] += 1;
                ofs[a] = 1;
            }
            if size[a] & 1 != 0 {
                size[a] += 1;
            }
            pos[a] >>= 1;
            size[a] = (size[a] >> 1).max(1);
        }
        if mipmap > 0 {
            r.limits = (r.limits / 2).max(IVec3::ONE);
        }
        r.prev_rect_ofs = ofs;
        r.prev_rect_size = r.rect_size;
        r.rect_pos = pos;
        r.rect_size = size;
        r.on_mipmap = mipmap > 0;
        r.mode = if mipmap < 0 {
            MODE_SHRINK_WRITE
        } else if k < maps - 1 {
            MODE_SHRINK_WRITE_PLOT
        } else {
            MODE_SHRINK_PLOT
        };
        records.push(r);
    }
    records
}

/// The commands for the dynamic objects (the teapot, `object`): their records
/// uploaded, then for each view its draw, its lighting, and its shrinks, each
/// dispatch 8 x 8 texels to a workgroup over its map's rectangle.
pub fn commands(
    out: &mut Vec<ProgramCommand>,
    probe: &Probe,
    object: (&ObjectData, u32),
    (light_count, settings): (u32, &LightSettings),
) {
    let views = views(probe, object, light_count, settings);
    let maps = map_count(probe);
    let words: Vec<u32> = views
        .iter()
        .flat_map(|v| v.records.iter().flat_map(|r| r.words()))
        .collect();
    out.push(ProgramCommand::Upload {
        buffer: Element::of("voxelgiDynamicRecords", 0),
        offset: 0,
        words,
    });
    for (j, v) in views.iter().enumerate() {
        // The view drawn, lit, and shrunk map by map, plotting from the first light mip on.
        out.push(ProgramCommand::Draw {
            pass: "voxelgiDynamicDraw",
            instance: 0,
            size: v.size,
            params: v.draw,
        });
        for (k, r) in v.records.iter().enumerate() {
            let mut params = [0u32; 16];
            params[0] = (j * maps + k) as u32;
            out.push(ProgramCommand::Dispatch {
                pass: if k == 0 {
                    "voxelgiDynamicLightingCS"
                } else {
                    "voxelgiDynamicShrinkCS"
                },
                instance: if k == 0 {
                    0
                } else {
                    k as u32
                },
                groups: [
                    (r.rect_size.x as u32).div_ceil(8),
                    (r.rect_size.y as u32).div_ceil(8),
                    1,
                ],
                params,
            });
        }
    }
}
