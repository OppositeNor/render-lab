# Checks the lab does on Lamlet's values, moved into Lamlet's types (plan, 2026-11-05)

Status (2026-11-05): the first, trivial step is done: render_lab's points, marks, hide lists, and records cross as
typed Data, and `tools/video/markup.py` has no String grammar for them. Stage 1 is done too, through lamlet-present
0.3.0 (556754d, 6f4cad8, 41f4339). The vocabulary writes typed data, the lab's option lists are closed, and every
cue attribute and heading attribute is checked against the key list (`render-lab lamlet ir --keys`), with the
headings read by the lab's heading keys. Stages 2 to 5 are open. All of this is lab-side work that needs no change
to the Lamlet language.

Out of scope: checks across documents or of a storyboard's structure (the zh-CN edition's alignment, unique ids,
heading order, "a beat has narration", budgets), and checks against investigation data (source keys, text keys,
published points, demos, listings, LaTeX). Those lookups already run in Lamlet, through the `rl` natives at the
storyboard's line. `lab/src/story_refs.rs` repeats them over the timeline as a backstop for raw `{attr}`s, and the
backstop stays: abstract key types in Lamlet would let a text key or callout be checked only once.

Not a goal: refusing values outside a value's `range`. That range is the dock slider's, and a story may go past it
(`test_numeric_cue_values_are_finite_without_slider_clamping`). The limits a type carries (a whole number, u32's
bounds) are kept.

## Stage 1: cue values as Data (done, with lamlet-present 0.3.0)

Done:
- The vocabulary writes typed data. `markup.py` decodes it with no grammar of its own: a range is
  `{from, to}`, and `stop` is a list.
- The lab's option lists are closed (`keys.rs` `closed`). render_lab's typed keys are `any`, and `image.<param>`
  and `view.<name>.<key>` are families.
- `lamlet ir --keys` (`script::apply_keys`) checks every cue attribute, `attr`s too, and reports every problem with
  the nearest key. `markup.py` lost its option, ease, and camera checks.
- The headings are read by the lab's heading keys (`keys::heading_keys`), with the pacing defaults. `headings.py`
  keeps only required keys and limits.

Left from it, small:
- `card` and `display` as Bools rather than closed `yes`/`no`.
- `markup.TYPED` and `timeline.KEY_FIELDS` read from `story keys` instead of their own copies.
- Values stay `float` whatever their type: storyboards write `1.0` for toggles and indices, and the timeline
  blends them all.

## Stage 2: typed hook results

`runtime.rs` decodes a hook's `Shape`, `Row`, and `Point` from untyped Data by hand (`field`, `number`, `text`,
`point`, at about 675-752), although `runtime.lam` types them. It also parses a colour String (`color`, 709-722:
four theme names, or `#rrggbb`).

- A colour becomes a variant in `runtime.lam`: the four theme colours as constructors, and `{rgb "#rrggbb"}` checked
  by a Lamlet function. The named colours (`normalColor` and the others) are static bindings (`[% …]`), so a wrong
  hex is refused when the script loads, not when it runs. Every script that writes a colour changes: so far only
  O3DE's `overlay.lam` (14) and `readout.lam` (4).
- `lab::lamlet_record!` also implements `FromLamlet` (the pinned `embed.rs` has the trait, line 662). Shapes, rows,
  and points are then decoded by type, which replaces the hand-written walking.
- Checks: every investigation's `script check` and selftest, the snapshots of the demonstrations the scripts draw,
  and the frame identity check (AGENTS.md), because this changes `lab/` and what overlays draw.

## Stage 3: a hook that almost matches is an error

`runtime.rs` `load` (about 470-505) treats a binding as a hook only when its function type matches exactly. Anything
else is ignored, so a hook with the wrong parameter type is silently dropped. Instead, a public top-level function
whose result is a hook's (`List Shape`, `List Row`, `List Write`) but whose parameters match no hook becomes an error
at load, naming the hook types it resembles. First check that no public helper in a package's scripts has such a
result (`script stubs`); make any that does private (`_`).

## Stage 4: finite numbers from hooks (a gap found on the way)

Nothing in `runtime.rs` refuses NaN or infinity from a hook. Writes (`typed`) and shapes go straight to `f32`, while
`markup.py`'s `number` refuses non-finite numbers. Refuse them in `writes` and `shapes` with the key or shape named,
so a failing hook keeps its last good result (in strict mode, it fails). This is the one check in the plan that stays
in Rust: Lamlet's Float includes NaN. Checks: as stage 2.

## Stage 5: text keys found by the parser, not by substrings

`lab/check` (`text_keys_used`, `lib.rs:279-355`) finds the text keys a `.lam` file uses by matching substrings on
each line. It misses a key that isn't a literal on the line of its call (`{labelText key}` inside a helper). Use the
pinned `lamlet` parser in `lab/check` (a build dependency at the lab's version, with no new features). Find the calls
of `imageTitle`, `imageMarkText`, `labelText`, `textKey`, `titleText`, and `{attr "<text field>" …}`, and refuse a
call whose argument is not a literal String: a computed key is then an error rather than missing from the check.
Open question: how often a helper passes its key through. Grep the storyboards and stories before deciding whether
helpers write the key at the call site (as `story.lam` already must).

## Order

Stages 2 and 3, which touch the same code, then 4 and 5, which are independent. Commit
each stage locally once it is validated.
