# Prerequisites and formulas

[Render Lab](../README.md) · [Build](../BUILD.md) · [简体中文](learning-content.zh-CN.md)

Paths and commands are relative to the repository root.

## Prerequisites

The algorithms assume a lot, so the lab keeps a **knowledge graph**: the knowledge every chapter needs, worked back to
three bases (GPU parallel execution; radiometry; the graphics pipeline).
Each investigation names the audience it is aimed at, which splits the graph into what the viewer should know, the
level the story starts from, and what the story teaches.

- The viewer's **Prerequisites** tab lists, for the current chapter (or the demonstration's chapters), what to know
  first, by area, the starting level, and what the story teaches.
- Beside it, the **knowledge graph** panel draws the whole graph, coloured by what the audience knows; drag, scroll,
  and click a topic to read about it. **Full screen** shows it over the whole window (Esc closes it).
- On the homepage, each feature's **Before you start** names the level its story starts from and the known topics it
  uses directly, with a button for the knowledge graph.

The graph is in `packages/render_lab/knowledge.toml`, each investigation's chapters and audience in its `knowledge.toml`,
and the method in the `knowledge-graph` skill:

```bash
cargo run --release -- --investigation godot/sdfgi knowledge         # checked, and the audience's split per chapter
```

## Formulas

On-screen text can hold formulas, as tags: `{@ formula="\\frac{1}{1+n}"}` in a text table, a caption, or a readout
(`Readout::formula`). The lab typesets them itself at run time (`lab/src/formula.rs`: MiTeX translates the LaTeX into
Typst math, and Typst typesets it), so no Node or committed images are needed; `render-lab formulas --check FILE…` checks
a file's formulas against the lab's limits. A storyboard uses the same tags, with shot cues timed to the narration's
words (see [`tools/video`](../tools/video/README.md#text-driven-shots-cues-in-a-sentence)), and a timeline key's `formula`
opens a formula card in the viewport.
