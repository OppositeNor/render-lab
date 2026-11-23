# Native-English review, chapters 7 to 11

## Chapter 7

1. "With the field built," - the opening is fine but "shine on" is a little poetic next to the dry rest. Suggestion: With the field built, the scene's lights now light the solid cells.
2. "With the distance field and" - repeats the opening beat almost word for word. Suggestion: Now that the distance field and the visibility are built, we can light the solid cells.
3. "A compute shader runs one" - "for each solid cell in the list that chapter five built" is a heavy tail. Suggestion: A compute shader runs one thread per solid cell, from the list built in chapter five.
4. "By default, a frame relights" - fine meaning, but "relights" is slightly off. Suggestion: By default, each frame updates the light of only every fourth cell in that list.
5. "So a moving light takes" - "reach a cell" is loose. Suggestion: So a moving light can take up to four frames to show up in a cell.
6. "Each cell here is" - "coloured by the light it stores" reads stiffly. Suggestion: Here, each cell is coloured with the light it stores.
7. "An extra omni light now" - "lights the room" does not say it is added. Suggestion: We add an omni light to the room.
8. "We follow the light it" - awkward. Suggestion: Let's follow the light it gives to one floor cell.
9. "To find how the light" - "find how" is not idiomatic. Suggestion: To work out how the light fades, the code divides the cell's distance by the light's range.
10. "Then it raises that ratio" - a long chain of verbs that trips when spoken. Suggestion: It raises that ratio to the fourth power, subtracts the result from one, and squares that.
11. "That result is the falloff," - "so the light fades out there" is redundant. Suggestion: That is the falloff, and it reaches zero at the range, where the light fades out.
12. "That falloff is then divided" - the "That ... That ... That" run is monotonous. Suggestion: The falloff is then divided by the distance raised to the light's attenuation setting, which is one by default.
13. "So by default, the light" - "falls off with the distance itself" is odd. Suggestion: So by default, the light falls off linearly with distance, not with its square.
14. "But the distance in these" - "these steps" is vague. Suggestion: But the distance used here is not the true distance.
15. "The code shows that every" - "The code shows that" is a repeated crutch, and "one and a half" after a comma is hard to hear. Suggestion: In the code, every height is multiplied by the y scale from chapter two, which is one and a half, before the distance is measured.
16. "So the light behaves as" - "hung" is fine, but "than it really does" is clumsy. Suggestion: So the light behaves as if it hung one and a half times higher than it really is.
17. "The readout shows that the" - "gives the same light as these steps" is stiff. Suggestion: The readout confirms that the lab's model matches these steps.
18. "To find out whether a" - fine start, but "steps toward" is thin. Suggestion: To find out whether a wall blocks the light, a shadow ray marches from the cell toward the light.
19. "That is the longest step" - "cannot jump over a surface" is okay, but "That is" is vague. Suggestion: That is the longest step that is guaranteed not to jump over a surface.
20. "The shadow is all or" - redundant. Suggestion: The shadow is all or nothing, so a cell is either fully lit or fully dark.
21. "Its light is summed separately" - "one for each axis direction" is clumsy. Suggestion: Its light is summed separately for each of its six faces, one per axis direction.
22. "It also stores a weight" - "how much of that colour the face gets" is loose. Suggestion: It also stores a weight per face, giving each face's share of that colour.
23. "The cell's light is also" - far too long to speak in one breath. Suggestion: The cell's light is also copied into the empty neighbours flagged in chapter five, the ones whose nearest surface is this cell.
24. "So a filtered read next" - "fading toward black" is fine, but "blends in" is off. Suggestion: So a filtered read next to a wall blends in the wall's light instead of fading to black.
25. "A static light is written" - "when its region is built" is fine. Suggestion: A static light is written into the cells it reaches once, when its region is built.
26. "The static light is added" - repeats the term and "on top of whatever" is unclear. Suggestion: It is added on top of any light the cell already stores.
27. "When a cascade jumps, the" - "carry their stored light into" is fine but heavy. Suggestion: When a cascade jumps, the cells it keeps carry their stored light over into the new cell list.
28. "The code shows that whenever" - second use of the crutch. Suggestion: In the code, whenever a static light overlaps the new slab, it is added again.
29. "It is added to the" - Suggestion: It is added to the kept cells as well, not only to the slab's.
30. "In the lab's model, a" - "ends up holding" and the count is clumsy. Suggestion: In the lab's model, a kept cell ends up with that static light twice, then three times, then four.
31. "The light pass also lets" - "lets light bounce" is odd. Suggestion: The light pass also allows more than one bounce.
32. "The eight probes around each" - "from these same cells a frame earlier" is okay. Suggestion: The eight probes around each cell already hold light, gathered from these same cells one frame earlier.
33. "The cell multiplies that probe" - two sentences that want to be one. Suggestion: The cell multiplies that probe light by its own colour and by a setting called the bounce feedback, then adds the result to its light.
34. "The bounce feedback scales each" - Suggestion: The bounce feedback sets the strength of each bounce.
35. "So the light makes a" - Suggestion: So the light loops from the cells to the probes and back, gaining one more bounce each frame.
36. "In the readout, a cell" - Suggestion: In the readout, a cell on the red wall gets brighter as the feedback grows.
37. "So each solid cell now" - Suggestion: So each solid cell now stores, face by face, the light that reaches it.
38. "That light is what the" - Suggestion: That light is the scene's light on the cell, plus last frame's bounce from the probes.

