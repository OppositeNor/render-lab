//! What the investigation's scripts (script/*.lam, lab/src/script/runtime.rs) read from its code: the probe's
//! octree, which the bake computes (world.rs), so the scripts only decide what a click picks.
//!
//! | Native | Returns |
//! |---|---|
//! | `octreeSize` | the probe's octree size in leaves, `{x, y, z}` |
//! | `leafOf point` | the leaf a point of the room falls in |

use lab::script::lamlet::embed::{Error, NativeRegistry};
use lab::script::runtime::V3;

lab::lamlet_record! {
    /// A leaf of the octree, or a size in leaves.
    pub struct Cell {
        pub x: i64 = "x",
        pub y: i64 = "y",
        pub z: i64 = "z",
    }
}

impl Cell {
    fn of(c: [i32; 3]) -> Cell {
        Cell {
            x: c[0] as i64,
            y: c[1] as i64,
            z: c[2] as i64,
        }
    }
}

/// Registers the natives (`Investigation::script_natives`).
pub fn register(natives: &mut NativeRegistry) -> Result<(), Error> {
    natives
        .function("godot_voxelgi/octree-size", |_, ()| {
            Ok(Cell::of(super::world::world().probe.octree_size.to_array()))
        })
        .pure()
        .doc("Fn([], {x: Int, y: Int, z: Int}). The probe's octree size, in leaves.")
        .register()?;
    natives
        .function("godot_voxelgi/leaf-of", |_, (p,): (V3,)| Ok(Cell::of(super::demos::leaf_of(&super::world::world().probe, p.0).to_array())))
        .pure()
        .doc("Fn([Vec3], {x: Int, y: Int, z: Int}). The leaf of the probe's octree a point of the room falls in.")
        .register()?;
    Ok(())
}
