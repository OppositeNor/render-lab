---
name: packages
description: Create, change, build, and troubleshoot Render Lab packages, the self-contained folders the lab loads at run time (sources in packages/, built beside the binary). Covers the manifest (import, export, library, negotiate, provides, series, notices), resource collections, kinds, full keys and references, overrides, built tool outputs, demos, a package's cdylib and its negotiation (`lab::negotiate!`), local packages with a Cargo workspace of their own, packing, the cook step, and the loader's errors. Use when adding or editing a manifest.toml or a collection, adding a resource, scene, shader module, text table, or demo entry, giving a package code, when a package or its library does not load, or when asked how packages, the loader, or the negotiation work. For an investigation's code inside its library use `new-investigation`; for live scripts use `render-lab-lamlet`.
---

# Packages

Everything the lab shows comes from **packages**: folders, each with a `manifest.toml`. The binary (`src/main.rs`)
only runs the lab (`lab/`, a Rust dylib). The **loader reads only the built packages**, the folders of `packages/`
beside the binary (`target/release/packages/<name>/`): it mounts each package's resources and opens each package's
library to get its investigations. The **source packages** are the folders of the checkout's `packages/`; every
launch from a checkout first builds them into the binary's `packages/` (`lab/src/cook.rs`), incrementally, so an edit
shows on the next launch. The formats are documented in the code: the manifest in
`lab/src/packages.rs`, a collection in `lab/src/pipeline/mod.rs`, the negotiation in `lab/src/plugin.rs`. Read them
before changing a format; this skill is how to use them.

## 1. A package is self-contained

Everything a package is lives in its directory, so it can be packed and shared. A source package:

```
packages/<name>/
  manifest.toml        what the package is: its metadata, imports, exports, library, and what it provides
  resources.toml ...   its collections: [[resource]] and [[shader]] tables (any file names; the manifest lists them)
  text/, shaders/, demos/, params.toml, pseudocode/, source-index.json, assets/ ...   its content
  knowledge.toml       its chapters' requirements in the knowledge graph and its audience (resource id `knowledge`; the knowledge-graph skill)
  shading.toml         its frame, if it shades the scene its own way (resource id `shading`; lab/src/shading_file.rs, the render-graph skill)
  video/               a narrated video's inputs (video.json, storyboards, diagrams), for tools/video
  Cargo.toml, build.rs, src/   its code, if it has investigations (section 5)
  build/               what its tools write: build/timelines/<lang>/, build/video/<lang>/ (git-ignored)
```

Its built package, which the build writes and the loader reads (never edit it; it is rebuilt from the source):

```
target/release/packages/<name>/
  manifest.toml        the source's, with one collection, and `cooked-from` (the source package it was built from)
  resources.toml       every resource: `file` (its output), `source` (for messages), `stamp`; and the shaders
  text/, shaders/, build/timelines/ ...   `file` and `directory` resources, at their paths in the package (hard links)
  cooked/<key>.bin     processed resources (radiance, mesh)
  lib/lib<name>.so     its library, linked from where Cargo put it (beside the binary)
```

Nothing a package needs may live outside its folder except the packages it imports and the lab itself. `render_lab`
is the lab's own package (the sky, the teapot, the scenes, fonts, the lab's WGSL and text); every investigation's
package imports it. `packages/o3de_lighting` is the full example of an investigation's package.

## 2. The manifest

