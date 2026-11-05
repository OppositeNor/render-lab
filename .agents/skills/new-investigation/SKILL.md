---
name: new-investigation
description: Create or change a Render Lab investigation of a game-engine feature (a package, packages/<engine>_<feature>/, whose library implements the Investigation trait) and hold it to the precision standard every investigation meets. Use when asked to investigate a new engine or feature, port an engine's shader to WGSL with a CPU mirror and selftest, give an investigation demonstrations, values, sliders, views, a source index, pseudo-code, formulas, a Play animation, actions, images, or its video's inputs and build, or to verify, audit, or correct an investigation's claims and source references against the engine's source. For the story's narration use `new-story`; for manifests and loading use `packages`; for passes and resources use `render-graph`.
---

# An investigation, and the standard it meets

An investigation is one engine feature, read from the engine's source and shown in the lab's study room: a set of
demonstrations with sliders, a readout or struct panel, Python pseudo-code of the engine's functions, a source index
that cites every claim by file, symbol, and line, and a narrated video. The lab (`lab/`) is shared and knows nothing
about any investigation; everything specific reaches it through the `Investigation` trait (`lab/src/investigation.rs`).

The worked example is the Open 3D Engine's lighting model, `packages/o3de_lighting/src/o3de/lighting/`. Copy its
shape; read its files alongside these steps.

## Choose the depth

AGENTS.md, "How deep a task goes", sets the levels; for an investigation they are:

| Task | Do |
|---|---|
| **Local**: one source-index entry, one listing, a value's default or help, a slider, one demonstration's view | Read the cited engine line and its function at the recorded commit; make the change through its script or format (`references/sources.md`, `references/code.md`); run the build, the selftest, and the snapshot of what it shows; check the narration and callouts that use what changed |
| **Subsystem**: a new demonstration, a port of another shader function, new GPU work, a commit change, a pass restructured | Procedure steps 1 to 6 for that part, the audit (step 8) of every claim it touches, all of "Check" below |
| **New investigation** | Every section, in order; the story by `new-story` |

The precision standard below holds at every depth.

## 0. Decide where it lives

- Read the engine's license in the checkout first; if it is unclear whether it allows publishing code derived from
  it, ask the user before writing any.
- **The engine's license allows publishing its code** (O3DE: Apache-2.0 or MIT): a package,
  `packages/<engine>_<feature>/`, published. Code ported from the engine keeps its copyright; add a row to
  `THIRD_PARTY.md` and its license text to `LICENSES/`, and give the package its `[[notice]]` (the `packages` skill,
  "The manifest").
- **It does not**: it belongs in the optional `commercial` module, which is not provided in this repo and stays local;
  nothing published may name the engine or reflect the module's contents. Read the module's own `AGENTS.md` files
  first, and the `packages` skill's "A local package".
- A public investigation never depends on, cites, or names a commercial one.
- Ask the user where the engine's source is (or read its variable, e.g. `O3DE_SOURCE_DIR`); never write its location
  into a file, since the user may move it. Keep it read-only, and record its version and commit in the source index.

## 1. Precision: the standard every investigation meets

An investigation is only as good as its claims. Each of these is required, not a polish step:

- **Cited and checked.** Every claim about the engine names a file, a symbol, and a line through the source index,
  read from the checkout at the recorded commit. Never narrate from memory, from documentation, or from a function's
  name: read the line and the function around it.
- **What runs, not what exists.** Follow the call chain from the entry point the pipeline really executes (the pass,
  then its shader, then the functions it calls) before saying "the engine does X". Check every compile-time switch on
  the way: `#define`s, `#if` blocks, shader options and permutations, material-type property defaults, console
  variables, quality levels, and platform paths. A function that is defined but compiled out, never called by this
  pipeline, or only used by another material is narrated as such ("it is compiled out", "only the Skin material calls
  it"), never as what runs. A branch that cannot be taken under the shipped defaults (a clamp that prevents its
  condition, say) is called out, not explained as if it fires.
- **This version.** Claims are about the recorded version and commit; nothing carries over from another version, or
  from how similar systems usually work.
- **Numbers as the code has them.** Formats by name (R11G11B10 float, RGBA8, R16G16B16A16 float), sizes, sample and
  step counts, mip counts, limits ("at most four frames"), thresholds, epsilons and clamps, and the order of
  operations. When the value depends on configuration, give the default the video assumes and name what changes it
  ("full resolution by default, half when the quality setting is low").
