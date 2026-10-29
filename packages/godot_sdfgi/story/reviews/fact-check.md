# Fact-check of the SDFGI storyboard against the Godot 4.7.2-stable checkout

Checkout: `/home/oppositenormal/Documents/programming/other/godot` (commit ed1daf0b, version.py 4.7.2 stable).
Method: every row was checked in the checkout by following the call chain, defines and defaults; the research notes were not used as evidence.
Classes: exact, imprecise (true in substance, wording misleads or omits a condition), wrong, unverifiable.
Paths are relative to `servers/rendering/renderer_rd/` unless noted. "gi.cpp" is `environment/gi.cpp`; shaders are in `shaders/environment/` (sdfgi_*.glsl, gi.glsl) or `shaders/forward_clustered/`.

Summary: no sentence is wrong in substance; every "read from the code" finding reproduces. 14 narration sentences are imprecise and 1 part of one is unverifiable (the 4.8 beta). 8 source-index entries are imprecise (none wrong).

## Chapter 1: Where SDFGI runs

| Sentence | Class | Evidence | Correction |
|---|---|---|---|
| Lab room, panel only / SDFGI adds bounce (lab content) | n/a | lab scene, not engine | |
| "followed through its source ... at the engine's own sizes" | exact | cascade 128, cell 0.2, probe 17, ring 30, 8 rays all match engine constants | |
| SDFGI is a setting of the environment, off by default | exact | scene/resources/environment.h:153 `sdfgi_enabled = false` | |
| Each frame, SDFGI first moves its grids and works out which parts are new | exact | renderer_scene_cull.cpp:3314 `sdfgi_update` (before culling) -> gi.cpp:1192 `SDFGI::update`; regions via `sdfgi_get_pending_region_*` (cull.cpp:3395-3410) | |
| New parts are turned into voxels, and a distance field is built | exact | `_update_sdfgi` render_forward_clustered.cpp:1255-1278 -> `render_region` gi.cpp:2061 (voxelize, jump flood, STORE) | |
| Light injected into solid voxels, then probes trace rays | exact | order in `_render_scene`: update_light :1759, then `update_probes` via `_post_prepass_render` :2132 | |
| Last, a screen pass reads probes per pixel, opaque pass shades with the result | exact | store_probes :1534, `process_gi` then scene_forward_clustered.glsl:2023-2024 | |
| Only Forward+ has it: Mobile and Compatibility replace every SDFGI call with an empty one | imprecise | Empty: render_forward_mobile.h:643-646 and `_render_sdfgi` render_forward_mobile.cpp:1830; rasterizer_scene_gles3.h:882-891. But Mobile inherits the real `environment_set_sdfgi_ray_count/frames_*` (renderer_scene_render_rd.cpp:95-104) and the Environment properties; only Compatibility's setters are empty (rasterizer_scene_gles3.cpp:1152-1160) | "Only the Forward+ renderer runs it: in the Mobile and Compatibility renderers the update, region-query and voxelizer entry points are empty, so nothing is ever built." |
| Defaults: four cascades, 128 cells a side | exact | environment.h:154 (4); gi.h:646 `cascade_size = 128` | |
| Three project settings decide rays, frames averaged, light update rate | exact | gi.cpp:3515-3517, rendering_server.cpp:3793-3795 (they can also be changed at run time through RenderingServer setters, not said) | |
| Probes are traced in one place only: inside the depth pre-pass branch | exact | `update_probes` has one caller, `_post_prepass_render` (renderer_scene_render_rd.cpp:1355); its only call is render_forward_clustered.cpp:2132, inside `if (depth_pre_pass)` (:2120) | |
| SDFGI does not ask for that pass; with the pre-pass setting off the probes stop updating | exact | :2112-2113 only `used_opaque_stencil` or the setting (default true, ProjectSettings.xml:2912) force it; no else-branch calls update_probes (grep). store_probes still runs (:1534) | |
| Setting lists sixteen rays at its default position | exact | rendering_server.cpp:3793: hint "8 (Fastest),16,32,64,96,128 (Slowest)", default 1 | |
| Code table starts one entry earlier, at four; default gives eight rays | exact | gi.cpp:1322 `{4, 8, 16, 32, 64, 96, 128}`, enum has 7 entries (rendering_server_enums.h:709-716) | |

## Chapter 2: Cascades that follow the camera

