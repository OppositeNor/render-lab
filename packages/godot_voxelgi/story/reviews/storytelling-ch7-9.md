# Storytelling review, chapters 7 to 9 (English)

Method: every sentence of ch7 to ch9 mapped (known / new / job) against storytelling.md, the ear rules and the listener card;
only faulty rows are listed. "L" is the line in packages/godot_voxelgi/video/storyboard.lam. Ids are chapter.beat.sentence.
Checked against ch1 to ch6 for recalls: the screen pass (ch1), the dynamic range (ch4/ch5), propagation (ch5), the culler (ch4),
alpha as coverage (ch2/ch3), the area light (ch4) were all introduced before. "interior setting" and "edge fade" were not.
Note: the "}" in c8-cache sentence 9 of en-ch8.md is an extraction artefact of the cue braces (storyboard L1783 is clean).

Tally: 62 faulty sentences (ch7: 21, ch8: 27, ch9: 14); 13 SPLITs; 6 needs-fact.

## Chapter 7

### c7-map
- **2 (L1459)** Row: known "a pixel reads that light" (1); new: cheaply + the screen pass answers + every frame. Fault: two new parts; the name "screen pass" comes as an answer before its act. Rewrite (SPLIT): "The question is how a pixel reads that light cheaply. / The screen pass, which runs every frame, answers it."

### c7-tag
- **3 (L1476)** Known: nothing (the two-VoxelGI rule from 2 gives no reason for a pass). New: depth pre-pass. Job: name with no purpose. Fault: chain broke / name before act. Rewrite: "To record them for every pixel, a pass called the depth pre-pass, which draws depth before the main shading, does the work."
- **5 (L1482)** Known: "That" (the indices; could also mean the pre-pass). New: two bytes per pixel. Job: bare fact, no consequence. Fault: pronoun with two referents; fact without why. Rewrite: "Those two indices take two bytes per pixel, one byte each." (one byte each follows from two indices in two bytes; needs fact if the format is not a byte per index).
- **8 (L1490)** Known: "Their" (VoxelGIs). New: one buffer + shared by up to eight. Job: topic turn with no bridge, two new parts. Fault: overloaded, no bridge. Rewrite (SPLIT): "Their settings are kept in one buffer. / That buffer is shared by up to eight VoxelGIs in view." Add the bridge "The pass also needs each VoxelGI's settings." before it.

### c7-space
- **2 (L1509)** Known: the ringed pixel. New: "the box's cells" as a space, plus position and normal. Job: act without purpose. Fault: act with no purpose; "the pass" drifts (screen pass). Rewrite: "The screen pass moves the pixel's position and normal into the box's cells, so it can look them up in the texture." (needs fact: confirm the purpose is the texture lookup, ch1 to ch5 imply it).
- **4 (L1517)** Known: "outside the box gets nothing". New: edge fade (unheard term). Job: name before meaning. Fault: name before act; 5 and 6 explain it. Rewrite (SPLIT with 5 and 6): "Inside the box, the light this VoxelGI gives the pixel is weighted by a fade. / The fade is one at the box's centre and falls to zero at its faces. / It is called the edge fade, and it exists to blend two boxes that overlap."
- **7 (L1527)** Known: "the fade exists to blend" (6). New: the division by the same fade, and the cancelling. Job: branch plus surprising consequence. Fault: two new parts; the division is never introduced. Rewrite (SPLIT): "But the weighted light is then divided by the fades added up. / With one box, that is this same fade, so the fade cancels out." (needs fact: that the divisor is the sum of the fades, verify in the shader).

### c7-low
- **1 (L1544)** Known: the pixel. New: four cones, forty-five degrees, weight one quarter. Fault: three new parts. Rewrite (SPLIT): "At the default quality, four cones leave the pixel. / They leave at forty-five degrees from its normal. / Each has a weight of one quarter."
- **2 (L1548)** Known: the cone. New: radius equals distance, with "radius" undefined. Fault: new word unexplained. Rewrite: "For a cone this wide, its radius at any point, how far it reaches sideways, equals the distance travelled."
- **3 (L1552)** Known: "that radius". New: doubling, coarser mip, texel twice as wide; "it" has two referents (step, cone). Fault: overloaded plus pronoun. Rewrite (SPLIT): "Each step advances by that radius. / So each step doubles the distance travelled. / The cone then reads one mip coarser, where a texel is twice as wide."
- **6 (L1564)** Known: the blended samples. New: stop at alpha one. Fault: "alpha" is used before it is defined or recalled; sentence 7.6 of this chapter defines it later. Rewrite: "The cone stops when its alpha, the coverage from chapter two, reaches one, or when it leaves the box." (recall only).

