# O3DE lighting: the story's outline

O3DE 2.8.0 (`d363107e`). Claims cite `research.md` by claim number (`R2.07`) or finding (`F6`), and the checkout by
file:line with the abbreviations of `research.md` §0. The story is rebuilt with the `new-story` process on the base
of the existing investigation (its port, CPU mirror, demonstrations, and storyboard); the agent decided the scope,
chapters, and budgets for the user (2026-10-29, `src/o3de/lighting/AGENTS.md`).

Total budget: 2,275 s (about 38 minutes), fifteen chapters.

## The subject and the scene

- **The study room** (`rlstd.scene/study-room`) lit by one simple point light and the global skylight, as
  today. StandardPBR everywhere; the teapot turns EnhancedPBR for transmission.
- **Subjects by chapter.** The floor beside the sphere for culling and decals; the sphere for the surface, the BRDF,
  clear coat, and multiscatter (its curvature sweeps N·L and N·V across one surface); the teapot for transmission;
  the polished floor for reflection probes; the corner behind the sphere for SSAO; the sphere's specular edge for the
  resolve.
- **Fade in close-ups.** The rotated box when it hides the sphere's lit side; the walls (`walls`) when the camera
  looks through them at the cubemaps. Both return for wide shots.
- **One, then many.** Each chapter works the inspected point (the readout, `panel=values`, a lit formula term), then
  sweeps the parameter across the room, then shows other objects.

## Chapters

### 1 · Where the lighting runs · 150 s

Bridge: the opening.

| Beat | Headline (the claim) | Claim, source → what shows it |
|---|---|---|
| c1-room | Atom lights the room in two halves | The room floating, the light orbiting. Opaque StandardPBR: the forward pass lights it from the point light; full-screen passes add the sky (R1.07, F17 context). |
| c1-pipeline | The default pipeline, with 2x MSAA | Main pipeline asset, `r_renderPipelinePath`, 2x MSAA (R1.01–R1.03; BootstrapSystemComponent.cpp:131; MainRenderPipeline.azasset:12-16) → callout; note the lab renders 4x (D1). |
| c1-frame | The opaque frame, pass by pass (mermaid) | Depth pre-pass → light culling → shadows → forward → diffuse GI → reflections → sky → composite → resolves → SSAO → merge (MainPipeline.pass, OpaqueParent.pass lines, R1.05–R1.15); `focus` lights each node as named. DiffuseProbeGrid gem replaces DiffuseGlobalFullscreen with DiffuseComposite in template projects (R7.07, F5). |
| c1-targets | Five targets out of the forward pass | Readout of the targets with formats: diffuse, specular RGBA16F (specular cleared to 0.4), albedo and F0 RGBA8, normal RGB10A2 octahedral with the IBL and multiscatter flags (Forward.pass:86-165, R1.08–R1.11); `viewMode` shows diffuse, specular, then all. |
| c1-stencil | Stencil bits route each pixel | Mesh stencil 0x83: 0x3 specular IBL, 0x80 diffuse GI; clear coat drops 0x3 (MeshFeatureProcessor.cpp:2421-2428, R1.12). Said with the listing. |
| c1-code | The pixel shader in five lines | `forward_pass` listing, `highlight` on surface → decals → lighting init → lights → IBL (transparent only) → targets (ForwardPassVertexAndPixel.azsli:46-101; ForwardPassEvaluateLighting.azsli:20-42). |
| c1-sources | Where to read | `forward-pixel`, `evaluate-lighting`, `msaa-default`, `opaque-pass-order`, `forward-targets`, `forward-ibl-stencil`, `diffuse-specular-merge`. |

### 2 · Light culling · 190 s

Bridge: from the frame's order to its first lighting step, which runs before any shading: which lights each tile needs.

