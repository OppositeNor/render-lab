# Open 3D Engine: the lighting model

How O3DE's Atom renderer lights a **StandardPBR** surface, read from the O3DE source (version 2.8.0, the commit in
[`source-index.json`](../../../source-index.json)) and rebuilt in the lab. The scene pass shades the study room with
[`shaders/standard_lighting.wgsl`](../../../shaders/standard_lighting.wgsl), a WGSL port of O3DE's shader code that runs as O3DE's
opaque passes do: the forward pass writes O3DE's targets (diffuse and specular in RGBA16F, albedo and specularF0 in
RGBA8, the normal signed-octahedron-encoded in RGB10A2 with the IBL and multiscatter flags), then
`DiffuseGlobalFullscreen`, `ReflectionGlobalFullscreen` into its own reflection target, `ReflectionComposite`, and
`DiffuseSpecularMerge` run as compute passes, rebuilding positions from depth;
[`brdf.rs`](brdf.rs) mirrors it on the CPU for the readout and for `selftest`.

Covered so far, in the main pipeline's order: the depth pre-pass and **tiled light culling**; the **surface**
(the metallic workflow, specular anti-aliasing, occlusion maps); **decals**; **GGX specular** and **clear coat**;
**diffuse** and its response; **transmission** (EnhancedPBR, with O3DE's sphere point light); **multiscatter
compensation**; **simple point lights** and **lighting channels**; and the **global skylight's image-based lighting**.
and **reflection probes** with their stencil, blend weight, and composite. The other light types (area lights through LTC,
directional lights and their shadows) and the other material types come later. Last in the opaque frame comes
**screen-space ambient occlusion**.

## What the demonstrations show

| Key | Demonstration | O3DE code it follows |
|---|---|---|
| 1 | One point light, StandardPBR | `PixelShader` (MainPipeline forward pass), `EvaluateLighting_ForwardPass`, `DiffuseSpecularMerge` |
| 2 | Tiled light culling | `LightCullingTilePrepare`, `LightCulling` (`CullPointLight`), `LightCullingRemap`, `LightCullingTileIterator` |
| 3 | The metallic workflow | `SurfaceData_StandardPBR::SetAlbedoAndSpecularF0`, `GetAlbedo`, `CalculateSpecularF0`, `CalculateRoughnessValues`, `CalculateSpecularAA`, `GetOcclusionInput` |
| 4 | Decals | `ApplyDecal`, `GetDecalAttenuation`, `ApplyDecals_ForwardPass`, `CullDecals`, `SortDecals` |
| 5 | GGX specular: D, G, and F | `SpecularGGX`, `NormalDistributionGGX`, `GeometricShadowingMaskingGGXCorrelated`, `FresnelSchlick` |
| 6 | Clear coat | `ApplyClearCoatToSpecularF0`, `ClearCoatSpecular`, `ClearCoatGGX`, the clear-coat terms of `InitializeLightingData_StandardPBR`, `GetDiffuseLighting_StandardPBR`, and `ApplyIBL` |
| 7 | Diffuse and its response | `InitializeLightingData_BasePBR`, `FresnelSchlickWithRoughness`, `DiffuseLambertian` (and `NormalizedDisneyDiffuse` for comparison) |
| 8 | Transmission (EnhancedPBR) | `GetBackLighting_Default`, `TransmissionKernel`, `PointLightUtil_PBR::Apply`, `ApplyPointLight`, `FinalizeLighting` |
| 9 | Multiscatter compensation | `GetMultiScatterCompensation`, `LightingData_BasePBR::Init`, `BRDFTextureCS` |
| 0 | The simple point light | `SimplePointLightUtil_PBR`, `ApplySimplePointLight`, `IsSameLightChannel`, `SimplePointLightDelegate::CalculateAttenuationRadius` |
| = | Reflection probes | `ReflectionProbeStencil`, `ReflectionProbeBlendWeight`, `ReflectionGlobalFullscreen`, `ReflectionProbeRenderOuter` and `Inner`, `ComputeProbeSpecular`, `ApplyParallaxCorrectionOBB`, `ReflectionComposite` |
| (header) | Screen-space ambient occlusion | `DepthDownsample`, `SsaoCompute`, `FastDepthAwareBlurVer` and `Hor`, `DepthUpsample`, `ModulateTexture` |
| − | Image-based lighting | `DiffuseGlobalFullscreen`, `ReflectionGlobalFullscreen`, `ReflectionComposite`, `CCubeMapProcessor::FilterCubeSurfacesGGX` and its cosine filter, the IBL presets |

Each demonstration has a **readout** in the struct panel's place: every term at the inspected point, computed by the CPU
mirror. Click a surface to inspect another point. In the close-ups the readout, the N, L, V, H vectors, and the lobes
see the point from an **observer** (the blue marker), so the plane of N, L, and V faces the camera; the scene pass
always shades from the camera. The lobes sweep the outgoing direction through that plane: the green one is the diffuse
term (the radiance seen from each direction), the pink one the specular term times N·V (the light reflected into each
direction; the term alone grows toward the horizon, where G's division by N·V outpaces it). The dock's **view** menu switches the scene pass to one part of the result (diffuse,
specular) or one term in false colour (D, G, F, the compensation, the diffuse response, NdotL, the radius attenuation).
The Code tab has a Python pseudo-code listing per step ([`pseudocode/`](../../../pseudocode)).

In the light-culling demonstration a dot marks each small light; the lights the inspected point's bin keeps are ringed
and joined to the point, and the ones its tile lists for other bins are brighter than the rest. The **motion** slider
turns each height's ring of lights about the room's centre, at its own rate and direction, so lights cross tiles and
bins and the false-colour counts and the point's list follow them. Play animates each demonstration's parameters (the
lights' motion and radius, the light's angle and the roughness in the close-ups, and so on); in the video, the camera
floats around each scene and the lights move, and it holds still only while a step is shown in detail (the culling
lights moving, the specular lobes) and where the narration quotes a value at the inspected point.

The reflection-probe demonstration's floor is polished metal (metallic 1, `baseColor.factor` 4, roughness 0.15), so it
mirrors the walls the probes saw. Its **baked cubemap** controls show what a probe stores: the **cubemap cross**
unfolds the probe's six faces over the viewport (`-Z` at the centre; *faces baked* fills them in one by one, *mip*
walks the six prefiltered mips), and the **mirror ball**, like O3DE's visualization sphere, reflects the cubemap at the
probe's position.

## What the source shows

- **StandardPBR's surface** is `SurfaceData_StandardPBR`, not the generic `Surface` in `PBR/Surfaces/StandardSurface.azsli`.
  Albedo is `baseColor * (1 - metallic)`; specular F0 is `lerp(0.08 * specularF0Factor, baseColor, metallic)`, so the
  default factor 0.5 gives a dielectric F0 of 0.04. Roughness is clamped to 0.024, then squared twice (`roughnessA`,
  `roughnessA2`); then specular anti-aliasing, on by default (`ENABLE_SPECULARAA` 1, the `applySpecularAA` property
  true), widens `roughnessA2` by the normal's screen-space derivatives (by at most 0.18) and recomputes `roughnessA` and
  `roughnessLinear` from it. The lab ports it, taking the derivatives with `dpdxFine` and `dpdyFine` (the
  `specularAA` parameter).
- **Decals** change the surface before `InitializeLightingData`, so every light and the full-screen IBL passes (through
  the albedo, specularF0, and normal targets) see them. Light culling puts them in the first group of each tile's list:
  a sphere of radius `length(halfSize)` around the decal's box against the tile's box and depth mask, then sorted by
  sort key (the component's default is 16) and index, so a larger key is applied later, on top. `ApplyDecal` maps the
  point into the box (from -1 to 1), and inside it blends the base map times the decal colour into the albedo by
  `alpha * opacity * attenuation`, into specularF0 by that times metallic, and bends the normal by the normal map.
  The attenuation `lerp(1, smoothstep(0.55, 1, dot(vertexNormal, decalZ)), angleAttenuation)` fades a decal out
  by about 57 degrees from its projection axis.
- **Diffuse** is Lambertian, times a **diffuse response** of `1 - FresnelSchlickWithRoughness(NdotV, F0, roughness)`.
  That Fresnel is the view's, not the light's half vector's, and its grazing value is `max(1 - roughness, F0)`.
  `NormalizedDisneyDiffuse` (which carries its own Fresnel terms) is used by the Skin material and by EnhancedPBR when
  subsurface scattering is on; `DiffuseTitanfall`, defined beside them, is called nowhere in this checkout.
- **Specular** is `D * G * F * NdotL` with GGX's D, height-correlated Smith visibility (already divided by
  `4 NdotL NdotV`), and Schlick's F on `HdotV`, times a **multiscatter compensation** `1 + F0 (1 / (A + B) - 1)`
  (Turquin 2019). A and B come from a 256 x 256 BRDF texture that `BRDFTextureCS` fills once with 1024 GGX samples per
  texel, indexed by (NdotV, 1 - roughness), and stores it as R16G16_FLOAT (the lab keeps f32). The compensation's
  shader option is declared `true`, but StandardPBR connects it to the material property
  `specularF0.enableMultiScatterCompensation`, which has no default and so loads as `false`: compensation is **off**
  unless a material turns it on, as 37 of the 39 PBR presets do. The lab turns it on (the `multiscatter` parameter).
- **Transmission** is EnhancedPBR's (its mode defaults to None). Simple point lights add no back lighting, so it needs
  another light type; O3DE's point light, a sphere area light (bulb radius 0.05 m by the editor's default), lights like
  a simple one, moves its specular direction to the bulb's nearest point to the reflection ray, and adds
  `GetBackLighting`. ThickObject gives `saturate(V . -normalize(L + N distortion))^power * scale *
  exp(-t attenuation) * saturate(1 - t)` times the light, with `t` the larger of the shadow map's thickness and the
  material's; ThinObject runs a diffusion kernel over the shadow map's thickness. The sum, times the tint, joins diffuse
  after the occlusion multiply. *Finding:* the shadow branch runs for every point light (its options default to true and
  per-mesh option flags are off), and for a light without shadows `GetThickness` returns 0, so a thin object gets the
  kernel's full value, not the "no shadow data" exit its comment describes.
