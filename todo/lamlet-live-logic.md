# Real-time logic in Lamlet scripts (plan)

Today a package's real-time logic is Rust in its library:
- **Actions:** `#[action]`, started by a story's `{run "…"}`.
- **Click handlers:** `#[click]` and `Investigation::click`.
- **The Play animation:** `Investigation::animation`.
- **What `build_frame` draws over the scene:** vectors, lobes, labels, and markers.

This plan lets the app run Lamlet scripts for all of it, the way a game engine runs its scripts:
- **The script decides behaviour:** what moves where, how it eases, what a click means, and what is drawn where.
- **The package's native Rust code does the heavy work:** rendering, BRDF evaluation, ray tracing, octrees.

A script is package content, read through `lab::resources` like a shader or a text table. An edit shows on the
next launch without rebuilding the package's library, and a package can have logic without Rust code.

It builds on the in-process plan (done and removed: the lab runs Lamlet in its own process, `lab/src/script/`) and follows the runtime rules below ("Runtime inputs,
updates, and failure behavior"). It asks Lamlet for nothing specific to the lab.

Status (rechecked 2026-11-05): milestones 0, 1, 3, 6, and 7 are done; 2, 4, and 5 in part (see "Implementation
notes" and "What is left"). The lab pins
Lamlet v0.4.0 and lamlet-present v0.1.4. The host is `lab/src/script/runtime.rs`, the runtime library
`packages/render_lab/language/runtime.lam`, and the first scripts are O3DE's (`packages/o3de_lighting/script/`:
overlay, readout, picks, animations) and VoxelGI's (`packages/godot_voxelgi/script/logic.lam`). The sketches below
are kept as the design; the scripts in the packages are the checked versions.

## The split: native or script

| Native (Rust, in the package's library or the lab) | Script (Lamlet, package content) |
|---|---|
| The frame's passes, shaders, and GPU work | Which values a demo sets over time: the camera, lights, and parameters |
| CPU mirrors of engine math: `brdf.rs`, cascades, octrees, SSAO kernels | Where an overlay's arrows, lobes, points, and labels go, how long they are, and how they grow in |
| Tracing a ray into the scene; projecting a point to a view | What a hit means: which values a click sets |
| Anything that loops over pixels, texels, or thousands of samples | Choosing the samples (a sweep's directions); scaling and drawing the results |
| Selftests, readouts that need the full CPU mirror | Story actions: steps, sweeps, reveals |

**Rule:** a native works on whole lists, not single values. The script makes one call with all 90 directions of a
lobe, not 90 calls. The per-frame cost then does not grow with the number of samples.

## No state in the script

Every hook is a pure function of what the lab passes in. The script keeps nothing between calls, and the lab
holds all state, as it does today in `ShotState`. Each call receives:

- **a snapshot:** the shot's values (read through the values package below) and the call context of the natives;
- **time:** the action's `elapsed`, or the demo's time;
- **arguments:** the action's arguments, or the click.

Anything that seems to need state is derived from time. A sequence such as "wait 2 s, then move" is a piecewise
function of time, and the combinators below (`seq`, `after`, `tween`) write it. "Blend from where the value was"
evaluates the previous segment at the same time. This is what `Timeline::state_at` already does: it rebuilds the
state from frame 0 for every frame (`lab/src/shot.rs`). Seeking, scrubbing, pausing, and frame-exact recording
keep working.

This is Lamlet's own host boundary (its `docs/rationale.md`, D10, "descriptions, not effects"):
- A script returns descriptions: writes of values and shapes to draw.
- The lab validates and applies them.
- Natives only *query*: they read the snapshot they are given and change nothing. They are pure functions of
  their arguments and the call's snapshot, so determinism holds.

Persistent simulations, such as physics integrated step by step, stay native. They are outside this plan.

## Threads

Every hook this plan touches runs on the main (winit) thread:
- **In the viewer:** `state_at`, clicks, `animation`, and `build_frame` through `shader_data::build_frame`.
- **In the recorder:** it is single-threaded.

The loading worker builds the renderer and runs no action, click, or animation. Milestone 0 checks that it never
calls `build_frame` either; if it does, the overlay is skipped there, because no picture is drawn from it.

- **The engine.** One Lamlet engine, its checked scripts, and their instances live in a `thread_local!` in
  `lab::script`, on the main thread. Lamlet's `Engine`, `Document`, and `Instance` are `!Send`, which is fine
  there.
- **Why a `thread_local!`.** The investigation (`Investigation: Sync`, held as `&'static dyn Investigation`)
  cannot own the engine. Actions are function pointers (`Actions::run`) with no host handle to reach one
  through.
- **Nothing `!Send` enters `Renderer` or `Statics`**, which must stay `Send` for the loading worker.
- **Unit tests** call `state_at` on test threads (`lab/src/values.rs`). With a `thread_local!`, each test thread
  gets its own engine, which is correct and costs one check of the scripts per thread.

Lamlet's phase 2 (sharing checked documents across threads) is not needed. It would matter only if scripts had
to run on several threads at once, and nothing here does.

## What a package writes

### An overlay: O3DE's vectors and lobe

Today `build_frame` draws the inspected point's vectors and lobes in Rust
(`packages/o3de_lighting/src/o3de/lighting/demos.rs:408-447`, `lobes` at `:725`):
- **The arrows:** N, L, V, H (and R in the IBL demo), half a metre long.
- **`vectorsShown`:** a fractional value of it grows the next arrow out of the point with `Ease::Smooth`, and an
  arrow's label shows past 0.6 of its length.
- **The dashed lines:** to the light and to the observer.
- **The two lobes:** swept in the plane of N and L, horizon to horizon, at sizes 0.8 and 0.4, each normalised to
  its peak.
- **The rest:** the label at the specular lobe's tip, and the dashed mirror direction.

All of that is layout around two native results: the inspected pixel (`brdf::Pixel`), and the BRDF terms in
given directions.

As a script (`packages/o3de_lighting/script/overlay.lam`; a sketch):

```text
$[Import* std]$[Import* render_lab_runtime]$[Import* o3de_lighting.values]$[Import* o3de_lighting_natives]

The inspected point's vectors; a fractional `vectorsShown` grows the next one out of the point.
$[# _vectors {^ {px}
  [# shown {!if {.lightOn px}
     {' {named "N" {.normal px} normal} {named "L" {.toLight px} light} {named "V" {.toView px} view} {named "H" {.half px} half}}
     {' {named "N" {.normal px} normal} {named "V" {.toView px} view}}}]
  {grownArrows {.point px} shown 0.5 {vectors} {vectorsShown}}}]

The specular and diffuse lobes in the plane of N and L, each on its own scale.
$[# _lobes {^ {px}
  [# dirs {sweep {.normal px} {inPlane {.normal px} {.toLight px}} 90}]
  [# terms {brdfTerms dirs}]
  {concat {' {lobeLine {.point px} dirs {.specular terms} specular 0.8 {lobe} {#some {rl.text "demos.specular_lobe"}}}
             {lobeLine {.point px} dirs {.diffuse terms} diffuse 0.4 {lobe} #none}
             {' {dashed {.point px} {v+ {.point px} {v* {mirror {.normal px} {.toLight px}} 0.65}} specular {* {lobe} 0.6}}}}}}]

$[# overlay {^ {t} {!match {inspected} {#none} {'} {#some px} {append {_vectors px} {_lobes px}}}}]
```

- **Natives it calls:**
  - `inspected` gives the inspected pixel's point, normal, and directions as a record, computed by the CPU mirror
    from the call's snapshot.
  - `brdfTerms` evaluates both terms in every direction of the list, in one call.

  Both are `o3de_lighting` natives (`Investigation::script_natives`).
- **Helpers it uses:** `sweep`, `inPlane`, `mirror`, `v+`, `v*`, `grownArrows`, `lobeLine`, `dashed`, and the
  colour names. They are plain Lamlet in render_lab's runtime library.
- **Native calls per frame:** two.
- **What is removed from Rust:** the Rust drawing of the vectors and lobes goes, once the script draws the same
  lists (milestone 1). The readout, the culling tiles, and the other overlays stay in Rust until they are
  ported, one at a time.

### A click handler: picking a pixel

`pick` (`packages/o3de_lighting/src/o3de/lighting/actions.rs:14`) traces a ray through the clicked point, flips
the normal to face the viewer, and sets `pickShown`, `inspectX/Y/Z`, `inspectNX/NY/NZ`, and `inspectObject`. Only
the trace is heavy. As a script (a sketch):

```text
Picks the surface point under the click, which the readout inspects and every screen-sized view rings.
$[# pickPixel {^ {click}
  {!match {rl.trace {.uv click} {.except {' panel}}}
    {#miss} {'}
    {#hit h} {' {setPickShown 1.0} {setInspect {.point h}} {setInspectN {facing {.normal h} {.dir h}}}
                {setInspectObject {float {.object h}}}}}}]
```

`rl.trace` is the lab's native. It casts the ray from the call's camera through `uv` into the frame's objects
(`scene::trace_scene`) and returns a variant. Ringing the picked point in other views is
`rl.project point view`, which an overlay calls.

### A script action

The Rust action of `godot_voxelgi` today (`packages/godot_voxelgi/src/godot/voxelgi/actions.rs`):

```rust
#[action]
fn march_cone(frame: &mut ActionFrame, seconds: f32, from: Option<u32>, to: Option<u32>) -> Result<(), String> {
    if seconds <= 0.0 {
        return Err(format!("a step takes {seconds} s; it takes longer than 0"));
    }
    let step = (from.unwrap_or(0) + (frame.elapsed / seconds) as u32).min(to.unwrap_or(16));
    frame.state.set("coneSteps", step as f32);
    Ok(())
}
```

The same action as a script (`packages/godot_voxelgi/script/actions.lam`):

```text
$[Import* std]$[Import* godot_voxelgi.values]

The arguments a story passes to `marchCone` (`run.seconds=…`); optional ones are options.
$[# MarchCone [% {Struct {' {Field "seconds" Float} {Field "from" {Option Int}} {Field "to" {Option Int}}}}]]

Steps a cone's march (`coneSteps`) from `from` (0 by default) to `to` (16 by default), one sample every `seconds`.
$[# marchCone {^ {elapsed run}
  [# args {: run MarchCone}]
  [# seconds {.seconds args}]
  [# from {withDefault {.from args} 0}]
  [# to {withDefault {.to args} 16}]
  {!if {<= seconds 0.0}
    {error f"{a step takes $seconds s; it takes longer than 0}"}
    {' {setConeSteps {float {min {+ from {truncate {/ elapsed seconds}}} to}}}}}}]
```

The story does not change: `${cue "key" {' {run "marchCone"} {runSeconds 0.7} {runFrom 1.0} {runTo 6.0}}}`.

### A behaviour: the Play animation and the camera

A demo's Play animation is a function of the demo's time. Today it is `Investigation::animation(demo, t)` in Rust;
O3DE's light sweep is in `packages/o3de_lighting/src/o3de/lighting/shot.rs:126`. As a script, it is written with
the combinators (a sketch):

```text
The light sweeps from near the normal to grazing and back, while the roughness widens and narrows the lobe.
$[# playSpecular {^ {t}
  {append {orbitLight {pingPong {tween 0.0 1.0 6.0 easeSine} t}}
          {' {setSphereRoughness {pingPong {tween 0.15 0.6 9.0 smooth} t}}}}}]
```

A demo's TOML names it: `animation = "playSpecular"`. The same functions serve a story. A cue
`{run "playSpecular"}` runs it as an action from the cue's frame, since an action is a function of `elapsed`.

### The contract

| Hook | A script exports | Type | Writes? |
|---|---|---|---|
| Action | a function | `Fn([Float, Args], List(Write))`: the seconds since the story started it; the arguments as one record | yes |
| Click handler | a function | `Fn([Click], List(Write))`, where `Click` is `{u: Float, v: Float}` (the point on the image, 0..1, y down) | yes |
| Play animation | a function per demo | `Fn([Float], List(Write))`: the demo's time in seconds | yes |
| Overlay | `overlay` | `Fn([Float], List(Shape))`: the frame's time; drawn over what `build_frame` drew | no |

- **The arguments come as one record**, because a host sees a record's field names and types but not a
  function's parameter names. The ascription (`{: run MarchCone}`) makes the function's type ground, so the lab
  reads the action's arguments from it. Field names are the story's argument names (`run.seconds`), and their
  types give the argument kinds:
  - `Float` is a number, `Int` an integer, and `Bool` a bool.
  - `{x, y}` and `{x, y, z}` are vec2 and vec3; `{x, y, z}` of `Int` is ivec3.
  - `Option(T)` is an optional argument.
  - `String` is a name.

  The lab derives `ActionSpec`s from them, so `render-lab actions` lists script actions with their arguments,
  and the storyboard's vocabulary (`render-lab story keys`) has them as it has Rust actions.
- **A script returns its writes; it never sets state itself.** The lab applies the whole list after the call
  returns, in order. If the call fails, the lab applies nothing. That is the batch rule
  ("Runtime inputs, updates, and failure behavior").
- **An overlay is display.** Its natives have no write capability, and its shapes are drawn but change no value.

### The generated values package

For each investigation the lab writes a Lamlet package, `<package>.values`, from the investigation's values
(`lab/src/values.rs`: `#[derive(Values)]` and shader tags). It holds:

- **`Write`**, the record `{key: String, value: <bool: Bool, float: Float, int: Int, vec2: …, vec3: …, vec4: …>}`.
- **A setter per key:** `setConeSteps : Fn([Float], Write)`. Setters are plain Lamlet functions, not natives,
  so they are pure and cost nothing to declare. Keys that form a vector have one setter for the vector as well
  (`setInspect {x, y, z}` beside `setInspectX`).
- **A reader per key:** `sliceHeight : Fn([], Float) !Runtime`, a native that reads the call's snapshot. For an
  action that is the state so far in `state_at`. For a click, it is the state on screen (`ClickFrame::shown`).
  For an overlay, it is the state being drawn.
- **A choice as a variant:** a choice's setter takes a variant of its option names
  (`{setQuality {#high unit}}`), not an index.

The names and types are then checked when the script is checked: a misspelled key or a wrong type is a
diagnostic at its line, before anything runs. The command `render-lab --investigation <id> script stubs` from
the in-process plan writes this package too.

The types map as follows:

| `values::Type` | Lamlet |
|---|---|
| `F32` | `Float` (the f32 is widened exactly; a write is narrowed to f32) |
| `I32`, `U32` | `Int` (a write out of range is an error) |
| `Bool` | `Bool` |
| `Vec2`, `Vec3`, `Vec4` | `{x, y}`, `{x, y, z}`, `{x, y, z, w}` of `Float` |
| A choice | a variant of its option names |

### render_lab's runtime library

A new module of render_lab's Lamlet package, `render_lab_runtime` (`packages/render_lab/language/runtime.lam`).
It is plain Lamlet, generic, and the same for every investigation:

- **Vectors:** `Vec3` is `{x, y, z}` of `Float`, and `v+`, `v-`, `v*`, `dot`, `cross`, `normalize`, `length`,
  `mirror`, `inPlane`, and `sweep` work on it. Lamlet's `sqrt`, `sin`, and `cos` are deterministic builtins, so
  no native is needed.
- **Shapes:** a variant the lab draws with `DrawLists` and `Label` (`lab/src/frame.rs:72`, `:133`):
  - `arrow`, `segment`, `dashed`, `polyline`, `point`, and `label`, each with a colour, an alpha, and a width;
  - helpers built from them: `grownArrows` and `lobeLine`.

  A label's text is a text key, checked by `rl.text`.
- **Colours:** the lab's palette by name (`normal`, `view`, `light`, `selected`, …). A package adds its own as
  constants (`specular`, `diffuse`, `half`), as `demos.rs` does today.
- **Curves and combinators:** `tween from to seconds ease`, `hold`, `seq`, `par`, `after`, `loop`, `pingPong`,
  and `orbit` and `path` as the lab's motions (`lab/src/animation.rs`). The eases are the lab's: `smooth`,
  `linear`, `easeIn`, `easeOut`, `easeInOut`, `easeSine`.

### Query natives and the call context

Before each call, the lab sets a call context in a `thread_local!`, and it clears the context after the call:
- the snapshot (`ShotState`);
- the frame's content (`FrameContent`) and `Statics`, where the call has them (overlays and clicks);
- the view being drawn (`lab::view::name()`).

Natives read the context; they never keep it. Natives are registered once on the engine:

| Native | From | Returns |
|---|---|---|
| a reader per value | the lab (values package) | the snapshot's value |
| `rl.trace uv except` | the lab | `#hit {point, normal, dir, object}` or `#miss`: a ray from the call's camera into its objects |
| `rl.project point view` | the lab | the point on a view's picture (0..1), or `#none` behind the camera |
| `rl.text`, `rl.source`, `rl.point`, `rl.formula` | the lab (exist) | the checked name |
| `<package>/<name>` | the package, `Investigation::script_natives` | its own data: O3DE's `inspected` and `brdfTerms`; VoxelGI's `leafOf` and its octree size |

`script_natives` registers `Pure` natives today, for storyboards. Runtime natives join it on the same
registry, with Lamlet's default `.runtime()` stage.

### Where scripts are declared

A package's manifest provides a `script` collection, as in the in-process plan (module name → file). The lab
reads hooks from its modules:

- an exported function of the action type is an action, named by its binding;
- one of the click type is a click handler;
- `overlay` is the overlay;
- a demo's TOML names its animation function (`animation = "playOrbit"`).

A script action or click handler with the same name as a Rust one is an error when the package loads.

## Runtime inputs, updates, and failure behavior

The rules every runtime use of Lamlet in the lab follows (scripts here, and live text later).

**Inputs.**
- Each evaluation batch receives an immutable snapshot of:
  - the selected investigation's and package's schemas;
  - typed parameter values;
  - logical time: the frame index and fps;
  - the elapsed cue time, where one applies;
  - locale and message arguments;
  - the event payload.
- Playback and recording derive time through the same helper from the requested frame. No ambient wall-clock
  callback is used to reconstruct a recorded frame.
- Native reads remain Runtime even when a particular snapshot looks constant.

**Reuse.**
- Immutable module initializers run once; they are not implicit reactive controls.
- Live labels and bindings must call functions with the current snapshot, or run a document entry with fresh
  local bindings. Reusing compiled code does not mean reusing a stale native read captured during the first
  initialization.
- Preserve source order when executing a complete document entry. Selecting or batching regions must not reorder
  observable calls.

**Timelines and clicks.**
- Pure bindings can be called independently for any requested frame order.
- A timeline animation computes its state from its anchor or interval and the requested frame, preserving
  `Timeline::state_at`. Firing callbacks only when a cue is crossed is insufficient.
- Viewer clicks and interactive edits are different inputs from timeline-owned state. Define pause, seek, reset,
  and demo-switch ownership, and don't let a click mutate a running timeline's reconstructed state by accident.

**Applying writes.**
- Collect command and effect descriptions during an event or explicit update evaluation, validate the complete
  batch, then apply it at the host update boundary.
- Display and layout evaluation has no write capability.
- Domain natives must not mutate the scene immediately and then leave partial changes when the VM exceeds a
  budget.
- Reject ambiguous multiple writers to one target until explicit composition or precedence is specified.
- Repeated painting does not restart animations or register another handler.
- Persistent simulations require an explicit event log or checkpoints, and are outside the initial binding
  implementation.

**Errors.**
- On error, discard the pending command batch.
- Offline preparation and recording report a source-located failure and do not reuse a corrupt artifact.
- Interactive presentation may keep the last successful value with an error indicator. Logs are deduplicated,
  and the next valid input can recover. Choose that visible policy before shipping.

## The lab's side (generic, in `lab/src/script/`)

- **The runtime engine.** A `thread_local!` engine with the resolver over the packages' `script` collections. It
  holds the values package, render_lab's runtime library, and the natives above. Each script is checked once per
  launch, when its package loads; after that, calls only run compiled functions. One instance serves all calls,
  with its inputs set before each call.
- **Dispatch.**
  - The running actions loop in `state_at` (`lab/src/shot.rs`) looks the key up among the script actions first,
    and otherwise calls the investigation's `Actions::run`.
  - Clicks go the same way: script handlers before `#[click]` handlers, then `Investigation::click`
    (`lab/src/viewer.rs`).
  - `Investigation::animation` falls back to the demo's script animation.
  - `shader_data::build_frame` (`lab/src/shader_data.rs:511`) calls the overlay after the investigation's
    `build_frame`, beside the story's annotations, once per view.
- **Arguments.** The lab converts a `RunningAction`'s arguments into the script's argument record, filling an
  omitted optional argument with `#none`.
- **Applying writes.** The lab validates the whole list (known key, matching type, range for integers), then
  applies it in order through `ShotState::set_value`.
- **Drawing shapes.** The lab decodes the overlay's list (`Data`) into `DrawLists` calls and `Label`s, with the
  text from the package's table.
- **Text keys.** The text check (`lab/check`) finds the keys a script names, as it finds a storyboard's. Unused
  and unknown keys stay build errors.
- **Budgets.** Each call runs under small `Limits`, on the order of 10⁶ instructions, so a broken script fails
  one call instead of freezing a frame. Lamlet's allocation budget is cumulative per call, not per instance; a
  long session is checked for it (milestone 0).
- **Errors.**
  - Check errors (syntax, types, an unknown key) are reported when the package loads, with the script's file and
    line, so they never reach a timeline.
  - A runtime error while recording or building a video fails the run with the script's file, line, and the
    frame. Today a failing action panics at that point (`shot.rs`).
  - In the viewer, the shot keeps its last good state, and an overlay its last good shapes. An error indicator
    shows the message, which is a text key in both languages. Further failures of the same message are not
    logged again, and the next successful call clears the indicator.

The lab keeps no code that only one investigation uses: everything above serves any package's scripts.

## Dependency

Done. The lab pins `lamlet` v0.4.0 (the in-process plan, "Dependency and distribution") and re-exports it as
`lab::script::lamlet`; packages use it only through the lab. Lamlet has no dependencies. This plan needs no new
crate and no new Lamlet release.

## Milestones

0. **The runtime host.** The `thread_local!` engine, the `script` collection and its resolver, the values
   package, the call context, and the `Write` and `Shape` decoding. It also covers:
   - budgets and the error policy, with the viewer's indicator and its text keys;
   - `render-lab --investigation <id> script stubs`;
   - checking that the loading worker never builds a frame, and measuring allocation over a long session.

   No investigation uses it yet, so every frame stays unchanged.
1. **The overlay pilot: O3DE's vectors and lobe.**
   - `render_lab_runtime`'s vectors, shapes, and colours.
   - O3DE's `inspected` and `brdfTerms` natives.
   - `overlay.lam`.

   The Rust drawing stays behind a switch until the draw lists match (Validation), then goes.
2. **Clicks.** `rl.trace` and `rl.project`, then O3DE's `pickPixel` as a script, then VoxelGI's
   `pick_slice_cell`, with the octree data as natives.
3. **Script actions.** `marchCone` ported. The Rust version stays until frame identity passes, then goes.
4. **Behaviours.** The combinators, demo Play animations from scripts (O3DE's first), and story actions written
   with them.
5. **More overlays.** O3DE's other `build_frame` drawing, one overlay at a time:
   - the falloff curve and radius;
   - the GGX samples;
   - the culling tiles;
   - the cascades.

   Each moves only if it is layout around native data.
6. **Live readouts and labels.** the in-process plan's milestone 4, on the same main-thread engine.
7. **Documentation.**
   - The `render-lab-lamlet` and `packages` skills: scripts, hooks, natives, and the split.
   - `tools/video/README.md`, where cues run script actions.
   - `AGENTS.md`'s list of content.

## Validation

- **Draw lists.** For the pilot, a unit test builds O3DE's frame at a set of states with the Rust drawing and
  with the script, and compares the draw lists and labels within 1e-5. The script computes in f64 and Rust in
  f32, so they are not compared bit for bit. Then:
  - Offscreen snapshots (`--page viewer:specular`, `viewer:diffuse`) are looked at.
  - O3DE's timeline frames that show vectors or lobes are compared by decoded-pixel hashes. O3DE's batch renders
    vary from run to run, so a mismatch alone is re-rendered first.
- **Frame identity.** With the Rust `marchCone` and with the script, `state_at` gives equal values for every
  frame of every timeline that runs it, and recorded frames are byte-identical.
- **Precision.** The script computes in f64, and the Rust action in f32. At an exact step boundary they could
  disagree, so frame identity is checked frame by frame, not only at a few times. If they disagree, the plan
  decides which one is correct.
- **Clicks.** A pick through the script sets the same values as `actions::pick` at a grid of points on each view
  of a screen-sized resource (the existing pick round-trip check).
- **Purity.** `state_at(f)` in a shuffled order of frames equals the sequential results, and an overlay at the
  same state gives the same list twice.
- **Hostile scripts.** A script with bad syntax, a misspelled key, a wrong type, an endless loop, or a deep type
  gives a located diagnostic when the package loads, or a budget error at the frame. Never a panic or a hang.
- **Cost.** The pilot's overlay and `state_at` with a running script action stay within a small factor of the
  Rust versions: the viewer's performance panel, or a unit test timing the calls.
- **Builds.** Both builds are clean: the lab, and the public-only copy alone. Each local package's workspace is
  rebuilt after the lab. Selftests of every investigation pass, as does
  `cargo test --release --locked --offline --workspace`.

## Implementation notes (2026-11-03)

What was built, and how it differs from the sketches above:

- **Scripts are resources**, not a `script` collection: each `script/<name>` resource of the investigation's
  package is a module, imported by the others as `<package>_<name>`. `render-lab --investigation <id> script check`
  lists the hooks; `script stubs [NAME]` prints what a script imports.
- **Readers** are functions of the generated values package (`{vectorsShown}`), each calling a generic native
  (`rl_runtime/value-float "vectorsShown"`): the names are still checked when a script is checked, with one native per
  type instead of one per key. A key whose name std, the library, or the natives declare is left out (listed by
  `script stubs`); so a package's native must not be named like one of its values.
- **Hooks by type** (open question 2): the types are the library's (`Overlay`, `Readout`, `ClickHandler`,
  `{Action Args}`, `Animation`), and a hook's type must be ground, so a script ascribes it. A sixth hook was added:
  **readouts**, rows the lab adds to the frame's readout (O3DE's point light rows).
- **The overlay's place** (open question 4): beside `build_frame`, but labels are placed in order, so drawing after
  everything moved O3DE's vector labels (seven of its 97 beats). An investigation marks the place its drawing had
  (`FrameContent::overlay_at`, a `DrawMark`), and the lab inserts the overlay's shapes there.
- **Frame data**: `FrameContent::script` holds what `build_frame` computed for the natives (O3DE's inspected pixel),
  read with `frame_data::<T>()`; the natives compute nothing twice.
- **Errors** (open question 3): in the viewer, the last good shapes and no writes, with the message as a note
  (`script.error`) until the hook succeeds; a recording or a selftest fails (`set_strict`).
- **Precision**: Lamlet computes in f64 and the lab in f32. An action's numbers are read at f32, as the code read
  them. `marchCone` disagreed with the Rust action on one of 487 frames, where the f64 quotient was a rounding short
  of the step's whole number; `steps` counts a step from its frame (the Rust action was right there by its rounding).
- **Clicks on the scene**: the script click handler named `scene` runs before `Investigation::click`.
- **Milestone 5, in part**: O3DE's vectors, lobes, GGX samples, attenuation radius, and falloff curve are its
  overlay's; the culling tiles, cascades, decal boxes, and SSAO samples stay in its code (camera and tile geometry).
- **Animations**: a demo's `animation` names a script animation, applied after `Investigation::animation`; O3DE's
  surface and IBL demonstrations' moved. Story curves per frame (the in-process plan milestone 5) stay keys.

Validation, as run: viewer snapshots of O3DE's specular, diffuse, IBL, and point light demonstrations identical to
the Rust drawing's; 93 of O3DE's 97 beat midpoints identical and four differing by one to six anti-aliased pixels
on a lobe or a curve (f64 against f32 endpoints), the same rendered alone; the selftests compare `pickPixel` with the
old pick at 336 points (the same), `pickSliceCell` at 6724 clicks, `marchCone` on every frame its story runs it, and
the animations with their waves; unit tests of the library, shapes, and action arguments.

## What is left, in stages (2026-11-05)

Done since the notes: milestone 3 (Godot SDFGI's `jumpFlood` is a script, `packages/godot_sdfgi/script/logic.lam`; no
public package has a Rust `#[action]` left), and Validation's linked-natives check (`require_linked` on the lab's
engines, `lab/src/script/mod.rs` and `runtime.rs`). Each stage below is one commit, validated as "Validation" says
(the old Rust and the script compared in the selftest first, then the Rust removed; frame identity for its frames).

1. **Scene clicks (milestone 2).** Done (2026-11-05): VoxelGI's and SSR's clicks on the scene are `scene` click
   handlers in their `script/logic.lam` (VoxelGI's from `rt::trace` and its `leafOf`; SSR's from `rt::viewport`, a
   new runtime native, the frame's viewport in pixels), checked in their selftests against the Rust they replace at
   41 x 41 points; `Investigation::click` stays a stub returning false in each (the trait requires it), as in SDFGI.
   No general `rl.project` was needed.
2. **Play animations (milestone 4).** SSR's, VoxelGI's, and SDFGI's `Investigation::animation` (each a
   `shot::demo_animation` table of waves) become `Animation` hooks in a `script/animations.lam` per package, as
   O3DE's did; the selftests compare the values set at a grid of `t` with the Rust before it goes.
3. **Overlays (milestone 5).** O3DE's culling tiles, cascades, decal boxes, and SSAO samples stay native: they are
   camera and tile geometry, not layout around native data (the rule above). SSR's, VoxelGI's, and SDFGI's
   `build_frame` drawing moves where it is layout (arrows, labels, markers around computed points), one package per
   commit: (a) SSR, (b) VoxelGI, (c) SDFGI.

Open question 5 (recording interactive sessions) stays for later.

Names in the sketches above are the design's: the built ones are in "Implementation notes" (scripts are
`script/<name>` resources, not a `script` collection; the natives are `rl_runtime/…`; a package's values are
imported as `<package>_values`).

## Open questions

Questions 1 to 4 were settled as recommended (see "Implementation notes"); only 5 is open.


1. **Readers as natives, or the shot passed in.** Natives per key keep the type of every read checked.
   Passing the whole shot as a record would make every script's type depend on every key. Recommended: natives.
2. **Hooks by type, or listed in the manifest.** By type keeps a script self-describing. A list in the manifest
   states the intent, so a helper function of the same type cannot become an action by accident. Recommended: by
   type, with names starting with `_` excluded, as Lamlet's own exports are.
3. **The error policy in the viewer.** Last good state with an indicator, or the default state? This plan
   assumes the former, as the runtime rules suggest.
4. **An overlay beside `build_frame`, or replacing parts of it.** Beside it (recommended): the investigation
   stops drawing what the script draws, and the lab draws the script's shapes after it. That keeps
   `build_frame`'s contract unchanged.
5. **Recording interactive sessions.** Logging each click with its time and its natives' results would let an
   interactive exploration be replayed or recorded as a video. Not needed by any hook above; later, if wanted.
