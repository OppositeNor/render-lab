# Source-index keys per beat

O3DE 26.05, commit 3db69432. Keys are in `source-index.json` (written by `src/o3de/lighting/source_index.py`). Each line: `key`: gist. Claims with no key (pure lab or derived-by-arithmetic beats) are marked "none".

## 1 · Where the lighting runs
- c1-open: none (lab demonstration); `engine-version`: the checkout is 26.05 (2.6.0).
- c1-pipeline: `render-pipeline-path`: default pipeline asset is a per-platform CVar trait; `pipeline-trait-desktop`: desktop selects MainRenderPipeline; `pipeline-trait-mobile`: Android and iOS select the Mobile pipeline; `msaa-default`: main pipeline renders with 2x MSAA; `mobile-pipeline-asset`: Mobile has one sample and subpass merging.
- c1-map: `main-pipeline-passes`: pre-pass, culling, shadows, opaque, transparent, post in order; `opaque-pass-order`: forward, IBL, reflections, resolves, SSS, SSAO, merge; `shadow-parent-order`: cascades, projected, ESM, fullscreen shadow, blur; `skybox-two-outputs`: sky writes specular and reflection targets together.
- c1-prepass: `depth-prepass`: D32_FLOAT_S8X24_UINT, reversed depth cleared to 0; `depth-prepass-children`: MSAA depth, resolve, linear depth; `depth-prepass-state`: pre-pass depth test GreaterEqual, no pixel stage; `forward-depth-state`: forward GreaterEqual passes equal pre-pass depth; `forward-early-z`: early depth/stencil unless OUTPUT_DEPTH; `depth-resolve`: depth resolve keeps sample 0; `depth-linear`: linear view distance from the resolved depth.
- c1-targets: `forward-pixel`: the pixel shader writes diffuse, specular, albedo, F0, normal; `forward-targets`: what the IBL passes read back; `target-formats`: RGBA16F twice, RGBA8 twice, RGB10A2; `forward-albedo-target`: albedo times diffuse response and AO, specular occlusion in alpha; `forward-f0-target`: F0 in rgb, roughnessLinear in alpha.
- c1-normal: `normal-encoding`: signed octahedron, z kept as a sign bit; `normal-flags`: two booleans packed in 2-bit alpha.
- c1-split: `evaluate-lighting`: init first, tile iterator, decals, direct, forward IBL, finalize; `forward-opaque`: opaque shaders skip transparent forward IBL path; `ibl-forward-transparent`: forward IBL only for transparent or opted-in materials; `diffuse-global`: fullscreen diffuse IBL adds onto diffuse target; `diffuse-composite`: probe-grid gem replaces the stock diffuse pass; `reflection-global`: fullscreen specular IBL writes its own target; `emissive-in-specular`: emission lives in the specular target.
- c1-stencil: `stencil-refs`: 0x3, 0x40, 0x80 stencil bit values; `stencil-forward-ibl-opt`: per-draw stencil reference built from forward-IBL choice; `forward-ibl-stencil`: forward-IBL meshes drop the specular-pass bits; `clear-coat-forward-ibl`: clear coat switches material to forward specular IBL.
- c1-models: `lighting-model-select`: script picks shaders by lighting model; `lighting-models`, `lighting-model-standard`, `lighting-model-enhanced`, `lighting-model-skin`: material types declare their model; `standard-lighting-defines`, `enhanced-lighting-defines`, `skin-lighting-defines`: per-model compile switches; `enhanced-draw-list`: Enhanced draws into ForwardSubsurface.
- c1-alternatives: `lowend-forward`: LowEnd writes one colour; `lowend-forward-target`: single RGBA16F target; `lowend-forward-ibl`: IBL applied in forward; `lowend-script-skin`: Skin gets no LowEnd shader; `mobile-pipeline`: Mobile forward turns off culling, caps lights at 5; `deferred-inert`: deferred CVar false, no consumer.
- c1-recap, c1-sources: `forward-pixel`, `depth-prepass`, `forward-targets`, `evaluate-lighting`, `stencil-refs`, `lighting-model-select`, `lowend-forward`, `deferred-inert`, `brdf-execute-once` (BRDF texture pipeline runs once), `diffuse-specular-merge`.

