# Godot VoxelGI: story outline

From `research.md`. Eight chapters, about thirteen minutes. Each chapter is one structure or one pass, in the order
the data is made and used. Index keys (in backticks) are the source-index entries the storyboard's callouts and
`sources:` lines name; `source_index.py` writes them from the checkout.

The lab's VoxelGI (decided in the guide): a node at the room's centre, 6.4 x 3.2 x 6.4 m, subdiv 128, so an octree of
128 x 64 x 128 cells of 5 cm in 8 levels; the ceiling panel is an area light; the teapot is the bake's worked subject
and the dynamic object; a pixel on the sphere is the read side's worked pixel.

## The architecture map (diagram `frame`, reused)

```
BAKE (editor, once):  meshes ─▶ OCTREE (sparse cells) ─▶ CELLS (albedo, emission, normal) ─▶ SDF (distance field)
                                                   │ upload once
LIGHT (on change):    LIGHTS ─▶ DIRECT (per leaf cell, SDF shadows) ─▶ BOUNCE (6 cones in TEX) ─▶ MIPS ─▶ TEX (RGBA8 3D)
                                                    ▲─────────────────── reads TEX ───────────┘
DYNAMIC (per frame):  moving meshes ─▶ 6 VIEWS ─▶ LIT ─▶ SHRINK ─▶ PLOT into TEX
READ (per frame):     PRE-PASS (normal, roughness, TAG) ─▶ CONE TRACE (gi.glsl) ─▶ BUFFERS ─▶ OPAQUE PASS mix by alpha
                                                                             └▶ (fog, transparent pass)
```

