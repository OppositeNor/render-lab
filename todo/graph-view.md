# One graph view for every graph the lab draws

Today the lab draws graphs three ways, each with its own model, layout, and drawing:

| Graph | Model | Layout | Drawing | Interaction |
|---|---|---|---|---|
| Story diagrams (2D beats) | Mermaid subset parsed by `lab/src/diagram.rs` (`video/diagrams/*.mmd`; the frame graph scene generates that text internally) | `layered::layout` | `diagram::draw` | zoom and pan (paused timeline) |
| Knowledge graph (panel, full screen) | `knowledge::Graph` | `layered::clustered` (areas boxed) | `ui::draw_knowledge_graph` | zoom, pan, select, centre on a topic |
| Frame graph DAG (panel, full screen) | `ui::GraphView` from `FrameGraph` | its own longest-path layers | `ui::draw_dag_panel` | pan, drag cards, inspect, scroll to call-outs |

Goal: one model, one layout, one drawing, one interaction, used by all three, and no Mermaid in the lab.

## The abstraction (`lab/src/graph_view.rs`)

- **Model.** `Diagram { nodes, edges, groups }`:
  - a node has an id, title lines, rows (id, label: a group of passes' members, a topic's summary), an optional
    picture (a texture and its size: a pass's result), a badge (GPU time), and a style (dashed, faint, colour);
  - an edge has its ends, a style (solid, dotted, faint), a colour, and an optional label;
  - a group (a cluster) has a title and members, boxed or loose.
- **Layout.** `layered::clustered` (the knowledge graph's: groups kept together, boxed), left to right, and top to
  bottom for the narrow panel (the axes swapped). Back edges are arcs, as `diagram.rs` draws them. Laid out once per
  model, cached by a key.
- **View.** Fit to the area. In the viewer: wheel zoom, drag to pan, drag a node to move it, double-click to reset.
  A story's view is computed from its state alone (fit, or centred on its focus), so a frame depends only on its
  state. Text never below the lab's 24 px: below it, rows fold into their titles (as the knowledge graph hides
  labels when zoomed out).
- **Drawing.** One `draw(ink, area, diagram, layout, view, focus) -> Picks` (hovered and clicked node or row). Lit
  nodes and rows use the accent, the rest dim; dashed boxes stay dashed when lit.

## The adopters

1. **Story diagrams.** `video/diagrams/*.toml` replace `*.mmd`: nodes (`id`, `title-text`, `rows`, `dashed`),
   edges (`from`, `to`, `dotted`, `label-text`), groups. Their text becomes keys of the package's text tables, so a
   translation is no longer a second diagram file. The remaining diagrams are converted: O3DE's
   `forward-one-target`, and the local package's `map`.
2. **The frame graph scene** (`graph: frame`) builds the model straight from `FrameGraph` and `frame.toml`: no
   generated text.
3. **The DAG panel and its full screen** use the same model: each pass a node with its result as its picture and its
   GPU time as its badge. The sections in `frame.toml` group them there too, so the panel and the story show one
   layout. That needs the sections as a package resource the viewer loads (they are in `video/` today).
4. **The knowledge graph** panel and full screen: areas as groups, topics as nodes, the audience's level and
   context as styles; selection and centring as picks and view targets.
5. The resources view and the grid stay as they are: they are a grid and a matrix, not graphs.

## Retiring Mermaid

- `lab/src/diagram.rs`'s parser and the `.mmd` files go; the video tools read the TOML.
- `render-lab graph --mermaid` and `render-lab knowledge --mermaid` export Mermaid text, and the video's guide embeds
  each diagram as a Mermaid block (GitHub draws it): all of them go (decisions below).

## Phases

1. `graph_view.rs`: model, layout (with the swapped axes), view, drawing; `diagram.rs`'s drawing ported onto it;
   tests of layout and picks.
2. Story diagrams in TOML; the two `.mmd` files converted; the video tools and their tests; `diagram.rs` retired.
3. The frame graph scene on the model; the sections as a package resource.
4. The DAG panel and full screen on the model (pictures, badges, dragging); `draw_dag_panel` retired.
5. The knowledge graph on the model; `draw_knowledge_graph`'s layout and drawing retired, its selection kept.
6. The `--mermaid` exports removed, the guide's diagrams as lists, skills and READMEs, the public build, frame identity of the 3D
   beats, and stills of every 2D diagram beat in both languages.

## Decisions (2026-11-02)

- **Mermaid is dropped everywhere**: no `--mermaid` exports (`render-lab graph` keeps `--dot`), no Mermaid blocks in
  the video's documents; the guide lists each diagram's boxes as text, as it does for the frame graph scene now.
- **The DAG panel uses the story's sections**: they move from `video/frame.toml` into a package resource the viewer
  loads, and the panel, its full screen, and the story's scene show one grouping (each pass still its own card).
- **Plan only for now**: the user reviews this plan before any of it is implemented.

## Status (2026-11-02)

Built: `lab/src/graph_view.rs` (model, a data form keyed by ids, layout in either flow with clusters and back
arcs, drawing with boxes and cards, view state and pointer handling, blending of two layouts); story diagrams as
`video/diagrams/*.toml` (`tools/video/diagrams.py`), the two Mermaid diagrams converted and removed, one diagram
directory per package; the frame graph scene built straight from `FrameGraph`; the sections as each package's
resource `graph/frame` (`graphs/frame.toml`), loaded by the lab (`GraphScene::current`); the DAG panel and its full
screen on the shared view, boxed by the sections; the knowledge graph's panel and full screen on it (its topics can
now be dragged too); `--mermaid` gone from `graph` and `knowledge`, the guide listing each diagram's boxes.

Added on request: transitions instead of fades for 2D diagram beats. Consecutive beats of one diagram in a
chapter move from one focus to the next (boxes glide, the view eases, the lit state blends, new pictures fade in as
they grow); a diagram's first beat builds it up layer by layer, each edge drawn along its path
(`lab/src/diagram.rs` `Transition`, `record::beat_fade`).

Kept: the resources view and the grid (a card grid and a matrix, not graphs).
