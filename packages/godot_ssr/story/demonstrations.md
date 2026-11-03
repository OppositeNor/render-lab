# Demonstrations: how each structure is stored, shown, and explored

A working file (deleted when the story is finished). Phase 3 of the story: for every structure the outline
explains, how Godot stores it (research.md), how the lab shows it (mirroring that storage), on which subject, and
what a viewer can do with it. Values named here are dock values a story can set too.

## 1. How Godot stores it

From research.md (sections 0.2, A.2, A.3, C, E.5, F); every row a separate resource in Godot.

| Structure | Godot resource | Format | Size, layout | Written by | Read by |
|---|---|---|---|---|---|
| Depth | the depth buffer | D32 or D24 (device) | internal size | pre-pass | Hi-Z mip 0 (copy or downsample), resolve |
| Normal-roughness | `RB_TEX_NORMAL_ROUGHNESS` | R8G8B8A8_UNORM: rgb best-fit normal * 0.5 + 0.5, a roughness * 127/255 (1 - that for dynamic) | internal size | pre-pass (scene.glsl:3019-3028) | downsample, resolve |
| Half normal-roughness | `RB_NORMAL_ROUGHNESS` (SSR scope) | R8G8B8A8_UNORM, the winning texel copied unchanged | SSR size | downsample | trace, resolve |
| Depth pyramid | `RB_HIZ` | R32_SFLOAT, reversed-Z device depth, each texel the max (nearest) of its 2x2 (3 wide at odd sizes) parents | SSR size, mips until a side is 1 | downsample (mip 0), Hi-Z passes (mips 1+) | trace, resolve (mip 0) |
| Reflection colour | `RB_SSR` | R16G16B16A16_SFLOAT: rgb last-frame colour * validity, then / (1 + luma); a = validity | SSR size, same mip count | trace (mip 0), filter (mips 1+) | resolve |
| Blur level | `RB_MIP_LEVEL` | R8_UNORM, mip / 14 | SSR size, 1 mip | trace | resolve (half), forward shader (full) |
| Full-size result | `RB_FINAL` | R16G16B16A16_SFLOAT, linear (un-compressed), premultiplied by validity | internal size | resolve | forward shader |
| Last frame | `RB_LAST_FRAME` | R16G16B16A16_SFLOAT, HDR internal colour after transparents | half size (SSR only) | the copy at the end of the frame | trace |
| Last camera | `ssr_last_frame_projections`, `_transform` | CPU matrices | per view | `_process_ssr` | the reprojection matrix (UBO) |

Godot's own debug views: none for SSR (research G.4: no `VIEWPORT_DEBUG_DRAW_*` mode shows it). The normal buffer
has one (`VIEWPORT_DEBUG_DRAW_NORMAL_BUFFER`), the internal buffer another. So every SSR view in the lab is the
lab's, mirroring the resources above; the narration says so once (chapter 1) and each view's title names the
Godot resource and format.

## 2. Mirror the storage

- Each resource above is its own view, titled `<Godot name> · <format> · <size>`, side by side where the story
  compares them, the same picked pixel outlined in each: depth and normal-roughness (2 views); the pyramid (one view
  per level, or a level slider on one view, since a level is a mip of one resource); `RB_SSR` (a view with a level
  slider and the channel decoding: rgb un-compressed and divided by alpha, or alpha alone); `RB_MIP_LEVEL` (a ramp,
  the value times 14); `RB_FINAL`; last frame.
- Packed channels are decoded in one view, not split into separate resources: normal-roughness shows the normal
  (rgb) or the roughness (a, unfolded) by a channel switch, as one resource; `RB_SSR` shows colour or validity.
- The validity terms of chapter 5 are computed in one shader in Godot and stored only as their product (alpha). The
  lab shows each term as its own view of the trace's pixels, titled as the lab's decomposition (the shader's terms
  in order), not as Godot resources.
- Where the lab differs (research 0.4): the depth is the lab's multisampled depth resolved to one sample for the
  pyramid; the mip level may be R32F where R8 storage is not available; the lab's SSR size is its own. Each view's
  title carries the lab's format where it differs from Godot's.

## 3. Subject

The study room's SSR copy with a glossy floor (roughness a dock value, `floorRoughness`, 0 to 1, default 0.15) and
the teapot as the reflected object: its curved silhouette shows the resolve's edges, its shading shows the colour
coming from last frame. The picked pixel P sits on the floor in front of the teapot; Q sits on the floor near the
image edge, its ray leaving the screen. Close-ups fade the walls that hide the floor from a low camera
(`wallFade`). A glossy-wall toggle (`wallRoughness`) gives the facing-mirror loop of chapter 5; a teapot motion
(`teapotMotion`) gives the one-frame lag.

## 4. Interactions, view by view

