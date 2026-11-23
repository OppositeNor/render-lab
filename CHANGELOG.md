# Changelog

Notable changes to Render Lab. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

## [0.0.8] - 2026-11-23

### Added

- Narration packages: each investigation's narrated videos are a package of their own,
  `packages/<name>_narration/`, with no code: its demos (`for = "<name>"`, a new demo field naming the imported
  package whose investigations it narrates) and the timelines and narration tools/video now writes into its `build/`.
  The investigations' packages no longer declare demos, timelines, or narration. The CI's release leaves the
  narration packages out of its platform archives; `tools/publish_narration.py` builds them locally, checks them
  (every file built, each language its own recording, no local path, the lab playing them beside the release's
  packages), and uploads each as `<name>_narration-v<version>.zip`, the same on every platform, with install
  instructions in the release's notes. Without one, the homepage names the archive to unpack inside the packages
  folder in the executable's directory.

### Changed

- `tools/video/build.py` builds what the viewer plays (narration, timeline, captions, documents) and stops; `--video`
  also renders, muxes, and verifies the video file. `--prepare-only` is still accepted.

## [0.0.7] - 2026-11-22

### Added

- Math scenes: a package's scripts may define `Math` hooks, which the lab draws on a canvas (manim's: 8 units
  high, +y up) instead of the 3D scene: formulas by their named parts, text, numbers, shapes, paths, and a frame
  that moves and zooms. A story's beat shows one with `kind=2d math=<name>`, a demonstration with `math = "<name>"`.
  The library `render_lab_math` builds them with manim's techniques, driven by values: fades, writing, growing,
  indicating, crossing out, transforms between matching parts; axes, plots, areas, Riemann sums, seeded samples and
  histograms, bars, braces, probability squares, tables, chance bars, and graphs with computed layouts. An
  investigation whose math scenes never read the frame (`Investigation::math_reads_frame`) pauses its 3D rendering
  under them. A new agent skill, `render-lab-math`, describes it, and `new-story` stages a core formula on it.
- Clips: a package's scripts declare named animations (runtime.lam's `Clip`): tracks of keyframes, motions, or runs
  of an `Animation` hook, composed with `after`, `sequence`, `together`, and others. A story plays one at its own
  pace (`play…`) or stretched over a range of words (`playOver…`); a demonstration names its `clips`, the first its
  Play (`animation = "…"` is refused with what to write instead). `render-lab clips` lists them, and the story check
  warns of a clip nothing plays.
- Viewer: the Animation tab shows a demonstration's clips as a dopesheet; dragging along a clip scrubs it.
- Viewer: a help card (F1 or the toolbar's "?") lists the keys and pointer gestures the current context accepts,
  read from the same table as the viewer's input (`src/bindings.rs`).
- Viewer: task layouts in the Windows menu (Explore, Experiment, Debug) beside the user's own, which "Your layout"
  brings back.
- Experiments: a "Keep changes across demonstrations" checkbox and a status of the session; an "A vs B" table of
  the values A and B set differently (a click filters the dock to one); a measurement marked stale when what it
  measured changes; a question before replacing a file or discarding unsaved work; the wipe's line dragged in the
  viewport (Shift+Left/Right nudge it); Pause A, and a note of what the timings include.
- Fields of study: a package declares a field (`[[provides]] type = "field"`) and others join it (`field =
  "<package>.<key>"`). The homepage's Play opens a folder per field (render_lab declares Computer Graphics, with a
  new icon; packages that join none go to Other), and a series is a folder of its parts.
- Frame: a general `raster` node in a package's `shading.toml` draws the scene's meshes with any vertex and
  fragment entries; `scene`, `prepass`, and `depth-views` are its shorthands, and every investigation's graph is
  unchanged.
- Video: the zh-CN edition speaks Chinese (Qwen3-TTS in one cloned narrator's voice, checked by Qwen3-ASR and timed
  by Qwen3-ForcedAligner; `setup.sh --zh`), each cue firing on the Chinese word it stands before. The voice is the
  0.6B model, six sentences at a time.
