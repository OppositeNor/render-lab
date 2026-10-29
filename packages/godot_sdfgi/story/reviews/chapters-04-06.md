# Review: chapters 4-6 (Voxelization, The distance field, Occlusion)

Method: storyboard text, outline, listings, and the three stills per beat (25/50/85 %). Subtitle presence in a still is used as a proxy for narration timing. Verify any timing claim against the audio before acting on it.

## Cross-cutting findings

1. Budget is far larger than the narration. Ch4: 12 sentences (~48 s) in 90 s. Ch5: 13 sentences (~52 s) in 105 s. Ch6: 6 sentences (~24 s) in 35 s. Several beats show a 50 % still with no subtitle (c5-seeds, c5-upscale, c4-passes) and so sit silent while an animation plays. Either add the missing explanations (below, preferred) or cut the budgets. The outline allowed 150/170/90 s, so the explanation was cut, not the time.
2. The slice inset in ch5/ch6 is about 1/6 of the frame width and unreadable at video size. It carries the entire point of ch5 (seeds spreading) and ch6 (visibility spreading). Make the inset at least 40 % of the frame during the flood, upscale and shell beats, or switch to a full-frame slice view.
3. The readout panel (nearest solid, distance, stored byte, march reads) is replaced by the code panel as soon as `panel=code` appears, so c5-store never shows the readout row it names ("one plus the distance", "reads minus one"). Show readout and code together, or give the readout beat its own shot.
4. Nothing in these chapters says why each step exists. Ch4 explains packing formats but not why a rasterizer is used; ch5 never says what a jump flood is; ch6 never says what the numbers are for.

## Chapter 3 to 4 bridge

c3-sources ends "These are the culling lines." then c4 opens "With the geometry chosen, the GPU draws it into the grid." The bridge works, but "the grid" has not been named as a 3D texture of cells in ch3 (check). Proposed: "With the geometry chosen, the GPU draws it into the grid, a three-dimensional image of cells, each holding what is in that cell of space." Ch3's last real beat (c3-finding) is a bug; consider a one-line return, e.g. at the start of ch4: "Back to the lights that do reach it: first, the geometry has to become voxels."

## Chapter 4 · Voxelization (90 s, ~48 s of speech)

**c4-passes**
- Depth: "drawn three times by a camera looking along each axis" is not explained. A viewer will ask why three, why orthographic, why no colour target. Missing: a surface seen edge-on from one axis covers no pixels, so another axis is needed. That is exactly what the animation (`voxelPasses=0..3`) shows: walls appear per pass. Say it.
- Intuition: stills confirm it works: 0 passes (shaded room), 2 (left wall and floor voxels, other wall missing), 3 (all). Good. But the sentence "Each view finds the surfaces facing it; together they cover the room" names no wall. Add colours: the red wall, the teal wall are visibly the ones that appear.
- Highlight: callout `render-sdfgi` is up for the first sentence only; the 85 % still has callout `render_list_template (cull variant)` while the narration is about the CPU trace (mismatch: the callout shows the culling source while the sentence says "the lab ... on the CPU"). Drop the callout for the CPU sentence or label it "the lab, not Godot".
- Visual: the voxel dots on the right wall at 85 % are large perspective discs overlapping into a smear (teal wall at right edge). Reduce dot size at grazing angles, or move the camera yaw path so it does not end at 24 deg.
- Proposed sentences:
  - replace "The region is drawn three times, ..." with "The region is drawn three times, by an orthographic camera looking along each axis; every pixel is one cell of the face it looks at, so a pixel that is drawn is a voxel that is solid."
  - replace the "Each view..." sentence with "A wall seen edge-on covers no pixels, so each view finds only the surfaces facing it: the floor and one wall first, then the other wall, and together the three cover the room."
  - c4 CPU sentence: "The lab does not rasterize: it traces each pixel's ray through the room's shapes on the CPU, and finds the same fragments."

