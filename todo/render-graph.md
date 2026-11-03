# Render graph (plan)

Turn each investigation's frame into a real render graph: passes and resources declared as data, the order and
lifetimes derived from them, and the graph itself something the viewer shows — every pass with its GPU time, every
resource inspectable as it stood after the pass that wrote it. A feature's whole rendering process then becomes a
demonstration of its own, and a story can point at any intermediate state.

This plan recorded the design and its phases (2026-11-01); the user then asked for all of it to be built
(2026-11-02, decisions left to the agent). Its status, and where the build departed from the design, are in
**Status** at the end. The skill that teaches it is `.agents/skills/render-graph/SKILL.md`.

The tables below describe the lab before the work (line numbers of that commit).

## What the lab does today

The lab already has a declarative, fixed-shape pass chain (`SceneShading`, `lab/src/investigation.rs:93`): named
resources and compute passes with `reads`/`writes`, bound by name. The order is not derived from them; the renderer
runs fixed slots.

| Area | Evidence | What it means for a graph |
|---|---|---|
| Frame order | `encode_scene` (`lab/src/renderer.rs:1757`): `encode_updates` (1761, 1713) → chain targets rebuilt (1765) → depth views, one render pass per layer, only when `depth_views_key` changes (1785–1808) → depth pre-pass (1811–1834) → `early` (1836) → MSAA scene pass (1838–1871) → `passes` (1873) → overlay (1876). `render_frame_with` (1314): compare reference (1385–1406), panes (1410–1422, the whole chain again per pane), free camera (1450–1470), then the main encoder (1561): image kinds without `reads` (1565), `encode_scene`, kinds with `reads` (1567), composite, compare overlays, UI, readback. | The fixed nodes become lab-provided nodes of every graph; the slots become an order derived from edges. |
| Chain steps | `ChainTargets::dispatch` (491): `early` and `passes` concatenated by index (495); timed: one compute pass per step labelled with its entry; untimed: one compute pass, group 2 rebound per step. wgpu places the barriers. | No barrier work is needed; a node list replaces the index split. |
| Resources | `ViewportTargets::new` (78): `scene hdr` (COPY_SRC), 4× MSAA hdr/overlay, 4× MSAA `depth` (no COPY_SRC). `ChainTargets::new` (374): targets TEXTURE_BINDING, + RENDER_ATTACHMENT for scene outputs (4× MSAA, 395), + STORAGE_BINDING if written; **no COPY_SRC**; buffers STORAGE \| COPY_SRC (398–401); depth views a D2 array (402–411). `VolumeSet::new` (263): 3D, mips, COPY_SRC, 4 sets kept by update key (256). | Every resource lives the whole frame; nothing is transient, aliased, or carried to the next frame. Intermediate targets cannot be copied out today. |
| Bindings | `chain_binding` (`lab/src/pipelines.rs:112`): target `i` read `2i`, written `2i+1`; `depth` `2N`; `hdr` `2N+1`; buffer `k` `2N+2+k`; depth views after the buffers. `chain_layout` (575) picks the binding type from the name. Names resolve in `renderer.rs:413–427` (panics on unknown). | Shaders keep these bindings; the graph changes the order of work, not a pass's interface. |
| Declarations | o3de_lighting (`packages/o3de_lighting/src/o3de/lighting/mod.rs`): 11 targets (65–77), 7 `early` (80–93), 9 `passes` (128–138), 12 buffers (95–115), 4-layer cascades (118), `scene_reads`. Buffers bind `read_write` whichever list names them, so passes list the buffers they only read under `writes` (`o3deLightCullingRemapCS` writes `lightList`, `tileData`; `o3deMergeCS` writes `ssaoFull`). godot_voxelgi (`packages/godot_voxelgi/src/godot/voxelgi/mod.rs:63–95`): 4 targets, 2 passes, half-resolution GI (`ShadingResolution`), a 128×64×128 volume with 8 mips filled by 4 update passes (`update.rs:28–44`); two of those declare neither reads nor writes. godot_sdfgi (`mod.rs:56`): the scene pass only. | Edges are only as good as the declarations: they must be audited before order can come from them. |
| History | No previous-frame resource exists. The only temporal state is the still-frame counter `FrameRequest::history` (570–577), packed into `viewParams.w`, 0 for panes and recordings; `reset_history` (1959) restarts it. | History resources are new, not a migration. |
| Timing | `lab/src/timing.rs`: 64 timestamp pairs per frame, rows keyed by label (pass entry, or "scene pass", "depth views", …), a label repeated in a frame summed (152–155), 120 frames kept; `Renderer::performance_view` (1172) → `ui::PerformanceView` (`lab/src/ui.rs:430`), drawn at 984. Main encoder only. | A node's time is its timing row; the performance panel and graph panel share it. |
| Seeing intermediates | Image kinds with `reads=` (`lab/src/views.rs`, tags at 200, `reads` at 225) sample chain resources after the chain (`dispatch_views(true)`, `renderer.rs:918`) through `read_group` (451). `read_view` (1228) reads back only the final display image and `hdr` (1258–1269); `metrics::Image` (`lab/src/metrics.rs:25`). | One view shader per thing shown; only end-of-chain state; experiments measure only the final frame. |
| Viewer | Panels: `PanelMode` (`lab/src/shot.rs:73–85`: Performance, Knowledge, Hidden); tabs: `DockTab` (`lab/src/ui.rs:275`), drawn at `lab/src/controls.rs:92–94, 166–168`. Snapshot pages: `lab/src/cli.rs:292–309` (help 55–70), modelled on `viewer::snapshot_performance` (`lab/src/viewer.rs:1635`). DAG layout: `lab/src/layered.rs` (Sugiyama, used by diagrams and the knowledge panel). | The graph panel reuses the layered layout, the timing rows, and the snapshot-page pattern. |
| Story tags | `tools/video/markup.py` (`parse_tag` 78, `STRING_FIELDS` 46, `image_attr` 320), `tools/video/story.py:104`, `tools/video/timeline.py:26` (`KEY_FIELDS`); Rust `Key` (`lab/src/shot.rs:393`, `image` 436, `image_from` 443), `check_image` (785–802), `image_source` (804), `ImageSource::View` applied at 968. | A graph image is a new image source beside view kinds and PNGs. |

