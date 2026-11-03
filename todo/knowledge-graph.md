# Knowledge graph, target audience, and the Prerequisites tab (implemented 2026-11-01)

The investigations explain high-level rendering algorithms, and each one assumes a lot of prior knowledge. This plan
adds four things:

1. **A knowledge graph.** It is a DAG of the knowledge the chapters need, worked back to three bases. It is package
   content, so investigations add to it.
2. **The target-audience system.** This is an agent skill, not lab code. It picks the level an investigation is aimed
   at: an antichain vertex separator of the graph. Everything before the separator is what the viewer should know.
3. **A Prerequisites tab** in the viewer's dock, listing the nodes before the separator.
4. **A knowledge-graph panel** in the viewer that draws the graph.

The draft graph (83 nodes, 138 edges) and each chapter's requirements are in [knowledge-graph-tree.md](knowledge-graph-tree.md).

## The three bases

These are the only nodes without requirements. Every other node reaches at least one of them.

| Key | Base | Covers |
|---|---|---|
| `gpu` | Fundamental understanding of the GPU | The GPU's execution model: invocations in lockstep subgroups, workgroups, latency hiding, divergence |
| `cg` | Fundamental understanding of computer graphics | Radiometry: solid angle, radiant flux, irradiance, radiance |
| `rt` | Fundamental understanding of real-time rendering | The graphics pipeline: rasterization and attribute interpolation |

The bases were narrowed on 2026-11-01, after a review against *Real-Time Rendering* (4th edition), *Physically Based
Rendering* (3rd edition), *Computer Graphics: Principles and Practice*, the Vulkan and CUDA guides, and the papers the
topics cite. Queues and command submission (`gpu-queues`), the rendering equation (`cg-rendering-equation`),
projection (in `rt-spaces`), and texture filtering (`rt-sampling`) became topics of their own, because the books treat
them after the bases and a viewer can know a base without them.

## The mathematics

**The graph.** The graph is G = (V, E), with an edge u → v when understanding v requires u. It must be acyclic. Its
only sources must be the three bases. It must be **transitively reduced**: no edge u → v may exist where another path
from u to v already does. The loader rejects a redundant edge, because "directly depends" has to mean an edge in the
reduced graph.

**One investigation's graph.** Each chapter is a sink node whose requirements are the concepts it uses directly. Let T
be the investigation's chapter nodes. Its graph is G_T, made of T and all of T's ancestors.

**The literal definition.** Partition G_T into A ⊔ S ⊔ B with these properties:

- S is an antichain.
- No edge joins A and B.
- The bases are in A, and T is in B.

The Prerequisites are then A, the nodes before S.

**Measured on the draft graph, this degenerates.** A node with requirements at different depths can't be placed in B
while one requirement is in A and another is not. For example, a chapter that uses the depth buffer directly and also
traces a distance field. The smallest valid A, starting from the bases or from a typical shader programmer, swallows
45 of 56 nodes for SDFGI and 55 of 63 for VoxelGI, leaving S = the chapters themselves. Such skip-level requirements
are not mistakes: stories do use basic tools directly next to advanced ones.

**The definition used: the separator in the graded subdivision.** This is the standard step from layered graph drawing.
Every edge that skips levels is split into a chain of *carry* vertices, one per level crossed. A carry of `u` at level
k means "u, still used directly at level k". In this subdivided graph the literal definition holds exactly, and it
reduces to a simple test on G_T itself:

> Choose a set A that is closed under requirements (a downset), contains the bases, and contains no chapter. Then
> S = the minimal nodes of G_T ∖ A, plus the carries of the edges that leave A for nodes that are not minimal.
> S is an antichain separator of the subdivision, and every such separator arises this way.

Here is why S is an antichain. Two minimal elements are incomparable. A carry on a → b is below a real s only through
b ≤ s, which minimality forbids. It is above s only through s ≤ a, which A being a downset forbids. Two carries are
comparable only if one edge's head lies in A, which can't happen.

The real nodes of S form **the level**. The carries are **known topics used directly above the level**. The Prerequisites
tab marks them, because a chapter leans on them without explaining them. B, apart from its carries, is **what the story
teaches**.

