---
name: render-graph
description: Declare, check, inspect, and demonstrate a Render Lab investigation's frame as a render graph - its nodes (the lab's depth views, pre-pass, scene pass, overlay, and composite; the investigation's update and compute passes) and resources (transient, imported, persistent, history), the order and culling the lab derives from their reads and writes, the lint that checks those declarations against the WGSL, the `graph` command, the inspector that shows any resource as it stood right after any node, the graph panel (passes by resources, GPU time per node, the engine pass each mirrors) its DAG (each pass a card holding its result, `SceneShading::shows`, dragged), and its resources (every resource's card), and the story (`imageGraph`, `imageShow`, `panel=graph`, `panel=dag`, `panel=resources`) and experiment (`[inspect]`) forms that point at intermediate states. Use when adding or changing a pass, target, buffer, volume, or history of an investigation's frame (`SceneShading`), when the lab refuses a shading because its reads and writes do not match its WGSL, when a graph does not compile, when mapping passes to the engine's, when showing an intermediate result in a story or the viewer, or when measuring a pass's output in an experiment.
---

# Render graph

An investigation's frame is a render graph (`lab/src/graph.rs`): **nodes** (passes) that read and write named
**resources**. The lab builds it from the investigation's `SceneShading` (`lab/src/investigation.rs`) and its own
passes, checks it, runs the frame in its order (`Renderer::encode_scene`), times every node, and can show any
resource right after any node. Declare the frame so it mirrors the engine's passes: the graph is a demonstration
of its own.

## 1. What a frame's graph is made of

Nodes, in declaration order:

| Node | Name | From |
|---|---|---|
| update passes | their entry points | `SceneShading::update` (run when their plan's key changes; their dispatches in the plan's order, before the rest) |
| program passes (`Start`) | their names | `SceneShading::program`, below |
| depth views | `depthViews` | `SceneShading::depth_views` (drawn when what they see changes) |
| pre-pass | its fragment entry | `SceneShading::prepass` (writes `depth` and `prepass_outputs`; reads nothing of group 2) |
| program passes (`AfterPrepass`) | their names | `SceneShading::program` |
| early compute passes | their entry points | `SceneShading::early` |
| program passes (`BeforeScene`) | their names | `SceneShading::program` |
| scene pass | its fragment entry (`scene` without a shading) | the scene's meshes into `outputs` (or `hdr`, with none), MSAA |
| compute passes | their entry points (a repeated entry: `name#2`) | `SceneShading::passes` |
| overlay, composite | `overlay`, `composite` | the lab's |

Resources:

| Lifetime | What | Read before this frame writes it |
|---|---|---|
| imported | the lab's: `hdr` (what the frame shows), `depth` (4x MSAA, reversed Z), `overlay`, `frame` | an error |
| transient | the chain's `targets` and `buffers` | an error, unless the reading node also writes it |
| persistent | `volumes`, the update's buffers, the depth views, a program's images and buffers | the last time it was computed |
| history | targets or buffers named in `SceneShading::history` | the frames before (kept, zero when the viewport's size changes) |

The order: for each resource, its accesses keep their declared order (a writer before the readers after it, those
before the next writer); nodes with no path between them keep their declared order. A compute node whose writes
nothing live reads is **culled**: live are the composite, cached nodes, writers of history, writers of what an image
kind reads (`reads=`), and the inspected resource's writer. General raster nodes declared as data are not built: the
scene pass, pre-pass, depth views, and a program's raster passes are the raster nodes an investigation has.

A chain without `outputs` but with `prepass_outputs` (a depth and normal pre-pass, Godot's normal-roughness buffer)
is the pre-pass and its `early` passes before an ordinary scene pass, which writes `hdr` and may read what they
wrote (`scene_reads`); it has no `passes` after the scene.

**Programs** (`SceneShading::program`, `lab/src/program.rs`, todo/pass-programs.md) are an engine's own GPU work that
the chain cannot express: images (2D, 2D arrays, 3D; integer formats; `count` elements, bound one at a time or as a
binding array) and buffers (indirect ones too), compute and raster passes, and a plan (CPU, with its own state) that
returns each frame's commands in order: uploads, clears, copies, dispatches (direct or indirect, each with 16 words
of parameters at group 2), and draws of the scene's meshes into a framebuffer of its own size. A pass's `uses` are
its group-3 bindings (`Which` picks an element: the command's `instance`, a fixed one, a table's, or all); each pass
is a node whose reads and writes come from its uses, always live (it writes what outlives the frame). The scene
pass and the chain's compute passes read `scene_uses` at group 3 (in a shading without volumes). The plan runs in
the main view only. Godot's SDFGI (packages/godot_sdfgi, `gpu.rs`) is one.

## 2. Declaring reads and writes

`ShadingPass::reads` and `writes` (and `UpdatePass`'s) are the graph's edges, so they say what the entry point
does:

- A buffer it reads is in `reads`, one it writes in `writes`, both if both. Every buffer is bound `read_write`
  either way (`pipelines::chain_binding`); the lists, not the binding, say which.
- A volume a chain pass samples (group 3) is in its `reads`. An update pass lists the volumes it samples and the
  update's buffers it reads in `reads`, the volumes and buffers it writes in `writes`.
- The dispatch covers the first of `writes` (a buffer: the viewport): keep it first when editing.
- Read last frame's data from a target or buffer only through `SceneShading::history`.

A program pass's `uses` are checked the same way: every group-3 binding its entry points use is declared, a write
only where the access writes, texture atomics only where it is `Atomic`, and every use used.

The lab checks this when it loads the shading (`graph::lint`, from naga's per-entry-point global uses) and refuses
it on a difference: `o3deMergeCS: reads buffer ssaoFull but does not list it in its reads`, or `lists X in its
writes but does not write it`. A use only through `arrayLength`/`textureDimensions` counts as declared either way.
naga counts a read the code makes even when its value is unused (a `select` of two buffers reads both): list it.

The bindings do not change: target `i` is `2i` read, `2i + 1` written; `depth` `2N`; `hdr` `2N + 1`; buffer `k`
`2N + 2 + k`; the depth views after the buffers (`lab/src/investigation.rs`, `ShadingPass`).

## 3. Checking it

```sh
cargo run --release -- --investigation <id> graph                   # nodes in order, reads and writes, resources and their lifetimes, culled nodes, engine passes
cargo run --release -- --investigation <id> graph --json            # the same, and the problems, as JSON
cargo run --release -- --investigation <id> graph --dot g.dot       # the graph in Graphviz's language
```

It fails on a compile error (an unknown name, a read before any write, a cycle, a name listed twice), on a lint
problem, and on a source-index pass that names no node. A note lists writes nothing after reads: an image kind or the
inspector may want them; otherwise they are dead work.

Restructuring must keep frames identical: render the timelines' beat midpoints with a HEAD-worktree build and with
yours and compare decoded pixels. Some frames vary from run to run (a measured CPU-time readout, some viewport noise):
re-render a mismatch alone with both binaries before calling it a regression.

## 4. The inspector

`<node>.<resource>` is the resource as it stood right after that node, whatever later nodes write over it. The
renderer draws it right after the node, in the main view only, with `packages/rlstd/shaders/inspect.wgsl` (one entry
per kind of resource: texture, multisampled texture, multisampled depth, depth array, buffer, volume, and a
program's integer volumes and 2D arrays), by a visualizer
(`graph::Show`) its format chooses or `imageShow` names:

| Show | For | Parameters (`image.<key>=`) |
|---|---|---|
| `color` | HDR colour (default for float targets, volumes) | `exposure` (stops), `channel` |
| `unorm` | stored values as they are (default for unorm targets) | `channel` (0 all, 1 r, 2 g, 3 b, 4 a) |
| `normal` | normals | `encoding` (0 signed xyz, 1 unsigned, 2 octahedral) |
| `depth` | reversed-Z depth on a log ramp (default for depth) | `range` (default 0.001, 1) |
| `ramp`, `signed` | one channel; diverging around 0 | `channel`, `range` |
| `count` | a buffer word as a count, 0 black (default for buffers of more than one word a tile) | `word`, `range` (default 0, 64) |
| `float` | a buffer word read as f32 (default for one word a tile) | `word`, `range` |
| `hash` | ids, stencil references | `channel`, `word` |

Every show also takes `average` (1: a multisampled resource's samples averaged, else sample 0); a volume takes
`axis` (0 x, 1 y, 2 z), `slice` (0..1), and `level` (its mip); a depth array takes `level` (its layer). A per-tile
buffer is drawn over the screen (each pixel its tile's `word`); an update buffer's elements row by row. A program's
images use their kind's entry (integer 3D images, 2D arrays of floats or integers too: `level` is the layer); an
integer image shows its word (`hash` by default, `signed` for sint), with `bits` (first bit, width) for a bit field
and `encoding=3` to decode it as RGB9E5 under `color`; `index` picks an element of an image with `count`; a
program's buffer shows its words row by row.

```sh
cargo run --release -- home --snapshot inspect.png --page "viewer-inspect:o3deLightCullingCS.tileData/count?range=0,8&word=3"
cargo run --release -- --investigation godot/voxelgi home --snapshot vol.png --page "viewer-inspect:voxelgiWriteCS.voxelgiLight/color?slice=0.05&exposure=2"
cargo run --release -- --investigation godot/sdfgi home --snapshot light.png --page "viewer-inspect:sdfgiDynamicLight.lightData/color?encoding=3&slice=0.4&index=1"
```

## 5. The graph panel

The dock's **Frame graph** tab (or the Experiments tab's **Open the frame graph**) shows the graph in the panel, the
DAG first; its header switches between **Grid**, **DAG**, **Resources**, and **Full screen**. The tab describes the
pass under the pointer in the panel, else the selected one (the inspected result's): the engine pass, what it does,
its GPU time, and what it reads and writes (a click inspects that resource right after it), beside every pass in
order. What a pass does is the investigation's text: `Investigation::pass_about` returns a key of its package's table
per entry point (`pass.<entry>`, both languages), the lab's own nodes have theirs (`graph::about`); give every new
pass one, accurate to its WGSL.

`PanelMode::Graph` (the **Grid**; a story's `panel=graph`): nodes top to bottom in
execution order, resources as columns, ■ where a node writes, ○ where it reads, each resource's lifetime a line
coloured by its kind; each node's GPU time (its timing row; two nodes of one entry share it). Hover a mark to name it
and the engine pass; click one to inspect it in the top-right inset (the image `graph`), its node called out; click it
again to hide it. `highlight=<node>` calls nodes out.

```sh
cargo run --release -- home --snapshot graph.png --page viewer-graph        # the inspector on what the costliest pass writes; look at it
```

**The DAG** (`PanelMode::Dag`: the panel's **DAG** button; a story's `panel=dag`) draws the same graph as cards in
the wider panel, laid out by the lab's graph view (`lab/src/graph_view.rs`: layers by the longest path to a node,
ordered to keep arrows from crossing, each section of `graphs/frame.toml` a box around its passes), top to bottom;
**Full screen** shows it over the viewer, left to right (Esc closes it). Each card is a
node: its name, its GPU time, and its result as it stood right after it (`FrameGraph::result_of`: its first write,
the one its dispatch covers; the composite's, the `hdr` it reads), drawn by the inspector at 256 texels. An arrow
runs from a writer to each reader of what it wrote, coloured by the resource's lifetime; a faint one is an order
without data (a later write of what the first used). Hover a card to name its arrows' resources and its engine pass;
click it to inspect its result in the inset. In the viewer, a drag on the background pans the view, the wheel zooms it (about the pointer, out to the whole graph), a
drag on a card moves it (its arrows follow), and a double-click on the background (or the header's **Reset nodes**) puts both back; a story always
shows the layout as computed. `highlight=<node>` calls nodes out and moves the view to them, so a story walks the
frame by moving `highlight`. A culled node's card says so (its result is
never drawn).

**The shader source.** Full screen, a selected pass's **Show shader source** splits the screen: the graph on the
left, on the right the WGSL its pipelines are compiled from (`lab/src/shader_source.rs`: the module as the lab composes
it now, cut to the pass's entry points and every function, struct, constant, and binding they use, entry points
first, each piece under its file and line), drawn by the lab's code view (`lab/src/code_view.rs`, as the Code tab's
pseudo-code is). It is read-only. The viewer reloads shaders live: about twice a second it reads again every WGSL file
of the checkout that changed and makes every pipeline again (`Renderer::reload_shaders`); a reload that does not
compile keeps the last good pipelines and says why, over the viewport and in the source pane. A change to a tagged
value or a struct the lab lays out in Rust still needs a relaunch.

How a card, or a graph image without `imageShow`, shows a resource is the investigation's choice in
`SceneShading::shows`, `(resource, "<show>[?<param>=<value>&…]")`, else its format's (`graph::default_show`). Pick
one for every buffer whose words are not counts: a packed index, a float, a depth.

```rust
shows: &[("tileData", "count?word=3&range=0,8"), ("lightList", "hash"), ("linearDepth", "float?range=0,12")],
```

**The resources** (`PanelMode::Resources`: the panel's **Resources** button; a story's `panel=resources`; full screen
too) are every resource of the frame as a card: its name, its state as the frame leaves it (right after its last
writer that runs every frame, `FrameGraph::last_user`; a resource only a cached node writes, after its last reader),
its lifetime and shape. Hover a card for its writers and readers; click it to inspect it. `highlight=<node>` lights
what those nodes read and write. It pans and its cards move as the DAG's.

```sh
cargo run --release -- home --snapshot res.png --page "viewer-resources[-full][:<demo>][@<node>,…]"   # every resource's card
cargo run --release -- home --snapshot dag.png --page "viewer-dag[:<demo>][@<node>,…]"        # the DAG in the panel, those nodes called out
cargo run --release -- home --snapshot dag.png --page "viewer-dag-full[:<demo>][@<node>,…]"   # the DAG full screen
```

The engine pass a node mirrors comes from the package's source index: `passes` (`lab` the node's name, `engine` the
pass, `sources` its entries), written by the package's `source_index.py` beside `mappings`. Add a node's pass there
when you add the node; the `graph` command checks that each names a node and cites entries that exist.

## 6. In a story

```text
{@ panel=graph highlight=o3deLightCullingCS}
{@ image=lights imageGraph=o3deLightCullingCS.tileData imageShow=count image.range=0,16}
{@ image=lights image.word=1 blend=1}
{@ panel=dag highlight=o3dePrepassFS}            the DAG, the pre-pass's card called out
{@ highlight=o3deLightCullingCS}                 the next step: the view scrolls to it
```

A 2D beat's `graph: frame` presents the frame graph as a scene of its own, drawn by the lab from the live graph
as the package's `graphs/frame.toml` (the resource `graph/frame`) groups it (sections of passes or resources, each
a box; dashed, what the frame does not run; `tools/video/frame_graph.py` has the format); the DAG panel boxes its
cards by the same sections. Consecutive beats of the scene move from one focus to the next (no fade), and its first
beat builds it up, layer by layer (`lab/src/diagram.rs` `Transition`). Its `focus:` lights a section, or a pass or resource
of one (its row). The lab renders the beat's 3D frame unseen behind the scene, and a lit box holds a picture of
what it makes (its last pass's result, its last resource as the frame leaves it, or the section's `picture =
"<pass>.<resource>"`, drawn as `SceneShading::shows` says); when the pictures would take the labels below 24 px the
view stays at 24 px on the lit boxes, and when even those do not fit, no box shows its picture. Pick a section's
`picture` when its last pass writes bookkeeping (an index list) rather than what the section is for. Every pass of
the frame must be in a section (the lab's overlay and composite need not be), so
adding a pass means adding it to a section: the timeline does not load otherwise.

```text
## c1-map-cull | 2d | Then the lights are sorted into tiles
graph: frame
focus: DEPTH, CULL
```

`imageGraph` makes a graph image (beside `imageFrom`, an image kind's instance, and published or PNG images); a later
key of the name sets its parameters (blended) or its `imageShow`. The lab checks the node, that it reads or writes the
resource, the visualizer, and each parameter when it loads the timeline. Walk a frame in its order and show each
node's output as the narration reaches it, changing the scene with each step, rather than describing passes.

## 7. In an experiment

```toml
[inspect]
of = "o3deSsaoUpsampleCS.ssaoFull"   # <node>.<resource>, right after the node
show = "float"
values = { range = [0.0, 1.0] }
```

With `[inspect]`, a run measures (and writes) the inspector's drawing of that resource, not the frame: PSNR, SSIM,
ꟻLIP against the reference's same resource, in display values (8 bits a channel; relMSE over those too). GPU time is
per node as always.

## Pitfalls

- A pass that lists a buffer it only reads under `writes` makes a false edge and a false "writes": the lint refuses
  it. Fix the lists, not the lint.
- A pass that reads a chain target before this frame writes it needs `history`; otherwise the graph refuses it.
- Two nodes of one entry (`name`, `name#2`) share one timing row.
- The inspector shows the main view; panes, the compare reference, and the free camera are not captured.
- Depth is reversed (near 1, far 0): `depth` draws near bright and the background black.
- The overlay discards the depth buffer when it ends: inspect `depth` after the pre-pass or the scene pass, not
  after `overlay`.
- A shading that stops computing once the view holds still (it keeps a converged history) leaves its
  intermediates stale or black there: they are what the frame computed, not what the history shows.
- A buffer of one word a pixel is drawn as `float` (0..1) and one of more as `count`: a card of flat colour usually
  means the words are something else; declare its view in `SceneShading::shows`.
- After changing the lab, rebuild each local package's workspace, or its library does not load.
