# Review of chapters 7 to 10

Timings (caption span of each chapter, from `timeline.json`) against budget: ch7 90 s of 100, ch8 120 s of 130,
ch9 141 s of 152, ch10 142 s of 152. All four are near 1.1x natural, so fit is fine; the issues are what the
frames show. Cross-chapter points first.

## Cross-chapter problems

1. **Formula cues point at a panel that is not on screen.** A `param.formula=N` cue lights a term only in the values
   panel, but these beats show the code panel at the same time: c8-base (formula 1 and 4), c8-specular (formula 3 and 2,
   first two sentences), c10-thick (formula 1, 2, 3 with `panel=code` for the whole beat), c7-lambert sentence 1.
   Fix: keep `panel=values` while a formula term is named, and switch to `panel=code` only for the sentence that
   walks code lines; or split the beat.
2. **Callout labels are cut off by the panel.** The bottom-left label runs under the right-hand panel and is clipped:
   `GetDiffuseLighting_Standard...` (c8-base), `ModelDataInstance (forward-pass pr...` (c8-ibl),
   `specularF0.enableMultiScatterCom...` (c9-compensation), `...transmissionMod` (c10-thick). Shorten these labels in
   `source-index.json` (symbol only, or a shorter note) or wrap them to two lines.
3. **Sources beats show only four entries.** c7 declares six, c8 nine, c9 six, c10 fourteen, but each still shows
   four, and the closing sentence names items that are not visible (c10: "both modes, and the shadow-branch finding").
   Check whether the page scrolls or pages through them; if not, split into two pages or raise the cap.
