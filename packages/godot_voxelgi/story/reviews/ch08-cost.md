# Chapter 8, "Keeping it fast": review and fact check (2026-11-01)

One reviewer (from 96 stills, the storyboard, and the outline) and one independent fact checker (against the
checkout, the source index, and the lab's code). What was found, and what was done.

## Fact check

No sentence was wrong; these were imprecise and are fixed:

| Sentence | Finding | Done |
|---|---|---|
| "the culler compares every light" | only the visible lights paired with the probe, plus directional ones (renderer_scene_cull.cpp:3925-4004) | "every visible light near the box" |
| "The bounce is the heaviest" | the code does not rank the passes; only a timing can | "On this machine the bounce takes the longest", from the panel (0.08-0.10 ms against at most 0.04) |
| "from two ... to three hundred eighty-one" | the counts are upper bounds (alpha 1 and the box end a trace early) | "at most" |
| "a plain bilinear blend and no depth weights" | true without MSAA; with it, a normal-aware pick of a neighbour comes first (sfc.glsl:1986-2013) | "without multisampling" |
| "The mips climb ..., leaves first" | the light pass lights the leaves; the mip passes start at their parents | "The light pass lights the leaves; then the mips climb" |
| "which Godot never does" | Godot relights whenever something changed | "which Godot does only when something changed" |
| research.md §9 | stale gi.cpp:4294-4298 (now :4298-4303); "dynamic objects exist" (or were plotted last frame) | fixed |

## Review

| Issue | Severity | Done |
|---|---|---|
| The panel's line graph and the scene pass unexplained; the opaque mix highlighted though the chapter never uses it | Medium | Sentences on the line (a spike is a frame that did more), the scene pass (the room's own shading), and the row this chapter shrinks |
| The readout never introduced | Medium | A first sentence in c8-sparse says what it counts |
| c8-sparse overloaded; the level sweep ran root-first under "leaves first" | High | Split into c8-sparse and c8-levels; the sweep runs 7 to 1 while the mips climb; the 32 dispatches said as one light or bounce pass, the mips, and eight writes |
| The cell array covered the scene's top-left | Medium | Moved to the bottom-left; the sentence says each colour is one level's run |
| c8-march one sentence with three ideas | High | Three sentences: the field as a safe jump, long and short steps with the one-cell floor, the early-outs |
| "one read stands in for all the rays" taught exact integration | High | "the average of the cells it covers, a cheap stand-in for many rays, at the price of blurring thin walls"; the 28 samples said |
| High's cause unexplained | Medium | "narrower cones widen more slowly, so their steps are shorter and more numerous" |
| The reflection cone's sweep ran opposite to its numbers, no reason for half-cell steps | High | Its own beat (c8-spec), roughness 1 to 0 under "two ... to three hundred eighty-one", and why a mirror steps half a cell |
| The split view introduced late | High | The first half-resolution sentence says which side is which |
| Half resolution's row barely shrank in the stills | High | The stills render only their own frames; the panel's rows now average the last 30 frames (lab/src/timing.rs `summary_last`). Measured: 0.46 to 0.12 ms whole, 0.23 to 0.06 split |
| The edge sentence mixed stair-stepping and blending | High | One mechanism: texels traced on the wall and the sphere blended across the edge, ragged two pixels at a time (visible at 1080p) |
| The recap's sweeps did not match its words | Medium | The half-resolution toggle under "half resolution frays the edges" |
| Chapter 9 repeated the bridge | Low | Shortened |

Not done (would need new tools): ghost rows for absent passes, a pixel-grid overlay of the 2 x 2 blocks, a
32-tick dispatch strip, a side-by-side fixed-step march with a sample count, a stable pass order in the panel.