## Chapter 8

1. "Now the probes gather the" - Suggestion: Now the probes gather the light stored in the cells by tracing rays.
2. "The cells now hold light," - Suggestion: The cells now hold light, but only at the surfaces.
3. "The probes gather that light," - "so that the points ... get light too" is stiff. Suggestion: The probes gather that light, so the points between the surfaces get lit as well.
4. "The rays' directions follow a" - Suggestion: The ray directions follow a spiral over a sphere of directions.
5. "Over thirty frames, the rays" - Suggestion: Over thirty frames, the rays cover two hundred and forty directions.
6. "The code numbers each ray." - blunt and ambiguous. Suggestion: The code gives each ray a number.
7. "Whether a ray's number is" - Suggestion: Whether a ray's number is odd or even decides which half of the sphere it points into.
8. "One frame's ray numbers are" - "thirty apart" is clipped. Suggestion: Within one frame, the ray numbers are thirty apart.
9. "So each frame's eight rays" - Suggestion: So all eight rays of a frame point into the same half of the sphere.
10. "They are shown here in" - Suggestion: Here, the two halves are shown in two colours.
11. "A probe's ray marches the" - Suggestion: A probe's ray marches just like the shadow ray, in steps as long as the stored distance.
12. "The ray stops when it" - "within a twentieth of a cell of a surface" has stacked "of"s. Suggestion: The ray stops when it comes within a twentieth of a cell from a surface.
13. "That stop is the hit." - Suggestion: That counts as a hit.
14. "When the ray leaves the" - Suggestion: When the ray leaves the first cascade, it carries on into the next one, which is coarser and larger.
15. "At a hit, the ray" - Suggestion: At a hit, the ray needs the surface normal.
16. "So the field's gradient, the" - Suggestion: So the field's gradient, the direction in which it grows fastest, gives us that normal.
17. "That normal decides how much" - Suggestion: That normal decides how much of each face's light the ray picks up.
18. "The ray brings that light" - Suggestion: The ray carries that light back to the probe.
19. "That light already includes the" - "that the last chapter added" is stiff. Suggestion: That light already includes the bounce from last frame's probes, which the last chapter added.
20. "A ray that leaves every" - Suggestion: A ray that leaves every cascade reads the sky instead.
21. "It reads a blurred copy" - Suggestion: It reads a blurred copy of the sky, mip level two.
22. "A comment in the code" - "that a probe traces few rays" is a stiff appositive. Suggestion: A comment in the code explains the blur by saying a probe traces few rays.
23. "With the blur, those few" - Suggestion: With the blur, those few rays still add up to a smooth colour.
24. "But reading the sky hides" - "hides a mismatch" is unidiomatic. Suggestion: But reading the sky exposes a mismatch in the code.
25. "The code shows that in" - Suggestion: In four point seven stable, the probe shader is built for a single sky texture.
26. "By default on desktop, though," - Suggestion: But on desktop, the sky is stored by default as an array of textures.
27. "That is a different texture" - Suggestion: That is a different texture type from the one the shader declares.
28. "So the code does not" - Suggestion: So the code leaves undefined what a ray that leaves every cascade reads.
29. "A probe stores its light" - long and packed. Suggestion: A probe stores its light as spherical harmonics, a compact way to describe light from every direction in a few numbers.
30. "The coefficients are then scaled" - Suggestion: The coefficients are then scaled by one thousand twenty-four and stored as integers, which is called fixed point.
31. "Integers add and subtract exactly." - fine, leave.
32. "The next step relies on" - Suggestion: The next step relies on that, because it adds and subtracts whole frames of rays.
33. "The probe keeps its last" - Suggestion: The probe keeps its last thirty frames in a ring, where each new frame replaces the oldest.
34. "So when a light turns" - Suggestion: So when a light turns on, the probe gets a thirtieth brighter each frame.
35. "The chart on screen shows" - Suggestion: The chart shows that brightening as a straight line.
36. "The line levels off after" - Suggestion: The line levels off after thirty frames.
37. "So a light that moves" - Suggestion: So a moving light takes thirty frames to show up fully.
38. "So each probe now keeps" - Suggestion: So each probe now keeps sixteen coefficients describing the light that arrives from every direction.
39. "They are the exact sum" - Suggestion: They are the exact sum of the rays it traced over its last thirty frames.
40. "A pixel cannot sample coefficients" - Suggestion: A pixel cannot sample coefficients the way it samples a texture, so the next chapter converts them into small textures.

