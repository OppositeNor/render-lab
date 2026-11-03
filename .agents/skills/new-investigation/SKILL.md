---
name: new-investigation
description: Start a new Render Lab investigation of a game-engine feature (a new package, packages/<engine>_<feature>/, whose library implements the Investigation trait) and animate it, both as the viewer's Play animation and as a narrated video built by tools/video from a storyboard. Use when asked to investigate a new engine or feature, add an engine to the homepage, port an engine's shader to WGSL for a demonstration, give an investigation its demonstrations, sliders, source index, pseudo-code, or formulas, or create, storyboard, and build its animation or video. Also the precision standard and procedure every investigation follows: use it when verifying or auditing an investigation's claims against the engine's source, or making one more precise.
---

# A new investigation, and its animation

An investigation is one engine feature, read from the engine's source and shown in the lab's study room: a set of
demonstrations with sliders, a readout or struct panel, Python pseudo-code of the engine's functions, a source index
that cites every claim by file, symbol, and line, and a narrated video. The lab (`lab/`) is shared and knows nothing
about any investigation; everything specific reaches it through the `Investigation` trait (`lab/src/investigation.rs`).

The worked example is the Open 3D Engine's lighting model, `packages/o3de_lighting/src/o3de/lighting/`. Copy its shape; read its files
alongside these steps.

## 0. Decide where it lives

- **The engine's license allows publishing its code** (O3DE: Apache-2.0 or MIT): a package, `packages/<engine>_<feature>/`, published.
  Code ported from the engine keeps its copyright; add a row to `THIRD_PARTY.md` and its license text to `LICENSES/`.
- **It does not**: it belongs in the optional `commercial` module, which is not provided in this repo and stays local;
  nothing published may name the engine or reflect the module's contents. Read the module's own `AGENTS.md` files first.
- A public investigation never depends on, cites, or names a commercial one.
- Ask the user where the engine's source is (or read its variable, e.g. `O3DE_SOURCE_DIR`); never write its location into
  a file, since the user may move it. Keep it read-only, and record its version and
  commit in the source index.

## 1. Precision: the standard every investigation meets

An investigation is only as good as its claims. Each of these is required, not a polish step:

- **Cited and checked.** Every claim about the engine names a file, a symbol, and a line through the source index, read
  from the checkout at the recorded commit. Never narrate from memory, from
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
- **The lab's numbers are measured.** A count or value that comes from the lab's scene ("forty-three of sixty-four
  tiles keep a light", "about fifty-six percent") is computed by the lab's code and, where it matters,
  asserted by the selftest; never estimated.
- **Say where the lab differs.** Scale, resolution, prepared or offline data, a CPU stand-in for a GPU pass, a
  schematic view: say it in the narration where it matters ("this lab's diffuse cubemap is thirty-two texels a face"), label
  schematic or scaled views on screen, and list every difference in the README's "Simplified" list.
- **Records map to the engine's structures.** Each record or readout the lab shows has an entry in the index's
  `mappings`: the engine structure it stands for, whether the relationship is exact or conceptual, the engine's exact
  layout (`exact`), and the lab's (`labLayout`).
- **Mirror, port, and test.** Shader code the video explains is ported (WGSL) and mirrored on the CPU, and the selftest
  compares them term by term, plus invariants the method promises (energy conservation, a white furnace, a normalised
  distribution, footprints that stay inside their tile). A deviation the port uncovers (an unguarded square root, a
  precision loss) is reported as a finding with its evidence, not silently fixed in the "follows the engine" path.
- **Coverage.** Every factual sentence of the narration is backed by a `callout` on that sentence or by its chapter's
  `sources`; aim for a callout on about half of all sentences. Every chapter has a reading table in the index's
  `chapters`, and every entry has `inspect` and `inspectZh` saying precisely what the line does, plus `details` for the
  other lines a claim depends on (each with its own hash, note, and noteZh).
- **Corrections are reported.** When verification contradicts a request, an earlier statement, or a common belief,
  say so to the user with the file:line evidence, and narrate the corrected version.

## 2. Procedure

For the story (the research, the outline, the storyboard, and its review, in phases with subagents), follow the
`new-story` skill; it builds on these steps.

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
   every `inspect` against the checkout, with file:line evidence, classifying each as **verified**, **correct but
   simplified** (fine only if the narration or README says it is simplified), **potentially misleading**,
   **incorrect**, or **needs investigation**. It also checks that the version and commit the story names are the
   checkout's, that no claim carries over from another version or from how similar systems usually work, and that
   the lab's own demonstrations are never presented as the engine's behaviour. Fix every claim not verified or
   stated as a simplification, in the narration, the index, the README, and the translation; an uncertain claim is
   verified, narrated as uncertain, or cut, never rewritten as fact.
9. **Build and review.** `--snapshots-only`, look at the stills (several per animated beat), then the full build in the
   background; report its checks and the corrections, and ask the user to watch it.

## 3. The package

```
packages/<package>/Cargo.toml       a cdylib (crate-type = ["cdylib"]) that depends on the lab and on crates the lab
                                    depends on, with the lab's versions and features (copy O3DE's); build.rs runs
                                    render-lab-check on the package
                 src/lib.rs         `mod <engine>;`, investigations(), and lab::negotiate!(<package>_negotiate, …)
                 src/<engine>/mod.rs              pub static ENGINE: Engine { id, name, summary }; pub mod <feature>;
                 src/<engine>/<feature>/mod.rs    the Investigation impl, Wording, `package()` → "<package>"
                       shot.rs      Demo enum, demos() (Demos::load("<package>")), for_demo, play_clip, demo_animation
                       demos.rs     build_frame: what each shot shows
                       controls.rs  the dock's sliders and toggles
                       selftest.rs  the verification suite (lab::utility::selftest: Report, still_timeline,
                                    request, Rng)
                       text.rs      lab::text_table!("<package>.text") → tr, tr_with, tr_key
                       sources.rs   pub static INDEX: PackageIndex = PackageIndex::new("<package>.source-index")
                       pseudocode.rs (NAMES; each the resource "<package>.listing/<name>")
                       formulas.rs (optional readout formulas: tags in text/*.json)
                       a script that writes source-index.json from the checkout
                       README.md
                 manifest.toml      name, author, license, import = ["render_lab"], export, library = "<package>",
                                    negotiate = "<package>_negotiate", and [[provides]]: its collections, and
                                    its demos (the video's timelines and narration)
                 resources.toml     a collection: text, source index, listings, shaders, demonstrations,
                                    data files, and the built timelines (`built = true`), by key
                 src/…/values.rs    every value: a #[derive(lab::Values)] struct (or params.toml: name, default, doc)
                 demos/             index.toml (the order), <key>.toml per demonstration, presets/
                 text/              en.json, zh-CN.json
                 pseudocode/*.py, source-index.json
                 shaders/*.wgsl     (optional own scene shading)
                 video/             the narrated video's inputs
                 build/             what tools/video writes (git-ignored): build/timelines/<lang>, build/video/<lang>
```

Everything the package is stays in its directory, so it can be packed and shared.

Register it:

- Add the package to `[workspace] members` and `default-members` in `Cargo.toml` (a local package is a workspace of
  its own instead; see AGENTS.md). Its library hands the lab its investigations when it negotiates
  (`lab/src/plugin.rs`); the homepage lists them under their engine, and `--investigation <engine>/<feature>` selects
  one on the command line. `render-lab packages` shows whether the library loaded, and its handles.
- If it brings its own shading (`SceneShading`: WGSL composed after `packages/render_lab/shaders/scene.wgsl`, a fragment entry point,
  uniforms in `FrameContent::shading`, compute kernels for the selftest), declare its file as a resource and a
  `[[shader]]` with `parts = ["render_lab.shader/scene", "<its resource>"]` in its package (copy
  `packages/o3de_lighting/resources.toml`), and name that shader's full key in `SceneShading::module`. Ported shading code stays in the
  investigation, never in `packages/render_lab/shaders`.

The trait's required methods are `engine`, `id`, `title`, `summary`, `wording`, `sources`, `listings`,
`build_frame`, `dock`, `click`, and `selftest`; `package` names its package, and `timelines` comes from the package's
demos. Return the demonstrations' files from `demo_specs` (and the
investigation's text from `text`), and `demos`, `demo_title`, `demo_tab`, `demo_hint`, `demo_scene` (which of the lab's
scenes, `packages/render_lab/scenes/*.toml` read by `lab/src/scene.rs`, a demonstration shows), `resets_camera`, `param_default`,
and `shot_state` come from them. Optional ones include `backdrop` (its homepage scene), `animation` (Play), and
`scene_shading`. Follow `packages/o3de_lighting/src/o3de/lighting/mod.rs`.