## Design

### The graph

`lab/src/graph.rs` holds a `FrameGraph`: nodes and resources, built for a state each frame from the investigation's
declaration plus the lab's own nodes.

- **Resources**: `Texture` (format, size as viewport divisor or fixed, layers, mips, samples), `Buffer` (words per
  tile, or fixed), `Volume`. Each is **transient** (lives from its first writer to its last reader), **imported** (the
  lab's: `hdr`, `depth`, the frame), **persistent** (kept across frames by key: today's volumes), or **history** (this
  frame's and the last frame's, swapped each frame; reset by `reset_history` and when its size changes).
- **Nodes**: `Compute` (entry, workgroup, reads, writes, dispatch over a resource or a count), `Raster` (vertex and
  fragment entries, what it draws: the scene's meshes, a fullscreen triangle, or an instance list; colour targets with
  load/clear/store; depth with its compare and write), and the lab's built-ins (`scene`, `prepass`, `depthViews`,
  `overlay`, `composite`). A node may carry a `when` (a `UpdatePlan`-style key: run only when it changes) and a `pass`
  label naming the engine pass it mirrors (shown in the panel, linked to the source index).
- **Compile** (once per change of declaration, resolution, or `when` keys): validate (every read has an earlier writer
  or is imported/persistent/history; no unknown names; formats match the binding types), sort topologically (ties
  keep declaration order, so today's frames stay identical), cull nodes whose outputs nobody reads (off while the
  inspector is on), and compute each transient's first and last use. Errors are load-time errors with the package and
  node, as other content checks are.
- **Execute**: walk the sorted nodes into one encoder, binding exactly as `chain_binding` does today, so no shader
  changes. Panes, the compare reference, and the free camera execute the same compiled graph in their own slots.
- **Aliasing**: transients of identical descriptors whose lifetimes do not overlap may share a texture. It is
  optional and off by default (it saves little at lab sizes, and it would make the inspector's "after node N" depend
  on the schedule); its value is to *show* memory reuse in the panel, as engines do.

`SceneShading` stays as written; the lab converts it to a graph (`SceneShading::graph`), so the packages migrate one
at a time, or never. A package that wants more declares a graph directly (`Investigation::frame_graph`, returning a
`GraphDecl` of `'static` data, like `SceneShading` now).

### The inspector

Any resource, at any node: **"`<node>.<resource>`"**, the resource as it stood right after that node.

- When a resource is inspected, the executor inserts one capture dispatch right after the node: a lab compute shader
  that reads the resource (resolving multisampled ones: sample 0, or the average) and writes display colours into an
  inspector texture. No COPY_SRC is needed to show it; a readback (experiments, `--snapshot`) copies the inspector
  texture.
- **Visualizers** chosen by format, overridable per resource (`show=`): colour (exposure, tone map), unorm, normal
  (rgb10a2 or octahedral), depth (reversed-Z, near/far remap), single channel with a ramp, signed (diverging), buffer
  as a per-tile heatmap or list counts (with its `words_per_tile` and `tile`), volume as a slice or projection of a mip,
  depth-view layer. Parameters (channel, range, slice, mip, layer, exposure) are image parameters, as a view kind's are.
