#!/usr/bin/env bash
# Recreate vendor/ from Cargo.lock, and point the project-local CARGO_HOME at
# it. Needs network access once; afterwards builds with
# CARGO_HOME=$PWD/.cache/cargo work offline from the vendored sources.
# `cargo vendor` ignores the [source] replacement by default, so it downloads
# the locked crates from crates.io into the project-local CARGO_HOME.
set -euo pipefail
lab_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
cd "$lab_dir"
export CARGO_HOME="$lab_dir/.cache/cargo"
target="${1:-vendor}"
mkdir -p "$target" "$CARGO_HOME"
target="$(cd -- "$target" && pwd)"
# `cargo vendor` prints the source replacement the vendored crates need: crates.io's, and one table per git
# repository (the Lamlet crates, pinned by tag in lab/Cargo.toml). Given an absolute path, it names the vendored
# directory absolutely, so builds find it from any directory.
replacement="$(cargo --config 'net.offline=false' vendor --locked "$target")"
cat > "$CARGO_HOME/config.toml" <<EOF
# Written by setup.sh: build from the vendored crates, offline.
$replacement

[net]
offline = true
EOF
printf 'Vendored %s crates into %s. Build with: CARGO_HOME=%s cargo run --release\n' "$(ls "$target" | wc -l)" "$target" "$CARGO_HOME"