```toml
name = "my_feature"                  # = the directory's name: lower-case words joined by `_`
author = "Your Name"
contact = "you@example.com"          # optional
license = "MIT"                      # the package's own content; each resource also names its source's license
import = ["render_lab"]              # the packages whose resources it names (a list, may be empty)
export = ["scene/garden"]            # its own keys other packages may name (a list, may be empty)
library = "my_feature"               # optional, with `negotiate`: its code (section 5)
negotiate = "my_feature_negotiate"
credit-text = "notice-credit"        # optional: a short copyright and license line, after the engine's version at the top right
requires = ["ray-tracing"]           # optional: what its investigations need of the lab (lab/src/packages.rs `REQUIREMENTS`)

[[provides]]
type = "resource"                    # a collection, relative to the manifest
path = "resources.toml"

[[provides]]
type = "demo"                        # a narrated timeline the homepage's Watch plays
key = "video"                        # letters, digits, `-`, `_`; unique in the package
title = "How My Feature Works"         # the edition picker names it by its timeline's language (locales.toml)
timeline = "timeline/en"             # resources (keys of this package, or `<package>.<key>`)
audio = "narration/en"

[[provides]]
type = "series"                      # investigations told one after another as the parts of one story
key = "indirect-lighting"            # letters, digits, `-`, `_`; unique in the package
title = { en = "Indirect lighting in Godot", zh-CN = "Godot 中的间接光照" }   # by language, `en` required
summary = { en = "Three parts, from the simplest to the most involved: …" }
parts = ["godot_ssr", "godot_sdfgi", "godot_voxelgi"]   # in order; each a package `import` lists

[[notice]]                           # optional: a legal notice of what the package holds
notice-text = "notice-copyright"     # a key of its text tables (both languages)
license = "LICENSE-Engine.txt"       # optional: the file of its terms, in the package; built with it
```

`requires` names what the package's investigations cannot run without. The lab knows one requirement,
`"ray-tracing"`: hardware ray queries (Vulkan; lab/src/gpu.rs), which the lab turns on where the GPU has them
(`RENDER_LAB_RAY_TRACING=off` turns them off). Where a requirement is not met, the loader (and `--list`)
list the package's investigations as unavailable, with the reason, and `--investigation` refuses with it; the
package's shaders still validate in its build (the lab validates with the ray-query capability).

- Unknown keys are errors. `library` and `negotiate` come together or not at all.
- Packages load after the packages they import (then by name); an import that is not in `packages/`, or a cycle, is
  an error. The homepage lists investigations in that order.
- Every `export` must name a resource or shader the package declares.
- A **series** orders investigations as the parts of one story (`packages/godot_indirect`, a package with a manifest
  only). Each part is a package with an investigation and its own chapters (numbered from 1) and narrated timelines.
  The homepage's Feature column lists the series (its title, "A story in n parts") in place of its parts; the detail
  panel shows "Part n of m" with the parts one under another on darker glass, to pick; each part is watched on its
  own (a timeline pauses on its last frame). The video builder writes `part` into each part's timeline, and its chapter tag reads `PART 1 / 3   01 / 08`.
  `render-lab series` lists the series and checks that a part's audience (knowledge.toml `known`) knows what the
  parts before it teach.
- A package that holds an engine's code, or names it, gives its **notices**: the copyright and license of code ported
  from it (the license's own requirement, e.g. MIT's notice in every copy of a substantial portion) and the engine's
  trademarks, with no claim of affiliation. The lab shows them on the homepage's selection page and on a card that ends
  every video of the package, and the video builder writes them, with each license's full text, to `NOTICE.txt`
  beside the video.
- Notice and credit texts write symbols as their characters (©, ®, ™); `{ $year }` in a credit is the current
  year.

## 3. Collections and keys

A collection is a list of `[[resource]]` and `[[shader]]` tables:

```toml
[[resource]]
id = "scene/garden"                  # its key: lower-case words joined by `-`, in groups joined by `/`
kind = "file"                        # its processor: file, radiance, or mesh (lab/src/pipeline/mod.rs, KINDS)
source = "scenes/garden.toml"        # relative to the collection
license = "MIT"                      # the source's license, as THIRD_PARTY.md records it

[[resource]]
id = "teapot"
kind = "mesh"                        # every parameter its kind takes, no defaults:
source = "assets/models/teapot.obj"  #   mesh: scale, leaf-triangles, max-depth
license = "…"                        #   radiance: sun-cap, levels
scale = 0.14
leaf-triangles = 4
max-depth = 30

[[shader]]
id = "shader/my-shading"             # a WGSL module, composed at run time from its parts, in order
parts = ["render_lab.shader/scene", "wgsl/my-shading"]
```

