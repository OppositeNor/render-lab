# Review: chapters 9, 10, 11 (and the end of 8)

Read: storyboard.story, outline.md, probe_store.py, screen_pass.py, research.md (F4, F5, 8.x, 9.x), and the contact sheets chapter-09/10/11.png plus ch09/s_001.png.
Stills are not mentioned beat by beat unless a problem was visible. Budget: sentences x about 4 s.

## Overall

- The chapters are accurate against research.md, but they are thin: ch9 has 9 sentences for a 65 s budget (about 36 s of speech), ch10 has 13 for 100 s (about 52 s), ch11 has 5 for 30 s (about 20 s). The outline gave 140 / 210 / 70 s. There is room to explain the things the viewer cannot guess; the budget is not the constraint.
- Several beats are static: the stills of c9-gain, c9-texel, c10-pass, c10-specular, c10-others are nearly identical to their neighbours. The animation that exists is the pixelZ sweep (works) and sdfgiMix sweep (barely visible).
- Terms used but never defined: octahedral map, irradiance vs radiance, l_mult (only appears in code/sources), the half texel, normal bias (in the code panel but never in a sentence), probe units.

## Bridge from chapter 8

c8-history ends "...for thirty frames, then holds." Then c9 opens "Sixteen numbers per probe are not something a pixel can sample". Good bridge, but ch8 never says the sixteen numbers are the SH of the *averaged* history, nor that they describe light from all directions. Add to c9-oct opening: "The ring of thirty frames gives each probe one averaged set of sixteen numbers: the light arriving from every direction."

## Chapter 9

### c9-oct
- S1 "Sixteen numbers per probe are not something a pixel can sample, so this step makes maps it can." OK, but add why: a pixel needs the light for one normal, quickly, with hardware filtering.
- S2 "Each probe gets two maps of six by six texels, its directions unfolded onto an octahedron: irradiance, and radiance." Depth gap: a graphics-literate viewer knows octahedral encoding only if told. Irradiance vs radiance is named but not explained. Say: irradiance = light already averaged over the hemisphere around a normal (for diffuse); radiance = light arriving from one direction (for reflections). "Unfolded onto an octahedron" is jargon: say "the sphere of directions is folded flat into a square, so one texel is one direction".
- S3 border: fine, but "past the map" is vague; say it is for bilinear taps at the edge, and that the border texels are copies from the mirrored position across the fold, which is why the seam does not show.
- Visual (ch09/s_001.png): the inset is a blurry colour smear; you cannot see 6x6 texels, the border, or two maps. Only one image appears under the label "irradiance | radiance", the caption reads "u (two maps)" and there is a stray ">" at the left (an axis glyph?). The left half of the inset (above the second map) is empty dark. The inset also covers the sphere and the marker's probe context, and stays the same through the whole chapter (it does not unfold, highlight a texel, or show the border). Nothing on screen shows "unfolded onto an octahedron".
- Change: draw both maps side by side as nearest-filtered 6x6 grids (8x8 with the border outlined), with the sphere-to-square fold as a short animated range ({@fold> ...}) or at least a labelled direction arrow (+y at the centre, -y at the corners). Make "irradiance" and "radiance" labels sit over their own maps. Move the inset off the sphere.
- Highlighting: the callout oct-store is the only cue; add `panel=code code=probe-store highlight=direction` here for "its directions", and `highlight=irradiance` / `highlight=radiance` when each map is named. Today the code panel first appears one beat later.
- Replacement for S2: "Each probe gets two small maps, six by six texels, where each texel is one direction: the sphere of directions folded flat into a square, an octahedral map. One holds irradiance, the light averaged for a surface facing that way; the other holds radiance, the light coming from that way."

### c9-gain
- "A uniform sky comes out four over pi too bright": the headline says "four over pi too bright" (127 % brighter?), the sentence says "about a quarter too bright". 4/pi = 1.273 is 27 % more, so "a quarter" is right and the headline is wrong or at least ambiguous ("four over pi times too bright"). Fix the headline: "A uniform sky comes out four over pi times too bright" or "27 % too bright".
- Depth: the sentence says "a four where four pi belongs" but the viewer is never told what the four pi is (the solid angle of the sphere, the normaliser of the SH sum), nor where l_mult enters (the cosine-lobe weights 1, 2/3, 1/4, which the code shows but the voice never mentions). A viewer cannot see why a uniform sky should give 1.0. Also "summed" is wrong in detail: the sum is an average over rays (the 4/N), the "4" is the STORE multiplier.
- Intuition: nothing animates. The panel's red line "a uniform sky stores 4/pi = 1.273" has been on screen since c9-oct (it appears from the first still), so it gives the punchline away a beat early, and the line "irradiance toward th..." is truncated in the readout.
- Proposed cue: a short two-state readout, `skyOnly=1` (sky radiance 1.00, expected irradiance 1.00, stored 1.27) fading in at this beat; hide the 4/pi row in c9-oct. Keep `highlight=gain` and add `highlight=irradiance` for the "times l_mult".
- Replacement: "Here the sum uses a four where the sphere's four pi belongs, so under a sky of brightness one, the stored irradiance is one point two seven, about a quarter too bright, read from the code and measured on the lab's model."

