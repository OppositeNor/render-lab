# Fact check, chapters 6 to 11 (Godot 4.7.2-stable, ed1daf0)

Paths are under `servers/rendering/renderer_rd/` in the Godot checkout unless a path starts with `packages/` (the lab). `gi.cpp` is `environment/gi.cpp`; shaders are in `shaders/environment/` unless named. Lab claims were checked against `packages/godot_sdfgi/src/godot/sdfgi/`.

Verdicts: VERIFIED, WRONG, UNCERTAIN, SIMPLIFICATION. No sentence in these chapters uses `${spoken}` or `${spokenFloat}`.

| Beat id | Sentence (first 8 words) | Verdict | Evidence / correction |
|---|---|---|---|
| c6-shells | While SDFGI builds the distance field for a region, | VERIFIED | `render_region` runs the preprocess (jump flood, then occlusion, then STORE) after the last region of a cascade: gi.cpp:2061-2385 (occlusion block 2342-2370). |
| c6-shells | That matters because a probe behind a wall | VERIFIED | The visibility multiplies the probe weight (`weight *= max(occlusion, 0.01)`, gi.glsl:246) when use_occlusion is on. |
| c6-shells | For each probe, it works on a cube of sixteen cells | VERIFIED | `OCCLUSION_SIZE` = CASCADE_SIZE / PROBE_DIVISOR = 128 / 16 = 8 (gi.cpp:3643, gi.h:562-563); each group covers `OCCLUSION_SIZE * 2` = 16 cells per side (sdfgi_preprocess.glsl:489-491). Probe spacing is 8 cells, so the cube reaches the neighbouring probes. |
| c6-shells | Each cell in that cube gets a visibility, from one | VERIFIED | Free cell next to the probe: `occ = 1.0` (preprocess.glsl:~585); solid: `occ = 0.0` (:~578). Values are in [0, 1]. |
| c6-shells | SDFGI fills in the visibility layer by layer, | VERIFIED | `OCC_STEPS = OCCLUSION_SIZE * 3 - 2` = 22 steps, each a shell of growing then shrinking `proc_abs` (preprocess.glsl:535-551), with a barrier after each. |
| c6-shells | Each cell takes the average visibility of its neighbours | VERIFIED | `read_x/y/z` one step toward the probe, `occ /= avg` (preprocess.glsl:~600-648). Up to three neighbours. |
| c6-shells | A solid cell has zero visibility, so the cells behind | SIMPLIFICATION | During the propagation a solid cell is 0 and still counts in the average (preprocess.glsl:~620-640). A later pass ("bias solid voxels away", :665-720) overwrites solid cells with the average of their free neighbours on the probe side, so the final stored value of a solid cell is not zero. |
| c6-store | Every cell lies inside a cube with a probe | VERIFIED | The probe lattice has spacing `cascade_size / PROBE_DIVISOR` = 8 cells (gi.cpp:2345); 17 probes per axis (gi.cpp:427). |
| c6-store | So each cell keeps eight visibilities, one for each | VERIFIED | STORE packs 8 parity classes, 4 bits each: `clamp(occ * 15.0, 0, 15) << occlusion_shift[i]` (preprocess.glsl:966-976), two halves of 16 bits (:979-983). |
| c6-store | These visibilities are read only when the environment's | VERIFIED | Every read is guarded by `use_occlusion`: gi.glsl:233, scene_forward_gi_inc.glsl:168, volumetric_fog_process.glsl:762, sdfgi_direct_light.glsl:253. Default `sdfgi_use_occlusion = false` (scene/resources/environment.h:157). |
| c6-store | The code shows that the visibility is still computed | VERIFIED | The block at gi.cpp:2342-2370 has no `uses_occlusion` test; `uses_occlusion` appears only at gi.cpp:421, 1270, 1848, 2575 (push constants), none around render_region. |
| c6-store | So by default, that work is done and never used. | VERIFIED | Follows from the two rows above. |
| c7-cells | With the distance field and the visibility built, the solid | VERIFIED | Order in `_render_scene`: `_update_sdfgi` (render_region, static lights), then `update_cascades`, `pre_process_gi`, `update_light` (render_forward_clustered.cpp:1734-1759). |
| c7-cells | Later, the probes' rays will hit these cells | VERIFIED | probe-hit reads `light_cascades` (sdfgi_integrate.glsl:262). |
| c7-cells | A compute shader runs one thread for each solid cell | VERIFIED | `compute_list_dispatch_indirect(... solid_cell_dispatch_buffer_call)` over the counted solid cells (gi.cpp:1305); thread `voxel_index` indexes `process_voxels` (sdfgi_direct_light.glsl:180-195). |
| c7-cells | By default, a frame relights only every fourth cell | VERIFIED | Default `frames_to_update_lights` = 2 (rendering_server.cpp:3795) selects `{1,2,4,8,16}[2]` = 4 (gi.cpp:1291-1298); `voxel_index = id * increment + offset` (direct_light.glsl:180-186). A just-rebuilt cascade is lit entirely (gi.cpp:1287). |
| c7-cells | The next frame takes the next quarter, and so on. | VERIFIED | `process_offset = frame_number % frames_to_update` (gi.cpp:1297). |
| c7-cells | So a moving light takes up to four frames to reach | VERIFIED | Cells not processed keep their old texel (direct_light.glsl:184-188); N = 4. |
| c7-omni | An extra omni light now lights the room, | VERIFIED | Lab demo (not an engine claim). |
| c7-omni | The falloff starts from the cell's distance, divided | VERIFIED | `get_omni_attenuation(distance, 1.0 / radius, decay)`: `nd = distance * inv_range` (direct_light.glsl:125-132); radius is `LIGHT_PARAM_RANGE` (gi.cpp:2000+, "radius"). |
| c7-omni | The fade is the square of one minus that ratio's fourth | VERIFIED | `nd^4`, `max(1 - nd, 0)`, squared (direct_light.glsl:127-131); zero at distance = range. |
| c7-omni | That fade is then divided by the distance raised | VERIFIED | `* pow(max(distance, 0.0001), -decay)` (direct_light.glsl:132); decay = the light's attenuation, passed as `lights.data[i].attenuation` (:333, :338). Light3D default attenuation 1 (scene/3d/light_3d.cpp:495). |
| c7-omni | So by default, the light falls off with the distance, | VERIFIED | With attenuation 1 the factor is d^-1. Godot's own lights use attenuation 1 as default too. |
| c7-omni | The code shows that the cell's distance to the light | VERIFIED | Light position is `pos.y *= y_mult` (gi.cpp:~2010); cell position is in the y-squashed cascade space; `light_distance = length(rel_vec)` (direct_light.glsl:333-335). The following `rel_vec.y /= params.y_mult` has no effect (rel_vec unused after). Default y scale 75% gives y_mult 1.5 (gi.cpp:423-424; environment.h:156). |
| c7-omni | So the light behaves as if it hung one and a half | VERIFIED | Height difference between light and cell is multiplied by 1.5, horizontal is not. |
| c7-omni | The readout shows that the lab's model gets the same result. | VERIFIED | Lab (`sim::omni` multiplies y by y_mult, demos.rs:286, sim.rs:441-444). |
| c7-shadow | A shadow ray starts at the cell and steps toward the light. | VERIFIED | direct_light.glsl:380-430 (start bias 0.48 cell + 0.4 cell along the ray). |
| c7-shadow | Each step is as long as the distance the field stores | SIMPLIFICATION | The step is `texture(sdf).r * 255.0 - 1.0` cells (direct_light.glsl:408): the stored distance minus one cell. |
| c7-shadow | That is the longest step that cannot jump over a surface. | VERIFIED | Sphere tracing on a distance field. |
| c7-shadow | When the ray reaches a solid cell, the light is blocked | VERIFIED | `distance < 0.001` is a hit, `hit = true`, and no light is added when `hit` (direct_light.glsl:409-412, :445-453). |
| c7-shadow | If the ray leaves the coarsest cascade without a hit, | SIMPLIFICATION | True (the loop ends and `!hit` adds the light, :445). The ray also ends lit when it travels the whole distance to the light (`advance >= local_distance`, :434-436), which is the common case. |
| c7-shadow | The shadow is all or nothing, so a cell is either | VERIFIED | `if (!hit) light_accum += ...` (direct_light.glsl:445-453); no partial value. Per light. |
| c7-shadow | Every light gets this shadow ray, even one whose own | VERIFIED | `has_shadow` is declared (direct_light.glsl:69) and never read; the march runs for every light with `attenuation >= 0.001` (:368). |
| c7-faces | A voxel has no single normal, so its light is summed | VERIFIED | `valid_aniso` bits: `light_accum[j] += max(0, dot(aniso_dir[j], direction)) * light` for each exposed face (direct_light.glsl:445-452). |
| c7-faces | The cell stores the sum of all six as one colour, | VERIFIED | `light_total` as E5B9G9R9 into `dst_light`; per-face `lumas[i] / luma_total` into aniso0/aniso1 (direct_light.glsl:456-520, 541-548). |
| c7-faces | The cell's light is also copied into the empty neighbours | VERIFIED | Neighbour bits are set where the neighbour's jump-flood position equals this cell (preprocess.glsl:1004-1019); the copy loop is direct_light.glsl:551-557. Solid cells point at themselves, so the flagged neighbours are empty cells. |
| c7-faces | So a filtered read next to a wall blends in the wall's | VERIFIED | Consequence of the copy (comment "so light interpolation during the indirect pass works", direct_light.glsl:~518). |
| c7-static | A static light is written into its cells once, | VERIFIED | `render_static_lights` runs only on a frame where regions were rebuilt and static lights culled (renderer_scene_cull.cpp:3650-3673; render_forward_clustered.cpp:1276), with `process_offset 0, increment 1` (gi.cpp:2573-2580). |
| c7-static | Its light is added on top of whatever light the cell | VERIFIED | The shader first adds the cell's existing stored light into `light_accum` (direct_light.glsl:296-309), then the light loop, then writes back (:560-570). |
| c7-double | When a cascade jumps, the cells it keeps carry their | VERIFIED | MODE_SCROLL copies each kept cell's light word to the new position (preprocess.glsl:180-205). |
| c7-double | That stored light includes any static light the cells | VERIFIED | The word is the one written by the static pass (index scroll-reinject). |
| c7-double | The code shows that whenever a static light overlaps | VERIFIED | Static lights are culled against the pending region bounds (renderer_scene_cull.cpp:3275, 3401), and the static pass runs over every solid cell of the cascade, kept ones included (gi.cpp:2579-2600); the light is read-modify-written. The lab selftest asserts the effect (selftest.rs:378-407). |
| c7-double | So each jump adds the light once more, and in the lab's model | VERIFIED | Lab: selftest.rs:407 checks "2x, 3x, 4x". |
| c7-bounce | The light pass also lets light bounce more than once. | VERIFIED | bounce_feedback path in MODE_PROCESS_DYNAMIC (direct_light.glsl:206-295). |
| c7-bounce | Before the lights are added, each cell reads the light | VERIFIED | The indirect block runs first ("Add indirect light first", direct_light.glsl:205) over 8 probes (`j < 8`, :228). Per-face weighted, so the plain wording is a simplification of the weighting only. |
| c7-bounce | The cell multiplies that probe light by its own colour | VERIFIED | `feedback = albedo * bounce_feedback` (below 1; `mix(albedo, 1, bounce - 1)` above), `light_accum[k] *= feedback` (direct_light.glsl:207, :288-292). |
| c7-bounce | The bounce feedback scales each bounce, and its default | VERIFIED | Default `sdfgi_bounce_feedback = 0.5` (environment.h:158). |
| c7-bounce | In the readout, a cell on the red wall brightens | VERIFIED | Lab demo. |
| c7-bounce | The probes were filled from these same cells a frame earlier. | VERIFIED | The octahedral maps read are stored by `store_probes` in the previous frame (render_forward_clustered.cpp:1534 is after `update_light` at 1759 in the frame order). |
| c7-bounce | So the light makes a loop, from the cells to the probes | VERIFIED | As above; one bounce per frame. |
| c8-rays | The cells now hold light, but only where the surfaces are. | VERIFIED | Light is stored per solid cell. |
| c8-rays | Every frame, each probe traces eight rays. | VERIFIED | `ray_count = {4, 8, ...}[gi->sdfgi_ray_count]` with default project setting index 1 (rendering_server.cpp:3793, gi.cpp:1322-1323) = 8; update_probes runs from render_forward_clustered.cpp:2132 each frame. |
| c8-rays | The rays' directions follow a spiral over the sphere | VERIFIED | `vogel_hemisphere` (sdfgi_integrate.glsl:89-95) with z sign from index parity. |
| c8-rays | Over thirty frames, the rays cover two hundred forty | VERIFIED | `ray_total = history_size * ray_count` = 30 x 8 = 240 (integrate.glsl:~180); default frames_to_converge index 5 = 30 (rendering_server.cpp:3794, render_forward_clustered.cpp:4053). |
| c8-rays | The code numbers each ray, and whether the number is odd | VERIFIED | `y * (float(p_index & 1) * 2.0 - 1.0)` (integrate.glsl:94): parity picks the z hemisphere. |
| c8-rays | One frame's ray numbers are thirty apart, and thirty is even | VERIFIED | `ray_offset + i * ray_mult` with `ray_mult = history_size` (integrate.glsl:~181-190). |
| c8-rays | So each frame's eight rays point into the same half | VERIFIED | Same parity for every i. |
| c8-rays | The halves alternate from frame to frame | VERIFIED | `history_index = render_pass % 30`, `render_pass++` per call (gi.cpp:1320, 1394); 29 to 0 also flips parity. |
| c8-march | A probe's ray marches the same way as the shadow ray | VERIFIED | Both step `sample * 255 - 1` (integrate.glsl:~228-234; direct_light.glsl:408). |
| c8-march | The ray stops when it is within a twentieth of a cell | VERIFIED | `distance < 0.05` (integrate.glsl:228). The distance is the stored value minus one cell, as in the shadow march. |
| c8-march | When the ray leaves the first cascade, it carries on | VERIFIED | Loop `for j = params.cascade; j < max_cascades` (integrate.glsl:~214, 241-246); starts in the probe's own cascade (the first for cascade-0 probes). |
| c8-hit | At a hit, the field's gradient yields the surface normal, | VERIFIED | Central differences of the stored distance, epsilon 0.001 (integrate.glsl:254-258). Distance grows away from the surface. |
| c8-hit | That normal decides how much of each face's light | VERIFIED | `dot(max(0, n * aniso0), 1) + dot(max(0, -n * aniso1), 1)` (integrate.glsl:266). |
| c8-hit | The ray brings that light back to the probe. | VERIFIED | `light.rgb` goes into the SH accumulation. |
| c8-hit | That light already includes the bounce that the last chapter | VERIFIED | The voxel light texture holds the dynamic pass's output including bounce. |
| c8-miss | A ray that leaves every cascade reads the sky instead. | SIMPLIFICATION | Only with a sky background and `read_sky_light` (default true). A colour background reads that colour, otherwise black (integrate.glsl:271-290). |
| c8-miss | It reads the sky's mip level two, a blurred copy. | VERIFIED | `textureLod(..., 2.0)` (integrate.glsl:280, 282). |
| c8-miss | A comment in the code says the reason is that a probe | VERIFIED | "Use second mipmap because we don't usually throw a lot of rays, so this compensates." (integrate.glsl:280). |
| c8-miss | With the blur, those few rays still add up to | VERIFIED | Same comment (intent). |
| c8-miss | The code shows that in four point seven stable, the probe | VERIFIED | gi.cpp:3669-3670 defines `USE_OCTMAP_ARRAY`; the shader tests `USE_RADIANCE_OCTMAP_ARRAY` (integrate.glsl:46-50). Same line in 4.7-stable, 4.7.1-stable and 4.7.2-stable (git show of each tag). |
| c8-miss | But by default on desktop, the sky is stored as an array | VERIFIED | `texture_array_reflections` default true, `.mobile` false (rendering_server.cpp:3697-3698; sky.cpp:700). |
| c8-miss | So the code does not define what an escaping ray reads. | VERIFIED | A `texture2D` declaration is bound to an array texture; the behaviour is not defined by the code. |
| c8-miss | A later commit fixes the mismatch. | VERIFIED | d9ea5c261e (6 May 2026, on origin/master, not an ancestor of 4.7.2-stable) changes the define to `USE_RADIANCE_OCTMAP_ARRAY` (gi.cpp:3670). Its title is "Add error checks for texture type and storage format mismatches"; it is not described as a fix in its title. |
| c8-sh | A probe stores its light as spherical harmonics, | VERIFIED | `SH_ACCUM` (integrate.glsl:295-322). |
| c8-sh | Each ray's light is added into sixteen of these | VERIFIED | `SH_SIZE = 16` (gi.h:568); 16 `SH_ACCUM` calls. |
| c8-sh | The coefficients are then scaled by one thousand twenty-four | VERIFIED | `value * 4 / ray_count`, then `* float(1 << HISTORY_BITS)`, HISTORY_BITS = 10, clamped to 16 bits (integrate.glsl:52, 328-331). |
| c8-sh | Integers add and subtract exactly, and the probe's history | VERIFIED | `average -= prev_value; average += ivalue` on ivec4 (integrate.glsl:335-336). |
| c8-history | The probe keeps its last thirty frames in a ring, | VERIFIED | History texture with `history_size` layers, slot `history_index` (gi.cpp:498-502, 1320). |
| c8-history | Each frame, the probe subtracts the oldest frame from its sum | VERIFIED | integrate.glsl:335-336. |
| c8-history | So when a light turns on, the probe brightens by a thirtieth | VERIFIED | Sliding-window box filter over 30 slots (lab chart shows it). |
| c8-history | The chart on screen shows that as a straight line, | VERIFIED | Lab demo, linear ramp to 30 frames. |
| c8-history | So a light that moves takes thirty frames to show up fully. | VERIFIED | Same window; the 4-frame light lag adds to it. |
| c9-oct | The sixteen coefficients from the last chapter are not | VERIFIED | Coefficients are in a 2D integer texture (`lightprobe_average_texture`). |
| c9-oct | Each probe gets two maps of six by six texels. | VERIFIED | `LIGHTPROBE_OCT_SIZE = 6` (gi.h:567); irradiance at layer `cascade`, radiance at `cascade + max_cascades` (integrate.glsl:~470). |
| c9-oct | Each is an octahedral map, which folds the sphere | VERIFIED | `octahedron_encode` (integrate.glsl:~110-120, 362). |
| c9-oct | One map holds irradiance, the light a surface facing | VERIFIED | SH summed with `l_mult` = cosine convolution (integrate.glsl:~372-420). |
| c9-oct | The other map holds radiance, the light arriving | VERIFIED | Same SH sum without `l_mult` (integrate.glsl:424-426); it is a low-order reconstruction. |
| c9-oct | A diffuse surface reads the irradiance map, and a reflection | VERIFIED | gi.glsl:208-262 (diffuse from `pos_uvw`, specular from `+ max_cascades`). |
| c9-oct | Each map has a border of one texel | VERIFIED | `OCT_SIZE + 2` block with `+1` offset (integrate.glsl:357; gi.glsl:206). |
| c9-oct | Bilinear filtering near a map's edge reads one texel | VERIFIED | Index oct-border. |
| c9-oct | So the border holds copies of the edge texels, mirrored | VERIFIED | `copy_to[1] = (OCT_SIZE - x - 1, -1)` etc. (integrate.glsl:437-456). |
| c9-gain | To turn the rays into the light over the whole sphere, | VERIFIED | Monte Carlo: (4 pi / N) sum of L Y. |
| c9-gain | Four pi is the solid angle of the whole sphere. | VERIFIED | Math. |
| c9-gain | The code instead multiplies by four over the ray count, | VERIFIED | `* 4.0 / float(params.ray_count)` (integrate.glsl:328). |
| c9-gain | Then it multiplies by four again, when it stores the maps. | VERIFIED | `sh.rgb * c[i] * 4.0` (integrate.glsl:424). |
| c9-gain | Four times four is sixteen instead of four pi, so the | VERIFIED | 16 / (4 pi) = 4 / pi. With `l_mult` 1, 2/3, 1/4 (cosine convolution divided by pi) a uniform sky of radiance L gives 4/pi L. |
| c9-gain | So a uniform sky comes out about twenty-seven percent | VERIFIED | 4/pi = 1.2732 (27.3 percent). Godot's sky ambient is 1.0 L. Whether the gain is intentional is not stated in the source (the narration says "too bright", which assumes a 1.0 reference). |
| c9-gain | The readout shows the same extra brightness in the lab's | VERIFIED | Lab (screen.rs/probes.rs port). |
| c9-texel | The code computes each texel's direction from the texel's | VERIFIED | `octahedron_encode(vec2(local_pos) / OCT_SIZE)` with no 0.5 (integrate.glsl:362). |
| c9-texel | But a lookup treats a texel's value as sitting at its | VERIFIED | Reader samples at `block + uv * 6` as a texel-corner coordinate with hardware bilinear (gi.glsl:204-208, 212). |
| c9-texel | So every lookup lands half a texel off and returns a | SIMPLIFICATION | The offset is half a texel on each axis, so a 2D bilinear fetch blends up to four texels (two per axis), not two. |
| c9-texel | In effect, the whole map is shifted by half a texel, | VERIFIED | Index oct-read. |
| c9-scroll | When a cascade jumps, its probes' histories move with it, | VERIFIED | MODE_SCROLL: `read_probe = probe_cell - scroll`, all `history_size` layers and the average copied (integrate.glsl:482-505). |
| c9-scroll | So the probes that remain keep their thirty frames | VERIFIED | Same. |
| c9-scroll | A probe that the jump brings in at the edge has no history | VERIFIED | The `else if (cascade < max_cascades - 1)` branch (integrate.glsl:507). |
| c9-scroll | It starts from the light of the coarser cascade's probes | SIMPLIFICATION | True for all but the last cascade, which copies what is already there (integrate.glsl:~573); the 8 parent probes are trilinear blended (:537-560). |
| c10-pass | Every probe now has its maps, and the screen pass from chapter | VERIFIED | `store_probes` (render_forward_clustered.cpp:1534) before `process_gi` (:1607). |
| c10-pass | After the depth pre-pass, a compute pass over the screen | VERIFIED | `_pre_opaque_render` (call at :2182) follows the depth pre-pass (:2120); position from depth in `reconstruct_position` (gi.glsl:159-190). |
| c10-pass | The pass also reads each pixel's normal, as at this point | VERIFIED | `fetch_normal_and_roughness` (gi.glsl:~602-608). |
| c10-cascade | A pixel reads from the finest cascade whose grid contains it. | VERIFIED | First cascade with `cascade_pos` in [0, probe_size) (gi.glsl:316-327). |
| c10-cascade | Near that grid's edge, the pixel blends toward the next | VERIFIED | `smoothstep(blend_from, blend_to, len)` mixing diffuse and specular with cascade + 1 (gi.glsl:336-362). |
| c10-cascade | The blend begins five and a half probe spacings from the camera, | VERIFIED | `blend_from = (17 - 1)/2 - 2.5 = 5.5`, `blend_to = 7.5`; `len` is the largest component of the camera-relative position in that cascade's probe units (gi.glsl:337-352). |
| c10-cascade | The readout follows a test point as it moves out | VERIFIED | Lab demo. |
| c10-probes | Before reading the probes, the pass pushes the point | VERIFIED | `cascade_pos += cam_normal * sdfgi.normal_bias` (gi.glsl:196); bias 1.1 cells = (1.1 / 128) x 16 = 0.1375 probe units (gi.cpp:1858; environment.h:161). |
| c10-probes | Then it weights the eight probes at the corners | VERIFIED | gi.glsl:208-232. |
| c10-probes | Each weight grows as the probe gets closer and as | VERIFIED | `trilinear * max(0.005, dot(normal, dir to probe))` (gi.glsl:228). |
| c10-probes | So a probe behind the wall counts for almost nothing, | VERIFIED | max(0.005, ...) against up to 1 is 200 times; relative to a probe straight in front (the trilinear factor also differs). |
| c10-probes | As the point slides along the wall, the weights move | VERIFIED | Lab demo. |
| c10-specular | A reflection looks up the radiance map along the mirror | VERIFIED | `specular_posf` from `cam_specular_normal` (the reflection) at layer `+ max_cascades` when roughness < 0.99 (gi.glsl:211, 264-266). |
| c10-specular | The rougher the surface, the more the lookup blends | VERIFIED | `mix(specular, irradiance, (roughness - 0.2) * 1.25)` above 0.2 (gi.glsl:267-269). |
| c10-specular | Below a roughness of a fifth, Godot also marches | VERIFIED | `if (roughness < 0.2)` reflection march (gi.glsl:401). |
| c10-specular | The lab's room is rougher than that, so that march never | VERIFIED | Room roughness: floor/walls/panel 1.0, sphere 0.5, boxes 0.6, teapot 0.35 (render_lab/scenes/study-room.lam:35-62); the lab port omits the march (screen.rs:93). The sphere slider in the demo goes 1.0 to 0.25. |
| c10-compose | Without SDFGI, the opaque pass fills the shadows with | VERIFIED | `ambient_light` from the sky; `ambient_light = mix(ambient_light, buffer_ambient.rgb, buffer_ambient.a)` only with the GI buffer (scene_forward_clustered.glsl:2023-2026). |
| c10-compose | With SDFGI, it takes the screen pass's ambient light instead. | SIMPLIFICATION | It mixes by the buffer's alpha: alpha 0 (no cascade holds the pixel) keeps the sky, and the last cascade's blend fades to the sky (gi.glsl:352-357). Inside the cascades, alpha is 1. |
| c10-compose | The opaque pass multiplies that ambient light by the albedo, | VERIFIED | `ambient_light *= albedo.rgb` (scene_forward_clustered.glsl:~2185); reflection added after. Also multiplied by AO and (1 - metallic), which the narration omits. |
| c10-compose | Without SDFGI, only the panel's direct light reaches the room, | VERIFIED | Lab demo: `color += sdfgiMix * (albedo * ambient + reflection)` and no sky ambient in that shading (sdfgi_shading.wgsl:467). |
| c10-compose | With SDFGI, the bounced light comes back | VERIFIED | Lab demo. |
| c10-others | Transparent surfaces skip the screen pass and repeat | VERIFIED | scene_forward_clustered.glsl:1883-1935 calls `sdfgi_process` from scene_forward_gi_inc.glsl:126. |
| c10-others | They never march a reflection ray. | VERIFIED | No march in scene_forward_gi_inc.glsl (`use_specular` always true, probe maps only). |
| c10-others | And SDFGI's energy setting, which scales its light, never | VERIFIED | `sdfgi.energy` is read only at gi.glsl:478-479; no match in scene_forward_gi_inc.glsl or scene_forward_clustered.glsl. |
| c10-others | Volumetric fog reads the probes' average light, one colour | VERIFIED | `texelFetch(sdfgi_ambient_texture ...)`, the SH l0 term (volumetric_fog_process.glsl:733-800; integrate.glsl:~345 stores it). |
| c10-others | The code shows that the fog never applies the y scale, | VERIFIED | `y_mult` appears only in the struct (volumetric_fog_process.glsl:132); `world_pos` is used unscaled (:~708, 746). 1.5 is the default y_mult. |
| c10-others | So the fog samples the probes at the wrong height. | VERIFIED | The cascade positions are y-stretched (gi.cpp:1208), so a real height is read as dy where 1.5 dy is expected. |
| c11-cascades | The cascades follow the camera. | VERIFIED | `update_cascades`/`sdfgi_update` drag the cascades (gi.cpp:1199-1245). |
| c11-cascades | Each jump leaves a dirty slab to rebuild. | VERIFIED | `dirty_regions` (gi.cpp:1210-1230). |
| c11-voxels | The slab is voxelized, and a jump flood turns it | VERIFIED | `render_region` (gi.cpp:2061+). |
| c11-voxels | Then the occlusion step measures how well each probe | VERIFIED | Order: jump flood, occlusion (gi.cpp:2342-2370), then STORE. |
| c11-light | Then the solid cells are lit, a quarter of them each | VERIFIED | See c7-cells. |
| c11-light | Each cell also adds the light of last frame's probes, | VERIFIED | See c7-bounce (default bounce 0.5 above the 0.001 threshold). |
| c11-probes | Each probe traces eight rays a frame, and sums | VERIFIED | See c8. |
| c11-probes | Then each probe stores its light in two small maps, | VERIFIED | `store_probes` after `update_probes` (render_forward_clustered.cpp:1534 and 2132, 2182 order). |
| c11-screen | Every pixel finds the finest cascade that holds it, | VERIFIED | See c10. |
| c11-screen | That light from the probes replaces the sky's ambient light. | SIMPLIFICATION | By alpha (see c10-compose). |
| c11-screen | That is SDFGI, as Godot's source builds it. | VERIFIED | Summary; omits the specular march and the parts noted above. |