## 2 · Direct light: which lights reach a tile
- c2-why: `light-culling-pass`: culling runs after pre-pass before opaque; `light-culling-tile`: tiles are 16 x 16; `tile-data-format`: one RGBA32_UINT texel per tile.
- c2-tile: `tile-prepare`: per tile nearest and farthest depth, mask, bins; `tile-sky-replace`: sky pixels take the farthest transparent depth.
- c2-mask: `tile-mask-bit`: depth to one of 32 slices; `tile-mask-or`: threads OR slice bits into the mask; `tile-transparent-fill`: transparent range filled between nearest and farthest; `tile-record`: record layout: zNear, zFar plus bins, mask.
- c2-bins: `tile-prepare` (log2 span gives 1, 2, 4, or 8 bins); `tile-record` (ratio over 1 m, NVIDIA refinement not ported); `nvlc-constants`: 256 entries, 8 bins, reversed depth; `tile-unpack`: unpack restores low zFar bits conservatively.
- c2-cull: `cull-aabb`: tile view-space box from eight points; `cull-point-light`: sphere against box, then mask; `is-object-inside-tile`: depth run ANDed with the mask; `cull-spot`, `cull-disk`, `cull-capsule`, `cull-quad`: each type's culling test.
- c2-list: `light-culling-main`: seven groups each closed by a marker; `light-index-pack`: index shifted 16, bin mask below; `cull-end-of-list`: 255-entry cap, later entries dropped; `light-culling-remap`: per bin lists ending in END_OF_LIST.
- c2-iterate: `tile-iterator`: reads only its pixel's tile and bin; `tile-bin-select`: bin from depth fraction times bin count; `tile-iterator-advance`: LoadAdvance steps past markers; `direct-lighting`: types in order, each behind an option; `decals-advance`, `punctual-group-skip`: skipped groups keep or lose alignment.
- c2-falloff: `falloff-window`: (1 - falloff^2)^2 reaches zero at radius; `point-light-apply`: intensity over d^2 times the window; `point-light-struct`: simple point light record; `point-light-loop`: skips lights beyond radius; `point-light-auto-radius`: radius from lumens and cutoff.
- c2-types: `spot-falloff`: cosine smoothstep between cones; `sphere-light-apply`: sphere lights shift specular direction; `sphere-representative-point`: nearest bulb point to reflection ray; `sphere-normalization`: energy normalisation for the widened highlight; `disk-light-direction`: disk specular direction; `capsule-intensity`: capsule area ratio; `area-lights`: quad LTC or fast approximation; `ltc-lookup`: 32 x 32 tables, edge integrals; `ltc-amplitude`: second table scales by F0; `ltc-textures`: two DDS tables; `quad-falloff-centre`: quad falloff from the centre only.
- c2-units: `units-ev100`: EV100 to lux 250, nits 12.5; `units-solid-angle`: spot solid angle is pi; `units-sphere-solid-angle`: sphere solid angle is 4 pi; `units-area-default`: new area light is 100 lumens; `point-light-luminance`: combined intensity is perceptual luminance; `cutoff-intensity`: automatic radius cutoff 0.1; `directional-lux`: sun is lux, 4 EV100 is 40.
- c2-findings: `polygon-not-culled`, `polygon-iteration`: polygon group never written, loop ends at once; `directional-channel-return`: return skips remaining directional lights; `directional-angular-unused`: angular size computed, never used; `capsule-auto-radius`: capsule radius linear in intensity.
- c2-recap, c2-sources: `tile-mask-bit`, `tile-prepare`, `cull-point-light`, `light-culling-main`, `tile-iterator`, `falloff-window`, `sphere-representative-point`, `ltc-lookup`, `polygon-iteration`.

## 3 · The surface
- c3-inputs: `surface-eval`: material inputs become the Surface; `base-colour-acescg`: base colour texture converted to ACEScg; `transform-color`: colour factors converted on the CPU; `surface-data`: the Surface's fields.
- c3-metal: `set-albedo-f0`: albedo and F0 from base colour and metallic; `material-utils`: dielectric F0 0.08 times factor, minimum roughness.
- c3-rough: `roughness-input`: map lerp or factor; `roughness-values`: clamp, square, square again, then specular AA.
- c3-aa: `specular-aa-default`: specular AA on by default; `specular-aa`: widens roughnessA2 by normal derivatives, cap 0.18.
- c3-occl: `occlusion-input`: map red to an exponent, 1 without a map; `occlusion-eval`: diffuse and specular occlusion maps; `emissive-input`: factor, optional alpha, colour in nits; `emissive-in-specular`: emission added after finalize.
- c3-decal: `decal-evaluate-order`: decals run after lighting init on a copy; `decal-apply`: albedo and normal only, arrays 0 to 4; `decal-attenuation`: angle smoothstep from 0.55 to 1; `decal-cull`, `decal-sort`: sphere culling and sort-key order; `decal-struct`, `decal-defaults`: decal record and defaults; `decal-colour-raw`: decal colour has no colour-space conversion; `forward-targets`: G-buffer written from the undecalled surface.
- c3-recap, c3-sources: `surface-eval`, `set-albedo-f0`, `roughness-values`, `specular-aa`, `occlusion-input`, `decal-evaluate-order`.

