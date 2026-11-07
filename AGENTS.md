# Render Lab Agent Guide

Render Lab is a Rust/wgpu/WGSL program for rendering demonstrations and experiments, organised as investigations of
game-engine features. `README.md` describes the shared lab.

**If the optional `commercial` module (its local packages in `packages/`; not provided in this repo) is
present, read every `AGENTS.md` inside them before working on anything else.** It is never published, and nothing published may reflect what it holds.

## Rules

- **Never publish engine source, or code derived from it, unless the engine's license allows it.** Work on an engine
  whose license does not allow it belongs in the optional `commercial` module, which stays local (ignored by git). Keep
  anything derived from such an engine's source out of the shared lab (`lab/`, `packages/render_lab`); cite published
  methods (papers, talks) by their publication.
- **Nothing outside the `commercial` module reflects what is inside it.** Published files (everything `git` would
  publish) may know only that an optional module, not provided in this repo, can add local packages to `packages/`.
  They must not name, describe, or mirror its contents: no engine or feature names, no code, records, shaders,
  parameters, text keys, comments, or file names that exist for it. What it needs from the lab goes through generic
  extension points (the `Investigation` trait, the viewer's `Feature`s, a package's library and negotiation, its imports and overrides) that any
  investigation could use. Its investigations are packages of their own in `packages/` (code, content, and a Cargo
  workspace of their own, so the published `Cargo.lock` never names them), ignored with a pattern in
  `.git/info/exclude`; lab code only they use stays in those packages. Local-only ignore patterns go in
  `.git/info/exclude`, not `.gitignore`.
- **Open-source engines are published.** The Open 3D Engine (Apache-2.0 or MIT) is investigated in `packages/o3de_lighting/`, which
  is public; code ported from it keeps O3DE's copyright and is credited in `THIRD_PARTY.md` (MIT). Its source is
  wherever the user keeps it: ask the user (or read `O3DE_SOURCE_DIR`); read it, never modify it. Public investigations do not depend on, cite, or name
  the commercial ones.
