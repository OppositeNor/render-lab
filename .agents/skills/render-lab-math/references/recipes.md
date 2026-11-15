# Recipes: manim's techniques in `render_lab_math`

Each recipe is code that ran in a scene (ReSTIR's `toyMath`, `packages/paper_restir/script/math.lam`, or the library's
check scenes), with `s` the stage value and `t` a progress. A scene starts:

```text
$[Import* std]$[Import* render_lab_runtime]$[Import* render_lab_math]$[Import* <package>_values]$[Import* <package>_natives]

$[# myScene {: {^ {t}
    [# s {myStage}]
    …
    {flatten {' figureA figureB {showFrame frame}}}} Math}]
```

## The Lamlet a scene needs

- **The hook** is a binding ascribed `Math`: `$[# myScene {: {^ {t} … result} Math}]`, `t` the seconds (most scenes
  ignore it and read their stage value instead). A helper is `$[# {_name a b} body]` (its name starts with `_`, so it
  is never taken for a hook), or a lambda `{^ {x} body}`; a function `plot` takes is either:
  `$[# {_f x} {* {* 2.0 x} {exp {neg x}}}]`, then `{plot "f" ax _f 0.0 4.0 _curveColor 3.0}`. Numbers are Floats
  (`2.0`, not `2`); std's `exp`, `neg`, `sin`, `sqrt`, `lerp`, `clamp`, `min`, `max` come with `$[Import* std]`.
- **Colours** are Strings: `"#rrggbb"`, or a theme colour's name through the runtime library's `textColor`,
  `textDimColor`, `textFaintColor`, `accentColor`.