| Beat | Headline | Claim → what shows it |
|---|---|---|
| c2-why | Every 16 × 16 tile gets its own list | 24 small lights (`extraLights`), false-colour counts (R2.01–R2.03). |
| c2-prepare | A tile's depth becomes a 32-bit mask | Tile box drawn (`cullTile`); listing `prepare`; mask linear between zNear and zFar, 2x path takes samples 0 and 1 (R2.04–R2.07; LightCullingTilePrepare.azsl). |
| c2-bins | One to eight bins, from the span in metres | Readout row for the bin count; thresholds 1.41/2.83/5.66 m (R2.08); camera pulls back so a tile's span grows and its bin count steps up. |
| c2-cull | A light is kept by box and mask | Sweep `extraLightRadius` 0.5 → 1.8: more tiles keep lights (R2.10–R2.13; CullPointLight). |
| c2-groups | Seven groups, in a fixed order | Listing `cull`; entry = index << 16 \| bin bits; END_OF_GROUP and END_OF_LIST (R2.15–R2.18). |
| c2-forward | The forward pass walks its bin's list | `extraLightMotion` sweep, ringed lights (R2.20–R2.22; LightCullingTileIterator). |
| c2-findings | The implicit order, read from the code | F1 polygon lights never shade (callout `polygon-not-culled`); F2 turning off punctual lights shifts the groups (listing `forward`, highlight on the skipped group). Said as read from the code. |
| c2-sources | Where to read | `light-culling-pass`, `tile-prepare`, `nvlc-constants`, `cull-point-light`, `light-culling-main`, `light-culling-remap`, `tile-iterator`, `polygon-not-culled`, plus a new `punctual-group-skip`. |

### 3 · The surface · 200 s

Bridge: with the lights sorted into lists, the next part turns to what they light: the surface.

| Beat | Headline | Claim → what shows it |
|---|---|---|
| c3-inputs | The material's inputs become a surface | `SurfaceData_StandardPBR`, not `StandardSurface` (R3.01); listing `surface_setup`. |
| c3-metal | Albedo and F0 from one base colour | `sphereMetallic` 0 → 1: albedo = base(1−m), F0 = lerp(0.08·factor, base, m) (R3.03–R3.05); readout rows lit. |
| c3-rough | Roughness squared, twice | `sphereRoughness` sweep; clamp 0.024 (R3.06); formula terms lit. |
| c3-aa | Specular AA widens rough-squared | `specularAA` toggle and a camera pull-back: edges of the sphere's highlight (R3.07–R3.08; ApplySpecularAA, screenVariance 0.25, cap 0.18). |
| c3-occlusion | Two occlusion maps, two powers | `sphereOcclusion`, factors swept (R3.10–R3.11). |
| c3-maps | What the lab leaves out of the surface | Normal maps, emissive (EV100 → nits 2^(EV−3)), parallax: said in passing with callouts (R3.09, R3.12, R3.13; D14). |
| c3-sources | Where to read | `surface-standardpbr`, `albedo-f0`, `roughness-values`, `specular-aa`, `occlusion-input`. |

### 4 · Decals · 160 s

Bridge: before any light reaches the surface, decals can change it.

| Beat | Headline | Claim → what shows it |
|---|---|---|
| c4-what | A texture projected through a box | `decals`, `decalBoxes` (R3.17). |
| c4-apply | Albedo, F0, and normal, never roughness | `decalOpacity` sweep; readout rows; roughness row unchanged (R3.18–R3.20, F22). |
| c4-angle | Fading with the angle | `decalTilt` 0 → 70: attenuation lerp(1, smoothstep(0.55, 1, n·z), angleAttenuation) (R3.19). |
| c4-order | Sorted by key, applied in order | `decalSplatSortKey` 15 ↔ 17 (R3.22). |
| c4-colour | A finding: decal colours skip the colour conversion | F6: material colours go through linear sRGB → ACEScg, decal colours do not (DecalTextureArrayFeatureProcessor.cpp:234-240; BaseColorInput.azsli:55). The readout shows the decal colour as stored and as ACEScg (new rows). |
| c4-sources | Where to read | `apply-decal`, `decal-attenuation`, `decals-forward`, `decal-cull`, `decal-sort`, new `decal-colour-raw`. |

### 5 · The simple point light · 150 s

Bridge: the surface is ready; now a light reaches it. The simplest one first.

