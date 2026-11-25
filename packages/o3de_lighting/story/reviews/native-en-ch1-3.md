# Native-English review, chapters 1 to 3

Most of the narration already sounds natural. These 18 sentences are the ones that trip.

1. c1-open 9, "How does O3DE light": "light a room from many lights" is a translated-sounding preposition. Suggestion: How does O3DE light a room with many lights and the sky, without shading every pixel against every light?
2. c1-map-merge 1, "Others apply subsurface scattering": a bare "Others" has no clear referent when heard once. Suggestion: Other passes apply subsurface scattering and ambient occlusion, which later chapters explain.
3. c1-map-cubes 1, "The passes for the sky's": "built with the assets" and the stacked clauses are stiff. Suggestion: The passes for the sky's light read two cubemaps of the sky, which were built along with the assets, long before the frame is drawn.
4. c1-prepass 1, "The pre-pass stores only depth": "that we come back to" dangles at the end. Suggestion: The pre-pass stores only depth, as a thirty-two bit float, with an eight bit stencil beside it, which we come back to later.
5. c1-prepass 13, "Another pass turns this depth": "for the shadow pass and ambient occlusion to read" mixes a pass and a technique. Suggestion: Another pass turns this depth into a plain distance from the camera, for the shadow pass and the ambient occlusion pass to read.
6. c1-targets 22, "So the pass that adds": "has only to multiply" is archaic. Suggestion: So the pass that adds the sky's diffuse light only has to multiply this colour by the sky's light.
7. c1-normal 4, "To store it, the": the passive and the stacked possessive "components' absolute values" are hard to say. Suggestion: First, the normal is divided by the sum of the absolute values of its three components.
8. c1-split 9, "It also takes the": "but just the specular part" sounds loose. Suggestion: It also takes the sky's light inside the forward pass, but only the specular part.
9. c1-models 6, "So they draw in": materials do not draw, so the subject is wrong. Suggestion: So they are drawn in a second forward pass, which also writes the scatter distance that chapter eight uses.
10. c1-recap 4, "Each of these steps,": a step does not cite, so the verb is odd. Suggestion: Each of these steps, like every claim in this video, comes with a citation of a file, a symbol, and a line in the O3DE checkout.
11. c2-why 13, "Teal is a few,": "is a few" is a bare equation. Suggestion: Teal means a few, and the ramp runs up to yellow at eight or more.
12. c2-tile 5, "The pass reads every": "multisampling sample" repeats itself. Suggestion: The pass reads every sample of each pixel, so a thin edge still counts.
13. c2-bins 8, "To choose the count,": "the base two log" is not how it is said. Suggestion: To choose the count, the pass takes the base-two logarithm of the span, rounds it, clamps it, and raises two to that power.
14. c2-cull 1, "The culling pass runs": "runs ... runs" in one sentence. Suggestion: The culling pass uses one thread group for each tile, a team of two hundred and fifty-six threads that the GPU runs together.
15. c2-list 19, "A comment in the": a comment does not "expect". Suggestion: A comment in the code warns of flickering when that happens.
16. c2-findings 1, "The code shows that": "shaped as polygons" is the wrong preposition. Suggestion: The code shows that polygon lights, area lights shaped like polygons, have no group in the list.
17. c2-recap 3, "These short lists keep": "close to the lights" is a vague phrase for cost. Suggestion: These short lists keep the forward pass's cost in line with the lights a pixel can really see.
18. c3-inputs 3, "They are the base": "metallic" stands alone as a noun. Suggestion: They are the base colour, the metallic value, the specular factor that scales F zero, roughness, the normal, and the occlusion.

## Habits seen across the chapters

- Passive or subjectless verbs where a native would name the actor or use the plain verb (draw, cite, expect, have only to).
- Bare adjectives or noun fragments used as nouns (metallic, Others, "Teal is a few"), which are hard to follow by ear.
- Stacked possessives and noun chains (components' absolute values, shadow pass and ambient occlusion to read) that are awkward to say aloud.
- Repeated verbs close together (runs ... runs, multisampling sample) that a speaker would vary or drop.
- Prepositions that follow the technical source rather than speech (light from, shaped as, built with).
