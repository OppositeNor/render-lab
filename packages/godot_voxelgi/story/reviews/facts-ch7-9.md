# Fact check: chapters 7 to 9 (Godot VoxelGI)

Godot checkout at `ed1daf0bf001b61586d9930840f2f1394092c079` (verified with `git rev-parse HEAD`), paths relative to it. Lab paths are relative to the repository. Classes: **verified**, **simplified** (correct but simplified), **misleading** (potentially misleading), **incorrect**, **investigate** (needs investigation). A sentence about the lab is checked against the lab's code. "Not measured" marks a GPU timing the lab shows and this check did not repeat (other GPU jobs were running).


## Chapter 7: Reading it per pixel

| Beat | L | Sentence | Class | Evidence |
|---|---|---|---|---|
| c7-map | 1457 | The texture is now full of light. | verified | lab framing: the texture is written by the light passes (update.rs `light_passes`) |
| c7-map | 1459 | The question is how a pixel reads that light cheaply, and the screen pass answers it every frame. | verified | the screen pass is dispatched in `_pre_opaque_render` each frame a VoxelGI is in view: servers/rendering/renderer_rd/forward_clustered/render_forward_clustered.cpp:1607, servers/rendering/renderer_rd/forward_clustered/render_forward_clustered.cpp:2182 |
| c7-tag | 1470 | First, the screen pass needs to know which VoxelGIs light each pixel. | verified | tag buffer read in `process_gi`: servers/rendering/renderer_rd/shaders/environment/gi.glsl:638-640 |
| c7-tag | 1472 | Each object keeps the first two VoxelGIs it overlaps, so a third one never lights it. | simplified | `pair_voxel_gi_instances` keeps `[0]`,`[1]` of the culler list (%s:5060-5071; list built from `geom->voxel_gi_instances`, renderer_scene_cull.cpp:3205-3215). Godot's HashSet iterates in insertion order, but `erase` moves the last key into the freed slot (core/templates/hash_set.h:290-293), so "first two it overlaps" holds until an overlap ends |
| c7-tag | 1476 | A pass called the depth pre-pass draws depth before the main shading. | simplified | `PASS_MODE_DEPTH_NORMAL_ROUGHNESS_VOXEL_GI` with a VoxelGI in view (servers/rendering/renderer_rd/forward_clustered/render_forward_clustered.cpp:1926-1927); the pre-pass itself runs only if `rendering/driver/depth_prepass/enable` (default true) or an opaque stencil forces it (servers/rendering/renderer_rd/forward_clustered/render_forward_clustered.cpp:2112-2113) |
| c7-tag | 1480 | It writes those two VoxelGIs' indices into every pixel it draws. | verified | servers/rendering/renderer_rd/shaders/forward_clustered/scene_forward_clustered.glsl:3030-3035: `voxel_gi_buffer.x/y = index & 0xFF` of `gi_offset`, in `MODE_RENDER_VOXEL_GI` (the pre-pass); index = render slot from `scene_state.voxelgi_ids` (servers/rendering/renderer_rd/forward_clustered/render_forward_clustered.cpp:1040-1056) |
| c7-tag | 1482 | That takes two bytes per pixel. | verified | `get_voxelgi_format` = `DATA_FORMAT_R8G8_UINT`: servers/rendering/renderer_rd/forward_clustered/render_forward_clustered.cpp:272 |
| c7-tag | 1484 | An index of two hundred fifty-five means none. | simplified | 0xFF is written for drawn pixels of objects without a VoxelGI (%s:3037-3038). Pixels the pre-pass never draws keep its clear colour (0,0,0,0) (%s:1948-1950, 2142), i.e. index 0, not 255 |
| c7-tag | 1486 | So the screen pass knows, for each pixel, which VoxelGIs to read. | verified | `process_gi` tests `i < max_voxel_gi_instances` against the tag: servers/rendering/renderer_rd/shaders/environment/gi.glsl:654-657 |
| c7-tag | 1490 | Their settings are kept in one buffer, shared by up to eight VoxelGIs in view. | verified | `VoxelGIData data[MAX_VOXEL_GI_INSTANCES]` with MAX = 8, one uniform buffer: servers/rendering/renderer_rd/shaders/environment/gi.glsl:80,98,102; `setup_voxel_gi_instances` loops to 8: servers/rendering/renderer_rd/environment/gi.cpp:3838-3908 |
| c7-tag | 1494 | This sphere's pixels name the room's only VoxelGI. | simplified | true of what Godot would write for this room (one VoxelGI, index 0). The lab has no tag buffer, because there is one VoxelGI (README "What the lab simplifies"); say "in Godot, this sphere's pixels would name" |
| c7-space | 1505 | Take this ringed pixel, on the side of the sphere that faces the red wall. | investigate | lab visual: the left wall is the red one (packages/render_lab/scenes/study-room.lam:67-71, albedo 0.70 0.24 0.18); that the ringed pixel (azimuth 60 to 100) faces it was not checked from a render |
| c7-space | 1509 | The pass moves the pixel's position and normal into the box's cells. | verified | servers/rendering/renderer_rd/shaders/environment/gi.glsl:534-541 (`xform` applied to position, ref_vec, normal, then `normal_bias`) |
| c7-space | 1513 | A pixel outside the box gets nothing from this VoxelGI. | verified | servers/rendering/renderer_rd/shaders/environment/gi.glsl:541-543: position below 0 or above `bounds` after the normal bias returns with no light |
| c7-space | 1517 | An edge fade then weights the light this VoxelGI gives the pixel. | verified | `blend` multiplies `out_diff`/`out_spec` and is summed in `out_blend`: servers/rendering/renderer_rd/shaders/environment/gi.glsl:595,603,605 |
| c7-space | 1521 | The fade is one at the box's centre and falls to zero at its faces. | verified | `blend = clamp(1 - max(\|2p/bounds - 1\|), 0, 1)`: servers/rendering/renderer_rd/shaders/environment/gi.glsl:548-549 (linear in the box's max-norm: 1 at the centre, 0 on the faces) |
| c7-space | 1525 | The fade exists to blend two boxes that overlap. | simplified | the code uses the fade as a weight between VoxelGIs (`blend_accum`, servers/rendering/renderer_rd/shaders/environment/gi.glsl:660-663); no comment states why. Say "The fade weights overlapping boxes against each other" |
| c7-space | 1527 | But with one box, the weighted light is then divided by that same fade, so the fade cancels out. | verified | `amb_accum /= blend_accum` when `blend_accum > 0`: servers/rendering/renderer_rd/shaders/environment/gi.glsl:660-663 |
| c7-space | 1531 | Only a pixel exactly on a face, where the fade is zero, loses the box's light. | verified | fade 0 adds 0 to the sums and to `blend_accum`, which then stays 0, so the division is skipped and the result is 0: servers/rendering/renderer_rd/shaders/environment/gi.glsl:660-663 |
| c7-low | 1544 | At the default quality, four cones leave the pixel at forty-five degrees from its normal, each with a weight of one quarter. | verified | `quality` default 0 = Low (`GLOBAL_DEF`, servers/rendering/rendering_server.cpp:3709; gi.h:552); four cones at (0.707,0,0.707) etc., weights 0.25: servers/rendering/renderer_rd/shaders/environment/gi.glsl:575-587 |
| c7-low | 1548 | For a cone this wide, the radius at any point equals the distance travelled. | simplified | `radius = max(0.5, dist)` (servers/rendering/renderer_rd/shaders/environment/gi.glsl:508-526): equal to the distance from half a cell on |
| c7-low | 1552 | Each step advances by that radius, so it doubles the distance and reads one mip coarser, where a texel is twice as wide. | verified | `dist += radius; radius = max(0.5, dist)`, `lod_level += 1.0` (servers/rendering/renderer_rd/shaders/environment/gi.glsl:521-524); with bias 1.4 or 1.5 the distance doubles each step |
| c7-low | 1556 | That texel already averages everything in the cone's footprint, so one sample stands in for many. | simplified | a parent texel is the sum of its children over `mix(8, count, propagation)` (voxel_gi.glsl:526-543), an average only at propagation 1 (default 0.5) |
| c7-low | 1560 | The samples are blended front to back, as in the bounce. | verified | `color += (1 - color.a) * sample` in both marches: servers/rendering/renderer_rd/shaders/environment/gi.glsl:496-499,521-523 (bounce: voxel_gi.glsl:497-510) |
| c7-low | 1564 | The cone stops when its alpha reaches one, or when it leaves the box. | simplified | `while (dist < max_distance && color.a < 1.0)` plus the break outside the box (servers/rendering/renderer_rd/shaders/environment/gi.glsl:513-519): it also stops at the box's diagonal |
| c7-high | 1577 | At High quality, the pixel traces the bounce's six cones instead. | verified | `params.high_quality_vct` (quality High, gi.cpp:4043): six cones, dirs, weights 0.25/0.15 and tan 0.577 identical to the bounce (servers/rendering/renderer_rd/shaders/environment/gi.glsl:558-575 vs voxel_gi.glsl:480-490) |
| c7-high | 1581 | Each of those cones is narrower, thirty degrees from its axis to its edge. | verified | tan 0.577 = tan 30 degrees: servers/rendering/renderer_rd/shaders/environment/gi.glsl:572 |
| c7-high | 1585 | Each takes smaller steps, half its width at a time. | verified | `dist += half_diameter` in `voxel_cone_trace`: servers/rendering/renderer_rd/shaders/environment/gi.glsl:487-505 (for a 45 degree cone the step is also half the width, so the steps are smaller because the cone is narrower) |
| c7-high | 1589 | So the six cones take more samples, each from a finer mip. | verified | diameter 1.154 x dist against 2 x dist, so a finer mip (lod log2 of it); the lab counts 11 steps against 7 (cost.rs, bias 1.5) |
| c7-high | 1593 | At the default, four wide cones do the work. | verified | default quality is Low, four cones: gi.h:552, servers/rendering/rendering_server.cpp:3709 |
| c7-spec | 1605 | The reflection gets one more cone, along the mirror direction. | verified | `voxel_cone_trace(..., ref_vec, tan(roughness*0.5*PI*0.99), ...)`: servers/rendering/renderer_rd/shaders/environment/gi.glsl:598-605; `reflection = reflect(-view, normal)`: servers/rendering/renderer_rd/shaders/environment/gi.glsl:631 |
| c7-spec | 1609 | That cone's width follows the surface's roughness. | verified | tangent of the half angle is a function of roughness: servers/rendering/renderer_rd/shaders/environment/gi.glsl:598 |
| c7-spec | 1611 | The pass squares the roughness first. | verified | `roughness *= roughness` before the loop: servers/rendering/renderer_rd/shaders/environment/gi.glsl:644 |
| c7-spec | 1613 | On a smooth sphere, the cone is narrow and travels far. | verified | tan(0.0225*0.5*pi*0.99) at roughness 0.15 is 2 degrees; the march steps half a cell at a time up to the box diagonal (cost.rs: 381 steps at roughness 0) |
| c7-spec | 1617 | On a rough one, it opens seventy degrees from its axis. | simplified | the lab's sphere is at roughness 0.9 there (storyboard.lam:1620 range 0.15 to 0.9): half angle 0.9^2 x 0.99 x 90 = 72.2 degrees; roughness 1 gives 89.1. Say "about seventy" or "seventy-two" |
| c7-spec | 1621 | The sphere's reflection of the room follows the roughness, from sharp to blurred. | verified | lab visual; follows from the cone width above |
| c7-scale | 1636 | The cones' light is multiplied back by the dynamic range, which undoes the texture's division. | verified | `light.rgb *= dynamic_range * exposure_normalization`: servers/rendering/renderer_rd/shaders/environment/gi.glsl:590 (the stored texel is light / dynamic_range: voxel_gi.glsl:550) |
| c7-scale | 1640 | It is also multiplied by the VoxelGI's energy, a brightness setting. | simplified | energy is folded into `dynamic_range` on the CPU (`voxel_gi_get_dynamic_range * voxel_gi_get_energy`, servers/rendering/renderer_rd/environment/gi.cpp:3893); the shader also multiplies by the camera exposure normalization |
| c7-scale | 1644 | The cones' alpha says how much of their view they found filled. | verified | `light` is a premultiplied front-to-back sum, `light.a` is the accumulated coverage: servers/rendering/renderer_rd/shaders/environment/gi.glsl:558-590 |
| c7-scale | 1646 | With the interior setting on, that alpha is forced to one. | verified | `if (!blend_ambient) light.a = 1.0` (also for the reflection): servers/rendering/renderer_rd/shaders/environment/gi.glsl:591-593,599-601; `blend_ambient = !interior`: servers/rendering/renderer_rd/environment/gi.cpp:3896 |
| c7-scale | 1648 | So the environment's light never mixes in. | verified | the opaque pass mixes the environment by (1 - alpha): servers/rendering/renderer_rd/shaders/forward_clustered/scene_forward_clustered.glsl:2023-2024 |
| c7-scale | 1650 | That alpha is stored with the light in the screen pass's buffers. | verified | `imageStore(ambient_buffer, pos, ambient_light)` RGBA16F: servers/rendering/renderer_rd/shaders/environment/gi.glsl:726-727; servers/rendering/renderer_rd/environment/gi.cpp:3996 |
| c7-quirk | 1664 | Before any of this, the pass checks that the pixel's normal is valid. | verified | servers/rendering/renderer_rd/shaders/environment/gi.glsl:620 `if (normal.length() > 0.5)` |
| c7-quirk | 1668 | But that test is always true. | verified | `vec3.length()` is the int 3 in GLSL |
| c7-quirk | 1670 | In GLSL, Godot's shading language, a vector's length method returns its component count, three. | misleading | GLSL is the language the renderer's shaders are written in, not "Godot's shading language" (that is the .gdshader language, which has no `.length()` method). Say "GLSL, the language of Godot's renderer shaders" |
| c7-quirk | 1674 | So every pixel is traced, whatever its normal. | misleading | the check passes for every pixel, but a pixel is traced only if its tag names a VoxelGI (servers/rendering/renderer_rd/shaders/environment/gi.glsl:654-657): objects without one (tag 255) trace nothing. Say "So the check never rejects a pixel" |
| c7-recap | 1684 | So every pixel traces four cones and a reflection cone through the texture. | misleading | only pixels whose tag names a VoxelGI trace; at Low quality, four cones plus the reflection cone (servers/rendering/renderer_rd/shaders/environment/gi.glsl:575-605). Say "every pixel of this room traces..." |
| c7-recap | 1688 | It also keeps how much of its view those cones found filled. | verified | alpha kept in the buffers: servers/rendering/renderer_rd/shaders/environment/gi.glsl:726-727 |
| c7-recap | 1692 | The light and its alpha go into buffers, which the opaque pass turns into colour. | verified | `mix(ambient_light, buffer_ambient.rgb, buffer_ambient.a)`: servers/rendering/renderer_rd/shaders/forward_clustered/scene_forward_clustered.glsl:2023 |
| c7-sources | 1698 | These are the screen pass's lines, from the per-pixel tag to the scaled cones. | verified | meta: the chapter's guide entries (source-index.json "chapters" 7) |

