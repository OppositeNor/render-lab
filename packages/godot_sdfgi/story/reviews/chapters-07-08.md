# Review: chapters 7 and 8 (Light in the voxels, Probes trace the field)

Based on the storyboard, outline, listings (direct_light.py, probe_rays.py) and the contact sheets (three stills per beat, 25/50/85 %). I did not re-run the build. Stills are judged from the contact sheets, so small readout text was read at reduced size.

## Overall

- Narration is correct and terse, but both chapters read as a list of facts about code. Almost every beat after the first is a still picture with a different callout label: the same room covered in grey/white voxel dots (ch7) or the same sphere and a few orange rays (ch8). Few of the sweeps the outline promised are visible.
- Budget: ch7 has 16 narrated sentences (about 65-80 s at roughly 4-5 s each) against 135 s; ch8 has 12 (about 50-60 s) against 95 s. There is room for about 50 s in ch7 and 35 s in ch8 without padding. Spend it on the missing explanations below, not on slower delivery. The outline asked for 220 s each, so the compression has cost explanation.
- The chapter 6 to 7 bridge works ("The field is built; now the solid cells are lit, so the rays have something to find."). The 7 to 8 bridge ("the probes gather it for every point between them") is fine but never closes the loop: chapter 7 ends on "probes light the cells", chapter 8 never says the hit reads light that already contains the previous frame's probes.

## Chapter 7