## 4 · The BRDF
- c4-vectors: `specular-ggx`: the product that uses N, L, V, H.
- c4-d: `ggx-d`: GGX distribution, a2 over pi b squared.
- c4-g: `ggx-g`: height-correlated Smith with 1/(4 NdotL NdotV) folded in.
- c4-f: `fresnel-schlick`: Schlick from F0 toward white; `fresnel-roughness`: grazing value max(1 - roughness, F0).
- c4-diffuse: `diffuse-lambert`: albedo NdotL over pi times diffuse response; `initialize-lighting-data`: diffuse response is one minus Fresnel.
- c4-perlight: `standard-brdf`: StandardPBR's diffuse and specular per light; `point-light-apply`: (diffuse plus specular) times intensity times shadow; `directional-apply-lit`: shadow factor multiplies finished terms.
- c4-models: `diffuse-disney`: normalised Disney ignores diffuse response; `enhanced-diffuse`, `skin-diffuse`: which model uses which diffuse; `anisotropic-ggx`: Enhanced anisotropy; `dual-specular`: Skin's second lobe; `clear-coat-default`, `clear-coat-f0`, `clear-coat-response`, `clear-coat-diffuse`, `clear-coat-specular`: coat off by default, F0 change, double diffuse loss, coat GGX.
- c4-lut: `brdf-texture`: 1024 GGX samples per texel, scale and bias; `brdf-texture-size`: 256 x 256 R16G16_FLOAT; `rhi-format`: format 24 is R16G16_FLOAT; `ggx-sample`: importance sampling; `hammersley`: the sample points; `brdf-texture-lookup`: coordinates (NdotV, 1 - roughness); `lighting-init`: sampled bilinearly at the pixel.
- c4-multi: `multiscatter`: 1 + F0 (1/(A+B) - 1) energy compensation.
- c4-default: `ms-option`: shader default true; `ms-material-property`: material property loads false; `bool-property-default`: Bool without default is false; `property-init`, `property-apply`: initialisation overwrites the option; `ms-preset`: presets turn it on.
- c4-recap, c4-sources: `ggx-d`, `ggx-g`, `fresnel-schlick`, `diffuse-lambert`, `specular-ggx`, `brdf-texture`, `multiscatter`, `ms-material-property`.

## 5 · Shadows
- c5-why: `shadow-parent-order`: cascades then projected then fullscreen shadow; `directional-shadow-index`: only one directional light is shadowed; `one-shadow-light`: the shadowing handle is a single light.
- c5-cascades: `cascade-image`: one D32 array, slice per cascade; `cascade-defaults`: four cascades, 1024, 100 m, ratio 0.9; `cascade-splits`: log and uniform blend by ratio; `shadow-caster-state`: caster depth bias 10, slope 4.
- c5-fit: `cascade-sphere-fit`: bounding sphere per cascade slice; `cascade-snap`: box floored to whole texels.
- c5-lookup: `shadow-coords`: offset, light view, bias, projection per cascade; `shadow-normal-offset`: normal bias times 2 over map size; `cascade-select`: first cascade 1.5 texels inside the map; `shadow-is-shadowed`: point sample, depth compare, 0 or 1.
- c5-bias: `shadow-slope-bias-off`: slope bias disabled by a constant; `shadow-normal-offset` (same offset in every cascade); `shadow-coords` (shadowBias toward the light).
- c5-fullscreen: `shadow-use-fullscreen`: opaque forward reads the screen buffer; `fullscreen-shadow`: one visibility value per pixel; `fullscreen-shadow-format`: R8_UNORM at output size; `fullscreen-shadow-normal`: faceted normal rebuilt from depth.
- c5-filter: `shadow-filter-default`: filter defaults to None; `shadow-blur-settings`: per-light blur constants; `shadow-blur-radius`: blur reaches 8 pixels; `blur-defaults`: struct defaults 2/3, 0, 50; `pcf-squared`: PCF kernels return the square.
- c5-local: `shadow-atlas`: projected maps packed in array slices; `point-shadow-faces`: six 91 degree views; `point-shadow-face-pick`: face by largest axis.
- c5-recap, c5-sources: `cascade-defaults`, `cascade-splits`, `cascade-sphere-fit`, `shadow-coords`, `shadow-is-shadowed`, `fullscreen-shadow`, `blur-defaults`, `point-shadow-faces`.

