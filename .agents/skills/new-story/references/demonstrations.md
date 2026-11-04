# Phase 3: Demonstrations, and how a viewer explores them

Load this before building a story's demonstrations, or when adding one to a chapter. It is a phase of its own, before
anything is built on the outline: find what each structure the story explains can show, and give the viewer a way to
explore it. Write it to `story/demonstrations.md`. How a demonstration is coded (values, views, clicks, images) is
`new-investigation`'s `references/code.md` and `references/presentation.md`.

1. **How the engine stores it.** For each data structure in the outline (a texture, an atlas, a buffer, a table),
   research the checkout (Phase 1's brief, `references/research.md`) for: its resources, each one's dimension, format,
   and size; which are separate resources and which are packed together (channels, layers, tiles of one atlas); how an
   element is addressed (page tables, indirection, tile coordinates); and the engine's own debug views of it (its
   visualize modes, show flags, debug draws). A debug view says what its authors found worth seeing.
2. **Mirror the storage** (required). Show the structure as the engine keeps it. Separate resources are separate
   views, side by side, each titled with its name and format, sharing its layout (the same element outlined in each);
   channels packed in one resource are one view with its channels decoded. Do not merge what the engine separates, or
   split what it packs, for convenience.
3. **Find the picture first.** For every concept the story demonstrates (an algorithm's step, not only a data
   structure), first think of a good way to see it. Search for how it is usually visualized (papers' figures, talks,
   course notes, the engine's documentation; online when your environment can) and take the clearest as a reference. A
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
   so move from one to the other on screen instead of cutting: show the structure as the engine has it (the depth map,
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
9. **Gate**: the views, the subjects, and the interactions (SKILL.md, "Gates").

Done when every structure in the outline has its storage recorded and mirrored, every view its interaction, every
taught topic its demonstration, and the pick round-trip passes in the selftest.
