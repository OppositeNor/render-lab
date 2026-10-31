# Godot VoxelGI: merged research report

Engine: Godot Engine 4.7.2-stable, commit `ed1daf0bf001b61586d9930840f2f1394092c079` (2026-08-16), the same checkout as
the SDFGI investigation. License: MIT (`LICENSE.txt` in the checkout).
Topic: VoxelGI, the baked-voxel, cone-traced global illumination of the Forward+ renderer.

Status: every claim was read from the source by a research agent; the main agent re-read the ones marked **(checked)**.
"Inferred" marks a conclusion drawn from reading, not stated by the code. Nothing was run. Paths are relative to the
checkout:

- `gi.cpp` / `gi.h`: `servers/rendering/renderer_rd/environment/gi.{cpp,h}`
- `rfc`: `servers/rendering/renderer_rd/forward_clustered/render_forward_clustered.cpp`
- `cull`: `servers/rendering/renderer_scene_cull.cpp`
- `vg.glsl`, `gi.glsl`, `vgdebug.glsl`: `servers/rendering/renderer_rd/shaders/environment/{voxel_gi,gi,voxel_gi_debug}.glsl`
- `sfc.glsl`: `servers/rendering/renderer_rd/shaders/forward_clustered/scene_forward_clustered.glsl`
- `fgi.glsl`: `servers/rendering/renderer_rd/shaders/scene_forward_gi_inc.glsl`

"Where the lab differs" notes say what the lab will have to do differently, as far as can be told before anything is
built. They are expectations, to be confirmed in Phase 3 and 4.

## 0. Overview: the data flow

```
 editor Bake (CPU, once)                      per frame, only on change              per frame, every frame
 ┌──────────────────────────┐   upload   ┌──────────────────────────────────┐   ┌──────────────────────────────────┐
 │ meshes → sparse octree   │ ─────────▶ │ light passes over the octree      │   │ depth pre-pass writes a per-pixel│
 │ (albedo, emission,       │  octree +  │ direct → (2nd bounce) → mip pass │   │ VoxelGI tag (≤ 2 instance ids)   │
 │  normal per leaf cell)   │  cell data │ → write RGBA8 3D texture (mips)  │   │ gi.glsl: cone trace per pixel    │
 │ + distance field (EDT)   │  + SDF     │ dynamic objects re-plotted       │──▶│ → ambient + reflection buffers   │
 └──────────────────────────┘            │ into it every frame              │   │ forward pass mixes them by alpha │
                                         └──────────────────────────────────┘   └──────────────────────────────────┘
                                                       │ also read by volumetric fog (all mips at a point)
```

The feedback loop: the second bounce cone-traces the 3D texture written by the first pass (vg.glsl:465-522, gi.cpp
pass loop :3114-3171). The SDF is read only by the light passes' shadow ray march, never by the final cone trace.

### Subtopics, in the order the data is made and used

1. The bake: meshes to a sparse octree and a distance field (CPU, editor)
2. Storage: VoxelGIData, the GPU buffers, the 3D texture and its mips
3. The light passes: direct light, the second bounce, mipmaps, the texture write
4. Dynamic objects: re-voxelising moving meshes every frame
5. The read side: the per-pixel tag and the screen-space cone trace
6. Compositing, other consumers, renderer support, and debug views

## 1. The bake (CPU, editor)