### Source-index entries of chapter 7 (inspect and details)

| Entry | Class | Evidence |
|---|---|---|
| `pairing-two` | simplified | see L1472: insertion order, changed by an erase |
| `instance-ubo` | verified | servers/rendering/renderer_rd/environment/gi.cpp:3838-3908 (xform, bounds, range x energy at 3893, bias, normal bias, blend_ambient at 3896, exposure) |
| `instance-ubo` detail 3893 (energy folds into the range) | verified | line checked in the checkout |
| `instance-ubo` detail 3896 (interior turns off the environment's share) | verified | line checked in the checkout |
| `tag-format` | verified | servers/rendering/renderer_rd/forward_clustered/render_forward_clustered.cpp:272 |
| `tag-write` | verified | servers/rendering/renderer_rd/shaders/forward_clustered/scene_forward_clustered.glsl:3030-3038 |
| `process-gi` | verified | servers/rendering/renderer_rd/environment/gi.cpp:3968-4000 |
| `process-gi` detail 3996 (the ambient buffer) | verified | line checked in the checkout |
| `normal-roughness` | simplified | `encode24` touches the unit cube and then multiplies by a best-fit factor from `best_fit_normal_texture` (servers/rendering/renderer_rd/shaders/forward_clustered/scene_forward_clustered.glsl:1185-1200), which the entry leaves out; roughness x 127/255 and mirrored 1 - r for dynamic: 3021-3027 |
| `normal-roughness` detail 3026 (a dynamic object's mirrored roughness) | verified | line checked in the checkout |
| `resolve-gi` | verified | resolve.glsl:75-183 (for 8x, `mix(best_index0, best_index1, min_f0 < min_f1)` picks the second half when the first count is smaller); 2x line 115; `resolve_gi` writes to destination images: effects/resolve.cpp:72-110 |
| `resolve-gi` detail 115 (two samples: the nearer) | verified | line checked in the checkout |
| `reconstruct-position` | verified | servers/rendering/renderer_rd/shaders/environment/gi.glsl:174-187 (integer pixel, no +0.5) |
| `normal-length-quirk` | verified | servers/rendering/renderer_rd/shaders/environment/gi.glsl:620 |
| `instance-loop` | verified | servers/rendering/renderer_rd/shaders/environment/gi.glsl:654-667 |
| `instance-loop` detail 661 (divided by the fades) | verified | line checked in the checkout |
| `instance-loop` detail 667 (over SDFGI, by its alpha) | verified | line checked in the checkout |
| `voxel-gi-compute` | verified | servers/rendering/renderer_rd/shaders/environment/gi.glsl:533-546 |
| `edge-blend` | verified | servers/rendering/renderer_rd/shaders/environment/gi.glsl:548-549 |
| `cones-low` | verified | servers/rendering/renderer_rd/shaders/environment/gi.glsl:575-587 |
| `cone-trace-45` | simplified | `radius = max(0.5, dist)`: servers/rendering/renderer_rd/shaders/environment/gi.glsl:508-526; "equals the distance" from half a cell on |
| `cones-high` | verified | servers/rendering/renderer_rd/shaders/environment/gi.glsl:558-573 |
| `cone-trace` | verified | servers/rendering/renderer_rd/shaders/environment/gi.glsl:487-505 |
| `spec-cone` | verified | servers/rendering/renderer_rd/shaders/environment/gi.glsl:598-605, squared at 644 |
| `spec-cone` detail 644 (squared first) | verified | line checked in the checkout |
| `range-restore` | verified | servers/rendering/renderer_rd/shaders/environment/gi.glsl:590-593 |

## Chapter 8: Keeping it fast

| Beat | L | Sentence | Class | Evidence |
|---|---|---|---|---|
| c8-map | 1704 | Before the opaque pass turns those buffers into colour, this chapter looks at the bill. | verified | order: `_pre_opaque_render` (screen pass) before the opaque pass: servers/rendering/renderer_rd/forward_clustered/render_forward_clustered.cpp:2182 |
| c8-map | 1706 | It asks what all of this costs, and how Godot keeps that cost small. | verified | meta |
| c8-map | 1708 | The work comes at three rates. | verified | meta (bake, light passes, screen pass) |
| c8-map-bake | 1712 | The bake runs once, in the editor. | simplified | `VoxelGI::bake` works "from the editor and in exported projects" (doc/classes/VoxelGI.xml:24; scene/3d/voxel_gi.cpp:434). Say "The bake runs once, usually in the editor" |
| c8-map-light | 1716 | The light passes run only when something changes. | simplified | also when the data or one of three settings change (gi.cpp:256,271,344) and, with dynamic objects, the write pass every frame (servers/rendering/renderer_rd/environment/gi.cpp:3067-3171) |
| c8-map-cone | 1720 | The screen pass runs every frame. | simplified | every frame a VoxelGI is in view (`process_gi` is called when `render_gi`, servers/rendering/renderer_rd/forward_clustered/render_forward_clustered.cpp:1606-1608) |
| c8-panel | 1733 | The panel on the right times the graphics card's work. | verified | lab panel: `performance_view` (lab/src/renderer/inspect.rs:223): GPU timestamps |
| c8-panel | 1735 | Its line is each frame's total time, so a spike is a frame that did more work. | simplified | the panel draws recent frames' GPU and frame times against the budgets (lab/src/ui/graph_panel.rs:89-93); "total time" is the GPU time |
| c8-panel | 1739 | Below the line, each row is one pass, averaged over the last frames. | verified | `summary_last(30)`: mean over the last 30 frames (lab/src/renderer/inspect.rs:233) |
| c8-panel | 1741 | A row shows the pass's name, a bar for its share of the frame, and its average time in milliseconds. | simplified | each row also keeps the 95th percentile and the last time (graph_panel.rs:66); the bar is the share of the GPU time |
| c8-panel | 1743 | The row called scene pass is the room's own shading, and it takes most of the frame. | investigate | a measurement of the lab (the "scene pass" row is `timing: "scene pass"`, lab/src/graph/mod.rs:311); that it takes most of the frame was not measured here (GPU busy) |
| c8-panel | 1745 | The screen pass is the row named voxelgiGiCS, after its compute shader. | verified | pass `voxelgiGiCS` in packages/godot_voxelgi/graphs/frame.toml:47 |
| c8-panel | 1747 | This chapter shows how Godot keeps that row small. | verified | meta |
| c8-panel | 1749 | These times are this machine's. | verified | lab: timings are this machine's |
| c8-panel | 1751 | Watch for the rows that appear only when their work actually runs. | verified | `passes` are those the last measured frame ran (inspect.rs:224-236) |
| c8-cache | 1760 | Right now, no light pass is listed. | verified | rows only for passes that ran last frame (inspect.rs); the plan runs the light passes only on a changed key (update.rs:213-232) |
| c8-cache | 1762 | Nothing changed, so Godot does no light-pass work at all. | simplified | true of this room, which has no dynamic object: with one, the write pass and the plot run every frame (servers/rendering/renderer_rd/environment/gi.cpp:3067,3118-3171; update.rs:223-239) |
| c8-cache | 1764 | Each frame, on the processor, the culler compares every visible light near the box with the copy it saved. | verified | `render_probes` runs on the CPU each frame for a VoxelGI in view and compares each paired light and each directional light with `probe->light_cache`: renderer_scene_cull.cpp:3906-3990 |
| c8-cache | 1768 | The light passes run only when a light, the data, or one of three settings has changed. | verified | `voxel_gi_needs_update` compares versions (servers/rendering/renderer_rd/environment/gi.cpp:4302); versions bump in allocate_data (gi.cpp:188), set_dynamic_range (256), set_propagation (271), set_use_two_bounces (344); energy, bias, normal bias, interior do not |
| c8-cache | 1772 | Those settings are the dynamic range, propagation, and the second bounce. | verified | same lines: dynamic range, propagation, use_two_bounces |
| c8-cache | 1776 | To measure the light passes, the lab now relights the room every frame, which Godot does only when something changed. | verified | lab: `giRelightEachFrame` (update.rs:213) |
| c8-cache | 1778 | Four rows appear, for the direct light, the bounce, the mips, and the writes. | verified | passes `voxelgiLightCS`, `voxelgiBounceCS`, `voxelgiMipmapsCS`, `voxelgiWriteCS` (frame.toml:23) |
| c8-cache | 1781 | On this machine, the bounce takes the longest, because it traces six cones from every leaf that has a normal. | investigate | a measurement ("on this machine"); that the bounce is the slowest row was not measured here (GPU busy). The cause given matches the code: six cones per leaf with `length(normal) > 0.2` (voxel_gi.glsl:465-518) |
| c8-cache | 1783 | } When the lab returns to Godot's behaviour, those rows are gone, so a still room's light costs nothing per frame. | verified | lab: update.rs plan; note: the sentence file shows a stray "}" before "When the lab returns" (storyboard.lam:1783, `unhighlight}}}`), check it is not spoken |
| c8-sparse | 1796 | The readout on the right counts what one update of the light passes costs, and what one frame costs. | verified | lab readout (cost.rs) |
| c8-sparse | 1798 | This chapter's numbers come from it. | verified | lab readout |
| c8-sparse | 1800 | When the light passes do run, they visit only the cells the bake made. | verified | dispatches cover each level's run of `cell_offset`/`cell_count`: servers/rendering/renderer_rd/environment/gi.cpp:3128-3171 |
| c8-sparse | 1804 | These are the cells shown here, not the empty grid around them. | verified | lab visual |
| c8-sparse | 1808 | That is forty-two thousand leaves, against more than a million texels in the texture's finest mip. | verified | lab's room: selftest `check_cost_numbers` (selftest.rs:487-507): 42,289 leaves, mip 0 = 1,048,576 texels; the texture is dense at the octree size (servers/rendering/renderer_rd/environment/gi.cpp:2636-2643) |
| c8-levels | 1825 | The cells are stored sorted by level, with the root first and the leaves last. | verified | `Voxelizer::_sort`: level first (scene/3d/voxelizer.cpp:538-596), root index 0 (line 560); leaves are the last level; mipmaps[0] is the leaf level (servers/rendering/renderer_rd/environment/gi.cpp:2658-2662) |
| c8-levels | 1829 | So each level is one run of the array, one colour in this bar. | verified | `mipmap.cell_offset`/`cell_count` per level: servers/rendering/renderer_rd/environment/gi.cpp:2658-2666 |
| c8-levels | 1833 | Each run is one dispatch, one call to the graphics card, with sixty-four cells to a workgroup. | verified | one dispatch per level (`wg_todo` split only above the workgroup limit, %s:3138-3146); `local_size_x = 64`: voxel_gi.glsl:10 |
| c8-levels | 1837 | The first light pass begins with the leaves. | verified | `i == 0` is the leaf level: servers/rendering/renderer_rd/environment/gi.cpp:3117-3119 |
| c8-levels | 1841 | Then the mip pass climbs one level per dispatch, with a barrier between dispatches. | verified | `for i in mipmaps` with `compute_list_add_barrier` before each (`pass == 1 \|\| i > 0`): servers/rendering/renderer_rd/environment/gi.cpp:3116-3125 |
| c8-levels | 1845 | The barrier is there because a parent sums its children's light, so the children must be finished first. | simplified | the mip pass sums the children and divides by `mix(8, count, propagation)` (voxel_gi.glsl:526-543); "sums" is true, the result is not a plain sum |
| c8-levels | 1849 | Then eight writes, one for each level of the octree, copy the cells into the texture's mips. | verified | write pass over all `mipmaps` after each pass: servers/rendering/renderer_rd/environment/gi.cpp:3151-3171 |
| c8-levels | 1853 | With the second bounce on, one update of this room takes thirty-two dispatches. | verified | 2 passes x (8 light/mip dispatches + 8 writes) = 32 for 8 levels (servers/rendering/renderer_rd/environment/gi.cpp:3097-3171); lab's count 32 (selftest.rs:496) |
| c8-levels | 1857 | The lab's light passes follow the same order on the graphics card. | verified | lab: update.rs `light_passes` mirrors it |
| c8-march | 1870 | Inside the light pass, a shadow ray reads the distance field. | simplified | only for a light with shadows (`has_shadow`, voxel_gi.glsl:297-340) |
| c8-march | 1874 | The field says how far the nearest surface is, so the ray can safely jump that far. | simplified | the field value is `byte * 255 - 1` cells (voxel_gi.glsl:182): it jumps one cell short of the nearest surface |
| c8-march | 1878 | So its steps are long in open air, and short only near a surface. | verified | voxel_gi.glsl:180-197 |
| c8-march | 1882 | A step is never shorter than one cell along the ray's main axis. | verified | `advance = max(distance_adv, advance - mod(advance, distance_adv))`, `distance_adv = 1 / \|dominant component\|`: voxel_gi.glsl:191,258-277,298 |
| c8-march | 1886 | So the march is never slower than a walk from cell to cell. | verified | same: a step is at least one cell on the dominant axis, as a cell-to-cell walk |
| c8-march | 1890 | A light that is out of range, too dim, or behind the cell is skipped before any march starts. | verified | `compute_light_vector` (radius, spot angle) and the `attenuation < 0.01` / back-face test before the march: voxel_gi.glsl:206-241,284-296; a spot-cone miss also skips |
| c8-cones | 1902 | Everything so far runs only on change, but the screen pass runs every frame, so its price is paid per pixel. | verified | the screen pass is dispatched per frame, per pixel: servers/rendering/renderer_rd/environment/gi.cpp:4267-4270 |
| c8-cones | 1904 | At the default quality, a pixel traces four cones. | verified | Low is the default quality: gi.h:552 |
| c8-cones | 1908 | In this box, each cone takes at most seven steps, so a pixel takes at most twenty-eight samples. | simplified | the lab's count (cost.rs `steps_45`, bias 1.5 = VoxelGIData's default, box diagonal 192): 7 steps, 28 samples. With 1.4, the server's own default, it is 8 steps (the walk 1.4, 2.8, ... 179.2). Say "at bias one and a half"; "at most" is right since cones also stop at alpha 1 or the box edge |
| c8-cones | 1912 | Each sample reads a prefiltered mip as wide as the cone at that point. | verified | `lod_level` = log2(2 radius) = log2(diameter): servers/rendering/renderer_rd/shaders/environment/gi.glsl:511,528 |
| c8-cones | 1916 | That texel is already the average of the cells it covers, a cheap stand-in for many rays. | simplified | an average at propagation 1 only (see L1556) |
| c8-cones | 1920 | The price is that thin walls blur. | simplified | an inference (a mip texel blends the cells it covers); not stated in the source. Say "so detail thinner than a texel is lost" |
| c8-cones | 1922 | At High, six narrower cones widen more slowly. | verified | tan 0.577 cone: diameter 1.154 x dist against 2 x dist |
| c8-cones | 1930 | So their steps are shorter and more numerous, and the screen pass's row grows. | verified | 11 against 7 steps (cost.rs, bias 1.5); the row's growth was not measured here |
| c8-cones | 1932 | The documentation says High also gives better reflections. | verified | doc/classes/ProjectSettings.xml:3079-3081: "more precise lighting and better reflections" |
| c8-cones | 1936 | But only the diffuse cones for ambient light change, and the reflection cone stays the same. | verified | `high_quality_vct` appears only in the diffuse branch: servers/rendering/renderer_rd/shaders/environment/gi.glsl:121,558 |
| c8-spec | 1954 | That reflection cone is the one to watch, because its width follows the roughness. | verified | servers/rendering/renderer_rd/shaders/environment/gi.glsl:598 |
| c8-spec | 1958 | A fully rough surface takes at most two steps, and a mirror at most three hundred eighty-one. | simplified | lab's box (diagonal 192) at bias 1.5: 2 steps at roughness 1 and 381 at roughness 0 (selftest.rs:499-500); at bias 1.4 they are 2 and 382. Say "in this box" |
| c8-spec | 1962 | A mirror's cone is nearly a ray, so it steps only half a cell at a time. | verified | tan 0 gives `diameter = max(1, 0) = 1`, `dist += 0.5`: servers/rendering/renderer_rd/shaders/environment/gi.glsl:487-505 |
| c8-spec | 1966 | Steps that short never jump over a wall. | simplified | a half-cell step cannot skip a cell, and the sampling is trilinear; a wall thinner than a cell is still not exact |
| c8-spec | 1968 | So smooth surfaces are the most expensive pixels. | verified | most steps at roughness 0 (381 against 2) |
| c8-half | 1991 | One project setting, half resolution, trades quality for speed directly, and it is off by default. | verified | `rendering/global_illumination/gi/use_half_resolution` default false: servers/rendering/rendering_server.cpp:3707, ProjectSettings.xml:3062 |
| c8-half | 1993 | On the left is the frame at full resolution, and on the right the frame with the setting on. | verified | lab: the A/B beat (`half-res` value) |
| c8-half | 1995 | The ambient and reflection buffers become half as wide and half as tall. | verified | `size.x >>= 1; size.y >>= 1`: servers/rendering/renderer_rd/environment/gi.cpp:3990-3993 |
| c8-half | 1999 | Each of their texels traces only the top-left pixel of its two by two block. | verified | `pos <<= 1` in `main`, `pos >>= 1` before the store: servers/rendering/renderer_rd/shaders/environment/gi.glsl:708,723 |
| c8-half | 2001 | So only a quarter of the pixels are traced, and the screen pass's row shrinks with them. | verified | `compute_list_dispatch_threads(w >> 1, h >> 1)`: servers/rendering/renderer_rd/environment/gi.cpp:4266-4270; the row's shrink was not measured here |
| c8-half | 2003 | Here are the ambient buffers at full and half size. | verified | lab |
| c8-half | 2005 | Without multisampling, the opaque pass reads the half-size one with a plain bilinear blend. | verified | `gi_upscale_for_msaa` is true only when MSAA is on (servers/rendering/renderer_rd/forward_clustered/render_forward_clustered.cpp:728-731); else `coord = screen_uv`, sampled with `SAMPLER_LINEAR_CLAMP`: servers/rendering/renderer_rd/shaders/forward_clustered/scene_forward_clustered.glsl:1982-2019 |
| c8-half | 2007 | That blend does not weight by depth. | verified | no depth term in the fetch: servers/rendering/renderer_rd/shaders/forward_clustered/scene_forward_clustered.glsl:2008-2020 |
| c8-half | 2009 | So along the sphere's edge, texels traced on the wall and on the sphere are blended together. | verified | follows from the two lines above (the traced pixel of a block may be on the wall) |
| c8-half | 2013 | The edge turns ragged, in steps two pixels wide. | simplified | lab observation; one half-size texel is 2 pixels wide, consistent with L2017 |
| c8-half | 2015 | These are the artifacts on polygon edges that the documentation warns of. | verified | ProjectSettings.xml:3062: "artifacts that may be visible on polygon edges" |
| c8-half | 2017 | Each artifact is one half-size texel wide, so at a higher resolution they shrink. | verified | ProjectSettings.xml:3062: "less noticeable as the viewport resolution increases" |
| c8-recap | 2027 | So VoxelGI pays for little each frame. | simplified | a judgement, supported by the chapter |
| c8-recap | 2029 | Its light passes run only on change, and only over occupied cells. | verified | gi.cpp:3067,4302; dispatch over cells only |
| c8-recap | 2033 | Each pixel takes a few cone samples, and half resolution traces only a quarter of the pixels when it is on. | verified | steps above; half resolution servers/rendering/renderer_rd/environment/gi.cpp:4267 |
| c8-recap | 2037 | Each saving has its price. | verified | each trade-off above |
| c8-recap | 2039 | A moving light must relight the room, coarse cones blur thin walls, and half resolution frays the edges. | verified | light cache (renderer_scene_cull.cpp:3943-3962), mip averaging, half-resolution fetch |
| c8-recap | 2043 | What is left is to turn the buffers into colour. | verified | the opaque pass |
| c8-sources | 2047 | These are the lines that keep it fast, from the gate to the half-size buffers. | verified | meta: the chapter's guide entries (source-index.json "chapters" 8) |

