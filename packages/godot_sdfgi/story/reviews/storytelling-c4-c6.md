# Storytelling review, chapters c4 to c6 (SDFGI)

Language only. 45 sentences flagged. Map rows are `known | new | job`. " / " separates the sentences of a SPLIT.
Every rewrite keeps the narrated facts; "needs fact" marks a spot where a proper fix needs a reason the narration lacks.

## c4 Voxelization

### c4-passes
- **S2 (L608)** | the meshes, the grid (S1) | three cameras, one per axis, and "region" (a new noun) | act. Fault: synonym drift. The dirty part was "slab" (c2) and "box" (c3); here it is "region". Rewrite: "The GPU draws the box three times, with a camera looking along each axis in turn."
- **S3 (L613)** | the three cameras | one pixel per cell, over the two axes each sees, and "region" again | act. Fault: two new parts, drift. Rewrite: "Each camera's image has one pixel per cell of the box. / Its two sides are the two axes that camera sees." Keep "box" throughout.
- **S7 (L632)** | the three views covering the room (S6) | drawn from both sides, back-face culling, off for every material | act with its cause, but the cause comes second and carries its gloss. Fault: overloaded, no bridge. SPLIT: "The views also draw every surface from both sides. / That is because back-face culling, which skips faces turned away from the camera, is turned off for every material drawn here."
- **S8 (L635)** | materials, from nowhere | alpha-blended materials leave no voxels | act (a topic turn). Fault: no bridge. Rewrite: "One kind of material is left out. / Alpha-blended materials are not drawn at all, so they leave no voxels."
- **S9 (L637)** | "the same fragments" (S4) | the lab's own way, by ray tracing | act. Fault: a lab difference said as an appended phrase, with no bridge. SPLIT: "The lab does this part differently. / It finds the same fragments by tracing a ray from each pixel into the scene."

### c4-facing
- **S1 (L687)** | the voxel's colour bits (previous beat) | six bits, one per axis direction | act. Fault: the purpose is missing (what the bits are for), so six bits of "facing" arrive as a bare fact. Needs fact: the narration has nowhere said why the facing is kept (c8-hit uses it, but that comes later). Restructured, no reason: "Each voxel also keeps six more bits, one for each axis direction. / They record which way the voxel's surface faced." Move the headline's "which way the surface faced" into the sentence.
- **S2 (L694)** | "six more bits" | a 3D texture of their own | none. Fault: a fact that neither says why nor what it changes; "They" can mean the voxels. Rewrite: "The six bits live in a 3D texture separate from the albedo."
- **S3 (L696)** | the six bits | "nearest its normal", the normal unglossed | act. Fault: term not unpacked. Rewrite: "Each fragment sets the one bit whose direction is nearest its normal, the direction the surface points."
- **S4 (L698)** | fragments landing in a voxel | their bits are combined | act. Fault: "their" can mean the fragments or the bits, and "combined" says nothing; S5 needs "set together". Rewrite: "When several fragments land in one voxel, the bits they set all stay set."

### c4-emission
- **S1 (L710)** | the voxel's colour and facing | "emissive surface", unglossed | act. Fault: term without gloss. Rewrite: "A surface that gives off light, an emissive one, also writes that light into the voxel."
- **S2 (L717)** | the emitted light | thirty bits, "a shared-exponent format" | act. Fault: a name before its meaning (S3 explains it). SPLIT order: "That light's colour is packed into thirty bits. / Red, green, and blue share one exponent, a single number that scales all three, which lets so few bits cover a wide range of brightness. / This packing is called a shared-exponent format." (S3 of the original becomes the middle sentence.)
- **S4 (L725)** | the thirty-bit light | a separate texture, a split over six directions, five bits each, a difference by direction | act, with a purpose. Fault: three new parts, and "five bits each" can mean per direction or per channel. SPLIT: "A separate texture splits that light over the six axis directions. / It keeps five bits for each direction, so the light can differ by direction."
- **S5 (L731)** | "the panel's light", the panel never shown or named | an area light, so no voxel is emissive | check. Fault: the known part comes from nowhere, and the beat ends on a negative with no landing. Rewrite: "In this room, though, there is nothing to see. / The light panel arrives as an area light, so no voxel is emissive."