| Claim | Source | Where the lab differs |
|---|---|---|
| The Bake button calls `VoxelGIEditorPlugin::_bake`; with no data it asks for a `.res` path first. `VoxelGI.bake` is also bound for run-time use (the class reference says it can be baked in an exported project). | editor/scene/3d/voxel_gi_editor_plugin.cpp:42, :169; voxel_gi.cpp:570; doc/classes/VoxelGI.xml:9 (fact check) | The lab bakes at load, from a CPU module that follows the voxelizer (no editor). |
| `VoxelGI::bake` maps `subdiv` {64,128,256,512} to depth {6,7,8,9}; defaults `subdiv = SUBDIV_128`, `size = (20,20,20)`; `size` axes are clamped to ≥ 1. | scene/3d/voxel_gi.cpp:434-435; voxel_gi.h:120-121 | The lab's room is smaller; it will need its own size and subdiv (likely 64 or 128); say so. |
| Order: camera exposure → `begin_bake(depth, AABB(-size/2,size), exposure)` → `_find_meshes` → `plot_mesh` per mesh → `end_bake` → `get_sdf_3d_image` → `allocate`. | voxel_gi.cpp:437-518 | |
| The scan root is the VoxelGI's parent; a mesh is collected if visible in tree, `gi_mode == GI_MODE_STATIC` (the default), and its AABB in VoxelGI-local space intersects the bounds. Layers are ignored. `get_meshes()` adds MultiMesh, CSG, GridMap meshes (surface materials only). | voxel_gi.cpp:321-333, visual_instance_3d.h:144 | |
| Only `PRIMITIVE_TRIANGLES` surfaces plot; triangles not overlapping the original bounds are culled, not clipped. | scene/3d/voxelizer.cpp:416, ~469 | |
| Bounds: the longest axis gets `1 << depth` cells; each shorter axis halves its cell count while `axis_size/2 >= size[i]`; the bounds become a cube of the longest side, anchored at the min corner. Cell size is uniform. | voxelizer.cpp:666-706 | |
| Worked example, defaults: 128³ cells, 8 levels (0..7), cell = 20/128 = 0.15625 m; `to_cell_space` = scale 6.4, offset 64. | voxelizer.cpp:704-706 (arithmetic) | |
| Octree plot: `_plot_face` recurses from the root; a child octant is created only if a separating-axis triangle–box test passes (`Geometry3D::triangle_box_overlap`). | voxelizer.cpp:78; core/math/geometry_3d.h:733 | The test is SAT; it resembles Akenine-Möller's tribox, but the code cites no paper: do not cite one. |
| At a leaf, the triangle is sampled on a 4×4 grid of columns along its dominant axis (`color_scan_cell_width = 4`, 16 samples): each column hit gives barycentric UV and normal; albedo/emission are read nearest from 128×128 material caches; sums are multiplied by 1/16, so alpha = coverage. A fallback samples the closest point with alpha 1/16. | voxelizer.cpp:78-300, voxelizer.h:~99 | |
| Materials: only `BaseMaterial3D` is read; albedo texture resized to 128² (cubic), `srgb_to_linear`, multiplied by the albedo colour; emission = colour × `emission_energy_multiplier` × bake exposure (× intensity with physical units), only with `FEATURE_EMISSION`. Other materials: white albedo, no emission. | voxelizer.cpp:298-401 | |
| Quirk: with `EMISSION_OP_MULTIPLY` and no emission texture, emission bakes to 0. | voxelizer.cpp (~380) | Inferred from the empty-image path; not worth narrating. |
| `end_bake`: `_sort` (by level, then x, y, z; root first) and `_fixup_plot`. Leaves: albedo/emission/normal divided by accumulated alpha; normal zeroed if its length < 0.01 ("too much fight"), else normalised; alpha set to 1. **Parents: albedo, emission, normal set to 0; alpha = sum(children alpha)/8.** **(checked)** | voxelizer.cpp:536, :598-660 | Key for the story: the bake stores colour only at leaves; the coarser levels are made on the GPU (§3). |
| Distance field: an exact separable Euclidean distance transform (3 passes; "based on prideout.net/blog/distance_fields"), seeded by leaf cells; one byte per leaf-resolution voxel: 0 = solid, else `MIN(uint(sqrt(d²)),254)+1`. | voxelizer.cpp:829-950 | CPU in the lab too; can be shown being computed pass by pass. |
| Bake exposure: 1.0 without camera attributes; else `exposure_multiplier`, or the physical `calculate_exposure_normalization()`. It scales emission only, and is stored as the probe's baked exposure. | voxel_gi.cpp:529; camera_attributes.cpp:228, :424 | |
| The editor's debug bake (`debug_bake`) adds a MultiMeshInstance3D of cubes at level `depth-1` coloured by the cells' albedo — which `_fixup_plot` has zeroed, so they would be black (inferred, not rendered). | voxelizer.cpp:959-1069 | Not shown in the story except perhaps as an aside. |

## 2. Storage