- **Keys and full keys.** A key belongs to its package; everywhere else a resource is named by its full key,
  `<package>.<key>` (`render_lab.font/dejavu-sans`). The lab's and an investigation's Rust code always name full keys:
  `lab::resources::get("my_feature.scene/garden")`.
- **References** in a package's content (shader `parts`, `override`, a demo's `timeline` and `audio`, a
  demonstration's `scene` and the presets it `include`s): a plain key is the package's own (a preset by its name,
  `"wide-camera"`); `<package>.<key>` must name a package in `import` and a
  key that package exports (`render_lab.demo-preset/room-camera`, the study room's overview camera several
  investigations share). Anything else is an error that names the manifest and the rule.
- **Kinds.** `file` is the source's bytes as they are; `directory` is a folder of files read where they are (the
  diagram frames a timeline names beside it). Both are linked into the built package at their paths. `radiance` makes
  an HDRI into the environment's levels, and `mesh` makes an OBJ into a BVH, at `cooked/<key>.bin`. The build remakes
  an output only when its stamp changes (its kind, processor code, parameters, and its source's size and time). A new
  kind is a processor in `lab/src/pipeline/`. A source must be inside its package.
- **Overrides.** A resource marked `overridable = true` (and exported) may be replaced, for everyone, by a resource of
  a package that imports it: `override = "<package>.<key>"`. Two packages may not override the same key.
- **Tool outputs.** `built = true` marks a file (or directory) a tool writes into the source package
  (`source = "build/timelines/en/timeline.json"`). It may be missing until the tool runs; the next launch builds it
  into the built package, and `render-lab resources` shows `not built` until then. Demos name such resources:
  tools/video writes them.
- **Experiments.** An experiment file (`lab/src/experiment.rs`) declared as `experiment/<name>`
  (`kind = "file"`, `source = "experiments/<name>.toml"`) is offered by the viewer's Experiments tab and run by
  `render-lab experiment <package>.experiment/<name>` (the `experiments` skill).
- **Quizzes.** A quiz file (`lab/src/quiz.rs`, its format in the module's comment) declared as `quiz/<name>`
  (`kind = "file"`, `source = "quiz/<name>.toml"`) gives the homepage's part its Quiz button and the viewer its Quiz
  tab; its text is the package's keys, in `-text` fields (the `new-story` skill, "The quiz").
- **Text.** A package's on-screen text is a Project Fluent file per language that render_lab's `locales.toml` registers
  (`text/en.ftl`, `text/zh-CN.ftl`), declared as `text/<code in lower case>` (`text/en`, `text/zh-cn`), read by the code
  with `tr("key")`, which `lab::text_table!("<package>.text")` defines in its text module. A complete language's table
  has every key; a draft one's may miss keys, which show in its fallback languages (`render-lab i18n` lists them). Data
  files name text keys in fields ending in `-text` (`title-text = "scene-garden"`), and pseudo-code listings in their
  `# title-text:` and `# dispatch-text:` headers.

## 4. Content without code

A package with no `library` is content only: scenes, shaders another package overrides, sounds, or a shared replay
(its demos' timelines and audio, with what they show). Its resources mount, its demos are listed for the
investigation whose package they belong to, and it needs no build. Check it with `render-lab packages` and
`render-lab resources`.

## 5. A package with code

An investigation's code is a library in its package, a **cdylib** the lab opens at run time.

`Cargo.toml` (copy `packages/o3de_lighting/Cargo.toml`):

- `[lib] crate-type = ["cdylib"]`.
- Depend on `lab = { package = "render-lab-core", path = "../../lab" }`, and otherwise **only on crates the lab
  depends on, with the lab's exact versions and no more features** (copy the lines from `lab/Cargo.toml`). The binary,
  the lab, and every library must share one build of the lab; a new dependency, or a new feature of one, splits it.
- `[build-dependencies] render-lab-check = { path = "../../lab/check" }`, and a `build.rs` that calls
  `render_lab_check::check_package(&dir, &dir.join("src"))`: the package's shaders validate with naga, and its text
  tables match the keys its code and data name. Copy `packages/o3de_lighting/build.rs`.

`src/lib.rs` (copy `packages/o3de_lighting/src/lib.rs`):

```rust
mod my_engine;

use lab::investigation::Investigation;
use lab::plugin::InvestigationsFn;

fn investigations() -> &'static [&'static dyn Investigation] {
    static ALL: [&dyn Investigation; 1] = [&my_engine::feature::FEATURE];
    &ALL
}

lab::negotiate!(my_feature_negotiate, ["investigations" => investigations as InvestigationsFn]);
```

- The symbol is the manifest's `negotiate`; the library's file is `lib<library>.so`.
- The investigation's `fn package(&self) -> Option<&'static str> { Some("my_feature") }` ties it to its package: its
  timelines are the package's demos, and its text, listings, demonstrations, and shading name its keys.
- The library names the lab's items as `lab::…`. A lab item the library uses must be `pub` (not `pub(crate)`), and a
  lab macro `#[macro_export]`.
- Add a public package to the root `Cargo.toml`'s `[workspace] members` and `default-members`, so
  `cargo build --release --locked --offline` builds it.
- **A tab of the viewer's dock** is a feature (`lab/src/feature/mod.rs`): a `Feature` (its id, its tab's label, or
  none for a feature without a tab, its order, whom it is `available` to, its `layout`, the story panels it draws,
  its homepage button, its snapshot pages with their `--page` help) and the `FeatureState` it makes per viewer and
  per recording (`Send`; hooks for its dock, its panel, its story panels, what it shows full screen, the frame's
  requests, views it adds, a change of demonstration, snapshot steps; each defaults to nothing). Every tab and panel
  of the lab is a feature (`lab/src/feature/`: Parameters, Experiments, Code, Sources, Prerequisites, Frame graph,
  Quiz, Captions, and the tab-less Performance): copy the nearest. A package adds its own with the optional
  `features` handle, `"features" => features as lab::plugin::FeaturesFn` (`fn() -> &'static [&'static dyn Feature]`)
  beside `investigations`; its label is its own text (`lab::i18n::package_text`), and `available` usually checks
  `investigation.package()`. An id another feature has is left out, and the loader says so. Nothing else in the lab
  changes for a new tab: no enum, action, field, or snapshot arm.
- **Behaviour can be content instead of code:** a resource `script/<name>` (Lamlet) is a script the lab runs: overlays,
  readout rows, click handlers, actions, Play animations (the `render-lab-lamlet` skill, section 6). The library keeps
  what is heavy and exposes it to them through `Investigation::script_natives`; `render-lab --investigation <id>
  script check` checks them. A script edit shows on the next launch without rebuilding the library.

### The negotiation, in short

The loader fills a C structure, `Negotiation` (its size, its interface version, its lab build's fingerprint, and a map),
and calls the function. `lab::negotiate!` expands in the library: it checks the size, the version, and that the
fingerprint is the one the library was compiled against (`lab::plugin::ABI`: the lab's version, the compiler, a hash of
its sources), and only then registers each handle in the map through the loader's `register`. A library built against
another lab refuses before it runs any lab code; the loader never unloads a library. Handles are looked up by name
(`Handles::investigations`, `Handles::features`; a library without one has none); a new kind of handle is a new name and type in `lab/src/plugin.rs`, both sides.

## 6. A local package

A package that must not be published (its engine's license does not allow it) is a package like any other, with
three differences:

- Its directory is ignored with a pattern in `.git/info/exclude` (never `.gitignore`), and nothing published names it.
- Its code is a Cargo workspace of its own (`[workspace]` in its `Cargo.toml`, its own `Cargo.lock`), so the
  published `Cargo.lock` never names it. Seed its lock with a copy of the root `Cargo.lock`, and copy the root's
  `[profile.release]`.
- It links the lab through a symlink in its directory, `lab -> ../../lab`, and depends on `path = "lab"` (and
  `lab/check`). The lab's path from its workspace's root is then the same as from the repository's, so Cargo builds
  the same lab with the same symbols.

Build it **after** the repository's workspace, every time the lab changes:

```sh
cargo build --release --locked --offline                                              # the binary, the lab, the public packages
cargo build --release --locked --offline --manifest-path packages/<name>/Cargo.toml   # the local package
```

Each of the two recompiles the lab once (Cargo's freshness check includes the path it was built through), with the
same symbols; that is expected. A local package's tests run with `cargo test … --manifest-path packages/<name>/Cargo.toml`.

## 7. Packing a package to share it

A built package is the packed form: copy its folder (`target/release/packages/<name>/`, following its hard links,
`cp -rL`) and drop it into the receiver's `packages/` beside their binary. It is self-contained: its manifest, its
built resources, and its library. A built package that did not come from the receiver's checkout (its `cooked-from`
is not theirs) is read as it is and never rebuilt or removed by their build; one with the name of a source package of
theirs is an error, so rename one of them.

- A content-only package (a shared replay: its demos' timeline, audio, and frames, with the scenes they show) needs
  nothing else.
- A package with code carries its library in `lib/`. It loads only into the identical lab build (the fingerprint), so
  between different builds share the source package instead, and let the receiver build it.

## 8. Commands and errors

```sh
cargo run --release -- packages     # each built package: author, license, imports, exports, its library and handles, demos, series
cargo run --release -- series       # each series' parts, their investigations and timelines, and the knowledge they carry on
cargo run --release -- resources    # every resource by full key: kind, its file's size (or not built), license, source
cargo run --release -- cook         # build the checkout's packages beside the binary (every launch does it), and report it
```

`RENDER_LAB_ROOT` (or `--root`) names the source checkout whose packages are built (by default the one the lab was
built from; a binary without one only reads what is beside it). `RENDER_LAB_PACKAGES` is the built packages' folder,
instead of `packages/` beside the binary.

| Message | Meaning |
|---|---|
| `imports `x`, which is not a package of packages` | add the package, or fix `import` |
| `the imports of … form a cycle` | two packages import each other, directly or not |
| `` `p.k`: the package `p` does not export `k` `` | add `k` to `p`'s `export`, or name another key |
| `` names the package `p`, which `import` does not list `` | add `p` to `import` |
| `exports `k`, which it does not declare` | a typo in `export`, or a missing resource |
| `` overrides `…`, which is not `overridable` `` / `is overridden by … and …` | the override rules (section 3) |
| `library …: not built` | build the workspace (or the local package, section 6); the next launch links it in |
| `… was not built from … (it came from elsewhere)` | a shared built package has a source package's name: rename one |
| `packages: no …/packages` | no built packages beside the binary and no checkout to build them from |
| `library …: … undefined symbol …` | the library was built against an older lab: rebuild it |
| `negotiation …: it was built against another build of the lab` | the lab changed since: rebuild the library |
| `no negotiation function `…`` | the manifest's `negotiate` does not match `lab::negotiate!`'s symbol |
| `no investigation is installed` | no library loaded: read the messages above it |

A package whose library fails is reported and its investigations are left out; its content still mounts.

## 9. Checks after changing a package

Every change:

1. `cargo build --release --locked --offline` (and each local package's workspace, after it) with no warnings; its
   `build.rs` has checked its shaders and text.
2. `render-lab packages` (the package is listed, its library loaded with its handles) and `render-lab resources` (each
   new resource built, not `not built` unless a tool has yet to write it).

Then what the change touches:

- Content an investigation shows (a scene, a demonstration, a shader, text): the investigation's selftest and an
  offscreen snapshot of it, looked at; a demo entry, its timeline in the homepage autotest.
- A format or the loader (`lab/src/packages.rs`, `lab/src/pipeline/`, `lab/src/cook.rs`, `lab/src/plugin.rs`): the
  unit tests (`cargo test --release --locked --offline --workspace`), the homepage autotest, every investigation's
  selftest, and the frame identity check (AGENTS.md, Validation).
- Anything in `lab/` or `packages/render_lab`: the frame identity check, and every local package's workspace rebuilt.
- A public package's files, or anything near the public/local boundary: the public build (AGENTS.md, Validation).
