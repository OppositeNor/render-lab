# Lamlet interpreter integration for Render Lab (plan)

Render Lab is a primary consumer of Lamlet: it needs authored logic inside rich text and stories, static content
before speech synthesis, and cheap deterministic evaluation during interactive playback and recording. Implement
Lamlet as an independent language library, then integrate it through consumer-owned packages and adapters here.
This plan records the actual use cases and integration gates; it does not implement the interpreter or migrate content.

## Repository boundary

- Lamlet's repository owns language semantics, std, scanner/checker/VM, generic package/native interfaces,
  artifacts, CLI, synthetic hosts, and benchmarks. It must contain no Render Lab names, paths, formats, examples,
  edition policies, integration records, or dependencies.
- This repository owns this plan, the [rlstd language library](lamlet-rlstd.md), `.story`/translation policy,
  presentation types, native bindings, package-resource mapping, timeline scheduling, viewer/recorder integration,
  and consumer fixtures/baselines. The dependency points from this project to Lamlet.
- Generic requirements may inform Lamlet's implementation without naming their consumer: preserved source spans,
  explicit static evaluation, metadata-only checking, immutable typed results, repeated calls, explicit input
  snapshots, structured CLI batches, portable artifacts, native contracts, and resource limits.

The standalone [interpreter plan](../../../../lamlet/plans/lamlet-interpreter.md) and
[language contract](../../../../lamlet/plans/lamlet-plan.md) are authoritative for the core. These relative links
refer to the sibling checkout arrangement; the installation/runtime interface must not depend on that layout.

## What the current lab does

| Path | Evidence from the implementation | Required use of Lamlet |
|---|---|---|
| Rich text, labels, captions, readouts | [rich_text.rs](../lab/src/rich_text.rs) defines Text/Formula/Code runs at lines 19–27. `pieces` parses at 253–280; `measure` and `text` independently consume it at 296–334. [ui.rs](../lab/src/ui.rs) measures labels then draws them, and draws captions twice for shadow/foreground. | Evaluate source before layout. Both measurement and drawing consume the same immutable prepared value; repeated passes must not execute scripts or register effects. |
| Localized values in text | [i18n.rs](../lab/src/i18n.rs), lines 65–127, lazily loads JSON tables, fills named `{name}` arguments, handles brace escapes, and preserves attribute tags. [check/lib.rs](../lab/check/src/lib.rs), lines 215–351, harvests keys and checks languages, placeholders, formulas, missing/unused entries. | Pass typed arguments to checked message functions. Keep missing/unknown/unused-key and locale checks, and never turn substituted/returned Strings into new code. |
| Formula preparation and rendering | [formula.rs](../lab/src/formula.rs), lines 83–123, checks formula-bearing files; rich text forces formulas inline while cards use display layout. | rlstd owns Formula descriptions, spoken alternatives, and layout limits. The VM returns data; it does not own a typesetter or GPU objects. |
| Offline narrated stories | [build.py](../tools/video/build.py), lines 182–217, validates, synthesizes narration, builds timed output, checks formulas, writes exports, then returns for `--prepare-only`. [story.py](../tools/video/story.py) treats paragraphs as sentences. | Prepare text and typed descriptions before TTS. Preserve source structure and multiline code before sentence splitting; use one Rust compiler through a batch interface. |
| Word-timed cues/captions | [narration.py](../tools/video/narration.py), lines 180–312, returns word starts and caches audio; [timeline.py](../tools/video/timeline.py), lines 147–244, maps cue word positions to frames and protects formula/code caption spans. | Resolve anchors against the actual prepared spoken words, with stable sentence/cue IDs independent of identical text. Resolve timing after synthesis. |
| Translated editions | [zh.py](../tools/video/zh.py), lines 21–53, checks chapter/beat correspondence, one translated sentence per English sentence, matching formulas, and absence of control cues. [build.py](../tools/video/build.py), lines 178–181, requires English timing first. | Keep English audio/timing and translated presentation policy here. Translation does not duplicate control logic or execute new host writes. |
| Typed controls/actions/images | [values.rs](../lab/src/values.rs) defines scalar/vector types, schema entries, and choices. [actions.rs](../lab/src/actions.rs), lines 69–110 and 211–295, defines typed arguments, action calls, and running actions. [views.rs](../lab/src/views.rs) and [inset.rs](../packages/rlstd/src/inset.rs) define image schemas/instances. | Export typed host APIs and immutable descriptors through rlstd. Validate keys, value types/ranges, choices, actions, and image parameters before applying output. |
| Direct seek and recording | [shot.rs](../lab/src/shot.rs), lines 800–1025, validates timelines and computes `state_at(frame)` from keys. [record.rs](../lab/src/record.rs), lines 223–256, evaluates each requested frame independently with explicit time and history disabled. | Reuse compiled code with an explicit requested-frame snapshot. Results cannot depend on having visited preceding frames or crossed a cue during playback. |
| Viewer interaction | [viewer.rs](../lab/src/viewer.rs), lines 302–319, distinguishes interactive and rendered state; 435–486 controls pause/seek; 762–789 dispatches image clicks with shown state and mutable state. | Separate timeline-owned computed state, displayed state, and user-editable state. Evaluate handlers in an explicit event context and apply validated commands at a defined update point. |
| Packages/build identity | [packages.rs](../lab/src/packages.rs), [cook.rs](../lab/src/cook.rs), and [pipeline/mod.rs](../lab/src/pipeline/mod.rs) enforce imports/exports and incrementally cook resources. [build.rs](../lab/build.rs), lines 17–45, fingerprints lab sources/Cargo.toml; [plugin.rs](../lab/src/plugin.rs) checks exact ABI compatibility. | Adapt the language resolver to built package resources. Include language/source/interface dependencies in artifact and ABI invalidation; current size/mtime stamps alone are insufficient. |

