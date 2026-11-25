# Cold listen, English, chapters 1 to 3

Listener: a graphics programmer (PBR forward renderer, shadow maps, compute), new to O3DE, one hearing each.
Kinds: (1) lost a reference, (2) undefined or too-distant term, (3) re-parse, (4) fact without reason, (5) two or more new ideas, (6) lost the thread.

## Flagged sentences

### Chapter 1
- c1-open 1 "This is the lab's" - 1,2 - "the lab", "Atom" arrive as names with no sense yet.
- c1-open 3 "Right now we see" - 1 - "computed itself": itself as opposed to what?
- c1-pipeline 5 "That default pipeline renders" - 4 - why two samples matters, I do not know.
- c1-pipeline 6 "The lab renders four" - 4 - what the difference changes for the story.
- c1-map-shadow 1 "The shadow passes draw" - 5 - drawing shadow maps and a per-pixel value at once.
- c1-map-forward 1 "Then the forward pass shades" - 2 - "five targets" with no idea what they are yet.
- c1-map-after 1 "Then full-screen passes read" - 1 - "those targets": which five?
- c1-map-after 2 "They add the sky's" - 2 - where the sky's light comes from is not said yet.
- c1-map-merge 1 "Others apply subsurface scattering" - 1,2 - "others" which passes; subsurface scattering undefined.
- c1-map-merge 2 "A merge pass then" - 5 - merge and transparent meshes in one breath.
- c1-map-cubes 1 "The passes for the sky's" - 2,4 - which two cubemaps, and why two.
- c1-prepass 13 "Another pass turns this" - 5 - a new pass, a new output, and two consumers.
- c1-targets 12 "The specular target is shown" - 5 - exposure boost, its size, and the reason together.
- c1-targets 19 "The diffuse response is" - 6,2 - jump from the albedo's colours to "diffuse response".
- c1-targets 20 "These colours are a" - 1,2 - "base colours" and "that response" not tied back.
- c1-targets 21 "So is the material's" - 3 - "So is" took a moment to resolve to "multiplied in".
- c1-targets 41 "Back in the F zero" - 6 - returned to a target three views ago.
- c1-targets 42 "So the later passes" - 4 - why the 8-bit precision matters is not said.
- c1-normal 6 "The normal's x and y" - 4,5 - rotation by 45 degrees, plus remapping, no reason.
- c1-normal 10 "A flat surface shows" - 1 - "the panel" which panel.
- c1-stencil 3 "Its two lowest bits" - 3 - "its", "they" attach loosely to stencil or bits.
- c1-stencil 9 "Later, the reflection probes" - 2,5 - probes undefined, plus bits, plus brighter drawing.
- c1-models 5 "Enhanced and Skin add" - 2 - subsurface scattering and transmission left for later.
- c1-models 6 "So they draw in" - 6,2 - "So" does not follow; "scatter distance" unknown.
- c1-recap 4 "Each of these steps" - 6 - turns to citations out of nowhere.

### Chapter 2
- c2-why 7 "That is eight thousand" - 4 - a big number with no point.
- c2-why 8 "The screen is now" - 1,5 - "split in two" is about the display, not the engine; counts introduced at once.
- c2-why 9 "Each tile's list is" - 5 - depth bins arrive while still decoding the colours.
- c2-tile 6 "This depth view is" - 4 - why the black point moved.
- c2-bins 8 "To choose the count" - 3 - log, round, clamp, power: a chain of verbs I could not hold.
- c2-bins 11 "So no bin is" - 3 - numeric conclusion I had to reconstruct.
- c2-list 15 "The end-of-list marker takes" - 3 - slot arithmetic, "those slots" resolved late.
- c2-list 16 "The seven group markers" - 1 - "them" the slots?
- c2-falloff 4 "The code squares that" - 3 - formula by ear.
- c2-falloff 6 "In its automatic mode" - 1,2 - "its" light or editor; the editor is new.
- c2-types 6 "Quads use linearly transformed" - 2,6 - the name before what it means.
- c2-types 9 "A cosine lobe has" - 4 - why the warped lobe keeps the integral is not clear.
- c2-types 10 "Those fitted matrices are" - 4 - table sizes with no consequence.
- c2-findings 2 "The forward pass's polygon" - 3 - start, marker, end: a loop resolved only at the end.

### Chapter 3
- c3-inputs 3 "They are the base" - 3,5 - a verbless list of seven inputs.
- c3-metal 3 "As metallic rises, the" - 3 - two changes and an aside in one sentence.
- c3-rough 2 "That roughness is squared" - 2,5 - "roughness A" named, squared, and given a use at once.
- c3-rough 3 "Roughness A is squared" - 1 - A, A two: I lost which is which.
- c3-rough 4 "Each later formula picks" - 4 - why three forms.
- c3-occl 2 "Each can come from" - 3 - "its factor" resolved late.

## What I understood

Chapter 1: O3DE lights in a fixed pass order: depth pre-pass, light culling into tiles, shadows, then a forward pass that writes direct light and the surface (albedo, F0 and roughness, normal) into five targets. Full-screen passes later add the sky's diffuse light and reflections, then occlusion and a merge. Stencil bits route pixels, and mobile skips culling. Still asking: what the sky's cubemaps are, what subsurface scattering and probes are, why the lab's sample count differs, and why the 10-bit packing is done this way.

Chapter 2: the screen is cut into 16 by 16 tiles, each split into 1 to 8 depth bins, and each bin keeps a short list of the lights that reach it. A compute pass tests lights against the tile's box and a 32-slice depth mask. The forward pass walks the list by type. Lights fall off to zero at their radius. Sized lights use closed-form approximations. Three findings are bugs in the engine. Still asking: how the bin count and the 248-light cap matter in practice, and how the quad light's integral works.

Chapter 3: a material's inputs become albedo, F0, three forms of roughness, a normal and two occlusion values. Metallic moves colour from diffuse to reflection. Specular anti-aliasing adds roughness where the normal changes fast. Decals change what direct lights see but not what the stored targets hold. Still asking: why roughness is squared twice, and what the three forms are each for.

## Counts per kind
- (1) lost a reference: 10
- (2) undefined or distant term: 13
- (3) re-parse: 10
- (4) fact without a reason: 10
- (5) several new ideas at once: 10
- (6) lost the thread: 5
Flagged sentences: 45.