## Chapter 9

1. "Next, each probe's coefficients become" - Suggestion: Next, each probe's coefficients are turned into small textures, called maps.
2. "The sixteen coefficients from the" - Suggestion: The sixteen coefficients from the last chapter are not something a pixel can sample.
3. "So this step turns each" - Suggestion: So this step turns each probe's coefficients into small maps that a pixel can sample.
4. "Each probe gets two maps" - Suggestion: Each probe gets two maps, six by six texels each.
5. "Every texel of the map" - Suggestion: Every texel in the map then stands for one direction.
6. "One map holds irradiance, the" - Suggestion: One map holds irradiance, the light that a surface facing a texel's direction receives.
7. "A diffuse surface, one with" - Suggestion: A diffuse surface, one with no mirror-like shine, reads the irradiance map.
8. "Each map has a border" - Suggestion: Each map has a one-texel border, drawn darker here.
9. "The border is there because" - Suggestion: The border is there because bilinear filtering, which blends the nearest texels, reads one texel past the edge near the border.
10. "So the border holds copies" - Suggestion: So the border holds copies of the edge texels.
11. "Each copy is mirrored across" - Suggestion: Each copy is mirrored across the fold, so it continues the map the way the sphere does.
12. "That way, filtering at the" - Suggestion: That way, filtering at the edge reads the directions that really lie beyond it.
13. "The maps also need the" - Suggestion: The maps also have to come out at the right brightness.
14. "Four pi is the solid" - Suggestion: Four pi is the solid angle of the whole sphere, all directions together.
15. "So the sum of the" - Suggestion: So the sum of the rays should be multiplied by four pi over the ray count, which gives the light over the whole sphere.
16. "The code instead multiplies by" - Suggestion: Instead, the code multiplies by four over the ray count when it adds up the rays.
17. "Then it multiplies by four" - Suggestion: Then it multiplies by four again when it stores the maps.
18. "Four times four is sixteen," - Suggestion: Four times four is sixteen, where four pi was needed.
19. "So the stored light is" - Suggestion: So the stored light is too large by a factor of sixteen over four pi, which is four over pi.
20. "So a uniform sky, the" - Suggestion: So a uniform sky, the same colour in every direction, comes out about twenty-seven percent too bright.
21. "The readout shows the same" - Suggestion: The readout shows the same extra brightness in the lab's model.
22. "The maps have a second" - Suggestion: The maps have a second mismatch, in their directions.
23. "The code computes each texel's" - Suggestion: The code computes each texel's direction from the texel's corner.
24. "But a lookup treats a" - Suggestion: But a lookup treats a texel's value as sitting at its centre, half a texel away.
25. "So every lookup lands half" - Suggestion: So every lookup lands half a texel off.
26. "It returns a blend of" - Suggestion: It returns a blend of two neighbouring texels.
27. "In effect, the whole map" - Suggestion: In effect, the whole map is shifted by half a texel, so each direction reads light meant for a neighbouring one.
28. "The probes' histories, their thirty" - Suggestion: The probes' histories, their thirty frames of light, also follow a cascade's jump.
29. "When a cascade jumps, each" - Suggestion: When a cascade jumps, each probe's history moves with it to the probe's new place in the grid.
30. "A probe that the jump" - Suggestion: A probe that the jump brings in at the edge has no history yet.
31. "It starts from the light" - Suggestion: It starts from the light of the surrounding probes in the coarser cascade, not from black.
32. "So each probe now has" - Suggestion: So each probe now has two small maps, irradiance for diffuse light and radiance for reflections.
33. "In the next chapter, the" - Suggestion: In the next chapter, the screen pass reads them for every pixel.

