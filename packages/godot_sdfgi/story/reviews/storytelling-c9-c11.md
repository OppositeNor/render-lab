# Storytelling review: chapters c9, c10, c11 (language only)

Map rows are `known | new | job`. Only faulty sentences are listed. "SPLIT" marks a rewrite that splits a sentence
(its sentences are separated by " / "). "needs fact" marks a fix that properly needs a reason the narration lacks;
a restructured version without the reason follows it.

## c9 From coefficients to maps

### c9-oct

- **4 (L1494)** "Each is an octahedral map, which folds the sphere of directions flat into a square."
  Row: two maps, just said | the name "octahedral map" plus the fold | name before act.
  Fault: label before meaning (show, then name).
  Rewrite (SPLIT): Each map folds the sphere of directions flat into a square. / That layout is called an octahedral map.
- **8 (L1507)** "A diffuse surface reads the irradiance map, and a reflection reads the radiance map."
  Row: the two maps | two uses, and "diffuse" is undefined | two jobs in one sentence.
  Fault: two new parts.
  Rewrite (SPLIT): A diffuse surface, one with no mirror-like shine, reads the irradiance map. / A reflection reads the radiance map.
- **10 (L1512)** "The border is there because bilinear filtering near a map's edge reads one texel beyond the edge."
  Row: the border | bilinear filtering (undefined), reading beyond the edge | purpose.
  Fault: a term not glossed where first said.
  Rewrite: The border is there because bilinear filtering, which blends the nearest texels, reads one texel beyond a map's edge when it samples near the edge.
- **11 (L1514)** "So the border holds copies of the edge texels, mirrored the way the fold continues them."
  Row: the border | copies, mirrored | act.
  Fault: "them" has two referents (the edge texels, the copies), and "the way the fold continues" is a compressed process.
  Rewrite: So the border holds copies of the edge texels. / Each copy is mirrored across the fold, so that it continues the map the way the sphere does.

### c9-gain

- **1 (L1530)** "Four pi is the solid angle of the whole sphere, all directions together."
  Row: nothing: the chapter just left the border, and no problem is stated | four pi, solid angle | definition first.
  Fault: the chain broke, and the new part opens the beat with no bridge or question.
  Rewrite (SPLIT): The maps also need the right brightness. / When the rays are summed, the sum must cover the whole sphere of directions. / The size of that sphere is four pi, its solid angle, all directions together.
- **5 (L1550)** "Four times four is sixteen instead of four pi, so the result is too large by a factor of four over pi."
  Row: the two multiplications | sixteen against four pi, and the ratio | act plus consequence.
  Fault: two new parts, and "the result" could mean the sum or the maps.
  Rewrite (SPLIT): Together the two factors are four times four, which is sixteen, where four pi was needed. / So the stored light is too large by sixteen over four pi, which is four over pi.
- **6 (L1554)** "So a uniform sky comes out about twenty-seven percent too bright."
  Row: four over pi | "uniform sky" (never introduced in the narration) and a percentage | check/consequence.
  Fault: a new noun arrives in the middle of a consequence, and the percentage is a second new number.
  Rewrite: Take a uniform sky, one of a single constant colour. / Its light comes out four over pi times too large, about twenty-seven percent too bright.
  (Sentence 7 then starts from the sky: "The readout shows the same extra brightness in the lab's model", fine.)

### c9-texel

- **1 (L1562)** "The code computes each texel's direction from the texel's corner."
  Row: nothing: the topic moves from brightness to direction with no bridge | corner | act.
  Fault: the chain broke, and the consequence is held back until sentence 2.
  Rewrite: The maps have a second mismatch, in their directions. / The code computes each texel's direction from the texel's corner.
- **3 (L1572)** "So every lookup lands half a texel off and returns a blend of two neighbouring texels."
  Row: corner against centre | half a texel off, and a blend of two texels | consequence.
  Fault: two new parts. Sentence 4 then says the same thing again with a new noun ("the whole map is shifted").
  Rewrite (SPLIT, with 4 folded in): So every lookup lands half a texel off. / It returns a blend of two neighbouring texels. / In effect, the whole map is shifted by half a texel, so each direction reads light meant for a nearby one.