### c9-texel
- Sentence: "Each texel's direction is taken at its corner, but the reader samples at its centre, so lookups blend two texels, read from the code." Likely backwards or at least confusing: research 8.07 says the *writer* stores direction u=k/6 in texel k (whose centre is k+0.5) and the *reader* samples at a texel corner, so the lookup blends texels k-1 and k. The sentence says the reader samples at the centre, the opposite (the headline says the same). Fix: "written for the corner, read at the corner of the texel, which is a boundary between two texels".
- Depth: "half texel" never says what it costs: the map is shifted by half a texel (1/12 of the map, a small, low-frequency error). Also should link to the border of c9-oct (this is why the border exists).
- Intuition: no still differs from the previous beat. The outline planned an inset zoom (texel k, sample position, 50/50 blend). Not built. Without it the sentence points at nothing.
- Proposed cue: zoom the inset to a row of texels, a marker at the texel centre (where the direction was computed? no: where the writer assumes it is, k/6, at the left edge of texel k) and a second marker at the reader's sample, with the blend 50/50 shown as two bars; use `{@ ... highlight=direction}` (already there) and a range for the marker.
- Replacement: "The writer computes texel k's direction as if k sat at its left edge, but the texel's centre is half a texel further, so a lookup lands between two texels and returns their blend: the map is shifted by half a texel, a small error, read from the code."

### c9-scroll
- Different demo (cascades, outdoor environment) with the room tiny and the probes (small dots, hard to see) at 30 m. Probe history "moves with it" is not shown: no probes highlighted, no scroll animation, `cameraDrift` alone. Cascade 0 readout says DIRTY_ALL, camera x 0.00: no scroll happens, so S1 and S2 point at nothing. Chapter 2's scroll is not recalled.
- S1 "history moves with it" is vague: say each probe's thirty-frame history is copied to its new slot (so stored light is not recomputed), S2 "borrow the parent's": which parent, how (a trilinear average of cascade 1's map, a one-frame seed, so it is correct in the average before its own rays arrive). Also say why: otherwise the new edge probes would fade up from black over thirty frames, flickering.
- Change: add a camera move (`camera.x` sweep by 4 cells to trigger a jump, as in chapter 2), highlight the uncovered slab of probes, and show the arrow from cascade 1.
- Replacement: "When a cascade scrolls, each probe keeps its thirty frames of history and simply moves to its new slot; a probe the jump uncovers is seeded from the coarser cascade's light at its position, so the edge does not start black."

### c9-sources: fine ("These are the probe map lines.").

## Chapter 10

### c10-pass
- S1 "Every probe now has its maps; the last pass turns them into each pixel's light." OK.
- S2 "After the depth pre-pass, a compute pass rebuilds each pixel's position and normal." The viewer is not told why rebuild (the G-buffer holds depth and a packed normal), nor what "each pixel" is in the lab. The outline asked for the reconstructed position and normal to be drawn: stills show a tiny pixel marker (a few pixels wide, near the wall) and a readout; no normal arrow, no y_mult step (the squash of chapter 2 appears only in code, never in a sentence).
- Pixel marker: far too small to read at 1080p, and it moves with the camera drift so the viewer loses it; the readout (cascade 0, blend 0.000, point 6.26 6.84 7.75, diffuse, reflection) is never explained in a sentence ("probe units").
- Change: enlarge the marker, draw the normal as an arrow, a ring around the pixel, and name the readout rows in the voice.
- Replacement: "After the depth pre-pass, a compute pass runs once per pixel: from the depth and the stored normal it rebuilds the point's position and normal, and squashes their height by the y scale, so they match the grid."
- Footer chip says `sdfgi_integrate.glsl main (MODE_SCROLL, parent)` in the first still (25 %): a leftover from c9-scroll while the sentence is already c10. Fix the label at the cut (fade timing).

