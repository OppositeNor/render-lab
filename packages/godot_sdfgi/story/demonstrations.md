# Demonstrations: the GPU part (chapters 11 to 15)

How each structure the GPU part shows is stored by Godot (research.md section 12, from `SDFGI::create`, gi.cpp:414-1129),
how the lab shows it, and what a viewer can do with it. Every view is of the lab's port of that resource, read back
by the inspector right after the pass that wrote it (the `render-graph` skill), unless it says otherwise. The subject
is the study room's teapot, as in chapters 4 to 10; the worked examples follow its spout's surface cell.

Godot's own debug views (research 18) say what its authors found worth seeing: the voxel light along camera rays,
the probes with their occlusion, and the GI buffer. The views below add the structures it has no view of: the
working images, the jump-flood seeds, the solid-cell list and its counters, the history ring, and the dispatches.

## Storage, mirrored

Separate resources are separate views, each titled with its name, format, and size; one layout, one outline: the
picked cell is outlined in every view that holds it.

| Structure | Godot's resources (one view each) | Layout and decoding | Chapter |
|---|---|---|---|
| Working images of a cascade update | `render_albedo` R16_UINT, `render_geom_facing` R32_UINT, `render_emission` R32_UINT, `render_emission_aniso` R32_UINT, 128^3, shared by all cascades | slice at a height; albedo decoded from RGB555 (bit 0 solid as a dot), facing as six bits (one colour per face), emission decoded from RGBE8985, aniso as the largest face | 12 |
| Jump-flood seeds | `render_sdf_half[0/1]` 64^3, `render_sdf[0/1]` 128^3, RGBA8_UINT | slice: each cell coloured by the hash of its seed (the lab's `hash` show), w = 0 black; the ping-pong pair side by side, the one written this step lit | 13 |
| Occlusion working set | `render_occlusion[0..7]` R8_UNORM, 128^3 each | eight small views (one per parity class), the picked probe's class lit; ramp black (occluded) to white | 13 |
| Distance field | `sdf_tex` R8_UNORM per cascade | slice, ramp, `(r*255 - 1)` cells as the readout | 13 |
| Packed occlusion | `occlusion_data` R16_UINT 256 x 128 x 512 | the cascade's slab of z; the two x halves side by side; the picked texel's 16 bits as four nibbles each | 13 |
| Solid-cell list | `solid_cell_buffer`, 16 B records | a strip of records (the list view): each record a cell coloured by the group that reserved it; the picked record's four words decoded field by field (position 7/7/7, RGB555, facing, neighbour bits, RGBE light, aniso) | 13, 14 |
| Counters and indirect arguments | `solid_cell_dispatch_buffer_storage`, `…_call`, 16 B | the four words {x, y, z, total_count} as a readout, before and after the copy | 13 |
| Voxel light | `light_data` R32_UINT (read as RGB9E5), `light_aniso_0` RGBA8, `light_aniso_1` RG8 | slice; light decoded from the word; aniso as six bars on the picked cell | 14 |
| SH history and average | `lightprobe_history_tex` R16G16B16A16_SINT 289 x 272 x 30, `lightprobe_average_tex` R32G32B32A32_SINT | one layer (the slot being written lit); a probe is a column, its 16 coefficients the rows; values as signed ramp; the picked probe's column outlined in both | 14 |
| Probe maps | `lightprobe_data` R32_UINT 2312 x 136 x 8 (read as RGB9E5) | a layer zoomed on one probe's 8 x 8 block, border texels marked; irradiance and radiance layers side by side | 14 |
| Screen inputs and outputs | depth, `normal_roughness` RGBA8, `ambient`, `reflection` RGBA16F | split view (`Investigation::panes`): the buffer on the left, the lit frame on the right, same camera | 15 |
| Resources by size | every resource above | the memory bars (generic, from the frame graph's resource sizes) | 11 |

## Dispatches (the lab's generic dispatch view)

A pass's dispatch drawn over its domain: the grid it covers (cells, list entries, texels, pixels) with its workgroups
outlined, live lanes coloured and idle lanes grey (past `total_count`, past the image's edge, a skipped occlusion
group), and its counts in a readout (groups, threads, live). One group or one lane can be picked.

| Dispatch | Domain | What it shows | Chapter |
|---|---|---|---|
| `sdfgiDynamicLight` | the solid-cell list | groups of 64 over the records; with `framesToUpdateLights` 4, lanes `t*4 + f%4` lit, groups past total/4 idle | 14 |
| `sdfgiJumpFloodShared` | a slice of the 64^3 half grid | one 8^3 group's lattice at `jfStep` (points `jfStep` apart), its 10^3 shared patch outlined | 13 |
| `sdfgiOcclusion` | a slice | one parity class's 16^3 cubes tiling the slice | 13 |
| `sdfgiStore` | a slice, then the list | one 4^3 group's solid cells, their local slots, the range they reserve | 13 |
| `sdfgiProbeProcess` | the 289 x 17 probe plane | 37 x 3 groups of 8 x 8, the live 4,913 threads | 14 |
| `sdfgiProbeStore` | the 1734 x 102 texel plane | 8 x 8 groups over 6 x 6 tiles, not aligned | 14 |
| `sdfgiGather` | the screen | 8 x 8 tiles over the frame | 15 |

## Interactions

Each sets values a story can set too (the dock's values), so a story shows the same pick.

| View | Click / control | What it shows everywhere at once |
|---|---|---|
| any slice view | click a cell | the cell outlined in every slice view and in 3D; its words decoded in the readout; if it is solid, its record in the list view and its lane in the light dispatch, with a line from each to the cell |
| list view | click a record | the cell in 3D and in the slices; the group that reserved the range |
| dispatch view | click a group / a lane | its cells (or texels, pixels) in the domain and in 3D; the lanes view of that group (trip counts of its loop) |
| probe plane | click a probe | the probe in 3D, its history column, its map block |
| map block | click a texel | its direction from the probe in 3D; the mirror texel it writes |
| screen pane | click a pixel | its tile and thread, its eight probes in 3D |
| dock | `cascades`, `jfStep`, `rayCount`, `framesToUpdateLights`, `lightFrame`, `halfResGI`, `raceOrder`, `facingAtomic`, `sliverWidth`, `storeOrderSeed`, `sliceHeight` | the sweeps of the outline |

## A demonstration for every taught topic

| Topic | Demonstration | Worked example (step values) | Exploration | Sweep |
|---|---|---|---|---|
| `gpu-frame` | the DAG by levels, the writer-to-reader arrows on a resource card | `graphStep` 0-3: two passes, the resource, the arrow, the level | click a resource: its writers and readers lit | toggle profiling: levels split |
| `gpu-atomics` | two triangles over one voxel | `raceStep` 0-4: fragment A reads, B reads, A writes, B writes, one value left; then the OR | `raceOrder`, `facingAtomic` | - |
| `gpu-scan` | STORE's compaction on one group, then all | `storeStep` 0-4: solid lanes, local slots, reserve, copy, the list | `storeOrderSeed`, pick a group | - |
| `gpu-indirect` | the counter buffer and the dispatch it sizes | `indirectStep` 0-2: atomicMax raises x, the copy, the launch | move the camera: a new cascade, a new count | - |
| `gpu-divergence` | the lanes view of one light group | `laneStep` 0-2: the bars grow, early lanes wait, the group ends | pick a group, move the light | the light's position |

## Lab differences shown on screen

- The voxelizer's dummy colour target (research "Where a GPU port in the lab differs" 1), and the lab's per-object
  albedo and emission (7): said when the voxelizer appears.
- RGB9E5 and 4-bit occlusion decoded in the readers (4): said where the light and probe maps are first read.
- Dispatches that Godot runs without barriers run in order in the lab (6): said with c11-levels.
- Where the adapter lacks a requested feature, the fallback the port took (2, 3, 5), shown in the viewer's notice.

## Checks (Phase 3, step 6)

- Pick round trips: a click on a cell picks it, and the record, lane, and texel it maps to pick the same cell
  (selftest).
- Stills of each view (offscreen), looked at.