| View | Click or value | What it shows |
|---|---|---|
| The scene (3D) | click a floor point (`Investigation::click`) | picks the pixel: `pickX`, `pickY` (SSR-target pixel); its ray drawn in 3D (start, bias, reflected direction, hit), its texel outlined in every screen view, a link line from each view's outline to the 3D point |
| Depth / normal-roughness | click a texel | picks it (same values); readout of the four bytes and their decode (normal, roughness, dynamic flag) |
| Pyramid | `hizLevel` 0..N-1; click a texel | the level; the clicked texel's footprint outlined in level 0 and the nearest pixel inside it marked |
| Trace stepper | `traceStep` 0..(the picked ray's iteration count); `ssrMaxSteps` | the ray at that step: its cell on its level (outlined on the screen grid and on the pyramid view), `t`, the cell's depth against the ray's, hit or miss, the level graph so far; the 3D ray up to `t` |
| Lanes | the picked pixel's 8x8 group | 64 bars of iteration counts (from the port's counter), the group's longest bar marked; `floorRoughness` changes them |
| Validity terms | `validityTerm` 0..6 | the term views multiplied up to that term; the picked pixel's value per term |
| Fade curve | `ssrFadeIn`, `ssrFadeOut` | the curve over screen-space ray length, its peak (computed), the picked ray's length marked on it |
| Reprojection | `freeCameraOn`; `teapotMotion` | both cameras' frusta, the hit, its point in last frame's image (inset with the sample marked) |
| Reflection colour | `ssrLevel` 0..N-1; `ssrChannel` colour or validity | the filtered level; the picked pixel's texel and the taps of the 7-tap filter that made it |
| Blur level | `floorRoughness` | the mip-level ramp; the cone and its circle for the picked ray (2D chart) |
| Resolve | click a full-size pixel on an edge | its four half-size taps, each tap's weights as bars (depth, normal, roughness, bilinear), the result |
| Composition | `ssrComposeStep` 0..7 | the indirect specular after each stage (sky, GI, probes, occlusion, SSR blend, BRDF scale) at the picked pixel, and the whole frame at that stage |

A pick round-trips: clicking the 3D point that a picked pixel's outline stands for picks that same pixel (a
selftest check).

## 5. A demonstration for every taught topic

| Topic | Worked example (step values) | Explore | Sweep that shows the relation |
|---|---|---|---|
| `gpu-frame` | the Hi-Z pass writes `RB_HIZ`, the trace reads it (the DAG panel, two cards) | click a card: its reads and writes | toggle `ssrHalfSize`: the downsample and resolve cards appear and disappear |
| `rt-gbuffer` | P's four bytes decoded (`packStep` 0..3) | click any texel | `normalPackingBestFit` 0..1: the angular error readout |
| `rt-fullscreen` | P's thread in its 8x8 group (`threadStep`) | click a pixel: its group and thread index | `ssrHalfSize`: the grid halves |
| `rt-reconstruct` | P: screen, NDC, view position (`reconstructStep` 0..3); its eight neighbours and the kept side (`normalStep` 0..3) | click a pixel | a depth edge moved under the pixel (`edgeOffset`): the kept side switches |
| `rt-depth-raymarch` | a linear march (`linearStep`, the lab's own, labelled) then the Hi-Z trace (`traceStep`) | click a pixel: its ray | `ssrDepthTolerance` over a thin rail; `ssrMaxSteps` |
| `gpu-divergence` | P's group's bars | click another group | `floorRoughness` across 0.7 |
| `rt-reprojection` | the hit through the matrix's four steps (`reprojectStep` 0..4) | free camera | `teapotMotion`: the lag; a camera cut (`cameraCut`): the wrong colour |
| `rt-bilateral` | the edge pixel's four taps (`resolveStep` 0..4) | click another pixel | `resolveDepthFactor` (the lab's demonstration value, 0 to Godot's 2048) |

## 6. Build and check

Each view is built with the base (Phase 4 provides the passes the views read), then checked before the storyboard:
offscreen stills of every view (`home --snapshot --page viewer:<demo>` and `viewer-inspect:<node>.<resource>`),
looked at; the pick round-trip in the selftest; every value above in `render-lab --investigation godot/ssr
values`. The demonstrations (`demos/*.toml`):

| Demo | Chapter | Shows |
|---|---|---|
| `room` | 1, 7, 8 | the glossy room, SSR on, the frame graph |
| `inputs` | 2 | depth and normal-roughness panes, the picked pixel |
| `pyramid` | 3 | the pyramid levels |
| `trace` | 4 | the stepper, the lanes |
| `hit` | 5 | validity terms, fades, reprojection |
| `blur` | 6 | blur level, filter levels, resolve |
| `compose` | 7 | the composition steps, the performance panel |
