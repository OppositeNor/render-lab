# Godot SDFGI: the story's outline

Godot 4.7.2-stable (`ed1daf0b`). Claims cite `research.md` by claim number (`R1.07`) and the checkout by
file:line, with the abbreviations of `research.md` §0. Findings (`F1` to `F25`) are narrated and shown where
their step is explained, as "read from the code", and the lab reproduces the engine's behaviour (user,
2026-10-29). The lab uses the engine's grid: 4 cascades of 128³ cells, cell 0.2 m, y_mult 1.5, 8 rays per probe
per frame, a history of 30.

Total budget: 1,830 s (about 30 minutes), eleven chapters.

## The subject and the scene

- **The study room** (`rlstd.scene/study-room`): 6 × 3 × 6 m, three coloured walls, the emissive ceiling
  panel, the sphere, two boxes, and the teapot. Its colour bleeding (red and teal walls onto the sphere and the
  floor) is what SDFGI is for.
- **Scale.** Cascade 0 is 25.6 m wide, so the whole room lies inside it: about 30 × 22 × 30 of its 128 × 128 × 128
  cells, and its probes, 1.6 m apart, fall only 4 × 2 × 4 inside the room. The story says so in chapter 2. For
  close-ups the cell grid is drawn over the room; for the cascades it pulls out to show all four boxes, with the
  room a small block at their centre.
- **Subjects by chapter.** The sphere for voxelization and the distance field (its curved surface shows the
  facing mask and the nearest-voxel distances); the floor cell under the panel for light injection; the probe
  nearest the sphere for tracing; a pixel on the red wall near the sphere for the screen pass.
- **Fade in close-ups**: the back wall when the camera looks through it at a slice; the teapot when it hides the
  sphere's cells. Both return for wide shots.
- **A static light.** The room has no static light; to show chapter 3's light lists and finding F1 the lab adds
  an omni light whose bake mode a parameter switches (Dynamic by default, so nothing changes).

## Chapters

### 1 · Where SDFGI runs · 150 s

Bridge: the opening.

| Beat | Headline (the claim) | Key sentences: claim, source, what shows it |
|---|---|---|
| c1-room | SDFGI lights the room with light that bounced | The room with SDFGI on, the camera floating; `sdfgiMix` 0 → 1 while "bounced light" is said (direct only → with SDFGI: the red wall tints the sphere). R0.0/§0 frame order. |
| c1-frame | Four stages in every frame (mermaid) | Diagram: update and cull (CPU) → voxelize dirty regions, distance field, static lights → dynamic light and bounce → probe rays (during the depth pre-pass) → store probe maps → screen GI pass → opaque shading. `focus` lights each node as named. rfc.cpp:1734, 1757, 2120-2133, 1534, 1606; gi.cpp:2061, 1249, 1311, 1414, 3968. |
| c1-switches | Forward+ only, off by default | Settings readout (rows: `sdfgi_enabled` false, cascades 4, min cell 0.2 m, y scale 75 %, rays, converge, update lights). Mobile and Compatibility have empty stubs (render_forward_mobile.h:643-646; rasterizer_scene_gles3.h:882-889). |
| c1-prepass | The probes only update during the depth pre-pass (F8) | Readout row "depth pre-pass" lit; `prepass` toggle off → the probe update node greys out in an inset of the diagram, the room's GI freezes while a light moves. rfc.cpp:2112-2133; renderer_scene_render_rd.cpp:1348-1356. |
| c1-rays | The default is 8 rays, not the 16 the setting names (F7) | Readout: the editor's label list next to the code's table, index 1 lit in both. rendering_server.cpp:3793; gi.cpp:1322-1323. |
| c1-sources | Where to read | `frame-order`, `sdfgi-update`, `settings`, `mobile-stub`, `prepass-gate`, `ray-count-table`. |

### 2 · Cascades that follow the camera · 190 s

Bridge: from where SDFGI runs to the space it covers: before anything is voxelized, the engine decides where.