**c4-albedo**
- Depth: the code panel highlights the right lines (`a |= ... << 11` etc.) and the readout was lost at the same moment (see cross-cutting 3). The voxel readout `0xd66d`, bits 26/25/22 sits in the first beat's panel only. The narration "sixteen bits" is never shown decoded for one voxel. Keep the readout for the beat and highlight the voxel under it, or show the decode: "0xd66d: red 26 of 31, green 25, blue 22".
- Highlight: lines are lit but it is a block of 4; split into two cues ("lowest marks solid" lights `a = 1`, then the three lines).
- Missing why: five bits per channel (32 levels) is coarse; say that is the price of fitting a colour into sixteen bits.
- Change: camera drift only. Fine.
- Proposed: "{@ highlight=solid} The lowest bit marks the voxel solid; {@ highlight=albedo} the other fifteen bits hold red, green and blue at five bits each, so a colour has only thirty-two levels."

**c4-facing**
- Depth: "six bits, one per axis direction" never says what the facing is used for (it later chooses which of six anisotropic faces a voxel can be lit on, ch7). Add a forward pointer, one clause.
- Intuition: nothing on screen shows a facing bit. The voxels are drawn all white; the sentence points at nothing visible. Colour the voxels by the six bits (or draw a small arrow/glyph per voxel), and light the bit in the readout row `facing (+X +Y +Z -X -Y -Z) 010000` while it is spoken. The readout exists in beat 1 and is gone here.
- Visual: the camera sits at 3.0 and the sphere fills the left of frame; the subtitle and callout label ("MODE_RENDER_SDF, facing") are cut off at the panel edge in the stills.
- Proposed: "{@ highlight=facing} A second image keeps six bits, one for each axis direction, and every fragment sets the bit of the axis its normal points along most; later, a voxel can only be lit on the faces it set."

**c4-emission**
- Depth: the weakest. "Shared-exponent format of thirty bits, with a five-bit weight for each of six faces" is two jargon items with no example. A graphics viewer knows RGBE but not the 6x5 split. Say that the exponent is shared so three channels fit in 30 bits, and that the weights say how much of the light goes out of each face.
- Intuition: nothing on screen is emissive in the stills, the emission row reads 0x00000000 in every still. The beat shows no emitter, so its subject is missing. Add an emissive object (the small orange box?) or light the emitter's voxels and show its nonzero readout.
- Highlight: line lit (`if length(emission) > 0.001`), but the second line (`emission_grid[cell] = rgbe8985`) should light for the packing sentence. The subtitle is split mid-sentence in the 50 % still ("of thirty bits, with a five-bit"), fine, the sentence is just long; split it in two.
- Proposed: "An emissive surface also writes its light. {@ highlight=emission} Red, green and blue share one exponent, which fits them in thirty bits; the last bits give a five-bit weight to each of the six faces, saying how much light leaves that way."

**c4-shell**
- Depth and intuition: good, the cut sweep (`cutHeight=4..0.6`) is the right animation and the 25/50/85 stills show a ring of voxels. But at 25 % the cut plane (white slab) is almost the whole frame top and hides voxels; at 85 % the hollow sphere is small and the ring is faint. Zoom to the sphere (camera.target on it) and tint the cut face.
- Missing consequence: why it matters. A hollow shell is fine because rays only need the surface and the distance field fills in the rest. Say it: "which is enough, because the distance field measures from the surface."
- Wording: "read from the code" is vague; name the reason: the drawing writes only fragments of surfaces, no filling step exists.
- Proposed: "Cut the room at the sphere's middle, and its voxels are a thin shell: only surfaces are drawn, nothing fills the inside, and the distance field needs no more."

**c4-sources**: fine; last still callouts are all legible. The sentence "These are the voxelizer's lines." is a placeholder; keep it (consistent with the other chapters).

## Chapter 5 · The distance field (105 s, ~52 s of speech)

