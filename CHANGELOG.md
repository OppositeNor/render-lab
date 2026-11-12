# Changelog

Notable changes to Render Lab. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- Investigations of papers: a source index may name a `paper` (its title, venue, DOI, and the PDF's SHA-256)
  instead of an engine checkout, its entries cited by section and page, and list the `references` a story cites in
  APA style. The sources card, callouts, the Code tab, and the homepage show a paper's sections and pages where an
  engine's show paths and lines; a 2D beat with `references=true` shows the references card; the video's
  verification checks the PDF's hash (`paperEnv` in `video.json`).
- Agent skills: `new-story` now holds what every story shares, with two variants: `new-story-engine` (an engine's
  feature, read from its checkout) and `new-story-paper` (a paper's method: reading its contributions, related work,
  and method; checking the authors' code against it; and its chapters, from the introduction to the experiments run
  on the viewer's machine and the references).

### Fixed

- A source index whose commit is shorter than twelve characters no longer panics the header and the sources card.

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

[Unreleased]: https://github.com/OppositeNor/render-lab/compare/v0.0.5...HEAD
[0.0.5]: https://github.com/OppositeNor/render-lab/compare/v0.0.4...v0.0.5
[0.0.4]: https://github.com/OppositeNor/render-lab/compare/v0.0.3...v0.0.4
[0.0.3]: https://github.com/OppositeNor/render-lab/compare/v0.0.2...v0.0.3
[0.0.2]: https://github.com/OppositeNor/render-lab/compare/v0.0.1...v0.0.2
[0.0.1]: https://github.com/OppositeNor/render-lab/releases/tag/v0.0.1
