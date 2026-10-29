# Godot SDFGI: merged research report

Engine: Godot Engine. Version: 4.7.2-stable. Commit: `ed1daf0bf001b61586d9930840f2f1394092c079` (2026-08-16); first researched at 4.8.0-beta `cafc17a1`; lines re-based. License: MIT (`LICENSE.txt` in the checkout).
Topic: SDFGI, the signed-distance-field global illumination of the Forward+ renderer.
Status of every claim: read from the source by one or more research agents (and spot-checked by the merging agent where this file says so). Nothing was run on a GPU. Where a number comes from a Python re-implementation of a shader formula, it says so.

## 0.0 4.7.2-stable against 4.8.0-beta

The investigation follows **4.7.2-stable** (`ed1daf0bf001b61586d9930840f2f1394092c079`, 2026-08-16); the research
was first read at 4.8.0-beta (`cafc17a1`), and every line below has been re-based onto stable. Between the two
(`git diff ed1daf0b cafc17a1`), SDFGI's own code is the same: `sdfgi_preprocess.glsl`, `sdfgi_direct_light.glsl`,
`gi.glsl`, `scene_forward_gi_inc.glsl`, `environment.cpp`, `fog.cpp`, and `environment_storage.cpp` are unchanged;
`sdfgi_integrate.glsl` differs in whitespace only (the `SH_ACCUM` macro's line continuations); the SDFGI parts of
`renderer_scene_cull.cpp` are unchanged. What differs:

| What | 4.7.2-stable | 4.8.0-beta | Effect on SDFGI |
|---|---|---|---|
| The probe shader's sky define | `gi.cpp:3670` defines `USE_OCTMAP_ARRAY`, but `sdfgi_integrate.glsl:45` and `:277` test `USE_RADIANCE_OCTMAP_ARRAY` | defines `USE_RADIANCE_OCTMAP_ARRAY` (commit `d9ea5c26`, "Add error checks for texture type and storage format mismatches") | With `rendering/reflections/sky_reflections/texture_array_reflections` on (default `true` on desktop, `false` on mobile; rendering_server.cpp:3697-3698), stable compiles the probe shader's `texture2D` sky path while the sky's radiance is an octahedral array. A finding of stable; the binding and what it reads are to be checked (see Findings). |
| `render_geom_facing`'s usage bits | no `TEXTURE_USAGE_STORAGE_ATOMIC_BIT` (gi.cpp:456-457), although the voxelizer writes it with `imageAtomicOr` | adds the atomic bit (commit `bf06d84c`) | A validation fix; the facing mask is the same. |
| What forces the depth pre-pass | only `scene_state.used_opaque_stencil` (render_forward_clustered.cpp:2112, `force_depth_pre_pass`); SSAO itself now needs the pre-pass | also SSAO, SSR, SSIL, and screen-space contact shadows | In both, SDFGI never forces it: with `rendering/driver/depth_prepass/enable` off (default on), `update_probes` does not run and the probes freeze. In stable, enabling SSAO no longer brings it back. |
| `process_gi`'s VoxelGI input | one `RID p_voxel_gi_buffer` | per-view slices | VoxelGI only; out of scope. |

Everything else in the diff (lightmaps, sky ambient refactors, contact shadows, directional-shadow culling) does not
touch SDFGI.

## 0. Scope, notation, and frame order

**Scope.** SDFGI exists only in the Forward+ renderer (`RenderForwardClustered`, Vulkan/D3D12/Metal through RenderingDevice). The Mobile renderer overrides `sdfgi_update` with an empty body and `sdfgi_get_pending_region_count` with `return 0` (`servers/rendering/renderer_rd/forward_mobile/render_forward_mobile.h:643-646`). The Compatibility renderer (GLES3) has the same empty stubs (`drivers/gles3/rasterizer_scene_gles3.h:882-889`), and so does the dummy rasterizer (`servers/rendering/dummy/rasterizer_scene_dummy.h:113-114`). In debug builds the environment setter warns once when SDFGI is enabled on a renderer other than Forward+ (`servers/rendering/storage/environment_storage.cpp`, line 822, checked).

**Path abbreviations** (all paths are relative to the Godot checkout):

| Short | Path |
|---|---|
| RD | `servers/rendering/renderer_rd` |
| gi.cpp, gi.h | `RD/environment/gi.cpp`, `RD/environment/gi.h` |
| rfc.cpp | `RD/forward_clustered/render_forward_clustered.cpp` |
| cull.cpp, cull.h | `servers/rendering/renderer_scene_cull.cpp`, `.h` |
| pre.glsl | `RD/shaders/environment/sdfgi_preprocess.glsl` |
| dl.glsl | `RD/shaders/environment/sdfgi_direct_light.glsl` |
| int.glsl | `RD/shaders/environment/sdfgi_integrate.glsl` |
| gi.glsl | `RD/shaders/environment/gi.glsl` |
| scene.glsl | `RD/shaders/forward_clustered/scene_forward_clustered.glsl` |
| fwdgi.glsl | `RD/shaders/scene_forward_gi_inc.glsl` |
| fog.glsl | `RD/shaders/environment/volumetric_fog_process.glsl` |

A line number prefixed with `~` is the approximate line a research report gave, which the merge did not re-check. Everything else is exact for this commit.

**Key constants** (compile-time, `gi.h:560-568`, `gi.h:646`): `MAX_CASCADES = 8`, `CASCADE_SIZE = 128` (member `cascade_size = 128`), `PROBE_DIVISOR = 16`, `MAX_DYNAMIC_LIGHTS = 128`, `MAX_STATIC_LIGHTS = 1024`, `LIGHTPROBE_OCT_SIZE = 6`, `SH_SIZE = 16`. Derived: `probe_axis_count = 17` (gi.cpp:427), probe spacing = 128/16 = 8 cells, probes per cascade 17^3 = 4913. Defaults: 4 cascades, `min_cell_size` 0.2 m, so cell sizes 0.2, 0.4, 0.8, 1.6 m, boxes of 25.6, 51.2, 102.4, 204.8 m, probe spacing 1.6, 3.2, 6.4, 12.8 m.

**Frame order (one camera render, one viewport).** `RendererSceneCull::_render_scene` (cull.cpp:3297) calls `scene_render->sdfgi_update` (cull.cpp:3314), which for Forward+ is `RenderForwardClustered::sdfgi_update` (rfc.cpp:4026-4070): it frees, creates (`SDFGI::create`, gi.cpp:414) or updates (`SDFGI::update`, gi.cpp:1192) the per-viewport SDFGI object and, in `update`, scrolls each cascade and records its dirty regions. Back in `_render_scene`, the pending regions become `cull.sdfgi` (cull.cpp:3391-3413), the per-instance culling loop picks the geometry and static lights that touch those regions (cull.cpp:3265-3283), the results are merged (cull.cpp:3647-3681), and `scene_render->render_scene` is called (cull.cpp:3706). `RenderForwardClustered::_render_scene` (rfc.cpp:1704) begins with `_update_sdfgi` (call rfc.cpp:1734, definition rfc.cpp:1255), which calls `SDFGI::render_region` once per region (gi.cpp:2061: voxelize, scroll old data, jump flood, occlusion, STORE the solid-cell list) and then `render_static_lights` if any static light was culled (gi.cpp:2422). Next come `update_cascades`, `pre_process_gi` and `update_light` (rfc.cpp:1757-1759; gi.cpp:1514, 1825, 1249), which upload the cascade data and inject the dynamic lights and bounce into the voxel light textures. Inside the depth pre-pass block (rfc.cpp:2120-2133) `_post_prepass_render` runs `update_probes` (RD/renderer_scene_render_rd.cpp:1348-1356; gi.cpp:1311): each probe traces rays through the SDFs and updates its spherical-harmonic history. `_pre_opaque_render` (call rfc.cpp:2182, definition rfc.cpp:1517) then runs `store_probes` (rfc.cpp:1534; gi.cpp:1414), which turns the averaged SH into octahedral irradiance and radiance maps, and `GI::process_gi` (rfc.cpp:1607; gi.cpp:3968), the compute pass that writes the screen-space AMBIENT and REFLECTION buffers. The opaque colour pass reads those buffers (scene.glsl:1982-2025). Transparent surfaces and volumetric fog read the probe data themselves (fwdgi.glsl; fog.glsl). Consequence: the voxel light of frame N reads probes stored at the end of frame N-1, and the probes of frame N+1 read the voxel light of frame N; that loop is the multi-bounce.

## 0.1 Settings table

"Recreate" means `rfc.cpp:4056` destroys and rebuilds the SDFGI object (all textures cleared, `DIRTY_ALL`) when the value differs. `GI::GI()` reads the three project settings once at start (gi.cpp:3515-3519, `GLOBAL_GET`, each `CLAMP`ed to its enum range); runtime changes go through `RenderingServer.environment_set_sdfgi_*` (RD/renderer_scene_render_rd.cpp:95-103).

| Setting | Default | Where read | Recreate? |
|---|---|---|---|
| Environment `sdfgi_enabled` | false | `environment_get_sdfgi_enabled` in `sdfgi_update` (rfc.cpp:4026) | Off frees everything at once; on creates |
| Environment `sdfgi_cascades` | 4 (editor range 1 to 8, enforced only in the scene-layer setter, environment.cpp:484; the server does not clamp) | `num_cascades` compare (rfc.cpp:4056) | Yes |
| Environment `sdfgi_min_cell_size` | 0.2 m (hint 0.01 to 64) | rfc.cpp:4056; `create` (gi.cpp:431, 563) | Yes |
| Environment `sdfgi_y_scale` | 75% (enum 50%, 75%, 100%) giving `y_mult` 2.0, 1.5, 1.0 | `y_mult = y_scale[mode]` (gi.cpp:423-424) | Yes |
| Environment `sdfgi_use_occlusion` | false | `uses_occlusion` (gi.cpp:421); consumers gi.glsl:233, dl.glsl:253, fog.glsl:762 | Yes |
| Environment `sdfgi_bounce_feedback` | 0.5 (hint 0 to 1.99) | `SDFGI::update` every frame (gi.cpp:1193-1197) | No |
| Environment `sdfgi_read_sky_light` | true | gi.cpp:1197 (live), `update_probes` (gi.cpp:1337) | No |
| Environment `sdfgi_energy` | 1.0 | gi.cpp:1194, applied in gi.glsl:478-479 (opaque screen pass only) | No |
| Environment `sdfgi_normal_bias` | 1.1 (cells) | gi.cpp:1195; UBO value `normal_bias/128*16` (gi.cpp:1858) | No |
| Environment `sdfgi_probe_bias` | 1.1 (cells) | gi.cpp:1196; push constant `ray_bias` (gi.cpp:1324) | No |
| Project `rendering/global_illumination/sdfgi/probe_ray_count` | index 1, which is 8 rays (table `{4, 8, 16, 32, 64, 96, 128}`, gi.cpp:1322-1323; the editor hint lists 6 entries from "8", rendering_server.cpp:3793) | gi.cpp:3515 (clamp 0 to 6) | No |
| Project `.../sdfgi/frames_to_converge` | index 5, which is 30 frames (table `{5, 10, 15, 20, 25, 30}`, rfc.cpp:4053) | gi.cpp:3516 | Yes (`history_size`) |
| Project `.../sdfgi/frames_to_update_lights` | index 2, which is every 4th frame (table `{1, 2, 4, 8, 16}`, gi.cpp:1291-1293) | gi.cpp:3517 | No |
| Project `rendering/driver/depth_prepass/enable` | true (restart required) | rfc.cpp:2113 and scene_shader_forward_clustered.cpp:408, both `GLOBAL_GET_CACHED` (the beta caches it in a `depth_prepass_enabled` member instead); gates `update_probes` (rfc.cpp:2112-2132) | n/a |
| Project `rendering/global_illumination/gi/use_half_resolution` | false | gi.cpp:3810; halves the screen GI buffers only | Rebuilds the GI context, not SDFGI |
| Project `rendering/lights_and_shadows/use_physical_light_units` | false | renderer_scene_render_rd.cpp:1824 | n/a |
| `solid_cell_ratio` | 0.25, a constant (no setting, no API) | gi.h:712; gi.cpp:429 | n/a |
| Light3D `light_bake_mode` | Dynamic | light_3d.h:85 | n/a |
| Light `max_sdfgi_cascade` | 2 (RenderingServer only: `light_set_max_sdfgi_cascade`, rendering_server.cpp:2532; no Light3D property; light_storage.h:68, cull.h:705) | cull.cpp:1639-1642 (comment: "should most likely make sdfgi dirty in scenario") | No, changing it does not re-bake |
| GeometryInstance3D `gi_mode` | Static | visual_instance_3d.h:144, setter visual_instance_3d.cpp:479-497 | No |
| Environment `volumetric_fog_gi_inject` | 1.0 | fog.cpp:1031 | n/a |

Editor-only helper properties `sdfgi_cascade0_distance` (= `min_cell_size*64`, 12.8 m) and `sdfgi_max_distance` (= `min_cell_size*64*2^cascades`, 204.8 m) are not stored (environment.cpp:1418-1431; doc/classes/Environment.xml:211, 226). The first is the half-extent of cascade 0; the second is the full width of the last cascade.

---

## 1. Settings, lifecycle and cascades (including scrolling and dirty regions)

**What it is for.** SDFGI keeps `N` nested cascades of 128^3 cells around the camera, each twice as coarse as the last. This section covers how the object is created and destroyed, what it allocates, where each cascade sits in the world, how it follows the camera in whole-probe steps, and how the engine records which part of the grid is new (the dirty region) so that only that part is voxelized again.

| Claim | Source | Numbers and formats | Where the lab differs |
|---|---|---|---|
| 1.01 The SDFGI object is a per-viewport `RenderBufferCustomDataRD` stored under `RB_SCOPE_SDFGI`; each viewport owns its own textures. | rfc.cpp:4026-4070; gi.cpp:3829 `create_sdfgi` | One full set per viewport | - |
| 1.02 `sdfgi_update` frees the object if SDFGI is off or no environment exists; a `sdfgi_reset()` (version bump) frees and recreates it. | rfc.cpp:4026-4070; gi.cpp:397 `sdfgi_reset`; editor_node.cpp:4685 (only engine caller) | Version counter only | - |
| 1.03 An existing object is rebuilt if `num_cascades`, `min_cell_size`, `history_size`, `uses_occlusion` or `y_scale_mode` changed. Energy, normal bias, probe bias, bounce and read-sky are not in that test. | rfc.cpp:4056; gi.cpp:1193-1197, 1125-1129 | See the settings table | - |
| 1.04 On the frame of creation `update()` is not called: `create` sets every cascade to `DIRTY_ALL`, and the same frame's cull reads it, so the whole grid is voxelized that frame. On later frames `update` first zeroes each `dirty_regions`. | gi.cpp:571 (create), 1204 (update); gi.cpp:45 `DIRTY_ALL` | `DIRTY_ALL = Vector3i(0x7FFFFFFF x 3)` | - |
| 1.05 `SDFGI::update` is the only place a cascade moves. Per cascade and axis, `drag_margin = (cascade_size / PROBE_DIVISOR) / 2 = 4` cells. | gi.cpp:1199 | 4 cells | Lab cascade size is a lab choice (section 5) |
| 1.06 The camera cell is `Vector3i(world_position / cell_size)` with `world_position.y` multiplied by `y_mult` first. The conversion truncates toward zero (no floor, no +0.5). | gi.cpp:1207-1209 | Truncation toward zero | - |
| 1.07 A cascade steps by `2*drag_margin = 8` cells, which is exactly one probe spacing, and only while the camera's cell is more than 4 cells from `cascade.position`. After the loop the offset lies in (-4, 4]. | gi.cpp:1211-1222 | Trigger 0.8 m and step 1.6 m for cascade 0; 1.6/3.2, 3.2/6.4, 6.4/12.8 for cascades 1 to 3 | - |
| 1.08 Sign convention: `dirty > 0` means fill from the start of the texture (camera moved toward -axis); `dirty < 0` means fill from the end (moved +axis). The same value is the scroll amount of the preprocess shader: `write_pos = read_pos + scroll`. | gi.cpp:1211-1222; pre.glsl:183 | Camera +x by one step gives `dirty.x = -8`, old data shifts to lower indices by 8 | - |
| 1.09 The textures are not toroidal. Old data is physically moved by the scroll compute shaders, so texture index 0 always means world cell `position - 64`; the box spans `[position-64, position+64)` cells. | pre.glsl:174-201; gi.cpp:2117-2207 | 128 cells per axis | A CPU mirror can use plain arrays |
| 1.10 `probe_half_size` is computed and then set to zero, so its term in the camera-cell expression is dead. | gi.cpp:1204-1205 (computed at 1204, zeroed at 1205; checked) | 0 | - |
| 1.11 If a move is 128 cells or more on any axis, the cascade goes `DIRTY_ALL` and the loop `break`s. A single-axis move of `d` cells also goes `DIRTY_ALL` when the dirty volume exceeds half of the kept volume: `128^2*d > 128^2*(128-d)/2`, so `3d > 128`, so `d >= 43`; with 8-cell steps that is 48 cells (6 steps). | gi.cpp:1226-1243 | 40 cells stays partial; the `>= 128` rule needs 16 steps | - |
| 1.12 The `break` on the `>= 128` rule skips the later axes' `while` loops, so their `position` is not advanced that frame; the next frame they catch up as a partial scroll on top of the full redraw. | gi.cpp:1226-1229 | Read, not run | - |
| 1.13 `create` allocates 128^3 3D scratch textures shared by all cascades: `render_albedo` R16_UINT, `render_emission` R32_UINT, `render_emission_aniso` R32_UINT, `render_geom_facing` R32_UINT (no `STORAGE_ATOMIC_BIT` in stable, gi.cpp:456-457, although the voxelizer writes it atomically; the beta adds the bit, see 0.0), `render_occlusion[8]` R8_UNORM, `render_sdf[2]` R8G8B8A8_UINT, plus `render_sdf_half[2]` 64^3 R8G8B8A8_UINT. All are cleared to 0 at creation. | gi.cpp:436-518, 457 | See the VRAM estimate below | - |
| 1.14 Per cascade: `sdf_tex` R8_UNORM 128^3; `light_data` R32_UINT 128^3 with an `E5B9G9R9_UFLOAT` view `light_tex`; `light_aniso_0_tex` R8G8B8A8_UNORM; `light_aniso_1_tex` R8G8_UNORM; `solid_cell_buffer` (524,288 x 16 B); two 16 B dispatch buffers; `lights_buffer` (1024 x 112 B); probe history and average textures. | gi.cpp:547-590 | See below | - |
| 1.15 Shared probe and occlusion storage: `occlusion_data` R16_UINT 256 x 128 x (128 x cascades) with an R4G4B4A4 view; `lightprobe_data` R32_UINT 2312 x 136 x (2 x cascades) layers with an E5B9G9R9 view; `ambient_texture` RGBA16F 289 x 17 x cascades; probe-scroll scratch textures. | gi.cpp:471-545 | 2312 = 17*17*8, 136 = 17*8 | - |
| 1.16 Initial cascade position: `world_position.y *= y_mult; probe_size = cell_size * 8; position = floor(world_position / probe_size + 0.5) * 8`, so it is always a multiple of 8. | gi.cpp:565-569 | Camera (3, 2, -1): cascade 0 gives (16, 16, -8) cells | - |
| 1.17 `y_mult = {2.0, 1.5, 1.0}[y_scale_mode]`: the labels 50%, 75%, 100% give 2.0, 1.5, 1.0. At the default 1.5, world y is multiplied by 1.5 before gridding, so a voxel is `cell_size/1.5` tall in real space (0.1333 m for cascade 0) and the cascade is `128*cell_size/1.5` tall (17.07 m for cascade 0, two thirds of its 25.6 m width). | gi.cpp:423-424, 565, 1208, 1468-1469 | Voxel 0.2 x 0.1333 x 0.2 m at cascade 0 | Lab may use y_mult 1.0 for the study room |
| 1.18 Cell size doubles per cascade (`cell_size_i = min_cell_size * 2^i`); the width in cells is always 128; positions are cells of that cascade, measured from the world origin, with y in squashed units. | gi.cpp:573 (`base_cell_size *= 2`) | 0.2, 0.4, 0.8, 1.6 m | - |
| 1.19 Min corner of cascade `i` in squashed space is `(position - 64) * cell_size` per axis; the real-y min corner has `y_real = (position.y - 64) * cell_size / y_mult`. | gi.cpp:1955-1957, 1514-1521 | Cascade 1 at position (16,16,-8): min x = -19.2 m | - |
| 1.20 `update_cascades` uploads a 48-byte `Cascade::UBO` per cascade: `offset = (-64 + position) * cell_size` (squashed), `to_cell = 1/cell_size`, `probe_offset = position/8`. The 8-slot array is a stack local with no initializer; slots at or above `num_cascades` are garbage. | gi.cpp:1514-1533; gi.h:572-578 | 384 bytes uploaded | - |
| 1.21 `pre_process_gi` writes the 496-byte `SDFGIData` UBO read by gi.glsl: `grid_size` 128, `probe_axis_size` 17, `cascade_probe_size` 16, `probe_to_uvw` 1/16, `occlusion_clamp` 0.9375, `normal_bias = normal_bias/128*16` (0.1375 at the default), `lightprobe_tex_pixel_size = (1/(8*289), 1/(8*17), 1)`, `occlusion_renormalize = (0.5, 1, 1/N)`, and per cascade a camera-relative `position`, `to_probe = 1/(8*cell_size)`, `probe_world_offset = position/8`, `to_cell`, `exposure_normalization = camera_exposure / baked_exposure`. | gi.cpp:1825-1905 | `probe_bias` is not in this UBO; it is a push constant | - |
| 1.22 The probe grid is 16 cells wide with 17 probes per axis (both ends enclosed); probe 16 lies on the cascade's far boundary, outside `[0, 128)`. | gi.cpp:427; int.glsl:166-173 | 17^3 = 4913 probes per cascade | - |
| 1.23 `RenderForwardClustered::_update_sdfgi` runs first in `_render_scene`; the cascade UBO (`update_cascades`) is rewritten only later (rfc.cpp:1757) and at the top of `render_static_lights` (gi.cpp:2430). | rfc.cpp:1734, 1757, 1255-1281; gi.cpp:2430 | See Finding 10 | - |
| 1.24 Reflection-probe renders skip SDFGI update, region culling and the dynamic light data (`p_reflection_probe.is_null()` guards). | cull.cpp:3314, 3391, 3676-3681 | - | - |

**Estimated VRAM** (derived by the first report from the formats, 1 MiB = 1,048,576 B, no allocator padding, defaults: 4 cascades, history 30; not read from the engine): shared about 122.9 MiB (of which the render scratch is about 62 MiB, `occlusion_data` 32 MiB, `lightprobe_history_scroll` 18 MiB, `lightprobe_data` 9.6 MiB), per cascade about 49.3 MiB (3D textures 22 MiB, solid cells 8 MiB, history 18 MiB, average 1.2 MiB), total about 320 MiB per viewport. History parts scale with `history_size` (0.6 MiB per layer per cascade).

**Dead or stale code seen here:** the comment "Always 64x64" (gi.cpp:435) is wrong (the grid is 128); `Cascade::SolidCell` is "unused, remains as reference for size"; `light_aniso_0_data`/`light_aniso_1_data` RIDs are declared and never assigned; `GI::sdfgi_ray_count = RAY_COUNT_16` in gi.h:708 is overwritten by the constructor; `SDFGI::free_data()` is empty (gi.cpp:1132).

### 1.A Worked example: a scroll (cascade 0, defaults, camera x from 0 to 1.0 m)

Before: the camera was at the origin at creation, so `position = (0,0,0)` and cascade 0 spans x in [-12.8, 12.8) m.

1. Camera x = 1.0 m: `pos_in_cascade.x = int(1.0 / 0.2) = 5`. y and z equal their positions, so no loop runs.
2. x axis: 5 > 0 + 4, so `position.x = 8` and `dirty.x = -8`. 5 > 8 + 4 is false, so the loop ends. |-8| < 128.
3. Volume check: kept `120*128*128 = 1,966,080`, dirty `2,097,152 - 1,966,080 = 131,072`, half the kept volume 983,040. Not larger, so the scroll stays partial.
4. One pending region: `dirty.x < 0`, so `from = (120,0,0)`, `to = (128,128,128)`, local offset (120,0,0), local size (8,128,128).
5. Bounds in real metres: position `x = (120 - 64 + 8) * 0.2 = 12.8`, `y = -64 * 0.2 / 1.5 = -8.533`, `z = -12.8`; size `(1.6, 17.067, 25.6)`. The cascade now spans x in [-11.2, 14.4) m; the dirty slab [12.8, 14.4) is exactly the strip that became visible.
6. The scroll compute shaders move the old content by -8 cells in x. Cascade 1 for the same move: `int(1.0/0.4) = 2`, not above 4, so it stays put.

Variants: a diagonal move x+8, z+8 gives `dirty = (-8,0,-8)` and two regions: x gives (120,0,0) to (128,128,128), z gives (0,0,120) to (120,128,128) after the corner is chipped (size (120,128,8)). A move to x = 10 m gives `pos = 50`, six steps, `dirty.x = -48`; kept 80*128*128 = 1,310,720, dirty 786,432 > 655,360, so `DIRTY_ALL`.

---

## 2. Culling: regions, geometry, lights

**What it is for.** The CPU decides what the GPU will voxelize and light this frame. It turns the dirty regions into world-space boxes, then walks every instance once and keeps the geometry that overlaps a region and the static lights that overlap a region. Dynamic lights are not culled by region at all.

| Claim | Source | Numbers and formats | Where the lab differs |
|---|---|---|---|
| 2.01 Regions are enumerated in one flat index over cascades ascending, then axes x, y, z. `DIRTY_ALL` gives one region (offset 0, size 128^3); otherwise one region per axis with non-zero dirty. | gi.cpp:1458-1512 `get_pending_region_data`; rfc.cpp:4072-4095 | At most 3 regions per cascade; arrays of 24 (cull.h:57-58) | - |
| 2.02 For a partial region on axis `j`: the `j` axis range is 0..dirty (positive) or `128+dirty`..128 (negative); other axes start at the full range, and each earlier axis `k < j` with non-zero dirty is chipped (`from[k] += dirty[k]` or `to[k] += dirty[k]`) so corners are not voxelized twice. | gi.cpp:1458-1512 | Diagonal example in 1.A | - |
| 2.03 Region bounds are real-world AABBs: `position = (from - 64 + cascade.position) * cell_size * (1, 1/y_mult, 1)`, `size = region_size * cell_size * (1, 1/y_mult, 1)`. | gi.cpp:1468-1469, 1500-1501 | At y_mult 1.5 the y extents are real metres | - |
| 2.04 The cull layer asks the renderer three times per region (count, bounds, cascade); each call re-runs `get_pending_region_data`, so the cost is quadratic in at most 24 regions. It records the distinct dirty cascades in order (`cascade_light_index`). | cull.cpp:3391-3413; cull.h:1112-1121 | - | - |
| 2.05 The per-instance SDFGI test runs for every instance in `scenario->instance_data`, outside the camera-visibility `if`: frustum, occlusion and visibility-range tests do not apply. Only visible instances with a valid transform are in that array. | cull.cpp:3265-3283, 1739 | Strict `in_aabb` overlap, no margin | The lab selects its own geometry |
| 2.06 Geometry qualifies if it is mesh, multimesh or particles, has `FLAG_USES_BAKED_LIGHT` (bit 16), and the camera's `visible_layers` match its `layer_mask`. Only `gi_mode = Static` sets that flag; Dynamic and Disabled do not. | cull.cpp:3278-3283, 1248-1265, 1833; cull.h:269; visual_instance_3d.cpp:479-497 | Static: baked_light true; Dynamic: baked_light false, dynamic_gi true | - |
| 2.07 The test does not look at `FLAG_CAST_SHADOWS_ONLY`; a Static mesh set to "shadows only" is still voxelized. | cull.cpp:3264-3281 vs 3011 | Read, not run | - |
| 2.08 Static lights qualify if `bake_mode == LIGHT_BAKE_STATIC`, the light AABB overlaps a region, and `region_cascade <= max_sdfgi_cascade`; a (light, cascade) dedupe suppresses repeats. They are stored in `sdfgi_cascade_lights[cascade_id]`. | cull.cpp:3269-3276 | Default `max_sdfgi_cascade = 2`: cascades 0, 1, 2 | - |
| 2.09 Directional lights never become static for SDFGI: their cull-side `bake_mode` stays `DISABLED` (set only for non-directional lights) and they are not in the instance AABB list. | cull.cpp:1626-1638, 712, 903; cull.h:752 | - | - |
| 2.10 After culling: per region the geometry list is merged into `render_sdfgi_data[i]`; `static_lights_culled` checks the lists for `i < cascade_light_count`; if true, `update_static = true` and the cascade count, index array and the whole 8-entry light array are handed on. For non-reflection renders all visible layer-matching directional lights and all `scenario->dynamic_lights` are attached. | cull.cpp:3647-3681; renderer_scene_render.h:289-300 | `RenderSDFGIData`, `RenderSDFGIUpdateData` | - |
| 2.11 A light's AABB comes from `light_get_aabb`: omni cube of the range, spot cone box, area box; directional gives an empty `AABB()`. | RD/storage_rd/light_storage.cpp:508-540 | - | - |
| 2.12 Receiving is separate from voxelizing: `can_sdfgi` is true when there is no lightmap, no VoxelGI on the geometry, and `use_baked_light` or `use_dynamic_gi`. The draw-time flag `INSTANCE_DATA_FLAG_USE_SDFGI` also needs `using_sdfgi` (SDFGI on and debug mode not Unshaded). Lightmapped or VoxelGI geometry is still voxelized. | rfc.cpp:4500-4506, 1059-1061, 1876 | Gi mode table below | - |
| 2.13 Positional light routing: Dynamic lights (default) are in `scenario->dynamic_lights` and enter the per-frame dynamic pass; Static lights only appear via region culling; Disabled lights go nowhere. Default `max_sdfgi_cascade = 2` excludes cascade 3 and above. | cull.cpp:610, 838, 1066-1070, 1627-1635; gi.cpp:1948-1960 | - | - |
| 2.14 The cull results are rebuilt each frame (`scene_cull_result.clear()`, `cascade_light_count` zeroed), so a light dropped by the mismatch in Finding 2 is not carried over. | cull.cpp:3416, 3395; cull.h:924-926 | - | - |

Geometry table:

| `gi_mode` | Voxelized | Receives SDFGI |
|---|---|---|
| Static (default) | yes | yes |
| Dynamic | no | yes |
| Disabled | no | no |

---

## 3. Voxelization

**What it is for.** The scene is turned into a 128^3 grid of "solid" voxels per cascade, with albedo, a facing mask and optional emission. It uses the GPU rasterizer: the region box is rendered three times with an orthographic camera looking along each axis, and the fragment shader writes straight into 3D images. Only surfaces are voxelized (there is no interior fill).

| Claim | Source | Numbers and formats | Where the lab differs |
|---|---|---|---|
| 3.01 `render_region` (per region) first, for the first region of a cascade, clears `render_albedo`, `render_emission`, `render_emission_aniso`, `render_geom_facing` to 0; several regions of one cascade accumulate into the same textures. | gi.cpp:2066-2079 | - | - |
| 3.02 `_render_sdfgi` builds a temporary render data with `cluster_size = 1`, sets `pass_mode = PASS_MODE_SDF`, fills the secondary render list and sorts it. Only surfaces with `FLAG_PASS_DEPTH` and `FLAG_PASS_OPAQUE` are added: alpha-blended surfaces are not voxelized (except those with `uses_depth_in_alpha_pass`). | rfc.cpp:3099-3120, 1189-1193, 4190-4201; scene_shader_forward_clustered.h:282-296 | - | The lab draws its own study room |
| 3.03 Alpha scissor and alpha hash count as opaque and voxelize; the shader's `discard` is honoured so holes are not voxelized. | scene.glsl:2887-2897 | - | - |
| 3.04 Three passes, axis `i = 0,1,2`: camera at the +axis face of the region box looking along -axis; orthographic projection `(-h, h, -v, v, 0, d)` with `h = half_size[right]`, `v = half_size[up]`, `d = 2*half_size[i]` (exactly the region box); framebuffer is empty (no attachments) of size `size[right] x size[up]`, one pixel per voxel face. | rfc.cpp:3122-3175 | Region sizes in cells | A CPU mirror can rasterize triangles directly |
| 3.05 Both faces are rasterized: in `_render_list_template`, `PASS_MODE_SDF` forces the double-sided cull variant, which maps to `POLYGON_CULL_DISABLED` for every material cull mode. No conservative rasterization. | rfc.cpp:397-399; scene_shader_forward_clustered.cpp:512-518 | - | - |
| 3.06 Multiview is refused in the SDF pass. | rfc.cpp:479-482 | - | - |
| 3.07 `sdf_to_bounds = affine_inverse(bounds_transform) * cam_transform` maps a view-space vertex into [0,1]^3 of the box; the region's voxel offset and size go into the scene UBO (`sdf_offset`, `sdf_size`). | rfc.cpp:3126-3162; scene_forward_clustered_inc.glsl:331-336 | `p_size` is the region size, not the cascade size | - |
| 3.08 The voxel written is derived from each fragment's interpolated 3D position, not the pixel coordinate: `grid_pos = sdf_offset + ivec3(local_pos * sdf_size)`, with no bounds check or clamp. | scene.glsl:2909-2910 | A fragment at exactly 1.0 would fall one voxel outside | - |
| 3.09 `y_mult` enters voxelization only through the bounds (already squeezed by 1/y_mult); the SDF fragment shader has no `y_mult`. | gi.cpp:1468-1469; rfc.cpp:3122-3123 | A voxel is `cell_size` wide in x and z, `cell_size/y_mult` tall | - |
| 3.10 Render targets are image bindings 24 to 27 (albedo, emission, emission-aniso, geom-facing). The pipeline is `SHADER_VERSION_DEPTH_PASS_WITH_SDF` compiled from `MODE_RENDER_DEPTH` + `MODE_RENDER_SDF` in the advanced shader group, so it exists only when SDFGI enables that group. | rfc.cpp:3768-3979; scene_shader_forward_clustered.cpp:656, h:64 | - | - |
| 3.11 Albedo voxel (R16_UINT): bit 0 = solid flag; bits 1-5 B, 6-10 G, 11-15 R, each `clamp(uint(c*31), 0, 31)` (truncated). Last fragment to write wins. | scene.glsl:2912-2917 | 5-5-5 plus solid bit | - |
| 3.12 Facing mask (R32_UINT): per fragment exactly one bit is chosen, the direction of `aniso_dir[6] = +X, +Y, +Z, -X, -Y, -Z` with the largest dot against the interpolated vertex normal (rotated to world, not flipped for back faces, no normal map), and ORed in with `imageAtomicOr`. | scene.glsl:2919-2948 | Bits 0-5 in that order | - |
| 3.13 The non-atomic fallback (`NO_IMAGE_ATOMICS`) is dead for this shader: only the fog shader variants define it. | scene.glsl:2940-2941; fog.cpp:223-225, 314 | - | - |
| 3.14 Emission (only if `length(emission) > 0.001`; value times `emissive_exposure_normalization`): per direction `strength = max(0, dot(n, aniso_dir[i]))`, `light = emission*strength`. `render_emission_aniso` holds `min(31, uint(luma_i/luma_total*31)) << (5*i)` for six directions (bits 0-29, same order). | scene.glsl:2950-2990, 1682; rfc.cpp:3164 | 6 x 5 bits | - |
| 3.15 `render_emission` (R32_UINT) holds RGBE "8985": a shared-exponent encode (N=9, B=15) stored as R top 8 of 9 bits (bits 0-7), G 9 bits (8-16), B top 8 of 9 bits (17-24), E 5 bits (25-29); bits 30-31 free for neighbour bits. This is the layout of the solid cell's light word; it is 30 bits. | scene.glsl:2985 (comment "store as 8985 to have 2 extra neighbor bits") | 30 bits | - |
| 3.16 After the last region of a cascade, the preprocess sequence runs (gi.cpp:2086): optional scroll (4.x), then jump flood, occlusion and STORE. `push_constant.scroll = dirty_regions` unless `DIRTY_ALL` (then 0); the same push constant, including `scroll`, is reused by every later dispatch of that cascade. | gi.cpp:2086-2115 | 48-byte `PreprocessPushConstant` (gi.h:279-290) | - |
| 3.17 The scroll step runs only when `dirty_regions != DIRTY_ALL`: `buffer_copy` of the previous solid-cell counters to the call buffer; `PRE_PROCESS_SCROLL` (indirect, 64 threads per group) re-injects the old solid cells into the cleared render textures at `read_pos + scroll`; `PRE_PROCESS_SCROLL_OCCLUSION` shifts the packed occlusion; then the integrate scroll modes (section 8). | gi.cpp:2117-2207; pre.glsl:174-201 | No barrier between SCROLL and SCROLL_OCCLUSION (disjoint outputs) | - |
| 3.18 SCROLL writes, for each kept cell, `albedo = ((stored.albedo & 0x7FFF) << 1) restore solid bit`, `facing = (stored.albedo >> 15) & 0x3F`, `light = stored.light & 0x3FFFFFFF`, `light_aniso = stored.light_aniso & 0x3FFFFFFF`; neighbour bits are masked off and recomputed by STORE. Cells whose destination falls outside the grid are dropped; indices at or above `total_count` return. | pre.glsl:174-201, 187-199 | - | - |
| 3.19 The solid-cell counter buffer is zeroed after the scroll reads were issued (`buffer_update(..., {0,0,0,0})`), so scroll reads the old list and STORE rebuilds it from zero. | gi.cpp:2210-2211 | - | - |
| 3.20 After the list: `light_tex`, `light_aniso_0_tex`, `light_aniso_1_tex` of the cascade are cleared ("previous garbage on next draw"); the next light update refills them from the new solid-cell list. | gi.cpp:2386-2388 | - | - |
| 3.21 Dead code in `render_region`: two `#if 0` blocks dumping PNGs (the second does not compile, `createcascade_size`). | gi.cpp:2390-2415 | - | - |
| 3.22 A cascade update that contains zero solid voxels would make every seed `(0,0,0,0)`; STORE does not test `p.w`, so voxel (0,0,0) would be listed as a solid cell and every other voxel would get `d = 1 + length(pos)`. Derived by reading, not run. | pre.glsl:954-971 | - | - |

---

## 4. The distance field: jump flood, STORE, solid-cell list

**What it is for.** From the voxelized surface the engine builds a distance field that rays can sphere-trace (probe rays, shadow rays, reflection rays), and a compact list of solid cells that every later light pass iterates. The "SDF" is a nearest-solid-voxel seed map built by jump flooding; distances are computed only when STORE converts seeds to numbers.

| Claim | Source | Numbers and formats | Where the lab differs |
|---|---|---|---|
| 4.01 `half_size = true` is hard-coded ("much faster, very little difference"), so only the half-resolution path is live; the full-size branch and `JUMP_FLOOD_INITIALIZE` are dead. | gi.cpp:2219, 2293-2343 | - | - |
| 4.02 Seed encoding (RGBA8_UINT): `xyz` = full-resolution voxel coordinates of the nearest known solid voxel (0 to 127), `w = 255` valid, `w = 0` unused. | pre.glsl:239-241, 258-262 | 8 bits per axis | - |
| 4.03 `INITIALIZE_HALF`: for each half cell, look at its 2x2x2 block of full-res voxels, collect the solid ones, and pick `closest[index % count]` where `index` is the half cell's parity bits (so neighbours pick different sub-voxels). None solid gives `(0,0,0,0)`. | pre.glsl:245-273; gi.cpp:2222-2230 | Dispatch 64^3 threads, 4^3 groups | - |
| 4.04 Regular `JUMP_FLOOD`: for the 26 neighbours at `pos + offset*step` inside the grid, skip `w == 0`; keep the seed with the smaller Euclidean distance to the stored full-res position. In half mode the cell centre is `pos*2 + 0.5`. | pre.glsl:275-349 | - | - |
| 4.05 `JUMP_FLOOD_OPTIMIZED`: same math in 8^3 groups; each group handles a strided sub-lattice (`group_offset = WorkGroupID % step`), loads a 10^3 neighbourhood into shared memory and scans 27 cells. Used when `grid/(s/2) >= 8`. | pre.glsl:351-444; gi.cpp:2238-2273 | Shared `uvec4 group_positions[1000]` | - |
| 4.06 Half-res schedule for a 128 cascade: 6 passes with steps 32, 16 (regular) then 8, 4, 2, 1 (optimized), ping-ponging `render_sdf_half[0/1]`; the result is in `render_sdf_half[0]`. | gi.cpp:2238-2273 | Steps in half-cell units (32 = 64 voxels) | A CPU mirror can use any exact algorithm |
| 4.07 `JUMP_FLOOD_UPSCALE`: each full-res voxel that is solid gets `(pos, 255)`; otherwise take the half seed at `pos >> 1`, test the 8 voxels of the 2x2x2 block containing the found voxel, and take any nearer solid one. `w` is inherited from the half seed. | pre.glsl:446-482 | 128^3 threads | - |
| 4.08 A final full-res `JUMP_FLOOD_OPTIMIZED` with step 1 fixes half-size artefacts. Total per cascade update: 1 init + 6 half passes + 1 upscale + 1 final = 9 jump-flood dispatches. | gi.cpp:2276-2290 | - | - |
| 4.09 Ping-pong parity: `passes = nearest_shift(128) - 1 = 7`; upscale reads `render_sdf_half[(passes&1)?0:1] = [0]`, writes `render_sdf[0]`; the final pass reads `render_sdf[0]` and writes `render_sdf[1]`; STORE reads `render_sdf[1]`. | gi.cpp:488, 944-955; math_funcs_binary.h:162-170 | - | - |
| 4.10 The "distance" is Euclidean in voxel index space, with no `y_mult` correction: the anisotropy is baked into the voxel size. | pre.glsl:954-971 | - | - |
| 4.11 `MODE_STORE` writes the final SDF to `cascade.sdf_tex` (R8_UNORM): solid voxel gives `d = 0`; empty voxel gives `d = 1.0 + length(seed.xyz - pos)`; then `d /= 255`. Units are cells of that cascade at full resolution; the largest representable distance is 254 cells. | pre.glsl:954-971 | A face neighbour of a solid voxel stores 2/255; 1/255 is never written | - |
| 4.12 The field is not signed. Consumers recover a pseudo-signed value by subtracting an offset after sampling: 1.0 in the probe march and the shadow march, 1.1 in the screen pass, 1.7 in the debug view. A solid texel reads -1 (or -1.1); linear filtering between solid and empty puts the zero crossing midway. | int.glsl:227; dl.glsl:408; gi.glsl:415, 431; sdfgi_debug.glsl:141 | Offsets 1.0, 1.0, 1.1, 1.7 | - |
| 4.13 Because voxelization is surface-only, a closed thick mesh has empty positive voxels inside its shell; interior voxels are not 0. | scene.glsl:2909-2948; pre.glsl:954-971 | - | - |
| 4.14 STORE also repacks occlusion (section 5) and builds the solid-cell list (below). Threads are 4x4x4; `dispatch_threads(128^3)`. | pre.glsl:948-1062; gi.cpp:2376-2383 | - | - |
| 4.15 Solid-cell record (16 bytes = four uint): `position = x \| y<<7 \| z<<14` (7 bits each, bits 0-20) with neighbour bits 11-21 in bits 21-31; `albedo` = RGB555 in bits 0-14, 6 facing bits in 15-20, neighbour bits 0-10 in 21-31; `light` = the voxelizer's 30-bit RGBE8985 emission plus neighbour bits 22-23 in bits 30-31; `light_aniso` = 6 x 5 bits plus neighbour bits 24-25 in bits 30-31. | pre.glsl:989-1062 | 524,288 entries per cascade (25% of 128^3) | - |
| 4.16 STORE reads emission from `render_emission` and `render_emission_aniso` (bindings 4, 5), not from `light_tex`. So a solid cell's `light` word starts as the surface emission (for a kept cell after a scroll: whatever SCROLL wrote there, which is the old emission plus static light). | pre.glsl:1030-1034; gi.cpp:618-630 | - | - |
| 4.17 Neighbour mask: 26 bits in loop order (i, j, k over -1..1 for x, y, z, centre skipped, `bit_index++` before the in-grid test). Bit n is set when the voxel at `pos + offset_n` has this cell as its jump-flood nearest solid (`seed.xyz == pos`); solid neighbours point at themselves and are never flagged. Bit 0 = (-1,-1,-1), bit 12 = (0,0,-1), bit 13 = (0,0,1), bit 25 = (1,1,1). | pre.glsl:1001-1017 | Split across albedo, position, light, light_aniso as in 4.15 | - |
| 4.18 Counters: each solid voxel takes a slot with a shared `atomicAdd`; the group reserves a range with `atomicAdd(total_count, n)`, raises `dispatch_data.x` to `ceil(total/64)` with `atomicMax`, and writes its records. `y = z = 1` is set by the thread at (0,0,0). The call buffer for indirect dispatch is a copy taken at the next use (gi.cpp:1253, 2117, 2557). | pre.glsl:989-1062 | - | - |
| 4.19 No overflow check: entries beyond 524,288 per cascade would be written out of range and `total_count` can exceed capacity. | pre.glsl:989-1062 | - | - |
| 4.20 The only reader of the neighbour mask is the dynamic light pass (it copies a cell's light into the 26 neighbours so trilinear sampling near a surface is lit); the static pass ignores it, and SCROLL strips it. | dl.glsl:497-557 (comment 514); pre.glsl:187-199 | - | - |

### 4.A Worked example: jump flood on one cascade update (128, half = 64)

1. Init: 64^3 half cells, each seeded from its 2x2x2 block (or `(0,0,0,0)`).
2. Pass 1: step 32 (regular). Check `64 / (32/2) = 4 < 8`, continue. Pass 2: step 16 (regular). Check `64/8 = 8 >= 8`, switch to optimized.
3. Passes 3 to 6: steps 8, 4, 2, 1 (optimized). After pass 6, `s == 1`.
4. Upscale to 128^3, then one full-res step-1 pass.
5. STORE: seed at its own position gives byte 0; face neighbour gives 2; diagonal neighbour gives `1 + sqrt(2)`, read as 1.41 after the consumer's `*255 - 1`.

---

## 5. Occlusion

**What it is for.** For every voxel and each of eight probe parity classes, a 0..1 "how visible is this voxel from that probe" value. The screen pass and fog use it (when `use_occlusion` is on) to down-weight probes that sit on the far side of a wall. It is purely geometric (no light).

| Claim | Source | Numbers and formats | Where the lab differs |
|---|---|---|---|
| 5.01 Occlusion is always computed; `use_occlusion` only gates the readers. | gi.cpp:2345-2373 (no `uses_occlusion` guard); gi.glsl:233; dl.glsl:253; fog.glsl:762 | - | - |
| 5.02 Eight dispatches, one per parity class `i`: `offset = (i&1, (i>>1)&1, (i>>2)&1)`, flipped on an axis if the global probe index (`cascade.position / 8`) is odd there; `groups = (9,9,9) - offset`; no barrier between, one after all eight. | gi.cpp:2345-2373 | 64 threads per group (`local_size 64x1x1`) | - |
| 5.03 Each group owns a 16^3 voxel region centred on one probe (`region_offset = -8 + region*16 + probe_offset*8`); a voxel lies in 8 probe regions, one per class, written to `render_occlusion[occlusion_index]`. | pre.glsl:10-13, 489-491 | `OCCLUSION_SIZE = 8` (gi.cpp:3643) | - |
| 5.04 When `scroll != 0`, regions that do not touch a newly dirty slab are skipped; their values come from SCROLL_OCCLUSION. | pre.glsl:495-505 | - | - |
| 5.05 Phase 1: load the 16^3 facing masks into shared memory; "solid" means `facing != 0`; outside the grid is empty. | pre.glsl:509-527 | - | - |
| 5.06 Phase 2: shells of equal `\|x\|+\|y\|+\|z\|` (0..7 per axis, 22 steps, `OCC_STEPS = 8*3-2`) are processed from the probe outward in all eight octants. A solid voxel gets 0; the shell next to the probe gets 1.0; any other free voxel takes the average of its up to three neighbours one step closer to the probe (a solid neighbour contributes 0 and still counts, except on the coordinate planes). Barrier after every shell. | pre.glsl:535-657 | Shell sizes 1, 3, 6, 10, 15, 21, 28, 36, 42, 46, 48 | - |
| 5.07 Phase 3 ("bias solid voxels away"): each solid voxel gets the average occlusion of its free neighbours on the probe side, only along directions it has facing bits for. | pre.glsl:659-795 | - | - |
| 5.08 Phase 4: a free voxel multiplies by `visible/occlude_total`, where the neighbours away from the probe that are solid count, and "visible" means their facing mask has the bit pointing back at the voxel. A voxel behind a wall seen from the probe goes toward 0. | pre.glsl:797-931 | - | - |
| 5.09 `block_mask` is overwritten with 0 at both uses, so its logic is dead; `#if 0` blocks at pre.glsl:564-581, 684-693. | pre.glsl:827, 878 | - | - |
| 5.10 STORE packs the eight float values to 4 bits each (`occ*15`, truncated) with shifts `{12, 8, 4, 0, 28, 24, 20, 16}`; the low 16 bits go to `(x, y, z + cascade*128)` and the high 16 bits to `(x+128, y, ...)`. Viewed as R4G4B4A4: R is bits 12-15 (octant 0 or 4), G bits 8-11, B 4-7, A 0-3. | pre.glsl:973-987 | 256 x 128 x (128 x cascades) | - |
| 5.11 Reader (screen pass, only if `use_occlusion`): class from `(probe_world_offset + probe) & 1` per axis (x bit 1, y bit 2, z bit 4); the point is clamped to within 0.9375 probe of the probe, scaled by 1/16, z bit selects the second half of x; value read trilinearly; `weight *= max(occlusion, 0.01)`. | gi.glsl:233-247; sdfgi_debug_probes.glsl:103-159 | Clamp = (8 - 0.5)/8 | - |

---

## 6. Direct light and bounce feedback

**What it is for.** Each solid voxel stores the light that leaves it. Static lights are baked into the solid-cell list when a region is rebuilt; dynamic lights, the sun, emission and bounced probe light are added every update by one compute shader over the solid-cell list. The result goes to the per-cascade light textures, which probe rays later read as hit radiance.

| Claim | Source | Numbers and formats | Where the lab differs |
|---|---|---|---|
| 6.01 Static pass (`render_static_lights`) runs only on frames where a cascade was rebuilt and a static light was culled into one; per dirty cascade it builds a `Light[1024]` array (breaks at 1024), filters again by `p_cascade_indices[i] > max_sdfgi_cascade` and `cascade_aabb.intersects(light_aabb)`, writes `lights_buffer` only if non-empty. | gi.cpp:2422-2541, 2442, 2461, 2465; cull.cpp:3661-3674 | `MAX_STATIC_LIGHTS = 1024` | - |
| 6.02 Static pass dispatch: `DIRECT_LIGHT_MODE_STATIC` (`MODE_PROCESS_STATIC`), `bounce_feedback = 0` ("do not multibounce yet"), `process_offset = 0`, `process_increment = 1`; indirect dispatch over the whole solid-cell list of the cascade, skipped when `light_count == 0`. | gi.cpp:2545-2608, 2571-2577, 2597-2603 | - | - |
| 6.03 Static pass writes no texture. It writes back into the solid cell: `light = (old & (3<<30)) \| light_total_rgbe` and rebuilt `light_aniso`; the start value of the accumulation is the cell's current light, so the net effect is `cell.light += static contribution`. Packed as RGBE8985 (loses the low bit of red and blue). | dl.glsl:293-311, 559-575, 488-490 | - | - |
| 6.04 Dynamic pass (`pre_process_gi`) builds per cascade `Light[128]`: directional lights first (all visible layer-matching ones, no bake-mode filter, no `max_sdfgi_cascade` filter, no AABB test; skipped only when sky mode is `SKY_ONLY`), then positional Dynamic lights that pass `i <= max_sdfgi_cascade` and the cascade-AABB test, until 128 slots are full. | gi.cpp:1825-2058, 1914-1960; cull.cpp:3355-3382 | `MAX_DYNAMIC_LIGHTS = 128` | - |
| 6.05 Light fields: directional `direction = -basis.z` with `dir.y *= y_mult` then normalize; positional `position.y *= y_mult`, direction unscaled and not renormalized; colour `srgb_to_linear`; `attenuation` = `LIGHT_PARAM_ATTENUATION` (Light3D default 1, so decay 1, not 2); `radius` = range (default 5); spot cos of `spot_angle` (45) and `1/spot_attenuation`. | gi.cpp:1930, 1980-1989; light_3d.cpp:490-497 | - | - |
| 6.06 Energy chain: `energy = light_energy * light_indirect_energy`; if physical light units: `* intensity`, omni `/ (4 pi)`, spot `/ pi`, directional unchanged, area unchanged (but `/ area` if `normalize_energy`); if camera attributes exist: `* exposure_normalization`. | gi.cpp:1941-1997, 2478-2496, 2010-2035; camera_attributes_storage.cpp:137-141 | Defaults: physical units off | - |
| 6.07 `has_shadow` is copied but never read by the shader: every light is traced against the SDF whatever its shadow checkbox says. | dl.glsl:69; gi.cpp:1930-2048 | - | - |
| 6.08 The same `lights_buffer` serves both passes; the dynamic fill overwrites the static fill each frame (the static dispatch was recorded earlier in `_update_sdfgi`). | gi.cpp:590, 2054-2058 | 112 bytes per `Light`, 1024 entries | - |
| 6.09 `update_light`: per cascade, copy counters to the call buffer; push `bounce_feedback` and `use_occlusion`; schedule: if `all_dynamic_lights_dirty` or setting index 0 then offset 0 / increment 1, else increment N from the table `{1,2,4,8,16}` and `offset = frame_number % N`. All cascades use the same offset in a frame; all cascades dispatch even with zero lights (needed for emission, static copy and bounce). | gi.cpp:1249-1309, 1286-1300 | Default N = 4: each solid cell refreshed every 4th frame | - |
| 6.10 `all_dynamic_lights_dirty` is initially true (gi.h:621) and set by `render_region`, so a rebuilt cascade is fully refreshed the same frame. | gi.cpp:2110 | - | - |
| 6.11 The dispatch is not shrunk for N > 1: `ceil(total/64)` groups; thread `t` handles cell `t*N + offset` and returns if it is at or above `total_count`; the rest exit at once. Unprocessed cells keep their old texels (a moving light lags up to N frames). | dl.glsl:182-188; pre.glsl:1043-1045 | - | - |
| 6.12 Per cell: decode position (`((positioni + 0.5) / to_cell) + offset`, y-stretched space), albedo (RGB555/31), `valid_aniso`; then (dynamic pass, bounce above 0.001) bounce; then add the stored static/emission light per face; then the light loop. | dl.glsl:183-311 | - | - |
| 6.13 Omni attenuation: `(1 - (d/r)^4)^2 * d^-decay`, with `decay = attenuation` (default 1). The line `rel_vec.y /= y_mult` is dead (`rel_vec` is not read after). | dl.glsl:125-132, 332, 340 | - | - |
| 6.14 Spot: after the omni term, `cos_angle = dot(-direction, light.direction)`; skip if below `cos_spot_angle`; `spot_rim = max(0.0001, (1 - scos)/(1 - cos_spot_angle))`; `attenuation *= 1 - pow(spot_rim, inv_spot_attenuation)`. The light's direction is the unscaled world direction while `direction` is in y-stretched space. | dl.glsl:336-353 | - | - |
| 6.15 Area light: skip if width or height is degenerate or the cell is behind the light; closest point on the rectangle; omni attenuation with `attenuation - 2` times an LTC diffuse integral, plus a projector-atlas colour. `cos_spot_angle` and `inv_spot_attenuation` are reused to carry the max mip and `1/(radius + \|area_size\|/2)`. | dl.glsl:354-365, 134-177; gi.cpp:2010-2035; area_lights_inc.glsl:343 | - | - |
| 6.16 Lights with `attenuation < 0.001` skip the shadow march. | dl.glsl:368 | - | - |
| 6.17 Shadow ray start bias replaces any normal offset: `ray_pos += sign(direction) * cell_size * 0.48` per component, then `ray_pos += ray_dir * 0.4 * cell_size`; `normal_bias` is not used here. | dl.glsl:378-381 | Up to 0.88 cell along an axis, about 1.23 along the cell diagonal | - |
| 6.18 Shadow march: loop `j = params.cascade ... max_cascades-1` (own cascade and coarser only). In cascade `j`: cell coordinates, skip if outside `[0,128)`, slab test for the exit distance, `max_advance = min(local_distance, exit)`; step = `texture(sdf).r*255 - 1` cells; `if (distance < 0.001)` hit and break (evaluated before the step, so a solid or negative sample ends the march); `advance += distance`. No iteration cap, no minimum step. | dl.glsl:383-434, 408-417 | Hit threshold 0.001 cell | - |
| 6.19 When the ray leaves cascade `j` without a hit, it continues from the exit point in cascade `j+1`, subtracting the advanced distance from `light_distance`; the bias offset is not subtracted, so the march overshoots the light by about the bias. Leaving the last cascade means lit: occluders beyond the outer cascade, or not voxelized, cast no shadow. Directional light distance is 1e20, so the ray runs to the box exits. | dl.glsl:425-434 | - | - |
| 6.20 Shadows are binary: the `occlusion` variable is computed but only the `hit` flag is used. | dl.glsl:386-444 | - | - |
| 6.21 Accumulation: if not hit, `light = albedo * colour * texture_color * energy * attenuation`; for each of six faces the cell actually exposes (`valid_aniso` bit), `light_accum[j] += max(0, dot(aniso_dir[j], direction)) * light`. No 1/pi. | dl.glsl:436-444 | - | - |
| 6.22 Pack: `lumas[i] = max channel of light_accum[i]`, `light_total = sum`, `luma_total = max channel of the total`; RGBE encode with `c` clamped to [0, 65408]; dynamic path stores E5B9G9R9 layout (`r \| g<<9 \| b<<18 \| e<<27`) into `light_data`; `aniso0 = (l0..l3)/luma_total`, `aniso1 = (l4, l5)/luma_total` (UNORM, relative weights; they do not sum to 1). The static path stores RGBE8985. | dl.glsl:447-574, 461-495 | - | - |
| 6.23 Neighbour fill (dynamic pass): the same light and aniso values are stored at each of the cell's set neighbour bits so trilinear sampling near a surface is lit; empty cells never get their own light computation. | dl.glsl:497-557 (comment 514) | Neighbour table at dl.glsl:520-546 | - |
| 6.24 Bounce (dynamic pass only, if `bounce_feedback > 0.001`): `feedback = bounce < 1 ? albedo*bounce : mix(albedo, 1, bounce - 1)`; the cell's position in probe units is `(positioni + 0.5) * 16/128`; for each of 8 surrounding probes and each valid face `k`: `weight = trilinear * max(0, dot(aniso_dir[k], probe_dir))` (times occlusion if on), sample the irradiance layer (layer = cascade) by `octahedron_encode(aniso_dir[k])*6`; after the eight probes `light_accum[k] = (sum / weight_accum) * feedback`. | dl.glsl:216-292 | At 0.5: half the albedo; at 1.99 nearly white | - |
| 6.25 The bounce term is added before the static term and the lights, so it is packed like direct light. The probe texture read is the one stored at the end of the previous frame (`update_light` runs before `update_probes`), plus an extra STORE after a scroll when `bounce_feedback > 0`. | dl.glsl:215; gi.cpp:1429-1446, 2193-2212 | One bounce per round trip, damped by albedo times bounce | - |
| 6.26 The voxel light texture holds emission + static light + dynamic light + bounce, summed over faces; it is the hit radiance that probe rays and the reflection march read. | dl.glsl; int.glsl:249-269 | - | - |
| 6.27 `luma_total == 0` gives `0/0 = NaN` in the aniso ratios (dynamic path stores to UNORM images, static path does `uint(NaN)`); there is no guard. The same cell's light is exactly 0, so the effect is probably harmless; not verified on hardware. | dl.glsl:500-506, 569; int.glsl:261-266 | - | - |
| 6.28 Emission enters the light word at voxelization (4.16) and is the starting value of both passes; it is re-added on every dynamic update. | pre.glsl:1030-1034; dl.glsl:295-311 | - | - |

### 6.A Worked example: one omni light, one solid cell (dynamic pass)

Assumptions (not engine defaults except where noted): cascade 0, cell size 0.2 m (`to_cell = 5`); floor voxel, `valid_aniso = 0b000010` (+Y only); albedo 0.5, quantized `uint(0.5*31) = 15`, so 15/31 = 0.48387; Light3D defaults (range 5, attenuation 1, energy 1, indirect 1, white); the light is 2.0 units straight above the cell centre in y-stretched space; nothing blocks it; no emission, static, bounce, physical units or exposure.

1. CPU: light survives the cascade-AABB test; stored with colour 1, energy 1, attenuation 1, radius 5.
2. Shader: `direction = (0,1,0)`, `light_distance = 2`. `nd = (2/5)^4 = 0.0256`; `(1 - 0.0256)^2 = 0.94946`; times `2^-1` = 0.47473.
3. Ray start: `0.2*0.48 = 0.096` plus `0.4*0.2 = 0.08`, so 0.176 above the centre. `local_distance = 2*5 = 10` cells. Open space: the march reaches 10 without a sample under 0.001: lit.
4. `light = 0.48387 * 0.47473 = 0.22971`; +Y face only: `light_accum[1] = 0.22971`.
5. `lumas = [0, 0.22971, 0, 0, 0, 0]`, `luma_total = 0.22971`. Pack: `floor(log2(0.22971)) = -3`, `expp = 13`, `sMax = floor(0.22971 / 2^-11 + 0.5) = 470 < 512`, so `exps = 13`, `rgbe = 470 \| 470<<9 \| 470<<18 \| 13<<27`, decoded 0.22949.
6. `aniso0 = (0,1,0,0)`, `aniso1 = (0,0)`; stored at the cell and at each cell whose neighbour bit is set.
7. With bounce 0.5 and probe irradiance 0.1 at +Y: `feedback = 0.2419`, `light_accum[1] += 0.0242`, so `luma_total = 0.2539`.

---

## 7. Probes: tracing and history

**What it is for.** Every probe of every cascade shoots a few rays per frame through the cascades' SDFs, reads the voxel light where a ray hits (or the sky where it escapes), and projects the result into 16 spherical-harmonic coefficients. A ring of 30 frames of coefficients is averaged, so a few rays per frame add up to a 240-ray estimate.

| Claim | Source | Numbers and formats | Where the lab differs |
|---|---|---|---|
| 7.01 `update_probes` has one call site, in `_post_prepass_render`, which is called only inside `if (depth_pre_pass)` and `if (needs_pre_resolve)` (true iff SDFGI custom data exists). If `rendering/driver/depth_prepass/enable` is off and no opaque stencil forces the pre-pass (stable's only forcing condition, rfc.cpp:2112), probes never update; in stable SSAO then is off too (`using_ssao` requires `depth_pre_pass`). The beta also forces the pre-pass for SSAO, SSR, SSIL and contact shadows. | renderer_scene_render_rd.cpp:1348-1356; rfc.cpp:2112-2133; scene_shader_forward_clustered.cpp:408 | Default `depth_prepass/enable = true` | - |
| 7.02 Push constants: `grid_size 128`, `max_cascades`, `probe_axis_size 17`, `history_index = render_pass % history_size` (computed before `render_pass++`, so it counts probe updates, not frames), `ray_count`, `ray_bias = probe_bias`, `image_size = (289, 17)`, `y_mult`, sky parameters. | gi.cpp:1311-1412, 1315-1335, 1391 | Default `history_size` 30, 8 rays | Lab will choose its own counts |
| 7.03 Ray count table `{4, 8, 16, 32, 64, 96, 128}[sdfgi_ray_count]`; default index 1 gives 8 rays per probe per frame (the editor label of index 1 is "16"). | gi.cpp:1322-1323; rendering_server.cpp:3793 | 4913 x 8 = 39,304 rays per cascade per frame | - |
| 7.04 Dispatch: for each cascade, `world_offset = cascade.position / 8`, `compute_list_dispatch_threads(289, 17, 1)`, local size 8x8x1; one thread per probe traces its rays serially. All cascades in one list, no barriers. | gi.cpp:1394-1408; int.glsl:9, 158 | 289 = 17*17 columns (z slices side by side) | - |
| 7.05 Probe position: `probe_cell = (pos.x % 17, pos.y, pos.x / 17)`; `probe_pos = cascade.offset + probe_cell * (8 * cell_size)`, in y-stretched space. | int.glsl:166-173 | Probes at cell coordinates 0, 8, ..., 128 | - |
| 7.06 Ray directions: Vogel spiral. `idx = history_index + i*history_size`; `r = sqrt(idx + 0.5)/sqrt(ray_total)`; `theta = idx*GOLDEN_ANGLE + offset`; `dir = (sin(r*pi/2) cos(theta), sin(r*pi/2) sin(theta), cos(r*pi/2) * ((idx&1)*2 - 1))`; then `dir.y *= y_mult` and normalize. `offset = hashf3(hash3(world_offset + probe_cell) & 0xFFFFF)`: fixed per world probe, no time input, so a scrolled probe keeps its rotation. | int.glsl:89-95, 181-192 | `GOLDEN_ANGLE = 2.39996`; `ray_total = history_size * ray_count` (240 default) | - |
| 7.07 Over `history_size` frames the indices `h + i*H` cover `0..H*N-1` exactly once, so the ring forms one complete Vogel set. | int.glsl:185-190 | 30 x 8 = 240 rays per probe | - |
| 7.08 The sphere is covered by parity: even `idx` points to -z, odd to +z. With even `H` (10, 20, 30) every ray in one frame has the parity of the frame slot, so a whole frame casts into one hemisphere only; with odd `H` the rays alternate. | int.glsl:89-95, 185-190 | Default H = 30: even frames -z, odd frames +z | - |
| 7.09 Distribution: polar angle is linear in `r`, not uniform in solid angle: density per solid angle is proportional to `phi/sin(phi)`, 1x at the pole and about 1.57x at the equator per hemisphere. The pole axis is local z (world z). No cosine weighting; the cosine lobe is applied by the SH convolution. | int.glsl:89-95 | - | - |
| 7.10 Ray start: `ray_pos = probe_pos + dir / max(abs(dir)) * bias / to_cell`, i.e. `ray_bias` (1.1) cells along the dominant axis (L-infinity), in the probe's own cascade. | int.glsl:201-203 | A cube of half-extent 1.1 cells | - |
| 7.11 March: loop `j = params.cascade .. max_cascades-1`; skip if outside `[0,128)`; slab test for the exit distance; `while (advance < max_advance)`: `distance = texture(sdf_cascades[j], uvw).r*255 - 1`; `if (distance < 0.05)` hit; else `advance += distance`. On exit without a hit, the exit point becomes the origin in cascade `j+1`, with no new bias. After the last cascade: miss. No iteration cap. | int.glsl:206-247, 228 | Hit threshold 0.05 cell | - |
| 7.12 A hit reads only from cascade `hit_cascade` (to keep texture-array access uniform): `hit_normal = normalize(gradient of the R8 SDF)` by central differences with `EPSILON = 0.001` in uvw; `hit_light = texture(light_cascades[hc], uvw)`; `aniso0.rgb` = weights for +x, +y, +z, `(aniso0.a, aniso1.rg)` for -x, -y, -z; `light = hit_light * (dot(max(0, n*aniso0), 1) + dot(max(0, -n*aniso1), 1))`. No BRDF, cosine to the probe or distance falloff; albedo is already in the stored light. | int.glsl:249-269 | The gradient picks face weights by sign | - |
| 7.13 A miss: sky flag first (`SKY_FLAGS_MODE_SKY 0x02`): rotate the ray by the inverse sky orientation and read the sky radiance octmap at mip 2 times `sky_energy` ("because we don't usually throw a lot of rays"); else colour flag: `sky_colour * sky_energy`; else 0. | int.glsl:271-291; gi.cpp:1333-1389 | - | - |
| 7.14 Sky source by background mode, only if `read_sky_light`: clear colour (`srgb_to_linear` of the default clear colour), custom colour (no conversion), sky with valid radiance; any other background, or a sky without radiance, gives black. | gi.cpp:1337-1389 | `USE_RADIANCE_OCTMAP_ARRAY` selects the sampler | - |
| 7.15 SH accumulation: per ray `sh[m].rgb += light.rgb * Y_m(d)` for 16 real SH coefficients (bands l = 0..3), in a per-thread `shared SH sh_accum[64]` (64 x 48 floats = 12 KiB). Coefficient order: 0; 1 = y, 2 = z, 3 = x; 4..8; 9..15 (l = 3). The constants are the standard real SH constants (0.282095, 0.488603, 1.092548, 0.315392, 0.546274, ...); z is the m = 0 axis for l = 2, 3. The `SH_SIZE != 16` branches are dead; the comment "l4 does not contribute" actually refers to l = 3. | int.glsl:293-321, 89-152, 401 | `SH_SIZE 16` (gi.cpp:3668) | - |
| 7.16 Fixed-point history: for each coefficient, `value = vec4(sh.rgb, 1) * 4.0 / ray_count`; `ivalue = clamp(ivec4(value*1024), -32768, 32767)`; `average -= old_layer; average += ivalue`; store the layer and the average. The average texture is the exact integer sum of the ring (no drift). | int.glsl:323-347 | Scale 1024 (`HISTORY_BITS 10`), range +-32; row `y*16 + i`, column `x + 17*z`, layer `history_index` | - |
| 7.17 Ambient texture (only when volumetric fog is enabled, coefficient 0): `ambient = (average/H/1024) * 0.88622` into an RGBA16F layer per cascade. | int.glsl:341-346; gi.cpp:1327 | 0.88622 = sqrt(pi)/2; equals the mean incoming radiance | - |
| 7.18 History mechanism is a sliding-window box filter, not an exponential average: after `k < H` frames from zero the mean is about `k/H` of the converged value; a change in lighting is fully visible after H frames (mean latency `(H-1)/2`). | int.glsl; doc/classes/ProjectSettings.xml:3066-3068 | 30 frames by default | - |
| 7.19 Normalization as coded: stored coefficient `(4/N) * sum(L * Y_i)`; STORE multiplies by 4 and by `l_mult`; for a uniform sky of radiance `L` the irradiance texel is `(4/pi) L = 1.273 L`, not `L`. With the real (non-uniform) ray set the Python check gives, at y_mult 1.5: +y about 1.58 L, +z about 1.06 L, +x about 1.17 L, mean over normals about 1.27 L; at y_mult 1.0: +y/+x 1.32 L, +z 1.18 L. | int.glsl:328, 415-426 | `l_mult = {1, 2/3 x3, 1/4 x5, 0 x7}` | - |
| 7.20 STORE of the probe maps (section 8) multiplies the same constants; the reader applies no `1/1.273` correction. | gi.glsl:478-479; scene.glsl:2185 | - | - |

### 7.A Worked example: one probe, one ray (defaults; Python re-implementation of the formulas)

Settings: H = 30, N = 8, cell 0.2 m, y_mult 1.5, bias 1.1. Assumed (not engine values): `probe_cell = (3,5,2)`, thread `(37, 5)`, hash offset 0, frame slot `h = 1`, ray `i = 1`, the other 7 rays and 29 frames contribute 0, and the ray hits a surface giving `hit_light * aniso = (0.6, 0.5, 0.4)`.

1. `probe_pos = offset0 + (3,5,2) * 1.6 = offset0 + (4.8, 8.0, 3.2)` m.
2. `idx = 1 + 1*30 = 31`, `ray_total = 240`; `r = sqrt(31.5)/sqrt(240) = 0.36228`; `phi = 0.56908`; `theta = 31*2.399963 = 5.2838 rad` (mod 2 pi); odd, so +z: `dir = (0.29143, -0.45324, 0.84240)`; `y*1.5`, normalize: `(0.25996, -0.60645, 0.75143)`.
3. Start offset: `dir / 0.75143 * 1.1 = (0.3806, -0.8878, 1.1)` cells = `(0.0761, -0.1776, 0.22)` m. Suppose a hit in cascade 0 with shaded value (0.6, 0.5, 0.4).
4. `Y = [0.2821, -0.2963, 0.3671, 0.1270, -0.1722, -0.4979, 0.2189, 0.2134, -0.1640, 0.0591, -0.3424, -0.7147, -0.0496, 0.2166, -0.3260, -0.1589]`. `value_0 = (0.6,0.5,0.4) * 0.2821 * 0.5 = (0.0846, 0.0705, 0.0564)`, `ivalue = (86, 72, 57)`, alpha `int(0.5*1024) = 512`; `i = 1`: (-91, -75, -60); `i = 2`: (112, 93, 75).
5. STORE for texel (3,3): `normal = (0,0,1)`; `irradiance = (0.009285, 0.007739, 0.006178)`; `radiance = (0.014330, 0.011995, 0.009507)`.
6. RGBE of the irradiance: exponent 9, mantissas (304, 254, 202), word `0x4B29FD30` (decodes to 0.009277, 0.007751, 0.006165), written to layer 0 at texel (300, 44); the radiance word goes to layer `0 + max_cascades` (4 by default).

---

## 8. Probe storage and scrolling

**What it is for.** The averaged SH of each probe is turned into two small octahedral maps (irradiance and radiance) that the screen pass can sample with hardware filtering. When a cascade scrolls, the probe history moves with it, and newly exposed probes are seeded from the coarser cascade.

| Claim | Source | Numbers and formats | Where the lab differs |
|---|---|---|---|
| 8.01 `store_probes` runs every frame GI is used (rfc.cpp:1534, in `_pre_opaque_render`), independent of whether the depth pre-pass ran; it re-encodes the current averages. | rfc.cpp:1534, 2182 | - | - |
| 8.02 Dispatch per cascade: `dispatch_threads(289*6, 17*6, 1)`, one thread per octahedral texel (1734 x 102). | gi.cpp:1414-1456 | 6x6 texels per probe | - |
| 8.03 Per texel: `local_pos = pos % 6`; `normal = octahedron_encode(vec2(local_pos)/6)` (no +0.5); SH average `(average/H)/1024`; `m = sh * c[i](normal) * 4`; `irradiance += m*l_mult[i]`; `radiance += m`. | int.glsl:350-426, 108-116 | `l_mult = {1, 2/3 x3, 1/4 x5, 0 x7}` | - |
| 8.04 The encode is the Vulkan E5B9G9R9 shared-exponent word: clamp each channel to [0, 65408] (negative SH ringing becomes 0), 9-bit mantissas, `r \| g<<9 \| b<<18 \| e<<27`. | int.glsl:118-144 | - | - |
| 8.05 `lightprobe_data` is R32_UINT, 2D array, width `17*17*8 = 2312`, height `17*8 = 136`, `2*cascades` layers; sampled through an `E5B9G9R9_UFLOAT` view. Layer `c` = irradiance of cascade `c`; layer `cascades + c` = radiance. Block origin of probe `p`: `x = (p.x + p.z*17)*8`, `y = p.y*8`; the 6x6 interior starts at +1. | gi.cpp:511-525; int.glsl:469-470; gi.glsl:204-206 | 8x8 block per probe = 6x6 + 1 texel border | - |
| 8.06 Border: each interior texel is written by its own thread to itself and 1 to 3 mirrored border locations across the octahedral fold: `y==0` to `(N-1-x, -1)`, `x==0` to `(-1, N-1-y)`, `y==N-1` to `(N-1-x, N)`, `x==N-1` to `(N, N-1-y)`; corners write three border corners. | int.glsl:436-470 | N = 6 | - |
| 8.07 Half-texel offset: the writer stores the direction for `u = k/6` in texel `k` (centre at `k + 0.5`); the reader (gi.glsl:208) asks for the same direction at coordinate `tex_pos + uv*6`, a texel corner. So a lookup returns about the 50/50 blend of texels `k-1` and `k`: data shifted by half a texel (1/12 of the map). The border exists for exactly this sample range. | int.glsl:356-361; gi.glsl:141-147, 206-208 | Corner direction -z is consistent (all four corners are the same direction) | - |
| 8.08 History and average textures per cascade: `R16G16B16A16_SINT` 289 x 272 x `history_size` layers and `R32G32B32A32_SINT` 289 x 272, cleared at creation; shared scratch textures of the same shape are used for scrolling. | gi.cpp:486-538, 573-583 | Row `probe.y*16 + coefficient` | - |
| 8.09 Probe scroll runs inside `render_region` only when `dirty_regions != DIRTY_ALL`: `INTEGRATE_MODE_SCROLL` (dispatch 289 x 17), barrier, `INTEGRATE_MODE_SCROLL_STORE`, barrier, and `INTEGRATE_MODE_STORE` for that cascade if `bounce_feedback > 0` ("multibounce requires this to be stored so direct light can read from it"). Push constants: `history_index = 0`, `ray_count = 0`, `scroll = dirty / 8`, `world_offset = position/8`. | gi.cpp:2117-2212, 2144-2174; int.glsl:475-620 | `scroll` is one probe per 8-cell step | - |
| 8.10 With `DIRTY_ALL` nothing is scrolled: the probe textures keep what they had (zeros after creation) and re-converge. | gi.cpp:2096-2117 | - | - |
| 8.11 SCROLL (one thread per destination probe): `read_probe = probe_cell - scroll`; if inside `[0,17)^3`, copy all history layers (16 rows each) and the 16 average rows from the old to the new position. | int.glsl:482-507 | - | - |
| 8.12 Otherwise, unless this is the last cascade, the probe is newly exposed: initialize it from the next coarser cascade's average texture (binding 13): world position to parent probe coordinates, trilinear blend of 8 parent probes, convert each average to a mean (`/H/1024`), requantize, write into every history layer and set `average = value * history_size`. A newly exposed probe is therefore a coarse estimate rather than black. No bounds check on the parent indices. | int.glsl:508-570; gi.cpp:1098-1109 | - | - |
| 8.13 For the last cascade, a probe not coming from inside "just copies what is there": the load uses the destination texel, so stale data stays until it re-converges. | int.glsl:572-596 | - | - |
| 8.14 SCROLL_STORE copies the scroll scratch textures back (double buffering because SCROLL reads neighbours' old data while writing). The ring position `render_pass % H` is unchanged, so ray stratification stays aligned with each layer. | int.glsl:600-620 | - | - |
| 8.15 Parent-index bound (needs re-check, recomputed here). With `drag_margin = 4` and 8-cell steps, `Delta = P_i - 2*P_{i+1}` in child cells satisfies `abs(Delta) <= about 12 to 14` (camera within 4 child cells of `P_i` and within 4 parent cells of `P_{i+1}`). Parent probe coordinate `q = (Delta + 64 + 8p)/16`; at the new edge `p = 16` this is `12 + Delta/16`, between about 11.1 and 12.9, so the indices `floor(q)` and `floor(q)+1` stay at or below 13: in range 0..16. A single scroll step is `scroll.x = -1` probe. (The first report assumed a drag margin of 8, a step of 16 cells and `scroll.x = -2`; those numbers are wrong and its bound is superseded.) | int.glsl:512-522, 533-541; gi.cpp:1199 | - | - |

### 8.A Octahedral lookup (reader side, same maps)

The reader computes `n /= \|n\|_1; if n.z < 0, n.xy = (1 - \|n.yx\|) * sign(n.xy); uv = n.xy*0.5 + 0.5` and samples at `tex_pos + uv*6` in texel coordinates, with bilinear filtering (gi.glsl:141-147, 206-208). Texel (0,0) is the direction (0,0,-1); the reader's (0,0,-1) lookup lands on coordinate `tex_pos + 6`, which is the border copy of that same value.

---

## 9. Reading it: the screen GI pass

**What it is for.** After the depth and normal-roughness prepass, one compute shader reconstructs each pixel's position and normal, picks the finest cascade that contains it, blends 8 probes, and writes AMBIENT and REFLECTION images whose alpha says how much to trust SDFGI. The opaque shader mixes the images with its sky ambient and reflection.

| Claim | Source | Numbers and formats | Where the lab differs |
|---|---|---|---|
| 9.01 `GI::process_gi` is called from `_pre_opaque_render` only when SDFGI or VoxelGI is in use (`render_gi`, `p_use_gi = using_sdfgi \|\| using_voxelgi`); at most 2 views. `using_sdfgi` needs SDFGI on and a viewport debug mode other than Unshaded. | gi.cpp:3968, 3972; rfc.cpp:1580, 1606-1607, 1876-1877, 2182 | - | - |
| 9.02 Output buffers `RB_TEX_AMBIENT` and `RB_TEX_REFLECTION`, `R16G16B16A16_SFLOAT`, storage and sampling, at the render size, or at half size per axis if `use_half_resolution`; in half mode the shader samples every second pixel (`pos <<= 1`, no averaging). | gi.cpp:3991-3997, 4267-4269; gi.glsl:43-44, 707-709 | Default full size | - |
| 9.03 Shader variants: `MODE_SDFGI` (`USE_SDFGI`), `MODE_COMBINED` with VoxelGI; specialization constants for half-res, full projection (multiview), VRS (tile replicate in `main`). | gi.cpp:3716-3753, 4063-4088; gi.glsl:17-19, 681-760 | 8 specializations per mode | - |
| 9.04 Per pixel: `vertex = reconstruct_position(pos)` from depth (view space, perspective or orthographic); normal and roughness by `texelFetch` of the normal-roughness buffer (normal `xyz*2-1`, roughness decoded `if r > 0.5, r = 1 - r; r /= 127/255`); `view`, `vertex` and `normal` rotated by `cam_transform` into world orientation with the camera at the origin; `reflection = reflect(-view, normal)`. | gi.glsl:161-192, 609-631, 677-727; scene.glsl:3020-3026 | The roughness passed on is linear (perceptual) roughness | - |
| 9.05 The test `normal.length() > 0.5` is always true (GLSL `length()` on a vec3 returns the component count, 3), so sky pixels are processed too. | gi.glsl:620 | - | - |
| 9.06 `sdfgi_process`: `vertex.y *= y_mult; normal.y *= y_mult; reflection.y *= y_mult`, then renormalize. | gi.glsl:290-296 | Default 1.5 | - |
| 9.07 Cascade selection: for `i = 0..max_cascades-1`, `cascade_pos = (cam_pos - cascades[i].position) * to_probe`; skip if any component is below 0 or at/above 16; the first hit is the finest cascade containing the point. None: ambient and reflection are `vec4(0)` (alpha 0: sky is used). | gi.glsl:311-324, 480-483 | - | - |
| 9.08 Blend band toward the next cascade, from the camera-relative position: `blend_from = 16/2 - 2.5 = 5.5`, `blend_to = 7.5` probe units; `len = max(abs(x), abs(y), abs(z))` of `cam_pos*to_probe`; `blend = smoothstep(5.5, 7.5, len)`. Last cascade: alpha = `1 - blend` (fades to sky). Otherwise recompute in cascade+1 and `mix` the two fully computed results (16 probe fetches). | gi.glsl:334-366 | The band ends 0.5 probe inside the 8-probe half-width because the cascade can be off-centre by up to 0.5 probe (drag margin 4 cells) | - |
| 9.09 `sdfvoxel_gi_process`: `cascade_pos += cam_normal * normal_bias` (the point is moved off the surface before the lattice cell is chosen), `base = floor(cascade_pos)`, then 8 probes with `trilinear = 1 - abs(probe_to_pos)`, `probe_dir = normalize(-probe_to_pos)` and `weight = trilinear.x*trilinear.y*trilinear.z * max(0.005, dot(cam_normal, probe_dir))`, times `max(occlusion, 0.01)` if on. | gi.glsl:194-286, 195-229, 246 | `normal_bias` 0.1375 probe units (1.1 cells); the `max(0.005, ...)` term suppresses probes behind the surface 200x | - |
| 9.10 Diffuse: sample the irradiance layer (layer = cascade) of each probe at `tex_pos + octahedron_encode(cam_normal)*6`, times `exposure_normalization`; weights are summed and the sums divided by the weight total (specular uses the same total). If the biased point lies exactly on a probe, `normalize(0)` is NaN (not observed in the example). | gi.glsl:204-208, 249-257, 268-283 | `diffuse_posf` by `lightprobe_tex_pixel_size` | - |
| 9.11 Specular from probes: `roughness < 0.99` reads the radiance layer (`c + max_cascades`) at the reflection direction; `roughness > 0.2` mixes in the irradiance layer at the reflection direction by `(roughness - 0.2) * 1.25`. | gi.glsl:259-272 | Full irradiance at roughness 1.0 | - |
| 9.12 For `roughness < 0.2`, an SDF march in the cascades' own SDF and light volumes is mixed with the probe result. `blend_size = 4` cells, `radius_sizes[i] = (1/to_cell_i) * (64 - 4)` = 60 cells of cascade i (12, 24, 48, 96 m); start cascade = first with `length(cam_pos) < radius`; `max_distance = radius_sizes[last]`. | gi.glsl:370-400 | 60 cells per cascade | - |
| 9.13 March start: `bias = (1 + base_blend) * 1.1` with `base_blend = (dist - prev_radius)/(radius - prev_radius)`; `ray_pos += (dir/max3(abs(dir)) + cam_normal*1.4) * bias / to_cell`; `softness = 0.2 + min(1, roughness*5) * 4.0` cells. | gi.glsl:393-401 | Softness 0.2 at r=0, 4.2 at r=0.2 | - |
| 9.14 March loop (`while (true)`): exits only when `length(ray_pos) >= max_distance` or `light_accum.a > 0.99`; each pass takes one SDF step in the first cascade `i >= cascade` whose radius contains the ray (`found` flag, "never go down"); `fdistance = texture(sdf).r*255 - 1.1`; if `fdistance < softness`, `hit = light.rgb*0.5` with `hit.a = clamp(1 - fdistance/softness, 0, 1)`; mix with cascade i+1 by `blend = clamp((\|ray\| - prev)/(radius - prev), 0, 1)`; `light_accum += hit` (plain addition); `ray_pos += ray_dir * fdistance` unconditionally. | gi.glsl:404-459 | The factor 0.5 is a stated approximation ("value read is actually meant for anisotropy") | - |
| 9.15 The step can be negative (solid texel gives -1.1 cells), but then `hit.a = 1`, so the next pass exits: a negative step is applied at most once, as the last step, at most 1.1 cells backward. In free space every step advances at least 0.2 cell. No iteration counter. | gi.glsl:405, 415-423, 451 | - | - |
| 9.16 Result: `light = rgb / max(a, 1e-5)`, `alpha = min(1, a)`, `b = min(1, roughness*5)`, `sa = 1 - b`, `reflection.a = alpha*sa + b`, `specular = (light*alpha*sa + specular*b) / reflection.a`. At roughness 0 a miss gives alpha 0 (pure sky); at 0.1 a miss gives 50% probe radiance and 50% sky; at 0.2 pure probe radiance. For `r < 0.2` in the last cascade the edge fade `1 - blend` is lost from the reflection alpha (the ambient alpha keeps it). | gi.glsl:461-476 | - | - |
| 9.17 `ambient_light.rgb *= energy` and `reflection_light.rgb *= energy` (default 1.0) at the end; opaque screen pass only. | gi.glsl:478-479 | - | - |
| 9.18 With VoxelGI instances present, VoxelGI is composited over SDFGI with `blend_color` (src-over by VoxelGI alpha, 1 unless `blend_ambient`). | gi.glsl:637-673, 149-159 | - | - |
| 9.19 Opaque forward shader: baseline `ambient_light` is the environment ambient colour, optionally mixed with the roughest radiance mip (`ambient_color_sky_mix`); then, if `!sc_use_forward_gi()` and `INSTANCE_FLAGS_USE_GI_BUFFERS`: `coord = screen_uv` (with MSAA: best of 5 taps by normal dot); `ambient_light = mix(ambient_light, buffer_ambient.rgb, buffer_ambient.a)`; same for `indirect_specular_light` with the reflection buffer. | scene.glsl:1735-1762, 1982-2025 | Non-MSAA path is plain bilinear (also the upsample at half res); no depth-aware upsampling | - |
| 9.20 Then: reflection probes accumulate over SDFGI (a probe covering the pixel overrides SDFGI specular); `ambient_light *= ao` (SDFGI has no AO of its own besides optional probe occlusion, so SSAO darkens it); specular occlusion derives from the SDFGI-inclusive ambient luminance (`*2`, `*4`, clamped); `ambient_light *= albedo`; SSIL; `*= 1 - metallic`; summed with emission and direct light. No pi or 1/pi anywhere on this chain. | scene.glsl:2038-2195, 3056, 3074-3090 | - | The lab's own scene shading reads probes directly (section 5) |

### 9.A Worked example: one pixel

Assumptions: defaults (4 cascades, 0.2 m, y_mult 1.5, normal bias 1.1, occlusion off, energy 1.0, full-res), camera at the centre of its cascades, a wall facing the camera with point P 11.0 m ahead in +x and 0.5 m up, normal `N = (-1,0,0)`, roughness 0.5.

1. After y_mult: `vertex = (11, 0.75, 0)`, `N` unchanged.
2. Cascade 0: probe spacing 1.6 m, `to_probe = 1/1.6`, `position = -12.8` on every axis; `cascade_pos = (23.8, 13.55, 12.8)/1.6 = (14.875, 8.469, 8.0)`: inside [0,16), so cascade 0.
3. Blend: `inner_pos = (6.875, 0.469, 0)`, L-infinity 6.875; `t = (6.875 - 5.5)/2 = 0.6875`; `smoothstep = 0.768`, so the result is 23% cascade 0 and 77% cascade 1.
4. Normal bias: `(14.875 - 0.1375, 8.469, 8.0)`; `base = (14, 8, 8)`; probes x in {14, 15}, y in {8, 9}, z in {8, 9}. Tent weights: x 0.2625 / 0.7375; y 0.531 / 0.469; z 1.0 / 0.0 (exactly on the lattice plane, so the four z = 9 probes get 0).
5. Normal-facing term: A (14,8,8) dot +0.844, weight 0.1176; B (14,9,8) dot +0.812, weight 0.0999; C (15,8,8) and D (15,9,8) are behind the wall plane (dot -0.49), clamped to 0.005, weights 0.00196 and 0.00173. Normalized shares: 53.2%, 45.2%, 0.9%, 0.8%.
6. Diffuse: oct uv of `N` is `(0, 0.5)`, times 6 gives (0, 3) on the left edge; probe A's texel origin is `((14 + 8*17)*8 + 1, 8*8 + 1) = (1201, 65)`, layer 0; the lookup is at (1201.0, 68.0, 0) with hardware bilinear mixing the border column with column 0.
7. Specular (roughness 0.5, no march): radiance layer `0 + 4` at the reflection direction, mixed with the irradiance layer by `(0.5 - 0.2)*1.25 = 0.375`.
8. Cascade 1: `cascade_pos = (11.4375, 8.234, 8.0)`; same procedure; result `mix(c0, c1, 0.768)`.
9. If the pixel were roughness 0.1: trace in cascade 0 (`11.03 < 12`), `base_blend = 0.919`, `bias = 2.11`, `softness = 2.2` cells; final = 50% trace + 50% probe radiance.
10. Compositing: ambient `mix(sky, diffuse, 1.0)`, reflection `mix(sky, specular, 1.0)`; then probes, SSAO, `*albedo`, `*(1 - metallic)`.

---

## 10. Transparents, fog and debug views

**What it is for.** The screen buffers hold the opaque surface, so a transparent surface cannot use them; it gets a reduced copy of the same lookup in its own fragment shader. Volumetric fog reads a different, non-directional texture. Two debug views show the voxel light and the probes.

| Claim | Source | Numbers and formats | Where the lab differs |
|---|---|---|---|
| 10.01 `uses_forward_gi` is set only for surfaces in the alpha render list of an instance that uses SDFGI (`can_sdfgi`) or VoxelGI and is not lightmapped; it becomes a pipeline specialization. Other transparent surfaces take the GI-buffer branch and see the opaque pixel behind them. | rfc.cpp:1149-1153, 1034, 440; scene.glsl:1875-1945 | Inferred from flags, not run | - |
| 10.02 The forward path repeats the same logic inline: `cam_pos = mat3(inv_view_matrix) * vertex`, y_mult on position, normal and reflection, finest containing cascade, 8 probes with the same `max(0.005, dot)`, occlusion decode, oct UV and `smoothstep(5.5, 7.5, len)` blend. | scene.glsl:1878-1913; fwdgi.glsl:126-242 | Uses `indirect_normal` (bent normal if present) | - |
| 10.03 No SDF march ever in the forward path: `use_specular` is hard-coded true, so specular is always the probe maps, even at roughness 0. | scene.glsl:1916 | - | - |
| 10.04 Different thresholds: radiance layer if `roughness < 0.99`; cross-fade to the irradiance layer only for `roughness > 0.5` with weight `(roughness - 0.5)*2` (screen pass: 0.2 and *1.25). | fwdgi.glsl:199-204 | At roughness 0.3: pure radiance (screen pass 12.5% irradiance) | - |
| 10.05 No `sdfgi.energy` is applied in the forward path (`.energy` occurs only in gi.glsl), so the environment's SDFGI energy scales opaque surfaces only. | fwdgi.glsl; scene.glsl | - | - |
| 10.06 Forward blend at the last cascade is done in the shader: `diffuse = mix(diffuse, ambient_light, blend)`, `indirect_specular_light = mix(specular, indirect_specular_light, blend)`; otherwise `mix` with cascade+1; outside all cascades nothing is written (sky kept). The result then goes through the common code (reflection probes, AO, SSIL). | scene.glsl:1923-1943 | - | - |
| 10.07 Volumetric fog uses SDFGI when `volumetric_fog_gi_inject > 0.0001`, SDFGI is enabled and valid; variant `MODE_DENSITY + ENABLE_SDFGI`; binds `sdfgi_ubo`, `ambient_texture` and `occlusion_texture`. | fog.cpp:1031, 316, 1033-1061, 1120 | `gi_inject` default 1.0 | - |
| 10.08 `ambient_texture`: RGBA16F array, one layer per cascade, 17*17 x 17, one texel per probe, written from SH coefficient 0 only (`average_SH_0 * 0.88622`) and only when volumetric fog is on. It is non-directional. | gi.cpp:533-543, 1327, 1430, 2162; int.glsl:341-345 | - | - |
| 10.09 Fog read: per froxel `world_pos = mat3(cam_rotation) * view_pos`; finest containing cascade with the same bounds test; floor to 8 probes; weight is trilinear only (no normal term, no normal bias, no cascade blend); manual trilinear from nearest `texelFetch`; occlusion if on; result `ambient_total / weight`, `break` after the first cascade; `total_light += ambient_total * gi_inject`. `exposure_normalization` applies; `energy` does not. | fog.glsl:703, 725-798 | - | - |
| 10.10 Fog applies no `y_mult`: it is declared in the struct (fog.glsl:132) and never read, while the cascade `position` in the UBO is in y-stretched space; a froxel `dy` above the camera samples as `dy` where the cascade expects `1.5*dy`. | fog.glsl:132, 703, 733; gi.cpp:1882-1886 | Default 75%: pulled toward the camera's height by 1.5; no error at y scale 100% | - |
| 10.11 `SDFGI::debug_draw` (`VIEWPORT_DEBUG_DRAW_SDFGI`): per pixel, sphere-trace the camera ray through the SDF cascades from cascade 0 outward (`distance = sdf*255 - 1.7`, hit below 0.001), then show the voxel light (`light_cascades` times the aniso weights from the SDF gradient) as clamped sRGB: the light the probes gather, not the final shaded GI. | gi.cpp:1535-1673; sdfgi_debug.glsl:116-175; rfc.cpp:2556-2567 | - | - |
| 10.12 `SDFGI::debug_probes` (`VIEWPORT_DEBUG_DRAW_SDFGI_PROBES`): draws all 17^3 probes of cascade 0 only as spheres of radius 0.2 (y divided by y_mult) coloured with the irradiance map (layer 0) by the sphere normal; clicking selects a probe and draws a 16-voxel cloud coloured by its 4-bit occlusion value (drawn even if `use_occlusion` is off). | gi.cpp:1675-1777; sdfgi_debug_probes.glsl:103-237; rfc.cpp:2286-2293 | - | - |
| 10.13 `VIEWPORT_DEBUG_DRAW_GI_BUFFER` shows the ambient and reflection buffers. | rfc.cpp:2597-2603 | - | - |
| 10.14 Dead or unreachable pieces in the readers: `dynamic_object` unused (gi.glsl:624), local `blend` shadowing (330 vs 307), unused accumulators in `sdfgi_process` (gi.glsl:301-313; scene.glsl:1890-1902), the forward path's `use_specular == false` branches, `#if 1` in the debug shader (only the no-interpolation branch exists). | gi.glsl; scene.glsl; sdfgi_debug.glsl | - | - |

---

## Findings (behaviours that look like engine bugs or surprises)

All by reading, not run on a GPU. "Agreed by N reports" counts how many of the five research reports independently describe the behaviour.

1. **Static light double counted after a scroll.** SCROLL copies each kept cell's stored `light` (emission plus earlier static light) back into `render_emission` (pre.glsl:195-199) and STORE re-reads it (pre.glsl:1030-1034); the static pass then runs over every solid cell of the cascade (gi.cpp:2545-2603, `process_increment = 1`), starts from the cell's current light and adds each culled light again (dl.glsl:295-311, 559-575). A light that overlaps both old content and the new slab is counted again on kept cells at each scroll. It does not happen with `DIRTY_ALL`, nor when no static light touches a dirty region. Status: read from the code (agreed by 2 reports, verified by the main agent at pre.glsl:195-199, dl.glsl:295-311 and 563-572) and measured on the lab's CPU model: a floor cell near a static omni light of range 15 m holds 1x, 2x, 3x, 4x the light after creation and three scrolls of cascade 0 (selftest). Not run in Godot.
2. **Static-light list indexed by cascade id on store and by dirty-list position on read.** The cull stores lights at `sdfgi_cascade_lights[cascade_id]` (cull.cpp:3275); `static_lights_culled` checks `[i]` for `i < cascade_light_count` (cull.cpp:3662-3667); `render_static_lights` picks the cascade by id from the dirty list but reads the lights at `[i]` (gi.cpp:2435-2453). They agree only when the dirty cascades are exactly 0..n-1. If only cascade 2 scrolls, `[0]` is empty, `update_static` stays false and the light is silently dropped for that frame; if cascades 1 and 2 scroll, cascade 1's lights are applied to cascade 2 and cascade 2's own lights are never read. A camera walk reaches this often, because the finer cascade does not always move when the coarser one does (a walk to x = 2.6 m leaves cascade 0 where it was: `int(2.6 / 0.2)` is 12, not past 8 + 4). Status: read from the code (cull.cpp:3271-3275, 3662-3663, gi.cpp:2448) and measured on the CPU model: walking 2.22 m dirties only cascade 1, whose light is stored under slot 1 and read from slot 0, so it gets none (selftest). Not run in Godot.
3. **y_scale label vs voxel shape.** The 75% label gives `y_mult = 1.5` (`{2.0, 1.5, 1.0}`, gi.cpp:423-424), so the cascade's real height is 2/3 of its width and voxels are `cell_size/1.5` tall (gi.cpp:1468-1469). The label reads like "75% of the height" but it is a vertical squash of the grid by 1.5 (finer vertical resolution, shorter box). Status: by reading, not run. Agreed by 5 reports on the value of `y_mult`, 2 on the voxel shape.
4. **The probe SH estimator uses 4 instead of 4 pi.** Stored `(4/N) * sum(L Y_i)`, read `* 4 * l_mult` with `l_mult = A_l/pi` (int.glsl:328, 415-426): a uniform sky of radiance `L` gives `(4/pi) L = 1.273 L`, about 27% more sky ambient than Godot's own sky ambient (`albedo * L`) on the opaque path; with the default ray set and y_mult 1.5, up-facing surfaces get about 1.58 L. No compensation was found (gi.glsl:478-479; scene.glsl:2185). A deliberate artistic gain cannot be excluded. Status: read from the code (agreed by 2 reports) and measured on the lab's CPU model: with no geometry and a sky of radiance 1 at y scale 100 %, the centre probe of cascade 0 stores irradiance 1.32 (+x), 1.32 (+y), 1.18 (+z), 1.19 (-z), mean 1.25, after 30 frames (selftest). Not run in Godot.
5. **Half-texel offset between STORE and the octahedral read.** STORE writes the direction for `u = k/6` into texel `k` (int.glsl:356-361); the reader samples at `tex_pos + uv*6`, a texel corner, with bilinear filtering (gi.glsl:206-208). Lookups return about a 50/50 blend of two texels, shifting the map by a half texel (1/12 of the map). The effect is small because the signal is low frequency. Status: by reading, not run. Agreed by 2 reports.
6. **Fog ignores y_mult.** fog.glsl:132 declares it and never reads it; froxels sample the y-stretched cascades with unstretched y (fog.glsl:703, 733). With the default 75% the vertical lookup is wrong by a factor 1.5. Status: by reading, not run. Single report.
7. **`probe_ray_count` hint text vs table.** The editor hint lists 6 entries from "8" (rendering_server.cpp:3793); the code table has 7 entries from 4 (gi.cpp:1322-1323). Default index 1 is labelled "16" but gives 8 rays; index 0 is labelled "8" but gives 4. Status: by reading, not run. Agreed by 2 reports.
8. **`update_probes` runs only inside the depth pre-pass branch.** Its one call is in `_post_prepass_render`, inside `if (depth_pre_pass)` and `if (needs_pre_resolve)` (rfc.cpp:2120-2133). With `rendering/driver/depth_prepass/enable = false` and nothing forcing a pre-pass, the probes freeze while `store_probes` keeps re-encoding stale averages. `history_index` counts probe updates, not frames. Status: by reading, not run. Single report.
9. **Cascade AABB vs light AABB in different y spaces.** `cascade_aabb` is built in y-squashed space (`(position - 64) * cell_size`, no `1/y_mult`) and tested against the light's real-y world AABB (gi.cpp:1955-1957, 1976, 2442-2444, 2465). At `y_mult != 1` the y interval is 1.5x too tall and off-centre (over-inclusive). Status: by reading, not run. Single report.
10. **Stale cascade UBO in the probe-scroll fallback.** `render_region` (with the probe SCROLL shader, int.glsl:508-570) runs before `update_cascades` writes the UBO (rfc.cpp:1734 vs 1836); so in the frame a cascade scrolls the shader's `cascades.data[].offset` is the previous frame's. New-edge probes sample the parent about one scroll step (8 cells, one probe, 1.6 m for cascade 0) off. It is only an initial guess that re-converges in H frames. Status: by reading, not run; needs re-check (the first report assumed a 16-cell step). Single report.
11. **Transparents ignore `sdfgi.energy`.** The forward path never multiplies by the environment's SDFGI energy (fwdgi.glsl; gi.glsl:478-479). Status: by reading, not run. Agreed by 2 reports.
12. **Exposure possibly applied twice.** Light energies already include the current camera exposure (gi.cpp:1944-1946, 2494-2496), and gi.glsl:257 multiplies the diffuse result by `exposure_normalization = current / baked` (gi.cpp:1897-1901); light injected with the current factor is scaled again by `current/baked`, which is 1 only if exposure has not changed since the cascade was voxelized. Status: by reading, not run; intent unknown. Single report.
13. **No NaN guard in the aniso ratios.** `lumas[i] / luma_total` has no epsilon when `luma_total == 0` (dl.glsl:500-506, 569). The cell's light is exactly 0, so the effect is probably harmless; an 8-bit UNORM texel cannot hold NaN. Status: by reading, not run; not verified on hardware. Single report.
14. **Occlusion is always computed.** `use_occlusion` (default off) only gates the readers; the eight occlusion dispatches run every time a cascade is rebuilt (gi.cpp:2345-2373). Status: by reading, not run. Agreed by 2 reports.
15. **The three SDF marches have no iteration cap and use different hit rules.** Shadow march: hit below 0.001 cell, tested before the step (dl.glsl:409); probe march: hit below 0.05 (int.glsl:228); screen reflection march: no hit test, `fdistance < softness` adds light, step is unconditional and can be negative once (gi.glsl:415-451). Decode offsets also differ (1.0, 1.0, 1.1, 1.7 in the debug view). Status: by reading, not run. Agreed by 3 reports.
16. **Surface-only voxelization.** A thick closed mesh has empty positive voxels inside its shell; the field has no interior (4.13). Status: by reading, not run. Single report.
17. **A "shadows only" Static mesh is voxelized**: the SDFGI branch of the cull does not test `FLAG_CAST_SHADOWS_ONLY` (cull.cpp:3264-3281). Status: by reading, not run. Single report.
18. **`has_shadow` is never read and shadows are binary.** Every light is ray-traced against the SDF whatever its shadow setting (dl.glsl:69, 436); the `occlusion` value of the march is never used. Status: by reading, not run. Single report.
19. **Break on `DIRTY_ALL` leaves later axes un-scrolled for one frame** (gi.cpp:1226-1229); they catch up the next frame. Status: by reading, not run. Single report.
20. **Spot direction is not y-stretched** while position and `rel_vec` are (gi.cpp:1980-1989; dl.glsl:336-353). Status: by reading, not run. Single report.
21. **STORE does not check `p.w`**, so a cascade update with no solid voxel lists voxel (0,0,0) as solid (pre.glsl:954-971); and the solid-cell buffer has no overflow check (pre.glsl:989-1062). Status: by reading, not run. Single report.
22. **Hemisphere parity per frame.** At even `history_size` each frame casts into a single z-hemisphere (int.glsl:185-190). Status: by reading, not run. Single report.
23. **The screen-pass normal test is dead**: `normal.length() > 0.5` is always true (gi.glsl:620). Status: by reading, not run. Single report.
24. **Static light store loses precision**: RGBE8985 drops the low bit of red and blue (dl.glsl:488-490). Status: by reading, not run. Single report.
26. **Area lights are injected at twice the forward pass's radiance.** In non-physical units the forward pass's light colour is `pi * energy * color` (light_storage.cpp:1046), divided by the area when `normalize_energy` is on (light_storage.cpp:1094-1099, default true for AreaLight3D, light_3d.cpp:750); `ltc_evaluate` divides the edge integral by 2 pi (area_lights_inc.glsl:322) and the area diffuse term has no 1/pi (scene_forward_lights_inc.glsl:1282), so a Lambertian surface shows `rho * E * C * I / (2A)`. SDFGI passes `energy = E * indirect_energy`, divided by the area (gi.cpp:2037-2041), and `ltc_evaluate_diff` keeps `integral = I` ("no division by (2.0 * M_PI) for GI calculations", area_lights_inc.glsl:410), so the voxel holds `rho * E * C * I / A`: twice the forward value. For directional, omni and spot lights the voxel light equals the forward pass's outgoing radiance (the forward pass's pi cancels its Lambert 1/pi, scene_forward_lights_inc.glsl:241). Status: read from the code (a scale research agent, key lines re-read by the main agent) and measured on the lab's CPU model: at y scale 100 %, the floor cell under the room's panel (an AreaLight3D with energy set to match the forward pass) holds 2.08 times the forward pass's outgoing radiance (selftest). Not run in Godot.
27. **At y scales other than 100 %, light distances are measured in the squashed space.** `pre_process_gi` multiplies a positional light's y by `y_mult` (gi.cpp:1987-1988) and the cell's position is in the same space, so `light_distance = length(rel_vec)` (sdfgi_direct_light.glsl:331) is the squashed distance; the next line, `rel_vec.y /= params.y_mult` (:332), would undo the squash but runs after the distance is taken and is never read again. Omni and spot attenuation therefore see a light straight above a surface 1.5 times farther away at the default 75 %, and an area light's rectangle (its edges left unscaled, gi.cpp:2024-2031) subtends a smaller solid angle from below. Measured on the lab's CPU model of the light pass: the floor cell under the room's panel holds 2.08 times the forward pass's outgoing radiance at 100 % (Finding 26) but 0.92 times at 75 %. Status: read from the code, measured on the model (selftest), not run in Godot.
25. **Stable only: the probe shader's sky binding has the wrong texture type.** `GI::init` defines `USE_OCTMAP_ARRAY` (gi.cpp:3670) where `sdfgi_integrate.glsl` tests `USE_RADIANCE_OCTMAP_ARRAY` (int.glsl:45, 277), so the probe shader is always compiled with `texture2D sky_irradiance`. With `texture_array_reflections` on (desktop default, rendering_server.cpp:3697), the sky's `radiance` is a `TEXTURE_TYPE_2D_ARRAY` (sky.cpp:1553-1574), and `update_probes` binds it to that `texture2D` slot (gi.cpp:1359-1363, `append_id(p_sky->radiance)` at 1362). Vulkan forbids the pairing; what a ray that misses reads is driver-defined (most likely layer 0 at mip 2, which the array path would read anyway). Fixed in 4.8 by `d9ea5c26`. Status: by reading (main agent), not run.

---

## Where the lab differs (planned; proposals, not facts)

The lab has no Godot port yet. These are differences the lab will probably need; the user decides.

1. **The scene** is the lab's study room, not a Godot scene. Static geometry corresponds to `gi_mode = Static` meshes; emitters and lights are the lab's own records.
2. **Cascade size.** The engine's cascade is 128^3 cells (`CASCADE_SIZE`, a compile-time constant), 4 cascades by default, cell 0.2 m, 17 probes per axis with spacing 8 cells, 8 rays per probe per frame, history 30, y_mult 1.5. A CPU mirror and a readable animation may want a smaller grid (for example 32^3 or 64^3 with `PROBE_DIVISOR = 16`, so spacing 2 or 4 cells, still 17 probes per axis). If so, the narration should say "the engine's cascade is 128 cells wide; this lab's cascade is N", and the scroll rules must be recomputed with the lab's `drag_margin = (N / 16) / 2` cells and step `2*drag_margin`.
3. **y_scale.** The lab may use `y_mult = 1.0` for the study room (simpler than the engine's 1.5); the narration can still explain the 1.5 default.
4. **Voxelization** by a CPU mirror (and maybe a WGSL compute port) instead of the engine's three orthographic raster passes into an empty framebuffer; the mirror must reproduce the "voxel from the interpolated position", last-write-wins and OR-of-facing rules, and the surface-only (no interior) result.
5. **Jump flood and STORE** as an exact CPU nearest-solid computation with the same byte encoding (`1 + d`, 0 for solid), plus an optional WGSL pass for display; the half-resolution schedule can be shown as an illustration.
6. **The camera scroll** shown from a CPU computation of `update()` (truncating camera cell, 4-cell margin, 8-cell steps, dirty slabs and the partial-vs-all rule), animated on the lab's grid.
7. **Light injection** ported to WGSL with a CPU mirror (one omni light at one cell, SDF shadow march with the engine's start bias, six-face anisotropic accumulation, RGBE packing), for both passes if the static double count is to be demonstrated.
8. **Probe tracing** ported to WGSL with a CPU mirror (Vogel ray set, 1.1-cell start bias, SDF march, hit shading by gradient and aniso weights, SH accumulation, 1024-scaled integer history), plus the octahedral STORE.
9. **The screen pass** ported as the lab's own scene shading (`SceneShading`, section 9 as the model), with the engine's 8-probe weights, normal bias, cascade blend and `energy`, and a choice whether to reproduce the 4/pi gain and the half-texel shift or to show them as findings.
10. **Findings in the lab.** Whether the lab reproduces engine quirks (static double count, 4/pi, half-texel) or shows them as annotations is a design choice; they are observations by reading, not run.

---

## Lab differences found while porting

- `hashf3` (int.glsl:93) takes `sin` of arguments near 10^7; the GPU's result differs from the CPU's by up to almost a whole period (selftest: 0.996), so a probe's spiral rotation is not reproducible between GPUs, nor between the lab's CPU model and its WGSL port. The lab's CPU model uses Rust's f32 `sin`.
- Textures are sampled with full-precision weights on the CPU; hardware filtering keeps 8 bits of fraction.

## Still unclear

Per research report (labels: cascades, voxelization, light, probes, consume).

**Cascades and culling.**
- Whether a camera render can call `sdfgi_update` twice before the regions are consumed (a second `update()` would wipe unconsumed dirty regions); not traced.
- Behaviour of `cascades` outside 1..8 set through the server API directly (no server clamp; fixed 8-element arrays); not tested.
- Effect of the cascade-AABB (squashed) vs light-AABB (real y) test beyond "over-inclusive at y_mult != 1" (Finding 9).
- Whether a static or deleted light moves or disappears correctly (no dirtying path found for a removed or moved static light).

**Voxelization and distance field.**
- Whether a fragment exactly on the far face can write outside the region (no clamp; depends on rasterizer rounding).
- What happens to a cascade with zero solid voxels in practice (Finding 21 is derived).
- Out-of-range behaviour when more than 25% of a cascade is solid (device-dependent).
- Whether a half cell with `w = 0` and a nonzero found block can occur in the upscale pass.
- Why the regular/optimized switch uses `grid/(s/2) >= 8` (no rationale in the code beyond comments).
- Whether a static **directional** light can ever be in the static list: directionals live in `scenario->directional_lights` with an empty AABB and the light report found they never reach the static pass; not traced to the end.

**Direct light.**
- NaN-to-UNORM conversion is driver-defined; not checked against the Vulkan spec.
- Whether the double application of exposure normalization is intentional; how `exposure_normalization` depends on physical light units was not traced.
- The `bool use_occlusion` push constant is 4 bytes in GLSL and `uint32_t` in C++; assumed to match.
- `max_sdfgi_cascade` has no Light3D property in this checkout; the editor and modules were not searched for another setter.

**Probes.**
- Whether the 4/pi factor is a deliberate gain.
- Whether the half-texel mismatch is deliberate.
- The parent-fallback bound in MODE_SCROLL needs a re-check (8.15): the first report's figures used a drag margin of 8; the recomputation here uses 4 and shows the indices in range, but was not run.
- The stale cascade UBO in the scroll shader (Finding 10) is read, not observed.
- Exact maximum step count of the probe march (no cap; the first report's 4400 is an upper bound).
- No runtime verification of any probe number; the worked example is from a Python re-implementation.

**Consume.**
- `without_sampler = sampler_is_format_supported_for_filter(R8G8_UINT, LINEAR)` (gi.cpp:4076) selects the `*_WITHOUT_SAMPLER` variants when linear filtering is supported, the opposite of the name; it affects only the VoxelGI index buffer, not SDFGI.
- Whether the 1-pixel octahedral border covers all bilinear taps at corners.
- The cost model of the `found`-flag sweep in the reflection march.
- Transparent surfaces with the GI-buffer flag but `uses_forward_gi == 0` read the opaque pixel behind them; inferred from flags, not run.
- Whether SSIL uses SDFGI in any way (none seen).
