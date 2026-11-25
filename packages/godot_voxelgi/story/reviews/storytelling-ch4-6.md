# Storytelling review, English, chapters 4 to 6 (language only)

Method: every sentence of en-ch4, en-ch5, en-ch6 was mapped (known / new / job) against chapters 1 to 3 and the
listener card. Only faulty sentences are listed. Row format: known | new | job. Line numbers are storyboard.lam.
"SPLIT" marks a rewrite that makes two sentences (the Chinese edition then needs a paragraph more). "needs fact" marks
a fix that needs a reason or definition the narration lacks; nothing was invented. Constants stated as the engine's
choice (thirty-two lights, dynamic range two, 0.25 / 0.15 weights, ninety-five percent) are not flagged.

Counts: 33 faulty sentences (ch4: 12, ch5: 10, ch6: 11), of which 7 are marked "needs fact".

## Chapter 4

**c4-when 3 (L833).** Row: the light passes only rerun on a change (2) | the culler, and a saved copy of each light's settings | act. Fault: two new parts
(a new name, "culler", and the copy), and a drift from ch1's "culling, the test of what is in view". Rewrite (SPLIT):
"Each frame, the culling code, which finds what is in view, also checks every light. / It compares the light's
settings with a copy saved earlier."

**c4-when 6 (L845).** Row: the passes (5) | positions and range converted to cell units | act, purpose given. Fault: "range" is the lamp's reach here
and "dynamic range" later in the chapter (drift); the sentence follows the limit in 5 with no bridge. Rewrite: "Before the passes run, each
light's position and its reach are converted into the box's cells, so the passes can work in cell units." (use "reach"
for the lamp, keep "range" only for the dynamic range).

**c4-direct 5 (L875).** Row: the direction to the light (3) | a cell that faces away gets nothing | branch. Fault: the test (normal against direction) is
named only in 6, so the branch is heard before its condition. Rewrite: swap 5 and 6, or: "The pass then compares the
cell's normal with that direction. / A cell that faces away from the light gets nothing from it."

**c4-direct 7 (L883).** Row: the weighted light (6) | albedo and emission give the light sent back | act. Fault: "light" means the lamp's
light and the cell's outgoing light in one breath (drift). Rewrite: "That weighted light times the cell's albedo,
plus the cell's emission, is the light the cell sends back out." (and see the drift list).

**c4-shadow 4 (L909).** Row: the ray jumps the distance read (3) | rounding down to whole cells, and an axis | act. Fault: two new parts; the phrase
"along the axis the ray mostly follows" could attach to the jump or the rounding; no purpose. Rewrite (SPLIT): "The
jump is rounded down to whole cells. / The cells are counted along the axis the ray mostly follows." Purpose of the
rounding: needs fact (ch3 says the field is stored in whole cells, which may be the reason, but the source must say so).

**c4-shadow 5 (L911).** Row: the jump (4) | "a solid cell, where the field is zero" | check. Fault: drift, "solid cell" here, "leaf" in ch3 ("zero always marks a leaf").
Rewrite: "Here the ray reaches the sphere's cells and reads a leaf, where the field is zero."

**c4-shadow 7 (L919).** Row: the ray's jumps and the hit at zero (4 to 6) | near a surface the field reads under one cell, and the light is dimmed | branch. Fault: a
relation left implicit and a mechanism missing. After 4 says jumps are rounded down to whole cells, a field under one cell would round to zero, which
is a hit. The listener cannot tell why it dims instead. "the cell's light" is also ambiguous (floor cell or the
cell the ray is at). Rewrite: "But close to a surface, the field reads less than one cell, and the floor cell's
light is then only dimmed, not blocked." Why dimmed rather than blocked and by how much: needs fact.

**c4-area 4 (L945).** Row: the rectangle integral, not summed point by point (2, 3) | closed form, and the name linearly transformed cosines | name. Fault: two new
parts, and the name arrives with no act: "a standard fit" is a fit of what? Rewrite (SPLIT): "Instead, it evaluates the
integral in closed form, as a formula with no sum. / The formula comes from a standard fit called linearly
transformed cosines, which approximates the surface's response with a cosine shape whose integral over a rectangle is
known exactly." The second sentence is a general-term gloss; the fit's role in VoxelGI: needs fact if the story wants
it said.

