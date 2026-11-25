# Cold listen, English, chapters 7 to 10

Kinds: (1) lost a reference, (2) undefined or too-distant term, (3) re-parse, (4) fact with no reason, (5) two or more new ideas, (6) lost the thread or name before what it names.

## Flagged sentences

### Chapter 7
- c7-why 10 "There, many of the samples": 2, 3. "Samples" arrive mid-sentence with an appositive; I had to re-parse which surface is "the other".
- c7-samples 9 "The first twelve are fixed,": 3, 5. Twelve of what, where "four and eight pixels away" fits, and "cross and diagonals" all at once.
- c7-samples 10 "Then forty more samples follow": 5. Spiral, forty, and ten rounds of four on top of the twelve.
- c7-samples 11 "The spiral is shared between": 4. A block of sixteen pixels sharing a pattern, and I do not know why it matters.
- c7-samples 15 "So samples on the teapot": 6. "So" does not follow clearly from the weight formula just before it.
- c7-radius 2 "At ten eighty, that is": 1. Ten eighty full-resolution, but the percentage was of the half-resolution height; the twenty-seven did not add up for me.
- c7-output 2 "That value is raised to": 4. A power of 2.2 with no reason given.
- c7-where 3 "The specular target, and the emission": 1. I was not told emission lives in the specular target.
- c7-where 4 "The way the passes are": 3. Long subject, then a negated appositive about "ambient-only" that I had to unpack.
- c7-material 6 "Inside the forward pass, the diffuse": 1, 4. "On its way in" is unclear, and I cannot tell what twice-applied does.

### Chapter 8
- c8-which 4 "In an EnhancedPBR material, transmission": 2. EnhancedPBR as a material versus the Enhanced model was not set apart.
- c8-thick 1 "The thick object mode, after": 3, 6. Two surnames arrive before anything they name, then "brightens where the eye looks toward the light through the object" is dense.
- c8-thick 2 "The model measures how directly": 3. "With the light's direction bent a little by the normal" attaches ambiguously.
- c8-thin 1 "The thin model, after Jimenez,": 6. A name before what it names.
- c8-thin 3 "The thin model then weakens": 2, 5. Measured thickness is not yet defined (a later beat), and scatter distance is introduced in the same sentence.
- c8-thickness 6 "But the code turns that": 2. "The camera's depth span" was not defined.
- c8-thickness 7 "When the thickness of a": 1, 2. What is shifted, and by what, is not said yet.
- c8-thickness 8 "With the default biases, the": 2, 5. "Thin mode's shift into the surface" is new, and it is set against the chapter five offset.
- c8-foliage 9 "Transmission and the shadow offset": 4. I do not hear what the unflipped normal changes.
- c8-sss 5 "The pack is the factor's": 5. Byte arithmetic on top of the two new settings.
- c8-sss 6 "That integer is stored in": 4, 5. Half-float exactness up to 2048 with no consequence I can use.
- c8-sss 7 "A compute pass then blurs": 2. Burley's profile is named before it is explained.
- c8-skin 4 "Eye, the material type for": 1, 4. "Skin's material setup but StandardPBR's direct lighting" has no referent and no reason.

### Chapter 9
- c9-own 8 "The terrain's feature processor, the": 2, 5. Feature processor is new, and it is followed straight away by compute passes.
- c9-own 11 "These compute passes are off": 4. The lab draws clipmaps as an illustration, and the passes are off; the relation between the two was lost.
- c9-fixed 5 "The terrain's lighting data never": 1. "Lighting data" has no referent for me.
- c9-fixed 7 "Even so, the multiscatter flag": 2, 3. The flag was a chapter one remark, long gone, and the "even so" structure is heavy.
- c9-fixed 11 "Up close, detail materials replace": 2. Detail materials and "plain weights" are new.
- c9-fixed 12 "Detail materials could also blend": 4. An option that is off by default, with no reason to care.
- c9-normal 4 "The vertex shader rebuilds the": 1. "The up component" versus the stored slopes was unclear.
- c9-normal 8 "The terrain can have a": 6. A macro normal map appears from nowhere between findings.
- c9-hair 6 "Each lobe is a Gaussian": 1, 5. "A second term for the angle around the strand" is vague, and two new ideas arrive together.

### Chapter 10
- c10-trace 6 "P's reflection came from the": 3. The one sentence packs mip, scaling, and the F zero A plus B formula.

## What I understood, per chapter

- **Chapter 7.** SSAO uses only depth to darken creases that hide the sky. It takes fifty-two weighted samples around a point. Its radius is a share of the screen, not metres, so it reaches farther as the camera pulls back. The result is blurred, upsampled, and multiplied into all diffuse light, direct too. Material occlusion maps are separate multipliers. Still asking: how the cosine average becomes the darkening, why the power 2.2, and what the double application of diffuse occlusion does visibly.
- **Chapter 8.** Transmission exists only in Enhanced and Skin models, in thick and thin modes. Thickness comes from the sun's shadow map and its scale is off. Leaves in O3DE are ordinary materials. Subsurface scattering is a blur of the diffuse light, which the lab does not run. Skin and Eye add lobes and diffuse variants. Still asking: what exactly the flipped-normal catch does, the double shift that cancels, and how Eye differs.
- **Chapter 9.** Terrain has its own shader but writes the same targets and reads the same lights and shadows. The macro values are fixed. Its normals are computed per vertex on the CPU in eight bits and are wrong on two-way slopes. Hair uses three Marschner lobes. Still asking: what the feature processor and clipmaps are for, why detail materials matter, and what each lobe term is.
- **Chapter 10.** P's path is retraced in order: depth, shadow, forward pass, sky light, reflection, SSAO, merge. The three takeaways are the thin G-buffer, reuse, and trade-offs. Still asking: where light culling and the shadow blur sit in the trace, since they were barely named.

## Counts per kind (33 flagged sentences)

1. Lost a reference: 8
2. Undefined or too-distant term: 10
3. Re-parse: 7
4. Fact with no reason: 8
5. Two or more new ideas: 8
6. Lost the thread, or a name before what it names: 4