## Chapter 10

1. "Last, a compute pass" - Suggestion: Last, a compute pass over the screen reads the probes for every pixel.
2. "Every probe now has its" - Suggestion: Every probe now has its maps, and the screen pass from chapter one turns them into light for each pixel.
3. "From each pixel's depth, it" - Suggestion: From each pixel's depth, it rebuilds the pixel's position.
4. "The pass also reads each" - Suggestion: The pass also reads each pixel's normal, like the one at this point on the red wall.
5. "The position and the normal" - Suggestion: The position and the normal are where the probe lookup starts.
6. "A pixel reads from the" - Suggestion: A pixel reads from the finest cascade whose grid contains it.
7. "Near that grid's edge, the" - Suggestion: Near the grid's edge, the pixel blends toward the next cascade, so there is no hard seam.
8. "The blend begins five and" - Suggestion: The blend begins five and a half probe spacings from the camera and is complete at seven and a half.
9. "The readout follows a test" - Suggestion: The readout follows a test point as it moves out through the blend.
10. "Before reading the probes, the" - Suggestion: Before reading the probes, the pass pushes the pixel's position out along the normal, away from the wall.
11. "The distance is one point" - Suggestion: It moves by one point one cells, which is the engine's choice.
12. "The moved position lies inside" - Suggestion: The moved position lies inside a probe cube.
13. "Then the pass weights the" - Suggestion: Then the pass weights the eight probes at the corners of that cube.
14. "A probe's weight grows as" - Suggestion: A probe's weight grows as the probe gets closer to the position.
15. "It also grows as the" - Suggestion: It also grows the farther the probe sits in front of the surface.
16. "So a probe behind the" - "about two hundred times less" is not idiomatic. Suggestion: So a probe behind the wall counts for almost nothing, with a weight about two hundred times smaller.
17. "As the position slides along" - Suggestion: As the position slides along the wall, the weight shifts from probe to probe.
18. "So each pixel's light is" - Suggestion: So each pixel's light is a mix of its eight probes.
19. "The probes light reflections too." - Suggestion: The probes light reflections as well.
20. "The pass looks up the" - Suggestion: The pass looks up the radiance map along the mirror direction, the direction a mirror would send the view ray.
21. "The rougher the surface, the" - Suggestion: The rougher the surface, the more the lookup blends toward the irradiance map.
22. "Below a roughness of a" - Suggestion: Below a roughness of a fifth, Godot also marches a reflection ray through the field.
23. "The lab's room is rougher" - Suggestion: The lab's room is rougher than that, so the march never runs here.
24. "Without SDFGI, the opaque pass" - Suggestion: Without SDFGI, the opaque pass fills the shadows with ambient light from the sky.
25. "With SDFGI, it takes the" - Suggestion: With SDFGI, it takes the screen pass's ambient light instead.
26. "The opaque pass multiplies the" - Suggestion: The opaque pass multiplies that ambient light by the albedo, the surface's own colour, and then adds the reflection.
27. "The lab's port leaves the" - Suggestion: The lab's port leaves out the sky's ambient light.
28. "So without SDFGI, only the" - Suggestion: So without SDFGI, only the panel's direct light reaches the room.
29. "With SDFGI, the bounced light" - Suggestion: With SDFGI, the bounced light returns.
30. "Transparent surfaces skip the screen" - Suggestion: Transparent surfaces skip the screen pass and repeat the probe lookup in their own shader.
31. "They never march a reflection" - Suggestion: They never march a reflection ray, even when they are smoother than a roughness of a fifth.
32. "And SDFGI's energy setting, which" - Suggestion: And SDFGI's energy setting, which scales its light, never applies to them.
33. "Volumetric fog, fog that fills" - Suggestion: Volumetric fog, fog that fills the air of the scene, also reads the probes.
34. "It reads their average light," - Suggestion: It reads their average light, one colour per probe.
35. "The code shows that the" - Suggestion: In the code, the fog never applies the y scale, the factor of one and a half from chapter two.
36. "So the fog samples the" - Suggestion: So the fog samples the probes at the wrong height.
37. "So every pixel's ambient light" - Suggestion: So every pixel's ambient light now comes from the maps of the eight probes around it.
38. "Those probes belong to the" - Suggestion: Those probes belong to the finest cascade that holds the pixel, the one with the closest probe spacing.
39. "The last chapter puts the" - Suggestion: The last chapter puts the whole frame together.

