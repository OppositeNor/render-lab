# Changelog

Notable changes to Render Lab. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

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

[Unreleased]: https://github.com/OppositeNor/render-lab/compare/v0.0.3...HEAD
[0.0.3]: https://github.com/OppositeNor/render-lab/compare/v0.0.2...v0.0.3
[0.0.2]: https://github.com/OppositeNor/render-lab/compare/v0.0.1...v0.0.2
[0.0.1]: https://github.com/OppositeNor/render-lab/releases/tag/v0.0.1
