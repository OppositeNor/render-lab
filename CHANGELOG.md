# Changelog

Notable changes to Render Lab. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

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

[Unreleased]: https://github.com/OppositeNor/render-lab/compare/v0.0.2...HEAD
[0.0.2]: https://github.com/OppositeNor/render-lab/compare/v0.0.1...v0.0.2
[0.0.1]: https://github.com/OppositeNor/render-lab/releases/tag/v0.0.1
