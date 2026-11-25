# Cold listen, English, round 2: chapters 4 to 6

Listener: a graphics programmer, no O3DE. One hearing, no scrolling back.

## Flagged sentences

Kinds: 1 lost a referent, 2 term undefined or defined too long ago, 3 had to re-parse, 4 fact with no reason, 5 two or more new ideas at once, 6 lost the thread.

### Chapter 4

- c4-diffuse #6 "So the response stays near" — 6. I do not see how "rises less toward white" gives "near one minus F zero across the whole sphere"; I lost the step.
- c4-perlight #5 "Diffuse and specular are each" — 3. "One more factor" is only named in the next sentence; I held it open.
- c4-models #6 "Back to Standard, its clear coat" — 5, 6. I was on Skin and Enhanced, then jumped back to Standard with a new lobe, a fixed F zero, and a number.
- c4-models #9 "Skin offers a second specular lobe" — 4. I could not tell why I need this.
- c4-models #12 "It dims that light once" — 1. "Each light's own calculation" did not tell me where the second dimming sits.
- c4-lut #5 "Schlick's Fresnel is F zero times" — 3. "One part, plus a second part" resolved only at the end, and I had to guess what the parts were.
- c4-lut #6 "So one sum is the scale" — 3, 5. It gives two new names, scale A and bias B, and ties them to two sums I was only told about a sentence ago.
- c4-multi #4 "A plus B is the reflection" — 3. "The share of the light that one bounce keeps" needed a second hearing.

### Chapter 5

- c5-why #11 "So in the far cascades" — 6. I could not tell how a wider ground area leads to the room taking a sliver of the depth range.
- c5-why #12 "It reads there as one" — 1. I was unsure whether "it" was the room or the sliver.
- c5-why #17 "A mesh that a smaller" — 1, 4. "Holds whole" lost me, and I did not see what skipping the mesh saves.
- c5-cascades #9 "This camera's near plane is" — 4, 6. I did not know why the near plane came up until the numbers followed.
- c5-cascades #11 "The full hundred metres would" — 4. A second set of numbers with no point stated.
- c5-fit #2 "In the light's view, the" — 3. "Stretched back toward the sun" arrived after I had already pictured a plain cube.
- c5-bias #10 "O3DE also computes a slope bias" — 1, 5. This is a second slope bias, the first being the one added when the map is drawn. I could not keep the two apart, and "uploads it" was another new idea.
- c5-bias #16 "Its texels are many times" — 4. It says the offset is small, but not what that costs or changes.
- c5-fullscreen #4 "The normal that this pass" — 4. I did not know why it matters which normal it uses.
- c5-blur #4 "A setting called the constant" — 1, 2. I did not know what fades, or what the falloff multiplies.
- c5-blur #12 "They are percentage-closer filtering" — 3, 5. It lists two filters with verbs missing, then explains one of them mid-sentence.
- c5-blur #13 "Percentage-closer filtering, or PCF" — 6. The acronym comes after the long name, and the thread had already moved on to the exponential one.
- c5-blur #14 "The code shows that the" — 1, 4. "Kernels" is unexplained, and I do not know why the squaring matters to me.

### Chapter 6

- c6-two #14 "Both cubes are bound for" — 4. I could not tell what one exposure and one orientation change for me.
- c6-diffuse #6 "That is the engine's own" — 1, 6. "It" and "the pass" were unclear, and I did not see how a project template relates to P's lookup.
- c6-diffuse #8 "That gem is the diffuse" — 2, 3, 5. It stacks "gem", the probe grid, light probes, and a definition inside a clause. That is a lot to take at once.
- c6-probe #5 "A stencil pass counts how" — 3, 5. It brings in nesting, counting, and a stencil pass in one sentence.
- c6-probe #6 "Every mesh already wrote three" — 6. "So the count starts at three" confused me. I took the three as the stencil value, not as zero boxes.
- c6-findings #4 "Inside a probe's inner box" — 2, 4. I did not know why the flag from chapter one would be off, or why that matters.
- c6-findings #5 "The code also clears the" — 6. I did not see how the grey follows from the flag being off.
- c6-findings #6 "Such a pixel keeps that" — 1. "Such a pixel" meant the flag-off pixel only after I worked it out.

## What I understood, per chapter

**Chapter 4.** The surface answers each light with D (how many microfacets face H), G (hiding, with the divisor folded in), and F (Schlick), times N dot L. Diffuse is Lambert, scaled by what reflection leaves. Per light, I multiply diffuse plus specular by the intensity, the window, and the shadow's lit ratio. The sky's reflection comes from a table of A and B built once. A rough metal is brightened by a compensation factor, and by default it is off. Still unclear: how Skin and Enhanced differ in use, and why the clear coat dims diffuse twice. The two sums behind A and B also stay vague.

**Chapter 5.** The sun's shadow is a four-cascade depth map. Each pixel rebuilds its position, is offset along its normal, picks a cascade, and compares depths. The result goes into a one-byte shadow buffer that a depth-aware blur softens before the forward pass reads it. I would still ask: what the second slope bias is, what the constant falloff does, and why the far-cascade offset being small is good or bad. The cascade-split numbers did not feel like they had a purpose.

**Chapter 6.** Two prefiltered cubemaps and the A/B texture give the sky's diffuse and specular light. For specular, the mip is picked by roughness and the split sum multiplies cube by (F0 A + B). Reflection probes replace the sky inside boxes, aimed by parallax correction and blended by weights. The two findings are that probes miss specular occlusion and that flag-off pixels keep a grey. I would still ask: how the gem and the probe grid relate to the lab, how the stencil count of probes works, and why a flag-off pixel gets the grey.

## Counts per kind

| Kind | Count |
|---|---|
| 1 lost a referent | 8 |
| 2 undefined or too-old term | 3 |
| 3 had to re-parse | 8 |
| 4 fact with no reason | 9 |
| 5 two or more new ideas | 6 |
| 6 lost the thread | 8 |

Flagged sentences: 29 (8 in chapter 4, 13 in chapter 5, 8 in chapter 6).
