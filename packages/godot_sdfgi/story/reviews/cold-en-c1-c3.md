# Cold listen, English, chapters 1-3

## Chapter 1: Where SDFGI runs

Flags (first five words; problems; what I lost):
- "Their sizes match the engine's" (2): "grids around the camera", "rays each probe traces" are used before any of them is defined.
- "Each citation also records a" (4): why a hash of the line matters to me, the viewer, is not said.
- "Each probe traces rays through" (5): rays, the distance field, and light stored in voxels arrive in one sentence, right after "probes record light from every direction".
- "Each cascade is a grid" (6): "cells" and "a hundred twenty-eight on a side" come before "A cell is the space one voxel fills" names them.
- "The second sets how many" (2): "frames each probe averages over" averages what? Never said.
- "The code starts the probe update" (3, 4): "in one place only, inside the code that draws..." resolves late, and I cannot tell why where the code starts it matters yet.
- "A comment there says that" (3): "that probe update and the pre-pass then run at the same time" is a nested clause, hard to hold.
- "That happens only when" (1): "That" could be the update, the comment, or running at the same time.
- "The code looks the second" (3): "a list of its own, which begins with four" attaches only after the label sentence; I had to re-parse.

What I understood: SDFGI is Godot's bounced light, off by default, a setting on the environment; per frame it moves cascades, voxelizes new space, builds a distance field, lights voxels, has probes trace rays, then a screen pass feeds the opaque pass as ambient light. Forward+ only, defaults 4 cascades of 128 cells. Probe updates ride on the depth pre-pass; without it they freeze. The default ray count is 8, not the labelled 16.
What I would still ask: what do the frame-averaging and voxel-update settings do; why the probes are tied to the depth pre-pass.

## Chapter 2: Cascades that follow the camera

- "Every cascade has one hundred" (6): repeats what I just heard, and I could not see how it follows from the doubling sentence.
- "The code shows that a" (5): two-thirds vs three-quarters, and what 75 percent was meant to suggest, are both new at once; I lost which number is which.
- "Eight cells is one probe" (4): fine as a fact, but I could not tell why probes landing where they stood matters.
- "But if the new space" (1, 3): "what is kept" is vague, and "But" attaches to the dirty-slab sentence before.

What I understood: four nested 128-cell grids, 20 cm cells in the first, each next twice as coarse; height squashed by the y scale; probes every 8 cells (17 per axis); a cascade jumps 8 cells when the camera is more than 4 cells off, only the new slab is voxelized, unless it is over half.
What I would still ask: why the jump is 8 cells in particular and what is gained by probes keeping their places.

## Chapter 3: What is voxelized, and which lights

- "The CPU works first" (6): the CPU has not been mentioned in the story so far; I did not know what it works first on.
- "In this view, that box" (3): three sizes listed with no verbs, after a division I was still following.
- "SDFGI computes a dynamic" (2): "light update" is never defined.
- "But each light also" (5): cutoff, maximum cascade, default third, and a consequence for the fourth arrive in one run.
- "A mesh set to Dynamic" (5): receives light but adds nothing to the field: two ideas in one breath.
- "The slot a list" (3): the closing abstract statement of "agree only when the cascades that moved are the first ones, in order" resolved only at the end.

What I understood: each dirty slab (region) becomes a world box; only Static-GI-mode meshes and Static-bake lights go into the voxels, with a per-light cutoff cascade; a slot-indexing mismatch makes a moved second cascade lose its static lights.
What I would still ask: what a "light update" is, and why a dynamic mesh may not shape the distance field.
