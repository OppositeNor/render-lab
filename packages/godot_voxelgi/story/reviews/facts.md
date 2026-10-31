# Fact check: Godot VoxelGI storyboard and source index

Checkout: Godot 4.7.2-stable, commit ed1daf0bf001b61586d9930840f2f1394092c079 (confirmed with `git rev-parse` and `version.py`).
All `source-index.json` entries (78) were opened at their cited line: every cited line exists and holds the named symbol or statement.
Paths below are relative to the checkout. `rrf` = `servers/rendering/renderer_rd`.

## Verified counts (engine-fact sentences, approximate)

| Chapter | Verified plainly | Listed below |
|---|---|---|
| 1 Where VoxelGI runs | about 11 | 6 |
| 2 Baking the octree | about 29 | 2 |
| 3 What the bake stores | about 27 | 0 |
| 4 Lighting the cells | about 20 | 3 |
| 5 Second bounce and mips | about 19 | 1 |
| 6 Moving objects | about 15 | 2 |
| 7 Reading it per pixel | about 21 | 1 |
| 8 Into the final image | about 13 | 3 |

Lab-measured sentences (cell counts, 11 steps, two thirds opaque, megabytes, 42,289 leaves, "zero in this room") are phrased as the lab's own. None is presented as an engine fact, apart from the two number items in the table.

## Not plainly verified

