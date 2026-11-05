//! The command line: without a command it opens the homepage, where an
//! engine and a feature are chosen and the interactive viewer is opened.
//! `render` records a timeline deterministically, `selftest` verifies the
//! renderer and the investigation's content, and `viewer-autotest` tests the
//! viewer's controls. The binary (src/main.rs in the repository) calls
//! `main`; the investigations come from the packages' libraries
//! (src/investigations.rs).

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use crate::home;
use crate::investigations;
use crate::text::DEFAULT_UI_SCALE;
use crate::{investigation, layout, record, shaders, viewer};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "render-lab", version, about = "Illustrated investigations of game-engine features: homepage, viewer, recorder, and verification")]
struct Cli {
    /// The investigation, as engine/feature (default: the first one; `--list` lists them).
    #[arg(long, global = true)]
    investigation: Option<String>,
    /// List the installed investigations (the values `--investigation` takes),
    /// with their titles and packages, and exit.
    #[arg(long)]
    list: bool,
    /// The lab's root, where its packages are (default:
    /// RENDER_LAB_ROOT, or the repository's directory).
    #[arg(long, global = true)]
    root: Option<PathBuf>,
    /// How the code panel draws pseudo-code listings this run, whatever the
    /// settings say: blocks or text (default: the settings' in the homepage
    /// and its viewer; blocks in recordings and self-tests).
    #[arg(long, global = true, value_parser = parse_listing_style)]
    listing_style: Option<crate::pseudocode::Style>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Open the homepage (the default): choose an engine and a feature.
    Home {
        #[arg(long, default_value_t = 1600)]
        width: u32,
        #[arg(long, default_value_t = 900)]
        height: u32,
        /// Size of the text and panels relative to the video's.
        #[arg(long, default_value_t = DEFAULT_UI_SCALE)]
        ui_scale: f32,
        /// Render the homepage offscreen to this PNG instead of opening a window.
        #[arg(long)]
        snapshot: Option<PathBuf>,
        /// The page the snapshot shows: title, select, config, trial (the Config page's countdown),
        /// starting (the homepage's loading page, while the title's scene loads), loading (the
        /// viewer's loading page, while the current investigation's first scene loads), viewer (the
        /// viewer on its first demonstration, with the dock), viewer:<demo> (the viewer on that demonstration), or viewer-timeline (the viewer playing
        /// the investigation's first timeline, with its bar), viewer-diagram (that timeline paused on its first 2D
        /// scene with an image, the image's panel zoomed in), viewer-sources (paused on its first sources card),
        /// viewer-experiments[:<demo>] (the Experiment button's viewer: the Experiments tab), or
        /// viewer-compare[:<demo>] (that viewer with a reference set and compared, a wipe), or
        /// viewer-layout (the viewer's panel and dock edges dragged; the layout written beside the PNG as .toml), or
        /// viewer-performance[:<demo>] (the performance panel, its most expensive pass called out), or
        /// viewer-graph[:<demo>] (the frame graph in the panel, the inspector on what its most expensive pass writes), or
        /// viewer-inspect:<node>.<resource>[/<show>][?<param>=<value>&…] (the frame graph, the inspector on that resource after that node), or
        /// viewer-dag[:<demo>][@<node>,…] (the frame graph as a DAG in the panel, each pass's result on its card, those nodes called out), or
        /// viewer-dag-full[:<demo>][@<node>,…] (that DAG full screen), or
        /// viewer-resources[-full][:<demo>][@<node>,…] (every resource as a card, its state at the end of the frame), or
        /// viewer-source:<node> (the full-screen DAG, that node selected, the WGSL it is compiled from beside it), or
        /// viewer-free[:<demo>] (the free camera on: the main camera and its view cone seen from beside it), or
        /// viewer-code:<listing>[@<tag>] (the Code tab on that listing, the tag's lines lit; see --listing-style), or
        /// viewer-prerequisites[:<demo>] (the Prerequisites tab, the knowledge graph in the panel), or
        /// viewer-knowledge[:<demo>] (that panel zoomed onto a topic of the audience's level, selected), or
        /// viewer-knowledge-graph[:<demo>] (the knowledge graph full screen over the viewer; viewer-knowledge-compact: only
        /// the topics its context needs), or select-knowledge
        /// (the selection page with the chosen feature's knowledge graph full screen).
        #[arg(long, default_value = "title")]
        page: String,
        /// Check the homepage's controls offscreen and write a JSON report.
        #[arg(long)]
        autotest: Option<PathBuf>,
    },
    /// The viewer's scripted control test (opens a window; for the video build's checks).
    ViewerAutotest {
        /// The JSON report to write.
        #[arg(long)]
        report: PathBuf,
        /// Also test timeline playback on this timeline.
        #[arg(long)]
        timeline: Option<PathBuf>,
        #[arg(long, default_value_t = DEFAULT_UI_SCALE)]
        ui_scale: f32,
        #[arg(long, default_value_t = 1600)]
        width: u32,
        #[arg(long, default_value_t = 900)]
        height: u32,
    },
    /// Render a timeline frame by frame and stream it to FFmpeg.
    Render {
        #[arg(long)]
        timeline: PathBuf,
        /// Video file to write (omit to only write snapshots).
        #[arg(long)]
        output: Option<PathBuf>,
        /// Frame range START:END (END exclusive).
        #[arg(long)]
        frames: Option<String>,
        /// Render this many frames before START without writing them, so a range rendered on its own starts with the
        /// same history (last frame's image, landed read-backs) as a whole render (a video split into parallel chunks).
        #[arg(long, default_value_t = 0)]
        warmup: u64,
        /// Frame size: WIDTHxHEIGHT, or 720p, 1080p, 1440p, 2160p (4k)
        /// (default: the timeline's, 1080p for the storyboards' videos).
        #[arg(long, value_parser = parse_resolution)]
        resolution: Option<[u32; 2]>,
        /// Comma-separated frame indices to save as PNG.
        #[arg(long, value_delimiter = ',')]
        snapshot: Vec<u64>,
        #[arg(long)]
        snapshot_dir: Option<PathBuf>,
        /// Write a JSON summary (text sizes, label checks, timing).
        #[arg(long)]
        report: Option<PathBuf>,
        #[arg(long, default_value = "h264")]
        codec: String,
        #[arg(long, default_value_t = 18)]
        crf: u32,
        #[arg(long, default_value = "medium")]
        preset: String,
        #[arg(long)]
        ffmpeg: Option<PathBuf>,
        /// Draw each frame's index as a binary stamp (for ordering tests).
        #[arg(long)]
        frame_stamp: bool,
    },
    /// Run the GPU and layout verification suite.
    Selftest {
        #[arg(long)]
        report: PathBuf,
        /// Also check labels and text for every scene beat of this timeline.
        #[arg(long)]
        timeline: Option<PathBuf>,
        #[arg(long)]
        work_dir: Option<PathBuf>,
        #[arg(long)]
        ffmpeg: Option<PathBuf>,
    },
    /// List the series the packages provide (src/packages.rs, `Series`): each
    /// one's parts in order, the investigation and narrated timelines of each,
    /// and check that a part's audience knows what the parts before it teach
    /// (and that no part teaches again what an earlier one taught).
    Series,
    /// Check the current investigation's knowledge graph (lab/src/knowledge.rs:
    /// render_lab's topics and its package's, its chapters, its audience) and
    /// print its audience split: the level, the known topics used directly
    /// above it, and what the story teaches.
    Knowledge {
        /// The audience's known topics instead of the stored ones (keys or
        /// full names, comma-separated): try an audience before writing it.
        #[arg(long, value_delimiter = ',')]
        known: Vec<String>,
        /// Also list the nearby audiences (one level topic added, or one
        /// known topic removed), fewest carries first.
        #[arg(long)]
        levels: bool,
    },
    /// Print the current investigation's frame as a render graph
    /// (lab/src/graph.rs): its nodes in execution order with what each reads
    /// and writes, its resources with their lifetimes, and the culled nodes;
    /// checks the declarations against the WGSL (fails on a problem).
    Graph {
        /// Write the graph in Graphviz's language to this file.
        #[arg(long)]
        dot: Option<PathBuf>,
        /// Print it as JSON.
        #[arg(long)]
        json: bool,
    },
    /// List the current investigation's values (lab/src/values.rs): the
    /// keys its dock, timelines, and stories set, with each one's type,
    /// default, range, unit, labels, and what it is.
    Values {
        #[arg(long)]
        json: bool,
        /// Only the values with this label.
        #[arg(long)]
        label: Option<String>,
        /// Only the values whose keys start with this scope (`camera`).
        #[arg(long)]
        scope: Option<String>,
    },
    /// Run an experiment headless (src/research.rs): its reference, then each
    /// run (the base, its sweeps, its variants), timed per pass and measured
    /// against the reference; writes the results, images, and a manifest of
    /// what ran into `--out`. The file format is in src/experiment.rs.
    Experiment {
        /// An experiment file, or a package's resource by full key (`godot_voxelgi.experiment/bias`).
        file: String,
        /// The directory to write the results into.
        #[arg(long)]
        out: Option<PathBuf>,
        /// List the runs and stop.
        #[arg(long)]
        dry_run: bool,
    },
    /// The current investigation's story: `keys`, what its cues may set.
    #[command(subcommand)]
    Story(StoryCommand),
    /// List the current investigation's choices (`#[derive(Choice)]` enums),
    /// their options, and the WGSL constants the lab writes for its shaders
    /// (its package's `build/generated/<name>.wgsl`) whenever it loads it.
    Choices,
    /// List the current investigation's actions (lab/src/actions.rs): what a
    /// story starts with `run=<key>` and stops with `stop=<key>`, and the
    /// arguments it gives them (`run.<argument>=…`); and its click handlers,
    /// which a story gives an image (`imageClick=<key>`).
    Actions,
    /// List the images the current investigation publishes for a story to
    /// show (`{@ image=<key>}`), and the image kinds its package's shaders
    /// define (`{@ image=<name> imageFrom=<kind> image.<param>=…}`, GPU
    /// images), and the points and arrow sets its story's arrows and labels
    /// can name; a story can also show a PNG resource by its key.
    Images,
    /// List the packages in packages/ (their `manifest.toml`): name, author,
    /// license, imports, exports, and demos.
    Packages {
        #[arg(long)]
        json: bool,
    },
    /// List the resources the built packages declare: full key, kind, the
    /// size of its file (or that it is not built yet), license, and source.
    Resources {
        #[arg(long)]
        json: bool,
    },
    /// Build the source checkout's packages into `packages/` beside the
    /// binary (what every launch from a checkout does first), and report it.
    Cook,
    /// Print the Rust and WGSL layouts of the displayed records.
    Layout {
        #[arg(long)]
        json: bool,
    },
    /// Typeset every formula tag in these JSON files (text tables, storyboards,
    /// timelines) and check the lab's limits: glyphs of at least 24 px, display
    /// formulas at most 546 px wide. Prints a JSON report; fails on a problem.
    Formulas {
        #[arg(long, required = true, num_args = 1..)]
        check: Vec<PathBuf>,
    },
    /// Lamlet and lamlet-present, from the versions the lab pins (src/script/): `ir`, `deck`, `vocab`. Every
    /// document may import `std`, `content`, `present`, and render_lab's package `render_lab`. Diagnostics go to
    /// stderr with the document's line, and the exit status is 1.
    #[command(subcommand)]
    Lamlet(LamletCommand),
    /// The investigation's scripts (src/script/runtime.rs): its package's `script/…` resources, which the lab runs
    /// while it draws and records.
    #[command(subcommand)]
    Script(ScriptCommand),
}