| Sentence | Class | Evidence | Correction |
|---|---|---|---|
| Four cascades, all 128 cells wide | exact | gi.cpp:414-430, gi.h:646 | |
| First cells 20 cm, spans 25.6 m; each next doubles, out to 205 m | exact | gi.cpp:431 `base_cell_size = min_cell_size` (0.2), :573 doubles; 128 x 1.6 = 204.8 m (horizontal) | |
| y scale has three choices, default reads 75 percent | exact | environment.cpp:1428 hint "50% (Compact),75% (Balanced),100% (Sparse)"; environment.h:156 | |
| "In the code it is a factor of 1.5": height multiplied before being cut into cells | exact | gi.cpp:423-424 `{2.0, 1.5, 1.0}`; :565 `world_position.y *= y_mult` | |
| A cell is only 13 cm tall; cascade height two thirds of width, not three quarters | exact | 0.2 / 1.5 = 0.133 m; box y size = 128 * cell / 1.5 (gi.cpp:1500-1501) | |
| Readout: 25.6 wide, 17.07 tall | exact | 25.6 / 1.5 = 17.07 | |
| 17 probes per axis, one every eight cells | exact | gi.cpp:427 `PROBE_DIVISOR + 1`; :566 128 / 16 = 8 | |
| In the first cascade probes are 1.6 m apart (horizontally) | exact | 8 x 0.2 m; vertically 1.6 / 1.5 = 1.07 m (not claimed) | |
| Camera position is cut into cells and compared with the cascade's centre | exact | gi.cpp:1209 `Vector3i((world_position + probe_half_size) / cell_size)`, `cascade.position` is the box centre | |
| Strays more than four cells: jumps eight cells, a whole probe | exact | gi.cpp:1199 `drag_margin = (128 / 16) / 2 = 4`; :1219 `+= drag_margin * 2` | |
| The eight-cell slab uncovered is dirty; only it is voxelized again | exact | gi.cpp:1220; `render_region` scrolls kept cells (:2092-2140) and voxelizes only the slab, unless DIRTY_ALL | |
| More than half of the kept volume would be new: whole cascade rebuilt | exact | gi.cpp:1235-1244 `dirty_volume > safe_volume / 2` | |

## Chapter 3: What is voxelized, and which lights

| Sentence | Class | Evidence | Correction |
|---|---|---|---|
| Each axis that moved gives one region; a later axis leaves out the corner an earlier one took | exact | gi.cpp:1478-1497 (chip loop), render_forward_clustered.cpp:4083-4087 | |
| Each region becomes a box in real metres, height divided back by the y scale | exact | gi.cpp:1500 `* Vector3(1, 1.0 / y_mult, 1)` | |
| Every instance is tested against the regions, with no test for the camera's view | imprecise | renderer_scene_cull.cpp:3265-3282: the block sits outside the frustum/occlusion/visibility-range tests (:2931+), only the layer mask is tested (:3279). But an instance hidden with `visible = false` is removed from `instance_data` (instance_set_visible :1046-1062 -> `_unpair_instance` :1915-1960; `_update_instance` :1739), so it is not tested | "Every visible instance is tested against the regions, with no frustum, occlusion or distance test." |
| Only meshes whose GI mode is Static are voxelized; Dynamic receives light but is not in the field | exact (necessary, not sufficient) | visual_instance_3d.cpp:479-497; cull.cpp:3279 `FLAG_USES_BAKED_LIGHT`; render_forward_clustered.cpp:4503. Also only opaque/depth-pass surfaces are drawn: `_fill_render_list` else-branch :1189-1192 needs FLAG_PASS_DEPTH or FLAG_PASS_OPAQUE, which blended materials lack (:4191-4199) | optional add: "...and only their opaque surfaces" |
| Lights split by bake mode: dynamic evaluated every update, static only when a region is built | imprecise | renderer_scene_cull.cpp:1627 skips directional lights; `pre_process_gi` (gi.cpp:1913-1950) takes every directional light with no bake-mode test, so a Static directional light is still dynamic. Static pass runs only when a cascade is rebuilt and a static light is culled into it (cull.cpp:3656-3674) | "Positional lights split by their bake mode ... (directional lights are always dynamic)." |
| Static light listed for a region it overlaps, up to its max cascade, default two | exact | cull.cpp:3271 `<= max_sdfgi_cascade`; default 2 at renderer_scene_cull.h:705, light_storage.h:70 | |
| Lists stored under cascade number, read back by position in the dirty list | exact | stored cull.cpp:3275 `sdfgi_cascade_lights[cascade]`; read :3662-3664 and gi.cpp:2452 `[i][j]` with `i` the position (:2430) | |
| Only second cascade moves: stored under one, read from zero, gets none | exact | with one dirty cascade `cascade_light_count = 1`, only slot 0 is checked (cull.cpp:3662), `static_lights_culled` stays false, whole static pass skipped. Lab selftest "static-light lists (F2)" covers it | |

