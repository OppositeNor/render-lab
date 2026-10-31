# O3DE lighting model: the story's outline

O3DE 26.05 (engine.json 2.6.0, tag 2605.0), commit `3db6943249d8bd7960b9ed7e9aee310b7668586e`. Claims cite
`research.md` by claim (`R2.17`) or finding (`F17`); the file:line is there. Scope agreed with the user
(2026-10-30): ten chapters, about 45 minutes; hair one beat; no foliage utility in the lab; engine findings narrated
as findings ("read from the code, not run in the engine").

Total budget: 2,790 s (46.5 min).

## The whole story in one paragraph

A pixel on the teapot is lit in O3DE in pieces, by different passes, at different times in the frame. The depth
pre-pass and tiled light culling decide, before any shading, which lights a 16 × 16 tile can see. The forward pass
turns the material into a surface (albedo, F0, roughness) and adds the direct light of those lights through one of
four lighting models, reading a shadow buffer made earlier; it writes five targets, not one colour. Fullscreen passes
then add what the forward pass left out: diffuse and specular image-based light, reflection probes, subsurface
scattering, and ambient occlusion, and a merge puts diffuse and specular together. The viewer should come out able to
say, for any pixel, which pass computed which part of its light, from which data, and what the engine trades for
speed at each step.

## The subject, the scene, the worked point

- **Scene.** `rlstd.scene/study-room-grounds` (the study room on a 600 m floor): indoor light for culling and the
  BRDF, the long outdoor floor for cascades. The lab's panel light is replaced, in this investigation's shading, by
  O3DE light types: one directional sun (shadowed), a cluster of simple point lights (`extraLights`, for culling),
  one spot, one sphere, one disk, one quad (for the light-type chapter); all are values, so the story turns them on
  as it needs them.
- **Subjects.** The teapot (irregular, the default subject), the sphere (curvature sweeps N·L and N·V over one
  surface), the floor around them (shadows, AO in the contact, probes). Walls fade (`walls`) when the camera looks
  through them; the rotated box fades when it hides the teapot's lit side.