| Beat | Headline | Key sentences: claim, source, what shows it |
|---|---|---|
| c2-nested | Four nested grids of 128 cells, each twice as coarse | Pull out: four wire boxes centred on the camera, labelled 25.6, 51.2, 102.4, 204.8 m; `cascadesShown` 1 → 4. R1.18, gi.cpp:573. The room is a block inside cascade 0. |
| c2-cells | In this room, cascade 0 holds everything | Back in the room: cascade 0's cell grid drawn as lines on the floor and walls; the readout counts the cells the room spans (lab-measured). |
| c2-yscale | "75 %" squashes the grid by 1.5: two thirds as tall (F3) | `yMult` 1.0 → 1.5: the box's height shrinks from 25.6 to 17.07 m and one cell's outline goes from 0.2 to 0.133 m tall; the readout shows the table {2.0, 1.5, 1.0} with 75 % lit. gi.cpp:423-424, 1468-1469. |
| c2-probes | 17 probes per axis, one every 8 cells | Probes of cascade 0 as points (17³, those in the room bright); `probeSpacing` label 1.6 m; probe 16 on the far face. R1.22, gi.cpp:427, int.glsl:166-173. |
| c2-scroll | The cascade jumps 8 cells when the camera strays 4 | Top view, 2D (`cameraDrift=0`): the camera marker moves +x from 0 to 1.0 m (`cameraX` sweep); its cell index counts 0 … 5; at 5 > 4 the box jumps 8 cells and the 8-cell slab at the far side lights as dirty. Worked example R1.A; gi.cpp:1199-1222. Cascade 1 stays put (its cell is 0.4 m). |
| c2-dirty | Only the new slab is rebuilt, unless too much moved | The old content shifts by −8 cells (arrows); a second sweep to 10 m: at 48 cells the whole cascade lights (DIRTY_ALL). R1.11, gi.cpp:1226-1243. |
| c2-sources | Where to read | `cascade-create`, `cell-size-double`, `y-mult-table`, `probe-axis-count`, `scroll-update`, `dirty-all-rule`. |

### 3 · What is voxelized, and which lights · 120 s

Bridge: a scroll leaves a dirty slab; the CPU now decides what goes into it.

| Beat | Headline | Key sentences |
|---|---|---|
| c3-regions | Each dirty axis becomes one box in world space | The slab from chapter 2 as a world AABB with its real-metre size (1.6 × 17.07 × 25.6 m); a diagonal move shows two boxes, the corner given to one only. R2.01-2.03, gi.cpp:1458-1512. |
| c3-geometry | Only Static meshes are voxelized, seen or not | Objects inside the slab light; a toggle on the teapot's `giMode` Static → Dynamic removes it from the list but it still receives GI (table). No frustum test: an object behind the camera lights too. R2.05-2.06, 2.12; cull.cpp:3265-3283. F17 in passing. |
| c3-lights | Static lights are baked once; dynamic ones every update | The extra omni light: `lightBake` Dynamic → Static moves it from the per-frame list to the region list; the sun is always dynamic. `max_sdfgi_cascade` 2: cascade 3 gets no positional lights. R2.08-2.09, 2.13; cull.cpp:3269-3276. |
| c3-finding | A list read by the wrong index (F2) | Two arrays side by side: stored by cascade number, read by position in the dirty list; with only cascade 2 dirty the read hits the empty slot 0. cull.cpp:3275, 3662-3667; gi.cpp:2448. |
| c3-sources | Where to read | `pending-regions`, `region-bounds`, `sdfgi-cull`, `gi-mode`, `static-light-cull`, `static-lights-index`. |

### 4 · Voxelization · 150 s

Bridge: with the geometry chosen, the GPU draws it into the grid.