## Non-VERIFIED items

SIMPLIFICATION (true in spirit; nothing needs correcting unless precision is wanted):

1. c6-shells, "A solid cell has zero visibility, so the cells behind it grow darker": zero during the propagation only; a later pass sets solid cells to the average of their free neighbours on the probe side (sdfgi_preprocess.glsl:665-720).
2. c7-shadow, "Each step is as long as the distance the field stores there": the step is the stored distance minus one cell (`* 255 - 1`, sdfgi_direct_light.glsl:408). The same applies to the probe march (c8-march).
3. c7-shadow, "If the ray leaves the coarsest cascade without a hit, the cell is lit": true, but the ray is also lit when it reaches the light's distance (direct_light.glsl:434-436).
4. c8-miss, "A ray that leaves every cascade reads the sky": only for a sky background; a colour background reads that colour, otherwise black (sdfgi_integrate.glsl:271-290).
5. c9-texel, "returns a blend of two neighbouring texels": per axis; a 2D bilinear fetch blends up to four texels.
6. c9-scroll, "It starts from the light of the coarser cascade's probes": not in the last cascade, which keeps what is there (sdfgi_integrate.glsl:~573).
7. c10-compose and c11-screen, "takes the screen pass's ambient light instead / replaces the sky's ambient": a mix by alpha (scene_forward_clustered.glsl:2023-2026); the sky stays where no cascade holds the pixel or the last cascade fades out. The opaque pass also multiplies by AO and (1 - metallic).

Notes (not defects):

- c8-miss, "A later commit fixes the mismatch": d9ea5c261e does change the define, but its title describes error checks, and it is not in the checkout's history (it is on origin/master only).
- c9-gain: the "27 percent too bright" assumes Godot's own sky ambient (1.0 L) as the reference; the source does not say the 4/pi is a mistake.

No WRONG or UNCERTAIN items.
