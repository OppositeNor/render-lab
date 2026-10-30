# Render Lab

English | [简体中文](README.zh-CN.md)

<p align="center">
  <img src="media/o3de-attenuation-radius.jpg" alt="O3DE's simple point light: its irradiance across the study room as a heat map, fading to zero at the attenuation radius, beside the falloff formula and its values" width="100%">
</p>

A lab for rendering demonstrations and experiments, organised as **investigations of game-engine features**. Each
investigation rebuilds parts of one engine feature on prepared data and shows the data involved, step by step, in an
interactive viewer and in narrated videos that the lab records frame by frame.

It uses **wgpu 30.0.1** on Vulkan, **winit 0.30.13**, **egui 0.37**, and **WGSL** shaders.

<p align="center">
  <img src="media/title-screen.jpg" alt="The title screen: Play, Config, and Quit on frosted glass over the study room" width="49%">
  <img src="media/o3de-point-light.jpg" alt="O3DE's forward pass at a point: the study room under one simple point light, with labelled objects, the lighting terms, and the sliders" width="49%">
</p>
<p align="center">
  <img src="media/o3de-tiled-light-culling.jpg" alt="O3DE's tiled light culling: the number of lights in each screen tile, with the culling formulas and the inspected tile's depth bins" width="49%">
  <img src="media/o3de-ggx-specular.jpg" alt="O3DE's GGX specular: a metallic sphere with its N, L, V, and H vectors and specular lobe, beside the D, G, and F formulas and their values" width="49%">
</p>

## What this repository contains

The shared lab (the homepage, the settings, and the code an investigation builds on) and the investigations of engines
whose license allows publishing code derived from them:

- **Open 3D Engine: Lighting model** ([`packages/o3de_lighting/src/o3de/lighting`](packages/o3de_lighting/src/o3de/lighting/README.md)). How O3DE 2.8's Atom
  renderer lights a StandardPBR surface, with its shader code ported to WGSL: the metallic workflow, GGX specular and
  its D, G, and F, the diffuse response, the BRDF texture and multiscatter compensation, the simple point light, and the
  global skylight's image-based lighting.
- **Godot Engine: SDFGI** ([`packages/godot_sdfgi/src/godot/sdfgi`](packages/godot_sdfgi/src/godot/sdfgi/README.md)). How Godot
  4.7's Forward+ renderer computes signed distance field global illumination, followed on the CPU at the engine's sizes:
  the cascades and their scrolling, voxelization, the jump-flood distance field, occlusion, light injection and bounce,
  probe tracing and its history, the octahedral probe maps, and the screen pass, ported to WGSL.

An optional module, not provided in this repo, can add local packages of its own to `packages/` (each with its own Cargo
workspace). Without it the lab runs with the public investigations alone.

Started without a command, `render-lab` shows a loading page while the scene behind its title screen loads, then the
**title screen**, like a game's, behind frosted glass, with **Play**, **Config**, and **Quit**, a globe with the language
in the top-right corner (it opens Config at the Language option), and the copyright, license, and version in the
bottom-left corner. A photosensitivity warning covers it first, in the configured language, until **Continue**, and
again, in the new language, whenever a saved setting changes the language.

- **Play** opens the selection page: the engines and features of the installed investigations.
- **Config** edits the settings in `~/.config/render-lab/config.toml` (or `$XDG_CONFIG_HOME/render-lab/config.toml`),
  a tab per category:
  - **Interface**: the **Language** (English or 中文: the homepage's text, and the narrated edition the selection page
    offers first, English when a feature has none in it; the demonstrations follow it too) and the **Theme**, the colours
    of the homepage and the viewer (below).
  - **Render**: **Full Screen**.
  - **Audio**: the volumes of the **Music** and the **Narration**, each a slider with a mute button that keeps its
    level. The viewer's dock, and the timeline bar while a video plays, also set the narration's volume; the viewer
    keeps it in the file when it closes.

  **Save and Apply** applies the Interface and Audio settings and writes the file at once. A change to the Render
  settings is tried behind a ten-second countdown instead: **Apply** keeps it and writes the file, while **Restore**,
  Esc, or letting the time run out puts the previous one back. Nothing is written before Save and Apply. With nothing to
  save, the first button is **Return**; a tab with edits has a dot, and an option that differs from its default is
  marked, with a **Restore** for that option alone; **Discard** drops the edits, and **Restore Defaults** shows the
  defaults.