- Video: `tools/video/lint.py` measures each edition's narration for the ear (length, commas, colons, openings; 的
  for zh-CN) and reports the sentences over its limits, never failing.
- Snapshots: `--page viewer-timeline@<frame>[+<frames>]` plays a timeline from a frame offscreen; new pages for the
  help card, the Animation tab, the fields page, and the Experiments tab's sessions, staleness, overwrite, wipe
  drag, and pause.
- ReSTIR: chapters 2 to 6's worked examples on the math canvas (importance sampling, RIS's weights and W, weighted
  reservoir sampling and the merge, the alias table, combining reservoirs, rounds of spatial reuse, temporal reuse's
  count, a neighbour's W, and the paper's 1D example of bias), on an example world of 24 lights; lights drawn by
  power from an alias table, as the paper does, and the environment map as a light (`restirEnvironment`).

### Changed

- Viewer: the demonstrations are one menu in the header instead of a row of tabs, so many of them no longer push
  the header's buttons off the window.
- Viewer: the Frame graph tab fills the Controls window, laid out with new layout containers (`src/ui/layout.rs`),
  and each dock tab keeps its own scroll.
- Graph panel: its drawn buttons and grid marks take the keyboard focus (Tab, Enter, Space).
- Scripts: a hook's results are read by type; a colour is runtime.lam's `Color` (a theme colour or `{rgb
  "#rrggbb"}`), checked when the script loads; a number too large for an f32 is refused with its item and field.
- The text check finds a `.lam` file's text keys with the Lamlet parser; a computed key fails the package's build.
- Stories: a formula's `card` and `display` are Bools.
- Godot SDFGI: the CPU mirror voxelizes by rasterizing triangles in the engine's three views, as Godot does, and the
  selftest compares it with the GPU cell by cell; both editions revised after cold-listener and native reviews; each
  chapter ends again on its sources card.

### Fixed

- Graph panel: Full screen opens again (it closed in the same frame), and from the grid too.
- The dock showed a tab opened after a short one in a strip of that one's height.
- A paused scene under a math scene no longer draws an experiment's reference, split panes, the free camera's
  view, or overlaid views.

## [0.0.6] - 2026-11-13

### Added