| Resource | Kind, format, size | Written by / read by | Source |
|---|---|---|---|
| `octree_buffer` | storage buffer, `CellChildren { uint children[8] }`, 32 B per cell; `0xFFFFFFFF` = no child | bake upload / mipmap pass, debug | gi.cpp:67-110; vg.glsl:12-31 |
| `data_buffer` | storage buffer, `CellData`, 16 B per cell (below) | bake upload / light, bounce, write, debug | gi.cpp:96-110 |
| `sdf_texture` | 3D `R8_UNORM`, `octree_size`, 1 mip, from the CPU EDT | bake upload / light passes' `raymarch` only | gi.cpp:113-123; vg.glsl:180-197 |
| `texture` (per instance) | 3D `R8G8B8A8_UNORM`, `octree_size`, `depth+1` mips, cleared to 0 | write pass, dynamic plot / cone trace, second bounce, fog, debug | gi.cpp:2631-2644 |
| `write_buffer` (per instance) | storage buffer, one `vec4` (16 B) per cell: the float light accumulation | light, bounce, mip passes / bounce, mip, write passes | gi.cpp:2646-2652 |
| `mipmaps[i]` | a shared 3D slice per mip; mip 0 = the leaf level, the last = the root (`level = levels-1-i`) | | gi.cpp:2654-2662 |
| `dynamic_maps[]` | 2D maps, allocated per instance even with no dynamic objects: 4 oversampled (16×, 8×, 4×, 2× of the largest axis), then one per mip. Map 0 has albedo/normal/orm RGBA8, emission RGBA16F, depth R32F, a depth buffer | dynamic pass | gi.cpp:2745-2955 |
| light UBO | 32 × `VoxelGILight` (128 B, std140) | | gi.h:164-182, :222 |
| instance UBO | 8 × `VoxelGIData` (112 B): xform, bounds, dynamic_range, bias, normal_bias, blend_ambient, mipmaps, exposure_normalization | per frame | gi.h:761-774; gi.cpp:3838-3945 |

`CellData`, 16 bytes (encoders voxelizer.cpp:750-790; decoders vg.glsl:416-428):

