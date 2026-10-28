//! Investigations of the Open 3D Engine (O3DE). Its source is licensed under
//! Apache-2.0 or MIT, so these investigations, and the shaders ported from it,
//! are published with the lab (THIRD_PARTY.md). The checkout they read is
//! `../../o3de` from the lab's root (or `O3DE_SOURCE_DIR`); each feature's
//! source index records its version and commit.

pub mod lighting;

use lab::investigation::Engine;

pub static ENGINE: Engine = Engine {
    id: "o3de",
    name: "Open 3D Engine",
    summary: "The Open 3D Foundation's open-source engine (Apache-2.0 or MIT), read from its source checkout. Every claim cites a file, a symbol, and a line.",
};