## Chapter 4: Voxelization

| Sentence | Class | Evidence | Correction |
|---|---|---|---|
| Region drawn three times, camera along each axis, one pixel per cell of the face it looks at | exact | render_forward_clustered.cpp:3133-3175: origin on +axis face, looks along -axis, `fb_size = p_size[right], p_size[up]`, projection = the box | |
| Each view finds the surfaces facing it; together they cover the room | imprecise | culling is off (:398-399), so a view rasterizes every triangle in the box whichever way it faces; the three axes ensure no surface is edge-on to all | "Each view draws whatever is not edge-on to it; together the three cover the room." |
| Culling is turned off for every material, so a surface is drawn from both sides | imprecise | :398-399 `CULL_VARIANT_DOUBLE_SIDED` for PASS_MODE_SDF, but only for surfaces that enter the SDF list (FLAG_PASS_DEPTH or OPAQUE, :1189-1192); alpha-blended materials are not drawn at all | "Every voxelized material is drawn without culling, so a surface is drawn from both sides." |
| Each fragment writes the voxel its own position falls in | exact | scene_forward_clustered.glsl:2909-2910 | |
| Albedo 16 bits: lowest marks solid, five bits each for r, g, b | exact | glsl:2912-2915, R16_UINT gi.cpp:443 | |
| Second image keeps six bits, one per axis direction; fragment sets the one nearest its normal | exact | glsl:2928-2943 | |
| Several fragments combine bits; an edge voxel can face two ways | exact | `imageAtomicOr` :2943 | |
| Emission: shared exponent of 30 bits, five-bit weight for each of six faces | exact | glsl:2946-2994 (8+9+8+5 = 30; 6 x 5 = 30) | |
| Only surfaces: a thick object is hollow, read from the code | exact | no fill step anywhere in render_region/preprocess; only rasterized fragments set the solid bit | |

## Chapter 5: The distance field

| Sentence | Class | Evidence | Correction |
|---|---|---|---|
| Every cell gets the distance to its nearest solid voxel; a ray can step that far | exact | sdfgi_preprocess.glsl:958-968; steps in sdfgi_integrate.glsl:227-234 | |
| Search at half size, 64 a side; each half cell starts with one solid voxel of its 2x2x2 block | exact | gi.cpp:2219-2232; glsl:245-272 | |
| Each pass looks at 26 neighbours a step away, keeps the nearest seed | exact | glsl:277-345 | |
| Steps halve from 32 to one | exact | gi.cpp:2243-2275 traced: 32, 16 regular; 8, 4, 2, 1 optimized | |
| First two passes regular; last four use shared memory | exact | same trace (regular stops when 64 / (s/2) >= 8 at s = 16) | |
| Scaled up to 128; each cell checks the block around its seed | exact | glsl:457-470 | |
| One last full-size pass, step one | exact | gi.cpp:2282-2290 | |
| STORE writes one byte: zero at a solid voxel, else 1 + distance in cells | exact | glsl:958-968 R8_UNORM | |
| The field is not signed: every reader subtracts the one again, so a solid voxel reads minus one | imprecise | probe march (sdfgi_integrate.glsl:227) and shadow march (sdfgi_direct_light.glsl:408) subtract 1.0; the screen reflection march subtracts 1.1 (gi.glsl:415, 431); the debug view subtracts 1.7 (sdfgi_debug.glsl:141); the gradient reads (integrate.glsl:256) subtract nothing | "The field is not signed: the probe and shadow marches subtract the one again, so a solid voxel reads minus one." |
| Every solid voxel goes into a list of 16-byte records: position, albedo, facing, stored light | exact | glsl:998-1036: four words (position, albedo+facing, light, light_aniso) | |
| 26 more bits mark empty neighbours whose nearest surface is this voxel | exact | glsl:1000-1033 | |

