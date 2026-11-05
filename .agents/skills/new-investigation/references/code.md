# The package and its code: layout, the trait, demonstrations, values, views

Load this when creating an investigation's package, or when adding or changing a demonstration, a value, a choice, a
view, GPU work, or the dock. The package format itself (manifest, collections, keys, negotiation, local packages) is
the `packages` skill's; the frame's passes and resources are the `render-graph` skill's; engine settings for
experiments are the `experiments` skill's. The worked example is `packages/o3de_lighting/src/o3de/lighting/`.

## The package

```
packages/<package>/Cargo.toml       a cdylib (crate-type = ["cdylib"]) that depends on the lab and on crates the lab
                                    depends on, with the lab's versions and features (copy O3DE's); build.rs runs
                                    render-lab-check on the package
                 src/lib.rs         `mod <engine>;`, investigations(), and lab::negotiate!(<package>_negotiate, …)
                 src/<engine>/mod.rs              pub static ENGINE: Engine { id, name, summary }; pub mod <feature>;
                 src/<engine>/<feature>/mod.rs    the Investigation impl, Wording, `package()` → "<package>"
                       shot.rs      Demo enum, demos() (Demos::load("<package>")), for_demo (Play animations: script/animations.lam)
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
                                    data files, scripts, and the built timelines (`built = true`), by key
                 src/…/values.rs    every value: a #[derive(lab::Values)] struct (or params.toml: name, default, doc)
                 demos/             index.toml (the order), <key>.toml per demonstration, presets/
                 script/*.lam       (optional) Lamlet the lab runs live: overlays, readouts, clicks, actions, animations
                 text/              en.ftl, zh-CN.ftl
                 pseudocode/*.py, source-index.json
                 shaders/*.wgsl     (optional own scene shading)
                 video/             the narrated video's inputs
                 build/             what tools/video writes (git-ignored): build/timelines/<lang>, build/video/<lang>
```

Everything the package is stays in its directory, so it can be packed and shared.

Register it:

- Add the package to `[workspace] members` and `default-members` in `Cargo.toml` (a local package is a workspace of its
  own instead; see AGENTS.md and the `packages` skill). Its library hands the lab its investigations when it negotiates
  (`lab/src/plugin.rs`); the homepage lists them under their engine, and `--investigation <engine>/<feature>` selects
  one on the command line. `render-lab packages` shows whether the library loaded, and its handles.
- If it brings its own shading (`SceneShading`: WGSL composed after `packages/render_lab/shaders/scene.wgsl`, a
  fragment entry point, uniforms in `FrameContent::shading`, compute kernels for the selftest), declare its file as a
  resource and a `[[shader]]` with `parts = ["render_lab.shader/scene", "<its resource>"]` in its package (copy
  `packages/o3de_lighting/resources.toml`). Declare the frame itself (its `[[node]]` list in the engine's order:
  passes and their reads and writes, the pre-pass, depth views, the scene pass, a program's passes; and the targets,
  buffers, volumes they use) in the package's `shading.toml`, a `shading` resource (format in
  `lab/src/shading_file.rs`; copy `packages/o3de_lighting/shading.toml`), with that shader's full key as its
  `module`; register the functions it names (`data`, a plan, a divisor) in `Investigation::shading_hooks`, and the
  lab's default `scene_shading` reads it. Ported shading code stays in the investigation, never in
  `packages/render_lab/shaders`.

## The trait

