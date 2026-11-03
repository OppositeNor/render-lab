# Real-time logic in Lamlet scripts (plan)

Today a package's real-time logic is Rust in its library: actions (`#[action]`, started by a story's
`{@ run=…}`), click handlers (`#[click]`), and the Play animation (`Investigation::animation`). This plan lets
the same logic be written as Lamlet scripts that are package content. A script is read through `lab::resources`
like a shader or a text table, so an edit shows on the next launch without rebuilding the package's library, and
a package can have logic without Rust code.

It builds on [lamlet-in-process.md](lamlet-in-process.md), which puts a Lamlet host in the lab (`lab::script`: an
engine, a resolver over package resources, natives). It follows the runtime rules of
[lamlet-integration.md](lamlet-integration.md) ("Runtime inputs, updates, and failure behavior"). The language
side is Lamlet's `plans/embedding.md`; this plan asks Lamlet for one general fix (below) and nothing specific to
the lab.

Status: plan (2026-11-02). The script in "A script action" was checked and run with Lamlet's embedding API
against a stand-in for the generated values package. It matches the Rust action at the sampled times. Nothing
in the lab is implemented.

## Why the fit is good

The lab already requires what Lamlet guarantees:

| The lab requires | Where | Lamlet gives |
|---|---|---|
| An action is a pure function of its arguments, its `elapsed`, and the state so far, because `state_at` replays every key from frame 0 for each frame, and seeking and recording rely on that | `lab/src/shot.rs` (`state_at`, "Pure function of (timeline, frame)"; the running actions loop) | No side effects; reads only explicit inputs; math that gives the same bits on every platform |
| A hook only sets values of the shot (`ShotState`); it never touches GPU resources or the scene | `lab/src/actions.rs` (`ActionFrame`, `ClickFrame`) | Code returns descriptions, and the host applies them |
| `state_at` runs several times per rendered frame | `lab/src/viewer.rs` | A call of a small compiled function costs about 0.2 µs (Lamlet's benchmarks) |
| A hook cannot run forever or crash the viewer | | Instruction, call-depth, and allocation budgets per call; caught panics; a checker bounded on every input |

## Threads: Lamlet's phase 2 is not needed

Every hook this plan touches runs on the main (winit) thread. That holds in the viewer (`state_at`, clicks,
`animation`) and in the recorder, which is single-threaded. The loading worker builds the renderer and runs no
action, click, or animation.

- **The engine.** One Lamlet engine, its checked scripts, and their instances live in a `thread_local!` in
  `lab::script`, on the main thread. Lamlet's `Engine`, `Document`, and `Instance` are `!Send`, which is fine
  there.
- **Why a `thread_local!`.** The investigation (`Investigation: Sync`, held as `&'static dyn Investigation`)
  cannot own the engine. Actions are function pointers (`Actions::run`) with no host handle to reach one
  through.
- **Nothing `!Send` enters `Renderer` or `Statics`**, which must stay `Send` for the loading worker.
- **Unit tests** call `state_at` on test threads (`lab/src/values.rs`). With a `thread_local!`, each test thread
  gets its own engine, which is correct and costs one check of the scripts per thread.

So [lamlet-in-process.md](lamlet-in-process.md)'s milestone 4 ("after Lamlet phase 2: a per-thread engine in the
viewer") can drop its dependency on phase 2. Phase 2 matters only if scripts must run on several threads at
once, and nothing here does.

## What a package writes

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
    {error [f "a step takes $seconds s; it takes longer than 0"]}
    {' {setConeSteps {float {min {+ from {truncate {/ elapsed seconds}}} to}}}}}}]
```

The story does not change: `{@ run=marchCone run.seconds=0.7 run.from=1 run.to=6}`.

### The contract

| Hook | A script exports | Type |
|---|---|---|
| Action | a function | `Fn([Float, Args], List(Write))`: the seconds since the story started it, the arguments as one record |
| Click handler | a function | `Fn([Click], List(Write))`, where `Click` is `{u: Float, v: Float}` (the point on the image, 0..1, y down) |
| Play animation | a function per demo | `Fn([Float], List(Write))`: the demo's time in seconds |

- **The arguments come as one record**, because a host sees a record's field names and types but not a
  function's parameter names. The ascription (`{: run MarchCone}`) makes the function's type ground, so the lab
  reads the action's arguments from it. Field names are the story's argument names (`run.seconds`), and their
  types give the argument kinds:
  - `Float` is a number, `Int` an integer, and `Bool` a bool.
  - `{x, y}` and `{x, y, z}` are vec2 and vec3; `{x, y, z}` of `Int` is ivec3.
  - `Option(T)` is an optional argument.
  - `String` is a name.

  The lab derives `ActionSpec`s from them, so `render-lab actions` lists script actions with their arguments,
  and a story's tags are checked against them as they are for Rust actions.
- **A script returns its writes; it never sets state itself.** The lab applies the whole list after the call
  returns, in order. If the call fails, the lab applies nothing. That is the batch rule of
  lamlet-integration.md.

### The generated values package

For each investigation the lab writes a Lamlet package, `<package>.values`, from the investigation's values
(`lab/src/values.rs`: `#[derive(Values)]` and shader tags). It holds:

- **`Write`**, the record `{key: String, value: <bool: Bool, float: Float, int: Int, vec2: …, vec3: …, vec4: …>}`.
- **A setter per key:** `setConeSteps : Fn([Float], Write)`. Setters are plain Lamlet functions, not natives,
  so they are pure and cost nothing to declare.
- **A reader per key:** `sliceHeight : Fn([], Float) !Runtime`, a native that reads the shot as it is when the
  script runs. For an action that is the state so far in `state_at`; for a click, the state on screen
  (`ClickFrame::shown`).
- **A choice as a variant:** a choice's setter takes a variant of its option names
  (`{setQuality {#high unit}}`), not an index.

The names and types are then checked when the script is checked: a misspelled key or a wrong type is a
diagnostic at its line, before anything runs. The command `render-lab --investigation <id> script stubs` from
lamlet-in-process.md writes this package too.

The types map as follows:

| `values::Type` | Lamlet |
|---|---|
| `F32` | `Float` (the f32 is widened exactly; a write is narrowed to f32) |
| `I32`, `U32` | `Int` (a write out of range is an error) |
| `Bool` | `Bool` |
| `Vec2`, `Vec3`, `Vec4` | `{x, y}`, `{x, y, z}`, `{x, y, z, w}` of `Float` |
| A choice | a variant of its option names |

### Package data a script needs

Logic often needs a package's own data. `godot_voxelgi`'s click handler (`pick_slice_cell`) reads the probe's
octree size and calls `leaf_of`. A script gets such data through natives the package registers, which is
lamlet-in-process.md's `Investigation::script_natives` hook. The logic moves to the script; computing the
octree stays in Rust.

### Where scripts are declared

A package's manifest provides a `script` collection, as in lamlet-in-process.md (module name → file). Its
module `actions` (resource `<package>.script/actions`) is read for hooks:

- an exported function of the action type is an action, named by its binding;
- one of the click type is a click handler;
- a demo's TOML names its animation function (`animation = "playOrbit"`).

A script action or click handler with the same name as a Rust one is an error when the package loads.

## The lab's side (generic, in `lab/src/script/`)

- **Values package generation** from the current investigation's values, with readers registered as natives
  that read a `ShotState` passed in for the call. The `ShotState` is passed as host context, never kept between
  calls.
- **Dispatch.**
  - The running actions loop in `state_at` (`lab/src/shot.rs`) first looks the key up among the script
    actions, and otherwise calls the investigation's `Actions::run`.
  - Clicks are dispatched the same way (`lab/src/viewer.rs`).
  - `Investigation::animation` falls back to the demo's script animation.
- **Arguments.** The lab converts a `RunningAction`'s arguments into the script's argument record, filling an
  omitted optional argument with `#none`.
- **Applying writes.** The lab validates the whole list (known key, matching type, range for integers), then
  applies it in order through `ShotState::set_value`.
- **Caching.** A script is checked once per launch, when its package loads, not on each `state_at`. `state_at`
  only calls compiled functions. One instance serves all calls; its inputs are set before each call.
- **Budgets.** Each call runs under small `Limits` (on the order of 10⁶ instructions), so a broken script fails
  one call instead of freezing a frame.
- **Errors.**
  - Check errors (syntax, types, an unknown key) are reported when the package loads, with the script's file and
    line, so they never reach a timeline.
  - A runtime error while recording or building a video fails the run with the script's file, line, and the
    frame. Today a failing action panics at that point (`shot.rs`).
  - In the viewer, the shot keeps its last good state, and an error indicator shows the message. The message is a
    text key in both languages. Further failures of the same message are not logged again, and the next
    successful call clears the indicator.

The lab keeps no code that only one investigation uses: everything above serves any package's scripts.

## Dependency and release

- **The lab depends on the `lamlet` library**, re-exported as `lab::script::lamlet` as lamlet-in-process.md
  plans. Packages use it only through the lab, since a package may not add a crate the lab lacks.
- **The dependency is a git dependency on a tagged release** of the public repository
  (`https://github.com/lamlet/lamlet`). A fresh clone then builds it from the network, and `setup.sh`
  vendors it like any other crate. A path outside this repository would break fresh clones. The `embed` API is not
  in a tag yet: Lamlet needs a release (`v0.2.0`) before this starts. `vendor/` is refreshed by the user with
  `setup.sh`, never edited here.
- **Lamlet has no dependencies**, so it adds no other crate to the lab's build or to `vendor/`.
- **Compiler version.** The lab requires Rust 1.88. Lamlet builds with a Rust 1.90 nightly from 2025-06-30; 1.88
  itself is not verified yet.
- **Rebuilds.** Changing the lab changes its ABI string, so every package library is rebuilt, as for any lab
  change.

## Needed from Lamlet (general, in its repository)

- **Locate errors raised through tail calls.** A function whose last step calls std's `error` loses its own
  frame before the error is raised: the VM releases the caller's frame for a tail call. The diagnostic then
  points at std's `error` definition instead of the script's line. This was seen with `marchCone`'s check
  above. Lamlet should keep a short trace of the frames tail calls replaced, so the script's line is in the
  diagnostic. Every script that rejects bad arguments needs this.
- **A tagged release with `lamlet::embed`** (`v0.2.0`).

## Milestones

1. **Prerequisites.**
   - Lamlet `v0.2.0`, with the tail-call trace.
   - The lab depends on it.
   - From lamlet-in-process.md milestone 1, only the engine and the resolver over package resources are needed
     here; `story prepare` is not.
2. **Script actions.** The values package, the dispatch in `state_at`, writes, `ActionSpec`s from script types,
   and load-time checking. `marchCone` is ported as a script, and the Rust version stays until frame identity
   passes; then the Rust version goes.
3. **Errors.** The recorder's located failure, the viewer's indicator with its text keys, and budgets.
4. **Click handlers.** `script_natives` from lamlet-in-process.md, and `pick_slice_cell` ported, with the octree
   data as natives.
5. **Play animations** from scripts.
6. **Live readouts and labels.** lamlet-in-process.md's milestone 4, on the same main-thread engine, without
   Lamlet phase 2.

## Validation

- **Frame identity.** With the Rust `marchCone` and with the script, `state_at` gives equal values for every
  frame of every timeline that runs it, and recorded frames are byte-identical.
- **Precision.** The script computes in f64, and the Rust action in f32. At an exact step boundary they could
  disagree, so frame identity is checked frame by frame, not only at a few times. If they disagree, the plan
  decides which one is correct.
- **Purity.** `state_at(f)` in a shuffled order of frames equals the sequential results.
- **Hostile scripts.** A script with bad syntax, a misspelled key, a wrong type, an endless loop, and a deep type
  each give a located diagnostic when the package loads, or a budget error at the frame. Never a panic or a hang.
- **Cost.** `state_at` on a timeline with a running script action stays within a small factor of the Rust
  action. Measure it with the viewer's performance panel or a unit test.
- **Builds.** Both builds are clean: the lab with `lamlet` vendored, and the public-only copy alone. Selftests
  of every investigation pass, as does `cargo test --release --locked --offline --workspace`.

## Open questions

1. **Readers as natives, or the shot passed in.** Natives per key keep the type of every read checked.
   Passing the whole shot as a record would make every script's type depend on every key. Recommended: natives.
2. **Hooks by type, or listed in the manifest.** By type keeps a script self-describing. A list in the manifest
   states the intent, so a helper function of the same type cannot become an action by accident. Recommended: by
   type, with names starting with `_` excluded, as Lamlet's own exports are.
3. **The error policy in the viewer.** Last good state with an indicator, or the default state? This plan
   assumes the former, as lamlet-integration.md suggests.
