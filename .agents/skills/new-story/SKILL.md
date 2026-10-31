---
name: new-story
description: Plan, research, write, and review a Render Lab investigation's narrated story (its storyboard and the demonstrations it needs), in phases with subagents - an overview agent, one research agent per subtopic with a round of follow-up questions, a merged research report, a chapter outline pairing every claim with what shows it, a demonstration phase that mirrors how the engine stores each structure and gives the viewer interactions to explore it, the shared tools the demonstrations need, the storyboard written chapter by chapter with its camera and parameter cues, and a per-chapter review from rendered stills. Use when asked to create or plan a new story or video for an investigation, to research a game-engine feature for one, to outline its chapters, or to review a story's depth and animations.
---

# A new story for an investigation

A story is an investigation's narrated video: research into the engine's source, an outline, the demonstrations
it needs, and a storyboard (`packages/<package>/video/storyboard.story`, its format in `tools/video/README.md`).
This skill is the process; `new-investigation` is the standard it meets (§1, precision) and the package it builds
(§3 to §7). Read both.

The main agent designs, decides, edits, and checks. Subagents research and review: Sonnet for research and
review, Haiku for single lookups (AGENTS.md). A subagent knows only its brief, so every brief carries what it
needs (below). A finished subagent keeps its context: send it follow-up questions with SendMessage, rather than
starting a fresh one.

The phases write their work into the package, in `story/`:

```
packages/<package>/story/research.md        phase 1: the merged report
                         outline.md         phase 2: the chapters, and what shows each claim
                         demonstrations.md  phase 3: how each structure is stored, shown, and explored
                         reviews/           phase 7: one review per chapter
```

They are published with the package, like the video that cites the same lines: cite by file, symbol, and line,
and keep quotes short (none at all for an engine whose license forbids them). For an investigation of the optional
`commercial` module they stay local, as the package does.

Keep a guide for the investigation beside its code (`src/<engine>/<feature>/AGENTS.md`) while the story is being
made: the checkout and its commit, the user's preferences for this story (the subject object, what to hide in
close-ups, a camera style), decisions the user made and ones an agent made that are open to change, and pending
requests with what doing them would take. Read it before each phase, and add to it whenever the user decides
something. It is a working file: git ignores it (`.gitignore`, `/packages/**/AGENTS.md`), so it is never published.
**When the story is finished** (the user has watched the built video and has nothing left to adjust), delete the
guide, and tell the user about any pending request in it that was not done, so none is lost with it.

## Phase 1: Research

1. **Overview.** Send one agent to survey the feature in the checkout. It reports the subtopics (four to eight),
   in the order the frame runs them, and for each the entry points, files, and symbols to start from.
2. **One agent per subtopic**, in parallel. Each brief gives:
   - the checkout's path, and the engine's version and commit;
   - the subtopic's scope, its neighbours (so two agents don't cover the same ground), and what is already known
     or ruled out;
   - the rule to follow what runs, not what exists: from the entry point the pipeline executes, through its
     passes and shaders, with every compile-time switch, shader option, console variable, and default on the way
     (`new-investigation` §1);
   - what to return: every claim with its file, symbol, and line; numbers as the code has them (formats, sizes,
     counts, limits, clamps); what depends on configuration, and its default; and what is still unclear.
3. **Questions.** Read every report. Write down what is unclear, contradicts another report, rests on a function's
   name rather than its body, or depends on a switch nobody checked. Send each agent its own questions, for one or
   two rounds at most.
4. **Merge** the reports into `story/research.md`: one section per subtopic, each claim with its file:line, and a
   column (or note) for **where the lab differs**: what the lab already computes another way (a fit where the
   engine samples a table, a CPU stand-in for a GPU pass, a smaller resolution). Catching a difference here is
   cheap; finding it in a finished storyboard means rewriting sentences.
5. **Check in with the user**: the subtopics, the scope, and what is left out. Don't outline until they agree.

## Phase 2: Outline

Write `story/outline.md`:

- **Chapters**, each with a title, a time budget (`targetSeconds`, about 1.2 times its narration), and its beats.
  A chapter is one structure or one pass; a beat is one idea, and its headline states the claim ("local bounds
  become an oriented box"), not the topic ("bounds").
- **Claim → what shows it**, for every key sentence: the claim, its source (file:line), and what is on screen
  while it is said (a demonstration's parameter, a camera move, a highlighted field or listing line, a formula,
  a readout row). A claim with nothing to show is either cut, said in passing, or given a visual.
- **What changes, and what it changes.** For each relation the story explains ("the closer, the sharper"), the
  parameter to sweep and the values that must visibly change, over a range where they do change.
- **Worth a detailed demonstration**: the points where a still or a cut would lose the idea (a lookup worked
  step by step, a vector field, a lobe, a texture and the data it holds).
- **The tools it needs**: overlays, insets, readouts, step parameters.

### The shape of a story

- **Open with what the feature adds, and the problem it solves.** The first beat shows the feature's
  contribution as a comparison: the same camera, exposure, and scene with the feature off and on, cued on what it
  adds (the bounced light, the colour bleeding), not two renders that merely differ. Then state the problem in
  one sentence ("how can the engine light a room with bounced light without tracing many rays for every
  pixel?"), show the architecture map (below), and say what the viewer will be able to explain by the end. Keep
  new terms out of the opening; each arrives in its own chapter.
- **Then where it runs.** The first chapter places it in the engine's frame (a Mermaid beat of the passes, the
  feature's lit) and says what decides whether it runs at all: project settings, quality levels, platform paths,
  and their defaults.
- **Explain each mechanism; showing its data is not explaining it.** A viewer who sees what a structure looks
  like may still not know what it is for. For every mechanism, the story answers, in this order where it fits:
  the **problem** it solves (and what would go wrong without it), an **intuitive example**, the **data** it
  keeps, the **algorithm** (its inputs, the calculation, its outputs), the **result** on screen, the
  **trade-off** it makes (quality, memory, time), and **where its output goes next**. The outline records the
  answers per chapter; a chapter that only shows a structure's layout is not finished.
- **One architecture map, reused.** One diagram of how the feature's parts relate, true to the engine (its
  feedback loops, caches, parallel or once-only stages; never forced into a line it is not), with the same
  names and colours everywhere. Each chapter opens on it with its own part lit, and ends by showing what its
  output feeds. Its terms are the narration's terms.
- **One encoding per kind of data.** Geometry, cells, distances, occlusion, and light (radiance, irradiance)
  are different things, even where one is derived from another: give each its own colour and graphic treatment
  for the whole story (the palette's roles), and never draw one so it reads as another.
- **Follow the data.** Order the chapters the way the data is made and used: built (often offline), stored,
  updated each frame, then read by the passes that need it. Each chapter's first beat says what the structure is
  for, before how it is built.
- **One, then many.** Work one example fully (one sample, one record, one ray, one page), then show it
  generalised across the scene, then several more examples. The worked one is a record in the struct panel
  (`record=…`, `panel=values`, `highlight=` on the field the sentence names). Work it as: highlight the one
  element (a cell, a probe, a texel) in the 3D scene, show its inputs, animate the calculation step by step,
  show its result, then say where that result goes. Simplify the scene to a single object first when the full
  room would hide the step, then return to the room.
- **One continuous example across stages.** Where data passes through several stages, follow the same element
  through all of them (one probe: its rays gather light, the light is compressed, the compressed form is
  evaluated later), so the viewer never rebuilds the picture. Near the end, follow one shaded point on a visible
  surface back through every mechanism it depends on (which data it reads, how it finds it, how occlusion
  weights it, what light it adds), in the engine's real order.
- **Compact representations, intuition first.** Before showing a representation's coefficients (spherical
  harmonics, an octahedral map, a quantised format), say in plain words what it does ("a few numbers that
  describe how light varies smoothly with direction"), then what it keeps, what it loses, why that is worth it,
  and where it is read back. Never assume a method because other systems use it: show what this engine's code
  does.
- **The code, step by step.** Animation comes first; pseudo-code is shown only alongside it. For a shading or
  lookup step, show its pseudo-code listing (`panel=code`) and light the line the sentence is on
  (`highlight=<tag>`), and with every highlight change the demonstration too, so the scene shows what that line
  does (its rays, its samples, its threshold, its result); a highlight that changes nothing on screen is cut, or
  the demonstration gains the step it lacks. Show only the
  lines the step needs, at a size that reads on a laptop, with the inputs and outputs of the lit line beside it
  (a readout row or label), and say whether it is the engine's code, the lab's pseudo-code of it, or the lab's
  own demonstration.
- **End with the whole, and three takeaways.** The recap returns to the full scene, shows each mechanism's
  contribution (switched off and on, in frame order), and closes on three takeaways: how the feature organises
  its data, how it computes and reuses light, and what accuracy it trades for speed.
- **Show the bytes.** Where the story names a format or a layout, show the record's fields and sizes, and how a
  stored value decodes to what the shader uses; map each record to the engine's structure in the index's
  `mappings`.
- **A subject that shows it.** Choose the scene's object whose shape shows the feature best, and fade the ones
  that would hide it in close-ups.
- **Mechanisms the lab doesn't run** (an offline build, a streaming policy) are still shown from a CPU
  computation that follows the engine's code, in a module of their own; say on screen when a view is schematic or
  scaled.
- **Bridge every chapter to the last.** A chapter's first sentence says what the previous one left, and what
  this one does with it; only then does the chapter start. Vary the wording, and give the reason the step comes
  next when there is one. From O3DE's story:
  - "With the lights sorted into lists, the next part turns to what they light: the surface."
  - "Before any light reaches that surface, decals can change it, and they are the next part."
  - "So far light has only bounced off surfaces; the next part lets it pass through them: transmission."
  - "That covers the simple point light; the next part leaves punctual lights behind."

  Beats within a chapter link the same way when the step changes ("once every brick is stored, the lookup …"),
  and the story ends with a short recap chapter that walks the frame again, naming each part in order.
- **Recap each chapter.** Before its sources beat, a chapter ends with two or three sentences: what it just
  explained, why it matters to the final image, and what the next chapter does with it.
- **Close each chapter with its sources**: a `sources` beat, "where to read it", listing the chapter's index keys
  in reading order, and a reading table in the index's `chapters`. The build turns them into the source-reading
  guide (`readingPath` in `video.json` orders its recommended path).

**Check in with the user** on the chapters and budgets before building anything.

### Reusing tools

For each tool the outline needs, look for it first:

- **In the lab** (`lab/`, `packages/rlstd`): use it.
- **In a public package**: reuse it in place, or, when a second investigation needs it, move it into the lab as
  a generic extension point (the `Investigation` trait, `FrameContent`, the dock kit) that names no investigation.
- **In a commercial package**: code derived from an engine whose license does not allow publishing never moves
  into the lab or a public package, however generic it looks. What can move is a generic mechanism written from
  scratch or from published methods (papers, talks), with no engine names, parameters, or text keys: for example,
  a picture inset the lab draws (`lab::utility::inset::PictureInset`, in `FrameContent::insets`: CPU pixels or a
  GPU texture the renderer keeps by key), while the engine-specific table that fills it stays in the package. If you are
  unsure whether a piece is derived, it stays where it is; ask the user.
- **Nowhere**: build it, in the investigation's package unless other investigations will need it.

Moving code keeps the layering: the lab never depends on an investigation, and nothing published names or
mirrors the commercial module's contents (AGENTS.md).

## Phase 3: Demonstrations, and how a viewer explores them

A phase of its own, before anything is built on the outline: find what each structure the story explains can
show, and give the viewer a way to explore it. Write it to `story/demonstrations.md`.

1. **How the engine stores it.** For each data structure in the outline (a texture, an atlas, a buffer, a
   table), send a research agent to the checkout (Phase 1's brief) for: its resources, each one's dimension,
   format, and size; which are separate resources and which are packed together (channels, layers, tiles of one
   atlas); how an element is addressed (page tables, indirection, tile coordinates); and the engine's own debug
   views of it (its visualize modes, show flags, debug draws). A debug view says what its authors found worth
   seeing.
2. **Mirror the storage.** Show the structure as the engine keeps it. Separate resources are separate views,
   side by side, each titled with its name and format, sharing its layout (the same element outlined in each);
   channels packed in one resource are one view with its channels decoded. Do not merge what the engine
   separates, or split what it packs, for convenience.
3. **Choose the subject.** Show each structure on the object whose data reads best: an irregular one (a teapot,
   a sphere) rather than a wall or a box, whose cards, bricks, or probes are too simple to tell apart, and the
   same object across chapters, so the viewer does not have to learn a new one. Keep the story's worked
   examples where its thread needs them.
4. **Interaction for each view.** For each one, write what a viewer can do with it in the viewer (a click on the
   scene or on an inset, a slider, a toggle), and what that shows. A click picks an element and shows it
   everywhere it lives at once: in the scene, in every view of the same layout, and in a readout of its value in
   each resource. Interactions use the lab's extension points (`Investigation::click`, `#[click]` handlers on
   an inset, the dock's values), and the values they set are values a story can set too, so a story can show
   the same pick.
5. **Build and check them** before the base: the views, the clicks, their values, and a selftest check that a
   pick round-trips (a click on what an element stands for picks it). Render stills of each view, and look at
   them.
6. **Check in with the user**: the views, the subjects, and the interactions.

## Phase 4: The base

Before the storyboard, build what it stands on (`new-investigation` §2, steps 3 to 6):

- the source-index entries for every claim in the outline, with their hashes from the checkout;
- the port, its CPU mirror, and the selftest's comparisons and invariants;
- the pseudo-code listings and formulas;
- a step parameter for every intermediate step the outline shows, each defaulting to "no change", with a slider
  in the dock.

## Phase 5: The storyboard, a chapter at a time

Write each chapter's sentences and their cues together, not the prose first: a sentence written without its
visual often names something nothing can show.

- Open the chapter with its bridge from the last (see "The shape of a story"), and write it when the chapter
  before it is written, so it names what that chapter really ended on.
- One paragraph per sentence; spoken numbers; say where the lab differs. Add a word the voice gets wrong to
  `pronunciations` in `video.json`.
- Motivation before mechanism, a concrete example before the abstraction, and at most one new term per
  sentence, each introduced the first time it is needed. Say each conclusion once, and give it a pause
  (`delay`) before the next idea. Never speed the voice up or cut explanation to fit a budget: raise the
  chapter's budget instead.
- Numbers about the lab's scene come from the lab (its code computes them, the selftest asserts the ones that
  matter): never type a measured value into the narration.
- The beat's first line of tags sets its starting keys, and resets the step parameters of the beat before it
  (instant keys, no `blend`).
- Images, the callout, the camera, and the panel are not step parameters: they carry into the next beat, across a
  demonstration switch too. Give each beat's first line `callout=""` and `image=""` (or `imageHide=` of the
  images it no longer needs) unless it keeps them, a `panel` (`hidden` when the investigation has no struct and
  the beat no readout or listing), and reset any camera key the beat before set.
- Cue each visual on the word that names it: `{@ rayStep=2}its distance`, not at the sentence's start.
- Wrap each code name the narration says (a function, struct, file, or console variable) in a code span,
  `{@ codespan="LightCullingTilePrepare"}`, in the translation too; give it `say="…"` when the voice would misread it.
- Work that unfolds over time (an algorithm's passes, a sweep) is an action the package declares, run from the story
  (`{@ run=jumpFlood run.seconds=0.5}` … `{@ stop=jumpFlood}`; `render-lab actions`), rather than a parameter keyed
  step by step: the story says when and how fast, the package's code what each moment shows.
- A picture (a slice, a table, a chart, a PNG) is an image the story shows by key, `{@ image=floodSeeds
  imageCorner=top-left}` … `{@ imageHide=floodSeeds}` (`render-lab images`): the package describes it, the story places
  it. A GPU image is an instance of an image kind the package's shaders define, made and driven by the story:
  `{@ image=near imageFrom=probeMap image.probe=7,7,8}`, then `{@ image=near image.probe=10,7,8 blend=2}`. An image a
  viewer may click when paused names the package's click handler: `{@ image=map imageClick=pickCell}`
  (`render-lab actions` lists them).
- **Say how to read every texture or buffer view the moment it appears**, before the story uses what it shows:
  what it is (the resource, its format), what its axes or layout are (screen pixels, tiles, texels of a
  cubemap's faces, a list's slots), what each colour or brightness means and its range (black is zero
  occlusion, yellow is eight lights, a texel's red is the scale), and where the element the sentence is about
  sits in it (the ring on P's pixel, the picked tile's row). Explain it in detail, not as a legend: what the
  resource stores and in what range, how the view maps that to colour, a reading of one marked part and why
  it looks that way (the floor is light green because it faces up, along y), the pattern to look for (where
  the values jump, and what that means), and what the next pass does with it. Go slowly: one fact per
  sentence, each after a short pause (`{@ delay=0.4}`), and light the part each sentence names while it is said: a region of the
  image (`{@ image=t0 imageMark=u0,v0,u1,v1 imageMarkText=<key>}`, cleared with `imageMark=""` on the
  sentence after), or a label or arrow at the world point a 3D view shows. A view shown and passed over, or
  one whose colours the viewer has to guess, is not finished: add the sentences, or cut the view. Say it
  again, briefly, when the view returns after other views have come between.
- An arrow or label that points something out is the story's: `{@ arrow=a arrowFrom=x,y,z arrowTo=field.seed}`,
  `{@ label=l labelAt=… labelText=<text key>}` (`arrowHide`, `labelHide`), its points fixed or ones the package publishes,
  and `{@ arrows=<set>}` for a set the package computes (`render-lab images` lists the points and sets).
- Keep the camera floating in 3D (`animate=[{"path": …}]`, `cameraDrift`), and glide to a still, 2D view
  (`cameraDrift=0`) only for a detailed step; glide back afterwards.
- Sweep the parameter a relation depends on while the sentence states it, so the values change on screen.
- A `callout` on about half the sentences, and the rest covered by the chapter's `sources`.

After each chapter, run `--prepare-only` and render a few stills across each animated beat:

```sh
python3 tools/video/build.py --investigation <engine>/<feature> --prepare-only
cargo run --release -- --investigation <engine>/<feature> render --timeline <timeline.json> \
    --snapshot A,B,C --frames A0:C1 --snapshot-dir <scratch>/stills    # A0 a little before A: blends need the frames before
```

The caption frames are in the timeline's `beats[].captions` (start, end, text). Look at the stills yourself
before moving on.

## Phase 6: Showing the data

A pass over the drafted chapters for what the story shows, not what it says.

**Make this feel more like a video instead of a powerpoint.** Something on screen moves while every sentence is
said: a sweep, a step, a camera glide, rays drawing in, a value counting. Still beats are the exception.

- **At least 32 animations per chapter**: parameter sweeps, step-by-step reveals, actions (`run=`), camera paths.
  Each is driven by a value the interactive view shows (a slider or a toggle in the dock), so a viewer can replay
  it, stop it anywhere, and change it; a timeline sets only values the dock can set. Count them per chapter
  before the review.
- **Every animation demonstrates something.** Reach the count with animations that show how the feature works:
  a ray marching to its hit, a texel picked and read, a count growing until the noise settles, a step of the
  algorithm played out. Don't pad it with motion that explains nothing: a filler sweep, a camera move with no
  subject, a value changed and changed back, or a whole-image swing (a mix or a frame count jumping mid-beat),
  which also flashes. When a chapter falls short, add a demonstration where a sentence names a mechanism, and
  cut an animation that only adds motion.
- **Interactions**: clicking scene elements (an object, a card, a probe, a point) and image elements (a texel, a
  probe's map, a slice) picks them; the pick is shown everywhere it lives at once, with arrows and connection
  lines between them (from a texel in an inset to its point in the scene, from a probe to its map, from a ray's
  hit to the record it reads). Stories show the same picks by setting the values a click sets, with the same
  arrows and lines.

- **Vectors, lobes, rays, and points** drawn where the narration names them, labelled, in the palette's colours.
- **Textures and the data in them**: show the texture, which texel a sample reads, and how its channels map to
  the values in the readout or struct panel; say how to read it (Phase 5).
- **A view of the screen's data beside the frame**, not a small inset over it: when a structure is laid out
  over the screen (tiles, a per-pixel buffer, a debug view mode), split the scene area into two views of the
  same camera (`Investigation::panes`), the structure on the left and the lit frame on the right, so every
  element lines up with what it stands for. Never show the same view twice (the scene in a debug mode and an
  inset of it).
- **Each check, per beat:**
  - *One moment, one idea.* What is on screen at once serves the sentence being said: one lit element, the
    rest dimmed or faded; no second dense panel (a listing and a readout and an inset together) unless the
    sentence compares them; complex pictures revealed in steps, not all at once; intermediate values only where
    they help. Nothing decorative that takes time without explaining. It must still read at laptop and phone
    size.
  - *The change is visible.* A sweep that leaves every value the same (because it is clamped or capped over
    that range) shows nothing: move the sweep to where the values change, or say why they don't.
  - *Nothing blocks the subject.* Validate, from stills at the start, middle, and end of every beat (a camera
    path or a moving light can swing an occluder in mid-beat), that what the sentence is about is in view:
    no wall, object, panel, inset, or readout between the camera and the worked example, the vector, the ray,
    or the label being named, and the subject inside the viewport and the camera's near and far planes. A
    camera outside the room sees the room through its walls: keep it inside, move it, or fade the walls. Fade an
    object that hides the worked example, and bring it back for wide shots. Look at the stills yourself; a check
    that only reads numbers cannot see this.
  - *Labels fit.* Labels inside the viewport and apart, text at least 24 px, the dock inside its width (the
    selftest checks these).

## Phase 7: Review, per chapter

Render a contact sheet of stills for each chapter (several per animated beat). Renders are GPU jobs, so the main
agent makes them one at a time and gives the reviewers the image paths.

Send one reviewer per chapter. Each gets the chapter's storyboard text, its outline section, its stills, and
these questions:

- **Depth.** Is each step explained, not just named? What would a viewer who knows graphics but not this engine
  still not understand?
- **Intuition.** Does each animation make its point clearer than a still would? Which sentence points at
  nothing on screen?
- **Highlighting.** Is each key concept lit (a highlight, a callout, a lit formula term, a label) while it is
  named?
- **Code and animation.** At each listing highlight, does the scene change to show that line's work? Which
  highlight leaves the scene as it was?
- **Change.** Where the narration says a value depends on another, does a sweep show it changing?
- **Visibility.** In every still, is what the sentence names in view, or does a wall, an object, an inset, or
  the readout block it, or does it fall outside the frame or past the far plane?
- **Reading the views.** For every texture or buffer view, does the narration say, when it appears, what it is,
  its layout, what its colours mean, and where the element in question sits in it? Which view is shown and
  passed over?
- **Flow.** Does the chapter's first sentence bridge from the one before, and does each beat follow from the
  last? Where does the viewer lose the thread?
- **Fit.** Is the chapter near its budget?
- **Purpose.** Could a viewer who has just watched it explain not only what happens, but why, and where the
  result is used next? For each mechanism, which of the questions in "The shape of a story" (problem, data,
  algorithm, result, trade-off, next stage) does it leave unanswered?
- **Mental model.** Act as a senior rendering engineer and a graphics professor, and do not simply agree with
  the draft: find missing reasoning steps, wrong causal links, unexplained terms, terms used inconsistently,
  data shown as if it were computation (or one kind of data drawn as another), oversimplified mechanisms, and
  visuals that look convincing but teach the wrong model. Don't ask for complexity for its own sake, nor for
  less depth to make it easier.
- **Load.** Does any moment show more than one idea, or a panel denser than the sentence needs? Is the code
  only the lines that matter, and readable?

Each reviewer returns a table, `| Beat (or timestamp) | Issue | Severity | Why it matters | Specific correction |`,
specific enough to act on ("add a sweep of `rayCount` 1 → 16 while the second sentence is said", never "improve
clarity").

Send one more agent, not anchored on the drafts, to check the facts (`new-investigation` §2, step 8): every
factual sentence and every `inspect`, against the checkout, in the classes of that step, with file:line
evidence. Uncertain claims are never rewritten as facts: they are verified, narrated as uncertain, or cut.

Save the reviews in `story/reviews/`. Then revise: more animation where the reviewers found a still idea, more
explanation where they found a gap, and every fact that is not verified (or a stated simplification) fixed in
the narration, the index, the README, and the translation. Report the corrections to the user with their
evidence.

## Phase 8: Translate and build

- Write `storyboard.zh-CN.story`: the same headings, one paragraph per English sentence, the same formulas, no
  cues. Technical terms stay English; keep its `glossary:` of the terms' translations consistent. The Chinese
  edition keeps the English narration, timed by the English sentences.
- Check (one build or GPU job at a time):
  - `cargo build --release --locked --offline`, and each local package's workspace, with no warnings;
  - `--prepare-only` for both languages;
  - the selftest with `--timeline`;
  - the stills of every beat (`--snapshots-only`);
  - the full build in the background, English first, then `--lang zh-CN`.
- Report the checks and the corrections, and ask the user to watch the result.

## Revising a finished story

When a story is built and a viewer (the user, or a review they pass on) asks for it to be better, improve it;
don't remake it. Review feedback is input to check, not a script: verify each point against the video and the
checkout before acting on it, say where it is wrong, and look for what it missed.

1. **Audit.** Watch the built edition through its stills (the beats the feedback names, and the rest) and map
   each point to beats (feedback timestamps are approximate: use the edition's `chapters.txt` and the timeline's
   captions). Run Phase 7's reviews and fact check over the whole story.
2. **Plan.** Rank the corrections by what they do for understanding and correctness, in a table like the
   reviewers'. Keep what works (named, so it is not lost in the rewrite); change only what is needed.
3. **Revise** the storyboard, its translation, the demonstrations, and the diagrams, in Phase 5 and 6's rules.
   A chapter may grow longer when that buys understanding.
4. **Validate and deliver** as in Phase 8, with a short change log of what changed and why, and the
   corrections with their evidence.