**Choosing A is the target-audience decision.** The agent describes the audience, maps the description to the nodes
it knows (K), and takes A = K ∪ ancestors(K), restricted to G_T. The tool ranks candidate levels by the number of
carries, since fewer carries make a cleaner cut. A sample run on the draft graph uses an audience who has written a
PBR forward renderer with shadow maps and compute shaders:

| Investigation | Relevant nodes | Known (A) | Level (S, real) | To teach | Carried |
|---|---|---|---|---|---|
| O3DE lighting | 73 | 26 | 23 | 15 | 12 nodes |
| Godot SDFGI | 56 | 24 | 10 | 12 | 12 nodes |
| Godot VoxelGI | 63 | 25 | 12 | 18 | 14 nodes |

For example, the SDFGI level is the following: clipmaps, distance fields, octahedral maps, spherical harmonics, HDR
packed formats, atomics, the frame as a pass graph, low-discrepancy samples, environment lighting, and reconstruction
from depth. The story teaches irradiance, distance transforms, sphere tracing, voxelization, probes, and their
tracing.

**One audience per investigation.** The separator is computed over the union of the chapters, so the whole video has
one level. The tab then filters it to the current chapter or demonstration: A ∩ ancestors(context).

## Data: `knowledge.toml` in packages

This uses an ordinary `kind = "file"` resource with id `knowledge`, so no new pipeline kind is needed. A package that
has one exports it (`export = ["knowledge"]`).

```toml
# packages/render_lab/knowledge.toml: the bases and the engine-neutral nodes
[[topic]]
key = "geo-sdf"
title-text = "topic.geo-sdf.title"       # text keys, checked by lab/check like every `-text` field
summary-text = "topic.geo-sdf.summary"   # one or two sentences: what knowing it means
area = "geo"                             # the panel's grouping: gpu, rt, geo, cg, light, gi
requires = ["geo-grids"]
reading = ["Hart 1996, Sphere tracing"]  # optional: published references (books, papers, talks)

[[topic]]
key = "gpu"
base = true
title-text = "topic.gpu.title"
summary-text = "topic.gpu.summary"
area = "gpu"
```

```toml
# packages/<investigation>/knowledge.toml: its chapters, and the audience the agent skill chose
[audience]
profile = "Has written a PBR forward renderer with shadow maps and compute passes."   # notes, not on screen
known = ["render_lab.knowledge#rt-shadow-maps", "render_lab.knowledge#cg-pbr-materials", "..."]          # K; A is its closure

[[chapter]]
number = 5
demos = ["field"]                                   # the demonstrations that show it (for the tab outside a timeline)
requires = ["render_lab.knowledge#geo-distance-transform"]

[[topic]]   # optional: an engine-neutral node only this package needs, until it is worth moving to render_lab
key = "..."
```

- **References into a file.** `render_lab.knowledge#geo-sdf` resolves `render_lab.topics` through `packages::resolve`, which
  checks the import and the export, and then names a node inside it. This is a small, generic addition to the loader:
  a fragment that names a record inside an exported file resource.
- **A local package's topics.** Engine-neutral topics its chapters need go in render_lab's graph, named by the published
  technique and never by an engine (decided 2026-11-01). Only what is one engine's own design stays in its own file.
- **The stored audience.** `known` is stored, not A or S, so editing the graph recomputes the level. The check
  reports when an edit moves the level by more than a few nodes.

## Lab code (`lab/src/knowledge.rs`)

None of this is investigation-specific. The lab reads the `knowledge` resource of the current investigation's package
(`Investigation::package`) and of the packages it imports. No change to the `Investigation` trait is needed.

- **Loading.** Parse and merge the nodes in import order. Resolve the references. Validate:
  - the graph is acyclic;
  - render_lab has exactly the three bases, and no other package declares a base;
  - every node reaches a base, and every edge is non-redundant;
  - no key is unknown and no reference fails to resolve;
  - every chapter number is one the timeline has;
  - `known` is closed or closable, and contains no chapter.

  Errors go through the loader's existing error path.
- **Partition.** Given the chapters and `known`, compute A, S (real nodes and carries), and B. Also compute the
  ancestor filter for a chapter or a demonstration.