- **The lab's numbers are measured.** A count or value that comes from the lab's scene ("forty-three of sixty-four
  tiles keep a light", "about fifty-six percent") is computed by the lab's code and, where it matters, asserted by the
  selftest; never estimated.
- **Say where the lab differs.** Scale, resolution, prepared or offline data, a CPU stand-in for a GPU pass, a
  schematic view: say it in the narration where it matters ("this lab's diffuse cubemap is thirty-two texels a face"),
  label schematic or scaled views on screen, and list every difference in the README's "Simplified" list. The lab's
  own demonstrations are never presented as the engine's behaviour.
- **Records map to the engine's structures.** Each record or readout the lab shows has an entry in the index's
  `mappings`: the engine structure it stands for, whether the relationship is exact or conceptual, the engine's exact
  layout (`exact`), and the lab's (`labLayout`).
- **Mirror, port, and test.** Shader code the video explains is ported (WGSL) and mirrored on the CPU, and the
  selftest compares them term by term, plus invariants the method promises (energy conservation, a white furnace, a
  normalised distribution, footprints that stay inside their tile). A deviation the port uncovers (an unguarded square
  root, a precision loss) is reported as a finding with its evidence, not silently fixed in the "follows the engine"
  path.
- **Coverage.** Every factual sentence of the narration is backed by a `callout` on that sentence or by its chapter's
  `sources`; aim for a callout on about half of all sentences. Every chapter has a reading table in the index's
  `chapters`, and every entry has `inspect` saying precisely what the line does, plus `details` for the other lines a
  claim depends on (each with its own hash and note), translated in the package's `text/sources/<locale>.ftl`.
- **Corrections are reported.** When verification contradicts a request, an earlier statement, or a common belief,
  say so to the user with the file:line evidence, and narrate the corrected version.

## 2. Procedure

For the story (the research, the outline, the storyboard, and its review, in phases), follow the `new-story` skill; it
builds on these steps.

1. **Scope.** Name the feature's entry points and list its passes in frame order. Note the engine version and commit.
2. **Sweep.** For a broad question (every pass a feature touches, every caller of a function), delegate the search
   when your environment has search or research subagents: give the agent the checkout path and the question, and ask
   for file, symbol, and line triples. Without one, search the checkout yourself (by symbol, then by its callers),
   recording each triple as you find it. Verify each triple yourself, by reading the line and its function, before
   using it.
3. **Index.** Write the entries with a script that reads the checkout and fills the hashes (the investigation's
   `source_index.py`), so lines and hashes never drift by hand (`references/sources.md`).
4. **Port, mirror, test.** Port the shader code, write the CPU mirror, and add the selftest comparisons and invariants
   before building demonstrations on them (`references/code.md`).
5. **Listings and formulas.** One pseudo-code listing per shading step, mirroring the engine function's names and
   steps; formulas for the readout where a demonstration evaluates one (`references/sources.md`).
6. **Demonstrations.** Step parameters for every intermediate step the video will show (`references/code.md`,
   `references/presentation.md`).
7. **Storyboard.** Draft each factual sentence from an index entry and attach its callout as you write it. Use spoken
   numbers, and say where the lab differs (`new-story`).
8. **Audit.** Before the full build, have a check not anchored on your drafts verify every factual sentence and every
   `inspect` against the checkout, with file:line evidence: a fresh agent that has not seen the drafting, when your
   environment has subagents; otherwise yourself, working from the checkout and the source index alone, claim by
   claim, writing each verdict and its evidence down before moving to the next. Classify each as **verified**,
   **correct but simplified** (fine only if the narration or README says it is simplified), **potentially
   misleading**, **incorrect**, or **needs investigation**. Also check that the version and commit the story names are
   the checkout's, that no claim carries over from another version or from how similar systems usually work, and that
   the lab's own demonstrations are never presented as the engine's behaviour. Fix every claim not verified or stated
   as a simplification, in the narration, the index, the README, and the translation; an uncertain claim is verified,
   narrated as uncertain, or cut, never rewritten as fact.
9. **Build and review.** `--snapshots-only`, look at the stills (several per animated beat), then the full build in the
   background; report its checks and the corrections, and ask the user to watch it.

## 3. The details, by task

Load only the reference the task needs:

| Reference | For |
|---|---|
| `references/code.md` | the package's layout and registration, the trait's methods, demonstrations, values (struct fields and WGSL tags), choices, help texts, GPU work (volumes, updates, history, programs), `build_frame`, views, step parameters, the dock, text, data files |
| `references/sources.md` | `source-index.json` and its script, updating an entry, pseudo-code listings, readout formulas |
| `references/presentation.md` | the Play animation, actions, published images and annotations, image click handlers, GPU image kinds, the video's inputs (`video.json`, storyboards, diagrams), registering timelines, and the video build |

Other skills own the neighbouring formats: `packages` (manifest, collections, keys, negotiation, local packages),
`render-graph` (a frame's passes and resources, the inspector), `experiments` (engine settings as values, experiment
files), `render-lab-lamlet` (storyboard syntax, live scripts), `knowledge-graph` (chapters' requirements, the
audience).

## 4. Check

For a subsystem change or a new investigation, all of this; a local change runs the build, the selftest, and the
snapshot of what it changed (and the public build when it touched a published file). From the lab's root, with
`CARGO_HOME=$PWD/.cache/cargo`:

```sh
cargo build --release --locked --offline                                    # no warnings
cargo run --release -- --investigation <engine>/<feature> selftest --report <scratch>/selftest.json [--timeline …]
cargo run --release -- home --autotest <scratch>/home.json                  # the homepage, offscreen
cargo run --release -- home --snapshot <scratch>/home.png                   # look at it
```

- What the change shows, offscreen: `home --snapshot … --page viewer:<demo>` for a demonstration,
  `viewer-code:<listing>` for a listing, `render-lab graph` and `viewer-graph` for passes (AGENTS.md lists the pages).
- Build the public-only tree too (AGENTS.md, Validation: the files `git ls-files --cached --others
  --exclude-standard` lists, `vendor/` linked) without warnings, and each local package's workspace after the main
  one.
- If you touched `lab/` or `packages/render_lab`, run the frame identity check (AGENTS.md, Validation) on the other
  investigations.
- Never open windows or take screenshots on the user's screen; look at offscreen renders.
- Keep the investigation's `README.md` current: what it shows, its demonstrations and sliders, what the lab simplifies,
  its files, the video, and how to verify. A new investigation is added to the root `README.md`.
