---
name: new-story-engine
description: The engine variant of a Render Lab story, loaded with `new-story`, for a story that explains a game engine's feature read from the engine's source at a recorded commit. Holds the research into the checkout (an overview, one brief per subtopic, where the feature is used, how it is optimised), the shape of an engine story (opening on the feature off and on, where it runs in the frame, following the data, showing the bytes, the recap and its three takeaways), demonstrations that mirror the engine's storage, and the fact check against the checkout. Use whenever `new-story` is used for an engine investigation (packages/<engine>_<feature>/); for a paper's method use `new-story-paper`.
---

# An engine's story

This skill is loaded with `new-story`, which holds the process every story shares; this one holds what is particular
to a story about an engine's feature. Its precision standard is `new-investigation`'s "Precision" (required): every
claim read from the checkout at the recorded commit and cited by file, symbol, and line, following what runs, not what
exists. Where the engine's license does not allow publishing code derived from it, the package and everything about
its story are local (AGENTS.md; `new-investigation`, step 0).

## Phase 1: Research

`references/research.md`: an overview of the feature in the checkout, one brief per subtopic, the two cross-cutting
briefs (where it is used, how it is optimised), a questions round, and the merge into `story/research.md` with where
the lab differs. Done when every subtopic has its claims cited, its open questions answered or listed, and its lab
differences noted; then the gate.

## The shape of an engine story

With `new-story`'s "The shape of a story" (`references/outline.md` there), which holds for every story:

- **Open with what the feature adds, and the problem it solves.** The first beat shows the feature's contribution as a
  comparison: the same camera, exposure, and scene with the feature off and on, cued on what it adds (the bounced
  light, the colour bleeding), not two renders that merely differ. Then state the problem in one sentence ("how can
  the engine light a room with bounced light without tracing many rays for every pixel?"), show the architecture map,
  and say what the viewer will be able to explain by the end, with what the story teaches and what the viewer gains.
  Keep new terms out of the opening; each arrives in its own chapter.
- **Then where it runs.** The first chapter places it in the engine's frame (the frame graph's 2D scene, a beat's
  `graph=frame`, the feature's passes lit) and says what decides whether it runs at all: project settings, quality
  levels, platform paths, and their defaults.
- **Follow the data**, as `new-story` says, from the research's subtopics in frame order; near the end, the chapter on
  how it is kept fast (the research's "How it is optimised").
- **Show the bytes.** Where the story names a format or a layout, show the record's fields and sizes, and how a stored
  value decodes to what the shader uses; map each record to the engine's structure in the index's `mappings`.
- **Mechanisms the lab doesn't run** (an offline build, a streaming policy) are still shown from a CPU computation that
  follows the engine's code, in a module of their own; say on screen when a view is schematic or scaled (required).
- **End with the whole, and three takeaways.** A short recap chapter walks the frame again, naming each part in order:
  it returns to the full scene, shows each mechanism's contribution (switched off and on, in frame order), and closes
  on three takeaways: how the feature organises its data, how it computes and reuses light, and what accuracy it
  trades for speed.

## Demonstrations of an engine's data

For `new-story`'s Phase 3 (`references/demonstrations.md` there), steps 1 and 2: for each data structure in the
outline, research the checkout (a brief like Phase 1's) for its resources, each one's dimension, format, and size;
which are separate resources and which are packed together (channels, layers, tiles of one atlas); how an element is
addressed (page tables, indirection, tile coordinates); and the engine's own debug views of it (its visualize modes,
show flags, debug draws). A debug view says what its authors found worth seeing. Mirror that storage (required): do
not merge what the engine separates, or split what it packs.

## The fact check

`new-story`'s fact check (`references/review.md` there) runs `new-investigation`'s procedure step 8 ("Audit"): every
factual sentence and every `inspect` against the checkout, with file:line evidence, classed as verified, correct but
simplified, potentially misleading, incorrect, or needs investigation. It also checks that the version and commit the
story names are the checkout's (`git -C <checkout> rev-parse HEAD` against the index's `engine.commit`), and that no
claim carries over from another version or from how similar systems usually work.

## Check

The checks are `new-story`'s for the depth of the change (its "Choose the depth" and Phase 8), with the engine's:

```sh
cargo run --release -- --investigation <engine>/<feature> selftest --report <scratch>/selftest.json --timeline <timeline.json>
python3 tools/video/build.py --investigation <engine>/<feature> --prepare-only
python3 tools/video/build.py --investigation <engine>/<feature> --snapshots-only   # the stills of every beat; look at them
```

The full build's verification (`tools/video/verify.py`) compares the checkout's commit with the index's.
