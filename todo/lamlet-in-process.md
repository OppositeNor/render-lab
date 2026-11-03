# Lamlet in the lab's process (plan)

The lab runs Lamlet in its own process, from crates it pins, so that scripts can call the lab and its packages
(typed checks of keys, images, actions, and text keys; values the packages compute), and later the viewer can
evaluate scripts live. The language-side work it relies on is general and lives in Lamlet (`lamlet::embed`, its
`docs/embedding.md`); nothing here asks Lamlet for a feature only this lab needs.

Status (2026-11-03): milestone 1 is done, and the storyboards are Lamlet presentations read through it
(`tools/video/presentation.py`; `tools/video/README.md` and the `render-lab-lamlet` skill describe the format).
The lab pins Lamlet v0.3.1 and lamlet-present v0.1.3, `lab/src/script/` is its host, and `render-lab lamlet
ir|deck|vocab|toml` and `render-lab story keys` are its commands. Milestone 3 is done and 2 in part
([lamlet-typed-content.md](lamlet-typed-content.md)); 4 and 5 are open.

## What runs where

| Use | Process | Stage | Lamlet API |
|---|---|---|---|
| Read a storyboard (its IR, every region evaluated) before speech | `render-lab lamlet ir`, run by the video builder | static | `Engine::check`, `lamlet_doc::evaluate_and_lower` (done) |
| The storyboard's cue vocabulary | `render-lab story keys`, `render-lab lamlet vocab` | static | lamlet-present's `vocab` (done) |
| Check a storyboard at build time; harvest the text keys and values it names | `lab/check` (build script) | metadata only | `Engine::check` with an interface-only registry (today `lab/check` finds the text keys of cue attributes by their text) |
| Numbers and decisions from package code (VoxelGI's counts in `cost.rs`) | `render-lab` | static, `Pure` natives | typed natives |
| Live text in the viewer (a readout or label computed from the shot), event handlers | the viewer's UI thread | runtime | `Instance::call`, per-thread instance |
| Story curves evaluated per frame instead of lowered to keys | the timeline's thread | runtime | `Instance::call` with an explicit time input |

## Dependency and distribution

- **Pinned by git tag**, because Lamlet is not stable yet: in `lab/Cargo.toml`,

  ```toml
  lamlet = { git = "https://github.com/lamlet/lamlet", tag = "v0.3.1" }
  lamlet-doc = { git = "https://github.com/lamlet/lamlet", tag = "v0.3.1" }
  lamlet-present = { git = "https://github.com/lamlet/lamlet-present", tag = "v0.1.3" }
  ```

  `Cargo.lock` records the exact commits. The lab's `lamlet` tag must be the one lamlet-present depends on (its
  `Cargo.toml`); otherwise the build holds two copies of Lamlet whose types do not match. The three crates add no
  crates.io dependency the lab does not already have (lamlet and lamlet-doc have none; lamlet-present has
  `serde_json`). Moving to a newer Lamlet is a deliberate change of these tags, checked like any dependency update
  (every storyboard's board and timeline unchanged). The lab never depends on a path outside the repository, and
  nothing in `tools/video` uses a Lamlet installed on the system.
- **Vendoring.** `setup.sh` vendors from `Cargo.lock` and writes the source replacement `cargo vendor` prints for
  git dependencies (a `[source."git+https://…?tag=…"]` table per repository) into `.cache/cargo/config.toml`, with
  the vendored directory as an absolute path. `vendor/` is never edited by hand. A local package's own workspace
  locks the same three crates (they are the lab's dependencies).
- `lamlet` has no dependencies, so `lab/check` (a build dependency) can use it without linking the lab.
- One build of the library, in the lab dylib, re-exported as `lab::script::lamlet`; packages use it through the lab
  (as they use the lab's wgpu), so native callbacks registered by a package's library and the lab's VM share one
  compiler's types (the plugin ABI check already pins the compiler and the lab build).

## The lab's host (`lab/src/script/`, generic)

- **Engine** (done, for static use): `std`, `content`, `present`, render_lab's package, and the packages a command
  names (a storyboard's vocabulary). Later one per thread that needs it, with the packages' Lamlet modules and the
  natives below; programs shared across threads (`Arc`).
- **Resolver.** Lamlet packages are package resources: render_lab's is the resource `render_lab.script/render-lab`
  (`packages/render_lab/language/render_lab.lam`), read through `lab::resources` as every other resource is.
  Later a `script` collection in any package's manifest maps module names to files, and the resolver enforces the
  manifest's import and export rules; Lamlet's export table controls symbols.
- **The vocabulary** (done): `render-lab story keys` (`lab/src/script/keys.rs`) lists the investigation's values,
  their components, and a timeline key's fields with their options; `render-lab lamlet vocab` turns the list into
  typed functions, so a misspelled key or a wrong value type fails at the storyboard's line, before speech.
- **Natives from the lab (render_lab).** The vocabulary checks names and types; a key whose value names something
  (a text key, a source-index key, an image, a point) takes any String. Natives would check those, declared once in
  Rust and written out as a Lamlet stub package, checked at build time with the same stubs:

  | Native | Stage | Checks or returns |
  |---|---|---|
  | `rl.text key` | Pure | a text key exists in both languages (and the text check counts it as used) |
  | `rl.source key` | Pure | a source-index key exists (callouts) |
  | `rl.image name`, `rl.imageKind kind params` | Pure | the image or GPU image kind exists, and its parameters |
  | `rl.action name args` | Pure | the action exists, with these arguments |
  | `rl.point name` | Pure | an annotation point or arrow set the package publishes |

  The vocabulary's functions would call them, which needs a way for a vocabulary key to name its check: a general
  lamlet-present change (a text key whose values a host function checks), asked of lamlet-present without naming
  the lab.
- **Natives from packages.** A generic hook, `Investigation::script_natives(&mut NativeRegistry)`, through which a
  package exposes its own `Pure` functions (VoxelGI: `voxelgi.cost/leaves`, `dispatches`, `lowSteps`, `specSteps`),
  so its storyboard narrates numbers its code computes: `${spoken {voxelgi.leaves}}`. The selftest assertions that
  today guard hand-typed numbers become unnecessary for those sentences.
- **Values across threads** are `Data`; nothing holding an `Rc` leaves its thread.
- **Errors** are diagnostics with the storyboard's file and line; a native's error carries its API id.

## render_lab's library

`packages/render_lab/language/render_lab.lam` holds what the lab adds to Lamlet for storyboards: numbers said in
words (`spoken`, `spokenFloat`, `rounded`). Content (text, formulas, code names, cues) is lamlet-doc's `content`
and lamlet-present's `present`, so the lab defines no presentation types of its own. Host kinds that must not be
forged (an image handle) would be native opaque types, with the natives above.

## Commands

- `render-lab lamlet ir|deck|vocab …` (done): Lamlet's and lamlet-present's operations, from the pinned crates.
  `tools/video/lamlet.py` runs `ir` and `vocab`; there is no fallback to an installed Lamlet.
- `render-lab --investigation <id> story keys` (done): the vocabulary's key list.
- Later, with natives: `render-lab --investigation <id> lamlet ir …` adds the lab's and that investigation's
  natives to the engine.
- `render-lab --investigation <id> script stubs`: the Lamlet stub package of the lab's and the investigation's
  natives, for editors and for reading.

## Milestones

1. **The dependency, `render-lab lamlet`, and presentations** (done, 2026-11-03). The crates pinned and vendored,
   the host, the commands, the vocabulary, and the storyboards as presentations: each storyboard's board, narration,
   timeline, and captions unchanged from the format they replaced (in O3DE's zh-CN edition, three keys the old
   reader had lost came back).
2. **Checks.** render_lab's `Pure` natives for text keys, sources, images, actions, and points, called by the
   vocabulary (the lamlet-present change above); `lab/check` checks a storyboard with them, replacing its textual
   harvest of cue attributes. In part (2026-11-03, [lamlet-typed-content.md](lamlet-typed-content.md) phase 3):
   `rl::source`, `rl::text`, `rl::point`, `rl::formula`, called by render_lab's own attribute functions rather than
   the vocabulary, with `render-lab --investigation <id> lamlet …`; images and actions keep the vocabulary's options,
   and `lab/check` keeps its harvest.
3. **Package values.** `Investigation::script_natives`; VoxelGI's chapter 8 numbers from `cost.rs`, the story
   unchanged in what it says. Done (2026-11-03).
4. **Threads and live use.** A per-thread engine in the viewer; one live readout row computed by a script, as the
   first runtime use ([lamlet-live-logic.md](lamlet-live-logic.md)).
5. **Curves.** Story animations evaluated per frame where a path cannot express them (optional). Typed motions,
   and the rest of the storyboards' Strings and repetition, are [lamlet-typed-content.md](lamlet-typed-content.md).

## Validation

- `python3 -m unittest tools/video/test_markup.py tools/video/test_presentation.py`, and every storyboard's board,
  narration (no new synthesis), timeline, and captions unchanged by a change to the host (`build.py
  --prepare-only` for each edition, compared with the files before).
- Frame identity: default frames unchanged (no investigation's frame reads a script until milestone 4).
- Both builds clean (the lab with Lamlet vendored; the public-only copy builds alone); a local package's
  workspace rebuilt after the lab.
- Selftests of every investigation; a new selftest check that every native the stub package declares has an
  implementation (`require_linked`).
- A two-thread test of the lab's engine, and a malformed-storyboard test (bad syntax, unknown key, a native's
  error) that reports a diagnostic, never a panic.
