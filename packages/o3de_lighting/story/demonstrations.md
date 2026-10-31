# O3DE lighting model: demonstrations

How each structure the story explains is stored in O3DE 26.05, how the lab shows it, on which subject, and what a
viewer can do with it. Storage facts cite `research.md` (formats, sizes, addressing, debug views per section).

## Principles

- **Mirror the storage.** Each O3DE resource is its own view, titled with its O3DE name and format. Packed
  channels are one view with the channels decoded side by side; separate resources are separate views laid out
  together, with the same element outlined in each.
- **One subject.** The teapot (the worked point P below the spout) and the sphere (curvature sweeps N·L, N·V);
  the floor contact for occlusion; the hills of `rlstd.scene/terrain` for chapter 9 only.
- **A pick lives everywhere at once.** Clicking a pixel of the scene or a texel of any view sets the same values
  (`pickX`, `pickY` in screen pixels, or `pickTile`, `pickCascade`, `pickTexel`, …), which every view outlines and
  the readout decodes; a link line joins the view's texel to the point in the scene (story arrows on published
  points `pick.scene`, `pick.<view>`).
- **Every view is an image kind** (GPU, live, parameters by key) or a picture (CPU, from the mirror) the story
  places by key; the dock shows the same views for the demonstration, so a viewer can reproduce every shot.

## The lab extension this needs

Image kinds (`lab/src/views.rs`) run before the scene with groups 0, 1 and 3; they cannot read the investigation's
pass-chain targets. Add a generic, opt-in read of the chain: a kind's tag `reads=<target>,<target>` binds those
targets (and buffers, and the depth views) at group 2 exactly as for a chain pass (`gpu::chain_binding`), and such a
kind is dispatched after the scene's chain. Kinds without `reads` keep today's behaviour and timing, so Godot's
frames stay byte-identical. Nothing in it names an investigation.

## Chapter 1: the frame and its targets

