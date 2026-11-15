---
name: render-lab-math
description: Write, change, and check a math scene in Render Lab with its math animation library, `render_lab_math` (manim's techniques, procedural and driven by values) - a story's `kind=2d math=<scene>` beat, a demonstration's `math`, the `Math` hook, and the library's figures (formulas by named parts, numbers, shapes, axes and plots of functions, areas and Riemann sums, seeded samples and histograms, bars, braces, probability squares, tables, graphs with computed layouts) and animations (write, create, fade, grow, indicate, circumscribe, cross out, transform matching parts, fly from a copy, the moving frame). Use when a beat works through a formula or a calculation, or shows a function, a distribution, a table, or a graph, and when adding to the library or the canvas (lab/src/math/). Not for light on a surface, which the 3D scene shows.
---

# Math scenes

A **math scene** draws mathematics on a canvas in the middle of the frame instead of the 3D scene: a formula built
term by term, the numbers of a worked example flying from a table into it, a curve redrawn as its parameter moves.
The canvas and its drawing are the lab's (`lab/src/math/`, the `Mob`s of
`packages/render_lab/language/runtime.lam`); the library that builds scenes is `render_lab_math`
(`packages/render_lab/language/math.lam`), which a package's scripts import. The worked example is ReSTIR's
`toyMath` (`packages/paper_restir/script/toy.lam`, chapter 2's beat `c2-toy`).

Read `render-lab-lamlet` first for the language, scripts, and storyboards; this skill is what is specific to math
scenes.

## When to cut to a math beat

- **On the canvas:** a core or bridge formula with a worked case (`new-story`'s `references/storytelling-formula.md`,
  "Show it on the math canvas"), a calculation, a probability, a plot of a function or a distribution, a table of
  what each candidate holds, a graph of what depends on what.
- **In the 3D scene:** a mechanism on a surface, light, rays, the frame's passes. A formula the scene needs beside
  it stays the formula card (`formulaSaid`).
- **Float, then focus:** cut in for the detailed step and back out to the scene; a chapter does not live on the
  canvas. The cut is a beat of its own (`### … {#c2-toy kind=2d math=toyMath}`), faded in and out by the lab.
- A math beat is a beat of its chapter: its supporting animations count toward ⌈T / 6 s⌉, under `new-story`'s
  Phase 6 rules, and each is a value the dock shows.

## The model: values, not a timeline of plays

manim builds a scene from `play()` calls on objects that change. A Render Lab scene keeps nothing between frames:
it is a function of its time and its values (the investigation's values package), as manim's `ValueTracker` with
`always_redraw`, made the only model. So:

- **One stage value per beat** (`toyStage`, 0 to its number of stages, default its last), a `#[value]` in the
  investigation's `values.rs` with a slider in the demonstration's dock group. Each sentence sweeps one stage
  (`${overText {' {toyStageRange 3.0 4.0}} "words"}`); `{phase s 3.0 4.0}` is that stage's progress in the scene.
- **Every animation takes its progress `t`** (0 to 1) and the figure, and gives the figure at that moment:
  `{write {phase s 5.0 5.8} formula}`. Several in one stage split it (`{phase s 5.0 5.4}`, `{phase s 5.4 5.8}`) or
  stagger (`lagged`).
- **A figure is a `List(Mob)`**; animations and layout map over it. Compose a stage's animations by nesting them on
  the same figure (`{circumscribe t2 {transform t1 a b} color}`), never by appending a second copy: an animation
  outside its stage returns its figure (`indicate`, `circumscribe`, `crossOut` included), so appending one draws the
  figure twice. To change some parts of a figure, filter them out and append the changed ones (ReSTIR's
  `crossParts`, `pointParts` in `script/common.lam`); to ring a figure for one stage alone, keep only the ring:
  `{drop {circumscribe t fig color} {length fig}}`.
- **The demonstration shows the same scene** (`math = "toyMath"` in its file): the viewer draws the canvas in place of
  the room, and dragging the stage value replays the beat.

## Procedural only

- Every figure is made by code from functions and data: a curve from its function (`plot`), a table from its rows,
  a graph's layout from its edges (`graph`, laid out by `lab/src/math/layout.rs`), samples from a seed. Nothing is
  placed by typed coordinates where a relation says it (`nextTo`, `inRow`, `onAxes`), and nothing is a picture.
- **Numbers come from the investigation's code**, through its natives (`Investigation::script_natives`; ReSTIR's
  `toyWeight`, `exampleTrace`, `biasCurves`, …), never typed into the scene. A demonstration on the canvas is
  for reading numbers, so give it **a small seeded example of its own** in the package's code, run through the
  same functions the passes mirror (ReSTIR's `examples.rs`: 24 lights, P and three neighbours, streamed through
  `cpu.rs`'s reservoir, combined as `Scene::combine` does), rather than the frame's data: values read back from the
  frame (`frame_data`) depend on the viewport's shape, differ between the viewer and a recording, and are often
  zero where the scene is dark. The scene may also read the dock's values (`{restirNeighbours}`), so it follows
  them. Write numbers as LaTeX with `fractionLatex`, `numberLatex`, or a scene's own helper (ReSTIR's `rational`,
  `short`).
- **Text** on the canvas is a formula (`tex`, `texParts`: LaTeX in `f"{…}"` templates) or a text key
  (`caption` with `{textKey "…"}`, in both languages' tables); `words` only for a symbol or a number.

## Writing a scene, in order

The hook's form, helpers, colours as Strings, LaTeX in templates, and anchors are in `references/recipes.md`, "The
Lamlet a scene needs".

1. **The palette.** One colour per symbol, the same in every figure (p blue, p̂ yellow, w pink, …), as private
   bindings (`_pColor`). The theme's `textColor`, `textDimColor`, `accentColor` follow the viewer's theme.
2. **The figures**, each a function of the stage (`_bars i s`, `_table drop`), their parts named: a formula's
   `\lit{n}{…}` terms get names (`texParts id latex size color names colors`), and a part keeps its name in every
   formula it appears in, so `transform` carries it.
3. **The layout**: canvas units, 8 high and 16:9 wide (`canvasWidth`), the origin at the centre, +y up. Leave the
   bottom to the lab: it keeps a band there for the beat's callout, and in a recording the canvas sits between
   the tag and the subtitles.
4. **The animations per sentence**: one stage each, its mechanism shown working (`new-story`), the figure's earlier
   stages kept in place.
5. **The frame's moves** (`showFrame {frameBetween fullFrame {frameOn fig 0.5} t}`), last in the list.

Choosing a figure: a **table** for what each case holds; **bars** to compare a few values; an **area square** for
the chances of combined draws; a **number line** for estimates, their spread, and their mean; a **plot** for a
function, with `area`, `riemann`, `tangentAt`, `dotOn` for what it means; a **histogram** over its density for
samples; a **graph** for dependencies or a flow.

`references/recipes.md` has manim's techniques as the library writes them; `references/library.md` lists every
function.

## Sizes and fit

- Text is at least 24 px at 1080p (`MIN_GLYPH_PX`). The canvas is about 100 px a unit in a recording, so an em of
  0.32 is the least for plain text, and 0.42 where a fraction's or a subscript's smaller digits must stay legible.
  Never shrink a figure with text below that; fade it out and keep what carries its numbers instead.
- Keep each stage's figures apart: a label beside a brace, a formula clear of a table. Look at every stage.
- A scene's call runs within the scripts' budget (2 million instructions). Lamlet's lists are linked, so a
  scene avoids `nth` in loops and long `append` chains; a tabulation or search over hundreds of items belongs in a
  native (`rl_runtime/samples-from`, `rl_runtime/graph-layout`).

## Checks

| Change | Check |
|---|---|
| A scene | `render-lab --investigation <id> script check`; the demonstration's canvas at several stages (`render-lab --investigation <id> home --snapshot <png> --page viewer:<demo>`, its stage value set in the demo's `[params]` while you look, then restored), looked at |
| A math beat | both languages' `--prepare-only`; `python3 tools/video/build.py --investigation <id> --preview <beat> --cached-audio-only`, every still looked at; the beat's frames rendered with `--report` (`render-lab --investigation <id> render --timeline … --snapshot … --report …`): smallest text at least 24 px, no label issues |
| The library | `cargo test --release --locked --offline -p render-lab-core -- script::runtime math` (its arithmetic and layouts), every investigation's `script check`, and their values packages unchanged (`script stubs`: a library name a value key also has drops that key's reader) |
| The canvas (`lab/src/math/`, the `Math` hook) | `render-lab-lamlet`'s checks for the runtime library, and the frame identity check (AGENTS.md) |
| Any `.lam` file | `python3 tools/lamlet_fmt.py --check` |