| Offset | Field | Encoding |
|---|---|---|
| +0 | position | `x | y<<11 | z<<21` (11/10/11 bits, in that level's coordinates); the struct's comment says "10 bits" |
| +4 | albedo | RGBA8 UNORM, linear; A = coverage (1 at leaves, sum/8 at parents) |
| +8 | emission | RGB9E5 shared exponent (`Color::to_rgbe9995`, core/math/color.h:135): 9-bit mantissas, 5-bit exponent, decode `m × 2^(e−15−9)` |
| +12 | normal | three snorm8 (`n×127`), byte 3 unused; zero = no normal |

- Cells are sorted coarse to fine, so each level is contiguous; `level_counts` has `depth+1` entries; a level's offset
  is the sum of coarser counts (voxelizer.h:73-91; gi.cpp:2658-2662).
- World → texel: `to_cell_xform` maps local space to cells with the longest axis spanning `[0, 2^depth]`; a cell at
  level L, position (x,y,z) is texel (x,y,z) of mip `depth−L` (voxelizer.cpp:704; gi.cpp:2657).
- The texel holds `rgb = light / dynamic_range` (clamped by RGBA8) and `a = coverage`; readers multiply by
  `dynamic_range × energy × exposure_normalization` (vg.glsl:550; gi.cpp:3890-3899).
- The VoxelGIData properties, defaults and consumers (voxel_gi.h:50-56; gi.h:84-91): `dynamic_range` 2.0 (1–8;
  write divide, bounce restore, readers' multiply), `energy` 1.0 (0–64; folded into the readers' dynamic_range only),
  `bias` 1.5 (0–8; cone start distance in cells), `normal_bias` 0.0, `propagation` 0.5 (mip divisor), `interior` false
  (`blend_ambient = !interior`), `use_two_bounces` true. `dynamic_range`, `propagation`, and `use_two_bounces` bump
  the version (relight); the others do not.
- **Default mismatch (checked):** `VoxelGIData::bias` defaults to 1.5 (voxel_gi.h:52) but the RenderingServer's
  `VoxelGI::bias` to 1.4 (gi.h:87). `set_bias` forwards to the server (voxel_gi.cpp:158-160); a resource whose bias is
  never set (1.5 being the default, it is not saved) would run at 1.4 (inferred; not tested). Narrate the value the
  shader gets, or avoid naming it.
- The GPU SDF shader `voxel_gi_sdf.glsl` exists but its only call site is under `#if 0`; the live SDF is the CPU one
  (gi.cpp:~124-195).
- Memory, defaults (arithmetic): the 3D texture 128³ × 4 B = 8 MiB, all mips ≈ 9.1 MiB; the SDF 2 MiB; per cell 64 B
  (octree 32 + data 16 + write 16); the dynamic maps ≈ 120 MiB per instance, allocated even with no dynamic objects
  (inferred from the formats). The editor's "Low/Medium/High" estimate counts only the dense RGBA8 texture
  (voxel_gi_editor_plugin.cpp:113-125).

Where the lab differs: WGSL has the same formats (`rgba8unorm` 3D, `r8unorm`, storage buffers), so the storage can be
mirrored as is; separate resources become separate views (Phase 3).

## 3. The light passes

When they run (cull:3853-4106; gi.cpp:2615-3171):

- Each frame a visible VoxelGI is queued (cull:2963) and `voxel_gi_update` called (cull:4102), but light passes run
  only when `update_lights`: the data or version changed (gi.cpp:4298-4302) or the light cache is dirty. The cache
  compares per light: instance, shadow, type, transform, colour, energy, intensity, indirect energy, range,
  attenuation, spot angle and attenuation, area parameters, sky mode, and the count (cull:3920-4000). Moving the
  VoxelGI node alone does not relight (inferred). There is no frame budget: one compute list (gi.cpp:3105-3171).
- Lights: those paired with the probe (bake mode Static or Dynamic, the default Dynamic; AABB overlap) plus every
  visible directional light; at most `voxel_gi_max_lights = 32` (hard-coded, `#define MAX_LIGHTS`), in HashSet order
  then directionals (cull:289-291, :3957-3993; gi.h:222; gi.cpp:2971, :3586). Types: directional, omni, spot, area.
- **Quirk (checked):** a sky-only directional light does `light_count--; continue` inside `for (i < light_count)`
  (gi.cpp:2985-2988): the slot keeps a stale record and the last light is dropped. Not worth narrating; note in the README.
- Light record: energy = `ENERGY × INDIRECT_ENERGY`; colour sRGB→linear; position, direction, range in cell space;
  physical light units multiply by intensity and baked exposure and divide by 4π (omni), 2π (area), π (spot)
  (gi.cpp:2980-3055).

Passes, per update (`passes = use_two_bounces ? 2 : 1` when lights changed, else 1 "only re-blitting"; gi.cpp:3096-3100):

| Pass | Mode | What it does | Source |
|---|---|---|---|
| direct light | `MODE_COMPUTE_LIGHT`, leaf cells, 64 per group | for each light: attenuation (`get_omni_attenuation`: `(max(1−(d/r)^4,0))^2 · d^−decay`), spot rim, back-face reject, shadow by **ray marching the SDF** (steps ≥ one cell along the dominant axis; hard 0/1); `light × albedo × max(0, n·−l)` (or no cosine if the cell has no normal); area lights by LTC, unshadowed; plus emission → `write_buffer` | vg.glsl:180-342, :423-461 |
| second bounce | `MODE_SECOND_BOUNCE`, leaf cells with a normal | six cones in the cell's tangent frame (0.25 along the normal, 0.15 ×5 at 60°, tan 0.577), start 1.5 cells, front-to-back until alpha 0.95, sampling the texture written by pass 0; × weight × albedo × dynamic_range, added to the direct light | vg.glsl:465-522 |
| mipmaps | `MODE_UPDATE_MIPMAPS`, one dispatch per level | parent = sum of children's light / `mix(8, child count, propagation)`: propagation 0 counts empty children as black, 1 averages only occupied ones | vg.glsl:526-544 |
| write | `MODE_WRITE_TEXTURE`, every level | `imageStore(texel, vec4(light / dynamic_range, coverage))`; texels without a cell stay 0 | vg.glsl:548-552 |

Order within a pass: light (or bounce) at mip 0, mips 1..n−1, barrier, write at every level; pass 1 recomputes the
mips from the new leaf values (gi.cpp:3114-3171). `aniso_strength`, `emission_scale`, `stack_size` are pushed but
never read (vg.glsl:88-97): no anisotropic (directional) storage in this version.

Where the lab differs: none expected for the math; the lab runs the passes once at load and on a light change, like
the engine.

## 4. Dynamic objects

- `gi_mode = Dynamic` sets `DYNAMIC_GI` (visual_instance_3d.cpp:479-497); a paired dynamic geometry is listed in
  `dynamic_geometries` (cull:261-281). Every frame the probe is visible, all its visible dynamic geometries are
  re-voxelised, whether they moved or not, with no frustum culling (cull:4074-4102). Mobile skips it
  (render_forward_mobile.cpp:3517).
- Per object, six orthographic views (±X, ±Y, ±Z) of its AABB in a grid oversampled 16× (`multiplier = 16`), each
  rendered with the object's own material by `_render_material` (rfc:2957-3014) into albedo, normal, orm, emission,
  and linear depth (sfc.glsl:1040-1047, :3001-3017). Then compute, per view (gi.cpp:3175-3395):
  - `MODE_DYNAMIC_LIGHTING`: each texel → a voxel position from its depth; the same direct-light function as the bake
    passes (shadows from the baked SDF, so only static geometry casts them); adds emission. No bounce. (vg.glsl:556-615)
  - the shrink chain: 2×2 → 1 downsampling, 16× → 8× → 4× → 2× (write only), then 1× and each coarser mip, where it
    also **plots** into the 3D texture mip: splat into two voxels along the view axis weighted by the depth's
    fraction, blended "over" the existing texel; coarser mips divide colour by `mix(8, count, propagation)` and
    alpha by 8 (vg.glsl:617-705).
- Composite: there is no separate dynamic texture. While dynamic data exists, each frame clears the whole 3D texture,
  re-writes the static light from `write_buffer` (the write pass), then plots the dynamic objects over it
  (gi.cpp:2962-2967, :3067-3175, :3394).
- Costs: per object per frame, 6 raster passes and 6 × (1 + 3 + mips) dispatches; no object cap; 32 lights in the
  buffer, but the dynamic push constant's `light_count = p_light_instances.size()` is not clamped (gi.cpp:3289,
  **checked**).
