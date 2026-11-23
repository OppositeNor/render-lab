# Storytelling review: chapters c1, c2, c3 (SDFGI, English narration)

Judged on language only (storytelling.md map, faults table, ear rules). Every rewrite keeps the facts. Row format: `known | new | job`.
"SPLIT" marks a rewrite that splits; its sentences are separated by " / ". "needs fact" marks a fix that would want a reason the narration lacks.

## c1 Where SDFGI runs

### c1-room
- **s3 (L25)** "Now SDFGI adds the light that bounced." Row: black surfaces from s2 | SDFGI and "bounced" (neither glossed; name expanded only in s4) | point. Fault: label before meaning. Rewrite: "Now SDFGI adds the light that bounced off other surfaces."
- **s4 (L27)** "SDFGI is short for ... and the coming chapters show what each part of that name means." Row: SDFGI from s3 | the expansion, plus a promise | name + bridge (two jobs). Fault: two new parts; and it sits between the demo (s3) and its check (s5), so s5's "The red wall tints the sphere" has lost its referent. Rewrite: move it after s5, SPLIT: "SDFGI is short for signed distance field global illumination. / The coming chapters show what each part of that name means."
- **s5 (L29)** "The red wall tints the sphere, and the sky through the open top tints the floor." Row: bounced light (s3, but s4 intervenes) | two tints | check. Fault: two new parts. SPLIT: "The red wall tints the sphere. / And the sky through the open top tints the floor."
- **s6 (L36)** "This is SDFGI the way Godot four point seven builds it." Row: screen | Godot 4.7 | point. Fault: Godot first said with no gloss; "This" is vague. Rewrite: "What you see is SDFGI as Godot, the game engine, builds it in version four point seven."
- **s7 (L45)** "SDFGI is a setting on the scene's environment, and it is off by default." Row: SDFGI | where it lives; the default | act. Fault: two new parts. SPLIT: "SDFGI is a setting on the scene's environment. / It is off by default."

### c1-lab
- **s1 (L50)** "The lab rebuilds it from Godot's source, step by step." Row: SDFGI, from the previous beat (a topic turn) | the lab's rebuild | act. Fault: "it" has two possible referents (SDFGI, Godot's version); no bridge. Rewrite: "The lab rebuilds SDFGI from Godot's source, step by step."
- **s2 (L53)** "The lab's copy runs Godot's own passes on the GPU, at the same sizes as the engine, from its grids to its ray counts." Row: the lab's rebuild | passes, GPU, sizes, grids, ray counts | act. Fault: overloaded (two appended phrases); "grids" and "ray counts" are used before they are told; "copy" drifts from "rebuild". SPLIT: "The lab's rebuild runs Godot's own passes on the GPU. / Their sizes match the engine's, from the grids around the camera to the number of rays each probe traces." (grids and probes are told next; the phrase works as a promise.)

### c1-frame
- **s1 (L74)** "SDFGI keeps its data in grids around the camera, called cascades." Row: nothing (topic turn from citations) | grids, cascades | name. Fault: broken chain, no bridge. Rewrite: "Now the frame itself. / SDFGI keeps its data in grids around the camera, called cascades." SPLIT.
- **s2 (L79)** "Each frame, SDFGI first moves the cascades to follow the camera." Row: cascades | first stage | act. Fault: a staged passage with no stage list ("first" promises more). Rewrite: "A frame runs in stages, and the first moves the cascades to follow the camera."
- **s3 (L89)** "When a cascade moves, new space enters it at one edge, and only that new part needs work." Row: moves the cascades | new space at an edge; only that needs work | act. Fault: two new parts; "new space" then "new part" (drift). SPLIT: "When a cascade moves, new space enters it at one edge. / Only that new space needs work."

### c1-frame-voxels
- **s1 (L95)** "The new part is cut into voxels, small boxes marked solid or empty." Fault: passive with no actor; "new part" for "new space". Rewrite: "SDFGI cuts the new space into voxels, small boxes marked solid or empty."

### c1-frame-probes
- **s1 (L124)** "Probes are points spread through the cascades, and each one records the light arriving from every direction." Row: cascades | probes + what they record | name + act. Fault: two new parts; name before what it names. SPLIT: "Points are spread through the cascades. / Each point records the light arriving from every direction, and is called a probe."

