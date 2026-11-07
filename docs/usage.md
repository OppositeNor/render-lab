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
    level. The viewer's timeline bar also sets the narration's volume; the viewer keeps it in the file when it
    closes.

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
- **The viewer's windows**: the header holds what acts on the viewer (the demonstrations, Play, Step, Reset, the
  timeline, the **Windows** menu, Home), and the footer the timeline bar; everything else is in a window, each with
  the same title bar. **Controls** has the tabs (Sliders, Experiments, Code, Sources, …) with **Reset parameters** and
  **Free camera** at their end, and the shown tab's values; **Data** (the panel) shows the frame's record, readout,
  listing, or graph; **Scene** lists what the scene holds (below). A window is docked to the left, the right, or the
  bottom, the viewport taking the rest, or floats over the viewport: drag its title bar to move it, and drop it near
  an edge to dock it there (the edge lights up first); drag a docked window's inner edge, or a floating one's corner,
  to resize it; the chevron collapses it to its title bar, and double-clicking the title bar puts it back in its
  place. The **Windows** menu shows or hides each one, and its **Reset layout** puts them all back, as Config's
  **Restore Defaults** does. Where each window is gets written to the settings file as each change ends, so the
  viewer opens with it next time. Recordings always use the fixed layout.
- **The camera** in the viewport: drag to orbit, **Shift**-drag (or the middle button) to pan, the wheel to zoom.
  Holding the **right button** flies: the pointer turns the view about the eye, **W A S D** move along it, **Q** and
  **E** down and up the world's vertical, **Shift** faster, and the wheel sets the speed while flying.
- **The Scene window** lists the scene's objects and the lights and points the demonstration places. Selecting one
  puts a gizmo on it in the viewport and its values below the list: **Move** drags it along an arrow or in a square's
  plane, **Rotate** turns it about the vertical with the ring, **Scale** sizes it with the handle, and the sliders
  set the same values exactly (as do timelines and stories: `object.teapot.x`, `object.teapot.yaw`, …). **Focus**
  (**F**) turns the camera to it; **Reset** puts it back. An investigation that draws the scene from data it baked keeps its
  objects fixed, and the walls and the floor always stay.
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
