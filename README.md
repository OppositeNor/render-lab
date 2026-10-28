# Render Lab

A lab for rendering demonstrations and experiments, organised as **investigations of game-engine features**. Each
investigation rebuilds parts of one engine feature on prepared data and shows the data involved, step by step, in an
interactive viewer and in narrated videos that the lab records frame by frame.

It uses **wgpu 30.0.1** on Vulkan, **winit 0.30.13**, **egui 0.37**, and **WGSL** shaders.

## What this repository contains

The shared lab (the homepage, the settings, and the code an investigation builds on) and the investigations of engines
whose license allows publishing code derived from them:

- **Open 3D Engine: Lighting model** ([`src/o3de/lighting`](src/o3de/lighting/README.md)). How O3DE 2.8's Atom
  renderer lights a StandardPBR surface, with its shader code ported to WGSL: the metallic workflow, GGX specular and
  its D, G, and F, the diffuse response, the BRDF texture and multiscatter compensation, the simple point light, and the
  global skylight's image-based lighting.

An optional module, not provided in this repo, can be built in with the `commercial` cfg (`build.rs` sets it when
`src/commercial/` is present). Without it the lab builds with the public investigations alone.

Started without a command, `render-lab` opens a **title screen**, like a game's, behind frosted glass, with **Play**,
**Config**, and **Quit**, and the copyright, license, and version in the bottom-left corner.

- **Play** opens the selection page: the engines and features of the installed investigations.
- **Config** edits the settings in `~/.config/render-lab/config.toml` (or `$XDG_CONFIG_HOME/render-lab/config.toml`);
  for now there is one, **Full Screen**. **Save and Apply** tries the edited settings behind a ten-second countdown:
  **Apply** keeps them and writes the file, while **Restore**, Esc, or letting the time run out puts the previous ones
  back. Nothing is written before Apply. With nothing to save, the first button is **Return**; an option that differs
  from its default is marked, with a **Restore** for that option alone; **Discard** drops the edits, and **Restore
  Defaults** shows the defaults.
- **F11** switches full screen for the session only; it is never saved.

## Commands

From this directory:

```bash
cargo run --release                                   # the homepage
cargo run --release -- home --snapshot home.png       # the title screen rendered offscreen (--page select or config for the others)
cargo run --release -- home --autotest home.json      # click through the homepage offscreen (it uses its own settings file)
```

The lab also has `render` (record a timeline into FFmpeg), `selftest`, `layout`, and `viewer-autotest`, and every
command takes `--investigation ENGINE/FEATURE` (default: the first one listed):

```bash
cargo run --release -- --investigation o3de/lighting selftest --report o3de.json   # the port against its CPU mirror, frames, sources
```

An investigation's narrated video is built by [`tools/video`](tools/video/README.md) from its storyboard:

```bash
bash tools/video/setup.sh --gpu                                     # once: Piper, FFmpeg, Node, Mermaid, and KaTeX under .cache/
python3 tools/video/build.py --investigation o3de/lighting          # build/video/o3de/lighting/en/o3de-lighting.mp4
python3 tools/video/build.py --investigation o3de/lighting --lang zh-CN
python3 tools/video/formulas.py --investigation o3de/lighting       # after editing formulas/formulas.json: its PNGs
```