- **Themes** are TOML files, one per theme, named by their file's stem (`theme = "tokyo-night"` in the settings).
  The lab reads them at run time from `themes/` beside the binary, where the build puts the ones it offers (from
  [`themes/`](themes): **One Dark Pro**, **Tokyo Night**, and **VS Code**), and from
  `~/.config/render-lab/themes/`, whose files take the place of the binary's of the same name. A theme names itself and
  sets every colour of the interface; [`themes/one-dark-pro.toml`](themes/one-dark-pro.toml) describes each key. The
  lab's own colours are the built-in theme **Render Lab**, the default, which recordings always use, so a recorded frame never depends
  on the settings.
- **F11** saves a screenshot of the window (the homepage or the viewer) to `build/screenshots/`.

## Commands

From this directory:

```bash
cargo run --release                                   # the homepage
cargo run --release -- --list                         # the installed investigations (the values --investigation takes)
cargo run --release -- home --snapshot home.png       # the title screen rendered offscreen (--page select, config, starting, viewer, or viewer-timeline for the others)
cargo run --release -- home --autotest home.json      # click through the homepage offscreen (it uses its own settings file)
```

The lab also has `render` (record a timeline into FFmpeg), `selftest`, `layout`, and `viewer-autotest`, and every
command takes `--investigation ENGINE/FEATURE` (default: the first one listed):

```bash
cargo run --release -- --investigation o3de/lighting selftest --report o3de.json   # the port against its CPU mirror, frames, sources
```

An investigation's narrated video is built by [`tools/video`](tools/video/README.md) from its storyboard:

```bash
bash tools/video/setup.sh --gpu                                     # once: Piper, FFmpeg, Node, and Mermaid under .cache/
python3 tools/video/build.py --investigation o3de/lighting          # build/video/o3de/lighting/en/o3de-lighting.mp4
python3 tools/video/build.py --investigation o3de/lighting --lang zh-CN
```