### c10-cascade
- S1 fine. S2 "between five and a half and seven and a half probes from the camera, it blends toward the next" gives numbers without a picture. Stills: `cascadesShown=2` produces no visible boxes; the blend stays 0.000 in all three stills; the outline's `pixelDistance` sweep through the band was dropped. "Blends toward the next" does not say what is blended (the finished diffuse and specular, not the maps) or why (to hide the seam where resolution halves). "From the camera" is per-axis (max of |x|,|y|,|z| in probe units), not distance.
- Change: show cascade 0 and 1 boxes, plus a range sweeping the pixel (add or reuse `pixelZ` or camera distance) from inside the box through the band with the readout "blend 0.00 -> 1.00" and the weights of cascades 0 and 1.
- Replacement for S2: "Near the box's edge, from five and a half to seven and a half probes out along any axis, the result is blended toward the next cascade, so the step to coarser probes does not show."
- Highlighting: panel highlights `blend` correctly; `cascade` highlight missing on S1 (no code panel on S1 at all).

### c10-probes
- Most important beat of the chapter, with the least explanation. S1: "weighted by their distance, and by how far they face the normal". Missing: the eight are the corners of the probe cell containing the point; distance weight is trilinear (a tent); the facing term max(0.005, dot(normal, direction to probe)) so a probe behind the wall gets nearly zero; the weights are then normalised, so they sum to 1; the normal bias of 1.1 cells moves the point off the wall first, so it does not select cells behind it. Normal bias is only in the code panel (not highlighted: `highlight=weight` only), never in a sentence.
- Stills show weights as labels (1 %, 70 %, 0 %, 17 %, 9 %, 3 %) with lines to the probes: good. Problems: the labels overlap the sphere and each other at the 25 % still ("0 %" and "17 %" over the sphere; the big yellow circle (70 %) is the one probe but the highlighted probe is not otherwise distinguished); the normal and the bias offset are not drawn; "0 %" labels clutter. The 85 % still shows a different camera framing and labels 0 %, 8 %, 1 %: only three probes visible, so five weights are lost off-frame; the sweep ends at pixelZ=1.4 and the pixel leaves the cell.
- The sweep sentence "As the point slides along the wall, the weights move between the probes" is the best animation in the three chapters, but the pixel's own position (the thing sliding) is not visible; only the labels change. Add a marker and a short trail.
- Replacement for S1: "The pixel's point is nudged off the wall along its normal, then the eight probes at the corners of its cell are weighted: by closeness, a tent in each axis, and by facing, so a probe behind the surface counts almost nothing; the weights are normalised to one."
- Add a sentence: "That nudge, the normal bias, is a bit over one cell, and it is why a thin wall does not leak light from behind it" only if checked (research 9.09: bias 1.1 cells; the leak claim is a consequence; keep it hedged or drop it).
- Add `highlight=bias` on the code panel for that sentence.

### c10-specular
- Four stills show the same frame; the readout shows tiny changes in the digits only. S1 and S2 point at nothing: no roughness change, no reflected direction, no radiance map. The outline planned a `roughness` 1 -> 0 sweep with softness circles; not present. The room is "rougher than that" so the march is never shown: the viewer is told about a feature that is not seen.
- Depth: "radiance maps along the reflected direction" (needs "reflect the view direction about the normal"), "blending toward irradiance as roughness grows" (the mix factor, up to 0.2? research 9.11-9.16: irradiance mixed above 0.2). If irradiance is "the light for a normal", then why does a rough reflection read it? Not said.
- Change: add a roughness sweep on the sphere (`roughness=0.5..0.1`), the reflected direction arrow, and the sentence on the march as a short beat.
- Replacement: "A shiny surface looks up its radiance map along the mirror direction; as the surface gets rougher the lookup blends toward the irradiance map, a blurred average."

### c10-compose
- S1 is dense: ambient replaces the sky's ambient (what is that?), is multiplied by albedo, plus reflection. Not said that blending is by an alpha the pass writes (outside all cascades alpha is 0 and the sky's own ambient stays). The sky's ambient is not introduced anywhere in the video as the thing GI replaces.
- Sweeps: sdfgiMix 1 -> 0 -> 1. Stills show 1.00, 1.00, 0.85: the "off" sweep in the first third barely changes the picture at 85 % (0.85), and the sentences about "without it, the room is lit only by the panel" are not visible in the stills; the first half of the beat is spent at 1.00. Compare against "the panel": which panel (the light) has not been introduced in this chapter; say "the room's one lamp". The sweep is too gentle; make it hold at 0 for a full sentence.
- Change: two-column split screen or a hold at 0 and 1 with the readout labelled "SDFGI off / on".
- Replacement for S3: "Without it, only the lamp's direct light reaches the walls, and the shadows are black."