| Structure (O3DE) | Storage (research) | Lab view | Interaction |
|---|---|---|---|
| Depth pre-pass `DepthMSAA` | D32_FLOAT_S8X24_UINT, 2x, reverse-Z, cleared 0 (R1.07) | `depthView`: the chain's depth, linearised to a blue ramp; title "DepthMSAA · D32 S8 · reverse-Z" | click a texel → P; readout: device depth, linear depth (DepthToLinearDepth formula, R6.07) |
| Five forward targets | Diffuse RGBA16F, Specular RGBA16F, Albedo RGBA8, SpecularF0 RGBA8 (a = roughness), Normal RGB10A2 (a = 2-bit flags) (R1.18–R1.20) | five image kinds side by side (`targetDiffuse`, `targetSpecular`, `targetAlbedo`, `targetF0`, `targetNormal`), each titled name · format; `channel=` parameter shows rgb, or one channel (alpha decoded: specular occlusion, roughness, flags) | click any → the pick in all five and in the scene; readout row per target with the texel's stored values and their decode |
| Normal encoding | signed octahedron in 10/10/10, z sign in 1 bit; flags (f1 + 2 f2)/3 (R1.20) | `octahedron` picture: the unit sphere folded onto the octahedron and unfolded; P's normal travelling through the three steps (`octStep` 0 → 3) | `octStep` slider; the pick's normal |
| Stencil routing | 8-bit stencil, 0x80 diffuse GI, 0x3 IBL specular, 3 + n inside probe boxes (R1.35, R1.36, R5.24) | `stencilView`: the stencil reference as data (lab has no stencil, §10.1), false-coloured by value, legend | click → value, bits, the passes that will touch it |
| Lighting model per object | `lightingModel` → draw list (R1.37–R1.40) | per-object label: model and draw list | `teapotModel` choice (Base, Standard, Enhanced, Skin) |
| What forward holds vs later passes | direct + emissive in forward; IBL, SSS, AO later (R1.21) | `forwardStage` value: stops the chain after a given pass (forward only → + diffuse IBL → + reflections → + SSS → + AO → merge); the scene shows that stage | `forwardStage` slider (also the opening's comparison) |

## Chapter 2: light culling

| Structure | Storage | Lab view | Interaction |
|---|---|---|---|
| `TileLightData` | RGBA32_UINT per 16 × 16 tile: zNear bits, zFar bits \| log2 bins, 32-bit mask, worst count (R2.03, R2.16) | `tileGrid`: tile counts as an orange heat with the grid lines; the picked tile outlined | click a tile → `pickTile`; readout: zNear, zFar, mask as 32 boxes, bins |
| Depth slices of one tile | 32 uniform slices zNear..zFar (R2.14) | 3D overlay: the picked tile's frustum and its 32 slices, occupied ones lit; the tile's pixels' depths as blue points | `sliceStep` reveals pixel → bit; camera distance changes span (bins 1/2/4/8) |
| `LightList` / `LightListRemapped` | 256 entries/tile, index << 16 \| bin mask, 0xFFFE markers; remapped 8 bins × 256 (R2.05, R2.06, R2.19, R2.22) | `lightListRow` picture: the picked tile's list as a row of cells, grouped by type, markers dark; the remapped bins as eight rows | click a cell → the light (its sphere in 3D, linked); `walkList` action steps the iterator |
| Culling tests | sphere vs AABB, mask overlap (R2.18, R2.20) | 3D: tile AABB box, a light's sphere; pass/fail colour | `cullLight` picks which light; `extraLightRadius` |
| Falloff | (1 − (d²/R²)²)² / d² (R2.30) | `falloffChart` picture: inverse-square vs windowed curve; floor irradiance heat (`viewMode=irradiance`) | `lightRadius` |
| Light types | sphere/disk/capsule/quad approximations, LTC 32 × 32 tables (R2.33–R2.37) | 3D: the representative point on the light, the reflection ray; `ltcTable` picture of the 32 × 32 LUT with the pick's (roughness, θ) | `lightType` choice, roughness |

## Chapters 3–4: surface and BRDF

| Structure | Storage | Lab view | Interaction |
|---|---|---|---|
| Surface record at P | albedo, F0, roughnessLinear/A/A2, occlusions, emissive (R3.05–R3.13) | struct panel record `Surface` (fields as O3DE names them), `highlight=` the field said | pick; sphere sliders (metallic, roughness, F0 factor) |
| Per-light terms | D, G (V), F, diffuseResponse, N·L, intensity, litRatio (R3.16–R3.21, R4.28) | readout rows per light with formula terms lit; 3D vectors N, L, V, H; D lobe mesh at P | light orbit, roughness, camera pitch |
| BRDF LUT | 256 × 256 R16G16_FLOAT, R = scale A, G = bias B, 1024 Hammersley GGX samples, built once (R3.37) | `brdfLut` image kind: the two channels side by side, the pick's (N·V, 1 − roughness) texel outlined; `lutSamples` builds it progressively (`buildLut` action) | click a texel → (N·V, roughness) set on the sphere probe point |
| Multiscatter | 1 + F0 (1/(A + B) − 1) (R3.33) | white-furnace row of spheres (lab demo), loss and compensation readout | `multiscatter` toggle, roughness row |
| Lighting models | Base/Standard/Enhanced/Skin BRDFs (R3.21–R3.27) | per-model lobe; readout table | `teapotModel` choice, clear coat, anisotropy |

## Chapter 5: shadows

| Structure | Storage | Lab view | Interaction |
|---|---|---|---|
| Cascade shadow map | Texture2DArray D32_FLOAT, 4 × 1024², one slice per cascade, cleared 1.0 (O3DE forward-Z for shadows, R4.02) | lab depth views (`DepthViews`, 4 layers) at the cascades' ortho matrices; `cascadeMap` image kind showing one or all four slices side by side, titled "slice i · D32_FLOAT" | click a texel → its world point (linked); `pickCascade` |
| Split and fit | splits by ratio, bounding sphere per cascade, texel snapping (R4.05, R4.07, R4.08) | 3D: split planes in the camera frustum, each cascade's sphere and snapped box (debug colours R, G, B, Y as O3DE's) | `cascadeRatio`, `cascadeCount`, camera nudge |
| P's lookup | normal offset, shadowBias, ortho projection, compare (R4.16–R4.19) | struct record `ShadowLookup`; 3D: P, P + offset, its texel's footprint on the map | `normalShadowBias`, `shadowBias` |
| Fullscreen shadow buffer | R8_UNORM, screen size; normal rebuilt from depth; then depth-aware blur R = 8 (R4.24, R4.25) | `shadowBuffer` image kind: raw and blurred side by side; a scanline plot of the blur falloff | `shadowBlur` toggle, filter choice (None, PCF 4/9/16, ESM) |
| Projected atlas | D32 array, quadtree packing, 256 maps, point light six faces at 91° (R4.31–R4.32) | `atlas` picture (CPU mirror of the packing) with thumbnails; 3D: a point light's six frusta | `pointShadow` toggle |

