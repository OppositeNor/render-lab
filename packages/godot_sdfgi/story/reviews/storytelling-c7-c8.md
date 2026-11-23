# Storytelling review: chapters c7 and c8

Scope: c7 "Light in the voxels", c8 "Probes trace the field". Language only. Rewrites keep every fact. "SPLIT" marks a
rewrite that splits a sentence, its sentences separated by " / ". Rows are known | new | job.

Checked against earlier chapters: omni, static light and region (c3), y scale (c2), slab and jump (c2), the light pass
copying to flagged neighbours (c5), filtered read (c5). Those are known, so they need no gloss here, only a recall.

## c7

### c7-omni

- **#3 (L1101)** known: the omni light and the floor cell (2) | new: "falloff" and "range" | job: act, but the name comes before any act.
  Fault: a label before its meaning; "range" is a bare term; the next sentence nests a formula.
  Rewrite: The light fades with distance. / To start, we divide the cell's distance to the light by the light's range.
  (SPLIT)
- **#4 (L1109)** known: that ratio | new: the squared one-minus-fourth-power formula | job: act, with a consequence.
  Fault: a formula read in one nested breath ("the square of one minus that ratio's fourth power" can mean two groupings).
  Rewrite: Raise that ratio to the fourth power, subtract it from one, and square the result. / That result is the falloff, and it reaches zero at the range, so the light fades out there. (SPLIT)
- **#5 (L1113)** known: that falloff (from 4) | new: the distance power and the attenuation setting | job: act.
  Fault: two new parts (a power of the distance, a setting that supplies it); the falloff is then also the name of the intermediate result, which drifts.
  Rewrite: Next, the light is divided by the distance raised to a power. / That power is the light's attenuation setting. (SPLIT)
- **#8 (L1123)** known: nothing (the topic turns from the formula to the distance measure) | new: heights multiplied by the y scale | job: act.
  Fault: no bridge; "the y scale's one and a half" is a clipped recall; the headline's "squashed space" is never said in the narration.
  Rewrite: But the distance in these steps is not the true distance. / The code shows that every height is multiplied by the y scale from chapter two, one and a half, before the distance to the light is measured. (SPLIT)
- **#10 (L1130)** known: the readout | new: "the same result" | job: check.
  Fault: "same" as what is unsaid, so the check has no known part.
  Rewrite: The readout shows that the lab's model gives the same light as these steps.

### c7-shadow

- **#7 (L1176)** known: this shadow ray | new: it ignores the light's own shadow setting | job: branch/exception.
  Fault: a topic turn with no bridge.
  Rewrite: The shadow ray has one more case. / Every light gets it, even one whose own shadow setting is off. (SPLIT)

### c7-faces

- **#1 (L1194)** known: the voxel from c4 (six facing bits) | new: no single normal; summed per face | job: act.
  Fault: two new parts joined by "so", and the "six faces it shows" is not tied to c4's "six axis directions".
  Rewrite: A voxel is a cube, so it has no single normal. / Its light is summed separately for each of its six faces, one for each axis direction. (SPLIT)
- **#2 (L1203)** known: the six sums | new: one total colour | job: act.
  Fault: reads as contradicting #1 ("summed separately" then "the sum of all six"); "all six" lacks its noun.
  Rewrite: The cell also keeps one colour, the sum of the six faces' light.
- **#3 (L1205)** known: that colour | new: a weight per face | job: act.
  Fault: "how much of that colour the face gets" repeats "colour" in two senses; mild.
  Rewrite: It also keeps a weight for each face, which says how much of that colour the face gets.
- **#4 (L1209)** known: the cell's light | new: copied out to flagged neighbours | job: act, with a long appended phrase.
  Fault: a topic turn with no bridge, and a long relative clause after the object.
  Rewrite: One more step copies the light outward. / The cell's light goes into the empty neighbours that chapter five flagged, the ones whose nearest surface is this cell. (SPLIT)

### c7-static

- **#1 (L1225)** known: static lights from c3 (computed once, when a region is built) | new: written into the cells once | job: act.
  Fault: no bridge from the per-frame quarter updates (c7-cells) and the shadow ray to this topic; "its cells" has no referent.
  Rewrite: One kind of light works differently. / A static light, the kind chapter three said is computed once, is written into the cells it reaches when its region is built. (SPLIT)