### c7-high
- **2 (L1581)** Known: six cones. New: "narrower", thirty degrees to its edge. Fault: relation implicit: the default cones' width was never given (sentence 1 of c7-low gave their tilt, not their width). Needs fact: the default cones' half-angle. Until then: "Each of those cones is thirty degrees wide from its axis to its edge, narrower than the default's."
- **3 (L1585)** Fault: "width" here, "radius" in c7-low; synonym drifts. Rewrite: "Each takes smaller steps, half its radius at a time." (needs fact: confirm width means radius here).

### c7-spec
- **3 (L1611)** Known: width follows roughness (2). New: squaring, "first" (before what?). Fault: implicit relation; constant stated without anchor. Rewrite: "The pass squares the roughness before it sets the width." Engine's choice, no reason added.

### c7-scale
- **1 (L1636)** Fault: the dynamic range is not recalled. Rewrite: "The cones' light is multiplied back by the dynamic range, the scale from chapter four, which undoes the texture's division."
- **2 (L1640)** "It" could be the light or the dynamic range. Rewrite: "The light is also multiplied by the VoxelGI's energy, a brightness setting."
- **4 (L1646)** Known: alpha. New: "interior setting", never defined anywhere before. Fault: new term unexplained. Needs fact: what the interior setting is and why it exists (source: the VoxelGI's interior flag). Do not invent.
- **5 (L1648)** Known: alpha forced to one. New: the environment's light never mixes in. Fault: consequence of a mix not yet told (chapter nine tells it). Rewrite: "Later, the opaque pass mixes the environment's light in where the alpha is low. / So with the alpha at one, none of it mixes in." (SPLIT)

### c7-quirk
- **1 to 3 (L1664 to L1670)** Known: "the pixel's normal is valid" (1). New: the test is true because length returns three. Fault: the test itself is never said, so 3 does not attach to 1. Needs fact: the actual expression (a comparison on the normal's length). Rewrite once known: "Before any of this, the pass checks that the length of the pixel's normal is above zero."

### c7-recap
- **2 (L1688)** "It" could be the pixel or the reflection cone. Fault: pronoun; "how much of its view found filled" drifts from alpha/coverage. Rewrite: "The pixel also keeps the cones' alpha, how much of their view they found filled."

### c7-sources
- **1 (L1698)** "per-pixel tag" names a thing the narration called "the first two VoxelGIs' indices". Fault: drift. Rewrite: "from the indices written per pixel to the scaled cones."

## Chapter 8

### c8-map
- **1 (L1704)** Known: buffers (ch7). New: "the bill". Fault: metaphor name drifts into "cost" (2). Rewrite: "…this chapter looks at what the screen pass and the rest cost."

### c8-panel
- **5 (L1743)** Two new parts (what the row is, that it takes most). Rewrite (SPLIT): "The row called scene pass is the room's own shading. / It takes most of the frame."
- **8 (L1749)** Fact with no consequence. Rewrite: "These times are this machine's, so other machines will differ." (follows by arithmetic from "this machine's").

### c8-cache
- **2 (L1762)** Cause told after consequence ("nothing changed" is defined in 4). Rewrite: "Nothing the light passes depend on changed, so Godot does no light-pass work at all."
- **3 (L1764)** Known: "the culler" (recalled in ch4), "the copy it saved". Fault: recall without cue. Rewrite: "Each frame, on the processor, the culler from chapter four compares every visible light near the box with the copy it saved."
- **4 (L1768)** "the data" has no referent. Rewrite: "…when a light, the baked data, or one of three settings has changed."
- **5 (L1772)** Three terms recalled with no cue (propagation, second bounce). Rewrite: "Those settings are the dynamic range, the propagation from chapter five, and the second bounce."
- **7 (L1778)** "the mips" and "the writes" drift from "the mip pass" and "write pass". Rewrite: "Four rows appear, for the direct light pass, the bounce pass, the mip pass, and the write pass."

### c8-sparse
- **1 (L1796)** Topic turn with no bridge; two things counted. Rewrite (SPLIT): "Now the cost of one update. / The readout on the right counts what one update of the light passes costs, and what one frame costs."
- **5 (L1808)** Known: "the cells the bake made". New: "leaves", "texels", "finest mip". Fault: cells/leaves/texels drift and no purpose. Rewrite (SPLIT): "That is forty-two thousand leaves, the finest cells. / The texture's finest mip has more than a million texels, so the passes visit far fewer places."

### c8-levels
- **2 (L1829)** Two new screen items. Rewrite: "So each level is one run of the array, drawn as one colour in this bar."
- **3 (L1833)** Two new parts. Rewrite (SPLIT): "Each run is one dispatch, one call to the graphics card. / Each workgroup in it takes sixty-four cells."
- **4 (L1837)** "The first light pass" has no referent among the four rows; no bridge. Rewrite: "Now the order of the passes. / The direct light pass begins with the leaves." (needs fact: confirm that the first pass is the direct light).
- **5 (L1841)** "Then the mip pass" skips the bounce pass heard in c8-cache; "barrier" named before its meaning (6). Rewrite: "Then, after the bounce, the mip pass climbs one level per dispatch, with a wait between dispatches called a barrier." (needs fact: bounce position in the order).
- **9 (L1857)** "the same order on the graphics card": Godot's passes also run on the graphics card. Rewrite: "The lab's own light passes run in that same order."

### c8-march
- **1 (L1870)** Known: nothing (which light pass, which ray). Rewrite: "Inside the direct light pass, each cell sends a shadow ray toward a light, and the ray reads the distance field." (needs fact: cell to light).
- **5 (L1886)** "the march" never named. Rewrite: "So this stepping, the march, is never slower than a walk from cell to cell."
- **6 (L1890)** Topic turn with no bridge. Rewrite: "One more saving comes before the march. / A light that is out of range…"

### c8-cones
- **1 (L1902)** Three relations in one sentence. Rewrite (SPLIT): "Everything so far runs only on change. / But the screen pass runs every frame, so its price is paid per pixel."
- **3 (L1908)** "At most seven steps" has no reason. Needs fact: why seven in this box (the number of levels the cone climbs). Do not invent.
- **4 and 5 (L1912, L1916)** "a prefiltered mip" then "That texel": drift mip/texel. Rewrite 4: "Each sample reads a texel of a prefiltered mip, as wide as the cone at that point."
- **6 (L1920)** "thin walls blur" with no why. Rewrite: "The price is that a thin wall blurs, because the averaged texel mixes what lies on both sides of it." (consequence of the narrated average).
- **8 (L1930)** Two new parts. Rewrite (SPLIT): "So their steps are shorter, and there are more of them. / The screen pass's row grows."
- **10 (L1936)** "the diffuse cones for ambient light" drifts from "the ambient and reflection cones". Rewrite: "But only the cones for ambient light change. / The reflection cone stays the same." (SPLIT)

### c8-spec
- **2 (L1958)** Two numbers. Rewrite (SPLIT): "A fully rough surface takes at most two steps. / A mirror takes at most three hundred eighty-one."
- **5 (L1968)** "smooth" vs "mirror" drift. Rewrite: "So the smoothest surfaces, the mirrors, are the most expensive pixels."

### c8-half
- **1 (L1991)** Rewrite (SPLIT): "One project setting, half resolution, trades quality for speed. / It is off by default."
- **3 (L1995)** "ambient and reflection buffers" are named here but defined in c9-map. Rewrite: "The screen pass's two buffers, one for ambient light and one for reflection, become half as wide and half as tall."
- **5 (L2001)** Two new parts. Rewrite (SPLIT): "So only a quarter of the pixels are traced. / The screen pass's row shrinks with them."
- **6 (L2003)** A buffer view shown without a reading (storyboard.md, "Reading a texture or buffer view"): what the colours mean, the axes, where to look. Needs fact: how the lab draws the buffer (colour = light, alpha shown?). Add sentences before "So along the sphere's edge…".
- **7 (L2005)** Overloaded condition + method. Rewrite (SPLIT): "Without multisampling, the opaque pass reads the half-size buffer. / It reads it with a plain bilinear blend."
- **11 (L2015)** Provenance appended. Rewrite: "The documentation warns of these artifacts on polygon edges."
- **12 (L2017)** "they" vs "Each artifact"; "higher resolution" ambiguous. Rewrite: "Each artifact is one half-size texel wide, so at a higher screen resolution the artifacts shrink."

### c8-recap
- **3 (L2033)** Two new parts; "it" is the setting. Rewrite (SPLIT): "Each pixel takes a few cone samples. / And with half resolution on, only a quarter of the pixels are traced."

### c8-sources
- **1 (L2047)** "the gate" never named in the narration (the change check), "it" unclear. Rewrite: "These are the lines that keep the cost small, from the check that skips unchanged light to the half-size buffers."

## Chapter 9

### c9-mix
- **2 (L2066)** Recall of alpha as coverage. Rewrite: "Its alpha, the coverage, holds how much of the view the cones found filled."
- **4 (L2074)** "the environment's own ambient light ... the sky": environment/sky drift. Rewrite: "…mixes toward the sky's own ambient light, the environment's light, which here comes from the sky." Better: choose "the sky" and say "the sky, the environment" once.
- **5 (L2078)** "the sky's reflection" (sky vs environment). One word: sky.
- **7 (L2086)** Repeats c7-scale 4 to 5; interior setting still undefined. Needs fact (see c7-scale 4).
- **9 (L2094)** Four new parts, no purpose. Rewrite (SPLIT): "The opaque pass applies ambient occlusion and the albedo to it. / Then it applies screen-space effects and the environment's reflectance."

### c9-others
- **1 (L2105)** Two new parts; "cannot" lacks why. Rewrite (SPLIT): "Transparent surfaces cannot use the buffers. / There are none in this room." Needs fact: why they cannot.
- **2 (L2109)** Three new parts. Rewrite (SPLIT): "Instead, each transparent surface traces its own four cones. / It does so in the forward pass that draws it, at every quality setting."
- **4 (L2119)** "it" is the view or the texture. Rewrite: "The editor's lighting view also reads the texture, but the view never applies the dynamic range."

### c9-follow
- **2 (L2141)** "around it" ambiguous. Rewrite: "Its cones read the texture's mips around the pixel."
- **4 (L2147)** "the panel's direct light" collides with "the panel" of the performance panel in ch8. Rewrite: "Those leaves hold the ceiling panel's direct light, plus one bounce."

### c9-recap
- **4 (L2181)** "and then" makes a per-frame step follow the on-change passes. Rewrite (SPLIT): "Light passes run when the lights change. / Every frame, every pixel then traces cones that read wider mips as they go."
- **6 (L2187)** Two facts. Rewrite (SPLIT): "The static scene must be baked. / Moving objects are only drawn in, frame by frame."
- **7 (L2191)** Rewrite (SPLIT): "The cells are five centimetres wide. / The cones see the room only as coarse averages."
- **8 (L2195)** "Area lights cast no shadows" with no bridge and no why (ch4 gives it). Rewrite: "Area lights, like the panel, cast no shadows."

### c9-sources
- **2 (L2203)** "it" is the guide, the frame or the source. Rewrite: "Every source this video cites is in its source-reading guide, in the order the frame runs the sources."

## Drifting terms
- screen pass / the pass / voxelgiGiCS / screen pass's row.
- default quality / Low / four wide cones; width / radius of a cone; diffuse cones / ambient cones / cones for ambient light.
- environment / sky (ambient and reflection); ambient buffer / buffers / the buffers.
- the mips / mip pass; the writes / write pass; the first light pass / direct light.
- cells / leaves / texels; mip / texel; alpha / coverage / "how much of its view found filled".
- the panel (timing panel, ceiling light panel); mirror / smooth; artifacts on polygon edges / the sphere's edge; indices / tag.
- bounce / second bounce / one bounce.

## Ten most important fixes
1. c7-scale 4 and c9-mix 7: "interior setting" is never defined (needs fact from the source).
2. c7-space 4 to 6: define the edge fade by its act before its name.
3. c7-quirk 1 to 3: say what the normal test is, so the length claim attaches.
4. c8-half 6: the ambient buffers at full and half size are shown with no reading of the view.
5. c8-half 3 and c9-map: name the two buffers where they are first used.
6. c8-levels 4, 5, 9: "first light pass", the missing bounce pass, and the unclear "on the graphics card".
7. c7-low 1 and 3: split the three-number and three-effect sentences.
8. c7-high 2 and 3: the default cones' width is never given; width/radius drift.
9. c9-recap 4: "and then" turns a per-frame step into a sequel of the on-change passes.
10. c8-cache 7 and c8-sparse 5: pass-name drift and the cells/leaves/texels comparison with no stated point.
