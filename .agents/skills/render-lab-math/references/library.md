# The library: `render_lab_math`

Every function of `packages/render_lab/language/math.lam`, by section, with its type and what it does (its doc
comment there, which is the reference when the two differ). Canvas units: 8 high, the origin at the centre, +y
up; widths of lines in pixels at 1080p; an em `size` in canvas units. `t` is an animation's progress, 0 to 1.

## Points and boxes

- `v2`: Fn([Float, Float], Vec2). A point on the canvas.
- `origin`: The canvas's centre.
- `add2`: Fn([Vec2, Vec2], Vec2). The sum.
- `sub2`: Fn([Vec2, Vec2], Vec2). The difference.
- `scale2`: Fn([Vec2, Float], Vec2). Scaled by `s`.
- `lerp2`: Fn([Vec2, Vec2, Float], Vec2). From `a` at 0 to `b` at 1.
- `dist2`: Fn([Vec2, Vec2], Float). The distance between two points.
- `Box`: A box on the canvas: its lower left and upper right corners.
- `box`: Fn([Vec2, Vec2], Box). A box.
- `boxCenter`: Fn([Box], Vec2). Its centre.
- `boxWidth`: Fn([Box], Float). Its width.
- `boxHeight`: Fn([Box], Float). Its height.
- `padded`: Fn([Box, Float], Box). Grown by `pad` on every side.
- `boxUnion`: Fn([Box, Box], Box). The box around both.
- `boxAround`: Fn([List(Vec2)], Box). The box around points (at least one).

## Mobs

- `texParts`: Fn([String, String, Float, Color, List(String), List(Color)], List(Mob)). A formula, centred on the origin, an em `size` canvas units: its ink outside the `\lit{n}{…}` terms (named `id`, in `color`), then each term n (named the n-th of `names`, in the n-th of `colors`; past their ends, `id.n` and `color`). Display style.
- `tex`: Fn([String, String, Float, String], List(Mob)). A formula in one colour (its terms named `id.n`).
- `words`: Fn([String, String, Float, Color], List(Mob)). A line of text, written out (a number, a symbol), centred on the origin, an em `size` high.
- `caption`: Fn([String, String, Float, Color], List(Mob)). A line of the package's text `key` (`textKey`), in the viewer's language.
- `numberLatex`: Fn([Float, Int], String). A number with `digits` decimals, as LaTeX (a minus sign, not a hyphen).
- `number`: Fn([String, Float, Int, Float, Color], List(Mob)). A number typeset as a formula: count it by drawing it at a value that moves (`{number "w" {lerp 0.0 12.0 t} 2 0.5 color}`).
- `fractionLatex`: Fn([Int, Int], String). A fraction `n/d` as LaTeX, in lowest terms (a whole number when `d` divides `n`).
- `rectMob`: Fn([String, Box, Color, Float, Color, Float, Float, Float], Mob). A rectangle: filled (`fillAlpha` 0 leaves the fill out), outlined `width` pixels (0 leaves it out), its corners rounded by `radius` canvas units.
- `filled`: Fn([String, Box, String, Float], List(Mob)). A filled rectangle.
- `outlined`: Fn([String, Box, Color, Float], List(Mob)). A rectangle's outline, `width` pixels, slightly rounded.
- `pathMob`: Fn([String, List(Vec2), Color, Float, Bool, Bool], Mob). A path through `points`, `width` pixels.
- `line`: Fn([String, Vec2, Vec2, String, Float], List(Mob)). A line.
- `arrowLine`: Fn([String, Vec2, Vec2, Color, Float], List(Mob)). An arrow from `a` to `b`.
- `dashedLine`: Fn([String, Vec2, Vec2, Color, Float], List(Mob)). A dashed line.
- `pathThrough`: Fn([String, List(Vec2), Color, Float], List(Mob)). A path through points (a curve, sampled).
- `bandBetween`: Fn([String, List(Vec2), List(Vec2), Color, Float], List(Mob)). The area between two lines of as many points.
- `dotAt`: Fn([String, Vec2, Float, String], List(Mob)). A dot `radius` canvas units across, at `at`.

## Measuring a figure

- `boxOf`: Fn([List(Mob)], Box). The box around a figure (a box of no size at the origin when it has nothing to measure).
- `centerOf`: Fn([List(Mob)], Vec2). A figure's centre.
- `widthOf`: Fn([List(Mob)], Float). A figure's width.
- `heightOf`: Fn([List(Mob)], Float). A figure's height.
- `idOf`: Fn([Mob], String). A mob's name (the frame's is empty).
- `withId`: Fn([List(Mob), String], List(Mob)). The mobs named `id`.

