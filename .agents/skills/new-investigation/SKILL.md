---
name: new-investigation
description: Start a new Render Lab investigation of a game-engine feature (a new src/<engine>/<feature>/ module implementing the Investigation trait) and animate it, both as the viewer's Play animation and as a narrated video built by tools/video from a storyboard. Use when asked to investigate a new engine or feature, add an engine to the homepage, port an engine's shader to WGSL for a demonstration, give an investigation its demonstrations, sliders, source index, pseudo-code, or formulas, or create, storyboard, and build its animation or video. Also the precision standard and procedure every investigation follows: use it when verifying or auditing an investigation's claims against the engine's source, or making one more precise.
---

# A new investigation, and its animation

An investigation is one engine feature, read from the engine's source and shown in the lab's study room: a set of
demonstrations with sliders, a readout or struct panel, Python pseudo-code of the engine's functions, a source index
that cites every claim by file, symbol, and line, and a narrated video. The lab (`src/lab`) is shared and knows nothing
about any investigation; everything specific reaches it through the `Investigation` trait (`src/lab/investigation.rs`).

The worked example is the Open 3D Engine's lighting model, `src/o3de/lighting/`. Copy its shape; read its files
alongside these steps.

## 0. Decide where it lives

- **The engine's license allows publishing its code** (O3DE: Apache-2.0 or MIT): `src/<engine>/<feature>/`, published.
  Code ported from the engine keeps its copyright; add a row to `THIRD_PARTY.md` and its license text to `LICENSES/`.
- **It does not**: the investigation goes under `src/commercial/` (git-ignored), listed in `crate::commercial::INVESTIGATIONS`,
  and nothing public may name the engine. Read the `AGENTS.md` files there first.
- A public investigation never depends on, cites, or names a commercial one.
- Keep the engine checkout read-only, next to the lab (O3DE: `../../o3de`, or `O3DE_SOURCE_DIR`), and record its version and
  commit in the source index.

## 1. Precision: the standard every investigation meets

An investigation is only as good as its claims. Each of these is required, not a polish step:

- **Cited and checked.** Every claim about the engine names a file, a symbol, and a line through the source index, and
  the build checks each cited line's hash against the checkout at the recorded commit. Never narrate from memory, from
  documentation, or from a function's name: read the line and the function around it.
- **What runs, not what exists.** Follow the call chain from the entry point the pipeline really executes (the pass,
  then its shader, then the functions it calls) before saying "the engine does X". Check every compile-time switch on
  the way: `#define`s, `#if` blocks, shader options and permutations, material-type property defaults, console
  variables, quality levels, and platform paths. A function that is defined but compiled out, never called by this
  pipeline, or only used by another material is narrated as such ("it is compiled out", "only the Skin material calls
  it"), never as what runs. A branch that cannot be taken under the shipped defaults (a clamp that prevents its
  condition, say) is called out, not explained as if it fires.
