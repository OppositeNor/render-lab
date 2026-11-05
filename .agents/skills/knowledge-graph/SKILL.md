---
name: knowledge-graph
description: Use and extend Render Lab's knowledge graph and choose an investigation's target audience. The graph is a DAG of the graphics knowledge the chapters need, down to three bases, in render_lab's knowledge.toml; each investigation's knowledge.toml lists its chapters' direct requirements and its `[audience]`, an antichain vertex separator of the graph (what the viewer should know, the level the story starts from, what it must teach). Use when adding or editing topics, groups, or chapter requirements, choosing or checking an audience (`render-lab knowledge --known --levels`), checking a series' parts, or working on the Prerequisites tab or the knowledge panel. How a story costs and teaches its level is `new-story`'s.
---

# The knowledge graph and the target audience

The investigations explain high-level algorithms that assume a lot. The **knowledge graph** says what, all the way down;
the **target audience** says where an investigation starts, so its story explains what is beyond that and nothing
below it. The code is `lab/src/knowledge.rs`.

## The graph

- **Nodes** are `[[topic]]` tables in `packages/render_lab/knowledge.toml`: `key`, `title-text` and `summary-text` (keys of
  render_lab's `text/en.ftl` and `text/zh-CN.ftl`, `topic-<key>-title` and `topic-<key>-summary`), `area` (`gpu`,
  `rt`, `geo`, `cg`, `light`, `gi`: the panel's and the tab's grouping), `requires`, and optional `reading`
  (published references: papers, books, talks, cited by their publication; a book by chapter and section, as
  `Akenine-Möller et al. 2018, Real-Time Rendering, 4th edition, chapter 7 (Shadows), §7.5`, naming the edition).
