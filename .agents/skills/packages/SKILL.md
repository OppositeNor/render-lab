---
name: packages
description: Create, change, build, and troubleshoot Render Lab packages, the self-contained folders the lab loads at run time (sources in the checkout's packages/, built into packages/ beside the binary). Covers the manifest (name, author, license, import, export, library, negotiate, provides), resource collections and their kinds, full keys and references between packages, overrides, built files (timelines, narration), demos (a timeline and its audio), a package's code (a cdylib that negotiates with the loader through lab::negotiate! and hands it investigations), local packages with a Cargo workspace of their own, packing a package to share it, the build of the packages beside the binary, and the loader's errors. Use when adding or editing a manifest.toml or a collection, adding a resource, scene, shader, text table, or demo, giving a package code, when a package or its library does not load, or when asked how packages, the loader, or the negotiation work.
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

[[provides]]
type = "resource"                    # a collection, relative to the manifest
path = "resources.toml"

[[provides]]
type = "demo"                        # a narrated timeline the homepage's Watch plays
key = "video"                        # letters, digits, `-`, `_`; unique in the package
title = "How My Feature Works"
edition = "English"                  # its name on the homepage's edition picker
timeline = "timeline/en"             # resources (keys of this package, or `<package>.<key>`)
audio = "narration/en"
```

- Unknown keys are errors. `library` and `negotiate` come together or not at all.
- Packages load after the packages they import (then by name); an import that is not in `packages/`, or a cycle, is
  an error. The homepage lists investigations in that order.
- Every `export` must name a resource or shader the package declares.

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
  demonstration's `scene`): a plain key is the package's own; `<package>.<key>` must name a package in `import` and a
  key that package exports. Anything else is an error that names the manifest and the rule.
- **Kinds.** `file` is the source's bytes as they are; `directory` is a folder of files read where they are (the
  diagram frames a timeline names beside it). Both are linked into the built package at their paths. `radiance` makes
  an HDRI into the environment's levels, and `mesh` makes an OBJ into a BVH, at `cooked/<key>.bin`. The build remakes
  an output only when its stamp changes (its kind, processor code, parameters, and its source's size and time). A new
  kind is a processor in `lab/src/pipeline/`. A source must be inside its package.
- **Overrides.** A resource marked `overridable = true` (and exported) may be replaced, for everyone, by a resource of
  a package that imports it: `override = "render_lab.wgsl/surface-lookup"`. Two packages may not override the same key.
- **Tool outputs.** `built = true` marks a file (or directory) a tool writes into the source package
  (`source = "build/timelines/en/timeline.json"`). It may be missing until the tool runs; the next launch builds it
  into the built package, and `render-lab resources` shows `not built` until then. Demos name such resources:
  tools/video writes them. Declare a timeline's diagram frames too (`kind = "directory"`, `build/timelines/en/mermaid`).
- **Text.** A package's on-screen text is `text/en.json` and `text/zh-CN.json`, declared as `text/en` and `text/zh-cn`,
  read by the code with `lab::i18n::Table::new("<package>.text")` and `tr("key")`. Data files name text keys in
  fields ending in `-text` (`title-text = "scene.garden"`).

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
- Code that used `crate::lab::…` inside the old single crate now writes `lab::…`. A lab item the library uses must be
  `pub` (not `pub(crate)`), and a lab macro `#[macro_export]`.
- Add a public package to the root `Cargo.toml`'s `[workspace] members` and `default-members`, so
  `cargo build --release --locked --offline` builds it.

### The negotiation, in short

The loader fills a C structure, `Negotiation` (its size, its interface version, its lab build's fingerprint, and a map),
and calls the function. `lab::negotiate!` expands in the library: it checks the size, the version, and that the
fingerprint is the one the library was compiled against (`lab::plugin::ABI`: the lab's version, the compiler, a hash of
its sources), and only then registers each handle in the map through the loader's `register`. A library built against
another lab refuses before it runs any lab code; the loader never unloads a library. Handles are looked up by name
(`Handles::investigations`); a new kind of handle is a new name and type in `lab/src/plugin.rs`, both sides.

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
cargo run --release -- packages     # each built package: author, license, imports, exports, its library and handles, demos
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

1. `cargo build --release --locked --offline` (and each local package's workspace) with no warnings; its `build.rs`
   has checked its shaders and text.
2. `render-lab packages`: the package is listed, its library loaded with its handles.
3. The checks in `AGENTS.md`'s Validation for what the change touches (the homepage autotest, the investigation's
   selftest, its video's `--snapshots-only` build); after touching `lab/` or `packages/render_lab`, check that the
   investigations' frames are byte-identical (decoded pixels).
4. For a public package, the public build: copy the files `git` would publish into an empty directory, link
   `vendor/`, and build there (AGENTS.md).
