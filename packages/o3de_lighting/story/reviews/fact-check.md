# Fact check of storyboard.story against the O3DE 2.8.0 checkout (d363107)

Method: every factual sentence about O3DE and the `inspect` text of all 143 callout-cited entries were read against the checkout (not against the drafts). Lab-only sentences (demos, params, viewer, false colour, lab numbers) are skipped.
Classes: exact, imprecise, wrong, unverifiable.

Path abbreviations (all under `/home/oppositenormal/Documents/programming/other/o3de/`):
- SL = `Gems/Atom/Feature/Common/Assets/ShaderLib/Atom/Features`
- SH = `Gems/Atom/Feature/Common/Assets/Shaders`
- PS = `Gems/Atom/Feature/Common/Assets/Passes`
- CL = `Gems/Atom/Feature/Common/Code/Source/CoreLights`
- CF = `Gems/AtomLyIntegration/CommonFeatures/Code`

## Summary of imprecise and wrong items

| # | Where | Class | One-line correction |
|---|---|---|---|
| 1 | c11-fit "A sphere does not turn, so the boxes hold still as the observer looks around" | wrong | The box keeps its size, not its place: the sphere centre is `camera.TransformPoint((0, depthCenter, 0))`, so it swings with the camera; snapping keeps texel edges fixed in the world. |
| 2 | entry `cascade-sphere-fit` inspect "so it does not change as the camera turns" | wrong (same cause) | "its size does not change as the camera turns; its position follows the camera and is snapped". |
| 3 | c11-fit "centred where the distances to its near and far corners are equal" | imprecise | Centre is `min(depthCenter, depthFar)`; with fov 90 deg and 16:9 the equal-distance point always lies beyond the far plane, so the sphere is centred on the slice's far plane. |
| 4 | c11-splits "a hundred metres by default" | imprecise | Far depth is `min(camera far clip, shadow far distance)`. |
| 5 | c11-light angular diameter "the size changes nothing" | imprecise | True in the forward pass; the ray-traced reflection shader (off by default) does use it. Say "in the forward pass". |
| 6 | c2-prepare "one bit for each slice that holds a surface"; entry `tile-prepare` "one bit per sample" | imprecise | Opaque samples set one bit each, but transparent geometry sets every slice between its nearest and farthest depth (filled mask). |
| 7 | c2-finding "the area lights after them read their neighbours' groups" | imprecise | Each later group reads the group two places earlier (point reads simple-point, disk reads simple-spot, capsule reads point, quad reads disk, polygon reads capsule); the quad group is never read. |
| 8 | c5-radius "square root of the light's lumens, weighted ..., over zero point one" | imprecise (ambiguous) | It is sqrt(weighted lumens / 0.1), not sqrt(weighted lumens) / 0.1. |
| 9 | c13-bake "draws one face per frame ... reads each back five frames later" | imprecise | One face is drawn (every frame) for five frames, then read back, then the camera turns to the next face. |
| 10 | c13-blend "one minus their summed weight, at most one" | imprecise (ambiguous) | The summed weight is clamped to one first: global weight = 1 - min(sum, 1). |
| 11 | c1-frame "applies decals, evaluates the material and its lights" | imprecise | The material's Surface is evaluated first; decals then modify it, then the lights run. |
| 12 | c1-frame DiffuseProbeGrid "applies the same sky light where no probe grid is placed" | imprecise | Same cubemap term, but the composite also skips pixels whose IBL flag is off and honours the debug toggles; the sky weight is 1 - probe weight. |
| 13 | c15 "the forward pass's eight-bit targets" | imprecise | Albedo and specularF0 are R8G8B8A8; the normal target is R10G10B10A2. |
| 14 | entry `ms-preset` "37 of the 39" | imprecise | 38 `.material` presets (the 39th file is `metal.txt`); 37 of the 38 set it (all metals; `default_grid` does not). |
| 15 | c3-metal "suits only transitions such as dust on metal" | unverifiable | Modelling advice; the code only lerps. Cut or attribute it. |

## Priority items asked for