### c1-frame-screen
- **s2 (L141)** "After it, the opaque pass shades the solid surfaces." Fault: "it" = the screen pass or the frame; "opaque pass" new, unconnected. Rewrite: "After that screen pass, the opaque pass shades the solid surfaces."
- **s3 (L143)** "It uses that probe light as each pixel's ambient light, the soft light that fills the shadows." Fault: "It" could be the screen pass or the opaque pass. Rewrite: "The opaque pass uses that probe light as each pixel's ambient light, the soft light that fills the shadows."

### c1-switches
- **s1 (L150)** "Godot has three renderers, and SDFGI works only in Forward+." Row: nothing (topic turn) | renderers; the restriction | point. Fault: two new parts, no bridge, "renderer" unglossed. SPLIT: "So much for the stages. Now where they run. / Godot has three renderers, three ways of drawing a frame. / SDFGI works in only one of them, Forward+."
- **s2 (L155)** "In the other two renderers, Mobile and Compatibility, SDFGI's functions are empty, so it does nothing." Fault: "functions are empty" is code talk. Rewrite: "In the other two renderers, Mobile and Compatibility, SDFGI's functions are empty, so SDFGI does nothing there." (pronoun "it" fixed; keep the fact.)
- **s3 (L158)** "In Forward+, the defaults are four cascades, each a grid of one hundred twenty-eight cells on a side, one voxel to a cell." Row: Forward+ | cascades count, size, cell = voxel | act. Fault: three new parts; "cell" is used before it is told (c2 defines it). SPLIT: "In Forward+, the defaults are four cascades. / Each cascade is a grid of one hundred twenty-eight cells on a side. / A cell is the space one voxel fills."

### c1-cells
- **s1 (L166)** "In the finest cascade, the cells are twenty centimetres across." Fault: "finest" is undefined (later called "first"); no consequence. Rewrite: "In the first cascade, the finest one, a cell is twenty centimetres across, so a voxel is a box of that size."

### c1-settings
- **s1 (L178)** fine except "more": "Three more project settings" has no known "other". Rewrite: "Three project settings shape SDFGI further."
- **s2 (L180)** Row: settings | three new parts as one list | act. Fault: overloaded; "frames it averages" has no object. SPLIT: "One sets how many rays each probe traces. / One sets how many frames each probe averages over. / One sets how often the light in the voxels is recomputed."

### c1-prepass
- **s1 (L185)** "The probes update only if one other pass runs." Row: probes | a condition | point. Fault: topic turn with no bridge; "update" is new for what the probes do (they trace). Rewrite: "One catch decides when the probes update. / They update only if one other pass runs." SPLIT.
- **s2 (L188)** "That pass is the depth pre-pass, which draws the scene's depth before any shading." Fault: "depth" unglossed. Rewrite: "That pass is the depth pre-pass, which draws the scene's depth, how far each surface is from the camera, before any shading."

### c1-prepass-probes
- **s1 (L192)** "The code starts the probe update in only one place, where it draws this pass." Fault: "it" ambiguous; "probe passes" (title) vs "probe update" drift. Rewrite: "The code starts the probe update in one place only, inside the code that draws the depth pre-pass."

### c1-prepass-together
- **s1 (L196)** "A comment there says that the two then run at the same time." Fault: "the two" has no clear referent. Rewrite: "A comment there says the probe update and the pre-pass then run at the same time."
- **s2 (L198)** "And it does that only when the pass runs." Fault: "it" and "that" each have two referents; opens with "And". Rewrite: "That happens only when the pre-pass runs."

### c1-prepass-gate
- **s1 (L202)** Row: pre-pass | two conditions | act. Fault: two new parts; "stencil" unglossed. SPLIT: "The code shows two ways the pre-pass runs. / One is the project setting for the depth pre-pass. / The other is a material that uses the stencil, a per-pixel mask the GPU can test while drawing."

### c1-prepass-freeze
- **s1 (L209)** "So with that setting off and no stencil material in view, the probes stop updating, and their light freezes." Fault: "in view" is not in the cause stated (a material that uses the stencil); the second clause restates. SPLIT: "So if that setting is off and no material uses the stencil, the probes stop updating. / Their light then stays frozen at its last value."

### c1-rays
- **s1 (L219)** "The setting for the probes' ray count lists eight, sixteen, thirty-two, and more." Fault: "lists" (editor list) vs "its own list" (s4): two lists named alike. Rewrite: "The setting for the probes' ray count offers a choice of eight, sixteen, thirty-two, and more."
- **s2 (L224)** "the editor" is new; fine otherwise. Rewrite: "The default is the second choice, which the editor shows labelled sixteen."
- **s4 (L231)** "It looks the second choice up in its own list, which begins with four." Fault: "It" = the code, three sentences back is "the code" in s3 so acceptable; "its own list" vs s1. Rewrite: "The code looks the second choice up in a list of its own, which begins with four."