On-screen text can hold formulas, as tags: `{@ formula="\\frac{1}{1+n}"}` in a text table, a caption, or a readout
(`Readout::formula`). The lab typesets them itself at run time (`lab/src/formula.rs`: MiTeX translates the LaTeX into
Typst math, and Typst typesets it), so no Node or committed images are needed; `render-lab formulas --check FILE…` checks
a file's formulas against the lab's limits. A storyboard uses the same tags, with shot cues timed to the narration's
words (see [`tools/video`](tools/video/README.md#text-driven-shots-tags-in-a-sentence)), and a timeline key's `formula`
opens a formula card in the viewport.

## How it fits together

| Path | Role |
|---|---|
| [`src/main.rs`](src/main.rs), [`lab/`](lab/Cargo.toml), [`lab/src/cli.rs`](lab/src/cli.rs) | The binary, which only runs the lab: a Rust dylib that the binary and every package's library link, so they share one lab (its code and its state). The command line is the lab's. |
| [`lab/src/home.rs`](lab/src/home.rs), [`lab/src/frost.rs`](lab/src/frost.rs), [`packages/rlstd/shaders/frost.wgsl`](packages/rlstd/shaders/frost.wgsl) | The homepage: the title, Config, and selection pages, over the title investigation's backdrop scene (`investigations::title`, O3DE's lighting for now; the loading page until it has loaded, `home --snapshot --page starting`), halved twice, blurred, tinted, and grained. |
| [`lab/src/config.rs`](lab/src/config.rs), [`lab/src/volume.rs`](lab/src/volume.rs) | The settings file: its location, reading it (with `toml_edit`), and writing it; a sound's volume (a level and a mute switch) and its slider, on the Config page and in the viewer's dock. |
| [`lab/src/theme.rs`](lab/src/theme.rs), [`lab/src/palette.rs`](lab/src/palette.rs), [`themes/`](themes) | Themes: finding and reading the theme files (the build copies `themes/` beside the binary), and the interface's colours in use (`palette::theme()`, `palette::text()`, ...), the lab's own until a theme is set; the overlays' colours stay constant. |
| [`packages/rlstd/src/`](packages/rlstd/src/mod.rs) | The standard package's code, built into the lab as `lab::utility`: what investigations share instead of each writing it, namely the picture inset and the images a story shows by key (`inset`), a selftest's report, still frames, and test inputs (`selftest`), a package's source index and its cited lines checked against the checkout (`sources`), its text table (`text_table!`), and a demonstration's Play animation sampled (`demo`). |
| [`lab/src/text.rs`](lab/src/text.rs), [`lab/src/readback.rs`](lab/src/readback.rs), [`lab/src/gpu.rs`](lab/src/gpu.rs) | Fonts and text, reading frames back and saving PNGs, and the GPU device. Text tables, pseudo-code listings, and source indexes are resources of each layer's package, read when first used (`i18n::Table`). |
| [`lab/src/formula.rs`](lab/src/formula.rs), [`lab/src/rich_text.rs`](lab/src/rich_text.rs), [`packages/rlstd/assets/mitex/`](packages/rlstd/assets/mitex) | Formulas: LaTeX typeset by Typst (through MiTeX's scope) into coverage masks tinted when drawn, and text with formula tags laid out on one baseline. |
| [`lab/src/scene.rs`](lab/src/scene.rs), [`packages/rlstd/scenes/`](packages/rlstd/scenes/study-room.toml), [`lab/src/terrain.rs`](lab/src/terrain.rs) | The lab's scenes, by `SceneId`: the study room, and hills of terrain under the sky. Each is a data file, the resource `scene/<key>` (its objects, shapes, materials, light, lighting constants, where a camera sees it from, and the terrain's heightfield), read when a scene is first built. Each demonstration shows one (`Investigation::demo_scene`, the study room by default); the renderer prepares a scene the first time a frame needs it and keeps it for switching back, and the viewer shows a tab per scene when an investigation's demonstrations use more than one, with the scene's demonstrations beside them. The terrain is a heightfield on a grid (`SHAPE_TERRAIN`), drawn as triangles and traced cell by cell on the GPU (`intersectTerrain`) and the CPU (`Terrain::intersect`); its heights are given samples or, opt-in, fractal Perlin noise with a fixed seed. |
| `lab/src/*.rs` (the rest) | The scene renderer, the deterministic recorder, the viewer and its dock, the timeline player and its animation (`animation.rs`: eases, paths, orbits, waves, and Play clips), the actions a story runs in a package's code (`actions.rs`, declared with `#[lab::actions]`), the GPU image kinds a package's shaders define and a story makes instances of (`views.rs`), the struct-panel records (`layout.rs`, `packages/rlstd/shaders/types.wgsl`), and the `Investigation` trait (`investigation.rs`). |
| [`lab/src/investigations.rs`](lab/src/investigations.rs), [`lab/src/plugin.rs`](lab/src/plugin.rs) | The investigations come from the packages' libraries. A package with code names its `library` (a cdylib) and its `negotiate` symbol; the loader opens the library at run time and calls that function with a C structure (the loader's interface version, its lab's build, and a map), and the function, which `lab::negotiate!` writes, checks them against the lab it was compiled with and registers the package's method handles in the map by name (`investigations`), the way the Vulkan loader negotiates with a layer. A library built against another lab is refused, not called. |
| [`tools/video/`](tools/video/README.md) | The narrated-video builder: narration, timeline, render, mux, captions, documents, and verification. |
| [`lab/build.rs`](lab/build.rs), [`lab/check/`](lab/check/src/lib.rs) | Each package's build checks its own shaders (with naga) and text tables (the lab's checks rlstd); the lab's also fingerprints the lab's build for the negotiation. They generate nothing. |
| [`lab/src/packages.rs`](lab/src/packages.rs), [`packages/`](packages) | Packages: the lab's content comes in folders of `packages/`, each with a `manifest.toml` (name, author, contact, license, the packages it `import`s, the keys it `export`s, and what it `provides`: collections of resources, and demos, a narrated timeline and its audio). The lab scans them at start-up and orders them by their imports. A package names its own resources by key and another's as `<package>.<key>`, only of a package it imports and only what that package exports. [`rlstd`](packages/rlstd/manifest.toml) holds what every investigation shares (the sky, the teapot, the scenes, fonts, the lab's WGSL and text); each investigation has its own (`Investigation::package`; [`o3de_lighting`](packages/o3de_lighting/manifest.toml)). A package is self-contained: its manifest, content, code (`src/`, `Cargo.toml`), and what its tools write (`build/`: timelines, narration) are all in its folder, and it may come with its library built, in its `lib/`. `render-lab packages` lists them, with their libraries and handles. |
| [`lab/src/pipeline/`](lab/src/pipeline/mod.rs), [`lab/src/resources.rs`](lab/src/resources.rs) | Built packages: the loader reads only `packages/` beside the binary (`target/release/packages/<name>/`: each a manifest, its resources, and its library), by full key (`rlstd.teapot`). Run from a source checkout, the lab first builds the checkout's packages there ([`lab/src/cook.rs`](lab/src/cook.rs)), incrementally: each resource is made by its kind (`file` and `directory`: linked; `radiance`: an HDRI made into the environment's levels; `mesh`: an OBJ made into a BVH) only when its source, parameters, or processor changed, so an edit shows on the next launch. A built package that did not come from the checkout is read as it is. `render-lab cook` runs the build and reports it; `render-lab resources` lists the resources. |
| [`lab/src/shaders.rs`](lab/src/shaders.rs), [`packages/rlstd/shaders.toml`](packages/rlstd/shaders.toml) | WGSL at run time: each `[[shader]]` a package declares is its `parts` in order (WGSL resources or other shaders; the lab's pass modules start with `rlstd.shader/prelude`), composed and validated by naga when the lab creates it, so an edited shader shows on the next launch and an error names its file and line. naga also lays out `wgsl/types` for the struct panel. |
| [`lab/src/values.rs`](lab/src/values.rs), [`lab/src/shader_data.rs`](lab/src/shader_data.rs), [`lab/derive/`](lab/derive/src/lib.rs) | Values by key: what the dock, timelines, and stories set, each with its type, default, range, unit, labels, name, colour, and text. A Rust struct declares them with `#[derive(lab::Values)]` (the camera's, `camera.yaw`, `camera.x`, …; an investigation's, `packages/o3de_lighting/src/o3de/lighting/values.rs`); a shader declares the members of its uniform struct with `{@ key=… default=… range=…}` tags in `///` comments, which the lab reads with naga and packs into the buffer each frame (the investigation fills the members tagged `derived`). The declarations are the schema; a shot stores every value by key in its type (`ShotState::values`: `state["key"]`, `p`, `set`, `set_value`, a vector's components by their own keys), and converts them to and from a typed struct with serde (`view`, `store`). A timeline blends them by number, and a flag or an index snaps where its type does; a timeline or a story may write a value as a number, `true`/`false`, a vector (`lightPosition=0.4,1.9,0.8`), or an option's name, which the lab resolves against the declaration when it loads. A choice is a `#[derive(lab::Choice)]` enum whose variants are its options (`viewMode=ndotl`): a shader's tag names it (`choice=ViewMode`), the dock offers its options, and the lab writes its WGSL constants into the package's git-ignored `build/generated/` whenever it loads the investigation (a `built = true` resource the shader composes; the build's shader check takes it once it exists). `render-lab choices` lists them. A derived struct also reads and writes its fields by short key (`camera.set_field("yaw", 30.0)`). `Dock::values`, `Dock::value`, and `Dock::choice` draw from the declarations; `render-lab values [--label L]` lists them. |
| [`lab/src/demo.rs`](lab/src/demo.rs) | Demonstrations as data: an investigation's package declares its parameters (a values struct, above, or `params.toml`), the order of its demonstrations, one file per demonstration (title, tab, hint, scene, listing, camera, and the parameters it changes, with shared presets), and the lab builds each demonstration's starting state from them. What a demonstration draws, its dock, and its Play animation stay the investigation's code (`packages/o3de_lighting/demos/` is the example). |
| [`lab/src/loading.rs`](lab/src/loading.rs) | The loading step: nothing a scene needs is read at start-up. When the viewer opens (or the homepage shows its backdrop), a worker thread prepares the scene's resources, its statics, and its renderer while the window shows the progress; `home --snapshot --page loading` draws that page offscreen. |

### Adding an investigation

An investigation is a package in `packages/<engine>_<feature>/`: its content, and its code, a library that implements
`Investigation` (`lab/src/investigation.rs`) and hands it to the lab when it negotiates (copy
[`packages/o3de_lighting/src/lib.rs`](packages/o3de_lighting/src/lib.rs), and add the package to the workspace in
`Cargo.toml`). Keep anything derived from an engine's source code in it, out of the shared lab,
and out of any public copy of the repository unless the engine's license allows it (then credit it in
[`THIRD_PARTY.md`](THIRD_PARTY.md)).

An investigation that shades the scene its own way returns a `SceneShading` from `Investigation::scene_shading`: WGSL
composed after the scene module, with a fragment entry point, per-frame uniforms (`FrameContent::shading`) and per-frame
data (`FrameContent::shading_frame`), static data, and compute kernels for its selftest (`Renderer::run_shading_verify`).
It may also be a **pass chain**, for an engine that renders through several targets: the fragment shader writes the
shading's `outputs` (multisampled targets), its `passes` run as compute passes over named targets at fixed bindings
(`ShadingPass`), and the last one writes `"hdr"`, which the frame shows. Declare its WGSL in the investigation's package as a
`[[shader]]` whose parts start with `rlstd.shader/scene`, and name that shader in `SceneShading::module`; build.rs validates it, and so
does the lab when it loads it. A frame without a struct-panel record can show a `Readout` (names and values) in its place.

## Building and local caches

- **Requirements:** Rust 1.88 or newer and a Vulkan driver. The shaders are WGSL, so no shader SDK is needed.
- **Crate sources:** a fresh clone builds with plain `cargo build`, which downloads the locked crates from crates.io.
  To keep every crate inside the project, run [`setup.sh`](setup.sh) once (with network access): it vendors them into
  `vendor/` and writes `.cache/cargo/config.toml`, so builds with `CARGO_HOME=$PWD/.cache/cargo` (as the video tools
  use) read `vendor/` offline. [`.cargo/config.toml`](.cargo/config.toml) keeps the build output in `target/`.
- **Fonts:** DejaVu Sans and DejaVu Sans Mono, with Source Han Sans CN for Chinese text, are bundled in
  [`packages/rlstd/assets/fonts`](packages/rlstd/assets/fonts) with their licenses.
- **Environment:** the sky that lights the study room and shows behind it is Poly Haven's "Tree Lined Driveway"
  HDRI (CC0), in [`packages/rlstd/assets/environments`](packages/rlstd/assets/environments); the resource pipeline clips its sun and scales it
  ([`packages/rlstd/resources.toml`](packages/rlstd/resources.toml)), and each investigation sets its level.
- **Resources:** the files the lab loads (the environment, the teapot, the fonts, MiTeX's scope) are declared by the
  [`rlstd`](packages/rlstd/manifest.toml) package and processed at run time, the first time they are needed
  (see `lab/src/packages.rs` and `lab/src/pipeline/mod.rs` for the formats). An investigation declares its own in its
  package, names it with `Investigation::package`, and reads them through `lab::resources`.
- **Sound:** the homepage plays a music loop while it is shown, and a click for its controls (neither in
  demonstrations nor videos), through FFplay (the video environment's, `FFPLAY`, or PATH); without FFplay the lab is
  silent. The music and the narration play at the volumes Config sets ([`lab/src/volume.rs`](lab/src/volume.rs)). The sounds in [`packages/rlstd/assets/audio`](packages/rlstd/assets/audio) are licensed from soundza, not under the MIT License: they are
  for this lab only (see [`THIRD_PARTY.md`](THIRD_PARTY.md)).

## License

Render Lab is released under the [MIT License](LICENSE), copyright © 2026 OppositeNor. Bundled fonts, the teapot mesh,
the environment, the sounds, and the vendored crates keep their own terms; see [`THIRD_PARTY.md`](THIRD_PARTY.md).