- Captures happen in the main encoder only, like timing; panes show the main view's.

### The graph panel

A `PanelMode::Graph` (and a `viewer-graph` snapshot page): the compiled graph laid out by `layered::layout`, left to
right in execution order.

- A node shows its name, the engine pass it mirrors, its GPU time (its timing row; most expensive called out, as the
  performance panel does), and whether it ran this frame (culled or cached by `when`).
- A resource shows its kind (transient, imported, persistent, history: colours from the theme), format, size, and its
  lifetime as a bar across the node columns; aliased resources share a row.
- Hovering a node lights its reads and writes; clicking an edge opens the inspector on `<writer>.<resource>`, in an
  inset like a view kind's, which the viewer can pin.
- A dock tab (`DockTab::Graph`) lists the nodes in order with their timing and the inspector's parameters.

### Stories and experiments

- `{@ image=gbuffer graph=o3deLightCullingCS.lightList show=heatmap image.range=0,32}`: an image whose source is the
  inspector (`ImageSource::Graph`, beside `View` and PNG), validated by `check_image` against the compiled graph.
  `{@ panel=graph highlight=<node>}` opens the panel and lights a node, for the narration to walk the frame.
- Experiments may name an intermediate as a metric's input (`metric.of = "<node>.<resource>"`), so a sweep can measure
  a pass's output, not only the final frame.
- `render-lab --investigation <id> graph` prints the compiled graph (`--dot`, `--state <demo>`; a `--mermaid` export was built, then removed on 2026-11-02), as
  `knowledge` does.

## Phases

Each phase builds both builds clean, keeps every existing snapshot page and selftest passing, and, until phase 5
changes rendering on purpose, keeps frames identical to a HEAD-worktree baseline (decoded-pixel hashes; re-render
O3DE mismatches alone before calling them regressions).

1. **Accurate declarations and the graph as data.** Split a buffer's reads from its writes (it stays bound
   `read_write`), audit every package's lists against its WGSL (o3de_lighting's buffers listed as written by their
   readers; voxelgi's update passes that declare nothing), and add the lint that catches it: from naga's validator
   (`ModuleInfo`'s per-entry `GlobalUse` read/write flags), a resource the entry point reads or writes but does not
   declare so, or one declared but unused. Build `FrameGraph` from `SceneShading` plus the lab's nodes, without
   changing execution; add the `graph` command. Install the skill (move `todo/render-graph-skill.md` to
   `.agents/skills/render-graph/SKILL.md`, link it from `.claude/skills/`, list it in `AGENTS.md`'s Skills), covering
   what this phase provides.
2. **The inspector.** Capture dispatches, the visualizers, `ImageSource::Graph` and its story tag, and a
   `viewer-inspect:<node>.<resource>` snapshot page. Unit tests for the visualizers' mapping (depth remap, octahedral
   decode) against CPU mirrors.
3. **The graph panel.** `PanelMode::Graph`, `DockTab::Graph`, text keys in both languages, `viewer-graph` snapshot
   page; `panel=graph highlight=` in stories.
4. **Execution from the graph.** Replace `encode_scene`'s fixed slots and `dispatch`'s index split with the sorted
   node list; culling; transient lifetimes (and optional aliasing); `Investigation::frame_graph`. Frames identical.
5. **General raster nodes and history.** Raster nodes as data (what they draw, their targets and depth state); the
   scene pass, pre-pass, and depth views become ordinary nodes the lab declares; history resources with ping-pong and
   reset. This is the first phase that may change pixels (only for packages that adopt it).
6. **Investigations mirror their engines' graphs.** Each package splits its frame into the passes its engine runs,
   named after them and linked to the source index: O3DE's pass system maps one-to-one onto nodes; Godot's renderer
   steps onto nodes per feature. Their stories gain a chapter that walks the graph.

## Status (2026-11-02)

Built, both builds clean, unit tests passing, frames identical to the HEAD baseline (333 beat-midpoint frames of
the four timelines; the only differences are a measured CPU-time readout in VoxelGI's panel and viewport noise that
also differs between two runs of the baseline binary):

1. **Declarations and the graph as data.** `lab/src/graph.rs`: `FrameGraph` (nodes, resources, edges, order,
   culling, lifetimes), `lint` (naga's per-entry-point global uses against `reads`/`writes`, run when a shading
   loads; a mismatch refuses it), `render-lab graph` (`--json`, `--dot`). Declarations fixed:
   o3de_lighting's buffers listed as written by their readers, voxelgi's update buffer and the volume its GI pass
   samples. `UpdatePass` lists now take the update's buffers; a chain pass lists the volumes it samples.
2. **The inspector.** `packages/render_lab/shaders/inspect.wgsl` (six entry points), `ImageSource::Graph`, story keys
   `imageGraph`/`imageShow` (Python and Rust), `viewer-inspect:` snapshot page with `?param=` values.
3. **The graph panel.** `PanelMode::Graph`, the Experiments tab's toggle, `viewer-graph` snapshot page, text keys in
   both languages.
4. **Execution from the graph.** `encode_scene` walks the compiled order; consecutive compute nodes share a compute
   pass when untimed; culled nodes are skipped (none of the four investigations has one). `[inspect]` in
   experiment files measures an intermediate.
5. **History.** `SceneShading::history`: chain targets and buffers read across frames; their writers stay live and a
   read before this frame's write is allowed only for them. The scene pass, pre-pass, and depth views are graph nodes.
6. **Engine passes.** The source index's `passes` (`lab`, `engine`, `sources`), written by each package's
   `source_index.py` beside `mappings` (o3de_lighting: 19 nodes, godot_voxelgi: 8); the panel names the pass and
   its first source; `graph` checks them.

Decisions made while building (open to change):

- **History is declared, not inferred.** Inferring it from "read before written" mislabelled O3DE's blur buffers
  (naga counts the discarded side of a `select` as a read). Resources are kept across frames, not ping-ponged: a
  package that needs both frames declares two (as the local package does).
- **A node that reads and writes a resource first** reads what it writes itself (atomics, accumulation); not an error.
- **Order comes from per-resource access order**, as engines' graphs do for multiple writers; a declaration's order
  still matters for nodes that touch the same resource.
- **The update passes run first, in their plan's dispatch order**, not reordered per node (they read nothing the
  frame writes).