### c4-shell
- **S2 (L750)** | the slice through the sphere (S1) | only the shell is voxels | check. Fault: the result is stated without its cause; the cause (only surfaces are drawn, S4 to S5 of c4-passes) is not recalled. Rewrite: "Only the surface was drawn, so the voxels form only the sphere's shell, and nothing fills the inside."
- **S3 (L753)** | "That", which can mean the hollow or the slice | enough for the distance field, because a ray stops at the shell | purpose. Fault: pronoun with two referents. Rewrite: "A hollow shell is enough for the distance field of the next chapter, because a ray stops at the shell."

## c5 The distance field

### c5-why
- **S2 (L778)** | "the voxels say where surfaces are" | every cell gets the distance to its nearest solid voxel | act. Fault: drift (voxel / cell): the same grid element is "voxel" when solid and "cell" otherwise. Say once: "Every cell, solid or empty, ..." and keep "solid voxel" only for the solid ones.

### c5-seeds
- **S1 (L797)** | the ray reaching a surface (c5-why S5) | each cell keeps a seed | act. Fault: the chain breaks; a new topic (how the field is found) starts with no bridge. Rewrite: "So how does each cell find its nearest solid voxel? / Every cell keeps a seed."
- **S4 (L810)** | "the flood" | half size, sixty-four cells a side, an eighth of the work | purpose. Fault: three new parts; "half size" before what it means (S5). SPLIT, with S5 rewritten to carry the arithmetic: "The flood runs on a grid of half the size, sixty-four cells a side. / (S5) Each half-size cell covers a block of two by two by two full cells, so the flood does an eighth of the work."
- **S5 (L813)** | half-size cell | the block of eight cells | act. Fault: the sentence that would explain the saving is separated from it (see S4). Rewritten above.
- **S7 (L821)** | blocks and first seeds | a block with no solid voxel has no seed | branch. Fault: the beat ends mid-process, no landing. Add after it: "The flood then passes these seeds from cell to cell."

### c5-flood
- **S1 (L831)** | "In each pass", where "pass" was never defined, and the flood | twenty-six cells, a fixed step away, an axis or a diagonal | act. Fault: two new parts (the twenty-six, the step) and "step" is used before S3 defines it. SPLIT: "The flood runs as a series of passes. / In each pass, a cell looks at the twenty-six cells around it. / Each one is the same fixed step away, along an axis or a diagonal." (Twenty-six is three cubed minus one.)
- **S2 (L836)** | the cell and its twenty-six | keeps the nearest seed | act. Fault: "their" and "itself" are loose. Rewrite: "The cell keeps whichever of those cells' seeds lies nearest to the cell."
- **S5 (L844)** | big steps (S3-S4) | a wrong seed, and a correction by smaller steps | branch. Fault: "X, and Y", the relation implicit. SPLIT: "But a big step can hand a cell a seed that is not its nearest. / The smaller steps after it correct that."
- **S6 (L848)** | the passes | "the plain version of the shader" | act. Fault: a label with nothing before it; no other version has been heard, so "plain" has no contrast. SPLIT: "The passes do not all run the same shader. / The first two, whose steps are large, use the plain version."
- **S7 (L851)** | the last four passes | a faster version, workgroup, shared memory | act with a cause. Fault: two terms with no gloss. Rewrite: "The last four use a version that is faster for small steps, because it keeps the nearby cells of each workgroup, a team of threads the GPU runs together, in shared memory, a fast memory the team shares. / So six passes in all spread each seed, from steps of thirty-two down to one." (The last sentence is a landing by arithmetic.)