## 6 · Indirect light
- c6-two: `ibl-srg`: two cubemaps, orientation, exposure; `specular-preset`: 256 faces, GGX, R9G9B9E5; `diffuse-preset`: 32 to 128 faces, BC6UH, cosine.
- c6-diffuse: `ibl-diffuse-lookup`: one cubemap lookup by the normal; `diffuse-global`: pass adds onto diffuse target; `diffuse-global-stencil`: runs where the 0x80 bit is set; `diffuse-composite`, `diffuse-probe-grid-template`: probe-grid gem disables the stock pass; `filter-cosine`: diffuse cube is cosine filtered.
- c6-specular: `reflection-global`: reflection direction, mip, F0 A + B; `roughness-mip`: mip equals roughness times 5; `cubemap-ggx`: six mips, 256 samples; `filter-ggx`: per-mip roughness m/5, mip 0 copied; `cubemap-coords`: axis swap into cubemap space; `vect-to-texel`: lookups sit up to half a texel off.
- c6-splitsum: `reflection-global` (cube times F0 A + B); `brdf-texture-lookup`: same LUT as chapter 4.
- c6-probe: `reflections-order`: stencil, weight, global, outer, inner; `probe-stencil`: depth-fail counts nested volumes; `probe-stencil-reduced`: inner volume drawn 0.1 m smaller; `probe-defaults`: 8 m extents, parallax on, Medium; `probe-sort`: larger volumes drawn first.
- c6-parallax: `parallax-obb`: ray intersected with the outer box from the probe centre; `probe-specular`: parallax applied when enabled.
- c6-blend: `probe-blend-weight`: weight 1 inside inner, ramp to outer; `obb-lerp`: weight ratio between boxes; `reflection-global-weight`: global takes 1 - sum of weights; `probe-render-outer`: outer probes add weighted specular; `probe-render-inner`: inner probe replaces the buffer; `reflection-global-stencil`: global pass only outside inner volumes; `composite-stencil`, `reflection-composite`, `reflection-composite-box`, `reflection-target`: composite adds reflection onto specular.
- c6-findings: `probe-specular` (no specular occlusion in probe passes); `reflection-global` (global applies specular occlusion); `reflection-clear`: reflection buffer cleared to 0.4 grey.
- c6-ssr-dpg: `ssr-default-off`: SSR off unless a component enables it; `diffuse-composite`: probe-grid composite replaces diffuse pass; `skybox-cubemap`: sky uses its own cubemap; `probe-bake-capture`, `probe-bake-preset`: probe bake path.
- c6-recap, c6-sources: `ibl-diffuse-lookup`, `reflection-global`, `roughness-mip`, `probe-blend-weight`, `parallax-obb`, `probe-specular`.

## 7 · Occlusion
- c7-why: `ao-placement`: SSAO modulates the diffuse target after SSS; `ssao-default`: SSAO on unless level settings disable it; `ao-defaults`: settings default enabled; `ssao-order`: downsample, compute, blur, upsample, modulate; `depth-downsample`: half-res depth takes the top-left pixel; `ssao-format`: R8_UNORM output.
- c7-samples: `ssao-view-position`: view position from depth; `ssao-normal`: normal from depth only; `ssao-local-samples`: 12 fixed taps; `ssao-compute`: 52 samples weighted 1/(d^2+0.07); `ssao-accumulate`: cosine times weight over total weight.
- c7-radius: `ssao-radius`: radius is a fraction of the half-res height; `ssao-defaults`: radius 0.05, strength 1.
- c7-output: `ssao-compute` (one minus AO then power 2.2); `blur-order`, `blur-step`: vertical then horizontal depth-aware blur; `blur-component-defaults`: 0.85, 0, 200; `depth-upsample`: depth-weighted 2 x 2 upsample.
- c7-where: `ssao-modulate`: multiplies all diffuse, direct and IBL; `ao-placement` (specular untouched).
- c7-material: `occlusion-input`: exponent on a map, 1 without; `occlusion-finalize`: finalize multiplies direct light too; `occlusion-defaults`: factors default 1; `forward-ibl-ao-twice`: forward diffuse IBL occluded twice.
- c7-recap, c7-sources: `ao-placement`, `ssao-normal`, `ssao-accumulate`, `ssao-radius`, `ssao-compute`, `ssao-modulate`, `occlusion-input`.

