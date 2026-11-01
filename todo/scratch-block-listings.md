# Scratch-style block view for pseudo-code listings (implemented 2026-11-01)

## As built (where it differs from the plan below)
- No new theme colours: the blocks use the theme's code colours (control = keyword, read = name, write = number,
  math = string, statement = dimmed text, hat = accent), so existing theme files, the user's included, stay valid.
- The dispatch text replaces the `def` only on a listing's first hat; later functions show their `def`.
- Spacing has five tiers, the last with blocks touching, so every listing fits the recording's panel in both styles.
- Blocks became the default (the settings, recordings, self-tests, and `tools/video/build.py`); text is the option.
- Recordings and self-tests use the default unless `--listing-style` says otherwise; `lab::pseudocode::with_style` lets a
  self-test check a listing in both styles.
- The marks and dispatch headers are in the godot_sdfgi listings; the other packages' listings have none yet.

## Context
The viewer's code panel shows each engine function as Python pseudo-code: up to 23 lines and about 8 nesting levels. The indentation is hard to follow in a video, and nothing shows which lines touch memory or how the code is dispatched on the GPU. We'll add a second listing style that draws the same `.py` files as Scratch-like blocks:
- each statement is a block;
- `for`, `while` and `if/elif/else` are C-blocks;
- a hat block on top states the dispatch;
- blocks are coloured by kind (control, read, write, math, statement);
- expressions stay as the current syntax-coloured monospace text.

Decisions (from the user):
- **Rollout.** A setting, `Listing style: text | blocks`, with text as the default. Snapshots and video builds take an explicit style.
- **Colours.** Control colour is inferred from keywords. Read, write and math come from optional per-line marks (`# @@read`, `# @@write`, `# @@math`). Lines with no mark use the statement colour.
- **Hat block.** An optional header, `# dispatch: English | 中文`. Without it, the hat shows the `def` signature.

The `.py` files must stay valid Python, because `tools/video/build.py` runs `compile()` on them. The new marks and header are comments.


## Design

### 1. Parse into a block tree (`lab/src/pseudocode.rs`)
- **`Listing::parse`**
  - Read the optional `# dispatch:` header into `dispatch`/`dispatch_zh`, with a `dispatch()` accessor like `title()`.
  - Split `# @@kind` marks out of the line's trailers into `Line::kind: Option<Kind>`, keeping them out of `tags`. Today `parse` splits at the last `# @`, so `@@` must be handled before that split.
  - Reject unknown kinds.
- **`pub enum Kind { Control, Read, Write, Math, Statement }`**, plus `Line::kind()`:
  1. an explicit mark wins;
  2. otherwise a line starting with `for`, `while`, `if`, `elif`, `else`, `return`, `break` or `continue` is Control;
  3. otherwise it is Statement.
- **`pub enum Block` and `pub fn blocks(&Listing) -> Vec<Block>`**
  - Variants:
    - `Hat { line }` for the `def`;
    - `Stmt { lines: Range }` for a statement plus its continuation lines;
    - `C { arms: Vec<(head_line, Vec<Block>)> }`, where `if`, `elif` and `else` are arms;
    - `Cap { line }` for `return`, `break` and `continue`;
    - `Note { line }` for a line that is only a comment;
    - `Gap` for a blank line.
  - Build the tree from the indentation stack.
  - A line whose indent is not a multiple of 4, or that follows an unclosed bracket, continues the previous statement. The survey found such lines (an 8.25-level indent).
- **Unit tests** in the same file:
  - nesting and elif/else arms;
  - continuation lines;
  - mark and dispatch parsing;
  - a line with both a comment and a tag (`x = f()  # 64^3  # @init`).

### 2. Style setting and override
- **`lab/src/config.rs`**
  - Add `listing_style: ListingStyle { Text, Blocks }` with `Default = Text`.
  - Add it to `load`, `to_toml` and `differs_in` (Interface).
  - Extend the round-trip test.
- **Global style.** Store the active style in a global like `palette::set_theme` / `palette::theme()`, for example `pseudocode::set_style()` / `style()` with an atomic. `draw_code` has no config parameter, so it reads the global.
- **Homepage.** In `lab/src/home.rs`, add a control to the Interface category, following the `theme`/`language` control. Apply the style on save.
- **Text keys.** Add labels for the setting and both options to `packages/rlstd/text/en.json` and `zh-CN.json`. The build fails on a missing or unused key.
- **CLI.** In `lab/src/cli.rs`, add a global `--listing-style text|blocks` flag that overrides the config for that run (snapshots, timelines, selftests). Document it next to the other flags.

### 3. Draw blocks (`lab/src/ui.rs`, `draw_code`)
- **Branching.** Keep the header (heading, title, engine source, separator). After it, branch on `pseudocode::style()`.
  - Text keeps today's loop unchanged.
  - Blocks goes to a new `draw_blocks(ink, painter, rect, listing, highlight, report)`.
- **Measure, then draw.** A recursive pass computes each block's height at the available width. A second pass draws.
  - Wrapping reuses `wrap_code`.
  - Token colouring reuses `pseudocode::tokens` and the `palette::code_*` mapping. Factor the token-drawing loop into a helper that both styles call.