## Chapter 6: Occlusion

| Sentence | Class | Evidence | Correction |
|---|---|---|---|
| Window of 16 cells around each probe | exact | OCCLUSION_SIZE = 128 / 16 = 8 (gi.cpp:3643), window 2 x 8; dispatch gi.cpp:2347-2372 | |
| Each shell takes the average of its neighbours one step closer, and a solid cell stops it | imprecise | sdfgi_preprocess.glsl:561-650: a solid cell gets 0 and a solid neighbour adds 0 but still counts in the average (except on the coordinate planes), so it darkens rather than blocks | "...each shell takes the average of its neighbours one step closer; a solid cell counts as zero." |
| Four bits for each of eight probes | exact | glsl:973-977 | |
| By default nothing reads them (use occlusion off), yet the pass runs every time a cascade is built | exact | every read is under `use_occlusion` (gi.glsl:233, direct_light.glsl:253, fog:762); dispatch gi.cpp:2347-2372 has no such test | |

## Chapter 7: Light in the voxels

| Sentence | Class | Evidence | Correction |
|---|---|---|---|
| One thread per solid cell in the list | exact | gi.cpp:1305 indirect dispatch; direct_light.glsl:184-190 | |
| Default updates every fourth cell per frame; a moving light lags up to four frames | exact | gi.cpp:1291-1298 index 2 -> 4, offset `frame % 4` | |
| Omni falloff: (1 - (d/r)^4)^2 over the distance | imprecise | glsl:125-132: times d^-decay, decay = the light's attenuation, default 1 (light_3d.cpp:495), so "over the distance" holds only at the default | add "at the default attenuation of one" |
| The distance is measured after squashing, so the light seems 1.5 times higher | exact | gi.cpp:1981 `pos.y *= y_mult`; cell position in squashed space (direct_light.glsl:196-201); `length(rel_vec)` taken before the dead `rel_vec.y /= y_mult` (:329-331). Lab selftest covers omni port and area-light squash (F26/F27); no dedicated omni-height check | |
| Shadow ray marched by the stored distance; blocked on reaching a solid cell; lit past the last cascade | exact | glsl:380-440 | |
| Shadow is all or nothing; each light traced whatever its own shadow setting says | exact | `has_shadow` is only declared (glsl:69); on a hit `attenuation *= occlusion` is skipped by `break` and `if (!hit)` | |
| Light summed per face the voxel shows, then shared-exponent texture, shares in two more | exact | glsl:441-515 | |
| Same values written into empty neighbours pointing back | exact | glsl:517-551 | |
| Static light evaluated when its region is built and added to the light the cell stores | exact | gi.cpp:2573-2590, direct_light.glsl:555-570 | |
| After a scroll kept cells carry stored light, static part included, into the new list | exact | preprocess.glsl:195-196 (light word copied), STORE :1030 | |
| The static pass adds the light again: held twice, three, four times | imprecise | omitted condition: only when the light overlaps the scrolled slab's region (cull.cpp:3266-3275), and when the cascade is dirty at slot 0 (F2). Mechanism confirmed (direct_light.glsl:295 sums from current light). Lab selftest F1 states the condition | "After every scroll whose slab the light overlaps, the static pass adds the light again..." |
| Each cell reads the probes around it, adds their light x albedo x bounce (half by default) | exact | direct_light.glsl:217-290; default environment.h:158 | |
| That light came from the previous frame's probes | exact | update_light :1759 precedes store_probes :1534 in the frame | |

## Chapter 8: Probes trace the field

