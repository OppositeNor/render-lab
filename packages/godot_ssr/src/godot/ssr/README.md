# Godot SSR

Screen-space reflections of Godot 4.7.2-stable's Forward+ renderer, ported to WGSL (`shaders/ssr_passes.wgsl`,
`shaders/ssr_shading.wgsl`) and mirrored on the CPU (`trace.rs`). The research and the story's plan are in
`../../../story/`; every claim cites the Godot source through `source-index.json` (written by `source_index.py`).

## The passes

In Godot's order (gpu.rs): the depth pre-pass writes depth and the normal-roughness buffer (best-fit normals, the
roughness byte); the half-size downsample keeps the nearest of each 2 x 2; the Hi-Z pyramid keeps the nearest of
four, one dispatch per level; the trace marches one ray per half-size pixel through it, validates the hit, and reads
last frame's colour; the roughness filter blurs each level from the one before; the resolve goes back to full size;
the opaque pass blends the result into the indirect specular; and after it the frame is kept for the next frame.

## Simplified

Where the lab differs from Godot:

- The lab's depth is 4x multisampled; the downsample and the resolve read its first sample (Godot resolves it).
- The normal-roughness buffer is multisampled too, and read at its first sample.
- The lab's objects are static, so no roughness byte carries the dynamic flag.
- The sky's reflection reads the lab's environment, box-filtered mips blended by `sqrt(roughness) * 7`, where Godot
  reads its radiance octmap's GGX-filtered roughness layers; there are no reflection probes or GI buffers in the room.
- The environment BRDF is Karis's analytic fit (2014), where Godot reads its DFG table.
- Godot's ambient light is the sky's irradiance; the lab's ambient term divided by pi stands in for it, in the
  specular occlusion too.
- Where Godot compiles odd-size variants of the downsample and Hi-Z shaders, the port reads a flag.
- Only the default half-size path runs; the full-size path is described, not run.
- A frame drawn alone (a recording, a snapshot) first draws the same shot four times, so its trace reads a last
  frame as a still camera's would; Godot's first frame reads a cleared texture.
- The demonstrations set the floor to a polished metal and, until the story brings it in, turn the sky's reflection
  off, so the reflections SSR adds stand out; Godot keeps the sky (or a probe, or GI) as the base it blends over.
- The lab has no transparent pass; Godot copies the frame after it, the lab after the opaque pass.
- The lab renders at its own size: the SSR size and mip count are the lab's, not 1080p's.