#[derive(Subcommand)]
enum ScriptCommand {
    /// Check the scripts and list their hooks (overlays, click handlers, actions, animations); fails with the
    /// first diagnostic.
    Check,
    /// The Lamlet packages a script may import besides `std` (the runtime library, the generated values package,
    /// the natives' stubs), for reading and for editors; with NAME, that one package's source.
    Stubs { name: Option<String> },
}

#[derive(Subcommand)]
enum StoryCommand {
    /// The keys a storyboard's cues may set, as JSON (src/script/keys.rs): the vocabulary key list
    /// `render-lab lamlet vocab` turns into the investigation's cue package.
    Keys,
}

#[derive(Subcommand)]
enum LamletCommand {
    /// The document's IR as JSON, its prose read as CommonMark (`lamlet build --to ir --prose commonmark`).
    Ir {
        file: PathBuf,
        #[arg(long = "package", value_name = "NAME=PATH", value_parser = parse_package)]
        packages: Vec<(String, PathBuf)>,
        /// Write it to this file instead of stdout.
        #[arg(short, long)]
        out: Option<PathBuf>,
    },
    /// A presentation's deck as JSON, with no timing (`lamlet present deck`).
    Deck {
        file: PathBuf,
        #[arg(long = "package", value_name = "NAME=PATH", value_parser = parse_package)]
        packages: Vec<(String, PathBuf)>,
        /// Write it to this file instead of stdout.
        #[arg(short, long)]
        out: Option<PathBuf>,
    },
    /// The cue vocabulary package NAME from a host's keys (`lamlet present vocab`), checked with the packages.
    Vocab {
        name: String,
        keys: PathBuf,
        #[arg(long = "package", value_name = "NAME=PATH", value_parser = parse_package)]
        packages: Vec<(String, PathBuf)>,
        #[arg(short, long)]
        out: Option<PathBuf>,
    },
    /// A document's static top-level binding NAME as the TOML document the lab's readers take (src/script/data.rs):
    /// what a `.lam` content file (a scene) is read as.
    Toml {
        file: PathBuf,
        name: String,
        #[arg(short, long)]
        out: Option<PathBuf>,
    },
}