- **The simple point light** gives `candelas / max(d², (1 mm)²) * (1 - (d²/r²)²)²`, skips the light from its
  attenuation radius `r` on (and outside its lighting channels), and casts **no shadows**. In the automatic mode, the
  component's default, `r = sqrt(I / 0.1)`, where `I` is the colour's perceptual luminance times the lumens; `r` is
  held at 1 mm or more.
- The forward pass writes **diffuse and specular to separate R16G16B16A16_FLOAT targets** (emission goes to the
  specular one). The opaque passes then run in this order: `DiffuseGlobalFullscreen`, the reflections, the sky box,
  `ReflectionComposite`, the MSAA resolves (the main pipeline uses 2x MSAA), subsurface scattering, ambient occlusion
  (SSAO, on by default, darkens the diffuse target), and `DiffuseSpecularMerge`, which adds the two.
- **Opaque surfaces get image-based lighting after the forward pass**, not in it: the opaque forward shader defines
  `FORCE_OPAQUE`, so `ApplyIbl_ForwardPass` runs IBL only for transparent materials (or specular IBL when a mesh or
  material forward-specular option, false by default, is set). The forward pass writes `albedo * diffuseResponse *
  diffuseAO` (specular occlusion in alpha), specularF0, and roughness to **8-bit** R8G8B8A8_UNORM targets;
  `DiffuseGlobalFullscreen` adds that albedo times the diffuse cubemap at the normal onto the diffuse target, and
  `ReflectionGlobalFullscreen` takes the specular cubemap along the reflection (rotated by `m_iblOrientation`) at mip
  `roughness * 5`, times `F0 * A + B` and the multiscatter compensation (with NdotV held at 0.01 or more), the weight
  reflection probes leave, and the specular occlusion, into its own target, which `ReflectionComposite` adds onto
  specular. Both are multiplied by `2^m_iblExposure` (0 stops by default). The skylight casts no shadows; only occlusion
  maps and SSAO dim it. For interiors, reflection probes replace its specular part and the DiffuseProbeGrid gem its
  diffuse part (later chapters).
