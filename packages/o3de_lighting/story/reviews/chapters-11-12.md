# Review: chapters 11 and 12

Timings from the timeline captions: chapter 11 runs about 204 s against its 220 s budget, chapter 12 about 195 s against 210 s. Both fit. I did not find the "natural" log lines, so the 1.2x check is approximate.

## Chapter 11 · The sun's cascades

### Narration that disagrees with the drawing or the code (fix first)

1. **c11-fit, "A sphere does not turn, so the boxes hold still as the observer looks around" (observerYaw 0 to 30) is wrong, and the drawing shows it.**
   - `cascades()` sets `center = observer.position + forward * depth`, so turning the observer swings every sphere centre and box sideways. By about 0.5 m per metre of centre depth at 30 degrees, that is over a metre for cascade 2.
   - Only the radius, and so the texel size, is rotation-invariant. That is why the sphere is used.
   - The point of snapping is that the box then moves in whole-texel steps, so a shadow edge never lands between texels and does not shimmer.
   - Fix the sentence to: "A sphere keeps its radius when the camera turns, so every texel keeps its size; the box slides, but only in whole texels, so a shadow's edge stays put on the map."
   - Add a readout row for the box's min corner in texels ("lo / texel": integers) so the sweep shows the steps. At 1.4 mm per texel, nothing in the picture can show the snap.
2. **c11-fit, "centred where the distances to its near and far corners are equal" holds for cascade 3 only.**
   - `slice_sphere` clamps: if the equidistant centre `(dn+df)/2*k` lies beyond the far plane, the centre becomes the far face's centre and the radius is its half diagonal.
   - With k = 2.39, cascades 0, 1 and 2 (0.1-0.61, 0.61-1.59, 1.59-4.17 m) are all clamped. Their circles are centred on the far ring, not mid-slice. A viewer checking the sentence against the drawing will see a mismatch.
   - Fix: "Each slice gets a bounding sphere, centred where its corners are equally far, but no farther than the slice's far face; for thin slices that puts the centre on the far face."
   - Add the clamp line to `update_cascades` in `cascade.py` (`# @sphere`) so the highlight covers it.
3. **c11-fit, "stretched toward the light to catch the casters between it and the view, and behind the camera" shows nothing.**
   - The boxes are drawn in the cascade colour at 0.7 alpha, twelve edges, and overlap with the sphere circles and frustum rings. The stretch cannot be told from the cube.
   - Fix: on this sentence, set `cascadeSpheres=0.15`, show only one cascade's box (needs `cascadeBoxes` per cascade, or `cascadesShown=3` with a cascade-index param), and add a faint "cube without stretch" outline, or a label "casters".
   - Also drop "behind the camera". In the code it is `camera light-y - far`, a distance along the light, and it reads as "behind the camera" only by accident.

### Depth

- **The reason for cascades is never given.** c11-splits says what is done, not why. Add a first sentence to c11-splits: "One map of a thousand and twenty-four texels over the whole view would be far too coarse near the camera, so the view is cut into slices, each with a map of its own." The texel readout (1.4, 3.7, 9.6, 27.6 mm) then pays this off.
- **Give the default's scale.** At the default shadow far of 100 m the last cascade has R about 118 m, so a texel is about 23 cm. Say that in c11-fit's last sentence, instead of only "millimetres close by, centimetres far away" for the twelve-metre room.
- **c11-select puts the lookup in the wrong place first.**
  - It says "To find its shadow, a pixel first moves along its normal...", then reverses with "Opaque pixels never sample the maps in the forward pass".
  - Reorder: the fullscreen-shadow sentences first ("A full-screen pass, before the forward pass, writes each pixel's visibility into a target; for each pixel it does this:"), then the offset and select sentences, then the blur and R8 target.
  - That is also the outline's c11-fullscreen beat, which the storyboard folded away.