- **The worked point P.** One point on the teapot's body, below the spout, lit by the sun and two point lights,
  near the floor contact (so it has AO), inside the reflection probe's inner box (chapter 6). It is a pick
  (`pick=…`, the readout's record) the story follows through every chapter, and the recap traces back.
- **One, then many.** Each chapter works P (or its tile, its texel, its cascade) in the struct panel or readout,
  then sweeps the parameter across the room, then shows the other objects.

## The architecture map (one Mermaid diagram, reused)

`diagrams/frame.mmd`: the opaque frame as the engine runs it, its passes in order with the data that flows between
them, and its parallel and once-only parts true to the engine (R1.06–R1.34, research §1.9):

```
[offline] BRDF LUT (once at start-up) · IBL cubemaps (asset build) · probe bakes
DepthPrePass ──depth──▶ LightCulling (TilePrepare → Cull → Remap) ──tile lists──▶ Forward
DepthPrePass ──depth──▶ Shadows (Cascades → FullscreenShadow → Blur) ──shadow buffer──▶ Forward
Forward ──5 targets──▶ DiffuseGlobal (probe-grid composite) ─▶ Reflections (stencil, weights, global, probes)
   ─▶ SkyBox ─▶ ReflectionComposite ─▶ MSAA resolves ─▶ SSS ─▶ SSAO ─▶ DiffuseSpecularMerge ─▶ Transparent ─▶ Post
```

Each chapter opens on it with its part lit (`focus`) and ends by lighting the arrow its output feeds.

## One encoding per kind of data (the palette's roles, for the whole story)

| Data | Colour | Treatment |
|---|---|---|
| Geometry, surfaces | neutral grey/white | solid, wire for boxes |
| Depth, distance | blue ramp | slices, lines along the view ray |
| Light lists, tiles, bins | orange | grid cells, counts |
| Shadow and visibility | violet | the shadow buffer, cascade outlines |
| Direct light (radiance at P) | warm yellow | light vectors L, lobes |
| Indirect light (IBL, probes) | cyan | reflection vector R, cube faces |
| Occlusion (AO, specular occlusion) | dark green | sample points, the AO term |
| Material data (albedo, F0, roughness) | magenta | target views, readout rows |

A light vector is never drawn cyan; an occlusion value is never drawn as light.

## Chapters

### 1 · Where the lighting runs · 330 s

Opening, then the frame. New terms stay out of the opening.

| Beat | Headline (the claim) | Claim → what shows it |
|---|---|---|
| c1-open | Lighting is assembled by many passes | The room with each contribution switched on in frame order (direct, shadow, diffuse IBL, specular IBL, AO), same camera and exposure, cued on what each adds. The problem in one sentence: how does the engine light a room from many lights and the sky without shading every pixel against every light. The map appears; what the viewer will explain by the end. |
| c1-pipeline | Desktop runs MainRenderPipeline, with 2x MSAA | R1.01, R1.02 → callout on the CVar; the map's root. Mobile (Android, iOS) and LowEnd named; the lab renders 4x (§10.1). |
| c1-map | The opaque frame, pass by pass | `frame.mmd` with `focus` stepping pass to pass as named (R1.06–R1.34). |
| c1-prepass | The depth pre-pass first, so lighting knows what is visible | R1.07, R1.15–R1.17 → depth view (blue), then the forward pass shading only where its depth equals the pre-pass (`GreaterEqual`, early-Z); overdraw count with and without (a lab readout, schematic). |
| c1-targets | The forward pass writes five targets, not one colour | R1.18, R1.19 → five views side by side, each titled with its name and format (Diffuse RGBA16F, Specular RGBA16F, Albedo RGBA8, F0+roughness RGBA8, Normal RGB10A2); P outlined in each; its texel values in the readout. |
| c1-normal | The normal is packed into two numbers and a sign, with two flags | R1.20 → signed-octahedral encode of P's normal, step by step (fold onto the octahedron, unfold, z sign bit); the 2-bit flags in alpha (IBL in fullscreen, multiscatter). |
| c1-split | Opaque forward holds only direct light; the rest is added later | R1.21, R1.22, R5.01 → `forwardMix`: the diffuse target with direct light only, then the fullscreen passes adding IBL; the map lights DiffuseGlobal and Reflections. |
| c1-stencil | Stencil bits route each pixel to its passes | R1.35, R1.36 → stencil view as data (0x83 everywhere, 0x80 for a clear-coat sphere, 3 + n inside probe volumes later), legend. |
| c1-models | A material's lighting model picks its shaders | R1.37–R1.40, R3.01 → the four lighting models table (Base, Standard, Enhanced, Skin) as a readout; draw lists `forward` / `forwardWithSubsurfaceOutput`; the teapot switches model and its draw list label changes. |
| c1-alternatives | One-target forward on Mobile and LowEnd; deferred exists but is off | R1.43, R1.45 (one target, IBL in forward), R1.48–R1.51, F15 (visibility-buffer scaffold, `r_deferredRenderingEnabled` false, no consumer) → the map collapsing to a single forward box; the deferred box drawn dashed. |
| c1-recap, c1-sources | | Recap: five targets, finished by later passes. Sources: R1.01, R1.07, R1.19, R1.21, R1.35, R1.38, R1.43, R1.50. |

### 2 · Direct light: which lights reach a tile · 330 s

Bridge: the forward pass shades only lights it is given; before it runs, the engine has sorted the lights into
tiles.

| Beat | Headline | Claim → what shows it |
|---|---|---|
| c2-why | Each 16 × 16 tile gets its own light list | R2.02, R2.03 → 24 point lights (`extraLights`), the screen tiled, per-tile counts (orange heat); 1920 × 1080 → 120 × 68 tiles (worked number). |
| c2-tile | P's tile: its depth range from the pre-pass | R2.10–R2.13 → P's tile highlighted; its pixels' depths drawn as blue points along the view; zNear, zFar. |
| c2-mask | The range becomes a 32-bit mask of occupied slices | R2.14, R2.16 → 32 slices drawn in the tile's frustum, the occupied ones lit; the bit string in the readout. Transparent fill (R2.15) in passing. |
| c2-bins | One to eight bins, from the span in metres | R2.17 → `cullTileSpan` sweep (camera moves so the span crosses 1.41, 2.83, 5.66 m), bin count stepping 1 → 2 → 4 → 8. |
| c2-cull | A light is kept by its sphere against the tile box, then by the mask | R2.18, R2.20 → the tile AABB, one light's sphere; `extraLightRadius` sweep; a light in the empty gap passes the box test and fails the mask (the reason the mask exists). |
| c2-list | Lights are written in seven groups, each with an end marker | R2.19, R2.23, R2.22 → the list as a row of cells (index << 16 \| bin bits), markers 0xFFFE; the remap into eight bins of 256 (R2.06, R2.22). |
| c2-iterate | The forward pass walks its bin's list, type by type | R2.24, R2.25 → P's bin chosen from its depth; the iterator stepping through groups (`run=walkList`), each light's contribution added at P. |
| c2-falloff | Every light fades to zero at its attenuation radius | R2.30 → falloff curve 1/d² vs windowed (1 − (d²/R²)²)²/d², `lightRadius` sweep; irradiance heat on the floor reaching 0 at R. |
| c2-types | Spot cones, spheres, disks, capsules, quads: one approximation each | R2.32 (cosine smoothstep), R2.33 (representative point + energy normalisation), R2.34, R2.35, R2.36–R2.37 (LTC: 32 × 32 tables, horizon clip) → each light on in turn on the teapot, its representative point drawn; roughness sweep shows the point move. |
| c2-units | Units: candela, nits, lux; EV100 | R2.40, R2.42, R2.39 → readout of P's incoming illuminance per light; 4 EV100 = 40 lux sun. |
| c2-findings | Read from the code: polygon lights never light; one directional return | F17 (polygon group missing → zero iterations, shown on the list row), F18, F19 (angular radius computed and unused). Said as read from the code. |
| c2-recap, c2-sources | | R2.02, R2.14, R2.17, R2.20, R2.19, R2.24, R2.30, R2.33, R2.37, F17. |

### 3 · The surface: from material to albedo, F0, roughness · 220 s

Bridge: with each light chosen, the next question is what it lights: the surface the material describes.

| Beat | Headline | Claim → what shows it |
|---|---|---|
| c3-inputs | The material's inputs become a surface | R3.04, R3.05 → listing `surface_setup`; base colour converted LinearSRGB → ACEScg (R3.05). |
| c3-metal | Albedo and F0 come from one base colour | R3.06 → `sphereMetallic` 0 → 1: albedo = base·(1 − m), F0 = lerp(0.08·factor, base, m); the Albedo and F0 target views change together. |
| c3-rough | Roughness is clamped, squared, and squared again | R3.08, R3.09 → `sphereRoughness` sweep, the three values in the readout (0.024 clamp visible at the low end). |
| c3-aa | Specular AA widens roughness where the normal changes fast | R3.10 → `specularAA` toggle on the sphere's edge at distance; kernel roughness heat; cap 0.18. |
| c3-occl | Occlusion and emissive inputs | R3.12, R3.13 → occlusion exponent (does nothing without a map, F34); EV100 → nits; emissive in the specular target (F40). |
| c3-decal | A decal changes albedo and normal, after lighting setup | R3.15, F6 → a decal on the floor beside the teapot; in 26.05 decal albedo/normal reach direct lighting but not the G-buffer, so the fullscreen IBL ignores them (shown: Albedo target without the decal). Finding. |
| c3-recap, c3-sources | | R3.05, R3.06, R3.09, R3.10, R3.12, R3.15, F6. |

### 4 · The BRDF: how the surface answers each light · 330 s

Bridge: a surface is a set of numbers; the BRDF turns them, a light direction, and the view into reflected light.

| Beat | Headline | Claim → what shows it |
|---|---|---|
| c4-vectors | N, L, V, H at P | R3.20 → vectors drawn at P (yellow L, white V, grey N, H), light orbiting. |
| c4-d | D: how many microfacets face H | R3.16 → D lobe drawn at P; `sphereRoughness` sweep; formula term lit. |
| c4-g | G: the facets that see both light and eye (height-correlated Smith) | R3.17 → grazing sweep (light toward the horizon), G value drop; the 1/(4 N·L N·V) folded in. |
| c4-f | F: Schlick from F0 toward white at grazing | R3.18 → camera to grazing on the sphere; F rising; F0 from chapter 3. |
| c4-diffuse | Diffuse: Lambert scaled by 1 − F | R3.21, R3.19, R3.30 → `diffuseResponse` heat on the sphere, bright at centre, dimmer at the rim. |
| c4-perlight | Per light: (diffuse + specular) × intensity × shadow | R3.20, R3.21, R4.28 → listing `simple_point_light`, highlight per line; P's readout rows. |
| c4-models | Four lighting models, four BRDFs | R3.22 (Disney diffuse), R3.24 (anisotropic GGX), R3.25–R3.26 (clear coat; F32 double attenuation), R3.27 (dual specular) → the teapot switching models with its lobe; table in the readout. |
| c4-lut | The BRDF LUT: built once, at start-up | R3.37 → the 256 × 256 R16G16F texture as an image, its two channels decoded, P's (N·V, 1 − roughness) texel marked; built with 1024 GGX samples (`run=buildLut`). |
| c4-multi | Multiscatter compensation returns the energy single scattering loses | R3.33, R5.12 → white furnace on a sphere row, roughness 0 → 1, compensation off/on; 1 + F0(1/(x+y) − 1). |
| c4-default | Finding: compensation is off unless a material turns it on | R3.38, F35 → the shader default true vs the material property false; shown as two switches. |
| c4-recap, c4-sources | | R3.16, R3.17, R3.18, R3.21, R3.20, R3.37, R3.33, R3.38. |

### 5 · Shadows: a map from the light, read through the screen · 330 s

Bridge: the per-light formula ends in a shadow factor; it was computed before the forward pass ran.

| Beat | Headline | Claim → what shows it |
|---|---|---|
| c5-why | The sun's shadow is a depth map seen from the light | R4.01, R4.02 → camera at the light, its depth view (blue); P projected into it. |
| c5-cascades | Four cascades of 1024², split near-dense | R4.05, R4.12 → split planes drawn in the camera frustum; `cascadeRatio` sweep 0 → 1 (the table's values); cascade debug colours. |
| c5-fit | Each cascade bounds a sphere, snapped to texels | R4.07, R4.08 → the bounding sphere and the snapped box; camera nudged sub-texel, the box steps (shimmer avoided). |
| c5-lookup | P in its cascade: offset along the normal, biased, compared | R4.16, R4.17, R4.19 → P's normal offset (2.5·2/1024 m), the shadowBias toward the light, the texel read, z − depth > 0 → shadowed; listing `shadow_lookup`. |
| c5-bias | Biases against acne; slope bias is off | R4.18, F50, F49 → `normalShadowBias` sweep on a slanted floor patch: acne → clean → peter-panning; F49 (same offset in every cascade). |
| c5-fullscreen | Opaque pixels read a screen-space shadow buffer, not the cascades | R4.24, R4.26 → the R8 buffer as an image, P's pixel; its normal rebuilt from depth (faceted), not the vertex normal. |
| c5-filter | By default the compare is binary; an 8-pixel depth-aware blur softens it | R4.19, R4.25, F57 → the hard 0/1 buffer, then the blur's running mix along a scanline (depth edges stop it); PCF 4/9/16 and ESM shown as options (R4.20, R4.22) with their footprints. |
| c5-local | Spot and point shadows live in an atlas; a point light has six faces | R4.29–R4.33 → the atlas slices as thumbnails, a point light's six 91° faces, the face picked by the max axis. |
| c5-recap, c5-sources | | R4.02, R4.05, R4.07, R4.16, R4.19, R4.24, R4.25, R4.32. |

### 6 · Indirect light: the sky and the probes · 360 s

Bridge: direct light is now complete; the rest of a pixel's light comes from everything around it, and the forward
pass left it out.

| Beat | Headline | Claim → what shows it |
|---|---|---|
| c6-two | Two cubemaps: one for diffuse, one for specular | R5.08, R5.09, R5.11 → the two cubes as cross layouts, titled with format and size. |
| c6-diffuse | Diffuse IBL: one lookup by the normal, in a fullscreen pass | R5.09, R1.23, R1.24, F7 → P's normal into the cosine-convolved cube; the pass adding into the diffuse target; the probe-grid composite replacing it in a default project. |
| c6-specular | Specular IBL: the reflection vector, a mip chosen by roughness | R5.11, R5.20, R5.21 → R drawn at P (cyan); the six mips strip; `sphereRoughness` sweep moves mip = 5r (trilinear between two). |
| c6-splitsum | The split sum: cube × (F0·A + B) | R5.12, R3.34 → the LUT texel from chapter 4 again, the product at P. |
| c6-probe | A reflection probe: a local cube for a box of space | R5.24–R5.28 → probe inner/outer boxes around the teapot; the stencil counting volumes (3 → 4); the weight ramp between inner and outer. |
| c6-parallax | Parallax correction: aim the lookup where the ray meets the box | R5.17 → R from P hitting the outer box; corrected direction from the probe centre; toggle on/off. |
| c6-blend | Probes and the global cube blend by weight | R5.25–R5.27 → teapot walked from outside to inside; the weight 0 → 1; global × (1 − Σw). |
| c6-findings | Read from the code: probe reflections skip specular occlusion; a 0.4 grey | F58, F8/F9 → specular occlusion applied to the global pass only; the reflection buffer clear. |
| c6-ssr-dpg | SSR and the probe grid: off, or out of reach | R5.34–R5.40 (off by default; 96 steps; replaces probe result by alpha), R5.41–R5.45 (probe grid's update ships as binaries; composite readable) → in passing, one beat. |
| c6-recap, c6-sources | | R5.09, R5.11, R5.20, R5.12, R5.24, R5.17, R5.26, F58. |

### 7 · Occlusion: what the light cannot reach · 200 s

Bridge: indirect light assumes the whole sky is visible; near the floor contact it is not.

| Beat | Headline | Claim → what shows it |
|---|---|---|
| c7-why | SSAO darkens creases from the depth buffer alone | R6.01, R6.02, R6.07, R6.08 → AO off/on at the teapot's floor contact; half-res R8 target as an image. |
| c7-samples | Fifty-two samples around P, each weighted by distance | R6.15–R6.22 → P's reconstructed position and depth-normal (R6.16); 12 cross taps + 40 spiral samples drawn in 3D (dark green), coloured by saturate(cos), sized by 1/(d² + 0.07); `run=ssaoSamples`. |
| c7-radius | The radius is a fraction of the screen, not metres | R6.23, F69 → camera pulled back: same 27 px disc, growing world reach. |
| c7-output | 1 − AO, then a 2.2 power, blurred and upsampled | R6.24–R6.27 → the curve, the blur slope test along a scanline, the 2 × 2 depth-weighted upsample. |
| c7-where | Finding: SSAO multiplies all diffuse light, direct included | R6.31, F68 → strong sun on the teapot, AO toggled: the lit side darkens too; specular untouched. |
| c7-material | Material AO: an exponent on a map, applied to direct light too | R6.33–R6.35, F34, F33 → the AO map on the teapot, factor 0/1/2. |
| c7-recap, c7-sources | | R6.01, R6.16, R6.18, R6.23, R6.24, R6.31, R6.34. |

### 8 · Light through surfaces · 280 s

Bridge: so far light bounced off surfaces; the Enhanced and Skin models also let it pass through.

| Beat | Headline | Claim → what shows it |
|---|---|---|
| c8-which | Only Enhanced and Skin carry transmission and SSS | R7.03, R7.04 → the lighting-model table again, two rows lit. |
| c8-thick | Thick objects: light from behind, scattered toward the eye | R7.07 → teapot lit from behind, `transmissionPower` and `transmissionDistortion` sweeps; the distorted L. |
| c8-thin | Thin objects: N·L from the back face, a kernel of thickness | R7.08, R7.09 → a thin sheet (leaf stand-in: a scaled box), thickness sweep; TransmissionKernel vs the skin profile T(s) as a chart. |
| c8-thickness | Thickness from the shadow map, and its units | R7.13, R7.14, F43, F83 → P's depth in the cascade vs its own: the difference, not metres; for a flat leaf it is about zero. Finding. |
| c8-foliage | A leaf in O3DE is a material, not a lighting model | R7.39–R7.41, R8.37, F93 → the same leaf as StandardPBR cutout (dark from behind) vs EnhancedPBR thin (glows); double-sided flips N but not the vertex normal (F72). |
| c8-sss | SSS: a screen-space blur of the diffuse light, profile by profile | R7.22–R7.28 → SSS on the sphere; the packed alpha (factor, quality) decoded; the 200-sample Burley footprint drawn; sample count steps 24/48/80. F3 in passing. |
| c8-skin | Skin and Eye | R7.29–R7.36 → Disney diffuse and dual specular on the sphere; Eye: iris refraction offset in one sentence. |
| c8-recap, c8-sources | | R7.04, R7.07, R7.08, R7.14, R7.41, R7.24, R7.27, R7.30. |

### 9 · Terrain, and a word on hair · 200 s

Bridge: every surface so far went through a material type; terrain brings its own forward shader.

| Beat | Headline | Claim → what shows it |
|---|---|---|
| c9-own | Terrain draws through the forward pass with its own shader | R8.02, R8.03, R8.26 → the map with Terrain's draw item in Forward; `rlstd.scene/terrain`. |
| c9-fixed | Fixed macro values: roughness 1, F0 0.04, metal 0 | R8.27 → readout; specular nearly flat on the hills. |
| c9-normal | Normals are made on the CPU, per vertex, quantised to 8 bits | R8.11, R8.12, F94 → O3DE's per-axis normal vs the exact one on the lab's heightfield, angle-difference heat on a steep slope. |
| c9-detail | Detail materials blend by weight; height blending is off by default | R8.19, R8.23, R8.24, F87 → two materials with a transition band; the weight formula. |
| c9-differs | It skips multiscatter in direct light but not in IBL | R8.28, F88 → toggle on the terrain. |
| c9-hair | Hair: three Marschner lobes | R8.41–R8.42 → strands (lab stand-in), R, TT, TRT toggled; n = 1.55, tilt 0.08. |
| c9-recap, c9-sources | | R8.03, R8.27, R8.11, R8.24, R8.28, R8.41. |

### 10 · One pixel, traced back · 150 s

| Beat | Headline | Claim → what shows it |
|---|---|---|
| c10-trace | P's light, in the order the engine made it | The map lit pass by pass; at each, P's value added (direct per light with its tile list and shadow; diffuse IBL; specular IBL and probe; AO); the readout accumulating. |
| c10-takeaways | Three takeaways | Data: five targets and stencil routing let fullscreen passes finish lighting. Computation and reuse: lights culled per tile once, cubemaps and LUT prefiltered once, shadows resolved once per screen. Trade-offs: binary shadows blurred in screen space, screen-fraction AO that darkens direct light, half-precision packing. |
| c10-sources | | The reading path (`readingPath`). |

## What changes, and what it changes (sweeps that must visibly move)

| Relation | Parameter | Range where it changes | What moves |
|---|---|---|---|
| Tile span → bins | camera distance | span 1.0 → 7 m | bin count 1 → 8 |
| Radius → tiles kept | `extraLightRadius` | 0.4 → 2.0 m | tiles lit, counts |
| Falloff window | `lightRadius` | 1 → 6 m | heat zero line |
| Metallic → albedo/F0 | `sphereMetallic` | 0 → 1 | Albedo, F0 targets |
| Roughness → lobe, mip | `sphereRoughness` | 0.05 → 1 | D lobe, mip index 0 → 5 |
| Grazing → F | camera pitch at the sphere | 0 → 85° | F, rim |
| Multiscatter | roughness row, toggle | 0 → 1 | furnace loss 0 → ~40 % (measured by the lab) |
| Split ratio | `cascadeRatio` | 0 → 1 | split distances (table) |
| Normal bias | `normalShadowBias` | 0 → 6 | acne → peter-panning |
| Blur | `shadowBlur` toggle, depth edge | | soft edge, stopped at depth steps |
| Parallax | toggle, P's position | | reflection lines up with the room |
| Probe weight | teapot position | outer → inner | weight 0 → 1 |
| SSAO radius | camera distance | 2 → 12 m | world reach of the disc |
| Transmission | `transmissionPower`, thickness | 1 → 16; 0 → 1 | back glow |

## Worth a detailed demonstration

- The five targets as separate views, with P's texel decoded in each (c1-targets, c1-normal).
- One tile's depth range → mask → bins → list → iteration (c2), step by step.
- The representative point moving with roughness (c2-types).
- The D lobe, G at grazing, F at grazing (c4).
- The BRDF LUT built sample by sample and read at P (c4-lut).
- P through the cascade lookup (c5-lookup) and the blur along a scanline (c5-filter).
- The reflection vector, the mip strip, the probe boxes and the parallax hit (c6).
- SSAO's 52 samples in 3D (c7-samples).
- The thickness measurement in the cascade (c8-thickness).

## The tools it needs

- **The investigation's shading as a pass chain** (`SceneShading`: outputs + compute passes), mirroring the frame:
  forward outputs (five targets, and stencil as a data target), light culling (early passes), the shadow
  cascades (depth views) and fullscreen shadow + blur, diffuse/specular IBL, probes, SSAO, SSS, merge.
- **GPU image kinds** for each target, the tile grid, the cascades, the cube crosses and mips, the BRDF LUT, the
  SSAO buffers, the shadow buffer: each clickable (pick a texel → the point in the scene, link line).
- **Picks**: P (a scene click), a tile (click on the tile image), a cascade texel, a cube texel, a probe.
- **Actions**: `walkList` (tile iteration), `buildLut` (LUT sample accumulation), `ssaoSamples`, `cascadeSnap`.
- **Overlays**: vectors (N, L, V, H, R), lobes, the tile frustum and slices, light spheres, cascade spheres and
  boxes, probe boxes, sample points.
- **Readouts and formulas**: P's record (surface, per-light terms, IBL terms, AO), formula tags per chapter.
- **The lab differs** (said on screen): 4x MSAA not 2x; stencil carried as data; the lab's sky as cubemaps built by
  the investigation (sizes said); the probe a lab-baked cube; strands and leaf stand-ins built from lab shapes.
