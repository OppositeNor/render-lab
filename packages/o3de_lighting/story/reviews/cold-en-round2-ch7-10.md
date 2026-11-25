# Cold listen, English, round 2: chapters 7 to 10

Listener: graphics programmer (PBR forward renderer, shadow maps, compute); chapters 1 to 6 heard earlier. Kinds: (1) lost a referent, (2) undefined or too-distant term, (3) re-parse, (4) fact with no reason, (5) two or more new ideas, (6) lost the thread.

## Flagged sentences

### Chapter 7
- c7-why 11 "There, SSAO reads depths": 3. "There" snaps back to the creases, but the previous sentences were about the target; first said it reads the half-resolution copy, now "the depth buffer".
- c7-samples 1 "We follow a floor": 6. Is this P? Chapter 1 gave P as low on the teapot; a new floor point appears without saying so.
- c7-samples 12 "Then forty more samples": 5. Spiral, forty, ten rounds of four, all at once after the fixed twelve.
- c7-samples 13 "The sixteen pixels of": 2, 5. A four by four block sharing one spiral arrives with no reason and no earlier hint.
- c7-samples 14 "In each round, a": 1, 3. "Reads three that its neighbours fetched": lost whose fetches, and why one fetch plus three reads.
- c7-samples 18 "The weight is one": 4. A formula with 0.07 in it; I could not tell why the constant is there or what it changes.
- c7-radius 2 "On a screen ten": 5. Three numbers in one sentence (1080, 540, 27); I lost the chain.
- c7-where 3 "The specular target is": 1. "Emitted light the forward pass wrote into it": into the specular target? I had not heard emission land there.
- c7-material 5 "But for transparent materials,": 2. Eye appears as a material type for the first time here; chapter 1 spoke only of Skin.

### Chapter 8
- c8-thick 4 "Before it measures, it": 4. Adding a little vertex normal to the light direction; I could not tell why.
- c8-thickness 3 "The engine takes the": 4. Why take the larger of two thicknesses; what it changes.
- c8-thickness 8 "But the code turns": 3, 5. A share of one range turned into a distance by another span; the contrast "different span" resolves only at the end, and the camera span is a new idea.
- c8-thickness 12 "The other is a": 1, 4. "A shift that the thin mode makes into the surface": which shift, in what direction, and why.
- c8-thickness 15 "Last is the place": 3. Odd structure ("Last is the place where the glow is added"); I had to rebuild it.
- c8-foliage 11 "Transmission and the shadow": 4. Both use the unflipped normal; I did not hear what goes wrong because of it.
- c8-sss 5 "The packed value is": 5. Byte times 256 plus byte, then half float, then 2048, in quick succession (sentences 5 to 8).
- c8-sss 8 "Above that, the value": 4. Rounded quality byte: why does that matter, what visible effect.
- c8-sss 9 "A compute pass then": 3. "Blurs the diffuse light along a curve of how far light spreads": the curve attaches awkwardly; I had to re-parse.

### Chapter 9
- c9-own 8 "The terrain is managed": 2. "Feature processor" is named and glossed only as "an engine part of its own"; I do not know what that is.
- c9-fixed 7 "Even so, the hills": 2, 6. The multiscatter flag of chapter one is far back; "even so" ties to the compensation never computed, but I lost how.
- c9-fixed 9 "Turning the flag off": 3, 4. Result stated before its reason (next sentence); I did not see why it matters.
- c9-fixed 14 "Detail materials could also": 2. "Blend by height" is not explained.
- c9-hair 12 "The model uses a": 5. Refractive index and then the cuticle tilt in the next line; two new parameters back to back with no picture of what each does.

### Chapter 10
- No sentence flagged. The trace and takeaways follow the earlier chapters in order.

## What I understood, per chapter

- Chapter 7: SSAO uses only depth, at half resolution. It rebuilds a normal from depth, takes 52 samples (12 fixed, 40 spiral), weights them by rise and distance, blurs and upsamples. Its radius is a share of the screen. It darkens all diffuse light, direct too. The material's own occlusion maps are a separate multiplier. Still ask: why a four by four block shares one spiral; what the 0.07 does; whether the squared sky light for Skin and transparent materials is visible.
- Chapter 8: Only Enhanced and Skin transmit. Thick mode gives a glow toward the light that fades with thickness. Thin mode uses the back face, scatter distance per channel, which gives orange. Thickness comes from the sun's cascade and is not in metres. Leaves are ordinary materials unless they use EnhancedPBR. Subsurface scattering is a blur of the diffuse light that the lab does not run. Skin and Eye add their own lobes. Still ask: what the "shift into the surface" is; what actually goes wrong with the unflipped normal; what a lost quality byte does to the picture.
- Chapter 9: Terrain writes the same five targets through its own forward shader, so later passes treat it like the room. The macro material is fixed (rough, non-metal). Its normal is built on the CPU and is wrong on two-way slopes. Hair is Marschner's R, TT, TRT lobes, composited after the opaque frame. Still ask: what a feature processor is; why the hills write a multiscatter flag that changes only the sky; what height blending is.
- Chapter 10: The pixel's path is recapped in order, and the three takeaways (thin G-buffer, reuse, trade-offs) are clear. Nothing further to ask.

## Counts per kind

- (1) lost referent: 3
- (2) undefined or distant term: 5
- (3) re-parse: 6
- (4) fact with no reason: 7
- (5) two or more new ideas: 6
- (6) lost thread: 2

Flagged sentences: 23.