- **Layout for the panel.** Do a layered layout once at load time:
  - rank by the longest path from the bases (as built: ranks are columns, bases on the left, which suits the
    tall side panel and the text-heavy nodes);
  - add dummy vertices on long edges (the same carries);
  - order each layer with a few barycentric sweeps;
  - make it deterministic, with node sizes from the measured title text.
- **CLI**, next to `values`, `choices`, and `images` (as built: one command with flags):
  - `cargo run --release -- --investigation <id> knowledge` checks the merged graph and the stored audience (it fails on
    a problem) and prints the split: the level, the carries, what to teach, and each chapter's line.
  - `--known a,b,…` tries another audience; `--levels` lists the neighbouring candidates (one level node added or
    one maximal known node removed), ranked by carries.
  - (`--mermaid <path>` wrote the diagram; removed on 2026-11-02 with every Mermaid export.)
- **Unit tests** in `cargo test --workspace`:
  - the reduction of a hand-made graph with a skip edge;
  - the antichain and separation properties checked on the subdivision for random downsets;
  - the public packages' graphs load and validate.

## The Prerequisites tab

- Add `DockTab::Prerequisites` (`lab/src/ui.rs:210`) and the label `controls.prerequisites` (render_lab `text/en.json`
  and `zh-CN.json`).
- Add the button to `Dock::buttons`'s array (`lab/src/controls.rs:121`).
- Draw it full width like Sources: a match arm at `controls.rs:93` (skip the scroll area) and at `controls.rs:112`.
- **Context.** In a timeline, the current chapter (`tl.chapter_at`, `lab/src/shot.rs:864`). Otherwise, the chapters
  whose `demos` include the current demo (`state.demo`). If no chapter lists the demo, use the whole investigation.
- **Content, top to bottom:**
  - **Header.** One line naming the audience level (the real nodes of S in this context), as a row of chips.
  - **"You should know".** The nodes of A ∩ ancestors(context), grouped by base area and ordered from the level down
    (the nearest first). Each row has its title and its summary. Rows for carried topics get a mark, "used directly
    here".
  - **Footer.** "Taught in this chapter": the B nodes in context, as chips, so the viewer sees what the chapter adds.
- **Interaction.** A click on a row or chip opens the knowledge-graph panel focused on that node. The list scrolls.
- No investigation code is involved: the tab reads the partition from `topics.rs`.

## The knowledge-graph panel

- Add `PanelMode::Knowledge` (the side panel, `draw_panel`, `lab/src/ui.rs:770`). Open it with `ViewerAction::SetPanel`
  from a button in the Prerequisites tab, or by clicking a node there.
- **Pan and zoom.** (As planned: reuse `DiagramView` and `fit_rect`; as built, a view of its own; since 2026-11-02
  the lab's shared graph view, `lab/src/graph_view.rs`, which every graph's panning, zooming, and dragging go through.)
  Double-click to reset. Add a `knowledge_view()` guard next to `paused_diagram()` so the panel gets the pointer.
- **Drawing.**
  - Nodes are rounded boxes with their translated titles, and the three bases sit along the bottom.
  - Edges are polylines through the layout's dummy points, with arrowheads.
  - The base area is drawn as a faint column band.
- **Colour by role** (theme palette roles, one encoding each, legible in both themes):
  - known (A);
  - level (S);
  - to teach (B);
  - outside this investigation (dimmed).

  The current chapter's ancestors are drawn at full weight and the rest faded.
- **Hover** shows the summary and the chapters that use the node. A **click** selects the node and highlights its
  ancestors and descendants.
- **Size.** At about 80 nodes the panel opens fitted to the current chapter's subgraph, and zooming out shows the
  whole graph.

## The agent skill: `.agents/skills/knowledge-graph/SKILL.md`

The skill is published, with a link `.claude/skills/knowledge-graph` like the others. It covers both systems.