- **LaTeX** goes in a template, `f"{\int_{0.5}^{2.5} \lit{1}{f(x)}\,dx}"`: backslashes and braces as LaTeX writes
  them, nothing escaped; `${…}` inserts a value (`f"{\frac{1}{${numberLatex x 1}}}"`). Terms are `\lit{1}`, `\lit{2}`,
  … numbered from 1 in the order the names list gives them (`texParts id latex size color names colors`: the n-th
  name and colour are term n's); `withId figure "name"` picks a term out by its name.
- **Anchors** for layout (`nextTo`, `frameOn`, `brace`) are figures as they stand when the stage is done: build the
  figure once (`[# axes {axesFigure …}]`), lay out from it, and animate the copy you draw
  (`{create {phase s 0.0 1.0} axes}`).
- **A figure that appears at a stage** without an animation of its own: `{fadeIn {phase s 3.0 3.3} fig}` (or
  `{!if {< s 3.0} {'} fig}` for a cut). `tangentAt`, `dotOn`, and the like take the value they move with, not a
  progress.

## A formula built term by term (manim's `Write` on `MathTex`)

Name the terms, colour them by the palette, and write them in; `write` wipes each part in from the left, the parts in
the formula's order.

```text
[# w {texParts "W"
    f"{W = \frac{\lit{1}{w_1 + w_2}}{\lit{2}{M}\,\lit{3}{\hat p(y)}}}"
    0.55
    textColor
    {' "sum" "M" "phat"}
    {' _weightColor textColor _targetColor}}]
{write {phase s 15.0 15.6} w}
```

## Symbols becoming numbers (`TransformMatchingTex`)

Give the same names to the parts that stand for the same thing; `transform` moves each named part from its place in
the first formula to its place in the second (cross-fading its glyphs), and fades the rest.

```text
[# symbols {texParts "est" f"{\lit{1}{\frac{f(y)}{\hat p(y)}} \cdot \frac{\lit{2}{w_1} + \lit{3}{w_2}}{\lit{4}{M}}}" 0.5 textColor {' "fp" "w1" "w2" "M"} colors}]
[# numbers {texParts "est" f"{\lit{1}{1} \cdot \frac{\lit{2}{$w1} + \lit{3}{$w2}}{\lit{4}{2}} = \lit{5}{$value}}" 0.5 textColor {' "fp" "w1" "w2" "M" "value"} colors}]
{transform {phase s 8.0 8.6} symbols numbers}
```

## Numbers flying from a table into a cell or formula (`TransformFromCopy`)

The source stays; a copy flies to the target's place and turns into it.

```text
{fromCopy {phase s 5.0 5.4} {withId bars "pvA"} {cellOf table 1 1}}
```

## A bar growing from its axis, its value appearing (`GrowFromEdge`, `FadeIn`)

```text
[# bar {filled "pA" {box {v2 -5.8 -2.0} {v2 -5.3 0.6}} _pColor 0.9}]
{append {growFrom {phase s 1.0 1.8} bar down} {fadeIn {phase s 1.5 2.0} {nextTo label bar up 0.12}}}
```

## Pointing at a part, then moving on (`Indicate`, `Circumscribe`)

Nest them on the figure they point at; outside their stage they return it unchanged.

```text
{circumscribe {phase s 10.0 11.0} {transform {phase s 9.0 9.6} listA listB} _targetColor}
{indicate {phase s 6.0 7.0} {write {phase s 5.7 6.0} {cellOf table 2 3}} accentColor}
```

To point at some parts of a larger figure, replace them in it:

```text
$[# {_pointParts t mobs ids color} {!if {!and {> t 0.0} {< t 1.0}}
    {append {filter mobs {^ {m} {not {contains ids {idOf m}}}}} {indicate t {filter mobs {^ {m} {contains ids {idOf m}}}} color}}
    mobs}]
```

## A counterfactual: a term struck out, and its effect (a `Cross`, then the figure moving)

Strike the parts (they dim to half), move what depends on them with the same stage, and undo both later in the
beat by running the progress back.

```text
[# struck {- {smooth {phase s 18.0 18.4}} {smooth {phase s 19.0 19.4}}}]
{_crossParts struck formula {' "sum" "M"} _weightColor}
[# value {lerp whole withoutAverage {smooth {phase s 18.0 18.6}}}]
{dotAt "dot" {onLine r a b value} 0.2 _estimateColor}
```

## Braces naming each part (`Brace.get_text`)

```text
[# br {brace "braceM" {withId w "M"} down 0.1 textColor}]
{append br {nextTo {caption "braceMText" {textKey "toymath-count"} 0.34 textColor} br down 0.1}}
```

`braceLabel id figure side gap color label` does both, the label beyond the brace's tip.

## Probabilities as areas

The unit square split by each draw's chances; a list of draws is its cells, its chance their area.

```text
[# cells {areaSquare "sq" {' 0.75 0.25} {' 0.75 0.25} {box {v2 -6.3 -3.4} {v2 -1.8 1.1}} {' "#3b4150"} 0.05}]
{faded {fadeIn {phase s 8.0 8.4} {withId cells "sq.0.0"}} 0.75}
```

## Estimates on a number line, weighted, and their balance point

```text
[# line {numberLine "line" {axisRange 0.0 12.0 2.0} {v2 -1.4 -2.6} {v2 6.6 -2.6} textDimColor 0.32}]
[# dot {dotAt "dot0" {onLine r a b estimate} {+ 0.07 {* 0.3 {sqrt chance}}} _estimateColor}]
[# mean {arrowLine "mean" {sub2 at {v2 0.0 0.95}} {sub2 at {v2 0.0 0.35}} _estimateColor 3.0}]
```

## A curve, what it encloses, and a point riding it (`Axes.plot`, `get_area`, `get_riemann_rectangles`)

```text
[# ax {axesIn {box {v2 -6.6 -0.4} {v2 -0.6 3.4}} {axisRange 0.0 3.0 1.0} {axisRange 0.0 2.0 1.0}}]
[# x {lerp 0.2 2.8 {phase s 4.0 8.0}}]
{flatten {'
    {create {phase s 0.0 1.0} {axesFigure "ax" ax textDimColor 0.32 true true}}
    {write {phase s 1.0 2.0} {riemann "r" ax f 0.0 3.0 8 "#6be3b0" 0.4}}
    {create {phase s 1.0 2.0} {plot "f" ax f 0.0 3.0 "#ffd166" 3.0}}
    {fadeIn {phase s 2.0 3.0} {area "area" ax f 0.5 2.0 "#8fb8ff" 0.35}}
    {tangentAt "tan" ax f x 1.2 "#ff6fb5"}
    {dotOn "dot" ax f x 0.08 "#ff6fb5"}}}
```

## Samples piling up into a histogram over their density

`firstOf` shows the first `count` figures, the next one fading in, so sweeping a count adds bars (or dots) one by one.

```text
[# samples {drawnFrom pdf 0.0 6.0 7 400}]
[# hist {histogram "h" hx samples 0.0 6.0 24 "#ff6fb5" 0.6}]
{append {firstOf {* 24.0 {phase s 3.0 6.0}} {map hist {^ {m} {' m}}}} {create {phase s 6.0 7.0} {plot "d" hx pdf 0.0 6.0 "text" 2.5}}}
```

## A graph laid out from its edges, something flowing along an edge

```text
[# nodes {map {range 0 7} {^ {i} {append {dotAt f"{n${str i}}" origin 0.28 "#3a4a6a"} {tex f"{n${str i}l}" {str i} 0.35 textColor}}}}]
[# g {graph "g" nodes {' {pair 0 1} {pair 0 2} {pair 1 3}} "layered" {box {v2 -6.6 0.4} {v2 -1.0 3.6}} true textDimColor 1}]
{append {write {phase s 0.0 2.0} {graphFigure g}} {flowAlong {phase s 2.0 4.0} {nth {.edges g} 1} 0.1 "#ffd166"}}
```

## Zooming onto a step and back (`MovingCameraScene`)

```text
{showFrame {frameBetween fullFrame {frameOn {append line spread} 0.5} {phase s 22.0 23.0}}}
```

Fade what the zoom leaves behind (`faded … away`), so its edges do not show at the frame's border.