- **The panel is a pass-by-resource matrix**, not a layered DAG: it fits the tall side panel and shows lifetimes;
  `--dot` gives the DAG as text; the viewer draws it (below).
- **Story keys are `imageGraph`/`imageShow`**, following `imageFrom`, rather than the plan's `graph=`.
- **No aliasing** of transients: it would make "right after node N" depend on the schedule and saves little here.
- **The inspector captures the main view only** (panes, reference, free camera are not).
- **Experiments measure the inspector's display image** (8 bits), not raw values.
- **`Investigation::frame_graph` was not added**: `SceneShading` already declares everything the investigations
  need, and a second declaration would duplicate it.

Not built, and why:

- **General raster nodes as data** (a package's own fullscreen or mesh raster passes): no investigation has one
  beyond the scene pass, pre-pass, and depth views, and the lab keeps no code that no investigation uses. Build it with
  the first that needs one (its targets need RENDER_ATTACHMENT, its pipeline the chain's group-2 layout).
- **Story chapters that walk each graph**: a chapter goes through the new-story skill's research, outline, and review,
  and rebuilds both editions' narration; that is its own task. The story keys and the panel are ready for it.
- **Godot SDFGI's pass mapping**: its frame is the scene pass alone.

## Open decisions (from the design)

- Whether the inspector captures "after node N" for a resource N later overwrites by copying the resource (exact) or
  by visualizing it in place (cheaper, no copy). The plan uses a capture dispatch, which covers both.
- Whether `Investigation::frame_graph` replaces `scene_shading` once every public package has moved, or both stay.
- How a raster node names what it draws without the lab knowing the investigation (a mesh filter by scene tag, or an
  instance buffer the node's resources provide).

## Since (2026-11-02)

- **The DAG**: each pass a card holding its result, boxed by the package's sections (`graphs/frame.toml`, the resource
  `graph/frame`), in the panel or full screen; dragged, panned, called out by a story's `highlight` (`panel=dag`).
  **Resources**: every resource's card as the frame leaves it (`panel=resources`). Both draw through the shared graph
  view (`lab/src/graph_view.rs`); the longest-path layers of the first DAG (`FrameGraph::layers`) are gone.
- **The Frame graph tab** in the dock: what the hovered or selected pass does (`Investigation::pass_about`, a
  `pass.<entry>` key per pass), its GPU time, and its reads and writes; the Experiments tab's button opens it.
- **`SceneShading::shows`**: how a resource is drawn unless told otherwise.
- **The frame graph scene** in stories (`graph: frame`): the sections as boxes of passes, drawn from the live graph,
  each lit box holding a picture of its result (the frame rendered unseen behind it); its beats move from one focus to
  the next, and its first beat builds it up.
