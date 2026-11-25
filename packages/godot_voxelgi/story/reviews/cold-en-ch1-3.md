# Cold listen, English chapters 1-3

Kinds: 1 lost a reference, 2 undefined or stale term, 3 re-parse, 4 fact without reason, 5 two or more new ideas, 6 lost thread or name before what it names.

## Chapter 1 flags

| Beat | # | First five words | Kinds | What I lost |
|---|---|---|---|---|
| c1-room | 9 | Every pixel reads their light | 1, 5 | "their" (the cells'), and cones plus read-back arrive together |
| c1-map-bake | 2 | It turns the meshes inside | 2 | "the VoxelGI's box" was never introduced |
| c1-map-sdf | 2 | Later, that field helps find | 4 | "which cells are in shadow" - shadow from what, and why it needs a field |
| c1-map-light | 2 | The light passes run again | 3 | a list of "or" conditions, then a conclusion |
| c1-map-dynamic | 1 | Moving objects are left out | 1 | "that texture" is from two beats back; "drawn into" it, how |
| c1-map-read | 1 | Every frame, a compute pass | 1, 5 | "that texture" again; screen pass, cones, box all at once |
| c1-gate | 3 | The Mobile renderer never | 3 | "links an object to a VoxelGI" - unclear what it means |
| c1-gate | 6 | A VoxelGI is a node | 6 | heard "VoxelGI's box" and "VoxelGI in view" before being told what it is |
| c1-gate | 9 | Besides the cells, one project | 3, 5 | cell count, project setting, cone count, default, finer option in one sentence |

What I understood: VoxelGI bakes the room into cells ahead of time, lights them with compute passes when something changes, draws moving objects into the same 3D texture each frame, and each pixel reads it back with a few widening cones in a pass, then the opaque pass adds that as indirect light. It runs only in Forward+ and the lab uses a 6.4 x 3.2 x 6.4 m box with 5 cm cells.
Still ask: what exactly is "the box" and when did a node appear; how does a cone sample "average many cells"; why a distance field matters for shadows; what a "light pass" computes.

## Chapter 2 flags

| Beat | # | First five words | Kinds | What I lost |
|---|---|---|---|---|
| c2-collect | 6 | Of those meshes, only | 1 | "triangle surfaces" versus what other surfaces |
| c2-collect | 7 | A triangle that misses | 4 | no sense why whole-not-clipped matters |
| c2-collect | 8 | The lab leaves the ceiling | 4 | why leave it out; reason only comes next sentence |
| c2-cube | 1 | At the default subdivision | 2 | "subdivision" never defined |
| c2-cube | 3 | Its count is halved | 3 | "while half the longest side still covers the axis" resolved only at the end, unclear |
| c2-cube | 5 | Then every side is | 6 | follows badly from "shorter axis gets fewer"; why stretch |
| c2-cube | 8 | The height's sixty-four cells | 3 | meaning only clear after the next sentence |
| c2-descent | 1 | Take one triangle of | 1 | "the teapot" not yet introduced |
| c2-sat | 1 | The overlap test looks | 2, 4 | "separating axis" new, and why a test is needed here |
| c2-sat | 3 | The test tries nine axes | 3, 5 | three counts of axes listed with no verbs, nine from what |
| c2-sat | 5 | The readout panel names | 1 | which panel, which child |
| c2-leaf | 1 | At a leaf, the | 4 | why pick the most-facing axis |
| c2-leaf | 4 | A sample adds the | 2, 5 | "leaf's sums" new; albedo, emission, normal at once |
| c2-leaf | 7 | The leaf's alpha adds | 2 | alpha first appears here, defined only as the shares |
| c2-material | 2 | Each material is read | 4 | why two tables of that size |
| c2-material | 4 | The other holds the | 2 | energy and exposure undefined |

What I understood: the voxelizer collects static meshes in the box, makes a power-of-two cube of cells, pushes each triangle down the octree testing children, keeps only the cells triangles touch (about 4 percent), and each leaf accumulates albedo, emission, normal sums and a coverage alpha from a 4x4 sampling.
Still ask: why stretch to a cube and leave the top half empty; what the sums become; what "exposure" is; why the axis choice.

## Chapter 3 flags

| Beat | # | First five words | Kinds | What I lost |
|---|---|---|---|---|
| c3-fixup | 5 | After the division, its | 4 | why set alpha to one |
| c3-fixup | 8 | The light passes then | 2, 4 | "facing every way" - what the passes do with a normal |
| c3-parents | 3 | A parent keeps only its | 4 | why keep coverage in a parent |
| c3-sort | 3 | Because the cells moved | 1, 6 | "child indices" not introduced until later |
| c3-field | 1 | The bake also measures | 1 | "leaf-sized places" |
| c3-field | 8 | A leaf stores zero | 3, 5 | "whole cells plus one" and the zero marker in one go |
| c3-bytes | 5 | It starts with the | 3 | "counted in that level's own cells" |
| c3-bytes | 6 | The three coordinates are | 4 | why those bit widths |
| c3-bytes | 7 | Then come its albedo | 3, 5 | list of fields, no verbs |
| c3-bytes | 10 | Last comes its normal | 3 | list continuation |
| c3-upload | 3 | Godot's source also holds | 4 | why mention a compiled-out version |
| c3-upload | 5 | Each VoxelGI in the | 3, 5 | "texture the size of the octree" and light passes fill it |
| c3-upload | 7 | Beside it is a buffer | 5, 6 | a second light store with no link to the texture |
| c3-upload | 8 | In the lab, the texture's | 1 | which texture, which of the two |

What I understood: leaves become averages, parents hold only coverage, cells are sorted by level and packed into two records of 48 bytes, a distance field is built with three passes and uploaded as one byte per texel, and GPU light textures are allocated for the next chapter.
Still ask: what coverage in a parent is for; why light is stored in both a texture and a float buffer; how shadow rays use the distance bytes.

## Totals

Flagged sentences: 39.
Kind 1: 9. Kind 2: 7. Kind 3: 11. Kind 4: 11. Kind 5: 9. Kind 6: 4.