### c10-others
- Both sentences point at nothing: stills show the room unchanged; no transparent object, no fog. Footer chip changes only. S2 "samples them at the wrong height" has no picture (the outline planned two markers). F11 (energy ignored) is said but not shown.
- Depth: do the transparents lack the reflection march because they cannot write the GI buffers (forward shading)? Not said. "Energy setting" appears without introduction (earlier in ch1?).
- Replacement for S2: "Volumetric fog reads the probes' average light too, but forgets the y scale, so it looks up the probes at the wrong height; two markers show where it should have looked and where it did."

### c10-sources: fine.

## Chapter 11

### c11-recap
- All three stills show the same diagram with no node visibly lit: every box has the same teal outline. The `focus:` line lists UPD, VOX, LIGHT, PRE, STORE, GI, but nothing visibly walks the order; the viewer cannot tell which node the sentence is naming. Check that callouts light their node; if the focus key only dims others, all six are focused at once.
- Sentences: S1 "The cascades follow the camera, and the slab each jump uncovers is voxelized into a distance field." covers two boxes (cascades/culling, voxels) at once; S2 "lit, a quarter each frame, with last frame's probes as the bounce" fine; S3 "The probes trace eight rays a frame and average thirty frames." fine, but it covers the depth pre-pass box without saying the timing; S4 "Their maps are stored, and every pixel reads the eight probes around it." puts two boxes (Probe maps, Screen pass) in one sentence and loses the cascade choice, the weights, and the compose with the sky (the end of the chapter 10 ideas). Omits occlusion (chapter 6) entirely, and the two findings (4/pi gain, half texel) and the other findings of the video; a closing recap normally restates the main caveats.
- Fit: 5 sentences, about 20 s of 30 s; there are 10 s to add the last box's sentence and a caveat line.
- Change: one cue per box, with a `{@ focus=...}` or a range so each node is lit as it is named; add a sentence for "Probe maps" separately from "Screen pass".
- Replacement S4/S5: "Then each probe's sixteen numbers are stored as two small octahedral maps. Last, every pixel finds its cascade, weights the eight probes around it, and the result replaces the sky's ambient."
- Closing: "That is SDFGI, read from Godot's source." Consider: "That is SDFGI, read from Godot's source; the findings along the way, such as the four over pi gain, are what the code says, not what was run in Godot."

## The 5 most important changes

1. c9-texel (and headline): the sentence says the reader samples at the texel centre; research 8.07 says the writer's direction is for the corner k/6 and the reader samples at a corner. Rewrite: "The writer computes texel k's direction as if k sat at its left edge, but the texel's centre is half a texel further, so a lookup lands between two texels and returns their blend: a half-texel shift, read from the code." Add a zoomed inset with the two markers and the 50/50 blend.
2. c9-oct: fix the octahedral inset (blurry smear, one image under a two-map label, a stray ">", covers the sphere) and define the terms. Replacement: "Each probe gets two small maps, six by six texels, where each texel is one direction: the sphere of directions folded flat into a square. One map holds irradiance, the light averaged for a surface facing that way; the other holds radiance, the light coming from that way."
3. c10-probes: explain the eight-probe weighting fully and show the normal bias: "The point is nudged off the wall along its normal, then the eight probes at its cell's corners are weighted by closeness and by facing, so a probe behind the surface counts almost nothing; the weights are normalised." Add `highlight=bias`, a normal arrow, a visible pixel marker, and keep all eight probes in frame (the 85 % still loses five).
4. c10-specular, c10-others, c10-cascade: three beats whose sentences point at nothing (no roughness sweep, no transparents or fog, no cascade boxes, blend stays 0.000). Add the roughness sweep (`roughness=0.5..0.1`) with the reflected direction, a pixel sweep through the 5.5 to 7.5 band with the blend readout, and the two fog markers; otherwise cut the sentences.
5. c11-recap: no node visibly lights in the diagram, so it does not walk the frame. Give each box its own lit cue and sentence (split "Probe maps" from "Screen pass"), e.g. "Then each probe's sixteen numbers are stored as two small octahedral maps. Last, every pixel finds its cascade, weights the eight probes around it, and the result replaces the sky's ambient." Also c9-gain: fix the headline ("four over pi times too bright") and hide the "4/pi = 1.273" readout row until this beat; define four pi (the sphere's solid angle) in the sentence.