- **4 (L1576)** see 3: it restates 3 in other words (lookup off / map shifted). Keep only as the landing, as written in the rewrite above.

### c9-scroll

- **1 (L1587)** "The probes' histories also follow a cascade's jump."
  Row: "histories" (c8 ring), "jump" (c2), with no recall | the histories move too | bridge.
  Fault: a term from an earlier chapter used without recalling it.
  Rewrite: In chapter two, a cascade jumps to follow the camera. / The probes' histories follow that jump too.
- **2 (L1591)** "When a cascade jumps, they move with it, to their new place in the grid."
  Row: jump | "they", "it" | act.
  Fault: "they" could be the probes or the histories, "it" the cascade or the jump.
  Rewrite: When a cascade jumps, each probe's history stays with its probe, and the probe moves to its new place in the grid.

## c10 Shading a pixel

### c10-pass

- **2 (L1624)** "After the depth pre-pass, the screen pass, a compute pass over the screen, rebuilds each pixel's position from its depth."
  Row: the screen pass, already named in sentence 1 and in the map beat | when it runs, and the position rebuilt from depth | act.
  Fault: two new parts, an opening phrase, and an appositive that repeats what the last two sentences said.
  Rewrite (SPLIT): The screen pass runs after the depth pre-pass. / From each pixel's depth, it rebuilds the pixel's position.
- **3 (L1628)** "The pass also reads each pixel's normal, as at this point on the red wall."
  Row: the pixel's position | the normal | act with no purpose; the beat ends mid-act.
  Fault: no landing. The next beat uses the position and the normal, but nothing says so.
  Rewrite (SPLIT): The pass also reads each pixel's normal, as at this point on the red wall. / The position and the normal are what the probe lookup starts from.

### c10-cascade

- **3 (L1654)** "The blend begins five and a half probe spacings from the camera, and it is complete at seven and a half."
  Row: "near that grid's edge" (sentence 2) | a measure from the camera, two numbers | act.
  Fault: two new parts, and the measure silently changes from "grid's edge" to "from the camera".
  Rewrite (SPLIT): The blend is measured as a distance from the camera, in probe spacings. / It begins at five and a half, and it is complete at seven and a half.
- **4 (L1658)** "The readout follows a test point as it moves out through that band."
  Row: the blend range | "band" (a new noun for the blend range), "readout" | check.
  Fault: a synonym drifts (blend, band), and the screen is not pointed at first.
  Rewrite: On screen, the readout follows a test point as it moves out through the blend, from five and a half to seven and a half.

### c10-probes

- **1 (L1671) and 2 (L1678)** "Before reading the probes, the pass pushes the pixel's point off the wall." / "It pushes the point along the normal, by one point one cells."
  Row 1: the screen pass, the wall | pushes the point | act with no purpose.
  Row 2: the same push | direction and amount | act, the same act again.
  Fault: two sentences for one act (the second is the same verb), no purpose for several sentences, and "point" against "position" (c10-pass) against "pixel".
  needs fact: why the engine pushes the point. Restructured without it (SPLIT): Before reading the probes, the pass moves the pixel's position along the normal, away from the wall. / The distance is one point one cells, the engine's choice.
- **3 (L1680)** "Then it weights the eight probes at the corners of the probe cube around the point."
  Row: the moved point | eight probes, a cube, weights | act.
  Fault: three new parts (the eight, the cube, the weighting).
  Rewrite (SPLIT): The moved point lies inside a cube of probes, one at each of its eight corners. / The pass weights those eight probes.
- **4 (L1685)** "A probe's weight grows as the probe gets closer to the point, and as the probe sits farther in front of the surface."
  Row: the weights | two rules | act.
  Fault: two new parts (distance, facing); the beat's two causes are one sentence.
  Rewrite (SPLIT): A probe's weight grows as the probe gets closer to the point. / It also grows as the probe sits farther in front of the surface.
- **5 (L1687)** "So a probe behind the wall counts for almost nothing, about two hundred times less."
  Row: the facing rule | a number | consequence.
  Fault: "two hundred times less" than what is not said.
  needs fact: the comparison base. Restructured without it: So a probe behind the wall counts for almost nothing, much less than one in front of it. (If the source gives the ratio against a probe straight in front, say "about two hundred times less than a probe in front".)