### c1-recap
- **s1 (L237)** "So each frame, SDFGI runs in stages, and only in the Forward+ renderer." Fault: two new parts hung with "and". SPLIT: "So each frame, SDFGI runs in stages. / It runs only in the Forward+ renderer."

## c2 Cascades that follow the camera

### c2-nested
- **s1 (L261)** "The frame begins with the cascades." Fault: repeats c2-map s1 and c1-frame s2 (no new part). Fix: fold into s2: "The frame begins with the cascades, because SDFGI must place its grids before it can draw any voxel into them." (drop s2)
- **s2 (L263)** (see s1) "grids" drifts from "cascades". Rewrite as above with "cascades" for the second mention: "SDFGI must place the cascades before it can draw any voxel into them."
- **s3 (L265)** "A cell is one box of a cascade's grid, the space that one voxel fills." Known: grid | cell | name. "cell" was used from c1 on, before this. Fix: introduce it in c1-switches (above); here recall: "Recall that a cell is one box of a cascade's grid, the space one voxel fills."
- **s7 (L284)** "So the fourth and coarsest cascade spans two hundred and five metres." Row: doubling from s6 | fourth span | act. Fault: a gap in a chain of numbers (cell sizes of the second and third never said; 205 is eight times 25.6). Rewrite: "Doubling three times makes the fourth and coarsest cascade eight times as wide as the first, about two hundred and five metres."
- **s8 (L286)** "That gives fine detail near the camera, and a long reach far away." Fault: "That" ambiguous; "and" adds a second new part. Rewrite: "So together the cascades give fine detail near the camera. / They also give a long reach far away." SPLIT (or keep one sentence with "Together, the cascades give ...").
- **s9, s10 (L288, L291)** s9 "That trade exists because ..." Row: "trade" (never named before) | the reason | purpose; s10 gives the example after. Fault: the rule before the example; "trade" appears as known. Rewrite s9+s10: "Every cascade has the same one hundred twenty-eight cells a side. / So a smaller cell gives finer detail and a shorter reach, and a larger cell the reverse. / That is the trade the cascades make."
- **s11 (L295)** "...so it fits inside the first cascade, at its centre." Fault: "its" = the room or the cascade. Rewrite: "The study room is only six metres across, so it fits inside the first cascade, at the cascade's centre."

