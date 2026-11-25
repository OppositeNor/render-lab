# Cold listen, English, chapters 4 to 6

Kinds: 1 lost reference, 2 undefined or forgotten term, 3 re-parse, 4 fact with no reason, 5 two or more new ideas, 6 lost thread.

## Flagged sentences

### Chapter 4
- c4-g 6 "O3DE folds that divisor into" (3, 4): I lost why the code then multiplies by N dot L directly; a divisor is folded in, yet the cosine appears as a multiplier.
- c4-diffuse 5 "That Fresnel term is made" (4): I could not tell how it is made gentler, or why that keeps the response near one minus F zero.
- c4-models 3 "Skin uses a different diffuse," (3, 5): Skin, Enhanced, a new diffuse and a condition all arrive in one "so does" sentence.
- c4-models 5 "Back to Standard, its clear" (6): the walk through models jumps Standard, Skin/Enhanced, Standard, Enhanced, Skin; I had to re-track which model I was in.
- c4-models 10 "The code shows that a" (4): I did not know why the coat dimming diffuse twice matters, or whether it is a flaw.
- c4-lut 3 "Each texel averages the GGX" (3, 5): averaging over the hemisphere "once for an F zero of zero and once for one" packs two averages into one sentence.
- c4-lut 4 "From those two averages come" (5): the scale A and the bias B arrive at once, with no sense of how two averages become them.
- c4-lut 14 "So the reflection over the" (6): "So" does not follow from the 1024 directions; it is a new claim.
- c4-multi 3 "A plus B is the" (4): I could not tell why A plus B is the share one bounce keeps.
- c4-multi 7 "To get the factor, that" (4): why the loss is multiplied by F zero is not given.

### Chapter 5
- c5-why 7 "The view lays the four" (2): "debug colour" is used before anything says what the debug colouring is.
- c5-why 14 "As for how the map" (6): after the compare is described, the talk goes back to how the map is drawn; "depth bias" is new here.
- c5-cascades 10 "The full hundred metres would" (4): three more numbers with no verb and no reason.
- c5-lookup 3 "The offset is the normal" (3): a formula spoken as a list of operations; I lost it before the result in sentence 4.
- c5-bias 6 "The blur comes later, so" (2): no blur has been mentioned, so I did not know which blur.
- c5-bias 8 "The normal offset lifts the" (5): the offset's job and its cost (shadows detaching from casters) both arrive in one sentence.
- c5-bias 9 "A slope bias, which grows" (2, 4): a second bias is introduced only to be switched off next; I could not tell why it matters.
- c5-fullscreen 5 "Every opaque and alpha cut-out" (2): "alpha cut-out" is not defined.
- c5-blur 3 "It reaches eight pixels to" (2): "constant falloff" is a term I could not place.
- c5-blur 9 "They are percentage-closer filtering" (2): exponential shadow maps are named and never explained.

### Chapter 6
- c6-two 3 "Each smaller mip is prefiltered" (2): "prefiltered" is not explained here.
- c6-diffuse 6 "But O3DE's default project template" (6): "But" contrasts with nothing I had just heard.
- c6-diffuse 8 "That gem is the diffuse" (2): "light probes" and the "diffuse probe grid" are used but not defined.
- c6-splitsum 1 "The prefiltered colour is then" (3): "multiplied by ... F zero times A, plus B" can be read two ways until the end.
- c6-probe 5 "A stencil pass counts how" (3): "how deeply nested" resolved only at the end, and "inner boxes" had just been defined once.
- c6-probe 9 "The weights of all probes" (4, 6): weights come after a stencil count with no link between them, and I could not tell what they are for.
- c6-findings 3 "Some pixels inside an inner" (1): I lost which pixels have image-based light off, and where that is set.
- c6-findings 5 "Such a pixel keeps the" (6): the grey was mentioned in sentence 4, so "such a pixel" attaches back across it to sentence 3.

## What I understood, and what I would still ask

Chapter 4: One light's specular is D times G times N dot L times F, built from the four directions N, L, V and H. D counts facets facing H, G removes hidden facets, and F rises toward white at grazing angles. Diffuse is Lambert scaled by what reflection leaves. A BRDF look-up texture, built once, stores a scale A and a bias B, so the sky's reflection is F zero times A plus B. Multiscatter gives back lost energy but is off by default. I would still ask why N dot L is multiplied after the divisor is folded in, how A and B come from the two averages, why A plus B is the kept share, and how the model list fits together.

Chapter 5: The sun has one shadow map of four cascades, split near-dense, each wrapped in a snapped sphere. A full-screen pass offsets P, picks a cascade, biases the depth and compares, giving a hard 0 or 1 in a shadow buffer. A depth-aware blur then softens it. Local lights share an atlas. I would still ask what the debug colouring and the slope bias are for, what the blur setting means, and what exponential shadow maps are.

Chapter 6: Two prefiltered cubemaps give sky light: diffuse by one lookup along the normal, specular by a mip chosen from roughness, times F zero A plus B. Reflection probes make local cubemaps, blended by weights from stencil counts and box ramps, with parallax correction. The probes ignore the specular occlusion, and some pixels keep a grey. Screen-space reflections and the diffuse probe grid are off or out of reach. I would still ask how the stencil count becomes the weights, what the "image-based light off" pixels are, and what the diffuse probe grid and gem do.

## Counts per kind
1 (lost reference): 1
2 (undefined or forgotten term): 8
3 (re-parse): 6
4 (fact with no reason): 8
5 (two or more new ideas): 4
6 (lost thread): 6
Flagged sentences: 28
