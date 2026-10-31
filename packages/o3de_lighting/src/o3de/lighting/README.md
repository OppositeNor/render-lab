# Open 3D Engine: the lighting model

How O3DE's Atom renderer lights a frame, read from the O3DE source (26.05, engine version 2.6.0, the commit in
[`source-index.json`](../../../source-index.json)) and rebuilt in the lab. The scene shading,
[`shaders/standard_lighting.wgsl`](../../../shaders/standard_lighting.wgsl), is a WGSL port of O3DE's shader code that runs
as O3DE's default `MainRenderPipeline` does: a depth pre-pass; tiled light culling (`LightCullingTilePrepare`,
`LightCulling`, `LightCullingRemap`); the sun's four cascaded shadow maps (drawn into the lab's depth views) and the
`FullscreenShadow` pass with its depth-aware blur; the forward pass writing O3DE's five targets (diffuse and specular
RGBA16F, albedo and specularF0 RGBA8, the signed-octahedral normal RGB10A2 with its two flags) and the stencil
reference as data; then `DiffuseGlobalFullscreen`, the reflection passes (global skylight and probes),
`ReflectionComposite`, SSAO, and `DiffuseSpecularMerge` as compute passes. [`brdf.rs`](brdf.rs) mirrors the shading on the
CPU for the readout and the selftest; [`cascades.rs`](cascades.rs), [`culling.rs`](culling.rs), [`ibl.rs`](ibl.rs),
[`reflections.rs`](reflections.rs), [`ssao.rs`](ssao.rs), [`decals.rs`](decals.rs), and [`transmission.rs`](transmission.rs)
follow the engine's arithmetic for theirs.

The story behind the video (research, outline, demonstrations, reviews) is in [`story/`](../../../story).

## What the demonstrations show

| Demonstration | What it shows | O3DE code it follows |
|---|---|---|
| room | the study room under the sun and a point light; the five targets, the stencil, the frame's stages (`forwardStage`) | `PixelShader`, `EvaluateLighting_ForwardPass`, `DiffuseSpecularMerge` |
| culling | 24 point lights, the tiles, depth masks, bins, and lists | `LightCullingTilePrepare`, `CullPointLight`, `LightCullingRemap`, `LightCullingTileIterator` |
| surface, decals | the metallic workflow, roughness, specular AA, occlusion; decals | `SetAlbedoAndSpecularF0`, `CalculateRoughnessValues`, `CalculateSpecularAA`, `ApplyDecal` |
| specular, diffuse, clearcoat, multiscatter, pointlight | GGX's D, G, F; the diffuse response; clear coat; the BRDF look-up texture and multiscatter; the falloff window | `SpecularGGX`, `DiffuseLambertian`, `ClearCoatSpecular`, `BRDFTextureCS`, `GetMultiScatterCompensation`, `SimplePointLightUtil_PBR` |
| cascades | the sun's cascades, rendered on the room's wide grounds, and the fullscreen shadow buffer | `UpdateBorderDepthsForSegments`, `CalculateShadowViewAabb`, `SnapAabbToPixelIncrements`, `ComputeShadowCoords`, `GetShadowCascadeRange`, `FullscreenShadow`, `FastDepthAwareBlur` |
| ibl, reflections | the global skylight's cubemaps; two reflection probes, their stencil, weights, and parallax | `GetIblDiffuse`, `ReflectionGlobalFullscreen`, `ReflectionProbeRenderOuter`/`Inner`, `ApplyParallaxCorrectionOBB` |
| ssao | SSAO's stages and the 52 samples of one pixel (`ssaoSamples`) | `SsaoCompute`, `FastDepthAwareBlur`, `DepthUpsample`, `ModulateTexture` |
| transmission | an EnhancedPBR teapot lit from behind by the sun, thick and thin, its thickness from the cascades | `GetBackLighting_Default`, `TransmissionKernel`, `GetThickness` |
| terrain | the lab's hills shaded as `TerrainPBR_ForwardPass` shades terrain, with O3DE's CPU vertex normals (`terrainO3deNormal`) | `TerrainPBR_ForwardPass`, `TerrainMeshManager` normals |

Image kinds (GPU views a story shows by key, `render-lab --investigation o3de/lighting images`): `chainTarget` (any
forward target, the depth, or the stencil), `tileGrid`, `lightList`, `cascadeMap`, `shadowBuffer`, `cubemap`,
`brdfLut`, `ssaoStage`. Each rings the inspected point (`pickShown`); a click on a view or the scene picks it
(`pickPixel`).

## What the lab leaves out

- 4x MSAA (O3DE's default is 2x); the stencil reference is carried as data.
- The cascades use reversed depth in the lab's depth views (the stored value is 1 - O3DE's depth); the shadow buffers
  are floats rounded to 8 bits. Only the None filter is ported (PCF, ESM, spot and point shadows, and the atlas are
  described, not rendered).
- The IBL and probe cubemaps are made by the lab from its sky and room, with O3DE's filters and sizes.
- Light types beyond simple point and sphere lights, the SSS pass, the diffuse probe grid, SSR, and hair are described,
  not run.
- The terrain is the lab's 97 x 97 heightfield, without O3DE's sectors, LODs, clipmaps, or macro and detail materials.
- The lab's world is Y-up (O3DE's is Z-up).

## Files

| File | Holds |
|---|---|
| `mod.rs` | the `Investigation`, the pass chain (targets, buffers, passes, depth views) |
| `values.rs`, `shot.rs`, `demos.rs`, `controls.rs` | the values, the demonstrations and Play, the frames and readouts, the dock |
| `actions.rs` | the pick and its click handler |
| `brdf.rs` and the other mirrors | the CPU copies of the engine's arithmetic |
| `selftest.rs` | the verification suite |
| `source_index.py` | writes `source-index.json` from the checkout |
| `../../../shaders/` | the port (`standard_lighting.wgsl`) and the image kinds (`views_*.wgsl`) |
| `../../../video/` | the storyboard, its translation, the diagrams, `video.json` |

## The video

```sh
python3 tools/video/build.py --investigation o3de/lighting --prepare-only
python3 tools/video/build.py --investigation o3de/lighting
python3 tools/video/build.py --investigation o3de/lighting --lang zh-CN
```

## Verifying

```sh
cargo run --release -- --investigation o3de/lighting selftest --report o3de.json
python3 packages/o3de_lighting/src/o3de/lighting/source_index.py   # after changing an entry or moving to another O3DE commit
```

`selftest` compares every BRDF term, the cubemap lookups, whole pixels, the BRDF texture, and the rendered frame with
the CPU mirror; checks energy and white-furnace invariants; checks that a pick round-trips; renders every
demonstration; and, when the checkout is present, checks every cited line's hash against it.
