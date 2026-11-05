# Fact check, chapters 1 to 5 (Godot 4.7.2-stable, ed1daf0)

Paths are relative to the Godot checkout unless marked "lab". RFC = servers/rendering/renderer_rd/forward_clustered/render_forward_clustered.cpp; GI = servers/rendering/renderer_rd/environment/gi.cpp; PRE = .../shaders/environment/sdfgi_preprocess.glsl; INT = .../shaders/environment/sdfgi_integrate.glsl; SC = servers/rendering/renderer_scene_cull.cpp. Spoken values (natives in mod.rs `script_natives`, from model.rs): cascades 4, cascadeCells 128, cellCentimetres 20, firstSpan 25.6, probeGaps 16, probeCells 8, probeAxisCount 17, probeSpacing 1.6, dragMargin 4; all match Godot (environment.h:154-155, gi.h:646 cascade_size=128, gi.h:563 PROBE_DIVISOR=16, gi.cpp:427, gi.cpp:1199).

| Beat id | Sentence (first 8 words) | Verdict | Evidence / correction |
|---|---|---|---|
| c1-room | This is the lab's study room, lit by the ceiling panel alone. | UNCERTAIN | Lab: packages/render_lab/scenes/study-room.lam has the panel light and `ambient 0.12` in the room's lighting; shaders/sdfgi_shading.wgsl:29 says mix 0 is "direct light only". Not checked that the 0.12 ambient is off at mix 0 (offscreen render not made). |
| c1-room | With only that direct light, every surface the panel cannot see stays black. | UNCERTAIN | Same as above (depends on whether the scene's 0.12 ambient is shown at sdfgiMix 0). |
| c1-room | Now SDFGI, Godot's signed distance field global illumination, adds the light that bounced | VERIFIED | doc/classes/Environment.xml:218 ("signed distance field global illumination"). |
| c1-room | The red wall tints the sphere, and the sky through the open top tints the floor. | VERIFIED | Lab scene: left wall albedo (0.70,0.24,0.18) red, three walls and floor and no ceiling (study-room.lam:36-38, 62); sky is read by default (environment.h:159, INT:263-272). Render not inspected. |
| c1-room | This is SDFGI as Godot four point seven builds it. | VERIFIED | Checkout is 4.7.2-stable. |
| c1-room | The lab reproduces it from Godot's source, step by step, on the CPU and at the engine's own sizes. | VERIFIED | Lab: model.rs mirrors the passes on the CPU at 128-cell cascades (mod.rs:1-13, voxels.rs:1-17). |
| c1-room | SDFGI is a setting of the scene's environment, and it is off by default. | VERIFIED | scene/resources/environment.h:153 `bool sdfgi_enabled = false;` and doc Environment.xml:217 default false. |
| c1-frame | SDFGI keeps its data in grids around the camera, called cascades. | VERIFIED | GI:414-436 (cascades vector, 128^3 textures), GI:573 cell size doubling, positions follow camera (GI:1192-1240). |
| c1-frame | Each frame, SDFGI first moves the cascades to follow the camera. | VERIFIED | SC:3314 `sdfgi_update` runs from `_render_scene` before culling; RFC:4068 `sdfgi->update(...)` moves cascades; voxelizing is RFC:1734 afterwards. |
| c1-frame | Moving a cascade brings new space into it at one edge, and only that new part needs work. | VERIFIED | GI:1219-1223 (dirty slab per axis), GI:1458-1510. Whole-cascade rebuild exception is said in chapter 2. |
| c1-frame | The new part is cut into voxels, small boxes marked solid or empty. | VERIFIED | RFC:3099 `_render_sdfgi`, scene_forward_clustered.glsl:2909-2917 (solid flag bit 0). |
| c1-frame | From those voxels, SDFGI builds a distance field, which tells a ray how far | VERIFIED | GI:2061-2290 render_region (jump flood then MODE_STORE), PRE:956-968 stores 1 + distance. |
| c1-frame | The solid voxels then store the light that falls on them. | VERIFIED | RFC:1759 `update_light`, GI:1249-1305 (one dispatch per cascade over the solid-cell list). |
| c1-frame | Probes are points spread through the cascades, and each one records the light arriving from every direction. | VERIFIED | GI:427, INT:166-175 (one probe per thread; SH accumulation INT:300+). |
| c1-frame | Each probe traces rays through the distance field, and each ray brings back the light of the voxel it hits. | VERIFIED | INT:219-262 (march by stored distance; at a hit `hit_light = texture(light_cascades...)`; else sky). |
| c1-frame | Last, a pass over the screen reads, for every pixel, the light of the probes around it. | VERIFIED | RFC:1607 `gi.process_gi`, gi.glsl:615 `process_gi` -> `sdfgi_process`. |
| c1-frame | The opaque pass, which shades the solid surfaces, then uses that light as each pixel's ambient light. | VERIFIED | scene_forward_clustered.glsl:1982-2020 (`ambient_buffer`/`reflection_buffer` read in the opaque pass when INSTANCE_FLAGS_USE_GI_BUFFERS). |
| c1-switches | Only the Forward+ renderer has SDFGI. | VERIFIED | Environment.xml:219 note; render_forward_mobile.h:643 and drivers/gles3/rasterizer_scene_gles3.h:882 are stubs. |
| c1-switches | In the other two renderers, Mobile and Compatibility, SDFGI's functions are empty | SIMPLIFICATION | `sdfgi_update` has an empty body (render_forward_mobile.h:643, rasterizer_scene_gles3.h:882), but the region queries return 0 / empty AABB (render_forward_mobile.h:644-646, gles3.h:883-891), and the environment setters stay real. True in effect: it does nothing. |
| c1-switches | The defaults are four cascades, each a grid of 128 cells a side. | VERIFIED | environment.h:154 `sdfgi_cascades = 4`; gi.h:646 `cascade_size = 128`. |
| c1-switches | In the finest cascade, the cells are twenty centimetres across. | VERIFIED | environment.h:155 `sdfgi_min_cell_size = 0.2`; GI:556-573 first cascade takes `min_cell_size`. |
| c1-switches | Three project settings also shape SDFGI. | VERIFIED | GI:3515-3517 reads `probe_ray_count`, `frames_to_converge`, `frames_to_update_lights`; rendering_server.cpp:3793-3795. |
| c1-switches | They set how many rays each probe traces, how many frames it averages, and how often the lights are updated. | VERIFIED | ProjectSettings.xml:3066-3077 (rays per frame; frames to converge = history averaged in INT:343 `/ history_size`; frames over which dynamic lights update, GI:1286-1298). |
| c1-prepass | The depth pre-pass draws the scene's depth before any shading. | VERIFIED | RFC:2119-2150 ("Render Depth Pre-Pass"). |
| c1-prepass | The code updates the probes in one place only, beside this pass, and only when the pass runs. | VERIFIED | `update_probes` has one caller, `_post_prepass_render` (renderer_scene_render_rd.cpp:1355), called only at RFC:2132 inside `if (depth_pre_pass)` (RFC:2119). Repo-wide grep finds no other caller. |
| c1-prepass | The code shows that the pre-pass runs only when its project setting is turned on. | WRONG | RFC:2112-2113: `depth_pre_pass = (force_depth_pre_pass \|\| GLOBAL_GET_CACHED(... "rendering/driver/depth_prepass/enable")) && depth_framebuffer.is_valid()`, with `force_depth_pre_pass = scene_state.used_opaque_stencil`. A material that uses the opaque stencil forces the pass even with the setting off (the index's own inspect text says so). The cited `prepass-call` entry (renderer_scene_render_rd.cpp:1355) shows the probe caller, not this condition; `prepass-gate` does. Fix: "only when its project setting is on, or a stencil material forces it". |
| c1-prepass | SDFGI never switches the pre-pass on by itself. | VERIFIED | RFC:2112 is the only forcing flag (stencil); nothing in gi.cpp touches it. |
| c1-prepass | So with that setting turned off, the probes stop updating, and the light freezes. | SIMPLIFICATION | True when nothing forces the pass (see above; a stencil material would keep it running). Probes stop (no `update_probes`, no `render_pass++`, GI:1391); `store_probes` still runs each frame (RFC:1532-1534) on the old data; voxel light still updates, so "the light freezes" means the probe-lit bounce, not everything. |
| c1-rays | The setting for the probes' ray count offers eight, sixteen, thirty-two, and more. | VERIFIED | rendering_server.cpp:3793 hint "8 (Fastest),16,32,64,96,128 (Slowest)". |
| c1-rays | The default is the second choice, the one the editor labels sixteen. | VERIFIED | rendering_server.cpp:3793 default `1`, second entry "16". |
| c1-rays | But the code looks that choice up in a different list, which begins with four. | VERIFIED | GI:1322 `ray_count[] = { 4, 8, 16, 32, 64, 96, 128 }`, indexed by `sdfgi_ray_count`. |
| c1-rays | So the second choice means eight rays per probe each frame, half of what the label says. | VERIFIED | GI:1322-1323: index 1 gives 8; label says 16. |
| c1-sources | Every claim in this video cites a file, a symbol, and a line in the Godot checkout. | SIMPLIFICATION | Many sentences (for example c1-room, c1-frame) have no callout; the video cites its main claims, not every one. |
| c1-sources | The build checks that each of those lines is still there. | UNCERTAIN | source_index.py (packages/godot_sdfgi/src/godot/sdfgi/source_index.py:793) writes `lineHash`; sources.rs:3 says `selftest` checks each hash "when [the checkout] is present", but selftest.rs has no code that does so (grep for hash/checkout finds none), and nothing in lab/ or tools/ reads `lineHash`. It is not part of the cargo build. |
| c2-nested | SDFGI must place its grids before it can draw any voxel into them. | VERIFIED | RFC:1757 `update_cascades` and GI:1192 `update` (positions) precede `render_region` bounds (GI:1458). |
| c2-nested | Around the camera sit four cascades, all 128 cells wide. | VERIFIED | environment.h:154; gi.h:646. |
| c2-nested | A cell is one box of a cascade's grid, the space that one voxel fills. | VERIFIED | Definition; one texel per cell (GI:431-436). |
| c2-nested | The first cascade has cells of twenty centimetres, so it spans 25.6 metres. | VERIFIED | 0.2 x 128 = 25.6 (GI:556-573; cascade bounds GI:1467-1468). |
| c2-nested | Each cascade after the first has cells twice the size of the one before, so it spans twice as far. | VERIFIED | GI:573 `base_cell_size *= 2.0`; same 128 cells per cascade. |
| c2-nested | So the fourth and coarsest cascade spans two hundred and five metres. | VERIFIED | 0.2 x 2^3 x 128 = 204.8 m (index inspect cell-size-double). Rounded. |
| c2-nested | The study room is only six metres across, so it fits inside the first cascade, at its centre. | VERIFIED | Lab: study-room.lam:33 `_half 3.0`; demos.rs:25 ORIGIN (0,1.6,0), cascade centred on the nearest probe (model.rs:94-110): centre x,z = 0. In y the cascade centre is 2.13 m (not exactly the room's middle at 1.5 m); still fits. |
| c2-yscale | The y scale setting has three choices, and its default is labelled seventy-five percent. | VERIFIED | environment.cpp:1428 enum "50% (Compact),75% (Balanced),100% (Sparse)"; environment.h:156 default 75 %. |
| c2-yscale | The documentation says it trades how far the grid reaches up and down for finer detail in height. | SIMPLIFICATION | Environment.xml:247: "Lower values will result in SDFGI cells being packed together more closely on the Y axis... balance between quality and covering a lot of vertical ground." Same trade; the wording "finer detail in height" is a paraphrase. |
| c2-yscale | In the code, the setting is a factor, and seventy-five percent becomes one and a half | VERIFIED | GI:423-424 `y_scale[3] = {2.0, 1.5, 1.0}`; `y_mult = y_scale[mode]`. |
| c2-yscale | Every height is multiplied by one and a half before it is placed in the grid | VERIFIED | GI:565 `world_position.y *= y_mult`; GI:1204 same in update. |
| c2-yscale | So a cell covers only thirteen centimetres of real height, twenty divided by one and a half. | VERIFIED | 20/1.5 = 13.3 cm (truncated); real box y is divided by `y_mult` (GI:1470). |
| c2-yscale | The first cascade is therefore just seventeen point oh seven metres high. | VERIFIED | 25.6 / 1.5 = 17.067 m. |
| c2-yscale | The code shows that a cascade is two thirds as tall as it is wide, not the three quarters | VERIFIED | 1/1.5 = 2/3 (GI:423-424, 1470). |
| c2-probes | Inside each cascade sits a lattice of probes, one every eight cells. | VERIFIED | INT:166-175 probe_cell_size = grid/16 = 8 cells; GI:566. |
| c2-probes | Sixteen gaps of eight cells span the 128 cells. | VERIFIED | PROBE_DIVISOR 16 (gi.h:563), 128/16 = 8. |
| c2-probes | Counting both ends, that gives seventeen probes along each axis. | VERIFIED | GI:427 `probe_axis_count = PROBE_DIVISOR + 1`. |
| c2-probes | In the first cascade, the probes are 1.6 metres apart. | VERIFIED | 8 x 0.2 m = 1.6 m. |
| c2-probes | So only a handful of probes fall inside the room, and this view draws those probes brighter. | VERIFIED | Lab: 6 m room / 1.6 m spacing is about 3 per horizontal axis; view highlighting is lab behaviour (not checked in a render). |
| c2-scroll | The code turns the camera's position into a cell of the grid. | VERIFIED | GI:1204-1206 (y times y_mult, divided by cell size, `Vector3i` conversion). |
| c2-scroll | Each cascade compares the camera's cell with its own centre cell. | VERIFIED | GI:1209-1223 compares `pos_in_cascade` to `cascade.position`, which is the centre (bounds use `-64 + position`, GI:1467). |
| c2-scroll | When the camera strays more than four cells away, the cascade jumps eight cells to follow it | VERIFIED | GI:1199 drag_margin = (128/16)/2 = 4; GI:1219-1223 `while > position + drag_margin` steps `drag_margin * 2` = 8. |
| c2-scroll | Eight cells is one probe spacing, so after the jump, every probe lands where a probe stood before. | VERIFIED | Probe spacing 8 cells; position starts as a multiple of 8 (GI:568-569) and moves in steps of 8. |
| c2-scroll | The jump carries the cascade's leading edge into new space, an eight-cell slab that holds nothing yet. | VERIFIED | GI:1219-1223 dirty amount 8 on that axis; the slab is voxelized fresh (GI:1458-1510). |
| c2-scroll | That slab is marked dirty, which means it must be voxelized again. | VERIFIED | `dirty_regions` (GI:1221-1222); each non-zero axis is a pending region (RFC:4085-4089). |
| c2-scroll | Only the dirty slab is voxelized, so the rest of the cascade keeps its work. | VERIFIED | GI:1475-1500 region covers only the dirty range; PRE scroll mode shifts the old data (GI:2090-2100). |
| c2-scroll | But if the new part would be more than half the size of the part that is kept, | VERIFIED | GI:1241-1246 `dirty_volume > safe_volume / 2` makes DIRTY_ALL. |
| c2-scroll | ...the whole cascade is rebuilt instead. | VERIFIED | GI:1245 `cascade.dirty_regions = DIRTY_ALL`; get_pending_region_data gives one full 128^3 region (GI:1465-1470). |
| c3-regions | Before the GPU voxelizes the slab, the CPU decides which meshes and lights belong in it. | VERIFIED | SC:3266-3285 (cull loop) runs in `_render_scene`; GPU `render_region` follows (RFC:1273). |
| c3-regions | A cascade can jump along more than one axis in the same frame. | VERIFIED | GI:1213 loop `for j in 0..3` per axis, independent. |
| c3-regions | Each axis that moved gives one dirty slab, called a region. | VERIFIED | RFC:4087-4091; GI:1475-1500. |
| c3-regions | Where two slabs overlap, only the first region keeps the shared corner, so no corner is voxelized twice. | VERIFIED | GI:1489-1497 "chip away previous regions"; the earlier axis's region is not reduced. |
| c3-regions | The CPU turns each region from cells into a box in metres. | VERIFIED | GI:1500-1501 (`r_bounds`), consumed at SC:3401. |
| c3-regions | The box's height is divided by the y scale of one and a half, which undoes the squash | VERIFIED | GI:1500-1501 `Vector3(1, 1.0 / y_mult, 1)`. |
| c3-regions | Here that box is 1.6 metres along the jump, seventeen metres high, and 25.6 metres deep. | VERIFIED | 8 x 0.2 = 1.6; 128 x 0.2 / 1.5 = 17.07; 25.6. |
| c3-geometry | When the camera jumps far, the whole first cascade becomes one region, by the last chapter's rebuild rule. | VERIFIED | GI:1226-1230 (>= 128 on an axis) / GI:1245; one region for DIRTY_ALL (GI:1465). |
| c3-geometry | Every mesh in the scene that is not hidden is tested against that box. | SIMPLIFICATION | SC:3266-3282 tests every entry of `instance_data` against each region AABB. Instances hidden with `visible = false` are not in `instance_data`; but the loop is outside the `!HIDDEN_BY_VISIBILITY_CHECKS` block (SC:3254-3257), so visibility-range/dependency hiding does not exclude a mesh, and the layer mask must match (SC:3279). Also cast-shadows-only meshes are not excluded. |
| c3-geometry | There is no test against the camera's view, so a mesh behind the camera is voxelized too. | VERIFIED | SC:3266-3282 has no frustum or occlusion test (the frustum test is inside the earlier block closed at SC:3256). |
| c3-geometry | Each mesh has a GI mode, and only meshes whose GI mode is Static are voxelized. | VERIFIED | visual_instance_3d.cpp:485-488 Static sets USE_BAKED_LIGHT; SC:3279 requires FLAG_USES_BAKED_LIGHT (set from `baked_light`, SC:1833). |
| c3-geometry | A mesh set to Dynamic still receives SDFGI's light. | VERIFIED | visual_instance_3d.cpp:490-493 Dynamic sets USE_DYNAMIC_GI; RFC:1059 `inst->can_sdfgi` -> INSTANCE_DATA_FLAG_USE_SDFGI. |
| c3-geometry | But it is left out of the voxels, so it adds nothing to the distance field. | VERIFIED | Dynamic has USE_BAKED_LIGHT false, so SC:3279 skips it. |
| c3-lights | Each positional light, an omni or a spot, also has a bake mode, Static or Dynamic. | WRONG | In 4.7.2 positional lights passed to SDFGI include area lights: GI:2024 (`light_area_get_size`, `type == RSE::LIGHT_AREA`), sdfgi_direct_light.glsl:60 `LIGHT_TYPE_AREA`; SC:3270-3271 tests bake mode for any non-directional light. The lab's own panel is such an area light (lab sim.rs:146-153), which contradicts this sentence and chapter 4's. Fix: "Each positional light (omni, spot, or area)". |
| c3-lights | SDFGI computes a dynamic light's effect on the voxels at every light update. | VERIFIED | SC:3680-3682 passes `scenario->dynamic_lights`; GI:1964-2000; refresh spread over frames_to_update_lights frames (GI:1286-1298). |
| c3-lights | SDFGI computes a static light only once, when a region is built. | VERIFIED | RFC:1276-1277 `render_static_lights` only when `update_static` (set in SC:3673 when regions are pending). |
| c3-lights | Directional lights are always dynamic, so a sun is recomputed at every update. | VERIFIED | SC:1627 bake mode only tracked for non-directional lights; directional lights go via `directional_lights` (SC:3678; GI:1914-1945). |
| c3-lights | For each region, the CPU lists the static lights whose range overlaps it. | VERIFIED | SC:3268-3277 (AABB overlap of the light with the region box). |
| c3-lights | But each light also has a maximum cascade, by default the third, and beyond it the light is left out. | VERIFIED | light_storage.h:70 `max_sdfgi_cascade = 2` (cascade index 2 = third); SC:3271 `region_cascade <= max_sdfgi_cascade`; render_static_lights GI:2461. The same limit also applies to dynamic lights (GI:1972), not only static ones. |
| c3-lights | So the coarsest cascade, the fourth, never gets a static light. | VERIFIED | With 4 cascades and default max 2, region_cascade 3 > 2 (SC:3271). Default settings only (light_set_max_sdfgi_cascade can change it; the node API does not expose it). |
| c3-finding | The CPU stores these light lists in slots, one slot per cascade. | VERIFIED | SC:3275 `sdfgi_cascade_lights[sdfgi_last_light_cascade]` indexed by cascade number. |
| c3-finding | But the code that reads them counts the slots differently, by position among the cascades that moved. | VERIFIED | SC:3662-3663 loop `i < cascade_light_count` reads `sdfgi_cascade_lights[i]`; GI:2448-2453 `p_positional_light_cull_result[i]`. |
| c3-finding | The two counts agree only when the cascades that moved are the first ones, in order. | VERIFIED | `cascade_light_index[k]` = k-th moved cascade (SC:3402-3411); stored index = cascade number, read index = k. Equal iff moved cascades are 0..n-1. |
| c3-finding | Suppose only the second cascade moves. | VERIFIED | Scenario (cascade index 1). |
| c3-finding | The code shows that the second cascade's light list is stored in the second slot but read from the first. | VERIFIED | Stored at [1] (SC:3275), read at [0] (SC:3662-3663, GI:2448-2453). |
| c3-finding | The first slot stays empty, because the first cascade did not move. | VERIFIED | Only moved cascades are culled into (SC:3270-3277). |
| c3-finding | So the second cascade gets no static light at all. | VERIFIED | SC:3662-3666: slot 0 empty, so `static_lights_culled` stays false and `update_static` is not set; even if set, render_static_lights would read the empty slot [0] (GI:2448-2453). |
| c3-finding | The readout shows the lab's model losing that light in the same way. | VERIFIED | Lab claim; not re-run, but lab demo logic mirrors SC (demos.rs / sim.rs); not rendered. Treat as lab behaviour, no Godot claim. |
| c4-passes | With the meshes chosen, the GPU draws them into the grid of voxels. | VERIFIED | RFC:1273-1274 `render_region` -> `_render_sdfgi` (RFC:3099). |
| c4-passes | The GPU draws each region three times, with a camera looking along each axis in turn. | VERIFIED | RFC:3132 loop `for i in 0..3`, camera on +axis face looking along -axis (RFC:3146-3152). |
| c4-passes | Each camera's image has one pixel per cell of the region, across the two axes that camera sees. | VERIFIED | RFC:3142-3144 `fb_size = p_size[right_axis], p_size[up_axis]`; empty framebuffer RFC:3170-3172. |
| c4-passes | A surface seen edge-on covers no pixels in that view | VERIFIED | Geometry; ortho rasterization with no area, nothing in the shader to the contrary. |
| c4-passes | But a surface edge-on to one camera faces another, so the three views together cover the room | SIMPLIFICATION | Three orthogonal views cover every axis-aligned surface; an oblique thin surface seen near edge-on in all three could still be missed or sparse. True for the room's shapes. |
| c4-passes | Back-face culling is turned off for every material drawn here, so a surface is drawn from both sides. | VERIFIED | RFC:398-399 `PASS_MODE_SDF` uses `CULL_VARIANT_DOUBLE_SIDED` whatever the material's cull mode. |
| c4-passes | Alpha-blended materials are not drawn at all, so they leave no voxels. | VERIFIED | RFC:1186-1190 for non-color passes only surfaces with FLAG_PASS_DEPTH or FLAG_PASS_OPAQUE are added; alpha-pass-only surfaces are not. (A material with alpha scissor or hash still has the opaque flag.) |
| c4-passes | The lab finds the same fragments a different way, by tracing a ray from each pixel into the scene. | VERIFIED | Lab: voxels.rs:6-9 and 160-186. |
| c4-albedo | Each fragment uses its own 3D position to find the voxel it falls in, and writes there. | VERIFIED | scene_forward_clustered.glsl:2909-2911 `grid_pos = sdf_offset + ivec3(local_pos * sdf_size)`, `local_pos` from `vertex`. |
| c4-albedo | So the fragment's depth, not just its pixel, decides which voxel it fills. | VERIFIED | Same lines: the 3D position (including depth) maps to `grid_pos`. |
| c4-albedo | The voxel's colour, its albedo, is packed into sixteen bits. | VERIFIED | glsl:2913-2917 `albedo16`; texture R16_UINT (GI:443). |
| c4-albedo | One bit marks the voxel as solid, so later passes can tell it from empty space. | VERIFIED | glsl:2913 `albedo16 = 0x1; //solid flag`; PRE init/jump-flood read `c & 1` (PRE:251). |
| c4-albedo | The other fifteen bits hold red, green, and blue, five bits each. | VERIFIED | glsl:2914-2916 shifts 11, 6, 1 with `clamp(uint(c*31),0,31)`. |
| c4-albedo | So each colour channel has thirty-two levels. | VERIFIED | 0..31. |
| c4-albedo | The readout panel decodes the voxel at the top of the sphere, bit by bit. | VERIFIED | Lab readout; the decoding follows glsl:2913-2917. Not rendered. |
| c4-facing | A second 3D texture keeps six more bits per voxel, one for each axis direction. | VERIFIED | `render_geom_facing` R32_UINT (GI:456-457), six bits 0-5 (glsl:2920-2936). |
| c4-facing | Each fragment sets the one bit whose direction is nearest its normal. | VERIFIED | glsl:2929-2938 largest dot against the six directions; `facing_bits = (1 << i)`. |
| c4-facing | When several fragments land in one voxel, their bits are combined. | VERIFIED | glsl:2941-2945 `imageAtomicOr` (or load/or/store with NO_IMAGE_ATOMICS). |
| c4-facing | So a voxel on the sphere's curve can face two or three ways at once. | VERIFIED | Consequence of OR over fragments with different dominant directions. |
| c4-emission | An emissive surface also writes its light into the voxel. | VERIFIED | glsl:2947 `if (length(emission) > 0.001)` then `imageStore(emission_grid...)` (glsl:2994-2995). |
| c4-emission | That light's colour is packed into thirty bits, in a shared-exponent format. | VERIFIED | glsl:2991-2994 RGBE "8985" = 8+9+8+5 = 30 bits; bits 30-31 left for neighbour bits (glsl:2993 comment "2 extra neighbor bits"). |
| c4-emission | Red, green, and blue share one exponent, which lets so few bits cover a wide range of brightness. | VERIFIED | glsl:2968-2991 single `exps` for all channels. |
| c4-emission | A separate texture splits that light over the six axis directions, five bits each | VERIFIED | `emission_aniso_grid`, `light_aniso |= min(31, ...) << (i*5)` (glsl:2956-2963). |
| c4-emission | In this room the panel's light arrives as an area light, so no voxel is emissive. | VERIFIED | Lab: sim.rs:146-153 panel as an area light; sim.rs:175-179 excludes the panel from the voxelized objects. |
| c4-shell | The code shows that the voxels form only the sphere's shell, and nothing fills the inside. | VERIFIED | The shader writes only where fragments land (glsl:2909-2917); no fill pass. |
| c4-shell | That is enough for the distance field of the next chapter, because a ray stops at the shell. | VERIFIED | A march stops when the field is below 0.05 cell (INT:228-231); a ray from outside reaches the shell first. |
| c5-why | The voxels say where surfaces are, but a ray needs to know how far away they are. | VERIFIED | Conceptual; INT:219-232 march by the stored distance. |
| c5-why | So every cell gets the distance to its nearest solid voxel. | VERIFIED | PRE:956-968 (`d = 1.0 + length(seed - pos)`; `d = 0` for solid). |
| c5-why | This slice through the room shows that distance, darker near a surface. | VERIFIED | Lab view; consistent with 0 at solids. Not rendered. |
| c5-why | A ray can then step that far without passing through anything. | VERIFIED | Sphere-tracing; INT:219-233 `advance += distance`. |
| c5-why | Each step lands closer to a surface, until the ray reaches it. | VERIFIED | INT:228-233 hit when `distance < 0.05`. |
| c5-seeds | SDFGI finds each cell's nearest voxel with a jump flood. | VERIFIED | GI:2219-2290; PRE:245-345 (jump flood). |
| c5-seeds | In a jump flood, each cell holds a seed. | VERIFIED | `src_positions` hold a position with `w = 255` for valid (PRE:250-262). |
| c5-seeds | A seed is the nearest solid voxel the cell has found so far. | VERIFIED | PRE:340-345 keeps the nearest `q` found. |
| c5-seeds | The flood runs at half size, sixty-four cells a side, so it does an eighth of the work. | VERIFIED | GI:2219 `half_size = true` hard-coded; GI:2225 `cascade_half_size = 64`; 64^3 = 128^3 / 8. |
| c5-seeds | Each half-size cell covers a block of two by two by two full cells. | VERIFIED | PRE:245-257 `base_pos = pos * 2`, 8 sources. |
| c5-seeds | If one of those is solid, the half-size cell takes it as its first seed. | SIMPLIFICATION | PRE:257-270: if several are solid it picks one by `closest[index % count]`, with `index` from the cell's own parity bits (not the nearest, not the first). With exactly one solid voxel the sentence is exact. |
| c5-seeds | A block with no solid voxel starts with no seed. | VERIFIED | PRE:265-266 `closest_count == 0` stores `uvec4(0)`. |
| c5-flood | In each pass, a cell looks at twenty-six other cells, each a fixed step away along an axis or a diagonal. | VERIFIED | PRE:300-327 26 offsets (3x3x3 minus centre) scaled by `step_size`. |
| c5-flood | The cell keeps whichever of their seeds is nearest to itself. | SIMPLIFICATION | PRE:329-345: it also compares with its own current seed `p`, ignores unused neighbours (`w == 0`) and neighbours outside the grid. |
| c5-flood | The step starts large and halves each pass, from thirty-two cells down to one | VERIFIED | GI:2238-2265 `s = 64; s /= 2` gives 32, 16, 8, 4, 2, 1. |
| c5-flood | So a seed jumps across the room in a few passes | VERIFIED | Six passes cover 63 half-cells; the room is about 30 half-cells wide. |
| c5-flood | A big step can hand a cell a seed that is not its nearest, and the smaller steps after it correct that | VERIFIED | Standard jump-flood property; cells keep the nearest seen (PRE:340-345). |
| c5-flood | The first two passes use the plain version of the shader. | VERIFIED | GI:2243-2258: steps 32 and 16 with `PRE_PROCESS_JUMP_FLOOD`, break once `64 / (s/2) >= 8`. |
| c5-flood | The last four use a version that is faster for small steps, because it keeps each workgroup's nearby cells in shared memory. | VERIFIED | GI:2260-2270 steps 8, 4, 2, 1 with `JUMP_FLOOD_OPTIMIZED`; PRE:40, 352 "optimized version using shared compute memory". |
| c5-upscale | The seeds are then scaled back up to the full one hundred twenty-eight cells. | VERIFIED | GI:2276-2281 upscale dispatch over `cascade_size` (128). |
| c5-upscale | Each full cell takes the seed of its half-size cell. | VERIFIED | PRE:446-475 `imageLoad(src_positions_half, pos >> 1)`. |
| c5-upscale | Then it checks the eight voxels of the block that seed lies in, and keeps a nearer solid one. | VERIFIED | PRE:457-475 `vbase` = seed with low bits cleared, 8 voxels, replaced if nearer and solid. |
| c5-upscale | Searching at half size still leaves some cells with a seed one cell off. | VERIFIED | GI:2282 comment "fix up half size artifacts" (code comment; exact "one cell off" is the lab's measurement). |
| c5-upscale | Those cells are shown here in orange. | VERIFIED | Lab visual; not rendered. |
| c5-upscale | One last pass, with a step of one, fixes them. | VERIFIED | GI:2282-2289 full-size optimized pass `step_size = 1`. |
| c5-store | STORE, the pass that writes the field, keeps one byte per cell. | VERIFIED | PRE:956-968 `dst_sdf`, R8_UNORM (GI:429-436). |
| c5-store | A solid voxel stores zero, and any other cell stores one plus its distance, in cells. | VERIFIED | PRE:956-968 `d = 0` if the seed is the cell, else `1.0 + length(seed - pos)`; `/ 255.0`. |
| c5-store | Despite the name SDFGI, the stored field has no sign, so it never goes negative. | VERIFIED | Same lines; unorm texture. |
| c5-store | The probe and shadow rays that march through the field subtract the one again after they read it. | VERIFIED | INT:227 `... .r * 255.0 - 1.0`; sdfgi_direct_light.glsl:408 same. (The screen reflection march subtracts 1.1, gi.glsl:415; not claimed.) |
| c5-store | So a solid voxel reads as minus one, and its empty neighbour reads as plus one. | VERIFIED | 0 - 1 = -1; a face neighbour stores 1 + 1 = 2, so 2 - 1 = +1. |
| c5-store | A filtered read between those two cells therefore crosses zero halfway | VERIFIED | Linear sampler (`linear_sampler`, INT:227): -1 to +1 crosses 0 at the midpoint. |
| c5-store | The readout shows a cell two cells from a surface, storing three. | VERIFIED | 1 + 2 = 3; lab readout, not rendered. |
| c5-cells | Every solid voxel also goes into a list of sixteen-byte records. | VERIFIED | PRE:1021-1034 `ProcessVoxel` of four uints (PRE:103-109; sdfgi_direct_light.glsl:23-28) = 16 bytes; written to `dst_process_voxels`. |
| c5-cells | A record holds where the voxel is, its albedo and facing, and its stored light. | VERIFIED | PRE:1024-1034 position, albedo (with facing bits 15-20), light, light_aniso. |
| c5-cells | With that list, later passes can visit only the solid cells. | VERIFIED | GI:1305 `dispatch_indirect` over the solid-cell counts; PRE:1043-1046 totals. |
| c5-cells | The record's spare bits hold twenty-six flags, one for each neighbour of the voxel. | VERIFIED | PRE:1005-1034 26 neighbour bits split over albedo 21-31, position 21-31, light 30-31, aniso 30-31. |
| c5-cells | A neighbour's flag is set when that neighbour is empty and this voxel is the nearest surface to it. | VERIFIED | PRE:1013 sets bit when the neighbour's seed equals this voxel (a solid neighbour's seed is itself, so only non-solid ones qualify). |
| c5-cells | With those flags, the light pass can later copy the voxel's light out to those neighbours. | VERIFIED | sdfgi_direct_light.glsl:514-553 `neighbors` read back, `imageStore(dst_light, neighbour_pos, ...)`. |

## Source-index entries cited by chapters 1 to 5: `inspect` text checked

All checked against the checkout and found accurate except the items below.

- `update-light` (c1): "dispatch one thread per solid cell" is loose. The dispatch is indirect over the solid-cell list, but each frame a thread only processes every `frames_to_update`-th cell (`process_offset = frame % N`, `process_increment = N`, GI:1286-1298) unless the cascade is fully dirty. SIMPLIFICATION.
- `prepass-call` (c1): accurate for what it says (only caller of `update_probes`), but the narration uses it to support the pre-pass condition, which it does not show.
- `sdf-decode`, `solid-cell-store`, `process-voxel` (c5): accurate; the albedo-word comment "bits 16-21" vs code 15-20 is confirmed (sdfgi_direct_light.glsl:25 vs PRE:1025).
- `static-light-cull` and `light-bake-route` (c3): accurate, but both say "positional" without saying that the type includes area lights (see the WRONG item).
- Others (frame-order, sdfgi-update, settings, mobile-stub, prepass-gate, ray-count-table, ray-count-hint, project-settings, cascade-create, cell-size-double, y-mult-table, probe-axis-count, probe-position, scroll-update, dirty-all-rule, pending-regions, region-bounds, sdfgi-cull, gi-mode, static-lights-index, render-sdfgi, sdf-double-sided, sdf-fragment, albedo-pack, facing-mask, emission-pack, jump-flood-init, jump-flood, jump-flood-schedule, jump-flood-upscale, sdf-store, render-region, update-probes, process-gi): VERIFIED against the cited lines (line numbers match; `prepass-gate` 2113 also needs `depth_framebuffer.is_valid()`).

## Every non-VERIFIED item

WRONG
1. c1-prepass: "The code shows that the pre-pass runs only when its project setting is turned on." RFC:2112-2113: `depth_pre_pass = (force_depth_pre_pass || setting) && depth_framebuffer.is_valid()`, `force_depth_pre_pass = scene_state.used_opaque_stencil`; a stencil material forces it. The cited `prepass-call` entry does not show this condition.
2. c3-lights: "Each positional light, an omni or a spot, also has a bake mode, Static or Dynamic." Area lights are also positional lights in SDFGI in 4.7.2 (GI:2024, sdfgi_direct_light.glsl:60, SC:3270-3271); the lab's panel is one.

UNCERTAIN
3. c1-room: "This is the lab's study room, lit by the ceiling panel alone." (scene has `ambient 0.12`; not checked it is off at sdfgiMix 0.)
4. c1-room: "With only that direct light, every surface the panel cannot see stays black." (same.)
5. c1-sources: "The build checks that each of those lines is still there." No code reading `lineHash` was found in lab/, tools/ or the package's selftest; only source_index.py writes it.

SIMPLIFICATION
6. c1-switches: "In the other two renderers... SDFGI's functions are empty": `sdfgi_update` is empty, the region queries return 0 / empty.
7. c1-prepass: "So with that setting turned off, the probes stop updating, and the light freezes." Holds unless a stencil material forces the pass; only the probe-lit light freezes.
8. c1-sources: "Every claim in this video cites a file, a symbol, and a line": most but not every claim does.
9. c2-yscale: "The documentation says it trades how far the grid reaches up and down for finer detail in height": paraphrase of Environment.xml:247.
10. c3-geometry: "Every mesh in the scene that is not hidden is tested": visibility-range hiding does not exclude; layer mask must match.
11. c4-passes: "...so the three views together cover the room": true for the room's axis-aligned and round shapes, not in general.
12. c5-seeds: "If one of those is solid, the half-size cell takes it as its first seed": with several solid voxels it picks one by cell parity.
13. c5-flood: "The cell keeps whichever of their seeds is nearest to itself": also compares with its own seed and skips unused/out-of-grid neighbours.
14. Index `update-light` inspect: "one thread per solid cell" (strided by frames_to_update_lights).