An investigation's readout can show a formula above its rows: LaTeX in `src/<engine>/<feature>/formulas/formulas.json`,
rendered by KaTeX into committed PNGs (see [`tools/video`](tools/video/README.md#formulas)), which the investigation
returns from `Investigation::formula`.

## How it fits together

| Path | Role |
|---|---|
| [`src/main.rs`](src/main.rs) | The command line. |
| [`src/lab/home.rs`](src/lab/home.rs), [`src/lab/frost.rs`](src/lab/frost.rs), [`shaders/wgsl/frost.wgsl`](shaders/wgsl/frost.wgsl) | The homepage: the title, Config, and selection pages, over an investigation's backdrop scene (or a plain field), halved twice, blurred, tinted, and grained. |
| [`src/lab/config.rs`](src/lab/config.rs) | The settings file: its location, reading it (with `toml_edit`), and writing it. |
| [`src/lab/text.rs`](src/lab/text.rs), [`src/lab/readback.rs`](src/lab/readback.rs), [`src/lab/gpu.rs`](src/lab/gpu.rs) | Fonts and text, reading frames back and saving PNGs, and the GPU device. |
| [`src/lab/scene.rs`](src/lab/scene.rs), [`src/lab/terrain.rs`](src/lab/terrain.rs) | The lab's scenes, by `SceneId`: the study room, and hills of terrain under the sky. Each demonstration shows one (`Investigation::demo_scene`, the study room by default); the renderer prepares a scene the first time a frame needs it and keeps it for switching back, and the viewer shows a tab per scene when an investigation's demonstrations use more than one, with the scene's demonstrations beside them. The terrain is a heightfield on a grid (`SHAPE_TERRAIN`), drawn as triangles and traced cell by cell on the GPU (`intersectTerrain`) and the CPU (`Terrain::intersect`); its heights are given samples or, opt-in, fractal Perlin noise with a fixed seed. |
| `src/lab/*.rs` (the rest) | The scene renderer, the deterministic recorder, the viewer and its dock, the timeline player and its animation (`animation.rs`: eases, paths, orbits, waves, and Play clips), the struct-panel records (`layout.rs`, `shaders/wgsl/types.wgsl`), and the `Investigation` trait (`investigation.rs`). Under the `commercial` cfg the lab hands the optional module its generic extension points (`Extension`, `src/lab/extension.rs`) and nothing else. |
| [`src/investigations.rs`](src/investigations.rs), [`src/o3de/`](src/o3de) | The list of investigations, and the Open 3D Engine's. |
| [`tools/video/`](tools/video/README.md) | The narrated-video builder: narration, timeline, render, mux, captions, documents, and verification. |
| [`build.rs`](build.rs) | Validates the WGSL modules (and each investigation's own scene shading) with naga and exports their struct layouts; decides whether the optional `commercial` module is built in, from the manifest that module carries; fails when an investigation's formula images no longer match their LaTeX. |

### Adding an investigation

An investigation is a module in `src/<engine>/<feature>/` that implements `Investigation` (`src/lab/investigation.rs`) and
is listed in `src/investigations.rs`. Keep anything derived from an engine's source code in it, out of the shared lab,
and out of any public copy of the repository unless the engine's license allows it (then credit it in
[`THIRD_PARTY.md`](THIRD_PARTY.md)).

An investigation that shades the scene its own way returns a `SceneShading` from `Investigation::scene_shading`: WGSL
composed after the scene module, with a fragment entry point, per-frame uniforms (`FrameContent::shading`) and per-frame
data (`FrameContent::shading_frame`), static data, and compute kernels for its selftest (`Renderer::run_shading_verify`).
It may also be a **pass chain**, for an engine that renders through several targets: the fragment shader writes the
shading's `outputs` (multisampled targets), its `passes` run as compute passes over named targets at fixed bindings
(`ShadingPass`), and the last one writes `"hdr"`, which the frame shows. List its file in `build.rs`'s `SCENE_SHADINGS`
so the build validates it. A frame without a struct-panel record can show a `Readout` (names and values) in its place.

## Building and local caches

- **Requirements:** Rust 1.88 or newer and a Vulkan driver. The shaders are WGSL, so no shader SDK is needed.
- **Crate sources:** a fresh clone builds with plain `cargo build`, which downloads the locked crates from crates.io.
  To keep every crate inside the project, run [`setup.sh`](setup.sh) once (with network access): it vendors them into
  `vendor/` and writes `.cache/cargo/config.toml`, so builds with `CARGO_HOME=$PWD/.cache/cargo` (as the video tools
  use) read `vendor/` offline. [`.cargo/config.toml`](.cargo/config.toml) keeps the build output in `target/`.
- **Fonts:** DejaVu Sans and DejaVu Sans Mono, with Source Han Sans CN for Chinese text, are bundled in
  [`assets/fonts`](assets/fonts) with their licenses.
- **Environment:** the sky that lights the study room and shows behind it is Poly Haven's "Tree Lined Driveway"
  HDRI (CC0), in [`assets/environments`](assets/environments); `src/lab/environment.rs` clips its sun and scales it,
  and each investigation sets its level.
- **Sound:** the homepage plays a music loop while it is shown, and a click for its controls (neither in
  demonstrations nor videos), through FFplay (the video environment's, `FFPLAY`, or PATH); without FFplay the lab is
  silent. The sounds in [`assets/audio`](assets/audio) are licensed from soundza, not under the MIT License: they are
  for this lab only (see [`THIRD_PARTY.md`](THIRD_PARTY.md)).

## License

Render Lab is released under the [MIT License](LICENSE), copyright © 2026 OppositeNor. Bundled fonts, the teapot mesh,
the environment, the sounds, and the vendored crates keep their own terms; see [`THIRD_PARTY.md`](THIRD_PARTY.md).