fn parse_package(value: &str) -> Result<(String, PathBuf), String> {
    let (name, path) = value.split_once('=').ok_or("expected NAME=PATH")?;
    Ok((name.to_string(), PathBuf::from(path)))
}

/// `render-lab [--investigation <id>] lamlet …`; an error is a diagnostic (or a file's error) for stderr. The lab's
/// natives (src/script/natives.rs) check names against `investigation`, the one given.
fn lamlet(command: LamletCommand, investigation: Option<&'static dyn crate::investigation::Investigation>) -> Result<(), String> {
    use crate::script;
    let read = |given: Vec<(String, PathBuf)>| -> Result<Vec<script::Package>, String> {
        script::packages(given.iter().map(|(name, path)| script::Package::read(name, path)).collect::<Result<_, _>>()?)
    };
    let write = |out: Option<PathBuf>, text: String| -> Result<(), String> {
        match out {
            Some(path) => std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display())),
            None => {
                print!("{text}");
                Ok(())
            }
        }
    };
    match command {
        LamletCommand::Ir { file, packages, out } => write(out, script::ir(&file, &read(packages)?, investigation)?),
        LamletCommand::Deck { file, packages, out } => {
            let deck = script::deck(&file, &read(packages)?, investigation)?;
            write(out, serde_json::to_string_pretty(&deck).map_err(|e| e.to_string())? + "\n")
        }
        LamletCommand::Toml { file, name, out } => {
            let text = std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
            write(out, script::data::toml(&file.display().to_string(), &text, &name)?)
        }
        LamletCommand::Vocab { name, keys, packages, out } => {
            let json = std::fs::read_to_string(&keys).map_err(|e| format!("{}: {e}", keys.display()))?;
            write(out, script::vocab(&name, &json, &read(packages)?).map_err(|e| format!("{}: {e}", keys.display()))?)
        }
    }
}

/// `render-lab --investigation <id> script …`.
fn script(command: ScriptCommand, investigation: &'static dyn crate::investigation::Investigation) -> Result<(), String> {
    use crate::script::runtime;
    match command {
        ScriptCommand::Check => {
            let hooks = runtime::check()?;
            if hooks.is_empty() {
                println!("{}: no scripts with hooks", investigation.path());
            }
            for h in hooks {
                println!("{h}");
            }
            Ok(())
        }
        ScriptCommand::Stubs { name } => {
            let (_, sources) = runtime::engine(investigation)?;
            match name {
                Some(name) => {
                    let p = sources.packages.iter().find(|p| p.name == name).ok_or_else(|| format!("no package `{name}`"))?;
                    print!("{}", p.source);
                }
                None => {
                    for p in &sources.packages {
                        println!("{}\t{}", p.name, p.file);
                    }
                    if !sources.skipped.is_empty() {
                        println!("values left out of the values package (their names are taken): {}", sources.skipped.join(", "));
                    }
                }
            }
            Ok(())
        }
    }
}

/// `--resolution`: a 16:9 preset by its height, or WIDTHxHEIGHT (even, for
/// FFmpeg's yuv420p).
fn parse_listing_style(value: &str) -> Result<crate::pseudocode::Style, String> {
    crate::pseudocode::Style::from_key(value).ok_or_else(|| format!("`{value}`: text or blocks"))
}