**c4-area 7 (L955).** Row: the integral (2) | "Godot's direct lighting" divides it by two pi | act. Fault: "Godot's direct lighting" is a new thing (the main
renderer's lighting, not VoxelGI's), and the division has no reason. Rewrite: "The renderer's own direct lighting,
the one that shades surfaces from lights, divides this integral by two pi." The reason for the division: needs fact; do not invent one.

**c4-all 1 (L975).** Row: the lit cells (previous beat) | "same pass", sixty-four leaves to a workgroup | act. Fault: "workgroup" is an unglossed term, and "the
same pass" has no referent (which pass?). Rewrite: "The light passes run these same steps for every leaf, sixty-four
leaves to a workgroup, a team of threads that the graphics card runs together."

**c4-write 8 and 9 (L1028, L1032).** Row 8: the texture's content (1 to 7) | no direction stored | point, no consequence. Row 9: 8 | the unread anisotropy value | act. Fault: 8 starts a new topic with no bridge and no
consequence; 9 uses "anisotropy value" with no gloss. Rewrite 8: "The texture also stores no direction with its light, so a texel gives the same light from every side." (arithmetic
consequence). Rewrite 9: "The code shows that the pass still receives an anisotropy value, a setting meant for light that depends on
direction, but no line reads it, so it changes nothing." What the value is meant to control: needs fact.

**c4-recap 2 (L1052).** Row: "the direct light it sends out" (1) | shadows come from the field, and the area light casts none | compress. Fault: two new parts; "that light" could mean the cell's light or the lamp's light. Rewrite (SPLIT): "Every shadow in the cells' light comes from marching the distance field. / The area light casts none."

**c4-recap 3 (L1056).** Row: shadows (2) | each cell holds only light straight from a light | bridge. Fault: "light that came straight from a light" is the same word for two things. Rewrite: "So far, each cell holds only direct light, light that came straight from a lamp." Prefer "direct light" everywhere in this chapter for the cell's light.

## Chapter 5

**c5-map 1 and 2 (L1070, L1072).** Row 1: the recap of ch4 | "one pass of light" reaches what the lights see | point. Row 2: 1 | "a second pass" reads back "the first pass's" texture | act. Fault: new nouns for passes ("pass of light", "first pass", "second pass") that do not match chapter four's "light passes" and "write pass"; "the first pass" could be the direct pass or the write pass. Rewrite (SPLIT): "Direct lighting reaches only the cells the lights see. / A second light pass reads back the texture that the write pass filled, so the cells light each other." Then use "bounce pass" for this pass throughout.

**c5-mips 1 and 6 (L1084, L1102).** Fault: drift. "coarser copies", "coarse levels", "each mip", "the last mip" all name one thing. Ch3 says one mip per octree level, but the chapter never says "levels are the mips". Rewrite 1: "A cone widens as it travels, so it must read coarser levels of the light, the texture's coarser mips." Rewrite 7: "The coarsest mip is a single texel for the whole box." Then say "level" or "mip" consistently (suggest "mip").

**c5-mips 4 (L1094).** Row: each parent sums its children (3) | divides by a divisor between eight and the real child count | act. Fault: two new parts ("divides by a divisor" is empty, and the range of the divisor) with no hint of why it varies until 5 and the next beat. Rewrite: "Then it divides that sum by a number between the count of children it really has and eight." (the purpose arrives in c5-prop; say "The next beat shows why it varies" as a bridge).

**c5-prop 5 (L1130).** Row: zero darkens, one keeps brightness (2, 3) | "fades with distance" versus "spreads too far" | compress. Fault: the relation is implicit. Nothing said links dark coarse levels to fading with distance. Rewrite: "Cones read coarser levels the farther they go, so dark coarse levels make light fade with distance, and bright ones carry it far." "too far" is a judgment; drop "too" unless the source says it.

**c5-cones 2 (L1148).** Row: the ringed floor cell (1) | the bounce pass, the second light pass, three axes around the normal | name before act. Fault: two names before the act, a purpose-less "three axes". Rewrite: "The second light pass, called the bounce pass, starts from the cell's normal and builds a set of three axes around it." Why three axes: needs fact (cones are aimed along them, but say it only if the source does).

**c5-cones 3 to 5 (L1152 to L1160).** Fault: "weight" is not given a job until c5-restore, and "axis" means the three axes in 2 and the cone's own direction in 5 (drift). Rewrite 3: "It sends one cone straight along the normal, with a weight of one quarter, its share of the bounce." Rewrite 5: "Each cone's edge lies thirty degrees from its own centre line." Add a landing: "So the cell sees the room through six wide cones."