| Beat | Headline | Claim → what shows it |
|---|---|---|
| c5-record | Candelas, a position, an inverse radius squared | Readout of the record (48 bytes); lumens / 4π (R5.01–R5.03). |
| c5-falloff | Inverse square, windowed to zero at r | `attenuationRadius` sweep; viewMode radius attenuation (R5.04–R5.05). |
| c5-auto | The automatic radius | r = sqrt(luminance·lm / 0.1) = 31.6 m by default; held at 1 mm (R5.06; F20 comment mismatch in passing). |
| c5-channels | Lighting channels | `sphereLightingChannels` 31 → 2: the sphere goes dark (R5.08). |
| c5-shadows | No shadows | Light behind the box still lights the floor (R5.09). |
| c5-sources | Where to read | `simple-point-light`, `simple-point-apply`, `attenuation-radius`, `lighting-channels`. |

### 6 · GGX specular · 150 s

Bridge: the light's intensity at the point is known; what the surface reflects of it is the BRDF, specular first.

| Beat | Headline | Claim → what shows it |
|---|---|---|
| c6-vectors | Four directions | N, L, V, H drawn and labelled; the light orbits (R4.03). |
| c6-terms | D, G, and F | Formula terms lit in turn; readout (R4.04–R4.07). |
| c6-sweep | The highlight follows H | Light orbit with `cameraDrift=0` close-up (R4.04). |
| c6-rough | Roughness widens D | `sphereRoughness` 0.15 → 0.7; lobe drawn (R4.04). |
| c6-sources | Where to read | `specular-ggx`, `ndf-ggx`, `visibility-smith`, `fresnel-schlick`. |

### 7 · Diffuse and its response · 110 s

Bridge: specular is one half; the light the surface scatters inside is the other.

| Beat | Headline | Claim → what shows it |
|---|---|---|
| c7-lambert | Lambert, times a response | viewMode diffuse response (R4.02, R4.08). |
| c7-response | The response dips at grazing angles | camera to grazing; readout row (R4.08). |
| c7-disney | A diffuse model StandardPBR does not use | NormalizedDisneyDiffuse: EnhancedPBR SSS and Skin; DiffuseTitanfall unused (R4.09). |
| c7-sources | Where to read | `diffuse-lambertian`, `diffuse-response`, `disney-diffuse`. |

### 8 · Clear coat · 130 s

Bridge: one layer so far; clear coat adds a second on top.

