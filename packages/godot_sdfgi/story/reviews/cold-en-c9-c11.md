# Cold listener review, English, chapters 9-11

Role: the card's listener (PBR forward renderer, shadow maps, compute); chapters before N counted as heard.

## Chapter 9, From coefficients to maps

Nothing flagged for the opening sentences (maps, texture, sixteen coefficients, two six-by-six maps, octahedral map, irradiance, radiance). Irradiance versus radiance is the one place I only half held on one hearing, but it was not clear enough to flag.

Flags:
- "Each copy is mirrored across" (1, 3): "the fold" has no referent yet (the seam of the octahedral square?). Also "copies of the edge texels" then "mirrored" made me re-parse what is copied and what is flipped.
- "The code instead multiplies by" (4): I did not know why the code would multiply by four per ray, or where the pi went, so the "instead" had no anchor.
- "Four times four is" (3, 5): two numbers (4x4 = 16, 4 pi) set against each other, then the next sentence adds "sixteen over four pi, which is four over pi" in one breath. I lost the arithmetic and had to take 27 percent on trust.
- "It returns a blend of" (3): lost why two texels (not four) are blended; the sentence follows "half a texel off" without saying along which axis or how.
- "The probes' histories, their thirty" (6): jump from the maps' brightness and direction problems to probe history with no bridge; "also follow a cascade's jump" does not say what is being followed.

Counts: 1:1, 2:0, 3:3, 4:1, 5:1, 6:1.

What I understood: each probe's sixteen coefficients are converted into two 6x6 octahedral maps, one for diffuse (irradiance) and one for reflections (radiance), with a one-texel mirrored border so filtering works at edges. The conversion is about 27 percent too bright because of a factor-of-4 slip, and directions are half a texel off. When cascades jump, probe histories move with their probes, and new probes start from the coarser cascade.

Still ask: what is the fold the border mirrors across? Why a blend of two texels? Why does the code use four per ray (where does pi go)? Is the half-texel shift visible?

## Chapter 10, Shading a pixel

Flags:
- "Before reading the probes, the" (4): moving the position along the normal, away from the wall, is stated with no reason; I could not tell what it prevents.
- "The lab's port leaves the" (6): a moment ago the opaque pass fills shadows with the sky's ambient light; now the lab leaves that out. I lost whether this is the engine's behaviour or the lab's simplification and how it follows.
- "It reads their average light," (2): "average light, one colour per probe" is a new object (not the maps I was just told about); I did not know where that average comes from or why fog uses it.

Counts: 1:0, 2:1, 3:0, 4:1, 5:0, 6:1.

What I understood: a compute pass after the depth pre-pass rebuilds each pixel's position from depth, reads its normal, picks the finest cascade that holds it (blending outward near the edge), nudges the point off the wall, and mixes the eight surrounding probes by distance and by facing, so probes behind a wall count almost nothing. Rough surfaces read the radiance map and blend toward irradiance. The result replaces the sky's ambient in the opaque pass. Transparent surfaces redo this themselves, and fog reads probes at the wrong height because it skips the y scale.

Still ask: why nudge off the wall? What does "facing" mean for a probe's weight? Where does fog's per-probe average come from?

## Chapter 11, The frame again

Flags:
- "First, SDFGI keeps the scene" (1): "keeps the scene as data" is vague until the next sentence names grids, field, probes.
- "Third, the price is" (3, 6): "three things" were announced, but the third holds three separate costs (cell size, Static only, thirty frames) joined by "and", and the first two are not costs, so the heading "trades for speed" and the list shape did not line up.

Counts: 1:1, 2:0, 3:1, 4:0, 5:0, 6:1.

What I understood: the recap is clear: cascades jump, slab voxelized, flood builds field, occlusion measured, cells lit a quarter at a time with last frame's probe light giving bounce, probes trace eight rays and sum thirty frames into two maps, pixels weight eight probes. The trade is scene-as-data and work spread over frames, paid for by cell-sized detail, Static-only meshes, and thirty frames of lag.

Still ask: nothing major; only how the three listed costs relate to the first two takeaways.