### Source-index entries of chapter 8 (inspect and details)

| Entry | Class | Evidence |
|---|---|---|
| `relight-gate` | verified | servers/rendering/renderer_rd/environment/gi.cpp:4296-4303; version bumps gi.cpp:188,256,271,344; renderer_scene_cull.cpp:3906-3990; servers/rendering/renderer_rd/environment/gi.cpp:3067 |
| `relight-gate` detail 251 (a setter that bumps the version) | verified | line checked in the checkout |
| `update-gate` | verified | servers/rendering/renderer_rd/environment/gi.cpp:3067 |
| `light-cache` | verified | renderer_scene_cull.cpp:3922-4000 (fields match the comparisons, point and directional loops) |
| `sort-cells` | verified | scene/3d/voxelizer.cpp:538-596 |
| `sort-cells` detail 560 (the root first) | verified | line checked in the checkout |
| `level-dispatch` | verified | servers/rendering/renderer_rd/environment/gi.cpp:3127-3146 ((count + 63) / 64 with `wg_size = 64`) |
| `level-dispatch` detail 3125 (a barrier between levels) | verified | line checked in the checkout |
| `local-size` | verified | voxel_gi.glsl:10 and 411 (`cell_index >= params.cell_count` returns) |
| `pass-order` | misleading | "Without a change, only the write runs" (detail at 3100): the write-only pass runs only when a dynamic object is or was plotted; with nothing changed and none plotted the gate at gi.cpp:3067 skips everything (see update-gate). Say "Without a light change, only the write runs, and only while dynamic objects are plotted" |
| `pass-order` detail 3100 (nothing changed: write only) | misleading | line checked in the checkout |
| `raymarch` | verified | voxel_gi.glsl:180-197 |
| `raymarch` detail 190 (a distance below one cell darkens partly) | verified | line checked in the checkout |
| `light-at-pos` | verified | voxel_gi.glsl:284-340 |
| `light-at-pos` detail 294 (the back-face test) | verified | line checked in the checkout |
| `cones-low` | verified | servers/rendering/renderer_rd/shaders/environment/gi.glsl:575-587 |
| `cones-high` | verified | servers/rendering/renderer_rd/shaders/environment/gi.glsl:558-573 |
| `quality-doc` | verified | ProjectSettings.xml:3079-3081; gi.glsl:558 is the only use of `high_quality_vct` |
| `spec-cone` | verified | servers/rendering/renderer_rd/shaders/environment/gi.glsl:598-605, squared at 644 |
| `spec-cone` detail 644 (squared first) | verified | line checked in the checkout |
| `half-res-setting` | verified | servers/rendering/renderer_rd/environment/gi.cpp:3810; ProjectSettings.xml:3062-3065; `gi_set_use_half_resolution` sets it at run time (renderer_scene_render_rd.cpp:1168) |
| `half-res-buffers` | verified | servers/rendering/renderer_rd/environment/gi.cpp:3988-4000 |
| `half-res-shift` | verified | servers/rendering/renderer_rd/shaders/environment/gi.glsl:708,723 |
| `half-res-shift` detail 723 (back to the buffer's texel) | verified | line checked in the checkout |
| `half-res-dispatch` | verified | servers/rendering/renderer_rd/environment/gi.cpp:4266-4270 |
| `gi-buffer-fetch` | verified | servers/rendering/renderer_rd/shaders/forward_clustered/scene_forward_clustered.glsl:1982-2021; `gi_upscale_for_msaa` only with MSAA (servers/rendering/renderer_rd/forward_clustered/render_forward_clustered.cpp:728-731) |
| `gi-buffer-fetch` detail 1986 (the normal-aware pick, with MSAA only) | verified | line checked in the checkout |
| `half-res-doc` | verified | ProjectSettings.xml:3062 |

## Chapter 9: Into the final image

| Beat | L | Sentence | Class | Evidence |
|---|---|---|---|---|
| c9-map | 2053 | Now the opaque pass turns the screen pass's two buffers into colour. | verified | servers/rendering/renderer_rd/shaders/forward_clustered/scene_forward_clustered.glsl:2019-2024 |
| c9-map | 2055 | One buffer holds the ambient light, and the other holds the reflection. | verified | `ambient_buffer`, `reflection_buffer`: servers/rendering/renderer_rd/shaders/forward_clustered/scene_forward_clustered.glsl:2016-2021 |
| c9-mix | 2062 | The ambient buffer holds the cones' light. | verified | cones' light in the ambient buffer: servers/rendering/renderer_rd/shaders/environment/gi.glsl:726 |
| c9-mix | 2066 | Its alpha holds how much of the view the cones found filled. | verified | alpha in the buffer's a channel |
| c9-mix | 2070 | A cone that leaves the box finds nothing, so its alpha stays low. | verified | a cone that breaks at the box edge stops accumulating: servers/rendering/renderer_rd/shaders/environment/gi.glsl:487-505 |
| c9-mix | 2074 | Where the alpha is low, the opaque pass mixes toward the environment's own ambient light, which here comes from the sky. | verified | `ambient_light = mix(ambient_light, buffer_ambient.rgb, buffer_ambient.a)`: servers/rendering/renderer_rd/shaders/forward_clustered/scene_forward_clustered.glsl:2023; the environment's ambient in this lab is its sky (README) |
| c9-mix | 2078 | The reflection is mixed with the sky's reflection the same way. | verified | servers/rendering/renderer_rd/shaders/forward_clustered/scene_forward_clustered.glsl:2024 |
| c9-mix | 2082 | So the sky still lights what the cones saw as empty, here through the room's open top and front. | verified | the room is three walls and a floor (study-room.lam:4,55-80), open at the top and the front |
| c9-mix | 2086 | With the interior setting on, the alpha is one, so the sky never mixes in. | verified | `blend_ambient = !interior` and alpha forced to 1: servers/rendering/renderer_rd/shaders/environment/gi.glsl:591-593; servers/rendering/renderer_rd/environment/gi.cpp:3896 |
| c9-mix | 2090 | After that, the mixed light is treated like any other ambient light. | verified | the sums continue as `ambient_light` after the block: servers/rendering/renderer_rd/shaders/forward_clustered/scene_forward_clustered.glsl:2023-2200 |
| c9-mix | 2094 | In turn, the opaque pass applies ambient occlusion, the albedo, screen-space effects, and the environment's reflectance to it. | verified | `ambient_light *= ao`, `*= albedo.rgb`, SSIL, SSR, `prefiltered_dfg` (environment BRDF): servers/rendering/renderer_rd/shaders/forward_clustered/scene_forward_clustered.glsl:2155-2260 |
| c9-others | 2105 | Transparent surfaces cannot use the buffers, and there are none in this room. | simplified | GI buffers are for the opaque list: the opaque instances get `INSTANCE_FLAGS_USE_GI_BUFFERS` (servers/rendering/renderer_rd/forward_clustered/render_forward_clustered.cpp:1032-1034) and `uses_forward_gi` is set only for the alpha list (servers/rendering/renderer_rd/forward_clustered/render_forward_clustered.cpp:1147-1153); "cannot" is the design, not a limit the code states. No transparent surface in the lab's room |
| c9-others | 2109 | Instead, each transparent surface traces its own four cones in the forward pass that draws it, at every quality setting. | verified | `sc_use_forward_gi()`, four cones, `cone_angle_tan = 0.98269`, quality-independent: scene_forward_gi_inc.glsl:81-109 |
| c9-others | 2113 | Volumetric fog reads every mip of the texture at each point of its grid, with no cone at all. | simplified | only at points inside the box (`position` between 0 and `bounds`), all `mipmaps`, front to back, scaled by dynamic range, exposure, and the fog's `gi_inject`: volumetric_fog_process.glsl:707-728 |
| c9-others | 2119 | The editor's lighting view also reads the texture, but it never applies the dynamic range. | simplified | a Debug Draw mode of the viewport (also in a running game), not only the editor; `color_interp.xyz *params.dynamic_range;` has no assignment: voxel_gi_debug.glsl:94 (both light modes define `MODE_DEBUG_LIGHT`, gi.cpp:3610-3612) |
| c9-others | 2123 | So at the default range of two, that view shows the light at half its strength. | verified | `dynamic_range` default 2.0: scene/3d/voxel_gi.h:50; gi.h:84; the texel is light / range (voxel_gi.glsl:550) |
| c9-follow | 2137 | Follow this pixel on the sphere back through it all. | verified | lab |
| c9-follow | 2141 | Its cones read the texture's mips around it. | verified | servers/rendering/renderer_rd/shaders/environment/gi.glsl:558-590 |
| c9-follow | 2143 | The mip pass built those mips from the leaves. | verified | `MODE_UPDATE_MIPMAPS` builds parents from children: voxel_gi.glsl:526-543 |
| c9-follow | 2147 | Those leaves hold the panel's direct light, plus one bounce. | verified | lab: the panel's area light (README "What the lab simplifies"); two bounces default 2 passes (gi.h:93) |
| c9-follow | 2151 | Each leaf gathered that bounce with six cones of its own. | verified | `MODE_SECOND_BOUNCE`, six cones per leaf with a normal: voxel_gi.glsl:465-518 |
| c9-follow | 2153 | And each leaf is the average of the samples the bake took from the room's triangles. | simplified | a leaf's albedo, emission, and normal are averages of the samples taken on the triangles (voxelizer.cpp:100-172, `_fixup_plot` 598-615), not its light |
| c9-recap | 2173 | The first takeaway is the data. | verified | meta |
| c9-recap | 2175 | A sparse octree is baked once, then poured into a dense texture with one mip per level. | verified | sparse octree (cells) baked once; dense texture with `mipmaps = levels.size()`: servers/rendering/renderer_rd/environment/gi.cpp:2636-2642 |
| c9-recap | 2179 | The second is the light. | verified | meta |
| c9-recap | 2181 | Light passes run when the lights change, and then every pixel traces cones that read wider mips as they go. | simplified | also when the data or settings change, and a dynamic object rewrites the texture every frame |
| c9-recap | 2185 | The third is the trade. | verified | meta |
| c9-recap | 2187 | The static scene must be baked, and moving objects are only drawn in, frame by frame. | verified | static geometry is the bake; dynamic objects are plotted each frame (servers/rendering/renderer_rd/environment/gi.cpp:3175-3260) |
| c9-recap | 2191 | The cells are five centimetres wide, and the cones see the room only as coarse averages. | misleading | the cell size is the lab's (box 6.4 m at subdivision 128 = 5 cm, README); Godot's default is a 20 m box at subdivision 128 (doc/classes/VoxelGI.xml:42,46), 15.6 cm. Say "In this room the cells are five centimetres wide" |
| c9-recap | 2195 | Area lights cast no shadows. | misleading | only in VoxelGI's light passes: `compute_area_light` has no march (voxel_gi.glsl:345-), while shadows exist for area lights elsewhere (volumetric_fog_process.glsl:690). Say "In the light passes, area lights get no shadows" |
| c9-recap | 2197 | In return, a pixel needs only a few cone samples. | verified | meta |
| c9-sources | 2201 | These are the lines of the opaque pass's mix, and those of the other passes that read the texture. | verified | meta |
| c9-sources | 2203 | Every source this video cites is in its source-reading guide, in the order the frame runs it. | simplified | every cited key is in a chapter's reading (checked by script); the order is the chapter's, which follows the story more than the frame |

### Source-index entries of chapter 9 (inspect and details)

| Entry | Class | Evidence |
|---|---|---|
| `gi-buffer-mix` | verified | servers/rendering/renderer_rd/shaders/forward_clustered/scene_forward_clustered.glsl:2023-2024; AO, albedo, SSIL, SSR, environment BRDF follow at 2155-2260 |
| `gi-buffer-mix` detail 2024 (the reflection) | verified | line checked in the checkout |
| `forward-gi-transparent` | simplified | servers/rendering/renderer_rd/forward_clustered/render_forward_clustered.cpp:1032-1034 flags every opaque-list instance for the buffers and 1147-1153 sets `uses_forward_gi` for the alpha list; a surface forced into the alpha list by a fade is forward too |
| `forward-gi-transparent` detail 1102 (reset for every surface) | verified | line checked in the checkout |
| `forward-cones` | verified | scene_forward_gi_inc.glsl:81-109 (four cones, tan 0.98269, `color.a < 0.95`, mix per cone) |
| `fog-inject` | simplified | volumetric_fog_process.glsl:707-728: only inside the box, and scaled by dynamic range, exposure, and `gi_inject` |
| `debug-noop` | verified | voxel_gi_debug.glsl:94; gi.cpp:3610-3612 |
| `bias-mismatch` | verified | servers/rendering/renderer_rd/environment/gi.h:87 (1.4) against scene/3d/voxel_gi.h:52 (1.5); `VoxelGIData::VoxelGIData` never calls `set_bias` (voxel_gi.cpp:261-263); stated as read, not tested |
| `frame-order` | verified | servers/rendering/renderer_rd/forward_clustered/render_forward_clustered.cpp:1763,1926-1927,2182 |
| `frame-order` detail 1927 (with a VoxelGI in view, the pre-pass also writes the tag) | verified | line checked in the checkout |
| `frame-order` detail 2182 (the screen pass runs here, before the opaque pass) | verified | line checked in the checkout |

## Counts

| Chapter | verified | simplified | misleading | incorrect | investigate |
|---|---|---|---|---|---|
| 7 | 59 | 13 | 3 | 0 | 1 |
| 8 | 86 | 16 | 2 | 0 | 2 |
| 9 | 34 | 8 | 2 | 0 | 0 |
| all | 179 | 37 | 7 | 0 | 3 |

(Entries are counted once per chapter that lists them, so an entry listed in two chapters counts twice; details count as items.)


## Not verified, with the correction proposed

Uncertain claims are narrated as uncertain, verified, or cut.

- **ch7 L1472** [simplified] "Each object keeps the first two VoxelGIs it overlaps, so a third one never lights it.": `pair_voxel_gi_instances` keeps `[0]`,`[1]` of the culler list (%s:5060-5071; list built from `geom->voxel_gi_instances`, renderer_scene_cull.cpp:3205-3215). Godot's HashSet iterates in insertion order, but `erase` moves the last key into the freed slot (core/templates/hash_set.h:290-293), so "first two it overlaps" holds until an overlap ends
- **ch7 L1476** [simplified] "A pass called the depth pre-pass draws depth before the main shading.": `PASS_MODE_DEPTH_NORMAL_ROUGHNESS_VOXEL_GI` with a VoxelGI in view (servers/rendering/renderer_rd/forward_clustered/render_forward_clustered.cpp:1926-1927); the pre-pass itself runs only if `rendering/driver/depth_prepass/enable` (default true) or an opaque stencil forces it (servers/rendering/renderer_rd/forward_clustered/render_forward_clustered.cpp:2112-2113)
- **ch7 L1484** [simplified] "An index of two hundred fifty-five means none.": 0xFF is written for drawn pixels of objects without a VoxelGI (%s:3037-3038). Pixels the pre-pass never draws keep its clear colour (0,0,0,0) (%s:1948-1950, 2142), i.e. index 0, not 255
- **ch7 L1494** [simplified] "This sphere's pixels name the room's only VoxelGI.": true of what Godot would write for this room (one VoxelGI, index 0). The lab has no tag buffer, because there is one VoxelGI (README "What the lab simplifies"); say "in Godot, this sphere's pixels would name"
- **ch7 L1505** [investigate] "Take this ringed pixel, on the side of the sphere that faces the red wall.": lab visual: the left wall is the red one (packages/render_lab/scenes/study-room.lam:67-71, albedo 0.70 0.24 0.18); that the ringed pixel (azimuth 60 to 100) faces it was not checked from a render
- **ch7 L1525** [simplified] "The fade exists to blend two boxes that overlap.": the code uses the fade as a weight between VoxelGIs (`blend_accum`, servers/rendering/renderer_rd/shaders/environment/gi.glsl:660-663); no comment states why. Say "The fade weights overlapping boxes against each other"
- **ch7 L1548** [simplified] "For a cone this wide, the radius at any point equals the distance travelled.": `radius = max(0.5, dist)` (servers/rendering/renderer_rd/shaders/environment/gi.glsl:508-526): equal to the distance from half a cell on
- **ch7 L1556** [simplified] "That texel already averages everything in the cone's footprint, so one sample stands in for many.": a parent texel is the sum of its children over `mix(8, count, propagation)` (voxel_gi.glsl:526-543), an average only at propagation 1 (default 0.5)
- **ch7 L1564** [simplified] "The cone stops when its alpha reaches one, or when it leaves the box.": `while (dist < max_distance && color.a < 1.0)` plus the break outside the box (servers/rendering/renderer_rd/shaders/environment/gi.glsl:513-519): it also stops at the box's diagonal
- **ch7 L1617** [simplified] "On a rough one, it opens seventy degrees from its axis.": the lab's sphere is at roughness 0.9 there (storyboard.lam:1620 range 0.15 to 0.9): half angle 0.9^2 x 0.99 x 90 = 72.2 degrees; roughness 1 gives 89.1. Say "about seventy" or "seventy-two"
- **ch7 L1640** [simplified] "It is also multiplied by the VoxelGI's energy, a brightness setting.": energy is folded into `dynamic_range` on the CPU (`voxel_gi_get_dynamic_range * voxel_gi_get_energy`, servers/rendering/renderer_rd/environment/gi.cpp:3893); the shader also multiplies by the camera exposure normalization
- **ch7 L1670** [misleading] "In GLSL, Godot's shading language, a vector's length method returns its component count, three.": GLSL is the language the renderer's shaders are written in, not "Godot's shading language" (that is the .gdshader language, which has no `.length()` method). Say "GLSL, the language of Godot's renderer shaders"
- **ch7 L1674** [misleading] "So every pixel is traced, whatever its normal.": the check passes for every pixel, but a pixel is traced only if its tag names a VoxelGI (servers/rendering/renderer_rd/shaders/environment/gi.glsl:654-657): objects without one (tag 255) trace nothing. Say "So the check never rejects a pixel"
- **ch7 L1684** [misleading] "So every pixel traces four cones and a reflection cone through the texture.": only pixels whose tag names a VoxelGI trace; at Low quality, four cones plus the reflection cone (servers/rendering/renderer_rd/shaders/environment/gi.glsl:575-605). Say "every pixel of this room traces..."
- **ch7 entry pairing-two** [simplified] "A geometry keeps the first two VoxelGIs it overlaps, in the order the culler's set gives them.": see L1472: insertion order, changed by an erase
- **ch7 entry normal-roughness** [simplified] "The pre-pass writes the normal-roughness buffer at 8 bits a channel: the normal scaled to the unit cube and mapped to 0..1, the roughness ti": `encode24` touches the unit cube and then multiplies by a best-fit factor from `best_fit_normal_texture` (servers/rendering/renderer_rd/shaders/forward_clustered/scene_forward_clustered.glsl:1185-1200), which the entry leaves out; roughness x 127/255 and mirrored 1 - r for dynamic: 3021-3027
- **ch7 entry cone-trace-45** [simplified] "The 45-degree march: the radius equals the distance, so each step doubles the distance and climbs one mip.": `radius = max(0.5, dist)`: servers/rendering/renderer_rd/shaders/environment/gi.glsl:508-526; "equals the distance" from half a cell on
- **ch8 L1712** [simplified] "The bake runs once, in the editor.": `VoxelGI::bake` works "from the editor and in exported projects" (doc/classes/VoxelGI.xml:24; scene/3d/voxel_gi.cpp:434). Say "The bake runs once, usually in the editor"
- **ch8 L1716** [simplified] "The light passes run only when something changes.": also when the data or one of three settings change (gi.cpp:256,271,344) and, with dynamic objects, the write pass every frame (servers/rendering/renderer_rd/environment/gi.cpp:3067-3171)
- **ch8 L1720** [simplified] "The screen pass runs every frame.": every frame a VoxelGI is in view (`process_gi` is called when `render_gi`, servers/rendering/renderer_rd/forward_clustered/render_forward_clustered.cpp:1606-1608)
- **ch8 L1735** [simplified] "Its line is each frame's total time, so a spike is a frame that did more work.": the panel draws recent frames' GPU and frame times against the budgets (lab/src/ui/graph_panel.rs:89-93); "total time" is the GPU time
- **ch8 L1741** [simplified] "A row shows the pass's name, a bar for its share of the frame, and its average time in milliseconds.": each row also keeps the 95th percentile and the last time (graph_panel.rs:66); the bar is the share of the GPU time
- **ch8 L1743** [investigate] "The row called scene pass is the room's own shading, and it takes most of the frame.": a measurement of the lab (the "scene pass" row is `timing: "scene pass"`, lab/src/graph/mod.rs:311); that it takes most of the frame was not measured here (GPU busy)
- **ch8 L1762** [simplified] "Nothing changed, so Godot does no light-pass work at all.": true of this room, which has no dynamic object: with one, the write pass and the plot run every frame (servers/rendering/renderer_rd/environment/gi.cpp:3067,3118-3171; update.rs:223-239)
- **ch8 L1781** [investigate] "On this machine, the bounce takes the longest, because it traces six cones from every leaf that has a normal.": a measurement ("on this machine"); that the bounce is the slowest row was not measured here (GPU busy). The cause given matches the code: six cones per leaf with `length(normal) > 0.2` (voxel_gi.glsl:465-518)
- **ch8 L1845** [simplified] "The barrier is there because a parent sums its children's light, so the children must be finished first.": the mip pass sums the children and divides by `mix(8, count, propagation)` (voxel_gi.glsl:526-543); "sums" is true, the result is not a plain sum
- **ch8 L1870** [simplified] "Inside the light pass, a shadow ray reads the distance field.": only for a light with shadows (`has_shadow`, voxel_gi.glsl:297-340)
- **ch8 L1874** [simplified] "The field says how far the nearest surface is, so the ray can safely jump that far.": the field value is `byte * 255 - 1` cells (voxel_gi.glsl:182): it jumps one cell short of the nearest surface
- **ch8 L1908** [simplified] "In this box, each cone takes at most seven steps, so a pixel takes at most twenty-eight samples.": the lab's count (cost.rs `steps_45`, bias 1.5 = VoxelGIData's default, box diagonal 192): 7 steps, 28 samples. With 1.4, the server's own default, it is 8 steps (the walk 1.4, 2.8, ... 179.2). Say "at bias one and a half"; "at most" is right since cones also stop at alpha 1 or the box edge
- **ch8 L1916** [simplified] "That texel is already the average of the cells it covers, a cheap stand-in for many rays.": an average at propagation 1 only (see L1556)
- **ch8 L1920** [simplified] "The price is that thin walls blur.": an inference (a mip texel blends the cells it covers); not stated in the source. Say "so detail thinner than a texel is lost"
- **ch8 L1958** [simplified] "A fully rough surface takes at most two steps, and a mirror at most three hundred eighty-one.": lab's box (diagonal 192) at bias 1.5: 2 steps at roughness 1 and 381 at roughness 0 (selftest.rs:499-500); at bias 1.4 they are 2 and 382. Say "in this box"
- **ch8 L1966** [simplified] "Steps that short never jump over a wall.": a half-cell step cannot skip a cell, and the sampling is trilinear; a wall thinner than a cell is still not exact
- **ch8 L2013** [simplified] "The edge turns ragged, in steps two pixels wide.": lab observation; one half-size texel is 2 pixels wide, consistent with L2017
- **ch8 L2027** [simplified] "So VoxelGI pays for little each frame.": a judgement, supported by the chapter
- **ch8 entry pass-order** [misleading] "With the lights changed, pass 0 lights the leaves, fills the mips, and writes the texture; with two bounces, pass 1 adds the bounce at the l": "Without a change, only the write runs" (detail at 3100): the write-only pass runs only when a dynamic object is or was plotted; with nothing changed and none plotted the gate at gi.cpp:3067 skips everything (see update-gate). Say "Without a light change, only the write runs, and only while dynamic objects are plotted"
- **ch8 detail pass-order:3100** [misleading] "nothing changed: write only": see the entry
- **ch9 L2105** [simplified] "Transparent surfaces cannot use the buffers, and there are none in this room.": GI buffers are for the opaque list: the opaque instances get `INSTANCE_FLAGS_USE_GI_BUFFERS` (servers/rendering/renderer_rd/forward_clustered/render_forward_clustered.cpp:1032-1034) and `uses_forward_gi` is set only for the alpha list (servers/rendering/renderer_rd/forward_clustered/render_forward_clustered.cpp:1147-1153); "cannot" is the design, not a limit the code states. No transparent surface in the lab's room
- **ch9 L2113** [simplified] "Volumetric fog reads every mip of the texture at each point of its grid, with no cone at all.": only at points inside the box (`position` between 0 and `bounds`), all `mipmaps`, front to back, scaled by dynamic range, exposure, and the fog's `gi_inject`: volumetric_fog_process.glsl:707-728
- **ch9 L2119** [simplified] "The editor's lighting view also reads the texture, but it never applies the dynamic range.": a Debug Draw mode of the viewport (also in a running game), not only the editor; `color_interp.xyz *params.dynamic_range;` has no assignment: voxel_gi_debug.glsl:94 (both light modes define `MODE_DEBUG_LIGHT`, gi.cpp:3610-3612)
- **ch9 L2153** [simplified] "And each leaf is the average of the samples the bake took from the room's triangles.": a leaf's albedo, emission, and normal are averages of the samples taken on the triangles (voxelizer.cpp:100-172, `_fixup_plot` 598-615), not its light
- **ch9 L2181** [simplified] "Light passes run when the lights change, and then every pixel traces cones that read wider mips as they go.": also when the data or settings change, and a dynamic object rewrites the texture every frame
- **ch9 L2191** [misleading] "The cells are five centimetres wide, and the cones see the room only as coarse averages.": the cell size is the lab's (box 6.4 m at subdivision 128 = 5 cm, README); Godot's default is a 20 m box at subdivision 128 (doc/classes/VoxelGI.xml:42,46), 15.6 cm. Say "In this room the cells are five centimetres wide"
- **ch9 L2195** [misleading] "Area lights cast no shadows.": only in VoxelGI's light passes: `compute_area_light` has no march (voxel_gi.glsl:345-), while shadows exist for area lights elsewhere (volumetric_fog_process.glsl:690). Say "In the light passes, area lights get no shadows"
- **ch9 L2203** [simplified] "Every source this video cites is in its source-reading guide, in the order the frame runs it.": every cited key is in a chapter's reading (checked by script); the order is the chapter's, which follows the story more than the frame
- **ch9 entry forward-gi-transparent** [simplified] "Only surfaces of the transparent list trace VoxelGI per pixel in the forward shader; opaque ones read the buffers.": servers/rendering/renderer_rd/forward_clustered/render_forward_clustered.cpp:1032-1034 flags every opaque-list instance for the buffers and 1147-1153 sets `uses_forward_gi` for the alpha list; a surface forced into the alpha list by a fade is forward too
- **ch9 entry fog-inject** [simplified] "Volumetric fog reads every mip of the texture at the froxel's point and composites them front to back; it traces no cone.": volumetric_fog_process.glsl:707-728: only inside the box, and scaled by dynamic range, exposure, and `gi_inject`