- **#2 (L1230)** known: that light | new: it is added on top of what the cell stores | job: act, no consequence yet.
  Fault: "Its light" is a pronoun with two readings (the static light, or the cell's).
  Rewrite: The static light is added on top of whatever light the cell already stores.

### c7-double

- **#1 (L1240)** known: the cascade jump (c2) | new: kept cells carry stored light into "the new list of cells" | job: act.
  Fault: synonym drift. c2 says "slab"; here "new list of cells", then "new slab" in #3; the headline says "scroll", the text says "jump".
  Rewrite: When a cascade jumps, the cells it keeps carry their stored light into the new list of cells, and the slab it takes in starts empty. (needs fact: that the slab starts empty is c2's "holds nothing yet", so it is a recall, not new.)
- **#3 (L1247)** known: static light the cells already had | new: it is added again, to the kept cells too | job: act (the fault).
  Fault: "the light" could be the static light or the cell's stored light; "to the kept cells too" is an appended phrase.
  Rewrite: The code shows that whenever a static light overlaps the new slab, it is added again. / It is added to the kept cells too, not only to the slab's. (SPLIT)
- **#4 (L1251)** known: that static light | new: one more copy each jump | job: act.
  Fault: "the light" ambiguous again.
  Rewrite: So each such jump adds that static light once more.
- **#5 (L1253)** known: the repeated additions | new: the lab's model shows two, three, four times | job: check.
  Fault: "it" has two possible referents; no landing, so the effect (too bright) is unsaid.
  Rewrite: In the lab's model, a kept cell ends up holding that static light twice, then three times, then four. / So the cell grows brighter with every jump. (SPLIT; the consequence follows by arithmetic)

### c7-bounce

- **#2 (L1265)** known: the light pass | new: each cell reads eight probes' light | job: act.
  Fault: the probes' light has not been introduced here, and the sentence that says where it came from (#5) comes three sentences later; a purpose or source several sentences late.
  Rewrite: The eight probes around a cell already hold light, gathered from these same cells a frame earlier. / Before the lights are added, the cell reads that light. (SPLIT; delete #5)
- **#3 (L1267)** known: that probe light | new: its own colour, then the bounce feedback, then add | job: act.
  Fault: overloaded (two multipliers and an addition, one a new named setting).
  Rewrite: The cell multiplies that probe light by its own colour. / It also multiplies by a setting called the bounce feedback, and adds the result to its light. (SPLIT)
- **#4 (L1271)** known: the bounce feedback | new: what it scales, its default | job: name/definition.
  Fault: "X, and Y" with a second new fact; "scales each bounce" restates the name.
  Rewrite: The bounce feedback scales each bounce. / Its default is one half. (SPLIT)
- **#5 (L1273)** known: the probes | new: filled from these cells a frame earlier | job: act.
  Fault: late (see #2). If #2 is rewritten as above, delete this sentence.

### c7-recap

- **#2 (L1285)** known: the stored light (1) | new: its two parts | job: compress.
  Fault: "the lights' own light" is clumsy and "That" is slightly loose.
  Rewrite: That light is what the scene's lights give the cell, plus last frame's bounce from the probes.

## c8

### c8-rays

- **#4 (L1309)** known: the rays | new: a spiral over "the sphere" | job: act.
  Fault: "the sphere" collides with the sphere in the scene (c4, c7).
  Rewrite: The rays' directions follow a spiral over a sphere of directions.
- **#6 (L1317)** known: each ray | new: numbering and the odd or even rule | job: act.
  Fault: two new parts joined with "and".
  Rewrite: The code numbers each ray. / Whether a ray's number is odd or even decides which half of the sphere of directions it points toward. (SPLIT)
- **#7 (L1320)** known: the numbers | new: thirty apart; thirty even; all odd or all even | job: act.
  Fault: two new parts ("and" plus "so"); "thirty apart" has no stated source (eight rays, two hundred forty directions, so thirty by arithmetic).
  Rewrite: One frame's ray numbers are thirty apart. / Thirty is even, so they are all odd or all even. (SPLIT)
- **#9 (L1324)** known: each frame's half | new: the halves alternate | job: act.
  Fault: the relation is implicit; nothing narrated says why consecutive frames switch halves (needs fact: how consecutive frames' ray numbers differ).
  Restructured without it: The code makes the halves alternate from frame to frame.

### c8-march

- **#3 (L1342)** known: the ray stepping | new: stopping within a twentieth of a cell | job: act.
  Fault: no landing and the hit is not named, though c8-hit starts "At a hit".
  Rewrite: The ray stops when it is within a twentieth of a cell of a surface. / That stop is the hit. (SPLIT)

### c8-hit

- **#1 (L1354)** known: the hit (never named before; see above) | new: the ray needs the normal | job: act, purpose three sentences late.
  Fault: purpose several sentences late, and "needs" has no reason until #3.
  Rewrite: A hit voxel keeps light for each face, so the ray must weigh them by the surface normal.
- **#2 (L1357)** known: the normal | new: stored distance grows fastest away; the gradient | job: act plus name.
  Fault: overloaded (a fact, a term and its gloss, and a conclusion).
  Rewrite: The stored distance grows fastest directly away from the surface. / So the field's gradient, the direction in which it grows fastest, gives that normal. (SPLIT)
- **#3 (L1362)** known: that normal | new: how much of each face's light is taken | job: purpose.
  Fault: with the rewrite of #1 it repeats its purpose; delete or fold into #1.

### c8-miss

- **#2 (L1382)** known: the sky | new: mip level two, a blurred copy | job: act then name.
  Fault: the name ("mip level two") comes before its meaning.
  Rewrite: It reads a blurred copy of the sky, called mip level two.
- **#3 (L1384)** known: the blurred copy | new: the comment's reason | job: purpose.
  Fault: "the reason" has no stated "for what"; "throws" drifts from "traces" (c8-rays).
  Rewrite: A comment in the code explains the blur, because a probe traces few rays.

### c8-sky

- **#1 (L1392)** known: the sky read | new: "a mismatch" | job: point.
  Fault: "here" and "mismatch" do not say where or between what.
  Rewrite: But reading the sky hides a mismatch in the code.
- **#3 (L1397)** known: the single sky texture | new: the default is an array of textures | job: contrast, consequence left to the next beat.
  Fault: "a different texture type" is the whole point and comes at the end of a long sentence; the consequence is a beat away.
  Rewrite: By default on desktop, though, the sky is stored as an array of textures. / That is a different texture type from the one the shader declares. (SPLIT)

### c8-sky-fix

- **#1 (L1401)** known: the mismatch | new: the code does not define what an escaping ray reads | job: consequence.
  Fault: "escaping ray" drifts from "a ray that leaves every cascade" (c8-miss).
  Rewrite: So the code does not define what a ray that leaves every cascade reads.

### c8-sh

- **#3 (L1423)** known: the coefficients | new: scaled by 1024, kept as integers | job: act.
  Fault: the purpose arrives in #4; "fixed point" in the headline is never said.
  Rewrite: The coefficients are then scaled by one thousand twenty-four and kept as integers, a way of storing fractions called fixed point. (needs fact only if "fixed point" is not defined by the sentence itself; it is, as a standard gloss)
- **#4 (L1425)** known: integers | new: they add and subtract exactly; the history depends on it | job: purpose.
  Fault: "X, and Y"; "the probe's history" is named before it is explained (c8-history follows); "that" is loose.
  Rewrite: Integers add and subtract exactly. / The next step relies on that, because it adds and subtracts whole frames of rays. (SPLIT)

### c8-history

- **#1 (L1433)** known: the probe | new: its last thirty frames in a ring | job: act.
  Fault: "frames" drifts between rendered frames and stored contributions.
  Rewrite: The probe keeps the light of its last thirty frames in a ring, where the newest frame replaces the oldest.
- **#2 (L1442)** known: the ring | new: a running sum; subtract oldest, add newest | job: act.
  Fault: "its sum" has no referent; two new parts.
  Rewrite: The probe keeps a running sum of those thirty frames. / Each frame, it subtracts the oldest frame from that sum and adds the newest. (SPLIT)
- **#4 (L1448)** known: the brightening by a thirtieth | new: the chart, a straight line that levels off | job: check.
  Fault: "which" can point at the line or the chart; the chart is not pointed at before.
  Rewrite: The chart on screen shows that brightening as a straight line. / The line levels off after thirty frames. (SPLIT)

### c8-recap-next

- **#1 (L1462)** known: the probe's coefficients | new: the next chapter's conversion | job: bridge.
  Fault: "sample" is not glossed; a long sentence with a reason attached.
  Rewrite: A pixel cannot read coefficients the way it reads a texture. / So the next chapter converts them into small textures. (SPLIT)

## (a) Drifting terms

- the cascade jump: "jump" (text), "scroll" (c7-double headline), "jumps"; and what it takes in: "slab" (c2, c7 #3), "new list of cells" (c7-double #1).
- faces: "six faces it shows" (c7-faces) and "six axis directions" (c4); pick one noun for the six per-direction values.
- the sphere: the scene's sphere versus the sphere of ray directions (c8-rays).
- the ray's act: "traces" (c8-rays), "marches" (c8-march), "throws" (c8-miss); a ray that "leaves every cascade", "escapes", and "misses".
- falloff: names the one-minus-fourth-power result and then the whole fade including the distance power (c7-omni #4 and #5).
- the probes' light: "gather" (c8-map, c8-rays), "read" (c7-bounce), "bring that light back" (c7-recap-next, c8-hit).
- "the light": the static light, the cell's stored light, the probe's light (c7-double, c7-bounce); repeat the noun.
- the squashed space (headline) versus y scale (text).

## (b) The ten most important fixes, ranked

1. c7-omni #3-#5: unpack the falloff into verbs (ratio, fourth power, subtract, square) and split the distance-power step.
2. c7-double #3-#5: replace each "the light" and "it" with "that static light", and land on "so the cell grows brighter every jump".
3. c7-bounce #2/#5: say that the probes already hold light from these cells before the cell reads it, and delete the late #5.
4. c8-hit #1: give the purpose of the normal right away (the voxel keeps light for each face) and split the gradient sentence.
5. c8-sh #4 and c8-history #2: define the running sum before the history is used; split the "X, and Y" in #4.
6. c8-rays #6-#9: split the numbering rule, the thirty-apart arithmetic, and the alternation (needs fact for why halves alternate).
7. c7-faces #1-#4: reconcile "summed separately" and "the sum of all six", and add a bridge before the copy to neighbours.
8. c7-omni #8: add a bridge ("the distance is not the true distance") and recall the y scale from chapter two.
9. c8-miss #2-#3 and c8-sky #1/#3: name after meaning (blurred copy, then mip level two); name the mismatch where it is first said.
10. c7-static #1 and c7-shadow #7: add a bridge for each topic turn.