4. **The vector and lobe overlay from chapter 6 stays up in chapters 7 to 10.** N, L, V, H labels, the observer, the
   pink "specular x N.V" lobe and callout clutter the diffuse sphere (c7) and cover the sphere top in c8 and c9, where
   nothing refers to them. Set `vectorsShown=0 lobe=0 readoutRows=0` (or the demo's equivalents) in the demos for
   diffuse, clearcoat and multiscatter, or turn them on only when named.
5. **Toggles come at the end of beats instead of where the idea is introduced** (c10 transmission on/off, c9
   multiscatter default off, c8 coat factor). See per chapter.

## 7 · Diffuse and its response

**Depth.** The Lambert line is explained; the response is stated, not derived. A viewer still asks why diffuse should
lose the specular share (energy split: what Fresnel reflects cannot also be scattered). Sentence 6 ("could send back
more light than arrives") is the reason, but it comes last and shows nothing.

**Intuition and change.**
- c7-lambert, "a smooth surface seen at a grazing angle keeps less of its light for diffuse": the readout row
  `1 - F (response)` reads 0.960 in every frame, so nothing changes. Add a camera path to a grazing angle
  (`camera.pitch` to about 80 degrees, or `camera.yaw` to about 70) with `sphereRoughness=0.1` while this sentence plays, and
  light the response row.
- c7-response false colour is almost all white (walls and floor too), so "dips only toward the rim" cannot be seen.
  Show `1 - response` (or a contrast-stretched map), or dim the room in this view mode; sweep `sphereRoughness`
  0.9 to 0.1 inside the false colour view, since the dip is the point. Currently the sweep (0.1 then 0.9) is spoken
  "smoother surfaces dip more" in the wrong order: go 0.9 then 0.1 so the rim visibly darkens.
- Energy argument (last sentence of c7-lambert): add a toggle that drops the response (`diffuseResponse=0`, a new
  cue if the demo lacks it) at a grazing view on a smooth sphere, so diffuse plus specular visibly exceeds the lit
  input; then turn it back on.
- c7-disney: switching `diffuseModel` 1 and 0 changes the readout rows but not the sphere much, and sentences 2 and
  3 change nothing on screen. The difference between the two models is never shown. Either sweep
  `sphereRoughness=0.9` with the light near grazing (Disney's retro-reflection) with `diffuseModel` 0 then 1, or cut the
  beat to two sentences: the model is unused by StandardPBR, so it is the weakest 26 s in the chapter.

**Highlighting.** The code highlights (`lambert`, `response`) are good. `fresnel-roughness` callout sentence has no
lit code line; highlight the `grazing = max(smooth, f0)` line.

**Flow.** Opening sentence bridges well. The Disney beat is a detour with no setup for why the viewer should care;
fold it into one sentence at the end of c7-response if the budget is needed elsewhere.

## 8 · Clear coat

**Depth.** The chapter lists formulas but never shows the layering. "Light now enters it from the coat rather than from
air" (c8-base) points at nothing: add a side-view layer diagram or inset (air, coat, base) with the ray bending, or
at least a label "IOR 1.5 above the base". The Filament F0 formula is read aloud as words ("one minus five root F zero,
over...") which a listener cannot follow; replace with "the F zero seen from inside the coat, which for a metal
changes it only slightly" and let the lit formula term carry the symbols.

**Intuition and change.**
- c8-layer: the coat is on from the first frame (factor 1.0) so "off by default; here the sphere is..." shows no
  contrast. Start at `clearCoatFactor=0`, then ramp to 1 over the sentence "here the sphere is a rough metal under a
  smooth coat" so the sharp highlight appears. Also sentences 1 and 2 say the same thing; merge them.
- c8-base: the "dims diffuse twice" finding is told on a rough metal (the sphere is metal, F0 about 0.8), where diffuse
  is almost zero, so the dimming cannot be seen. Use a coloured dielectric (`sphereMetallic=0`) for c8-base, sweep
  `clearCoatFactor` 0 to 1 with the `diffuseResponse` and diffuse rows visible, then return to the metal in
  c8-specular. Also show the values panel here (see cross-chapter point 1).
- c8-specular: the factor sweep (0 then 1, 1 s and 2 s blends) is the only sweep. Add `clearCoatRoughness` 0.05 to 0.6
  during "at an alpha of its roughness squared" so the coat highlight visibly widens; slow the factor ramp to at least
  4 s so the metal dimming is readable.
- c8-ibl: no sentence has a visible referent except the code lines. For the stencil sentence, show a debug view of what
  the full-screen reflection pass draws, with the coated sphere as a hole, then the forward pass filling it. The
  last sentence ("smallest probe that overlaps it, between inner and outer boxes") highlights no code (the `ibl` lines
  from the previous sentence stay lit) and uses probes before chapter 13 introduces them: cut to "blends in the nearest
  probe that overlaps it; chapter 13 covers probes".

**Highlighting.** The `clear-coat-default` and `clear-coat-inputs` callouts in c8-layer appear with the formula
panel, but no term is lit for "F zero of four percent" or "factor and roughness"; set `param.formula` to the Fc and
the readout rows for factor and roughness.

**Flow.** First sentence bridges. c8-ibl is the least connected beat (reflections in the forward pass arrive without
setup); open it with "So far the coat changed the direct light; the sky reflection needs the same change."

**Fit.** Good (120 of 130).

## 9 · The BRDF texture and multiscatter

**Depth.** The key logical step is missing: why the texture holds A and B and why A + B is the energy single scattering
keeps. Add one sentence in c9-texture: "Schlick's Fresnel is linear in F zero, so the integral is F zero times A plus B;
at F zero one, A plus B is all the light a single bounce keeps." and light the `accumulate` lines while it is said.
Also say what a texel is in plain terms (the fraction of light that survives one bounce for that view angle and
roughness) and why it is precomputed (one integral per texel instead of one per pixel).

**Intuition.**
- c9-loss: toggling `multiscatter` 0 and 1 at roughness 0.8 barely changes the sphere in the stills, so "brighter, as a
  real rough metal would be" is not visible. Use a furnace-style setup (uniform sky, F0 near 1) or a split view, and
  sweep `sphereRoughness` 0.2 to 1.0 with `multiscatter` 0 then 1. The readout row "A + B single-scattering
  energy" is on screen from the start but not named until c9-compensation; light it when "loses energy" is said.
- c9-texture: the 256 x 256 texture itself is never shown. Add a 2D inset of A + B with a cursor at
  (N.V, smoothness) = (0.82, 0.20), the point this sphere uses. "ggxSamples" steps 16, 64, 256 but the sentence says
  one thousand and twenty four: reach 1024 if the demo allows, otherwise say "here, two hundred fifty six".
  The spoken weight ("visibility times four, V dot H, and N dot L, over N dot H") is unreadable aloud; show it
  as a formula tag and say "weights each sample by the standard importance-sampling factor".
- c9-compensation: the "off by default in the material" finding (the important practical point) has no visual. At
  "defaults to off" set `multiscatter=0` and at "the lab turns it on" set `multiscatter=1`.

**Cut or shrink.**
- c9-texture sentence 1 carries four facts (size, half floats, once at startup, lab uses 32-bit); keep "once at
  startup" and move the sizes to the sources panel. The "rows stored upside down" sentence (c9-texture last) is a
  detail the viewer cannot use; fold into one clause of the lookup sentence.
- c9-finding (34 s) is about the lab's GPU port, not O3DE; it shows nothing (the compensation readout reads 1.00 at
  roughness 0.1 and the sphere is a mirror), and ends hedged ("depends on the GPU"). Either show it: sweep
  `sphereRoughness` 0.2 to 0.01 with a plot of A + B against roughness, GPU texture against CPU texture, the GPU one
  dropping below 0.03; or reduce it to two sentences and move it to the end of c9-texture.

**Flow.** Opening sentence is wrong for the previous chapter: "Under the coat, the base's specular still loses
light" (the sphere here has no coat, and chapter 8 ended on forward-pass probes). Replace with: "One more piece of the
specular lobe: a single GGX bounce loses energy on rough surfaces, and a texture O3DE fills at startup puts it back."

**Fit.** 141 of 152; c9-texture is the longest beat at 41 s with the least on screen that changes.

## 10 · Transmission (EnhancedPBR)

**Depth.** Thick mode explained as a list of operations ("compared with the light's direction bent by the normal,
raised to a power of six, and scaled"); the picture behind it (light entering the back, scattering, leaving toward the
viewer) is not drawn. Thin mode is one sentence with no explanation of the kernel; the finding relies on the fact
that kernel(0) = 1, which is never said. Add "at zero distance the kernel is one: nothing has been absorbed".

**Intuition and change.**
- c10-why: the point of the chapter (a back-lit teapot is dark without transmission and glows with it) is shown only at
  the very end of c10-thick, 100 s in. Move `teapotTransmission=0` then `1` to c10-why, at "let light through from
  behind", and keep a short repeat in c10-thick. Also reorder: "Here the teapot is EnhancedPBR, lit from behind" is the
  anchor and should come first. The frame also shows a large pale sphere filling the left of the picture and a label
  "point light (sphere)" while the narration says "the simple point light is off"; hide the sphere (or move the
  camera) and relabel.
- c10-light: "Simple point lights add no back lighting" and the sphere light's nearest-point trick have no visual. Show
  the bulb with its radius, the reflection ray, and the nearest point on the bulb; sweep the bulb radius 0.05 to 0.3 and
  watch the highlight stretch. "Here it casts no shadows" is an orphan sentence; move it into the thin beat where it matters.
- c10-thick (44 s, six sentences): the light's wave (`sphereLightX`) is missing from this beat's animation (it exists
  in the others), so the row `V.-(L+N d)^p x scale` (0.95 to 0.71 in other beats) never moves where it is explained.
  Restore the wave while this is spoken. Sweep `thickness` 0.05 to 0.5 during "e to the minus four times the
  thickness" so the attenuation row falls. Use the values panel (cross-chapter point 1). The final comparison is a
  one-second toggle mid-sentence ("dark; with it, the light glows through"); give it 3 s each.
- c10-thin: no change shown. Show the shadowed case (thickness data present) against the shadowless one, with the
  back-lighting row 0.40 against the data case; without the contrast, "the brightest back lighting it can have" is
  just asserted.

**Ordering of the finding (c10-thin).** Currently: comment says no back lighting without data, then "a finding ... shadow
branch runs even for a light without shadows", then "There the lookup returns zero, not the negative value", then the
result. "There" is ambiguous and "its options default to true" does not say whose. Rewrite: "The code expects the
shadow lookup to return minus one when there is no shadow data. But a point light without shadows still takes the
shadow branch, because its options default to true, and the lookup returns zero. Zero distance means the kernel's full
value: the brightest back lighting."

**Highlighting.** `highlight=thick` lights the whole function for six sentences; light the specific lines
(`tr = ...`, `a = ...`, `t = max(...)`) as each is described.

**Flow.** First sentence bridges well. c10-light starts abruptly (the viewer has not been told that the simple
point light of chapter 5 cannot do this); add that as the first sentence of c10-why rather than a callout in c10-light.

**Fit.** 142 of 152, fine; c10-thick is 44 s and can lose the repeated "thickness" sentence if the sweep is added.

## Top fixes

See the list returned with this file.