(As today's chapter 6, with the forward-IBL stencil from c1-stencil recalled.) Coat F0 0.04, Kelemen visibility,
base specular × (1 − Fc·factor), diffuse attenuated twice (R4.11–R4.12); forward-pass IBL for coated materials
(R7.05). Sources: `clear-coat-f0`, `clear-coat-specular`, `clear-coat-attenuation`, `forward-ibl-material`.

### 9 · The BRDF texture and multiscatter · 150 s

Bridge: single scattering loses energy at high roughness; this part puts it back, from a texture.

(As today's chapter 9.) The texture: 256², R16G16_FLOAT, 1024 Hammersley samples, (NdotV, smoothness) (R4.13–R4.15);
compensation 1 + F0(1/(A+B) − 1) on direct and IBL specular (R4.10); off unless a material sets it (F16); the lab's
GPU finding (README "Observed in the lab"). `sphereMetallic` 1 + `sphereRoughness` sweep shows the compensation
growing. Sources: `brdf-texture-cs`, `multiscatter`, `ms-material-property`, `brdf-texcoords`.

### 10 · Transmission (EnhancedPBR) · 150 s

Bridge: so far light only bounced off surfaces; the next part lets it pass through them.

(As today's chapter 8.) Sphere light (R5.12–R5.13), thick and thin modes (R5.15–R5.21), the thin-mode finding (F8),
and which lights transmit (R5.22). Sources unchanged.

### 11 · The sun's cascades · 170 s (new)

Bridge: point lights reach a few metres; the light that reaches everything is the directional one, and its shadows
are the only shadows the opaque pass computes before shading.

The lab has no directional light (D3); this chapter is drawn from a CPU computation that follows
DirectionalLightFeatureProcessor.cpp, on an observer camera inside the room, and says so on screen.

| Beat | Headline | Claim → what shows it |
|---|---|---|
| c11-light | Lux, and no angular size | 40 lux default (R6.01); angular diameter unused (F4, DirectionalLight.azsli:56-63); one shadowed light only (R6.05). |
| c11-splits | Four cascades, split between log and uniform | Observer frustum drawn with its four slices; `cascadeSplitRatio` 0 → 0.9 → 1 moves the splits; readout of the four far depths (R6.09–R6.10). `shadowFar` 100 → 10 m (said: scaled to the room). |
| c11-fit | Each slice in a sphere, snapped to texels | The bounding spheres and the light-space boxes; the observer turns and the boxes stay still, snapped (R6.11, R6.13). |
| c11-select | A pixel picks its cascade by bounds | The inspected point's coordinates in each cascade; the first inside [1.5, size − 1.5] wins (R6.19). |
| c11-fullscreen | Opaque pixels read one shadow value | Fullscreen shadow R8 target, blur radius 8, filter None by default (R6.20–R6.26), said with the listing. |
| c11-others | The rest of the forward pass's lights | Disk, capsule, quad (LTC), polygon in one sentence each (R6.30–R6.33); not in the lab. |
| c11-sources | Where to read | new entries `directional-lux`, `directional-angular-unused`, `cascade-splits`, `cascade-sphere-fit`, `cascade-snap`, `cascade-select`, `fullscreen-shadow`, `area-lights`. |

### 12 · The global skylight · 200 s

Bridge: every light so far is direct; the rest of the opaque frame adds the sky, after the forward pass.

(As today's chapter 11.) Corrections: DiffuseComposite replaces DiffuseGlobalFullscreen under the default template
(R7.07, F5); mip 0 of the specular cubemap is a one-texel blur, not a copy (R7.33, D8); the diffuse cubemap up to 128²
in the engine (D6). Sources as today plus `diffuse-composite`.

### 13 · Reflection probes · 250 s

Bridge: the sky's cubemap reflects the outdoors; inside, reflection probes replace it with the room.

(As today's chapter 12.)

### 14 · Resolve, ambient occlusion, and the merge · 190 s

Bridge: every term is now in the two targets, at two samples per pixel; the last passes resolve, darken, and add.

| Beat | Headline | Claim → what shows it |
|---|---|---|
| c14-resolve | Specular resolves with a clamp and a log mean | New port: `specularResolve` 0 → 1; viewMode resolve difference; the sphere's highlight edge (R8.03, F13). |
| c14-sss | Subsurface passes that do nothing here | Said in passing (F17). |
| c14-ssao | Fifty-two samples at half resolution | (Today's chapter 13 beats, condensed.) |
| c14-merge | The merge adds the two | `DiffuseSpecularMerge` (R8.12). |
| c14-sources | Where to read | `msaa-resolve-custom`, `ssao-*`, `diffuse-specular-merge`. |

### 15 · Recap · 75 s

Walk the frame again (mermaid), naming each part in order, from depth to merge.

## Worth a detailed demonstration

- The tile mask and bins (c2-prepare/c2-bins): the tile box in view space with its slices filled as the camera moves.
- The cascades (c11): four slices of the observer's frustum, their spheres, their snapped boxes, and the inspected
  point's choice, with the split ratio and shadow distance swept.
- The resolve (c14-resolve): a two-sample pixel's specular before and after the neighbour clamp.

## The tools it needs

- **New:** `cascades.rs` (CPU: splits, sphere fit, snapping, selection, following FP.cpp; overlay segments and
  readout rows), its parameters (`cascades`, `cascadesShown`, `cascadeSplitRatio`, `shadowFar`, `observerYaw`), a
  demonstration `cascades` with its dock group; `specularResolve` in the WGSL merge path with its CPU mirror and a
  selftest comparison, and a false-colour `viewMode` for the resolve's difference; decal colour readout rows.
- **Reused:** every existing demonstration, the readout, formulas, listings, the tile overlay, the probes' views.
- **Index:** new entries for the new claims (via `source_index.py`).