### c7-cells
- "One thread runs for each solid cell in the list." Shows nothing: the voxels are uniform dots, and threads are not drawn. Depth is fine (what a thread is will be known to this audience), but "the list" has not been introduced as the compact list of solid cells from chapter 5.
- "By default a frame updates only every fourth cell, then the next quarter, and so on": lightPhase 0..3 is set per clause. The 85 % still shows readout "cells this frame 1/4: quarter 3" and the camera has swung to a top view, but the lit quarter is not distinguishable from the rest (all dots look alike). Highlighting: the cells updated this frame must be a different colour/size from the rest. At 25 % and 50 % the readout says "all" (lightPhase=4), which is fine as the opening state.
- The 25 % still carries a stale callout label from chapter 6 ("sdfvoxel_gi_process (occlusion)") during the bridge sentence. Clear the callout or give the sentence one.
- "A moving light lags up to four frames" is stated but not shown. Add a moving light and one cell whose brightness steps every four frames.
- Fix: a quarter of the dots lit bright, the rest dimmed, rotating with lightPhase; keep the camera still (the overhead swing also makes the near wall's voxels huge teal cylinders, which look odd).

### c7-omni
- Sentence 1 ("The extra omni light shows one cell's sum") is fine, but the readout is hidden: the beat shows the code panel in place of the readout, so the number that should change with lightHeight (attenuation, 0.27 at height 2.9) is never on screen. The light marker moves between stills (25 % low, 50 % and 85 % high), but the voxels do not change: nothing shows the cell getting dimmer.
- "one minus the fourth power of distance over range, squared, over the distance" is a formula read aloud; the highlight is on the attenuation line, good, but the three factors are not individually lit. A viewer will not grasp why a fourth power. Say what it does: it reaches exactly zero at the range, and the 1/d keeps it bright up close.
- The squashed-space sentence ("seems one and a half times higher") has no visual: the real 1.94 m versus squashed 2.90 m appears in the readout at c7-static (truncated "distance the shad.. 2.90 m (real 1.94 m)") but not here, where it is said. Move the readout here or show both heights as two markers.
- Fix: show readout and code side by side (or put the attenuation row in the callout label), colour the floor cell by its light, and sweep lightHeight with a needle moving on a small falloff curve.

### c7-shadow
- All three stills are nearly identical. No ray, no circles, no blocked/lit state is visible, and the lightX sweep ("behind") does not show: the light stays in the same place in the 25/50/85 % stills. This is the chapter's main promised animation (circles of each step, ray stops at a solid cell) and it is not on screen. Probably the shadowRay marker is not being drawn at this camera (cameraDrift 0.3 at far range) or it is hidden behind the code panel; check the still at full size.
- Depth: "stepping each time by the distance the field stores" is unexplained for a viewer who has not seen sphere tracing. One sentence: "the stored distance is the largest step that cannot skip over a surface".
- "Past the last cascade, it is lit" and "all or nothing, and each light is traced whatever its own shadow setting says" are two findings with no visual; the second is a bug-like quirk read at a speed no one can use. Consider moving it to the sources page or showing a light with shadows off still casting.
- Highlighting: the march line is lit in the code while named: good. The "hit" line (`d < 0.001`) is not lit when "reaches a solid cell" is said; add highlight=march,hit then lit on the last sentence.

### c7-faces
- The six-face anisotropy is the least explained concept here. "The light is summed for each face the voxel shows, then stored in a shared-exponent texture, with the faces' shares in two more." A new viewer does not know why a cell has faces (a voxel has no normal; light arriving from different sides must be kept separately, so a later ray hitting it from the left reads the left light), what a shared exponent is (one exponent for three colour channels), or why "shares" are stored (ratios so the second texture is a normalised weight per face).
- Visual: the three stills are identical, with only the code highlight. The outline promised six bars per face and a struct panel of two texels. Show a cube with its six face colours, the light coming from the omni lamp lighting only the facing sides, then the packed texels as a panel.
- The neighbour fill ("written into the empty neighbours that point back at the cell") has no picture and no reason stated except "blending near a surface stays lit". The reason: the probes and rays sample the texture with linear filtering, so a sample taken in the empty cell next to a wall would otherwise blend with black. Show a slice where the empty neighbour cells turn on and a smooth ramp without and with.
- Highlighting: highlight=faces,store lights both at once; the neighbours line is never lit when named. Add highlight=neighbours for the second sentence.

### c7-static
- One sentence for a concept ("static light") whose difference from dynamic has not been drawn: dynamic is recomputed each update, static was added once when the region was built. The beat shows no change between stills (all three identical, readout "static light after scrolls 0.094 (1.0x)").
- Readout rows are truncated ("distance the shad.. 2.90 m", "attenua... (1 - (d/5)^4)^2 / d = 0.2705"). Widen the label column or shorten labels.
- Fix: say what the user does ("a light marked static is baked into the cell when its region is voxelized, and is not traced again") and show the cell's stored light as a bar split in 'static' and 'dynamic'.

### c7-double
- This is the best beat in the chapter: the readout visibly goes 0.094 (1.0x) to 0.378 (4.0x) across the stills (25 %: 1.0x, 50 %: 1.0x, 85 %: 4.0x). But the sweep happens late (the last clause), and the middle still, at about half the beat, is still 1.0x, so the viewer sees "1x ... 4x" only in the final second. Start the sweep during the first sentence ("After a scroll...") and hold 2x, 3x for a beat each.
- The sentence "back into the new list" is unclear; "list" again. Replace by "the cell copies its stored light into the new grid".
- The finding ("measured on the lab's model", "read from the code") is fine; add "this is a bug in the engine" in plain words so a viewer knows it is not by design. A visual with a bar stacking up each scroll would beat the number in a corner.
- The big caption is correct. Do not leave "scrolls=0" in the stills of c7-static and c7-double identical (same params at 25 %).

### c7-bounce
- Why the bounce works across frames is not explained. "That light came from the previous frame's probes, so every frame adds one more bounce." A viewer needs the loop drawn: cells -> probes (chapter 8) -> cells -> probes..., each pass one bounce, which is why the room brightens over frames. Nothing on screen: the three stills are identical, no probes, no readout of the bounce term, no bounceFeedback sweep (outline: 0 -> 0.5 -> 1.5). The readout shows only "static light after scrolls" and "brightest cell 1.078".
- "bounce feedback, a half by default" is said without showing what changes. Sweep bounceFeedback 0 -> 0.5 -> 1.0, show a wall cell's number and the colour bleed from the red wall onto the floor.
- Fix sentence: "Each cell also reads the probes around it, takes their light times its own colour and times the bounce feedback, half by default, and adds it; the probes were filled last frame, so each frame the light travels one bounce further, and after a few frames the red wall tints the floor."

### c7-sources
- The source card overflows: the last paragraph is cut/overlaps the subtitle ("These are the light pass's lines" printed on top of the card text). Shorten the entries or drop `sources:` entries that are not needed on screen (the page lists eleven keys; only five fit).

## Chapter 8

### c8-rays
- Strongest beat: rays accumulate across rayFrames (25 %: a few, 50 %: more, 85 %: a full sphere), so "fill in two hundred forty directions" lands. However, `rayFrames=1` is set at the start while the 25 % still already shows a couple of rays; the first sentence ("each probe traces eight rays") should be the eight-ray state, then the burst.
- "Because thirty is even, each frame's eight rays all point into the same half of the sphere" points at nothing: the 85 % still is a complete sphere of rays while that sentence is spoken, which contradicts it. Show one frame (eight rays) coloured by its frame parity (blue frames -z, orange +z), or step rayFrames 1, 2, 3 for that sentence and let the viewer see the halves alternate. Also say what it costs: the spiral is not evenly spread frame to frame, so the lighting flickers by half-sphere, a visible consequence, if this is the point of the finding. If it is not visible on the lab's model, cut the sentence.
- Why the ray count is 8 and not 240 is said in chapter 1; repeat briefly: "so the work per frame is small, and the history carries the rest".
- The readout shows "frames 60 / 30" and "rays a frame 8 (240 over the history)": 60 of 30 is confusing; the probe's frame counter is above the history length. Show the ring position (frame mod 30) instead.
- First still carries a stale callout (chapter 7's "MODE_PROCESS_DYNAMIC, bounce") during the bridge. It does, however, usefully show what was just described; consider keeping it and saying so.
- "Vogel spiral" is never explained: "evenly spaced points on a sphere, like a sunflower, rotated per probe so that no two probes shoot the same directions" (the listing has `rotation`; neither is mentioned).

### c8-march
- The marching circles are drawn but are tiny rings around the probe at this camera distance (cameraDrift 0.3 and the shot is as wide as c8-rays); in the three stills (raySteps 0, then to 30) they are barely distinguishable, so the sweep does not read. Zoom in on the one ray (camera near the probe, ray lit in white, circles larger and fading) and slow raySteps 0..30 across the sentence. The ray must travel visibly: the stills show the ring pattern the same at 25 %, 50 % and 85 %.
- "Within a twentieth of a cell" is a threshold with nothing to see; show the last circle touching the surface (contrast with chapter 7's 0.001).
- "When it leaves the first cascade, it carries on through the next one" has no visual; draw the cascade box and the ray crossing it, with the next circles bigger (a coarser field has larger steps). This is also a good place to say the ray is limited by the last cascade.
- Depth: one sentence that this is the same step as the shadow ray, only the stop condition differs, saves explaining twice and ties the chapters together.

### c8-hit
- One sentence for three ideas: normal from the field's slope, face weights, return light. Nothing is shown (the stills have just a probe and a couple of rays; no hit marker). A viewer will not understand "the normal comes from the field's slope" (the distance field increases away from the surface, so its gradient points out of it) nor "how much of each face's light" (links back to c7-faces, the six faces).
- Fix: three short sentences, with the hit point drawn as a small cube showing its six face colours, the normal as an arrow, and weights as bars; read out "light x weights = result". Say the connection: "the light this ray brings back is the light chapter 7 stored for that voxel, including last frame's bounce". That closes the loop of c7-bounce.
- Highlight: the beat has no code panel; use probe-rays with highlight=hit, which is the line `light = light_tex(hit) * aniso(hit, normal)`.

### c8-miss
- "A ray that escapes reads the sky, at its second mip." Why mip 2: it is blurred, so eight rays per frame give a smooth result instead of noise from a few sharp sky pixels. Not said; add it.
- The second sentence (4.7 stable shader built for a single sky texture while the desktop default uses an array) is jargon, a tangent, and has no visual. A viewer cannot act on it. Either cut to the sources page or say "in this release the sky lookup can read the wrong kind of texture, so check the beta" in a sentence and show the callout.
- Stills: at 50 % and 85 % the callout reads "gi.cpp - GI::init (integrate defines)" and the picture is the same fan of rays. Show a ray hitting the sky side with a sample of the sky colour at its end, and rays hitting walls in another colour.

### c8-sh
- Biggest depth gap. For a graphics-literate viewer: why spherical harmonics (a compact way of storing light arriving from all directions, so one probe stores a whole sphere as a few numbers), why 16 (four bands: 1+3+5+7), what "spread over" means (each ray adds its colour times the basis functions evaluated in its direction), why integers (the ring sum below needs exact add and subtract, floats would drift, and the textures are integer), and why 1024 and "x 4 / ray count" (the normalisation). None is said, and the visual is only the code panel and the same sphere; no coefficient readout (outline: 16 rows growing as rays add).
- Fix: show the 16 coefficients as a growing bar group per colour channel while rays are added (rayFrames 1 -> 8), then the same numbers multiplied by 1024 and rounded.
- Highlighting: highlight=sh,fixed lights both lines at once; split by clause (sh for the first, fixed for the second). The two clauses are split across stills but the highlight does not change between them.

### c8-history
- "The last thirty frames are kept, and a running sum subtracts the oldest and adds the newest": a viewer understands the mechanism only if the ring is drawn. panel=values shows readout numbers (LO, irradiance), not a ring. Show 30 slots, the write head moving, the oldest slot leaving. The `HistoryInset` the outline names is not used.
- "So a probe brightens in a straight line for thirty frames, then holds" is the point of the beat but the stills only show the endpoints (0.000 at 25 % and 50 %, converged at 85 %; the sweep happens in the last moments). Show a graph of irradiance against frame with the straight line. Add the reason: each frame replaces one slot of thirty, so the sum rises by 1/30 of the final amount per frame, from zero. Also state the consequence for the viewer: a moved light takes thirty frames to show up in the probes (at 60 fps half a second), and a scrolled-in probe starts dark (chapter 9 handles the parent's seed).
- Readouts: the converged value at 30/30 (0.221 0.233 0.306) is not the 60/30 value of the first beats (0.232 0.239 0.311). The viewer could notice that "converged" moved. Explain (more frames, more bounce) or make the readouts agree.
- Truncated row "irradiance, toward th..." in every beat of the chapter; widen the label.

### c8-sources
- Fits, but the card lists three of the seven keys; fine. Check that the last paragraph does not clip.

## Cross-cutting visual problems
- Voxel colours: in chapter 7, the floor/dot colours (dark blue-grey on a dark floor) have low contrast, and all voxels look the same whether lit or not. Colour voxels by light (not by albedo) in this chapter so every sweep shows. The near wall at the overhead camera swing (c7-cells 85 %) is huge teal cylinders; keep the camera behind the wall.
- Readout truncation in both chapters (`distance the shad..`, `attenua...`, `irradiance, toward th...`).
- Code panel hides the readout (c7-omni to c7-faces): the readouts that carry the numbers are replaced. Keep both, or put the number in the callout.
- The circles of the marches are tiny (ch8) or absent (ch7); both need a closer camera.
- Callouts from the previous beat bleed into the bridge sentence (ch7 first, ch8 first).
- Three consecutive beats in ch7 (shadow, faces, static) have identical stills: an audience will feel nothing moved for about 40 s.

## Five most important changes

1. **c7-shadow / c8-march: make the marches visible.** The circles and the stop are the chapter's promised animation. Close camera on one ray, white ray, circles of each step, lightX sweep across the sentence. Replace: "A ray starts at the cell and steps toward the light; each step is as long as the distance the field stores, the longest step that cannot jump over a surface." and for ch8: "A probe's ray does the same march, one circle per step, and stops within a twentieth of a cell of a surface." Cue: `{@ camera close, raySteps=0..30 }`, `highlight=march,hit`.
2. **c7-faces / c8-hit: explain the six faces and show them.** Replace c7-faces: "A voxel has no normal, so each cell keeps light for each of the six faces it shows, as a colour plus six shares; a ray that hits it from the left later reads the left light." Add: "Empty cells next to a wall get a copy, so smooth blending next to a surface does not fade to black." Add a cube with six coloured faces and the neighbour fill in a slice; light neighbours in the code. Replace c8-hit: "At a hit, the field's slope gives the surface's direction; it picks how much of each face's light to take, and the ray brings back that light, which includes last frame's bounce."
3. **c7-bounce: draw the loop and sweep bounceFeedback.** Replace: "Each cell reads the probes around it, and adds their light times its own colour times the bounce feedback, half by default. The probes were filled from these same cells a frame earlier, so each frame the light travels one bounce further." Cue: `bounceFeedback=0..0.5..1` with a wall cell readout, and a loop diagram cells -> probes -> cells.
4. **c8-sh / c8-history: explain SH, fixed point, and the ring.** Replace c8-sh: "Each ray's light is added into sixteen coefficients, a compact way to store light from every direction, then scaled by one thousand twenty-four and kept as integers, so the history can be added and subtracted exactly." Replace c8-history: "The probe keeps thirty frames; each frame it subtracts the oldest and adds the newest, so a probe brightens by a thirtieth per frame, a straight line, and a moved light takes thirty frames to show." Cue: HistoryInset of 30 slots plus an irradiance-vs-frame graph over `frames=0..30` starting at the first clause; split highlight `sh` then `fixed`.
5. **Fix c8-rays half-sphere and c8-miss, and cut tangents.** Show one frame's eight rays coloured by parity (rayFrames 1, 2, 3 stepping) while saying: "Because thirty is even, each frame's eight rays all point into one half of the sphere, and the halves alternate." (or cut it if not visible). For c8-miss: "A ray that leaves the field reads the sky, blurred to its second mip so that a few rays give a smooth colour." Move the 4.7-stable texture mismatch to the sources page. Also spend the spare budget: about 50 s in ch7 and 35 s in ch8 allow these extra sentences, plus a lag sentence in c7-cells ("so a moving light takes up to four frames to reach a cell") with a moving light on screen, and a sweep of scrolls starting in the first sentence of c7-double.
