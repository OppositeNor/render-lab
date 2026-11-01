---
name: knowledge-graph
description: Use and extend Render Lab's knowledge graph and choose an investigation's target audience. The knowledge graph is a DAG of the computer-graphics knowledge the chapters need, worked back to three bases (GPU parallel programming; radiometry and the rendering equation; real-time rendering fundamentals); it lives in rlstd's knowledge.toml, and each investigation's knowledge.toml lists its chapters' direct requirements and its audience. The target audience is an antichain vertex separator of that graph: what the viewer should know (the Prerequisites tab), the level the story starts from, and what it must teach. Use when adding or editing topics or chapter requirements, when choosing or checking an investigation's audience, when outlining or reviewing a story's depth against what its viewer knows, or when asked about the Prerequisites tab or the knowledge panel.
---

# The knowledge graph and the target audience

The investigations explain high-level algorithms that assume a lot. The **knowledge graph** says what, all the way down;
the **target audience** says where an investigation starts, so its story explains what is beyond that and nothing
below it. The plan and the mathematics are in `todo/knowledge-graph.md`; the code is `lab/src/knowledge.rs`.

## The graph

- **Nodes** are `[[topic]]` tables in `packages/rlstd/knowledge.toml`: `key`, `title-text` and `summary-text` (keys of
  rlstd's `text/en.json` and `text/zh-CN.json`, `topic.<key>.title` and `topic.<key>.summary`), `area` (`gpu`,
  `rt`, `geo`, `cg`, `light`, `gi`: the panel's and the tab's grouping), `requires`, and optional `reading`
  (published references: papers, books, talks, cited by their publication).
- **The three bases** are the only nodes without requirements (`base = true`): `gpu` (invocations, subgroups,
  workgroups, device, queues, commands), `cg` (radiometry and the rendering equation), `rt` (rasterization,
  interpolation, vertex projection, filtering). Every other node reaches at least one. Do not add a base.
- **An edge is a direct requirement**: understanding the node needs that topic directly. The loader rejects a
  cycle and a **redundant edge** (one another path already implies): the graph is transitively reduced, because
  "directly requires" must mean an edge for the separator to mean anything.
- **One concept per node, engine-neutral.** Engine specifics (O3DE's tile size, Godot's cascade count) belong to the
  chapters, not to nodes. Split a node when stories need half of it at one level and half at another (the loader's
  partition shows this: a node whose requirements are partly known and partly not).
- **A package's own nodes.** An investigation may declare `[[topic]]` tables in its own `knowledge.toml` and name
  rlstd's as `rlstd.knowledge#<key>` (rlstd exports `knowledge`); move a node to rlstd once another investigation needs
  it. **A local package's engine-neutral topics go in rlstd's graph** (decided 2026-11-01): a published technique named
  by what it is ("per-mesh distance fields", "screen-space probe gather"), never by an engine, with no engine named in
  its summary or reading. Only what can be described solely as one engine's design stays in the local package's file,
  or in its chapters.

## An investigation's chapters

`packages/<investigation>/knowledge.toml` (a `kind = "file"` resource with id `knowledge` in its collection):

```toml
[audience]
profile = "Has written a PBR forward renderer with shadow maps and compute passes: …"   # a note, not shown
known = ["rlstd.knowledge#rt-shadow-maps", "rlstd.knowledge#cg-pbr-materials", "…"]

[[chapter]]
number = 5                                         # the storyboard's chapter number
demos = ["field"]                                  # the demonstrations that show it (the tab outside a timeline)
requires = ["rlstd.knowledge#geo-distance-transform"]
```

List what each chapter **uses** directly, as nodes: read its outline section and storyboard chapter, and for every
concept a sentence leans on, name the node (add it to rlstd with its requirements down to existing nodes if it is
missing). A requirement already implied by another of the chapter's is rejected; keep the higher one. Leave recap
chapters out (they require nothing new).

Delegate the listing to a subagent (Sonnet) per investigation with the outline, the storyboard, and the node table
(`render-lab --investigation <id> knowledge --mermaid <file>` or `todo/knowledge-graph-tree.md`); check its result with the
`knowledge` command.

## The target audience

The audience is a set A of known topics, closed under requirements, containing the bases. Restricted to the
chapters' ancestors:

- **The level** S: the topics outside A whose requirements are all in A. They are an antichain (no two depend on
  each other). The story recalls each in a sentence the first time it uses it.
- **Carries**: edges from A to a topic beyond the level (or to a chapter): known topics used directly above the
  level. The tab marks them (↗); the story names them as tools, without explaining them.
- **What the story teaches**: everything else the chapters need. Each is taught before its first use, like a
  teacher: a sequence of beats with a demonstration of its own, animated and explorable (`new-story`, "Teaching a
  topic").

With every level-skipping edge subdivided, S and the carries form an antichain vertex separator between A (what
you should know: the Prerequisites tab) and the rest (what you might not know). The proof is in the plan.

The procedure:

1. Choose the level as `new-story`'s "The knowledge level" says: the lowest one at which the story can still teach
   everything above it in its chapters and time. Start from an audience that knows only the bases and the topics
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

- **Outline (new-story Phase 2):** run the procedure; plan the beats that teach every topic the story teaches,
  before its first use, with their seconds, and a one-sentence reminder for each level topic. The opening names
  the main topics taught, the starting level, and what the viewer will gain.
- **Review (new-story Phase 7):** check each chapter against its line of the `knowledge` output: nothing below the level
  explained at length, every topic it teaches explained, every carried topic named.
- Reviewers may propose graph changes (a missing node, a split); the main agent makes them and reruns the check.

## In the viewer

- **The Prerequisites tab** (the dock): the starting level, what the viewer should know by area (nearest the level
  first, ↗ on carried topics), and what the story teaches, each topic with its summary, for the current chapter (on a
  timeline) or the demonstration's chapters (`demos`), else the whole story. A click selects the topic in the panel.
- **The knowledge panel** (`PanelMode::Knowledge`, opened with the tab): the whole graph, bases on the left, coloured by
  role (known, level, taught, not needed), the context's topics at full strength; drag to pan, scroll to zoom,
  double-click to fit, click a topic for its card. Its Full screen button (and the tab's) shows the graph over the
  whole window; Esc or Close closes it.
- **The homepage**: the selection page's "Before you start" names the starting level and the known topics the story
  uses directly, and its Knowledge graph button shows the graph full screen.

## Commands

From the lab's root (`CARGO_HOME=$PWD/.cache/cargo`):

```sh
cargo run --release -- --investigation godot/sdfgi knowledge                    # check the graph and the audience; print the split
cargo run --release -- --investigation godot/sdfgi knowledge --known rt-depth,cg-gi --levels   # try an audience; nearby ones
cargo run --release -- --investigation godot/sdfgi knowledge --mermaid /tmp/knowledge.mmd         # the graph as Mermaid
cargo run --release -- --investigation godot/sdfgi home --snapshot /tmp/p.png --page viewer-prerequisites:field   # the tab and panel
cargo run --release -- --investigation godot/sdfgi home --snapshot /tmp/s.png --page viewer-knowledge:field          # the panel zoomed onto a level topic
cargo run --release -- --investigation godot/sdfgi home --snapshot /tmp/g.png --page viewer-knowledge-graph:field     # the graph full screen
cargo run --release -- --investigation godot/sdfgi home --snapshot /tmp/h.png --page select-knowledge                # over the homepage
cargo test --release --locked --offline --workspace                              # knowledge::tests: the separator, the public graphs
```

Look at the snapshots. Titles and summaries are in both languages; the text check fails on a key missing from one.
