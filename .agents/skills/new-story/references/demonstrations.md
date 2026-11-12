# Phase 3: Demonstrations, and how a viewer explores them

Load this before building a story's demonstrations, or when adding one to a chapter. It is a phase of its own, before
anything is built on the outline: find what each structure the story explains can show, and give the viewer a way to
explore it. Write it to `story/demonstrations.md`. How a demonstration is coded (values, views, clicks, images) is
`new-investigation`'s `references/code.md` and `references/presentation.md`.

1. **How the source keeps it.** For each data structure in the outline (a texture, an atlas, a buffer, a table, a
   record), find how its source keeps it: for an engine, its resources, formats, sizes, packing, addressing, and debug
   views, read from the checkout (`new-story-engine`, "Demonstrations of an engine's data"); for a paper, the
   structure as the paper defines it (its fields, its update rule, its figures of it) and, with the authors' code, as
   the code stores it (`new-story-paper`, "Demonstrations of a paper's method").
2. **Mirror it** (required). Show the structure as its source keeps it. Separate resources (or a paper's separate
   structures) are separate views, side by side, each titled with its name and format, sharing its layout (the same
   element outlined in each); channels packed in one resource are one view with its channels decoded. Do not merge
   what the source separates, or split what it packs, for convenience.
3. **Find the picture first.** For every concept the story demonstrates (an algorithm's step, not only a data
   structure), first think of a good way to see it. Search for how it is usually visualized (papers' figures, talks,
   course notes, the engine's documentation, the paper's own figures; online when your environment can) and take the clearest as a reference. A
   reference is an inspiration, not the truth: design from it, and where you see a clearer way to show the concept
   (another angle, an animation instead of a still, a 3D form of a 2D figure, fewer elements), take it. Then build that
   picture with the lab's 3D and 2D scenes: geometry, draws, insets, diagrams, and their animation. If the lab cannot
   draw it yet, implement what is missing. When other stories and future chapters could use it too (a way to draw an
   image, a field, a tree, a trace), build it generically in the `render_lab` package or in the lab itself, named for
   what it draws and not for the investigation; otherwise in the investigation's package.
   For example, screen-space tracing through a Hi-Z pyramid reads best as a **height field**: the depth map laid out as
   columns, one per texel, as tall as the surface is near. Each pyramid level keeps the tallest column of its 2 x 2
   below, so a level is the same field in wider, taller columns. The march is then visible: the ray tries a big step on
   a coarse level; if the column there does not block it, it goes on to the next cell; if it does, it goes down to a
   finer level, and repeats until a single texel's column blocks it. Lay the depth map out as that field, animate the
   levels as real columns rising and merging, and run the trace over it, the blocked column lit at each step.
4. **Plan the transitions between scenes.** A picture that explains a structure is not how the viewer first meets it,
   so move from one to the other on screen instead of cutting: show the structure as the source has it (the depth map,
   flat on the screen), then animate it into the explaining picture (each texel rising into its column), and back when
   the story returns to the frame. Morphs, camera moves, and fades are values the storyboard animates like any other,
   so the viewer sees that the two pictures are the same data.
5. **Choose the subject.** Show each structure on the object whose data reads best: an irregular one (a teapot, a
   sphere) rather than a wall or a box, whose cards, bricks, or probes are too simple to tell apart, and the same
   object across chapters, so the viewer does not have to learn a new one. Keep the story's worked examples where its
   thread needs them.
6. **Interaction for each view.** For each one, write what a viewer can do with it in the viewer (a click on the scene
   or on an inset, a slider, a toggle), and what that shows. A click picks an element and shows it everywhere it lives
   at once: in the scene, in every view of the same layout, and in a readout of its value in each resource.
   Interactions use the lab's extension points (`Investigation::click`, `#[click]` handlers on an inset, a script's
   click handler, the dock's values), and the values they set are values a story can set too, so a story can show the
   same pick.
7. **A demonstration for every taught topic** (`references/outline.md`, "Teaching a topic"): its worked example as step
   values, the interactions that let the viewer try it, and the sweep that shows its relation. Prefer the story's
   subject object, and the same scene the feature runs in, so the topic and the feature meet on screen.
8. **Build and check them** before the base: the views, the clicks, their values, and a selftest check that a pick
   round-trips (a click on what an element stands for picks it). Render stills of each view offscreen, and look at
   them.
