# Godot VoxelGI: demonstrations

How each structure the outline explains is stored by the engine (research.md §2, read from the checkout), how the
lab mirrors it, and what a viewer can do with it. One subject per role across the story: the teapot for the bake's
worked cell and the dynamic object, the sphere for the worked pixel.

## Storage, as the engine keeps it

| Resource | Engine | Separate or packed | Addressed by | Engine debug view |
|---|---|---|---|---|
| octree buffer | storage buffer, `CellChildren` 8 x u32, 32 B per cell | separate | cell index (sorted, levels contiguous) | none |
| data buffer | storage buffer, `CellData` 4 x u32, 16 B per cell: position 11/10/11 bits, albedo RGBA8, emission RGB9E5, normal 3 x snorm8 | separate; four fields packed in one record | same cell index | `VOXEL_GI_ALBEDO`, `VOXEL_GI_EMISSION` (one cube per leaf) |
| SDF texture | 3D `R8_UNORM`, octree size, 1 mip | separate | texel = leaf cell | none |
| light texture | 3D `R8G8B8A8_UNORM`, octree size, levels + 1 mips; rgb = light / dynamic range, a = coverage | separate; mips are the octree's levels | texel (x, y, z) of mip `depth - level` | `VOXEL_GI_LIGHTING` (one cube per leaf, the texel ÷ dynamic range) |
| write buffer | storage buffer, one `vec4` float per cell | separate | cell index | none |
| dynamic maps | 2D RGBA16F light + R32F depth per map, 4 oversampled + one per mip; map 0 also RGBA8 albedo, normal, orm | separate | texel of the object's view rectangle | none |
| tag buffer | 2D `R8G8_UINT`, two instance indices per pixel | separate | pixel | none |
| GI buffers | 2D `RGBA16F` ambient and reflection | two separate | pixel | `GI_BUFFER` (ambient + reflection) |

## The lab's views

Each separate resource is its own view, titled with its name and format; a pick is outlined in every view that holds
it.

1. **Cells in the room** (`cellView`): the octree's cells at `octreeLevel` as points sized to their cell, coloured by
   the field the view shows: 0 albedo (data buffer), 1 coverage alpha (blue ramp), 2 normal, 3 emission, 4 light
   (the texture's texel x dynamic range). Mirrors the engine's three debug draws (albedo, emission, lighting) and adds
   the ones the story needs (alpha, normal), each said to be the lab's.
2. **The worked cell's records** (readout): its index, level, position word, then the 32-byte children record and the
   16-byte data record, one row per word with its hex and its decoded value; `byteRow` lights a row. Its texel in the
   light texture and its SDF byte are rows of their own resource, labelled so.
3. **Slices** (insets, each a separate resource): the SDF at a height (grey, 0 solid black), the light texture's mip
   at that height (RGB, alpha as a checker), the write buffer is not sliced (it is per cell, not per texel; its values
   appear as rows). The slice inset is drawn at the mip's real resolution, so mip 3 is 16 x 16 cells.
4. **Cones** (scene): a cone as its axis and the sample spheres of each step (radius = half the diameter in cells),
   each sphere coloured by the sample it read; the readout lists step, distance, diameter, LOD, sample, alpha.
5. **The six views** (insets): the dynamic object's albedo, depth, and lit maps for the view `dynamicView`, at the
   shrink level `shrinkLevel`.
6. **Pixel buffers** (`viewMode`): the final image, the ambient buffer, the reflection buffer, the alpha (coverage).

## Interactions

- **Click a surface** (`Investigation::click`): picks the leaf cell under the point (`cellX`, `cellY`, `cellZ` set to
  its cell coordinates, `cellPicked` 1). The cell is outlined in the scene, its records fill the readout, its texel
  is marked in the slice insets (link lines from the inset's marker to the cell). A story shows the same pick by
  setting the same values.
- **Click a slice inset** (`#[click] pick_slice_cell`): picks the cell under the click at the slice's height and mip.
- **Sliders**: `octreeLevel`, `cellView`, `byteRow`, `edtPass`, `sliceHeight`, `mipLevel`, `dynamicRange`,
  `propagation`, `lightX/Z/Height`, `marchSteps`, `conesShown`, `coneSteps`, `quality`, `sphereRoughness`,
  `pixelU/V` (the worked pixel's place on the sphere), `dynamicX`, `dynamicView`, `shrinkLevel`, `viewMode`, `interior`,
  `voxelgiMix`.
- **Selftest round trip**: a click at the projected centre of a cell picks that cell.

## Subjects

- Bake and records: the teapot's spout cell (irregular normals, partial coverage) and a floor cell (full coverage).
- Light: a floor cell beside the sphere (in the area light's view, under the omni light's shadow when it is on).
- Bounce: a cell of the floor near the red wall (its cones reach the wall).
- Pixel: a point on the sphere facing the red wall.
- Dynamic: the teapot, moved along x.