- **6 (L1690)** "As the point slides along the wall, the weights move between the probes."
  Row: the weights | movement | check.
  Fault: the passage ends mid-act, with no landing and no compression.
  Rewrite (SPLIT): As the point slides along the wall, the weights move between the probes. / So each pixel's light is a mix of its eight probes, and the mix changes smoothly from pixel to pixel.
  (The second sentence follows from weights that vary continuously with position; check it against the shader before keeping "smoothly".)

### c10-specular

- **1 (L1703)** "The probes light reflections too, by looking up the radiance map along the mirror direction."
  Row: the diffuse lookup | reflections, "mirror direction" | point plus act.
  Fault: a relation left implicit (the beat moves from diffuse to reflected light with no bridge), and "mirror direction" is unglossed.
  Rewrite (SPLIT): That was the diffuse light. / The probes light reflections too. / The pass looks up the radiance map along the mirror direction, the direction a mirror would send the view ray.

### c10-compose

- **1 (L1723)** "Without SDFGI, the opaque pass fills the shadows with ambient light from the sky."
  Row: nothing: the previous beat ends on roughness | the opaque pass, ambient light from the sky | point.
  Fault: the chain broke, since the passage moves from lookups to where the result goes.
  Rewrite: Now where this light goes. / Without SDFGI, the opaque pass fills the shadows with ambient light from the sky.
- **3 (L1731)** "The opaque pass multiplies that ambient light by the albedo, and adds the reflection."
  Row: the screen pass's ambient light | albedo (c4 gave it: the surface's colour), "the reflection" | act.
  Fault: two acts, and "that ambient light" could be the sky's or the screen pass's.
  Rewrite (SPLIT): The opaque pass multiplies the screen pass's ambient light by the albedo, the surface's own colour. / Then it adds the reflection.
- **4 (L1736)** "The lab's port leaves the sky's ambient light out, so without SDFGI only the panel's direct light reaches the room, and the shadows are black."
  Row: the sky's ambient light | the lab's port, the consequence for the room, black shadows | consequence.
  Fault: three new parts. "Port" is new here.
  Rewrite (SPLIT): The lab's port leaves the sky's ambient light out. / So without SDFGI, only the panel's direct light reaches the room. / The shadows are black.

### c10-others

- **2 (L1750)** "They never march a reflection ray."
  Row: transparent surfaces | no reflection march | act, a fact with neither why nor consequence.
  Fault: "they never" carries no change for the listener, and the march was named only for roughness below a fifth.
  needs fact: what a transparent surface reads instead. Restructured without it: They never march a reflection ray, the ray the screen pass marches for rough surfaces below a fifth.
  (This also corrects the implied scope: the march exists only below roughness one fifth.)
- **3 (L1752)** "And SDFGI's energy setting, which scales its light, never reaches them."
  Row: transparent surfaces | the energy setting, glossed | act.
  Fault: no landing: the beat ends on a third "never" with no summary.
  Rewrite (SPLIT): And SDFGI's energy setting, which scales its light, never reaches them. / So transparent surfaces read the same probes as opaque ones, but without the reflection march and without the energy scale.

### c10-fog

- **1 (L1766)** "Volumetric fog is another reader of the probes."
  Row: transparent surfaces | volumetric fog, undefined | point.
  Fault: a term unglossed where first said.
  Rewrite: Volumetric fog, fog that fills the air of the scene, is another reader of the probes.
- **2 (L1769)** "It reads their average light, one colour per probe."
  Row: fog reads probes | "average light" | act.
  Fault: "average" over what is not said (the narration already has averages over thirty frames), so it drifts from the sums of c8.
  Rewrite: It reads one colour per probe, the average of that probe's light over all directions.
  (Check "over all directions" against the source. If the source says otherwise, drop the clause and say "one averaged colour per probe".)

## c11 The frame again

### c11-voxels

- **1 (L1797)** "The slab is voxelized, and a jump flood turns it into a distance field."
  Row: the dirty slab | two acts | act, act.
  Fault: two new parts ("voxelized" is c4, "jump flood" is c5), no bridge between them.
  Rewrite (SPLIT): The slab is voxelized. / Then a jump flood turns the voxels into a distance field.