**c5-march 4 (L1180).** Row: the cone steps and reads mips (2, 3) | blending front to back | act. Fault: passive with no actor; two appended phrases ("front to back", "like layers of glass"); how a sample counts is not said, and 5 then relies on "opaque". Rewrite (SPLIT): "Each sample is blended in behind what the cone has already seen. / A sample's alpha says how much light it stops, so the cone builds up opacity front to back, like layers of glass." (alpha as coverage, from ch4 sentence "fully transparent").

**c5-march 6 and 7 (L1184, L1188).** Row 6: the stop rule (5) | eleven steps, two thirds opaque | check. Row 7: 6 | "That is because..." | cause. Fault: "That" has two referents (the eleven steps, the low opacity); the stop that happened (ran out of distance) is not said. Rewrite (SPLIT): "The last cone here takes eleven steps. / It ends only two thirds opaque, because the room's cells are thin shells around open air, so it stops by running out of distance." (the last clause follows from 5).

**c5-restore 1 to 3 (L1209 to L1217).** Row 1: cones and their weights (previous beat) | multiplications | act. Fault: "the cell" in 1 may mean the sampled cells or the ringed floor cell; "The result" in 3 does not say whether it is one cone's or the six cones' sum. Rewrite 1: "Each cone's light is multiplied by its weight, and by the albedo of the ringed floor cell, for the bounce off that cell." Rewrite 3: "The six results are summed, and the sum is added to the cell's direct light." (that the cones are summed: needs fact, since the narration does not say it).

**c5-restore 7 and c5-recap 1 (L1225, L1242).** Fault: "no third bounce" and "two bounces of light" make the direct light the first bounce, but chapter four calls it "direct light". Drift in counting. Rewrite 7: "They trace them only once, so the texture holds direct light and one bounce, and no more." Recap 1: "So the texture now holds direct light and one bounce, at every level from five-centimetre cells to the whole box." Also the chapter title says "second bounce": use the same count.

**c5-recap 2 (L1248).** Fault: "baked" for the lit texture is a drift (the bake stored albedo and normals in chapter three, the light passes lit them). Rewrite: "All of it comes from objects that stand still."

## Chapter 6

**c6-mode 3 (L1280).** Fault: a long clause before the subject ("In every frame where..."), and "its" could mean the room's or the VoxelGI's. Rewrite: "Each dynamic object is drawn into the texture in every frame where the room's VoxelGI is in view."

**c6-mode 6 to 8 (L1290 to L1296).** Row 6: the points (5) | shown brighter than stored | check, with two readings. Row 7: 6 | the same plot on the graphics card | act. Row 8: 7 | the points come from the processor's plot | act. Fault: three faults. (a) "plot" is used before the chapter defines it (c6-shrink 3). (b) 7 tells the card first, 8 tells the processor run, and "the same plot" has no referent in 7. (c) "the light it received, drawn brighter than the light stored" gives two lights. Rewrite (SPLIT, in this order): "The points on screen are the texels the teapot wrote, found by a step called plotting, which writes each lit texel into the texture. / The lab draws each point in the light it received, brighter than the light stored. / These points come from the plot run on the processor. / The graphics card runs the same plot, so the texture the room is shaded from holds the teapot too." Why the points are drawn brighter: needs fact; do not invent.

**c6-views 1 and 2 (L1307, L1311).** Row 1: a dynamic object, in the box | drawn at sixteen texels per cell | act. Row 2: 1 | "fine enough to measure, later, how much of each cell the object covers" | purpose. Fault: "texel" means a view's sample here (sixteen to a cell) and the texture's cell-sized texel later (drift); the promise "later" is never kept in the narration, since no beat measures coverage. Rewrite 1: "The object's bounding box is drawn with sixteen view samples to each cell, which the next steps call texels." Rewrite 2: keep, and when coverage is measured: needs fact (nothing in the beats says where it is).

**c6-shrink 4 to 8 (L1353 to L1365).** Row 4: plotting at one texel per cell (3) | depth falls between two cells | act. Row 6: 5 | blended over what is already there | act. Row 7: 6 | each further halving plots to the next mip | act. Fault, three. (a) 5: "by distance" does not say which cell gets more; needs fact. (b) 6: no purpose; the texture was cleared and the static light restored later in c6-cost, so "what is already there" has no referent yet. Add: "Several texels can land in one cell, so each share is blended over what is already there." (arithmetic). (c) 7 reopens the halving that 2 ended at one texel a cell: say "The halving goes on past one texel a cell. / Each further halving plots into the next, coarser mip." 8: "Each halving" should read "Each further halving", or the way the divisor is applied is unclear.

