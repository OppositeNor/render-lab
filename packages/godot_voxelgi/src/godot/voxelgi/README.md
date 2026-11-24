# Godot Engine: VoxelGI

How Godot's Forward+ renderer computes **VoxelGI**, its baked voxel global illumination, read from the Godot source
(4.7.2-stable, the commit in [`source-index.json`](../../../source-index.json)) and followed in the lab at the engine's
default subdivision: one VoxelGI around the study room, 6.4 x 3.2 x 6.4 m at subdivision 128, so an octree of
128 x 64 x 128 cells of 5 cm in 8 levels. The room is baked by [`bake.rs`](bake.rs) and [`sdf.rs`](sdf.rs) (the
voxelizer and its distance transform) and packed by [`storage.rs`](storage.rs), once, as the editor bakes it
([`world.rs`](world.rs): the room, and the room without the teapot for when its GI mode is Dynamic). Everything Godot
does at run time runs on the GPU, scheduled each frame by one plan as `GI::VoxelGIInstance::update` schedules it
([`update.rs`](update.rs), [`shaders/voxelgi_passes.wgsl`](../../../shaders/voxelgi_passes.wgsl)): the bake's distance
field uploaded into an R8 3D texture; the shot's lights uploaded and the light passes run (the direct light with its
shadow march and the area light's LTC integral, the bounce, one mip dispatch per level, and the writes into an RGBA8 3D
texture, over the occupied cells only) only when Godot's light cache or the VoxelGI's settings change; and, every frame
a dynamic object is in the box, the texture rewritten from the float buffer and the object drawn in six views, lit,
shrunk, and plotted into it ([`dynamic_gpu.rs`](dynamic_gpu.rs)). [`light.rs`](light.rs) and [`dynamic.rs`](dynamic.rs)
are their CPU mirrors, each following the engine's shader or C++ function line by line, which the demonstrations draw
and the selftest compares with the GPU. The room is shaded by
[`shaders/voxelgi_shading.wgsl`](../../../shaders/voxelgi_shading.wgsl) in Forward+'s order: a depth pre-pass, the scene
pass, a compute pass running the WGSL port of the screen pass (`voxel_gi_compute` and its two cone traces) once per
pixel (or per 2 x 2 block at Godot's half resolution), then the opaque pass's mix; [`cone.rs`](cone.rs) is its CPU
mirror. The selftest compares the texture the GPU writes (with the panel alone, with the extra light added, and with
the dynamic teapot plotted), its sampler, and the ports of the light pass, the bounce's cone, and the screen pass with
the CPU mirrors. [`cost.rs`](cost.rs) counts what an update and a frame cost (the cost demonstration).

## What the demonstrations show

| Key | Demonstration | Godot code it follows |
|---|---|---|
| 1 | The room with VoxelGI | `RenderForwardClustered::_render_scene`, VoxelGI and VoxelGIData's defaults |
| 2 | The box becomes cells | `Voxelizer::begin_bake` |
| 3 | A triangle goes down the octree; a leaf's 4 x 4 columns | `Voxelizer::_plot_face`, `Geometry3D::triangle_box_overlap` |
| 4 | A cell's records, the sorted array | `_fixup_plot`, `_sort`, `get_voxel_gi_octree_cells`, `get_voxel_gi_data_cells` |
| 5 | The distance field | `Voxelizer::get_sdf_3d_image`, `edt` |
| 6 | The light pass | voxel_gi.glsl `MODE_COMPUTE_LIGHT`: `compute_light_at_pos`, `raymarch`, `compute_area_light` |
| 7 | The second bounce | voxel_gi.glsl `MODE_SECOND_BOUNCE` |
| 8 | The mips and the texture | voxel_gi.glsl `MODE_UPDATE_MIPMAPS`, `MODE_WRITE_TEXTURE` |
| 9 | A dynamic object (the scene's GI plotted on the GPU; the overlays from the CPU mirror) | `GI::VoxelGIInstance::update`'s dynamic path, voxel_gi.glsl `MODE_DYNAMIC_*` |
| 10 | A pixel's cones | gi.glsl `voxel_gi_compute`, `voxel_cone_trace`, `voxel_cone_trace_45_degrees` |
| 11 | What it costs | `GI::VoxelGIInstance::update`'s gate and per-level dispatches; `process_gi` at half resolution; the quality switch |
| 12 | The opaque pass | scene_forward_clustered.glsl's use of the GI buffers |

Each demonstration has a **readout** in the struct panel's place: the record it shows, word by word with its decoded
values, or the values of its step, computed by the CPU model. Its **step parameters** (in the dock) walk the octree's
levels, a triangle's descent, a leaf's columns, the distance transform's passes, a light's steps and its shadow march,
the cones and their samples, the dynamic object's views and shrink maps. A **click** on a surface picks the leaf cell
under it (`cellX`, `cellY`, `cellZ`; `scene`, script/logic.lam), and a click on a slice inset picks the cell at that point (`pickSliceCell`, script/logic.lam, with the `marchCone` action its story runs).
Cell points hidden behind the room's objects are dimmed, since the overlay has no depth test of its own.

## Findings

Read from the code (and, where marked, measured on the lab's port):

- **The edge fade cancels for one VoxelGI.** `voxel_gi_compute` weights its light by a fade that is 1 at the box's
  centre and 0 at its faces, but `process_gi` divides the sums by the sum of the fades, so with one box the fade
  cancels; only a pixel exactly on a face (fade 0) gets nothing (gi.glsl, measured by the selftest).
- **A plane on a cell boundary can drop out of the bake.** In the lab's f32 port of `triangle_box_overlap`, the room's
  back and left walls, lying exactly on a cell boundary, were rejected by both cells they touch and baked into no leaf;
  the lab offsets its box by a quarter cell. Whether a given Godot build drops them depends on the same rounding.
- **Parents keep no colour.** `_fixup_plot` zeroes a parent's albedo, emission, and normal; the coarse levels' light
  comes only from the GPU's mip pass.
- **Area lights are not shadowed** in the light passes (`compute_area_light` has no march), and their LTC integral is
  used without the division by 2 pi.
- **The bounce's cones have no bounds test** (it is commented out); in the lab's room, a floor cell's cone runs eleven
  steps and ends two thirds opaque (measured).
- **The GPU distance-field pass is compiled out** (`#if 0` in `voxel_gi_allocate_data`); the bake's CPU field is used.
- **`normal.length() > 0.5` is always true** in gi.glsl: GLSL's `.length()` method on a vector is its component count.
- **The debug lighting view** misses an assignment (`color_interp.xyz *params.dynamic_range;`), so it shows the light
  divided by the dynamic range.
- **Bias defaults differ:** VoxelGIData's is 1.5, the rendering server's 1.4 (`gi.h`); a resource that never calls
  `set_bias` runs at 1.4 (read, not tested).
- **A sky-only directional light** is skipped with `light_count--` inside the upload loop, leaving a stale slot that the
  shader still evaluates and dropping the last light; the **dynamic pass's light count** is not clamped to the buffer's
  32; a directional light with bake mode Static or Dynamic whose origin is inside the box is probably uploaded twice
  (paired and as a directional light; the BVH's zero-size overlap test not read).
- **Unused push constants:** `aniso_strength`, `emission_scale`, `stack_size` are never read.
- **`resolve_gi` with 8× MSAA keeps the wrong half.** It finds, in each half of the eight samples, the sample whose
  depth the fewest samples share, then `mix(best_index0, best_index1, min_f0 < min_f1)` picks the second half's when
  the first half's count is the smaller, the opposite of "the rarest" (resolve.glsl; read, the lab ports it as written).
- **The material pass's normal is not decoded as it was encoded:** it is written as `encode24(normal) * 0.5 + 0.5`
  and read back by `MODE_DYNAMIC_LIGHTING` as `xyz * 2 - 1`; `encode24` only scales the normal along itself, so the
  normalisation that follows restores its direction (scene_forward_clustered.glsl, voxel_gi.glsl).

## What the lab simplifies

- The room's materials are flat colours (no textures, no UVs), so the bake's material tables are one colour each.
- The box is offset by a quarter cell (above).
- The ceiling panel stands for an AreaLight3D of its size (range 10 m, attenuation 2), its mesh left out of the bake;
  the light demonstration adds an OmniLight3D with shadows.
- The depth pre-pass writes depth alone; the scene pass after it writes Godot's normal-roughness buffer (RGBA8, the
  roughness the object's, mirrored for a dynamic object) beside the lab's direct light and a full-precision normal for
  the composite; the GI pass reads them on the MSAA sample `resolve_gi` picks (Godot copies that sample out first), and
  the opaque pass's mix with the environment is a compute pass over the samples. There is no tag buffer
  (one VoxelGI). With MSAA, Godot's opaque pass first moves the GI fetch to the neighbour whose normal is closest
  (`gi_upscale_for_msaa`); the lab's compose reads the buffers bilinearly at the pixel's centre without it.
- The environment's ambient and reflection are the lab's sky (its lighting level; the reflection averaged over nine
  directions spread by roughness), not Godot's radiance map; the reflection is weighed by the lab's environment BRDF fit.
- The dynamic object is drawn by the lab's program raster pass (its mesh, a depth test, five colour targets), not
  Godot's material pipeline: its material is the object's flat colour, roughness, and emission. Godot's twelve dynamic
  maps are the twelve mips of one light image and one depth image here, each the size of its map; each map's push
  constant goes up as a record in a buffer (the lab's dispatch parameters hold sixteen words). The CPU mirror
  (dynamic.rs) casts a ray through each texel's column instead of drawing, and keeps its maps unflipped; the
  demonstration's points and insets are drawn from it.
- The normals in the normal-roughness buffer and the dynamic maps are scaled to touch the unit cube without Godot's
  best-fit table (`encode24`'s `best_fit_normal_texture`), which changes only their 8-bit rounding.
- Both bakes (the room, and the room without the teapot) sit in the shading's data buffer from the start; the plan
  uploads the shown one's distance field and lights its cells when the demonstration switches, as Godot does when a
  VoxelGI's data changes.
- The extra omni light's direct light in the scene pass is the lab's (Godot's omni falloff and the cosine, shadowed by
  the lab's ray tracing); in VoxelGI's light passes it is Godot's.
- The lab's direct light is its own (the panel's analytic integral with ray-traced shadows), not Godot's forward lights.

## Files

| File | What it is |
|---|---|
| `mod.rs` | The `Investigation`, its scene shading, the scene click |
| `bake.rs` | The voxelizer: bounds, the triangle-box test, the octree descent, leaf sampling, sort, fixup |
| `sdf.rs` | The distance field |
| `storage.rs` | The records, RGB9E5, the light texture and its sampling |
| `light.rs` | The light, mip, write, and bounce passes (the CPU mirror) |
| `update.rs` | The program's plan: each frame's uploads, light passes, and dynamic objects, in Godot's order |
| `dynamic_gpu.rs` | The dynamic objects' views and their maps' records, for the plan |
| `cost.rs` | What an update and a frame cost, counted |
| `dynamic.rs` | The dynamic-object path |
| `cone.rs` | The screen pass (the WGSL port's CPU mirror) |
| `world.rs` | The lab's VoxelGI, its lights, its variants, the shading data |
| `demos.rs`, `shot.rs`, `controls.rs`, `values.rs`, `actions.rs` | The demonstrations, their values, the dock, the actions |
| `selftest.rs` | The checks |
| `source_index.py` | Writes `source-index.json` from the checkout |

## Verify

```sh
cargo run --release -- --package godot_voxelgi selftest --report /tmp/voxelgi.json
python3 packages/godot_voxelgi/src/godot/voxelgi/source_index.py
python3 tools/video/build.py --package godot_voxelgi --snapshots-only
python3 tools/video/build.py --package godot_voxelgi
```