### c5-upscale
- **S1 (L857)** | seeds at half size | back to full size, one hundred twenty-eight cells | act. Fault: passive, no actor, "cells" missing "a side". Rewrite: "Then the field goes back to full size, one hundred twenty-eight cells a side."
- **S3 (L862)** | the full cell and its seed | the eight voxels of the seed's block, and a nearer one | act. Fault: two new parts, "that seed" and "a nearer one" (nearer than what?). SPLIT: "Then the cell looks at the block of eight full cells that holds its seed. / If one of them is solid and nearer to the cell than the seed, the cell keeps that one instead."

### c5-store
- **S1 (L881)** | seeds and the cleanup pass | STORE, the pass that writes the field, one byte | name before act; the chain from "seed" to "distance" is missing. Fault: label first, and nothing says that a cell's distance is its distance to its seed. Rewrite: "Each cell's distance is how far it lies from its final seed. / A last pass, called STORE, writes that distance into the field, one byte per cell."
- **S3 (L888)** | the stored byte | "Despite the name SDFGI": the name's "signed" is never said | none clear. Fault: a joke the listener cannot decode, and it interrupts the rule before its reason (S4 to S7). Move it after S7 and gloss: "SDF in SDFGI stands for signed distance field. / The stored byte has no sign, but subtracting the one gives a signed value."
- **S4 (L891)** | the stored byte | "march", the probe and shadow rays | act. Fault: "march" is a process never told here (c5-why said the ray "steps"); drift. Rewrite: "The rays that step through the field, the probes' and the shadows', read the byte, then subtract the one that was added."
- **S6 (L899)** | solid reads minus one, empty reads plus one | "a filtered read", unglossed | act. Fault: term. Rewrite: "A filtered read, which blends two neighbouring cells by position, therefore crosses zero halfway between them."

### c5-cells
- **S1 (L918)** | the field's byte per cell | a list of sixteen-byte records | act. Fault: a topic turn with no bridge; the headline says "cell", the sentence "voxel". Rewrite: "The stage also keeps a second structure. / Every solid voxel goes into a list of sixteen-byte records."
- **S2 (L927)** | the record | position, albedo, facing, "its stored light" | act. Fault: "stored light" is new and unglossed (the lighting is c7). Rewrite: "A record holds where the voxel is, its albedo and facing, and room for its light, which a later chapter fills in."
- **S4 (L935)** | the record | "spare bits", twenty-six flags | act. Fault: "spare" has no contrast, since nobody said which bits are used. Rewrite: "The bits left over in the record hold twenty-six flags, one for each neighbour of the voxel."
- **S6 (L947)** | the flags | the light pass copies the voxel's light to the neighbours, "later" | purpose. Needs fact: why the neighbours need that light is not narrated. Restructured with no reason: "The light pass can later read these flags, and copy the voxel's light out to those neighbours."

### c5-recap
- **S1 (L951)** | the field | "nearest surface" | compress. Fault: drift; the chapter said "nearest solid voxel". Rewrite: "... how far it is to the nearest solid voxel."

## c6 Occlusion

