# Outline: the GPU part (chapters 11 to 15; the recap becomes 16)

A working file (deleted when the story is finished). Chapters 1 to 10 stay as they are, except where noted under
"Changes to the existing chapters". Sources are `research.md` claim numbers (sections 11 to 18 for the GPU layer);
every number on screen comes from the lab (the port's own counts, the selftest, the GPU timer), never typed.

## The architecture map

The frame graph's 2D scene (`graph: frame`, `graphs/frame.toml`), now with the ported passes. Its sections, in
the engine's order and colours, each a box of the lab's passes:

| Section | Passes (the port) | Godot |
|---|---|---|
| Cascades (CPU) | update, cull, uploads | `SDFGI::update`, `update_cascades`, `pre_process_gi` |
| Voxelize (when a cascade is dirty) | `sdfgiVoxelX/Y/Z` (raster) | `_render_sdfgi`, 3 draw lists |
| Build the field (when dirty) | `sdfgiScroll`, `sdfgiScrollOcclusion`, `sdfgiJumpFloodInit`, `sdfgiJumpFlood` (x2), `sdfgiJumpFloodShared` (x4), `sdfgiUpscale`, `sdfgiJumpFloodFull`, `sdfgiOcclusion` (x8), `sdfgiStore` | `render_region` |
| Light | `sdfgiStaticLight` (when dirty), `sdfgiDynamicLight` | `render_static_lights`, `update_light` |
| Probes | `sdfgiProbeScroll` / `ScrollStore` (when scrolled), `sdfgiProbeProcess`, `sdfgiProbeStore` | `update_probes`, `store_probes` |
| Screen | `prepass` (+ normal-roughness), `sdfgiGather`, `sdfgiForwardFS` | `process_gi`, opaque pass |

Dashed: what runs only on a dirty frame. A back arc from the probe maps to the light pass (last frame's maps: the
bounce loop). Each GPU chapter opens on it with its section lit and ends on what its output feeds.

## Knowledge level

Audience unchanged (`knowledge.toml [audience]`: knows `gpu-compute`, `gpu-sync`, `gpu-formats`, `gpu-passes`, the
render-target and BRDF topics). The GPU part uses, and must teach:

| Topic | Status | Taught in | Beats | Seconds |
|---|---|---|---|---|
| `gpu-frame` (a frame as passes declaring reads and writes; ordering and barriers derived) | not known; chapter 1 recalls it, so it is taught here properly | 11 | c11-graph, c11-levels | 45 |
| `gpu-atomics` (read-modify-write, lost updates, counters) | not known | 12 | c12-race, c12-or | 40 |
| `gpu-scan` (stream compaction: count, reserve, write contiguous) | not known | 13 | c13-compact | 40 |
| `gpu-indirect` (**new topic** in rlstd's graph: a dispatch whose size the GPU wrote; requires `gpu-compute`, `gpu-atomics`) | not known | 13 | c13-indirect | 25 |
| `gpu-divergence` (**new topic**: lanes of a workgroup run in lockstep; idle lanes, divergent loops, the slowest lane sets the time; requires `gpu-compute`) | not known | 14 | c14-lanes | 40 |
| `gpu-compute`, `gpu-sync` | known: recalled in a sentence where used (workgroups and shared memory in 13, barriers in 11 and 13) | | | |

Taught topics: 190 s of the 615 s. Each has its own demonstration (Phase 3).

## Chapter 11 | The frame on the GPU | 120 s

Bridge: "So far each stage has been an algorithm; this part follows the same frame as the GPU runs it."

| Beat (claim headline) | Claim, source | On screen |
|---|---|---|
| c11-steady: A steady frame is thirteen dispatches | 18 (steady frame), 15.02, 16.02-16.06, 17.01 | the frame graph, the four always-on passes lit in turn as each is named; a dispatch counter counts up 4 + 4 + 4 + 1 |
| c11-dirty: A scroll adds a chain in front of them | 18, 14 table | camera steps over the drag margin (`cascadeCam` sweep): the dashed sections light; the counter adds 3 draw lists + 20 dispatches per dirty cascade |
| c11-graph (teach `gpu-frame`): the GPU work is ordered by what each pass reads and writes | 11.01, 11.03 | question: "which pass may start first?"; worked example on two passes: the light pass writes `light_data`, the probe pass samples it: an arrow from writer to reader appears on the resource's card |
| c11-levels: independent passes share a level and no barrier | 11.05, 16.02, 17.07 | the DAG laid out by levels; the probe pass slides beside the depth pre-pass, the gather beside the shadows; "(Parallel)" labels; the lab's own graph says where the lab differs (11.04, lab difference 6) |
| c11-loop: the light reads last frame's probe maps | 18 (ordering), 11.06 | the back arc pulses; frame N and N-1 side by side as two copies of the strip |
| c11-memory: three hundred and twenty megabytes per viewport | 12 | a bar per resource, shared and per cascade, in the encoding colours (distance, light, cells, probes); sweep `cascades` 1 -> 8: total 141 -> 559 MiB |
| c11-profiler: measuring serialises it | 11.07 | timestamps inserted: the overlapped pairs separate |
| c11-recap, c11-sources | | |

Detailed demonstration: the level layout of the DAG with its barrier lines (one per level); a dispatch counter.

## Chapter 12 | Voxelizing with the rasterizer | 115 s

Bridge: "The first GPU work of a dirty cascade is the voxelizer, and it uses the rasterizer, not compute."

| Beat | Claim, source | On screen |
|---|---|---|
| c12-views: three orthographic views, one pixel per voxel column | 13.02, 13.03 | the region box; a camera on each +axis face; the three framebuffers as pixel grids of the region's size; sweep a slab 128 -> 8: framebuffers 128x8, 8x128 |
| c12-fragment: a fragment's position picks its voxel | 13.06 | one triangle of the teapot; one fragment lit; its interpolated position mapped to `grid_pos` (readout: local_pos, grid_pos); the voxel lights in 3D |
| c12-empty: no attachment, no depth test, both faces | 13.03, 13.04 | the same triangle seen from behind still writes; a second triangle behind it also writes (no depth test) |
| c12-race (teach `gpu-atomics`): two fragments, one voxel | 13.08 | question; two triangles overlapping one voxel; both `imageStore` albedo: the outcome flips between runs (step parameter `raceOrder`); a lost update shown as two writes, one value left |
| c12-or: OR keeps both faces | 13.07 | the facing word's six bits: fragment A sets +Y, B sets +X, `atomicOr` leaves both; a plain store would leave one (toggle `facingAtomic` in the demonstration) |
| c12-missed: a triangle smaller than a pixel is missed in all three views | 13.10 | a thin sliver swept smaller (`sliverWidth`) until its voxels vanish |
| c12-writes: four images, cleared once per cascade | 13.07, 13.09, 12 | the four render images as views (albedo RGB555, facing bits, emission RGBE, aniso), each titled with format and size |
| recap, sources | | |

Lab difference said on screen: the dummy colour target (13.03, lab difference 1), material code (13.05).

## Chapter 13 | Eighteen dispatches build the field | 135 s

Bridge: "The rasterizer left four images of one slab; a chain of compute dispatches turns them into the field."

| Beat | Claim, source | On screen |
|---|---|---|
| c13-chain: one list, eighteen dispatches, ten barriers | 14 table | the chain as a strip of dispatches, each its group count; barrier lines between; a run (`run=preprocess`) steps through, the target texture of each flashing |
| c13-pingpong: each step reads the last one's texture | 14.04 | half[0] and half[1] as two slices; the arrow alternates; the barrier as the step's wait |
| c13-half: the expensive steps run on an eighth of the cells | 14.01 | 128^3 vs 64^3 cell counts; init picks one solid voxel per 2^3 block (one block worked) |
| c13-lattice: a workgroup of the shared-memory step is a lattice, not a block | 14.03 | worked example: one 8^3 group at step 4 (one slice: 8 x 8 lattice points 4 cells apart); its 10 x 10 halo patch loaded by 125 threads into shared memory; then one cell reads its 26 neighbours from shared memory instead of the texture; sweep `jfStep` 8 -> 1: the lattice tightens into a block |
| c13-occlusion: one group per probe; eight parities tile the grid | 14.05 | the 16^3 cubes of one parity class tiling a slice without overlap; the eight classes in eight colours; the 22 shell steps as a sweep with a barrier tick each |
| c13-compact (teach `gpu-scan`): STORE packs the solid cells into a list | 14.07 | question: "how do 32,768 groups write one list without gaps?"; one 4^3 group: its solid cells take local slots (shared counter), one thread reserves a range of the list (global `atomicAdd`), the records copy out contiguous; then all groups: the list fills in an order that changes between runs (`storeOrderSeed`) |
| c13-indirect (teach `gpu-indirect`): STORE also writes the next dispatch's size | 14.07, 14.08 | the 16-byte counter buffer {x, y, z, total}: `atomicMax` raises x as groups finish; copied into the indirect buffer; the light dispatch launches with exactly that many groups |
| recap, sources | | |

## Chapter 14 | Light and probes, thread by thread | 135 s

Bridge: "The list and its count now drive the light; the light then feeds the probes."

| Beat | Claim, source | On screen |
|---|---|---|
| c14-thread: one thread per solid cell | 15.01-15.03 | the list as a row of records; a group of 64 threads over 64 records; one thread's cell lit in 3D |
| c14-stagger: a quarter of the cells each frame, interleaved | 15.04 | frames 0-3: cells with index = f mod 4 light up across the room (`lightFrame` sweep); the groups past total/4 grey out (idle) |
| c14-lanes (teach `gpu-divergence`): the slowest ray sets the group's time | 15.06 | 64 lanes as bars of shadow-march step counts; a lane deep in shadow ends early and idles; the group's time = the longest bar; sweep the light's position: the bars change |
| c14-neighbours: a cell writes its empty neighbours too | 15.07 | one cell's 26 neighbour bits; the extra texels it writes in the light texture |
| c14-probe-thread: one thread per probe, its rays a loop | 16.03, 16.04 | 289 x 17 thread grid over the probe texture; one probe's rays drawn one after another in the thread; `rayCount` 4 -> 32: the dispatch stays 111 groups, the loop lengthens |
| c14-integers: the history ring is integers so the sum never drifts | 16.05 | one coefficient: float vs Q.10 sum over 30 frames (drift vs exact); the slot `frame % 30` overwritten |
| c14-store: one thread per map texel, and its border | 16.06, 16.07 | the 6 x 6 tile in an 8 x 8 group grid (they do not align); one texel writes itself and its mirror; a bilinear tap at the edge reads the border |
| recap, sources | | |

## Chapter 15 | The gather, and what it costs | 110 s

Bridge: "Everything so far filled textures; the last compute pass turns them into light for the pixels on screen."

| Beat | Claim, source | On screen |
|---|---|---|
| c15-gather: one thread per pixel, eight by eight | 17.01, 17.03 | split view: depth and normal-roughness (left), the lit frame (right); one 8 x 8 tile outlined; one thread's pixel |
| c15-taps: eight probe taps, more for smooth surfaces | 17.04 | the pixel's 8 probes and their taps; sweep roughness: radiance taps appear below 0.99, the march below 0.2 |
| c15-buffers: ambient and reflection, mixed by trust | 17.05, 17.06 | the two RGBA16F buffers as views (rgb, alpha), then the forward pass's mix |
| c15-half: half resolution and its edges | 17.06, Finding 29 | `halfResGI` toggle: GI bleeds across the teapot's silhouette; the 9/3/3/1 weights on one pixel |
| c15-costs: where the time goes | 18 table, lab GPU timer | the performance panel (GPU time per pass, measured in the lab) on a steady frame, then a dirty one; the most expensive pass called out; said as the lab's timing, not Godot's |
| c15-trade: what each saving costs | 18 table | the savings list, one at a time, each with its cost shown (latency, quantisation, edges) |
| recap, sources | | |

## Chapter 16 | The frame again (the old 11, extended)

The walk through the frame names the GPU passes too, in order; three takeaways unchanged, plus one sentence on how
the GPU runs it (lists, barriers, indirect).

## Changes to the existing chapters

- c1-frame: the frame graph scene now holds the ported passes; its sentences name the sections (unchanged claims).
- The opening's "what you will learn" list gains "how the GPU runs it".
- `knowledge.toml`: chapters 11-15 added (requirements above), the recap renumbered to 16.

## What changes, and what it changes (sweeps)

| Relation | Parameter | Range where it changes |
|---|---|---|
| more cascades, more memory | `cascades` | 1 -> 8 (141 -> 559 MiB) |
| a thinner slab, thinner framebuffers | slab thickness (camera step) | 128 -> 8 cells |
| smaller step, tighter lattice | `jfStep` | 8 -> 1 |
| more rays, same dispatch | `rayCount` | 4 -> 32 |
| light update stride | `framesToUpdateLights` | 1 -> 16 (idle groups 0 -> 15/16) |
| roughness and taps | surface roughness | 0 -> 1 |

## The tools it needs

- **The ported passes** (Phase 4): each a lab frame pass, so the DAG, the inspector, and the GPU timer show them.
- **A dispatch view**: a domain (grid, list, texture) with its workgroups outlined, live and idle lanes coloured,
  one group or thread picked (a click), its counts in a readout. Generic (lab), any investigation's compute pass.
- **A lanes view**: per-lane bars (a loop's trip count) for one group.
- **A list view**: the solid-cell buffer as a strip of records, groups' ranges coloured, the counter buffer beside it.
- **The memory bars**: resources by bytes (from the frame graph's resource sizes; generic).
- Existing: the frame graph scene and DAG, the inspector (`imageShow`), insets, the performance panel, arrows/labels.

## Worth a detailed demonstration

The lattice workgroup in shared memory (13), the two-level compaction (13), the race and the OR (12), the lanes (14),
and the 6 x 6 tile against the 8 x 8 groups (14).