- Investigations of papers: a source index may name a `paper` (its title, venue, DOI, and the PDF's SHA-256)
  instead of an engine checkout, its entries cited by section and page, and list the `references` a story cites in
  APA style. The sources card, callouts, the Code tab, and the homepage show a paper's sections and pages where an
  engine's show paths and lines; a 2D beat with `references=true` shows the references card; the video's
  verification checks the PDF's hash (`paperEnv` in `video.json`).
- Hardware ray tracing, optional: where the GPU has ray queries (Vulkan), the lab turns them on, builds a BLAS per
  object and a TLAS per frame, and lets a program pass bind it (`resource = "tlas"`, `access = "acceleration"`) and
  trace it with `sceneRayClear` (`render_lab.shader/rays`). A package that needs it says `requires =
  ["ray-tracing"]` in its manifest; elsewhere its investigations are listed as unavailable, with the reason.
  `RENDER_LAB_RAY_TRACING=off` turns it off.
- Papers: ReSTIR (Bitterli et al. 2020), implemented from the paper in a new public package, `paper_restir`: its
  passes in WGSL with a CPU mirror and selftest, demonstrations of RIS, reservoir sampling, spatial and temporal
  reuse, visibility reuse, and the bias and its removal (the paper's 1D example), an experiment that measures each
  method on the machine that runs it, and a narrated story in English and Chinese. The knowledge graph gains
  resampled importance sampling, weighted reservoir sampling, and reservoir reuse.
- Agent skills: the `storytelling` skill is now part of `new-story` (its `references/storytelling.md`, with its
  examples and its whole-story revision procedure beside it), since every story's narration is written with it, and
  so is the guide to teaching a formula (`references/storytelling-formula*.md`).
- Agent skills: `new-story` now holds what every story shares, with two variants: `new-story-engine` (an engine's
  feature, read from its checkout) and `new-story-paper` (a paper's method: reading its contributions, related work,
  and method; checking the authors' code against it; and its chapters, from the introduction to the experiments run
  on the viewer's machine and the references).
- C interfaces between packages' libraries (`lab/src/plugin/abi.rs`): a package's library exports C functions with
  `lab::export!` (each guarded against panics, with a signature beside it) and declares them in an `interface/`
  crate; a package that imports it names them in its `lab::negotiate!`, and the loader hands it the imported
  library, so each import is resolved and its signature checked when the importer loads. `render-lab packages` lists
  what each library resolved. The `packages` skill describes it.
- Video: `render-lab render --codec h264-nvenc` encodes with NVENC (a constant quantiser of `--crf`), and
  `tools/video/build.py --encoder` (`auto`, `nvenc`, `x264`) picks it when FFmpeg can use it.

### Changed

- Rust 1.100 or newer (`rust-version` in every crate): the build relies on Cargo 1.100's layout of `target/`
  (`build/<package>/<hash>/out`), and Cargo refuses an older toolchain with that message.
- The loader opens the packages' libraries in the packages' order, each after the libraries of the packages it
  imports; a package whose imported library is refused is refused too, with the reason down the chain, and a cycle
  of imports is reported with its path. The negotiation is interface version 2 (it hands a library its imports); a
  library of another version is refused: rebuild it.
- Godot SDFGI imports Godot VoxelGI's area-light integral and omni-light falloff through VoxelGI's C interface
  instead of keeping a copy; its selftest reports the same values.
- Helpers several packages copied are the lab's: `lab::utility::parallel::par_map`, `lab::utility::program`'s
  dispatch helpers, and `selftest::Report::write`.

### Removed

- Dead code found by an audit: the `lamlet deck` command, the terrain's heightmap source, the dock's unused record
  and struct-panel controls, and the video tools' JSON-string forms of `animate` and the highlights (typed data
  only).

### Fixed

- A source index whose commit is shorter than twelve characters no longer panics the header and the sources card.
- A package's library built before a change to render_lab's code, the lab's derive macros, or the versions of the
  lab's dependencies is now refused ("built against another build of the lab") instead of loading against them.
- Windows: with Cargo 1.100 the binary could not start, because the build put the standard library's DLL and the
  themes in the wrong folder.
- Commands that draw frames (`home`, `render`, `selftest`, `experiment`) refuse an investigation that cannot run on
  the machine, and say why.
- ReSTIR's video: each recorded frame builds on the one before it (temporal reuse, as in the live view).

## [0.0.5] - 2026-11-10

### Added

- Size limits: a Rust or Python function holds at most 100 lines and a file at most 1400. Clippy's
  `too_many_lines` is denied in every crate (`clippy.toml`), and `tools/check_sizes.py` checks files and Python
  functions, in CI.
- A package's live scripts now fail to load when a function of their own looks like a hook but has the wrong
  signature (it returns a hook's list but takes what no hook takes, or it is generic); the error names the
  parameters a hook takes. Helpers whose names start with `_` are exempt.
- The camera's sway (`swayYaw`, `swayPitch`) is available to every story.

### Changed

- Refactoring: functions over 100 lines and files over 1400 lines split into named steps and child modules across
  the lab, the public packages, and `tools/video/build.py`, without changing what a frame shows.
- Hard-to-follow parts of function bodies in the lab, the packages, and the tools now carry one-line comments, and
  `AGENTS.md` sets the rules for them.
- Godot SDFGI, Godot SSR, and O3DE lighting: every chapter has supporting animations scaled to its narration
  length, with new demonstrations, views, and clickable picks for the mechanisms the narration names (each with a
  selftest round trip), and narration revised after cold-listener and native reviews in both languages.
- Godot VoxelGI: more supporting animations in chapters 1, 5, and 8.
- O3DE lighting: the hair's longitudinal lobes are ported from `HairLightingEquations.azsli` (credited in
  `THIRD_PARTY.md`), and the lab illustrates the parts the port does not run.

### Fixed

- The performance panel's chart again starts below its headline numbers.
- SDFGI chapter 1 now says that its passes run on the GPU.

## [0.0.4] - 2026-11-08

### Added

- Testing acknowledgements and shared rules for listing names in alphabetical order.

### Changed

- Project copyright notices now credit both OppositeNor and Render Lab Contributors, including the homepage and
  documentation.
- The video builder writes `chapters.txt` as timestamp/title lines for video descriptions, using `H:MM:SS` for
  videos of at least an hour.
- The `new-story` guide now scales supporting animations with narration length, requires every named mechanism to
  be shown, and avoids narration gaps longer than about 12 seconds without supporting animation. It calls for full
  video builds only when requested or when delivering a story.

### Fixed

- A wrongly lit rim at smooth-mesh silhouettes. Back-face normal flips now follow the triangle's side instead of
  treating smooth normals that lean away from the eye as back faces.

## [0.0.3] - 2026-11-07

### Added

- A `storytelling` skill for coding agents (`.agents/skills/storytelling`): how a sentence orders what is known and
  what is new, how sentences chain, how a passage builds an idea, and a sentence map that finds and fixes what is
  hard to follow. Its references hold worked examples, the faults a revision finds most, and the procedure for
  revising a whole story's narration (reviewing at scale, editing sentences that carry cues, keeping the Chinese
  edition in step). The `new-story` skill requires it whenever narration is written or revised, and its
  cold-listener review flags a sentence that loses the thread.

### Changed

- The narration of the Godot SSR, VoxelGI, and SDFGI stories and the O3DE lighting story, revised sentence by
  sentence in both editions: overloaded sentences split, names given after what they name, bridges at topic turns,
  purposes next to their steps, and one word kept for one thing.
- SDFGI: the narration now says that the lab's port adds no sky ambient light, which is why its shadows are black
  without SDFGI; chapter 10's budget is 175 seconds.
- The Rust code is formatted with rustfmt (`rustfmt.toml`), and CI checks it (`cargo fmt --all --check`).

## [0.0.2] - 2026-11-07

### Fixed

- macOS: the release binary failed to start (`dyld: Library not loaded: …/target/release/deps/liblab.dylib`).
  macOS builds now use `-C rpath`, so the lab's library is named `@rpath/liblab.dylib` and is found beside the
  binary rather than at the build machine's path (`.cargo/config.toml`, and `RUSTFLAGS` in CI).

### Changed

- CI: the macOS release package fails if the binary or any package library links a library by a path outside
  `@rpath` and the system, and runs `render-lab --list` from the packaged folder.
- CI: release artifacts are named `package-<platform>`, and the release collects only those, not the render
  diagnostics.

## [0.0.1] - 2026-11-07

First release: Linux (x86_64), Windows (x86_64), and macOS (Apple Silicon) packages.

[Unreleased]: https://github.com/OppositeNor/render-lab/compare/v0.0.8...HEAD
[0.0.8]: https://github.com/OppositeNor/render-lab/compare/v0.0.7...v0.0.8
[0.0.7]: https://github.com/OppositeNor/render-lab/compare/v0.0.6...v0.0.7
[0.0.6]: https://github.com/OppositeNor/render-lab/compare/v0.0.5...v0.0.6
[0.0.5]: https://github.com/OppositeNor/render-lab/compare/v0.0.4...v0.0.5
[0.0.4]: https://github.com/OppositeNor/render-lab/compare/v0.0.3...v0.0.4
[0.0.3]: https://github.com/OppositeNor/render-lab/compare/v0.0.2...v0.0.3
[0.0.2]: https://github.com/OppositeNor/render-lab/compare/v0.0.1...v0.0.2
[0.0.1]: https://github.com/OppositeNor/render-lab/releases/tag/v0.0.1