### c11-takeaways

- **1 (L1830)** "So SDFGI keeps the scene around the camera as data."
  Row: the recap of the pipeline | "as data" | claims a consequence with "So", but the screen step does not lead to it.
  Fault: no bridge that tells the listener a list of takeaways starts, and "as data" is vague until c11-data names the pieces. The chapter then gives three points (data, frames, the price) with nothing marking the count.
  Rewrite (SPLIT): Three things are worth keeping from the frame. / First, SDFGI keeps the scene around the camera as data.

### c11-frames

- **1 (L1852)** "It spreads its work across many frames, and each frame reuses what the earlier frames computed."
  Row: SDFGI, its data | spreading, and reuse | two jobs.
  Fault: two new parts. No landing (a "second" marker) after the first takeaway.
  Rewrite (SPLIT): Second, it spreads its work across many frames. / Each frame reuses what the earlier frames computed.

### c11-static

- **1 (L1881)** "It sees only meshes whose GI mode is Static."
  Row: the price | the Static mode | act, with no consequence.
  Fault: a fact with neither why nor what it changes, and "GI mode" is an unglossed setting name.
  needs fact: what happens to a mesh that is not Static. Restructured without it: The price has two parts. / The first is that it sees only meshes whose GI mode, a setting on each mesh, is Static.

### c11-lag (term only)

- **1 (L1887)** "And it takes about thirty frames to catch up with a change." Row is fine, but "catch up" is a new name for what c8 called "show up fully". Use c8's words: "And a change takes about thirty frames to show up fully." (Listed in the drift table below.)

## (a) Drifting terms

| One thing | Names used | Keep |
|---|---|---|
| the screen pass's position for a pixel | the pixel's point, the pixel's position, the point | "the pixel's position" until the push, then "the moved position" |
| the blend range | the blend, that band | "the blend" |
| the maps a probe holds | maps, small textures (c9-map, recap), the probes (c10-map "reads the probes"), "the maps of eight probes" | "maps" once named; c10-map and c10-pass should say "the probes' maps" |
| the light a probe holds | coefficients, light, average light, one colour per probe | "coefficients" before c9, "maps" after; fog: "one averaged colour" |
| the offset of the lookup | half a texel off, shifted by half a texel | "half a texel off" |
| the screen pass | the screen pass, a compute pass over the screen, the pass | "the screen pass" after c10-pass 1 |
| the ambient light | the screen pass's ambient light, SDFGI's light, the bounced light, that light | "ambient light" |
| the delay | thirty frames to show up fully (c8), catch up (c11-lag) | c8's phrase |
| the engine's slab | slab, dirty slab, region (c3) | "dirty slab" |

## (b) The ten most important fixes, ranked

1. c10-probes 1 and 2: one act told twice with no purpose; state the move once, give the value as the engine's choice, and unify "point/position".
2. c9-gain 1 (L1530): open with a bridge and the problem instead of the solid angle out of nowhere; also introduce "uniform sky" before it is used (6).
3. c10-compose 4 (L1736): three facts in one sentence; split into port, direct light only, black shadows.
4. c10-probes 3 to 6: split the cube from the weighting, split the two weight rules, name the comparison of "two hundred times less", and land the beat.
5. c9-oct 10 and 11: gloss bilinear filtering where first said, and unpack "mirrored the way the fold continues them" with its referent.
6. c10-pass 2: remove the repeated appositive and split "after the pre-pass" from "rebuilds the position".
7. c9-texel 1, 3, 4: bridge into the second mismatch, split the half-texel offset from the blend, drop the restatement.
8. c10-cascade 3 and 4: one measure for the blend (distance from the camera), one name ("the blend").
9. c11-takeaways, c11-frames, c11-static: mark the three takeaways with a bridge and ordinals, and give "Static" a gloss (needs fact for its consequence).
10. c10-others 2 and 3: give "never march" its scope (below roughness one fifth) and end on a landing that compares transparent with opaque; c10-fog 1 and 2: gloss "volumetric fog" and "average".

Flagged: 34 sentences (c9: 12, c10: 18, c11: 4; some are counted once with their pair).