- **Reflection probes** run after the forward pass, in this order. The stencil pass draws each probe's inner box (0.1 m
  smaller) with depth testing, adding one to the stencil of every surface inside it; the blend-weight pass adds, outside
  every inner box (stencil exactly 3), each probe's weight into an R16_FLOAT target: 1 inside its inner box, falling to 0
  at its outer one. The global pass then runs only on stencil 3, weighted by `1 - min(W, 1)`; the outer pass adds each
  probe's specular times `w / max(1, W)`; the inner pass, with no blending, overwrites with the probe's specular wherever
  its inner box holds the surface, probes sorted by descending inner volume so smaller ones win. Probes use their own
  cubemap with parallax correction against the outer box (on by default) and, unlike the global pass, no specular
  occlusion. Materials with forward-pass IBL (clear coat) skip these passes; their forward pass blends in the smallest probe
  overlapping the mesh, by the same inner-to-outer weight, with that probe's exposure. The reflection target is cleared
  to (0.4, 0.4, 0.4); *finding:* with the IBL flag off, the inner pass
  discards and surfaces inside an inner box get that grey added to their specular. Screen-space reflections are
  disabled by default.
- **A probe's bake** (`EnvironmentCubeMapPass`) renders six square 90-degree views from the probe's position (near
  0.1 m, far 100 m), one face per frame into a 1024-texel R16G16B16A16_FLOAT target, each read back five frames after
  it is drawn. The editor writes the faces to a DDS cubemap named with the quality's suffix (`_iblspecularcm256` at the
  default Medium), and the image builder applies the skylight's `IBLSpecular` preset to it: 256-texel faces, GGX, six
  mips, R9G9B9E5. *Show Visualization*, on by default, draws a sphere at each probe with a white, metallic,
  roughness-0 material; the capture excludes it.
