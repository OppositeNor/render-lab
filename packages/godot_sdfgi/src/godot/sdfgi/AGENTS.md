# Godot SDFGI investigation: agent guide

Godot's signed distance field global illumination (SDFGI), read from the engine's source and shown in the lab.
Follow `.agents/skills/new-investigation` (the standard) and `.agents/skills/new-story` (the story's phases).

## Checkout

- `../../godot` from the lab's root (`/home/oppositenormal/Documents/programming/other/godot`), read-only.
- Godot 4.7.2-stable, commit `ed1daf0bf001b61586d9930840f2f1394092c079` (2026-08-16): the user checked it out
  (2026-10-29) after the research was read at 4.8.0-beta (`cafc17a1`). SDFGI's code is the same in both apart
  from what `story/research.md` §0.0 lists; the index records the stable commit.
- License: MIT (`LICENSE.txt`), so this package is public. Code ported from Godot keeps its copyright
  ("Godot Engine contributors", "Juan Linietsky, Ariel Manzur") and is credited in `THIRD_PARTY.md` with
  `LICENSES/Godot-MIT.txt`.
- It does not depend on, cite, or name any local (commercial) investigation, and reuses none of its code.

## Story state

- Phase 7: storyboard reviewed (story/reviews/), corrected, translated; full video build next.

## Decisions

- User: investigate Godot's SDFGI with the `new-story` process (2026-10-29).
- User: scope is the eight subtopics; debug views, multiview, half-resolution GI, and VoxelGI combined are left
  out or said in passing; transparents and fog are one beat (2026-10-29).
- User: the lab uses the engine's grid, 128³ cells per cascade, and the default y scale (y_mult 1.5).
- User: checked out 4.7.2-stable and asked to compare it with the beta. Agent (open to change): the
  investigation follows stable, the version the user checked out.
- User: findings that look like engine bugs are narrated and shown where their step is explained ("read from
  the code"), and the lab reproduces the engine's behaviour (2026-10-29).
- User: the outline as drafted (11 chapters, 1,830 s) (2026-10-29).
- User: add one omni light for the static-light chapters, Dynamic by default. Agent: it lives in this
  investigation's frame content, not in the shared study-room scene, so other investigations' frames are unchanged.
- Agent (open to change): the ceiling panel stands for a dynamic AreaLight3D of its size (range 10 m,
  attenuation 2, energy = radiance x area / pi, so Godot's forward pass would match the lab's direct light);
  its mesh is left out of the voxels (GI mode Disabled), or it would shadow its own light.
- Agent (open to change): records are shown as readouts (one row per field and its bits), not the struct
  panel: the panel reads only layouts the lab's types.wgsl declares, and adding Godot's there would make the lab
  name this investigation.
- Agent (open to change): the screen pass runs per fragment of the scene pass, from probe maps of the CPU world
  converged over 60 frames at the default settings; the specular march below roughness 0.2 is not in the WGSL.
- Agent (open to change): chapter budgets follow the narration (about 1.2x, about 20 minutes in all) instead of
  the outline's 30 minutes, which would have been mostly pauses; reviewers can ask for more depth.
- Agent (open to change): package name `godot_sdfgi`, investigation id `godot/sdfgi`.

## Preferences

(none recorded yet)

## Pending requests

(none)
