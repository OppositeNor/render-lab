# Shader sandbox (future plan, not started)

The user's wish (2026-11-02): a sandbox mode in which a pass's shaders can be edited in the viewer. Today the
frame graph's full-screen view shows a selected pass's WGSL read-only (`lab/src/shader_source.rs`, drawn by
`lab/src/code_view.rs`), and the viewer reloads the checkout's WGSL live when a file changes
(`resources::reload_changed`, `Renderer::reload_shaders`: a broken edit keeps the last good pipelines).

What a sandbox would add, to plan when it is asked for: editing in the source pane (the code view gains a cursor
and selection), where an edit goes (the package's file, or an overlay kept apart from the checkout until saved),
how its errors are shown on the lines they name (naga's spans), and what a reset puts back.