## Chapter 6: indirect

| Structure | Storage | Lab view | Interaction |
|---|---|---|---|
| Global specular cube | R9G9B9E5, 6 mips (256 → 8), mip m = GGX for roughness m/5, (−x, z, −y) addressing (R5.19–R5.21) | `cubeSpecular` image kind: a cross layout of the six faces at a chosen mip, plus the mip strip; R's texel outlined | `cubeMip`, roughness (mip = 5 r) |
| Global diffuse cube | BC6UH 1 mip, 32–128, cosine-convolved (R5.22) | `cubeDiffuse` cross; N's texel outlined | pick |
| Probe | inner/outer OBB, baked cube, parallax on outer box (R5.17, R5.24–R5.28) | 3D: the boxes, R's hit, corrected direction; `probeCube` cross | `probeParallax`, teapot position |
| Blend weight | R16_FLOAT additive (R5.25) | `blendWeight` image kind | pick |
| Lab differs | the lab's sky is an equirect; the investigation builds the cubes from it (sizes in titles) | | |

## Chapter 7: occlusion

| Structure | Storage | Lab view | Interaction |
|---|---|---|---|
| Half-res depth, SSAO, blur, upsample | R32F half; R8 half; R8 half; R8 full (R6.07–R6.11) | four image kinds in a row (`ssaoHalfDepth`, `ssaoRaw`, `ssaoBlur`, `ssaoFull`), titled with names and formats | click → pick |
| P's 52 samples | 12 cross taps + 40 spiral, weights 1/(d² + 0.07) (R6.17–R6.22) | 3D sample points at P coloured by saturate(cos), sized by weight; readout running ratio | `ssaoSamples` action, `ssaoRadius` |

## Chapter 8: through surfaces

| Structure | Storage | Lab view | Interaction |
|---|---|---|---|
| Transmission params | float4 per mode, tint/thickness float4 (R7.19, R7.20) | struct record `Transmission` | mode choice, sliders |
| Thickness from the cascade | deltaDepth × camera range (non-metric, R7.14) | the cascade map row at P; readout: lab's exact metric thickness vs O3DE's | `transmissionShrink`, leaf distance |
| SSS diffuse alpha, scatter distance | diffuse.a packed factor/quality in fp16; ScatterDistance R11G11B10F (R7.23–R7.25) | `targetDiffuse` channel=alpha decoded; `scatterDistance` view; footprint circle | SSS factor, quality |
| Kernels | TransmissionKernel vs T(s) (R7.09) | `kernelChart` picture | thickness |

## Chapter 9: terrain and hair

| Structure | Storage | Lab view | Interaction |
|---|---|---|---|
| Terrain vertex normal | int8 xy, z rebuilt; per-axis normalised (R8.08, R8.11, R8.12) | `terrainNormalError` heat over the hills (O3DE form vs exact) | pick a vertex |
| Detail ID image | R8G8B8A8_UINT ids + blend (R8.19) | `detailIds` picture (two materials, a band) | blend width, height blending toggle |
| Hair lobes | R, TT, TRT (R8.41, R8.42) | lobe chart around a strand | lobe toggles |

## Subjects and fades

- Teapot: P, lobes, AO, transmission (thick). Sphere: BRDF sweeps, LUT probe, SSS, skin. Floor: shadows, cascades,
  decals, probe parallax. Rotated box fades when it hides the teapot's lit side; walls fade for outside cameras.

## Selftest checks for this phase

- Every image kind composes, validates, and renders a non-empty picture in its demonstration.
- A pick round-trips: a click on the scene at P, then a click on P's texel in each target view, set the same pick;
  a click on a tile picks the tile containing P; a cascade texel click maps back within one texel.
- The view titles and labels are at least 24 px and inside the viewport.
