# A data-driven frame: the scene shading read from a package's TOML (plan, 2026-11-03)

## Why

An investigation's frame (`SceneShading`, lab/src/investigation.rs) is mostly a declaration — targets, buffers,
volumes, passes and their reads and writes, history, shows, a program's images, buffers, passes and uses — written as
Rust `static`s in its library. Changing a target's format or a pass's reads means a rebuild, while everything else a
package holds (scenes, demos, scripts, text) is content read at run time. The frame graph already derives order and
culling from the reads and writes, and the lint checks them against the WGSL, so the declaration needs no logic: TOML
is enough.

What computes stays code: the CPU scene data (`data`), an update's plan and buffer sizes, a resolution's divisor, and a
program's per-frame plan (SDFGI's is a port of the engine's CPU side and shares types with its CPU mirror and selftest).
The file names those functions; the library registers them by name.

## Stage 1: the declaration as data (fixed slots kept)

- A package's `shading.toml`, a `file` resource with id `shading` (not exported: only its package reads it).
- `lab/src/shading_file.rs` parses it (toml_edit, as knowledge.rs and packages.rs do: unknown keys are errors, each
  error names the file and the table) into a `SceneShading` whose strings and slices are leaked (`'static`, as the
  renderer's types are), once per package and run: the renderer matches shadings by pointer (`renderer.rs`
  `std::ptr::eq(s.source, source)`), so a package's shading is loaded once and kept.
- `Investigation::shading_hooks() -> &'static [(&'static str, ShadingHook)]`: the named functions —
  `Data`, `UpdatePlan`, `Words` (an update buffer's size), `Divisor`, `Program { init, plan }`. The file names a hook
  (`data = "shading-data"`); a missing or wrongly typed hook is an error.
- `Investigation::scene_shading()`'s default reads the package's `shading` resource if it has one (None otherwise).
  The investigations stop overriding it and drop their `SHADING` statics.
- The file's form follows `SceneShading`'s roles (scene pass, pre-pass, early, passes, update, depth views, program
  with explicit `stage`): this stage changes where the declaration lives, not what a frame does. Engine-source
  comments of the statics move into the file as TOML comments.
- Format names are WebGPU's (`rgba16float`, `rg11b10ufloat`, `rgba8unorm-srgb`).

Conversion: a temporary writer prints each investigation's static as TOML; the comments are carried over by hand.
Checks: `render-lab graph --json` identical before and after for every investigation; the selftests; the frame identity
check (the change is in `lab/`); the public build; each local package's workspace.

Order: lab support and SSR (smallest, a program); then O3DE (depth views), VoxelGI (update, resolution), SDFGI
(program, resolution), and any local packages; then the docs (render-graph and packages skills, `SceneShading`'s doc
comment, README's mention if any).

Status (2026-11-03): done. Every investigation's frame is its package's `shading.toml`; the statics and `scene_shading`
overrides are gone. Each file read back equals its old static field by field (through a temporary writer, since
removed); `render-lab graph --json` is unchanged for each; the unit tests (two new for the parser), the selftests, the
frame identity check (16 beat midpoints each; the only differences are the performance panel's measured times and the
CPU-time readout, which differ between two runs of the old binary too), and the public build pass. O3DE's selftest
check of the narration's point-light numbers fails, as it does at the parent commit.

## Stage 2: the frame's order as data

The fixed slots (update, program `Start`, depth views, pre-pass, `AfterPrepass`, early, `BeforeScene`, scene,
`AfterScene`, passes, in `FrameGraph::new` and `Renderer::encode_scene`) are replaced by the file's ordered `[[node]]`
list (`SceneShading::nodes`, `FrameNode`). Each node has a `kind`, which carries its own declaration:

- `update` (they come first: they run before the frame, when their plan's key changes), `depth-views`, `prepass`,
  `compute` (those before the scene node are `early`, those after it `passes`), `program`, and `scene` (exactly one).
- The graph declares the nodes in that order and checks it: each declared node once, update nodes first, the pre-pass
  before the scene pass. Its order is the list's, since every edge follows declaration order.
- `ProgramStage` is gone. A run of consecutive program nodes is where the plan's commands for its passes are recorded
  (`SceneShading::program_runs`); the commands after the plan's last pass command are recorded right before the scene
  pass, as `BeforeScene`'s trailing commands were. `ProgramRuntime::finish` ends the frame's recording, which the
  `AfterScene` stage used to do.
- A `SceneShading` built in Rust with empty `nodes` (the graph's tests) gets the usual order (`frame_nodes`).

The files were converted by a script that keeps comments, placing the nodes in the old slot order. Status
(2026-11-03): done. `render-lab graph --json` is unchanged for every investigation; the unit tests (the parser's and
the graph's node-order checks), the selftests, the frame identity check against the parent commit (same frames and
the same run-to-run differences as stage 1's), and the public build pass.

Next (not started): a general `raster` node (the scene's meshes with a fragment entry into named attachments, a depth
mode, a clear) that the scene pass, the pre-pass, and the depth views become instances of, so an engine's other raster
passes (a G-buffer split, a decal pass) can be declared too. Its scene-specific parts (the sky behind a frame without a
chain, the program's scene group, MSAA attachments) need a design first.

Not planned: plans or scene data in Lamlet or TOML (they compute; a simple declarative command list could come later
for programs as small as SSR's).