| Beat | Sentence (short) or index key | Class | Evidence (file:line) | Correction |
|---|---|---|---|---|
| c1-gate | "the Mobile and Compatibility renderers leave its functions empty" (callout mobile-stub) | Potentially misleading | Compatibility: `drivers/gles3/rasterizer_scene_gles3.cpp:1255-1269` stubs, true. Mobile: only `pair_voxel_gi_instances` is empty (`forward_mobile/render_forward_mobile.h:583`). `RendererSceneRenderRD` creates and updates VoxelGI instances for Mobile too (`renderer_scene_render_rd.cpp:200`, `.h:236-240`). The only Mobile texture binding is commented out (`render_forward_mobile.cpp:606-626`). | Say Compatibility's functions are stubs. In Mobile nothing pairs or reads VoxelGI, so it has no visible effect, though the shared light passes still run for a VoxelGI in view. |
| c1-gate | "a bias of one and a half cells" as the data's default | Potentially misleading | `scene/3d/voxel_gi.h:52` has `bias = 1.5`, but `VoxelGIData::VoxelGIData()` (`voxel_gi.cpp:261`) never calls `set_bias`. The server default is `gi.h:87` `bias = 1.4`. A resource that never calls the setter, such as one loaded from a file with the default, runs with 1.4. | "One and a half" is the property's displayed default. Say the server starts at 1.4 and the property mirrors 1.5. Alternatively keep 1.5 for the lab's own data and cite `bias-mismatch` in ch1. It is only cited in ch8. |
| c1-gate / c1-map / index `voxelgi-gate` | "its work runs in any frame whose culling finds a VoxelGI in view" | Correct but simplified | The cull adds the probe to `voxel_gi_update_list` (`renderer_scene_cull.cpp:2963-2969`). `render_probes()` runs before `draw_viewports` (`rendering_server_default.cpp:106-109`). | The light passes run at the start of the next frame, from the list built by the previous frame's cull. The per-pixel work (`using_voxelgi`, `render_forward_clustered.cpp:1871`) is same-frame. A one-frame lag, not worth a sentence unless asked. |
| c1-map | "The bake runs in the editor" (callout `bake-entry`, index `bake-entry`) | Potentially misleading | `VoxelGI::bake` is bound unconditionally (`voxel_gi.cpp:570`, not under `TOOLS_ENABLED`). `doc/classes/VoxelGI.xml:9` says "VoxelGI can be baked in an exported project". | Say "The bake is started by the editor's Bake button (or by calling `bake()`), and runs the voxelizer on the CPU". Do not say it is editor-only. |
| c1-parts | "the coarser levels that hold it averaged" | Potentially misleading | `voxel_gi.glsl:541` has `light_accum / mix(8.0, count, propagation)`. At the default 0.5 this is neither a mean nor sum/8. Narrated correctly in c5. | "that hold it summed and scaled down" or "filled from their children". |
| c8-follow | "the mips the mip pass averaged from the leaves" | Potentially misleading | Same as above (`voxel_gi.glsl:541`). | "the mips the mip pass filled from the leaves". |
| c8-follow | "Those leaves hold the panel's direct light and one bounce, gathered by six cones of their own, shadowed through the field" | Potentially misleading | The panel is an area light, so `compute_area_light` (`voxel_gi.glsl:345-399`) never calls `raymarch`. Shadow marching applies only to punctual lights with `has_shadow` (`voxel_gi.glsl:300-330`). c4-area says so itself. | Drop "shadowed through the field" here, or say "shadowed through the field for punctual lights". c4-recap already has this qualifier. |
| c1-room, c1-map | "without tracing rays through the whole scene for every pixel" (callout `voxelgi-doc`) | Correct but simplified | `VoxelGI.xml:7` only says it precomputes emitters and static geometry for real-time indirect light. It does not state the problem framing. Cones are still traced per pixel, through the texture. | Narrator's gloss, not a quote of the doc. Keep it phrased as the lab's framing, or cite only "precompute the effect of static geometry". |
| c1-map | "a screen pass traces cones ... for every pixel" | Correct but simplified | Pixels are tagged by their object's paired VoxelGIs (`render_forward_clustered.cpp:5060`, `scene_forward_clustered.glsl:3034`); untagged pixels get `0xFF` and add nothing (`gi.glsl:656`). | "for every pixel of an object a VoxelGI covers". Transparent surfaces trace separately (c8). |
| c2-collect | "collects every visible mesh whose GI mode is Static ... bounds touch the box" (callout `find-meshes`, index `find-meshes`) | Correct but simplified | `voxel_gi.cpp:327-349`: the GI-mode test applies only to `GeometryInstance3D`. Any visible `Node3D` passes. Besides `MeshInstance3D` the walk also takes `MultiMeshInstance3D` and any node with a `get_meshes` method (`voxel_gi.cpp:354-380`). It starts at the bake's `from_node`, default the VoxelGI's parent (`voxel_gi.cpp:437`). | Index `inspect` says "Only visible GeometryInstance3D nodes ..." and is slightly too narrow. Add "(and multimeshes, and nodes with get_meshes)". The narration is fine. |
| c4-when | "only when the data, one of its light settings, or a light changes" | Correct but simplified | Version bumps only in `allocate_data`, `set_dynamic_range`, `set_propagation`, `set_use_two_bounces` (`gi.cpp:188,256,271,344`). Energy, bias, normal bias and interior do not bump it. They are applied per frame in the UBO (`gi.cpp:3893`). | "light settings" means dynamic range, propagation and bounces. Index `relight-gate` is right. Narration could say "one of its range, propagation or bounce settings". |
| c4-when | index `relight-gate`: "otherwise an update only re-writes the texture" | Potentially misleading | `gi.cpp:2958-3100`: the block (light upload, mips, write) runs only if `p_update_light_instances`, or dynamic objects exist, or `has_dynamic_object_data`. With a static scene and nothing changed, nothing is re-written. | "otherwise nothing runs, unless dynamic objects are present, in which case only the write runs". |
| index `light-cache` | list of compared parameters | Correct but incomplete | `renderer_scene_cull.cpp:3943-3957` also compares `intensity`, indirect energy (`bake_energy`), area size, area normalize flag, area texture, and for directional lights `sky_mode`. | Add these, or say "among others". |
| c4-shadow | "jumps that far, in whole cells along its main axis" | Correct but simplified | `voxel_gi.glsl:180-199`: advance is rounded down to a multiple of `distance_adv = 1/|dot(dir, dominant axis)|`, at least one such step. The field is sampled with a linear sampler (`gi.cpp:2706`), so values are trilinearly interpolated. | Fine. The interpolation is why partial values appear near surfaces. |
| c6-cost | "six draws and seventy-two dispatches per object per frame" | Correct but simplified (engine formula, size-dependent) | Per view: 1 lighting dispatch + (`dynamic_maps.size()` - 1) shrink dispatches. `dynamic_maps` = 4 oversample maps + one per mip (`gi.cpp:2745-2757`, `3300-3367`). 12 for 8 mips, so 6 x 12 = 72 holds only for subdivision 7 (128 cells on the longest axis). | Say "in this room" or "at 128 cells". It is 6 x (4 + mips) in general. |
| c6 index `shrink` | "averages 2 x 2 texels" | Correct but simplified | `voxel_gi.glsl:604-650`: sums non-empty texels and divides by 4 (empty ones count as zero), not by the count. On a mip, light is divided by `mix(8, count, propagation)` and alpha by 8. | "sums the non-empty texels, divided by 4". |
| c7-spec | "a rough one's opens to seventy degrees" | Potentially misleading | `gi.glsl:598-599, 644`: tangent of the half-angle = `tan(roughness^2 x 0.5 pi x 0.99)`. At roughness 0.9 that is 72 degrees as a half-angle, so the full opening is about 144 degrees. At roughness 1.0 the half-angle is 89 degrees. | State "half-angle", the roughness used (0.9), and that the roughness is squared first. Otherwise "seventy degrees" reads as the full cone angle. |
| c8-others | "the editor's lighting view forgets to apply the dynamic range, so it shows the light at half" | Correct but simplified | `voxel_gi_debug.glsl:94` has `color_interp.xyz *params.dynamic_range;` with no assignment, so the stored texel (light / range) shows. "Half" is true only at the default range 2. It is a viewport debug-draw mode, not editor-only (`render_forward_clustered.cpp:2110,2273-2280`). | "darker by the dynamic range (half at the default of two)". |
| c8-recap | "for a few dozen texture reads per pixel" | Needs investigation | Derived, not in the source. Four 45-degree cones (each doubles distance per step, bias 1.5, box diagonal) plus one reflection cone (general march, half-diameter steps, count depends on roughness). The range depends on box size. | Measure it in the lab, or say "a handful of reads per cone". Not a source fact. |