These findings concern public shared paths. No investigation-specific implementation belongs in the language
core or shared lab merely because a script can call it.

## Interpreter suitability requirements

| Requirement supplied to the general interpreter design | Why the lab needs it |
|---|---|
| Source spans and ignored text survive compilation | Prose stays visible while only dollar regions execute; prepared output must retain sentence/cue provenance. |
| Metadata-only check without native runtime initialization | `lab/build.rs` uses `lab/check` before lab compilation; that build dependency cannot link back to lab. |
| The same VM handles explicit preparation and runtime calls | Captions, generated documents, live labels, viewer state, and recorded state must not acquire separate semantics. |
| Exact stage/capability checks through closures and higher-order calls | Runtime reads cannot enter speech preparation through a hidden callback or type annotation. |
| Reusable compiled programs and batch calls | Labels/captions may be used multiple times per update; parsing or subprocesses cannot be a draw-time dependency. |
| Isolated instance/context with bounded allocation/work | A document error must not hang interaction, leak through reloads, or corrupt the next recorded sample. |
| Explicit deterministic inputs and portable prepared constants | Direct seek, out-of-order frames, offline tools, and separate processes must reconstruct equivalent results. |
| Structured errors and dependency fingerprints | Authors need source locations; stale native schemas, source imports, or prepared text must invalidate the right outputs. |

Those are generic API/workload requirements. Their concrete mappings, names, resource keys, and consumer tests stay here.

## Proposed ownership and call path

1. The lab and its build checker use the same version of the independent Lamlet library. Build-time checking uses
   source/type/native metadata and no viewer/device initialization. The lab embeds reusable runtime instances.
2. `packages/rlstd/language/` contains the authored document/presentation/formula/timeline APIs. Native adapters
   live with rlstd's existing package code, compiled as `lab::utility`; do not assume a new rlstd cdylib.
   Investigation packages export their own capabilities through generic schemas/extension points.
3. A lab-side resolver maps a language package/module to declared built resource keys and checks both the lab
   manifest import/export rules and the language export table. The language never calls `lab::resources` itself.
4. `tools/video` sends structured batches to a lab-configured Lamlet preparation helper/CLI. Python orchestrates
   source/translation/TTS/timing/export; it does not reimplement expressions. Its isolated `-I` launch is compatible
   with an explicit subprocess/JSON interface. Interactive evaluation remains in-process Rust.
5. A host preparation layer combines retained prose and evaluated source regions into typed presentation items,
   spoken projections, stable anchors, and deferred effects. Layout/rendering consume prepared items only.

Use one application configuration of the generic interpreter, rather than a separate interpreter in every package.
Host schemas and native callback implementations are separate: a checker can validate a declaration without
instantiating runtime handles. Explicit preparation that calls a native Pure/Static helper requires a linked,
CPU-safe implementation. Prefer ordinary Lamlet constructors for portable records/descriptions. If a validator
must be shared between runtime and build scripts, keep the pure code in a package-owned dependency without a lab
runtime dependency; do not introduce a `lab/check -> lab` cycle.

## Preparation stages

### A. Authored source to static document

Recognize dollar regions before `.story` paragraph/sentence splitting. Retain ignored text as prose and exact
source spans. Compile imports/functions/types once, evaluate explicit static expressions, and obtain presentation
values, spoken alternatives, stable sentence IDs, and unresolved cue descriptions. An `AnimationTrigger` is data,
not an executed action. Static evaluation cannot read live controls, clicks, runtime time, or future word timestamps.

Use source/chapter/beat/sentence identities, not only expanded text, to distinguish repeated identical sentences.
Preserve the existing after-final-word cue policy unless explicitly changed: normal cues fail there, while range
closes may use sentence end. A failed expansion or missing speech alternative is an authoring diagnostic before TTS.

