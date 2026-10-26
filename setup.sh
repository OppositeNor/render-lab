#!/usr/bin/env bash
# Recreate vendor/ from Cargo.lock. Needs network access once; afterwards
# `cargo run` works offline from the vendored sources (.cargo/config.toml).
# `cargo vendor` ignores the [source] replacement by default, so it downloads
# the locked crates from crates.io into the project-local CARGO_HOME.
set -euo pipefail
lab_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
cd "$lab_dir"
export CARGO_HOME="$lab_dir/.cache/cargo"
target="${1:-vendor}"
cargo --config 'net.offline=false' vendor --locked "$target" > /dev/null
printf 'Vendored %s crates into %s. Build with: cargo run --release\n' "$(ls "$target" | wc -l)" "$target"
