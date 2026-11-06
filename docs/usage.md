# Using Render Lab

[Render Lab](../README.md) · [Build](../BUILD.md) · [简体中文](usage.zh-CN.md)

## Homepage and settings

Started without a command, `render-lab` shows a loading page while the scene behind its title screen loads, then the
**title screen**, like a game's, behind frosted glass, with **Play**, **Config**, and **Quit**, a globe with the language
in the top-right corner (it opens Config at the Language option), and the copyright, license, and version in the
bottom-left corner. A photosensitivity warning covers it first, in the configured language, until **Continue**, and
again, in the new language, whenever a saved setting changes the language.

- **Play** opens the selection page: the engines and features of the installed investigations. Each feature can be
  watched as its narrated video, explored in its demonstrations, experimented with, and, when its package has one,
  taken as a quiz (**Take the quiz**): multiple-choice questions on its story, each at the right of a live view of the
  demonstration it asks about, which can be turned, clicked, and changed to find the answer; the score at the end.
- **Config** edits the settings in `~/.config/render-lab/config.toml` (or `$XDG_CONFIG_HOME/render-lab/config.toml`),
  a tab per category:
  - **Interface**: the **Language** (English or 中文: the homepage's text, and the narrated edition the selection page
    offers first, English when a feature has none in it; the demonstrations follow it too) and the **Theme**, the colours
    of the homepage and the viewer (below).
  - **Render**: **Full Screen**.
  - **Audio**: the volumes of the **Music** and the **Narration**, each a slider with a mute button that keeps its
    level. The viewer's dock, and the timeline bar while a video plays, also set the narration's volume; the viewer
    keeps it in the file when it closes.

  **Save and Apply** applies the Interface and Audio settings and writes the file at once. A change to the Render
  settings is tried behind a ten-second countdown instead: **Apply** keeps it and writes the file, while **Restore**,
  Esc, or letting the time run out puts the previous one back. Nothing is written before Save and Apply. With nothing to
  save, the first button is **Return**; a tab with edits has a dot, and an option that differs from its default is
  marked, with a **Restore** for that option alone; **Discard** drops the edits, and **Restore Defaults** shows the
  defaults.
- **Themes** are TOML files, one per theme, named by their file's stem (`theme = "tokyo-night"` in the settings).
  The lab reads them at run time from `themes/` beside the binary, where the build puts the ones it offers (from
  [`themes/`](../themes): **One Dark Pro**, **Tokyo Night**, and **VS Code**), and from
  `~/.config/render-lab/themes/`, whose files take the place of the binary's of the same name. A theme names itself and
  sets every colour of the interface; [`themes/one-dark-pro.toml`](../themes/one-dark-pro.toml) describes each key. The
  lab's own colours are the built-in theme **Render Lab**, the default, which recordings always use, so a recorded frame never depends
  on the settings.
- **Every slider, checkbox, and choice** has a **?** after its name: hovering it says what the value does, in the
  interface's language.
- **The viewer's layout** can be resized by dragging the edge between the viewport and the panel (the struct panel and
  the Code tab's listing each keep their own width) and the edge above the dock. The sizes are written to the settings
  file as each drag ends, so the viewer opens with them next time. Double-clicking an edge puts that size back, the
  dock's **Reset layout** puts them all back, and Config's **Restore Defaults** does too. Recordings always use the
  default layout.
- **F11** saves a screenshot of the window (the homepage or the viewer) to `build/screenshots/`.

## Commands

Run these commands from the repository root:

```bash
cargo run --release                                   # the homepage
cargo run --release -- --list                         # the installed investigations (the values --investigation takes)
cargo run --release -- home --snapshot home.png       # the title screen rendered offscreen (--page select, config, starting, viewer, viewer-timeline, viewer-experiments, viewer-compare, or viewer-quiz for the others)
cargo run --release -- home --autotest home.json      # click through the homepage offscreen (it uses its own settings file)
```

The lab also has `render` (record a timeline into FFmpeg), `selftest`, `layout`, and `viewer-autotest`, and every
command takes `--investigation ENGINE/FEATURE` (default: the first one listed):

```bash
cargo run --release -- --investigation o3de/lighting selftest --report o3de.json   # the port against its CPU mirror, frames, sources
```

## Narrated videos

An investigation's narrated video is built by [`tools/video`](../tools/video/README.md) from its storyboard:

```bash
bash tools/video/setup.sh --gpu                                     # once: Piper and FFmpeg under .cache/
python3 tools/video/build.py --investigation o3de/lighting          # packages/o3de_lighting/build/video/en/o3de-lighting.mp4
python3 tools/video/build.py --investigation o3de/lighting --lang zh-CN
```