### B. Speech and timing resolution

Send the exact prepared spoken text through pronunciation substitutions and TTS. Resolve cue descriptions against
the returned words/timestamps. Retain both source and prepared-text anchors; character offsets into original
markup are not a replacement for word alignment. Derive subtitle/caption timing from that same prepared result.

Do not invent a third core language stage for TTS. After timing exists, the host can consume cue descriptors and,
if necessary, call already checked pure functions with measured times as explicit inputs in a second preparation
batch. An earlier `$[% ...]` cannot read missing future timing and then silently retry as runtime code.

Keep the current edition policy in the host: English narration/timing first, matching translated sentence/beat
structure, formula consistency, and no translated control cues. Record the actual prepared speech and its inputs
in audio/timing cache identities, including voice/pronunciation/preparation versions and language dependencies.
Current audio caching by spoken text/voice/speed and timeline lookup by sentence text need deliberate migration.
Translated caption segments currently use display-width proportions within the shared English sentence interval;
English caption segments use word timestamps. Preserve that approximation explicitly or record a tested timing
policy change; translated caption boundaries are not measured Chinese word alignment.

### C. Prepared programs to runtime bindings

Load versioned constants/functions and resolve native handles before active playback. Portable artifacts contain
no process pointers or unencoded opaque handles. Bind only the relevant package/scene's resources when needed;
do not eagerly load fonts/images/scene data merely to enumerate or type-check packages.

The host returns ready-to-layout items for each source key, locale, argument snapshot, and active binding. Reuse
those same items across fitting, measurement, shadow/foreground drawing, and export. Typesetting/layout caches
are distinct from VM result caches so repeated formula work is visible in profiling.

## Runtime inputs, updates, and failure behavior

Each evaluation batch receives an immutable snapshot of selected investigation/package schemas, typed parameter
values, logical time/frame index and fps, applicable elapsed cue time, locale/message arguments, and event payload.
Playback/recording derive time through the same helper from the requested frame; no ambient wall-clock callback
is used to reconstruct a recorded frame. Native reads remain Runtime even when a particular snapshot looks constant.

Immutable module initializers run once; they are not implicit reactive controls. Live labels/bindings must call
functions with the current snapshot or run a document entry with fresh local bindings. Reusing compiled code
does not mean reusing a stale native read captured during the first initialization. Preserve source order when
executing a complete document entry; selecting/batching regions must not reorder observable calls.

Pure bindings can be called independently for any requested frame order. A timeline animation computes its state
from its anchor/interval and requested frame, preserving `Timeline::state_at`; firing callbacks only when a cue is
crossed is insufficient. Viewer clicks and interactive edits are different inputs from timeline-owned state. Define
pause/seek/reset/demo-switch ownership and don't let a click mutate a running timeline's reconstructed state by accident.

Collect command/effect descriptions during an event or explicit update evaluation, validate the complete batch,
then apply it at the host update boundary. Display/layout evaluation has no write capability. Domain natives must
not mutate the scene immediately and then leave partial changes when the VM exceeds a budget. Reject ambiguous
multiple writers to one target until explicit composition/precedence is specified. Repeated painting does not
restart animations or register another handler. Persistent simulations require an explicit event log/checkpoints
and are outside the initial binding implementation.

On error, discard the pending command batch. Offline preparation/recording reports a source-located failure and
does not reuse a corrupt artifact. Interactive presentation may keep the last successful value with an error
indicator, but logs are deduplicated and the next valid input can recover. Choose that visible policy before shipping.

## Typed native/schema adapters

Expose concrete functions and Types that fit the Native stub ABI. Key-dependent return types must not become an
unimplemented dependent FFI: use generated concrete getters/setters or typed families such as Float/Int/Bool/vector
accessors, with schema validation. Unknown dynamic keys/types remain checked host errors; known constant keys can
be checked earlier. Actions with heterogeneous arguments can get generated signatures from `ActionSpec`.

Preserve host schemas deliberately:

- Lamlet Int is checked 64-bit; host I32/U32 writes validate range and signedness rather than truncating.
- Host F32 values convert explicitly to/from finite Lamlet Float, with documented rounding/range behavior. Do not
  silently infer integer, Boolean, or vector semantics from a plain number.
- Choices use named options or typed descriptors; current internal indexes do not become the public source API.
- Validate vectors/components, ranges/wrap behavior, action required/optional arguments, image kinds/parameters,
  click payloads, and resource permissions before applying descriptors.
- Localization receives named typed arguments. Strings substituted into a message are values, not new code;
  translated presentation functions may use only the intended pure/read-only API and keep shared control IDs.

Opaque native values need explicit lifetime and portable encoding rules. The VM owns no GPU objects and native
callbacks follow its bounded/cooperative budget/error contract. The language's interface remains consumer-neutral.