**c5-why**
- Intuition: three stills are identical (the inset slice never changes, camera drift only). The outline promised a sphere-trace step drawn next to the slice; it is absent. The sentence "a ray can step that far without passing through anything" points at nothing.
- Change: add a sweep: animate a marker ray stepping, each step drawing a circle of the stored distance (parameters for the readout: "a march reads 3 - 1 = 2"). The readout row `a march reads` already exists; have it change.
- Depth: "distance to nearest solid voxel" in cells; mention one byte, 128 cells, and that this is what makes tracing cheap (ch8 uses it).
- Proposed: "The voxels say where surfaces are; a ray needs to know how far away they are. {@ callout=sdf-store} So every cell stores the distance to its nearest solid voxel, and a ray can step exactly that far, as the circle shows, without passing through anything."

**c5-seeds**
- Depth: half size is announced, not justified. A graphics viewer will ask why half. Reason (research: half is hard-coded true; one fewer pass of 8x cost; the full-size pass fixes artifacts). Say: "an eighth of the cells to flood".
- Intuition: the inset at 25/50/85 % is the same small seeds picture (frame ~5 % of the width of cells). Show seeds big; the subtitle at 50 % is mid-sentence. Silence/dead time here: a single sentence of ~10 s for a beat.
- Highlight: the callout `jump-flood-init` shows; nothing in the slice labels the 2x2x2 block. Draw one block outlined at full size, and its seed.
- Proposed: "The search runs at half size, sixty-four cells a side, an eighth of the work. {@ highlight=init} Each half cell starts with one solid voxel from its two-by-two-by-two block, if the block has one, and that voxel is its first guess at the nearest surface."

**c5-flood** (the most important beat; it does not explain a jump flood)
- Depth: "a cell looks at its twenty-six neighbours a step away and keeps the nearest seed it finds" is the whole mechanism but never says why steps 32..1 work: a seed travels far in the early big steps, then the small steps correct near-misses, so after log2(N) passes every cell knows (approximately) the nearest seed. A viewer who has not seen a jump flood will not get it. Also, "the nearest" means nearest to the cell itself, not to the neighbour: that is the `dist(p, q) < dist(p, seeds[p])` line, which is not lit.
- Intuition: floodStep goes 0..6, but the 50 % still is already at step 6; the sweep ends before half the beat. The inset (hsv colour map = seed identity) is good, but tiny. Add a one-cell example: pick a cell, draw its eight in-slice neighbours at the current step as arrows, and a readout for which one wins.
- Highlight: lit `for o in OFFSETS_26 (neighbours)` only; the `@nearest` line is never lit. Highlight the while loop / step halving while the steps are named (the 85 % still lights both loops for a separate sentence, fine).
- Fit: the sweep should be as long as its sentence; add pacing: `floodStep=0..6` over the beat's second sentence only.
- Proposed:
  - "{@ highlight=neighbours} Each pass, a cell looks at its twenty-six neighbours, a fixed step away in every direction, {@ highlight=nearest} and keeps whichever seed is nearest to the cell itself."
  - "{@flood> floodStep=0..6}The step starts large and halves, thirty-two, sixteen, down to one, so a seed first jumps across the room, and then the small steps correct the cells it misled{<flood}."
  - add: "After the last step every cell holds the seed that is, within the half-size error, its nearest."
  - the shared-memory sentence: "The first two passes read the neighbours straight from memory; the last four copy the neighbourhood into shared memory first, which is only faster, and finds the same seeds."

**c5-upscale**
- Depth: "each cell checks the block around its seed" is opaque: the upscale takes the half-size seed and, at full resolution, picks among the solid voxels of the seed's block. Explain: "a half-size seed is only accurate to two cells". "Fixes what the half size missed" does not say what was missed. Say: cells near a half-cell border can pick the wrong block's seed by one cell.
- Intuition: stills at 25/50/85 % (steps 7, 7, 8) look the same; no visible change between step 7 and 8, and the 50 % still has no subtitle (dead air). Show the difference between step 7 and 8 as a highlighted diff (cells whose seed changed), or drop the pretence of animation and show step-7 and step-8 maps side by side.
- Highlight: callout `jump-flood-upscale` appears; code panel absent, the `@upscale` and `@final` lines (exist in the listing) are not lit. Add `panel=code code=jump-flood highlight=upscale` and `highlight=final`.
- Proposed: "{@ highlight=upscale} The half-size result is scaled up to the full one hundred twenty-eight, and each cell picks the nearest solid voxel in the block of its seed. {@ highlight=final floodStep=8} A last pass with a step of one fixes the cells whose seed was off by a cell, an error that comes from having searched at half size, shown here as the cells that change."