## The frame

- `Frame`: The part of the canvas the frame shows: its centre and its height, as manim's camera frame.
- `frameAt`: Fn([Vec2, Float], Frame). A frame.
- `fullFrame`: The whole canvas: 8 high about the origin.
- `canvasWidth`: The canvas's width when the frame is 8 high, at 16:9.
- `frameOn`: Fn([List(Mob), Float], Frame). The frame around a figure, with `pad` canvas units around it, at 16:9.
- `frameBetween`: Fn([Frame, Frame, Float], Frame). From one frame to another, smoothly, at `t` (its height by its logarithm, so a zoom runs at an even pace).
- `showFrame`: Fn([Frame], List(Mob)). The frame as the scene's mob (the last one a scene returns counts).

## Moving a figure

- `mapped`: Fn([List(Mob), Fn([Vec2], Vec2), Float], List(Mob)). Every point of a figure mapped by `f`, and its sizes (an em, a dot, a corner) scaled by `s`. Widths of lines stay in pixels, as manim's strokes do.
- `shift`: Fn([List(Mob), Vec2], List(Mob)). Moved by `d`.
- `moveTo`: Fn([List(Mob), Vec2], List(Mob)). Moved so its centre is at `p`.
- `scaleAbout`: Fn([List(Mob), Vec2, Float], List(Mob)). Scaled by `s` about `c`.
- `scaled`: Fn([List(Mob), Float], List(Mob)). Scaled by `s` about its centre.
- `fitInto`: Fn([List(Mob), Box], List(Mob)). Moved and scaled (evenly) from where it is into the box `b`: centred on it, as high as it (or as wide, whichever fits).
- `Side`: The four sides a figure can be put on.
- `right`, `left`, `up`, `down`: the four `Side`s.
- `sideDir`: Fn([Side], Vec2). A side's direction.
- `nextTo`: Fn([List(Mob), List(Mob), Side, Float], List(Mob)). Moved beside `anchor`, on its `side`, `gap` canvas units from it, centred on it along that side (manim's `next_to`).
- `toEdge`: Fn([List(Mob), Side, Float], List(Mob)). Moved to an edge of the whole canvas (8 high, 16:9), `margin` from it, centred along it (manim's `to_edge`).
- `inRow`: Fn([List(List(Mob)), Float], List(List(Mob))). Figures side by side, left to right, `gap` apart, their centres on one line, the whole row centred on the origin (manim's `arrange(RIGHT)`).
- `inColumn`: Fn([List(List(Mob)), Float], List(List(Mob))). Figures one under another, top to bottom, `gap` apart, centred on one vertical line, the whole column centred on the origin (manim's `arrange(DOWN)`).

## Colour and opacity

- `faded`: Fn([List(Mob), Float], List(Mob)). Its opacity multiplied by `k`.
- `tinted`: Fn([List(Mob), String], List(Mob)). Recoloured (a rectangle's fill and outline both).
- `renamed`: Fn([List(Mob), String], List(Mob)). Renamed: every mob called `id` (a figure that stands for one thing).
- `hsvColor`: Fn([Float, Float, Float], String). The colour of hue `h`, saturation `s`, and value `v` (each 0 to 1), as `#rrggbb`.
- `spectrum`: Fn([Int], List(Color)). `n` colours around the hue circle, neighbours far apart (one per outcome of a distribution).

## Progress and rate functions

- `phase`: Fn([Float, Float, Float], Float). How far `x` is from `a` to `b`, 0 before and 1 after: one stage of a value that steps a sequence (`{phase step 3.0 4.0}` runs while `step` goes from 3 to 4; manim's `Succession`). Equal bounds give a step at `b`.
- `heldMark`: Fn([Float, Float, Float, Float, Float], Float). A mark held for a while (a strike that stays, then lifts), at the progress `t` of a score's emphasis from `from` over `length` steps: rising over 0.4 of a step from `on`, falling over 0.4 from `off`.
- `thereAndBack`: Fn([Float], Float). There and back: 0 at both ends, 1 in the middle, smoothly (manim's `there_and_back`).
- `rushInto`: Fn([Float], Float). Starting from rest, ending at speed (manim's `rush_into`).
- `rushFrom`: Fn([Float], Float). Starting at speed, ending at rest (manim's `rush_from`).
- `lagged`: Fn([Float, Int, Int, Float], Float). The progress of the `i`-th of `n` animations that start one after another, each `lag` of one's length after the one before, at the whole group's progress `t` (manim's `LaggedStart`).

## Steps

- `stepOf`: Fn([String, Int], Float). Where the scene `name` of `count` steps is: its progress, or `count` (the finished picture) while no clip has set it.
- `stepClip`: Fn([String, Int, Float, Float], Clip). Step `k` from `a` to `b` of it, over a second, at an even pace.
- `stepsAt`: Fn([String, Int, Int], Clip). The scene set at once to the end of step `n`.
- `stepsInOrder`: Fn([String, Int], Clip). Every step in order from none, a second each: its demonstration's Play.
- `stepsRun`: Fn([String, Float, Float], Clip). From `a` to `b` across several steps over a second, at an even pace.

## Animations

- `fadeIn`: Fn([Float, List(Mob)], List(Mob)). Fades in while rising a little into place (manim's `FadeIn(shift=UP)`).
- `fadeOut`: Fn([Float, List(Mob)], List(Mob)). Fades out while rising a little away.
- `write`: Fn([Float, List(Mob)], List(Mob)). Written in, mob by mob from the first, each starting before the last ends (manim's `Write`; a formula's parts are its mobs, so it writes left to right as its parts are ordered).
- `create`: Fn([Float, List(Mob)], List(Mob)). Drawn all at once (manim's `Create`): paths along their length, the rest faded in.
- `growFrom`: Fn([Float, List(Mob), Side], List(Mob)). Grown out of one side of its box (manim's `GrowFromEdge`): a bar rises from its axis with `down`.
- `growUp`: Fn([Float, List(Mob)], List(Mob)). Grown up from its bottom edge (`growFrom` with `down`, as an entering animation a part can name).
- `indicate`: Fn([Float, List(Mob), String], List(Mob)). Pointed at: scaled up and lit in `color`, then back (manim's `Indicate`); the figure stays whole, the tint over it.
- `indicateEach`: Fn([Float, List(Mob), Color], List(Mob)). Each mob pointed at about its own centre (a row lit together, none pushed outward).
- `circumscribe`: Fn([Float, List(Mob), String], List(Mob)). An outline drawn around the figure, then faded (manim's `Circumscribe`): the figure with it.
- `crossOut`: Fn([Float, List(Mob), String], List(Mob)). Struck out: a line drawn across the figure, which dims to half (a term a counterfactual leaves out).
- `transform`: Fn([Float, List(Mob), List(Mob)], List(Mob)). One figure turned into another (manim's `TransformMatchingTex`): the pieces named alike in both move (and scale: texts by their ems, so two forms of one size keep it; other pieces by their boxes) from where they are in the first to where they are in the second, cross-fading into the new form (the old whole until 0.4, gone at 0.8; the new from 0.2, whole at 0.6) unless both forms draw the same (then it only moves); the first's other pieces fade out, the second's fade in. Pieces named "" never match.
- `fromCopy`: Fn([Float, List(Mob), List(Mob)], List(Mob)). A copy of `source` flying into `target`'s place and turning into it; `source` stays (manim's `TransformFromCopy`). Draw `source` beside it.
- `write` gives each mob at least a quarter of the write (many mobs overlap more rather than flash in).
- `during`: Fn([Float, Float, Float, List(Mob)], List(Mob)). A piece shown while `s` runs from `a` to `b`, faded in over its first 0.3 of a step and out over its last 0.3 (less in a shorter span).

## Scores

- `score`: Fn([String, Int, List(ScorePart), List(FrameKey)], Score). A scene of `count` steps named `name` (its progress): its parts in drawing order, and its frame's moves.
- `part`: Fn([String, Fn([Float], List(Mob)), List(_Change)], ScorePart). A part named `id`: its figure at `s`, and its changes in the order of their steps. A part with no `enter` shows from the start.
- `enter` / `enterOver`: Fn([Float, Fn([Float, List(Mob)], List(Mob))], _Change) / with a length. The part enters at `at` (half a step by default) by an animation that shows nothing at 0 and the figure at 1.
- `exit` / `exitOver`: Fn([Float], _Change) (a fade over half a step) / Fn([Float, Float, Fn(…)], _Change). The part leaves; its emphases' marks leave with it.
- `become` / `becomeOver`: Fn([Float, Fn([Float], List(Mob))], _Change) (0.6 of a step) / with a length. The part turns into another figure (`transform`).
- `emphasize`: Fn([Float, Float, Fn([Float, List(Mob)], List(Mob))], _Change). The part pointed at from `from` to `to` by an animation that leaves it as it was at both ends; applied over the part's turns.
- `hold`: Fn([Float, Float, Fn([Float, List(Mob)], List(Mob))], _Change). The part marked from `on` to `off`, across steps: `how` takes how far the mark is on (0 to 1 over 0.4 of a step at `on`, back over 0.4 at `off`), so it must stay drawn at 1 (`crossOut`, `crossParts`, `circumscribe`'s ring kept; not `indicate`).
- `frameKey`: Fn([Float, Float, Fn([Float], Frame)], FrameKey). The frame moves to the frame given at `at` over `length` steps.
- `scoreAt`: Fn([Score, Float], List(Mob)). The score at `s`: every part as its changes leave it, then the frame. Changes shorter than `shortestChange` (0.1 of a step) are lengthened; changes out of order, and a change (or a frame's move) that runs across the start of a step, are errors (a story holds each step's end between sentences; a `hold` is checked by its two transitions).
- `playScore`: Fn([Score], List(Mob)). The score where its scene is (`stepOf`): what a `Math` hook returns.

## Axes and number lines

- `Range`: A range of numbers on an axis: from `lo` to `hi`, a tick every `step`.
- `axisRange`: Fn([Float, Float, Float], Range). A range of an axis.
- `ticksOf`: Fn([Range], List(Float)). The range's ticks, from `lo` to `hi` (a rounding short of `hi` counts).
- `tickLatex`: Fn([Float], String). A tick's label: a whole number without decimals, a tenth with one, else with two.
- `Axes`: Axes: where they are on the canvas (a box), the ranges they show, and how a point of theirs maps to the canvas. Every figure drawn on them (`plot`, `area`, `riemann`, `histogram`, `dotOn`) takes them, so moving the axes moves what is on them.
- `axesIn`: Fn([Box, Range, Range], Axes). Axes filling `b`.
- `onAxes`: Fn([Axes, Float, Float], Vec2). The canvas point of the axes' point (x, y).
- `axesFigure`: Fn([String, Axes, Color, Float, Bool, Bool], List(Mob)). The axes drawn: each axis an arrow through its range (at the other's zero, or its edge), ticks, and (`labelX`, `labelY`) each tick's number, `size` canvas units an em.
- `numberLine`: Fn([String, Range, Vec2, Vec2, String, Float], List(Mob)). A number line from `a` to `b` on the canvas, its range's ticks numbered, `size` an em (an axis alone).
- `onLine`: Fn([Range, Vec2, Vec2, Float], Vec2). Where `x` is on a number line from `a` to `b`.

## Functions

- `samplesOf`: Fn([Axes, Fn([Float], Float), Float, Float], List(Vec2)). A function's samples from `a` to `b`, in the axes' own numbers, refined where it bends.
- `plot`: Fn([String, Axes, Fn([Float], Float), Float, Float, String, Float], List(Mob)). The graph of `f` from `a` to `b` on the axes, `width` pixels (manim's `Axes.plot`); `create` draws it along its length.
- `parametric`: Fn([String, Axes, Fn([Float], Vec2), Float, Float, Int, String, Float], List(Mob)). A curve in the plane, `g` of a parameter from `a` to `b` in `n` even steps, in the axes' numbers (manim's `ParametricFunction`).
- `area`: Fn([String, Axes, Fn([Float], Float), Float, Float, String, Float], List(Mob)). The area between the graph of `f` and the x axis, from `a` to `b` (manim's `get_area`), its height clamped to the axes.
- `areaBetween`: Fn([String, Axes, Fn([Float], Float), Fn([Float], Float), Float, Float, String, Float], List(Mob)). The area between two graphs from `a` to `b`.
- `riemann`: Fn([String, Axes, Fn([Float], Float), Float, Float, Int, String, Float], List(Mob)). `n` bars under the graph of `f` from `a` to `b`, each as high as `f` at its middle (manim's `get_riemann_rectangles`): their area is the midpoint sum, which nears the integral as `n` grows. Each bar is named `id.i`, so `write` raises them in turn.
- `slopeOf`: Fn([Fn([Float], Float), Float], Float). The slope of `f` at `x` (a central difference).
- `tangentAt`: Fn([String, Axes, Fn([Float], Float), Float, Float, String], List(Mob)). The tangent to the graph of `f` at `x`, `length` long in the axes' x numbers.
- `dotOn`: Fn([String, Axes, Fn([Float], Float), Float, Float, String], List(Mob)). A dot on the graph of `f` at `x` (sweep `x` and it rides the curve).
- `dropLine`: Fn([String, Axes, Fn([Float], Float), Float, String], List(Mob)). A dashed line down from the graph of `f` at `x` to the x axis (manim's `get_vertical_line`).
- `curveLabel`: Fn([List(Mob), Axes, Fn([Float], Float), Float, Float], List(Mob)). A label moved beside the graph of `f` at `x`, `gap` above it, on the side the curve leaves clear: up and left where it rises, up and right where it falls (manim's `get_graph_label`).

## Samples

- `uniform`: Fn([Int, Int], Float). The `i`-th of a seed's uniform numbers in [0, 1) (splitmix64, `rl_runtime/uniform`), each unrelated to its neighbours.
- `uniforms`: Fn([Int, Int], List(Float)). `count` uniform numbers in [0, 1).
- `stratified`: Fn([Int, Int], List(Float)). `count` stratified numbers in [0, 1): one in each of `count` even strata, jittered.
- `drawnFrom`: Fn([Fn([Float], Float), Float, Float, Int, Int], List(Float)). `count` numbers from `lo` to `hi` drawn in proportion to `f` (not negative, not all 0), seeded: `f` tabulated in 256 bins, drawn from by inverting their running total (`rl_runtime/samplesFrom`).
- `histogram`: Fn([String, Axes, List(Float), Float, Float, Int, String, Float], List(Mob)). A histogram of samples on the axes, `bins` bars from `lo` to `hi`, each as high as its density (its share over its width), so it lies on the curve of the density it was drawn from. Each bar is named `id.i`.
- `firstOf`: Fn([Float, List(List(Mob))], List(Mob)). The first `count` figures of a list, the next one fading in as `count` passes its index (sweep a count and the newest arrives while the earlier stay).

## Bars, braces, and areas

- `bars`: Fn([String, List(Float), Float, Box, List(Color), Float], List(List(Mob))). Bars for values, side by side in `b`, each as high as its value over `top` (the value the box's height stands for), `gap` (a fraction of a bar) between them, in its colour (the last colour when they run out). Each bar is named `id.i`; `growFrom … down` raises one.
- `brace`: Fn([String, List(Mob), Side, Float, String], List(Mob)). A curly brace along one side of a figure (manim's `Brace`), `gap` from it; `braceLabel` puts a label beyond its tip.
- `braceLabel`: Fn([String, List(Mob), Side, Float, String, List(Mob)], List(Mob)). A brace and its label beyond its tip.
- `areaSquare`: Fn([String, List(Float), List(Float), Box, List(Color), Float], List(Mob)). Probabilities as areas (a mosaic): the box split into columns as wide as `across`'s shares and rows as high as `down`'s (top to bottom), each cell a rectangle named `id.i.j` (column i, row j) whose area is the product of its two shares, in the colour of its column (the last colour when they run out), `gap` canvas units apart.

## Tables

- `Table`: A table laid out from its rows, as `table` measures it: each cell placed, and where each cell is.
- `table`: Fn([String, List(List(List(Mob))), Float, String], Table). A table of cells (`rows`, each a list of figures, any size), every column as wide as its widest cell and every row as high as its highest, `pad` canvas units around each cell's content, centred on the origin; a rule under the first row and after the first column (headers), in `color` (manim's `Table`, `MathTable`).
- `tableShifted`: Fn([Table, Vec2], Table). A table moved by `d`.
- `cellOf`: Fn([Table, Int, Int], List(Mob)). A cell's figure, as placed.
- `rowOf`: Fn([Table, Int], List(Mob)). A row's cells, as placed.
- `columnOf`: Fn([Table, Int], List(Mob)). A column's cells, as placed (rows shorter than it give none).
- `cellLit`: Fn([Table, Int, Int, String, Float], List(Mob)). A cell's highlight, its box filled (draw it before the table).
- `tableFigure`: Fn([Table], List(Mob)). The whole table: rules and cells.

## Graphs

- `Graph`: A graph laid out by `graph`: its nodes' figures placed, its edges drawn between them, and where each node is.
- `graph`: Fn([String, List(List(Mob)), List({first: Int, second: Int}), String, Box, Bool, String, Int], Graph). A graph (manim's `Graph` and `DiGraph`): each node a figure (centred on the origin), placed by the `layout` (`circular`, `grid`, `bipartite`, `tree`, `layered`, or `spring`, seeded by `seed`; lab/src/math/layout.rs) in the box `b`, and each edge (from, to) a line or, `directed`, an arrow from the edge of one node's box to the other's, in `color`. Each node's figure keeps its names; edge k is named `id.k`.
- `graphFigure`: Fn([Graph], List(Mob)). The whole graph: edges under nodes.
- `flowAlong`: Fn([Float, List(Mob), Float, Color], List(Mob)). A dot travelling along an edge (or any path) at `t`, `radius` canvas units: what flows along it.
