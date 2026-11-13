# Adding an investigation

[Render Lab](../README.md) · [Build](../BUILD.md) · [简体中文](adding-investigations.zh-CN.md)

An investigation is a package in `packages/<engine>_<feature>/`: its content, and its code, a library that implements
`Investigation` (`lab/src/investigation.rs`) and hands it to the lab when it negotiates (copy
[`packages/o3de_lighting/src/lib.rs`](../packages/o3de_lighting/src/lib.rs), and add the package to the workspace in
`Cargo.toml`). Keep anything derived from an engine's source code in it, out of the shared lab,
and out of any public copy of the repository unless the engine's license allows it (then credit it in
[`THIRD_PARTY.md`](../THIRD_PARTY.md)).

Code of one package that another uses crosses between their libraries as C functions: the exporter writes them with
`lab::export!` and declares them in an `interface/` crate, and the importer imports its package and names them in its
negotiation, which resolves them when it loads ([`lab/src/plugin/abi.rs`](../lab/src/plugin/abi.rs); the pilot is
[`packages/godot_voxelgi/interface`](../packages/godot_voxelgi/interface/src/lib.rs)). Engine-neutral helpers that
several packages share are the lab's own, in `lab::utility`.

An investigation that shades the scene its own way declares it in its package's `shading.toml` (a `shading` resource,
read at run time into a `SceneShading`; the format is in [`lab/src/shading_file.rs`](../lab/src/shading_file.rs)), and
registers the functions the file names, which compute rather than declare (its static data, an update's or a program's
plan), with `Investigation::shading_hooks`. The shading is WGSL composed after the scene module, with a fragment entry point, per-frame uniforms (`FrameContent::shading`) and per-frame
data (`FrameContent::shading_frame`), static data, and compute kernels for its selftest (`Renderer::run_shading_verify`).
It may also be a **pass chain**, for an engine that renders through several targets: the fragment shader writes the
shading's `outputs` (multisampled targets), its `passes` run as compute passes over named targets at fixed bindings
(`compute` nodes of its `[[node]]` list, the frame in order), and the last one writes `"hdr"`, which the frame shows. Declare its WGSL in the investigation's package as a
`[[shader]]` whose parts start with `render_lab.shader/scene`, and name that shader in the file's `module`; build.rs validates it, and so
does the lab when it loads it. A frame without a struct-panel record can show a `Readout` (names and values) in its place.

What the viewer's Scene window lists and its gizmo moves ([`lab/src/entity.rs`](../lab/src/entity.rs)): the scene's
objects, and the lights and points the investigation's values place, which it declares with `Investigation::entities`
(a name from its text, a kind, and the three values of a position). A scene's objects marked `movable` move too when
the investigation's frame follows them (`Investigation::movable_objects`: its records come from the scene's, which the
lab then poses); one that draws the scene from data it baked leaves it false, and they stay.

See the [architecture and code map](architecture.md) for the shared lab and loader.
