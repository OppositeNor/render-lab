# Phase 2: Outline

Load this to outline a new story, to restructure a chapter, or when a review asks whether a chapter teaches what it
should. It holds what the outline records, how the story's knowledge level is chosen, how a topic is taught, the shape
of a story, and where its tools come from.

Write `story/outline.md`:

- **Chapters**, each with a title, a time budget (the chapter heading's `target=` seconds, `targetSeconds` in the
  timeline; about 1.2 times its narration), and its beats. A chapter is one structure or one pass; a beat is one idea,
  and its headline states the claim ("local bounds become an oriented box"), not the topic ("bounds").
- **Claim → what shows it**, for every key sentence: the claim, its source (file:line, or the paper's section, page,
  and equation), and what is on screen while it
  is said (a demonstration's parameter, a camera move, a highlighted field or listing line, a formula, a readout row).
  A claim with nothing to show is either cut, said in passing, or given a visual.
- **What changes, and what it changes.** For each relation the story explains ("the closer, the sharper"), the
  parameter to sweep and the values that must visibly change, over a range where they do change.
- **Worth a detailed demonstration**: the points where a still or a cut would lose the idea (a lookup worked step by
  step, a vector field, a lobe, a texture and the data it holds).
- **The tools it needs**: overlays, insets, readouts, step parameters.
- **The knowledge level** (below): the level the story requires, and for every topic it teaches, the chapter and beats
  that teach it and their seconds.
- **The listener card**, a few lines every writer and reviewer reads: who listens (the audience's `profile`, and that
  they hear each sentence once, at speaking speed, and cannot scroll back); the terms they do not know, which the
  story defines where it first says them (from the knowledge level); and, per chapter, the one question it answers.
  The narration is written for this listener, clear and in-depth (`references/storyboard.md`, "Written for the ear").

Done when every key claim has its source and its visual, every chapter its budget, and the knowledge level is
recorded and checked. Then the gate (SKILL.md, "Gates"): the chapters and budgets, before anything is built.

## The knowledge level

Decide what the story requires before writing a beat: the lowest knowledge level at which the story can still teach
everything above it, in the chapters and time it has. A lower level reaches more viewers; each topic below it is one
the story must teach well, and that takes time. The graph, its commands, and the audience's rules are the
`knowledge-graph` skill's; this is how a story costs a level.

1. **List what each chapter uses**, as topics of the knowledge graph, in the package's `knowledge.toml`; add missing
   topics to render_lab's graph.
2. **Start low.** Take an audience that knows only the bases and the topics every graphics programmer meets first
   (rasterization's pipeline, textures, a BRDF, a light): `render-lab --investigation <id> knowledge --known …` prints
   the level and what the story would have to teach, per chapter.
3. **Cost it.** Give each topic to teach a time: a taught topic is a sequence of beats (below), about 60 to 120
   seconds with its demonstration, more for a deep one. Add them to each chapter's budget for the feature's own
   explanation.
4. **Raise the level only where it does not fit.** When a chapter, or the whole story, runs over the length agreed
   with the user, move the topics most general and furthest from the feature (the ones most viewers have met
   elsewhere) into the known set, one at a time, and cost again; among equal choices, take the audience with fewer
   carries (`--levels` lists them). Never raise it to spare a topic the feature itself is about.
5. **Record it**: `[audience]` in `knowledge.toml` (its `profile` says who that viewer is), and in the outline the
   level, the topics taught with their chapters, beats, and seconds, and the topics recalled in a sentence (the
   level's). Check it with `render-lab --investigation <id> knowledge`.

## Teaching a topic

Every topic the story teaches is taught like a teacher at a board, not mentioned: the viewer should be able to explain
it afterwards without the engine or the paper. Give it its own beats, before its first use, in this order where it fits:

1. **The question** it answers, as a problem the viewer can see ("how far may this ray step without passing through
   anything?").
2. **An intuition** first: a plain-words picture or an everyday analogy, then the same idea in the scene.
3. **One worked example**, animated step by step on a single element (one ray, one texel, one probe), each step a
   value the viewer can see change: inputs shown, the calculation advancing, the result marked.
4. **Exploration**: the demonstration's values and clicks that let the viewer try it (move the point, change the
   roughness, pick another texel), named in the narration, with a sweep that shows the relation it teaches.
5. **The general rule**: the formula or the algorithm, its terms lit as they are said, after the example.
6. **Its limits**: where it fails or what it costs, shown, not just said.
7. **The link back**: how the feature uses it, in one sentence, before the story moves on.

Each taught topic gets a demonstration of its own in Phase 3, with its interactions; a topic taught only in narration,
or only with a still, is not taught (required). The order of the seven steps is the default; reorder or merge steps
where the topic reads better another way, but leave none out.

## The shape of a story

These hold for every story. The bridges, the recaps, and the sources beats are expected in every story; the rest are
the standard to meet, applied where the chapter has the thing they describe. The order of the chapters, the opening,
and the ending are the variant's: `new-story-engine`, "The shape of an engine story", and `new-story-paper`, "The
chapters of a paper's story".

- **Say what the story teaches, and what the viewer gains.** In the opening, a beat names the main topics the
  story teaches (the three to six that matter most, from the knowledge level, in plain words), what the viewer should
  already know (the starting level; the viewer's Prerequisites tab lists it), and what they will gain: what they will
  be able to explain, reason about, or do afterwards (read the engine's code or the paper for it, tune its settings,
  judge its trade-offs, apply the techniques elsewhere). Show it on screen as a short list beside the narration.
- **Explain each mechanism; showing its data is not explaining it.** A viewer who sees what a structure looks like may
  still not know what it is for. For every mechanism, the story answers, in this order where it fits: the **problem**
  it solves (and what would go wrong without it), an **intuitive example**, the **data** it keeps, the **algorithm**
  (its inputs, the calculation, its outputs), the **result** on screen, the **trade-off** it makes (quality, memory,
  time), and **where its output goes next**. The outline records the answers per chapter; a chapter that only shows a
  structure's layout is not finished.
- **One architecture map, reused.** One diagram of how the feature's or the method's parts relate, true to the source (its feedback
  loops, caches, parallel or once-only stages; never forced into a line it is not), with the same names and colours
  everywhere. Each chapter opens on it with its own part lit, and ends by showing what its output feeds. Its terms are
  the narration's terms.
- **One encoding per kind of data.** Geometry, cells, distances, occlusion, and light (radiance, irradiance) are
  different things, even where one is derived from another: give each its own colour and graphic treatment for the
  whole story (the palette's roles), and never draw one so it reads as another.
- **Follow the data.** Order the chapters the way the data is made and used: built (often offline), stored, updated
  each frame, then read by the passes that need it. Each chapter's first beat says what the structure is for, before
  how it is built.
- **One, then many.** Work one example fully (one sample, one record, one ray, one page), then show it generalised
  across the scene, then several more examples. The worked one is a record in the struct panel (the `record` key,
  `panel` `values`, `highlight` on the field the sentence names). Work it as: highlight the one element (a cell, a
  probe, a texel) in the 3D scene, show its inputs, animate the calculation step by step, show its result, then say
  where that result goes. Simplify the scene to a single object first when the full room would hide the step, then
  return to the room.
- **One continuous example across stages.** Where data passes through several stages, follow the same element through
  all of them (one probe: its rays gather light, the light is compressed, the compressed form is evaluated later), so
  the viewer never rebuilds the picture. Near the end, follow one shaded point on a visible surface back through every
  mechanism it depends on (which data it reads, how it finds it, how occlusion weights it, what light it adds), in the
  source's real order.
- **Compact representations, intuition first.** Before showing a representation's coefficients (spherical harmonics,
  an octahedral map, a quantised format), say in plain words what it does ("a few numbers that describe how light
  varies smoothly with direction"), then what it keeps, what it loses, why that is worth it, and where it is read
  back. Never assume a method because other systems use it: show what this engine's code, or this paper, does
  (required).
- **The code, step by step.** Animation comes first; pseudo-code is shown only alongside it. For a shading or lookup
  step, show its pseudo-code listing (`{panelCode}`) and light the line the sentence is on (`{highlightCode "<listing>" {' "<tag>"}}`),
  and with every highlight change the demonstration too, so the scene shows what that line does (its rays, its
  samples, its threshold, its result); a highlight that changes nothing on screen is cut, or the demonstration gains
  the step it lacks. Show only the lines the step needs, at a size that reads on a laptop, with the inputs and outputs
  of the lit line beside it (a readout row or label), and say whether it is the source's code or algorithm, the lab's
  pseudo-code of it, or the lab's own demonstration (required).
- **A subject that shows it.** Choose the scene's object whose shape shows the feature best, and fade the ones that
  would hide it in close-ups.
- **Bridge every chapter to the last.** A chapter's first sentence says what the previous one left, and what this one
  does with it; only then does the chapter start. Vary the wording, and give the reason the step comes next when there
  is one. From O3DE's story:
  - "With the lights sorted into lists, the next part turns to what they light: the surface."
  - "Before any light reaches that surface, decals can change it, and they are the next part."
  - "So far light has only bounced off surfaces; the next part lets it pass through them: transmission."
  - "That covers the simple point light; the next part leaves punctual lights behind."

  Beats within a chapter link the same way when the step changes ("once every brick is stored, the lookup …"), and the
  story ends with a short recap chapter that walks the frame again, naming each part in order.
- **Recap each chapter.** Before its sources beat, a chapter ends with two or three sentences: what it just explained,
  why it matters to the final image, and what the next chapter does with it.
- **Close each chapter with its sources**: a `sources` beat, "where to read it", listing the chapter's index keys in
  reading order, and a reading table in the index's `chapters`. The build turns them into the source-reading guide
  (`readingPath` in `video.json` orders its recommended path).

## Reusing tools

For each tool the outline needs, look for it first:

- **In the lab** (`lab/`, `packages/render_lab`): use it.
- **In a public package**: reuse it in place, or, when a second investigation needs it, move it into the lab as a
  generic extension point (the `Investigation` trait, `FrameContent`, the dock kit) that names no investigation.
- **In a local package of the optional module**: code derived from an engine whose license does not allow publishing
  never moves into the lab or a public package, however generic it looks. What can move is a generic mechanism written
  from scratch or from published methods (papers, talks), with no engine names, parameters, or text keys: for example,
  a picture inset the lab draws (`lab::utility::inset::PictureInset`, in `FrameContent::insets`: CPU pixels or a GPU
  texture the renderer keeps by key), while the engine-specific table that fills it stays in the package. If you are
  unsure whether a piece is derived, it stays where it is; ask the user.
- **Nowhere**: build it, in the investigation's package unless other investigations will need it.

Moving code keeps the layering: the lab never depends on an investigation, and nothing published names or mirrors the
optional module's contents (AGENTS.md).