fn parse_resolution(value: &str) -> Result<[u32; 2], String> {
    let size = match value.to_ascii_lowercase().as_str() {
        "720p" => [1280, 720],
        "1080p" => [1920, 1080],
        "1440p" => [2560, 1440],
        "2160p" | "4k" => [3840, 2160],
        other => {
            let (w, h) = other.split_once('x').ok_or("expected WIDTHxHEIGHT or 720p, 1080p, 1440p, 2160p")?;
            [w.parse().map_err(|_| format!("bad width `{w}`"))?, h.parse().map_err(|_| format!("bad height `{h}`"))?]
        }
    };
    if size.iter().any(|&n| n < 2 || n > 8192 || n % 2 != 0) {
        return Err(format!("{}x{}: width and height are even, from 2 to 8192", size[0], size[1]));
    }
    Ok(size)
}

/// What the binary tells the lab.
pub struct App {
    /// The investigation whose scene the homepage shows behind every page
    /// (`engine/feature`); the first one if no package provides it.
    pub title: &'static str,
}

/// Runs the command line; the binary's `main`.
pub fn main(app: App) -> Result<()> {
    let cli = Cli::parse();
    if let Some(root) = &cli.root {
        crate::resources::set_root(root.clone());
    }
    if let Some(style) = cli.listing_style {
        crate::pseudocode::force_style(style);
    }
    investigations::set_title(app.title);
    let manifest_dir = crate::resources::root().to_path_buf();
    if cli.list {
        let all = investigations::all();
        if all.is_empty() {
            println!("no investigation is installed");
        }
        let width = all.iter().map(|i| i.path().len()).max().unwrap_or(0);
        for i in all {
            println!("{:width$}  {} ({}, package {})", i.path(), i.title(), i.engine().name, i.package().unwrap_or("render_lab"));
        }
        return Ok(());
    }
    let command = cli.command.unwrap_or(Command::Home { width: 1600, height: 900, ui_scale: DEFAULT_UI_SCALE, snapshot: None, page: "title".into(), autotest: None });
    // Lamlet runs before an investigation is chosen, unless one is given for its natives to check against.
    let command = match command {
        Command::Lamlet(command) if cli.investigation.is_none() => {
            if let Err(e) = lamlet(command, None) {
                eprintln!("{}", e.trim_end());
                std::process::exit(1);
            }
            return Ok(());
        }
        command => command,
    };
    let chosen = {
        let known: Vec<String> = investigations::all().iter().map(|i| i.path()).collect();
        let path = cli.investigation.clone().or_else(|| known.first().cloned()).context("no investigation is installed")?;
        let chosen = investigations::find(&path).with_context(|| format!("unknown investigation `{path}`; known: {} (render-lab --list)", known.join(", ")))?;
        investigation::set(chosen);
        chosen
    };
    match command {
        Command::Home { width, height, ui_scale, snapshot, page, autotest } => {
            let options = home::HomeOptions { manifest_dir: manifest_dir.clone(), width, height, ui_scale: ui_scale.clamp(0.4, 1.5), config_path: crate::config::Config::default_path() };
            match (snapshot, autotest) {
                (Some(png), _) if page == "loading" => crate::loading::snapshot(&png, [width.max(64), height.max(64)], options.ui_scale),
                (Some(png), _) if page == "starting" => home::snapshot_starting(&options, &png),
                (Some(png), _) if page == "viewer" || page == "viewer-timeline" => home::snapshot_viewer(&options, page == "viewer-timeline", None, &png),
                (Some(png), _) if page == "viewer-diagram" => home::snapshot_viewer_page(&options, true, None, Some(crate::viewer::PausedPage::Diagram), &png),
                (Some(png), _) if page == "viewer-sources" => home::snapshot_viewer_page(&options, true, None, Some(crate::viewer::PausedPage::Sources), &png),
                (Some(png), _) if page == "viewer-layout" => home::snapshot_layout(&options, &png),
                (Some(png), _) if page.starts_with("viewer-code:") => {
                    let (listing, tag) = page["viewer-code:".len()..].split_once('@').map_or((&page["viewer-code:".len()..], None), |(l, t)| (l, Some(t)));
                    home::snapshot_code(&options, listing, tag, &png)
                }
                (Some(png), _) if page == "viewer-free" || page.starts_with("viewer-free:") => home::snapshot_free(&options, page.strip_prefix("viewer-free:"), &png),
                (Some(png), _) if page == "viewer-performance" || page.starts_with("viewer-performance:") => home::snapshot_performance(&options, page.strip_prefix("viewer-performance:"), &png),
                (Some(png), _) if page == "viewer-graph" || page.starts_with("viewer-graph:") => home::snapshot_graph(&options, page.strip_prefix("viewer-graph:"), None, Default::default(), &png),
                (Some(png), _) if page.starts_with("viewer-inspect:") => home::snapshot_graph(&options, None, page.strip_prefix("viewer-inspect:"), Default::default(), &png),
                // viewer-source:<node>: the full-screen DAG, that node selected, its WGSL beside it.
                (Some(png), _) if page.starts_with("viewer-source:") => {
                    let node = page.strip_prefix("viewer-source:").unwrap_or_default();
                    let graph = crate::graph::FrameGraph::current(&[]);
                    let i = graph.node(node).ok_or_else(|| anyhow::anyhow!("no node `{node}`"))?;
                    let key = format!("{node}.{}", graph.result_of(i).ok_or_else(|| anyhow::anyhow!("`{node}` has no result to select"))?);
                    home::snapshot_graph(&options, None, Some(&key), crate::viewer::GraphSnapshot { dag: true, full_screen: true, source: true, ..Default::default() }, &png)
                }
                (Some(png), _) if page.starts_with("viewer-dag") || page.starts_with("viewer-resources") => {
                    // viewer-dag[-full][:<demo>][@<node>,…], viewer-resources[-full]…
                    let resources = page.starts_with("viewer-resources");
                    let page = page.strip_prefix(if resources { "viewer-resources" } else { "viewer-dag" }).unwrap_or_default();
                    let (rest, full_screen) = match page.strip_prefix("-full") {
                        Some(rest) => (rest, true),
                        None => (page, false),
                    };
                    let (rest, highlight) = match rest.split_once('@') {
                        Some((rest, nodes)) => (rest, nodes.split(',').map(str::to_string).collect()),
                        None => (rest, Vec::new()),
                    };
                    let demo = rest.strip_prefix(':').filter(|d| !d.is_empty());
                    home::snapshot_graph(&options, demo, None, crate::viewer::GraphSnapshot { dag: true, resources, full_screen, highlight, source: false }, &png)
                }
                (Some(png), _) if page == "viewer-experiments" || page.starts_with("viewer-experiments:") => home::snapshot_experiments(&options, page.strip_prefix("viewer-experiments:"), false, &png),
                (Some(png), _) if page == "viewer-compare" || page.starts_with("viewer-compare:") => home::snapshot_experiments(&options, page.strip_prefix("viewer-compare:"), true, &png),
                (Some(png), _) if page == "viewer-prerequisites" || page.starts_with("viewer-prerequisites:") => home::snapshot_prerequisites(&options, page.strip_prefix("viewer-prerequisites:"), false, false, &png),
                (Some(png), _) if page == "viewer-knowledge" || page.starts_with("viewer-knowledge:") => home::snapshot_prerequisites(&options, page.strip_prefix("viewer-knowledge:"), true, false, &png),
                (Some(png), _) if page == "viewer-knowledge-graph" || page.starts_with("viewer-knowledge-graph:") => home::snapshot_prerequisites(&options, page.strip_prefix("viewer-knowledge-graph:"), false, true, &png),
                (Some(png), _) if page == "viewer-knowledge-compact" || page.starts_with("viewer-knowledge-compact:") => {
                    crate::ui::set_knowledge_compact(true);
                    home::snapshot_prerequisites(&options, page.strip_prefix("viewer-knowledge-compact:"), false, true, &png)
                }
                (Some(png), _) if page == "select-knowledge" => home::snapshot_knowledge(&options, &png),
                (Some(png), _) if page.starts_with("viewer:") => home::snapshot_viewer(&options, false, page.strip_prefix("viewer:"), &png),
                (Some(png), _) => {
                    let trial = page == "trial";
                    home::snapshot(&options, if trial { home::Page::Config } else { home::Page::parse(&page)? }, trial, &png)
                }
                (None, Some(report)) => home::autotest(&options, &report),
                (None, None) => home::run(options),
            }
        }
        Command::ViewerAutotest { report, timeline, ui_scale, width, height } => {
            // Silent: the test plays the timeline without its narration.
            viewer::run(viewer::ViewerOptions { timeline, audio: None, manifest_dir: manifest_dir.clone(), ui_scale: ui_scale.clamp(0.4, 1.5), autotest: Some(report), width, height, locale: None, start_frame: 0, fullscreen: false, narration: crate::config::Config::default().narration, experiments: false, layout: Default::default(), settings: None, next: false, autoplay: false })
        }
        Command::Render { timeline, output, frames, warmup, resolution, snapshot, snapshot_dir, report, codec, crf, preset, ffmpeg, frame_stamp } => {
            // A frame a script did not draw must not reach a video.
            crate::script::runtime::set_strict(true);
            let frames = frames
                .map(|r| -> Result<(u64, u64)> {
                    let (a, b) = r.split_once(':').context("--frames expects START:END")?;
                    Ok((a.parse()?, b.parse()?))
                })
                .transpose()?;
            if output.is_none() && snapshot.is_empty() {
                bail!("nothing to do: give --output and/or --snapshot");
            }
            let options = record::RecordOptions {
                timeline,
                output,
                ffmpeg: ffmpeg.unwrap_or_else(|| record::default_ffmpeg(&manifest_dir)),
                codec,
                crf,
                preset,
                frames,
                warmup,
                resolution,
                snapshots: snapshot,
                snapshot_dir,
                report,
                frame_stamp,
            };
            let summary = record::record(&options)?;
            eprintln!("rendered {} frames in {:.1} s ({:.1} fps); smallest text {:.1} px", summary.frames, summary.seconds, summary.frames as f64 / summary.seconds, summary.min_text_pt);
            Ok(())
        }
        Command::Selftest { report, timeline, work_dir, ffmpeg } => {
            crate::script::runtime::set_strict(true);
            let work_dir = work_dir.unwrap_or_else(|| manifest_dir.join("target/selftest"));
            let passed = chosen.selftest(&investigation::SelftestOptions { report: report.clone(), timeline, ffmpeg: ffmpeg.unwrap_or_else(|| record::default_ffmpeg(&manifest_dir)), work_dir })?;
            if !passed {
                bail!("selftest failed; see {}", report.display());
            }
            eprintln!("selftest passed; report: {}", report.display());
            Ok(())
        }
        Command::Formulas { check } => {
            let report = crate::formula::check_files(&check)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
            if report.iter().any(|r| r["error"] != serde_json::Value::Null) {
                bail!("a formula failed its check");
            }
            Ok(())
        }
        Command::Lamlet(command) => {
            if let Err(e) = lamlet(command, Some(chosen)) {
                eprintln!("{}", e.trim_end());
                std::process::exit(1);
            }
            Ok(())
        }
        Command::Script(command) => {
            if let Err(e) = script(command, chosen) {
                eprintln!("{}", e.trim_end());
                std::process::exit(1);
            }
            Ok(())
        }
        Command::Knowledge { known, levels } => knowledge(chosen, &known, levels),
        Command::Series => series(),
        Command::Graph { dot, json } => graph(chosen, dot.as_deref(), json),
        Command::Values { json, label, scope } => {
            use crate::values::Value;
            let entries: Vec<&crate::values::Entry> = crate::values::list()
                .into_iter()
                .filter(|e| label.as_deref().is_none_or(|l| e.has_label(l)))
                .filter(|e| scope.as_deref().is_none_or(|s| e.key.starts_with(&format!("{s}."))))
                .collect();
            let numbers = |v: Option<Value>| v.map(|v| v.components().iter().map(|n| n.to_string()).collect::<Vec<_>>().join(", "));
            if json {
                let list: Vec<serde_json::Value> = entries
                    .iter()
                    .map(|e| {
                        serde_json::json!({"key": e.key, "type": e.ty.name(), "default": e.default.map(|v| v.components()), "range": e.range.map(|r| [r.0, r.1]), "componentRanges": e.component_ranges.iter().map(|r| [r.0, r.1]).collect::<Vec<_>>(),
                            "step": e.step, "unit": format!("{:?}", e.unit), "wrap": e.wrap, "labels": e.labels, "text": e.text.map(|_| e.label_text()), "doc": e.doc, "engine": e.engine, "help": e.help.map(|_| e.help_text()), "components": e.components, "choices": e.choices.map(|c| c.options.iter().map(|o| o.name).collect::<Vec<_>>())})
                    })
                    .collect();
                println!("{}", serde_json::to_string_pretty(&list)?);
            } else {
                for e in entries {
                    let channels = if e.components.is_empty() { String::new() } else { format!(" ({})", e.components.join(", ")) };
                    let default = numbers(e.default).map(|d| format!(" = {d}")).unwrap_or_default();
                    let ranges: Vec<String> = (0..e.ty.components()).filter_map(|i| e.range_of(i)).map(|r| format!("{}..{}", r.0, r.1)).collect();
                    let range = if ranges.is_empty() { String::new() } else if e.component_ranges.is_empty() { format!(" in {}", ranges[0]) } else { format!(" in {}", ranges.join(", ")) };
                    let labels = if e.labels.is_empty() { String::new() } else { format!(" [{}]", e.labels.join(", ")) };
                    let choice = e.choices.map(|c| format!(" choice of {} ({})", c.name, c.names())).unwrap_or_default();
                    let engine = e.engine.map(|p| format!(" engine {p}")).unwrap_or_default();
                    println!("{}{channels}: {}{default}{range} {:?}{labels}{choice}{engine}", e.key, e.ty.name(), e.unit);
                    if !e.doc.is_empty() {
                        println!("    {}", e.doc);
                    }
                }
            }
            Ok(())
        }
        Command::Experiment { file, out, dry_run } => {
            let out = match (out, dry_run) {
                (Some(out), _) => out,
                (None, true) => PathBuf::new(),
                (None, false) => bail!("give --out, the directory for the results (or --dry-run)"),
            };
            crate::research::run(&crate::research::ResearchOptions { experiment: file, out, dry_run, root: manifest_dir.clone() })
        }
        Command::Story(StoryCommand::Keys) => {
            let keys = crate::script::keys::keys(investigation::current()).map_err(anyhow::Error::msg)?;
            println!("{}", serde_json::to_string_pretty(&keys)?);
            Ok(())
        }
        Command::Choices => {
            for choices in investigation::current().choices() {
                let options: Vec<String> = (0..choices.options.len()).map(|i| format!("{i} {} ({})", choices.options[i].name, choices.option_text(i))).collect();
                println!("{}: {}", choices.name, options.join(", "));
                if let Some(prefix) = choices.wgsl {
                    println!("    WGSL {prefix}_*, written to {}'s {}", choices.package, choices.wgsl_file());
                }
            }
            Ok(())
        }
        Command::Actions => {
            let actions = investigation::current().actions();
            for click in actions.clicks {
                println!("click {}", click.key);
                if !click.doc.is_empty() {
                    println!("    {}", click.doc);
                }
            }
            for click in crate::script::runtime::click_handlers() {
                println!("click {click} (a script's)");
            }
            for action in crate::script::runtime::actions() {
                let args: Vec<String> = action.args.iter().map(|a| a.describe()).collect();
                println!("{}({}) (a script's)", action.name, args.join(", "));
            }
            for spec in actions.specs {
                let args: Vec<String> = spec.args.iter().map(|a| format!("{}: {}{}", a.key, a.kind.describe(), if a.required { "" } else { " (optional)" })).collect();
                println!("{}({})", spec.key, args.join(", "));
                if !spec.doc.is_empty() {
                    println!("    {}", spec.doc);
                }
            }
            Ok(())
        }
        Command::Images => {
            for image in investigation::current().images() {
                println!("{}", image.key);
                if !image.doc.is_empty() {
                    println!("    {}", image.doc);
                }
            }
            for a in investigation::current().annotations() {
                let kind = match a.kind {
                    crate::utility::annotations::AnnotationKind::Point => "point",
                    crate::utility::annotations::AnnotationKind::ArrowSet => "arrow set",
                };
                println!("{kind} {}", a.key);
                if !a.doc.is_empty() {
                    println!("    {}", a.doc);
                }
            }
            let package = investigation::current().package().unwrap_or("render_lab");
            for kind in crate::views::kinds(package)? {
                let params: Vec<String> = kind.params.iter().map(|p| format!("{} = {}", p.key, p.default.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(","))).collect();
                println!("kind {} ({}x{}{}, {} · {}): {}", kind.key, kind.size[0], kind.size[1], if kind.viewport_aspect { ", the viewport's shape" } else { "" }, kind.module, kind.entry, params.join(", "));
                if !kind.doc.is_empty() {
                    println!("    {}", kind.doc);
                }
            }
            Ok(())
        }
        Command::Packages { json } => {
            let packages = crate::resources::packages();
            // Its library: the file, and its handles or why it is not loaded.
            let library = |m: &crate::packages::Package| -> Option<(String, Result<Vec<String>, String>)> {
                let (name, _) = m.library.as_ref()?;
                let loaded = investigations::libraries().iter().find(|l| l.package == m.name)?;
                Some((crate::packages::library_path(m, name).display().to_string(), loaded.loaded.as_ref().map(|h| h.names().map(String::from).collect()).map_err(Clone::clone)))
            };
            if json {
                let list: Vec<serde_json::Value> = packages
                    .iter()
                    .map(|m| {
                        let demos: Vec<serde_json::Value> = m.demos.iter().map(|d| serde_json::json!({"key": d.key, "title": d.title, "edition": d.edition, "timeline": d.timeline, "audio": d.audio})).collect();
                        let library = library(m).map(|(file, loaded)| match loaded {
                            Ok(handles) => serde_json::json!({"file": file, "handles": handles}),
                            Err(error) => serde_json::json!({"file": file, "error": error}),
                        });
                        serde_json::json!({"name": m.name, "author": m.author, "contact": m.contact, "license": m.license, "manifest": m.manifest, "import": m.imports, "export": m.exports, "library": library, "demos": demos})
                    })
                    .collect();
                println!("{}", serde_json::to_string_pretty(&list)?);
            } else {
                for m in packages {
                    let contact = m.contact.as_deref().map(|c| format!(" <{c}>")).unwrap_or_default();
                    println!("{}  by {}{contact}, {}  ({})", m.name, m.author, m.license, m.manifest);
                    println!("    import: {}", if m.imports.is_empty() { "-".to_string() } else { m.imports.join(", ") });
                    println!("    export: {}", if m.exports.is_empty() { "-".to_string() } else { m.exports.join(", ") });
                    match library(m) {
                        Some((file, Ok(handles))) => println!("    library: {file}, handles: {}", handles.join(", ")),
                        Some((file, Err(_))) => println!("    library: {file}, not loaded (above)"),
                        None => {}
                    }
                    for d in &m.demos {
                        println!("    demo {}: {} ({}), {} + {}", d.key, d.title, d.edition, d.timeline, d.audio);
                    }
                    for s in &m.series {
                        println!("    series {}: {}, parts {}", s.key, s.title_in("en"), s.parts.join(" -> "));
                    }
                }
            }
            Ok(())
        }
        Command::Resources { json } => {
            let all = crate::resources::all();
            let size = |r: &crate::resources::Resource| std::fs::metadata(r.path()).ok().map(|m| m.len());
            if json {
                let list: Vec<serde_json::Value> = all
                    .iter()
                    .map(|r| serde_json::json!({"id": r.id(), "kind": r.kind(), "file": r.path(), "bytes": size(r), "built": r.built(), "license": r.license(), "manifest": r.manifest(), "source": r.source()}))
                    .collect();
                println!("{}", serde_json::to_string_pretty(&list)?);
            } else {
                for r in all {
                    let size = size(r).map(|b| format!("{b:>10} B")).unwrap_or_else(|| format!("{:>12}", "not built"));
                    println!("{:<44} {:<9} {size}  {:<36} {}", r.id(), r.kind(), r.license(), r.source());
                }
            }
            Ok(())
        }
        Command::Cook => {
            let Some(report) = crate::resources::build_packages()? else { bail!("{} has no source packages (RENDER_LAB_ROOT or --root names a checkout)", crate::resources::root().display()) };
            for id in &report.produced {
                println!("built   {id}");
            }
            for package in &report.libraries {
                println!("library {package}");
            }
            for path in &report.removed {
                println!("removed {}", path.display());
            }
            println!("{} built, {} up to date, in {}", report.produced.len(), report.kept, crate::resources::packages_dir().display());
            Ok(())
        }
        Command::Layout { json } => {
            let layouts = layout::parse_layouts(&shaders::layout_json()?)?;
            let checks = layout::check_all(&layouts);
            if json {
                println!("{}", serde_json::to_string_pretty(&serde_json::json!({"wgsl": layouts, "checks": checks}))?);
            } else {
                for c in &checks {
                    println!("{:<16} {:>4} B  padding {:>3} B  {}", c.record, c.wgsl_size, c.padding_bytes, if c.ok { "Rust = WGSL" } else { "MISMATCH" });
                    for f in &c.fields {
                        println!("    {:>4}  {:<22} {:<12} {:>3} B{}", f.wgsl_offset, f.name, f.wgsl_type, f.wgsl_size, if f.ok { "" } else { "  <-- mismatch" });
                    }
                }
            }
            Ok(())
        }
    }
}