- **"By the map's bounds, not by the pixel's depth" is stated but never shown.** Nothing changes on screen between cascade choices. Best fix: draw a grid of floor dots coloured by `select()` (a cascade debug view; the CPU function exists). With the dots on, sweep `observerYaw` 0 to 35 (it already has a wave) and `shadowFar` 12 to 6 so the colour boundaries move, and point out a dot that sits at a greater depth than another yet picks a lower cascade (bounds).
- **The readout sentence in c11-select ("which cascade the inspected point lands in, and its texel") does not light the row.** Add `param.formula` or a row highlight for "the point's cascade", the only row that is new in that beat.
- The "angular diameter" sentence says "a finding" with no visual. The sun disc could be drawn as a cone of half a degree on the light arrow, crossed out when the specular call uses the plain direction. As it is, three of c11-light's five sentences (lux, angular size, one shadowed light) have nothing on screen but the room and a label.

### Intuition and highlighting

- The first beat (43 s) has no cascade graphics. It is four facts read over a drifting camera. Move "The lab has no directional light, so this chapter draws O3DE's arithmetic..." to be the second sentence, so the viewer knows the following pictures are a schematic; the panel's "Schematic: the lab renders no shadow maps." line is easy to miss. Consider shrinking c11-light to about 25 s: lux and one-shadow-light stay, angular diameter goes to the sources beat or a line with a cue.
- **Scale of the pictures.** The cascade 3 slice (4.17-12 m) and its ring run off screen. Its label never appears (labels are placed at the far ring's top edge and clipped). Cascade 3's sphere (R 14 m) fills the frame once spheres are on. In c11-splits and c11-fit use `camera.distance` about 15-16: through `[[22,36,16],[48,30,15],[30,38,15.5]]`. Also show spheres one at a time: `cascadeSpheres` driven per cascade, 0 then 1 with a cue per slice, instead of all four at once.
- **Label clutter.** The inspected point's label "cascade 2" sits beside the slice label "cascade 1 · 1.59 m" and is easily taken for a slice label. Rename the point's label ("this point: cascade 2" in the selected colour) or draw it only in c11-select.
- Slice naming: labels say "cascade 2 · 4.17 m" at the far end, but there is no near boundary; "cascade 0 · to 0.61 m" would read as a range.
- The source tag at the foot of the viewport is clipped by the right panel in nearly every frame ("::UpdateBorde", "CascadeShadowCameraCon"). The values panel's title "DirectionalLightFeatureProces..." is truncated as well. Either shorten the callout labels or widen the tag.

### Change

- The splits sweep is good: ratio 0, 1 and 0.9 with a lit `d_i` formula term, and `shadowFar=40`. But at the end "every slice grows with it" is narrated while the blend runs three seconds, so the stills catch 13.9 m. Give the sweep a longer blend (4 s) and hold at 40 for a second with the readout visible; the texel row should be lit then, since the sentence is about texels.
- Missing sweep: the sun direction. The boxes and light-space stretch depend on it, and `sunYaw`/`sunPitch` exist. A slow `sunYaw` wave in c11-fit would show the boxes rotating with the light while the spheres stay.

### Flow

- Opening sentence: "Point lights reach a few metres; ..." follows chapter 10 (transmission, ending on the thin-object finding). The bridge is one chapter late. Better: "So far every light was local; the one that lights everything, the sun, needs shadows, and those take a chapter."
- "its shadows are the next part", but the next two sentences are lux and angular size. Say "the sun has two numbers worth knowing before its shadows."
- c11-others: three sentences, 28 s, about lights the lab does not render, with a panel (the point light readout) that is unrelated to area lights. Either cut it to one sentence and fold into chapter 2's culling discussion, or show the area-light shapes (four icons). "Five kinds of area light" but only quad, disk, capsule, polygon are named (sphere is not). Say "sphere, disk, capsule, quad and polygon lights".
- c11-sources: 6.5 s, eleven entries, only four visible and they are lux, angular, one-shadow-light, defaults. The narration ("the feature processor for the split, the fit, and the snap, then the shadow calculator") names entries that are off screen. Reorder `sources:` so the cascade entries (`cascade-splits`, `cascade-sphere-fit`, `cascade-snap`, `cascade-select`) come first, and lengthen the beat to about 10 s.

### Fit

Near budget (204 of 220 s). Sentence-level adds above (about +15 s) fit if c11-light and c11-others are trimmed by about 20 s.

## Chapter 12 · The global skylight

### Highlighting and intuition

1. **c12-cosine, "Seen alone, the diffuse light from the sky is soft, and coloured by the ground below and the blue above" points at a black sphere.**
   - The `ibl` demo has `sphereMetallic = 1.0`, so the sphere's albedo target is (0,0,0) and the diffuse view is black. The values panel shows "albedo target 0,0,0" while the narration talks about colour.
   - Fix: for this beat set `sphereMetallic=0` (with `blend=1`, and put it back at the start of c12-passes), or say it of the teapot and box. Without that the chapter's diffuse step has no visible effect.
   - The same applies in c12-passes: the diffuse-pass sentence ("multiplies that albedo by the diffuse cubemap") is shown on a sphere whose albedo is zero.
2. **The two cubemaps are never shown.** "an environment, stored as two cubemaps" (c12-sky) and the bake and prefilter beats show only the chrome sphere and a diagram. The lab has an unfolded cube-cross view (`probeCubemap`) used in chapter 13. Show the specular cubemap's six mips (or the cross with mip 0, 2, 5 on successive sentences) in c12-bake or c12-prefilter, and the 32-texel diffuse map beside it in c12-cosine. With that, "mip m is filtered for roughness m over five" and "rougher mips gather a wider cone" become visible, instead of inferred from the sphere.
3. **c12-prefilter's sample sweep is invisible.** `ggxSamples=32` to 128 to 256 only changes a pink sparkle of rays at the sphere's top, at roughness 0.2 (barely a cone), and the reflection looks identical in the frames. The sentence that explains why is "which hides the noise of so few samples", so the sweep should show noise: render the sample directions on a larger sphere or a hemisphere plot, or at least show a rough sphere (`sphereRoughness=0.6`) with `ggxSamples` 8, 32, 256 and the cone fan scaled up. Else drop the sweep and describe 256 once.
4. **The diagram's focus does not follow the sentences.** c12-bake keeps `focus: GGX, COS` for all four sentences, so both filters are lit while only one is named at a time, and the BRDF texture node (the fourth sentence) is not lit. If per-sentence `focus` is supported, use GGX, then COS, then BRDF. Also "two hundred fifty six texel faces" is spoken next to a lit node that says "256 samples per texel": two different 256s. Say "faces of two hundred fifty six texels, and two hundred fifty six samples for each one".
5. **c12-sky opens with a values panel full of numbers that nothing narrates** (albedo target 0,0,0; mip; prefiltered R; compensation) and the R, N, V arrows with no sentence naming them. Hide the panel (`panel=none`, or use a hint) for c12-sky and c12-prefilter's first sentence, and bring it up in c12-passes where its terms are narrated. In the sky beat, name the R arrow in the chrome-sphere sentence: "the reflection vector R picks the point of the environment the sphere shows."
6. **c12-passes sentence 4** ("applies the split sum, F zero times A plus B, with the compensation and N dot V held at one hundredth or more, then the skylight's exposure and the specular occlusion") is a list of seven items read in one breath while a code panel highlights five lines. Switch to `panel=values param.formula=1` there, so the formula's terms light as they are named, or split it into two sentences: the split sum, then the three modifiers.

### Depth and correctness against the research

- **The diffuse pass that runs is `DiffuseComposite`, not `DiffuseGlobalFullscreen`** under the default template (research R7.07, F5). Chapter 1 says it (storyboard line 42), but c12-passes names `diffuse-global` and its pseudo-code and source tags are the fullscreen pass. The outline asked for this chapter's sources "plus `diffuse-composite`". Add one sentence after the diffuse step, with `callout=diffuse-composite`: "In a project from O3DE's default template, the probe-grid gem swaps this pass for a composite that applies the same sky light where no grid is placed," and add `diffuse-composite` to both source lists.
- **Mip 0 is not a copy (R7.33, D8).** "Mip m is filtered for a roughness of m over five" begs the question for m = 0. Add a clause: "mip zero, roughness zero, is not a copy but a blur of about one texel." (c12-prefilter, `callout=filter-ggx`).
- c12-bake's "diffuse cubemap of thirty-two to one hundred twenty-eight texels a face; the lab's is thirty-two" - fine; but the specular size is not given for the lab (state if it is the same 256).
- c12-readout: "the eight-bit roughness, its mip, the prefiltered light, and the split sum" - the sheet shows the first row (roughness 0.2 -> mip 1.0), then 0.27 -> 1.35, 0.45 -> 2.27. Good sweep (0.1 to 0.6), but only the prefiltered row visibly changes and no row is lit. Add row highlights for mip and prefiltered while they are named.

### Flow

- The opening two sentences are redundant: "That covers the lights the forward pass evaluates; what the opaque frame adds after it comes from the sky." then "The second source of light is the sky." Delete the second, or merge: "The forward pass has done every light; what the opaque frame adds after it is the sky." ("Second" is also wrong: the sky is not the second light source after the sun, in the chapter's own order.)
- After c12-sky, c12-bake is a diagram of the offline bake, then c12-prefilter returns to the lab scene, c12-cosine again, then c12-passes: four scene switches in 100 s. Fine, but the c12-cosine beat (19 s) is the weakest: its first two sentences have only a cue `ggxSamples=0 sphereRoughness=0.2` and no picture of a hemisphere average. Show the cosine lobe at the point (the `diffuse lobe` visual exists for chapter 7) or a dotted hemisphere of sample rays, in place of the sphere.
- c12-occlusion: the room beat is a different scene from the sky ("The lab's room has no occlusion maps...") and the panel is chapter 11's "Forward pass at the point", so nothing in it shows the claim "only SSAO darkens the sky's light". Add a viewMode (the diffuse-only IBL view with and without SSAO) or a sweep of the SSAO strength, or say it in one sentence and move on. The last sentence refers the viewer onward ("in the next chapter, ... which the lab leaves out") - good bridge, keep.
- c12-sources (9.6 s): four of eight entries visible; the narration ("the bake, the targets, and the two passes") matches the order, but `filter-ggx`, `filter-cosine` and `vect-to-texel` are off screen. Lengthen to about 12 s or drop to six entries; the AMD BSD licence remark is fine.

### Fit

About 195 of 210 s. Adding the composite sentence, the mip-0 clause and the cubemap views costs about 10-12 s; trimming the redundant opening sentence and the SSAO beat's repetition recovers about 6 s.

## Top fixes in order

1. c11-fit: the boxes do not hold still when the observer turns; correct the sentence and show the snap steps (readout in texels).
2. c11-fit: the sphere centre is clamped to the far face for cascades 0-2; fix the sentence and the listing.
3. c12-cosine and c12-passes: the sphere is metallic, so the diffuse view is black; use a dielectric sphere for those beats.
4. c11-select: show the selection (floor dots coloured by cascade, sweeping observerYaw and shadowFar) and put the full-screen pass first.
5. c11-splits: state why cascades exist, and zoom the camera out so cascade 3 and its label are visible.
6. c12: show the two cubemaps (cube cross with mips, 32-texel diffuse map) and make the sample-count sweep visible.
7. c12-passes: add the `DiffuseComposite` sentence and callout; add the mip 0 one-texel blur clause.
8. c12-sky, c12-passes: hide the unreferenced values panel at the start, and light formula terms for the split-sum sentence.
