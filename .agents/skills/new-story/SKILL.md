---
name: new-story
description: Plan, research, write, and review a Render Lab investigation's narrated story (its storyboard and the demonstrations it needs), in phases with subagents - an overview agent, one research agent per subtopic with a round of follow-up questions, a merged research report, a chapter outline pairing every claim with what shows it, the shared tools the demonstrations need, the storyboard written chapter by chapter with its camera and parameter cues, and a per-chapter review from rendered stills. Use when asked to create or plan a new story or video for an investigation, to research a game-engine feature for one, to outline its chapters, or to review a story's depth and animations.
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
packages/<package>/story/research.md    phase 1: the merged report
                         outline.md     phase 2: the chapters, and what shows each claim
                         reviews/       phase 6: one review per chapter
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

- **Open with where the feature runs.** The first chapter places it in the engine's frame (a Mermaid beat of the
  passes, the feature's lit) and says what decides whether it runs at all: project settings, quality levels,
  platform paths, and their defaults.
- **Follow the data.** Order the chapters the way the data is made and used: built (often offline), stored,
  updated each frame, then read by the passes that need it. Each chapter's first beat says what the structure is
  for, before how it is built.
- **One, then many.** Work one example fully (one sample, one record, one ray, one page), then show it
  generalised across the scene, then several more examples. The worked one is a record in the struct panel
  (`record=…`, `panel=values`, `highlight=` on the field the sentence names).
- **The code, step by step.** For a shading or lookup step, show its pseudo-code listing (`panel=code`) and light
  the line the sentence is on (`highlight=<tag>`), while the demonstration runs the same step.
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

## Phase 3: The base

Before the storyboard, build what it stands on (`new-investigation` §2, steps 3 to 6):

- the source-index entries for every claim in the outline, with their hashes from the checkout;
- the port, its CPU mirror, and the selftest's comparisons and invariants;
- the pseudo-code listings and formulas;
- a step parameter for every intermediate step the outline shows, each defaulting to "no change", with a slider
  in the dock.

## Phase 4: The storyboard, a chapter at a time

Write each chapter's sentences and their cues together, not the prose first: a sentence written without its
visual often names something nothing can show.

- Open the chapter with its bridge from the last (see "The shape of a story"), and write it when the chapter
  before it is written, so it names what that chapter really ended on.
- One paragraph per sentence; spoken numbers; say where the lab differs. Add a word the voice gets wrong to
  `pronunciations` in `video.json`.
- Numbers about the lab's scene come from the lab (its code computes them, the selftest asserts the ones that
  matter): never type a measured value into the narration.
- The beat's first line of tags sets its starting keys, and resets the step parameters of the beat before it
  (instant keys, no `blend`).
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

## Phase 5: Showing the data

A pass over the drafted chapters for what the story shows, not what it says:

- **Vectors, lobes, rays, and points** drawn where the narration names them, labelled, in the palette's colours.
- **Textures and the data in them**: show the texture, which texel a sample reads, and how its channels map to
  the values in the readout or struct panel.
- **Each check, per beat:**
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

## Phase 6: Review, per chapter

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
- **Change.** Where the narration says a value depends on another, does a sweep show it changing?
- **Visibility.** In every still, is what the sentence names in view, or does a wall, an object, an inset, or
  the readout block it, or does it fall outside the frame or past the far plane?
- **Flow.** Does the chapter's first sentence bridge from the one before, and does each beat follow from the
  last? Where does the viewer lose the thread?
- **Fit.** Is the chapter near its budget?

Send one more agent, not anchored on the drafts, to check the facts (`new-investigation` §2, step 8): every
factual sentence and every `inspect`, against the checkout, classed as exact, imprecise, wrong, or unverifiable,
with file:line evidence.

Save the reviews in `story/reviews/`. Then revise: more animation where the reviewers found a still idea, more
explanation where they found a gap, and every imprecise or wrong fact fixed in the narration, the index, the
README, and the translation. Report the corrections to the user with their evidence.

## Phase 7: Translate and build

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
