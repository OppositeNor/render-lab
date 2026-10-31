//! The Godot VoxelGI investigation (packages/godot_voxelgi/src/godot/voxelgi),
//! built as the godot_voxelgi package's library. The lab opens it at run time
//! and calls `godot_voxelgi_negotiate` (manifest.toml's `negotiate`), which
//! hands the lab the package's handles (lab's src/plugin.rs).

mod godot;

use lab::investigation::Investigation;
use lab::plugin::InvestigationsFn;

/// The package's investigations, as the lab lists them.
fn investigations() -> &'static [&'static dyn Investigation] {
    static ALL: [&dyn Investigation; 1] = [&godot::voxelgi::VOXELGI];
    &ALL
}

// The package's negotiation function (manifest.toml's `negotiate`): it hands
// the lab its handles once the lab's interface version and build match its own.
lab::negotiate!(godot_voxelgi_negotiate, ["investigations" => investigations as InvestigationsFn]);