**c5-store**
- Depth: "zero at a solid voxel, otherwise one plus the distance, in cells" is explained. But the byte is 8-bit: what happens past 255 (not an issue at 128 cells?) and why one plus: so zero can mean "inside" (so the field is unsigned, said next).
- Highlight: `d` lines lit; good. But the readout rows "stored byte 3 = 1 + d" and "a march reads 3 - 1 = 2" are the point of the second sentence and are not visible (panel replaced by code).
- Change: the readout shows one cell; the slice cursor (yellow ring) does not move across the three stills. Move it from outside to a solid voxel (distance 0, byte 0) and let the readout show "reads -1".
- Proposed second sentence: "{@ callout=sdf-decode} The field is not signed: a reader subtracts the one again, so the zero stored in a solid voxel reads minus one, and that is how a ray knows it is inside."

**c5-cells**
- Depth: "sixteen-byte record: position, albedo, facing, stored light" lists contents but not purpose; one clause: the list is what the lighting pass (ch7) walks, so it lights solid cells only, not 524,288 cells (readout says 3156 / 524288, use it). Neighbour bits sentence: "Twenty-six more bits mark the empty neighbours whose nearest surface is this voxel" is hard to follow. Say what they are for (light on a solid voxel can reach the empty cells that point at it) or cut.
- Intuition: nothing on screen is a record. The struct panel (`SdfgiSolidCell`) the outline planned is not here; the highlights are lines of Python. Show the record as bit fields, light the fields in turn.
- Visual: the 25 % still has the subtitle "records: its position, albedo, facing, and stored light." cut mid sentence (subtitle splitting, fine).
- Proposed: "{@ highlight=cell} Every solid voxel also goes into a list of sixteen-byte records, and this list, three thousand cells instead of half a million, is what the lighting walks. {@ highlight=neighbours} The top bits mark which of its twenty-six neighbours are empty and nearest to this voxel, so light can later be passed on to them."

## Chapter 6 · Occlusion (35 s, ~24 s of speech)

**c6-shells**
- Flow: "The same pass also asks, for each probe, what it can see." Ch5 never mentioned probes (first appears here); the sentence is the only introduction. Add one clause on what a probe is, and why the distance field pass computes visibility.
- Depth: the mechanism is clear in one sentence, but "takes the average of its neighbours one step closer" with "a solid cell stops it" is hard without seeing a solid cell zero. Also the shell numbering (0..22, |x|+|y|+|z|) and the 16-cell window are never connected. Missing: what the value means (0 = hidden from the probe, 1 = visible).
- Intuition: 25 % (shell 0): inset nearly empty, 85 %: white square with a dark spot (the box) and red marker. The sweep runs 0..22 but the 50 % still is already at 0 as well (still 2 is same as still 1?), and the white filling is only seen at the third. Reasonable, but the shadow behind the box is barely visible (red/dark dot near the top-right); with a mostly white slice it reads as "everything visible". Choose a probe next to the box so the shadow wedge is large; use a colour ramp (black = hidden, white = visible) with a legend.
- Highlight: callout shows (`occlusion-dispatch`, then `occlusion-shells`), but the listing `occlusion.py` (@shells, @solid, @average) is not used. Add `panel=code code=occlusion highlight=shells` then `average`, `solid`.
- Fit: sweeps 22 shells in the span of one sentence (about 8 s): fine.
- Proposed:
  - "The same pass also asks, for each probe, how much of the room around it the probe can see, since a probe that sits behind a wall should not pick up light from the other side."
  - "{@ callout=occlusion-dispatch} A window of sixteen cells is opened around each probe."
  - "{@shells> occlusionShell=0..22}Starting next to the probe, each shell of cells takes the average of the cells one step closer to the probe, so visibility is one beside the probe and fades outward{<shells}; {@ highlight=solid} a solid cell is zero, and the cells behind it inherit the dark."