## 8 · Light through surfaces
- c8-which: `lighting-model-enhanced`, `lighting-model-skin`, `enhanced-lighting-defines`, `skin-lighting-defines`: only these compile transmission and SSS; `transmission-option`: mode is a shader option.
- c8-thick: `back-lighting`: thick-object formula with distortion and power; `transmission-defaults`: mode None, thickness 0.5, power 6; `transmission-params`: parameters packed per mode.
- c8-thin: `back-lighting` (thin mode, constant 300); `transmission-kernel`: thin diffusion kernel, 1 at zero thickness; `enhanced-brdf`: Enhanced model's switches.
- c8-thickness: `shadow-thickness`: directional thickness not metric; `shadow-thickness-camera-span`: scaled by camera depth span; `shadow-thickness-none`: no shadow means thickness 0; `sphere-light-shadow-branch`: point thickness from projected shadow; `transmission-finalize`: back light added after occlusion.
- c8-foliage: `foliage-sample-material`: engine's thin leaf sample; `double-sided-cull`: culling off for all shaders; `back-face-normal`: normal flipped on back faces; `vertex-normal-unflipped`: vertex normal never flipped; `cutout-clip`, `cutout-custom-z`: alpha clip in every pass; `simple-point-no-backlight`: simple point lights add no back light.
- c8-sss: `sss-pack`: factor and quality packed into fp16 alpha; `sss-unpack`: unpacked with frac; `msaa-resolve-diffuse`: resolve averages the packed alpha; `sss-sample-count`: up to 200 samples scaled by quality; `sss-kernel`: Burley profile weight; `sss-barrier`: barrier after early return; `diffuse-specular-merge`: 2 x 2 filter on SSS pixels.
- c8-skin: `skin-diffuse`: Skin always uses Disney diffuse; `dual-specular`: second GGX; `eye-direct-lighting`: Eye uses Lambert and one GGX; `eye-iris-mask`: iris mask and refraction UV offset; `enhanced-metallic-sss`: metallic stays 0 under SSS.
- c8-recap, c8-sources: `lighting-model-enhanced`, `back-lighting`, `shadow-thickness`, `foliage-sample-material`, `sss-pack`, `sss-sample-count`, `skin-diffuse`.

## 9 · Terrain, and a word on hair
- c9-own: `terrain-pass-request`: only compute passes added; `terrain-forward-draw-list`: terrain joins the forward draw list; `terrain-stencil`: both IBL stencil bits set.
- c9-fixed: `terrain-macro-constants`: roughness 1, F0 factor 0.5, metalness 0.
- c9-normal: `terrain-normal-cpu`: per-vertex CPU normals, 8-bit; `terrain-normal-ps`: z rebuilt in the pixel shader.
- c9-detail: `terrain-height-blend`: height blending off in C++; `terrain-height-blend-option`: shader default true, overridden.
- c9-differs: `terrain-no-direct-multiscatter`: direct lighting keeps factor 1; `terrain-multiscatter-flags`: fullscreen IBL still compensated; `terrain-decal-order`: decals after lighting init.
- c9-hair: `hair-default-model`: Marschner default; `hair-marschner`: n 1.55, three lobes; `hair-pass-after-opaque`: hair blends after the opaque parent.
- c9-recap, c9-sources: `terrain-forward-draw-list`, `terrain-macro-constants`, `terrain-normal-cpu`, `terrain-height-blend`, `terrain-multiscatter-flags`, `hair-marschner`.

## 10 · One pixel, traced back
- c10-trace: `light-culling-pass`, `tile-iterator` (the tile list), `evaluate-lighting` (direct per light), `shadow-use-fullscreen` (shadow read), `diffuse-global`, `reflection-global`, `probe-render-inner` (IBL and probe), `ssao-modulate` (AO), `msaa-resolve-custom`, `diffuse-specular-merge` (final merge).
- c10-takeaways: `forward-targets`, `stencil-refs` (five targets and routing); `brdf-execute-once`, `cubemap-ggx`, `light-culling-pass` (computed once); `shadow-filter-default`, `ssao-radius`, `ssao-modulate` (trade-offs).
- c10-sources: the chapter reading lists.
