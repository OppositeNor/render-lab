# Outline: How Godot's SSR Reflects a Room

A working file (deleted when the story is finished). Part 1 of a planned story of Godot's indirect lighting
(SSR, then SDFGI, then VoxelGI; the parts structure is proposed in the investigation's guide and not built yet), so
this part teaches the screen-space and frame topics the later parts recall. Sources are `research.md` sections:
`A.2.4` is section A ("Setup, inputs, buffers"), its question 2.4; `0.3#1` is the cross-check's finding 1. Every
number on screen comes from the lab (the port's own counts, the selftest, the GPU timer), never typed; numbers
quoted below from the research are Godot's at 1920x1080 and are said as Godot's, or replaced by the lab's.

Engine: Godot 4.7.2-stable, `ed1daf0b`. Path followed: the default half-size path through every pass; the full-size
path gets one beat (chapter 6). Subject: an SSR copy of the study room with a glossy floor (roughness swept from 0
to the 0.6-0.7 cutoff), the teapot the worked examples' object.

## The worked thread

One floor pixel, **P**, in front of the teapot, whose reflected ray hits the teapot's side. It is picked in
chapter 2 and followed through every stage: its depth and normal-roughness texel (2), its cells in the pyramid (3),
its ray, cell by cell (4), its hit's validity and colour (5), its blur level and its four resolve taps (6), and its
final indirect specular (7). Chapter 8 walks P back through every stage in frame order. A second pixel, **Q**,
whose ray leaves the screen, is the counter-example where a chapter needs one (4, 5).

## The architecture map

The frame graph's 2D scene (`graph=frame`), Godot's Forward+ frame as the lab ports it. It is drawn to be shared by
the later parts (the SDFGI and VoxelGI passes sit in it, dashed in this part); until the story package exists, this
package carries it in `graphs/frame.toml`.

| Section | Passes (the port) | Godot |
|---|---|---|
| Pre-pass | `ssrPrepass` (depth + normal-roughness) | depth pre-pass, `PASS_MODE_DEPTH_NORMAL_ROUGHNESS` (rfc.cpp:1925-1940) |
| GI (dashed: parts 2 and 3) | (SDFGI, VoxelGI gather) | `_pre_opaque_render`: `process_gi` (rfc.cpp:1607) |
| Screen space | `ssrDownsample`, `ssrHiz` (per mip), `ssrTrace`, `ssrFilter` (per mip), `ssrResolve` | "SSR Downsample", "SSR HI-Z", "SSR Main", "SSR Roughness Filter", "SSR Resolve" (sse.cpp:1503-1722) |
| Opaque | `ssrForwardFS` | "Render Opaque Pass", scene.glsl:2197-2228 |
| Transparent (dashed: the room has none) | | rfc.cpp:2410-2423 |
| History | `ssrCopyLastFrame` | "Copy Framebuffer for SSIL/SSR" (rfc.cpp:2450-2453) |

A back arc from History to the trace: last frame's colour, read this frame (the one-frame loop, 0.3#9, A.5). Each
chapter opens on the map with its section lit and ends on what its output feeds.

## Knowledge level

Audience (`knowledge.toml [audience]`): has written a PBR forward renderer with shadow maps and compute passes, the
profile godot_sdfgi uses, minus what this part now teaches for the parts after it. Known: `gpu-formats`,
`gpu-compute`, `gpu-sync`, `gpu-passes`, `rt-spaces`, `rt-depth`, `rt-sampling`, `rt-cubemaps`, `rt-blend`,
`cg-brdf`, `cg-fresnel`, `cg-microfacet`, `cg-ggx`, `cg-pbr-materials`, `cg-lights`, `cg-visibility`.

| Topic | Status | Taught in | Beats | Seconds |
|---|---|---|---|---|
| `gpu-frame` (a frame as passes ordered by what they read and write; a history resource; on the shared Godot frame graph) | not known; moved here from godot_sdfgi chapter 11 | 1 | c1-graph, c1-history | 55 |
| `rt-gbuffer` (a pre-pass that writes what later passes need; packing a normal and a roughness into eight bits each) | not known; moved here from godot_sdfgi chapter 15 | 2 | c2-prepass, c2-pack | 60 |
| `rt-fullscreen` (a compute pass over a render target, one thread per pixel, eight by eight; a half-resolution pass) | not known; moved from godot_sdfgi chapter 15 | 2 | c2-threads, c2-half | 40 |
| `rt-reconstruct` (a pixel's view position from its depth; a normal from neighbouring depths) | not known; moved from godot_sdfgi chapter 10 (built: see "Changes to the other parts") | 2 | c2-position, c2-geonormal | 70 |
| `rt-depth-raymarch` (a ray stepped through the depth buffer, the thickness test; hierarchical steps through a depth pyramid) | not known | 3, 4 | c3-why, c3-max, c4-linear, c4-climb | 130 |
| `gpu-divergence` (**new topic** in render_lab's graph, planned by godot_sdfgi's outline: lanes of a group run in lockstep; the slowest lane sets the time; requires `gpu-compute`) | not known; moved here from godot_sdfgi chapter 14 | 4 | c4-lanes | 40 |
| `rt-reprojection` (reading last frame's image where a point was; camera-only versus motion vectors; disocclusion) | not known | 5 | c5-where, c5-limits | 60 |
| `rt-bilateral` (a weighted blur; joint bilateral upsampling of a half-size result) | not known | 6 | c6-resolve, c6-weights | 50 |
| `rt-ssr` | the feature | 1-8 | | |
| `gpu-amortize`, `rt-motion-vectors` | level: recalled in a sentence (c5-where: "Godot reuses last frame's image, and reads no motion vectors") | | | |
| `rt-reflection-probes`, `rt-split-sum` | level: recalled in a sentence each (c7-before: what SSR falls back to; c7-brdf: the scale applied after it) | | | |

Taught topics: 505 s of the 965 s budget. `rt-tonemapping` is not needed: the tone compression of chapter 5 is
explained in place, as a filter trick, not as tone mapping. Check with `render-lab --investigation godot/ssr
knowledge --known …` once the package exists.

## Chapter 1 | Where SSR runs | 130 s

Opening (no new terms): the floor with SSR off and on, same camera and exposure, the cue on what it adds: the
teapot and the lit wall appear in the floor.

| Beat (claim headline) | Claim, source | On screen |
|---|---|---|
| c1-adds: SSR puts the room into the floor | G.6, 0.3#1 | `ssrEnabled` 0 -> 1 on the word; a wipe (`ssrWipe` 0 -> 1) across the floor; the camera drifts low over the floor |
| c1-problem: a probe cannot see the teapot | G.1 rows 1, 6 | the same pixel with only the sky/probe reflection: an inset of the probe's view without the teapot; question: "how can the floor reflect what is on screen without tracing the scene?" |
| c1-learn: what the viewer will be able to explain | | a short list beside the narration: the depth pyramid, a ray marched through it, where its colour comes from, how it is blurred and blended; known: a PBR forward renderer (the Prerequisites tab) |
| c1-map: the passes in Godot's frame | 0 (frame order) | `graph=frame`, the screen-space section lit, its passes named in turn as the sentence names them |
| c1-graph (teach `gpu-frame`): passes are ordered by what they read and write | 0, O.7 | question: "which pass may start first?"; worked example on two passes: the Hi-Z pass writes `RB_HIZ`, the trace reads it: an arrow from writer to reader on the resource's card (`panelDag`); then all six, ordered by their arrows |
| c1-history: last frame's colour closes a loop | 0, A.5.1 | the History section lights; the back arc pulses; frame N-1 and N side by side as two copies of the strip; the reader comes before the writer only because it reads a copy kept across frames (`history` resource) |
| c1-settings: what switches it on | 0.1, U.1, U.2 | the settings card: `ssr_enabled` off by default, max steps 64, fade in 0.15, fade out 2.0, depth tolerance 0.5, half size true; Forward+ only; never in reflection probes or with a transparent background |
| c1-recap, c1-sources | | |

Detailed demonstration: the DAG of the frame with its writer -> reader arrows and the history arc (`panelDag`,
the lab's graph panel, existing).

## Chapter 2 | Depth, normals, and one thread per pixel | 130 s

Bridge: "Before any ray is traced, the frame needs two images of what is on screen: how far, and which way it faces."

| Beat | Claim, source | On screen |
|---|---|---|
| c2-prepass (teach `rt-gbuffer`): the pre-pass writes depth and normal-roughness | 0 step 3, A.2.1 | split view (`Investigation::panes`): depth (left, visualizer `depth`) and normal-roughness (right, `normal`), the lit frame below; the floor, the teapot, the walls named as each lights |
| c2-pack: a normal in three bytes, a roughness in seven bits | A.2.2, A.2.5, 0.3#7 | P's texel: four bytes shown; the normal's direction rebuilt from them (an arrow at P in 3D); best-fit normals: the vector scaled to the cube, then to its best length (sweep `normalPackingBestFit` 0 -> 1: the angular error readout falls); the alpha byte: roughness times 127/255, flipped above one half for moving objects |
| c2-threads (teach `rt-fullscreen`): one thread per pixel, in groups of eight by eight | O.7 | the SSR target as a grid; one 8x8 group outlined; P's thread lit; every SSR pass named with its dispatch size (from the lab) |
| c2-half: the trace runs at half size | O.1, C.2 | sweep `ssrHalfSize` 0 -> 1: the grid halves; one half-size texel over its 2x2 full pixels; the nearest of the four wins, and its normal comes with it (the winning pixel lit) |
| c2-position (teach `rt-reconstruct`): a pixel's depth gives back its position | D.1 | question: "where in the room is P?"; worked example: P's screen position and depth -> NDC -> the inverse projection -> a point in view space (readouts at each step, `record`); a marker lands on the floor in 3D |
| c2-geonormal: the true surface, from neighbouring depths | D.2 | P's four horizontal and four vertical neighbours; on each side, the depth extrapolated from two neighbours; the side with the smaller error is kept (sweep a depth edge under P: the kept side switches); the cross product of the two tangents; why: the normal map may disagree with the surface |
| c2-recap, c2-sources | | |

Lab difference said on screen: the lab's depth is multisampled and is resolved before the pyramid (research 0.4).

## Chapter 3 | The depth pyramid | 100 s

Bridge: "With every pixel's depth in hand, a ray could walk the depth image one pixel at a time; this chapter
makes that walk short."

| Beat | Claim, source | On screen |
|---|---|---|
| c3-why (teach `rt-depth-raymarch`, part 1): one pixel per step is a thousand steps | O.3 | P's ray drawn across the screen grid, pixel by pixel; a step counter counts up (the lab's count for P) |
| c3-max: each level keeps the nearest depth of four | C.3 | the Hi-Z mips side by side (`imageGraph "ssrHiz.hiz"`, `level` 0..N); one level-3 texel over its 8x8 pixels: the nearest (largest, reversed Z) wins; "nothing in this cell is nearer than this" |
| c3-levels: halve until a side reaches one | C.4, A.3.1 | the chain counted down from the lab's SSR size (the lab's mip count, not Godot's 10); sweep `hizLevel` through them |
| c3-odd: an odd size reads a third row | C.2, C.3 | a 5-wide level: the extra column shared by two parents; the footprint overlaps by one, so no pixel is skipped |
| c3-skip: a cell can be skipped when the ray stays in front of it | D.5 | P's ray over one coarse cell: the ray's depth against the cell's nearest depth; in front all the way: skip it |
| c3-recap, c3-sources | | |

## Chapter 4 | Marching the ray | 170 s

Bridge: "The pyramid says where a ray cannot hit; now P's ray uses it, cell by cell."

| Beat | Claim, source | On screen |
|---|---|---|
| c4-start: the ray starts just off the surface | D.2, D.4 | P's view direction reflected about its normal (arrows in 3D); the bias along the geometric normal (sweep the normal map's tilt: the bias length `1 - cos^8` grows); the bounce when the ray points into the surface |
| c4-screen: the same ray across the screen | D.4 | the end point one unit along the ray, projected; the screen-space ray with its depth change normalised to one (Supnik's parametric t: readout `t`, `depth`); the camera-plane clip (0.3#6); `t_max` where it leaves the screen |
| c4-linear (teach `rt-depth-raymarch`, part 2): a linear march, then the thickness test | D.5 | a schematic linear march first (the lab's own demonstration, labelled): steps until the ray's depth passes the depth image; a thin object: the ray passes behind it; the test: more than the tolerance behind is not a hit (sweep `ssrDepthTolerance` 0.1 -> 1.0 over a thin rail: hits appear behind it) |
| c4-climb: a miss climbs a level, a hit descends | D.5 | the engine's loop, `${code "ssrTrace"}`'s listing (`panelCode`), each line lit as P's ray takes it (`traceStep` 0 -> P's iteration count, a range over the sentence): the cell outlined on its level, the level graph 0,1,2,3,4,5,4,3... drawn as it goes; levels and `t` from the port |
| c4-hit: at level zero the hit is final | D.5 | the last descent; the tolerance test at level zero only; `t` stops at the cell entry (no refinement); the hit marked in 3D on the teapot |
| c4-exits: three ways out | D.5 | the counter-example Q: its ray reaches `t_max` and stops; a third ray runs out of its 64 iterations (sweep `ssrMaxSteps` 8 -> 64: it reaches its hit) |
| c4-lanes (teach `gpu-divergence`): the slowest lane sets the group's time | O.2, 0.3 | P's 8x8 group: 64 bars of iteration counts from the port (sky and rough lanes at zero, idle); the group's time = the longest bar; sweep the floor's roughness across 0.7: the bars of rough lanes drop to zero, the group's time does not until all do |
| c4-recap, c4-sources | | |

Detailed demonstration: the trace stepper (a step parameter over P's iterations; the cell, level, `t`, and
comparison shown per step on the screen grid and on its Hi-Z level, and the ray in 3D).

## Chapter 5 | Is it a hit, and what colour | 150 s

Bridge: "The loop has stopped somewhere; this chapter asks whether that is a reflection, and what colour it is."

| Beat | Claim, source | On screen |
|---|---|---|
| c5-validity: validity instead of a yes or no | E.1, E.2 | the validity terms as separate views of the SSR target, side by side, multiplied in order: off-screen or sky, back-face, depth confidence, margin, fades; P's value in each (readout) |
| c5-confidence: how far the ray ended from the surface | E.1 item 4 | P's end point and the surface under it, the distance between them; `1 - smoothstep(0, tolerance, d)` squared (formula card, terms lit) |
| c5-backface: the back-face test only near the start | 0.3#2 | the two-pixel zone around P outlined; a hit inside it facing away is rejected; a far one is not tested |
| c5-fades: the fades never reach one | 0.3#1, E.1 items 7-9 | the fade curve `L^0.15 (1-L)^2` drawn over screen-space ray length; its peak (the lab computes 0.58 at 0.07); sweep `ssrFadeOut` and `ssrFadeIn`: the peak moves; "at least four tenths of the probe's reflection always remains" |
| c5-where (teach `rt-reprojection`): the colour is read from last frame's image | A.4, A.5 | the hit reprojected: this frame's camera, last frame's camera (free camera on, `freeCameraOn`), the matrix read right to left, the point landing in last frame's image (inset of `ssrLastFrame`, P's sample marked); level recall: "Godot reuses last frame's image and reads no motion vectors" |
| c5-limits: what a camera-only reprojection gets wrong | A.4, O.4 | a moving teapot (`teapotMotion`): its reflection lags one frame; a disocclusion: a surface newly in view reads what was there before; the first frame after a resize is black |
| c5-loop: reflections of reflections, one bounce a frame | 0.3#9, A.5.1 | two facing glossy surfaces (a glossy wall toggled on, `wallRoughness`): frame by frame (`frameStep`), one more reflection appears in the reflection |
| c5-compress: the colour is compressed before it is blurred | E.3 | P's colour premultiplied by its validity; `c / (1 + L)` and its inverse (formula card); a bright highlight blurred with and without it (toggle `ssrCompress`: the halo shrinks) |
| c5-recap, c5-sources | | |

## Chapter 6 | Blurring by roughness | 130 s

Bridge: "A mirror needs P's one colour; a rougher floor needs it spread, and the next passes do that."

| Beat | Claim, source | On screen |
|---|---|---|
| c6-cone: roughness becomes a cone, the cone a circle | E.4 | in 2D: the cone from P along the ray (half-angle = roughness times 90 degrees), the circle inscribed where it ends; sweep the floor's roughness 0 -> 0.69: the circle grows; the circle's radius in pixels -> a mip level (formula card: `log2(r * max(w,h) / 16 + 1)`, the 16 said as Godot's unexplained constant) |
| c6-miplevel: the level, per pixel | E.4, E.5 | the mip-level target (`RB_MIP_LEVEL`, R8 as mip / 14) as a view: dark near contacts, brighter with distance and roughness; P's value |
| c6-filter: each level blurs the one before | F.2 | the SSR colour's mips side by side (`imageGraph "ssrFilter.ssr"`, `level`); one output texel: a 2x2 average fetched at the corner, then seven taps across and seven down (weights shown, sigma 2.02 as Godot's weights give it); invalid texels weigh 0.2 per level |
| c6-resolve (teach `rt-bilateral`): back to full size, without bleeding across edges | F.3 | question: "how does a half-size result go back to full size without blurring across the teapot's edge?"; worked example at a full-size pixel on the teapot's silhouette: its four half-size taps, each tap's depth, normal, and roughness weights as bars, the bilinear weights, the product; the far tap's weight goes to zero |
| c6-weights: three similarities and a distance | F.3 | formula card `exp(-2048 |dz|) exp(-32 (1 - n.n')) exp(-16 |dr|)`, terms lit; sweep the depth factor (`resolveDepthFactor`, the lab's demonstration value, labelled): the edge bleeds at 0 |
| c6-full: at full size the forward shader picks the level | F.4, 0.3#3 | toggle `ssrHalfSize` 1 -> 0: no resolve pass in the map; the opaque pass reads the mip level and samples the pyramid itself; the flag named `RESOLVE_SSR` is set for this path |
| c6-recap, c6-sources | | |

## Chapter 7 | Into the final image | 110 s

Bridge: "P now has a blurred colour and a validity at full size; the opaque pass decides how much of it the floor shows."

| Beat | Claim, source | On screen |
|---|---|---|
| c7-before: what the floor had before SSR | G.1 rows 1-7 | the indirect specular built up in order (`ssrComposeStep` 0 -> 7): sky, GI buffers (dashed), probes, specular occlusion; level recall of reflection probes |
| c7-blend: SSR covers it by its validity | G.2, 0.3#1 | `I (1 - a) + c` with P's numbers (readouts); sweep P's alpha via `ssrFadeOut`; the 0.6-0.7 roughness band fading SSR out (sweep the floor's roughness across it) |
| c7-brdf: then Fresnel, like any probe | G.3 | the envBRDF scale after the blend (level recall of the split sum); grazing camera (`cameraPitch` low): the reflection strengthens |
| c7-cost: what it costs, pass by pass | O.1-O.7 | the performance panel (`panelPerformance`), the trace and the resolve called out; sweep `ssrHalfSize`: the trace's time falls by about four (the lab's numbers); what is skipped (sky, rough, off-screen rays) and what is not (the filter and resolve run over every pixel, O.8 F2, F4) |
| c7-recap, c7-sources | | |

## Chapter 8 | The frame again | 45 s

The full room; P followed back through every stage in frame order on the map (pre-pass texel, pyramid cells, ray,
validity and colour, blur level and taps, blend); each mechanism switched off and on in frame order. Three
takeaways: the data (a depth pyramid and last frame's colour, nothing else of the scene), the computation (one ray
per half-size pixel, its colour reused, blurred by a mip chain), the trade (what leaves the screen or moved is lost,
so SSR only ever covers part of the probe's reflection).

## Changes to the other parts

godot_sdfgi (its guide records the GPU part as outlined, not built; chapters 1-10 are built):

- chapter 11 (`c11-graph`, `c11-levels`): recall `gpu-frame` in a sentence; draw its frame on the shared Godot frame
  graph once the story package exists, with the SDFGI section lit (the user's question "could they share render
  graphs"; open).
- chapter 14 (`c14-lanes`): recall `gpu-divergence`; keep the shadow-march bars as its own example, shorter.
- chapter 15 (`c15-gather`): recall the pre-pass's normal-roughness and one thread per pixel.
- chapter 10 (built): `rt-reconstruct` becomes a recall; the beats that teach it are shortened, and the chapter's
  video rebuilt. Decide with the user when; until then the topics are in both stories.
- `knowledge.toml [audience] known` gains the topics this part teaches.

## What changes, and what it changes (sweeps)

| Relation | Parameter | Range | What must visibly change |
|---|---|---|---|
| rougher floor, blurrier reflection | floor roughness | 0 -> 0.69 | the circle in c6-cone; P's mip level; the reflection's blur |
| the 0.6-0.7 band fades SSR out | floor roughness | 0.55 -> 0.75 | the reflection fades into the probe's |
| more steps reach farther hits | `ssrMaxSteps` | 8 -> 64 | the third ray reaches its hit; the iteration bars |
| tolerance trades leaks for holes | `ssrDepthTolerance` | 0.1 -> 1.0 | hits behind a thin rail appear |
| the fades cap validity | `ssrFadeOut`, `ssrFadeIn` | 0.5 -> 4, 0 -> 0.5 | the curve's peak; P's alpha |
| half size quarters the trace | `ssrHalfSize` | 1 -> 0 | the grid; the trace's GPU time; the resolve pass in the map |
| grazing view, stronger Fresnel | `cameraPitch` | high -> low | the reflection's strength |

## The tools it needs

- **Mip chains on 2D targets or program images**, written per mip by compute (a generic lab extension: research
  0.4); and the inspector's `level` on them.
- **A history target** for last frame's colour, and **last frame's camera** in the investigation's plan state.
- **A pre-pass output** packed as Godot packs it (best-fit normals, the roughness byte), and the best-fit table.
- **The trace stepper**: a step parameter over one ray's iterations, a CPU mirror of the loop that reports each
  step (cell, level, `t`, comparison), and overlays on the screen grid, the Hi-Z level, and the 3D ray.
- **A picked pixel** (`Investigation::click` on the floor) shown everywhere it lives: in 3D, in every screen view,
  in each Hi-Z level, with link lines between them.
- **Lane bars**: an image of one group's 64 iteration counts from the port.
- **Validity views**: each term as its own view of the SSR target (one view per term, as Godot computes them in one
  shader; labelled as the lab's decomposition).
- **Charts**: the fade curve, the cone and its circle, the resolve weights.
- **Scene**: a glossy floor (roughness a dock value), a glossy wall toggle, a moving teapot.

## Worth a detailed demonstration

- the trace, step by step on one ray (chapter 4);
- the Hi-Z level over its pixels, and the cell a ray is in (chapter 3);
- reprojection with two cameras (chapter 5);
- the resolve's four taps and their weights at an edge (chapter 6);
- the indirect specular built up in order (chapter 7).