**c6-store**
- Depth: "four bits for each of eight probes" is not explained: eight is the probes around a cell (the 2x2x2 probe cell, trilinear), which ch8-9 uses; four bits = sixteen levels (0 to 15), as the readout "10 / 15" shows. The readout rows `class 0..7` with numbers, a yellow class 3 `10 / 15`, are never narrated: point at one.
- Intuition: all three stills identical (inset static, readout static). Animate: cycle which probe class is highlighted, or pick a cell and animate the eight classes lighting. Right now the sentence points at nothing.
- Highlight: callout `occlusion-store`; the listing line `@store` is not lit; add it.
- c6-off ("By default nothing reads them: use occlusion is off, yet the pass runs every time a cascade is built.") is the interesting finding but is delivered flat and has no visual: the readout row `use_occlusion false (computed anyway)` is on screen; highlight it (`highlight=use_occlusion`) while spoken. Also, "use occlusion" should be said as "the use-occlusion setting".
- Proposed:
  - "{@ highlight=store} Every cell packs its visibility for the eight probes that surround it, four bits each, sixteen levels, into one thirty-two bit word; {@ readout=class3} here the third probe sees this cell at ten parts in fifteen."
  - "{@ highlight=use_occlusion} Yet nothing reads them by default: the use-occlusion setting is off, and the pass still runs every time a cascade is built, a cost paid for nothing."
- Add a bridge sentence to ch7 at the end of c6-store or c6-sources: "The distance field and the occlusion are now ready; next, the light that goes into the voxels." Currently c6-sources ends flatly.

**Chapter 6 budget**: 35 s with 24 s of speech after the fixes grows to roughly 45 s (11 sentences); raise the budget to 50 s, or the extra sentences will overrun. The outline had 90 s.

## Top 5 changes

1. **c5-flood**: explain why a jump flood works (big steps carry seeds far, small steps correct, nearest means nearest to the cell) and light the `nearest` line; enlarge the slice inset and pace `floodStep` over the sentence. Sentence: "The step starts large and halves, thirty-two, sixteen, down to one, so a seed first jumps across the room, and then the small steps correct the cells it misled."
2. **Slice insets everywhere in ch5/ch6** are too small; enlarge them and keep the readout visible beside the code panel (c5-store, c5-cells). Cue: set the inset to the large size for `floodStep`/`occlusionShell` shots; do not let `panel=code` replace the readout.
3. **c4-facing and c4-emission show nothing**: colour voxels by facing bit; make an emitter visible with a non-zero emission readout. Sentence: "An emissive surface also writes its light. Red, green and blue share one exponent, which fits them in thirty bits; the last bits give a five-bit weight to each of the six faces, saying how much light leaves that way."
4. **c5-why, c5-seeds, c5-upscale have no change on screen** (identical stills, silence at 50 %): draw a sphere-trace circle stepping with the `a march reads` row changing; outline one 2x2x2 block and its seed; show what step 8 changes. Sentence for upscale: "A last pass with a step of one fixes the cells whose seed was off by a cell, an error that comes from having searched at half size, shown here as the cells that change."
5. **Ch6**: introduce probes and what the values mean, use the `occlusion` listing (`panel=code`), animate the eight classes in c6-store, highlight `use_occlusion` for the finding, and raise the budget to ~50 s; add the bridge to ch7. Sentence: "The same pass also asks, for each probe, how much of the room around it the probe can see, since a probe behind a wall should not pick up light from the other side."
