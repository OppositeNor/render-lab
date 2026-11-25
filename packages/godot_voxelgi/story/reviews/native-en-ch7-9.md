# Native-English review, chapters 7 to 9

Format: chapter, beat, number, first five words | what sounds wrong | suggestion.

## Chapter 7

1. ch7, c7-tag, 8, "Their settings are kept in" | "Their" has no clear antecedent after the pixel sentence | The settings of up to eight VoxelGIs in view are kept in one buffer.
2. ch7, c7-tag, 9, "This sphere's pixels name the" | "name" and "the room's only VoxelGI" are stiff | Pixels on this sphere name the room's one VoxelGI.
3. ch7, c7-space, 2, "The pass moves the pixel's" | "moves ... into the box's cells" reads as a literal rendering of a transform | The pass transforms the pixel's position and normal into the box's cell space.
4. ch7, c7-space, 7, "But with one box, the" | "is then divided" makes the clause drag | But with one box, the weighted light is divided by that same fade, so the fade cancels out.
5. ch7, c7-low, 3, "Each step advances by that" | "so it doubles the distance" has an unclear subject | Each step advances by that radius, so the distance doubles and the read moves one mip coarser, where a texel is twice as wide.
6. ch7, c7-spec, 5, "On a rough one, it" | "opens seventy degrees" is not the usual verb pattern | On a rough one, it opens out to seventy degrees from its axis.

## Chapter 8

7. ch8, c8-cache, 3, "Each frame, on the processor," | "Each frame" opener and "the copy it saved" are clumsy | Every frame, on the CPU, the culler compares each visible light near the box with its saved copy.
8. ch8, c8-cache, 6, "To measure the light passes," | "which Godot does only when something changed" has the wrong tense | To measure the light passes, the lab now relights the room every frame, which Godot would do only when something changes.
9. ch8, c8-cache, 9, "} When the lab returns" | A stray brace opens the sentence, and it would be read aloud as noise | When the lab returns to Godot's behaviour, those rows are gone, so a still room's light costs nothing per frame.
10. ch8, c8-sparse, 4, "These are the cells shown" | "shown here" after "These" is redundant | Those are the cells on screen, not the empty grid around them.
11. ch8, c8-cones, 10, "But only the diffuse cones" | "the diffuse cones for ambient light" is a heavy noun chain | But only the ambient diffuse cones change, and the reflection cone stays the same.
12. ch8, c8-half, 1, "One project setting, half resolution," | "trades quality for speed directly" is close but not idiomatic | One project setting, half resolution, trades quality straight for speed, and it is off by default.
13. ch8, c8-half, 11, "These are the artifacts on" | "that the documentation warns of" trails off with a dangling preposition | These are the polygon-edge artifacts the documentation warns about.
14. ch8, c8-half, 12, "Each artifact is one half-size" | The result clause comes last and weakens the point | Each artifact is one half-size texel wide, so they shrink at a higher resolution.
15. ch8, c8-recap, 1, "So VoxelGI pays for little" | "pays for little" suggests the wrong meaning when spoken | So VoxelGI costs little each frame.

## Chapter 9

16. ch9, c9-mix, 9, "In turn, the opaque pass" | "In turn" implies other agents taking turns | From there, the opaque pass applies ambient occlusion, the albedo, screen-space effects, and the environment's reflectance to it.
17. ch9, c9-follow, 1, "Follow this pixel on the" | "back through it all" is vague and the order is awkward | Follow this pixel on the sphere back through the whole chain.
18. ch9, c9-sources, 2, "Every source this video cites" | "the order the frame runs it" gives "source" an odd verb | Every source this video cites is in its source-reading guide, in the order the frame uses them.

## Habits seen across the chapters

- A phrase placed at the end of a sentence where a native speaker would put it first ("so at a higher resolution they shrink").
- A literal verb for a transform or an action ("moves into", "opens seventy degrees", "runs it") where English wants "transforms", "opens out to", "uses".
- Noun chains ("the diffuse cones for ambient light", "the screen pass's row") that are easy to read and hard to hear.
- A dropped or unclear subject after "so" ("so it doubles the distance", "Their settings").
- Tense slips when contrasting the lab with Godot ("does only when something changed" for "would do only when something changes").
- "Pays for", "trades directly", and similar money words used a little off.
- Many short flat sentences in a row (c7-tag, c9-follow), which the voice will read as a list. No fix suggested, since the meaning is clear.
- A transcription glitch, the stray "}" in c8-cache.
