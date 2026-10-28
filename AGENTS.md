# Render Lab Agent Guide

Render Lab is a Rust/wgpu/WGSL program for rendering demonstrations and experiments, organised as investigations of
game-engine features. `README.md` describes the shared lab.

**If the optional `commercial` module (`src/commercial/`, not provided in this repo) is present, read every `AGENTS.md`
inside it before working on anything else.** It is never published, and nothing published may reflect what it holds.

## Rules

- **Never publish engine source, or code derived from it, unless the engine's license allows it.** Work on an engine
  whose license does not allow it belongs in the optional `commercial` module, which stays local (ignored by git). Keep
  anything derived from such an engine's source out of the shared lab (`src/lab`, `shaders/wgsl`); cite published
  methods (papers, talks) by their publication.
- **Nothing outside the `commercial` module reflects what is inside it.** Published files (everything `git` would
  publish) may know only that an optional module, not provided in this repo, can be built in with the `commercial` cfg.
  They must not name, describe, or mirror its contents: no engine or feature names, no code, records, shaders,
  parameters, text keys, comments, or file names that exist for it. What it needs from the lab goes through generic
  extension points (the `Investigation` trait, the lab's `Extension`) that any investigation could use; its own code,
  shaders, records, and build manifest live inside it. Local-only ignore patterns go in `.git/info/exclude`, not
  `.gitignore`.
- **Open-source engines are published.** The Open 3D Engine (Apache-2.0 or MIT) is investigated in `src/o3de/`, which is
  public; code ported from it keeps O3DE's copyright and is credited in `THIRD_PARTY.md` (MIT). Its checkout is
  `../../o3de` (or `O3DE_SOURCE_DIR`); read it, never modify it. Public investigations do not depend on, cite, or name
  the commercial ones.
- **Layering.** `src/lab` must not depend on an investigation; everything feature-specific reaches it through the
  `Investigation` trait (including an investigation's own scene shading, `SceneShading`). The lab's `commercial` cfg
  code only hands the optional module its generic extension points; the rest of the lab always builds.
- **Both builds stay clean.** `cargo build --release --locked --offline` must produce no warnings with the investigation
  present, and the public part alone (the files `git` would publish, plus `vendor/`) must also build without warnings.
- Make the smallest change that solves the problem; do not refactor unrelated code. Do not modify `vendor/`.
- Shaders are WGSL; do not introduce Slang or SPIR-V passthrough.
- On-screen text is a key in the layer's `text/en.json` and `text/zh-CN.json`, read with `tr("key")` (or `tr_with("key", args!(name = value))` for text with values, named `{name}` in both languages). build.rs fails on a key missing from a language, an unknown key, or an unused one.
- Python: type-annotate parameters and returns, `X | None` unions, concrete generics. Rust: follow the surrounding code.
- Delegate investigation and research tasks (code surveys, source lookups, reading across files) to subagents on
  cheaper models: Sonnet, or Haiku for simple lookups. Keep design, edits, and final checks on the main model.

## Validation

From this directory (set `CARGO_HOME=$PWD/.cache/cargo`):

```sh
cargo build --release --locked --offline
cargo run --release -- home --autotest /path/to/home.json   # the homepage's controls, offscreen (its own settings file)
cargo run --release -- home --snapshot /path/to/home.png    # the title screen as a PNG, offscreen; look at it
cargo run --release -- --investigation o3de/lighting selftest --report /path/to/o3de.json   # O3DE: the WGSL port against its CPU mirror
python3 src/o3de/lighting/source_index.py                  # O3DE: rewrite source-index.json from the checkout
python3 tools/video/build.py --investigation o3de/lighting --snapshots-only   # the video's timeline and one still per beat; look at them
python3 tools/video/build.py --investigation o3de/lighting                    # the full video (~15 min, GPU; run it in the background)
python3 -m unittest tools/video/test_markup.py tools/video/test_story.py   # the .story format and its tags (cues and formulas)
cargo run --release -- formulas --check src/o3de/lighting/text/en.json       # typeset a file's formula tags against the lab's limits
```

The video builder is described in `tools/video/README.md`. Run one GPU job at a time. The zh-CN edition
(`--lang zh-CN`) needs the English edition built first, and `src/o3de/lighting/video/storyboard.zh-CN.story` must keep one sentence per
English sentence.

To check the public build, copy the files `git ls-files --cached --others --exclude-standard` lists (with `cp -P --parents`, so
links stay links) into an empty directory, link `vendor/` into it, and build there.

## Skills

Task guides for coding agents are in `.agents/skills/<name>/SKILL.md` (the Agent Skills layout); `.claude/skills` holds
one link per skill to it for Claude Code. `new-investigation` (starting an investigation and animating it) is published.

Do not open windows or take screenshots on the user's screen; check visuals from offscreen renders.