**c6-mips 1 to 4 (L1378 to L1388).** Fault: "drawn" here for what c6-shrink calls "plotted", and "coarser copies" again for mips (drift). Rewrite 1: "At the finest mip, the teapot is plotted in five-centimetre cells." Rewrite 4: "These coarser mips are what the wider cones will read."

**c6-cost 1 and 2 (L1399, L1403).** Row 1: the mips plotted (previous beat) | the whole texture is cleared first | act. Fault: a topic turn with no bridge, a clause before the subject, and "static light" (a new name for what chapters four and five called the cells' direct light and bounce, or "baked light", a drift). Rewrite: "Each frame starts over. / The whole texture is cleared before the teapot is plotted. / Then the static light, the direct light and bounce the light passes produced, is written back from the float buffer, which keeps every cell's light at full precision."

**c6-cost 7 (L1421).** Row: no light bounces off, no shadow (5, 6) | cost: six draws and seventy-two dispatches | point. Fault: opens with "Here", a topic turn with no bridge, and seventy-two is not derived from anything narrated (six views times what?). Rewrite: "One more cost matters. / Each object takes six draws and seventy-two compute dispatches, every frame." The derivation of seventy-two: needs fact.

**c6-cost 8 and 9 (L1425, L1429).** Row 8: the cost (7) | a recall of ch4's limit | bridge. Row 9: 8 | this pass has no limit and reads past the light buffer | consequence. Fault: no bridge; "this pass" is not named (the plot pass? the lit-view pass?); "the light buffer" is a new noun no earlier chapter introduced. Rewrite (SPLIT): "One limit does not carry over. / In chapter four, at most thirty-two lights took part. / The code shows that the pass that lights each texel has no such limit, so past thirty-two lights it reads beyond the end of the light buffer." Where the lights are stored and why that ends at thirty-two: needs fact.

## Drifting terms

- **light**: the lamp, the radiance a cell sends out, the stored value, "direct light", "direct lighting" (renderer's own, c4-area 7), "static light", "baked light", "the room's light". Pick: lamp = "light" only for the source; the cell's = "direct light" / "bounce"; the stored one = "stored light".
- **range**: the lamp's reach (c4-when 6) and the dynamic range (c4-write). Use "reach" for the first.
- **axis**: the three axes (c5-cones 2), a cone's axis (5), the jump's axis (c4-shadow 4), the view axis (c6-views).
- **leaf / solid cell / cell / texel / sample**: "solid cell" c4-shadow 5; "texel" for a view sample and a texture texel in c6.
- **mip / level / coarse copy / coarser copies**: c5-mips, c6-mips.
- **pass names**: "light passes", "first pass", "second pass", "second light pass", "bounce pass", "write pass", "mip pass", "same pass", "this pass".
- **plot / draw / write**: c6-mode, c6-mips ("drawn"), c6-shrink ("plots").
- **culling / culler**: c4-when 3 against chapter one.
- **bounce count**: "second bounce", "no third bounce", "two bounces" against "direct light" (c5).
- **baked**: used for the lit texture (c5-recap 2) and the static light (c6-cost).

## The ten most important fixes

1. c5-march 4, 6, 7: say how a sample builds opacity, and split the "That" with two referents. The whole cone-march rests on "opaque", which is never tied to alpha.
2. c6-mode 6 to 8: define "plot" before it is used, and put the processor's run before the card's, splitting the two-light sentence.
3. c4-shadow 7 (with 4): a field under one cell dims instead of hitting, which contradicts the rounding down; needs fact.
4. c5-map 1, 2 and c5-cones 2: one name per pass (direct, write, bounce), "second light pass" only once.
5. c4-area 4: gloss linearly transformed cosines (a card term) as a cosine shape with an exactly integrable rectangle, with an act before the name.
6. c5-restore 1 to 3 (and c5-recap 1, c5-restore 7): which cell's albedo, what "the result" is, and count bounces the same as chapter four.
7. c6-cost 1, 2, 7 to 9: bridges, "static light" defined once, the light buffer introduced, seventy-two derived (needs fact).
8. c6-shrink 5 to 8: say why shares overlap, and that halving continues past one texel a cell (the chain broke at "then one").
9. c4-write 8, 9: give the missing consequence and gloss "anisotropy value" (needs fact).
10. c5-mips 1 to 7 and c6-mips: one noun for mip, level, and coarser copy.
