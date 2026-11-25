# Storytelling review, English narration, chapters 1 to 3

Method: every sentence mapped (known | new | job) against `storytelling.md`, its examples, and the ear rules. Only faulty sentences are listed. 34 sentences are flagged. A rewrite never adds a fact. SPLIT marks a rewrite into several sentences (parts separated by " / "). NEEDS FACT marks a fix that needs a reason the narration does not give.

## Flags

Row format: beat, sentence number, first six words | map (known | new | job) | fault | rewrite

### Chapter 1

1. c1-map-lut, 1, "The forward pass also reads a" | known: forward pass | new: BRDF look-up texture, its gloss, "chapter four explains", "built once at start-up" | job: act | Overloaded, more than one appended phrase | SPLIT: The forward pass also reads a BRDF look-up texture, a table of reflection values. / Chapter four explains that table. / It is built once, at start-up.
2. c1-map-cubes, 1, "The passes for the sky's light" | known: sky's light passes | new: two cubemaps, built with the assets, long before the frame | job: act | Two new parts, two appended phrases | SPLIT: The passes for the sky's light read two cubemaps of the sky. / These were built with the assets, long before the frame.
3. c1-prepass, 1, "The pre-pass stores only depth, as" | known: pre-pass | new: 32-bit float depth, an 8-bit stencil, "we come back to" | job: act | Overloaded: two new things (depth format, stencil) in one sentence | SPLIT: The pre-pass stores only depth, as a thirty-two bit float. / An eight bit stencil sits beside it, and we come back to that.
4. c1-targets, 10, "So the floor is bright where" | known: tone mapping (9) | new: floor bright where lights reach | job: check | Wrong "so": the floor is bright because the lights reach it, not because it is tone mapped | Now the floor is bright where the sun and the point light reach it.
5. c1-targets, 19, "The diffuse response is the share" | known: nothing (18 is about plain colours) | new: the diffuse response | job: name | Chain broke: a term with no act before it; 20 needs it | Put it after 20's claim: These colours are a little darker than the base colours, because the diffuse response is already multiplied in. / The diffuse response is the share of light that is not reflected at the surface.
6. c1-targets, 21, "So is the material's diffuse" | known: "multiplied in" (20) | new: diffuse occlusion, with gloss | job: act | Elliptical "So is" is hard to follow by ear; the new part is buried | The material's diffuse occlusion, how much of the diffuse light a crevice blocks, is multiplied in too.
7. c1-normal, 4, "To store it, the normal" | known: normal target | new: division by absolute sum | job: act | "it" has two referents (target, normal); a passive with no actor | To store each normal, the pass first divides it by the sum of its three components' absolute values.
8. c1-normal, 10, "A flat surface shows one flat" | known: colours do not show a direction (9) | new: flat colour, panel decodes P | job: check, then point | Two new parts joined with "and" | SPLIT: A flat surface shows one flat colour. / So the panel decodes P's normal for us.
9. c1-split, 7, "Only transparent materials, and the skin" | known: opaque case (1-6) | new: who takes image-based light in the pass | job: act | "Only" is contradicted by 8 and 9 ("One more case"); the new part opens the sentence | Transparent materials, and the skin materials of chapter eight, also take image-based light inside the forward pass itself. (Then 8 reads "A third case is ...".)
10. c1-stencil, 1, "Each mesh also writes a stencil" | known: stencil (prepass) | new: its byte value, the passes' test | job: name plus act | Two new parts with "and" | SPLIT: Each mesh also writes a stencil value, a byte per pixel. / The full-screen passes test it to choose which pixels they shade.
11. c1-stencil, 3, "Its two lowest bits together make" | known: top bit | new: low bits make three, reflection passes | job: act | "Its" and "they" could point at the stencil or the bits; two new parts | SPLIT: The stencil's two lowest bits together make three. / When those two bits are set, the reflection passes shade the pixel.
12. c1-stencil, 9, "Later, the reflection probes, which" | known: bits (8) | new: probes' own bits, brighter colour | job: act | Interrupting clause, two new parts | SPLIT: Later, the reflection probes add bits of their own, and chapter six explains them. / The view draws those pixels brighter.
13. c1-models, 6, "So they draw in a second" | known: Enhanced, Skin add SSS | new: second forward pass, scatter distance | job: act | "So" claims a cause the narration does not give; two new parts | NEEDS FACT (why SSS needs a second pass); until then: They draw in a second forward pass. / That pass also writes the scatter distance, which chapter eight uses.
14. c1-alt-deferred, 2, "But no pass uses that" | known: deferred code (1) | new: no pass uses it, the cvar is off | job: branch | Two new parts with "and" | SPLIT: But no pass uses that code. / The console variable that would turn deferred shading on is also off.
15. c1-recap, 4, "Each of these steps, like" | known: recap of targets (1-3) | new: every claim cites file, symbol, line | job: none clear | Topic turn with no bridge, and it interrupts the landing | One more thing about this video before we move on. / Each of these steps, like every claim in it, cites a file, a symbol, and a line in the O3DE checkout.
16. c1-recap, 6, "Before any of that, the" | known: "that" = the steps or the citations (4-5) | new: culling is next | job: bridge | "that" has two referents | Before the forward pass can shade a pixel, the engine has to decide which lights reach each tile. / That decision is the next part.