| Beat | Headline | Key sentences |
|---|---|---|
| c4-passes | Three orthographic views, one pixel per voxel | Three cameras on the region's faces, one per axis; a frustum box each; `voxelAxis` 0, 1, 2 fills the sphere's voxels from each view. No colour target, culling off. R3.04-3.05; rfc.cpp:3122-3175, 397-399. The lab fills the same voxels from the same rules on the CPU (said on screen). |
| c4-albedo | A solid bit and 5-5-5 albedo in 16 bits | Struct panel: the voxel record `SdfgiAlbedo` (bit 0 solid, R 11-15, G 6-10, B 1-5), `highlight` per field; a sphere voxel's colour decoded. R3.11, scene.glsl:2912-2917. |
| c4-facing | Six bits say which way the surface faced | Six arrows on one sphere voxel; the bit of the axis nearest the normal lights; OR of several fragments. R3.12, scene.glsl:2919-2948. |
| c4-emission | Emission in a shared exponent, split over six faces | The panel's voxels: record `SdfgiEmission` (RGBE 8985, bits 0-29) and the 6 × 5-bit face weights. R3.14-3.15, scene.glsl:2950-2990. |
| c4-shell | Only surfaces: a thick object is hollow (F16) | Clip plane through the box and the sphere (`sliceY`): voxels only on the shell. R4.13. |
| c4-sources | Where to read | `render-sdfgi`, `sdf-fragment`, `albedo-pack`, `facing-mask`, `emission-pack`. |

### 5 · The distance field · 170 s

Bridge: the voxels say where surfaces are; rays need to know how far they are.

| Beat | Headline | Key sentences |
|---|---|---|
| c5-why | One byte per cell: the distance to the nearest surface | Image inset: a horizontal slice of cascade 0 through the sphere, grey by distance; a sphere-trace step drawn next to it (a circle of that radius). R4.11. |
| c5-seeds | Each half-size cell starts with a solid voxel of its 2 × 2 × 2 | Slice at half resolution, 64²: seeds as dots; `floodStep` 0. R4.02-4.03, pre.glsl:245-273. |
| c5-flood | Neighbours at 32, 16, 8, 4, 2, 1 pass their seeds on | `floodStep` 1 → 6: each pass, arrows from 26 (in the slice, 8) neighbours at the step's distance; the nearest-seed map fills in. R4.04-4.06, gi.cpp:2238-2273. |
| c5-upscale | Back to full size, then one more pass | `floodStep` 7, 8: upscale and the step-1 pass at 128². R4.07-4.08. |
| c5-store | A byte of one plus the distance; zero inside (unsigned) | Readout for the cell under the cursor: seed, distance, stored byte `(1 + d)/255`, and what each reader subtracts (1.0, 1.0, 1.1). R4.11-4.12, pre.glsl:954-971. |
| c5-cells | A 16-byte record for every solid cell | Struct panel `SdfgiSolidCell` (position 7-7-7, albedo + facing, light, aniso, and the 26 neighbour bits across the top bits); `highlight` per field. The neighbour bits drawn as the empty cells that point back. R4.15-4.18, pre.glsl:989-1062. |
| c5-sources | Where to read | `jump-flood-init`, `jump-flood`, `jump-flood-schedule`, `sdf-store`, `solid-cell-store`. |

### 6 · Occlusion · 90 s

Bridge: the same pass also asks, for each probe, what it can see.

| Beat | Headline | Key sentences |
|---|---|---|
| c6-shells | Visibility spreads from a probe, shell by shell | One probe's 16³ region; `occlusionShell` 0 → 22: values fill outward, zero behind the box. R5.03-5.08, pre.glsl:535-657. |
| c6-store | Four bits for each of eight probes | Readout of one cell's packed word (8 × 4 bits, R4G4B4A4 view). R5.10. |
| c6-off | Always computed, read only when enabled (F14) | `use_occlusion` false by default; gi.cpp:2345-2373 has no guard. |
| c6-sources | Where to read | `occlusion-dispatch`, `occlusion-shells`, `occlusion-store`, `occlusion-read`. |

### 7 · Light in the voxels · 220 s

Bridge: the field is built; now the solid cells are lit, so rays have something to find.

