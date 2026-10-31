# Godot Engine: VoxelGI

How Godot's Forward+ renderer computes **VoxelGI**, its baked voxel global illumination, read from the Godot source
(4.7.2-stable, the commit in [`source-index.json`](../../../source-index.json)) and followed in the lab at the engine's
default subdivision: one VoxelGI around the study room, 6.4 x 3.2 x 6.4 m at subdivision 128, so an octree of
128 x 64 x 128 cells of 5 cm in 8 levels. The room is baked by [`bake.rs`](bake.rs) and [`sdf.rs`](sdf.rs) (the
voxelizer and its distance transform), packed by [`storage.rs`](storage.rs), lit by [`light.rs`](light.rs) (the light
pass with its shadow march and the area light's LTC integral, the mip pass, the write pass, and the second bounce), and
its dynamic-object path is followed by [`dynamic.rs`](dynamic.rs), each following the engine's shader or C++ function
line by line; [`world.rs`](world.rs) builds it once. The room is shaded by
[`shaders/voxelgi_shading.wgsl`](../../../shaders/voxelgi_shading.wgsl) in Forward+'s order: a depth pre-pass, the scene
pass, a compute pass running the WGSL port of the screen pass (`voxel_gi_compute` and its two cone traces) once per
pixel on the lit texture of that CPU world, then the opaque pass's mix; [`cone.rs`](cone.rs) is its CPU mirror. [`shaders/voxelgi_passes.wgsl`](../../../shaders/voxelgi_passes.wgsl) ports the light pass and the bounce's
cone for the selftest, which compares them with the CPU model.

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
| 9 | A dynamic object | `GI::VoxelGIInstance::update`'s dynamic path, voxel_gi.glsl `MODE_DYNAMIC_*` |
| 10 | A pixel's cones | gi.glsl `voxel_gi_compute`, `voxel_cone_trace`, `voxel_cone_trace_45_degrees` |
| 11 | The opaque pass | scene_forward_clustered.glsl's use of the GI buffers |

Each demonstration has a **readout** in the struct panel's place: the record it shows, word by word with its decoded
values, or the values of its step, computed by the CPU model. Its **step parameters** (in the dock) walk the octree's
levels, a triangle's descent, a leaf's columns, the distance transform's passes, a light's steps and its shadow march,
the cones and their samples, the dynamic object's views and shrink maps. A **click** on a surface picks the leaf cell
under it (`cellX`, `cellY`, `cellZ`), and a click on a slice inset picks the cell at that point (`pickSliceCell`).
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

## What the lab simplifies

- The room's materials are flat colours (no textures, no UVs), so the bake's material tables are one colour each.
- The box is offset by a quarter cell (above).
- The ceiling panel stands for an AreaLight3D of its size (range 10 m, attenuation 2), its mesh left out of the bake;
  the light demonstration adds an OmniLight3D with shadows.
- The depth pre-pass writes depth alone; the scene pass after it writes the normal (Godot's normal-roughness buffer;
  the roughness is the object's) beside the lab's direct light, the GI pass reads them on each pixel's first MSAA
  sample, and the opaque pass's mix with the environment is a compute pass over the samples. There is no tag buffer
  (one VoxelGI); the RGBA8 3D texture is a buffer sampled trilinearly by hand.
- The environment's ambient and reflection are the lab's sky (its lighting level; the reflection averaged over nine
  directions spread by roughness), not Godot's radiance map; the reflection is weighed by the lab's environment BRDF fit.
- The dynamic object's six views are found by casting a ray through each texel's column (nearest hit), not by
  rasterizing its mesh; its maps are kept unflipped. The dynamic demonstration draws the plotted texels as points; the
  scene's shading keeps the static bake.
- The lab's direct light is its own (the panel's analytic integral with ray-traced shadows), not Godot's forward lights.

## Files

| File | What it is |
|---|---|
| `mod.rs` | The `Investigation`, its scene shading, the scene click |
| `bake.rs` | The voxelizer: bounds, the triangle-box test, the octree descent, leaf sampling, sort, fixup |
| `sdf.rs` | The distance field |
| `storage.rs` | The records, RGB9E5, the light texture and its sampling |
| `light.rs` | The light, mip, write, and bounce passes |
| `dynamic.rs` | The dynamic-object path |
| `cone.rs` | The screen pass (the WGSL port's CPU mirror) |
| `world.rs` | The lab's VoxelGI, its lights, its variants, the shading data |
| `demos.rs`, `shot.rs`, `controls.rs`, `values.rs`, `actions.rs` | The demonstrations, their values, the dock, the actions |
| `selftest.rs` | The checks |
| `source_index.py` | Writes `source-index.json` from the checkout |

## Verify

```sh
cargo run --release -- --investigation godot/voxelgi selftest --report /tmp/voxelgi.json
python3 packages/godot_voxelgi/src/godot/voxelgi/source_index.py
python3 tools/video/build.py --investigation godot/voxelgi --snapshots-only
python3 tools/video/build.py --investigation godot/voxelgi
```
