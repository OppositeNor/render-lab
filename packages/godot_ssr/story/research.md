# Godot SSR: merged research report

Engine: Godot Engine. Version: 4.7.2-stable. Commit: `ed1daf0bf001b61586d9930840f2f1394092c079` (2026-08-16), the
commit godot_sdfgi follows. License: MIT (`LICENSE.txt` in the checkout).
Topic: screen-space reflections (SSR) of the Forward+ renderer.
Status of every claim: read from the source by one or more research agents; the merging agent re-read the lines this
file marks "checked". Nothing was run on a GPU. Numbers that come from a Python evaluation of a shader formula say so.

## 0. Scope, notation, and frame order

**Scope.** SSR exists only in the Forward+ renderer (`RenderForwardClustered`). The Mobile and Compatibility renderers
and the dummy rasterizer have stubs only; the one warning (`environment_storage.cpp:651-653`) prints only in debug
builds (section U). SSR is applied to opaque surfaces only: transparent surfaces and reflection-probe renders never
get it (sections G, A).

**Path abbreviations** (relative to the Godot checkout):

| Short | Path |
|---|---|
| RD | `servers/rendering/renderer_rd` |
| rfc.cpp | `RD/forward_clustered/render_forward_clustered.cpp` |
| sse.cpp, sse.h | `RD/effects/ss_effects.cpp`, `.h` |
| ssr.glsl | `RD/shaders/effects/screen_space_reflection.glsl` |
| hiz.glsl, downsample.glsl, filter.glsl, resolve.glsl | `RD/shaders/effects/screen_space_reflection_{hiz,downsample,filter,resolve}.glsl` |
| scene.glsl | `RD/shaders/forward_clustered/scene_forward_clustered.glsl` |

A line prefixed with `~` is approximate (within a few lines); everything else was read at this commit.

**Frame order (one view, Forward+).** Inside `RenderForwardClustered::_render_scene` (rfc.cpp:1704):

1. **Gating** (rfc.cpp:1879-1886): `using_ssr = environment_get_ssr_enabled(env) && !transparent_bg` (a transparent
   background only prints `WARN_PRINT_ONCE`, :1884). It is computed only outside reflection-probe renders (:1842).
2. **Normal-roughness is requested** (rfc.cpp:1925-1940): `using_ssr` selects `PASS_MODE_DEPTH_NORMAL_ROUGHNESS`
   (or `..._VOXEL_GI` with VoxelGI instances).
3. **Depth pre-pass** (rfc.cpp:2112-2160): writes depth and `RB_TEX_NORMAL_ROUGHNESS` (R8G8B8A8_UNORM). SSR does not
   force the pre-pass: only `used_opaque_stencil` or `rendering/driver/depth_prepass/enable` (default true) does
   (rfc.cpp:2112-2113). With the pre-pass disabled SSR still runs on unwritten normal-roughness data (section A, from
   code only); the story assumes the default.
4. **`_pre_opaque_render`** (call rfc.cpp:2182, definition :1517): shadows and GI first; then, if SSIL or SSR is on,
   the last-frame buffer is allocated (:1619-1621); SSAO and SSIL run (:1624-1637); then `_process_ssr` (:1639-1641,
   definition :1483, checked).
5. **`_process_ssr`** (rfc.cpp:1483-1508, checked): allocates the SSR buffers, builds the per-view reprojection
   matrix `last_projection * last_transform^-1 * transform * (correction * projection)^-1`, stores this frame's
   projection and transform for the next frame, and calls `SSEffects::screen_space_reflection` (sse.cpp:1467).
6. **The SSR passes** (sse.cpp:1467-1722; their GPU labels):
   - "SSR Downsample" (half size only): depth and normal-roughness to half size, keeping the nearest sample;
   - "SSR Copy Depth" (full size only): depth into Hi-Z mip 0;
   - "SSR HI-Z": mips 1 to N-1, each the max (nearest, reversed Z) of its 2x2 parents (3 wide at odd sizes);
   - "SSR Main": the trace, validation, colour fetch, and mip level, per view;
   - "SSR Roughness Filter": mips 1 to N-1 of the SSR colour, each a 2x2 downsample plus a separable 7-tap blur;
   - "SSR Resolve" (half size only): a joint-bilateral upsample to full size.
7. **The opaque pass** reads the result in the forward shader (scene.glsl:2197-2228) and blends it into the indirect
   specular light before the BRDF scale.
8. **After the transparent pass and the MSAA resolve**, the internal HDR colour is copied into `RB_LAST_FRAME`
   (rfc.cpp:2450-2453, sse.cpp:396-420): next frame's SSR reflects this image. It holds this frame's reflections
   too, so reflections of reflections build up one bounce per frame (section O).

## 0.1 Settings

| Setting | Default | Range, clamp | Where read |
|---|---|---|---|
| Environment `ssr_enabled` | false | | rfc.cpp:1879 (gating), :762 (shader flag) |
| `ssr_max_steps` | 64 | 32..512 (environment.cpp:1335) | rfc.cpp:1507, push constant `num_steps` |
| `ssr_fade_in` | 0.15 | setter `MAX(x, 0)` (environment.cpp:282) | push constant `curve_fade_in`, ssr.glsl:274 |
| `ssr_fade_out` | 2.0 | | push constant `distance_fade`, ssr.glsl:275 |
| `ssr_depth_tolerance` | 0.5 (0.2 before commit c128886c63, 2025-09-16) | 0.01..128 (environment.cpp:1338) | ssr.glsl:204, :256 |
| Project `rendering/environment/screen_space_reflection/half_size` | true | | rendering_server.cpp:3781; read once in the SSEffects constructor (sse.cpp:281); the editor re-applies it live (editor_node.cpp:508) |
| `rendering/driver/depth_prepass/enable` | true | | rfc.cpp:2113 |
| `environment_set_ssr_roughness_quality` | deprecated no-op | | rfc.cpp:4009-4010 |

## 0.2 Resources

All in `RB_SCOPE_SSR`, created by `ssr_allocate_buffers` (sse.cpp:1428-1465), kept with the render buffers, and
recreated when `half_size` changes. Array layers = view count. SSR size = internal size, or half of it by default.
Mip count: halve (floor) until either side reaches 1: 11 levels at 1920x1080, 10 at 960x540 (sections C, D).

| Texture | Format | Size | Mips | When | Line |
|---|---|---|---|---|---|
| `RB_NORMAL_ROUGHNESS` | R8G8B8A8_UNORM | SSR size | 1 | half size only | sse.cpp:1455 |
| `RB_HIZ` | R32_SFLOAT | SSR size | full chain | always | sse.cpp:1458 |
| `RB_SSR` | `get_base_data_format()` = R16G16B16A16_SFLOAT in Forward+ (checked: render_scene_buffers_rd.h:195, renderer_scene_render_rd.cpp:1160; Forward+ does not override it) | SSR size | full chain | always | sse.cpp:1459 |
| `RB_MIP_LEVEL` | R8_UNORM, stores mip / 14 | SSR size | 1 | always | sse.cpp:1460 |
| `RB_FINAL` | as `RB_SSR` | full internal size | 1 | half size only | sse.cpp:1463 |
| `RB_LAST_FRAME` (`RB_SCOPE_SSLF`, shared with SSIL) | R16G16B16A16_SFLOAT | half when SSR is half and SSIL off, else full | 6 with SSIL, else 1 | SSR or SSIL on | sse.cpp:366-394 |

## 0.3 Findings and corrections from the cross-check

Checked by the merging agent against the source unless marked otherwise.

1. **SSR never fully replaces the probe reflection with the default fades** (checked, ssr.glsl:270-284). The fade is
   `pow(ray_len, 0.15) * pow(1 - ray_len, 2.0)` on the ray's screen-space length (UV units). Its maximum is 0.581 at
   `ray_len` = 0.15 / 2.15 = 0.070 (derivative), so validity, and with it the SSR alpha, is at most 0.58; at
   `ray_len` 0.5 it is at most 0.23. The forward shader blends `indirect_specular * (1 - a) + rgb` (scene.glsl:2227),
   so at least 42 % of the probe or GI reflection always remains.
2. **The back-face test only runs within two pixels of the ray's start** (checked, ssr.glsl:245-250:
   `all(lessThan(abs(screen_ray_dir.xy * t), 2.0 / params.screen_size))`). Hits further away are not tested for facing;
   only the depth confidence rejects them.
3. **`RESOLVE_SSR` (bit 3) is set for full size** (checked, rfc.cpp:768: `!ssr.half_size`), when no resolve pass ran;
   the forward shader then picks the mip and undoes the tone compression itself. In the default half-size path the
   resolve pass has done both, and the forward shader reads `RB_FINAL` at mip 0.
4. **The specular-merge SSR path is dead in 4.7.2**: its only caller passes `RID()` (rfc.cpp:2346); composition moved
   into the forward shader in c128886c63 (2025-09-16, first in 4.6), the commit that rewrote SSR around the Hi-Z
   pyramid (sections G, U).