/// The `graph` command: print the frame's graph; fail if its declarations
/// do not compile or do not match the WGSL.
fn graph(chosen: &'static dyn investigation::Investigation, dot: Option<&std::path::Path>, json: bool) -> Result<()> {
    let shading = chosen.scene_shading();
    let graph = crate::graph::FrameGraph::new(shading, &[]);
    let mut problems = graph.errors.clone();
    if let Some(s) = shading {
        problems.extend(crate::graph::lint(s)?);
    }
    // The engine passes the source index maps the nodes to.
    let index = crate::sources::index();
    for p in &index.passes {
        if graph.node(&p.lab).is_none() {
            problems.push(format!("source index: pass `{}` maps `{}`, which is no node of the graph", p.engine, p.lab));
        }
        for k in p.sources.iter().filter(|k| !index.entries.contains_key(*k)) {
            problems.push(format!("source index: pass `{}` cites `{k}`, which is no entry", p.engine));
        }
    }
    if json {
        let mut value = graph.json();
        value["errors"] = serde_json::json!(problems);
        value["engine"] = serde_json::json!(index.passes.iter().map(|p| serde_json::json!({"node": p.lab, "pass": p.engine, "sources": p.sources})).collect::<Vec<_>>());
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        print!("{}", graph.text());
        if !index.passes.is_empty() {
            println!("engine passes ({}):", chosen.engine().name);
            for p in &index.passes {
                println!("  {:32} {} [{}]", p.lab, p.engine, p.sources.join(", "));
            }
        }
    }
    if let Some(path) = dot {
        std::fs::write(path, graph.dot()).with_context(|| format!("write {}", path.display()))?;
        eprintln!("wrote {}", path.display());
    }
    if !problems.is_empty() {
        bail!("{} problem(s):\n  {}", problems.len(), problems.join("\n  "));
    }
    Ok(())
}