- **Numbers as the code has them.** Formats by name (R11G11B10 float, RGBA8, R16G16B16A16 float), sizes, sample and
  step counts, mip counts, limits ("at most four frames"), thresholds, epsilons and clamps, and the order of operations.
  When the value depends on configuration, give the default the video assumes and name what changes it ("full
  resolution by default, half when the quality setting is low").
- **The lab's numbers are measured.** A count or value that comes from the lab's scene ("four hundred and forty-eight
  of five hundred and twelve bricks", "about fifty-six percent") is computed by the lab's code and, where it matters,
  asserted by the selftest; never estimated.
- **Say where the lab differs.** Scale, resolution, prepared or offline data, a CPU stand-in for a GPU pass, a
  schematic view: say it in the narration where it matters ("this lab's atlas is sixteen by eight pages"), label
  schematic or scaled views on screen, and list every difference in the README's "Simplified" list.
- **Records map to the engine's structures.** Each record or readout the lab shows has an entry in the index's
  `mappings`: the engine structure it stands for, whether the relationship is exact or conceptual, the engine's exact
  layout (`exact`), and the lab's (`labLayout`).
- **Mirror, port, and test.** Shader code the video explains is ported (WGSL) and mirrored on the CPU, and the selftest
  compares them term by term, plus invariants the method promises (energy conservation, a white furnace, a normalised
  distribution, footprints that stay inside their page). A deviation the port uncovers (an unguarded square root, a
  precision loss) is reported as a finding with its evidence, not silently fixed in the "follows the engine" path.
- **Coverage.** Every factual sentence of the narration is backed by a `callout` on that sentence or by its chapter's
  `sources`; aim for a callout on about half of all sentences. Every chapter has a reading table in the index's
  `chapters`, and every entry has `inspect` and `inspectZh` saying precisely what the line does, plus `details` for the
  other lines a claim depends on (each with its own hash, note, and noteZh).
- **Corrections are reported.** When verification contradicts a request, an earlier statement, or a common belief,
  say so to the user with the file:line evidence, and narrate the corrected version.

## 2. Procedure

1. **Scope.** Name the feature's entry points and list its passes in frame order. Note the engine version and commit.
2. **Sweep.** For a broad question (every pass a feature touches, every caller of a function), delegate the search to an
   Explore agent: give it the checkout path and the question, and ask for file, symbol, and line triples. Verify each
   triple yourself before using it.
3. **Index.** Write the entries with a script that reads the checkout and fills the hashes (the investigation's
   `source_index.py`), so lines and hashes never drift by hand.
4. **Port, mirror, test.** Port the shader code, write the CPU mirror, and add the selftest comparisons and invariants
   before building demonstrations on them.
5. **Listings and formulas.** One pseudo-code listing per shading step, mirroring the engine function's names and
   steps; formulas for the readout where a demonstration evaluates one.
6. **Demonstrations.** Step parameters for every intermediate step the video will show.
7. **Storyboard.** Draft each factual sentence from an index entry and attach its callout as you write it. Use spoken
   numbers, and say where the lab differs.
8. **Audit.** Before the full build, have a fresh agent (not anchored on your drafts) check every factual sentence and
   every `inspect` against the checkout, classifying each as exact, imprecise, wrong, or unverifiable with file:line
   evidence. Fix every imprecise or wrong one, in the narration, the index, the README, and the translation.
9. **Build and review.** `--snapshots-only`, look at the stills (several per animated beat), then the full build in the
   background; report its checks and the corrections, and ask the user to watch it.

## 3. The module

```
src/<engine>/mod.rs                 pub static ENGINE: Engine { id, name, summary }; pub mod <feature>;
src/<engine>/<feature>/mod.rs       the Investigation impl, Wording, TIMELINES
                       shot.rs      Demo enum, PARAMS, for_demo, play_clip, demo_animation
                       demos.rs     build_frame: what each shot shows
                       controls.rs  the dock's sliders and toggles
                       selftest.rs  the verification suite
                       sources.rs   include_str!("source-index.json") → SourceIndex
                       pseudocode.rs + pseudocode/*.py
                       source-index.json (+ a script that writes it from the checkout)
                       shaders/*.wgsl (optional own scene shading)
                       formulas/ + formulas.rs (optional readout formulas)
                       video/, timelines/ (the narrated video)
                       README.md
```

Register it:

- `src/main.rs` declares `mod <engine>;`, and `src/investigations.rs` adds it to `PUBLIC` (raise the array's length);
  the homepage lists it under its engine, and `--investigation <engine>/<feature>` selects it on the command line.
- If it brings its own shading (`SceneShading`: WGSL composed after `shaders/wgsl/scene.wgsl`, a fragment entry point,
  uniforms in `FrameContent::shading`, compute kernels for the selftest), list the file in `build.rs`'s
  `SCENE_SHADINGS`. Ported shading code stays in the investigation, never in `shaders/wgsl`.

The trait's required methods are `engine`, `id`, `title`, `summary`, `wording`, `timelines`, `sources`, `listings`,
`demos`, `demo_title`, `demo_tab`, `param_default`, `shot_state`, `build_frame`, `dock`, `click`, and `selftest`.
Optional ones include `backdrop` (its homepage scene), `animation` (Play), `scene_shading`, `formula`, `demo_hint`, and
`resets_camera`. Follow `src/o3de/lighting/mod.rs`.

## 4. Demonstrations and parameters (`shot.rs`, `demos.rs`, `controls.rs`)

- A `Demo` enum with `ALL`, `key()` (the storyboard's `demo` name), `title()`, `tab()`, `id()` (`DemoId::new(key)`), and
  `of(DemoId)`.
- `PARAMS: &[(name, default, description)]`: every number a shot or the video can animate. **A parameter's default
  must change nothing**, so existing frames stay byte-identical. An unknown name in a storyboard fails the build.
- `for_demo(demo) -> ShotState`: the demonstration's camera, panel mode, and parameters. A storyboard `demo` key resets
  to this state.
- `build_frame` fills a `FrameContent` (`src/lab/frame.rs`): objects, uniforms, overlay draw lists (segments, points,
  boxes), labels, notes, and either a struct-panel record or a `Readout` (title, subtitle, rows of name, value, colour,
  and an optional formula). It must be a pure function of its arguments: no wall clock, no state from earlier frames.
- **Step parameters** show an algorithm's intermediate steps, so the video can animate them rather than cut from the
  input to the result: how many vectors are drawn, how many samples or rays, how many readout rows (O3DE:
  `vectorsShown`, `ggxSamples`, `readoutRows`).
- Every demonstration parameter gets a slider or toggle in `controls.rs`, built with the dock kit (`Dock::group`,
  `rows`, `slider`, `toggle`, `show`). Keep each demonstration's dock inside its width: a fourth row per group at most.
- On-screen text is a key in the layer's `text/en.json` and `text/zh-CN.json`, read with `tr("key")` (or `tr_with("key", args!(name = value))` for text with values, named `{name}` in both languages); the investigation's `text.rs` reads its own tables (copy `src/o3de/lighting/text.rs`), and build.rs's `TEXT_TABLES` lists them. Technical terms stay English in both.
- Mirror and test: GPU logic the video explains keeps a CPU mirror (O3DE: `brdf.rs` beside `standard_lighting.wgsl`), and
  the selftest compares them.

## 5. Sources, pseudo-code, and formulas

- **`source-index.json`**: `engine` (name, version, commit, `checkoutFromLabRoot`, license) and `entries`
  (`path`, `line`, `symbol`, `lineHash`, `inspect`, `inspectZh`, `details`). `lineHash` is the first 16 hex digits of
  the SHA-256 of the cited line, trimmed. Write the index with a script that reads the checkout (O3DE:
  `source_index.py`), so lines and hashes never drift by hand. Even for a published engine, keep quotes short.
- **Pseudo-code** (`pseudocode/*.py`): a header `# title: English | 中文`, `# source: <index key>`, `# demo: <demo>`;
  a line ending in `# @tag` lights when a shot highlights `tag` with the code panel. It must compile as Python and fit
  the code panel at 24 px. Mirror the engine function's real steps and names; one listing per step.
- **Formulas** (optional): LaTeX in `formulas/formulas.json`, `{key: latex}`, with `\lit{n}{#rrggbb}{...}` marking term n
  in its readout row's colour. `python3 tools/video/formulas.py --investigation <engine>/<feature>` renders the PNGs
  (KaTeX; glyphs at least 24 px, at most 546 px wide), commit them with `manifest.json`, embed them in `formulas.rs`
  (`include_bytes!`), return them from `Investigation::formula`, and name one in `Readout::formula`. Add the directory
  to `build.rs`'s `FORMULAS`, which fails the build on a stale image. A `formula` parameter can pick the lit term.

## 6. The Play animation

A demonstration's Play animation is a `Clip` (`src/lab/animation.rs`) of looping motions: a `Wave` (a sine about a
centre), an `Orbit` (a point turned about a centre; a `pingPong` of `Ease::Sine` swings it back and forth), or a `Path`
through waypoints (`closed` with `Repeat::Loop` goes round). `play_clip(demo)` builds it, and
`demo_animation(demo, t) -> Vec<(param, value)>` samples it `t` seconds after Play for the trait's `animation`. The
viewer adds the camera's slow sway, the sliders follow, and stopping leaves the values in them. Sweep the parameter the
demonstration is about (O3DE: the light circles the room; roughness sweeps through the cubemap's mips). `backdrop(t)`
can reuse it for the homepage's scene.

## 7. The narrated video

Inputs in `src/<engine>/<feature>/video/`:

- **`video.json`**: `investigation`, the paths (storyboard, translations, diagrams, source index, pseudo-code,
  timelines), output names, `engineLabel`, `checkoutEnv`, `durationMinutes`, `readingPath` (index keys for the
  source-reading guide), `pronunciations`, and the documents' texts per language (intro, demo descriptions,
  simplifications). Copy O3DE's and adapt it.
- **`storyboard.json`**: `chapters` (`number`, `title`, `targetSeconds`, `summary`, `sources`) of `beats` (`id`
  `c<chapter>-<name>`, `kind`: `scene`, `mermaid`, or `sources`, `headline`, `narration`, `keys`). A sentence is a
  string or `{text, keys?, sequence?, pause?}`. Keys apply at the sentence's start; `sequence` keys are spread over it.
  A key sets `demo`, `camera`, `params`, `panel` (`values`, `code`, `hidden`, ...), `code` (a listing), `highlight`
  (listing tags or fields), `callout` (an index key), or `record`, with `delay` and `blend` in seconds and `ease` for
  the blend (`smooth`, `linear`, `in`, `out`, `inOut`, `sine`). `animate` starts motions on parameters or `camera.*`
  channels: a `path` through waypoints, an `orbit`, or a `wave`, whose `seconds` may be `"sentence"` or `"beat"`
  (`tools/video/README.md`).
- **`diagrams/*.mmd`**: Mermaid diagrams for `mermaid` beats (`focus` lights nodes); `zh-CN/diagrams/` for Chinese.
- **`zh-CN/translation.json`**: one Chinese sentence per English sentence, per beat, plus titles and a glossary.

Animate, don't cut:

- Drive step parameters across sentences or `sequence` keys: rays appearing, samples filling in, a light sweeping, a
  term lit while the narration names it.
- Reset a step parameter with an instant key (no `blend`) at the next beat of the same demonstration; a `demo` key
  resets everything.
- Keep the camera alive: float it on a `path` of `camera.yaw`, `camera.pitch`, and `camera.distance` for 3D views,
  with `cameraDrift` on top, and glide it (`camera` with a `blend`) to a front view when a step is clearer in 2D. Move
  lights on paths and orbits too, so vectors and highlights change while the narration explains them. Use several
  examples, not one.
- A `demo` key resets every channel, camera included unless the key sets `camera`; to keep a motion continuous across
  beats of one demonstration, reset only the parameters that need it.
- Write numbers as spoken words for the voice, say where the lab simplifies the engine, and back each claim with a
  `callout` or a chapter `sources` entry.
- Budget each chapter (`targetSeconds`) near 1.2 times its narration; the builder fills the rest with pauses.

Register the timelines in the investigation's `TIMELINES` (`TimelineInfo`: id, title, edition, timeline path,
narration path) so the homepage's Watch plays them, and let its selftest honour `options.timeline` by rendering each
scene beat and checking that text is at least 24 px, labels stay inside and apart, and listings and readouts fit.

Build (one GPU job at a time; the full render runs in the background):

```sh
bash tools/video/setup.sh --gpu                                               # once
python3 tools/video/build.py --investigation <engine>/<feature> --prepare-only  # narration, timeline, captions, documents
python3 tools/video/build.py --investigation <engine>/<feature> --snapshots-only  # one still per beat: look at them
python3 tools/video/build.py --investigation <engine>/<feature>                 # the full video and its verification
python3 tools/video/build.py --investigation <engine>/<feature> --lang zh-CN    # after the English edition
```

Render a few stills across a beat (`render --timeline … --snapshot A,B,C --snapshot-dir D`) to see an animation's
steps, not just its midpoint. Outputs go to `build/video/<engine>/<feature>/<lang>/` (ignored by git); the timelines
are tracked. `tools/video/README.md` has the details.

## 8. Check

From the lab's root, with `CARGO_HOME=$PWD/.cache/cargo`:

```sh
cargo build --release --locked --offline                                    # no warnings
cargo run --release -- --investigation <engine>/<feature> selftest --report <scratch>/selftest.json [--timeline …]
cargo run --release -- home --autotest <scratch>/home.json                  # the homepage, offscreen
cargo run --release -- home --snapshot <scratch>/home.png                   # look at it
```

- Build the public-only tree too: copy what `git ls-files --others --exclude-standard` lists (with `--parents`) into
  an empty directory, link `vendor/` into it, and build there without warnings.
- If you touched `src/lab` or `shaders/wgsl`, check that the other investigations' frames are byte-identical: hash a
  dozen stills of their timelines before and after.
- Never open windows or take screenshots on the user's screen; look at offscreen renders.
- Write the investigation's `README.md`: what it shows, its demonstrations and sliders, what the lab simplifies, its
  files, the video, and how to verify. Add it to the root `README.md`.
