# Building Render Lab

[Render Lab](README.md) · [简体中文](BUILD.zh-CN.md)

Paths and commands are relative to the repository root.

## Requirements

Rust 1.100 or newer and a GPU driver with Vulkan (on Windows, Direct3D 12 also works; on macOS, Metal). wgpu picks
the API; `WGPU_BACKEND` (`vulkan`, `dx12`, `metal`) chooses one. The shaders are WGSL, so no shader SDK is needed.

## Build from a checkout

A normal build fetches the locked dependencies and builds the binary, shared lab, and public package libraries:

```bash
cargo build --release --locked
cargo run --release
```

## Offline builds and local caches

Run [`setup.sh`](setup.sh) once with network access to put the locked dependencies in `vendor/` and write
`.cache/cargo/config.toml`. Builds using `CARGO_HOME=$PWD/.cache/cargo`, including the video tools, then read
`vendor/` offline. [`.cargo/config.toml`](.cargo/config.toml) keeps build output in `target/`.

```bash
bash setup.sh
export CARGO_HOME="$PWD/.cache/cargo"
cargo build --release --locked --offline
```

If an optional module adds local packages with their own workspaces, build the shared lab first, then build each
package by replacing `NAME` below:

```bash
cargo build --release --locked --offline --manifest-path packages/NAME/Cargo.toml
```

## Verification

With the local cache configured, run the unit tests and offscreen checks:

```bash
export CARGO_HOME="$PWD/.cache/cargo"
cargo test --release --locked --offline --workspace
cargo run --release --locked --offline -- home --autotest /tmp/render-lab-home.json
cargo run --release --locked --offline -- --investigation o3de/lighting selftest --report /tmp/render-lab-o3de.json
```

The offscreen checks need such a driver. See [Using Render Lab](docs/usage.md#commands) for more commands and snapshots.
The [video builder](tools/video/README.md) covers its environment and checks; [experiment reports](tools/experiments/README.md)
cover the reporting tools.

## Runtime assets and sound

- **Fonts:** DejaVu Sans and DejaVu Sans Mono, with Source Han Sans CN for Chinese text, are bundled in
  [`packages/render_lab/assets/fonts`](packages/render_lab/assets/fonts) with their licenses.
- **Environment:** the sky that lights the study room and shows behind it is Poly Haven's "Tree Lined Driveway"
  HDRI (CC0), in [`packages/render_lab/assets/environments`](packages/render_lab/assets/environments); the resource pipeline clips its sun and scales it
  ([`packages/render_lab/resources.toml`](packages/render_lab/resources.toml)), and each investigation sets its level.
- **Resources:** the files the lab loads (the environment, the teapot, the fonts, MiTeX's scope) are declared by the
  [`render_lab`](packages/render_lab/manifest.toml) package and processed at run time, the first time they are needed
  (see `lab/src/packages.rs` and `lab/src/pipeline/mod.rs` for the formats). An investigation declares its own in its
  package, names it with `Investigation::package`, and reads them through `lab::resources`.
- **Sound:** the homepage plays a music loop while it is shown, and a click for its controls (neither in
  demonstrations nor videos), through FFplay (the video environment's, `FFPLAY`, or PATH); without FFplay the lab is
  silent. The music and the narration play at the volumes Config sets ([`lab/src/volume.rs`](lab/src/volume.rs)). The sounds in [`packages/render_lab/assets/audio`](packages/render_lab/assets/audio) are licensed from soundza, not under the MIT License: they are
  for this lab only (see [`THIRD_PARTY.md`](THIRD_PARTY.md)).