9. **The interaction review** (required; below): a dedicated agent looks for the interactions the views still lack,
   and the main agent decides on and builds what it finds.
10. **Gate**: the views, the subjects, and the interactions (SKILL.md, "Gates").

Done when every structure in the outline has its storage recorded and mirrored, every view its interaction, every
taught topic its demonstration, the interaction review answered row by row, and the pick round-trip passes in the
selftest.

## The interaction review (required)

The agent that builds a view judges its interactions by what it meant the view to show; a fresh one, given only what
the viewer will get, finds what a viewer would want to try and cannot. Its job is to find interactions that build
understanding by letting the viewer change something and see the consequence at once, and that are easy to reach:
one click, one drag, one slider in the dock.

**When.** In Phase 3, once the views are built and their stills rendered, before the gate. Again in Phase 7, one
review per chapter of the storyboard as built (`references/review.md`), and in "Revising a finished story" when a
view is added or changed.

**The agent.** With subagents, a fresh agent on a cheaper model (AGENTS.md), never the one that built the views;
without them, a separate pass of your own that reads only the materials below and answers every question before
looking at the code that draws the views. Brief it with:

- what each view shows and of which resource (`story/demonstrations.md`), and the outline's claim each one supports;
- the investigation's values (`render-lab --investigation <id> values`), its click handlers and actions (`render-lab
  --investigation <id> actions`), and its images (`render-lab --investigation <id> images`): what a viewer can
  already change;
- offscreen stills of every view (`home --snapshot … --page viewer:<demo>`, and the free camera's,
  `viewer-free:<demo>`), rendered by the main agent one at a time;
- the lab's interaction kinds it may propose: a click on the scene (`Investigation::click`), a click or drag on an
  inset (`#[click]` handlers), an outline in an inset linked to a point of the scene (`Link`), a selection drawn in
  the scene (an outline, a frustum, a highlighted object), a dock value (a slider, a toggle, a choice), a value the
  demonstration's animation sweeps (Play), a comparison (`classic`-style reference demonstrations, the Experiments
  tab's A/B wipe), the free camera.

**Its questions**, for every view:

- **Picking.** Does every element a view shows as a grid, a list, or a picture (a tile, a cell, a texel, a sample, a
  light, a probe) pick that element when clicked? Is the picked element then shown everywhere it lives at once: in
  the scene (projected onto the viewport, or drawn as the volume it covers), in every other view of the same layout,
  and in the readout?
- **Linking.** When an inset points out an element, does a line or an outline tie it to the place in the scene it
  stands for, and does a click in the scene select the same element in the inset?
- **Tweaking.** For every quantity the narration names (a count, a radius, a threshold, a weight, a frame count), is
  there a value in the dock that changes it, with a range wide enough to see its effect, and does the view respond at
  once and visibly? Which parameter would a curious viewer reach for that the dock lacks?
- **Cause and effect.** For every "because", "so", or "only when" in the outline, can the viewer switch the cause off
  or move it and see the effect change (a toggle of a step, the source's path against the lab's reference, a sweep)?
- **Live data.** Does every view that shows state accumulating over frames keep updating while the camera holds still,
  and does it show its own frame's data (`lab::fetch` tags, `render-graph`)?
- **Reach.** Is each interaction found without reading code: a hint in the dock group, a cursor change, a label in
  the inset? Is any key interaction buried behind several steps?
- **Restraint.** Which proposed interaction would add controls without adding understanding? Leave it out.

**It returns** a table, `| View | Interaction | What the viewer does | What it shows (the claim it makes tangible) |
Value or handler it needs | Cost |`, ranked by how much each would help a viewer understand the algorithm, and
specific enough to build ("a click on the light-grid inset picks the cell's centre pixel as P; P's cell drawn as its
frustum in the scene, linked to the inset's outline", never "make it more interactive").

**Acting on it.** Save it as `story/reviews/interactions.md` (or `interactions-c<n>.md` per chapter). Build what
helps, as any demonstration is built (`new-investigation`'s `references/code.md` and `references/presentation.md`):
its values, its handlers, a selftest round trip for each new pick, stills looked at. Answer every row you do not
build with a reason. Record the interactions in `story/demonstrations.md`, so the storyboard can show the same picks
with the values they set.