- Debug `LIGHT_FULL` exists because dynamic voxels can be outside the baked cell set (gi.cpp:3487-3499).

Where the lab differs: the plot is a read-modify-write of an `rgba8` 3D storage image; WGSL storage textures are
read-write only for a few 32-bit formats, so the lab will need a ping-pong copy or an `r32uint` packed stand-in, or a
CPU stand-in. To decide in Phase 3; say on screen if it differs.

## 5. The read side: the per-pixel tag and the cone trace

- Pairing: a geometry overlapping several VoxelGIs keeps the first two of a HashSet (no ordering by distance or size)
  (cull:3205-3215; rfc:5060-5071). Per frame, at most 8 visible VoxelGIs (`MAX_VOXEL_GI_INSTANCES`, gi.h:449) get
  render indices; each geometry's two are packed into `gi_offset` (rfc:1038-1056). A lightmap takes precedence; a
  geometry with a VoxelGI never uses SDFGI's forward path (rfc:1052-1060, :4503).
- With any VoxelGI visible, the depth pre-pass is `PASS_MODE_DEPTH_NORMAL_ROUGHNESS_VOXEL_GI` (rfc:1871, :1926) and
  writes normal+roughness (RGBA8; roughness × 127/255, inverted for dynamic objects) and an `R8G8_UINT` tag: the two
  render indices, 0xFF for none (rfc:271-273; sfc.glsl:1052, :3012-3038). MSAA: `resolve_gi` picks one sample, no
  averaging (effects/resolve.cpp:72-114).
- `process_gi` (gi.cpp:3968) dispatches gi.glsl 8×8 over the screen (half size with
  `rendering/global_illumination/gi/use_half_resolution`, default false: one full-res pixel of each 2×2), writing
  ambient and reflection `RGBA16F` (gi.cpp:3987-4000). Variants: VoxelGI, SDFGI, combined, each with or without a
  sampler for the tag (gi.cpp:3721-3725, :4076-4083).
- **(checked)** `if (normal.length() > 0.5)` (gi.glsl:620) uses GLSL's `.length()` method, the component count 3, so
  it is always true: the "valid normal" test does nothing.
