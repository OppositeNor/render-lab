# Cold listen, English, chapters 4 to 6

Listener: the card's graphics programmer; chapters 1 to 3 heard (as "already heard") before chapter 4, and 1 to 4, 1 to 5 before chapters 5 and 6.

## Chapter 4, Voxelization

Flags (first five words, problem numbers, what I lost):

- "Each camera's image has" (3): "one pixel per cell ... across the two axes that camera sees" resolved only at the end; I had to rebuild which axes.
- "Alpha-blended materials, though, are" (4): they leave no voxels, but I could not tell why or what that changes.
- "The readout panel decodes the" (1): which panel and which voxel; it is on screen, not introduced.
- "These six bits live in" (4): why a separate 3D texture matters, I did not know.
- "A separate texture splits that" (1, 5): "that light" is the thirty-bit colour or the emission as a whole; a second texture and the five-bits-per-direction arrive together.
- "In this room, though," (4): no voxel is emissive; what that means for the pass was unclear.

What I understood: the GPU draws the chosen meshes three times from three axes, one pixel per cell, both faces, no blended materials. Each fragment lands in a voxel by its 3D position. A voxel stores a solid bit plus 5-5-5 colour in sixteen bits, six facing bits (kept when several fragments land), and an emission term in a shared-exponent format. Only the surface is voxelized, so thick things are hollow shells.

I would still ask: why facing bits and per-direction emission are kept at all; why the colour has only five bits.

## Chapter 5, The distance field

- "If one of those" (1): "those" (the eight full cells), and "takes it" (which solid one) are unclear.
- "That version keeps the nearby" (2, 5): "workgroup" is only glossed in passing; shared memory plus the shader split come together.
- "Then the cell checks" (3): "the block that holds its seed" resolved only at the end; I had to re-parse.
- "Searching at half size still" (4): why a half-size search leaves a seed one cell off; I took it on trust.
- "Those cells are shown here" (1): which cells, shown where.
- "STORE, the pass that" (2): new name appears with no setup.
- "A solid voxel stores zero," (4): why add one; the reason arrives two sentences later.
- "So a solid voxel reads" (3): minus one and plus one needed arithmetic on the fly.
- "A filtered read, which blends" (2, 5): "filtered read" and the zero crossing together; the idea of a sign from an unsigned byte is two ideas in one sentence.
- "A record holds where the" (2): "its stored light" was not defined; light has not been computed yet for me.
- "The bits left over in" (4): why the leftover bits hold neighbour flags was not stated.
- "A neighbour's flag is set" (3): a condition with two parts, "that neighbour is empty and this voxel is the nearest surface to it", hard to hold.
- "The readout shows a cell" (1): which readout.

What I understood: each cell needs its distance to the nearest solid voxel so rays can step. A jump flood on a half-size grid passes seeds with steps 32 down to 1, then goes to full size with a one-step fix. The byte is zero for solid, otherwise one plus the distance; rays subtract the one, so the filtered value crosses zero at the surface. A second list holds sixteen-byte records of solid voxels, with twenty-six neighbour flags for copying light.

I would still ask: how a seed one cell off arises; what the flags let the light pass do exactly; why a seed comparison by distance is enough.

## Chapter 6, Occlusion

- "Each cell takes the average" (3): average over neighbours "one layer closer" needed a second pass to picture.
- "Every cell lies inside a" (1, 3): a second "cube" (corners are probes, spacing eight) versus the earlier cube of sixteen cells around one probe; I lost which cube was meant.

What I understood: for each probe, the cells in a sixteen-cell cube get a visibility from 1 down to 0, spreading outward layer by layer with solid cells giving 0. Each cell stores eight four-bit visibilities, one for each surrounding probe. The screen pass reads them only with use occlusion on, which is off by default, so the work is done and unused.

I would still ask: how a layer's neighbours are chosen; whether "behind a wall" really becomes below one for every cell behind it.