- **SSAO** (on by default; GTAO is the disabled alternative) runs after subsurface scattering on the resolved diffuse
  target. Linear depth comes from the depth resolve's sample 0; the half-res depth takes the upper-left pixel of each
  2 x 2. `SsaoCompute` rebuilds a position and a normal from depth and takes 12 local and 10 x 4 shared samples, each
  weighted `1 / (d^2 + 0.07)`, for `(1 - AO * strength)^2.2` in 8 bits (strength 1, radius 0.05 of the half-res
  height). A depth-aware recursive blur runs down, then across, each output half from 8 pixels before and half from 8
  after; `DepthUpsample` weighs four half-res pixels by distance and depth; the diffuse target is multiplied by the
  result, so SSAO darkens direct diffuse light and diffuse IBL alike and leaves specular alone. Without an AO
  component the blur keeps its pass's defaults (falloff 2/3, depth strength 50); the component's own are 0.85 and 200.
- **The cubemaps are prefiltered offline** by the image processor with AMD's CubeMapGen. The specular one has 256²
  faces and exactly six mips, mip `m` filtered for roughness `m / 5` with 256 GGX samples per texel (N = V = the texel's
  direction), each sample read one mip blurrier than the mip whose texels match its solid angle and blended between two
  mips; mip 0 is copied, not convolved. Those are the default preset's sizes: the High and VeryHigh presets make 512 and
  1024. The diffuse one is a cosine
  filter over the sphere, normalised by its weights, so it holds irradiance / π, which is why the diffuse pass
  multiplies by albedo with no 1 / π. CubeMapGen's lookups map a face onto texel indices rather than texel centres, so
  they sit up to half a texel off at the face's edges.

