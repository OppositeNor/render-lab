# Free camera

## Goal

A second camera in the viewer that looks at the scene from anywhere, rendered by the same pipeline, while
everything the lab demonstrates keeps following the **main camera** (the shot's `state.camera`, which timelines,
camera cues and the dock drive):

- 3D overlay arrows, balls, labels, rays: computed from the main camera, drawn in 3D in the free view.
- GPU buffers shown in insets (image kinds: depth, targets, tile grid, SSAO, shadow buffer): the main camera's.
- Engine state that follows the camera (O3DE's shadow cascades and tile culling, readouts, picks):
  the main camera's.
- The free view draws the main camera: its position, a label, and its view cone (frustum).

## Design

### State and input (`lab/src/shot.rs`, `lab/src/viewer.rs`, `lab/src/camera.rs`)

- `ShotState.free_camera: Option<OrbitCamera>`. `None` means off (today's behaviour). It is not a timeline channel:
  a timeline keeps driving the main camera while the free camera stays put.
- Toggle: a key (`C`) and a dock button "Free camera". Turning it on starts the free camera from the main one,
  pulled back and raised so the main camera's cone is in view.
- While it is on, left-drag, pan and wheel move the free camera (the same `orbit`, `pan`, `zoom`). The dock's
  camera sliders, timelines and camera cues still move the main camera.
- Clicks in the free view: picks use the free camera's ray (they pick world points). Inset clicks are unchanged.

### One frame, two renders (`lab/src/renderer.rs`)

`build_frame` runs once, with the shot's state, so every package's CPU demonstration code is untouched and keeps
using `content.uniforms.camera` = the main camera. With the free camera on:

1. **Main render**, into the main slot's targets, as today: chain, image kinds (`dispatch_views` reads this
   render's depth and targets), stills and history counts. It is not composited to the screen.
2. **Free render**, into a second slot (`ViewSlot`, as Compare's reference uses, so its targets, chain and history
   stay separate): a clone of the content with `uniforms.camera` replaced by the free camera's `CameraData`
   (keeping the main camera's near and far planes, so packages that change the far plane keep it), the main camera
   marker added to its draw lists, then composited to the viewport.

The free render runs the same passes, so screen-space techniques (tile culling, SSAO, screen-space GI) run in the
free camera's screen, while CPU-built engine data in `shading_frame` (O3DE's cascades) stays the main camera's.

### A hook for render-camera data in packages (`lab/src/investigation.rs`)

`Investigation::view_from(&self, content: &mut FrameContent, camera: &CameraData)`, default: nothing. A package
whose shading data holds the render camera rewrites it there. O3DE: `shading_frame[0..3]`, the inverse
`world_to_clip` its depth reconstruction uses. SDFGI and VoxelGI read the camera only from `uniforms.camera`, so
they need nothing. This is generic: any investigation with camera-derived shading data uses it.

### The main camera's marker (lab, generic)

Added to the free render's draw lists (`content.draw`):

- A small camera body at the main camera's eye, and its four frustum edges from the eye out to a set distance
  (the orbit target's distance), with the far rectangle closed. Built from the main `CameraData` (inverse
  `world_to_clip`), so it matches the actual projection, aspect included.
- A label "Main camera" at the eye (new rlstd text key, en + zh-CN).

### Screen-space UI in the free view (`lab/src/ui.rs`, `packages/rlstd/src/inset.rs`)

- Labels and inset link lines project their world points with the free camera (the content they get is the free
  one).
- Insets of image kinds show the main render's textures. A link line to an off-screen point is hidden, as today.
- The main camera's own view as a small inset in a corner (its render is made anyway).

### Snapshots and checks

- Snapshot page `viewer-free[:<demo>]`: the viewer on a demo with the free camera on, for offscreen review.
- The autotest or a selftest check: with the free camera on, the image kinds' textures and the CPU readout match
  those with it off (same main camera), and the free view's image differs.

### Story cues (`lab/src/shot.rs`, `lab/src/animation.rs`)

The free camera is a second `OrbitCamera` value scope, `freeCamera`, with the same channels as `camera.*`
(`freeCamera.x/y/z`, `.yaw`, `.pitch`, `.distance`, `.fov`) plus `freeCamera.on` to switch it on. A story places
it as it places the main camera (`{@ freeCamera.on=1 freeCamera.yaw=40 freeCamera.distance=12}`), animates it with
`animate` paths, and blends it like any channel. Placed nowhere, it has a fixed overview pose. `cameraDrift` sways
the free camera while it is on (the main camera then holds still unless cued). A `demo` switch turns it off, as it
resets everything else. In the viewer, C (or the dock's button) toggles it, starting beside the main camera, and
input moves it, over what a timeline says, until Reset or a change of mode.

### Unifying the stories' own second cameras

Three features already show "a camera the engine follows, seen from elsewhere". Each becomes the main camera, seen
from the free camera:

| Feature | Today | Unified |
|---|---|---|
| O3DE cascade observer (`cascadeCamera`, `observerYaw`, `cascade_observer`) | A fixed observer at (2.3, 1.6, 2.3) the cascades fit when `cascadeCamera=1`; its frustum and cascade slices drawn; the viewer looks from elsewhere | `shadow_camera` always fits the main camera. The cue `cascadeCamera=1` becomes `freeCamera=1` with the old `camera.*` as `freeCamera.*` and the observer's pose as `camera.*`; `observerYaw` sweeps become `camera.yaw` sweeps. `cascade_overlay` draws the main camera's slices; the lab draws the camera and its cone. `cascadeCamera`, `observerYaw` and their keys go |
| O3DE `observer` (`observerX/Y/Z`: the BRDF's eye) | The readout's vectors seen from a fixed point (the `close-up` preset, six demos, about 23 beats) | **Kept** (see Status): not a camera the engine follows, and moving the main camera there would move the buffers those beats mark by region |
| SDFGI camera walk (`cameraX`, `cameraJump`, `ORIGIN`) | A simulated SDFGI camera at `ORIGIN + x·cameraX` the cascades follow; a ball and a label, no cone; the shading uses `ORIGIN` | SDFGI follows the main camera's eye in every demo (cascades, scrolls, shading's `sdfgiCamera`). Walk cues become main-camera moves with the free camera on to watch them; the lab draws the camera and its cone |

VoxelGI has no such feature. Its PANEL-hiding check now reads the camera that renders (`ShotState::view_camera`).

Each story's cues are rewritten to match, then checked with `--snapshots-only` stills of the affected beats.

## Scope and order

1. State, toggle, input, text keys; the `freeCamera` channels and cues.
2. Renderer: the second slot, the free render, the hook, the marker, the main view's corner inset.
3. UI projection in the free view; the snapshot page.
4. O3DE's `view_from`.
5. Unify O3DE's observers and SDFGI's walk; rewrite their cues.
6. Checks: both builds clean, the public build, home autotest, the three selftests, snapshots of each investigation
   (a demo with arrows, one with an image kind) with the free camera on and off, and stills of the rewritten beats.

## Decisions

- Main view's own image as a corner inset while the free camera is on: yes.
- Controls: orbit, as the main camera.
- Stories use it (`freeCamera` cues), and the stories' own second cameras are unified into it.
- SDFGI's camera is the main camera's eye everywhere, as in Godot: the cascades and the shading follow it;
  `cameraX`, `cameraJump` and `ORIGIN` as the camera go; walk cues become main-camera moves. Readouts and selftest
  numbers change; the cost of re-walking when the eye crosses a cell is measured.

## Status (2026-11-01)

Done and checked (both builds clean, home autotest, the three selftests, `viewer-free` snapshot, O3DE's two cascade
beats rendered from the rebuilt timeline):

- The free camera: `FreeCamera` (src/camera.rs), `ShotState::free_camera` and `view_camera`, the `freeCamera.*`
  channels and cues, C and the dock's "Free camera" button, input routed to it, the snapshot page `viewer-free`.
- The free render (src/free_view.rs, renderer `free_slot`), `Investigation::view_from` (O3DE rewrites its
  clip-to-world matrix), the main camera's marker and cone, its own view framed in a corner.
- O3DE's cascade observer unified: the cascades always fit the main camera; `cascadeCamera`, `observerYaw`,
  `cascade_observer` and their keys are gone; its two beats use `freeCamera` with the main camera at the old
  observer's pose. O3DE's camera near plane is now O3DE's 0.1 m (the narration's splits depend on it).

Not done:

- O3DE's `observer` (the BRDF's eye in the close-ups): kept, as above.
- SDFGI's walk: blocked on a decision. Its whole simulation (voxels, light, probes over `CONVERGED_FRAMES`) is
  converged once at `ORIGIN` and cached; following the main camera's eye means converging it again for each snapped
  cascade position the eye reaches, which orbiting reaches constantly. Options: converge per snapped position in the
  background with a cache (large); follow the main camera only in the cascades and regions demos, where the walk
  already re-runs; or keep `cameraX` and draw it with the lab's camera marker.
- The check that image kinds and readouts match with the free camera on and off; the zh-CN stills of the beats.
