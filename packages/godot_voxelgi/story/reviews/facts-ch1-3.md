# Fact check, chapters 1-3 (Godot VoxelGI)

Checkout: /home/oppositenormal/Documents/programming/other/godot at ed1daf0bf001b61586d9930840f2f1394092c079 (confirmed with git rev-parse HEAD).
Classes: V verified, S correct but simplified, M potentially misleading, X incorrect, N needs investigation.
Paths are relative to the checkout unless they start with `lab:` (the lab's files under packages/godot_voxelgi/). Line numbers were read in the checkout; the `source-index.json` line numbers of the entries below were also spot-checked and match the cited symbols.
"ren-fc" = servers/rendering/renderer_rd/forward_clustered/render_forward_clustered.cpp; "gi.cpp" = servers/rendering/renderer_rd/environment/gi.cpp; "vox" = scene/3d/voxelizer.cpp; "vgi.glsl" = servers/rendering/renderer_rd/shaders/environment/voxel_gi.glsl.
Sentences that only state a purpose or transition ("By the end...", "Take one triangle...", "Next: ...") are not listed.

## Chapter 1: Where VoxelGI runs

| Sentence | Class | Evidence |
|---|---|---|
| L22 lab's room lit only by ceiling panel and sky | V (lab) | study-room.lam:4 "lit by a ceiling panel and the sky"; lab README "What the lab simplifies" (the sky is the lab's, not Godot's radiance map). Said as the lab's. |
| L24 a surface the panel cannot reach gets only the sky's flat light | V (lab) | Lab's shading with GI off: ambient is the lab's sky (README, simplifies). Said as the lab's. |
| L26 VoxelGI adds light that bounces around the room | V | doc/classes/VoxelGI.xml:6-8 "real-time indirect light". |
| L30 red wall warms the sphere and floor | V (lab) | study-room.lam:67-71 left wall albedo (0.70, 0.24, 0.18); sphere at x=-1.05. A picture claim; the render was not re-viewed. |
| L34 bounced light comes from every other surface it can see | V | Definition of indirect light, not a Godot claim. |
| L38 gathering ray by ray per pixel per frame would cost far too much | V | Motivation; not a Godot claim (Godot's own design choice: cone tracing, gi.glsl:487). |
| L42 cuts the room into a grid of small boxes (cells) ahead of time | V | VoxelGI::bake builds the octree, scene/3d/voxel_gi.cpp:434-495 and vox:666. |
| L46 It then lights those cells | S | The light pass lights only the leaf cells (mipmaps[0]); the coarse levels get light from the mip pass: gi.cpp:3102-3171, vgi.glsl:528-546 (MODE_UPDATE_MIPMAPS). The chapter says so in L114 and ch3 L601. |
| L50 every pixel reads their light through a few wide cones | S | gi.glsl:556-587: 4 (Low) or 6 (High) diffuse cones, plus one reflection cone (gi.glsl:581) per pixel. "A few" is true; the reflection cone is not named here. |
| L52 one sample of a cone averages many cells; few samples | V | voxel_cone_trace picks the mip from the cone diameter: gi.glsl:487-505 (`textureLod(..., log2_diameter)`). |
| L58 bake "usually runs once, in Godot's editor" | V | VoxelGI::bake scene/3d/voxel_gi.cpp:434; VoxelGI.xml:8-9 (can be baked in an exported project). Editor Bake button is the usual path. |
| L61 turns the meshes in the box into an octree, each cell splits into eight | S | voxel_gi.cpp:327-360 (_find_meshes: visible, GI mode Static, bounds touch the box); vox:239-300 (8 children per cell). "The meshes" is Static ones touching the box; chapter 2 says so. |
| L65 distance field records how far each place is from the nearest surface | S | vox:868-945: distance to the nearest *leaf cell* in leaf units, not to a surface. Ch3 L692 says "nearest leaf". |
| L67 the field later helps find which cells are in shadow | S | vgi.glsl:284-336: the march (raymarch with the SDF) runs only `if (lights.data[index].has_shadow)`. Area lights are never marched (compute_area_light, vgi.glsl:345). Add "for lights with shadows". |
| L71 light passes light every cell, store result in a 3D texture | S | The light pass runs on mipmaps[0] (the leaves) and writes the RGBA8 3D texture; coarser cells come from the mip dispatches: gi.cpp:3102-3171. "Every cell" = every leaf. |
| L74 light passes run again only when a light changes, or VoxelGI data or settings change; a still room costs nothing | S | Gate: renderer_scene_cull.cpp:4004 (`update_lights = voxel_gi_needs_update`), 4023-4090 (light cache compare), gi.cpp:4302 (version), gi.cpp:3067-3175 (`p_update_light_instances` only then runs the light and mip dispatches). Only four events bump the version: new data (gi.cpp:188), dynamic range (256), propagation (271), two bounces (344). Energy, bias, normal bias, interior do not (gi.cpp:280-339). "Settings" is wider than the code. With dynamic objects the texture is cleared and re-blitted every frame (gi.cpp:2963-2966, 3067-3175). |
| L78 moving objects are left out of the bake, drawn into the texture every frame | S | Godot has no "moving": the objects with GI mode Dynamic are skipped by the bake (voxel_gi.cpp:327-335) and are plotted every frame the VoxelGI is in the update list (gi.cpp:2963-3175; renderer_scene_cull.cpp:4063-4102, no change test). A Disabled object is neither baked nor drawn. Say "objects marked Dynamic". |
| L82 every frame the screen pass traces cones through that texture for each pixel inside the box | V | process_gi compute: ren-fc:1607 (`gi.process_gi`), gi.glsl:540-545 (returns for positions outside the box), 556-587. "Screen pass" is the lab's name for process_gi. |
| L87 the opaque pass mixes the cones' light into the image as indirect light | V | scene_forward_clustered.glsl:1982 (`INSTANCE_FLAGS_USE_GI_BUFFERS` reads the GI buffers into ambient and specular). |
| L106 the room's baked cells drawn as one point each | V (lab) | Lab overlay; the cells are the lab's bake (lab:bake.rs). |
| L112 each cell stores light | V | gi.cpp:2634-2665 (each instance texture, one texel per cell, light in RGB) after lighting. |
| L114 each bigger cell stores a blend of the eight smaller cells inside it | S | vgi.glsl:528-546: sum of existing children / `mix(8.0, count, params.propagation)` (default 0.5, voxel_gi.h:54). Neither a plain average (that is divisor 8 for propagation 0) nor a sum over count (propagation 1). "Blend" is fair; do not say average. |
| L118 pixel reads light through a few cones growing wider | V | gi.glsl:556-587. |
| L128 wider cone reads bigger cells, like a coarser mip | V | gi.glsl:495-505: `diameter = max(1, 2 tan_half dist)`, `textureLod(..., log2(diameter))`. |
| L137 three renderers; VoxelGI only in Forward+ | V | VoxelGI.xml:8 "only supported in the Forward+ rendering method, not Mobile or Compatibility". |
| L141 Compatibility leaves its VoxelGI functions empty | V | drivers/gles3/rasterizer_scene_gles3.cpp:1255-1270 (create returns RID(); others empty bodies), .h:368. |
| L143 Mobile never links an object to a VoxelGI, so no surface reads its light | V | render_forward_mobile.h:583 (`pair_voxel_gi_instances ... override {}`); mobile shader only defines `INSTANCE_FLAGS_USE_VOXEL_GI` (scene_forward_mobile_inc.glsl:255) and never reads it. |
| L145 no project setting turns VoxelGI on or off | V | Only `rendering/global_illumination/voxel_gi/quality` exists (rendering_server.cpp:3709); the gate is ren-fc:1871-1872. |
| L147 work runs in any frame where culling finds a VoxelGI | V | renderer_scene_cull.cpp:2962-2969 (frustum cull pushes the instance) then ren-fc:1871-1872 `using_voxelgi = true`, inside the non-reflection-probe branch (ren-fc:1860): reflection-probe renders do not run it. |
| L151 a VoxelGI is a node placed in the scene | V | scene/3d/voxel_gi.h (class VoxelGI : VisualInstance3D). |
| L155 default box twenty metres a side | V | voxel_gi.h:121 `size = Vector3(20, 20, 20)`. |
| L157 cut into 128 cells along its longest axis | V | voxel_gi.h:120 `subdiv = SUBDIV_128`; voxel_gi.cpp:435 `subdiv_value = {6,7,8,9}` -> 1<<7 = 128 (vox:681). |
| L161 one setting picks how many cones each pixel traces, four by default, six for finer | S | Project setting `rendering/global_illumination/voxel_gi/quality` 0 = Low 4 cones, 1 = High 6 (rendering_server.cpp:3709, gi.cpp:3584, 4043, gi.glsl:558-587). These are the diffuse cones; one specular cone more is traced either way (gi.glsl:581). Say "diffuse cones". |
| L174 lab's box 6.4 x 3.2 x 6.4 m | V (lab) | lab:world.rs:30 `SIZE`; README. |
| L180 finest cells five centimetres | V (lab) | 6.4 / 128 = 0.05 (vox:703-704 `cell_size`). |
| L186 a full grid would hold about a million cells | V (lab) | 128 x 64 x 128 = 1,048,576 (selftest.rs:495 `texels`). |
| L192 the lab bakes, lights, and reads this box with code ported from Godot's own | M | Lab README: the bake (voxelizer, distance field) is Rust on the CPU only (bake.rs, sdf.rs), like Godot's; only the run-time work (light passes, mips, dynamic objects, screen pass) is WGSL. See item 1 below. |
| L196 it runs that code on the GPU and again on the CPU and checks the two agree | M | Same: the bake has no GPU version in the lab (shaders/voxelgi_passes.wgsl has no voxelizer or distance transform entry), so the GPU/CPU agreement covers the light passes, the sampler, the screen pass, and the dynamic objects (README, selftest.rs). |
| L205 VoxelGI is a box of baked cells | V | |
| L207 cells are lit when the lights change; every pixel inside the box reads them | V | See L74, L82 (same limits as L74). |
| L211 everything VoxelGI can light starts in that bake | V | Light is stored per baked cell; dynamic objects are plotted into the same texture (gi.cpp:3067-3175). |
| L219 every claim in this video cites a file, a symbol, and a line | M | Lab claims (the room, counts) cite none; and callouts exist for only some Godot claims. Say "Each Godot claim here cites ...", or "The Godot claims ... are cited". |
| L221 when the video is built, a check confirms each of those lines is still there | M | The line hashes are checked by `selftest` against the checkout (lab:sources.rs:3), and need the checkout; tools/video/build.py:226 and check.py:38 only check that callout keys exist in source-index.json. Say "the lab's self-test checks...". |

### Chapter 1 source-index entries (`inspect` and `details`)

| Entry | Class | Evidence |
|---|---|---|
| frame-order (inspect; details 1927, 2182) | V | ren-fc:1737-1763 (render index, setup_voxel_gi_instances), 1926-1927 (PASS_MODE_DEPTH_NORMAL_ROUGHNESS_VOXEL_GI), shader writes the tag scene_forward_clustered.glsl:3030-3038, ren-fc:1607 and 2182 (`_pre_opaque_render` before "Render Opaque Pass"). |
| voxelgi-gate (inspect; detail 1837) | V | ren-fc:1871-1872. Add: only in the non-reflection-probe branch. |
| relight-gate (inspect; detail 251) | V | gi.cpp:4302, version bumps 188/256/271/344, renderer_scene_cull.cpp:4004-4090, gi.cpp:2963-2966 (clear when dynamic data), 3067-3175 (one pass re-blit). |
| voxelgi-doc | V | VoxelGI.xml:8. |
| mobile-stub | V | render_forward_mobile.h:583. |
| gles3-stub | V | rasterizer_scene_gles3.cpp:1255-1257. |
| node-defaults (detail 121) | V | voxel_gi.h:120-121. |
| data-properties (details 52, 54, 56) | V | voxel_gi.h:49-56: 2.0, 1.0, 1.5, 0.0, 0.5, false, true; bias is the cone's starting `dist` (gi.glsl:488, in cells); propagation is the mip divisor blend (vgi.glsl:540). |
| quality-setting (detail 3707) | V | rendering_server.cpp:3707, 3709; gi.cpp:4043 push constant. |

## Chapter 2: Baking the octree

| Sentence | Class | Evidence |
|---|---|---|
| L227 everything VoxelGI knows about the room comes from one bake | V | Plus the lights and dynamic objects at run time, which ch1 says. |
| L241 Pressing Bake starts the voxelizer | V | VoxelGI::bake, voxel_gi.cpp:434-446 (`Voxelizer baker; baker.begin_bake`); the Bake button calls it (editor plugin). |
| L245 walks the scene from the VoxelGI's parent, collects meshes | V | voxel_gi.cpp:436 `p_from_node = p_from_node ? p_from_node : get_parent()`, 327-392 (`_find_meshes` recurses over children). |
| L249 collected when visible and bounds touch the box | V | voxel_gi.cpp:320-325 (`is_visible_in_tree`), 336-341 and 369-372 (`AABB(-size/2, size).intersects(...)`). |
| L253 baked only when GI mode is Static, the default | V | voxel_gi.cpp:327-331; visual_instance_3d.h:144 `gi_mode = GI_MODE_STATIC`. |
| L257 Dynamic objects are not baked | V | voxel_gi.cpp:329 (any mode but Static is skipped; Disabled too). |
| L261 only triangle surfaces are baked | V | vox:493-496 (`PRIMITIVE_TRIANGLES` only). |
| L265 a triangle that misses the box is skipped, one that crosses its edge is kept whole, not clipped | S | vox:500-505, 536-540 (`triangle_box_overlap(original_bounds...)` culls, no clipping). The whole triangle is sent down, but cells are made only inside the grid: `_plot_face` skips children beyond `axis_cell_size` (vox:260-265), so the part outside the box bakes nothing. Add "but only the cells inside the box are made". |
| L269 the lab leaves the ceiling panel's mesh out of the bake | V (lab) | lab:world.rs:84-88 (`skip = [ids::PANEL]`); README "What the lab simplifies". Said as the lab's. |
| L273 the panel's light comes from an area light instead | V (lab) | lab:world.rs:113-117 `panel_light`, README. Said as the lab's. |
| L282 128 cells along the longest axis, two to the seventh | V | vox:666-681. |
| L288 a shorter axis gets fewer cells | S | Only when it is at most half the longest side (vox:689-692); a 5 m axis beside a 6.4 m one keeps 128. True for the lab's height. |
| L292 count halved, and halved again, while half the longest side still covers the axis | V | vox:686-692 `while (axis_size / 2.0 >= po2_bounds.size[i]) { axis_size /= 2; axis_cell_size[i] >>= 1; }`. |
| L294 3.2 m tall, half the 6.4 m width, so height gets 64 | V (lab) | 6.4/2 = 3.2 >= 3.2 (exact in float), one halving: lab README "128 x 64 x 128". |
| L298 every side stretched to the longest so the octree covers a cube | V | vox:694 `po2_bounds.size[i] = po2_bounds.size[longest_axis]`. |
| L302 cells are cubes of one size | V | vox:703-706. |
| L304 smallest are five centimetres, seven levels below the whole cube | V (lab) | 6.4/128 = 0.05 m; `cell_subdiv` 7 levels below the root (vox:801-811: depth + 1 levels). |
| L306 the height's 64 cells stop at 3.2 m | V | vox:260-265. |
| L308 the top half of that cube holds no cells at all | V | `ny >= axis_cell_size[1]` (64) is skipped at every level (vox:260-265), so nothing above the box's height is made. |
| L327 the triangle enters at its root, the whole cube | V | vox:511 and 540 `_plot_face(0, 0, 0, 0, 0, ..., po2_bounds)`. |
| L331 root cut into eight children, one per octant | V | vox:254-285 (`for i < 8`). |
| L335 each child tested against the triangle | V | vox:268-275 `triangle_box_overlap(test_aabb...)`. |
| L339 only the overlapped children become cells; the path goes one level deeper at a time | V | vox:277-285 creates a child only after the test passes, then recurses. |
| L343 seven levels down the children are 5 cm; smallest cells are called leaves | V (lab) | `p_level == cell_subdiv` (vox:115) is the leaf case. "Leaves" is the engine's and the lab's word (`leaf_voxel_count`, vox:619). |
| L347 the triangle creates only the handful of cells it touches | V | Follows from the descent. |
| L365 overlap test looks for a separating axis | V | geometry_3d.h:733-736. |
| L369 on such an axis the projections do not overlap | V | SAT definition. |
| L371 nine axes from the triangle's edges, then the box's three, then the triangle's plane | V | geometry_3d.h:756-780 (9 AXISTESTs), 782-800 (min/max in x, y, z), 802-807 (plane). |
| L375 first separating axis rules the child out | V | each test `return false` on separation. |
| L379 readout names the axis that rejected the red child | V (lab) | Lab readout (lab:demos). |
| L397 every triangle goes down the same way | V | vox:487-540. |
| L401 triangles reaching the same cell share it | V | vox:277-283 (`if children[i] == CHILD_EMPTY` creates once, else reuses). |
| L403 level by level the cells shrink and multiply | V | |
| L405 42,289 leaves out of fifty-six thousand cells in all | S | 42,289 is asserted by lab:selftest.rs:494 (`c.leaves == 42_289`); the total is 56,376 (story/research.md:276, a lab measurement not asserted by any test). Say "about fifty-six thousand cells". |
| L409 the cells follow only the surfaces | V | |
| L411 only about four in a hundred of the grid's million places hold a leaf | V (lab) | 42,289 / 1,048,576 = 4.03 %. |
| L415 an octree with most space left empty is called sparse | V | Standard term. |
| L432 at a leaf the voxelizer picks the axis the plane faces most | V | vox:122-135 (largest |normal . axis|). |
| L436 cell cut into 4 x 4 columns along that axis | V | vox:107, voxelizer.h:105 `color_scan_cell_width = 4`; vox:147-152. |
| L440 each column the triangle crosses gives one sample | V | vox:153-161 (`triangle_box_overlap` per column; `continue` if not). |
| L442 a sample adds albedo, emission, and the triangle's normal there to the leaf's sums | V | vox:165-198, 218-238 (interpolated normal, `get_uv_and_normal`). |
| L446 each sample is scaled by a sixteenth | V | vox:209-221 (`accdiv = 1/16`; applied after summing, same result). |
| L450 a triangle covering half the leaf adds half the weight of one covering all | S | True in columns hit (alpha = columns / 16), not in area; also when no column is hit, one sample of weight 1/16 is added (vox:178-205). |
| L454 alpha adds those shares, the fraction of columns hit | V | vox:196-198 `alpha += 1.0` then `*= accdiv`. |
| L458 next chapter uses alpha to turn the sums into an average | V | vox:618-631. |
| L475 colours come from the materials | V | vox:336-390. |
| L477 each material read once into two tables, 128 a side | V | vox:338-341 (cache), 111 `bake_texture_size = 128`. |
| L481 one table holds albedo, in linear colour | V | vox:352-358, 320-324 (`srgb_to_linear`). |
| L483 the other holds emission times its energy and the bake's exposure | S | vox:362-379: with the emission feature on; also adds colour (EMISSION_OP_ADD) and, with physical light units, the intensity (`emission_intensity`). A material that is not a BaseMaterial3D bakes white, no emission (vox:381-386). |
| L487 each leaf shows the albedo it received | V (lab) | Lab overlay. |
| L489 room's materials are flat colours, so every sample of a mesh is the same | V (lab) | README: "The room's materials are flat colours"; the scene's albedos are single colours (study-room.lam:59-115). |
| L506 the bake keeps only the cells surfaces pass through | V | |
| L510 each leaf sums what its triangles left in it | V | |
| L514 those sums are not yet what the GPU reads | V | vox:618-631 and 538-560 run afterwards (vox:709-713 `end_bake`). |
| L516 the next chapter finishes the cells and packs them | V | |

### Chapter 2 source-index entries

| Entry | Class | Evidence |
|---|---|---|
| bake-entry (detail 444) | V | voxel_gi.cpp:435-446, 456-468 (plot loop, `end_bake`), 500-513 (SDF before `allocate`); "usually from the editor" matches VoxelGI.xml:8. |
| find-meshes | V | voxel_gi.cpp:327-392; CSGShape3D::get_meshes (modules/csg/csg_shape.cpp:1140), GridMap::get_meshes (modules/gridmap/grid_map.cpp:1556). |
| begin-bake (details 689, 706) | V | vox:666-709. |
| plot-mesh | V | vox:487-540 (triangle test against the original box). The note "culled, not clipped" is exact for triangles; cells beyond the grid are dropped in `_plot_face`. |
| plot-face-recurse (details 270, 280) | S | Accurate, but omits the grid limit at vox:260-265, which is why the cube's top half stays empty. Add it to the inspect text. |
| tri-box (detail 807) | V | geometry_3d.h:733-807. |
| leaf-scan (details 209, 178) | V | vox:107-221. |
| material-cache | S | vox:336-390: also the emission operator and, with physical light units, `emission_intensity`; emission only when the material's emission feature is on. |

## Chapter 3: What the bake stores

| Sentence | Class | Evidence |
|---|---|---|
| L528 the bake left a tree of cells that hold raw sums | V | vox:196-221 (sums), 618 (fixup later). |
| L530 before the GPU sees them the cells are finished, sorted, and packed | S | Order in Godot: `end_bake` sorts first, then `_fixup_plot` (vox:709-713); packing is later. The list is fine; do not read it as the order. |
| L543 this teapot leaf holds the sums from every triangle that reached it | V (lab) | Lab's worked leaf (63, 6, 29), research.md:277. |
| L547 its alpha is the total of those triangles' shares | V | vox:218-221. |
| L549 finishing divides every sum by alpha | V | vox:597-615. |
| L551 so the leaf holds the average albedo and normal | S | Weighted by columns hit, and the normal is then normalised to unit length (vox:617-627) unless it is shorter than 0.01. Say "average direction". |
| L555 alpha is then set to one | V | vox:629 `alpha = 1.0`. |
| L559 normals can point opposite ways and cancel | V | vox:617-619 comment "too much fight over normal". |
| L563 when their average is shorter than one hundredth the leaf keeps no normal | V | vox:617 `n.length() < 0.01` -> zeroed. |
| L567 the light passes then treat that leaf as facing every way | S | Direct light: `length(normal) > 0.2` else "all directions" (vgi.glsl:442-446, also gi_at_pos 301-304). The bounce skips it instead (vgi.glsl:468, only if length > 0.2). Say "the direct light treats". |
| L583 a parent cell gets no colour at all | V | vox:635-657. |
| L587 albedo, emission, normal set to zero, because the bake never stores light in a parent | V | vox:635-657; the stated reason is the code's own (comment "go down", light made by mip pass vgi.glsl:528-546). |
| L591 parent keeps only its alpha, the sum of its children's alpha divided by eight | V | vox:660-662. |
| L595 a parent with four leaves holds one half | V | 4 x 1.0 / 8. |
| L597 coarser levels hold more open air, fewer children exist, coverage drops | S | True for surfaces (a plane gives 1/2 per level); a corner or a thick wall can keep it higher. |
| L601 coarse levels get light later, rebuilt from leaves on the GPU every time the lights change | V | gi.cpp:3102-3171 (mip dispatches), vgi.glsl:528-546. Same gate as ch1 L74. |
| L618 cells are then sorted by level first, then position | M | They are sorted before the fixup, not after (vox:709-713 `end_bake`: `_sort()` then `_fixup_plot(0, 0)`). The key is level, then x, y, z (voxelizer.h:73-89). Replace "then" with "Godot also sorts...". |
| L622 so the root comes first at index zero | V | vox:536-560 (`ERR_FAIL_COND(sorted_cells[0].level != 0)`). |
| L626 each parent's child indices rewritten | V | vox:579-590. |
| L630 each level fills one unbroken run | V | key puts level first. |
| L634 the bake hands over how many cells each level has | V | vox:801-820. |
| L638 the renderer uses those counts to find where each level's run starts and ends | V | gi.cpp:2631, 2660-2666 (`cell_offset`, `cell_count`). |
| L656 each cell is stored as two records | V | vox:730-799. |
| L658 first record: eight child indices, thirty-two bytes | V | vox:730-745. |
| L662 an index of all ones means empty | V | voxelizer.h:49 `CHILD_EMPTY = 0xFFFFFFFF`. |
| L664 second record sixteen bytes | V | vox:752-799 (4 words). |
| L668 starts with position, in that level's own cells | V | vox:757-770; cell x/y/z are `nx / half` of the child level (vox:282-285). |
| L672 coordinates packed into eleven, ten, eleven bits | V | `position = x | y << 11 | z << 21`: x 11 bits, y 10, z 11. |
| L674 then albedo and coverage, one byte per channel | V | vox:772-780. |
| L678 emission is three nine-bit numbers sharing one exponent | V | color.h:135-175; vgi.glsl:427. |
| L684 this room has no emission | V (lab) | The only emissive object is the panel (study-room.lam:81-87), left out of the bake (lab:world.rs:84). |
| L686 last comes the normal, three signed bytes | V | vox:788-796 (fourth byte unused). |
| L692 bake also measures, on a grid of leaf-sized places, distance to the nearest leaf | V | vox:868-875, 882-889. |
| L696 the field lets shadow rays skip empty space | V | vgi.glsl:25-43 (raymarch advance by the SDF). |
| L700 leaves start at zero, others far away | V | vox:888-901 (`BIG_VAL` 1e20, 0 for leaves). |
| L704 a pass along every column in z finds each place's distance in its column | V | vox:903-912 `edt(..., z_mult, size.z)`. |
| L708 runs again along y, then x | V | vox:914-935. |
| L716 three passes are an exact distance transform | V | squared Euclidean transform (vox:831-866); exact before the byte encoding. |
| L718 each place keeps its distance in one byte | V | vox:945-955. |
| L722 a leaf stores zero, any other stores its distance in whole cells plus one | S | `MIN(uint(sqrt(d2)), 254) + 1`: truncated, and capped at 254 (so at most 255) (vox:947-953). Omits the cap. |
| L726 at the sphere's centre height the field forms a ring around each object | V (lab) | A property of the field; the still was not re-viewed. |
| L730 shadow rays use those distances to jump | V | vgi.glsl:25-43. |
| L748 upload turns the two records into two storage buffers | V | gi.cpp:92-97. |
| L752 turns the field into a 3D texture, one byte per texel | V | gi.cpp:106-115 (R8_UNORM, 3D). |
| L754 Godot also holds a GPU version of the field, compiled out | V | gi.cpp:117 `#if 0`, ends at line 185. |
| L758 so the field the CPU bake made is the one used | V | gi.cpp:113 uploads `p_distance_field`. |
| L760 each VoxelGI in the renderer gets a colour texture the size of the octree | V | gi.cpp:2634-2645 (octree_size). |
| L764 one mip per octree level, each a half-size copy of the one before | V | `tf.mipmaps = levels.size()` (8 for depth 7), gi.cpp:2638, 2660. |
| L768 beside it a buffer of four floats per cell | V | gi.cpp:2652-2657 `total_elements * 16`. |
| L772 in the lab the texture's finest mip takes four megabytes | V (lab) | 128 x 64 x 128 x 4 B = 4 MiB. |
| L774 the cells take under four megabytes, the field one megabyte | S | 56,376 x 48 B = 2.7 MB (true, but loose); field 1 MiB exactly. Say "under three". |
| L791 every leaf stores what its surface is | V | |
| L795 the field stores how far each place is from the nearest leaf | V | |
| L799 none of it is light yet | V | the 3D texture is cleared to zero (gi.cpp:2646). |

### Chapter 3 source-index entries

| Entry | Class | Evidence |
|---|---|---|
| fixup-leaf (detail 629) | V | vox:597-629. |
| fixup-parent | V | vox:630-663. |
| sort-cells (detail 560) | V | vox:538-596. |
| level-counts | V | vox:801-820. |
| export-octree | V | vox:730-745. |
| export-data (details 785, 793) | V | vox:752-799. |
| rgbe9995 | V | color.h:135-180; decode in vgi.glsl:427 (`pow(2, e - 15 - 9)`). |
| sdf-edt (details 898, 926) | V | vox:868-935. |
| edt-1d | V | vox:831-866 (the comment cites prideout.net/blog/distance_fields). |
| sdf-encode | V | vox:945-955. |
| allocate-data (detail 113) | V | gi.cpp:67-120. |
| sdf-texture | V | gi.cpp:106-116 (`mipmaps` left at 1). |
| sdf-gpu-disabled | V | gi.cpp:117-185. |
| instance-texture (detail 2660) | V | gi.cpp:2634-2666. |

## Not verified, with proposed corrections

1. L192 + L196 (M): the lab bakes on the CPU only. Replace with: "The lab bakes this box on the processor, as Godot does. It lights and reads it on the graphics card with code ported from Godot's own, and a copy on the processor checks that the two agree."
2. L219 (M): "Every claim in this video cites..." Replace with "The claims about Godot cite a file, a symbol, and a line in the checkout."
3. L221 (M): the check is the lab's self-test, not the video build: "The lab's self-test checks that each of those lines is still there in the checkout."
4. L618 (M): sorted before the fixup. Replace "The cells are then sorted" with "Godot also sorts the cells, before it finishes them"; or move the beat before c3-fixup.
5. L530 (S): reads as an order; say "the cells are finished, sorted, and packed" only if the order is not implied, or "sorted, finished, packed".
6. L74 (S): "or the VoxelGI's data or settings change" -> "or its data, dynamic range, propagation, or bounce count change". Add that objects marked Dynamic make the texture re-written every frame (ch1 L78 covers this).
7. L78 (S): "Moving objects" -> "Objects marked Dynamic".
8. L46, L71 (S): "lights those cells" / "light every cell" = every leaf; coarser cells get light from the mip pass.
9. L65 (S): "from the nearest surface" -> "from the nearest cell that holds a surface".
10. L67 (S): "helps find which cells are in shadow" -> "...for lights that cast shadows".
11. L50, L161 (S): the cones are the diffuse ones; a reflection cone is also traced. Say "diffuse cones".
12. Parts L114 (S): "a blend" is exact; do not turn it into "average" (divisor `mix(8, count, 0.5)`).
13. L265 (S): add "though only the cells inside the box are made".
14. L288 (S): "A shorter axis gets fewer cells, when it is at most half the longest".
15. L405 (S): "about fifty-six thousand" (56,376). Consider asserting the total in selftest.rs.
16. L450 (S): "adds only half the weight" is in columns hit, not area; keep, or say "crosses half the columns".
17. L483 (S): emission "multiplied by its energy and the bake's exposure" omits the emission operator and physical-unit intensity; acceptable for the story, but the source-index `material-cache` inspect could name them.
18. L551 (S): "average ... normal" -> "average direction" (normalised).
19. L567 (S): "The light passes" -> "The direct light"; the bounce skips such a leaf.
20. L597 (S): "so fewer of its children exist" holds for surfaces; fine, but mark it as the typical case.
21. L722 (S): add "capped at 254".
22. L774 (S): "under three megabytes" (2.7 MB).
23. Source-index `plot-face-recurse` (S): add the grid limit (vox:260-265) to the inspect text. `material-cache` (S): add the emission operator.

No claim was found incorrect, and none needs investigation beyond the visual-only lab statements marked above (not re-rendered).

## Counts

| Scope | V | S | M | X | N |
|---|---|---|---|---|---|
| Ch1 sentences | 29 | 10 | 4 | 0 | 0 |
| Ch1 entries (9) | 9 | 0 | 0 | 0 | 0 |
| Ch2 sentences | 50 | 5 | 0 | 0 | 0 |
| Ch2 entries (8) | 6 | 2 | 0 | 0 | 0 |
| Ch3 sentences | 47 | 6 | 1 | 0 | 0 |
| Ch3 entries (14) | 14 | 0 | 0 | 0 | 0 |
| Total | 155 | 23 | 5 | 0 | 0 |