### c2-yscale
- **s2 (L314)** "It has three choices, and its default is labelled seventy-five percent." Fault: two new parts; "its" ambiguous. SPLIT: "The y scale has three choices. / The default choice is labelled seventy-five percent."
- **s4 (L320)** "In the code, the setting is a factor, and seventy-five percent becomes one and a half." Fault: two new parts. SPLIT: "In the code, the setting is a factor, a number the height is multiplied by. / Seventy-five percent becomes one and a half."
- **s5 (L324)** "Every height is multiplied by one and a half before it is placed in the grid." Fault: passive, no actor. Rewrite: "The code multiplies every height by one and a half before placing it in the grid."
- **s6 (L328)** "real height" drifts (the grid's height vs "real" vs later "in the world"). Rewrite: "So a cell covers only thirteen centimetres of height in the world, twenty divided by one and a half."
- **s7 (L330)** Fault: chain gap (cells of 13 cm to the height of the cascade). Rewrite: "The first cascade is one hundred twenty-eight of those cells high, so it is just seventeen point oh seven metres."

### c2-probes
- **s1 (L339)** "lattice" is unglossed; the sentence starts with the place. Rewrite: "Each cascade holds probes in a regular lattice, one every eight cells."
- **s3 (L348)** "Counting both ends, that gives seventeen probes along each axis." Fault: opens with a participle. Rewrite: "If we count both ends, that makes seventeen probes along each axis."
- **s4 (L352)** Fault: arithmetic hidden. Rewrite: "In the first cascade, eight cells of twenty centimetres put the probes one point six metres apart."
- **s5 (L355)** "So only a handful of probes fall inside the room." Fault: vague quantity (six over one point six is about four per axis). Rewrite: "So only about four probes fall along each side of the room."
- **s6 (L357)** "This view draws those probes brighter." Fault: "those probes" and "brighter than what" are open. Rewrite: "In this view, the probes inside the room are drawn brighter."

### c2-scroll
- **s3 (L381)** "When the camera strays more than four cells away, the cascade jumps eight cells to follow it." Fault: overloaded (threshold, jump size); "away" from what; "it" ambiguous. SPLIT: "When the camera's cell is more than four cells from the cascade's centre cell, the cascade jumps to follow the camera. / The jump is eight cells."
- **s5 (L388)** "The jump takes in new space on the side it moves toward, an eight-cell slab that holds nothing yet." Fault: "it" ambiguous; two new parts (slab, holds nothing); "new space"/"slab" drift. SPLIT: "On the side the cascade moves toward, the jump takes in an eight-cell slab of new space. / The slab holds nothing yet."
- **s6 (L396)** "That slab is marked dirty, which means it must be voxelized again." Fault: "again" contradicts "holds nothing yet" (needs fact: whether "again" is right). Without it: "That slab is marked dirty, which means it must be voxelized."
- **s7 (L398)** "...the rest of the cascade keeps its work." Fault: "work" is vague. Rewrite: "Only the dirty slab is voxelized, so the rest of the cascade keeps the voxels it already has."
- **s8 (L400)** "But if the new space would be more than half the size of what is kept, the whole cascade is rebuilt instead." Fault: a long clause before the subject. Rewrite: "But the whole cascade is rebuilt instead, if the new space would be more than half the size of what is kept."

### c2-recap
- **s1 (L408)** Fault: "grids" for cascades. Rewrite: "So SDFGI keeps four cascades around the camera, each twice as coarse as the cascade inside it."
### c2-recap-slab
- **s1 (L412)** Fault: "grid" (cascade); "slab"/"new space" ok. Rewrite: "When the camera strays, a cascade jumps, and only the slab of new space it takes in needs building."

## c3 What is voxelized, and which lights

### c3-map
- **s1 (L420)** "Before the GPU draws the meshes of that new slab, the CPU decides which meshes and lights belong to it." Row: the slab from c2 | CPU decides; meshes; lights | point. Fault: long clause first; "mesh" unglossed; repeated almost word for word by c3-regions s2. Rewrite: "The CPU decides first which meshes and lights belong to that new slab. / A mesh is a model made of triangles. The GPU draws the meshes afterwards." (SPLIT, three sentences; the gloss of mesh is standard.)

### c3-regions
- **s2 (L436)** Fault: restates c3-map s1. Cut it, or turn it into the bridge: "To decide, the CPU first gives each slab a box in the world."
- **s3 (L441)** Fault: topic turn with no relation. Rewrite: "But a cascade can jump along more than one axis in the same frame."
- **s5 (L449)** "Where two regions overlap, only the first keeps the shared corner, so no corner is voxelized twice." Fault: rule before the example; "first" has no clear referent. SPLIT: "Two regions can overlap at a corner. / Only the first region keeps that corner, so no corner is voxelized twice."
- **s8 (L459)** "which undoes it": "it" is the multiplying or the one and a half. Rewrite: "So the box's height is divided by one and a half, which undoes that multiplication."
- **s9 (L465)** "Here that box is ..." Fault: three numbers; "seventeen" disagrees with "seventeen point oh seven"; "Here" unanchored. Rewrite: "Take the first cascade, jumping along one axis. / The box is one point six metres along the jump, seventeen point oh seven metres high, and twenty-five point six metres deep." SPLIT.

### c3-geometry
- **s1 (L476)** "When the camera jumps far, the new space is more than half of what is kept, so the whole first cascade becomes one region." Fault: no relation to the title (Only Static meshes are voxelized, seen or not: title and beat disagree, settle before rewording); "camera jumps" for "cascade jumps"; repeats c2-scroll s8 without recall. Rewrite as a bridge: "One case changes the box. / When the camera jumps far, the new space is more than half of what is kept, as in chapter two. / Then the whole first cascade becomes one region." SPLIT (and retitle the beat, or move the sentence to c3-regions).

### c3-gimode
- **s1 (L493)** Fault: passive; "that box" across a beat boundary. Rewrite: "The CPU tests every mesh in the scene that is not hidden against the region's box."
- **s3 (L499)** "Each mesh has a GI mode, and only meshes whose GI mode is Static are voxelized." Fault: two new parts; "GI mode" unglossed. SPLIT: "Each mesh has a GI mode, a setting that says how SDFGI treats it. / Only meshes whose GI mode is Static are voxelized."
- **s4, s5 (L511, L513)** Row: Static (s3) | Dynamic, from nowhere | check. Fault: "Dynamic" appears without being named as the other mode; s5 "But" restates s3. Rewrite SPLIT: "Any other mesh, say one set to Dynamic, is left out of the voxels. / So it adds nothing to the distance field. / It still receives SDFGI's light."

### c3-lights
- **s1 (L527)** Row: lights | positional lights, three types, bake mode, two values | name. Fault: four new parts; name before act. Rewrite SPLIT: "SDFGI treats each positional light, an omni, a spot, or an area light, in one of two ways. / Which one is set by its bake mode, Static or Dynamic."
- **s4 (L536)** "Directional lights are always dynamic, so a sun is recomputed at every update." Fault: topic turn with no bridge; "sun" for directional light. Rewrite: "One kind of light has no choice. / Directional lights, such as a sun, are always dynamic, so they are recomputed at every update." SPLIT.
- **s5 (L538)** Fault: turn with no bridge. Rewrite: "Back to static lights. / For each region, the CPU lists the static lights whose range overlaps it." SPLIT.
- **s6, s7 (L543, L545)** "But each light also has a maximum cascade." Fault: name before meaning; s7 two new parts. SPLIT: "But each light also has a cutoff, its maximum cascade. / By default that is the third cascade. / Past it, the light is left out."

### c3-finding
- **s1 (L561)** Fault: no signal that this is the fault the title names; "these light lists" lost its beat. Rewrite: "One detail goes wrong with these lists. / The CPU stores the static lights' lists in slots, one slot per cascade." SPLIT.
- **s3 (L570)** acceptable, but "counts the slots differently" and "index"/"position" drift (see drifts).
- **s7 (L579)** "The readout shows the lab's model losing that light in the same way." Fault: "readout" never pointed at. Rewrite: "Look at the readout. / It shows the lab's model losing that light in the same way." SPLIT.
- **s8 (L581)** "The two counts" is vague. Rewrite: "The slot where a list is stored and the position it is read from agree only when the cascades that moved are the first ones, in order."

### c3-recap-next
- **s1 (L593)** Acceptable. Note the chapter title says "which lights" and the next chapter takes only the meshes; add "The lights come back in chapter seven." SPLIT if wanted (fact by outline, check).

## (a) Drifting terms

1. The new space a cascade takes in: "new space", "new part", "slab", "dirty slab", "region", "dirty axis", "box". Pick: "slab" for the new space, "region" only for its record on the CPU, "box" for its metres form, and say each once.
2. Cascade and "grid" (c1-frame, c2-nested, c2-recap, c2-recap-slab); "grids" is also what the cascades hold. Use "cascade" for the nested thing, "grid" only for its cells.
3. Voxel and "cell" (c1-switches uses "one voxel to a cell" before "cell" is told).
4. First cascade, "finest", "inner", "coarsest": name the first cascade "the first cascade, the finest" once and keep "first".
5. Probe update, probe passes, "update" for light update, "recomputed": the light in the voxels and the probes' update are two things both "updated".
6. Pass, stage, step: "stages" in c1-frame, "passes" for the screen, opaque, depth pre-pass, probe update.
7. Height, "real height", "in the world", "y": pick "height in the world".
8. Slot, count, index (title), position (c3-finding): pick "slot" for storage and "position" for the read.
9. Directional light and "sun"; light "bake mode" vs "computes/recomputed"; GI mode Static vs bake mode Static (two settings with the same two values, said apart each time).
10. Two lists named alike in c1-rays (the editor's labels and the code's list).

## (b) Ten most important fixes, ranked

1. c3-geometry s1: the only sentence of a beat whose title is about Static meshes; settle the fact and retitle or move.
2. c3-gimode s4-s5: "Dynamic" arrives without being named as the other mode; reorder.
3. c2-nested s9-s10 (and s7, s8): the rule ("that trade") comes before its example, and the number chain skips cells.
4. c1-switches s1 and s3: topic turn with no bridge, "cell" used before it is told, three new parts in one sentence.
5. c1-room s4/s5: the name sentence cuts between the demo and its check; split the two tints.
6. The "new space / slab / region / box" drift across c1, c2, c3 (one noun for one thing).
7. c1-prepass-together s1-s2: "the two", "it", "that" each with two referents.
8. c1-frame-screen s2-s3: "it" could be the screen pass or the opaque pass.
9. c3-lights s1 and s6: name before act, four new parts; bake mode and maximum cascade need show-then-name.
10. c2-scroll s3, s5, s6: threshold and jump size in one sentence, "it" ambiguous, "again" contradicting "holds nothing yet".