1. **The graph's rules.**
   - One concept per node, named engine-neutrally.
   - Requirements are the direct prerequisites only (the loader rejects redundant edges).
   - The three bases are fixed.
   - Every title and summary is in both languages.
   - Reading lists cite published work.
   - Engine-specific details belong to chapters, not nodes.
   - A local package's engine-neutral topics go in render_lab's graph, named by technique; only its engine's own design
     stays in its file.
   - When to split a node: when stories need half of it at one level and half at another.
2. **Listing a story's requirements.** Go chapter by chapter from the outline and storyboard. Record what the chapter
   *uses*, as nodes, and add any missing nodes with their requirements down to existing ones. Then run `knowledge`.
3. **The target-audience procedure.**
   1. Write the profile in one sentence.
   2. Choose K, the nodes such a viewer knows.
   3. Run `knowledge --known K --levels` and read the level, the carries, and what to teach.
   4. Try the neighbouring candidates. Prefer the one with the fewest carries that does not move the profile.
   5. Record `[audience]`.
   6. Run `knowledge`.
4. **What the level means for the story.**
   - A nodes are not explained.
   - Each S node gets a one-sentence reminder the first time it is used.
   - Each B node gets a beat that teaches it, with a demonstration (the 32-animations rule applies), before its first
     use.
   - Carried nodes are named as tools on screen.
   - Reviewers check each chapter against its partition.
5. **Commands and their output**, and the snapshot pages to look at.

`new-story` gets one line in Phase 2 (outline: run the target-audience procedure and plan a beat for every B node)
and one in Phase 6 (review against the partition). `packages` gets `knowledge.toml` in its list of conventional file
resources.

## Phases

Validate each phase as AGENTS.md describes before starting the next. Run one build at a time.

1. **Graph content.**
   - `packages/render_lab/knowledge.toml`: the 83 draft nodes, reviewed and edited.
   - Titles and summaries in `text/en.json` and `zh-CN.json`.
   - `knowledge.toml` for `o3de_lighting`, `godot_sdfgi`, and `godot_voxelgi`, with chapters and `demos`.

   Subagents (Sonnet) check each chapter's requirements against its storyboard. The main model writes the summaries.
2. **Loader and CLI.** `topics.rs` (parse, merge, fragment references, validation, partition, levels), the
   `knowledge` commands, and the unit tests. Then `cargo build` and `cargo test`.
3. **Target audience.** Write the agent skill. Run it for the three public investigations and record `[audience]`. Report
   each level and its "to teach" list against what each story actually teaches; gaps go into the stories' own todo
   files, not here.
4. **The Prerequisites tab.** Add the tab, its text keys, and the snapshot page `--page viewer-prerequisites[:demo]`
   (modelled on `viewer-experiments`, `home.rs:1326`). (Not built: `viewer-timeline-prerequisites`.)
5. **The panel.** Add the layout, drawing, and input, and the snapshot page `--page viewer-knowledge[:demo]`, fitted and
   then zoomed onto a node. Look at both themes.
6. **Documentation.** The AGENTS.md validation list (the `knowledge` commands and the two pages), the README (the tab and
   the panel), and the `cli.rs` help text. Check the public build without the optional module.

## Decisions

- **2026-11-01: the proposal is accepted as written.** This covers three things:
  - The separator is taken in the graded subdivision: carries mark known topics used directly above the level.
  - The tab lists only the nodes before the separator, and shows the level in its header.
  - All three public investigations share one audience: a PBR forward renderer, shadow maps, and compute passes.
- **2026-11-01: implemented** (`lab/src/knowledge.rs`, the Prerequisites tab in `lab/src/controls.rs`, the panel in
  `lab/src/ui.rs`, the `knowledge` command, the `knowledge-graph` skill). The panel's interaction is egui's (an interactive
  area over the graph keeps its view in egui's memory), so the viewer's pointer code is unchanged.
- **2026-11-01: renamed** from the skill graph to the knowledge graph (a node is a topic), so it is not confused with
  agent skills.
- **2026-11-01: engine-neutral topics of local packages go in the global graph**, named by technique, never by engine.
- **2026-11-02:** the panel and its full screen draw through the shared graph view (`lab/src/graph_view.rs`, from
  `knowledge::Graph::view`): the same layout, with its areas and groups as clusters; its topics can be dragged; the
  `knowledge --mermaid` export is gone.