### Chapter 2

17. c2-why, 8, "The screen is now split" | known: tiles | new: split in two, left view colours by count | job: point | Two new parts; "split" already means tiles and later depth bins | SPLIT: The picture now shows two views. / The left view colours each pixel by how many lights the culling kept for it.
18. c2-why, 9, "Each tile's list is split" | known: tile lists | new: depth bins | job: act | An act with no purpose; the purpose arrives ten sentences later (c2-bins 13) | Each tile's list is split again by depth, into slabs called depth bins, so a pixel reads only the lights near its own depth.
19. c2-list, 6, "The lights of one type" | known: cells coloured by type | new: group, marker | job: name | Two new parts with "and" | SPLIT: The lights of one type form a group. / A darker cell, called a marker, closes each group.
20. c2-list, 14, "That makes up to eight" | known: remap copies entries | new: lists of 256 entries | job: act | "entries" here, "slots" in 15, "entries" in 15 again: drift, and 15 corrects 14 | That makes up to eight lists, each with two hundred and fifty-six slots, and each ended by an end-of-list marker.
21. c2-iterate, 7, "Each light type has its" | known: "the loop" over the bin's list (2, 5) | new: a loop per type | job: act | One name, "loop", for two things (the walk over the list, and each type's loop) | Inside that walk, each light type has its own inner loop, which reads indices until it meets its group's marker.
22. c2-falloff, 1, "For each light in a" | known: the loop adds each light (c2-iterate 8) | new: falloff | job: act | Topic turn with no bridge | Once the loop has a light, it needs to know how bright that light is at P. / The light's intensity falls as one over the distance squared.
23. c2-types, 1, "Lights that have a size" | known: lights in the loops | new: closed form, no sampling, each own approximation | job: name | Topic turn with no bridge, and three appended phrases | One more kind of light needs more care. / Lights that have a size are evaluated in closed form, by a direct formula with no sampling. / Each has its own approximation.
24. c2-types, 6, "Quads use linearly transformed cosines." | known: disks and capsules (5) | new: LTC | job: name | A name before its act | SPLIT: Quads work differently. / A fitted matrix warps a simple cosine lobe into the shape of the BRDF. / That method is called linearly transformed cosines.
25. c2-types, 8, "The lab has no quad" | known: warp method (7) | new: lab has no quads | job: aside | Separates the act (7) from its purpose (9) | Move it after 9: A cosine lobe has a closed-form integral over a quad, so the warped lobe does too. / The lab has no quad lights, so it draws this as an illustration.
26. c2-findings, 4, "A light can be put" | known: first finding done (3) | new: lighting channels | job: act | Topic turn with no bridge | The second finding is about lighting channels. / A light can be put on a lighting channel, so that it lights only surfaces on the same channel.
27. c2-findings, 9, "These findings come from reading" | known: findings | new: provenance, lab does not run the engine | job: provenance | Provenance appended at the end, and the beat ends mid-list, not on a landing | Move it first, and drop "The code shows" from 1: Because the lab does not run the engine itself, these three findings come from reading O3DE's source. Then end on 8.

### Chapter 3