## Checks that passed (so they are not repeated)

- begin-bake halving, cube stretch and cell-size claims (`voxelizer.cpp:666-710`). The box is 128x64x128 cells in the octree while `po2_bounds` is the full cube. The "top half has no cells" claim is right because children are dropped at `axis_cell_size` (`voxelizer.cpp:268`).
- The SAT order: nine edge axes, then the box axes, then the plane (`geometry_3d.h:733-807`).
- The 4x4 columns, 1/16 scaling and alpha as the share of columns (`voxelizer.cpp:115-215`). The fallback sample at weight 1/16 also checks out. A latent bug there (`get_uv_and_normal(..., normal)` writes to `normal`, not `lnormal`) changes nothing visible.
- Material tables of 128 x 128 (`voxelizer.h:106`). The leaf division, the 0.01 normal cutoff, parents zeroed and keeping alpha/8, and sort order level, x, y, z with root first (`voxelizer.h:73-82`).
- The 32+16 byte records and `x | y<<11 | z<<21` (11/10/11 bits). RGB9E5 decode `2^(e-15-9)` (`voxel_gi.glsl:427`). The normal as three snorm bytes. CHILD_EMPTY `0xFFFFFFFF`.
- The EDT order z, y, x and the encoding `min(d,254)+1` (`voxelizer.cpp:868-955`).
- 32 lights, 64 per group, the `light_count--` stale-slot bug, the 2-pass order, and write-only re-blit (`gi.cpp:2971-3160`).
- The omni attenuation formula and the area-light LTC without 2 pi (`area_lights_inc.glsl:322` vs `396`).
- Second-bounce cones: 6 cones, weights .25 + 5 x .15, tan 0.577, start at 1.5 cells, stop at alpha 0.95, restore x albedo x range (`voxel_gi.glsl:470-520`).
- The mip divisor `mix(8, count, propagation)`: with 4 children, divisor 6 at the default 0.5.
- Dynamic path: 16x oversample (`gi.cpp:2745`), six views (`3230`), clear-then-write (`2965`), the plot's two-voxel depth split (`voxel_gi.glsl:696`), and the unclamped light count (`3289`).
- The pre-pass tag (`R8G8_UINT`, 0xFF none, first two per object), 8 instances, edge fade 1 at centre and 0 at faces, and division by the summed fade (`gi.glsl:548,661`).
- Low quality is 4 cones at 45 degrees, quarter each, default 0. High is 6 cones at tan 0.577. The project setting is a push-constant flag. Roughness is squared before the reflection cone.
- Interior forces alpha to 1. The opaque mix toward the buffer by alpha, for ambient and reflection (`scene_forward_clustered.glsl:2023-2024`).
- Transparent surfaces trace four cones in the forward pass (`scene_forward_gi_inc.glsl`, tan 0.98269, stop at 0.95), regardless of quality.
- Fog reads every mip with no cone (`volumetric_fog_process.glsl:710-720`).
- `normal.length() > 0.5` is always true in GLSL (`gi.glsl:620`).

## Generic-claim check

No carried-over claims found. The narration says "no direction in its light" (matches `aniso_strength` unread, `voxel_gi.glsl:97`; texture is RGBA8 colour+alpha) and never claims light leaking, anisotropic voxels, or a 6-cone default. The only generality-flavoured spots are the c1 problem statement ("without tracing rays through the whole scene") and the c7-high "cost more and blur less", which is derived from the half-angle difference (30 vs 45 degrees) and is not a source quote.