### c6-shells
- **S1 (L972)** | the distance field, the stage | "a region", measuring | act. Fault: drift ("region", where c4 used "region", c3 "box", c6-store "cascade"). Rewrite: "While SDFGI builds the distance field for the box, it also measures how well each probe sees the cells around it."
- **S3 (L981)** | the probe | a cube of sixteen cells, reaching the neighbouring probes | act. Fault: "which" can mean the cube or the probe; two new parts. SPLIT: "Around each probe, SDFGI works on a cube of sixteen cells a side. / Probes are eight cells apart, so that cube reaches out to the neighbouring probes."
- **S5 (L994)** | visibility per cell (S4) | "layer by layer", where the headline says "shell" | act. Fault: drift (layer / shell) and no start given. Needs fact only if the start must be explained (the probe's own cell). Restructured: "SDFGI fills in the visibility layer by layer, starting at the probe, each layer one step farther from it."
- **S6 (L998)** | the layers | the averaging rule | act. Fault: a rule said after a run of steps, and "neighbours" unspecified. Rewrite: "Each cell takes the average visibility of its neighbours in the layer one step closer to the probe."
- **S7 (L1002)** | the averaging rule | solid cells zero, cells behind them "grow darker" | consequence. Fault: "darker" turns a number into a picture, so "how much" is never said. Rewrite: "A solid cell has zero visibility, so each cell behind it averages in that zero, and its visibility falls below one."

### c6-store
- **S1 (L1014)** | the visibility cube (c6-shells) | the "probe cube", a second cube | name. Fault: a bridge is missing and "cube" now names two things (the sixteen-cell cube, the cube of eight probes). Rewrite: "Now to where the visibility is kept. / Every cell lies inside one probe cube, the cube whose eight corners are probes." Keep "the sixteen-cell cube" for the first.
- **S3 (L1028)** | the visibilities | read only when a setting is on, and it is off | act. Fault: passive with no actor, and "X, and Y". SPLIT: "The pass over the screen reads these visibilities only when the environment's use occlusion setting is on. / By default that setting is off."
- **S4 (L1031)** | "still computed" | "every time a cascade is rebuilt" | check. Fault: drift again ("cascade" here, "region" in S1 of c6-shells, "region"/"slab" elsewhere); pick one for what is rebuilt. Rewrite: "The code shows that SDFGI still computes the visibility every time it rebuilds a cascade."

### c6-recap-gi
- **S1 (L1043)** | the recap sentence's "how well that probe sees it" | "use it" | act. Fault: "it" can mean the cell or the visibility. Rewrite: "The pass over the screen can use that visibility to give less weight to a probe behind a wall."

## (a) Drifting terms

1. The dirty part of a grid: "slab" (c2), "box" (c3), "region" (c4-passes, c6-shells), "grid"/"cascade" (c4, c6-store). Pick "box" for the world-space part being voxelized.
2. Voxel / cell: "solid voxel", "solid cell", "each cell", "every cell is lit", "light in the voxels". One grid element; use "solid voxel" only for the solid ones.
3. "Nearest solid voxel" (c5-why, c5-seeds) / "nearest surface" (c5-recap, c5-why headline).
4. "Cube": the sixteen-cell cube around a probe and the probe cube (c6).
5. Layer / shell (c6-shells headline versus narration).
6. Step / jump / march (c5-flood and c5-store); "pass" for a view (c4), a flood round (c5), STORE, the pass over the screen, the light pass.
7. See / visible / visibility / lend its light (c6).
8. Facing / "which way it faced" / normal (c4-facing).
9. Seed / first seed / nearest voxel (c5).
10. Pixel / fragment (c4-passes S3 to S4).

## (b) The ten most important fixes, ranked

1. c5-flood S1: define "pass" and "step" before the twenty-six cells (L831), the key sentence of the chapter.
2. c5-seeds S4 to S5 (L810, L813): put "half size" before its numbers and the eighth after the block arithmetic.
3. c5-store S1/S3 (L881, L888): link seed to distance, and gloss the "S" in SDFGI before "no sign".
4. c4-passes S7 and S9 (L632, L637): split the culling sentence, bridge the lab difference.
5. c4-emission S2 and S4 (L717, L725): the name after the act, and settle "five bits each".
6. c6-shells S3 and S5 to S7 (L981, L994 to L1002): two referents of "which", the start of the fill, and "darker".
7. c6-store S1 (L1014): the two "cubes", name both.
8. c5-flood S6 to S7 (L848, L851): the "plain version", and gloss workgroup and shared memory.
9. c4-facing S1 to S2 (L687, L694): the purpose of the six bits (needs fact), and the dangling "They".
10. Drift across c4 to c6: "region / slab / box / cascade" for one thing, and "voxel / cell".