| Beat | Headline | Key sentences |
|---|---|---|
| c7-cells | A thread per solid cell, a quarter of them each frame | The solid cells as small boxes; `lightPhase` 0 → 3 lights every fourth cell in turn (interleaved, all cascades the same frame). R6.09-6.11, gi.cpp:1286-1300, dl.glsl:182-188. |
| c7-omni | Each light falls off with (1 − (d/r)⁴)² / d | The floor cell under the extra omni light; `lightHeight` sweep 0.5 → 4 m; formula readout with the terms lit; the worked numbers of R6.A. dl.glsl:125-132. |
| c7-shadow | A shadow ray sphere-traces the distance field | The march from the cell to the light: circles of each step's radius; `lightX` moves the light behind the box and the ray stops at a solid cell (hit below 0.001 cell); out of the last cascade means lit. Binary, `has_shadow` ignored (F18). R6.17-6.20, dl.glsl:378-434. |
| c7-faces | Light is kept per face, then packed | The six faces' amounts as bars; packed into E5B9G9R9 and two UNORM textures of face ratios; the 26 neighbours get the same value. Struct panel of the two texels. R6.21-6.23, dl.glsl:436-557. |
| c7-static | Static light is written into the cell once … | `lightBake` Static: the static pass adds it to the cell's stored light. R6.01-6.03. |
| c7-double | … and added again after every scroll (F1) | Static light on, `scrolls` 0 → 3: a kept cell's stored light readout grows by the light's contribution each scroll; a new cell gets it once. pre.glsl:195-199, dl.glsl:295-311, 563-572. |
| c7-bounce | Last frame's probes light the cells again | `bounceFeedback` 0 → 0.5 → 1.5: the wall cells gain the probe irradiance times albedo × 0.5 (above 1, mixed toward white). One bounce per frame round trip. R6.24-6.25, dl.glsl:216-292. |
| c7-sources | Where to read | `update-light`, `light-schedule`, `omni-attenuation`, `shadow-march`, `aniso-pack`, `static-pass`, `scroll-reinject`, `bounce-feedback`. |

### 8 · Probes trace the field · 220 s

Bridge: the cells now hold light, but only where surfaces are; the probes gather it for every point between.

| Beat | Headline | Key sentences |
|---|---|---|
| c8-rays | Eight rays a frame, 240 over thirty frames | The probe near the sphere; `rayFrame` 0 → 29 adds 8 rays a frame; the Vogel spiral fills in; even frames point to −z, odd to +z (F22). R7.06-7.08, int.glsl:89-95, 181-192. |
| c8-march | Each ray steps by the stored distance | One ray: `rayStep` 0 → n, circles of each step's radius, hit below 0.05 cell; leaving cascade 0 it continues in cascade 1. R7.10-7.11, int.glsl:206-247. |
| c8-hit | A hit reads the voxel light, weighted by its faces | At the hit: the gradient normal and the face weights; readout of light × weights. R7.12, int.glsl:249-269. |
| c8-miss | A miss reads the sky, blurred (F25) | A ray out of the window of cascades reads the sky at mip 2. In stable, the sky texture's type does not match the shader's. R7.13-7.14; gi.cpp:1359, 3670; int.glsl:45; sky.cpp:1553-1574. |
| c8-sh | Sixteen coefficients, in fixed point | Readout of 16 SH rows (bands 0 to 3) growing as rays add; `× 4 / N`, `× 1024`, integers. R7.15-7.16, int.glsl:293-347. |
| c8-history | A ring of thirty frames, summed exactly | History inset: 30 slots; the sum subtracts the oldest and adds the newest; `frame` 0 → 30: the probe brightens linearly to full. R7.18, int.glsl:323-347. |
| c8-sources | Where to read | `update-probes`, `vogel-rays`, `probe-march`, `probe-hit`, `probe-sky`, `sh-accum`, `history-ring`. |

### 9 · From coefficients to maps · 140 s

Bridge: sixteen numbers per probe are not something a pixel can sample; this step makes maps it can.

