# Godot Engine: SDFGI

How Godot's Forward+ renderer computes **signed distance field global illumination**, read from the Godot source
(4.7.2-stable, the commit in [`source-index.json`](../../../source-index.json)) and followed in the lab at the engine's
own sizes: four cascades of 128³ cells (0.2 m cells in the first), 17³ probes per cascade, eight rays per probe per
frame, and a history of 30 frames. The study room is voxelized, flooded, lit, and traced on the CPU by
[`voxels.rs`](voxels.rs), [`occlusion.rs`](occlusion.rs), [`light.rs`](light.rs), [`probes.rs`](probes.rs), and
[`sim.rs`](sim.rs), each following the engine's shader or C++ function line by line; the scene pass shades the room
with [`shaders/sdfgi_shading.wgsl`](../../../shaders/sdfgi_shading.wgsl), a WGSL port of the screen pass
(`sdfgi_process`), reading the probe maps of that CPU world. [`shaders/sdfgi_passes.wgsl`](../../../shaders/sdfgi_passes.wgsl)
ports the light pass (with the area light's LTC integral), the probe rays, and the probe store for the selftest, which
compares them with the CPU model.

## What the demonstrations show

| Key | Demonstration | Godot code it follows |
|---|---|---|
| 1 | The room with SDFGI | `RenderForwardClustered::_render_scene`, the Environment's SDFGI settings |
| 2 | Cascades that follow the camera | `GI::SDFGI::create`, `SDFGI::update` |
| 3 | The dirty slab, its geometry, and its lights | `get_pending_region_data`, `RendererSceneCull::_scene_cull`, `render_static_lights` |
| 4 | Voxelizing the room | `RenderForwardClustered::_render_sdfgi`, `MODE_RENDER_SDF` (scene_forward_clustered.glsl) |
| 5 | The distance field | sdfgi_preprocess.glsl: the jump flood and `MODE_STORE`, as `render_region` schedules them |
| 6 | A probe's occlusion | sdfgi_preprocess.glsl `MODE_OCCLUSION` |
| 7 | Light in the voxels | sdfgi_direct_light.glsl (dynamic and static passes), `update_light`, `pre_process_gi` |
| 8 | A probe traces the field | sdfgi_integrate.glsl `MODE_PROCESS`, `update_probes` |
| 9 | A probe's octahedral maps | sdfgi_integrate.glsl `MODE_STORE`, `store_probes` |
| 0 | Shading a pixel | gi.glsl `sdfgi_process`, `sdfvoxel_gi_process` |

Each demonstration has a **readout** in the struct panel's place: the record it shows, field by field with its bits,
or the values of its step, computed by the CPU model. The **step parameters** (in the dock) walk the camera, run the
voxelizer's passes one at a time, step the jump flood, grow the occlusion shells, choose which quarter of the cells a
frame lights, scroll a static light, add frames of rays, and move the inspected pixel.

## Findings

Behaviours that look like engine bugs, read from the code and, where the lab can, measured on its CPU model (the
selftest reproduces F1, F2, F4, F26, and F27); all of them are listed with their evidence in
[`story/research.md`](../../../story/research.md):

- Static light is added again to the kept cells after every scroll it overlaps (F1).
- Static-light lists are stored by cascade number and read by position in the dirty list (F2).
- The probes' irradiance is about 4/π too bright under a uniform sky (F4); the octahedral maps are written at texel
  corners and read at centres (F5); fog never applies the y scale (F6).
- The 75 % y scale is a factor of 1.5 (two thirds of the height, F3), and light distances are measured in that squashed
  space (F27); area lights are injected at twice the forward pass's radiance (F26).
- The default ray count is 8, not the 16 its setting names (F7); probes update only inside the depth pre-pass (F8).
- In 4.7.2-stable the probe shader's sky define does not match its binding (F25, fixed in the 4.8 beta).

## What the lab simplifies

- The voxels are found by tracing each pixel of the engine's three orthographic views through the room's shapes,
  with exact normals, not by rasterizing triangles; writes the GPU orders by scheduling follow the lab's loop order.
- SDFGI follows a camera that stays at the room's centre; the viewer's camera only looks. The world is run 60 frames
  from creation at the default settings; the screen pass reads its maps.
- The screen pass is evaluated per fragment of the scene pass, without the specular march below roughness 0.2.
- The ceiling panel stands for a dynamic AreaLight3D (range 10 m, attenuation 2, energy chosen so Godot's forward
  pass would match the lab's direct light), and its mesh is left out of the voxels.
- Textures are sampled with full-precision weights; the probes' sky is the lab's environment at full resolution
  (Godot reads mip 2); `hashf3`'s `sin` is evaluated in f32 on the CPU, so a probe's ray rotation differs from a GPU's.
- Occlusion is recomputed over a whole cascade after a scroll, where Godot recomputes only the windows a new slab
  touches; records are shown as readouts, not in the struct panel.

## Files

| File | What it is |
|---|---|
| `mod.rs` | The `Investigation`, its scene shading |
| `model.rs` | The cascades: create, update, the dirty regions |
| `voxels.rs` | Voxelization, the jump flood, STORE, scrolling |
| `occlusion.rs` | The occlusion pass and its packing |
| `light.rs` | The light pass, the area light's LTC integral, the shadow march |
| `probes.rs` | The probe rays, the SH history, the octahedral store, the probe scroll |
| `sim.rs` | Frames in the engine's order, the static-light cull, the converged world |
| `screen.rs` | The screen pass (the WGSL port's CPU mirror) |
| `demos.rs`, `shot.rs`, `controls.rs`, `values.rs` | The demonstrations, their values, the dock |
| `selftest.rs` | The checks |
| `source_index.py` | Writes `source-index.json` from the checkout |
| `AGENTS.md` | The investigation's guide for agents |

## Verify

```sh
cargo run --release -- --investigation godot/sdfgi selftest --report /tmp/sdfgi.json
python3 packages/godot_sdfgi/src/godot/sdfgi/source_index.py
python3 tools/video/build.py --investigation godot/sdfgi --snapshots-only
python3 tools/video/build.py --investigation godot/sdfgi
```
