//! The ReSTIR investigation (packages/paper_restir/src/paper/restir), built as
//! the paper_restir package's library. The lab opens it at run time and calls
//! `paper_restir_negotiate` (manifest.toml's `negotiate`), which hands the lab
//! the package's handles (lab's src/plugin.rs).

mod paper;

use lab::investigation::Investigation;
use lab::plugin::InvestigationsFn;

/// The package's investigations, as the lab lists them.
fn investigations() -> &'static [&'static dyn Investigation] {
    static ALL: [&dyn Investigation; 1] = [&paper::restir::RESTIR];
    &ALL
}

// The package's negotiation function (manifest.toml's `negotiate`): it hands
// the lab its handles once the lab's interface version and build match its own.
lab::negotiate!(paper_restir_negotiate, ["investigations" => investigations as InvestigationsFn]);