- **Layering.** The lab (`lab/`, a Rust dylib the binary and every package's library link) must not depend on an
  investigation; everything feature-specific reaches it through the `Investigation` trait (including an investigation's
  own scene shading, `SceneShading`), from the libraries the loader opens at run time. The binary (`src/main.rs`) only
  runs the lab. The lab has no code that only one investigation uses; it builds the same with or without the optional module.
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
  Shaders (`[[shader]]` modules, lab/src/shaders.rs), scripts (Lamlet the lab runs live: a package's `script/<name>`
  overlays, readouts, click handlers, actions, and animations, lab/src/script/runtime.rs), text tables, listings, source indexes, scenes
  (`packages/render_lab/scenes/`), demonstrations (a package's `params.toml` and `demos/`, lab/src/demo.rs), and
  narrated timelines (a package's demos) are such content: an edit shows on the next launch, without a rebuild. Nothing
  a scene needs is read before it plays (lab/src/loading.rs).
- On-screen text is a key (`group-name`: Fluent ids have no dots) in the package's `text/en.ftl` and `text/zh-CN.ftl` (Project Fluent, loaded at run time), read with `tr("key")` (or `tr_with("key", args!(name = value))` for a message with variables, `{ $name }` in both languages). Formulas and code names in text are Fluent function calls, `{ FORMULA("LaTeX") }` and `{ CODE("Name") }` (lab/src/rich_text.rs). The package's build.rs (the lab's, for render_lab) fails on a key missing from a language, variables that differ between languages, an unknown key, or an unused one.
- Python: type-annotate parameters and returns, `X | None` unions, concrete generics. Rust: follow the surrounding code.
- Delegate investigation and research tasks (code surveys, source lookups, reading across files) to subagents when
  your environment has them, on a cheaper model when you can choose one (Sonnet; Haiku for simple lookups). Keep
  design, edits, and final checks with the main agent. Without subagents, do the same research yourself, one
  explicitly scoped question at a time, and write each finding (with file, symbol, and line) into the task's
  working notes before the next. Either way, verify a delegated finding before building on it.

## How deep a task goes

Match the workflow to what the change can break, not to how many files it touches. The skills' full pipelines are
for the last level; each skill says what the levels mean for its tasks (a "Choose the depth" table, or "Checks, by
change").

1. **Local change** (one cue, one source-index line, one pass's reads, one script hook, one value): find what reads
   the thing you change, make the smallest correct change, run the targeted checks the skill names, and look at the
   offscreen render of what it changes.
2. **Chapter or subsystem change** (a chapter rewritten, a demonstration added, a pass restructured, a package's
   format): also re-verify the technical claims it touches against the engine's source, check the content,
   resources, and behaviour beside it, and run the investigation's tests and visual reviews.
3. **New investigation, new story, or major redesign**: the skill's whole pipeline (research, implementation,
   review, validation).

Raise the level when a small change reaches an invariant: a narrated fact or `inspect` text (verify it against the
checkout), anything in `lab/` or `packages/render_lab` (every investigation's frames, the frame identity check
below), the public/local boundary (the public build), a storyboard's form or its reader (`tools/video/compare.py`),
or text keys (both languages). These are never skipped because a change is small.

## Validation

From this directory (set `CARGO_HOME=$PWD/.cache/cargo`):

```sh
cargo build --release --locked --offline                    # the binary, the lab, and the public packages' libraries
cargo test --release --locked --offline --workspace         # the unit tests (the loader, its negotiation, text, formulas)
cargo run --release -- home --autotest /path/to/home.json   # the homepage's controls, offscreen (its own settings file)
cargo run --release -- home --snapshot /path/to/home.png    # the title screen as a PNG, offscreen; look at it
cargo run --release -- home --snapshot /path/to/loading.png --page loading   # the viewer's loading page, offscreen, while a scene loads
cargo run --release -- home --snapshot /path/to/starting.png --page starting # the homepage's loading page, while the title's scene loads
cargo run --release -- home --snapshot /path/to/viewer.png --page viewer     # the viewer offscreen: its windows (viewer:<demo> for one demonstration; viewer-timeline: a timeline playing, its bar)
cargo run --release -- home --snapshot /path/to/diagram.png --page viewer-diagram   # a timeline paused on a 2d scene with an image: its panel, zoomed
cargo run --release -- home --snapshot /path/to/sources.png --page viewer-sources   # a timeline paused on a 2d scene's sources card
cargo run --release -- --investigation o3de/lighting selftest --report /path/to/o3de.json   # O3DE: the WGSL port against its CPU mirror
O3DE_SOURCE_DIR=/path/to/o3de python3 packages/o3de_lighting/src/o3de/lighting/source_index.py   # O3DE: rewrite its package's source-index.json from the source (asks where it is without the variable)
python3 tools/video/build.py --investigation o3de/lighting --snapshots-only   # the video's timeline and one still per beat; look at them
python3 tools/video/check.py --investigation godot/ssr --explain c1-adds      # one beat: its sentences, cues in source order, the channels each writes and takes over (frames once prepared)
python3 tools/video/build.py --investigation godot/ssr --preview c1-adds --cached-audio-only   # one beat's stills: each cue, the frame before it, motion and blend ends
python3 tools/video/build.py --investigation o3de/lighting                    # the full video (~15 min, GPU; run it in the background)
python3 tools/video/build.py --all                                            # every package's video, in all its languages, one after another (GPU; background)
python3 -m unittest tools/video/test_markup.py tools/video/test_presentation.py tools/video/test_compare.py   # cue attributes, the storyboard format (a Lamlet presentation; the lab built first), the comparison
python3 tools/video/compare.py save /tmp/before; python3 tools/video/compare.py /tmp/before /tmp/after   # every edition's prepared outputs unchanged by a change (after build.py --all --prepare-only each time)
cargo run --release -- lamlet toml packages/render_lab/scenes/study-room.lam studyRoom   # a Lamlet content file's binding as the TOML the lab reads
python3 tools/lamlet_fmt.py --check                                           # every tracked .lam file laid out (without --check: lays them out)
cargo run --release -- --investigation godot/voxelgi story keys               # the keys a storyboard's cues may set: its vocabulary, as `render-lab lamlet vocab` reads them
cargo run --release -- --investigation godot/sdfgi story schema              # the same keys with type, unit, range, default, doc; and what the vocabulary leaves out, with the `attr` that writes it
cargo run --release -- --investigation o3de/lighting script check             # its package's scripts (script/*.lam) checked, and their hooks (`script stubs`: what they import)
cargo run --release -- formulas --check packages/o3de_lighting/text/en.ftl    # typeset a file's formulas against the lab's limits
cargo run --release -- i18n --check                                           # every text table per language: missing keys, fallbacks, texts left in English, characters the fonts lack
cargo run --release -- --investigation o3de/lighting values                   # the keys the dock, timelines, and stories set (shader tags, derived structs)
cargo run --release -- --investigation o3de/lighting choices                  # the choices (enums) and their options; the lab writes their WGSL constants into build/ itself
cargo run --release -- --investigation godot/sdfgi actions                    # the actions a story runs (`{run "…"}` with `run.<arg>`, `{stop "…"}`) and their arguments
cargo run --release -- --investigation godot/sdfgi images                     # the images it publishes for a story to show (`{inset "…"}`; a PNG resource is one too), and its shaders' GPU image kinds (`imageFrom`) with their parameters
cargo run --release -- home --snapshot /path/to/perf.png --page viewer-performance   # the performance panel (GPU time per pass), its most expensive pass called out
cargo run --release -- home --snapshot /path/to/free.png --page viewer-free         # the free camera on (C): the main camera and its view cone seen from beside it, its own view in a corner
cargo run --release -- home --snapshot /path/to/code.png --page viewer-code:<listing>@<tag>   # the Code tab on a listing, its tag lit (blocks; --listing-style text for the text view)
cargo run --release -- home --snapshot /path/to/prereq.png --page viewer-prerequisites   # the Prerequisites tab, the knowledge graph in the panel (viewer-knowledge: zoomed onto a level topic; viewer-knowledge-graph: full screen; viewer-knowledge-compact: full screen, only the topics its context needs; select-knowledge: over the homepage)
cargo run --release -- --investigation godot/sdfgi knowledge                  # the knowledge graph and the audience: checked, and its split (--known, --levels)
cargo run --release -- --investigation o3de/lighting graph                    # the frame's render graph: passes in order, reads and writes (checked against the WGSL), lifetimes, engine passes (--json, --dot)
cargo run --release -- home --snapshot /path/to/graph.png --page viewer-graph   # the frame graph in the panel, the inspector on what the costliest pass writes
cargo run --release -- home --snapshot /path/to/inspect.png --page "viewer-inspect:<node>.<resource>[/<show>][?<param>=<value>&…]"   # a resource right after a node, in the inspector
cargo run --release -- home --snapshot /path/to/dag.png --page "viewer-dag[:<demo>][@<node>,…]"   # the frame graph as a DAG, each pass's result on its card (viewer-dag-full: full screen)
cargo run --release -- home --snapshot /path/to/res.png --page "viewer-resources[:<demo>][@<node>,…]"   # every resource of the frame as a card, as the frame leaves it (viewer-resources-full)
cargo run --release -- home --snapshot /path/to/src.png --page viewer-source:<node>   # the full-screen DAG, that pass selected, the WGSL it is compiled from beside it
cargo run --release -- home --snapshot /path/to/layout.png --page viewer-layout   # the viewer's windows moved, docked, and resized by the pointer; the layout written to layout.toml beside it
cargo run --release -- home --snapshot /path/to/camera.png --page viewer-camera   # the camera's modes driven through the viewer's input and checked: drag orbits, Shift-drag pans, right-held flies (WASD, Q/E on world Y, wheel speed)
cargo run --release -- home --snapshot /path/to/scene.png --page "viewer-scene[:<entity>][@move|rotate|scale]"   # the Scene window open, an entity selected (object.teapot, value.lightX), its gizmo dragged
cargo run --release -- home --snapshot /path/to/exp.png --page viewer-experiments   # the Experiment button's viewer: the Experiments tab (viewer-compare: a reference, a wipe, a measurement)
cargo run --release -- home --snapshot /path/to/quiz.png --page viewer-quiz:<n>@<letter>   # the Quiz button's viewer: question n answered with that choice (none: unanswered; past the last: the score)
cargo run --release -- --investigation godot/voxelgi experiment godot_voxelgi.experiment/cones-and-bias --out /path/to/dir   # a research experiment headless (--dry-run lists its runs; GPU)
python3 tools/experiments/report.py /path/to/dir                              # its report: tables, plots, image grid
python3 -m unittest tools/experiments/test_report.py                          # the report's tests
cargo run --release -- --list                                                 # the installed investigations: the values --investigation takes
cargo run --release -- packages                                               # the packages: metadata, imports, exports, libraries and their handles, demos, series
cargo run --release -- series                                                 # the series (investigations as the parts of one story): parts in order, and each part's audience knowing what the parts before it teach
cargo run --release -- resources                                             # the built packages' resources: file size, or not built yet
cargo run --release -- cook                                                  # build the checkout's packages beside the binary, and report it
```

The video builder is described in `tools/video/README.md`. Run one GPU job at a time. The zh-CN edition
(`--lang zh-CN`) needs the English edition built first, and `packages/o3de_lighting/video/storyboard.zh-CN.lam` must keep one sentence per
English sentence.

To check the public build, copy the files `git ls-files --cached --others --exclude-standard` lists (with `cp -P --parents`, so
links stay links) into an empty directory, link `vendor/` into it, and build there.

The frame identity check, after any change to `lab/` or `packages/render_lab` (or a restructuring that must not change
what a frame shows): build the change's parent commit in a `git worktree` as the baseline, render the same stills with
both binaries (the timelines' beat midpoints, a dozen or more per investigation), and compare hashes of their decoded
pixels, not of the PNG files. Some frames vary from run to run (a measured CPU-time readout, viewport noise):
re-render a mismatch alone with both binaries before calling it a regression.

## Skills

Task guides for coding agents are in `.agents/skills/<name>/SKILL.md` (the Agent Skills layout), with detail they load
only when a step needs it in `references/` beside them; `.claude/skills` holds one link per skill to it for Claude
Code. All are published. Load the skill that owns the task; load another only where the table says it is required.

| Task | Skill | Also required | Useful |
|---|---|---|---|
| A new investigation; porting an engine's shader; a source index, listing, formula, demonstration, value, or Play animation; verifying an investigation's claims against the engine | `new-investigation` | `packages` (a new package) | `render-graph`, `experiments` |
| Researching, outlining, writing, reviewing, or revising an investigation's narrated story | `new-story` | `new-investigation` (its precision standard), `render-lab-lamlet` (when writing cues) | `knowledge-graph` |
| A storyboard's syntax or cues, a package's live scripts (`script/*.lam`), the runtime library, `render_lab.lam`, the lab's Lamlet code | `render-lab-lamlet` | the pinned `lamlet` / `lamlet-present` skills it names | `new-story` (what a story should say and show) |
| A pass, target, buffer, volume, history, or program of a frame; the lint's refusals; the inspector, graph panel, DAG | `render-graph` | | `experiments` (`[inspect]`) |
| Topics, chapter requirements, an investigation's audience, the Prerequisites tab | `knowledge-graph` | | `new-story` (costing a story's level) |
| Comparing settings, sweeps, ablations, quality or cost measurement; making an engine setting tunable | `experiments` | | `render-graph` |
| A manifest, collection, resource, key, override, demo entry, a package's library and negotiation, local packages, packing, loader errors | `packages` | | |

Validate the skills themselves after editing them: `python3 tools/check_skills.py` (front matter, names, links, the
paths they cite, and the `.claude/skills` links).

Do not open windows or take screenshots on the user's screen; check visuals from offscreen renders.

## Code Search and Context Efficiency

Use XERJ as the preferred discovery tool when it is available and its index is current.

- Search before reading. Locate relevant files, symbols, implementations, and documentation through indexed queries instead of loading large files or directory trees into context.
- Retrieve only the relevant passages initially. Expand to surrounding code and related files when needed to understand behavior.
- Treat search results as navigation aids, not authoritative evidence. Verify important claims against the actual source code, especially execution paths, configuration, defaults, and version-specific behavior.
- Use direct file reads, `rg`, or other repository tools when exact text matching, exhaustive search, or inspection of recent unindexed changes is required.
- Never assume that an indexed result reflects the current checkout. Verify the index's freshness before relying on it.
- Do not repeatedly reindex unchanged repositories. Reuse existing indexes when possible.
- If XERJ is unavailable, unconfigured, stale, or unsuitable for a task, fall back to ordinary repository search without blocking the work.
- Optimize total task cost and correctness, not search-tool usage or token count alone.

For detailed XERJ setup and query workflows, consult the project's code-search instructions when needed.
