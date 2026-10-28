# Render Lab Agent Guide

Render Lab is a Rust/wgpu/WGSL program for rendering demonstrations and experiments, organised as investigations of
game-engine features. `README.md` describes the shared lab.

**If the optional `commercial` module (`src/commercial/`, and its packages in `packages/`; not provided in this repo) is
present, read every `AGENTS.md` inside them before working on anything else.** It is never published, and nothing published may reflect what it holds.

## Rules

- **Never publish engine source, or code derived from it, unless the engine's license allows it.** Work on an engine
  whose license does not allow it belongs in the optional `commercial` module, which stays local (ignored by git). Keep
  anything derived from such an engine's source out of the shared lab (`lab/`, `packages/render_lab`); cite published
  methods (papers, talks) by their publication.
- **Nothing outside the `commercial` module reflects what is inside it.** Published files (everything `git` would
  publish) may know only that an optional module, not provided in this repo, can be built in with the `commercial` cfg.
  They must not name, describe, or mirror its contents: no engine or feature names, no code, records, shaders,
  parameters, text keys, comments, or file names that exist for it. What it needs from the lab goes through generic
  extension points (the `Investigation` trait, a package's library and negotiation, its imports and overrides) that any
  investigation could use. Its investigations are packages of their own in `packages/` (code, content, and a Cargo
  workspace of their own, so the published `Cargo.lock` never names them), ignored with a pattern in
  `.git/info/exclude`; the lab code only they use stays in `src/commercial/lab`. Local-only ignore patterns go in
  `.git/info/exclude`, not `.gitignore`.
- **Open-source engines are published.** The Open 3D Engine (Apache-2.0 or MIT) is investigated in `packages/o3de_lighting/`, which
  is public; code ported from it keeps O3DE's copyright and is credited in `THIRD_PARTY.md` (MIT). Its checkout is
  `../../o3de` (or `O3DE_SOURCE_DIR`); read it, never modify it. Public investigations do not depend on, cite, or name
  the commercial ones.
- **Layering.** The lab (`lab/`, a Rust dylib the binary and every package's library link) must not depend on an
  investigation; everything feature-specific reaches it through the `Investigation` trait (including an investigation's
  own scene shading, `SceneShading`), from the libraries the loader opens at run time. The binary (`src/main.rs`) only
  runs the lab. The lab's `commercial` cfg code only serves the optional module; the rest of the lab always builds.
- **Packages are self-contained.** Everything a package is lives in its directory: its manifest, content, code
  (`src/`, `Cargo.toml`, `build.rs`), and what its tools write (`build/`, git-ignored: timelines, narration, video
  builds). A package's library is a cdylib that depends on the lab, and on crates the lab depends on with the lab's
  versions and no more features (so every crate shares one build of the lab), and defines its negotiation with
  `lab::negotiate!` (`lab/src/plugin.rs`). It may also come built, in its `lib/`.
- **Both builds stay clean.** `cargo build --release --locked --offline` (the binary, the lab, and the public packages)
  must produce no warnings, and so must each local package's own workspace, built after it
  (`cargo build --release --locked --offline --manifest-path packages/<name>/Cargo.toml`); the public part alone (the
  files `git` would publish, plus `vendor/`) must also build without warnings.
- Make the smallest change that solves the problem; do not refactor unrelated code. Do not modify `vendor/`.
- Shaders are WGSL; do not introduce Slang or SPIR-V passthrough.
- Content is in packages: each source package a directory of `packages/` with a `manifest.toml` (name, author, license, `import`,
  `export`, its `library` and `negotiate` symbol if it has code, and what it `provides`: resource collections and demos;
  the format is in `lab/src/packages.rs`, a collection's in `lab/src/pipeline/mod.rs`). `render_lab` is the lab's own;
  an investigation names its package with `Investigation::package`. The loader reads only the built packages beside the
  binary (`target/release/packages/<name>/`: manifest, resources, library), which every launch from a checkout first
  brings up to date with their sources (`lab/src/cook.rs`, incrementally); resources are read through
  `lab::resources` by full key (`render_lab.font/dejavu-sans`), not with `include_bytes!`; a package's content
  names another package's resources as `<package>.<key>`, only of a package it imports and only keys that package exports.
  Shaders (`[[shader]]` modules, lab/src/shaders.rs), text tables, listings, source indexes, scenes
  (`packages/render_lab/scenes/`), demonstrations (a package's `params.toml` and `demos/`, lab/src/demo.rs), and
  narrated timelines (a package's demos) are such content: an edit shows on the next launch, without a rebuild. Nothing
  a scene needs is read before it plays (lab/src/loading.rs).
- On-screen text is a key in the package's `text/en.json` and `text/zh-CN.json` (loaded at run time), read with `tr("key")` (or `tr_with("key", args!(name = value))` for text with values, named `{name}` in both languages). The package's build.rs (the lab's, for render_lab) fails on a key missing from a language, an unknown key, or an unused one.
- Python: type-annotate parameters and returns, `X | None` unions, concrete generics. Rust: follow the surrounding code.
- Delegate investigation and research tasks (code surveys, source lookups, reading across files) to subagents on
  cheaper models: Sonnet, or Haiku for simple lookups. Keep design, edits, and final checks on the main model.

## Validation

From this directory (set `CARGO_HOME=$PWD/.cache/cargo`):

```sh
cargo build --release --locked --offline                    # the binary, the lab, and the public packages' libraries
cargo test --release --locked --offline --workspace         # the unit tests (the loader, its negotiation, text, formulas)
cargo run --release -- home --autotest /path/to/home.json   # the homepage's controls, offscreen (its own settings file)
cargo run --release -- home --snapshot /path/to/home.png    # the title screen as a PNG, offscreen; look at it
cargo run --release -- home --snapshot /path/to/loading.png --page loading   # the viewer's loading page, offscreen, while a scene loads
cargo run --release -- home --snapshot /path/to/starting.png --page starting # the homepage's loading page, while the title's scene loads
cargo run --release -- home --snapshot /path/to/viewer.png --page viewer     # the viewer offscreen: the dock (viewer-timeline: a timeline playing, its bar)
cargo run --release -- --investigation o3de/lighting selftest --report /path/to/o3de.json   # O3DE: the WGSL port against its CPU mirror
python3 packages/o3de_lighting/src/o3de/lighting/source_index.py   # O3DE: rewrite its package's source-index.json from the checkout
python3 tools/video/build.py --investigation o3de/lighting --snapshots-only   # the video's timeline and one still per beat; look at them
python3 tools/video/build.py --investigation o3de/lighting                    # the full video (~15 min, GPU; run it in the background)
python3 -m unittest tools/video/test_markup.py tools/video/test_story.py   # the .story format and its tags (cues and formulas)
cargo run --release -- formulas --check packages/o3de_lighting/text/en.json   # typeset a file's formula tags against the lab's limits
cargo run --release -- packages                                               # the packages: metadata, imports, exports, libraries and their handles, demos
cargo run --release -- resources                                             # the built packages' resources: file size, or not built yet
cargo run --release -- cook                                                  # build the checkout's packages beside the binary, and report it
```

The video builder is described in `tools/video/README.md`. Run one GPU job at a time. The zh-CN edition
(`--lang zh-CN`) needs the English edition built first, and `packages/o3de_lighting/video/storyboard.zh-CN.story` must keep one sentence per
English sentence.

To check the public build, copy the files `git ls-files --cached --others --exclude-standard` lists (with `cp -P --parents`, so
links stay links) into an empty directory, link `vendor/` into it, and build there.

## Skills

Task guides for coding agents are in `.agents/skills/<name>/SKILL.md` (the Agent Skills layout); `.claude/skills` holds
one link per skill to it for Claude Code. `new-investigation` (starting an investigation and animating it) and `packages`
(packages: manifests, collections, references, code and its negotiation, local packages, packing) are published.

Do not open windows or take screenshots on the user's screen; check visuals from offscreen renders.
