# Cold listener, English, round 2, chapters 1 to 3

Kinds: 1 reference lost, 2 term undefined or defined too long ago, 3 re-parse, 4 fact with no reason, 5 two or more new ideas, 6 thread lost.

## Flagged sentences

### Chapter 1
- c1-pipeline 1 "Everything here comes from" [4]: why the version and commit matter, I could not tell.
- c1-map-forward 1 "Then the forward pass shades" [2]: "five targets" with no idea what they are yet.
- c1-map-cubes 1 "The passes for the sky's" [2]: which two cubemaps, and what each holds.
- c1-prepass 14 "Another pass turns this depth" [5, 6]: a new pass and a new quantity ("plain distance") dropped at the end of a list about this depth.
- c1-targets 28 "That four percent is the" [2]: "specular factor" not defined here; I could only guess it scales reflectance.
- c1-normal 6 "The normal's x and y" [4]: a forty-five degree turn with no stated purpose.
- c1-split 7 "Transparent materials, and the skin" [5]: exceptions for transparents and skin, plus a forward-pass reason, in one sentence.
- c1-stencil 4 "The stencil's two lowest bits" [3]: "two bits together make three" took a re-parse.
- c1-stencil 11 "Later, the reflection probes" [2]: reflection probes not yet defined.
- c1-stencil 12 "The view draws those pixels" [1]: which pixels, the probe ones or something else.
- c1-alt-deferred 1 "O3DE also contains code" [2]: "visibility buffer" not defined.
- c1-recap 4 "One more thing about" [6]: a jump from the forward pass to how the video is made.

### Chapter 2
- c2-why 10 "Each tile's list is" [5, 6]: depth bins introduced mid-description of the left view, a new idea I had to hold.
- c2-why 11 "The left view counts, for" [3]: it contradicts or refines sentence 9 (each pixel's count), so I re-parsed what the colour means.
- c2-bins 1 "The tile also chooses" [6]: bins versus the thirty-two slices just before; unclear how they relate.
- c2-bins 10 "So a span under one" [3, 4]: thresholds appear right after a log-round-clamp recipe, and I could not connect them.
- c2-bins 12 "So in any tile up" [4]: why the bin depth bound matters, and where eleven and one and a half come from.
- c2-cull 8 "The test also records" [5]: a second job (bin bits) tacked onto the culling test.
- c2-iterate 6 "Inside that list, the" [3]: a list and its groups against the end-of-list stop; I lost which loop is outer.
- c2-iterate 7 "Inside that walk, each" [3]: inner loops per type reading until a group marker; hard to hold against sentence 5.
- c2-falloff 7 "The editor, O3DE's tool" [4, 6]: why the editor is here, I could not tell.
- c2-falloff 8 "The radius is the" [2]: a cutoff of 0.1 with no unit or meaning.
- c2-types 9 "A fitted matrix warps" [5]: matrix, warping, cosine lobe, BRDF shape, and the method's name at once.
- c2-types 12 "Those fitted matrices are" [4]: why two tables.
- c2-findings 11 "But the specular uses" [1]: "the specular" of what, the light or the pass.

### Chapter 3
- c3-inputs 3 "It builds them from" [3]: a long list with an embedded clause, resolved only at the end.
- c3-inputs 5 "A colour property is" [2]: ACEScg not defined.
- c3-rough 4 "Roughness A is squared" [1]: "roughness A two" heard as a name I could not pin down.
- c3-rough 5 "Each later formula picks" [4]: no reason for having three forms.
- c3-aa 5 "Twice that variance is" [4]: the factors (a quarter, twice) have no stated purpose.
- c3-occl 8 "It is authored in" [2]: EV100 is called "a photographic exposure scale" but I got no sense of it.
- c3-decal 12 "But the targets are" [3, 6]: after "direct lights see the decal", which targets and why they differ took a re-parse.

## What I understood, per chapter

Chapter 1: O3DE lights in a forward pass that does only direct light and writes five targets (diffuse, specular, albedo, F0 with roughness, normal). Later full-screen passes add the sky, reflections and occlusion. A stencil byte routes pixels to those passes. Mobile and deferred paths exist but do not matter. Still to ask: what the five targets are for as a whole; what the two cubemaps are; why the normal is packed that way; how a clear coat avoids double reflections.

Chapter 2: lights are culled into 16 by 16 tile lists, split by depth bins using a 32-slice depth mask, then each pixel loops only its bin's lists, with a falloff window to zero at the radius. Sized lights use closed-form approximations, quads use LTC. Three oddities were found in the code. Still to ask: how the bin count formula gives those numbers; why entries carry bin bits; how the two matrix tables are used.

Chapter 3: a material's inputs become albedo, F0 (from metallic and specular factor), three forms of roughness, a normal and two occlusions, with specular anti-aliasing and decals changing parts of that. Decals do not reach the sky passes. Still to ask: why three roughness forms; what the anti-aliasing numbers are for; what ACEScg and EV100 are.

## Counts per kind
- 1 reference lost: 3
- 2 term undefined: 8
- 3 re-parse: 7
- 4 fact with no reason: 8
- 5 two or more new ideas: 5
- 6 lost the thread: 6

Flagged sentences: 33.