| Item | Verdict | Evidence |
|---|---|---|
| c11 lux, 4 EV100 = 40 lux | exact | `CF/.../DirectionalLightComponentConfig.h` m_intensityMode `Ev100Illuminance`, m_intensity 4.0; `Gems/Atom/Feature/Common/Code/Source/CoreLights/PhotometricValue.cpp:100` = 250 * 2^4 / (100 * 1) = 40. |
| c11 angular diameter 0.5 deg default, unused | exact for forward pass | Default `m_angularDiameter = 0.5f` (config h); `SL/PBR/Lights/DirectionalLight.azsli:55-63`: `viewDirToLight` computed, `GetSpecularLighting(..., dirToLight, ...)` uses the plain direction. Item 5 above. |
| one shadowed light | exact | `CL/DirectionalLightFeatureProcessor.cpp:462-470` `SetShadowEnabled` keeps a single `m_shadowingLightHandle` (last enabled wins; disabling any light resets it); `SL/Pipeline/Forward/ForwardPassDirectionalLights.azsli:66` only `index == shadowIndex` gets the visibility. |
| split formula | exact | `CL/DirectionalLightFeatureProcessor.cpp:1337-1343`: border = r*near*(far/near)^((i+1)/N) + (1-r)*(near + (far-near)(i+1)/N); last = far; ratio 0.9, 4 cascades, 100 m, 1024 in config h. |
| sphere fit | imprecise / wrong | Items 1-3. Radius and centre: `cpp:1575-1614`, `cpp:113`; box = centre +/- radius (`cpp:1496-1500`), y extended to the entire-frustum sphere and `camera.y - far` (`cpp:1512-1535`). |
| snapping | exact | `cpp:1369-1387`: min and max divided by texel = (max-min)/size, floored, multiplied back; applied to the ortho matrix (`cpp:1411-1415`). Floors min and max separately, as narrated. |
| cascade selection, 1.5 texels both sides | exact | `SL/Shadow/DirectionalLightShadowCalculator.azsli:18` PIXEL_MARGIN 1.5; `:109-135` first cascade with xy in [1.5, size-1.5]. Note: the max cascade can extend when z > 1-1e-8 (light ray passes through the nearer cascade's depth range); the first cascade is by xy bounds. |
| normal offset 2.5 * 2 / size | exact | `SL/Shadow/NormalOffsetShadows.azsli:16-19` offset = n * bias * 2/size; bias default 2.5 (config h); = 4.9 mm at 1024. Note: in the full-screen pass the normal is rebuilt from depth differences (`SH/Shadow/FullscreenShadow.azsl`), not the vertex normal. |
| full-screen shadow R8, one value per pixel | exact | `PS/FullscreenShadow.pass:68` R8_UNORM sized by PipelineOutput; `PS/ShadowParent.pass` blur (`FullscreenShadowBlur`) feeds `FullscreenShadow` slot; forward reads `m_fullscreenShadow.Load` (`SL/Shadow/DirectionalLightShadow.azsli:134`). Note the shortcut applies to Opaque and Cutout modes (`:56-66`). |
| c14 MSAAResolveCustom: diagonal neighbours | exact | `SH/PostProcessing/MSAAResolveCustom.azsl:117-141` loads (x+-1, y+-1) for every sample. |
| brightest luminance, 1.5 contrast | exact | `:156` `lumFactor = maxNeighborContrast * brightestNeighborLuminance / sampleLuminance`; `sampleColor *= min(1, lumFactor)`; `PS/OpaqueParent.pass:421-428` enableNeighborClamping 1, maxNeighborContrast 1.5; `CLAMPING_STRATEGY` is `CLAMP_NEIGHBOR_MAX_LUMINANCE` (`:29`). |
| log-domain mean | exact | `:62`, `:75` `LOG_TONEMAPPING_STRATEGY`: log, sum, divide by N, exp = per-channel geometric mean. |
| c2 o_enablePunctualLights shifts groups | exact (see item 7) | `SL/Pipeline/Forward/ForwardPassDirectLighting.azsli:30-34` groups walked only inside the `if`; each `ApplyX` does `LoadAdvance` then loops to END_OF_GROUP (`.../ForwardPassSimplePointLights.azsli:36-44`); culling order `SH/LightCulling/LightCulling.azsl:608-650`; decals keep walking (`ForwardPassDecals.azsli:25` comment). Material property `enablePunctualLights` (`Materials/Types/MaterialInputs/GeneralCommonPropertyGroup.json:63-71`) feeds the option. |
| c4 decal colour not converted to ACEScg | exact | `Gems/Atom/Feature/Common/Code/Source/Decals/DecalTextureArrayFeatureProcessor.cpp:234-241` stores colour with no `TransformColor`; `SL/PBR/Decals.azsli:140-143` blends sampled base map (also unconverted) times colour into `surface.albedo`; materials: `Gems/Atom/RPI/Code/Source/RPI.Public/ColorManagement/TransformColor.cpp:27`, `SH/Materials/MaterialInputs/BaseColorInput.azsli:55`. Optional addition: the decal texture is unconverted too, and `m_decalColorFactor` multiplies in. |
| c1 DiffuseProbeGrid sentence | imprecise (item 12), core exact | Template enables gem: `Templates/DefaultProject/Template/project.json:27`; `Gems/DiffuseProbeGrid/Code/Source/Render/DiffuseProbeGridFeatureProcessor.cpp:889-922` adds passes to pipelines that have `DiffuseGlobalFullscreenPass` and calls `SetEnabled(false)` on it; composite adds `globalDiffuse * (1 - probeWeight)` (`Assets/Shaders/DiffuseGlobalIllumination/DiffuseComposite.azsl:~170-185`). |
| c9 multiscatter compensation defaults off for StandardPBR materials | exact | `Materials/Types/MaterialInputs/SpecularPropertyGroup.json:50-58` no `defaultValue`; `Gems/Atom/RPI/Code/Source/RPI.Edit/Material/MaterialPropertySerializer.cpp:227` Bool without default = false; shader option itself defaults true (`SL/PBR/LightingOptions.azsli:82`); metal presets set true (37 of 38 `.material` presets). |

## Beat-by-beat table

### Chapter 1

| Beat | Sentence / entry | Class | Evidence | Correction |
|---|---|---|---|---|
| c1-room | "Open 3D Engine's Atom renderer ... StandardPBR" | exact | (framing) | |
| c1-room | "a simple point light ... no shape and no shadows" (point-light-loop) | exact | `SL/Pipeline/Forward/ForwardPassSimplePointLights.azsli:11-30` litRatio 1, no shape | |
| c1-room | "version two point eight" (engine-version) | exact | `engine.json:4` "2.8.0" | |
| c1-room | "lit twice: once by the light in forward, once by the sky in later passes" | exact | OpaqueParent order | |
| c1-frame | depth pre-pass, light culling sorts lights into screen tiles | exact | `PS/MainPipeline.pass:47,94` | |
| c1-frame | "applies decals, evaluates the material and its lights, writes diffuse and specular into two targets" | imprecise | `SL/Pipeline/Forward/ForwardPassEvaluateLighting.azsli:31-46`; surface evaluated before in `Materials/Pipelines/MainPipeline/ForwardPassVertexAndPixel.azsli` | "evaluates the material, applies decals, then each light" |
| c1-frame | forward-targets: albedo x diffuse response, specular colour with roughness, normal | exact | `ForwardPassVertexAndPixel.azsli:84-98` (also x diffuse AO; spec occlusion in alpha) | |
| c1-frame | full-screen passes add sky diffuse, reflections; SSAO on diffuse; merge | exact | `PS/OpaqueParent.pass:235-505` order | |
| c1-frame | DiffuseProbeGrid composite | imprecise | priority table above | item 12 |
| c1-code | surface, init, lights (directional, simple point/spot, area) | exact | `ForwardPassDirectLighting.azsli:22-42` | |
| c1-code | IBL only for transparent / mesh or material forward-specular | exact | `SL/Pipeline/Forward/ForwardPassIbl.azsli:15-37`, both options default false | |
| c1-code | diffuse and specular separate targets, merge sums | exact | `SH/PostProcessing/DiffuseSpecularMerge.azsl:70` | |
| c1-views | sky light added by later passes | exact | | |
| c1-sources | "build checks each line is still there" | unverifiable | lab tooling | |

### Chapter 2

| Beat | Sentence / entry | Class | Evidence | Correction |
|---|---|---|---|---|
| c2-why | O3DE decides per screen part which lights it needs | exact | | |
| c2-why | tiles of 16 x 16 px, own list (light-culling-tile) | exact | `SL/LightCulling/LightCullingShared.azsli:14-15` | |
| c2-why | "Three compute passes ... after a depth pre-pass and before any shading" | exact | `PS/LightCullingParent.pass`: TilePrepare, LightCulling, Remap (plus two transparent-depth raster passes feeding TilePrepare) | |
| c2-prepare | one thread per pixel, one group per tile | exact | `SH/LightCulling/LightCullingTilePrepare.azsl:263` numthreads(16,16,1) | |
| c2-prepare | nearest depth, farthest short of the sky, opaque and transparent | exact | `SL/LightCulling/NVLC.azsli:238-247` (`InfDepthToNearPlane`), `LightCullingTilePrepare.azsl:285-300` | |
| c2-prepare | "32-slice mask, one bit per slice that holds a surface" | imprecise | transparent range is filled: `LightCullingTilePrepare.azsl:~150-190` | item 6 |
| c2-prepare | 1, 2, 4, or 8 bins from log2 of span in metres | exact | `LightCullingTilePrepare.azsl:~196-205` ratio = span/1 m, log2 clamped 0..3, rounded | |
| c2-cull | light kept when sphere touches tile box and depth range touches mask | exact | `SH/LightCulling/LightCulling.azsl:~145-150, 293-300`; `NVLC.azsli:139-179` | |
| c2-cull | index << 16 with one bit per bin (light-index-pack) | exact | `NVLC.azsli:80-83` | |
| c2-cull | groups: decals, simple point, spot, area; end markers | exact | `LightCulling.azsl:608-650`; END_OF_GROUP 0xFFFE | |
| c2-cull | threads append in finish order, decals sorted | exact | `LightCulling.azsl:~186-195` InterlockedAdd; `:~136-170` bitonic decal sort | |
| c2-forward | remap: one list per depth bin; forward walks its bin | exact | `LightCullingRemap.azsl:100-166`; `LightCullingTileIterator.azsli:19-45` | |
| c2-finding | forward walks a polygon group; culling writes none; polygon never shade (default culling on) | exact | `ForwardPassPolygonLights.azsli:36-46`; `LightCulling.azsl:608-650` no polygon pass; `SL/PBR/LightingOptions.azsli:25-26` culling default 1 | |
| c2-finding | punctual-group-skip: simple groups walked only while punctual enabled; later groups misread; decals keep walking | imprecise (wording "neighbours'") | `ForwardPassDirectLighting.azsli:30-34`; `ForwardPassDecals.azsli:25` | item 7 |
| entries | light-culling-pass, light-culling-tile, cull-point-light, light-culling-main, light-culling-remap, tile-iterator, polygon-not-culled, light-index-pack, nvlc-constants (256/bin, 8 bins) | exact | as above | |
| entries | tile-prepare ("one bit per sample") | imprecise | item 6 | "a mask slice per depth sample; transparent geometry fills its range" |
| entries | is-object-inside-tile, punctual-group-skip, decals-advance | exact | `NVLC.azsli:139-179`; `ForwardPassDecals.azsli:25` | |

### Chapter 3

| Beat | Sentence / entry | Class | Evidence | Correction |
|---|---|---|---|---|
| c3-inputs | surface record, material gives base colour, metallic, F0 factor, roughness | exact | `Materials/StandardPBR/StandardPBR_SurfaceData.azsli:46-100`; `BasePBR_SurfaceEval.azsli:87-93` | |
| c3-metal | albedo = base x (1 - metallic); metal has no diffuse colour | exact | `SL/PBR/MaterialUtils.azsli:46-48` | |
| c3-metal | F0 blends from dielectric value to base colour | exact | `MaterialUtils.azsli:51-55` lerp | |
| c3-metal | dielectric 0.08 x factor, default 0.5 gives 0.04 | exact | `MaterialUtils.azsli:16`; `SpecularPropertyGroup.json:11` default 0.5 | |
| c3-metal | between 0 and 1 "suits only transitions such as dust on metal" | unverifiable | no engine text; code only lerps | cut or attribute |
| c3-rough | clamps at 0.024, squares into alpha, squares again | exact | `MaterialUtils.azsli:20, 28-32` | |
| c3-rough | roughness 0.5 gives alpha squared 1/16 | exact | 0.5^4 | |
| c3-aa | specular AA on by default | exact | `GeneralCommonPropertyGroup.json:17-21` default true (StandardPBR; multilayer default is false) | |
| c3-aa | kernel = half squared normal change, max 0.18; recompute alpha and linear roughness | exact | `SL/PBR/SpecularAA.azsli:28-41`; `MaterialUtils.azsli:34-44` | |
| c3-aa | fine derivatives | exact | `SpecularAA.azsli:36-37` ddx_fine, ddy_fine | |
| c3-occlusion | two occlusion maps; red channel ^ factor, factor default 1, 1 without map | exact | `SH/Materials/MaterialInputs/OcclusionInput.azsli:31-46`; `OcclusionPropertyGroup.json:36-41` | |
| c3-occlusion | end of forward: diffuse x AO, specular x specular occlusion; albedo target carries both, spec occlusion in alpha | exact | `Materials/BasePBR/BasePBR_LightingData.azsli:84-103`; `ForwardPassVertexAndPixel.azsli:84-85` | |
| entries | surface-data (field list), surface-eval, set-albedo-f0, material-utils, roughness-values, specular-aa(-default), occlusion-* | exact | as above | |

### Chapter 4

| Beat | Sentence / entry | Class | Evidence | Correction |
|---|---|---|---|---|
| c4-what | decals project a texture inside a box | exact | `SL/PBR/Decals.azsli:38-60` | |
| c4-what | on by default; change the surface before lighting data | exact | `LightingOptions.azsli:30, 99`; `ForwardPassEvaluateLighting.azsli:31-34` | |
| c4-what | later passes see decal albedo, F0, normal | exact | targets written from `surface` after decals | |
| c4-apply | box via rotation transpose and half size; skip outside -1..1; xy to UV, v flipped | exact | `Decals.azsli:42-59` | |
| c4-apply | base x colour replaces albedo by alpha x opacity x angle fade | exact | `Decals.azsli:136-141` (colour factor also multiplies, unnarrated) | |
| c4-apply | F0 moves by that x metallic | exact | `Decals.azsli:146` | |
| c4-apply | normal map bends normal; channels swapped, z rebuilt | exact | `Decals.azsli:150-151`; `Gems/Atom/RPI/Assets/ShaderLib/Atom/RPI/TangentSpace.azsli:86-100` | |
| c4-angle | cos clamped to 0.55, smoothstep to 1, gone by about 57 deg; attenuation setting 1 default | exact | `Decals.azsli:28-36`; acos 0.55 = 56.6 deg; `CF/.../DecalComponentConfig.h:28` | |
| c4-order | decals first group, spheres around boxes; sorted by key then index, larger later | exact | `LightCulling.azsl:~136-150, 260-290, 613` | |
| c4-order | default key 16 | exact | `DecalComponentConfig.h:25` | |
| c4-order | forward walks the sorted group before lighting | exact | `ForwardPassDecals.azsli:22-28` | |
| c4-colour | O3DE lights in ACEScg; material and light colours converted | exact | `TransformColor.cpp:27`; light feature processors; `BaseColorInput.azsli:55` (IBL cubemaps are not converted; decals excepted, as the next sentence says) | |
| c4-colour | finding: decal colour stored raw, blended into ACEScg albedo | exact | priority table | |
| entries | decal-* , transform-color, base-colour-acescg, tangent-space-normal, decal-half-size | exact | as above | |

### Chapter 5

| Beat | Sentence / entry | Class | Evidence | Correction |
|---|---|---|---|---|
| c5-record | record: position, candela RGB, 1/r^2, channels, GI flags | exact | `SL/PBR/Lights/LightStructures.azsli:18-26` | |
| c5-record | inverse square, then window to zero at the radius | exact | `SL/PBR/Lights/SimplePointLight.azsli:47-53` | |
| c5-record | window = (1 - (d^2/r^2)^2)^2 | exact | `SimplePointLight.azsli:48-49` | |
| c5-record | at half radius 88% | exact | (1 - 0.0625)^2 = 0.879 | |
| c5-radius | beyond the radius skipped | exact | `ForwardPassSimplePointLights.azsli:22-25` | |
| c5-radius | automatic radius "square root of lumens, luminance-weighted, over 0.1" | imprecise | `CF/Source/CoreLights/SimplePointLightDelegate.cpp:29-30` `Sqrt(intensity / lightThreshold)`; cutoff 0.1 (`AreaLightComponentConfig.h:50`); automatic is default (`:56`); luminance weights `PhotometricValue.cpp:141-146` | item 8 |
| c5-channels | five bits each; components start all set; light needs shared bit | exact | `Gems/Atom/Feature/Common/Code/Include/Atom/Feature/LightingChannel/LightingChannelConfiguration.h:34`; `SL/PBR/Lights/LightTypesCommon.azsli:70-74` | |
| c5-channels | skylight, reflection passes, decals ignore channels | exact | no `IsSameLightChannel` in those shaders | |
| c5-channels | directional loop `return`s on another channel | exact | `ForwardPassDirectionalLights.azsli:53-58` | |
| c5-shadows | simple point lights no shadows; cost little more than pixels in radius | exact | `ForwardPassSimplePointLights.azsli:27` | |
| entries | point-light-struct/init/apply/loop/radius(1 mm)/auto-radius/cutoff-intensity, point-light-luminance | exact | as above | |

### Chapter 6

| Beat | Sentence / entry | Class | Evidence | Correction |
|---|---|---|---|---|
| c6-vectors | specular depends on N, L, V, H | exact | `SL/PBR/Microfacet/Brdf.azsli:87-93` | |
| c6-terms | SpecularGGX = D, G, F, N.L, multiscatter | exact | `Brdf.azsli:95-106` | |
| c6-terms | D = GGX density, far above one for smooth | exact | `SL/PBR/Microfacet/Ggx.azsli:36-41` | |
| c6-terms | G height-correlated Smith, returned divided by 4 N.L N.V | exact | `Ggx.azsli:52-58` | |
| c6-terms | F Schlick on H.V | exact | `SL/PBR/Microfacet/Fresnel.azsli:19-23`; `Brdf.azsli:97` | |
| entries | specular-ggx, ggx-d, ggx-g, fresnel-schlick, standard-brdf | exact | | |

### Chapter 7

| Beat | Sentence / entry | Class | Evidence | Correction |
|---|---|---|---|---|
| c7-lambert | Lambertian: albedo x N.L / pi, times response | exact | `Brdf.azsli:25-28` | |
| c7-lambert | response = 1 - Fresnel at view angle; grazing = max(1 - roughness, F0) | exact | `BasePBR_LightingEval.azsli:55-57`; `Fresnel.azsli:50-56` | |
| c7-lambert | smooth surface at grazing keeps less diffuse | exact | follows from the formula | |
| c7-lambert | "Without that response, diffuse and specular could send back more light than arrives" | unverifiable | rationale, no engine text | optional attribution |
| c7-disney | normalised Disney exists; StandardPBR never calls it; Skin does; EnhancedPBR with SSS | exact | `Brdf.azsli:33`; `Materials/Skin/Skin_LightingBrdf.azsli:35`; `EnhancedPBR_LightingBrdf.azsli:37-40` | |
| c7-disney | takes no diffuse response | exact | `Brdf.azsli:33-48` | |

### Chapter 8

| Beat | Sentence / entry | Class | Evidence | Correction |
|---|---|---|---|---|
| c8-layer | off by default | exact | `ClearCoatPropertyGroup.json:11` | |
| c8-layer | dielectric, F0 4%, own factor and roughness | exact | `StandardPBR_LightingEval.azsli:38`; `ClearCoatInput.azsli:100-101` | |
| c8-base | base F0 changes under coat; Filament form | exact | `MaterialUtils.azsli:58-64` | |
| c8-base | response shrinks by coat Fresnel at view angle | exact | `StandardPBR_LightingEval.azsli:39` | |
| c8-base | each light's diffuse shrinks again by coat Fresnel at H; dims twice | exact | `StandardPBR_LightingBrdf.azsli:42-43` | |
| c8-specular | base x (1 - Fc) + coat lobe; Kelemen visibility; alpha = roughness^2, min 0.0005 | exact | `Brdf.azsli:172-183` | |
| c8-ibl | clear coat moves sky reflections into forward pass; stencil skip | exact | `Materials/Types/StandardPBR_ClearCoatEnableFeature.lua:25-26`; `Gems/Atom/Feature/Common/Code/Source/Mesh/MeshFeatureProcessor.cpp:2422-2425` | |
| c8-ibl | base x (1 - Fc)^2 + coat IBL | exact | `SL/PBR/Lights/Ibl.azsli:148-149` | |
| c8-ibl | forward pass blends the smallest overlapping probe between inner and outer boxes | exact | `MeshFeatureProcessor.cpp:2846`; `Ibl.azsli:94-104` | |
| c8-ibl | "full-screen probe passes never touch a clear-coated mesh" | exact | stencil low bits None (`MeshFeatureProcessor.cpp:2422`), probe passes test stencil | |
| entries | all clear-coat-* , forward-ibl-stencil, forward-probe-pick | exact | | |

### Chapter 9

| Beat | Sentence / entry | Class | Evidence | Correction |
|---|---|---|---|---|
| c9-loss | single GGX bounce loses energy; compensation brightens rough metal | exact | `SL/PBR/LightingUtils.azsli:24-43` | |
| c9-texture | 256 x 256, two 16-bit floats, once at startup | exact | `Textures/BRDFTexture.attimage:12-15` (format 24 = R16G16_FLOAT, `Gems/Atom/RHI/Code/Include/Atom/RHI.Reflect/Format.h`); `Gems/Atom/Bootstrap/Code/Source/BootstrapSystemComponent.cpp:713` executeOnce | |
| c9-texture | texel = one view angle and one roughness | exact | `SH/BRDFTexture/BRDFTextureCS.azsl:30-33` | |
| c9-texture | 1024 GGX samples; weight visibility x 4 V.H N.L / N.H | exact | `BRDFTextureCS.azsl:44, 60-64` | |
| c9-texture | scale A and bias B on F0 | exact | `:66-70` | |
| c9-texture | rows upside down; lookup by smoothness | exact | `:74`; `BasePBR_LightingEval.azsli:45-47` | |
| c9-compensation | A + B is kept energy; adds missing part in proportion to F0 (Turquin) | exact | `LightingUtils.azsli:41-42` | |
| c9-compensation | "At roughness 0.8 ... about fifty-seven percent" | unverifiable | lab-computed A + B; not a stored constant | |
| c9-compensation | shader option defaults on, StandardPBR property defaults off | exact | priority table | |
| c9-finding | sampler takes sqrt(1 - cos^2) with no guard | exact | `SL/PBR/Microfacet/Ggx.azsli:121` | |
| c9-finding | roughness below about 0.03 loses energy on this GPU | unverifiable | GPU-dependent, as the story says | |

### Chapter 10

| Beat | Sentence / entry | Class | Evidence | Correction |
|---|---|---|---|---|
| c10-why | StandardPBR no transmission; EnhancedPBR adds it, off by default, thick/thin | exact | `.../SubsurfaceAndTransmissionPropertyGroup.json:93-94`; `Pipelines/MainPipeline/ForwardPass_StandardLighting.azsli:18` ENABLE_TRANSMISSION 0 | |
| c10-why | other lighting StandardPBR's unless SSS or anisotropy on | exact | `EnhancedPBR_LightingBrdf.azsli:34-63` | |
| c10-light | simple point lights add no back lighting | exact | `SimplePointLight.azsli:45-65` | |
| c10-light | point light = sphere area light, editor default radius 5 cm | exact | `CF/Source/CoreLights/EditorAreaLightComponent.cpp:430` | |
| c10-light | moves specular direction to nearest bulb point; adds back lighting | exact | `SL/PBR/Lights/PointLight.azsli:82-100` | |
| c10-thick | Barre-Brisebois and Bouchard; V against light bent by normal, power 6, scale | exact | `SL/PBR/BackLighting.azsli:62-72`; power 6 default in property group | |
| c10-thick | e^(-4 t) x (1 - t); t = max(shadow, material); default thickness 0.5 | exact | `BackLighting.azsli:64-67`; attenuation 4, thickness 0.5 defaults | |
| c10-thick | sum x tint added after occlusion | exact | `BasePBR_LightingData.azsli:95-106` | |
| c10-thin | Jimenez; diffusion profile over shadow thickness; no shadow data = no back lighting | exact | `BackLighting.azsli:74-110` | |
| c10-thin | finding: shadow branch runs for shadowless point light (options true); thickness 0, not negative; kernel at full value | exact | `SL/Pipeline/Forward/ForwardPassPointLights.azsli:90-111`; `PointLightFeatureProcessorInterface.h:36` indices 0xFFFF; `SL/Shadow/ProjectedShadow.azsli:127-129`; kernel(0) = 1 (`BackLighting.azsli:20-24`) | |
| entries | transmission-* , enhanced-brdf, sphere-light-*, per-mesh-flags, shadow-thickness-none | exact | | |

### Chapter 11

| Beat | Sentence / entry | Class | Evidence | Correction |
|---|---|---|---|---|
| c11-light | intensity in lux; default 4 EV100 = 40 lux | exact | priority table | |
| c11-light | angular diameter 0.5 deg; shader adjusts a direction, passes plain direction; size changes nothing | imprecise | `DirectionalLight.azsli:55-63`; used in `SH/Reflections/ReflectionScreenSpaceRayTracingCommon.azsli:104` | item 5 |
| c11-light | only one directional light casts shadows | exact | priority table | |
| c11-light | "lab has no directional light" | exact | lab statement | |
| c11-splits | shadow out to far distance, 100 m by default | imprecise | `CL/DirectionalLightFeatureProcessor.cpp:88-91` min(camera far, shadow far); 100 in config h | item 4 |
| c11-splits | four cascades; blend of log and uniform | exact | config h cascades 4; `cpp:1337-1343` | |
| c11-splits | ratio 0.9 leans log; first cascade short | exact | | |
| c11-splits | ratio 0 equal, ratio 1 constant factor | exact | `cpp:1337-1340` | |
| c11-splits | larger shadow distance grows every slice | exact | borders scale with far | |
| c11-fit | bounding sphere per slice | exact | `cpp:1589-1614` | |
| c11-fit | centred where distances to near and far corners are equal | imprecise | `cpp:1575-1586` `min(GetDepthCenter, depthFar)`, `cpp:58-65, 113`; default fov 90 and 16:9 always clamps | item 3 |
| c11-fit | box is the sphere's cube, stretched toward light and behind camera | exact | `cpp:1496-1535` | |
| c11-fit | corners floored to whole texels of the 1024 map | exact | `cpp:1369-1387` | |
| c11-fit | "A sphere does not turn, so the boxes hold still as the observer looks around" | wrong | `cpp:1575-1586` centre follows camera; only the size is rotation-invariant | item 1 |
| c11-fit | snapping keeps shadow edges from shimmering | exact | comment `cpp:1371` | |
| c11-fit | readout: radius and texel mm close, cm far | exact | texel = 2r / 1024 (about 1.4 mm to 2.8 cm for far 12) | |
| c11-select | offset 2.5 x 2 / size, about five millimetres | exact | priority table (4.9 mm) | |
| c11-select | first cascade holding it at least 1.5 texels inside; by bounds not depth | exact | priority table | |
| c11-select | full-screen pass writes visibility, blurs it, forward reads one value | exact | priority table | |
| c11-select | 8-bit target at output size, per pixel not sample | exact | `FullscreenShadow.pass:68`; depth sampled at one MSAA sample position (`FullscreenShadow.azsl:~100-115`) | |
| c11-others | after directional: simple lights, then five area kinds | exact | `ForwardPassDirectLighting.azsli:26-42` | |
| c11-others | quad lights: LTC, two 32 x 32 tables, or fast approximation | exact | `SL/PBR/Lights/Ltc.azsli:54` LUT_SIZE 32; `QuadLight.azsli:132-147` (table files are Git LFS pointers; 32 from the shader constant) | |
| c11-others | disk, capsule, polygon have own methods; polygon never shade with culling | exact | chapter 2 | |
| entries | directional-lux, directional-angular-unused, one-shadow-light, cascade-defaults, cascade-splits, cascade-snap, cascade-select, fullscreen-shadow, fullscreen-shadow-format, area-lights | exact | as above | |
| entries | cascade-sphere-fit | wrong in last clause | item 2 | |

### Chapter 12

| Beat | Sentence / entry | Class | Evidence | Correction |
|---|---|---|---|---|
| c12-sky | skylight = IBL, two cubemaps | exact | `Gems/Atom/Feature/Common/Assets/ShaderResourceGroups/SceneSrg.azsli:36-39` | |
| c12-bake | baked offline by the image processor | exact | `IBLSpecular.preset`, `IBLDiffuse.preset` | |
| c12-bake | specular 256 per face, R9G9B9E5, six mips | exact | `.../ImageProcessingAtom/Assets/Config/IBLSpecular.preset:13-16`; six mips `Cubemap.cpp:539` | |
| c12-bake | diffuse 32 to 128 per face | exact | `IBLDiffuse.preset` (BC6UH on PC) | |
| c12-prefilter | mip m filtered for roughness m / 5; N = V = texel direction | exact | `.../CubeMapGen/CCubeMapProcessor.cpp:1606-1636` | |
| c12-prefilter | 256 GGX samples | exact | `Cubemap.cpp:643` | |
| c12-prefilter | each sample read one mip blurrier than solid-angle match, blended between two | exact | `CCubeMapProcessor.cpp:1663-1680` | |
| c12-cosine | cosine average normalised; stores irradiance / pi | exact | `CCubeMapProcessor.cpp:735-741, 804` | |
| c12-passes | forward stores albedo x response x AO, F0, roughness in eight bits | exact | `PS/Forward.pass:181-249`; `ForwardPassVertexAndPixel.azsli:84-90` | |
| c12-passes | diffuse pass: albedo x diffuse cubemap at normal | exact | `SH/DiffuseGlobalIllumination/DiffuseGlobalFullscreen.azsl:75-79` | |
| c12-passes | reflection pass: mip roughness x 5; F0 A + B; compensation; N.V >= 0.01; exposure; spec occlusion | exact | `SH/Reflections/ReflectionGlobalFullscreen.azsl:100-121`; `LightingUtils.azsli:60` | |
| c12-passes | diffuse adds onto diffuse target; reflection own target; composite adds onto specular | exact | blend states One/One; `ReflectionComposite.shader:34-36` | |
| c12-occlusion | skylight casts no shadows; SSAO on by default | exact | `SsaoPasses.cpp:35-62`; `AoParams.inl:15,19` | |
| c12-occlusion | interiors: probes replace reflections; diffuse probe grid left out by lab | exact | | |
| entries | ibl-srg, specular-preset, diffuse-preset, cubemap-ggx, filter-ggx, filter-cosine, diffuse-global, target-formats, reflection-global, reflection-composite, roughness-mip | exact | | |

### Chapter 13

| Beat | Sentence / entry | Class | Evidence | Correction |
|---|---|---|---|---|
| c13-why | probe = cubemap baked from a point, inner and outer box | exact | `CF/Source/ReflectionProbe/ReflectionProbeComponentController.h:33-38` | |
| c13-why | 256 texels, default Medium quality | exact | `.h:44`; `CF/Source/CubeMapCapture/EditorCubeMapRenderer.h:30-34` | |
| c13-bake | six faces, each 90 deg square view | exact | `Gems/Atom/RPI/Code/Source/RPI.Public/Pass/Specific/EnvironmentCubeMapPass.cpp:96` | |
| c13-bake | "draws one face per frame, ... 16-bit float 1024 target, reads back five frames later" | imprecise | `EnvironmentCubeMapPass.cpp:173-222` (same face every frame during a 5-frame delay); 1024 and float16: `EnvironmentCubeMapPass.h:41`, `.cpp:124` | item 9 |
| c13-bake | editor writes DDS; image builder turns it into the cubemap with the skylight's specular preset | exact | `EditorCubeMapRenderer.cpp:199-202`; `ImageBuilder.settings:130` | |
| c13-bake | Medium: 256, R9G9B9E5, six mips, GGX | exact | `IBLSpecular.preset` | |
| c13-ball | visualisation sphere on by default; white metal roughness 0; mirrors sharpest mip; excluded from capture | exact | `.h:41`; `Materials/ReflectionProbe/ReflectionProbeVisualization.azsli:78-83`; `ReflectionProbe.cpp:70` | |
| c13-stencil | five passes after forward | exact | `PS/Reflections.pass` order | |
| c13-stencil | stencil pass draws inner box, slightly reduced, adds one for surfaces inside | exact | `ReflectionProbe.cpp:122`; `SH/Reflections/ReflectionProbeStencil.shader:8-35` | |
| c13-stencil | outside every inner box keep value three | exact | `RenderCommon.h:38` UseIBLSpecularPass 0x3; global pass Equal 3 | |
| c13-stencil | blend-weight: 1 at inner, 0 at outer; weights add | exact | `ReflectionProbeBlendWeight.azsl:70-91`; One/One | |
| c13-blend | global pass outside inner boxes keeps what probes leave | exact | `ReflectionGlobalFullscreen.azsl:101`; stencil Equal 3 | |
| c13-blend | "one minus their summed weight, at most one" | imprecise (ambiguous) | `:101` 1 - min(sum, 1) | item 10 |
| c13-blend | outer pass: probe x weight / total when above one | exact | `ReflectionProbeRenderOuter.azsl:108-114` | |
| c13-blend | inner pass replaces reflection with the probe's | exact | `ReflectionProbeRenderInner.azsl:82-88`, no blend | |
| c13-blend | largest first; smaller probe has last word | exact | `ReflectionProbeFeatureProcessor.cpp:140-146` | |
| c13-blend | probes apply no specular occlusion | exact | `ReflectionProbeRenderCommon.azsli:15-70` | |
| c13-parallax | ray traced to the outer box, cubemap read toward the hit from the probe centre | exact | `SL/PBR/LightingUtils.azsli:84-104`; default on `.h:40` | |
| c13-composite | composite adds target where stencil marks a surface | exact | `PS/OpaqueParent.pass:383` StencilRef 1 LessEqual | |
| c13-composite | finding: target cleared to 0.4 grey; global never runs inside inner boxes; IBL off: inner discards, grey added | exact | `PS/ReflectionGlobalFullscreen.pass:63-70`; `ReflectionGlobalFullscreen.azsl:75-80`; `ReflectionProbeRenderCommon.azsli:29-33` | |
| entries | reflections-order, probe-*, parallax-obb, composite-stencil, reflection-clear, forward-probe-pick, cubemap-quality | exact | | |

### Chapter 14

| Beat | Sentence / entry | Class | Evidence | Correction |
|---|---|---|---|---|
| c14-resolve | diffuse resolves as plain average; specular through custom resolve | exact | `PS/OpaqueParent.pass:388-402` MSAAResolveColor vs MSAAResolveCustom | |
| c14-resolve | scaled to at most 1.5 x brightest luminance of four diagonal neighbours' samples | exact | priority table | |
| c14-resolve | samples averaged in log domain (geometric mean) | exact | priority table | |
| c14-resolve | lab four samples, O3DE two | exact | `PS/MainRenderPipeline.azasset:13` samples 2 | |
| c14-why | SSAO from depth alone, runs by default in main pipeline | exact | `SsaoPasses.cpp:35-62`; `PS/AoParent.pass:38-39` | |
| c14-why | after reflections and SSS; multiplies only the diffuse target | exact | `PS/OpaqueParent.pass:484-491` | |
| c14-compute | linear depth of each pixel's first MSAA sample | exact | `SH/PostProcessing/MSAAResolveDepth.azsl:37`; `DepthToLinearDepth.azsl:41` | |
| c14-compute | half resolution, upper-left of each 2 x 2 | exact | `DepthDownsample.azsl:27-29` | |
| c14-compute | 52 samples: 12 close, 10 rotations of 4 shared | exact | `SsaoCompute.azsl:32-38, 330-390` (12 + 10 x 4) | |
| c14-compute | cosine, weight 1 / (d^2 + 0.07), weighted average | exact | `SsaoCompute.azsl:206-247` | |
| c14-compute | (1 - AO x strength)^2.2, 8 bits | exact | `:395-400`; `PS/SsaoCompute.pass:34` R8_UNORM | |
| c14-compute | strength 1; radius 5% of half-res height | exact | `SsaoConstants.h:17-18`; `SsaoCompute.azsl:~365` radius x output height | |
| c14-blur | depth-aware blur, down then across | exact | `PS/FastDepthAwareBlur.pass:43-63` vertical then horizontal | |
| c14-blur | keeps two thirds of previous, less where depth slope changes | exact | `FastDepthAwareBlurCommon.azsli:206-219` | |
| c14-blur | half the chain from 8 before and half from 8 after | exact | `FastDepthAwareBlurHor.azsl:20, 32, 206-245` | |
| c14-blur | pass defaults 2/3, used with no AO component; component's 0.85 and 200 | exact | `SsaoPasses.cpp:90-110` only calls SetConstants with settings; `FastDepthAwareBlurPasses.h:31-40`; `AoConstants.h:19-21` | |
| c14-upsample | four nearest half-res pixels, by distance and depth closeness | exact | `DepthUpsample.azsl:116-190` | |
| c14-upsample | diffuse multiplied by it; merge adds specular untouched | exact | `ModulateTexture.azsl:55`; `DiffuseSpecularMerge.azsl:70` | |
| entries | msaa-resolve-custom, resolve-contrast, ao-placement, ao-parent, ssao-*, depth-*, blur-*, ssao-modulate, ao-defaults | exact | as above | |

### Chapter 15

| Beat | Sentence / entry | Class | Evidence | Correction |
|---|---|---|---|---|
| c15-recap | depth pre-pass, culling into tiles and depth bins | exact | | |
| c15-recap | cascades fitted to view; shadow resolved into one value a pixel | exact | chapter 11 | |
| c15-recap | forward: decals, GGX, Lambertian x response, clear coat, transmission for EnhancedPBR | exact | | |
| c15-recap | passes "read back from the forward pass's eight-bit targets" | imprecise | `PS/Forward.pass:249` normal is R10G10B10A2 | item 13 |
| c15-recap | resolve specular with neighbour clamp; SSAO; merge | exact | | |
| c15-recap | leaves out SSR, "which are off by default" | exact | `PS/Reflections.pass:265-268` Enabled false | |
| entries | msaa-default (2 samples) | exact | `PS/MainRenderPipeline.azasset:13` | |