| Beat | Headline | Key sentences |
|---|---|---|
| c9-oct | Two 6 × 6 octahedral maps per probe | Image inset: one probe's irradiance and radiance maps with their 1-texel borders; the sphere of directions unfolding onto the square. R8.02-8.06, int.glsl:350-470. |
| c9-gain | A uniform sky comes out 4/π too bright (F4) | `skyOnly` scene (a uniform sky, no geometry): readout "sky radiance L, stored irradiance 1.27 L, Godot's sky ambient 1.0 L"; formula with the 4 in place of 4π lit. Numbers measured by the lab. int.glsl:328, 415-426. |
| c9-texel | Written at texel corners, read at centres (F5) | Inset zoom: the direction of texel k at u = k/6, the reader's sample at the corner, the 50/50 blend. int.glsl:356-361; gi.glsl:206-208. |
| c9-scroll | Scrolled probes keep their history; new ones borrow the parent's | The scroll of chapter 2 again: probe history moves one probe; the new edge probes take the trilinear average of cascade 1. R8.09-8.14, int.glsl:475-620. F10 in passing. |
| c9-sources | Where to read | `store-probes`, `oct-store`, `oct-border`, `oct-read`, `probe-scroll`, `parent-seed`. |

### 10 · Shading a pixel · 210 s

Bridge: every probe now has its maps; the last pass turns them into each pixel's light.

| Beat | Headline | Key sentences |
|---|---|---|
| c10-pass | A compute pass rebuilds each pixel's position and normal | The pixel on the red wall; its reconstructed position and normal drawn; `y_mult` applied. R9.01-9.06, gi.glsl:161-192, 290-296. |
| c10-cascade | The finest cascade that holds it, blended near its edge | Cascade boxes; the blend band from 5.5 to 7.5 probe units shaded; `pixelDistance` sweep through the band: the weights of cascades 0 and 1 in the readout. R9.07-9.08, gi.glsl:311-366. |
| c10-probes | Eight probes, weighted by distance and by facing | The pixel's eight probes with weights as labels (the worked example R9.A: 53 %, 45 %, 1 %, 1 %); the normal bias of 1.1 cells drawn. R9.09-9.10, gi.glsl:194-286. |
| c10-specular | Rough surfaces read the maps; smooth ones trace | `roughness` 1 → 0 on the sphere: radiance map, irradiance mixed above 0.2, below 0.2 a traced reflection ray through the field (softness circles). R9.11-9.16, gi.glsl:259-476. |
| c10-compose | The result replaces the sky's ambient by its alpha | Readout of the composition: `mix(sky, gi, alpha)`, then AO, albedo, 1 − metallic; `sdfgiMix` 0 → 1 on the whole room. R9.17-9.20, scene.glsl:1982-2195. |
| c10-others | Transparent surfaces and fog read the probes themselves | Fog in the room with the probes' ambient; the fog's lookup at the wrong height (F6) drawn as two markers; transparents never trace and ignore the energy (F11). R10.01-10.10, fwdgi.glsl:126-242, fog.glsl:703-798. |
| c10-sources | Where to read | `process-gi`, `cascade-select`, `cascade-blend`, `probe-weights`, `specular-trace`, `gi-compose`, `forward-gi`, `fog-sdfgi`. |

### 11 · The frame again · 70 s

Bridge: from one pixel back to the whole frame.

| Beat | Headline | Key sentences |
|---|---|---|
| c11-recap | One frame, in order | The diagram of chapter 1 with each node lit while it is named: the cascades follow the camera, a dirty slab is voxelized into a distance field, cells are lit, probes trace and average, maps are stored, and every pixel reads eight probes. |

## What changes, and what it changes