Colours (one encoding per kind of data, the lab's palette roles): geometry and cells in the cell colour (`AABB`),
distances in grey and the field's outline colour (`OUTSIDE`), occlusion in dark red, light (radiance stored in cells,
the texture's RGB) in the incident colour (`INCIDENT`) or the cell's own light colour, coverage (alpha) as a blue
ramp, cones and samples in the view colour (`VIEW`), the worked element in `SELECTED`.

## 1 | Where VoxelGI runs | 110 s

Bridge: the opening. Problem: "how can the renderer light a surface with the light that bounced off the rest of the
room, without tracing rays from every pixel through the scene?"

| Beat | Claim | Source | On screen |
|---|---|---|---|
| c1-room | Off and on, same camera: the bounced light VoxelGI adds (red wall tints the sphere, the floor's colour reaches the walls) | (lab) | `voxelgiMix` 0 → 1 on "adds", camera floating |
| | It is a node, a box placed in the scene, Forward+ only | `voxelgi-doc`, `mobile-stub`, `gles3-stub` | the box drawn (`showBox` 0 → 1) |
| c1-frame | The map: baked once, lit on change, read every frame | `bake-entry`, `relight-gate`, `process-gi` | Mermaid `frame`, all lit in turn |
| c1-gate | It runs when any VoxelGI is in view; the depth pre-pass writes normal, roughness, and a tag | `voxelgi-gate`, `depth-pass-mode` | readout rows lit (`focusRow`) |
| c1-settings | Node: size 20 m, subdiv 128; data: dynamic range 2, energy 1, bias 1.5, propagation 0.5, two bounces, interior off; project: quality Low (4 cones) | `node-defaults`, `data-properties`, `quality-setting` | readout of settings, rows lit one by one |
| c1-lab | The lab's box: 6.4 x 3.2 x 6.4, subdiv 128, cell 5 cm | `begin-bake` | box over the room, `cellGrid` fades in |
| c1-sources | Where to read | | sources card |

Mechanism questions: problem (bounced light cheaply), data (named on the map), result (the comparison), trade-off
(baked: static geometry, memory) said in the recap.

## 2 | Baking the octree | 120 s

Bridge: "Everything VoxelGI knows about the room comes from one bake in the editor; this chapter follows it."

| Beat | Claim | Source | On screen |
|---|---|---|---|
| c2-collect | Bake collects visible GeometryInstance3D with GI mode Static inside the box; only triangle surfaces | `find-meshes`, `plot-mesh` | objects highlighted in turn (`bakeObject` sweep) |
| c2-cube | The box becomes a cube of its longest side; the longest axis gets 2^subdiv cells, shorter axes halve while they still fit: 128 x 64 x 128 | `begin-bake` | `boundsStep` 0 → 2: box, cube, cell counts |
| c2-root | One triangle of the teapot enters the root cell and is pushed down: a child is made only where the triangle overlaps it | `plot-face-recurse`, `tri-box` | `octreeLevel` 0 → 7 on the worked triangle, the child boxes it creates (worked example) |
| c2-sat | The overlap test is a separating-axis test: 9 edge-axis tests, the box's 3 axes, the triangle's plane | `tri-box` | `satStep` 0 → 3 on one child |
| c2-many | Every triangle of every object does this; the cells are shared | `plot-face-recurse` | all cells at `octreeLevel` 3 → 7, counts per level in the readout (lab-measured) |
| c2-leaf | At a leaf (5 cm) the triangle is sampled on a 4 x 4 grid of columns along its dominant axis; each hit reads albedo, emission, normal; alpha = the fraction of columns hit | `leaf-scan` | `leafColumns` 0 → 16 on the worked leaf |
| c2-material | Albedo is the material colour converted to linear; emission x energy x exposure | `material-cache` | readout rows |
| c2-recap/sources | | | |

Sweep relation: "the closer to the leaves, the more cells": `octreeLevel` sweep, the count row grows ~4x per level.

## 3 | What the bake stores | 120 s

Bridge: "The bake leaves a tree of cells; before it is uploaded, the cells are finished, sorted, and packed."

| Beat | Claim | Source | On screen |
|---|---|---|---|
| c3-fixup | Leaves: sums divided by alpha (an average over the samples), normal normalised or zeroed if the samples disagree (<0.01), alpha set to 1 | `fixup-leaf` | worked leaf readout: before/after (`fixupStep`) |
| c3-parents | Parents keep no colour: albedo, emission, normal set to zero; alpha = sum of children's alpha / 8 | `fixup-parent` | `octreeLevel` 6 → 4: parent cells drawn by alpha (blue ramp), colour black |
| c3-sort | Cells sorted by level, then x, y, z: each level contiguous, root first | `sort-cells`, `level-counts` | a bar of the cell array, levels coloured, `sortStep` |
| c3-bytes | A cell is two records: 32 bytes of children (8 indices, 0xFFFFFFFF = none) and 16 bytes of data: position 11/10/11 bits, albedo RGBA8, emission RGB9E5, normal snorm8 | `export-octree`, `export-data`, `rgbe9995` | readout rows of the worked cell, `byteRow` highlight per field, decode beside |
| c3-sdf | The distance field: an exact Euclidean distance transform on the leaf grid, three 1D passes (z, y, x), one byte per cell: 0 solid, else min(d, 254) + 1 | `sdf-edt`, `edt-1d`, `sdf-encode` | slice inset, `edtPass` 0 → 3; worked empty cell's byte |
| c3-sdf-gpu | A GPU version of the field exists but is compiled out; the CPU one is uploaded | `sdf-gpu-disabled` | callout |
| c3-upload | Upload: octree buffer, data buffer, the SDF as a 3D R8 texture; per instance an RGBA8 3D texture with one mip per level and a float light buffer | `allocate-data`, `instance-texture`, `write-buffer` | the storage views side by side (`storageView` 0 → 4), each titled with name and format |
| c3-memory | The lab's numbers: cells, bytes, texture size (computed) | (lab) | readout |
| c3-recap/sources | | | |

## 4 | Lighting the cells | 120 s

Bridge: "The cells now hold what surfaces are; the light passes give them what light they receive."

| Beat | Claim | Source | On screen |
|---|---|---|---|
| c4-when | The passes run only when the data or a light changes (a cache of every light's parameters), not every frame | `relight-gate`, `light-cache` | readout row; light moved (`lightX` sweep) relights, camera move does not |
| c4-lights | At most 32 lights; directional, omni, spot, area; energy x indirect energy | `max-lights`, `light-upload` | readout |
| c4-direct | Per leaf cell (64 per group): the light vector, attenuation, back-face reject, cosine with the cell's normal, times albedo; plus emission | `compute-light`, `light-vector`, `omni-atten`, `light-at-pos` | worked cell with `lightStep` 0 → 4: vector, attenuation, cosine, result |
| c4-shadow | Shadows march the distance field from the cell toward the light, stepping by the field's distance (a multiple of one cell along the dominant axis); a solid sample stops it | `raymarch`, `normal-advance` | `extraLight` on, `marchSteps` 0 → N spheres on the shadow ray |
| c4-area | The ceiling panel is an area light: its LTC integral, no shadow march at all | `area-light`, `ltc` | panel light only: cells under the sphere still lit (`cellLight`), callout |
| c4-all | Every leaf cell's light | | `cellLight` on, all cells coloured by light, `lightPhase` reveal |
| c4-write | The write pass stores light / dynamic range in RGBA8 (so up to 2x white survives), alpha = coverage | `write-texture`, `data-properties` | `dynamicRange` sweep 1 → 4: clipped cells turn white, decode row |
| c4-recap/sources | | | |

## 5 | The second bounce and the mips | 120 s

Bridge: "One pass of light reaches only what a light sees; the second pass lets the cells light each other."

| Beat | Claim | Source | On screen |
|---|---|---|---|
| c5-mips | Before the bounce can trace, the coarser levels are filled: parent = sum of children / mix(8, count, propagation) | `update-mipmaps` | `mipLevel` 0 → 7 view of the texture, a worked parent with its children |
| c5-prop | Propagation 0 divides by 8 (empty children count as black), 1 by the occupied count | `update-mipmaps` | `propagation` 0 → 1 sweep, parent value row changes |
| c5-texture | The texture's mips are the octree's levels: mip 0 the leaves, mip 7 the root | `mip-levels` | mip slices as insets |
| c5-cones | Each leaf cell with a normal traces six cones in its tangent frame (one along the normal, weight 0.25; five at 60 degrees, 0.15 each), half-angle tan 0.577 | `second-bounce`, `bounce-cones` | worked cell, `conesShown` 0 → 6 |
| c5-march | Each cone steps from 1.5 cells, its diameter 2 tan x dist, sampling the mip log2(diameter), front to back until alpha 0.95 | `bounce-march` | `coneSteps` 0 → N spheres growing, readout of alpha accumulating |
| c5-restore | The result x weight x albedo x dynamic range is added to the cell's direct light; then the mips are rebuilt and the texture written again | `bounce-restore`, `pass-order` | `bounces` 1 → 2: room's cells brighten in shadow |
| c5-recap/sources | | | |

## 6 | Moving objects | 100 s

Bridge: "The bake knows only what stood still; an object marked Dynamic is voxelized again every frame."

| Beat | Claim | Source | On screen |
|---|---|---|---|
| c6-mode | GI mode Dynamic: not baked; plotted into the same texture every frame the box is visible, moved or not | `gi-mode-flags`, `dynamic-list` | teapot moves (`dynamicX` sweep), its plotted voxels follow |
| c6-views | Six orthographic views of its box, 16x oversampled, rendered with its material: albedo, normal, emission, depth | `dynamic-views`, `dynamic-oversample`, `material-outputs` | `dynamicView` 0 → 5 insets |
| c6-light | Each texel becomes a voxel position from its depth and is lit like a cell (shadows from the baked field only) | `dynamic-lighting` | inset lit, `dynamicStep` |
| c6-shrink | 2 x 2 shrinks: 16x → 8x → 4x → 2x, then 1x and every coarser mip, where it is plotted, blended over the texel | `shrink`, `plot-blend` | `shrinkLevel` 0 → N |
| c6-clear | The texture is cleared and the static light rewritten from the float buffer before each frame's plot; no bounce for dynamic voxels | `dynamic-clear` | readout |
| c6-cost | Per object per frame 6 draws and 6 x (1 + 3 + mips) dispatches; the lab's shading keeps the static bake | `dynamic-oversample` | readout, callout says lab difference |
| c6-recap/sources | | | |

## 7 | Reading it per pixel | 130 s

Bridge: "Static or moving, everything ends in one 3D texture; now the frame reads it, one pixel at a time."

| Beat | Claim | Source | On screen |
|---|---|---|---|
| c7-tag | The pre-pass writes, per pixel, the indices of up to two VoxelGIs its object overlaps (R8G8_UINT, 0xFF none) | `pairing-two`, `tag-write`, `tag-format` | readout of the worked pixel's tag; overlay |
| c7-space | The pixel's position and normal move into the box's cell space; outside → nothing; an edge fade 1 at the centre to 0 at the faces | `voxel-gi-compute`, `edge-blend` | worked pixel on the sphere, `blendView` |
| c7-low | Low quality (the default): four cones at 45 degrees around the normal, weight a quarter each | `cones-low` | `conesShown` 0 → 4 |
| c7-march | Each step doubles the distance and goes up one mip; samples composited front to back until alpha 1 | `cone-trace-45` | `coneSteps` 0 → N, sample spheres, readout alpha |
| c7-high | High: six cones of tan 0.577 (30 degrees), the general march | `cones-high`, `cone-trace` | `quality` 0 → 1 |
| c7-spec | One more cone along the reflection, its half-angle roughness squared x 0.99 x 90 degrees | `spec-cone`, `roughness-square` | `sphereRoughness` sweep, cone widens |
| c7-scale | x dynamic range x energy x exposure; interior forces alpha 1 | `range-restore`, `interior-alpha` | readout |
| c7-quirk | The "valid normal" test is always true (`.length()` is 3) | `normal-length-quirk` | callout |
| c7-recap/sources | | | |

## 8 | Into the final image | 100 s

Bridge: "The cone trace leaves two buffers per pixel; the opaque pass turns them into colour."

| Beat | Claim | Source | On screen |
|---|---|---|---|
| c8-mix | ambient = mix(environment, buffer, alpha); same for reflections | `gi-buffer-mix` | `viewMode` final / ambient / alpha; `interior` toggle |
| c8-after | then ambient occlusion, x albedo, SSIL, SSR, the environment BRDF | `ao-after`, `albedo-mult`, `env-brdf` | readout rows lit |
| c8-others | Transparent surfaces trace per pixel in the forward pass (4 cones, always); fog samples every mip at its point | `forward-gi-transparent`, `fog-inject` | callouts |
| c8-follow | One pixel followed back through every stage, in order | (all) | the worked pixel's chain: cones → samples → mips → cells → lights; camera glides through |
| c8-recap | Three takeaways: data (sparse octree baked once into a dense mip-mapped 3D texture), light (passes on change, cones per pixel), trade (static geometry, 5 cm cells, light leaks, memory) | | full room, mechanisms switched off and on in frame order |
| c8-sources | | | |

## What changes, and what it changes

| Relation | Parameter | Values that must visibly change |
|---|---|---|
| deeper levels, more cells | `octreeLevel` 0..7 | cell count row, cell boxes |
| the more columns hit, the higher alpha | `leafColumns` 0..16 | alpha row 0 → coverage |
| dynamic range sets what clips | `dynamicRange` 1..4 | stored RGB of the brightest cells |
| propagation sets the parents' brightness | `propagation` 0..1 | parent light row, mip slices |
| the farther the cone, the coarser the mip | `coneSteps` | LOD row, sphere radius |
| roughness widens the reflection cone | `sphereRoughness` 0..1 | cone width, reflection row |
| interior removes the sky | `interior` 0/1 | alpha row 1, room's floor colour |

## Worth a detailed demonstration

The triangle's descent (c2-root), the leaf's 4 x 4 columns (c2-leaf), the cell's 48 bytes (c3-bytes), the 1D distance
passes (c3-sdf), the shadow march (c4-shadow), a bounce cone marching through the mips (c5-march), the six views of
the teapot (c6-views), the pixel's cones (c7-march).

## Tools it needs

All in the package: cell overlays (points sized to cells, wire boxes for single cells), slice insets of 3D data (the
SDF, the texture's mips), readouts of a cell's bytes, cone and sample-sphere drawing, the six-view insets of the
dynamic stand-in. From the lab: the dock values, insets (`PictureInset`), labels, readouts, annotations, actions.