/// The `knowledge` command: check the investigation's knowledge graph, print its
/// audience split (stored, or `known`), and optionally its nearby audiences.
/// Fails on a graph or audience problem.
/// `render-lab series`: each series, its parts, and the knowledge they carry from one to the next.
fn series() -> Result<()> {
    let mut problems = Vec::new();
    let all: Vec<_> = crate::resources::series().collect();
    if all.is_empty() {
        println!("no package provides a series");
    }
    for (package, series) in all {
        println!("{}.{}: {} ({} parts)", package.name, series.key, series.title_in("en"), series.parts.len());
        // Each part's graph, and what the parts before it taught (by topic name).
        let mut taught: Vec<(usize, String)> = Vec::new();
        for (k, part) in series.parts.iter().enumerate() {
            let investigation = crate::investigations::all().iter().copied().find(|i| i.package() == Some(part.as_str()));
            let timelines = crate::resources::timelines(part).len();
            match investigation {
                Some(i) => println!("  part {}: {part}: {} ({}), {timelines} narrated timeline(s)", k + 1, i.title(), i.path()),
                None => problems.push(format!("{}: part {} (`{part}`) provides no investigation", series.key, k + 1)),
            }
            let graph = match crate::knowledge::load(part) {
                None => {
                    println!("    no knowledge graph");
                    continue;
                }
                Some(graph) => graph.map_err(|e| anyhow::anyhow!(e))?,
            };
            let audience = graph.audience();
            for (earlier, topic) in &taught {
                let Some(i) = graph.nodes.iter().position(|n| &n.id == topic) else { continue };
                if audience.relevant.get(i).copied().unwrap_or(false) && !audience.is_known(i) {
                    let why = if audience.teach.contains(&i) || audience.level.contains(&i) { "teaches it again" } else { "does not know it" };
                    problems.push(format!("{}: part {} (`{part}`) needs `{topic}`, which part {earlier} teaches, and {why} (add it to its audience's `known`)", series.key, k + 1));
                }
            }
            let teaches: Vec<usize> = audience.level.iter().chain(&audience.teach).copied().collect();
            println!("    teaches {} topic(s); its audience knows {} the parts before it taught", teaches.len(), taught.iter().filter(|(_, t)| graph.nodes.iter().position(|n| &n.id == t).is_some_and(|i| audience.is_known(i))).count());
            for i in teaches {
                if !taught.iter().any(|(_, t)| t == &graph.nodes[i].id) {
                    taught.push((k + 1, graph.nodes[i].id.clone()));
                }
            }
        }
    }
    for p in &problems {
        println!("problem: {p}");
    }
    if problems.is_empty() { Ok(()) } else { bail!("{} problem(s) in the series", problems.len()) }
}