## Build, distribution, and cache gates

Source scripts and generated artifacts stay inside their owning lab package; generated output lives under its
ignored `build/` and is declared as built resources. Add a generic script compilation processor only when its
inputs/outputs are defined. Keep resource loading lazy and use declared full keys/imports/exports.

Include the Lamlet implementation/STL version, compiled interface/native schema IDs, imported sources, static
inputs, locale/presentation dependencies, and relevant tool versions in script artifact keys. Include the selected
language dependency's compatibility identity in lab/plugin fingerprints; hashing only `lab/src` misses a changed
path dependency. Existing cook stamps include resource kind/processor/parameters and source size/mtime: an explicit
source/dependency hash is additionally needed for portable program identity, not a guess based only on timestamps.

Resolve dependency delivery before adding code: the public build and each local workspace must find one compatible
Lamlet crate/version with the locked offline workflow. A personal absolute sibling path is not a distributable
dependency. The current no-vendor-edits rule means an offline source/distribution method or an explicit dependency
policy change must be settled first; don't quietly break the public-only build to obtain a prototype. Local
development overrides stay local; the published build contract must stand on its own.

## Status (2026-11-01)

A first slice of stage A, without a Rust dependency on Lamlet: `tools/video/lamlet.py` expands a storyboard's
Lamlet regions with the `lamlet` command (`prepare` for the prose spans, `eval` for every region's value) before
story.py reads it, and `packages/rlstd/language/story.lam` is the beginning of rlstd's library (cues and images
written from typed values, spoken numbers). Tested by `tools/video/test_lamlet.py`. No published storyboard uses it
yet. Still open: Lamlet under version control, the adapter out of Lamlet's repository (`crates/lamlet-present`),
the text check seeing keys a region names, typed checks of keys/images/actions against the lab's registries
(natives), and values computed by package code (natives).
The in-process plan for those is [lamlet-in-process.md](lamlet-in-process.md); its language-side work is Lamlet's
`plans/embedding.md`.

## Integration milestones

1. **Baselines and boundary.** Record current public story text/captions/cues and representative offscreen views,
   text-key/placeholder/formula checks, seeked states, and CPU layout costs. Keep all consumer records here.
   Confirm Lamlet's repository has no consumer names/paths/libraries/fixtures.
2. **Core readiness.** Pass the standalone scanner, checker, VM, static preparation, native registry, batch protocol,
   artifact, and resource-limit gates. Exercise equivalent requirements using synthetic fixtures there.
3. **Check/preparation bridge.** Add the dependency through the agreed offline mechanism; expose package metadata
   to `lab/check` and an offline helper. One public message/story beat compiles, including a multiline lambda,
   typed arguments, static text, presentation values, and useful source diagnostics. No GPU/window is initialized.
4. **Presentation.** Supply rlstd modules/adapters and change rich text to consume prepared runs. Preserve plain
   text and formula fallback policy deliberately. Verify measurement and repeated drawing cause zero evaluations
   or writes, and keep missing/unused/locale checks intact.
5. **Stories.** Evaluate expansions before TTS, resolve stable cue IDs after word alignment, and produce both
   editions from prepared projections. Python has no independent expression semantics. Invalidate caches when
   prepared speech or dependencies change.
6. **Live/recorded behavior.** Bind typed reads and event commands; validate snapshots, writer conflicts, pause/seek,
   reset, and failure policy. The same runtime context builder feeds viewer and recorder. Check arbitrary frame order.
7. **Migration and performance.** Select old/new content explicitly by a version boundary, not parse-success
   guessing. Migrate public content/docs/skills and remove duplicate scanners after consumers move. Keep shader
   attribute metadata separate and non-executable. Record warm call/allocation/layout/type-setting costs and set
   per-update budgets from measurements rather than assuming the VM dominates all text work.

## Validation when implementation begins

Use core semantic tests in Lamlet and consumer integration tests here. This planning change needs no renderer build.
Implementation changes must run the applicable root AGENTS.md checks: locked offline build/tests, local workspaces
after the lab, public-only build, affected Python story/markup tests, formula checks, and relevant offscreen views.
Run one GPU job at a time; never open windows or capture the user's screen.

Consumer cases must include repeated layout/paint with zero script calls; typed localized args with code-looking
Strings; Runtime-to-speech rejection through a callback; repeated identical sentences with distinct anchors;
formula/code caption protection; translation parity/control restrictions; descriptor batch rollback; direct seek
and out-of-order recorded frames; paused/playing interactive ownership; resource laziness; source/native/compiler
cache invalidation; and bounded failures with recovery. Benchmark prepare/compile, warm evaluation, native marshaling,
typesetting, layout, and rendering separately. A passing isolated interpreter benchmark is not sufficient evidence
that the integrated UI has the required cost or correct timing.
