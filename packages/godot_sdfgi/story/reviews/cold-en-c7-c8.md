# Cold listen, English, chapters 7 and 8

Listener: the card only, chapters 1 to 6 already heard.

## Chapter 7: Light in the voxels

Flags (first five words, problems, what I lost):

- "Then it raises that ratio" (3): a chain of arithmetic steps with "it", "that ratio", "the result"; I could follow it only by holding each step in mind until "that result is the falloff".
- "That falloff is then divided" (5): a second operation, and a new idea (a power of distance) starts before the first formula has settled.
- "That power is the light's" (5, 3): "power" is first the fourth power, then a different exponent; I briefly attached it to the wrong one.
- "The cell also keeps one colour" (5, 6): after per-face sums, a single colour appears. I could not tell how it relates to the six faces or why both exist.
- "It also stores a weight" (5, 4): a third per-cell quantity; I did not know what the weight is for, or what "the face gets" means.
- "The cell's light is also copied" (2): the neighbour flags came from chapter five, which I had mostly forgotten; the sentence leans on "flagged in chapter five".
- "The static light is added" (4): I did not know why this fact matters until the next beat.
- "The bounce feedback scales each bounce." (4): a restatement; I did not learn what it changes.
- "The eight probes around each" (5, 6): "probes already hold light from these cells a frame earlier" arrives with the eight probes, the loop, and the previous frame at once, and it sits oddly against "later the probes' rays will read this light".

Counts: 1:0, 2:1, 3:2, 4:2, 5:4, 6:2.

What I understood: each frame a quarter of the solid cells are relit. Each light's effect is a falloff with an odd distance measure from the stretched height. A shadow ray steps by the stored distance and the shadow is all or nothing. Light is stored per face and copied to nearby empty cells. A static light is added again on every jump, so kept cells keep getting brighter. Last frame's probe light feeds back into the cells and makes a multi-bounce loop.

What I would still ask: why both a summed colour and per-face light? What is the weight for? Is the static-light repeat intended?

## Chapter 8: Probes trace the field

Flags:

- "Over thirty frames, the rays" (5): the 240 directions and the thirty-frame window come in together, and I did not know why thirty.
- "Whether a ray's number" (4, 3): I did not know why the sphere is split into halves or what the split changes.
- "One frame's ray numbers are" (3, 6): "thirty apart" was never explained, since I had not been told how a frame's eight numbers are chosen; I could not follow the parity argument.
- "The halves alternate from" (4): what this achieves is not said.
- "At a hit, the ray" (6): I heard that the ray needs a normal before hearing what the normal is for.
- "So the code does not" (4): I could not tell what actually happens when the sky is undefined, or whether it matters.
- "The coefficients are then scaled" (5): the scale and the fixed-point storage arrive together.
- "So when a light turns" (6): I could not tell how this follows from subtracting and adding frames, and the thirtieth went by quickly.

Counts: 1:0, 2:0, 3:2, 4:3, 5:2, 6:2.

What I understood: each probe traces eight rays a frame by stepping through the distance field. A hit takes the voxel's light, weighted by the face normal from the gradient. A miss reads a blurred sky, which the code leaves undefined in 4.7 stable. The light is stored as 16 fixed-point spherical-harmonic coefficients, and a thirty-frame ring with an exact running sum gives a slow, linear response to changes.

What I would still ask: what the even/odd halves are for, why thirty frames, and what a miss reads in 4.7.