28. c3-inputs, 3, "They are the base colour," | known: "a surface, a small set of numbers" (2) | new: six items | job: name | "They" gives the surface a list of inputs (base colour, metallic, specular factor), while the recap defines the surface as albedo, F zero, roughness forms, normal, occlusions: one word, two things; "the occlusion" is singular here, two later | NEEDS FACT (settle whether the list names the material's inputs or the surface, in the surface-setup listing); then say "the material's inputs" in 3 and "the surface" only for the evaluated numbers.
29. c3-rough, 2, "That roughness is squared into" | known: clamped roughness | new: roughness A, its use, next chapter | job: name plus purpose | Two new parts, an appended relative clause | SPLIT: That roughness is squared into a second value, called roughness A. / The microfacet formulas of the next chapter use roughness A as the width of a highlight's lobe.
30. c3-occl, 2, "Each can come from a" | known: two occlusion inputs | new: map, red channel, power of its factor | job: act | "its" has two referents (map, input), and "factor" is not yet introduced | SPLIT: Each input has a factor, and can also come from a map. / The map's red channel is raised to the power of that factor.
31. c3-decal, 5, "A decal also fades as" | known: decal effects | new: fade by angle, smoothstep gloss | job: act | Two appended phrases | SPLIT: A decal also fades as the surface turns away from its projection. / The fade follows a smoothstep of the angle, a curve that eases it in and out.
32. c3-decal, 8, "Only then does it apply" | known: the first set-up (7) | new: decals applied to a copy | job: act | Inverted opening, hard by ear; two new parts | It applies the decals only then, to a copy of the surface. / The lights read that copy.
33. c3-decal, 11, "So the sky's light, which" | known: targets from the original surface (10) | new: the sky's light ignores the decal | job: reframe | Long clause between subject and verb | A full-screen pass adds the sky's light later, from those targets. / So the sky's light ignores the decal.
34. c3-recap, 1, "So a pixel's surface comes" | known: chapter | new: list, "two occlusion factors" | job: compress | "factor" means the exponent in c3-occl and the occlusion value here | ... a normal, and two occlusions.

## (1) Drifting terms

- The sky's light: "the sky's light", "the sky's diffuse light", "image-based light", "the light of the sky" (c1-normal 12, c1-split 1, 5-7, c1-stencil). Pick one: "the sky's light", with "image-based light" glossed once.
- "loop": the walk over a bin's list and each light type's loop (c2-iterate 2, 5 versus 7); the culling "loop over lights" in c2-cull 2 is a third.
- "entries", "slots", "lights" in one list (c2-list 14-17); the "list" is the tile's first row, each bin's row, and the whole picture (c2-why, c2-list 3, 10).
- The light's size of reach: "sphere of influence" (c2-cull 4), "reach" (7), "attenuation radius" and "radius" (c2-falloff 2, 5, 7), "the sphere the culling tested".
- Depth divisions: "slabs called depth bins" (c2-why 9), "bins", and "slices" with "slice bits" (c2-mask, c2-cull 5): the same word could be heard for mask slices and bins.
- "factor": specular factor (c1-targets 28, c3), occlusion factor as the exponent (c3-occl), occlusion factor as the value (c3-recap 1).
- "surface": the evaluated numbers (c3-inputs 2) versus the inputs listed (c3-inputs 3) versus the geometry a decal covers (c3-decal), and "original surface" versus "a copy of the surface" (c3-decal 7-10).
- "occlusion": "the occlusion" (c3-inputs 3), "two occlusion inputs" (c3-occl 1), "two occlusion factors" (c3-recap 1), "ambient occlusion" (c1), "specular occlusion" (c1-targets).
- "mobile": "a mobile one" (c1-pipeline 4), "the low-end and mobile pipelines" (c1-alternatives 1), "the mobile pipeline" (c1-alt-mobile).
- "pipeline", "default pipeline", "MainRenderPipeline" are consistent; but "the frame" and "the forward pass" are both sometimes the unit of "first" (c1-map "First, depth", c2-why "The frame's first lighting step").
- "view" for the viewer's display, for a view-space region ("view space", c2-cull 3), and for each panel ("left view"): keep, but gloss "view space" once.

## (2) The ten most important fixes

1. c3-inputs 3 and c3-recap 1: settle what "the surface" is (inputs versus evaluated numbers) and what "factor" and "occlusion" name (NEEDS FACT).
2. c1-targets 19-21: move "the diffuse response" after the claim that uses it, and say the occlusion fold-in in a plain active sentence.
3. c2-list 14-17: one word for the list's positions ("slots"), so 256 slots, one marker, seven group markers, 248 lights and decals add up by ear.
4. c1-recap 4-6: give the provenance a bridge, then land on the real bridge with its "that" repaired.
5. c2-findings 4 and 9: bridge to each finding, and move the provenance to the start.
6. c1-split 7: remove "Only", which 8 contradicts.
7. c2-why 9: put the purpose of the depth bins next to them.
8. c2-types 6 and 8: act first, name second, and keep the lab aside after the purpose.
9. c2-iterate 7 and c2-falloff 1: one name for each loop, and a bridge into the falloff.
10. c3-decal 5, 8, 11 and c3-occl 2: split the stacked appended phrases and repair "its", "it", and the inverted opening.