| Sentence | Class | Evidence | Correction |
|---|---|---|---|
| Every frame each probe traces eight rays | exact | gi.cpp:1322-1323 default index 1 | (only while the pre-pass runs, chapter 1) |
| Spiral; over thirty frames fill 240 directions | exact | integrate.glsl:176-190: idx = history_index + i * 30, ray_total 240 | |
| Because thirty is even, each frame's eight rays all point into the same half, read from the code | exact | glsl:89-95 z sign = parity of idx; idx parity = history_index parity when history_size is even | |
| Steps by the field's distance until within a twentieth of a cell | exact | glsl:227-228 | |
| Leaving the first cascade it carries on through the next | exact | glsl:241-247 | |
| At a hit the normal comes from the field's slope and chooses how much of each face's light | exact | glsl:254-267 | |
| A miss reads the sky at its second mip | imprecise | glsl:280 `textureLod(..., 2.0)`: mip level two (third image; the code comment calls it "second mipmap"); only with a sky background, a colour background gives that colour, otherwise black (:286-292) | "...reads the sky at mip level two." |
| In 4.7 stable the probe shader is compiled for a single sky texture while the desktop default makes it an array, a mismatch the beta fixes | exact for stable; beta part unverifiable | gi.cpp:3670 defines `USE_OCTMAP_ARRAY`; sdfgi_integrate.glsl:45 and :277 test `USE_RADIANCE_OCTMAP_ARRAY` (never defined for this shader; compare fog.cpp:307, render_forward_clustered.cpp:5112); desktop default true (rendering_server.cpp:3697); radiance is a 2D array (sky.cpp:1565) bound at gi.cpp:1352. The 4.8 beta is not in the checkout | drop "the beta fixes" or cite the beta commit |
| 16 SH coefficients, stored as integers, scaled by 1024 | exact | integrate.glsl:295-318, :330 HISTORY_BITS 10 | |
| Last thirty frames kept; running sum subtracts oldest, adds newest; straight line for 30 frames then holds | exact | glsl:333-336 | |

## Chapter 9: From coefficients to maps

| Sentence | Class | Evidence | Correction |
|---|---|---|---|
| Two 6x6 octahedral maps per probe: irradiance and radiance | exact | integrate.glsl:361-470, gi.h LIGHTPROBE_OCT_SIZE 6, layers cascade and cascade + max | |
| A border of one texel copies the opposite edges | imprecise | glsl:437-466: an edge texel is also written to its mirror across the octahedral fold, e.g. (x, 0) -> (5 - x, -1); corners write the three other corners. It is a mirrored copy of the map's own edge texels, not "the opposite edge" | "A border of one texel holds mirrored copies of the map's own edge texels, where the octahedral fold continues them, so filtering never reads past the map." |
| Coefficients summed with a four where four pi belongs, so a uniform sky is stored about a quarter too bright | imprecise | Numbers right, decomposition off: estimator `4 / N` (glsl:328) then STORE `* 4` (:422) gives 16 / N where 4 pi / N is exact; irradiance of a uniform sky = (4 / pi) L = 1.273 L (selftest F4: 1.15..1.40). 4 / pi is about 27 percent | "The coefficients are scaled by sixteen over the ray count where four pi over it belongs, so a uniform sky is stored four over pi, about 27 percent, too bright." |
| Texel direction at its corner, but the reader samples at its centre, so lookups blend two texels | imprecise | writer: `octahedron_encode(local_pos / 6)` (glsl:361), texel k holds u = k / 6. Reader: coordinate = block + uv * 6 (gi.glsl:208), an integer = texel boundary, so it blends texels k - 1 and k. The reader samples at a corner, the value sits at the texel's centre; the narration swaps this | "Each texel's direction is taken at its corner, but its value sits at the texel's centre, so a lookup lands between two texels." |
| Scrolled probes keep their history | exact | integrate.glsl:482-500 | |
| New probes start from the coarser cascade's light | exact | glsl:518-575 (last cascade copies what is there) | |

## Chapter 10: Shading a pixel

| Sentence | Class | Evidence | Correction |
|---|---|---|---|
| After the depth pre-pass a compute pass rebuilds each pixel's position and normal | imprecise | gi.glsl:615-643: position rebuilt from depth (`reconstruct_position`), normal and roughness read from the normal-roughness buffer (`fetch_normal_and_roughness`) | "...a compute pass rebuilds each pixel's position from depth and reads its normal." |
| Finest cascade whose box holds it | exact | gi.glsl:316-326 | |
| Blends toward the next between 5.5 and 7.5 probes from the camera | exact | gi.glsl:336-347 (Chebyshev distance from the camera in probe units; box edge at 8 +- 0.5) | |
| Eight probes weighted by distance and by facing | exact | gi.glsl:228-229 | |
| Reflections read radiance maps along the reflected direction, blending toward irradiance as roughness grows | exact | gi.glsl:264-269 | |
| Below roughness a fifth Godot also marches a reflection ray | exact | gi.glsl:401 `roughness < 0.2` | |
| Opaque pass takes SDFGI ambient in place of the sky's, multiplies by albedo, adds reflection | exact | scene_forward_clustered.glsl:2023-2024 (mix by alpha), :2185 | |
| Transparent surfaces repeat the lookup, without reflection march and without energy | exact | scene_forward_clustered.glsl:1873-1935; `sdfgi.energy` read only at gi.glsl:478-479 | |
| Fog reads the probes' average light but never applies y scale, so wrong height | exact | fog `y_mult` only declared (volumetric_fog_process.glsl:132); `world_pos` :703 unscaled while cascade positions are squashed (gi.cpp:1888-1890) | |