| Relation | Parameter swept | What must visibly change (range where it does) |
|---|---|---|
| A cascade moves in 8-cell steps | `cameraX` 0 → 1.0 m, then → 10 m | The box jumps at 0.8 m (cell 5 > 4); the dirty slab is 1.6 m; DIRTY_ALL from 48 cells (9.6 m). |
| The y scale squashes the grid | `yMult` 1.0 → 1.5 → 2.0 | Box height 25.6 → 17.07 → 12.8 m; voxel height 0.2 → 0.133 → 0.1 m. |
| The jump flood fills the field | `floodStep` 0 → 8 | The nearest-seed map: empty → complete, with visibly wrong regions before the last passes. |
| Attenuation | `lightHeight` 0.5 → 4 m (range 5) | The cell's light and the readout's attenuation term (2.0 → 0.0 near 5 m). |
| Shadowing is binary | `lightX` across the box's edge | The cell flips between lit and unlit; the march's last circle touches a solid cell. |
| The static light doubles | `scrolls` 0 → 3 | A kept cell's stored static light 1×, 2×, 3×, 4× (lab-measured). |
| Bounce feedback | `bounceFeedback` 0 → 0.5 → 1.5 | Wall cells' light and the room's colour bleeding grow; above 1 the bounce whitens. |
| History convergence | `frame` 0 → 30 | The probe's irradiance grows linearly to its converged value (k/30). |
| Cascade blend | `pixelDistance` across 5.5 → 7.5 probe units | The cascade-0 weight 1 → 0 (smoothstep). |
| Specular source | `roughness` 1 → 0 | Irradiance share falls to 0 at 0.2; below 0.2 the traced reflection's share grows to 100 % at 0. |

## Worth a detailed demonstration

- The scroll (c2-scroll, c2-dirty): a 2D top view where a still loses the threshold and the jump.
- The jump flood (c5-flood): eight steps, each showing which neighbour passed which seed.
- The shadow march and the probe march (c7-shadow, c8-march): circles of each step.
- The probe's ray set over thirty frames (c8-rays) and the history ring (c8-history).
- The octahedral map (c9-oct, c9-texel): the sphere of directions and the texels it lands in.
- The pixel's eight probes (c10-probes): weights changing as the pixel moves.

## The tools it needs

From the lab (reuse): boxes, segments, points, labels (frame.rs `DrawLists`); the struct panel (`PanelRecord`);
the readout with formulas (`Readout`); the picture inset (`PictureInset`, packages/rlstd/src/inset.rs, for slices and octahedral maps); the
history inset (`HistoryInset`, for the ring of thirty); a mermaid diagram beat; the dock kit.

To build, in the package (no other investigation needs them yet):

- **The SDFGI model, on the CPU**: cascades and `update()` (scroll, dirty regions), voxelization of the study
  room's shapes by the engine's rules (fragment position to voxel, last write wins, OR of facing), the jump flood
  and STORE, occlusion, the light pass, the probe march and SH history, the octahedral STORE. It mirrors the
  shaders line by line, and the selftest compares it with the WGSL port.
- **The WGSL port**: the light pass, the probe integration and STORE, and the screen pass as the investigation's
  `SceneShading` (the room shaded with SDFGI), with step parameters.
- **A voxel overlay**: solid cells of a region as instanced boxes (a few thousand at the room's scale), coloured
  by albedo, light, or face.
- **Slice insets**: a 2D slice of the distance field, the seed map, the occlusion values, or the light texture,
  with a marker on the worked cell.
- **March overlays**: a ray's steps as circles (2D) or spheres (3D) of the stored distance.
- **A probe overlay**: probes as small spheres coloured by their irradiance in a direction, and one probe's ray
  set.
- **Step parameters**: `cascadesShown`, `cameraX`, `yMult`, `voxelAxis`, `sliceY`, `floodStep`,
  `occlusionShell`, `lightPhase`, `lightHeight`, `lightX`, `lightBake`, `scrolls`, `bounceFeedback`, `rayFrame`,
  `rayStep`, `frame`, `pixelDistance`, `roughness`, `sdfgiMix`, each defaulting to no change.

## Where the lab differs (said on screen and in the README)

- The voxels are filled on the CPU from the study room's shapes by the engine's rules, not by three raster
  passes; the jump flood runs the engine's schedule on the CPU.
- The scroll is a CPU computation of `update()` along a camera path longer than the room, drawn schematically.
- The room is small next to cascade 0: only cascades 0 and 1 carry anything at this scale, said in chapter 2.
- The extra omni light (static or dynamic by a parameter) exists for chapters 3 and 7; the room's own light is
  its emissive panel.