## 4. Demonstrations and parameters (`shot.rs`, `demos.rs`, `controls.rs`)

- The demonstrations are data, in the investigation's package (the format is in `lab/src/demo.rs`; copy
  `packages/o3de_lighting/demos/` and `src/o3de/lighting/values.rs`, and declare each file in its `resources.toml`; a demonstration's
  `scene` is one `render_lab` exports, `render_lab.scene/study-room`):
  - The values: every number a shot or the video can animate, as the fields of a `#[derive(lab::Values,
    serde::Serialize, serde::Deserialize)]` struct (`#[values(package = "…", rename_all = "camelCase")]`; each field's
    `#[value(default = …, range = a..=b, unit = m, text = "…" or name = "…", color = …, label(group), toggle, help = "help.<key>")]` and its
    `///` doc), given to `Demos::with_values::<T>(package)`. A field the video fades through fractions stays `f32`
    (`toggle` only marks it as on/off for the dock). A shot stores the values by key (`state.p("key")`,
    `state["key"]`, `state.view::<T>()` for the struct). `params.toml` (`name`, `default`, `doc`) still works for
    plain numbers. **A value's default must change nothing**, so existing frames stay byte-identical. An unknown name
    in a storyboard fails the build.
  - A value the shader reads goes in its uniform struct instead, declared by a tag in a `///` comment
    (`lab/src/shader_data.rs`): `/// {@ key=lightIntensity default=6 range=0,30 unit=m label=light text=controls.x}`
    on a member, `/// {@ derived}` on one the investigation computes (`Investigation::shader_data`), and
    `/// {@ shader_data}` on the `@group(1) @binding(0)` uniform. The lab adds its values to the package's parameters
    (so demonstrations, timelines, and stories set them by key), packs them each frame, and `Dock::values(ui, title,
    hint, label)` or `Dock::value(ui, key)` draws their sliders, as for the struct's values (a field's `label` names its
    group). A value that picks one of several options is a choice: a `#[derive(lab::Choice, serde::Serialize,
    serde::Deserialize)]` enum (`#[choice(package = "…", wgsl = "PREFIX")]`, each variant's `text` or `label`; see
    `brdf::ViewMode`), returned by `Investigation::choices`, named by the member's tag (`{@ key=viewMode
    choice=ViewMode default=final}`), and drawn with `Dock::choice(ui, key)`. The lab writes its WGSL constants (`PREFIX_<VARIANT>`) into the package's
    git-ignored `build/generated/<name>.wgsl` whenever it loads the investigation: declare that file a `built = true`
    resource and compose it in the shader (a fresh clone's build checks that shader once the lab has run). Stories then
    write `viewMode=ndotl`. `render-lab values` lists every key
    (`packages/o3de_lighting/shaders/standard_lighting.wgsl`'s `O3deShading` is the example).
  - `demos/<key>.toml`: one demonstration (its title and tab text keys, hint, scene, listing, presets, camera, and the
    parameters it changes); `demos/index.toml` orders them; `demos/presets/` holds cameras and parameters several share.
    A storyboard `demo` key resets to the demonstration's state.
- **Every value has a help text**: `help = "help.<key>"` (`help=help.<key>` in a WGSL tag), a key of the package's text in
  both languages, saying in a sentence or two what moving it does. The dock shows a "?" after each value's name that
  shows it on hover; a value without one falls back to its English doc comment. Give every value a translatable name
  too (`text`, not `name`, unless the name is an identifier that stays English).
- **The engine's settings, for experiments** (the `experiments` skill):
  - Every value that is one of the engine's user-facing settings gets `engine = "<property>"` (`engine=` in a WGSL
    tag). Give it the engine's default and range, cited in its doc. The Experiments tab groups these values, and
    research runs sweep them.
  - Prefer making a hard-coded setting a value (same default, so frames stay identical) to leaving it a constant.
  - Settings baked into the scene shading's data are rebuilt through `Investigation::scene_data`; work the GPU redoes
    when a setting changes runs as update passes (`SceneShading::update`).
- **GPU work at the engine's rates** (`SceneShading`, lab/src/investigation.rs), when the port needs more than a pass
  chain per frame:
  - `volumes`: 3D textures with mips, sampled by the chain's passes and verify kernels at group 3 (a trilinear,
    mip-linear sampler at binding 0), written by update passes one mip per dispatch.
  - `update`: compute passes that run only when their plan's key changes (`UpdatePlan`: the key, `always` for a
    measurement, the dispatches in order, each with its workgroup count, mip, and 16 words of parameters), before
    the frame's chain; the renderer keeps the volumes of the last few keys, and the performance panel times each pass
    while it runs. `Renderer::read_volume` reads a mip back for the selftest.
  - `resolution`: chain targets whose divisor a state chooses (an engine's half-resolution setting).
  - `history`: chain targets and buffers a frame reads from the frames before it (a temporal filter's).
  - Every pass's `reads` and `writes` say what its entry point reads and writes (buffers in both lists when both,
    volumes it samples in `reads`): they are the frame's render graph, which the lab checks against the WGSL when
    it loads the shading. Name the engine pass each mirrors in the source index's `passes`. Check with
    `render-lab graph`; the `render-graph` skill has the rules, the inspector, and the graph panel.
  - `shows`: how the graph's views draw each buffer whose words are not counts (`count?word=3`, `hash`, `float`).
  - `Investigation::pass_about`: what each pass does, a `pass.<entry>` key of the package's text in both languages.
  - See `godot_voxelgi` (update.rs, the light passes into its RGBA8 texture; half-resolution GI).
  - Ship one experiment in `experiments/` that compares a cheap setting with the engine's most expensive one.
- A `Demo` enum with `ALL`, `key()` (the storyboard's `demo` name), `id()` (`DemoId::new(key)`), and `of(DemoId)`, for
  what stays code: `build_frame`, the dock, picking, and Play.
- `build_frame` fills a `FrameContent` (`lab/src/frame.rs`): objects, uniforms, overlay draw lists (segments, points,
  boxes), labels, notes, and either a struct-panel record or a `Readout` (title, subtitle, rows of name, value, colour,
  and an optional formula). It must be a pure function of its arguments: no wall clock, no state from earlier frames.
  Solid geometry for a picture goes in the draw lists too: `draw.cuboid(center, half_axes, color, alpha)` is a solid,
  lit box that writes depth (columns, voxels, bars; lines behind it are dashed). `lab::utility::height_field` draws an
  image as columns on a plane that can cover the camera's view exactly (the image as the screen shows it) and tilt
  back as the columns rise (`Placement::facing`, `HeightField::column`): the SSR package's Hi-Z field is its example.
  The GPU's own data for such a picture comes back through `content.fetch` (`lab::fetch`, render-graph skill).
  A placement with no tilt is a canvas over the view for 2D charts drawn with the same primitives
  (`Placement::rect` for bars and panels, `Placement::on` for lines and `content.labels`): the SSR trace's
  cross-section is its example.
- **Views** (lab/src/view.rs): every picture on screen is a view of its own: the main view, the panes of a split area
  (`Investigation::panes`), the experiment's reference, the free camera's, and the views an investigation stacks over
  the main one (`Investigation::views` → `OverView { name, state, camera, alpha }`). Each draws from its own slot
  (targets, chain, history, program images), in its own scene (`Investigation::scene`, asked while `lab::view::name()`
  names the view; `render_lab.scene/canvas` is an empty one for pictures), and reads its own data (`lab::fetch` is
  scoped by `lab::view::current()`; `fetch::get_in(view, key)` reads another view's, a data link). A view's values are
  `view.<name>.<key>` in the state (`ShotState::view_state`, `own_camera`, `set_own_camera`): a story sets them with a
  `{view "<name>" {' …}}` cue, and the dock does when the view is focused. The pointer goes to the view under it (a
  drag orbits an over view's own camera; the main camera keeps its place), and a click focuses a view: the dock then
  shows and sets that view's camera and values. SSR's height field and cross-section are over views (its demos.rs).
- **Step parameters** show an algorithm's intermediate steps, so the video can animate them rather than cut from the
  input to the result: how many vectors are drawn, how many samples or rays, how many readout rows (O3DE:
  `vectorsShown`, `ggxSamples`, `readoutRows`).
- Every demonstration parameter gets a slider or toggle in `controls.rs`, built with the dock kit (`Dock::group`,
  `rows`, `slider`, `toggle`, `show`; `values` and `value` for declared values, `camera` for the camera's). Keep each demonstration's dock inside its width: a fourth row per group at most.
- On-screen text is a key in the layer's `text/en.json` and `text/zh-CN.json`, read with `tr("key")` (or `tr_with("key", args!(name = value))` for text with values, named `{name}` in both languages); the investigation's `text.rs` reads its own tables (`lab::text_table!("<package>.text");`; declare both files in its package as `text/en` and `text/zh-cn`), and build.rs checks them against the directory of the file with `Table::new("<package>.text")` and the package's data. Technical terms stay English in both.
- Data files the investigation loads (meshes, images, tables) are resources: declare them in a collection of its
  package (`[[resource]]` with `id`, `kind`, `source`, `license`, and the kind's parameters; the format and the kinds
  are in `lab/src/pipeline/mod.rs`, the manifest's in `lab/src/packages.rs`), and read them with
  `crate::lab::resources::get("<package>.<id>")`. Content that names another package's resource writes
  `<package>.<key>`: the package must be in `import` and the key in its `export`. A new kind of processing is a processor in `lab/src/pipeline/` (its `Kind`:
  parameters, code, and `process`). `render-lab resources` lists them; every launch builds them into the package beside the binary (`render-lab cook` reports it).
- Mirror and test: GPU logic the video explains keeps a CPU mirror (O3DE: `brdf.rs` beside `standard_lighting.wgsl`), and
  the selftest compares them.

## 5. Sources, pseudo-code, and formulas

- **`source-index.json`**: `engine` (name, version, commit, license) and `entries`
  (`path`, `line`, `symbol`, `lineHash`, `inspect`, `inspectZh`, `details`). `lineHash` is the first 16 hex digits of
  the SHA-256 of the cited line, trimmed. Write the index with a script that reads the checkout (O3DE:
  `source_index.py`; it reads the engine's variable, e.g. `O3DE_SOURCE_DIR`, or asks where the source is), so lines
  and hashes never drift by hand. Even for a published engine, keep quotes short.
- **Pseudo-code** (`pseudocode/*.py`): a header `# title: English | 中文`, `# source: <index key>`, `# demo: <demo>`,
  and optionally `# dispatch: English | 中文`, how its first function runs ("for each probe, in parallel", "on the CPU,
  each frame"; check it in the engine's dispatch code); a line ending in `# @tag` lights when a shot highlights `tag`
  with the code panel. A line may also end in a mark of what it does, `@@read` or `@@write` (memory: a texture, a
  buffer) or `@@math`, in the same trailer (`# @tag @@read`) or its own (`# @@read`). The code panel draws a listing as
  Scratch-like blocks (the default) or as text (the Config page's listing style, `--listing-style`): loops and branches wrap their
  bodies, each line coloured by its mark (control flow by its keyword), the dispatch on the first function's hat. It
  must compile as Python and fit the code panel at 24 px in both styles (`home --snapshot … --page
  viewer-code:<listing>`). Mirror the engine function's real steps and names; one listing per step.
- **Formulas** (optional): a formula tag in the text tables, `{@ formula="LaTeX" display=yes colors="#rrggbb,…"}`,
  with `\lit{n}{…}` marking term n and `colors` giving each term its readout row's colour (the same LaTeX in every
  language; `build.rs` checks it). Name the tag's text in `Readout::formula` and the lit term in `Readout::formula_lit`;
  the lab typesets it (`lab/src/formula.rs`; glyphs at least 24 px, at most 546 px wide, which
  `render-lab formulas --check` and the selftest enforce). A `formula` parameter can pick the lit term. Narration can
  show formulas too, with a formula cue and `say` in a sentence (`tools/video/README.md`).

## 6. The Play animation

A demonstration's Play animation is a `Clip` (`lab/src/animation.rs`) of looping motions: a `Wave` (a sine about a
centre), an `Orbit` (a point turned about a centre; a `pingPong` of `Ease::Sine` swings it back and forth), or a `Path`
through waypoints (`closed` with `Repeat::Loop` goes round). `play_clip(demo)` builds it, and
`demo_animation(demo, t) -> Vec<(param, value)>` samples it `t` seconds after Play for the trait's `animation`
(`lab::utility::demo::animation(demos(), &play_clip(demo), t)`). The
viewer adds the camera's slow sway, the sliders follow, and stopping leaves the values in them. Sweep the parameter the
demonstration is about (O3DE: the light circles the room; roughness sweeps through the cubemap's mips). `backdrop(t)`
can reuse it for the homepage's scene.

A story drives the package's code through actions (`lab/src/actions.rs`): `#[lab::actions]` on an `impl` block of the
investigation's type, one `#[action]` function each, `fn jump_flood(frame: &mut ActionFrame, seconds: f32, from:
Option<u32>)`. The parameters after the frame are its arguments, their kinds read from their types (`ActionArg`: numbers,
indices, toggles, vectors, names; `Option` for optional ones); the function sets the shot's values from
`frame.elapsed`, the seconds since the story started it, so any frame still renders on its own. Return
`&Self::ACTIONS` from `Investigation::actions`, and test one through a timeline in the selftest (Godot's
`actions.rs`). The story's cues write `{run "jumpFlood"} {runSeconds 0.5}` and `{stop "jumpFlood"}` (the vocabulary, `render-lab story keys`).

A picture the story shows is an image by key (`packages/render_lab/src/inset.rs`). Declare the ones the package computes in
`Investigation::images` (`ImageSpec { key, doc }`), and publish each in `FrameContent::images` as a `PictureInset`
(its pixels, title, axes, marker) only in the frames whose state shows it (`state.shows_image(key)`); Godot's
`field_demo` publishes its slices so. A PNG needs no code: a `file` resource of the package is an image by its key.
The story's cues write `{inset "key"} {imageCornerTopLeft}` and `{imageHide {' "key"}}`, and choose the corner and the title.
A story's arrows and labels name points the package publishes (`lab/src/frame.rs`, `FrameContent::points`) and
arrow sets it computes (`FrameContent::arrow_sets`, only while `state.shows_arrows(key)`), declared in
`Investigation::annotations` (`AnnotationSpec`): publish what an arrow would follow (a hit, a seed, a probe) rather
than drawing a fixed annotation in Rust. Godot's `field_demo` publishes `field.worked` and `field.seed`.

A click on an inset never reaches the scene behind it. What a click on an image's picture does is a click handler, a
`#[click]` function in the `#[lab::actions]` block, `fn pick_cell(frame: &mut ClickFrame) -> bool` (the state on
screen, the state it sets, and `frame.uv`, the point clicked, 0..1 with y down). The story gives an image one,
`{inset "map"} {imageClick "pickCell"}`; an inset the package draws names one in `PictureInset::click`. A click happens
while no timeline drives the state (a demonstration, or a timeline paused).

A GPU image is an image kind a shader of the package defines (`lab/src/views.rs`, Godot's
`shaders/sdfgi_views.wgsl`): a `texture_storage_2d<rgba8unorm, write>` at `@group(3) @binding(0)` tagged
`/// {@ view=probeMap entry=<compute entry> size=w,h text=<title key>}`, and its parameters' uniform at
`@group(3) @binding(1)` tagged `/// {@ view_params=probeMap}`, its members tagged `{@ key=probe default=7,7,8
range=0,16}`. The entry point runs with the frame (group 0) and the scene shading (group 1) bound, one invocation per
texel, and writes display colours. A kind that pictures the screen adds `aspect=viewport`: its texture then takes
the viewport's shape (the longer side of `size` kept), so a tall viewport gives a tall picture. No Rust: the story makes instances and drives their parameters
(`{inset "near"} {imageFrom "probeMap"} {imageProbe {' 7.0 7.0 8.0}}`). Keep `//` comments away from just above a tagged
declaration: naga then drops its `///` lines.

## 7. The narrated video

Inputs in `packages/<package>/video/` (tools/video finds the `video.json` whose `investigation` names it):

- **`video.json`**: `investigation`, the paths (storyboard, translations, diagrams, source index, pseudo-code,
  timelines), output names, `engineLabel`, `checkoutEnv`, `durationMinutes`, `readingPath` (index keys for the
  source-reading guide), `pronunciations`, and the documents' texts per language (intro, demo descriptions,
  simplifications). Copy O3DE's and adapt it.
- **`storyboard.lam`**: a Lamlet presentation (`tools/video/README.md`; the `render-lab-lamlet` skill): the video's
  heading (`# Title {#video fps=30 …}`), chapters (`## Title {#c<n> number=<n> target=<seconds> summary="…"
  sources="…"}`) of beats (`### Headline {#c<chapter>-<name> kind=3d}`; the scene type `3d`, or `2d` for a diagram
  (`diagram=`) or a sources card (`sources=`)), each paragraph one sentence. Shots are cues with the investigation's
  vocabulary (`render-lab --investigation <path> story keys`, typed functions the builder generates) and
  render_lab's typed attributes: a paragraph of cues before the first sentence sets the beat's keys, and cues in a
  sentence fire on the word they stand before: `${cue "key" {' {demoRoom} {cameraYaw 24.0} {callout "settings"}}}`
  (shot parameters, `demo`, `camera.*`, `panel`, `listing` for `code`, `{highlight {' "tag"}}`, `callout`,
  `recordCamera`, `blend`, `easing` for `ease`, `delay`), ranges `${overText {' {radiusRange 0.0 2.0}} "words"}`,
  motions `{motion {' {cameraPath {' {' 18.0 16.0 6.2} …}}}}`, and formulas `${formulaSaid "…" "…" {'}}`. Groups
  the storyboard repeats (a beat's `opening` keys) go in the package's story library, `video/story.lam`.
- **`diagrams/*.toml`**: diagrams for `diagram=` beats, boxes and edges whose text is keys of the package's text
  (`tools/video/diagrams.py` has the format; the lab draws them, `lab/src/diagram.rs`; `focus` lights boxes or rows).
  The frame's own map is the frame graph's scene (`graph=frame`, the package's `graphs/frame.toml`).
- **`storyboard.zh-CN.lam`**: the same headings (the same ids) in Chinese and one paragraph per English sentence
  (formulas as `${math "…"}`, code names as `${code "…"}`, no cues), the video's heading with `subtitle`, `locale`,
  and `glossary` (comma-separated).

Animate, don't cut:

- Drive step parameters across sentences, cues, and ranges: rays appearing, samples filling in, a light sweeping, a
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

Register the timelines as demos of its package (`[[provides]] type = "demo"`: key, title, edition, and the `built`
resources of its timeline and narration) so the homepage's Watch plays them, and let its selftest honour `options.timeline` by rendering each
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
steps, not just its midpoint. Outputs go to the package's `build/video/<lang>/` and the timelines to
its `build/timelines/<lang>/` (both ignored by git: generated files are never tracked). `tools/video/README.md` has the details.

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
- If you touched `lab/` or `packages/render_lab/shaders`, check that the other investigations' frames are byte-identical: hash a
  dozen stills of their timelines before and after.
- Never open windows or take screenshots on the user's screen; look at offscreen renders.
- Write the investigation's `README.md`: what it shows, its demonstrations and sliders, what the lab simplifies, its
  files, the video, and how to verify. Add it to the root `README.md`.
