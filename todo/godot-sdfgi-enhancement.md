# Godot SDFGI video: the enhancement (plan, not started)

A viewer's review of the zh-CN edition (about 18 min 20 s) asked for the story to explain, not only show, what
SDFGI's structures are for. Its general lessons are now in the skills (`new-story`: "The shape of a story", Phase 5
to 7, and "Revising a finished story"; `new-investigation` §2 step 8). This is the Godot-specific work, to run with
"Revising a finished story": improve the story, don't remake it.

## Where it applies

The review's timestamps are approximate; the zh-CN edition's chapters are:

| zh-CN time | Chapter (`packages/godot_sdfgi/video/storyboard.story`) | Review section |
|---|---|---|
| 00:00 | 1 Where SDFGI runs | Introduction and overview |
| 02:28 | 2 Cascades that follow the camera | Cascades and spatial organisation |
| 04:21, 05:50 | 3 What is voxelized; 4 Voxelization | Voxels and SDF construction |
| 07:33, 09:41, 10:25 | 5 The distance field; 6 Occlusion; 7 Light in the voxels | Voxel data, occlusion, and code |
| 12:42, 14:30 | 8 Probes trace the field; 9 From coefficients to maps | Probes, directional light, SH |
| 15:54, 17:42 | 10 Shading a pixel; 11 The frame again | Updates, cascades, final rendering |

## Keep

The transparent cascade boxes, the voxel views, the probe placement, the debugging pictures, the worked numbers,
the code that is correct, and the one study room throughout.

## Changes, by chapter

1. **Opening.** Start on a matched comparison (same camera and exposure, `sdfgiMix` 0 → 1) cued on the colour
   bleeding; state the problem (bounced light without tracing many rays per pixel); show the architecture map
   (below); say what the viewer will be able to explain. Fewer new terms here.
2. **Cascades.** Say why several scales: near detail, far coverage, a fixed budget of cells. Show the camera,
   the cascade boxes, and the room together while the camera moves, with the update the engine really does.
   Keep cascade regions and the cells inside them visibly distinct. Recap: the cascades decide where SDFGI keeps
   its data, and spend a fixed budget across distance.
3. **Voxels and the field (3 to 5).** Say what a voxel holds here, how the distance field is built from the
   voxels, what the field means geometrically, how it makes ray marching cheap, and what the cell size loses.
   Work it on the sphere alone first, then the room. Give occupancy, distance, occlusion, and light their own
   colours, and never draw them as interchangeable.
4. **Occlusion and light in the voxels (6, 7).** One worked cell, lit in the room: its inputs, the calculation
   step by step, its result, and where the result goes. Code: only the lines that matter, larger, the active
   line lit with its input and output values beside it, labelled engine source, pseudo-code, or lab
   demonstration. One panel at a time.
5. **Probes and SH (8, 9).** One probe throughout. (A) the light it gathers, the rays as the engine traces
   them; (B) why compress it, and SH explained in plain words before any coefficient; (C) how it is read back
   when shading. Say what is kept, what is lost, and why it pays. Verify every SH step against the source.
6. **Shading a pixel, and the end (10, 11).** One shaded point on a visible surface, followed back through what
   it reads: which probes and cascade, how they are found and weighted, how occlusion enters, what light comes
   out. End on the whole room with each mechanism's contribution, and three takeaways (how SDFGI organises its
   data, how it computes and reuses light, what accuracy it trades for speed).
7. **Throughout.** A recap after each chapter (what, why it matters, what is next), one idea per moment,
   readable at laptop and phone size, and longer chapters where that buys understanding.

## The architecture map

One Mermaid diagram (`video/diagrams/`), true to Godot 4.7.2: cascades, voxelization and the distance field,
occlusion, light injection and the bounce from the probes back into the voxels (a feedback loop, not a line),
probe tracing and their history, and the screen pass. Each chapter opens on it with its part lit, and ends by
showing what it feeds.

## Accuracy audit

Before revising, a fresh agent audits every claim against the checkout (`new-investigation` §2 step 8):
cascade layout and scrolling, voxelization and the jump flood, occlusion, light injection and the bounce, probe
layout and history, the SH (its order, how it is filled, how it is evaluated), and the screen pass. It also
confirms that 4.7.2-stable at `ed1daf0b` is the checkout, and checks the memory and time trade-offs the narration
states.

## Deliverables

- A review table: beat or time, issue, severity, why it matters, correction (`story/reviews/`).
- The accuracy report, with corrections and file:line evidence.
- The revised storyboard and its zh-CN translation (only the sections that need it), with the new demonstrations
  and the map.
- Both editions rebuilt, and a short change log.

## Open question

The review asks for revised Mandarin narration, but the zh-CN edition keeps the English voice with Chinese
subtitles and on-screen text (`tools/video`). Confirm with the user whether "narration" means the Chinese text
(the default reading) or a Chinese voice track.