5. **SSR uses the unjittered projection** (checked: `_process_ssr` receives `scene_data->view_projection`,
   rfc.cpp:1640, while the TAA jitter is added only to the UBO's matrices, render_scene_data_rd.cpp:45, 65, 86), so with
   TAA the trace's projection and the jittered depth disagree by the sub-pixel jitter. Effect not measured.
6. **The "near-plane clip" clips to the camera plane** z = -0.00001 (ssr.glsl:148-150), not to the near plane
   (section D).
7. **The normal decode is exact in direction.** The pre-pass writes best-fit normals (`encode24`, scene.glsl:1185);
   `normalize(xyz * 2 - 1)` recovers the direction (section A: mean error 0.03 degrees, maximum 0.31, from a Python
   evaluation over 200,000 random normals). The back-face test (ssr.glsl:246) skips the normalize, which does not
   change the sign of its dot product.
8. **The tone-compression inverse is not clamped** (ssr.glsl:291; scene.glsl:2220; resolve.glsl ~:39-40): a stored
   compressed luminance that rounds to 1 in half float (a premultiplied luminance above about 4096) divides by zero.
   From arithmetic only; not run.
9. **No temporal accumulation, jitter, or denoiser**: the only cross-frame data are the previous frame's HDR colour
   and camera; no previous depth is kept, so disocclusion is not detected (sections O, A).
10. **No tests or demos in the checkout use SSR** (section U).

Where a section below disagrees with this list, this list is the checked version.

## 0.4 Where the lab differs

From a survey of the lab at the time of writing (`lab/src/investigation.rs`, `renderer.rs`, `gpu.rs`, `scene.rs`). Each
row is something the port must build or state on screen.

| Godot | The lab today | What the port needs |
|---|---|---|
| Depth from a depth pre-pass, single-sampled, read in compute (reversed Z, D32 or D24) | `Depth32Float`, reversed Z (Greater, clear 0), but 4x multisampled: compute reads `texture_depth_multisampled_2d` | A resolve (or sample 0) into Hi-Z mip 0; say which on screen |
| `RB_TEX_NORMAL_ROUGHNESS`, R8G8B8A8_UNORM, best-fit normals plus roughness (7 bits and a dynamic flag) | `SceneShading::prepass` with `prepass_outputs` (named for Godot's buffer); no investigation writes roughness yet | A pre-pass output with the same packing, so the decode in ssr.glsl ports line for line |
| Hi-Z (R32_SFLOAT) and SSR colour (RGBA16F) with full mip chains written per mip by compute | Chain targets and program images have one mip (`renderer.rs:65-70`, `program.rs:218, 480`); only 3D volumes have per-mip views | A lab extension (mips on `ProgramImage` or `ShadingTarget`, one view per mip, a dispatch per mip), generic, naming no investigation |
| Previous frame's HDR colour (`RB_LAST_FRAME`), copied after transparents | `SceneShading::history` keeps named targets across frames; no investigation uses it yet | A history target and a copy of `hdr` into it at the end of the frame |
| Previous frame's projection and camera transform | Not kept by the lab | Kept in the investigation's plan state (or a generic lab field) |
| `R8_UNORM` storage for the mip level | `TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES` is requested, so read-write R8Unorm and R32Float storage are available where the adapter allows | Check the adapter, or store the mip in R32Float and say so |
| Subgroups: not used by SSR | Not requested | Nothing |
| Glossy surfaces in a game scene | Scene materials have albedo and roughness only; the study room's floor and walls are roughness 1.0, the teapot 0.35, the sphere and box 0.5-0.6 | A glossy floor (a scene edit, roughness well below 0.6) so the reflections show; metallic is not needed by SSR itself |
| 1920x1080 internal size, half-size SSR at 960x540 | The lab renders at its window or snapshot size | Say the lab's SSR size on screen; numbers in the story are computed by the lab, not taken from 1080p |

No screen-space effect in the lab or a public package uses a depth pyramid yet; O3DE's SSAO uses per-pixel buffers
(`packages/o3de_lighting`).


## 1. Setup, inputs, buffers, and the temporal part (section A)

Paths: RD = servers/rendering/renderer_rd. rfc.cpp = RD/forward_clustered/render_forward_clustered.cpp, sse.cpp = RD/effects/ss_effects.cpp,
scene.glsl = RD/shaders/forward_clustered/scene_forward_clustered.glsl, ssr.glsl = RD/shaders/effects/screen_space_reflection.glsl.
All line numbers were read.

### 1. Gating chain

#### 1.1 Conditions (in order)
1. **Environment values** (settings defaults): environment_storage.h:139-143 `ssr_enabled=false, ssr_max_steps=64, ssr_fade_in=0.15, ssr_fade_out=2.0, ssr_depth_tolerance=0.5`
   (the Environment resource has the same defaults, scene/resources/environment.h:124-128). The project setting
   `rendering/environment/screen_space_reflection/half_size` is `GLOBAL_DEF(..., true)` (rendering_server.cpp:3781), read once in the SSEffects
   constructor (sse.cpp:281) and also settable via `environment_set_ssr_half_size` (rfc.cpp:4004 -> sse.cpp:1424). The class member default is `false`
   (ss_effects.h:179) but the constructor overwrites it from the setting.
2. **`using_ssr`** is declared false (rfc.cpp:1835) and can only become true in the `else` branch of `if (is_reflection_probe)` (rfc.cpp:1857, the else of the :1842 branch), so a
   reflection probe render never uses SSR. Inside, rfc.cpp:1879-1886:
   `if (p_render_data->environment.is_valid()) { ... if (environment_get_ssr_enabled(env)) { if (!p_render_data->transparent_bg) using_ssr = true; else WARN_PRINT_ONCE(...) } }`.
   So: valid environment AND env ssr_enabled AND NOT transparent_bg. Nothing else (no quality setting, no roughness/viewport check). The whole `render_scene` is the
   Forward+ (clustered) renderer; the Mobile renderer has no SSR (forward_mobile/render_forward_mobile.cpp:870 comment: "not using ssr/sss (currently not supported)").
3. **Depth pass mode** (rfc.cpp:1925-1940, inside `if (!is_reflection_probe)`): 
   - if `using_voxelgi` -> `PASS_MODE_DEPTH_NORMAL_ROUGHNESS_VOXEL_GI`;
   - else if environment valid and (`using_ssr || using_sdfgi || ssao_enabled || using_ssil || ce_needs_normal_roughness || debug normal buffer || scene_state.used_normal_texture`) ->
     `PASS_MODE_DEPTH_NORMAL_ROUGHNESS`;
   - else (no env) only debug normal / used_normal_texture selects it. Otherwise it stays `PASS_MODE_DEPTH` (rfc.cpp:1830).
   The switch (rfc.cpp:1942-1957) then sets `depth_framebuffer = rb_data->get_depth_fb(DEPTH_FB_ROUGHNESS)` (depth + RB_TEX_NORMAL_ROUGHNESS, get_depth_fb at rfc.cpp:215-222 calls
   `ensure_normal_roughness_texture()` which creates the texture, rfc.cpp:63-72) and `depth_pass_clear = {Color(0,0,0,0)}`.
   With VoxelGI, a second target (R8G8_UINT) is also attached.
4. **Depth pre-pass flag** rfc.cpp:2112-2113:
   `force_depth_pre_pass = scene_state.used_opaque_stencil;`
   `depth_pre_pass = (force_depth_pre_pass || GLOBAL_GET_CACHED(bool,"rendering/driver/depth_prepass/enable")) && depth_framebuffer.is_valid();`
   The setting defaults to **true** (rendering_server.cpp:3715; doc ProjectSettings.xml:2912). `disable_for_vendors` ("PowerVR,Mali,Adreno,Apple", rendering_server.cpp:3716) is only
   consumed in drivers/gles3/storage/config.cpp:204; grep in RD/ and drivers/vulkan found nothing, so for the RD renderer the vendor list does not change it.
   **Nothing forces the pre-pass when SSR is on.** The only forcing condition is an opaque stencil use. (Contrast: `using_ssao` requires `depth_pre_pass`, rfc.cpp:2118; `using_ssil` does not.)
5. **Pre-pass render** (rfc.cpp:2120-2165) `if (depth_pre_pass)`: draws RENDER_LIST_OPAQUE with `depth_pass_mode` into `depth_framebuffer`, `DRAW_CLEAR_ALL` (clear depth to 0.0 = reversed-Z far, and
   normal-roughness to (0,0,0,0)); with MSAA, `resolve_gi(depth_msaa, normal_roughness_msaa, ... -> depth_texture, normal_roughness)` (rfc.cpp:2144-2150).
6. **_pre_opaque_render** is called with `p_use_ssr = using_ssr` at rfc.cpp:2182 (after the pre-pass block, before the opaque pass). The normal-roughness slices passed in come from
   rfc.cpp:2176-2181 (`rb_data->has_normal_roughness()` -> `get_normal_roughness(v)`).
7. **_pre_opaque_render's SSR block** (rfc.cpp:1613-1642): `if (rb_data.is_valid() && ss_effects)`:
   - `if (p_use_ssil || p_use_ssr) ss_effects->allocate_last_frame_buffer(rb, p_use_ssil, p_use_ssr)` (:1619);
   - `if (p_use_ssao || p_use_ssil)`: downsample_depth + SSAO/SSIL (not needed by SSR);
   - `if (p_use_ssr) _process_ssr(rb, environment, p_normal_roughness_slices, view_projection, view_eye_offset, cam_transform)` (:1639-1641).
   So SSR runs after SSAO/SSIL, all in the "pre opaque" region, before the opaque colour pass, and **does not depend on SSAO/SSIL being on**.
8. **_process_ssr** (rfc.cpp:1483-1508): `ssr_allocate_buffers(..., get_base_data_format())`, reprojection math (section 4), then `ss_effects->screen_space_reflection(...)` with env max_steps/fade_in/fade_out/depth_tolerance.
9. Last-frame copy (after transparents): `if (using_ssil || using_ssr) _copy_framebuffer_to_ss_effects(...)` rfc.cpp:2450-2453 -> `ss_effects->copy_internal_texture_to_last_frame` (rfc.cpp:1510-1515). Section 5.

#### 1.2 If `rendering/driver/depth_prepass/enable = false` (and no opaque stencil)
Follow the code:
- `using_ssr` still true; `depth_pass_mode` is still `PASS_MODE_DEPTH_NORMAL_ROUGHNESS`; `depth_framebuffer` is still created (normal-roughness texture allocated by `ensure_normal_roughness_texture`),
  and `is_valid()` is true, but `depth_pre_pass` is **false**, so the block rfc.cpp:2120-2165 is skipped: no draw, no clear, no MSAA resolve.
- `_pre_opaque_render` is still called with `p_use_ssr = true` (rfc.cpp:2182), `has_normal_roughness()` is true (texture exists), so SSR runs **on whatever the buffers hold**:
  - **Normal-roughness texture:** is only written by the `MODE_RENDER_NORMAL_ROUGHNESS` fragment path of the depth pass (scene.glsl:3019-3028). The colour pass (PASS_MODE_COLOR) does not write it
    (the `MODE_RENDER_NORMAL_ROUGHNESS` define is for the depth-with-normal modes; colour-pass outputs are colour/specular/motion). So it is **never written and never cleared** in that frame (the clear is part of the pre-pass draw):
    first use = freshly created texture contents (not initialised by Godot code I read: no texture_clear for RB_TEX_NORMAL_ROUGHNESS found in rfc.cpp:63-72), later frames the same unwritten content.
    Decoded, an all-zero texture means normal = normalize(-1,-1,-1), roughness 0 (roughness 0 <0.7 so SSR traces for every non-sky pixel).
  - **Depth:** `rb->get_depth_texture()` is the main depth attachment. The opaque pass clears it at its start (`depth_pre_pass ? DRAW_DEFAULT_ALL : DRAW_CLEAR_DEPTH`, rfc.cpp:2224), which is **after**
    `_pre_opaque_render`. So at SSR time the depth texture holds the *previous frame's* depth (no-MSAA: the depth attachment as left by the last opaque pass; MSAA: the resolved depth,
    resolved at "Resolve" rfc.cpp:2433-2436 at the end of the previous frame). First frame: whatever the fresh texture contains.
  So SSR would run with last-frame depth and an unwritten normal buffer. I did not find any guard or warning for this combination (grep "depth_pre_pass" in rfc.cpp: lines 2113, 2118, 2120, 2167, 2224 only).
  Not run; this is read from code only. Practical conclusion for the story: SSR needs the pre-pass (default on); the code does not enforce it.
- Also: with pre-pass off, SSAO is off too (`using_ssao` needs `depth_pre_pass`), but SSIL/SSR still run.

### 2. Normal-roughness buffer

#### 2.1 Writer
- Texture: `RB_TEX_NORMAL_ROUGHNESS`, format **R8G8B8A8_UNORM** (rfc.cpp:263-265), usage `get_color_usage_bits(resolve, msaa, storage)` (rfc.cpp:267-269); MSAA twin `RB_TEX_NORMAL_ROUGHNESS_MSAA`.
  Full internal size, view_count layers.
- Written by the depth pre-pass fragment shader, scene.glsl:3019-3028:
  ```
  normal_roughness_output_buffer = vec4(encode24(normal) * 0.5 + 0.5, roughness);
  w = w * (127.0/255.0);
  if (instances.data[instance_index].flags & INSTANCE_FLAGS_DYNAMIC) w = 1.0 - w;
  ```
  (the trailing `w = w;` at :3028 is a no-op). `normal` is the fragment's **view-space** normal after normal mapping (scene.glsl:1442 `normal = normalize(mix(normal, tbn*normal_map, depth))`, geometry normal otherwise :1444),
  flipped on back faces when DO_SIDE_CHECK (scene.glsl:1264-1266, `normal_highp = -normal_highp`). `roughness` is the material roughness after decals (:1635) and, when the
  screen-space roughness limiter is on (project default `rendering/anti_aliasing/screen_space_roughness_limiter/enabled=true`, amount 0.25, limit 0.18, rendering_server.cpp:3745-3747), after
  Kaplanyan-style geometric specular AA (scene.glsl:1655-1672, the "Improved Geometric Specular AA" square-enix paper). I did not follow whether the pre-pass variant compiles these blocks (needs NORMAL_USED, which the normal output implies).
- INSTANCE_FLAGS_DYNAMIC: instance flag set for non-static objects (meaning only checked at the shader level; not traced further, belongs to composition/other subtopics).

#### 2.2 encode24 (scene.glsl:1183-1203, guarded `#if defined(MODE_RENDER_NORMAL_ROUGHNESS) || defined(MODE_RENDER_MATERIAL)`)
```
vec3 encode24(vec3 v) {
  vec3 vNormalUns = abs(v);
  float maxNAbs = max(vNormalUns.z, max(vNormalUns.x, vNormalUns.y));
  vec2 vTexCoord = vNormalUns.z < maxNAbs ? (vNormalUns.y < maxNAbs ? vNormalUns.yz : vNormalUns.xz) : vNormalUns.xy;
  vTexCoord /= maxNAbs;
  vTexCoord = vTexCoord.x < vTexCoord.y ? vTexCoord.yx : vTexCoord.xy;
  vTexCoord.y /= vTexCoord.x;
  float fFittingScale = texture(sampler2D(best_fit_normal_texture, SAMPLER_NEAREST_CLAMP), vTexCoord).r;
  vec3 result = v / maxNAbs;     // touch the unit cube
  result *= fFittingScale;       // scale along the ray to the best-fit length
  return result;
}
```
This is "Best-Fit Normals" from Kaplanyan, *CryENGINE 3: Reaching the Speed of Light* (SIGGRAPH 2010; comment cites it, scene.glsl:1184, best_fit_normal.glsl:12-15).
The scale-lookup table: `best_fit_normal_texture` (set 0 binding 16, scene_forward_clustered_inc.glsl:303), a 1024x1024 **R8_UNORM** texture generated once at renderer init by compute shader
best_fit_normal.glsl (rfc.cpp:5160-5198; created rfc.cpp:5171-5176). Per texel: `uv=(gid+0.5)/1024; uv.y*=uv.x; dir=(uv.x,uv.y,1)`; `find_minimum_error` tries 126 lengths t = nstep/127.5, nstep=1.5..126.5
(max t = 126.5/127.5 = 0.99216), quantises `dir*t` to 8 bits (`round(clamp(c*.5+.5,0,1)*255)/255*2-1`) and keeps the t with the smallest max-component relative error `max|(q-vp)/t|`.
So the table stores the best length (stored as 8-bit, quantised to 1/255, so t itself has 8-bit precision, a small additional error) for direction (x, y, 1) with 1 >= x >= y/x ... i.e. a collapsed 2D cube-face parametrisation (x in [0,1], y/x in [0,1]).
Godot differs from CryEngine only in that the table is generated on the GPU at startup, not shipped as a file (not claimed to differ from the paper otherwise).

#### 2.3 Decodes in SSR shaders
All are `normalize(texel.xyz * 2.0 - 1.0)` unless noted:
- ssr.glsl:116 (trace start; `source_normal_roughness` = the half-size copy when half_size, else the main normal buffer) - normalised.
- ssr.glsl:246 (hit validation: `texelFetch(source_normal_roughness, cur_pixel_pos, 0).xyz * 2.0 - 1.0`, **not normalised**; only the sign of `dot(ray_dir, hit_normal)` is used, so the missing normalize is harmless).
- screen_space_reflection_resolve.glsl:26 (half sample, `source_normal_roughness_half`) and :67 (full-res pixel) - normalised.
- screen_space_reflection_downsample.glsl:58: copies the texel unchanged (no decode, rgba8 -> rgba8) from the **sample that won the depth test** (greatest depth value = nearest, reversed-Z, `if (depth < sample_depth)` :23); with odd sizes extra row/column samples are included (:43-55).
- screen_space_reflection_filter.glsl: no reference to normal (grep: none).
- screen_space_reflection_hiz.glsl: none.
Also outside SSR: scene.glsl:1990-2001 (SSIL upsample) decodes the same way.

#### 2.4 Is `normalize(xyz*2-1)` the inverse of encode24?
It is the exact inverse **in direction**: encode24 returns `v * (fittingScale / maxNAbs)` with a positive scalar; `*0.5+0.5` is affine, so `xyz*2-1` gets back that vector (up to 8-bit rounding), and `normalize` removes the scale. The scale is exactly what the table optimises. Not an approximation of a different mapping, not a mismatch.
The decoded vector is not unit length before the normalize (its components are within [-0.992, 0.992], the longest equal to fittingScale <= 0.992); skipping normalize at ssr.glsl:246 is harmless because only a dot-product sign is used.
Numerical check (Python evaluation, 200k random unit normals; table emulated by the compute shader's own formula at the nearest texel centre; 8-bit UNORM rounding):
- unquantised decode error: 1.7e-6 degrees (i.e. exact);
- with 8-bit quantisation: mean 0.033 deg, p99 0.123 deg, max 0.309 deg;
- for comparison, plain `n*0.5+0.5` in 8 bits: mean 0.170, p99 0.318, max 0.382 deg.
(The simulation does not model the 8-bit rounding of the table value itself nor texture storage of the R8 table; so treat the 0.31 deg max as indicative.)
No sign/alias issue: no component can quantise to exactly 0 (127.5 is not an integer), irrelevant for normalize.

#### 2.5 Roughness packing and decode
- Encode (scene.glsl:3023-3027): `w = roughness * 127/255` in [0, 0.498]; dynamic instances: `w = 1 - w` in [0.502, 1]. Stored in the alpha channel of R8G8B8A8_UNORM (8-bit, 1/255 steps; 127 roughness levels, resolution about 0.0079).
  Meaning: alpha < 0.5 => static instance, > 0.5 => dynamic instance (used by SSIL/SDFGI-like consumers; SSR only wants the roughness).
- Decode (identical in ssr.glsl:117-121, resolve.glsl:27-31 and :68-72):
  ```
  roughness = w; if (roughness > 0.5) roughness = 1.0 - roughness; roughness /= (127.0/255.0);
  ```
  Exact inverse (modulo 8-bit rounding). Example: roughness 1.0 static -> w 0.498 (stays), dynamic -> 0.502 -> 0.498 -> 1.0. The 0.5 threshold is never hit by an encoded value (max static 0.498).
  Clear value (0,0,0,0) (no geometry: sky) decodes as roughness 0 / normal normalize(-1,-1,-1), but sky pixels are skipped via depth == 0 (ssr.glsl:111 `should_trace = screen_pos.z != 0.0`).
- Gate in the trace: `roughness >= 0.7` -> write zero colour and zero mip and return (ssr.glsl:125-129). (Trace details belong to D.)

### 3. Resource allocation

#### 3.1 `ssr_allocate_buffers` (sse.cpp:1428-1465), called every frame from _process_ssr (rfc.cpp:1492) with `p_color_format = rb->get_base_data_format()`
- Preamble: `if (p_ssr_buffers.half_size != ssr_half_size) clear_context(RB_SCOPE_SSR)` (:1429-1431). SSRRenderBuffers.half_size starts false (ss_effects.h:149), so the first call clears an empty context (harmless).
- `size = ssr_half_size ? internal_size/2 : internal_size` (integer division: odd sizes lose a pixel), :1433-1434.
- **mipmaps** (:1436-1451): start 1; `while (cur_width > 1 && cur_height > 1) { cur_width/=2 if >1; cur_height/=2 if >1; ++mipmaps; }`. So the count is determined by the **smaller** dimension (stops when either reaches 1; the loop does not count mips until the larger hits 1, unlike the usual floor(log2(max))+1).
  Examples (computed by hand from the loop): 960x540 -> 10 mips (960,540 -> 480,270 -> 240,135 -> 120,67 -> 60,33 -> 30,16 -> 15,8 -> 7,4 -> 3,2 -> 1,1); 1920x1080 -> 11 mips. Note the shader's `mip_level / 14.0` and `ssr_mip_level *= 14.0` assume max 14 (scene.glsl:2209).
  `mipmaps` is stored in `p_ssr_buffers.mipmaps` and passed to the shaders as `mipmaps`.
- Textures, all `RB_SCOPE_SSR`, `TEXTURE_SAMPLES_1`, `view_count` layers (2D array when >1, via create_texture rfc: render_scene_buffers_rd.cpp:301-303), usage = SAMPLING | STORAGE unless stated:
  | name | when | format | size | mips |
  |---|---|---|---|---|
  | RB_NORMAL_ROUGHNESS | half_size only | R8G8B8A8_UNORM | size (half) | 1 |
  | RB_HIZ | always | R32_SFLOAT | size | mipmaps |
  | RB_SSR | always | base_data_format = R16G16B16A16_SFLOAT | size | mipmaps |
  | RB_MIP_LEVEL | always | R8_UNORM | size | 1 |
  | RB_FINAL | half_size only | base_data_format | internal_size (full) | 1 |
  `get_base_data_format()` = `force_hdr ? R16G16B16A16_SFLOAT : preferred_data_format` (render_scene_buffers_rd.h:195), `preferred_data_format` = `_render_buffers_get_preferred_color_format()` = **R16G16B16A16_SFLOAT**
  (renderer_scene_render_rd.cpp:1160-1162, set at :247). Both branches give R16G16B16A16_SFLOAT in 4.7.2.
  No COLOR_ATTACHMENT, no CAN_COPY bits on these. No explicit clear value: they are created uninitialised and fully overwritten every frame (HiZ mip 0 by downsample/copy-depth, other mips by the HiZ passes; SSR by the main pass which writes every pixel, except pixels with `pixel_pos >= screen_size`; roughness filter writes mips 1+).
- **Recreation:** `create_texture` returns the existing texture if the name exists (render_scene_buffers_rd.cpp:331-335) and never resizes. Textures are therefore recreated only when the context is cleared: (a) half_size toggled (:1429), (b) the whole RenderSceneBuffersRD is reconfigured (viewport/internal-size/MSAA/view-count change clears contexts; I did not follow configure()). The RB_HIZ/RB_SSR textures are not separately checked against size.
- MIP_LEVEL (R8) stores `mip_level/14` (ssr.glsl:328) so 8 bits give 1/255 of 14 levels.

#### 3.2 `allocate_last_frame_buffer(rb, use_ssil, use_ssr)` (sse.cpp:366-394)
- `last_frame_size = internal_size`; `if (!use_ssil && use_ssr && ssr_half_size) last_frame_size /= 2` (:371-373).
- `mipmaps = 1`; `if (use_ssil) mipmaps = 6` (:375-377). (6 is SSIL's blur levels; SSR just reads mip 0 or 1.)
- View count = `get_view_count()` layers.
- Recreate when: no texture, or width/height/mipmaps/array_layers differ (:381-386). Then `clear_context(RB_SCOPE_SSLF)` and
  `create_texture(RB_SCOPE_SSLF, RB_LAST_FRAME, R16G16B16A16_SFLOAT, SAMPLING|STORAGE|CAN_COPY_TO, 1 sample, last_frame_size, view_count, mipmaps)`, immediately `texture_clear(tex, Color(0,0,0,0), all mips, all layers)` (:391-394).
  So **on every (re)creation the previous frame is black transparent**. Triggers: first SSR/SSIL frame, resolution change, SSIL toggled while SSR on (size full <-> half and mips 1 <-> 6), half_size setting toggled.
- It is allocated inside `_pre_opaque_render` the same frame, before SSR runs (rfc.cpp:1619) and **before the copy of that frame**, so on the first frame the SSR trace samples the cleared texture.
- Not freed when SSR/SSIL are switched off (no code path found), but not updated while they are off either (copy is conditional, rfc.cpp:2450).

### 4. Reprojection matrix (rfc.cpp:1498-1505; `_process_ssr`)
Per view v:
```
Projection correction; correction.set_depth_correction(true);               // flip_y=true, reverse_z=true, remap_z=true (projection.h:76 defaults)
Projection projection = correction * p_projections[v];                      // P_cur  (p_projections = scene_data->view_projection, rfc.cpp:1640; NOT jittered)
reprojections[v] = ssr_last_frame_projections[v]                            // P_prev
                 * Projection(ssr_last_frame_transform.affine_inverse())    // T_prev^-1  (world -> prev camera view)
                 * Projection(p_transform)                                  // T_cur      (cur camera view -> world), p_transform = scene_data->cam_transform
                 * projection.inverse();                                    // P_cur^-1
ssr_last_frame_projections[v] = projection;                                 // store for next frame
ssr_last_frame_transform = p_transform;                                     // (after the loop, once for all views)
```
- **depth_correction matrix** (projection.cpp:787-807, column-major, m[col*4+row]): identity except m[5]=-1 (y flipped), m[10]=-0.5 (reverse_z, remap_z), m[14]=0.5 (column 3 row 2). Applied to a clip-space vector c (OpenGL-style from Godot's camera
  projection): x' = c.x, y' = -c.y, z' = -0.5*c.z + 0.5*c.w, w' = c.w. After the divide: ndc_z' = 0.5 - 0.5*ndc_z_gl, i.e. GL near (-1) -> 1, far (+1) -> 0. This is the reversed-Z [0,1] Vulkan convention the depth buffer uses, y points down (texture row 0 = top).
  So `P_cur` maps view space (camera looks down -z) straight to Vulkan clip space; screen uv = ndc.xy*0.5+0.5, z = depth buffer value (1 near, 0 far).
- **Reading right to left** the matrix maps the point `(ndc.x, ndc.y, depth, 1)` of the **current** frame's pixel:
  1. `P_cur^-1`: current screen/NDC -> current view space (homogeneous; the w from the inverse is divided later because the final divide covers it; valid because `(ndc,1)` is a legal clip vector for a projective inverse);
  2. `T_cur`: current view space -> world (cam_transform is camera-to-world);
  3. `T_prev^-1`: world -> previous camera view space;
  4. `P_prev`: previous view space -> previous NDC (same depth correction).
  The shader applies it at ssr.glsl:224-229: `reprojected_pos = reprojection * vec4(cur_screen_pos.xy*2-1, cur_screen_pos.z, 1); xy = xy/w*0.5+0.5` and uses xy as the UV into last-frame colour (:287). z of the result is discarded.
- **Which point is reprojected:** `cur_screen_pos` = the ray's end point (hit) of this frame, i.e. a surface seen now, assumed to have been at the same world position last frame.
- **Transforms in multiview:** `p_projections[v]` = `projection[v] * view_offset[v].inverse()` (renderer_scene_render.cpp:188, XR) so P includes the eye offset; `cam_transform` is the central camera. The product is consistent (per-eye NDC -> centre view -> world -> prev centre view -> prev per-eye NDC).
- **First frame / initial state:** `ssr_last_frame_projections[v]` and `ssr_last_frame_transform` are default-constructed members; `Projection` defaults to identity (projection.h:55-60 default member initialiser), `Transform3D`/`Basis` are `= default` (identity basis assumed, not read). So on the first frame
  reprojection = I * I * T_cur * P_cur^-1: a meaningless mapping. The sampled last-frame texture is cleared to 0 on the first frame (3.2), so rgb = 0; but `color = vec4(rgb, 1.0) * validity` (ssr.glsl:287), so **alpha is validity, not 0**: the first frame (and the first frame after any last-frame texture recreation) can write opaque-black reflections that darken the indirect specular via `indirect_specular_light*(1-ssr.a)` (scene.glsl:2227), subject to the other validity terms.
- **Camera cuts / teleports:** no detection or reset anywhere in _process_ssr (no comparison of transforms, no flag). After a cut, the reprojection still maps hits to the old image; the sample is simply wrong/ghosted. The `margin_blend` and `validity` reject only samples whose reprojected UV is near/outside the screen edge (ssr.glsl:259-268). 
- **When SSR was off for some frames:** transforms/projections are updated only inside _process_ssr, while the colour copy happens when SSIL **or** SSR is on. With SSIL on and SSR off for a while and SSR switched on later, the last-frame texture is fresh but the stored transform is stale -> one frame of mis-registered reflections (inference from the two code locations).
- **Moving objects:** the matrix is camera-only (T_prev/T_cur are camera transforms; no per-object or velocity buffer, no motion-vector pass is read). A moving object's hit point gets projected to where a *static* point at that world position would have been; the colour sampled from last frame is wherever the object (or something else) was, so reflections of moving objects lag/ghost by one frame of motion, and a reflection ray that hits a surface seen last frame as something else picks that colour. Consequently an object reflected in a mirror is seen at last frame's camera-relative place only for static geometry.
- **TAA jitter:** SSR's `projection` uses the un-jittered `view_projection`, while the scene matrices applied to rasterised geometry get the jitter offset (render_scene_data_rd.cpp:42-47 `correction.add_jitter_offset(taa_jitter)`). Depth reconstruction in SSR is therefore sub-pixel off when TAA/FSR2 jitter is active (I read the jitter code only at that level; magnitude not measured).

### 5. Last-frame colour

#### 5.1 What is copied and when
- `_copy_framebuffer_to_ss_effects` (rfc.cpp:1510-1515) -> `copy_internal_texture_to_last_frame` (sse.cpp:396-420) for each view and mip m of RB_LAST_FRAME:
  - source for m=0: `rb->get_internal_texture(v)` = `RB_TEX_COLOR` slice (render_scene_buffers_rd.h:257-262), the **internal (render-resolution, pre-post-process) HDR colour**, format R16G16B16A16_SFLOAT. For m>0: the previous mip of the last-frame texture itself.
  - dest m=0 same size: `copy_to_rect(source, dest, rect, flip_y=false, force_luminance=false, all_source=false, 8bit=false, alpha_to_one=false, sanitize_inf_nan=true)` (sse.cpp:408; signature copy_effects.h:369). Alpha is copied as is; NaN/Inf are replaced by (100,100,100,1) (copy.glsl:233-236).
  - otherwise (`m>0` or dest smaller than the source, i.e. half-size w/o SSIL): `make_mipmap(source, dest, dest_size)` = **2x2 box average** with `texelFetch` at `(pos<<1)+{0,1}^2`, same NaN/Inf -> 100 sanitising (copy.glsl:82-94). For odd source sizes the last row/column is ignored.
- **Where in the frame** (rfc.cpp:2410-2453, in order): opaque pass (incl. sky, `draw_sky` in the opaque/sky step) -> post-opaque compositor effects -> [SSS + "merge specular" if separate specular, rfc.cpp:2334-2345] -> transparent pass (rfc.cpp:2410-2423) ->
  "Resolve" MSAA colour into the internal texture (:2425-2437, `texture_resolve_multisample(color_msaa, internal_texture)` per view) -> **copy** (:2450-2453, label "Copy Framebuffer for SSIL/SSR") -> post-transparent compositor effects -> (afterwards, :2460+) FSR2/TAA/upscale and the post-process (tonemap, glow, DOF, exposure) in `_render_buffers_post_process`.
  So the copy is **pre-tonemap, pre-glow, pre-TAA/FSR, MSAA-resolved, including sky, fog/volumetric fog already blended in the shader, SSS-processed colour, SSR itself (previous frame's reflections are in it: reflections of reflections accumulate), and transparents**. Alpha channel of the internal texture is cleared to 0 for opaque (rfc.cpp:2207-2214) but SSR reads `.xyz` only (ssr.glsl:287).
  Post-transparent compositor effects and anything done after run too late to be in the copy.
- It contains the luminance multiplier / exposure normalisation baked in by the scene shaders (renderer uses `rb->get_luminance_multiplier()`, rfc.cpp:710; sky is divided by it, rfc.cpp:2010). I did not trace how it is applied in scene.glsl (grep for luminance_multiplier in scene.glsl found nothing), so the exact scaling of "HDR value" is not verified.
- **Latency: exactly one frame.** SSR (in frame N, before the opaque pass) reads the copy made at the end of frame N-1; the current frame's colour does not exist yet.

#### 5.2 Sizes and mips
| SSIL | SSR half_size | last-frame size | mips | SSR reads |
|---|---|---|---|---|
| off | true (default) | internal/2 | 1 | mip 0 slice (size == SSR size), filled by 2x2 box downsample from internal |
| off | false | internal | 1 | mip 0 (full-res copy) |
| on | true | internal | 6 | slice mip **1** (size internal/2; built by `make_mipmap` box 2x2 of mip 0, sse.cpp:403-416) |
| on | false | internal | 6 | mip 0 |
Selection code sse.cpp:1553-1558: `last_frame_texture = get_texture_slice(RB_SCOPE_SSLF, RB_LAST_FRAME, v, 0); if (ssr_half_size && texture_size(last_frame_texture) != ssr_buffers.size) last_frame_texture = ...slice(v, 1);` with the comment "SSIL is likely also enabled".
Note `get_texture_slice(..., mip)` with default 1 mip, so the shader sees a single-level image; `textureLod(source_last_frame, uv, 0)` always reads level 0 of that slice.

#### 5.3 How ssr.glsl samples it
- Binding: `uniform sampler2D source_last_frame` (set 0 binding 0, ssr.glsl:9), bound with `linear_sampler` = `sampler_rd_get_default(LINEAR_WITH_MIPMAPS, REPEAT_DISABLED)` (sse.cpp:1497, :1596 -> u_last_frame) => bilinear, clamp-to-edge.
- ssr.glsl:287: `color = vec4(textureLod(source_last_frame, reprojected_pos.xy, 0).xyz, 1.0) * validity;` Only done if `validity > 0.0`; LOD 0 explicit (no mip bias). The roughness blur is not done here (it is the later mip chain of the SSR output, built by the filter pass); the last-frame texture is only sampled at mip 0 (or the mip-1 slice).
- Then tone-mapped for filtering: `color.rgb /= 1 + dot(color.rgb, rec709_luminance_weights)` (ssr.glsl:290-291); inverted in the resolve pass (resolve.glsl:38-40) or in the forward shader (scene.glsl:2218-2221).
- UV: `reprojected_pos.xy` in [0,1] of the previous frame's image at the **trace resolution** (full UVs are resolution independent).

### 6. UBO and push constants for "SSR Main"

#### 6.1 UBO `SceneData` (ssr.glsl:15-21, std140, set 0 binding 5; C++ `ScreenSpaceReflectionSceneData`, ss_effects.h:447-452; filled sse.cpp:1469-1498; the buffer `ssr.ubo` created once, `buffer_update` each frame)
Each array has 2 slots (view 0, view 1); `store_camera` writes `columns[i][j]` to `[i*4+j]` (column-major, as GLSL expects; sse.cpp:43-49). Slot 1 is left uninitialised (struct on stack, not zeroed) when view_count == 1; shader only reads `params.view_index`.
- `mat4 projection[2]` = `correction * p_projections[v]` (depth correction: flip_y, reverse-Z, 0..1 z), view space -> screen NDC (x,y in [-1,1], z depth [0,1], 1 near). Used by `compute_screen_pos` (ssr.glsl:59-64).
- `mat4 inv_projection[2]` = inverse of the above. Used by `linearize_depth` (:44-48, returns view-space z, negative in front of camera) and `compute_view_pos` (:50-57).
- `mat4 reprojection[2]` = section 4 matrix.
- `vec4 eye_offset[2]` = `p_eye_offsets[v].xyz, 0` (`scene_data->view_eye_offset`; (0,0,0) for non-XR). Used for `view_dir = normalize(pos + eye_offset)` in perspective (:137).

#### 6.2 Push constant `Params` (ssr.glsl:23-36; C++ `ScreenSpaceReflectionPushConstant`, ss_effects.h:454-464; set sse.cpp:1613-1624)
| field | value |
|---|---|
| `ivec2 screen_size` | `ssr_buffers.size` (internal/2 in half mode, else internal) = size of HiZ mip 0 / SSR output |
| `int mipmaps` | `ssr_buffers.mipmaps` (section 3.1: count of HiZ/SSR mips) |
| `int num_steps` | `p_max_steps` = env `ssr_max_steps` (default 64) - max loop iterations (`cur_iteration`, :170) |
| `float distance_fade` | env `ssr_fade_out` (default 2.0); exponent `pow(clamp(1-ray_len,0,1), distance_fade)` (:275). 0 disables |
| `float curve_fade_in` | env `ssr_fade_in` (default 0.15); exponent `pow(clamp(ray_len,0,1), curve_fade_in)` (:274). 0 disables |
| `float depth_tolerance` | env `ssr_depth_tolerance` (default 0.5), view-space units (metres), thickness test `z0 - z1 > tolerance` (:204) and confidence `smoothstep(0, tolerance, delta)` (:256) |
| `bool orthogonal` | `p_projections[v].is_orthogonal()` -> view_dir (0,0,-1) (:137) |
| `int view_index` | v (index into UBO arrays) |
| `pad1..3` | unused (struct is 12 ints/floats = 48 bytes, multiple of 16 for push constants) |
Note the C++ param names: C++ `distance_fade` <- `p_fade_out`, `curve_fade_in` <- `p_fade_in` (sse.cpp:1620-1621); the arguments are passed from rfc.cpp:1507 in the order (max_steps, fade_in, fade_out, depth_tolerance), matching the declaration (sse.cpp:1467).
One dispatch per view: `compute_list_dispatch_threads(size.width, size.height, 1)` (sse.cpp:1633), local size 8x8x1 (ssr.glsl:7).

#### 6.3 Other passes' inputs (for reference)
- SSR Downsample (half only): push `{screen_size = half size, pad}` (screen_space_reflection_downsample.glsl:14-18), reads depth texture + normal-roughness (nearest sampler), writes `dest_depth` r32f (HiZ mip 0) and `dest_normal_roughness` rgba8; picks the **max-depth (nearest) of the 2x2 quad (plus extra row/col for odd sizes)** and its normal-roughness; four shader variants ODD_WIDTH/ODD_HEIGHT/both/default (sse.cpp:1511-1520).
- SSR Copy Depth (full only): `copy_depth_to_rect(depth_texture, HiZ mip 0)` (sse.cpp:1535-1540).
- Main trace inputs: `source_hiz` = HiZ texture, all mips (`get_texture_slice(RB_SCOPE_SSR, RB_HIZ, v, 0, 1, mipmaps)`, sse.cpp:1593), nearest sampler; `source_normal_roughness` = half copy (half) or the main buffer (full), nearest; outputs `output_color` rgba16f (RB_SSR mip 0) and `output_mip_level` r8 (`mip_level/14`).

### Corrections to the "known" facts
- Known list says "gating: env ssr_enabled && !transparent_bg; never for reflection probes" - confirmed, plus a valid environment is required and Forward+ only; nothing forces the depth pre-pass.
- "ssr_max_steps 64 ... half_size = true" confirmed. "last frame copy rfc.cpp:2450-2453, sse.cpp:396-420" confirmed; allocation is sse.cpp:366-394 (not :396-420).
- The depth pre-pass is not part of "gating"; if `depth_prepass/enable=false` SSR reads last frame's depth and an unwritten normal-roughness texture (section 1.2).
- Normal decode is the exact inverse of encode24 (Best-Fit Normals); only a roughness-style decode quirk is worth mentioning: ssr.glsl:246 does not normalize.
- The last-frame colour is pre-tonemap, post-transparent, post-MSAA-resolve, pre-TAA/FSR, includes the previous frame's SSR; and the mip count of the SSR/HiZ chain stops when the smaller dimension reaches 1.

### Unclear / needs checking
1. Behaviour with `depth_prepass/enable=false`: read only from code; texture initial contents for the normal-roughness texture are undefined/zero depending on the driver (no explicit clear found). Not run.
2. `Transform3D`/`Basis` default identity not read (Projection default identity confirmed); irrelevant in effect since the colour texture is cleared.
3. Whether `RenderSceneBuffersRD` reconfigure (resolution/MSAA change) clears RB_SCOPE_SSR (HiZ/SSR) - not followed; only the half_size toggle clear is in the SSR code.
4. How `luminance_multiplier` is applied to colour in scene.glsl (grep found no occurrence by that name there); matters for stating "the units of the last-frame colour".
5. Whether the roughness limiter blocks (scene.glsl:1655-1672) are compiled in the depth-normal-roughness variant; and `INSTANCE_FLAGS_DYNAMIC` semantics.
6. MSAA resolve of the normal-roughness (`resolve_gi`, rfc.cpp:2144-2150): which sample/averaging it uses was not read.
7. The numeric normal error was computed with the table emulated at the nearest texel centre; the GPU table is also stored in 8 bits (R8), not modelled.


## 2. The Hi-Z pyramid (section C)

Paths: sse.cpp = servers/rendering/renderer_rd/effects/ss_effects.cpp; shaders in renderer_rd/shaders/effects/.

### 1. Passes (sse.cpp screen_space_reflection)
- Samplers: `linear_sampler` and `nearest_sampler` (NEAREST_WITH_MIPMAPS) created at sse.cpp:1500-1501 area; all Hi-Z/downsample reads are `texelFetch`, so the sampler is irrelevant to values.
- Half size (`ssr_half_size`, project setting read at sse.cpp:281; sse.cpp:1503 `if (ssr_half_size)`): label "SSR Downsample", per view v:
  - push constant `ScreenSpaceReflectionDownsamplePushConstant {int screen_size[2]; int pad[2]}` (ss_effects.h ~427-430), screen_size = `p_ssr_buffers.size` = HALF size (internal_size/2, integer division; sse.cpp:1434 area in ssr_allocate_buffers), not the source size.
  - sources: `get_depth_texture(v)` (full-size scene depth; D32_SFLOAT_S8 or D24_UNORM_S8 depending on storage support, render_scene_buffers_rd.cpp:810-817) and `p_normal_roughness_slices[v]` (full-size R8G8B8A8_UNORM). Destinations (storage images): RB_HIZ mip 0 slice (R32_SFLOAT) and the SSR-scope RB_NORMAL_ROUGHNESS (R8G8B8A8_UNORM, half size, created only when half size, sse.cpp ~1455).
  - odd test is on the SOURCE depth texture size: `parent_size = texture_size(source_depth_texture)`, width/height `% 2` -> 4 modes (sse.cpp ~1514-1527): DEFAULT / ODD_WIDTH / ODD_HEIGHT / ODD_WIDTH_AND_HEIGHT. Enums ss_effects.h:422-426 (down) and :432-437 (hiz); shader variants are strings of `#define MODE_ODD_WIDTH` / `MODE_ODD_HEIGHT` pushed in order, sse.cpp:285-311 (down 287-290, hiz 301-304); one pipeline per variant. Variant is chosen per dispatch at run time from the size (so a 1921x1081 internal size picks ODD_WIDTH_AND_HEIGHT).
  - `compute_list_dispatch_threads(list, W_half, H_half, 1)`: the shader has local size 8x8x1, so groups = ceil(W/8) x ceil(H/8); the shader early-outs `any(greaterThanEqual(pixel_pos, screen_size))`.
- Full size: label "SSR Copy Depth" (sse.cpp ~1549-1556): `p_copy_effects.copy_depth_to_rect(depth_texture(v), RB_HIZ slice(v, mip 0), Rect2i(0,0,size))` (see section 5).
- "SSR HI-Z" (sse.cpp ~1558-1606): for each view, `for m = 1; m < mipmaps`:
  - push constant `{screen_size = (MAX(1, W>>m), MAX(1, H>>m)), pad}` = dest level size; dispatch_threads of that size (8x8 groups).
  - source: full-size mode and m==1: `get_depth_texture(v)` DIRECTLY (comment at sse.cpp ~1570: "Reuse the depth texture to not create a dependency on the previous depth copy pass"); otherwise (half size m==1, or any m>=2) the RB_HIZ slice m-1. So in full-size mode mip 1 is computed from the raw depth buffer, in parallel (no dependency) with the copy that writes mip 0. In half-size mode mip 1 reads the downsample output (mip 0).
  - odd test again on the SOURCE texture: `texture_size(source)` (for full-size m==1 that is the depth texture size = mip-0 size, same thing; for the others the parent level's size, which is the full-size `W>>(m-1)` the allocator made). Same 4 variants.
  - Dest = RB_HIZ slice m (storage image, r32f binding 1). Source binding 0 is a sampler+texture single mip slice.
- Barriers: none written by hand. Every dispatch is its own `compute_list_begin()/end()` (no barrier args; 4.7 RenderingDevice builds dependencies automatically from the resources each list reads/writes). That is also why the full-size m==1 trick matters: reading depth (not the HIZ mip 0) avoids a read-after-write edge between copy and mip 1.
- Later consumer: SSR Main binds `get_texture_slice(RB_SCOPE_SSR, RB_HIZ, v, 0, 1, mipmaps)` (layer v, all mips, sse.cpp ~1637) as `source_hiz`, with push constants screen_size, mipmaps = `p_ssr_buffers.mipmaps`.

### 2. screen_space_reflection_downsample.glsl (59 lines, all read)
- local_size 8x8. Bindings: 0 `sampler2D source_depth`, 1 `sampler2D source_normal_roughness`, 2 `r32f writeonly image2D dest_depth`, 3 `rgba8 writeonly image2D dest_normal_roughness`; push constant ivec2 screen_size (half size), ivec2 pad.
- main (lines 29-59): `sample_pos = pixel*2`; depth = texel at (2x,2y); `get_sample` for (+1,0),(0,+1),(+1,1); the winner rule (lines 20-27): `if (depth < sample_depth) {depth = sample_depth; winner_pos = sample_pos;}` i.e. LARGEST raw depth wins; ties keep the earlier one in scan order (0,0),(1,0),(0,1),(1,1),(extras). Under reversed Z (1 = near, 0 = far) largest = NEAREST surface in the footprint.
- Odd extension (`#ifdef MODE_ODD_WIDTH`: add (2,0),(2,1); `MODE_ODD_HEIGHT`: add (0,2),(1,2); both: add (2,2)). So for an odd source dimension EVERY half-res texel gets a 3-wide (or 3-tall) footprint, not only the last one: pixel p reads 2p..2p+2. Neighbouring half texels therefore overlap by one source column/row (column 2p+2 is read by p and by p+1 as its 2p'+0). Hmm, this is the code as written: with MODE_ODD_WIDTH the +2 samples are added for all pixel_pos, not just the last. (Same in hiz.glsl.) The trailing source column (index W-1 = 2*half_W) is read only by the last texel; with W odd, half_W = (W-1)/2, last texel reads 2*half_W-... = W-3, W-2, W-1. OK, nothing is dropped.
- Output: `dest_depth[pixel] = vec4(depth,0,0,0)` (raw device depth of the winner, no conversion; R32F). `dest_normal_roughness[pixel] = texelFetch(source_normal_roughness, winner_sample_pos, 0)` - the full RGBA8 texel at the SAME source position that won the depth contest (normal xyz*0.5+0.5, roughness/alpha as the pre-pass wrote it). So normal+roughness are those of the nearest surface, consistent with the depth; no averaging, no re-normalisation.
- Reads at the *full-resolution* depth directly (no mip), so the half-size Hi-Z mip 0 is already a 2x2 (3x3 odd) max.

### 3. screen_space_reflection_hiz.glsl (45 lines, all read)
- Bindings: 0 `sampler2D source`, 1 `r32f writeonly image2D dest`; push ivec2 screen_size (dest size), pad.
- main: `depth = max` over (2x,2y),(2x+1,2y),(2x,2y+1),(2x+1,2y+1) with texelFetch lod 0 (.x); plus (2,0),(2,1) in MODE_ODD_WIDTH, (0,2),(1,2) in MODE_ODD_HEIGHT, (2,2) when both (lines 25-42). Store `vec4(depth,0,0,0)`.
- Reduction operator: max of raw device depth, no min channel, no linearisation. With reversed Z that is the NEAREST depth in the texel's footprint. Hi-Z texel at level n = nearest surface depth over its footprint of source pixels; value 0 means every pixel in the footprint is at depth 0 (sky/far plane, nothing rendered; ssr.glsl:111 `should_trace = screen_pos.z != 0.0`, :241 `hit_depth == 0.0` invalid).
- Why conservative for the skip test (ssr.glsl:184-220; trace details belong to D): the cell value is the nearest depth in the cell, so a ray that is still in front of (nearer than) that value across the whole cell cannot intersect any surface in the cell and can skip to the cell edge. Facing camera (screen_ray_dir.z=+1, depth increasing): `hit = t <= depth_t` with `depth_t = (cell_depth - screen_pos.z)*dir.z` -> the ray's depth (screen_pos.z + t) is <= cell_depth, ray at/behind nearest surface at the point t -> refine (mip -1); otherwise skip (`t = edge_t`, mip +1). Away from camera (dir.z=-1): `hit = depth_t <= edge_t` -> ray's depth at the cell exit (z0 - edge_t) <= cell_depth, i.e. goes behind the nearest surface somewhere in the cell. Max is the only choice giving no missed hits (a min/farthest value would let rays skip over near geometry). Cost: over-refinement near depth discontinuities (a cell with a near occluder next to far background is "hit" for rays passing in the background; they descend until level 0 resolves it, and the level-0 test `(z0 - z1) > depth_tolerance` (linearised, ssr.glsl:200-208) rejects thick/under-occluder hits).
- Raw device depth (not linear) is stored because NDC/device depth is affine in screen space, so `screen_pos + screen_ray_dir*t` (ssr.glsl:156-157, 185) interpolates z linearly and compares directly against hi-z texels without per-sample unprojection. Linearisation (`linearize_depth`, ssr.glsl:25-29 area: inv_projection*(0,0,d,1), z/w = view-space z, negative) is only done at level 0 for the tolerance test.
- No min-max pyramid: only max. No separate near/far channels.

### 4. Mip count and level sizes
- ssr_allocate_buffers (sse.cpp:~1428-1447 region, read in full): `size = half ? internal/2 : internal`; `mipmaps = 1; while (w>1 && h>1) { w/=2 (if >1); h/=2 (if >1); ++mipmaps; }` - integer floor halving, STOPS when EITHER dimension reaches 1 (the loop test is w>1 && h>1 before halving; the inner ifs are redundant). Texture: RB_HIZ R32_SFLOAT, SAMPLING|STORAGE, layers = view_count, mips = mipmaps (RB_SSR colour has the same count; RB_MIP_LEVEL R8_UNORM 1 mip).
- Level n size = (max(1,W>>n), max(1,H>>n)) (same as the dispatch size sse.cpp ~1564-1565 and `compute_cell_count` ssr.glsl:38-42, `max(1, screen_size >> level)`).
- 1920x1080 full size: levels 1920x1080, 960x540, 480x270, 240x135, 120x67, 60x33, 30x16, 15x8, 7x4, 3x2, 1x1 -> loop ends when w hits 1 at the 10th halving (h: 1080,540,270,135,67,33,16,8,4,2,1 reached on the same step) => mipmaps = 11.
- 1920x1080 half size (960x540): 960x540, 480x270, 240x135, 120x67, 60x33, 30x16, 15x8, 7x4, 3x2, 1x1 => mipmaps = 10.
- Level 0 of height 1080 and 540 happens to become 1 simultaneously with width only at the last step; for non-square cases the loop stops when the SHORT side reaches 1, and the long side is then still >1 (e.g. 1920x1 -> mipmaps 1; 1000x10: 1000,500,250,125,62,31,15,7,3,1 for width... h:10,5,2,1 -> mipmaps 4 with level 3 = 125x1).
- Non-power-of-two: footprint of a level-n texel. Each level reads its PARENT texel (2x,2y)..(2x+1,2y+1) plus one extra column/row if the parent size is odd (applied to every texel, see section 2 note). Recursively the footprint at level n is 2^n source pixels wide plus overlap: for odd parents the footprint is 2^n+(2^(n-1)+...) grown by at most one parent texel per odd level, never smaller. Example 1080 high: level 4 (67 rows) covers rows 16k..16k+15 plus extras from the odd level 3 (135 rows) -> the footprint of the last row grows. Because of the overlap each texel's value is >= (nearer or equal) the true max over [k*2^n,(k+1)*2^n), so the pyramid stays conservative. But the trace addresses level n with `cell_index = floor(uv * (W>>n))`, i.e. cell k spans uv [k/(W>>n), (k+1)/(W>>n)) = a slightly LARGER pixel span than 2^n when W is not a multiple of 2^n (e.g. 1080 -> level 3 135 cells of 8 px exact; level 4 67 cells map to 16.12 px each, while hiz cell k covers pixels 16k..16k+15(+extras)). The mismatch is within the odd-extension overlap but it is a place the footprint and addressing are not identical; flag for D.
- Level 0 vs half size: in half mode level 0 is already a 2x2 reduction of the full-res depth; the whole trace then runs in half-res coordinates (screen_size = half), SSR Main also reads the half-size normal/roughness.

### 5. copy_depth_to_rect (copy_effects.cpp:497-532, copy.glsl)
- Used in full-size mode only. `CopyMode mode = COPY_MODE_SIMPLY_COPY_DEPTH` -> variant `#define MODE_SIMPLE_COPY_DEPTH` (copy_effects.cpp:96). flip_y false here. Push: section = (0,0,w,h), target = rect position (0,0).
- Source: bound as sampler+texture (default LINEAR sampler, irrelevant: `texelFetch`) at set 0 binding 0 (the depth texture; sampled as its depth aspect, one float in .r). Dest: `layout(r32f, set=3, binding=0) writeonly image2D dest_buffer` (copy.glsl:59-60), the RB_HIZ mip-0 slice.
- Shader (copy.glsl:242-252): `vec4 color = texelFetch(source_color, pos + section.xy, 0); imageStore(dest_buffer, pos + target, vec4(color.r));` i.e. raw depth, no linearisation, no flip; format conversion D32F/D24 -> R32_SFLOAT is the sampler's depth read (D24_UNORM -> float in [0,1]) then r32f store. Dispatched by `dispatch_threads(w,h,1)`.
- There is a different `copy_depth_to_rect_and_linearize` (copy_effects.cpp:534, MODE_LINEARIZE_DEPTH_COPY, copy.glsl:254+) that does linearise; NOT used by SSR.

### 6. Godot-specific choices vs the textbook Hi-Z
- Max-only pyramid on raw reversed-Z depth (nearest-surface pyramid); no min channel (textbook "HZB for SSR" min/farthest variants exist for other conventions; for Z-reversed the naming flips). Raw depth allows straight linear interpolation in screen space, avoiding unprojection in the loop; cost: precision is non-uniform (float depth, reversed Z is friendly) and the thickness test needs the explicit linearisation at level 0 only.
- Mip 0 is the full depth copy in full mode (extra read+write pass of W*H*4B + depth read), in half mode it is the nearest-winner downsample that ALSO carries the matching normal/roughness texel (so the 2x2 representative is consistent).
- Odd sizes handled by 4 compile-time variants adding extra taps (3 samples per axis for every texel of the odd level), instead of power-of-two padding; mip chain halves with floor and is cut when the shorter side reaches 1 (a dimension that reaches 1 early stays at 1 via max(1,...), but the loop already stopped).
- Mip 1 in full mode reads the depth buffer, not the copy (dependency avoidance). One dispatch per level (no single-pass SPD), 10-11 small dispatches per view; cost mostly the first levels (~W*H/4 and W*H/16 threads).
- Half mode throws away resolution: the half-res depth is the nearest of 4 (or up to 9) pixels, so thin background gaps close and SSR geometry is biased nearer; the mixed-roughness lookups use the winner's normal.
- Reference cited in the trace (not Hi-Z build): hacksoflife.blogspot.com parametric-t tracing (ssr.glsl:155), FidelityFX SSSR for hit validation (ssr.glsl:235-236). The Hi-Z construction has no citation in the files.

### Unclear / needs checking
- The odd-size extra taps are applied to every texel (not just the last): confirmed by reading the shader (`MODE_ODD_WIDTH` adds (2,0),(2,1) for all pixel_pos), but I did not verify the intent; the effective footprint is therefore 3 wide at all odd levels.
- Exact depth buffer format depends on device (D24 vs D32F) - not checked per device.
- Whether the "-1" after `max(1, ...)` in mip count can produce a level whose parent has size 1 on one axis (non-square, e.g. 1000x10): parent_size.height=1 is odd -> ODD_HEIGHT variant reads rows 2y..2y+2, which are out of range for y=0 (texelFetch out of bounds returns 0 for depth textures/ implementation-defined). The allocation loop stops at min side 1, so this case does not arise: HIZ mips are never computed beyond the point where a side reaches 1. Level sizes with a side 1 exist only as the last level. Not tested.
- Barrier behaviour in RenderingDevice (automatic) inferred from the API usage, not read in rendering_device_graph.cpp.
- Line numbers in sse.cpp around 1500-1606 for sub-statements (e.g. ~1514-1527, ~1570) are from a numbered read of the whole region only for the first listing; those marked "~" are approximate within +-3.


## 3. The trace (section D)

Paths: ssr.glsl = servers/rendering/renderer_rd/shaders/effects/screen_space_reflection.glsl. All ssr.glsl lines below were read with cat -n.
Other files read: effects/ss_effects.cpp (ssr_allocate_buffers :1428-1465, screen_space_reflection :1467-1660), forward_clustered/render_forward_clustered.cpp:_process_ssr :1483-1508,
core/math/projection.{h,cpp} (set_depth_correction), shaders/effects/screen_space_reflection_{hiz,downsample}.glsl.
The two cited web sources were fetched and read (see Q2, Q4).

### 0. What the trace runs on (inputs, so each value has a meaning)

- One thread per SSR pixel, 8x8 group (ssr.glsl:7, :98). Pixel grid = SSR buffer size: internal_size/2 when half_size (default true), else internal_size (ss_effects.cpp:1434). push `screen_size` = that size (:1621-1622).
- `source_hiz` (binding 1) is sampled with a NEAREST_WITH_MIPMAPS sampler (ss_effects.cpp:1501, :1643) via `texelFetch`, so sampler state is irrelevant; level `l` is fetched by integer coordinate.
  Level 0 = depth copy (full size: copy_depth_to_rect; half: "SSR Downsample" which keeps the larger of 4..9 samples: downsample.glsl:20-41). Level m>=1: `max` of the 2x2 (plus odd-size extra row/col) of level m-1 (hiz.glsl:25-44).
  Depth is reversed-Z (see 1.), so `max` = NEAREST surface in the cell. A Hi-Z texel at level l = "depth of the nearest surface found anywhere under that cell". This is the C-subtopic meaning I rely on.
  R32_SFLOAT texture (ss_effects.cpp:1458), `mipmaps` levels.
- `mipmaps` count (ss_effects.cpp:1438-1448): starts at 1, halves both dims while BOTH >1, +1 each. Half of 1920x1080 = 960x540 gives sizes 960x540, 480x270, 240x135, 120x67, 60x33, 30x16, 15x8, 7x4, 3x2, 1x1 => mipmaps = 10 (computed by hand from the loop). Pushed as `mipmaps` (:1623).
- Push constants (ssr.glsl:23-36, filled ss_effects.cpp:1621-1629): num_steps = env ssr_max_steps (default 64), depth_tolerance = env ssr_depth_tolerance (default 0.5), orthogonal = projection.is_orthogonal(), view_index = v.
- Scene UBO (:15-21; filled :1485-1497): `projection`/`inv_projection` are of `correction * view_projection`, where correction = `Projection::set_depth_correction(true)` = defaults (flip_y=true, reverse_z=true, remap_z=true) (projection.h:76).
  From projection.cpp:787-807 that matrix is y -> -y, z_clip -> -0.5*z_clip + 0.5*w. So after the divide: NDC z' = 0.5*(1 - z_ndc_GL): near plane = 1.0, far plane = 0.0 (reversed Z, range [0,1]), and screen y = 0 is the TOP row (texel row 0). `eye_offset` = per-view `view_offset.origin` (renderer_scene_render_rd.cpp:1383); zero for non-XR (inferred: only multiview/XR gives a non-zero view offset; I did not trace the XR path).
- View space is Godot's: camera at origin, looking down -Z. A visible point has view z < 0. (Evidence: the near clip at :148 treats `z > 0` as behind the camera.)

### 1. Setup helpers (:38-64)

- `compute_cell_count(level)` (:38-42): (max(1, W>>level), max(1, H>>level)) as vec2 = the pixel size of Hi-Z level `level` (matches the texture's mip dims, floor division).
- `linearize_depth(depth)` (:44-48): inv_projection * (0,0,depth,1), returns pos.z/pos.w = VIEW-SPACE z of that depth value (negative number, same units as the scene: world units). Uses xy = 0, fine because view z depends only on depth for perspective and ortho. Used only for the depth tolerance (:201-202) and E.
- `compute_view_pos(screen_pos)` (:50-57): screen_pos = (u, v in [0,1] with v=0 at top, z = reversed depth in [0,1]). xy -> *2-1 (NDC), z kept, w=1, times inv_projection, divide by w. Returns the view-space position (z<0 in front). The y flip is hidden inside the corrected matrix.
- `compute_screen_pos(pos)` (:59-64): inverse: projection * (pos,1), divide by w, xy*0.5+0.5. Returns (u, v, depth): u,v in [0,1] (outside if offscreen), depth in reversed [0,1]. Behind-camera points (w<0 for perspective) give garbage, hence the clip at :148.
- `orthogonal` only changes `view_dir` (:137). `eye_offset` also only enters :137. Everything else (matrices) handles ortho because w = 1 there.
- M_PI define (:95) is used by E only.

### 2. Geometric normal (:66-93) and the self-intersection bias (:131-135)

Source cited in the comment: https://habr.com/ru/articles/744336/ = "Как я нормали реконструировал" ("How I reconstructed normals"), Habr, 2023-06-27, author Snailwithtea (fetched). It compares 3-tap/4-tap/5-tap/9-tap normal reconstruction from depth; the 9-tap one comes from atyuwen's article (per the Habr text), and the author's variant uses signs to choose the side and saves 5 depth reads and 2 NDC->view conversions. Godot's function is that sign variant (same names H, V, he, ve, h_der, v_der). The article was written for Godot.
Why it exists here: the shading normal in `source_normal_roughness` can be normal-mapped, so it may disagree with the real surface; the code needs the true surface plane (a) to push the ray origin off the surface and (b) to detect rays that start pointing into it. It does not read a normal buffer for this; it reconstructs from Hi-Z level 0 (in half mode: from the half-res max-downsampled depth, so the "geometry" is the half-res conservative one).

How (:67-93), per axis (H shown, V is identical on y):
1. H = (L1, L2, R1, R2) = Hi-Z level-0 depth at pixel offsets (-1,0), (-2,0), (+1,0), (+2,0). `texelFetch` with no clamping: at the borders the code never guards the offset (out-of-range fetch; behaviour not handled in the code).
2. `he = abs((2*H.xz - H.yw) - depth_c)` => he.x = |2*L1 - L2 - c|, he.y = |2*R1 - R2 - c|. Each is the error of linearly extrapolating the depth from two samples on one side to the centre. (Reversed-Z NDC depth is affine in 1/z_view, which is affine in screen xy for a plane; so the extrapolation is exact on a plane, and the error is large across a depth discontinuity. This is my reasoning, not a code comment.)
3. `h_sign = he.x < he.y ? -1 : 1` picks the side with smaller error (ties go right). Same for `v_sign`.
4. `H[1 + h_sign]` = L1 (sign -1 -> index 0) or R1 (sign +1 -> index 2). The neighbour's view position is rebuilt with uv = (pixel + (sign,0) + pixel_offset)/screen_size, pixel_offset = 0.5 (the call passes 0.5, :131), i.e. the neighbour's texel centre.
5. `h_der = h_sign * (view_h - view_c)`: view-space vector toward +x pixel direction regardless of which side was used. `v_der` toward +y pixel (screen DOWN, as v=0 is top).
6. Returns `cross(v_der, h_der)` (not normalised; the caller normalises, :131). Worked orientation: v_der ~ (0,-1,0) (view down), h_der ~ (1,0,0) gives cross = (0,0,+1): points toward the camera (view looks -z). So geom_normal faces the viewer.
Arguments: depth_c = screen_pos.z (Hi-Z level 0 at the pixel, :109), view_c = `pos` (view position of the pixel centre, :113).

Use (:131-135):
`pos += geom_normal * (1.0 - pow(clamp(dot(normal, geom_normal), 0.0, 1.0), 8.0))`; then `screen_pos = compute_screen_pos(pos)` (:135).
- `normal` = decoded shading normal (:116; decode belongs to A). `pos` is in view space so the bias is in world/view units, not pixels, and independent of depth.
- Bias length = 1 - cos^8: 0 when the normal map agrees with geometry; dot 0.99 -> 0.077; dot 0.95 -> 0.337 (0.95^8 = 0.663); dot <= 0 -> 1.0 unit. Direction: along the geometric normal (toward the camera side).
- Side effect: screen_pos (uv AND depth) is recomputed from the biased point, so the trace starts from a point slightly above the surface, not from the Hi-Z texel; `screen_pos.z` therefore is no longer the Hi-Z value. All later t and depth_t values are relative to this biased start.
- Comment (:133): "to prevent self intersections".

### 3. Early outs (:100-129, :111, :327-328)

1. :100-102: thread outside `screen_size` returns, writes nothing.
2. `should_trace = screen_pos.z != 0.0` (:111), z = Hi-Z level-0 value (:109). Reversed Z: 0 = far plane/cleared depth = sky/no geometry. When false the whole block (:112-325) is skipped and the thread reaches :327-328 with the initial values: `output_color = vec4(0)`, `output_mip_level = vec4(0)` (mip_level 0 /14).
3. Roughness (:115-129): `roughness = normal_roughness.w`; if > 0.5, `1 - roughness`; then `/ (127/255)`. (Decode belongs to A: it undoes the packing of the 7-bit roughness plus a flag in the sign-like top bit.) If `roughness >= 0.7` (:125): writes color 0 and mip level 0 explicitly (:126-127) and returns. Identical outputs to the sky case; the comment says it is for performance "at the cost of subtle artifacting". Threshold is on the decoded 0..1 roughness. Nothing else is skipped: a mirror (roughness 0) and 0.69 trace identically (the roughness only matters later: mip level in E).
4. No other early out before the loop: no check on normal facing, on the ortho flag, or on being offscreen. Problems found later set `validity = 0` (E) instead of skipping, because invalid pixels still write a mip level (comment :231-232).
5. Loop-skipping states: if the start `t` already >= `t_max` the loop body never runs (see Q5) and the code continues to E with t = edge_t.

### 4. The reflected ray (:137-182)

View direction (:137): `orthogonal ? (0,0,-1) : normalize(pos + eye_offset)`. `pos` is the (biased) view-space point; with a zero eye_offset this is the unit vector from the camera (origin) to the surface = the INCIDENT direction (pointing away from the eye). Ortho: constant forward -z.
Note a possible sign inconsistency for XR only: scene.glsl:542 uses `-normalize(vertex_interp - eye_offset)` for the view vector; SSR adds the offset. Not verified (see Unclear).
`ray_dir = normalize(reflect(view_dir, normal))` (:138): GLSL reflect(I,N) = I - 2 dot(N,I) N, view space, unit length. Uses the SHADING normal (with normal map), not the geometric one.

Bounce (:141-143): `if (dot(ray_dir, geom_normal) < 0)` the reflected ray points into the real surface (possible with normal maps or grazing views: shading normal tilted away from the geometry). The ray is reflected again about the geometric normal: `ray_dir = normalize(reflect(ray_dir, geom_normal))`. Reflecting a vector with negative dot about the plane normal flips its normal component, giving dot > 0 afterwards. Comment: "immediately intersecting with itself. If so, bounce!"

Ray end (:145-150): `end_pos = pos + ray_dir` (one view-space unit along the ray; any second point on the ray would do since a 3D line projects to a 2D line in (x/w, y/w, z/w), and the rest is normalised by |dz|). Clip: `if (end_pos.z > 0.0)` (end behind the camera plane z=0) then `end_pos -= ray_dir / ray_dir.z * (end_pos.z + 0.00001)`. This moves end_pos along the ray by -(end_pos.z + 1e-5)/ray_dir.z, so the new z is exactly -0.00001. NOTE: the comment says "Clip to near plane", but the code clips to the CAMERA plane z = -1e-5 (not to the near plane distance). 1e-5 keeps the projection from dividing by zero ("so we don't go to infinity").
Because of this the end point projects to a very large reversed depth (nearer than near plane, > 1) and far-away screen xy; the ray in screen space is still a correct straight line toward it.

Screen-space ray (:152-159):
- `screen_end_pos = compute_screen_pos(end_pos)`; `screen_ray_dir = screen_end_pos - screen_pos` (u,v,depth deltas); then `/= abs(screen_ray_dir.z)`.
- Result: dz = +-1 exactly. The ray is p(t) = screen_pos + screen_ray_dir * t with t = distance travelled in REVERSED-Z NDC DEPTH units. A step of t = 1.0 spans the whole depth range [0,1]; xy changes by screen_ray_dir.xy per unit t (uv units).
- `facing_camera = screen_ray_dir.z >= 0` (:159): reversed Z so +z = toward the camera. The ray travels toward the viewer (depth value rises) vs away (depth value falls; the common case for floor->wall reflections).
- Source in the comment: https://hacksoflife.blogspot.com/2020/10/a-tip-for-hiz-ssr-parametric-t-tracing.html = "A Tip for HiZ SSR - Parametric 't' Tracing", Benjamin Supnik (Hacks of Life), 2020-10-21 (fetched). Idea from the post: parametrise the ray by t, not by the depth-buffer z; normalise so the ray has |Z| = 1 whatever its direction, so intersecting a depth plane needs no divide; t is small near the start (good float precision with reversed float Z); "lower t is always sooner" so towards/away-from-camera need no special-casing of the order; and the Hi-Z test is "does the range of t inside the search cell overlap the range of t implied by the cell's min-max Z". Godot adopts: t parametrisation, |dz| = 1, no-divide depth_t (:190). It differs in using only ONE depth per cell (nearest) and handling direction with the `facing_camera` conditional in :197/:211, not a min-max range overlap. (The post gives no formulas, so I cannot say more about equivalence.)

Edge t (:161-165): `t0 = (0 - p.xy)/dir.xy`, `t1 = (1 - p.xy)/dir.xy` (t at which each axis reaches the 0 and 1 borders), `t2 = max(t0, t1)` per axis = the positive one (exit), `t_max = min(t2.x, t2.y)` = t where the ray leaves the screen square [0,1]^2. Axis with dir.xy == 0 gives +-inf and loses to the other axis in the min. Only xy is bounded: nothing stops the ray at depth 0 or beyond 1 (it just can't hit there). If both xy components are 0 (ray along the view axis) t_max = inf and edge_t = inf (my inference): the loop condition `t < t_max` is false at once. Units of t_max: depth units, like t.
`cell_step` (:167) = (+-1, +-1) by the sign of dir.xy (zero counts as +1): direction of travel per axis in cells.

Start advance (:169-182): `cur_level = 0`, `cur_iteration = num_steps`. Block: `cell_index = floor(screen_pos.xy * screen_size)` = level-0 cell of the (biased) start point; `new_cell_index = cell_index + clamp(cell_step, 0, 1)` = the cell boundary line in the travel direction (index +1 if moving +, same index if moving -, i.e. the near edge of this cell in that direction); `new_cell_pos = new_cell_index/screen_size + cell_step*1e-6` (boundary, nudged 1e-6 uv into the next cell); `pos_t = (new_cell_pos - screen_pos.xy)/dir.xy`; `t = edge_t = min(pos_t.x, pos_t.y)`: t at which the ray leaves its own start cell. So the trace starts at the first neighbouring cell, skipping the pixel's own cell entirely ("to prevent immediate self intersection"). The first loop iteration thus evaluates the neighbour cell, at level 0.

### 5. The Hi-Z loop (:184-220)

Variables (all inside the loop at :185-198 unless noted):
| name | meaning |
|---|---|
| `cur_level` (:169) | current Hi-Z mip, int. 0 = full SSR resolution. Loop runs while >= 0. Starts 0. |
| `cur_iteration` (:170) | remaining step budget, starts at num_steps (default 64), decremented every iteration (:219) |
| `t` (:173) | ray parameter, reversed-depth units (see Q4). Starts at the exit of the start cell. Only increases. |
| `cur_screen_pos` (:185) | (u, v, depth) of the ray at t |
| `cell_count` (:187) | pixel size of level cur_level (vec2) |
| `cell_index` (:188) | integer cell (as float) holding the ray at this level: floor(uv*cell_count) |
| `cell_depth` (:189) | Hi-Z texel at (cell_index, cur_level): depth of the nearest surface in that cell |
| `depth_t` (:190) | t at which the ray's depth equals cell_depth: (cell_depth - screen_pos.z) * dir.z. Since dir.z = +-1 it needs no divide; sign flips handle direction. NOTE it is relative to the START depth screen_pos.z (not to the current point) |
| `new_cell_index`, `new_cell_pos` (:192-193) | boundary of the current cell in the travel direction (nudged 1e-6) |
| `pos_t`, `edge_t` (:194-195) | t at which the ray crosses each boundary axis; edge_t = min = t at which it leaves the current cell, just inside the next cell |
| `hit` (:197) | does the ray possibly intersect the geometry inside this cell (conservative at levels >0; final at level 0) |
| `mip_offset` (:198) | -1 if hit, +1 if not (0 when the tolerance rejects at level 0) |
| `cell_step` (:167) | travel direction per axis |
| `facing_camera` (:159) | dir.z >= 0 |
| `z0, z1` (:201-202) | view-space z of the cell's depth and of the ray (tolerance test, level 0 only) |

Hit decision (:197): `hit = facing_camera ? (t <= depth_t) : (depth_t <= edge_t)`.
Derivation (reversed Z: bigger depth = nearer; a Hi-Z texel = the NEAREST depth = max in the cell):
- Ray depth at t is d(t) = z_s + dir.z*t, z_s = screen_pos.z. depth_t = (cell_depth - z_s)*dir.z is the t where d(t) == cell_depth.
- facing_camera (dir.z=+1): d(t) = z_s + t rises (ray nears the camera). depth_t = cell_depth - z_s. `t <= depth_t` <=> d(t) <= cell_depth: at this point the ray is at or BEHIND the nearest surface in the cell. As the ray only gets nearer from here, it may cross a surface inside the cell -> candidate hit (go down). If d(t) > cell_depth the ray is already in front of everything in the cell and only moves further in front: no surface in the cell can block it -> skip the cell.
- !facing_camera (dir.z=-1): d(t) = z_s - t falls (ray recedes). depth_t = z_s - cell_depth, d(t) == cell_depth at t = depth_t, and the ray is behind the nearest surface for t >= depth_t. `depth_t <= edge_t` <=> the ray reaches the nearest-surface depth before it leaves the cell -> candidate hit. Otherwise the ray exits the cell still in front of the nearest surface -> skip.
Both are conservative: they can say "hit" when the ray only passes the cell's nearest surface in a part of the cell it doesn't cross, so a further level down is needed; they never say "miss" when a hit exists (for a properly built max-Hi-Z). For the sky texel (depth 0): facing: depth_t = -z_s < 0 -> miss; receding: depth_t = z_s (the ray reaches the far plane), which in practice is beyond t_max or edge_t, so miss. Sky never blocks (my derivation).

Level-0 tolerance (:200-208): only when `cur_level == 0`: `z0 = linearize_depth(cell_depth)`, `z1 = linearize_depth(cur_screen_pos.z)` (view z of the surface and of the ray point at the START of this iteration, i.e. the entry point t). `if (z0 - z1 > depth_tolerance)` (default 0.5 view units): view z is negative, so z0 - z1 > 0 means the SURFACE is nearer than the ray, by more than the tolerance: the ray is more than 0.5 units BEHIND the surface (thickness test: it went behind a thin occluder). Then `hit = false` and `mip_offset = 0` (stay at level 0; comment: "to prevent it from decreasing and increasing in repeat"). Effects:
- One-sided: a ray IN FRONT of the surface gives z0 - z1 < 0, never rejected.
- It overrides a would-be hit AND a would-be miss: the +1 of a plain miss at level 0 becomes 0 only when the ray is more than the tolerance behind; otherwise a level-0 miss goes up to level 1 as normal.
- Levels >= 1 have no tolerance (and no thickness: a ray found behind a coarse cell's nearest surface always descends).
- Then `t = edge_t` (miss branch) so the ray steps cell by cell at level 0 under the thin object.
Units: world/view units (same unit as the scene), not NDC.

Update (:210-219):
- hit: `if (!facing_camera) t = max(t, depth_t)` (move to the depth where the ray reaches the cell's nearest surface; if already behind it, t unchanged). Facing camera: t unchanged. Then level goes down.
- miss: `t = edge_t` (advance to the next cell at this level, nudged 1e-6 inside it). Level goes up by exactly +1.
- `cur_level = min(cur_level + mip_offset, mipmaps - 1)` (:218): capped at the top level (mipmaps-1 = 9 for 960x540), no lower clamp: a hit at level 0 gives -1 and ends the loop. `--cur_iteration` every iteration, whichever way the level moves.
- Note: the final level-0 hit leaves t at the cell-entry point (or at depth_t when receding), not at an exactly computed intersection; there is no binary-search refinement. Hit position resolution = one SSR pixel.

Exit conditions (:184), loop continues only while `cur_level >= 0 && cur_iteration > 0 && t < t_max`:
1. cur_level < 0: hit confirmed at level 0 (only way to a real hit).
2. cur_iteration == 0: step budget (default 64) exhausted: t stays wherever it is; the code does NOT mark this as a miss here.
3. t >= t_max: left the screen. E later sets validity 0 for `t >= t_max` (:241).
4. NaN/inf t (e.g. both screen_ray_dir.xy zero): comparisons false -> exits (inference).
After the loop :222 recomputes cur_screen_pos at the final t; E decides validity from t >= t_max, `hit_depth == 0`, etc. Exits 1 and 2 are not distinguished by a flag; E's delta-based `confidence` (:255-257) is what rejects an exhausted trace (E's topic).

#### Worked example (illustrative, not an engine capture; 960x540 SSR size, mipmaps = 10, num_steps = 64)
Floor pixel near the camera, reflection ray going up-screen and away toward a wall that is farther than the floor point (so !facing_camera, depth falls). Wall occupies the upper part of the screen. Level sequence the algorithm produces (per iteration: level used -> result -> next level):
1. L0, cell = neighbour of start cell (:172-182 put t at its edge): the ray is above the floor; the floor texel in this cell is farther than the ray (nearer value smaller) -> depth_t > edge_t -> miss -> t = edge_t, level 1.
2. L1 (2x2 px cell): still above floor -> miss -> L2.
3. L2 (4 px) miss -> L3. 4. L3 (8 px) miss -> L4. 5. L4 (16 px) miss -> L5 (cells doubling, so the ray crosses open floor in a handful of steps; each +1 miss widens the cell).
6. L5 (32 px): the cell now covers part of the wall; its max depth is the WALL's nearest depth (nearer than the ray's current depth could be? the ray, receding, reaches the wall depth before leaving the cell): depth_t <= edge_t -> HIT (conservative) -> t = max(t, depth_t) (jump forward to where the ray reaches that depth) -> L4.
7. L4 (16 px): part of cell is wall, wall depth reached within cell -> hit -> t = max(t, depth_t) -> L3.
8. L3 (8 px): suppose cell is floor-only after the jump (ray not yet at wall) -> miss -> t = edge_t -> L4. 9. L4: hit -> L3. 10. L3: hit -> L2. 11. L2: hit -> L1. 12. L1: hit -> L0.
13. L0: hit; tolerance: z0 - z1 = wall view z minus ray view z; ray is at (or just behind) the wall depth, within 0.5 -> not rejected -> hit stays true -> t = max(t, depth_t) -> cur_level = -1 -> loop exits. 13 of 64 iterations.
Shape to show: climb 0,1,2,3,4,5 on misses, then descend with occasional bounces back up; sequence 0,1,2,3,4,5,4,3,4,3,2,1,0,(-1). Actual numbers depend on depth values; for a demo use real Hi-Z levels.
Variant: if the ray passes behind a pole (>0.5 behind its depth) at level 0, hit is rejected, mip_offset = 0, t = edge_t, and it walks level 0 cell by cell until the pole is passed or the budget ends.

### 6. Compared with the textbook methods (what the code shows; published-method details are from my background knowledge unless said, and were not re-read)

Code facts only (Godot):
- KEEPS: hierarchical min/max pyramid traversal with level up on miss / down on hit and cell-edge stepping (the GPU Pro 5 / FidelityFX structure); the t parametrisation with |dz| = 1 from the Supnik post (cited); ray clipped before it reaches behind the camera; start advanced out of the pixel's own cell; hit validation from FidelityFX ffx_sssr.h (cited at :235-236, E's topic).
- ADDS / DIFFERS: (1) only the NEAREST depth per cell (one max-Hi-Z, R32F), not a min-max pair, hence the thickness handling is a separate one-sided linear test at level 0 only (:204), in view units. (2) Self-intersection avoidance through three things: geometric normal bias (:134), the "bounce" (:141-143) and the start advance (:172-182). (3) The tracing is in biased reversed-Z screen space, with t_max only from the screen edge; there is NO world-space max distance in the trace; distance enters only as a fade by screen-space ray length in E. (4) No refinement after the final level-0 hit; no jitter/stride; per-pixel budget is `num_steps` iterations counting both up and down moves. (5) Roughness >= 0.7 skipped; roughness otherwise only picks a blur mip afterwards (cone-tracing role of GPU Pro 5 played by a screen-space blur radius in E, not by the trace). (6) Colour comes from the previous frame, reprojected (E).
- LEAVES OUT (as seen in the code): no binary/linear refinement stage, no per-pixel noise, no max-distance parameter, no early exit on a ray that goes behind the camera apart from the clip, no depth bound on t (ray can leave depth [0,1] unnoticed).
- McGuire & Mara 2014 (background): a per-pixel 2D DDA with perspective-correct interpolation of 1/z and a thickness test, no hierarchy; Godot's near clip (:147-150) plays the role of their clip of the ray end to the near plane (but at z = -1e-5, not at the near plane). Uludag (GPU Pro 5, background): Hi-Z + cone tracing; Godot has no cone tracing in the loop. FidelityFX SSSR (background): initial advance + FFX_SSSR_AdvanceRay with floor/ceil cell offsets; Godot's :172-182 and :192-195 correspond to that role; I did not open ffx_sssr.h.

### 7. Facts a demonstration can show per iteration

Per step i (all derived from the loop, in order of the code):
- `t` before the step; `cur_screen_pos` = (u,v,depth) -> pixel (u*W, v*H), view position via compute_view_pos.
- `cur_level` and `cell_count = (W>>level, H>>level)`; `cell_index` -> the cell rectangle in pixels = [index * 2^level, (index+1)*2^level); the cell centre.
- `cell_depth` (the Hi-Z texel) and its view-space z (`z0`); the ray's depth at t (`cur_screen_pos.z`, `z1`).
- `depth_t`, `edge_t`, and the two compared quantities: facing: t vs depth_t; receding: depth_t vs edge_t -> `hit` boolean (before tolerance).
- Tolerance (level 0 only): z0 - z1 vs depth_tolerance -> the override flag.
- `mip_offset` (-1/0/+1), new `cur_level`, new `t` (t = edge_t, or max(t, depth_t), or unchanged), `cur_iteration` remaining.
- Whole-trace: screen_ray_dir, facing_camera, t_max (the screen exit point), start edge_t, the biased start point vs the original pixel, geom_normal, normal, bias length, bounce taken yes/no, ray_dir before/after the bounce, end_pos before/after the near clip.
- Exit reason: level -1 (hit) / iterations 0 / t >= t_max.

### Unclear / needs checking
- XR/multiview: SSR does `pos + eye_offset` (:137), scene.glsl:542 does `vertex - eye_offset`; space/sign not traced.
- Border fetches in compute_geometric_normal (+-2 px offsets) are unguarded; real behaviour at the screen edge not verified.
- Whether Hi-Z level cell mapping `floor(uv * (W>>level))` exactly matches the texel footprint for odd sizes (C's ODD modes add the extra row/col); not tested.
- Behaviour when screen_ray_dir.xy == 0 (inf) inferred, not run.
- The paper/FidelityFX comparisons rest on memory of the methods; only the Habr and Hacks of Life pages were fetched (WebFetch summaries, not full text).
- The worked example is a schematic, not a measured trace.


## 4. Hit validation, fades, colour, and the blur level (section E)

Files (RD = servers/rendering/renderer_rd): ssr.glsl = RD/shaders/effects/screen_space_reflection.glsl, sse.cpp = RD/effects/ss_effects.cpp,
rfc.cpp = RD/forward_clustered/render_forward_clustered.cpp. All lines read. Everything below is inside `if (should_trace)` (ssr.glsl:112) unless said.
Buffer size W x H = the SSR buffer (half of internal size by default, sse.cpp:1434). All "pixels" below are SSR-buffer pixels. The lab's reverse-Z convention is the same.

### 0. State at the start of this part
- t = final ray parameter from the trace loop; cur_screen_pos = screen_pos + screen_ray_dir*t (ssr.glsl:~222; xy in [0,1] UV, z = NDC depth).
- screen_ray_dir is normalised so |z| = 1 (ssr.glsl:~157), so t is a depth-NDC distance, NOT a length. `screen_ray_dir.xy * t` is the screen-UV displacement.
- Early out earlier (ssr.glsl:125-129): roughness >= 0.7 writes color=0, mip=0 and returns (roughness is the normalised one: `w`, folded if > 0.5, then / (127/255), ssr.glsl:117-121).
- Pixels with depth 0 (sky, should_trace false, :111) skip everything and fall to the stores at :327-328 with color=vec4(0), mip=0.

### 1. Validity terms in order (validity starts 1.0, :233; written in alpha, :287)

1. Reprojected position (:224-229). Hit point (NDC xy*2-1, z=cur_screen_pos.z, w=1) times `scene_data.reprojection[view]` (matrix built in rfc.cpp:_process_ssr ~1495-1505; A covers it), then perspective divide, xy*0.5+0.5. Result: UV in the LAST frame's image where the hit was. Used for (a) margin blend, (b) the colour fetch. Not clamped; can fall outside [0,1] (the linear sampler's clamp-to-edge applies, see quirks).
2. Trace-end / sky test (:241-243): validity = 0 if `t >= t_max` (ray left the screen; t_max = the edge distance computed at ~:160-163) OR `hit_depth == 0.0` (hit_depth = HI-Z mip 0 texel at cur_pixel_pos = ivec2(cur_screen_pos.xy*screen_size), :239; 0 is reversed-Z far = sky/nothing). Binary.
3. "Back-face" test (:245-250). Only runs if the hit is within 2 pixels of the origin: `all(lessThan(abs(screen_ray_dir.xy*t), 2.0/screen_size))`. It then reads the normal-roughness texel at the hit (`.xyz*2-1`, NOT renormalised; half-size copy when half_size, sse.cpp:1638) and sets validity=0 if `dot(ray_dir, hit_normal) >= 0` (view space reflected ray vs surface normal: ray travelling along the normal = it arrived from behind the surface). So it is not a general back-face test; it only catches immediate self-hits right next to the start pixel. Hits further away are not tested for facing at all (only the depth-confidence below).
4. Depth confidence (:252-257). cur_pos = compute_view_pos(cur_screen_pos) (point on the ray); hit_pos = compute_view_pos(xy, hit_depth) (the surface point stored in the depth buffer under that xy); delta = length(cur_pos - hit_pos) in VIEW SPACE UNITS (metres in scene units). confidence = 1 - smoothstep(0, depth_tolerance, delta); validity *= clamp(confidence^2, 0, 1). Default depth_tolerance = 0.5 (environment_storage.h:143; push constant `depth_tolerance`, sse.cpp:1627). Range [0,1]: delta 0 -> 1; 0.25 m -> smoothstep .5 -> conf .5 -> .25; >=0.5 m -> 0. Note the same tolerance also gates the trace loop at mip 0 (`z0 - z1 > depth_tolerance`, :~203, D's topic).
   - FidelityFX SSSR comparison: the comment (:250-251) cites ffx_sssr.h (GPUOpen FidelityFX-SSSR, `ffx_sssr_validate_hit`). I did NOT have that source in the checkout, so from memory of that header (unverified): SSSR (a) rejects hits outside the screen, (b) back-face test with the hit normal, `dot(ray_dir, hit_normal) > 0` -> reject, applied to every hit (Godot only within 2 px), (c) a depth-difference confidence computed from LINEARISED view-space depth difference `confidence = 1 - smoothstep(0, depth_tolerance, |depth_surface - depth_hit|)` squared, plus a screen-edge fade (`ffx_sssr_validate_hit` multiplies by a depth-buffer-thickness-based term and a distance-to-border factor). Godot's structure and the squared smoothstep match; Godot's differs in using a 3D distance and a different edge fade (margin_blend below) and distance fades. Verify against the SSSR header before claiming exact equivalence.
5. Margin blend (:259-268). reprojected_pixel_pos = reprojected UV * (W,H). margin = (W+H)*0.05 on both axes (:262): 150 px at 1920x1080, 75 px at the 960x540 half buffer (default). margin_grad = per axis the distance to the NEAREST edge of that axis (`mix(size-pos, pos, pos < size/2)`, :266). margin_blend = smoothstep(0, margin.x*margin.y, grad.x*grad.y) = smoothstep(0, margin^2, dx*dy) (:267). The test is on the PRODUCT of the two edge distances, not min: margin^2 = 22500 (full) / 5625 (half). Only near a corner both are small; at mid-screen edge (dx ~ W/2 = 480 on half buffer) it fades only when dy < 5625/480 = 11.7 px (half), i.e. very thin near flat edges. Range [0,1]. It uses the REPROJECTED (last frame) position, not the current-frame hit.
6. ray_len (:270) = length(screen_ray_dir.xy * t): screen-space length of the ray in UV units (fraction of screen width/height; unit square diagonal = 1.414). Not view space, not t. Both fades use it.
7. fade_in (:274) = curve_fade_in == 0 ? 1 : pow(clamp(ray_len,0,1), curve_fade_in). Parameter = environment ssr_fade_in, default 0.15 (environment_storage.h:141; push constant `curve_fade_in`, sse.cpp:1625). Goes 0 at ray_len 0 and rises steeply: 0.01 -> 0.50, 0.05 -> 0.64, 0.1 -> 0.71, 0.25 -> 0.81, 0.5 -> 0.90, 1.0 -> 1. Purpose: hide the contact area near the origin.
8. fade_out (:275) = distance_fade == 0 ? 1 : pow(clamp(1 - ray_len, 0, 1), distance_fade). Parameter = ssr_fade_out, default 2.0 (:142; sse.cpp:1626). 0.25 -> 0.56, 0.5 -> 0.25, 0.75 -> 0.0625, >=1 -> 0. So any hit farther than one screen-width/height-fraction of 1.0 is dead; practical reach is far less.
9. fade = fade_in*fade_out (:276); if fade > 0.999 -> fade = 1.0 (:280-282; comment: precision error must not leave a slight fade since strong specular leaks through (1-a)). fade peaks at ~0.58 for ray_len 0.05-0.1 with defaults (0.576 at .05, 0.573 at .1; never > 0.999 with defaults except when both exponents are 0). So with defaults the 0.999 snap is effectively inactive; it matters only for fade_in=fade_out=0 or tiny exponents.
10. Final (:284): validity *= fade * margin_blend. Total validity = [t<t_max && hit_depth!=0] * [not near-origin back face] * conf^2 * fade_in * fade_out * margin_blend. Max possible with defaults ~0.58 at ray_len ~0.05-0.1 (!), so SSR never reaches alpha 1 with the defaults (e.g. ray_len .5 -> 0.225 max). This is a notable point for the video.

### 2. "Validity in alpha" (comment :231-232)
- alpha = validity in [0,1]: how much of the indirect specular to replace with SSR colour. Comment: "Instead of hard rejecting samples, write sample validity to the alpha channel. This allows invalid samples to write mip levels to let valid samples have smoother roughness transitions."
- Invalid pixels (validity 0) write color = vec4(0) (the initial value :104; the colour block :286 is skipped only if validity <= 0) BUT still write a mip level (:294-328) computed from ray_len, so that neighbours at mip lookups see a smooth mip field instead of holes; colour/alpha at 0 contributes weight 0 to the roughness filter (see F).
- Premultiplied: `color = vec4(last_frame.xyz, 1.0) * validity` (:287): rgb = scene colour * validity, a = validity. Then rgb is tonemapped (/(1+luma) of the PREMULTIPLIED rgb, :291). Forward (scene.glsl:2227): `indirect_specular_light = indirect_specular_light*(1.0 - ssr.a) + ssr.rgb`, with the comment "Alpha is premultiplied" (:2226). Before that, ssr is multiplied by smoothstep(0,1, 1 - clamp((roughness-0.6)/(0.7-0.6),0,1)) (:2224) so the 0.6-0.7 roughness band fades the whole (rgb and a) to 0, softening the 0.7 hard cutoff of ssr.glsl:125.
- F (roughness filter, screen_space_reflection_filter.glsl:30-32): `get_weight(c) = mix(clamp(mip_level*0.2, 0, 1), 1.0, c.a)`; sample weight = alpha-driven at mip 1 weight floor 0.2, 0.4 at mip 2, ... 1.0 from mip 5, so invalid texels (a=0) fade into the blur as mips go up (the "pyramid" the :321-323 comment mentions). Weighted average then divided by weight sum, applied to premultiplied colour and alpha equally. (F owns details.)
- In the half-size path resolve shader (screen_space_reflection_resolve.glsl:23-56) reads mip per half texel (`texelFetch(source_mip_level,..)*14.0`, :35), `textureLod(source_color, uv, mip)` (:36) then inverts the tonemap (:39-40) and bilinear-weights 4 half-res samples with depth (2048), normal (32), roughness (16) exp weights; alpha is just carried through color.a.

### 3. Colour fetch (:286-292)
- Texture: source_last_frame (binding 0), the RB_LAST_FRAME copy (R16G16B16A16_SFLOAT, sse.cpp:393) of the previous frame's internal HDR colour after transparents (copy at rfc.cpp:2450-2453, sse.cpp:396-420, A's topic). Half-size only without SSIL: the texture is internal_size/2 (sse.cpp:367-370); with SSIL (6 mips) it is full size and mip 1 slice is bound (sse.cpp:1632-1635).
- Sampler: linear_sampler = LINEAR_WITH_MIPMAPS, repeat disabled, (sse.cpp:1500) i.e. bilinear, clamp-to-edge; `textureLod(..., reprojected_pos.xy, 0)`: always LOD 0 (the blur happens later via the SSR image's own mips). Alpha of the fetched texel is discarded (`.xyz`, alpha := 1.0 * validity).
- Skipped when validity <= 0 (:286).
- Tonemap (:289-291): `color.rgb /= 1.0 + dot(color.rgb, vec3(0.2126, 0.7152, 0.0722))`: Rec.709 luminance weights, applied to the already validity-premultiplied rgb. Comment: smoother roughness filtering across samples with varying luminance (a Reinhard-style compression: bright pixels would otherwise dominate Gaussian/mip averages, causing fireflies/halo; averaging in compressed space then expanding weights them like in the tonemapped domain). Output luminance L' = L/(1+L) is in [0,1).
- Inverse: forward scene.glsl:2218-2221 (only `if (resolve_ssr)` flag, i.e. the FULL-SIZE path; see below): `ssr.rgb /= 1.0 - dot(ssr.rgb, rec709_luminance_weights)`; resolve shader :39-40 identical. Math: c'/(1-L') = c exactly for one texel; after averaging in the filter it is the inverse of the averaged compressed colour (a harmonic-style mean, biased toward dimmer samples).
- Blow-up: NOTHING is clamped. 1 - L' = 1/(1+L). In fp16 (rgba16f) the stored channels round (spacing 2^-11 below 1), so for luminance >~ 2^12 ~ 4096 (premultiplied) L' rounds to >= 1.0 -> divide by 0 or negative -> Inf/NaN (even above ~1000 error is large: 1-L' quantised in steps of 2^-11). Averaging by the filter keeps L' <= max of inputs so it cannot push it over by itself; but the 4-tap bilinear of mip-sampled values stays < 1 too. HDR sun/emissive/specular highlights in last frame > 4096 cd-equivalent are plausible (also last-frame colour is in the scene-buffers' luminance units, not clamped on copy), so Inf/NaN can enter indirect_specular_light; the forward `ssr *= smoothstep` does not clamp. I did not test.
- Flag naming trap (rfc.cpp:768): bit (1<<3) "RESOLVE_SSR" is set when `!ssr.half_size`, i.e. when NO resolve pass ran. So in the full-size path the forward shader reads the mip texture (nearest) and inverts the tonemap; in the half-size path (default) the resolve pass already did both, the forward samples RB_FINAL (internal-size, 1 mip) at ssr_mip_level = 0 with no inverse (scene.glsl:2199-2221; rfc.cpp:3737-3763 chooses the texture, binding 36 mip texture only if !half_size).

### 4. Roughness -> cone -> blur radius -> mip (:294-324)
Condition roughness > 0.001 (:294); otherwise mip stays 0.
- roughness: the normalised value 0..1 (<0.7 here, see :117-125).
- cone_angle = min(roughness, 0.999) * PI * 0.5 (:295): a linear map of roughness to a half... angle in [0, ~pi/2); comment calls it "cone angle"; it is used as half-angle-ish: base = op_len = 2*tan(cone_angle)*cone_len, so cone_angle is the HALF-angle (full opening 2*angle at the apex; opposite side is 2*tan(half)*len). This is a heuristic, not GGX lobe width (no cited formula). 
- cone_len = ray_len (screen UV units).
- op_len = 2 tan(cone_angle) * cone_len (:297): the base of the iso triangle (width of the cone at the hit), UV.
- blur_radius (:298-312): a = op_len, h = cone_len: blur_radius = a(sqrt(a^2+4h^2) - a)/(4h). This is exactly the inscribed-circle (incircle) radius of the isosceles triangle with base a and height h (the comment: "fit to sphere inside cone, sphere ends at end of cone"): r = a h/(a + sqrt(a^2+4h^2)), algebraically identical. Closed form: with s = sin(cone_angle): blur_radius = ray_len * s/(1+s) (derived, checked numerically). Units: UV (fraction of the screen), linear in ray_len. Max as roughness -> 0.7 (angle 63 deg): 0.469*ray_len.
- Mip bias (:314-318): mip_level = clamp(log2(blur_radius * max(W,H) / 16.0 + 1.0), 0, mipmaps-1). blur_radius*max(W,H) = radius in SSR-buffer pixels; the comment says log2(screen_space_blur_radius + 1) and "+1 because mip level is logarithmic to the DIAMETER (in pixels) of the sampling region, 1 pixel when no blur (log2(1)=0)". The /16 is an undocumented fudge factor (not a diameter conversion: radius->diameter would be x2). Max(W,H) uses the SSR buffer size (960 default at 1080p), push constant screen_size (sse.cpp:1620-1621).
- mipmaps: sse.cpp:1438-1448 counts levels until width or height reaches 1: 1920x1080 -> 11; 960x540 -> 10. mip is clamped to mipmaps-1 (10 / 9) but never reaches there for roughness < 0.7.
- End-of-ray fade (:321-324): mip_level *= pow(clamp(1.25 - ray_len, 0, 1), 0.2) (applied for ALL pixels, including those with roughness<=0.001 where mip is 0, and invalid ones). Factor: ray_len<=0.25 -> 1.0; 0.5 -> 0.944; 0.8 -> 0.852; 1.0 -> 0.758; 1.2 -> 0.549; 1.25 -> 0. Comment: reduces the "pyramid-like" shape for very rough invalid pixels.

Numeric table (Python evaluation, mip value stored/used, before the /14 encoding; ray_len L in screen-UV units; includes the end-of-ray fade). Roughness values are the normalised ones (r=0.69 is just under the 0.7 cut).

1920x1080 SSR buffer (full-size SSR; 11 mips); blur radius / ray_len = s/(1+s):
| r | blur/L | L=0.02 | 0.05 | 0.1 | 0.2 | 0.4 | 0.6 | 0.8 | 1.0 |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| 0.1 | 0.1353 | 0.41 | 0.86 | 1.39 | 2.09 | 2.81 | 3.14 | 3.24 | 3.11 |
| 0.3 | 0.3122 | 0.81 | 1.52 | 2.25 | 3.09 | 3.87 | 4.18 | 4.22 | 3.99 |
| 0.5 | 0.4142 | 1.00 | 1.80 | 2.58 | 3.45 | 4.24 | 4.54 | 4.56 | 4.29 |
| 0.69 | 0.4691 | 1.09 | 1.93 | 2.73 | 3.62 | 4.41 | 4.70 | 4.71 | 4.43 |

960x540 (the default half-size SSR buffer of a 1080p view; 10 mips):
| r | L=0.02 | 0.05 | 0.1 | 0.2 | 0.4 | 0.6 | 0.8 | 1.0 |
|---|---|---|---|---|---|---|---|---|
| 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| 0.1 | 0.22 | 0.49 | 0.86 | 1.39 | 2.02 | 2.34 | 2.48 | 2.42 |
| 0.3 | 0.46 | 0.95 | 1.52 | 2.25 | 2.99 | 3.32 | 3.41 | 3.26 |
| 0.5 | 0.58 | 1.17 | 1.80 | 2.58 | 3.34 | 3.66 | 3.74 | 3.56 |
| 0.69 | 0.64 | 1.27 | 1.93 | 2.73 | 3.50 | 3.82 | 3.88 | 3.69 |
Note: the mip is not monotone in ray_len beyond ~0.7 (the 0.25-end fade pulls it down). A buffer one level down halves the radius in pixels, so half-size SSR is exactly 1 mip lower (log2(2) = 1 ... roughly, from the +1 inside the log, 0.2-0.9 lower in the table).

### 5. What is stored (:327-328)
- output_color (binding 3): `rgba16f` image (`layout(rgba16f ...)` ssr.glsl:12) = RB_SSR texture, created with p_color_format = p_render_buffers->get_base_data_format() (rfc.cpp:1492) which is R16G16B16A16_SFLOAT when force_hdr else the preferred data format (render_scene_buffers_rd.h:195; the shader hard-codes rgba16f, so a different format would mismatch). Size = SSR size, mipmaps as above, only mip 0 written here (sse.cpp:1459, 1639). rgb = premultiplied tonemapped colour, a = validity.
- output_mip_level (binding 4): `r8` image -> RB_MIP_LEVEL, DATA_FORMAT_R8_UNORM, 1 mip, SSR size (sse.cpp:1460). Stores mip_level / 14.0 (:328). 8-bit: 255 levels over [0,14] -> quantisation step 14/255 = 0.0549 mip (max error 0.027 mip); the cap 14 means up to 15 mip levels (a 16384+ px buffer). Decoded `* 14.0` in the forward shader (scene.glsl:2209, full-size path) and resolve (:35), with a NEAREST sampler (no interpolation of the mip value). Rough early-outs write 0.
- Both written for every pixel in the buffer (ssr.glsl:100-102 only skips pixels beyond screen_size).

### 6. Hard thresholds and constants (line numbers in ssr.glsl unless noted)
- :111 should_trace = depth != 0
- :118-121 roughness fold at 0.5, /(127/255)
- :125 roughness >= 0.7 -> skip (writes 0)
- :241 t >= t_max, hit_depth == 0.0 -> invalid
- :245 2.0/screen_size (2-pixel zone for back-face), :247 dot >= 0.0
- :256-257 smoothstep(0, depth_tolerance) squared; depth_tolerance 0.5 (environment_storage.h:143)
- :262 margin = (W+H)*0.05, :267 smoothstep(0, margin.x*margin.y, dx*dy)
- :274-275 clamp(grad, 0, 1), exponents 0.15 / 2.0 (environment_storage.h:141-142), exponent 0 disables the term
- :280 fade > 0.999 -> 1.0
- :290 luma weights 0.2126/0.7152/0.0722
- :294 roughness > 0.001; :295 min(roughness, 0.999) (avoid tan(pi/2)); :318 /16.0, +1.0, clamp(0, mipmaps-1); :324 1.25 and exponent 0.2; :328 /14.0
- scene.glsl:2209 *14.0, :2220 1.0 - luma, :2224 smoothstep over roughness 0.6-0.7 (fade window)

### Quirks worth a callout
- Margin blend with reprojected UV outside the screen: both axes negative (pos < 0 on both axes, or > size on both) gives a positive product, so margin_blend can be non-zero outside the corner. Only one axis out gives a negative product -> 0. (Reading of :266-267; untested.)
- Defaults cap total validity near 0.58 (fade_in*fade_out peak), so SSR never fully replaces the probe specular.
- Back-face check applies only within 2 px of the origin.
- hit_depth is read from HI-Z mip 0 at the integer pixel, no interpolation; the hit position for validation uses that depth while colour uses the exact reprojected UV.
- "RESOLVE_SSR" flag is inverted in meaning (set when no resolve pass).

### Unclear / needs checking
1. The FidelityFX SSSR comparison is from memory; no SSSR source was available locally; verify in ffx_sssr.h (`ffx_sssr_validate_hit`: screen bounds, back-face, depth_buffer_thickness confidence).
2. Inf/NaN from the luma inverse is by arithmetic; not tested in-engine. Whether anything clamps the last-frame HDR colour earlier (A/rfc.cpp:2450 copy) is not verified here.
3. Exact t_max/ray_dir lines (~157-163) cited from D's region, only skimmed; "lines ~" approximations.
4. Whether the preferred data format in practice makes RB_SSR rgba16f always (shader fixes rgba16f; if preferred format is e.g. R11G11B10 the image-format mismatch would matter; renderer_scene_render_rd.cpp:247 `_render_buffers_get_preferred_color_format`, not read).
5. Derived closed form blur = L*s/(1+s) checked numerically only through the table, algebra by hand.


## 5. The roughness filter and the half-size resolve (section F)

Files: sse.cpp = servers/rendering/renderer_rd/effects/ss_effects.cpp; shaders in RD/shaders/effects/.

### 1. Filter dispatches (sse.cpp:1658-1686)
- Label "SSR Roughness Filter" (:1658). Loops `for v in views`, `for m = 1; m < p_ssr_buffers.mipmaps` (:1663-1664): ONE dispatch per mip m>=1, each doing BOTH blur directions (horizontal then vertical inside one shader, shared memory). Not separable across dispatches.
- Source = slice mip m-1 of RB_SSR, bound as sampler2D with linear_sampler (LINEAR_WITH_MIPMAPS, repeat disabled = clamp-to-edge) (:1669,1673, sampler :1500). Dest = slice mip m of RB_SSR, storage image (:1670,1674). So each level is computed from the previous level, serially (m=1 needs mip 0 from "SSR Main"). Mip 0 is untouched by the filter.
- Push constant ScreenSpaceReflectionFilterPushConstant {int screen_size[2]; uint mip_level; int pad} (ss_effects.h:466-470). screen_size = (max(1,W>>m), max(1,H>>m)) = DEST mip size; mip_level = m (:1665-1667).
- `compute_list_dispatch_threads(size_x,size_y,1)` (:1679) = ceil(size/8) groups (rendering_device.cpp:6924-6948; local size 8x8x1, filter.glsl:7). Each dispatch is its own compute_list_begin/end (:1676,1681); no explicit barriers in this code; RenderingDevice inserts them from resource usage (not verified in the RD graph code).
- Mip count: ssr_allocate_buffers (:1430-1450): mipmaps=1, then while (w>1 && h>1) halve both (integer div) and ++ . RB_SSR has that many mips, format = render buffers' base data format (:1459, rfc.cpp:1492 passes get_base_data_format(); RGBA16F if force_hdr else preferred_data_format (render_scene_buffers_rd.h:195)).

### 2. Filter shader (screen_space_reflection_filter.glsl, 131 lines)
- Group 8x8 threads; `shared vec4 cache[16][16]` (:19), `shared vec4 temp_cache[8][16]` (:68). Shared memory = 4096 + 2048 = 6144 bytes/group.
- Tile load (:112-115): thread local (lx,ly) loads 4 texels at global+local-4+{(0,0),(1,0),(0,1),(1,1)} = group_origin + 2*local - 4 + offset -> the 16x16 tile covers group_origin-4 .. +11; the 8x8 outputs sit at tile index 4..11. Apron is 4 texels each side (only 3 are used by a 7-tap kernel). Each fetch is `textureLod(source, (pos+0.5)/params.screen_size, 0)` (:104-106). KEY: params.screen_size is the DEST size but the source is the previous mip (2x bigger), so uv lands exactly on the corner shared by four source texels: with the linear sampler each fetch is a 2x2 box average of mip m-1 (hardware bilinear). Downsample and blur are fused. Out-of-range uv clamp to the edge.
- Pass 1 (:120-121): each thread blurs horizontally 2 tile rows (local.y*2, +1) at column local.x+4, writes temp_cache[local.x][row] for 16 rows (so the vertical apron rows are computed too). Pass 2 (:130): vertical 7-tap from temp_cache at (local.x, local.y+4 +-3). Early return (:126) for out-of-size threads happens after both barriers (barrier safe). Barriers: memoryBarrierShared()+barrier() at :117-118 and :123-124.
- WEIGHTS[7] (:21-28) = 0.0713034320, 0.1315141208, 0.1898792329, 0.2146064286, 0.1898792329, 0.1315141208, 0.0713034320. Sum = 1.0 (checked numerically). Fit: a Gaussian with sigma = 2.021 texels (w3/w2, w2/w1, w1/w0 each give 2.0209-2.0210), truncated to 7 taps and renormalized (sigma 2.0 gives 0.0702,0.1311,0.1907,0.2161: close but not equal). Taps are 1 destination-texel apart.
- get_weight (:30-32): `mix(clamp(mip_level*0.2,0,1), 1.0, c.a)` = a + (1-a)*min(0.2*m,1). Per tap, multiplied by WEIGHTS[i]. So alpha-weighted: a fully valid texel (a=1) weighs 1; an invalid texel (a=0) weighs 0.2*m (m=1: 0.2; m=2: 0.4; ... m>=5: 1.0, i.e. alpha has no effect from mip 5 up). Invalid texels are NOT excluded, they are down-weighted, more weakly at coarser levels. (Trace writes vec4(0) for misses/rough pixels, screen_space_reflection.glsl:126, :104; valid texels are premultiplied by validity, alpha = validity.)
- Normalization (:56-65, :92-101): c = sum(c_i*w_i)/sum(w_i), applied to all four channels (rgb AND alpha); if sum(w)==0 -> vec4(0) (can only happen at mip>=1 when every a==0 and... never actually: weight is >=0.2 at m>=1 so w>0 always; the else is dead code for m>=1). Effect: weighted mean, rgb is premultiplied by alpha so mixing in zero-colour invalid texels at low weight biases colour/alpha toward the valid neighbours (reflection edge "bleeds" outward slightly at low mips; alpha stays higher than a plain mean). Blur is performed on the TONEMAPPED colour (trace divides by 1+luma, ssr.glsl:290-291, Reinhard-style) to avoid fireflies dominating.
- What mip m of RB_SSR holds: the alpha-weighted blur of mip m-1: 2x2 box downsample (bilinear at corner) then separable 7-tap Gaussian sigma 2.02 texels at the new level's pixel spacing (= 2^m SSR-buffer pixels). Cumulative std-dev in SSR-buffer pixels (my computation: variance_m = sum over k<=m of 4^k*2.0209^2 + 4^(k-1)/4 (bilinear 2-tap term)): m1 4.1, m2 9.1, m3 18.7, m4 37.6, m5 75.2, m6 150.5, m7 301, m8 602, m9 1204, m10 2408 (FWHM = 2.355x). Roughly sigma ~ 4.7*2^(m-1)... doubles per level. In full-res pixels: full size: same numbers; half size: double them (SSR buffer px = 2 full px).
- Which mip is used where is set by the trace: mip_level = clamp(log2(blur_radius*max(w,h)/16 + 1), 0, mipmaps-1) (ssr.glsl:318, w,h = SSR buffer size), scaled by pow(clamp(1.25-ray_len,0,1),0.2) (:324), stored in RB_MIP_LEVEL as R8_UNORM value/14 (:328; allocated :1460). Quantization 14/255=0.055 mip per step.

### 3. Resolve (half size only) (sse.cpp:1688-1722, screen_space_reflection_resolve.glsl)
- Only `if (ssr_half_size)` (:1688). One dispatch per view, dispatch_threads(internal W, internal H) (:1717) = full internal-res threads, 8x8 groups. Push constant {ivec2 screen_size = internal (full) size; ivec2 pad} (:1696-1699).
- Bindings: 0 source_depth = render-buffer FULL-res depth texture (get_depth_texture, :1701; DEVICE depth, reversed-Z, non-linear), 1 full-res normal_roughness (p_normal_roughness_slices, :1710), 2 depth_half = RB_HIZ mip 0 (:1704,1712), 3 normal_roughness_half = RB_SCOPE_SSR RB_NORMAL_ROUGHNESS (R8G8B8A8_UNORM half, :1455), 4 source_color = RB_SSR ALL mips (get_texture_slice(...,0,1,mipmaps), :1707) with linear_sampler (trilinear), 5 mip level = RB_MIP_LEVEL (nearest), 6 output = RB_FINAL (storage). All "nearest" ones are used with texelFetch.
- Half depth/normal come from "SSR Downsample": for each half texel it takes the NEAREST (max reversed-Z depth) of the 2x2 (plus extra texels for odd sizes) and copies the normal/roughness of that same winning texel (downsample.glsl:19-58). So half data is not averaged.
- Per output pixel p (resolve.glsl main :58-99): fetch full depth d, normal n=normalize(nr.xyz*2-1), roughness r: `if r>0.5 r=1-r; r/=127/255` (:68-72, decodes the sign-bit packing to 0..1).
- Bilinear footprint (:74-88): half_tex_coord=(p+0.5)*0.5; bilinear_weights=fract(that) = (0.25 for even p, 0.75 for odd p) per axis. Base texel = (p-1)/2 (integer division truncating toward zero), taps at base+(0,0),(1,0),(0,1),(1,1). Weights: tap0 bx*by, tap1 (1-bx)*by, tap2 bx*(1-by), tap3 (1-bx)(1-by). This equals correct bilinear (tap at base gets 0.25 for even p, i.e. weight of the nearer texel centre is 0.75 on tap base+1). Edge quirks: p=0 gives base 0 not -1 so texel 0 gets 0.25 and 1 gets 0.75 instead of 1.0/0; p=W-1 reads texel W/2 out of range (texelFetch OOB, normally returns 0 -> depth weight ~0). Minor.
- get_sample (:23-56) per tap: depth_half = texelFetch(HIZ mip0), normal/roughness decoded the same way; uv=(tap+0.5)/(screen_size*0.5) (:33); mip = texelFetch(RB_MIP_LEVEL, tap).x * 14.0 (:35); color = textureLod(RB_SSR, uv, mip) (:36; trilinear between two levels, bilinear within level, at texel centre of mip 0 so exact); UN-TONEMAP `rgb /= 1 - dot(rgb, (0.2126,0.7152,0.0722))` (:38-40, inverse of c/(1+L): c' = c/(1-L_c)). Done per tap, BEFORE the weighted average, so averaging is in linear HDR; alpha untouched.
- Weights (:42-55), constants DEPTH_FACTOR 2048, NORMAL_FACTOR 32, ROUGHNESS_FACTOR 16:
  - weight_depth = exp(-|d - d_half| * 2048): on DEVICE depth (reversed-Z; not linearized). With reversed-Z and (near/far-type) projection d ~ near/z, so the tolerance in view-space is distance-dependent: |dz| ~ z^2*dd/near; 1/e at dd=1/2048=4.9e-4 (e.g. near 0.05 -> ~1 cm at z=1 m, ~1 m at z=10 m; rough, assumes d = near/z).
  - weight_normal = exp(-clamp(1 - dot(n, n_half),0,1) * 32).
  - weight_roughness = exp(-|r - r_half| * 16), r on 0..1 scale.
  - total weight = product, then times the bilinear weight (:85-88).
- Combine (:90-96): result = sum(color_i*w_i)/sum(w_i) over RGBA (premultiplied alpha averaged as is); if sum(w)==0 (exactly zero, e.g. exp underflow) -> vec4(0). There is NO fallback to plain bilinear when all weights are tiny but non-zero: it renormalizes whatever tiny weights remain (only an exact 0 gives black).
- Output: imageStore RB_FINAL (:98), size = internal (full) size, same colour format as RB_SSR (rfc.cpp passes base data format), created only when half (:1462-1464), 1 mip. Colours are now linear HDR (un-tonemapped), alpha = validity, and the forward shader reads it with mip 0.

### 4. Full-size path (confirmed, with a naming trap)
- No resolve, no RB_FINAL. rfc.cpp:3739-3747 binds RB_FINAL if ssr.half_size else RB_SSR as binding 35; binding 36 (mip level) = RB_MIP_LEVEL only if !half_size else a black default (:3759).
- Flag SCREEN_SPACE_EFFECTS_FLAGS_RESOLVE_SSR (1<<3, scene_forward_clustered_inc.glsl:195) is set when ssr is NOT half size (rfc.cpp:768). Despite the name, it means "the forward shader must do the mip pick + un-tonemap itself".
- scene.glsl:2197-2228: if flag: ssr_mip_level = textureLod(ssr_mip_level_buffer, NEAREST, screen_uv, 0).x * 14 (:2203-2209); ssr = textureLod(ssr_buffer, LINEAR_WITH_MIPMAPS_CLAMP, screen_uv, ssr_mip_level) (:2213-2215); then un-tonemap `ssr.rgb /= 1 - dot(rgb, rec709)` (:2218-2221), then roughness fade `ssr *= smoothstep(0,1,1 - clamp((roughness-0.6)/0.1,0,1))` (:2224) and `indirect_specular = indirect_specular*(1-ssr.a)+ssr.rgb` (:2227). In half mode the flag is off: mip 0 of RB_FINAL, no un-tonemap, but the same roughness fade and blend follow.
- Differences in result: (a) Where the blur level is chosen: full = per full-res pixel at that pixel, trilinear in tonemapped space then un-tonemap of the mixed value; half = per half-res texel (blur radius computed with HALF width in the trace, so same angular blur uses one mip less; mips of the half buffer are 2 full px), each of 4 tap colours fetched at its own mip, un-tonemapped, then combined by the joint bilateral weights (depth/normal/roughness-aware, edges of geometry preserved) at full res. (b) Un-tonemap: full = in forward shader once on the blended sample; half = in resolve on each of 4 samples before averaging (HDR-linear averaging). (c) Full-size has no edge-aware upsampling: mip chain is plain trilinear, so reflection/blur bleeds across geometric edges, but mip level is exact per pixel. (d) Half uses depth/normal from nearest of 2x2 as proxy for half texel. (e) Cost: full traces 4x the pixels, filter chain 4x larger; half adds the downsample and resolve.

### 5. Published methods
- No comment in filter/resolve shaders names a paper. Concept is joint (cross) bilateral upsampling (Kopf et al., "Joint Bilateral Upsampling", SIGGRAPH 2007) as widely used in SSAO/SSR upsampling; Godot's version uses exp() weights over depth/normal/roughness times bilinear weights, with a low-res guide from nearest-depth downsample. I cite Kopf as the closest published method; Godot never names it (my inference). Roughness filter: it is the roughness-driven mip-chain blur of SSR (as in Stachowiak/"Stochastic SSR" 2015 style prefiltered colour), but in Godot the Gaussian is applied per mip on the SSR texture itself; no source comment names it either. Godot differences from plain JBU: exp falloffs (not Gaussian on range function with sigmas), 4 taps, depth in device space, no alpha treatment other than premultiplied averaging. The only URLs in the SSR shaders are in ssr.glsl: :66 habr.com/ru/articles/744336, :155 hacksoflife (Hi-Z parametric t tracing), :236 FidelityFX SSSR (these are trace refs, E/other subtopics).

### 6. Cost facts
- Filter shader, per output pixel of a mip: 4 textureLod fetches/thread (each hardware 2x2 bilinear = 16 source texel reads per thread... i.e. 4 API fetches per output px, over a tile 4x the output area, each fetch bilinear), 6144 B shared, 2 horizontal blurs per output pixel (each 7 shared reads + 7 get_weight) + 1 vertical (7 reads) = 21 shared vec4 reads, 3 barriers-worth (2 barrier pairs). ALU: 3x7-tap weighted sums per thread (2 horiz + 1 vert) plus 3 divides. Writes 1 rgba16f texel.
- Resolve, per full-res pixel: 2 texelFetch (depth, normal) + 4 taps x (texelFetch depth_half, normal_half, mip + 1 textureLod trilinear = up to 8 texel reads) = 14 API fetches + 4 trilinear lookups = 18 API fetches, up to ~32 hw texel reads in color; 4 exp3, 4 normalize, no shared memory.
- Dispatches at 1920x1080, 1 view, 8x8 groups:
  - Full: SSR size 1920x1080, mipmaps=11, filter = 10 dispatches, dest sizes 960x540, 480x270, 240x135, 120x67, 60x33, 30x16, 15x8, 7x4, 3x2, 1x1 -> groups 120x68, 60x34, 30x17, 15x9, 8x5, 4x2, 2x1, 1x1, 1x1, 1x1 (8160+2040+510+135+40+8+2+3 groups ~ 10.9k). No resolve.
  - Half: SSR size 960x540, mipmaps=10, filter = 9 dispatches, dest sizes 480x270, 240x135, 120x67, 60x33, 30x16, 15x8, 7x4, 3x2, 1x1 -> groups 60x34 ... (~2.7k groups). Resolve: 1 dispatch 1920x1080 threads = 240x135 = 32400 groups.
  - Multiview multiplies all by view count.
- Trace-time mip is clamped to mipmaps-1 (9 half, 10 full) then x1/14 quantized in R8.

### Unclear / needs checking
- RB_SSR / RB_FINAL format is force_hdr ? RGBA16F : preferred_data_format (render_scene_buffers_rd.h:195); the default value of preferred_data_format not checked.
- Automatic barrier insertion between consecutive compute lists (assumed from RD graph; not read).
- Resolve edge cases p=0 and p=W-1 and odd internal sizes (screen_size*0.5 is float, half buffer is floor(W/2)) - uv slightly off for odd sizes.
- Sigma 2.021 identification is my numeric fit; Godot has no comment on it.
- The distance-dependent depth tolerance assumes d ~ near/z for Godot's reversed-Z projection.
- Kopf attribution is inference; no comment in source.


## 6. Composition in the forward shader (section G)

Paths: S = RD/shaders/forward_clustered/scene_forward_clustered.glsl, INC = same dir scene_forward_clustered_inc.glsl,
rfc.cpp = RD/forward_clustered/render_forward_clustered.cpp. All lines read in the checkout.

### 1. The indirect-specular path in S (opaque pass), in order
`indirect_specular_light` is a vec3 (S:1677), starts at 0. All of it sits inside
`#if !defined(MODE_RENDER_DEPTH) && !defined(MODE_UNSHADED)` (S:1685, 1799) and `#ifndef AMBIENT_LIGHT_DISABLED` (S:1687, 1800; closes S:2230 for the SSR part, S:2281 for the BRDF part).

| # | Stage | Lines | Operation on indirect_specular_light | Branch kind |
|---|---|---|---|---|
| 1 | Sky radiance octmap | S:1690-1733 | REPLACE (assignment). ref_vec = reflect(-view,normal), bent toward normal by mix(.., normal, roughness^2) (S:1702-1703). Sample octmap at lod sqrt(roughness)*MAX_ROUGHNESS_LOD (S:1726; the array variant blends two layers by modf, S:1715-1722). Then *= IBL_exposure_normalization (S:1730), *= horizon^2 (S:1731, horizon=min(1+dot(ref_vec,normal),1) S:1709), *= ambient_light_color_energy.a (S:1732) | runtime flag `scene_data.flags & SCENE_DATA_FLAGS_USE_REFLECTION_CUBEMAP` (S:1690); octmap-array vs single is `#ifdef USE_RADIANCE_OCTMAP_ARRAY` (compile); anisotropy `#ifdef LIGHT_ANISOTROPY_USED` |
| 1b | Custom radiance (shader `RADIANCE`) | S:1735-1737 | BLEND mix(old, custom.rgb, custom.a) | `#if defined(CUSTOM_RADIANCE_USED)` compile |
| 2 | Lightmap capture / SDFGI-via-lightmap-instance | S:1800-1944 (`#ifdef USE_LIGHTMAP`: this block exists only in lightmap shader variants) | SDFGI cascade: REPLACE with specular (S:1942), or `mix(specular, old, blend)` on the last cascade (S:1926) | `#ifdef USE_LIGHTMAP` compile; per-instance flags runtime |
| 3 | VoxelGI (forward) | S:1946-1979 | voxel_gi_compute accumulates into spec_accum (up to 2 probes, S:1962-1967); after normalising, REPLACE `indirect_specular_light = spec_accum.rgb` (S:1978). Note: this discards the sky value entirely even when spec_accum.a<1 (the sky is passed into voxel_gi_compute as the in/out argument, so it is blended there) | `sc_use_forward_gi()` specialization constant + instance flag |
| 4 | GI buffers (SDFGI/VoxelGI deferred) | S:1981-2024 | BLEND `mix(indirect_specular_light, buffer_reflection.rgb, buffer_reflection.a)` (S:2024), reflection_buffer sampled at screen coord (4-neighbour normal-guided when gi_upscale_for_msaa) | `!sc_use_forward_gi()` + instance flag USE_GI_BUFFERS; the whole of 2-4 is `#ifndef USE_LIGHTMAP`-guarded (ends S:2026) except the lightmap block itself |
| 5 | SSAO read | S:2028-2036 | none on specular (ao=min(ao,ssao); light_affect) | runtime flag SSAO bit |
| 6 | Reflection probes (clustered) | S:2038-2137; reflection_process in RD/shaders/scene_forward_lights_inc.glsl:1315 | Each probe adds `reflection` premultiplied with alpha = max(0, blend - reflection_accum.a) (lights_inc:1355-1366) into reflection_accum (front-to-back, early-out when a>=1, S:2084/2088). After the loop: if a<1, `reflection_accum.rgb = indirect_specular_light*(1-a) + reflection_accum.rgb` (S:2112-2114); if a>0, REPLACE indirect_specular_light = reflection_accum.rgb (S:2122-2124). Probe ref_vec uses roughness^4 mix (S:2069), not roughness^2 as the sky does | cluster lookup runtime; clearcoat variant compile |
| 7 | ambient *= ao; specular occlusion | S:2139-2184 | MULTIPLY `indirect_specular_light *= specular_occlusion` (S:2173 bent-normal variant, S:2182 default). Default: luminance of ambient_light (after *=ao) *2, min(*4,1); boosted for smooth metals: max(min((1-roughness)*metallic*occ*10,1), occ) (S:2175-2181) | `#ifndef SPECULAR_OCCLUSION_DISABLED`, `#ifdef BENT_NORMAL_MAP_USED` compile |
| 8 | SSIL | S:2186-2195 | none on specular: `ambient_light *= 1-ssil.a; ambient_light += ssil.rgb*albedo` | runtime SSIL bit |
| 9 | SSR | S:2197-2228 | BLEND (premultiplied over): see 2 | runtime SSR bit (no #ifdef) |
| 10 | BRDF scale | S:2246-2281 | MULTIPLY by `energy_compensation * ((f90 - f0)*envBRDF.x + f0*envBRDF.y)` (S:2254-2260). Toon: `*= specular*metallic*albedo*2` (S:2250). Clearcoat: `*= cc_attenuation`, `+= cc_specular_light*F` (S:2271-2275). Note clearcoat's own cc_specular_light does not get SSR | `#if defined(DIFFUSE_TOON)`, `#ifdef LIGHT_CLEARCOAT_USED` compile |

Output: separate-specular variant writes `specular_buffer = vec4(direct_specular_light + indirect_specular_light, metallic)` (S:3075), else `frag_color = vec4(emission + ambient_light + diffuse_light + direct_specular_light + indirect_specular_light, alpha)` (S:3090). direct_specular_light is added AFTER, and never touched by SSR.

### 2. The SSR block, line by line (S:2197-2228)
- 2198: `ss_effects_flags & SCREEN_SPACE_EFFECTS_FLAGS_USE_SSR` (INC:194, `1<<2`).
- 2199: `resolve_ssr = flags & SCREEN_SPACE_EFFECTS_FLAGS_RESOLVE_SSR` (INC:195, `1<<3`).
  **Correction/important**: the CPU sets bit 3 when `!ssr.half_size` (rfc.cpp:768), i.e. in FULL-resolution SSR mode. So the name means "the forward shader must resolve (mip-select + un-tonemap) the SSR itself". In the default half-size mode (project setting half_size=true) the separate "SSR Resolve" compute pass (ss_effects.cpp:1708-1744) already did that and wrote RB_FINAL, so bit 3 is 0 and the shader does a plain mip-0 fetch.
- 2201-2210: `ssr_mip_level=0`; only if resolve_ssr: read ssr_mip_level_buffer (binding 36, R8_UNORM written by the main trace as `mip_level/14.0`, ssr.glsl:328) with NEAREST_CLAMP at screen_uv, level 0, `*14.0` (S:2209). 14 = the hardcoded max mip (ssr.glsl:328 divides by 14 too).
- 2212-2216: `textureLod(ssr_buffer, SAMPLER_LINEAR_WITH_MIPMAPS_CLAMP, screen_uv, ssr_mip_level)`. screen_uv is the fragment's screen UV; in full mode this samples the roughness-filter mip pyramid (RB_SSR, R? format = base color format, mipmaps down to 1x1, ss_effects.cpp:1453-1465) trilinearly at a fractional mip, in half mode RB_FINAL (internal size, 1 mip).
- 2218-2221: if resolve_ssr: `ssr.rgb /= 1.0 - dot(ssr.rgb, (0.2126,0.7152,0.0722))`: inverts the trace pass's tonemap `rgb /= 1 + L(rgb)` (ssr.glsl:291). The same inversion happens in the resolve pass for the half-size path (resolve.glsl:40), so exactly one inversion happens either way. Hazard: 1-L -> 0 if filtered rgb luminance approaches 1 (no clamp).
- 2223-2224: roughness fade `ssr *= smoothstep(0,1, 1 - clamp((roughness-0.6)/(0.7-0.6),0,1))` on rgb AND a. `roughness` here is the shader's final roughness (after the roughness limiter, S:1660-1672), the same variable written to the normal-roughness target (S:3019-3028), which the trace reads (7-bit-ish quantised: roughness*127/255 then 8 bits). The trace itself writes 0 for roughness >= 0.7 (ssr.glsl:123-128), so the fade removes the hard step.
  Values: r<=0.6 -> 1; r=0.65 -> t=0.5 -> 0.5; r>=0.7 -> 0 (smoothstep of the linear ramp, so zero slope at both ends).
- 2226-2227: `indirect_specular_light = indirect_specular_light*(1.0 - ssr.a) + ssr.rgb;` with the comment "Alpha is premultiplied".

Bindings (rfc.cpp:3728-3762, set RENDER_PASS_UNIFORM_SET, INC:475-480 texture2D or texture2DArray under USE_MULTIVIEW):
- binding 35 `ssr_buffer`: RB_FINAL if half_size (and it exists), else RB_SSR (rfc.cpp:3737-3748); default = black default texture (2D or 2D-array) if invalid (:3750). Black = rgb 0, a 0? NOTE: texture_rd_get_default BLACK is opaque black in many engines; I did not check its alpha. If alpha were 1 the floor would go black; since the flag is only set when buffers are allocated this path is only hit in edge cases (see Unclear).
- binding 36 `ssr_mip_level_buffer`: RB_MIP_LEVEL only when `!half_size` (:3759), else default black (never read then, since resolve_ssr is 0).
- Sampler: SAMPLER_LINEAR_WITH_MIPMAPS_CLAMP for colour, SAMPLER_NEAREST_CLAMP for mip level.

Premultiplied? Yes, traced:
- Trace writes `color = vec4(last_frame.rgb, 1.0) * validity` (ssr.glsl:288) => (v*c, v); then `color.rgb /= 1 + L(color.rgb)` (:291) => rgb = v*c/(1+v*L(c)), a = v. Invalid/rough pixels: vec4(0).
- Roughness filter (filter.glsl:20-130) Gaussian with weights `mix(clamp(mip*0.2,0,1),1,c.a)` (get_weight, :29-31) normalised by sum of weights: so transparent texels still contribute (weight 0.2*mip, capped 1) with rgb=0,a=0, which darkens/fades alpha near the reflection boundary at high mips. Works on rgba together => premultiplied stays premultiplied.
- Resolve (half): bilinear-ish 4-tap with depth/normal/roughness weights, un-tonemaps each tap (resolve.glsl:40), normalises by weights.
- After inversion (either place): rgb' = (v*c/(1+vL)) / (1 - vL/(1+vL)) = v*c exactly. So rgb = alpha*colour.
Pixel with alpha 0.5 (and roughness <=0.6): out = 0.5*indirect_before + 0.5*c where c = last-frame HDR colour at the hit point (rgb = 0.5*c is added, not 0.5*c*0.5). With roughness fade f: out = indirect*(1 - f*a) + f*rgb.

### 3. Where SSR sits relative to the BRDF
Confirmed by lines: the SSR blend (S:2227) happens before `indirect_specular_light *= energy_compensation * ((f90-f0)*envBRDF.x + f0*envBRDF.y)` (S:2260; envBRDF = prefiltered_dfg(roughness, NdotV).xy, S:2254; f90 = clamp(50*f0.g, metallic, 1), S:2259). So SSR is treated as incoming radiance exactly like the sky/probe value: Fresnel, roughness response and multi-scatter compensation are applied to it. The same scale is applied to whatever the blend produced.
- Specular occlusion: since commit 614d0e2f0d (Skyth, 2026-01-08, "Prevent SSR from getting affected by specular occlusion") the SSR block is AFTER the occlusion multiply (S:2173/2182 vs 2227). In the original overhaul (c128886c63, 2025-09-16) it was before (git show c128886c63: block inserted before "//finalize ambient light here"). So today SSR is NOT dimmed by specular occlusion and replaces the already-occluded sky/probe by alpha; where alpha=1 the occlusion is fully overridden (a hit means real geometry reflects, so that is intended).
- SSIL: only changes ambient_light (diffuse), not specular -> no direct double count. Indirect coupling: both SSIL and SSR read RB_LAST_FRAME (the previous frame's final HDR colour, which already contains previous SSR and SSIL result); so it is a temporal feedback (multi-bounce, one frame lag), not double counting within a frame. SSAO: ao affects ambient and (via luminance of ambient) specular occlusion, but never SSR (after change above).
- Double counting with sky: none, SSR replaces by alpha (1-a)*prev + premult. Sky/probe contribution is only kept in proportion (1-a). Direct specular (lights) is added afterwards and the last-frame colour already contains direct+indirect lighting of the reflected surface, which is the intended bounce.
- Exposure: the sky term is scaled by IBL_exposure_normalization (S:1730) but the SSR colour is the final internal-buffer colour from last frame (already in the scene's units); no extra normalisation is applied in the SSR block (inference from code absence).

### 4. CPU side
- `_setup_environment` (rfc.cpp:691; flags :750-772): `if debug_draw == UNSHADED: ss_effects_flags=0` (:750-751); else `if reflection_probe is null && is_environment(env)`: flags set only `if (p_opaque_render_buffers)` (:757): bit0 SSAO enabled, bit1 SSIL enabled, bit2 SSR enabled (environment_get_ssr_enabled, :762), bit3 if the RenderBufferDataForwardClustered has `!ssr.half_size` (:764-769); else 0 (:771).
- Which passes: called with p_opaque_render_buffers = false for the depth pre-pass (rfc.cpp:1908), TRUE only for the opaque colour pass (:2198), false for the transparent pass (:2415), shadows/probes/sdfgi/etc (2836...3164) use defaults/false. So only the opaque pass shader reads the SSR textures. Transparent objects get NO SSR (nor SSAO/SSIL); they use the same shader source but ss_effects_flags=0 there. (They can still see SSR indirectly: they are drawn over an opaque result that has it, and the last-frame copy happens after transparents, rfc.cpp:2450-2453, so SSR of next frame reflects transparents.)
- Note the flag uses environment_get_ssr_enabled, but trace gating (`using_ssr`, rfc.cpp:1879-1886) additionally requires !transparent_bg. With transparent_bg the flag bit is set but no SSR is computed this frame; the shader then reads whatever texture binding 35 resolves to (default black, or stale RB_FINAL/RB_SSR if it still exists from earlier frames). Edge case, not verified further.
- UNSHADED debug: flags=0 (above) and shader excludes the block via MODE_UNSHADED. But using_ssr is not cleared by UNSHADED (only using_sdfgi is, rfc.cpp:1876), so the SSR compute still runs and is just not consumed.
- VIEWPORT_DEBUG_DRAW_* enum (rendering_server_enums.h:564-592): there is NO SSR debug mode (there are SSAO, SSIL, GI_BUFFER, NORMAL_BUFFER, INTERNAL_BUFFER, ...). rfc.cpp:2585-2597 draws only SSAO/SSIL/GI buffer textures. VIEWPORT_DEBUG_DRAW_LIGHTING (rfc.cpp:367) swaps in the default material shader (so SSR applies per that material's roughness; default material roughness not checked). INTERNAL_BUFFER shows the internal colour (SSR included) without effects.
- Mobile renderer: no SSR; rfc-mobile has only a comment "not using ssr/sss (currently not supported)" (forward_mobile/render_forward_mobile.cpp:870); grep of forward_mobile shaders for ssr finds nothing. Compatibility (gles3): `environment_set_ssr_half_size` and `_roughness_quality` are empty stubs (drivers/gles3/rasterizer_scene_gles3.cpp:1139,1142); no SSR. So Forward+ only.
- Separate specular (SSS enabled or compositor effect needing it): specular_buffer already contains SSR (S:3075); merged later additively.

### 5. The dead specular_merge SSR path
- CopyEffects::merge_specular picks SPECULAR_MERGE_SSR / ADDITIVE_SSR when `p_reflection.is_valid()` (copy_effects.cpp:1454-1460), shader specular_merge.glsl `#ifdef MODE_SSR`: `frag_color.rgb = mix(specular.rgb, ssr.rgb, ssr.a)` (specular_merge.glsl:70-74; binding set1 :45-52).
- The only caller (rfc.cpp:2346) passes `RID()` as p_reflection (and at :2389 area only comments mention ssr), so MODE_SSR variants are compiled (copy_effects.cpp:303-318) but never selected. Dead in 4.7.2 (grep of merge_specular( in rfc.cpp: only :2346).
- History: before c128886c63 (Skyth, 2025-09-16, "Overhaul screen space reflections", first in 4.6-stable, `git tag --contains`), `_process_ssr` (old rfc.cpp:1600-1624) ran the trace and called `merge_specular(dest, specular_buffer, base, output, view_count)` with a real SSR texture, i.e. SSR was composited in a post-pass by mixing the separate specular buffer with the SSR. That commit moved composition into the forward shader (+39 lines in scene_forward_clustered.glsl, flags bit 2/3 in INC) and replaced the call with RID(). Then 614d0e2f0d (Skyth, 2026-01-08) moved the block after specular occlusion. The old MODE_SSR path in copy_effects/specular_merge was left behind as dead code. (The pre-2025 mix model used specular_buffer = full specular incl. direct light being mixed by SSR alpha, whereas now only the indirect part is replaced.)

### 6. What a viewer sees when SSR is toggled on a glossy floor
- Pixels changed: only opaque-pass pixels with SSR flag, roughness < 0.7 (fading from 0.6), whose traced ray found a hit (alpha>0) in the previous frame's colour buffer. Pixels whose ray left the screen, or fade_in/fade_out/margin attenuated (validity from ssr.glsl:271-289) get partial alpha; pixels that missed keep the sky/probe reflection unchanged (a=0 => exact identity).
- By how much: at alpha a the incoming radiance goes from I (sky/probe, after occlusion) to (1-a)*I + a*c, then the usual Fresnel*envBRDF scale. So at a=1 the sky/probe reflection at that pixel is fully replaced by the screen colour of whatever is reflected (objects, lit walls, last frame's own SSR and SSIL); in the Fresnel-weighted amount, F0 ~0.04 for a dielectric floor at normal view, rising toward grazing: reflections are faint head-on and strong at grazing; for metals strong everywhere. Direct specular highlights unchanged.
- Rough floors: the cone-based mip (trace shader, mip = log2(blur_radius*max(w,h)/16 + 1) clamped to mips-1, ssr.glsl:313-315, additionally *pow(clamp(1.25-ray_len,0,1),0.2) ssr.glsl:324) blurs the reflection with roughness; invalid pixels at high mips also pull alpha down through the Gaussian weights (filter.glsl:29), so edges fade.
- Roughness 0.6..0.7 band: SSR contribution (rgb and a together) is multiplied by smoothstep ramp 1 -> 0 (S:2224): at 0.6 full, 0.65 half, 0.7 zero (where the trace writes nothing anyway). Result: the reflection on a floor whose roughness is painted/textured across this band fades smoothly into the sky/probe reflection instead of a step. Below 0.6 no fade.
- At the toggle: when off, the flag bit is 0 and the sky/probe value is untouched. When turned on, floor reflections appear one frame late (they read RB_LAST_FRAME), and moving objects/camera show one frame lag due to reprojection (rfc.cpp:1483-1508).

### Unclear / needs checking
1. Alpha of the default BLACK texture bound when SSR textures are missing (texture_rd_get_default) was not read; matters only for the transparent_bg/stale-texture edge.
2. Exact RB_SSR/RB_FINAL formats: `p_color_format` = `get_base_data_format()` (rfc.cpp:1492), likely RGBA16F for HDR; not confirmed here.
3. LIGHTING debug mode: default material roughness not checked.
4. Whether `reflection_buffer` (S:2024) can ever carry a VoxelGI/SDFGI specular for the same pixel as SSR at the same time: yes by order (SSR is after), not run.
5. Line ranges for the table row 2-4 (lightmap/SDFGI) I read at S:1800-1944 and 1946-2024; the two `#endif` structure around S:2026 was inferred from `#endif // !USE_LIGHTMAP`.


## 7. Where it is used (section U)

Paths relative to the checkout; rfc.cpp = RD/forward_clustered/render_forward_clustered.cpp, sse.cpp = RD/effects/ss_effects.cpp, scene.glsl = RD/shaders/forward_clustered/scene_forward_clustered.glsl.

### 1. Where it is switched on

#### Environment resource (scene/resources/environment.{h,cpp})
| property | type | default | editor hint | setter clamp | refs |
|---|---|---|---|---|---|
| ssr_enabled | bool | false | PROPERTY_HINT_GROUP_ENABLE, group "SSR" prefix "ssr_" | none | environment.h:124; .cpp:1333-1334 |
| ssr_max_steps | int | 64 | RANGE "32,512,1" | none (hint only) | h:125; cpp:1335 |
| ssr_fade_in | float | 0.15 | EXP_EASING "positive_only" | MAX(v,0) | h:126; cpp:281-284, 1336 |
| ssr_fade_out | float | 2.0 | EXP_EASING "positive_only" | MAX(v,0) | h:127; cpp:290-293, 1337 |
| ssr_depth_tolerance | float | 0.5 | RANGE "0.01,128,0.1" | none (hint only) | h:128; cpp:299-302, 1338 |
- Every setter calls `_update_ssr()` (cpp:308-315) -> `RS::environment_set_ssr(env, enabled, max_steps, fade_in, fade_out, depth_tolerance)`; also called on resource creation (cpp:1653). So a script can set max_steps=1 or tolerance=0 and bypass the editor range; only fade_in/out are clamped in C++.
- No `_validate_property` rule touches ssr_* (cpp:1107 onwards handles sky/fog/ambient/etc. only; I read the head, grep found no "ssr" in it). Properties are shown in every renderer's inspector.

#### RenderingServer path
- Binding: rendering_server.cpp:3076 (`environment_set_ssr` args env,enable,max_steps,fade_in,fade_out,depth_tolerance), :3084 `environment_set_ssr_half_size`, :3085 `environment_set_ssr_roughness_quality`, enum consts :3132-3135 (ENV_SSR_ROUGHNESS_QUALITY_DISABLED/LOW/MEDIUM/HIGH).
- Call chain: RenderingServerDefault FUNC6 (rendering_server_default.h:858) -> RendererSceneCull PASS6 (renderer_scene_cull.h:1312) -> RendererSceneRender (renderer_scene_render.cpp:570) -> RendererEnvironmentStorage::environment_set_ssr (servers/rendering/storage/environment_storage.cpp:648-661), which just stores the five fields (struct defaults ssr_enabled=false, max_steps 64, fade_in 0.15, fade_out 2.0, tolerance 0.5: environment_storage.h:139-143). No clamping there.
- Getters used by Forward+: environment_get_ssr_enabled/max_steps/fade_in/fade_out/depth_tolerance (renderer_scene_render.cpp:574-591).
- `environment_set_ssr_roughness_quality`: Forward+ only does `WARN_PRINT_ONCE("... deprecated and no longer does anything.")` (rfc.cpp:4009-4011); Mobile, Compat, dummy: empty stubs (forward_mobile/render_forward_mobile.h:433-434; gles3/rasterizer_scene_gles3.cpp:1139-1143; dummy/rasterizer_scene_dummy.h:131-132).
- `environment_set_ssr_half_size`: Forward+ -> `ss_effects->ssr_set_half_size` (rfc.cpp:4004-4007; sse.cpp:1424-1426); stubs in the other three.

#### Docs vs. code
- Environment.xml:305-309 ssr_enabled: "Screen-space reflections are more accurate than reflections from VoxelGIs or ReflectionProbes, but are slower and can't reflect surfaces occluded by others. Note: SSR is only supported in the Forward+ rendering method, not Mobile or Compatibility. Note: SSR is not supported on viewports that have a transparent background." Matches code (see section 2), except the transparent_bg flag nuance below.
- ssr_depth_tolerance doc (Environment.xml:302-304) is one line, "The depth tolerance for screen-space reflections." Units are not documented; default in doc 0.5 matches code. (Overhaul commit c128886c63 changed the doc/default from 0.2 to 0.5.)
- ssr_fade_in doc: "Affects the area from the reflected material to the screen-space reflection"; fade_out: "from the screen-space reflection to the 'global' reflection". Clamp to 0 claim matches setters.
- ssr_max_steps: "Higher values are slower." No mention of the 32 minimum (hint range was "1,512,1" before c128886c63, changed to "32,512,1").
- RenderingServer.xml:1504-1527: environment_set_ssr "Sets the variables ... See [Environment]"; half_size "Half size is faster, but may look pixelated or cause flickering"; roughness_quality marked deprecated="This option no longer does anything."
- ProjectSettings.xml:2935-2937 half_size: "rendered at half size and then upscaled before being added to the scene. This is faster but may look pixelated or cause flickering." Default true in doc and code.
- Disagreement: BaseMaterial3D.xml:490 (grab-bag note on `stencil`/ near-camera scale property) says screen-space effects "like SSAO and SSR may break with lower scales". Decal.xml:111: decal ORM roughness does NOT affect SSR (SSR reads the depth-pre-pass normal-roughness buffer, decals are not in it). BaseMaterial3D.xml:379: specular_mode does not affect SSR.

#### Project settings
- Only one: `rendering/environment/screen_space_reflection/half_size`, `GLOBAL_DEF("...", true)` (rendering_server.cpp:3781). No type hint, no `.mobile`/`.web` override, no restart-required flag. The old `.../roughness_quality` setting was removed in c128886c63 (project_upgrade does not rename it).
- Read at SSEffects construction: sse.cpp:281 (`ssr_half_size = GLOBAL_GET(...)`). At run time nothing re-reads it; only the editor re-applies it: editor/editor_node.cpp:508 in `EditorNode::_update_from_settings` (defined :472) next to the SSAO/SSIL quality calls, so editing the project setting takes effect live in the editor only. A running game must call `RenderingServer.environment_set_ssr_half_size`.
- A change of half_size triggers re-allocation of SSR buffers next frame (sse.cpp:1428-1430: `if (p_ssr_buffers.half_size != ssr_half_size) clear_context(RB_SCOPE_SSR)`).
- Related, not SSR: `rendering/driver/depth_prepass/enable` (default true, rendering_server.cpp:3715) - see 3.
- WorldEnvironment (scene/3d/world_environment.cpp) has no SSR code: it only assigns the Environment resource. CameraAttributes: no SSR code path (grep none). Camera `environment` override and viewport/world Environment choose which Environment is read; `p_render_data->environment` is checked with `.is_valid()` (rfc.cpp:1876).

### 2. Which renderers and platforms
- Forward+ only. rfc.cpp is the sole consumer of the getters (rfc.cpp:762, 1507, 1879).
- Mobile: all SSR RS calls are stubs; `environment_set_ssr` itself is non-virtual in RendererSceneRender and stores the values, but forward_mobile never reads them (grep of forward_mobile for ssr: only the two stubs and a stale comment render_forward_mobile.cpp:870 "not using ssr/sss (currently not supported)"). No per-frame warning.
- Compatibility (GLES3): same stubs (rasterizer_scene_gles3.cpp:1139-1143); never read.
- The only user-visible warning: environment_storage.cpp:652-655, `#ifdef DEBUG_ENABLED` and rendering method != "forward_plus" and enable: `WARN_PRINT_ONCE_ED("Screen-space reflections (SSR) are only available when using the Forward+ renderer.")` (editor/debug builds only; not in export release templates).
- Transparent viewports: rfc.cpp:1879-1886: `if ssr_enabled { if (!transparent_bg) using_ssr = true; else WARN_PRINT_ONCE("Screen-space reflections are not supported in viewports with a transparent background. Disabling SSR in transparent viewport."); }`. Nuance: `_setup_environment` (rfc.cpp:762) still sets shader flag bit 2 (USE_SSR) from `environment_get_ssr_enabled`, independent of transparent_bg; the shader then samples binding 35, which falls back to the default black texture when no RB_SCOPE_SSR texture exists (rfc.cpp:3737-3750) -> alpha 0 -> no effect. If the same RenderSceneBuffers previously rendered SSR (viewport toggled transparent_bg at run time), a stale SSR texture could still be bound (RB_SCOPE_SSR not cleared) - unverified.
- Reflection probes: never. rfc.cpp:1842-1843 `using_ssil` excludes probes; `using_ssr` stays false in the `is_reflection_probe` branch (set only in the else branch :1865-1886); `_setup_environment` gives ss_effects_flags=0 for probes (rfc.cpp:756-772 `p_render_data->reflection_probe.is_null()` condition); depth_pass_mode normal-roughness only `if (!is_reflection_probe)` (:1924).
- Debug view "Unshaded": ss_effects_flags = 0 (rfc.cpp:752-753) but using_ssr stays true so the SSR passes still run (wasted).
- Orthogonal cameras: handled. `push_constant.orthogonal = p_projections[v].is_orthogonal()` (sse.cpp:1628); screen_space_reflection.glsl:137 `view_dir = params.orthogonal ? vec3(0,0,-1) : normalize(pos + eye_offset)`. Fixed by commit 56ed2cb6d1 (2024-07-10, "Fix SSR orientation issues when using orthogonal camera"). Downsample pass also gets orthogonal (sse.cpp:586).
- XR/multiview: supported. `_process_ssr` loops over `get_view_count()` building one reprojection per view (rfc.cpp:1497-1503); sse.cpp:1625-1660 loops per view with `view_index` push constant and per-view texture slices; the last-frame texture has `view_count` array layers (sse.cpp:361-399); shader uses `ViewIndex` with sampler2DArray (scene.glsl:2203-2215); `eye_offset[view]` used in view_dir (screen_space_reflection.glsl:137). Disparity between eyes is not shared; each view traces independently from its own depth.
- MSAA: works. Depth pre-pass renders to the MSAA depth + `RB_TEX_NORMAL_ROUGHNESS_MSAA`; for PASS_MODE_DEPTH_NORMAL_ROUGHNESS(_VOXEL_GI), `resolve_effects->resolve_gi(depth_msaa, normal_roughness_msaa, ..., depth_texture, normal_roughness, ...)` produces single-sample depth and normal-roughness (rfc.cpp:2145-2152); SSR reads those resolved textures (normal_roughness_views, rfc.cpp:2170-2175). The last-frame colour copy is taken after the MSAA colour resolve (rfc.cpp:2433-2442 then copy :2450-2453, sse.cpp:408-425 uses `get_internal_texture`, the resolved colour). SSR output is single-sample and is added in the MSAA opaque pass by screen_uv sampling (every sample of a pixel sees the same SSR). The resolve of normal/roughness picks a sample/filters per resolve_gi (not re-derived here).
- Scaling/FSR/TAA: SSR runs at internal (3D render) size: `internal_size` (sse.cpp:1434). See 3.
- `rendering/driver/depth_prepass/enable`: Forward+ depth_pre_pass = (force_stencil || setting) && depth_framebuffer valid (rfc.cpp:2112-2113). The normal-roughness is only written by the pre-pass. `using_ssao` requires depth_pre_pass (rfc.cpp:2118) but `using_ssr` does not, so with the pre-pass disabled SSR would run without a freshly written normal-roughness buffer. The vendor list "PowerVR,Mali,Adreno,Apple" (rendering_server.cpp:3716) is only consumed in drivers/gles3/storage/config.cpp:204 (Compatibility), so Forward+ on those GPUs keeps the pre-pass. Behaviour of SSR with the pre-pass disabled not verified (see Unclear).

### 3. What SSR shares and competes with

#### Frame order (rfc.cpp)
render_scene: gating :1879 -> normal-roughness required `depth_pass_mode = PASS_MODE_DEPTH_NORMAL_ROUGHNESS` if `using_ssr || using_sdfgi || ssao_enabled || using_ssil || ce_needs_normal_roughness || debug normal || scene_state.used_normal_texture` (:1926-1938; VoxelGI chooses _VOXEL_GI mode first :1926) -> pre-pass (:2120) with MSAA resolve (:2145) -> `_pre_opaque_render` (:2182): GI, then `allocate_last_frame_buffer(rb, use_ssil, use_ssr)` (:1619-1621), then if ssao||ssil `downsample_depth` + `_process_ssao` + `_process_ssil` (:1623-1637), then `_process_ssr` (:1639-1641) -> opaque pass -> sky -> (separate specular/SSS merge) -> transparent pass -> resolve -> "Copy Final Framebuffer (SSIL/SSR)" (:2449-2453).
- SSR forces: the depth pre-pass output with normal-roughness (R8G8B8A8_UNORM, per overview), and the RB_LAST_FRAME copy. It does NOT force separate specular: `using_ssr` appears in the condition at rfc.cpp:1929 only for the normal-roughness decision. (Line 1929 is the depth_pass_mode condition, not the separate-specular one; separate-specular conditions at :1967-1999 involve SSS/lightmaps/voxelgi.)
- SSR is composed ONLY in the opaque pass: `_setup_environment(..., p_opaque_render_buffers=true)` at :2198 sets flags; depth pre-pass (:1908) and transparent pass (:2415) pass false, so ss_effects_flags=0 (rfc.cpp:757-772). Transparent surfaces therefore get no SSR (they would only see the screen_texture).

#### Shared last-frame buffer with SSIL (sse.cpp:365-425)
- One texture RB_SCOPE_SSLF/RB_LAST_FRAME, R16G16B16A16_SFLOAT, view_count layers, cleared to (0,0,0,0) on creation. Size = internal size, except `!ssil && ssr && ssr_half_size` -> internal/2 (sse.cpp:371-373). Mip count = 6 if SSIL else 1 (:375-377). Reallocation whenever size/mips/layers change (:380-393).
- With SSIL + half-size SSR the SSR samples mip 1 of the full-size texture (sse.cpp:1632-1636). With SSR full-size, mip 0.
- Copy (sse.cpp:408-425): m==0: `copy_to_rect` if sizes match, else `make_mipmap` (downsample) from `get_internal_texture(v)`; higher mips are built from the previous slice with make_mipmap. Content = the final HDR scene colour of the previous frame: opaque + sky + transparent, scene-referred, before tonemapping/glow/TAA. Because fog (analytic and volumetric) and sky are computed inside the scene fragment shaders (scene.glsl:1103-1160 fog; ubo volumetric_fog_enabled rfc.cpp:731-737), the last-frame colour already contains fog on surfaces and the sky; last frame's SSR reflections are in it too, giving multi-bounce reflections-of-reflections. Commit d27e4dab16 (2025-11-13) sanitises INF/NaN in this copy.
- The reprojection matrix in `_process_ssr` (rfc.cpp:1497-1505): `last_proj * inverse(last_cam_transform) * cam_transform * inverse(cur_proj)`, with `Projection::set_depth_correction(true)` applied; it handles camera motion only - no motion vectors, so moving objects reflect their previous-frame colour at the current-frame position (ghost trails). Separate last_frame_projections/transform from SSIL's (`ssil_last_frame_*` rfc.cpp:1474-1480) so enabling both does not interfere.
- Since the copy runs when `using_ssil || using_ssr`, either effect pays for it once; the buffer is shared.

#### SSAO / SSIL order and interaction
- SSAO, SSIL then SSR run in that order (rfc.cpp:1623-1641); SSR does not read AO/SSIL outputs. In the scene shader the AO/specular-occlusion are applied BEFORE SSR is blended: `indirect_specular_light *= specular_occlusion` (scene.glsl:2173/2182), then SSR blend at :2227, so SSR is not darkened by AO-derived specular occlusion, while the sky/probe fallback is.
- SSIL's ambient is added before SSR (scene.glsl:2187-2195).

#### Fallback on miss, via alpha (scene.glsl:2197-2228)
- `indirect_specular_light` has already been assembled from: sky radiance (:1722-1732), lightmap/probes (the "reflection" accumulation, :2113-2123, with `indirect_specular_light = reflection_accum.rgb` after mixing with the sky at 2113), VoxelGI (`voxel_gi_compute` :1962-1978, result replaces it), SDFGI (the "reflection_buffer" mix :2024, `mix(indirect_specular_light, buffer_reflection.rgb, buffer_reflection.a)`). Then SSR: `indirect_specular_light = indirect_specular_light * (1.0 - ssr.a) + ssr.rgb;` (:2227, "Alpha is premultiplied"). So a miss (alpha 0) leaves the previous GI/probe/sky specular; a hit replaces it; partial alpha (edge/distance fade, fade_out) blends. Before that: `ssr *= smoothstep(0,1, 1 - clamp((roughness-0.6)/(0.7-0.6),0,1))` (:2224) so reflections vanish by roughness 0.7 (the trace pass also early-outs at roughness>=0.7, screen_space_reflection.glsl:125).
- After SSR the combined radiance is multiplied by the BRDF (`specular*metallic*albedo*2` or `energy_compensation*((f90-f0)*envBRDF.x + f0*envBRDF.y)`, :2250-2260), so SSR is treated as IBL-like radiance, not as a mirror.
- With resolve (half-size) the shader reads `ssr_mip_level_buffer` and `ssr.rgb /= 1 - dot(ssr.rgb, rec709)` (:2199-2221): that is the resolve-side format, covered by the composition agent. Note `ssr_mip_level` is only sampled when `resolve_ssr` flag bit 3 is set, which rfc.cpp:768 sets when `!ssr.half_size` (bit named RESOLVE_SSR but = full size path). **Check naming**: scene_forward_clustered_inc.glsl:195 `RESOLVE_SSR (1<<3)`, rfc.cpp:768 sets it for `!half_size`; so in full-size mode the shader uses the per-pixel mip level texture + luminance un-premultiply, and in half-size mode it samples the already-resolved RB_FINAL texture at mip 0. (Verify with the composition agent; the SSR pass agent owns this.)
- Reflection probes never contain SSR (see 2), so a probe cubemap seen on a surface where SSR hit is replaced; at a miss the probe shows. Docs of ReflectionProbe.xml:8 say probes "can be combined with ... screen-space reflections".
- GI interplay: SDFGI/VoxelGI reflection is the fallback; the lab should note SDFGI normal-roughness is the same buffer (VoxelGI uses a second attachment).

#### TAA / FSR / temporal noise
- SSR has no jitter, no blue noise, no temporal accumulation of its own: grep of screen_space_reflection*.glsl for jitter/noise/random/taa/temporal finds nothing; the only temporal element is reading last frame's colour. The result is deterministic given depth/normal.
- Projections passed to SSR are the camera's un-jittered `view_projection[v]` (rfc.cpp:1633-1640 uses `scene_data->view_projection` straight; jitter is added only in `RenderSceneDataRD::get_view_projection` / get_cam_projection (render_scene_data_rd.cpp:44-66) and the UBO (:114)). The depth/normal buffers were rasterised with the jittered projection, so under TAA/FSR2 the SSR unprojection has a sub-pixel (<=0.5 px of internal size) mismatch that changes each frame; the TAA resolve later smooths it. (Inference from code reading; not tested.)
- The last-frame buffer is pre-TAA, internal resolution, so reflected content is the (jittered, non-reconstructed) previous frame. With FSR2/upscaling the SSR runs at internal size, not target size.

#### Volumetric fog
- Fog is in the scene colour (see above) so reflected last-frame surfaces carry their own fog. SSR itself ignores fog and does not apply fog to the reflection (no fog code in SSR shaders). The hit's colour is a surface radiance that already contains fog at the hit's depth from camera, not along the reflection ray.

### 4. Godot's own samples/tests/demos
- tests/: no test mentions SSR or Environment (grep for ssr/Environment in tests/scene, tests/servers: no results; no test_environment*.cpp). Nothing in compatibility_test.
- Editor templates / default scenes: no ssr_* default; the editor's preview environment does not enable SSR (grep editor/ finds ssr only in editor_node.cpp:508 and the 3-to-4 rename table editor/project_upgrade/renames_map_3_to_4.cpp:1156-1160: ss_reflections_* -> ssr_*).
- Docs: Environment.xml tutorials links (lines 14-19) are "Environment and post-processing" and "High dynamic range lighting" ($DOCS_URL/tutorials/3d/...), external; docs repo not in the checkout, SSR-specific tutorial: none.
- Class docs limitations (verbatim ideas): can't reflect occluded/off-screen surfaces; Forward+ only; no transparent_bg viewports; half_size "may look pixelated or cause flickering"; Decal ORM roughness not applied (Decal.xml:111); BaseMaterial3D `specular_mode` doesn't affect it; near-camera-scale ("grow") scale <1 breaks screen-space effects (BaseMaterial3D.xml:490).
- Behavioural limitations visible in code: no SSR on transparent surfaces; roughness >= 0.7 never traced; no motion vectors in reprojection; probes never see SSR.

### 5. History (git log, read-only; 84,229 commits in checkout)
- 87c658b304 2020-04-01 "Re-Added screen space reflection." (Vulkan renderer, original `screen_space_reflection.glsl`).
- 2787ad65be 2020-12-04 RenderingServer reorganization (file move).
- eefcb5ed67 2022-06-28 "Move screen space effects into a separate class" (SSEffects); 02b0c22e38 2022-09-29 moved SSEffects settings into the class.
- 2022 tweaks: d38671827b (2022-02-28) fade towards inner margin; e1bcadd12d/7745bd42a6 (merged 2022-12-15, commit 2022-01-15) max roughness cutoff; e6f8c8517c (2022-12-09) fade with roughness; d09ad714bf (2022-10-02) artifacts; 65f0113bc6 metallic scaling for dielectrics.
- 2023: 5b2e54c17b (2023-07-26) Fresnel darkens SSR instead of blending with specular; 09b92a144d (2023-10-11) merge PR #79921 "ssr_transparency"; 43cf21cb71 (2023-12-18) best-fit normals for the normal buffer.
- 2024: d950f5f838 (2024-02-14) reverse Z; 56ed2cb6d1 (2024-07-10) ortho fix; b99d0d778a (2024-08-12) transformed roughness for roughness fade.
- **c128886c63 2025-09-16 "Overhaul screen space reflections." (author Skyth)**: the rewrite. Replaced screen_space_reflection_scale.glsl + _inc.glsl with _hiz, _downsample, _resolve; rewrote trace + filter; added `half_size` project setting and `environment_set_ssr_half_size`; deprecated `roughness_quality` (setting removed); depth_tolerance default 0.2 -> 0.5; max_steps hint 1..512 -> 32..512; added the ss_effects_flags bits 2/3 and the SSR block in scene.glsl.
- 703b3f3dc2 2025-12-16 padding in push constants; d27e4dab16 2025-11-13 INF/NaN sanitise in last-frame copy; fe8709d077 2026-02-05 "Fix inconsistent SSR blurriness on rough surfaces."; f5a290ac46 2026-02-17 enums to RSE namespace.

### Unclear / needs checking
1. SSR with `rendering/driver/depth_prepass/enable=false`: `using_ssr` still true, normal-roughness texture is only written by the pre-pass, `normal_roughness_views` filled only if `rb_data->has_normal_roughness()` (rfc.cpp:2170-2175). Not traced what the SSR trace then reads (empty/never-written normals).
2. Stale SSR texture bound after toggling transparent_bg at run time (rfc.cpp:3737-3750, RB_SCOPE_SSR not cleared when not used).
3. Bit 3 name RESOLVE_SSR vs half_size logic (rfc.cpp:768 vs scene.glsl:2199-2221): above reading is mine; confirm against composition agent.
4. The TAA jitter mismatch is inferred (unjittered view_projection vs jittered depth); no runtime test.
5. Mobile stale comment (render_forward_mobile.cpp:870) read only in the one-line context.
6. I did not read the resolve_gi shader to say how MSAA normal-roughness samples are combined.
7. In the "Frame order" section I wrote that the normal-roughness format is R8G8B8A8_UNORM from the overview; not re-verified here.


## 8. How it is kept fast (section O)

Abbreviations as in the BRIEF. SSE = sse.cpp = servers/rendering/renderer_rd/effects/ss_effects.cpp. SH = RD/shaders/effects/. All line numbers read in the files.

### 1. Resolution (half_size)

Setting: `rendering/environment/screen_space_reflection/half_size`, default true (servers/rendering/rendering_server.cpp:3781; doc ProjectSettings.xml:2935-2936: "faster but may look pixelated or cause flickering"). Read at SSE:281 and pushed via `environment_set_ssr_half_size` (rfc.cpp:4004-4006 -> SSE:1424 `ssr_set_half_size`). Editor re-pushes it (editor/editor_node.cpp:508).

Buffer sizes: `ssr_allocate_buffers` SSE:1434 `size = half ? internal_size/2 : internal_size` (integer division; 1920x1080 -> 960x540). Changing the setting clears the whole SSR context (SSE:1429-1431).

At HALF size (all sized `p_ssr_buffers.size`):
- "SSR Downsample" (SSE:1503-1546, SH downsample): 2x2 (or 3x3 partial for odd sizes, modes at SSE:285-289, 1514-1528; downsample.glsl:43-55) max-depth reduction. `if (depth < sample_depth)` (downsample.glsl:23) keeps the greatest value = NEAREST surface in reversed Z. The normal+roughness texel is copied from the winning sample's position (downsample.glsl:58), so depth and normal come from the same full-res pixel.
- Hi-Z chain, SSR trace ("SSR Main"), roughness filter: all at half size.
At FULL size (internal_size), only:
- "SSR Resolve" (SSE:1688-1725, dispatch SSE:1722 over `internal_size`), the bilateral upsample into RB_FINAL (RGBA16F, internal_size, SSE:1462-1464, format = `get_base_data_format()` passed at rfc.cpp:1492; that is R16G16B16A16_SFLOAT, renderer_scene_render_rd.cpp:1160-1162). The opaque shader then reads RB_FINAL (rfc.cpp:3739-3742).
Pixel counts @1080p: trace 518,400 threads (half) vs 2,073,600 (full) = 4x fewer ray-marches; the 4x saving is the main cost lever, since the trace is the only variable-cost pass.

Full-size mode (`half_size=false`): no Downsample; instead "SSR Copy Depth" (`copy_depth_to_rect`, SSE:1549-1557) into Hi-Z mip 0; the HI-Z mip 1 reads the engine depth texture directly "to not create a dependency on the previous depth copy pass" (SSE:1570-1572); the trace reads the engine's RB_TEX_NORMAL_ROUGHNESS directly (SSE:1638); no Resolve (guard SSE:1688) and the opaque shader samples the SSR texture itself with `textureLod(..., screen_uv, ssr_mip_level)` where the mip level comes from RB_MIP_LEVEL (rfc.cpp:768 flag bit 3, rfc.cpp:3759; scene.glsl:2199-2215). Half-size-only allocations: RB_NORMAL_ROUGHNESS copy (SSE:1454-1456), RB_FINAL (SSE:1462).

Bilateral upsample (resolve.glsl) cost vs quality:
- Per full-res pixel: 4 taps, each = 1 depth texelFetch + 1 normal/roughness texelFetch (half res) + 1 mip-level texelFetch + 1 `textureLod(source_color, uv, mip_level)` with LINEAR_WITH_MIPMAPS sampler (trilinear => up to 8 texels) (resolve.glsl:258-291, 315-318), i.e. ~16 fetches plus 3 `exp()` per tap (12 exps) (resolve.glsl:282,285,288). Plus 2 full-res fetches (resolve.glsl:300-301). Full-res output RGBA16F write (resolve.glsl:333).
- Weights: depth `exp(-|dz|*2048)` (DEPTH_FACTOR, :277,:282) on raw (reversed-Z, non-linear) depth; normal `exp(-clamp(1-dot,0,1)*32)` (:278,:285); roughness `exp(-|dr|*16)` (:279,:288). Multiplied by the bilinear weights from `fract((pixel+0.5)*0.5)` (:309-310, :320-323). If the total weight is 0 the result is vec4(0) (:327-331) => no reflection (hole) instead of a fallback.
- Stencil: `((pixel_pos - 1)/2) + (0|1)` (:315-318): 2x2 half-res neighbourhood centred on the full-res pixel (fix for half-texel offset).
- Colour is tone-compressed in the trace (`c/(1+L)`, ssr.glsl:291) and re-expanded `c/(1-L)` per tap BEFORE weighting (resolve.glsl:275), also again in scene.glsl:2218-2221 only if `!resolve_ssr`... NOTE see Findings F1.
- Quality cost: reflections of half-res depth; thin features (rails, wires) lost since the half-res depth/normal is the max-depth representative of 2x2 (downsample.glsl:23); edge pixels take colour from neighbours chosen by depth/normal/roughness similarity only; in doc words "pixelated or flickering" (ProjectSettings.xml:2936). The trace itself uses the half-res depth and normal (ssr.glsl:109, 115) and computes `geom_normal` with a +-2 half-res-pixel stencil (ssr.glsl:68-78), so geometric detail below 2 px is gone.

Last-frame buffer size (`allocate_last_frame_buffer`, SSE:366-398; called each frame when SSIL or SSR, rfc.cpp:1619-1621):
- `last_frame_size = internal_size`, halved ONLY if `!use_ssil && use_ssr && ssr_half_size` (SSE:371-373). mipmaps = 1, or 6 when SSIL (SSE:375-377). Format R16G16B16A16_SFLOAT (SSE:395), cleared to 0 on (re)creation (SSE:396).
- Copy (`copy_internal_texture_to_last_frame`, SSE:400-424 region; code at SSE:401-420): m=0 copies the internal colour; if sizes equal -> `copy_to_rect`, else `make_mipmap` (SSE:414-418). make_mipmap = 4 `texelFetch` average, with inf/nan replaced by 100 (copy.glsl:84-91; compute dispatch copy_effects.cpp:952-984), so the SSR-only half-size copy is a single 2x2 box downsample, no extra pass.
- SSIL effect: with SSIL on, last frame is full-size with 6 mips; SSR at half size then samples mip 1 (SSE:1630-1635: `texture_size(last_frame) != ssr size` -> slice mip 1). Cost: the full-res copy + 5 further mip passes (SSE:402-419, loop `m < mipmaps`) are paid anyway; SSR only reuses mip 1 (a box 2x2 of a 2x2 of ... no: mip 1 = box of mip 0). If SSIL on and SSR full size: mip 0 used. (If SSIL is on and half_size=false the size check SSE:1632 is false so slice 0.)
- Memory @1080p: half last-frame 960x540 RGBA16F = 4.15 MB vs 16.6 MB (full) (computed: w*h*8 bytes).

### 2. Skips and culling (ssr.glsl, all cites in that file)

- Sky/far: `screen_pos.z = texelFetch(source_hiz,pixel,0).x` (:109); `should_trace = z != 0.0` (:111). Reversed Z: 0 = far plane / sky. Not traced => falls to :327-328 stores `color=0, mip=0`. At half size a half pixel counts as sky only if ALL covering full-res pixels are sky (max-depth downsample, downsample.glsl:23); sky/edge pixels use the nearest geometry's depth/normal.
- Roughness >= 0.7: decoded `if (w>0.5) w = 1-w; w /= 127/255` (:117-121; encoding scene.glsl:3019-3026, the high bit of w flags dynamic instances; roughness has only 127 levels); `if (roughness >= 0.7)` writes 0 to colour and mip and `return`s (:125-129) BEFORE the geometric-normal stencil (8 fetches, :131) and the trace. Comment (:123-124): "to improve performance at the cost of subtle artifacting". The cost: a hard cut hidden by a fade in the opaque shader `ssr *= smoothstep(0,1, 1 - clamp((roughness-0.6)/(0.7-0.6),0,1))` (scene.glsl:2224) - fully faded by 0.7, starts 0.6. So the SSR contribution of 0.6..0.7 is attenuated even though traced. Roughness filter for these pixels is also moot: written mip 0 and alpha 0.
- Rays leaving the screen: `t_max` = nearest xy exit of the [0,1] screen square along the Z-normalised ray (:161-165); loop condition `t < t_max` (:184). After the loop `t >= t_max` -> validity 0 (:241). Thus rays heading off screen terminate in the minimum of steps, and are never validated; all per-ray cost for them is bounded by distance to the edge, not by steps.
- Near-plane clip: `if (end_pos.z > 0.0) end_pos -= ray_dir/ray_dir.z*(end_pos.z+0.00001)` (:147-150): view space looks along -z so z>0 is behind the camera; clamps the ray end onto the near side so the projective divide stays finite (correctness, not a cost saving; it also bounds the screen-space ray length). Not an early-out.
- Immediate self-intersection: a) bias of pos along the geometric normal, `pos += geom_normal * (1 - pow(clamp(dot(n,gn),0,1), 8))` (:134), b) rays heading into the surface are reflected again about the geometric normal, `if dot(ray_dir, geom_normal) < 0` (:141-143), c) the start is advanced to the next cell edge (:172-182), so level 0 never samples the start cell. This saves failed first iterations.
- Back-facing hits: only tested if the ray moved less than 2 half-res pixels in x and y (`all(lessThan(abs(screen_ray_dir.xy*t), 2.0/screen_size))`, :245) and then `dot(ray_dir, hit_normal) >= 0` => validity 0 (:246-248). It is a cheap 1-fetch test rather than a general back-face rejection; for farther hits, the depth-thickness test (z0-z1 > depth_tolerance, :200-208) and the confidence (:255-257) handle it. `hit_normal` is not octa-decoded (no normalisation, :246) - sign only is needed.
- Loop early exits (:184): `cur_level >= 0` (hit resolved at level 0 gives level -1 => exit with success), `cur_iteration > 0` (num_steps cap), `t < t_max`.
- Thickness at level 0 (:200-208): if the Hi-Z cell depth is more than `depth_tolerance` (view-space units, default 0.5) in front of the ray (z0 - z1 > tol), the hit is rejected, `mip_offset=0`, and the ray steps to the cell edge (:214-217); this stops the ray from stopping behind thin foreground objects at the cost of rays "passing through" thick walls only if tol is exceeded.
- No tile/indirect culling: every pass dispatches over the whole target with `compute_list_dispatch_threads` (SSE:1522,1597,1651,1681,1722), 8x8 groups (local_size at ssr.glsl:7 etc); early returns are per-thread so divergence inside 8x8 groups remains (my inference: a group that mixes a rough and a smooth pixel still occupies the SM until the slowest finishes).

### 3. Hi-Z acceleration, steps and cap

- Hi-Z texture: R32_SFLOAT, mips = `while (w>1 && h>1)` count (SSE:1438-1448) (halving w and h separately; stops when either hits 1). 1080p half 960x540 -> 10 mips (down to 1x1 via 3x2 -> 1x1) ; full 1920x1080 -> 11 mips (computed from the loop). Mip m>0: max of 2x2 (3x3 odd, hiz.glsl:84-101): MAX of reversed Z = nearest depth in the cell. This is the conservative bound for "is anything in this cell in front of my ray".
- `cur_level = min(cur_level + mip_offset, mipmaps-1)` (:218): miss => +1 (ray skips cell, coarser next), hit => -1 (refine). Starts at level 0 (:169). So empty space doubles the cell each step (classic hierarchical traversal), a linear march would need one step per texel crossed.
- Steps: `num_steps` default 64 (Environment ssr_max_steps = 64: scene/resources/environment.h:125; doc Environment.xml:316-318 "Higher values are slower"; range 32..512 step 1: `PROPERTY_HINT_RANGE, "32,512,1"` at scene/resources/environment.cpp:1335). Decremented once per loop iteration (:219), even for the level-hold case (:204-206).
- Reference scale (derived, not in code): at half 960x540 a full diagonal is ~1100 half-px; a linear march at 1 px/step would need up to ~1100 steps; Hi-Z with 64 iterations covers that if there are about 6 levels of up/down per cell sequence. Complex depth (many occluders) uses more steps. I did not measure iteration histograms.
- What the cap costs: loop ends with `cur_iteration == 0`, `t` wherever it reached; no flag is set. Validation then runs on the position reached: `hit_depth` at level 0 (:240); `delta = length(cur_pos - hit_pos)` view-space distance between the ray point and the depth buffer's surface at that pixel (:252-255); `confidence = 1 - smoothstep(0, depth_tolerance, delta)`; `validity *= confidence^2` (:256-257). A ray stranded in mid-air, far from any surface, gets delta > tol => validity 0 => alpha 0 => falls back to env/probe reflection (scene.glsl:2227 `indirect_specular*(1-a)+rgb`). A ray stranded close to a surface gets partial/false hits. So the miss is "soft by tolerance" not flagged; the missed far hits are also faded by distance: `fade_out = pow(clamp(1-ray_len,0,1), distance_fade=2.0)` where `ray_len = length(screen_ray_dir.xy * t)` (screen-uv distance travelled) (:270-276), `fade_in = pow(ray_len, curve_fade_in=0.15)` (:274). Long rays (the ones that need more steps) are the faintest, so the cap errors land where the contribution is already small. The shortfall is that rays hitting after exactly the cap are cut abruptly.
- Margin: `margin_blend` smoothstep on the reprojected last-frame pixel distance to the screen centre-edge, margin = 5% of (w+h) (:259-268, :284) - hides lookups near the border of the last frame image.

### 4. Reuse over time

- No second shading pass: the trace samples last frame's internal HDR colour `textureLod(source_last_frame, reprojected_pos.xy, 0)` (ssr.glsl:287), i.e. the colour of the scene as it was when the previous frame finished transparents (copy at rfc.cpp:2449-2453 inside `"Copy Framebuffer for SSIL/SSR"`, executing `_copy_framebuffer_to_ss_effects` rfc.cpp:1510-1515). That colour is the final internal buffer BEFORE tonemap/post and BEFORE TAA/upscaling (TAA label at rfc.cpp:2538-2540 is later than the copy at 2449-2453; the last-frame buffer is therefore the jittered, un-resolved image). It already contains the previous frame's SSR (reflection-of-reflection feedback; my inference from ordering: the opaque pass applies SSR, scene.glsl:2197-2227, before the copy) and transparents (the copy is after "Render 3D Transparent Pass", rfc.cpp:2411-2431).
- Reprojection: `reprojection = last_proj * inverse(last_transform) * transform * inverse(proj)` (rfc.cpp:1494-1505) maps the hit point (x,y,depth) of THIS frame into last-frame clip space; the shader applies it at ssr.glsl:224-229. The hit's depth is from this frame's depth, so it assumes the hit surface was static. Costs: one frame of latency in reflected lighting; moving objects reflected are reprojected as if static (ghost/smear); disocclusion: surface newly visible this frame is not in last frame => reads whatever was at that screen position (can show wrong colour; the margin blend and validity do not detect this - there is no depth comparison against last frame's depth; there is no stored last-frame depth at all: only colour RB_LAST_FRAME is kept). The first frame after (re)creation reads zeros (SSE:396).
- No history of SSR results: `RB_SSR`, `RB_HIZ`, `RB_MIP_LEVEL`, `RB_FINAL` are all rebuilt each frame from scratch. Evidence: the SSR scope textures are created in `ssr_allocate_buffers` and written (not read) by the trace (`restrict writeonly image2D output_color`, ssr.glsl:12-13); the only cross-frame state is RB_LAST_FRAME (SSE:366-398) and the projection/transform in `ssr_last_frame_*` (rfc.h:110-111). No temporal blend or denoiser pass exists: pass list in 7. The only noise reduction is the spatial mip chain (6) + bilateral upsample (1). Result is deterministic per pixel (no random rays: ray dir = reflect of view dir, ssr.glsl:137-138, no jitter), so there is no noise to denoise; instead low quality shows as blur/banding.
- TAA: SSR runs before TAA and its input is the previous pre-TAA colour; TAA then resolves the final image including SSR (rfc.cpp:2537-2540; the motion pass is separate, rfc.cpp:2243-2245). No TAA-specific handling in ssr.glsl (no jitter removal): the hit lookup uses un-jittered reprojection of jittered colour; the TAA resolves the sub-pixel shimmer of the result only partially.

### 5. Formats chosen (bandwidth)

| Texture | Format | Size | Where |
|---|---|---|---|
| RB_HIZ | R32_SFLOAT, mips | internal/2 (half) or internal | SSE:1458 |
| RB_SSR | base colour fmt = R16G16B16A16_SFLOAT, mips | same | SSE:1459, rfc.cpp:1492, renderer_scene_render_rd.cpp:1160 |
| RB_MIP_LEVEL | R8_UNORM | same, 1 mip | SSE:1460; ssr.glsl:13 `r8` |
| RB_NORMAL_ROUGHNESS (copy) | R8G8B8A8_UNORM | half only | SSE:1455; downsample.glsl:12 `rgba8` |
| engine normal-roughness | R8G8B8A8_UNORM | internal | rfc.cpp:263-265 |
| RB_FINAL | base colour fmt (RGBA16F) | internal, half only | SSE:1462-1463 |
| RB_LAST_FRAME | R16G16B16A16_SFLOAT | internal (/2 if SSR-only half) | SSE:395 |

Sizes @1080p computed from mip chains (sum of mip pixels = 1.333 x base): half Hi-Z (R32F) 2.76 MB; half SSR RGBA16F 5.53 MB; half mip-level R8 0.52 MB; full-res Hi-Z 11.06 MB, SSR 22.1 MB, mip R8 2.07 MB; RB_FINAL (RGBA16F 1920x1080) 16.6 MB.
- Mip level: stored as `mip_level / 14.0` in 8 bits (ssr.glsl:328; decoded `*14.0` resolve.glsl:270 / scene.glsl:2209); 255 levels over [0,14] => step 0.055 mip, plenty (mip count max 11, SSE:1438). The `/14` is a hard-coded max, independent of the actual mip count.
- Depth is reversed-Z float32 (no precision loss); R32F kept because linear depth is reconstructed through `inv_projection` per-iteration. Normal (octahedral-like `encode24` stored in 3x8 bits, scene.glsl:1185, 3020) 24-bit precision; roughness 7 bits + 1 dynamic flag bit (scene.glsl:3023-3026). `encode24` is a best-fit-normal scale (scene.glsl:1185-1201) so 8 bits/channel keep direction accurate; the SSR shader decodes with just `normalize(xyz*2-1)` (ssr.glsl:116).
- Half-size normal copy (SSE:1455): taken from the same winner texel as depth (downsample.glsl:58), so a nearest-surface-consistent triple; costs 3.6 MB less on every fetch in the trace (8-fetch normal stencil reads Hi-Z, not normal).

### 6. Blur by mip chain instead of per-pixel cone sampling

- Trace stores only ONE colour per pixel (the hit's last-frame colour, ssr.glsl:287, tone-mapped :291) and a per-pixel mip level `log2(blur_radius * max(w,h)/16 + 1)` clamped to [0, mipmaps-1] (:294-318), from a cone: `cone_angle = min(r,0.999)*pi/2` (:295), `op_len = 2 tan(cone) * ray_len` (:297), sphere fit radius (:307-312), scaled by distance `ray_len` (uv units) => blur grows with distance travelled, like a real glossy cone. Faded to 0 near ray_len = 1 by `pow(clamp(1.25-ray_len,0,1),0.2)` (:324).
- Cost of importance sampling avoided: a stochastic/cone-sampled glossy trace costs N rays per pixel (+ denoise); here the cost is 1 ray + a mip lookup, independent of roughness. Evidence: ssr.glsl is the only per-pixel ray loop (1 loop, :184); roughness only changes `mip_level`.
- Filter chain "SSR Roughness Filter": one dispatch per mip m in 1..mipmaps-1 (SSE:1660-1685; mipmaps-1 = 9 at 960x540 half, 10 at 1080p full) on RB_SSR, source = mip m-1 (SSE:1669-1670), dest = mip m, size `max(1, w>>m)` (SSE:1665-1666). Pixels filtered @1080p half: 172,655 (all mips 1..9) = 33% of the trace thread count; full: 691,055.
- Kernel (filter.glsl): separable 7-tap gaussian weights (WEIGHTS :125-132; sigma ~ 1.8 px at that mip), horizontal then vertical in shared memory (`cache[16][16]`, `temp_cache[8][16]`, :123, :172), an 8x8 group loads 16x16 source texels (4 loads per thread, :216-219: 4x apron overhead) with two barriers (:221-222, :227-228); each thread computes 2 horizontal results and 1 vertical result (:224-225, :234) => 21 weighted taps/pixel (3 x 7). Weights are `mix(clamp(mip*0.2,0,1), 1, c.a)` (:134-136): pixels with alpha 0 (misses) still contribute with weight 0.2*mip so the blur extends into/out of invalid areas ("invalid samples write mip levels"; comment ssr.glsl:231-232). The result is renormalised by the weight sum (:160-167).
- Cost of the chain: ~1.33x of base-res pixels written, 4.2 MB read+write of RGBA16F @ half 1080p order of magnitude (computed sums above, my estimate). Because each mip is blurred from the previous (a 7-tap on a 2x-smaller image) the effective footprint doubles each level, so large blur radii cost the same as small ones.
- Quality cost: blur is isotropic in screen space (square-ish gaussian pyramid, trilinear), the "elongated" shape is only approximated by the cone fit; one colour per pixel so no occlusion by anything between the mirrored surface and the hit; roughness is sampled at the reflecting pixel not per-hit; "pyramid-like shape at very rough levels" is acknowledged (ssr.glsl:321-324). Mip level is chosen by resolve/opaque per-pixel from RB_MIP_LEVEL, NOT trilinear-filtered with a gradient: `textureLod` explicit LOD (resolve.glsl:271; scene.glsl:2213) so no hardware derivative.

### 7. Scheduling and profiling labels

Pass order (all inside `_pre_opaque_render` -> `_process_ssr`; each a separate `compute_list_begin/end`; no merging, no async compute):
1. rfc.cpp:1619-1621 `allocate_last_frame_buffer` (texture ensure, clear on create)
2. `RENDER_TIMESTAMP("Process SSR")` rfc.cpp:1490 (CPU timestamp name)
3. label **"SSR Downsample"** (SSE:1504; half only) - 1 dispatch per view, mode by odd width/height
   or label **"SSR Copy Depth"** (SSE:1549; full only) - raster `copy_depth_to_rect`
4. label **"SSR HI-Z"** (SSE:1560): mipmaps-1 dispatches (9 half / 10 full @1080p) per view, each a separate list
5. label **"SSR Main"** (SSE:1611): 1 dispatch per view, `dispatch_threads(size.w, size.h)` (SSE:1651)
6. label **"SSR Roughness Filter"** (SSE:1658): mipmaps-1 dispatches per view
7. label **"SSR Resolve"** (SSE:1689; half only): 1 dispatch per view at internal_size
8. later: label **"Copy Framebuffer for SSIL/SSR"** (rfc.cpp:2449) (contains the last-frame copy/make_mipmap), consumption inside "Render Opaque Pass" (rfc.cpp:2190, SSR sampled in the PBR fragment shader scene.glsl:2197-2228, no separate composite pass).
Total dispatches for half-size 1080p mono: 1 + 9 + 1 + 9 + 1 = 21 + last-frame make_mipmap (1) = 22; full-size: copy_depth (raster) + 10 + 1 + 10 = 21+. Per view: multiview repeats everything per eye in loops (SSE:1506,1551,1562,1613,1662,1693).
- Barriers: none in SSR code: `RenderingDevice::compute_list_end()` just `draw_graph.add_compute_list_end()` (rendering_device.cpp:7090-7098); the old `barrier()` API is deprecated no-op "Barriers are automatically inserted by RenderingDevice" (rendering_device.cpp:7101-7103). So the dependency (Hi-Z mip m-1 -> m, filter m-1 -> m) is resolved by the render graph automatically; consecutive per-mip dispatches of tiny mips (7x4, 3x2, 1x1) are separate lists with barriers (overhead-dominated, my inference).
- Compute local size 8x8x1 in all five shaders (line 7 of each: ssr.glsl, _downsample, _hiz, _filter, _resolve).
- Pipelines: 4 downsample + 4 hi-z variants (odd width/height, SSE:285-304), 1 each for trace/filter/resolve (SSE:315-341). Push constants only (num_steps, fade params, SSE:1621-1632); scene UBO of projection matrices (SSE:1478-1497).
- "RENDER_TIMESTAMP" names near SSR: "Process SSR" (rfc.cpp:1490), "Pre Opaque Render" (rfc.cpp:1644), "Copy Final Framebuffer (SSIL/SSR)" (rfc.cpp:2451). The pass-level labels in 7.3-7.7 are what a GPU capture shows.
- Gating (cost when off): SSR needs `environment_get_ssr_enabled && !transparent_bg` (rfc.cpp:1879-1886, 1835); when on, the depth pre-pass also writes the normal/roughness target (rfc.cpp:1929-1934) - a cost the scene pays even for pixels that never reflect.

### 8. Documentation and findings

Docs: Environment.xml:316-318 `ssr_max_steps`: "Higher values are slower." Environment.xml:305-308: SSR "more accurate ... but are slower and can't reflect surfaces occluded by others"; Forward+ only; not with transparent_bg. ProjectSettings.xml:2935-2936 half_size: "faster but may look pixelated or cause flickering". `ssr_depth_tolerance`: only "The depth tolerance" (Environment.xml:302-304). No doc of cost numbers.

Findings with evidence:
- F1 (correction to the premise "RB_MIP_LEVEL written but unused at half size"): at half size it IS used, by the Resolve pass (resolve.glsl:270; bound SSE:1708, 1717). It is unused by the opaque shader at half size (rfc.cpp:3759 binds a default black texture unless `!half_size`; scene.glsl:2202-2210 reads it only `if (resolve_ssr)`, and the flag SCREEN_SPACE_EFFECTS_FLAGS_RESOLVE_SSR is set when half_size is false, rfc.cpp:768). Despite the name, that flag means "the opaque shader does the mip lookup and tone-map inverse itself" (scene.glsl:2218-2221); at half size Resolve already inverted the tone map (resolve.glsl:275). So no redundancy found.
- F2: The trace writes zeros for skipped pixels (:126-127, :327-328) but the filter and resolve still process every pixel; no mask of "active" pixels is kept, so fully rough/sky screens still pay the Hi-Z, filter and resolve costs (the three passes are unconditional once `using_ssr`).
- F3: Filter apron: the 16x16 `cache` holds 4 loads per thread (filter.glsl:216-219), but the horizontal 7-tap reads x = local.x+4+-3 = 1..14 only (filter.glsl:139-157, called with `local.x + 4` at :224-225), so cache columns 0 and 15 are loaded and never read (2/16 = 12.5% of the loads). Likewise temp_cache rows 0 and 15 are computed horizontally but the vertical pass reads rows 1..14 (:175-193, :234); my index reading, not measured.
- F4: Resolve uses 4 gathers of full detail (4 x `textureLod` trilinear) per pixel on RGBA16F with a `exp` triple per tap (resolve.glsl:282-288, 315-318): the most expensive per-pixel pass at full res (2.07 M threads) - an obvious saving would be skipping pixels whose roughness>=0.7 or depth==0 (not done: sky/rough pixels still compute 16 fetches; resolve.glsl:300-331 has no early-out).
- F5: Hi-Z mip 0 of half-size is the downsample's depth (SSE:1512), shared with the trace and the resolve ("depth_half", SSE:1705) - no duplicate depth buffer is made, a bandwidth saving.
- F6: `ssr_half_size` + SSIL forces the full-size last-frame copy with 6 mips even though SSR only needs mip 1 (SSE:371-377, 1630-1635).
- F7: No hit-distance-based early stop for rays that hit sky: `hit_depth == 0.0` only invalidates (:241), after the loop ran.

### Unclear / needs checking
- (resolved) encode24 is a best-fit-normal encoding (scene.glsl:1185-1201, samples `best_fit_normal_texture`, scales the vector to touch the unit cube), so `normalize(xyz*2-1)` in ssr.glsl:116 is the correct decode: a non-unit vector is fine.
- Measured step counts / GPU timings: none; every cost statement is from code structure (my inference) except the pixel/byte counts, which are computed from the mip loop.
- `RB_SSR` colour format equal to RGBA16F relies on `get_base_data_format()` (force_hdr or preferred = R16G16B16A16_SFLOAT, renderer_scene_render_rd.cpp:1160); `force_hdr` path not traced.
- Whether TAA's jitter is compensated anywhere in SSR reprojection: no code found; not exhaustively searched.
- My statement that the last-frame colour includes the previous frame's SSR is from pass order only.
