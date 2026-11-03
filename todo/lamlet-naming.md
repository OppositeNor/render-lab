# Following Lamlet's naming conventions (plan)

Lamlet recommends names for everything where a host meets it (Lamlet's `docs/conventions.md`):
- camel case in code (`cameraYaw`, `isEmpty`), capitalized types
- package names in lower case joined by `_` (a dashed project name becomes `_`)
- ids, cue names, files, and extension programs in lower case joined by `-`
- a host's vocabulary keys as the host spells them

This plan lists what the lab changes to follow them. Most of the lab already does.

Status: step 1 done (2026-11-02): `rlstd` is `render_lab`, its Lamlet package is `render_lab.lam` (written with
Lamlet 0.3's function definitions), and `tools/video/lamlet.py` runs `lamlet FILE`, which Lamlet 0.3 has in place
of `prepare`. Steps 2 and 3 come with [lamlet-story.md](lamlet-story.md)'s milestones.

## Already following

- Investigation packages: `godot_sdfgi`, `godot_voxelgi`, `o3de_lighting` (lower case, `_`).
- Beat ids: `c1-room` and the like (lower case, `-`).
- The functions of `packages/render_lab/language/render_lab.lam`: `imageTitled`, `codespanSaid`, `spokenFloat`; private
  helpers start with `_`.
- Shot parameters and key fields: camel case (`voxelgiMix`, `imageCorner`), with dotted channels
  (`camera.yaw`). They stay as they are, since they are the lab's vocabulary.

## To change

1. **`rlstd` becomes `render_lab`.** It is the lab's own package, not a standard library, and the convention
   spells render-lab's package `render_lab`.
   - The directory: `git mv packages/rlstd packages/render_lab`.
   - Resource names: `rlstd.scene/…`, `rlstd.environment`, `rlstd.text`, … become `render_lab.…`. That covers
     37 demo files, plus `lab/src` (`resources::get`, `package = "rlstd"` on the camera values, the
     `unwrap_or("rlstd")` defaults in `cli.rs` and `controls.rs`).
   - The Lamlet package: `$[Import* rlstd]` becomes `$[Import* render_lab]`, `tools/video/lamlet.py`'s
     `--package rlstd=…` becomes `--package render_lab=…`, and `story.lam` becomes `render_lab.lam`, after the
     package.
   - Text: `README.md`, `README.zh-CN.md`, `THIRD_PARTY.md`, `AGENTS.md`'s list of tracked packages, the
     `.agents/skills`, `tools/icons`, and the `todo/` plans (`lamlet-rlstd.md` becomes
     `lamlet-render-lab.md`).
   - In all: 115 tracked files, 337 lines (as of 2026-11-02). Anything outside git that uses `rlstd.` names
     needs the same change.
   - Check:
     - `cargo test` and the lab's own package check (`lab/check`)
     - the Python tests (`tools/video/test_*.py`)
     - `render-lab --investigation X values` for each investigation, to confirm resources resolve
     - `git grep -i rlstd` returns nothing
2. **Presentation names** ([lamlet-story.md](lamlet-story.md)):
   - storyboard files `storyboard.lam` and `storyboard.zh-CN.lam`
   - chapter ids `c1`
   - cue and range names in lower case with `-`; the converter keeps today's range names (`m0`, `p1c`), which
     already fit
   - the generated vocabulary package named after its investigation (`godot_voxelgi`), written to
     `build/godot_voxelgi.lam`
3. **JSON the lab writes for Lamlet tools** (the vocabulary from `story keys`) uses camel case keys, as the
   lab's other JSON does.

## Not changed

- Rust code keeps Rust's conventions (`snake_case` functions, `CamelCase` types). The conventions cover only
  where the lab meets Lamlet.
- The `render-lab` binary and repository keep their dashed name, as programs and repositories do.