The trait's required methods are `engine`, `id`, `title`, `summary`, `wording`, `sources`, `listings`, `build_frame`,
`dock`, `click`, and `selftest` (`lab/src/investigation.rs`); `package` names its package, and `timelines` comes from
the package's demos. Return the demonstrations' files from `demo_specs` (and the investigation's text from `text`), and
`demos`, `demo_title`, `demo_tab`, `demo_hint`, `demo_scene` (which of the lab's scenes, `packages/render_lab/scenes/`
read by `lab/src/scene.rs`, a demonstration shows), `resets_camera`, `param_default`, and `shot_state` come from them.
Optional ones include `backdrop` (its homepage scene), `animation` (Play), `shading_hooks`, `script_natives`,
`images`, `annotations`, `actions`, `panes`, `views`, `scene`, `scene_data`, `shader_data`, `choices`, and
`pass_about`. Follow `packages/o3de_lighting/src/o3de/lighting/mod.rs`.

## Demonstrations and parameters (`shot.rs`, `demos.rs`, `controls.rs`)

- The demonstrations are data, in the investigation's package (the format is in `lab/src/demo.rs`; copy
  `packages/o3de_lighting/demos/` and `packages/o3de_lighting/src/o3de/lighting/values.rs`, and declare each file in its `resources.toml`; a
  demonstration's `scene` is one `render_lab` exports, `render_lab.scene/study-room`):
  - The values: every number a shot or the video can animate, as the fields of a `#[derive(lab::Values,
    serde::Serialize, serde::Deserialize)]` struct (`#[values(package = "…", rename_all = "camelCase")]`; each field's
    `#[value(default = …, range = a..=b, unit = m, text = "…" or name = "…", color = …, label(group), toggle, help =
    "help.<key>")]` and its `///` doc), given to `Demos::with_values::<T>(package)`. A field the video fades through
    fractions stays `f32` (`toggle` only marks it as on/off for the dock). A shot stores the values by key
    (`state.p("key")`, `state["key"]`, `state.view::<T>()` for the struct). `params.toml` (`name`, `default`, `doc`)
    still works for plain numbers. **A value's default must change nothing** (required), so existing frames stay
    byte-identical. An unknown name in a storyboard fails the build.
  - A value the shader reads goes in its uniform struct instead, declared by a tag in a `///` comment
    (`lab/src/shader_data.rs`): `/// {@ key=lightIntensity default=6 range=0,30 unit=m label=light text=controls.x}`
    on a member, `/// {@ derived}` on one the investigation computes (`Investigation::shader_data`), and
    `/// {@ shader_data}` on the `@group(1) @binding(0)` uniform. The lab adds its values to the package's parameters
    (so demonstrations, timelines, and stories set them by key), packs them each frame, and `Dock::values(ui, title,
    hint, label)` or `Dock::value(ui, key)` draws their sliders, as for the struct's values (a field's `label` names
    its group). A value that picks one of several options is a choice: a `#[derive(lab::Choice, serde::Serialize,
    serde::Deserialize)]` enum (`#[choice(package = "…", wgsl = "PREFIX")]`, each variant's `text` or `label`; see
    `brdf::ViewMode`), returned by `Investigation::choices`, named by the member's tag (`{@ key=viewMode
    choice=ViewMode default=final}`), and drawn with `Dock::choice(ui, key)`. The lab writes its WGSL constants
    (`PREFIX_<VARIANT>`) into the package's git-ignored `build/generated/<name>.wgsl` whenever it loads the
    investigation: declare that file a `built = true` resource and compose it in the shader (a fresh clone's build
    checks that shader once the lab has run). Stories then set `viewMode` to `ndotl`. `render-lab values` lists every
    key (`packages/o3de_lighting/shaders/standard_lighting.wgsl`'s `O3deShading` is the example).
  - `demos/<key>.toml`: one demonstration (its title and tab text keys, hint, scene, listing, presets, camera, the
    parameters it changes, and optionally the script `animation` its Play runs); `demos/index.toml` orders them;
    `demos/presets/` holds cameras and parameters several share. A storyboard `demo` key resets to the
    demonstration's state.
- **Every value has a help text**: `help = "help.<key>"` (`help=help.<key>` in a WGSL tag), a key of the package's text
  in both languages, saying in a sentence or two what moving it does. The dock shows a "?" after each value's name that
  shows it on hover; a value without one falls back to its English doc comment. Give every value a translatable name
  too (`text`, not `name`, unless the name is an identifier that stays English).
- **The engine's settings**: a value that is one of the engine's user-facing settings carries `engine =
  "<property>"`, with the engine's default and range cited in its doc. How to expose settings (hard-coded constants,
  settings baked into scene data, settings whose GPU work is redone) is the `experiments` skill's, "Exposing an
  engine's settings". Ship one experiment in `experiments/` that compares a cheap setting with the engine's most
  expensive one.
- **GPU work at the engine's rates** (`SceneShading`, `lab/src/investigation.rs`, declared in `shading.toml`), when the
  port needs more than a pass chain per frame:
  - `volumes`: 3D textures with mips, sampled by the chain's passes and verify kernels at group 3 (a trilinear,
    mip-linear sampler at binding 0), written by update passes one mip per dispatch.
  - `update`: compute passes that run only when their plan's key changes (`UpdatePlan`: the key, `always` for a
    measurement, the dispatches in order, each with its workgroup count, mip, and 16 words of parameters), before the
    frame's chain; the renderer keeps the volumes of the last few keys, and the performance panel times each pass while
    it runs. `Renderer::read_volume` reads a mip back for the selftest.
  - `resolution`: chain targets whose divisor a state chooses (an engine's half-resolution setting).
  - `history`: chain targets and buffers a frame reads from the frames before it (a temporal filter's).
  - `program`: an engine's own GPU work the chain cannot express (images, buffers, compute and raster passes, a CPU
    plan; `lab/src/program.rs`).
  - Every pass's `reads` and `writes` say what its entry point reads and writes: they are the frame's render graph,
    which the lab checks against the WGSL when it loads the shading. Name the engine pass each mirrors in the source
    index's `passes`. Check with `render-lab graph`; the `render-graph` skill has the rules, the programs, the
    inspector, and the graph panel.
  - `shows`: how the graph's views draw each buffer whose words are not counts (`count?word=3`, `hash`, `float`).
  - `Investigation::pass_about`: what each pass does, a `pass.<entry>` key of the package's text in both languages.
  - See `godot_voxelgi` (update.rs, the light passes into its RGBA8 texture; half-resolution GI).
- A `Demo` enum with `ALL`, `key()` (the storyboard's `demo` name), `id()` (`DemoId::new(key)`), and `of(DemoId)`, for
  what stays code: `build_frame`, the dock, picking, and Play.
- `build_frame` fills a `FrameContent` (`lab/src/frame.rs`): objects, uniforms, overlay draw lists (segments, points,
  boxes), labels, notes, and either a struct-panel record or a `Readout` (title, subtitle, rows of name, value, colour,
  and an optional formula). It must be a pure function of its arguments: no wall clock, no state from earlier frames
  (required: recordings and seeking depend on it). Solid geometry for a picture goes in the draw lists too:
  `draw.cuboid(center, half_axes, color, alpha)` is a solid, lit box that writes depth (columns, voxels, bars; lines
  behind it are dashed). `lab::utility::height_field` draws an image as columns on a plane that can cover the camera's
  view exactly (the image as the screen shows it) and tilt back as the columns rise (`Placement::facing`,
  `HeightField::column`): the SSR package's Hi-Z field is its example. The GPU's own data for such a picture comes back
  through `content.fetch` (`lab::fetch`, the `render-graph` skill). A placement with no tilt is a canvas over the view
  for 2D charts drawn with the same primitives (`Placement::rect` for bars and panels, `Placement::on` for lines and
  `content.labels`): the SSR trace's cross-section is its example.
- **Behaviour can be a script instead of code**: an overlay's shapes, readout rows, click handlers, actions, and Play
  animations may be Lamlet in `script/*.lam`, which the lab runs live, while the heavy work stays in the library and
  reaches the scripts through `Investigation::script_natives` (the `render-lab-lamlet` skill, "Scripts").
- **Views** (`lab/src/view.rs`): every picture on screen is a view of its own: the main view, the panes of a split area
  (`Investigation::panes`), the experiment's reference, the free camera's, and the views an investigation stacks over
  the main one (`Investigation::views` → `OverView { name, state, camera, alpha }`). Each draws from its own slot
  (targets, chain, history, program images), in its own scene (`Investigation::scene`, asked while `lab::view::name()`
  names the view; `render_lab.scene/canvas` is an empty one for pictures), and reads its own data (`lab::fetch` is
  scoped by `lab::view::current()`; `fetch::get_in(view, key)` reads another view's, a data link). A view's values are
  `view.<name>.<key>` in the state (`ShotState::view_state`, `own_camera`, `set_own_camera`): a story sets them with a
  `{view "<name>" {' …}}` cue, and the dock does when the view is focused. The pointer goes to the view under it (a drag
  orbits an over view's own camera; the main camera keeps its place), and a click focuses a view: the dock then shows
  and sets that view's camera and values. SSR's height field and cross-section are over views (its demos.rs).
- **Step parameters** show an algorithm's intermediate steps, so the video can animate them rather than cut from the
  input to the result: how many vectors are drawn, how many samples or rays, how many readout rows (O3DE:
  `vectorsShown`, `ggxSamples`, `readoutRows`).
- Every demonstration parameter gets a slider or toggle in `controls.rs`, built with the dock kit (`Dock::group`,
  `rows`, `slider`, `toggle`, `show`; `values` and `value` for declared values, `camera` for the camera's). Keep each
  demonstration's dock inside its width: a fourth row per group at most.
- On-screen text follows AGENTS.md (a key in both languages, `tr`/`tr_with`); the investigation's `text.rs` reads its
  own tables (`lab::text_table!("<package>.text");`; declare both files in its package as `text/en` and `text/zh-cn`),
  and build.rs checks them against the package's code and data with `Table::new("<package>.text")`. Technical terms
  stay English in both.
- Data files the investigation loads (meshes, images, tables) are resources of its package (the `packages` skill,
  "Collections and keys"), read with `lab::resources::get("<package>.<id>")`. A new kind of processing is a processor
  in `lab/src/pipeline/` (its `Kind`: parameters, code, and `process`).
- Mirror and test: GPU logic the video explains keeps a CPU mirror (O3DE: `brdf.rs` beside `standard_lighting.wgsl`),
  and the selftest compares them (required; "Precision" in SKILL.md).
