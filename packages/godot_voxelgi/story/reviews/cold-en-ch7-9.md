# Cold listen, English chapters 7 to 9

Kinds: 1 lost reference, 2 undefined or forgotten term, 3 re-parse, 4 fact with no reason, 5 two or more new ideas, 6 lost thread.

## Chapter 7: Reading it per pixel

- c7-tag, 5, "That takes two bytes per pixel." Kind 4. Why two bytes, or why I should care about the size, is not said.
- c7-tag, 8, "Their settings are kept in..." Kinds 1, 4. "Their" is the VoxelGIs, but nothing has been said about settings; why a shared buffer matters is not said.
- c7-space, 2, "The pass moves the pixel's..." Kind 2. "Into the box's cells" is a coordinate change, but I only guess that.
- c7-space, 4, "An edge fade then weights..." Kind 2. "Edge fade" is introduced as if I knew it.
- c7-space, 7, "But with one box, the..." Kinds 3, 6. The fade is weighted and then divided away, which cancels the point made in the line before; I had to rebuild what it all amounts to.
- c7-low, 1, "At the default quality, four..." Kind 5. Count, angle and weight arrive in one sentence.
- c7-low, 2, "For a cone this wide,..." Kinds 1, 6. "Radius equals distance" does not follow from the forty-five degrees I just heard; I lost what "this wide" points at.
- c7-low, 3, "Each step advances by that..." Kinds 3, 5. Step size, doubling and the coarser mip are packed together; the mip clause attaches late.
- c7-spec, 3, "The pass squares the roughness..." Kind 4. No reason or effect is given.
- c7-scale, 4, "With the interior setting on,..." Kind 2. The "interior setting" has not been introduced.
- c7-scale, 5, "So the environment's light never..." Kinds 1, 6. "The environment's light" was never set up in the lessons I heard, and "so" implies a link I did not follow.
- c7-scale, 6, "That alpha is stored with..." Kind 1. "The screen pass's buffers" were not introduced.
- c7-quirk, 1, "Before any of this, the..." Kind 4. I could not tell why the check matters until the next line.

## Chapter 8: Keeping it fast

- c8-panel, 9, "Watch for the rows that..." Kinds 1, 4. Which rows, and why watch for them, is not said.
- c8-cache, 3, "Each frame, on the processor,..." Kind 3. The "on the processor" aside splits subject from verb.
- c8-cache, 6, "To measure the light passes,..." Kinds 3, 5. The purpose clause, the lab's change and Godot's behaviour all come in one sentence.
- c8-levels, 3, "Each run is one dispatch,..." Kinds 2, 5. "Dispatch" and "workgroup" come in together with the sixty-four.
- c8-levels, 5, "Then the mip pass climbs..." Kind 2. "Barrier" is used before the next line explains it.
- c8-levels, 8, "With the second bounce on,..." Kind 4. Thirty-two dispatches has no breakdown, so I cannot check or use it.
- c8-levels, 9, "The lab's light passes follow..." Kinds 1, 4. "The same order" and its relevance are unclear.
- c8-march, 6, "A light that is out..." Kinds 3, 6. The list jumps from marching to skipping before any march; "too dim" is unclear.
- c8-cones, 1, "Everything so far runs only..." Kind 5. "On change", "every frame" and "per pixel" all in one sentence.
- c8-cones, 10, "But only the diffuse cones..." Kinds 3, 6. The "but" depends on a doc claim two lines back, and "for ambient light" attaches oddly.
- c8-half, 7, "Without multisampling, the opaque..." Kind 2. Multisampling is not defined.
- c8-half, 12, "Each artifact is one half-size..." Kinds 3, 6. It seems to conflict with the "two pixels wide" steps just before.
- c8-recap, 3, "Each pixel takes a few..." Kind 5. Two savings in one sentence.

## Chapter 9: Into the final image

- c9-mix, 4, "Where the alpha is low,..." Kinds 2, 3. "The environment's own ambient light" is a new idea, with a trailing "which here comes from the sky".
- c9-mix, 9, "In turn, the opaque pass..." Kinds 3, 5. A list of four unrelated effects with no verbs between them.
- c9-others, 1, "Transparent surfaces cannot use..." Kind 4. Why they cannot is not said.
- c9-others, 3, "Volumetric fog reads every..." Kinds 2, 5. Fog, its grid and reading every mip are all new at once.
- c9-others, 4, "The editor's lighting view..." Kinds 2, 6. A new consumer appears with no link to the topic.
- c9-others, 5, "So at the default range..." Kind 4. I cannot tell what this changes for me.
- c9-recap, 4, "Light passes run when the..." Kind 5. Two stages in one sentence.
- c9-recap, 8, "Area lights cast no shadows." Kind 6. It is dropped into the list of trade-offs with no lead-in.

## Counts

Kind 1: 6. Kind 2: 9. Kind 3: 9. Kind 4: 9. Kind 5: 9. Kind 6: 8.

## What I understood, and what I would still ask

Chapter 7. Each pixel is tagged with up to two VoxelGIs during the depth pre-pass. It is moved into the box, and it traces four wide cones (six narrower ones at High) through the mips, plus one reflection cone whose width follows roughness. The light is then scaled back, the alpha is kept, and an always-true normal check was a Godot bug. I would ask what the edge fade is for if it cancels, what "interior" means, how the reflection cone's cost works, and what the screen pass's buffers are.

Chapter 8. Bake runs once, light passes run only on change, and the screen pass runs every frame. The light passes visit only occupied cells, one dispatch per level, with distance-field shadow steps. Per-pixel cost is a few cone samples, the mirror is the most expensive case, and half resolution traces a quarter of the pixels with ragged edges. I would ask where the thirty-two dispatches come from, what the panel rows I should watch are, and what multisampling changes.

Chapter 9. The opaque pass mixes ambient and reflection buffers with the sky by alpha, then treats the result as ordinary ambient light. Transparent surfaces, fog and the editor view read the texture another way. The pixel traces back to leaves, bounce and bake. The takeaways are a baked sparse octree in a dense texture, light on change plus cones, and the trade-off of baking, coarse averages and no area-light shadows. I would ask what the "environment" is, why transparent surfaces cannot use the buffers, and how fog reads the texture.