## Chapter 11: The frame again

All four recap sentences: exact (same evidence as chapters 1, 2, 7, 8, 9, 10).

## Task 3: claims "measured on the lab's model"

c3-finding (F2), c7-omni, c7-double (F1) and c9-gain (F4) are consistent with the lab code: `src/godot/sdfgi/selftest.rs` carries checks for F2 (:433), F1 (:416, "after every scroll the light overlaps"), F4 (:236, 1.15..1.40 for 4/pi) and the area-light squash (F26/F27 :215-217). There is no dedicated omni-height measurement; that sentence rests on the code (omni port matching, :327). I could not run the lab.

## Source-index entries

Checked: all 97 entries, cited line plus details, against the checkout. All cited lines hold. inspectZh was compared number-for-number and meaning-for-meaning with inspect; no discrepancy found (the Zh text of every entry below says the same as the English, so each correction applies to both).

Imprecise entries:

| Entry | Problem | Correction |
|---|---|---|
| project-settings | "A later change needs a restart" is wrong for the engine: RenderingServer setters change `gi.sdfgi_ray_count`, `frames_to_converge` (which rebuilds SDFGI, :4056), `frames_to_update_light` at run time (renderer_scene_render_rd.cpp:95-104); ProjectSettings.xml:3068 says so | "The three project settings are read once, when the GI object is made; at run time only the RenderingServer setters change them." |
| mobile-stub | Fine for the stubs; the narration's "every call" is wider than the entry. Mobile also keeps real setters (above) | add "the setters stay real on Mobile" |
| sdfgi-cull | omits that hidden (`visible = false`) instances are not in the instance array (renderer_scene_cull.cpp:1739, 1915-1960) | "Every visible instance of the scenario ..." |
| sdf-decode | "every reader" is too wide: the screen-pass reflection march uses 1.1 (listed), the debug view 1.7 (sdfgi_debug.glsl:141), gradients subtract nothing | "The probe and shadow marches subtract 1.0 ..." |
| occlusion-shells | fine; but a "solid cell stops it" reading must not be drawn from it (average counts solid neighbours as 0) | none to the entry; narration fixed above |
| static-light-cull / sdfgi-cull pair | static list described without the visible-light condition (same cause) | add "visible" |
| fog-sdfgi | "trilinear weights only" omits the optional occlusion factor when use_occlusion is on (volumetric_fog_process.glsl:762-775) | "trilinear weights (times occlusion when it is on)" |
| light-bake-route | correct; add that directional lights are in `dynamic_lights`-free path but always dynamic (already stated) | none |
| omni-attenuation | "decay = the light's attenuation (default 1, not 2)": correct; the key point is the default is 1 | none |
| probe-sky | "mip 2" is mip level two, the comment says "second mipmap" | write "mip level 2" |
| sky-define | "4.8.0-beta renames this define": not checkable in this checkout | mark unverified |
| process-gi-dispatch | "SDFGI counts as in use only with SDFGI enabled and a viewport debug mode other than Unshaded": correct (:1876); also requires a valid SDFGI object | none |

Entries whose text states a bug and was confirmed in the code: ray-count-table/hint (F7), y-mult-table (F3), static-lights-index (F2), sdfgi-cull F17 note, dynamic-lights (F9, F20), shadow-march (F18), static-write (F24), scroll-reinject (F1), vogel-rays (F22), probe-sky/sky-define (F25), sh-gain (F4), oct-store/oct-read (F5), fog-sdfgi (F6), gi-energy/forward-gi (F11), occlusion-dispatch (F14), prepass-gate (F8), stale-cascade-ubo (F10), process-gi (`length()` of a vec3 is 3), process-voxel (comment says bits 16-21, code uses 15-20).

Not wrong but worth knowing: scene shader's `NO_IMAGE_ATOMICS` fallback (glsl:2940) is never defined for the scene shader (only fog.cpp defines it), so the facing write is always `imageAtomicOr`, as the entries say.
