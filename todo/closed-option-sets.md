# Closed option sets in the cue vocabulary (waits on lamlet-present)

The cue vocabulary (`render-lab story keys`, turned into a Lamlet package by `render-lab lamlet vocab`) gives a key
with options one constant per option, but its String function still accepts any String: a wrong panel, demo, or
listing name is a String, so Lamlet's type checker cannot refuse it at the storyboard's line.

lamlet-present could mark a key's options as closed (`"closed": true` in the key list) and generate a variant, or a
checked function, instead. That is a change to lamlet-present, released and pinned in `lab/Cargo.toml` (now
lamlet-present v0.1.4), and the user's call; this repository does not modify `vendor/`.

Until then the story check refuses those names before speech, at the cue's line (`lab/src/story_refs.rs`,
`tools/video/check.py`), so nothing is lost but the editor's earlier error. When a release has it:

1. `lab/src/script/keys.rs`: mark the option lists the lab owns (panels, eases, corners, demos, listings, images,
   image kinds, actions, click handlers) closed.
2. Re-pin lamlet-present, vendor it, and check every storyboard and both editions with `tools/video/check.py`.
3. Keep `story_refs.rs`'s checks: they also cover raw `attr`s and the inherited context of highlights.