fn knowledge(chosen: &'static dyn investigation::Investigation, known: &[String], levels: bool) -> Result<()> {
    let package = chosen.package().context("this investigation has no package")?;
    let graph = match crate::knowledge::load(package) {
        None => bail!("{package} has no knowledge graph (no `knowledge` resource of its own)"),
        Some(graph) => graph.map_err(|e| anyhow::anyhow!(e))?,
    };
    let names = |list: &[usize]| list.iter().map(|&i| graph.nodes[i].key.as_str()).collect::<Vec<_>>().join(", ");
    let edges: usize = graph.nodes.iter().map(|n| n.requires.len()).sum();
    println!("{package}: {} topics, {edges} requirements, {} chapters; acyclic, transitively reduced, every topic reaches a base", graph.nodes.len(), graph.chapters.len());
    let known: Vec<usize> = if known.is_empty() {
        graph.known.clone()
    } else {
        known.iter().map(|k| graph.find(k).with_context(|| format!("no topic `{k}`"))).collect::<Result<_>>()?
    };
    if let Some(profile) = &graph.profile {
        println!("audience: {profile}");
    }
    let p = graph.partition_for(&known);
    let relevant = p.relevant.iter().filter(|r| **r).count();
    let known_count = p.known.iter().filter(|k| **k).count();
    println!("relevant {relevant}, known {known_count}, level {}, to teach {}, carries {}", p.level.len(), p.teach.len(), p.carries.len());
    println!("level: {}", names(&p.level));
    println!("to teach: {}", names(&p.teach));
    println!("carried (known, used directly above the level): {}", names(&p.carried()));
    for c in &graph.chapters {
        let context = graph.context(&[c.number]);
        let own = |list: &[usize]| list.iter().copied().filter(|&i| context[i]).collect::<Vec<_>>();
        println!("chapter {}: level {}; teaches {}", c.number, names(&own(&p.level)), names(&own(&p.teach)));
    }
    if levels {
        println!("nearby audiences (carries):");
        for n in crate::knowledge::neighbours(&graph.requires(), &graph.bases(), &graph.chapter_needs(), &known) {
            let q = graph.partition_for(&n.known);
            println!("  {}{} ({}): level {}, to teach {}", if n.added { "+" } else { "-" }, graph.nodes[n.node].key, n.carries, q.level.len(), q.teach.len());
        }
    }
    let demos: Vec<&str> = chosen.demos().iter().map(|d| d.key()).collect();
    let mut problems = graph.check(&demos);
    // Trying an audience, the stored one's problems do not apply.
    if known != graph.known {
        problems.retain(|p| p.contains("no demonstration"));
    }
    if !problems.is_empty() {
        bail!("{}", problems.join("\n"));
    }
    Ok(())
}
