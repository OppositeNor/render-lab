//! The Godot SDFGI investigation (packages/godot_sdfgi/src/godot/sdfgi), built
//! as the godot_sdfgi package's library. The lab opens it at run time and
//! calls `godot_sdfgi_negotiate` (manifest.toml's `negotiate`), which hands
//! the lab the package's handles (lab's src/plugin.rs).

mod godot;

use lab::investigation::Investigation;
use lab::plugin::InvestigationsFn;

/// The package's investigations, as the lab lists them.
fn investigations() -> &'static [&'static dyn Investigation] {
    static ALL: [&dyn Investigation; 1] = [&godot::sdfgi::SDFGI];
    &ALL
}

// The package's negotiation function (manifest.toml's `negotiate`): it hands
// the lab its handles once the lab's interface version and build match its own.
lab::negotiate!(godot_sdfgi_negotiate, ["investigations" => investigations as InvestigationsFn]);
