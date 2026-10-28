//! The O3DE lighting investigation (packages/o3de_lighting/src/o3de/lighting), built as the
//! o3de_lighting package's library. The lab opens it at run time and calls
//! `o3de_lighting_negotiate` (manifest.toml's `negotiate`), which hands the
//! lab the package's handles (lab's src/plugin.rs).

mod o3de;

use lab::investigation::Investigation;
use lab::plugin::InvestigationsFn;

/// The package's investigations, as the lab lists them.
fn investigations() -> &'static [&'static dyn Investigation] {
    static ALL: [&dyn Investigation; 1] = [&o3de::lighting::LIGHTING];
    &ALL
}

// The package's negotiation function (manifest.toml's `negotiate`): it hands
// the lab its handles once the lab's interface version and build match its own.
lab::negotiate!(o3de_lighting_negotiate, ["investigations" => investigations as InvestigationsFn]);