- **Groups** are `[[group]]` tables (`key`, `title-text` as `group.<key>.title`, `area`): subsections of an area
  that gather related topics ("Shadow maps", "Distance fields", "Monte Carlo"). A topic joins one with
  `group = "<key>"` (a group of its own area; another package's as `render_lab.knowledge#<key>`). The panel draws each
  group as a box around its topics, each in a band of its own across the columns it spans (the lab's shared graph
  view, `lab/src/graph_view.rs`). Groups say nothing about requirements: give a new topic the group a reader would
  look in, or none when no group fits.
- **The three bases** are the only nodes without requirements (`base = true`): `gpu` (the execution model:
  invocations, subgroups, workgroups, latency hiding, divergence), `cg` (radiometry: solid angle, flux, irradiance,
  radiance), `rt` (the graphics pipeline: rasterization and interpolation). Queues, the rendering equation, projection,
  and texture filtering are topics of their own. Every other node reaches at least one. Do not add a base.
- **An edge is a direct requirement**: understanding the node needs that topic directly. The loader rejects a
  cycle and a **redundant edge** (one another path already implies): the graph is transitively reduced, because
  "directly requires" must mean an edge for the separator to mean anything.
- **One concept per node, engine-neutral, named as the textbooks name it.** Titles and Chinese terms follow
  *Real-Time Rendering* (4th edition; the Chinese translation for Chinese terms), *Physically Based Rendering*, and
  the seminal papers; a topic's chapters in those books are its reading. Engine specifics (O3DE's tile size, Godot's cascade count) belong to the
  chapters, not to nodes. Split a node when stories need half of it at one level and half at another (the loader's
  partition shows this: a node whose requirements are partly known and partly not).
- **A package's own nodes.** An investigation may declare `[[topic]]` tables in its own `knowledge.toml` and name
  render_lab's as `render_lab.knowledge#<key>` (render_lab exports `knowledge`); move a node to render_lab once another investigation needs
  it. **A local package's engine-neutral topics go in render_lab's graph** (decided 2026-11-01): a published technique named
  by what it is ("per-mesh distance fields", "screen-space probe gather"), never by an engine, with no engine named in
  its summary or reading. Only what can be described solely as one engine's design stays in the local package's file,
  or in its chapters.

## An investigation's chapters

`packages/<investigation>/knowledge.toml` (a `kind = "file"` resource with id `knowledge` in its collection):

```toml
[audience]
profile = "Has written a PBR forward renderer with shadow maps and compute passes: …"   # a note, not shown
known = ["render_lab.knowledge#rt-shadow-maps", "render_lab.knowledge#cg-pbr-materials", "…"]

[[chapter]]
number = 5                                         # the storyboard's chapter number
demos = ["field"]                                  # the demonstrations that show it (the tab outside a timeline)
requires = ["render_lab.knowledge#geo-distance-transform"]
```

List what each chapter **uses** directly, as nodes: read its outline section and storyboard chapter, and for every
concept a sentence leans on, name the node (add it to render_lab with its requirements down to existing nodes if it is
missing). A requirement already implied by another of the chapter's is rejected; keep the higher one. Leave recap
chapters out (they require nothing new).

Delegate the listing per investigation when your environment has subagents (a cheaper model is enough: AGENTS.md),
with the outline, the storyboard, and the node table (the `knowledge.toml` files);
otherwise list it yourself, a chapter at a time. Check the result with the `knowledge` command either way.

## The target audience

The audience is a set A of known topics, closed under requirements, containing the bases. Restricted to the
chapters' ancestors:

- **The level** S: the topics outside A whose requirements are all in A. They are an antichain (no two depend on
  each other). The story recalls each in a sentence the first time it uses it.
- **Carries**: edges from A to a topic beyond the level (or to a chapter): known topics used directly above the
  level. The tab marks them (↗); the story names them as tools, without explaining them.
- **What the story teaches**: everything else the chapters need. Each is taught before its first use, like a
  teacher: a sequence of beats with a demonstration of its own, animated and explorable (`new-story`,
  `references/outline.md`, "Teaching a topic").

With every level-skipping edge subdivided, S and the carries form an antichain vertex separator between A (what
you should know: the Prerequisites tab) and the rest (what you might not know).

**Why the subdivision.** Let G_T be the chapters T (sinks) and their ancestors. The literal definition, a partition
A ⊔ S ⊔ B with S an antichain, no edge between A and B, the bases in A and T in B, degenerates: a node whose
requirements lie at different depths (a chapter that reads the depth buffer directly and also traces a distance field)
cannot be in B while one requirement is in A and another is not, so the smallest valid A swallows nearly everything and
S becomes the chapters. Such skip-level requirements are not mistakes. So every edge that skips levels is split into a
chain of *carry* vertices, one per level crossed (a carry of u at level k: "u, still used directly at level k"), the
standard step from layered graph drawing. In the subdivision the literal definition holds exactly, and it reduces to
a test on G_T: for A a downset containing the bases and no chapter, S = the minimal nodes of G_T ∖ A plus the carries
of the edges that leave A for nodes that are not minimal; S is an antichain separator of the subdivision, and every
such separator arises this way. S is an antichain: two minimal elements are incomparable; a carry on a → b is below a
real s only through b ≤ s, which minimality forbids, and above s only through s ≤ a, which A being a downset forbids;
two carries are comparable only if one edge's head lies in A, which cannot happen. The separator is taken over the
union of the chapters (one audience per investigation); the tab filters it to the current context, A ∩
ancestors(context).

The procedure:

1. Choose the level as `new-story`'s "The knowledge level" (its `references/outline.md`) says: the lowest one at which
   the story can still teach everything above it in its chapters and time. Start from an audience that knows only the bases and the topics
   every graphics programmer meets first, cost what the story would have to teach, and raise the level, one general
   topic at a time, only where a chapter or the story's length does not fit.
2. Choose K, the topics that viewer knows (only topics some chapter needs: the check names the others), and write
   the audience in one sentence (`profile`).
3. `render-lab --investigation <id> knowledge --known <k1,k2,…> --levels` prints the level, the carries, what to
   teach (per chapter too), and the nearby audiences (one level topic added, one known topic removed) with their
   carry counts.
4. Among audiences that fit, prefer the lower one, then the one with fewer carries; never make a chapter teach
   nothing (the check fails when a chapter needs nothing beyond the audience).
5. Write `[audience] known`, and run `render-lab --investigation <id> knowledge` (it fails on any problem).

## What it means for the story

- **Outline** (`new-story` Phase 2, its `references/outline.md`): run the procedure; plan the beats that teach every topic the story teaches,
  before its first use, with their seconds, and a one-sentence reminder for each level topic. The opening names
  the main topics taught, the starting level, and what the viewer will gain.
- **Review** (`new-story` Phase 7, its `references/review.md`): check each chapter against its line of the `knowledge` output: nothing below the level
  explained at length, every topic it teaches explained, every carried topic named.
- Reviewers may propose graph changes (a missing node, a split); the main agent makes them and reruns the check.

## In the viewer

- **The Prerequisites tab** (the dock): the starting level, what the viewer should know by area (nearest the level
  first, ↗ on carried topics), and what the story teaches, each topic with its summary, for the current chapter (on a
  timeline) or the demonstration's chapters (`demos`), else the whole story. A click selects the topic in the panel.
- **The knowledge panel** (`PanelMode::Knowledge`, opened with the tab): the whole graph, bases on the left, its
  groups boxed and titled, coloured by role (known, level, taught, not needed), the context's topics at full strength; drag to pan, scroll to zoom,
  double-click to fit, click a topic for its card. Its Hide unused button lays out only the topics its context
  needs (the chapter, the demonstration's chapters, or the whole story), and Show all brings back the rest. Its Full screen button (and the tab's) shows the graph over the
  whole window; Esc or Close closes it.
- **The homepage**: the selection page's "Before you start" names the starting level and the known topics the story
  uses directly, and its Knowledge graph button shows the graph full screen.

## Commands and checks

From the lab's root (`CARGO_HOME=$PWD/.cache/cargo`):

```sh
cargo run --release -- --investigation godot/sdfgi knowledge                    # check the graph and the audience; print the split
cargo run --release -- --investigation godot/sdfgi knowledge --known rt-depth,cg-gi --levels   # try an audience; nearby ones
cargo run --release -- --investigation godot/sdfgi home --snapshot /tmp/p.png --page viewer-prerequisites:field   # the tab and panel
cargo run --release -- --investigation godot/sdfgi home --snapshot /tmp/s.png --page viewer-knowledge:field          # the panel zoomed onto a level topic
cargo run --release -- --investigation godot/sdfgi home --snapshot /tmp/g.png --page viewer-knowledge-graph:field     # the graph full screen
cargo run --release -- --investigation godot/sdfgi home --snapshot /tmp/c.png --page viewer-knowledge-compact:field   # only what its chapters need
cargo run --release -- --investigation godot/sdfgi home --snapshot /tmp/h.png --page select-knowledge                # over the homepage
cargo test --release --locked --offline --workspace                              # knowledge::tests: the separator, the public graphs
```

Look at the snapshots. Titles and summaries are in both languages; the text check fails on a key missing from one.

Checks, by change: a topic or edge added or changed in render_lab's graph is checked with the `knowledge` command of
every investigation (the loader rejects a cycle or a redundant edge for all of them), `cargo test` (the public
graphs), `render-lab series`, and both languages' text; a chapter's requirements or an audience, with that
investigation's `knowledge` command and its Prerequisites snapshot; a change to `lab/src/knowledge.rs`, with all of
these. Write snapshots to a scratch folder.