### Observed in the lab

On the GPU the lab ran on (an NVIDIA RTX 4050 under Vulkan), the WGSL port of `BRDFTextureCS` loses energy at low
roughness: `ImportanceSampleGGX` takes `sqrt(1.0 - cosTheta * cosTheta)` unguarded, and when the GPU's division rounds
`cosTheta` above 1 the sample becomes NaN and is dropped. With the square root guarded, the port matches the CPU. The
loss is largest below roughness 0.03 (A + B is 0.41 instead of 1.0 at roughness 0.002); because roughness is clamped at
0.024, lighting reads those rows only for the smoothest surfaces. Whether O3DE's own texture shows the same loss depends
on the GPU's division, and this was not measured in O3DE. The lab fills the texture on the CPU, so its frames are the
same on every GPU. `selftest` reports the texels that lose energy.

## What the lab leaves out

The port keeps O3DE's arithmetic in f32 (O3DE's `real` can be half precision) and leaves out
the skylight's orientation, screen-space reflections (off by default), and the other light types. SSAO's compute
shaders share samples through groupshared memory; the lab computes each pixel on its own (its result depends only on
its place in its group), keeps each stage in a buffer of f32 rounded to 8 bits where O3DE's targets are R8_UNORM,
rounds half-resolution sizes up, and does `ModulateWithSsao`'s multiply in the merge pass. The
reflection passes (probe stencil, blend weight, global, outer, inner, and the composite's test) run per sample in one
compute pass, in O3DE's order; each reads only its own sample, so the result is the same. The probe stencil is the
count of reduced inner boxes holding the surface, which is what the depth-fail operations give for a box the camera
is outside of or inside. The probes' cubemaps are baked by the lab on the CPU, shading the room with the CPU mirror
(the point light and the global skylight, at the room demonstration's start) from each probe's centre, straight at
256 texels a face, then prefiltered with the skylight's GGX filter; O3DE renders the bake through its pipeline at 1024
texels and the image builder downsamples it. The probes' boxes are the lab's, with their faces on the walls, since
parallax correction draws whatever a probe saw as if it lay on its outer box: the first covers the room, the second,
smaller, fits the back-right corner and replaces the first there. The cubemap cross and the mirror ball are the lab's views, drawn
in the merge pass; the ball reads the cubemap along the reflection ray, without the reflection passes. EnhancedPBR's transmission
uses O3DE's sphere point light without shadows. Light culling appends a group's lights in index order, where O3DE's threads append them in the order they finish;
it sorts a tile's decals with an insertion sort, where O3DE uses a bitonic sort across the group (both give the same
order). The decal and occlusion textures are the lab's own, procedural, read at mip 0 only from a storage buffer with the
samplers' filtering and addressing done by hand; the decal textures are not in texture arrays. The BRDF
texture is f32 rather than R16G16_FLOAT, and multiscatter compensation is on, where StandardPBR's material default is
off. The environment is the lab's (Poly Haven's "Tree Lined Driveway" photograph, `lab/src/environment.rs`), with its sun
clipped, since the lab has no directional light to carry it, and scaled to a mean radiance of 0.31; it is indexed by the lab's +Y-up directions
(O3DE's `GetCubemapCoords` converts its +Z-up ones). The cubemaps stay in f32 rather than R9G9B9E5 or BC6H; the
diffuse one is 32² (O3DE allows 32 to 128); and the run-time lookups clamp at a face's edge where a GPU filters
seamlessly. The lab renders with 4x MSAA where O3DE's main pipeline uses 2x; its full-screen passes run per sample, as
O3DE's do, and average their samples where O3DE resolves after the composite. Compute passes cannot blend, so where O3DE
adds onto a target (One/One), the lab writes the sum to a new one. The lab has no sky box pass: the merge pass draws the
environment where samples hit nothing (a dark background in the false-colour views), and a false-colour target carries the dock's views. The scene is the lab's study room, the tonemapper is the lab's, and
`DiffuseSpecularMerge`'s neighbourhood blend for subsurface scattering is not used.

## Files

| Path | Contents |
|---|---|
| `mod.rs` | The `Investigation`: demonstrations, the scene shading (`SHADING`), picking |
| `shaders/standard_lighting.wgsl` | The WGSL port: surface, BRDF, BRDF texture, point light, the pixel, and the selftest kernels |
| `brdf.rs` | The CPU mirror of every shader function, and the BRDF texture |
| `ibl.rs` | The environment, its specular and diffuse cubemaps (CubeMapGen's filters), and the CPU lookups |
| `culling.rs`, `decals.rs`, `transmission.rs`, `reflections.rs`, `ssao.rs`, `maps.rs` | The CPU mirrors of light culling, the decals, and transmission with the sphere light; the procedural occlusion and decal textures |
| `shot.rs`, `demos.rs`, `controls.rs` | Parameters and animations, what each shot shows, the dock |
| `source-index.json`, `source_index.py` | Every cited O3DE file, symbol, and line; the script writes the index from the checkout |
| `pseudocode/` | The Python listings |
| `formulas.rs` | The readout's formulas: one formula tag per demonstration in `text/*.json` (`formula.<demo>`), and the lit term |
| `selftest.rs` | The verification suite (with `--timeline`, every scene beat of the video) |
| `video/` | The narrated video's inputs (its timelines are built into the module's `build/timelines/`) |

## Formulas

Above its readout, each demonstration shows the formula it evaluates: the forward pass's sum, the metallic workflow and
the roughness remap, SpecularGGX with D, G (the height-correlated Smith visibility) and F, the Lambertian diffuse and its
response, the multiscatter compensation, the point light's falloff and window, and the split-sum IBL. The `formula`
parameter (the dock's "formula term" slider) lights one term in the colour of its readout row; 0 lights none, and -1
hides the formula. Each formula is a tag in [`text/en.json`](../../../text/en.json) and [`text/zh-CN.json`](../../../text/zh-CN.json)
(`formula.<demo>`: its LaTeX, with `\lit{n}{…}` marking the terms and `colors` their readout rows' colours), typeset by
the lab at run time (`lab/src/formula.rs`); the selftest checks that each typesets within the panel.

## The video

[`video/`](../../../video) holds the narrated video's storyboard (`storyboard.story`), its diagrams, the zh-CN translation (`storyboard.zh-CN.story`), and
`video.json`; [`tools/video`](../../../../../tools/video/README.md) builds it. It runs about 35 minutes in fourteen chapters,
following the opaque frame's passes in order. The timelines it writes into the module, `build/timelines/` (generated, so
git ignores them; `--prepare-only` builds them in a fresh checkout), are what the homepage's Watch plays; the narration
and the video itself go to its `build/video/`.

```sh
python3 tools/video/build.py --investigation o3de/lighting [--lang zh-CN] [--snapshots-only]
```

## Verifying

```sh
cargo run --release -- --investigation o3de/lighting selftest --report o3de.json
python3 src/o3de/lighting/source_index.py   # after changing an entry or moving to another O3DE commit
```

`selftest` compares every BRDF term, the cubemap lookups, whole pixels in every view, the BRDF texture, and the
rendered frame with the CPU mirror; checks that GGX's D integrates to 1, that the texture conserves energy, and that
prefiltering keeps a constant environment constant (a white furnace); renders every demonstration; and,
when the checkout is present, checks every cited line's hash against it.
