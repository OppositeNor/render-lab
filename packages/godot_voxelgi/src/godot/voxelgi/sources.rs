//! The VoxelGI investigation's source index (source-index.json): every Godot
//! file, symbol, and line the viewer cites, written by source_index.py from
//! the checkout. `selftest` checks each line's hash against the checkout
//! when it is present.

use lab::utility::sources::PackageIndex;

pub static INDEX: PackageIndex = PackageIndex::new("godot_voxelgi.source-index");
