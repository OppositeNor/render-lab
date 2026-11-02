# Pass programs: an investigation's own GPU resources and commands each frame (plan, 2026-11-02)

## Why

An engine's frame often runs work the lab's pass chain cannot express (lab/src/investigation.rs `SceneShading`):
compute passes over 3D grids, lists, and texture arrays rather than the viewport; read-write and integer storage
textures; texture atomics; dispatches whose size the GPU wrote (indirect); a raster pass that draws the scene's
meshes into a framebuffer of its own size to write storage images from its fragments; several dispatches of one pass
per frame, each with its own parameters; work that runs only on some frames (a rebuild when the camera moves). Today
the chain's compute passes are 2D over the viewport, targets are float and write-only, volumes are written only by
update passes (1D dispatches, run when a key changes), and no pass draws meshes except the scene, the pre-pass, and
depth views.

A **pass program** is a generic extension point for that: the investigation declares its resources and passes, and
each frame a plan function (CPU, with state of its own) returns the commands to record, in order. The lab creates the
resources, builds the pipelines from the declarations, records the commands, and shows every pass in the frame
graph, the inspector, and the GPU timer like any other.

## The declaration (`SceneShading::program: Option<&'static ShadingProgram>`)

```rust
pub struct ShadingProgram {
    /// Its resources, for a layout key (rebuilt, zeroed, when the key changes: a setting that changes sizes).
    pub layout: fn(&ShotState) -> ProgramLayout,
    pub passes: &'static [ProgramPass],
    /// The plan's state, made once per renderer (and again when the layout key changes).
    pub init: fn() -> Box<dyn std::any::Any + Send>,
    /// This frame's commands, in order, from the state and the shot (the camera, the values).
    pub plan: fn(&ShotState, &mut dyn std::any::Any) -> Vec<ProgramCommand>,
}
pub struct ProgramLayout { pub key: u64, pub images: Vec<ProgramImage>, pub buffers: Vec<ProgramBuffer> }
pub struct ProgramImage {
    pub name: &'static str,
    pub kind: ImageKind,              // D2, D2Array { layers }, D3
    pub size: [u32; 3],
    pub format: wgpu::TextureFormat,  // float, unorm, uint, sint
    pub count: u32,                   // 1, or N images bound as one binding array
}
pub struct ProgramBuffer { pub name: &'static str, pub bytes: u64, pub indirect: bool }
pub struct ProgramPass {
    pub name: &'static str,           // the node's name and its timing row
    pub kind: PassKind,               // Compute { entry, workgroup: [u32; 3] } | Raster { vertex, fragment }
    /// Its bindings at group 3 (the command's parameters, a 64-byte uniform with a dynamic offset, are group 2).
    pub uses: &'static [ProgramUse],
    /// Where its commands run in the frame: before the depth views, after the pre-pass, before the scene pass.
    pub at: ProgramStage,
}
pub struct ProgramUse { pub binding: u32, pub resource: &'static str, pub access: Access }
pub enum Access { Sampled, Load, Read, Write, ReadWrite, Atomic, Uniform }
pub enum ProgramCommand {
    Upload { buffer: &'static str, offset: u64, words: Vec<u32> },
    Clear(&'static str),
    Copy { from: &'static str, to: &'static str, bytes: u64 },
    Dispatch { pass: &'static str, groups: [u32; 3], params: [u32; 16] },
    DispatchIndirect { pass: &'static str, buffer: &'static str, offset: u64, params: [u32; 16] },
    /// Draws the scene's objects (instance = object index) into an attachment of `size` that it does not write
    /// (wgpu needs one), no depth, no culling: for fragments that write storage images.
    Draw { pass: &'static str, size: [u32; 2], params: [u32; 16] },
}
```

The scene pass (and the chain's compute passes) can read program images and buffers: `scene_reads` names them, at
group 3 for the scene shading's fragment entry.

## What the lab does

- **Device** (lab/src/gpu.rs): also request, where the adapter has them, `TEXTURE_ATOMIC`,
  `TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES`, `TEXTURE_BINDING_ARRAY`, and
  `SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING`; naga's validation capabilities follow
  (lab/src/shaders.rs). `Investigation`s see which are on (`Gpu::features`), so a program can choose a fallback
  (and say so in the viewer's notice).
- **Resources**: made from the layout, per renderer (the main view runs the plan; other views of the same renderer
  read what it wrote, a stated simplification); rebuilt on a new key.
- **Pipelines**: one per pass; group 0 the frame, group 1 the shading, group 2 empty (the chain's), group 3 the
  pass's uses; binding-array uses for images with `count > 1`.
- **Recording**: each frame, `plan` runs and its commands are recorded at their pass's stage, in order; consecutive
  dispatches of one pass share one compute pass and one timed span (as update dispatches do). wgpu inserts the
  barriers between dispatches; where an engine runs dispatches without barriers between them, the lab does not
  reproduce that overlap (the investigation says so).
- **Frame graph** (lab/src/graph.rs): each program pass is a node whose reads and writes come from its uses;
  program resources are Persistent (images, `Shape::Image`) or Persistent buffers (`Shape::Bytes`); a pass with
  no command this frame is shown dashed ("did not run"). The lint checks each entry's real uses against `uses`
  (access flags included: an `Atomic` use must be atomic in the WGSL).
- **Inspector** (packages/rlstd/shaders/inspect.wgsl): integer images (uint, sint: `Hash`, `Count`, `Signed`, a
  bitfield `word`/`range`, and `encoding=rgb9e5`), 2D arrays (a `layer` parameter), 3D images (the volume view's
  `slice`/`axis`), binding arrays (an `index`), and byte buffers (words, as buffers are now).
- **Timing**: one row per program pass.
- **Graph command, DAG, resource cards**: unchanged, they read the graph.

## What it does not do

No resource aliasing, no reordering of commands, no async queue; no automatic mips; commands run in the main view
only.

## Status

Implemented (2026-11-02): `lab/src/program.rs`, the declarations in `lab/src/investigation.rs` (`ShadingProgram`,
with elements per resource (`count`, `Element`, `Which`) and instances per pass, which the plan above did not
have), the graph's nodes and lint (`lab/src/graph.rs`), the inspector's integer and array entries
(`packages/rlstd/shaders/inspect.wgsl`), `Renderer::read_program` for selftests, and the device's optional
features (`lab/src/gpu.rs`). The command parameters are at group 2, not group 3 binding 0: wgpu keeps dynamic
offsets and uniform buffers out of a group with binding arrays. Its first user is Godot's SDFGI
(packages/godot_sdfgi/src/godot/sdfgi/gpu.rs). Alongside it: `SceneShading::prepass_outputs`, a pre-pass that
writes targets (a depth and normal pre-pass) for `early` passes before an ordinary scene pass.