## Chapter 11

1. "The cascades follow the camera." - Suggestion: The cascades follow the camera, and each jump leaves a dirty slab to rebuild.
2. "The slab is voxelized." - Suggestion: The slab is voxelized.
3. "Then the occlusion step" - Suggestion: Then the occlusion step measures how well each probe sees the cells around it.
4. "Then the solid cells are" - Suggestion: Then the solid cells are lit, a quarter of them each frame.
5. "Each cell also adds the" - Suggestion: Each cell also adds the light of last frame's probes, and that creates the bounce.
6. "Each probe traces eight rays" - Suggestion: Each probe traces eight rays a frame and sums its last thirty frames.
7. "Every pixel finds the finest" - Suggestion: Every pixel finds the finest cascade that holds it and weights the eight probes around it.
8. "That light from the probes" - Suggestion: The light from those probes replaces the sky's ambient light.
9. "Three things are worth keeping" - Suggestion: Three things are worth taking away from this frame.
10. "First, SDFGI keeps the scene" - Suggestion: First, SDFGI keeps the scene around the camera as data.
11. "Third, the price is that" - Suggestion: Third, the price is that its light is only as fine as its cells, which are twenty centimetres and larger.
12. "It sees only meshes whose" - Suggestion: It sees only meshes whose GI mode is Static.
13. "And a change takes about" - Suggestion: And a change takes about thirty frames to show up fully.
14. "That is SDFGI, the way" - Suggestion: That is SDFGI, the way Godot's source builds it.

## Habits seen

- "The code shows that ..." opens many facts (chapters 7, 8, 10). Say "In the code, ..." or just state the fact.
- "So ..." starts a large share of sentences in a row. Vary with "That means" or fold into the previous sentence.
- Stacked prepositions and appositives ("a twentieth of a cell of a surface", "the list that chapter five built") make long sentences hard to speak. Split them or use a relative clause.
- "That ... That ... That" openings (chapter 7 falloff) repeat the demonstrative. Merge into one flowing sentence.
- Loose verbs for technical actions ("lights", "hides", "lets", "finds", "gets") where "updates", "exposes", "allows", "works out" read natural.
- Comparatives like "two hundred times less" and "behaves as it really does" are translated-sounding. Prefer "smaller" or "than it really is".
- Repeated filler openers, "also" and "then", add little when sentences are in sequence.