- Per pixel, for each of up to 8 instances whose index is in the tag (gi.glsl:655-659), `voxel_gi_compute`
  (gi.glsl:533-607):
  1. position, normal, reflection → cell space (`xform` = to_cell × probe⁻¹ × camera translation); `position +=
     normal × normal_bias`; outside `[0, bounds]` → nothing.
  2. edge fade `blend = clamp(1 − max(|pos/bounds × 2 − 1|), 0, 1)`: 1 at the centre, 0 at the faces.
  3. diffuse: Low (default, `voxel_gi/quality` 0): 4 cones at 45°, weight 0.25, traced by
     `voxel_cone_trace_45_degrees` (start at `bias`, radius = max(0.5, dist), LOD + 1 per step, dist doubles).
     High: 6 cones (0.25 + 0.15 × 5, tan 0.577) by `voxel_cone_trace`. Quality reaches the shader as a push-constant
     bool (gi.cpp:4043).
  4. `voxel_cone_trace` **(checked)** (gi.glsl:487-506): from `dist = bias`, `diameter = max(1, 2 tan θ dist)`, sample
     `textureLod(uvw, log2(diameter))` trilinearly, `color += (1 − color.a) × sample`, `dist += diameter/2`, until
     alpha ≥ 1, `dist ≥ length(bounds)`, or the cone leaves the box.
  5. specular: one cone along the reflection, half-angle `roughness² × 0.99 × π/2`, by `voxel_cone_trace`.
  6. × `dynamic_range × energy × exposure_normalization`; interior forces alpha 1; weighted by `blend`.
- The pixel's sum is divided by the sum of blends; with SDFGI, the VoxelGI result is composited over SDFGI's by its
  alpha (`blend_color`, gi.glsl:149-159, :660-670).
- `dynamic_object` is decoded but nothing uses it (gi.glsl:623-626).

Where the lab differs: the lab has one VoxelGI, so the tag and the two-instance blend can be shown but not stressed;
a second, overlapping VoxelGI could demonstrate them.

## 6. Compositing, other consumers, support, debug views

- Frame order (rfc `_render_scene` :1704): instance setup (:1736-1763) → depth pre-pass with the tag (:2137) →
  `_pre_opaque_render`: shadows, then `process_gi` (:1606), SSAO/SSIL, SSR, clusters, volumetric fog (:1680) →
  opaque colour pass (:2200) → debug cubes (:2275) → sky (:2296) → transparent pass (:2411) → post (:2550).
- The only gate is `voxel_gi_instances->size() > 0` (rfc:1871); UNSHADED/OVERDRAW debug modes empty the list
  (renderer_scene_render_rd.cpp:1482-1487).