- **Shapes.** Use egui `Shape::convex_polygon` / `PathShape` for the notches and the hat curve. Everything else is `rect_filled` / `rect_stroke` with `CornerRadius`, as elsewhere in `ui.rs`.

  | Block | Shape |
  |---|---|
  | Stmt | rounded rect, kind colour fill at about 35% alpha, 4 px solid left edge in the full colour, small notch tab on top |
  | C | head bar, an 18 px arm down the left, a 10 px foot. Each `elif`/`else` arm adds a middle bar |
  | Hat | curved top. Shows the dispatch text if present, otherwise the `def` line in tokens |
  | Cap | flat bottom, no notch below |
  | Note | faint italic comment text, no block |

  - The 18 px arm is narrower than the text style's 4-column indent (about 58 px). Deep listings therefore get wider lines than in the text style, which offsets the vertical padding blocks add.
- **Spacing.** Rows are 28 px, plus 4 px padding per block and 2 px between blocks.
- **Overflow.** If the measured height exceeds the panel, redo the layout with compact padding (0/1 px). Only then count overflow in `report.code_overflow`, exactly as the text style does.
- **Highlight.**
  - A lit block is drawn in full colour with a 2 px `palette::accent()` outline.
  - Unlit blocks are dimmed with `gamma_multiply(0.55)`, as today.
  - A lit C-block head lights only its bar, not its body.
- **Colours.** Add theme colours `block_control`, `block_read`, `block_write`, `block_math`, `block_statement` and `block_hat` (light and dark).
  - Add the fields to `palette::Theme` and accessors in `palette.rs`.
  - Add the keys in `lab/src/theme.rs`, next to the `code_*` keys.
  - Add the values in every theme TOML (find them through `theme.rs`'s loader).

### 4. Keep the rest of the system consistent
- **Highlight tags and kinds.**
  - `lab/src/controls.rs`: the Code tab builds one step button per tag. Check that `tags` never contains kinds.
  - `lab/src/shot.rs`: highlight validation stays tag-only.
- **Python tooling** (`tools/video/outputs.py`)
  - `read_listing` must ignore `# @@kind` marks and the `# dispatch:` header.
  - `listing_markdown` should strip marks from the guide's code, as it does tags.
  - `build.py`'s header validation must accept `dispatch`.
- **Video builds.** `tools/video/build.py` gets `--listing-style` (default `text`) and passes it to the renderer command it runs. A video's look then doesn't depend on the builder's personal config.
- **Selftests.** The three investigations' selftests check that listings fit (`code_overflow == 0`). Run those checks once per style, setting the global around each check:
  - `packages/o3de_lighting/src/o3de/lighting/selftest.rs`
  - `packages/godot_sdfgi/src/godot/sdfgi/selftest.rs`
  - `packages/godot_voxelgi/src/godot/voxelgi/selftest.rs`
- **Snapshot page.** Add `home --snapshot <png> --page viewer-code:<pkg.listing> [--listing-style blocks]`.
  - It opens the viewer on the listing's demo with `PanelMode::Code` and the listing, using the selftest recipe (`state.panel = PanelMode::Code; state.code = Some(listing)`).
  - It also accepts a `highlight=<tag>` suffix, or the first tag by default.
  - Wire it in `cli.rs` and `home.rs`, modelled on `snapshot_performance`.
- **Docs.**
  - The `pseudocode.rs` module comment (marks and dispatch header).
  - The listing section of `.agents/skills/new-investigation/SKILL.md`.
  - The new page and flag in AGENTS.md's Validation list.

### 5. Content (first pass only)
- Add `# dispatch:` and `@@read`/`@@write`/`@@math` marks to the 12 `godot_sdfgi` listings, starting with `cascade_update.py`, so the comparison shows the full design.
- Other packages get marks gradually. Without marks they still render, with control and statement colours only.

## Order
1. Parse and block tree, with unit tests.
2. Config, global, CLI flag and text keys.
3. Theme colours, then `draw_blocks`.
4. Snapshot page, selftests, and the Python tooling.
5. sdfgi marks and headers.
6. Docs.

Build after each step, one build at a time, with no GPU job running alongside.

## Verification
Set `CARGO_HOME=$PWD/.cache/cargo` and run from the repo root.
1. `cargo build --release --locked --offline`, with no warnings.
2. Rebuild each local package's own workspace, if any (`--manifest-path packages/<name>/Cargo.toml`), with no warnings.
3. `cargo test --release --locked --offline --workspace` passes, including the block-tree and config round-trip tests.
4. Render `cargo run --release -- home --snapshot <scratch>/text.png --page viewer-code:godot_sdfgi.listing/cascade_update`, then the same with `--listing-style blocks`.
   - Look at both PNGs side by side.
   - Repeat for the deepest listing (`jump_flood.py`) and a 23-line O3DE listing (`diffuse_response.py`).
5. Run each investigation's selftest (`--investigation <id> selftest --report …`) and check that both styles report no code overflow.
6. `python3 -m unittest tools/video/test_markup.py tools/video/test_story.py`, then `python3 tools/video/build.py --investigation godot/sdfgi --snapshots-only --listing-style blocks`. Look at a beat that shows code.
7. Check the public build: copy the published files into an empty directory, link `vendor/`, and build without warnings.
8. `cargo run --release -- home --autotest <scratch>/home.json` still passes with the new Interface control.
