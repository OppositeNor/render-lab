# Phases 5 and 6: the storyboard, and what it shows

Load this to write or revise storyboard sentences and their cues, from one cue to a whole chapter. The storyboard's
syntax (headings, cue forms, ranges, motions, the translation's form) is the `render-lab-lamlet` skill's; this file is
what a story says and shows. Keys written here as timeline keys (`panel`, `record`, `highlight`, `run`, `delay`) are
written in a storyboard with their vocabulary functions (`{panelValues}`, `{highlight {' "tag"}}`, `{run "name"}`,
`{delay 0.4}`): `render-lab --investigation <id> story keys` lists them.

## Phase 5: the storyboard, a chapter at a time

Write each chapter's sentences and their cues together, not the prose first: a sentence written without its visual
often names something nothing can show.

### Sentences

- Open the chapter with its bridge from the last (`references/outline.md`, "The shape of a story"), and write it when
  the chapter before it is written, so it names what that chapter really ended on.
- One paragraph per sentence (required: the translation and the timing are by sentence); spoken numbers; say where the
  lab differs. Add a word the voice gets wrong to `pronunciations` in `video.json`.
- **One primary idea per sentence.** Each sentence has one instructional objective (one step, one comparison, one
  result, one change on screen), and the picture changes in step with it. A sentence may hold a comparison ("the
  coarse level steps further than the fine one"), a cause and its effect ("the column is taller, so the ray stops
  here"), or two closely related facts when splitting them would lose the relation between them. Split when a second
  clause brings a new term, a new step, or a second change on screen: do not chain such ideas with "and", a colon, or
  a semicolon, but give the second its own sentence and its own cue. Animate one change per sentence, in order (a
  stepped algorithm advances a fraction of a step per sentence, each fraction one change; see SSR's trace stepper,
  `traceStep`).
- **Not too dense.** Leave room between ideas: a pause after each result, a beat that only shows before the next one
  explains, and no more facts in a chapter than a viewer can take in on one viewing. If a beat lists many values or
  steps, show a few and summarise the rest ("up and down it goes, a level at a time"). Raise the chapter's budget
  rather than packing more into each second.
- Motivation before mechanism, a concrete example before the abstraction, and at most one new term per sentence, each
  introduced the first time it is needed. Say each conclusion once, and give it a pause (`delay`) before the next idea.
  Never speed the voice up or cut explanation to fit a budget: raise the chapter's budget instead.
- **Numbers about the lab's scene come from the lab** (required): its code computes them, the selftest asserts the
  ones that matter; never type a measured value into the narration. Where the package's code computes one, the
  narration says it from there (`Investigation::script_natives`: `${spoken {dispatches}}`), and a number both a cue and
  a sentence use is one binding (`{coneStepsRange 0.0 {float {lowSteps}}}` and `${spoken {lowSteps}} steps`).
- Wrap each code name the narration says (a function, struct, file, or console variable) in a code span,
  `${code "LightCullingTilePrepare"}`, in the translation too; write it `${say {code "…"} "…"}` when the voice would
  misread it.
- Every factual sentence is backed by a `callout` on it or by its chapter's `sources` (required); a `callout` on about
  half the sentences, the rest covered by the chapter's `sources`.

### Written for the ear: clear and in-depth

The narration is heard once, at speaking speed, by the story's listener (its card, `references/outline.md`). It must be
**clear**: every sentence followed on one hearing. It must be **in-depth**: every mechanism explained, each fact saying
why it is so or what it changes. Clarity is never bought by cutting depth: a dense passage becomes more sentences, not
fewer facts. Write the draft for accuracy, then edit it for the ear under these rules (required), then check its facts
again, since rewording can change a claim.

- **Subject and verb first.** At most one short phrase before them. Never open with a participle ("Averaged in as
  black, …") or with a formula.
- **One appended phrase at most.** A second fact hung on a sentence with a comma becomes its own sentence.
- **No colons or semicolons** in narration. Two sentences, or "so", "because", "but".
- **A formula is said in words first**, then shown: `The level is ${formulaSaid f"…" "the log, base two, of …" {'}}.`
- **Every pronoun has one referent**: "it", "its", "the result", "the level" can each point at only one thing in the
  sentence before; when they could point at two, repeat the noun.
- **Define a term where it is first said, with what it is for**, and recall one from an earlier chapter where it is
  used again ("the validity, the alpha from chapter five").
- **Every fact says why, or what it changes.** A fact with neither is cut from the narration (the listing and the
  sources card still show it). A constant the engine gives no reason for is stated as the engine's choice, with no
  invented reason.
- **Say where a fact comes from at the start**, in one fixed form ("The code shows that …"), or leave it to the
  callout. Never append it ("…, read from the code").
- **Consequences, not motives.** Say what a step does and what follows from it. Say why an engine does it only when its
  source or its authors say so.
- **Re-check every edited sentence** against this list, not only the one a review flagged: a fix often breaks a rule
  elsewhere.

Before and after, from the stories:

| Before | After |
|---|---|
| Invalid texels still count, a fifth as much as a valid one at level one, as much at level five. | Some texels hold no colour, because their trace found no valid hit. / Each of them is averaged in as black, which darkens the blur around it. / On level one, such a texel counts only a fifth as much as a valid one, so it darkens the blur less. / That share grows with each level, and from level five up such a texel counts fully. |
| A hit facing away is rejected only within two SSR pixels of the start; farther hits are not tested for facing. | A hit on a surface that faces away from the ray is rejected, but only within two SSR pixels of the start. / Farther away, the trace never checks which way the surface faces. |
| One last pass with a step of one fixes the cells whose seed was a cell off, the error searching at half size leaves, shown here in orange. | Searching at half size leaves some cells with a seed one cell off. / They are shown here in orange. / One last pass, with a step of one, fixes them. |
| $\frac{c}{1 + L(c)}$: before any blur, each colour is divided by one plus its luminance, so one bright texel cannot dominate an average. | Before any blur, each colour is divided by ${formulaSaid …"one plus its luminance"…}. / That way, one bright texel cannot dominate an average. |

A slash separates the sentences, each its own paragraph.

**The translation is written as Chinese**, not mirrored from the English: still one paragraph per English sentence,
but restructured freely inside it. Put the verb early. A noun takes at most one 的-modifier before it; a longer one
becomes its own clause. Split with 。 rather than chaining with ，. Keep each term the same across the edition (one word
for "the level before", not 上一级 here and 下一级 there). Avoid the constructions that read as translated: 对于……,
取而代之的是, 依据是……, 在……下 stacked on another preposition, and passives like 被完整计入; prefer a clause of time or
condition (……时), the active voice, and a pass as the subject. Say quantities plainly (边长是上一份的一半, not 小一半).

In English, read a formula in its true grouping ("the log, base two, of one plus the radius over sixteen", not "… over
sixteen, plus one"), and keep its words in one stretch rather than split around the formula's card.

### Cues

- The beat's first paragraph of cues sets its starting keys, and resets the step parameters of the beat before it
  (instant keys, no `blend`).
- Images, the callout, the camera, and the panel are not step parameters: they carry into the next beat, across a
  demonstration switch too. Give each beat's first cues `{callout ""}` and `{inset ""}` (or `imageHide` of the images
  it no longer needs) unless it keeps them, a `panel` (`{panelHidden}` when the investigation has no struct and the
  beat no readout or listing), and reset any camera key the beat before set.
- Cue each visual on the word that names it: `${cue "key" {' {rayStep 2.0}}}its distance`, not at the sentence's
  start.
- Work that unfolds over time (an algorithm's passes, a sweep) is an action the package declares, run from the story
  (`{run "jumpFlood"} {runSeconds 0.5}` … `{stop "jumpFlood"}`; `render-lab actions`), rather than a parameter keyed
  step by step: the story says when and how fast, the package's code (or script) what each moment shows.
- A picture (a slice, a table, a chart, a PNG) is an image the story shows by key, `{inset "floodSeeds"}
  {imageCornerTopLeft}` … `{imageHide {' "floodSeeds"}}` (`render-lab images`): the package describes it, the story
  places it. A GPU image is an instance of an image kind the package's shaders define, made and driven by the story:
  `{inset "near"} {imageFrom "probeMap"} {imageProbe {' 7.0 7.0 8.0}}`, then `{inset "near"} {imageProbe {' 10.0 7.0
  8.0}} {blend 2.0}`. An image a viewer may click when paused names the package's click handler: `{inset "map"}
  {imageClick "pickCell"}` (`render-lab actions` lists them).
- An arrow or label that points something out is the story's: `{arrow "a"} {arrowFrom {at x y z}} {arrowTo {point
  "field.seed"}}`, `{label "l"} {labelAt {at …}} {labelText "<text key>"}` (`arrowHide`, `labelHide`), its points
  fixed or ones the package publishes, and `{arrows "<set>"}` for a set the package computes (`render-lab images`
  lists the points and sets).
- Keep the camera floating in 3D (`{motion {' {cameraPath {' {' 18.0 16.0 6.2} …}}}}`, `cameraDrift`), and glide to a
  still, 2D view (`{cameraDrift 0.0}`) only for a detailed step; glide back afterwards.
- Sweep the parameter a relation depends on while the sentence states it, so the values change on screen.

### Reading a texture or buffer view

Say how to read every texture or buffer view the moment it appears, before the story uses what it shows (required):
what it is (the resource, its format), what its axes or layout are (screen pixels, tiles, texels of a cubemap's faces,
a list's slots), what each colour or brightness means and its range (black is zero occlusion, yellow is eight lights, a
texel's red is the scale), and where the element the sentence is about sits in it (the ring on P's pixel, the picked
tile's row).

Explain it in detail, not as a legend: what the resource stores and in what range, how the view maps that to colour, a
reading of one marked part and why it looks that way (the floor is light green because it faces up, along y), the
pattern to look for (where the values jump, and what that means), and what the next pass does with it. Go slowly: one
fact per sentence, each after a short pause (`{delay 0.4}`), and light the part each sentence names while it is said: a
region of the image (`{inset "t0"} {mark u0 v0 u1 v1} {imageMarkText "<key>"}`, cleared with `unmark` on the sentence
after), or a label or arrow at the world point a 3D view shows. A view shown and passed over, or one whose colours the
viewer has to guess, is not finished: add the sentences, or cut the view. Say it again, briefly, when the view returns
after other views have come between.

### Check each chapter

After each chapter, run `--prepare-only` and render a few stills across each animated beat:

```sh
python3 tools/video/build.py --investigation <engine>/<feature> --prepare-only
cargo run --release -- --investigation <engine>/<feature> render --timeline <timeline.json> \
    --snapshot A,B,C --frames A0:C1 --snapshot-dir <scratch>/stills    # A0 a little before A: blends need the frames before
```

The timeline is the package's `build/timelines/<lang>/timeline.json`; the caption frames are in its
`beats[].captions` (start, end, text). Look at the stills yourself before moving on.

## Phase 6: showing the data

A pass over the drafted chapters for what the story shows, not what it says.

**Make this feel more like a video instead of a powerpoint.** Something on screen moves while every sentence is said:
a sweep, a step, a camera glide, rays drawing in, a value counting. Still beats are the exception.

- **At least 32 animations per chapter** (required for a new or rewritten chapter): parameter sweeps, step-by-step
  reveals, actions (`run`), camera paths. Each is driven by a value the interactive view shows (a slider or a toggle
  in the dock), so a viewer can replay it, stop it anywhere, and change it; a timeline sets only values the dock can
  set. Count them per chapter before the review. A local edit to one beat keeps its chapter's count from falling below
  it.
- **Every animation demonstrates something** (required; it is what the count means). Reach the count with animations
  that show how the feature works: a ray marching to its hit, a texel picked and read, a count growing until the noise
  settles, a step of the algorithm played out. Don't pad it with motion that explains nothing: a filler sweep, a
  camera move with no subject, a value changed and changed back, or a whole-image swing (a mix or a frame count
  jumping mid-beat), which also flashes. When a chapter falls short, add a demonstration where a sentence names a
  mechanism, and cut an animation that only adds motion.
- **Interactions**: clicking scene elements (an object, a card, a probe, a point) and image elements (a texel, a
  probe's map, a slice) picks them; the pick is shown everywhere it lives at once, with arrows and connection lines
  between them (from a texel in an inset to its point in the scene, from a probe to its map, from a ray's hit to the
  record it reads). Stories show the same picks by setting the values a click sets, with the same arrows and lines.
- **Vectors, lobes, rays, and points** drawn where the narration names them, labelled, in the palette's colours.
- **Textures and the data in them**: show the texture, which texel a sample reads, and how its channels map to the
  values in the readout or struct panel; say how to read it (above).
- **A view of the screen's data beside the frame**, not a small inset over it: when a structure is laid out over the
  screen (tiles, a per-pixel buffer, a debug view mode), split the scene area into two views of the same camera
  (`Investigation::panes`), the structure on the left and the lit frame on the right, so every element lines up with
  what it stands for. Never show the same view twice (the scene in a debug mode and an inset of it).
- **Each check, per beat:**
  - *One moment, one idea.* What is on screen at once serves the sentence being said: one lit element, the rest dimmed
    or faded; no second dense panel (a listing and a readout and an inset together) unless the sentence compares them;
    complex pictures revealed in steps, not all at once; intermediate values only where they help. Nothing decorative
    that takes time without explaining. It must still read at laptop and phone size.
  - *The change is visible.* A sweep that leaves every value the same (because it is clamped or capped over that
    range) shows nothing: move the sweep to where the values change, or say why they don't.
  - *Nothing blocks the subject.* Validate, from stills at the start, middle, and end of every beat (a camera path or a
    moving light can swing an occluder in mid-beat), that what the sentence is about is in view: no wall, object,
    panel, inset, or readout between the camera and the worked example, the vector, the ray, or the label being named,
    and the subject inside the viewport and the camera's near and far planes. A camera outside the room sees the room
    through its walls: keep it inside, move it, or fade the walls. Fade an object that hides the worked example, and
    bring it back for wide shots. Look at the stills yourself; a check that only reads numbers cannot see this.
  - *Labels fit.* Labels inside the viewport and apart, text at least 24 px, the dock inside its width (the selftest
    checks these with `--timeline`).