- Opaque surfaces (`INSTANCE_FLAGS_USE_GI_BUFFERS`) read the buffers at `screen_uv`: `ambient = mix(environment
  ambient, buffer.rgb, buffer.a)`, the same for the sky reflection (sfc.glsl:1982-2024). The alpha is VoxelGI's
  coverage: where the cones found nothing, the environment shows through (unless interior). Then reflection
  probes, `× ao`, specular occlusion from the ambient's luminance, `× albedo`, SSIL, SSR, the environment BRDF, `×
  (1 − metallic)`, and the sum (sfc.glsl:2028-2270, :3056, :3090).
- Transparent surfaces with a paired VoxelGI trace per pixel in the forward shader (`sc_use_forward_gi`, set only for
  alpha-list surfaces, rfc:1102, :1150-1153): always 4 cones, tan 0.98269, stop at alpha 0.95, the environment mixed
  per cone (fgi.glsl:4-108). Opaque surfaces never take this path.
- Volumetric fog samples every mip at the froxel's point and composites them front to back (not a cone), scaled by
  `volumetric_fog_gi_inject` (default 1.0) (fog.cpp:1135; volumetric_fog_process.glsl:705-722).
- Support: Forward+ only. Compatibility's entry points are stubs (drivers/gles3/rasterizer_scene_gles3.cpp:1255-1270);
  on Mobile the shared RD code still creates and updates the instances (the light passes run), but no geometry is
  paired (render_forward_mobile.h:583) and the texture binding is commented out (render_forward_mobile.cpp:606-626), so
  nothing reads it (fact check); the class doc says Forward+ only (doc/classes/VoxelGI.xml:8).
- Debug views: `DEBUG_DRAW_VOXEL_GI_ALBEDO / _LIGHTING / _EMISSION` draw one cube per **leaf** cell (level 0 of the
  mip list), decoded from `CellData` or `texelFetch` of the light texture; with dynamic data, `LIGHT_FULL` draws every
  grid cell with alpha > 0, dithered (gi.cpp:3431-3510; vgdebug.glsl). **(checked)** The LIGHT mode's
  `color_interp.xyz *params.dynamic_range;` (vgdebug.glsl:94) has no `=`, so it shows texel ÷ dynamic_range.
  `DEBUG_DRAW_GI_BUFFER` shows ambient + reflection (rfc:2597-2601). The gizmo draws the box and its cell grid.
- Interactions: SSAO multiplies the GI ambient; SSIL replaces a share of it; SSR replaces specular; reflection
  probes with ambient override it where their weight is 1; SDFGI is under it (above).

## 7. Settled in the follow-up round

- **Directional lights.** Bake mode is never checked for them: every visible directional light lights every VoxelGI
  through `scenario->directional_lights` (cull:3964-3995, :4041-4066), even one with bake mode Disabled. A directional
  light with bake mode Static/Dynamic is also inserted in the volumes BVH with a zero-size AABB at its origin
  (cull:1760-1764, :1887-1890; light_storage.cpp:535-537) and paired like any light, so one whose origin lies inside the
  probe is probably uploaded twice (inferred: the BVH's zero-box overlap test was not read). README note, not narration.
- **Sky-only slot.** The skipped slot keeps only its new `type`; its other fields are whatever the last light in
  that slot left (`memnew_arr`, gi.cpp:3582). Unless it is the last slot, the shader evaluates a ghost directional
  light and the real last light is dropped.
- **Without a depth pre-pass** (`rendering/driver/depth_prepass/enable`, default true; disabled for the vendors in
  `disable_for_vendors`, "PowerVR,Mali,Adreno,Apple"), nothing writes or clears normal-roughness and the tag
  (only the pre-pass draws them, rfc:2120-2152; the colour pass has no such attachments, rfc:2222), yet `process_gi`
  still runs on them (rfc:1580, :1607). No fallback was found. Out of scope for the story (a desktop Forward+ path).
- **Dynamic material pass alpha.** The `min(alpha, length(ambient_light))` line is shadow-to-opacity, compiled out
  under `MODE_RENDER_DEPTH` (sfc.glsl:2883-2885). Coverage is the material's alpha (sfc.glsl:3002-3003), binarised to
  0 or 1 by alpha scissor, hash, or clip; plain blended materials keep fractional alpha.
- **Dynamic light count.** Confirmed: the dynamic pass loops `params.light_count = p_light_instances.size()`
  (gi.cpp:3289; vg.glsl:586) over `Light data[MAX_LIGHTS = 32]` (vg.glsl:70): past 32 lights it reads out of bounds.
- **Self-lighting.** Nothing excludes a dynamic object's own voxels from its cone trace; only the start distance
  `bias` (and `normal_bias`, default 0) keep a surface from sampling itself (gi.glsl:487-488, :538).
- **Debug LIGHT view.** Confirmed no-op (vgdebug.glsl:94): it shows the stored texel, darker than the traced light by
  `dynamic_range` (2 by default), and without `energy` and exposure.

Still open: the bias the shader gets after a save and load (1.4 vs 1.5, §2): to test in the lab's port only if the
story names the number.

## 8. After the reviews (2026-10-31)

The chapter reviews and an independent fact check are in `reviews/`. Corrections the fact check made to this file's
picture: the bake can also run at run time; on Mobile the light passes still run but nothing reads them; the mip pass
is a weighted blend (sum / mix(8, count, propagation)), not an average, at the default propagation; the reflection
cone's angle from roughness is a half-angle (72 degrees at roughness 0.9); only dynamic range, propagation, two
bounces, and new data bump the version (gi.cpp:188, :256, :271, :344); "seventy-two dispatches" is 6 x (4 + mips) for
128 cells on the longest axis. Lab measurements used by the story: 42,289 leaves of 56,376 cells; the worked leaf
(63, 6, 29) of the teapot; the bounce's worked cell's last cone, 11 steps, alpha 0.64; the brightest leaf's light 3.3
(so it clips at a dynamic range of 1 or 2).
