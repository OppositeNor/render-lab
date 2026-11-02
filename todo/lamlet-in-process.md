# Lamlet in the lab's process (plan)

Today a storyboard's Lamlet regions are expanded by the `lamlet` command, run by `tools/video/lamlet.py` (the
"Status" section of [lamlet-integration.md](lamlet-integration.md)). This plan moves Lamlet into the lab's own
process, so that scripts can call the lab and its packages (typed checks of keys, images, actions, and text keys;
values the packages compute), the build's text check sees what regions name, and later the viewer can evaluate
scripts live. The language-side work it relies on is general and is planned in Lamlet's repository,
`plans/embedding.md` (phases 0 to 7); nothing here asks Lamlet for a feature only this lab needs.

Status: plan (2026-11-01).

## What runs where

| Use | Process | Stage | Lamlet API (embedding plan) |
|---|---|---|---|
| Expand a storyboard's regions before speech | `render-lab` (a subcommand the video builder runs) | static | `Engine::check`, `Document::render` |
| Check a storyboard's regions at build time; harvest the text keys and values they name | `lab/check` (build script) | metadata only | `Engine::check` with an interface-only registry |
| Numbers and decisions from package code (VoxelGI's counts in `cost.rs`) | `render-lab` | static, `Pure` natives | typed natives (phase 3) |
| Live text in the viewer (a readout or label computed from the shot), event handlers | the viewer's UI thread | runtime | `Instance::call`, per-thread instance (phase 2) |
| Story curves evaluated per frame instead of lowered to keys | the timeline's thread | runtime | `Instance::call` with an explicit time input |

The first two rows are the reason to start; the last two come later and follow the same host.

## Dependency and distribution

- The lab depends on the `lamlet` library only after Lamlet's phase 0: a versioned release the lab can vendor
  (`vendor/` is refreshed by the user, never edited here). The published build must build alone, so the lab
  never depends on a path outside the repository.
- `lamlet` has no dependencies, so `lab/check` (a build dependency) can use it without linking the lab.
- One build of the library, in the lab dylib, re-exported as `lab::script::lamlet`; packages use it through the lab
  (as they use the lab's wgpu), so native callbacks registered by a package's library and the lab's VM share one
  compiler's types (the plugin ABI check already pins the compiler and the lab build).
- Until the release: keep the subprocess (`tools/video/lamlet.py`), and add natives behind a local-only feature in
  a separate workspace if they are needed sooner.

## The lab's host (`lab/src/script/`, generic)

- **Engine.** One per thread that needs it, made from the current investigation: `std`, rlstd's library, the
  packages' Lamlet modules, and the natives below. Programs are shared across threads (`Arc`, embedding phase 2).
- **Resolver.** Lamlet packages are package resources: a `script` collection in a package's manifest maps a
  module name to a file (`rlstd.script/story` is `packages/rlstd/language/story.lam`). The resolver enforces the
  manifest's import/export rules and reads built resources through `lab::resources`, as every other resource is
  read; Lamlet's export table controls symbols. Language files are cooked like other resources.
- **Natives from the lab (rlstd).** Interface declared once in Rust and written out as a Lamlet stub package
  (embedding phase 3), checked at build time with the same stubs:

  | Native | Stage | Checks or returns |
  |---|---|---|
  | `rl.value key` | Pure | the key exists in the investigation's values; its type and range |
  | `rl.choice key option` | Pure | a choice's option exists |
  | `rl.image name`, `rl.imageKind kind params` | Pure | the image or GPU image kind exists, and its parameters |
  | `rl.action name args` | Pure | the action exists, with these arguments |
  | `rl.text key` | Pure | a text key exists in both languages (and the text check counts it as used) |
  | `rl.source key` | Pure | a source-index key exists (callouts) |
  | `rl.point name` | Pure | an annotation point or arrow set the package publishes |

  rlstd's cue helpers call them, so a misspelled key fails when the storyboard is prepared, before speech, with
  the region's line, instead of when the timeline loads.
- **Natives from packages.** A generic hook, `Investigation::script_natives(&mut NativeRegistry)`, through which a
  package exposes its own `Pure` functions (VoxelGI: `voxelgi.cost/leaves`, `dispatches`, `lowSteps`, `specSteps`),
  so its storyboard narrates numbers its code computes: `${spoken {voxelgi.leaves}}`. The selftest assertions that
  today guard hand-typed numbers become unnecessary for those sentences.
- **Values across threads** are `Data`; nothing holding an `Rc` leaves its thread.
- **Errors** are diagnostics with the storyboard's file and line (the region's position in the `.story` file); a
  native's error carries its API id.

## rlstd's library, adjusted to Lamlet's design

Lamlet keeps structural records and no variadic functions (its rationale, D2 and D3). The rlstd design draft
([lamlet-rlstd.md](lamlet-rlstd.md)) was written with `Nominal`, `wrap`, and `K.variadic`; it changes to:

- presentation values (Color, Formula, CodeSpan, Content) as records with a `kind` field or as a variant
  `<text, formula, code, colored>`, built by rlstd's constructors and checked by them;
- `content` takes one list (`{content {' a b c}}`), not a variadic call;
- host kinds that must not be forged (an image handle) as native opaque types.

The cue helpers that exist (`packages/rlstd/language/story.lam`) keep their names and gain the native checks.

## Commands

- `render-lab --investigation <id> story prepare <file>`: the storyboard with its regions expanded, on stdout (or
  `--json` for spans and diagnostics). `tools/video/lamlet.py` calls it instead of the `lamlet` command; the
  subprocess path stays as a fallback while both exist, then goes.
- `render-lab --investigation <id> script stubs`: the Lamlet stub package of the lab's and the investigation's
  natives, for editors and for reading.

## Milestones

1. **Release gate.** Lamlet phase 0 and 1 done and vendored; `lab::script` with an engine, the resolver over
   package resources, and `story prepare` reproducing `tools/video/lamlet.py`'s output byte for byte
   (`tools/video/test_lamlet.py` run against both).
2. **Checks.** rlstd's `Pure` natives for values, images, actions, text keys, sources, and points; the cue helpers
   use them; `lab/check` harvests text keys from regions (closing the text check's gap).
3. **Package values.** `Investigation::script_natives`; VoxelGI's chapter 8 numbers from `cost.rs`, the story
   unchanged in what it says.
4. **Threads and live use.** After Lamlet phase 2: a per-thread engine in the viewer; one live readout row
   computed by a script, as the first runtime use.
5. **Curves.** Story animations evaluated per frame where a path cannot express them (optional).

## Validation

- `tools/video/test_lamlet.py` passes against the in-process path; the expanded text of every storyboard is
  unchanged by the switch (they have no regions today; the test fixtures cover cues and numbers).
- Frame identity: default frames unchanged (no investigation's frame reads a script until milestone 4).
- Both builds clean (the lab with `lamlet` vendored; the public-only copy builds alone); the local package's
  workspace rebuilt after the lab.
- Selftests of every investigation; a new selftest check that every native the stub package declares has an
  implementation (`require_linked`).
- A two-thread test of the lab's engine, and a malformed-storyboard test (bad syntax, unknown key, a native's
  error) that reports a diagnostic, never a panic.
