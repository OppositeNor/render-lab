//! The Godot SSR investigation (packages/godot_ssr/src/godot/ssr), built as
//! the godot_ssr package's library. The lab opens it at run time and calls
//! `godot_ssr_negotiate` (manifest.toml's `negotiate`), which hands the lab
//! the package's handles (lab's src/plugin.rs).

mod godot;

use lab::investigation::Investigation;
use lab::plugin::InvestigationsFn;

/// The package's investigations, as the lab lists them.
fn investigations() -> &'static [&'static dyn Investigation] {
    static ALL: [&dyn Investigation; 1] = [&godot::ssr::SSR];
    &ALL
}

// The package's negotiation function (manifest.toml's `negotiate`): it hands
// the lab its handles once the lab's interface version and build match its own.
lab::negotiate!(godot_ssr_negotiate, ["investigations" => investigations as InvestigationsFn]);
