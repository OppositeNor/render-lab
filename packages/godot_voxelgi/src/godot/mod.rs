//! Investigations of the Godot Engine. Its source is licensed under the MIT
//! License, so these investigations, and the code ported from it, are
//! published with the lab (THIRD_PARTY.md). The source they read is wherever
//! the user keeps it (`GODOT_SOURCE_DIR`, or asked); each feature's source
//! index records its version and commit.

pub mod voxelgi;

use lab::investigation::Engine;

pub static ENGINE: Engine = Engine {
    id: "godot",
    name: "Godot Engine",
    summary: "The open-source Godot Engine (MIT), read from its source checkout. Every claim cites a file, a symbol, and a line.",
};
