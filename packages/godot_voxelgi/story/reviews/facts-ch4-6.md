# Fact check, Godot VoxelGI chapters 4-6

Checkout: /home/oppositenormal/Documents/programming/other/godot at ed1daf0bf001b61586d9930840f2f1394092c079 (confirmed with `git rev-parse HEAD`). Paths: `gi.cpp` = servers/rendering/renderer_rd/environment/gi.cpp, `glsl:` = servers/rendering/renderer_rd/shaders/environment/voxel_gi.glsl, `cull.cpp` = servers/rendering/renderer_scene_cull.cpp. Lab claims were read against packages/godot_voxelgi (README, dynamic_gpu.rs, demos, story/research.md); their classes say 'Lab' in the evidence. Classes: V verified, S correct but simplified, M potentially misleading, I incorrect, N needs investigation. All 29 source-index lineHashes (and 8 detail lines) of chapters 4-6 were recomputed against the checkout and match.

## Chapter 4: Lighting the cells (sentences)

| Line | Sentence | Class | Evidence |
|---|---|---|---|
| L811 | The cells now hold what each surface is. | V | Cells carry albedo, normal, emission (voxelizer.cpp:603-628, 773-790); chapter 3's records. |
| L813 | The light passes ... give each cell the light it receives. | S | Stored value is the light the cell sends back (incident x albedo + emission): voxel_gi.glsl:459 `outputs = accum + emission`; 'receives' is the incident part only. Suggest 'the light each cell sends back'. |
| L827 | The light passes do not run every frame. | V | gi.cpp:4302 voxel_gi_needs_update; renderer_scene_cull.cpp:4004,4067 `update_lights` only on version change or cache_dirty; gi.cpp:3098 `passes=1` re-blit otherwise. |
| L829 | They run only when a light changes, when the bake is redone, or when one of the VoxelGI's settings changes. | S | Version bumps only in set_probe_data (gi.cpp:188), dynamic_range (256), propagation (271), two bounces (344). Energy, bias, normal bias, interior do not bump it. Suggest 'dynamic range, propagation, or the two-bounce switch'. |
| L833 | The culler ... compares every light each frame with a copy of that light's settings, saved earlier. | S | renderer_scene_cull.cpp:3913-4067: compares visible paired lights and visible directional lights (`if (!instance->visible) continue`), only for a VoxelGI in view (update list, cull.cpp:796). The copy is `probe->light_cache`. |
| L837 | Move a light, and the cells are lit again, here the floor under its new position. | V | Transform is in the compared set (cull.cpp:3942 `cache->transform != instance->transform`); floor is the lab's scene. |
| L841 | At most thirty-two lights take part, the engine's limit. | V | gi.h:222 `voxel_gi_max_lights = 32`; gi.cpp:2971 `MIN(32, size)`; shader compiled with MAX_LIGHTS (gi.cpp:3586). Holds for the light passes; the dynamic pass is unclamped (ch6). |
| L845 | Each light's position and range are first converted into the box's cells ... | V | gi.cpp:2975-3002 `to_probe_xform`, `l.radius = to_cell.basis.xform(range).length()`, position via to_probe_xform (also direction, area vectors). |
| L857 | Take this floor cell beside the sphere, the ringed one. | V | Lab scene (demos), not Godot. |
| L861 | The lab adds an extra omni light here, a point light that shines in every direction. | V | Lab: README 'What the lab simplifies' (extra OmniLight3D with shadows). |
| L865 | The pass first finds the direction from the cell to the light. | S | Code computes attenuation first (glsl:210-291 `compute_light_vector`), then `light_dir = normalize(pos - light_pos)` (glsl:290): the vector points from the light to the cell. Order and sign differ; same line. |
| L869 | Then it finds how much the light falls off over the distance between them. | V | glsl:201-208 get_omni_attenuation (windowed (1-(d/r)^4)^2 d^-decay), range cutoff glsl:218; result below 0.01 drops the light (glsl:293). |
| L875 | A cell that faces away from the light gets nothing from it. | V | glsl:293 `length(normal) > 0.2 && dot(normal, light_dir) >= 0` returns false. (A cell with no usable normal is not culled; the lab's cells here have one.) |
| L879 | Otherwise, the light is weighted by the cosine ... | V | glsl:442-443 `max(0.0, dot(normal, -light_dir)) * light`. |
| L883 | That light times the cell's albedo, plus the cell's emission, is the light the cell sends back out. | S | glsl:425-459: `light *= albedo.rgb`, summed, `+ emission` stored. 'Sends back out' is the reading of the stored value, not a name in the code. |
| L897 | When the light casts shadows, a ray starts on the light's side and marches back toward the cell through the distance field. | V | glsl:297-334: `has_shadow`; `light_pos = to - light_dir*distance` after clipping to the box (clip_segment), `raymarch(distance, distance_adv, light_pos, light_dir)` runs light to cell. |
| L901 | At each step, the ray reads from the field how far the nearest surface is, shown as the circle. | V | glsl:184 reads texture_sdf; encoding voxelizer.cpp:947-953 (0 solid, else min(d,254)+1, d = truncated cell distance), so advance = byte*255-1. |
| L905 | Then it jumps that far. | V | glsl:191-194 `from += direction*advance`. |
| L909 | The jump is rounded down to whole cells, along the axis the ray mostly follows. | V | glsl:191 `max(distance_adv, advance - mod(advance, distance_adv))`; distance_adv = get_normal_advance (glsl:244-) = ray length per cell step along the dominant axis. |
| L911 | Here the ray reaches the sphere's cells and reads a solid cell, where the field is zero. | V | Lab scene; texture value 0 = solid (voxelizer.cpp:949-950); glsl:185-188 advance < 0 -> occlusion 0. |
| L915 | So the floor cell is in shadow, and it gets none of this light. | V | glsl:333-335 `occlusion == 0.0` returns false. |
| L919 | But close to a surface, the field reads less than one cell, and then the cell's light is only dimmed. | S | glsl:190 `occlusion = min(advance, occlusion)` (starts 1.0), glsl:338 `attenuation *= occlusion`. Texel values are integers (>= 1 cell) so a value below 1 only appears through trilinear filtering next to a solid texel (gi.cpp:2701, linear, clamp). Say 'the filtered field'. |
| L923 | That gives the shadow a slightly soft edge. | V | Same lines: occlusion in (0,1) scales light. |
| L935 | The ceiling panel is an area light, a glowing rectangle rather than a point. | V | Lab: panel stands for an AreaLight3D (README). |
| L939 | Its light on a cell is an integral over the whole rectangle. | V | glsl:345-397 builds the four corner points, `ltc_evaluate_diff` integrates the quad (area_lights_inc.glsl:343-397). |
| L943 | VoxelGI does not sum the rectangle point by point. | V | No sampling loop in compute_area_light (glsl:345-397). |
| L945 | It evaluates the integral in closed form, with a standard fit called linearly transformed cosines. | M | The integral is a sum of polygon-edge integrals (area_lights_inc.glsl:247-259) using the Hill-Heitz rational approximation (lines 22-39), so analytic but approximate. For diffuse there is no fit: `ltc_evaluate_diff` only rotates the quad into the normal's frame, i.e. the cosine itself (lines 343-397); the fitted matrices (LUT) are specular only (325-340). Suggest 'with the method called linearly transformed cosines (for diffuse, the cosine itself, integrated over the rectangle's edges)'. |
| L947 | The area light never marches a ray through the distance field. | V | compute_area_light (glsl:345-397) has no raymarch call. |
| L951 | So the panel lights the cells under the sphere as if the sphere were not there. | V | Consequence of L947; lab scene. |
| L955 | Godot's direct lighting divides this integral by two pi. | V | area_lights_inc.glsl:322 `integral = I / (2.0 * M_PI)` in ltc_evaluate, which scene_forward_lights_inc.glsl:1144 calls with `mat3(1)` for the diffuse area light. |
| L959 | The code shows that VoxelGI leaves that division out, so the panel lights the cells two pi times as brightly as direct lighting would. | I | First half verified (area_lights_inc.glsl:396). The brightness ratio is not 2 pi in default units: direct lighting scales a light's energy by pi (light_storage.cpp:1046 `energy *= Math::PI`, applied to area lights too, :974) while VoxelGI uses `energy * indirect_energy` with no pi (gi.cpp:2990 / glsl:395); direct area diffuse = I/(2pi) * (E pi) = I E/2, VoxelGI = I E, so about 2x. 2 pi holds only with physical light units (light_storage.cpp:1037-1043 and gi.cpp:2992-3000 both apply 1/(2pi), no pi). Proposed: 'VoxelGI leaves that division out, so its integral is two pi times larger than the one direct lighting uses.' (drop the brightness comparison). |
| L975 | Every leaf runs the same pass, sixty-four leaves to a workgroup on the graphics card. | V | glsl:6-9 `local_size_x = 64`; gi.cpp:3105-3121 dispatches mipmaps[0] (leaves) with wg_size 64; lab: voxelgi_passes.wgsl @workgroup_size(64). |
| L979 | The room's cells take on the panel's light. | V | Lab visual. |
| L981 | Cells the panel sees are bright. | V | Lab visual (LTC clipped to the horizon, glsl area light). |
| L983 | Cells it cannot see stay dark, waiting for the bounce in the next chapter. | V | Lab: selftest.rs:341 (bounce lights leaves the area light did not reach). |
| L1002 | A last pass, the write pass, stores each cell's light in that cell's texel of the three-dimensional texture. | V | glsl:548-551 `imageStore(color_tex, ivec3(posu), ...)`; gi.cpp:3152-3171. |
| L1006 | Each channel of a texel has only eight bits, for values from zero to one. | V | gi.cpp:2634 R8G8B8A8_UNORM. |
| L1008 | So the pass first divides the light by a scale called the dynamic range. | V | glsl:550 `rgb / params.dynamic_range`. |
| L1012 | The dynamic range is two by default, so light up to twice white survives. | V | voxel_gi.h:50 `dynamic_range = 2.0`; doc/classes/VoxelGIData.xml:69 default 2.0; hint range 1-8 (voxel_gi.cpp:234). |
| L1016 | At a range of one, the bright cells under the panel clip to white, and the readout's count of clipped leaves climbs. | V | Lab: demos/lighting.rs:186-215 counts leaves with light > range; brightest leaf 3.3 (story/research.md:277), so it clips at 1 and 2. Not re-rendered. |
| L1020 | The alpha channel stores the cell's coverage, one for a leaf and less for a sparse parent. | V | glsl:550 alpha = albedo.a; leaf alpha = 1 (voxelizer.cpp:629), parent = sum of child alpha / 8 (:662-666), packed in the top byte (:774). |
| L1024 | Texels with no cell stay at zero, fully transparent, so a cone sees straight through them. | V | gi.cpp:2644 texture_clear to 0; only cells are written; cone adds `(1-a)*scolor` (glsl:511). |
| L1028 | Godot's VoxelGI stores no direction with its light. | V | Texture is RGBA8 colour + coverage alpha (gi.cpp:2634); no directional data in any pass. |
| L1032 | The code shows that the pass still receives an anisotropy value, but no line reads it, so that value changes nothing. | V | gi.cpp:3087 `aniso_strength = 0`; glsl:97 declared, no other `aniso` in voxel_gi.glsl. |
| L1050 | So every leaf now holds the direct light it sends out. | V | glsl:459. |
| L1052 | Every shadow in that light comes from marching the distance field, and the area light casts none. | V | Only shadow code in the light pass is raymarch (glsl:297-338); area light has none. |
| L1056 | So far, each cell has only light that came straight from a light. | V | Pass 0 only (gi.cpp:3098). |
| L1060 | The next chapter lets the cells light each other. | V | Chapter 5. |
| L1064 | These are the light pass's lines, from the check that decides when it runs to the texture. | V | Chapter 4's `reading` list runs relight-gate ... unused-push. |

## Chapter 5: The second bounce and the mips (sentences)

| Line | Sentence | Class | Evidence |
|---|---|---|---|
| L1070 | One pass of light reaches only what the lights see. | V | Pass 0 lights from lights only (glsl:425-459). |
| L1072 | A second pass reads back the texture the first pass wrote, so the cells light each other. | V | gi.cpp:3098-3150: pass 1 runs COMPUTE_SECOND_BOUNCE after pass 0's write; glsl:465- `color_texture` read. |
| L1084 | A cone widens as it travels, so it must read coarser copies of the light. | V | glsl:498-506 diameter grows with dist, `textureLod(..., log2_diameter)`. |
| L1088 | The bake left the coarse levels without light, so a pass called the mip pass fills them first. | V | Parents' albedo/emission/normal zeroed (voxelizer.cpp:643-651); gi.cpp:3112-3128 mip pipeline (i>0) runs before the write. |
| L1092 | Each parent sums the light of its children. | V | glsl:526-540. |
| L1094 | Then it divides that sum by a divisor, somewhere between eight and the number of children it really has. | V | glsl:541 `mix(8.0, count, params.propagation)`; propagation range 0-1 (voxel_gi.cpp:238). |
| L1098 | A setting called propagation picks where that divisor falls. | V | Same line. |
| L1102 | Level by level, each mip holds a coarser copy of the light. | V | gi.cpp:3112-3128 one dispatch per level; write per level (3152-3171). |
| L1104 | The last mip is a single texel for the whole box. | V | Texture mips = levels (gi.cpp:2634-2640); the lab's octree 128x64x128 has 8 levels (README), top level 1 cell. |
| L1116 | At a propagation of zero, the divisor is always eight. | V | mix(8,count,0)=8. |
| L1118 | So empty children count as black, and the coarse levels darken. | V | Sum over existing children / 8. |
| L1122 | At a propagation of one, only the children that exist are averaged, so the coarse levels keep their brightness. | V | mix(8,count,1)=count. |
| L1126 | At the default of one half, a parent with four children divides by six, halfway between eight and four. | V | voxel_gi.h:54 `propagation = 0.5`; mix(8,4,0.5)=6. |
| L1130 | Propagation is a choice between light that fades with distance and light that spreads too far. | M | Editorial reading, not in the source. Verified part: higher propagation = smaller divisor = brighter coarse levels that wide cones read (glsl:541). The class doc says only 'higher values result in brighter indirect lighting' (VoxelGIData.xml:82). Suggest 'Higher propagation keeps the coarse levels brighter, so wide cones pick up more light; lower darkens them'. |
| L1144 | Take this floor cell near the red wall, the ringed one. | V | Lab scene. |
| L1148 | The bounce pass ... builds a set of three axes around the cell's normal. | V | glsl:473-477 tangent, bitangent, normal; only for leaves with `length(normal) > 0.2` (glsl:472). |
| L1152 | It sends one cone straight along the normal, with a weight of one quarter. | V | glsl:481-488 cone_dirs[0]=(0,0,1), weight 0.25. |
| L1156 | Five more cones lean sixty degrees out at a weight of point one five each, so the six weights add up to one. | V | z=0.5 -> 60 degrees (glsl:482-486), weights 0.15 (:488); 0.25+5*0.15=1. |
| L1160 | Each cone opens thirty degrees from its axis. | V | glsl:489 `tan_half_angle = 0.577` = atan -> 29.99 degrees half-angle. |
| L1172 | A cone starts one and a half cells from the cell's centre, clear of the cell itself. | V | glsl:495-496 `dist = 1.5` from `pos = posu + 0.5` (cell centre); 1.5 > half the cell's diagonal 0.87, so outside the cell (geometry; the code gives no reason). |
| L1176 | The cone widens as it goes, and on each step it advances by half its current width. | V | glsl:499-500, 509 `dist += half_diameter`. |
| L1178 | At each step, it reads the mip whose texels match that width. | V | glsl:508-509 `textureLod(..., log2(diameter))`. |
| L1180 | Each sample is blended in behind what the cone has already seen, front to back, like layers of glass. | V | glsl:511 `color += (1 - color.a) * scolor`. |
| L1182 | The cone stops once it is ninety-five percent opaque, or when it runs out of distance. | V | glsl:499 `dist < max_distance && color.a < 0.95`; max_distance = length(limits) (box diagonal in cells), bounds test commented out (:504). |
| L1184 | The last cone here takes eleven steps and still ends only two thirds opaque. | S | Lab measurement: 11 steps, alpha 0.64 (story/research.md:277; README 'two thirds opaque (measured)'). 0.64 is rounded to two thirds; not re-run (no GPU job). Suggest 'about two thirds' or 'sixty-four percent'. |
| L1188 | That is because the room's cells are thin shells around open air. | V | Lab: 42,289 leaves of 1,048,576 texels (research.md:277); leaves are surface voxels only. |
| L1209 | Each cone's light is multiplied by its weight, and by the cell's albedo for the bounce off that cell. | V | glsl:515 `color *= cone_weights[i] * vec4(albedo.rgb,1) * dynamic_range`; albedo is the tracing cell's. |
| L1213 | It is also multiplied by the dynamic range, which undoes the division the texture applied. | V | glsl:515 comment 'restore range'; write divides at :550. |
| L1217 | The result is added to the cell's direct light. | V | glsl:470, 516-517 `accum` starts at outputs, `+= color.rgb`. |
| L1219 | Then the mips are filled again, and the texture is written again. | V | gi.cpp:3112-3171 pass 1 repeats mip loop and write loop. |
| L1221 | The floor cells beside the red wall brighten. | V | Lab visual; selftest.rs:341. |
| L1223 | Only the leaves trace bounce cones. | V | gi.cpp:3115-3118 second-bounce pipeline bound for i==0 (leaf level) only. |
| L1225 | They trace them only once, so there is no third bounce. | V | gi.cpp:3098 passes <= 2. |
| L1242 | So the texture now holds two bounces of light, at every level from five-centimetre cells to the whole box. | V | Godot's own term (use_two_bounces, VoxelGIData.xml:85); lab cell 5 cm, 8 levels (README). |
| L1248 | All of it was baked from objects that stand still. | V | Static GI mode only is baked (voxel_gi.cpp:327). |
| L1250 | The next chapter adds one that moves. | V | Chapter 6. |
| L1254 | These are the mip and bounce passes, and the order they run in. | V | Chapter 5 reading list. |

## Chapter 6: Moving objects (sentences)

| Line | Sentence | Class | Evidence |
|---|---|---|---|
| L1260 | The bake knows only what stood still. | V | voxel_gi.cpp:327 skips non-Static GI modes. |
| L1262 | Baking again every frame would be far too slow. | S | Design rationale, not in the code; verified part: the bake is a CPU voxelizer over every mesh plus a distance transform (voxelizer.cpp). Suggest 'The bake is a CPU job over every mesh, so it is not redone each frame.' |
| L1264 | So an object marked Dynamic is drawn into the texture every frame instead. | S | True while the VoxelGI is in view and the object overlaps its box and is visible (cull.cpp:279, 4074-4090; gi.cpp:3175). |
| L1274 | Here the teapot's GI mode is Dynamic, so the bake leaves it out. | V | visual_instance_3d.cpp:492-493 sets USE_DYNAMIC_GI; voxel_gi.cpp:327 skips it. |
| L1276 | The lab's room was baked without it, so the teapot has no cells and no baked light. | V | Lab: README (the room without the teapot, world.rs). |
| L1280 | In every frame where the room's VoxelGI is in view, each of its dynamic objects is drawn into the texture. | V | cull.cpp:796 (added to the update list when culled in view), 4074-4090 loop over `dynamic_geometries` (visible ones), gi.cpp:3175-3392; render_probes once per frame (rendering_server_default.cpp:106). |
| L1284 | That happens whether the object moved or not, so it costs work even when the object stands still. | V | No change test anywhere on that path (cull.cpp:4074-4100, gi.cpp:3175). |
| L1288 | The points on screen are the texels the teapot wrote. | V | Lab overlay from the CPU mirror (README, demos.rs:985). |
| L1290 | Each is shown in the light it received, drawn brighter than the light stored. | V | Lab: demos.rs:946-947 DYNAMIC_GAIN = 4. |
| L1292 | The lab runs the same plot on the graphics card, so the texture the room is shaded from holds the teapot too. | V | Lab: README, dynamic_gpu.rs. |
| L1296 | These points come from the same plot, run again on the processor. | V | Lab: dynamic.rs is the CPU mirror the demonstration draws (README). |
| L1307 | The object's bounding box is drawn at sixteen texels per cell. | V | gi.cpp:2748-2755 first map = 16 x longest axis; :3179 multiplier. The box is the object's AABB in probe space widened by a cell margin (:3198-3210). |
| L1311 | That is fine enough to measure, later, how much of each cell the object covers. | S | The coverage is real: shrink averages 2x2 blocks including empty texels (glsl:650 `accum /= 4.0`), so alpha at 1 texel/cell is the covered fraction (glsl:681-700 uses it as blend alpha). The purpose is an inference; the code states none. |
| L1315 | The object is drawn six times, with orthographic views along each axis in both directions, so no side is missed. | S | gi.cpp:3230-3260 six directions, `set_orthogonal` (:3265). 'So no side is missed' is the narration's reason, not stated in the code. |
| L1319 | Each view records the material's albedo, normal, and emission at every texel, along with its depth. | V | scene_forward_clustered.glsl:3003-3017 (also ORM and sss strength); gi.cpp:2800-2845 targets. |
| L1323 | A texel's depth gives its place in the grid. | V | glsl:574-578 `abs(z_dir) * int(z)`. |
| L1327 | Each texel is then lit like a cell, with shadows from the baked distance field alone. | V | glsl:586-604 same compute_light_at_pos / compute_area_light; area lights unshadowed. |
| L1341 | The lit view is then halved again and again, with each two-by-two block of texels becoming one. | V | glsl:617-662; gi.cpp:3326-3392. |
| L1345 | It goes from sixteen texels a cell to eight, then four, then two, then one. | V | gi.cpp:2748-2760 maps 16x, 8x, 4x, 2x, then mip 0 at 1x. |
| L1349 | At one texel a cell, a compute pass plots each texel, writing it into the texture's finest mip. | V | gi.cpp:2882-2883, 2933 (plot into mipmaps[dmap.mipmap].texture), pipeline choice :3378-3383. |
| L1353 | A texel's depth usually falls between two cells along the view. | V | glsl:673 `z_frac = fract(accum_z)`. |
| L1357 | So the texel is split between those two cells, by distance. | V | glsl:681 weights 1-z_frac and z_frac. |
| L1361 | Each share is blended over what is already there. | V | glsl:681-705 alpha-over with the texel's existing colour. |
| L1363 | Each further halving plots into the next, coarser mip. | V | gi.cpp:2882, 2933, 3374. |
| L1365 | Each halving divides the light the way the mip pass does, by the divisor that propagation sets. | I | Only the halvings onto mips coarser than the finest: `on_mipmap = dynamic_maps[k].mipmap > 0` (gi.cpp:3374), glsl:647-648 `rgb /= mix(8,count,propagation); a /= 8`. The halvings 16->8->4->2->1 (the last one onto mip 0) take `accum /= 4.0` (glsl:650). Proposed: 'Each halving onto a coarser mip divides the light the way the mip pass does; the halvings before it average each block's four texels.' |
| L1378 | At the finest mip, the teapot is drawn in five-centimetre cells. | V | Lab: 5 cm cells (README). |
| L1382 | One mip up, the cells are ten centimetres wide. | V | Lab: each mip halves resolution. |
| L1384 | Each mip halves the detail again. | V | Mip chain. |
| L1388 | These coarser copies are what the wider cones will read. | V | Cone lod = log2(diameter) (gi.glsl cone trace; voxel_gi.glsl:508). |
| L1399 | Before the teapot is plotted, the whole texture is cleared. | V | gi.cpp:2963-2966 `texture_clear` on all mips when last update had dynamic data (:3175, 3394); first dynamic frame has nothing to clear. |
| L1403 | Then the static light is written back from the float buffer, which keeps every cell's light at full precision. | V | gi.cpp:3067-3171 re-blit (passes = 1, :3100); outputs buffer is vec4 float (glsl:101-103; gi.cpp:2649 16 bytes per cell). |
| L1407 | So the dynamic texels exist only for this frame, at every mip. | V | Cleared next frame; plot hits every mip (gi.cpp:2882). |
| L1411 | The bounce already ran in the light passes, before the teapot was plotted. | V | gi.cpp:3067-3171 precedes :3175-3392. |
| L1415 | So no light bounces off the teapot. | V | Second bounce only reads the static texture and writes leaves (glsl:465-522); dynamic texels never reach the buffer. |
| L1417 | It casts no shadow either, because the baked distance field does not contain it. | S | True of VoxelGI's light passes (glsl:297-338 read texture_sdf only); shadow-mapped direct light is separate. Say 'in VoxelGI's light'. |
| L1421 | Here the cost is six draws and seventy-two compute dispatches per object, every frame. | S | 6 views x (1 lighting + 11 shrinks): maps = 4 + 8 mips = 12 (gi.cpp:2748-2760, 3321-3387). Lab: dynamic_gpu.rs commands(), 6 draws. Draws in Godot are per surface (`_render_material`, gi.cpp:3279): six for a one-surface mesh like the lab's teapot. |
| L1425 | In chapter four, at most thirty-two lights took part. | V | See L841. |
| L1429 | The code shows that this pass has no such limit, so past thirty-two lights it reads beyond the end of the light buffer. | V | gi.cpp:3289 `light_count = p_light_instances.size()` (not clamped); glsl:70 `Light data[MAX_LIGHTS]` (32) indexed to light_count (glsl:583). Result is undefined (out-of-bounds uniform read). |
| L1439 | So a moving object shows up in the same texture, redrawn every frame. | V | gi.cpp:3175-3394. |
| L1443 | The object is lit directly, but no light bounces off it. | V | See L1415. |
| L1445 | Static or moving, everything ends in that one texture. | V | Texture `texture` (gi.cpp:2641). |
| L1447 | The next chapter reads it back, one pixel at a time. | V | gi.glsl per-pixel pass (chapter 7); Godot's default is full resolution (half-resolution option off). |
| L1451 | These are the lines of the dynamic objects' path. | V | Chapter 6 reading list. |

## Source-index `inspect` texts (chapters 4-6)

| Entry | Class | Evidence |
|---|---|---|
| relight-gate | V | gi.cpp:4302; bumps at 188/256/271/344; cull.cpp:4004-4067; dynamic path gi.cpp:2963-3171. |
| light-cache | S | cull.cpp:3913-4067: list matches (shadow, type, transform, colour, energy, intensity, bake_energy=indirect, radius=range, attenuation, spot angle/attenuation, area size/normalize/texture; directional: sky mode). Only visible lights are compared; also the light instance identity and the count. 'paired' omits 'visible'. |
| max-lights | V | gi.h:222; gi.cpp:3586. (The dynamic pass is not clamped, see dynamic-light-count.) |
| light-upload | V | gi.cpp:2970-3063; with physical light units energy also takes intensity, exposure and a per-type factor (:2993-3005). |
| compute-light | V | glsl:425-459 (area lights add no separate cosine: it is inside the LTC integral). |
| light-at-pos | M | 'a march through the distance field toward the light' reverses the march: it runs from the light's end toward the cell (glsl:297-334; the raymarch text says so). Also omits the range cutoff (glsl:221) and spot cone (glsl:229-240). Suggest 'between the light and the cell'. |
| omni-atten | V | glsl:201-208; distances in the VoxelGI's local metres via cell_size (gi.cpp:3076-3081). |
| raymarch | V | glsl:180-199; encoding voxelizer.cpp:947-953. Omits the loop's end (`distance > 0.5`). |
| sdf-sampler | V | gi.cpp:2701; material_storage.cpp:2717-2720 REPEAT_DISABLED = CLAMP_TO_EDGE; 3D linear filter. |
| area-light | V | glsl:372-383 closest point sets light_length/range window; LTC; no march. |
| ltc | V | area_lights_inc.glsl:396 comment. |
| write-texture | V | glsl:548-551; RGBA8 gi.cpp:2634. |
| unused-push | V | glsl:84-98, only aniso_strength/emission_scale/stack_size declared, never read; gi.cpp:3079-3088 sets them. |
| update-mipmaps | V | glsl:526-545. |
| pass-order | V | gi.cpp:3098-3171; write after each pass; without a light update only the write runs (when dynamic data exists, :3067). |
| second-bounce | V | glsl:465-522. |
| bounce-cones | V | glsl:481-488. |
| bounce-march | V | glsl:495-512 (also ends at max_distance = box diagonal; the text omits that end). |
| bounce-restore | V | glsl:515-517. |
| gi-mode-flags | V | visual_instance_3d.cpp:492-494; cull.cpp:279 (`A->dynamic_gi` -> dynamic_geometries); voxel_gi.cpp:327. |
| dynamic-list | V | cull.cpp:4074-4100 (visible instances); 'whether it moved or not': no change test. |
| dynamic-oversample | V | gi.cpp:2748-2760, 3179. |
| dynamic-views | V | gi.cpp:3230-3249, 3279. |
| material-outputs | V | scene_forward_clustered.glsl:3003-3017 (the ORM alpha also carries the SSS strength). |
| dynamic-lighting | V | glsl:574-613. |
| shrink | S | glsl:617-662. 'before the first mip it divides the sum by 4' is ambiguous: the map plotted into mip 0 still takes /4 (`on_mipmap = mipmap > 0`, gi.cpp:3374); only mips 1.. use mix(8,count,propagation) and alpha/8. Suggest 'up to the finest mip ... ; onto each coarser mip ...'. |
| plot-blend | V | glsl:673-705. |
| dynamic-clear | V | gi.cpp:2963-2966, 3067-3171. |
| reblit | V | gi.cpp:3097-3100; write loop 3152. |
| dynamic-light-count | V | gi.cpp:3289 against 2971. |

## Counts

- verified: 139
- correct but simplified: 15
- potentially misleading: 3
- incorrect: 2
- needs investigation: 0
- total: 159 (sentences 129, inspect texts 30)

## Not verified, with the proposed correction

1. **Incorrect, ch4 L959** (c4-area): 'two pi times as brightly as direct lighting would'. Direct lighting multiplies a light's energy by pi in default units (light_storage.cpp:1046) and VoxelGI does not (gi.cpp:2990), so the integral's missing 1/(2 pi) nets about 2x, not 2 pi; 2 pi holds only with physical light units. Say only: 'VoxelGI leaves that division out, so its integral is two pi times larger than the one direct lighting uses.'
2. **Incorrect, ch6 L1365** (c6-shrink): 'Each halving divides the light the way the mip pass does'. Only halvings onto mips coarser than the finest do (`on_mipmap`); the halvings from 16 texels a cell down to mip 0 average the four texels (`/ 4`). Say: 'Each halving onto a coarser mip divides the light the way the mip pass does; the halvings before it average each block's four texels.'
3. **Misleading, ch4 L945**: 'a standard fit called linearly transformed cosines'. Diffuse uses no fit (the matrix is the identity, the cosine itself; the fits are specular); the edge integral is an analytic approximation. Say: 'with the method called linearly transformed cosines, which for diffuse light is the cosine itself, integrated over the rectangle's edges'.
4. **Misleading, ch5 L1130**: 'light that fades with distance and light that spreads too far' is an editorial reading. Say: 'A higher propagation keeps the coarse levels brighter, so wide cones pick up more light; a lower one darkens them.'
5. **Misleading, inspect `light-at-pos`**: 'a march ... toward the light' runs the other way (light to cell). Say 'between the light and the cell'.
6. **Simplified, ch4 L829**: 'one of the VoxelGI's settings' is only dynamic range, propagation, and two bounces (or new data). Name them.
7. **Simplified, ch4 L813 / L883**: 'the light it receives' vs 'the light the cell sends back out': the stored value is albedo x incident light + emission. Use 'sends back' in both.
8. **Simplified, ch4 L833**: only visible lights are compared, and only for a VoxelGI in view. (Inspect `light-cache`: add 'visible'.)
9. **Simplified, ch4 L865**: code computes the falloff first and the vector points from light to cell; reorder or soften 'from the cell to the light'.
10. **Simplified, ch4 L919**: the field's texels are whole cells; a value below one cell comes from filtering next to a solid texel. Say 'the filtered field'.
11. **Simplified, ch5 L1184**: the lab measured alpha 0.64 (11 steps); 'two thirds' rounds it. Say 'about two thirds'. Not re-rendered (no GPU job run).
12. **Simplified, ch6 L1262/L1264**: 'far too slow' is a rationale, not code; the dynamic draw happens only while the VoxelGI is in view and the object overlaps its box.
13. **Simplified, ch6 L1311/L1315**: the purposes ('fine enough to measure coverage', 'so no side is missed') are inferences from the code's effect, not stated in it; hedge or keep as the lab's reading.
14. **Simplified, ch6 L1417**: 'casts no shadow' means in VoxelGI's light; shadow-mapped direct light is separate.
15. **Simplified, ch6 L1421**: six draws is for a one-surface mesh (Godot draws per surface); 72 = 6 x (4 + 8 mips) holds for the lab's 8-level octree.
16. **Simplified, inspect `shrink`**: 'before the first mip' is ambiguous (mip 0 also takes /4). Say 'up to the finest mip ... onto each coarser mip'.
