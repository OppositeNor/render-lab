//! The command line: without a command it opens the homepage, where an
//! engine and a feature are chosen and the interactive viewer is opened.
//! `render` records a timeline deterministically, `selftest` verifies the
//! renderer and the investigation's content, and `viewer-autotest` tests the
//! viewer's controls. The binary (src/main.rs in the repository) calls
//! `main`; the investigations come from the packages' libraries
//! (src/investigations.rs).

use crate::home;
use crate::investigation::Investigation;
use crate::investigations;
use crate::text::DEFAULT_UI_SCALE;
use crate::{investigation, layout, record, shaders, viewer};
use anyhow::{Context, Result, bail};
use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
use std::path::{Path, PathBuf};

mod report;
mod run;
use report::*;
use run::*;

#[derive(Parser)]
#[command(
    name = "render-lab",
    version,
    about = "Illustrated investigations of game-engine features: homepage, viewer, recorder, and verification"
)]
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
        /// viewer-layout (the viewer's windows moved, docked, and resized; the layout written beside the PNG as .toml), or
        /// viewer-scene[:<entity>][@move|rotate|scale] (the Scene window open, an entity selected, its gizmo dragged), or
        /// viewer-camera (the camera's modes driven through the input handlers and checked: orbit, pan, fly), or
        /// viewer-free[:<demo>] (the free camera on: the main camera and its view cone seen from beside it), or
        /// select-knowledge (the selection page with the chosen feature's knowledge graph full screen),
        /// or a page of a viewer feature's (src/feature/; --help lists them).
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
        /// The encoder: h264 (x264), h264-nvenc (the GPU's NVENC), ffv1, or rawvideo.
        #[arg(long, default_value = "h264")]
        codec: String,
        /// x264's CRF, or NVENC's constant quantiser.
        #[arg(long, default_value_t = 18)]
        crf: u32,
        /// x264's preset (NVENC uses p7).
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
    /// (lab/src/graph/): its nodes in execution order with what each reads
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
    /// show (`{inset "<key>"}`), and the image kinds its package's shaders
    /// define (`{inset "<name>"} {imageFrom "<kind>"}`, their parameters
    /// `image.<param>`, GPU
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
    /// Report the text tables of every package, per language: the keys a language's own table lacks (shown in a
    /// fallback language), the texts equal to English's (often fine: technical terms), and the characters the
    /// bundled fonts cannot draw. Prints JSON; with --check, fails on a missing key of a complete language or a
    /// character without a glyph.
    I18n {
        #[arg(long)]
        check: bool,
    },
    /// Print a package's text table in one language, as the lab resolves it: a JSON object of key → text (a
    /// message with variables as Fluent writes them missing, `{$seconds}`). The video builder reads tables so.
    Text {
        package: String,
        #[arg(long, default_value = "en")]
        locale: String,
        /// The package's source table instead (`text/sources/<locale>.ftl`: the source index's text in another
        /// language, along the locale's fallback chain; empty in English, the index's own).
        #[arg(long)]
        sources: bool,
    },
    /// Typeset every formula in these files (Fluent text tables, timelines) and check the lab's limits: glyphs of at least 24 px, display
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
    /// The key list with what writing each needs (src/script/keys.rs `schema`): a value's type, unit, range,
    /// default, and doc; the attribute and function of each key; and what the vocabulary leaves out (options that
    /// make no Lamlet name, keys whose function name another has), each with the `attr` that still writes it.
    Schema,
    /// The keys a storyboard's headings take (src/script/keys.rs `heading_keys`, also in `keys`), or with
    /// --translation its translation's, as a key list for `lamlet ir --keys`; needs no investigation.
    Headings {
        #[arg(long)]
        translation: bool,
    },
    /// Check untimed cues as JSON, without speech or a GPU; `-` reads stdin.
    /// With --timeline, check a complete measured timeline instead.
    Check {
        file: PathBuf,
        #[arg(long)]
        timeline: bool,
    },
}

#[derive(Subcommand)]
enum LamletCommand {
    /// The document's IR as JSON, its prose read as CommonMark (`lamlet build --to ir --prose commonmark`).
    Ir {
        file: PathBuf,
        #[arg(long = "package", value_name = "NAME=PATH", value_parser = parse_package)]
        packages: Vec<(String, PathBuf)>,
        /// Check every cue attribute against these keys, and read every heading's attributes by them (lamlet-present's
        /// key list, `--keys` on `lamlet present`'s commands; `render-lab story keys` writes the lab's).
        #[arg(long)]
        keys: Option<PathBuf>,
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
fn lamlet(
    command: LamletCommand,
    investigation: Option<&'static dyn crate::investigation::Investigation>,
) -> Result<(), String> {
    use crate::script;
    let read = |given: Vec<(String, PathBuf)>| -> Result<Vec<script::Package>, String> {
        script::packages(
            given
                .iter()
                .map(|(name, path)| script::Package::read(name, path))
                .collect::<Result<_, _>>()?,
        )
    };
    let write = |out: Option<PathBuf>, text: String| -> Result<(), String> {
        match out {
            Some(path) => {
                std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))
            }
            None => {
                print!("{text}");
                Ok(())
            }
        }
    };
    match command {
        LamletCommand::Ir {
            file,
            packages,
            keys,
            out,
        } => {
            let keys = keys
                .map(|k| std::fs::read_to_string(&k).map_err(|e| format!("{}: {e}", k.display())))
                .transpose()?;
            write(
                out,
                script::ir(&file, &read(packages)?, investigation, keys.as_deref())?,
            )
        }
        LamletCommand::Deck {
            file,
            packages,
            out,
        } => {
            let deck = script::deck(&file, &read(packages)?, investigation)?;
            write(
                out,
                serde_json::to_string_pretty(&deck).map_err(|e| e.to_string())? + "\n",
            )
        }
        LamletCommand::Toml { file, name, out } => {
            let text =
                std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
            write(
                out,
                script::data::toml(&file.display().to_string(), &text, &name)?,
            )
        }
        LamletCommand::Vocab {
            name,
            keys,
            packages,
            out,
        } => {
            let json =
                std::fs::read_to_string(&keys).map_err(|e| format!("{}: {e}", keys.display()))?;
            write(
                out,
                script::vocab(&name, &json, &read(packages)?)
                    .map_err(|e| format!("{}: {e}", keys.display()))?,
            )
        }
    }
}

/// `render-lab --investigation <id> script …`.
fn script(
    command: ScriptCommand,
    investigation: &'static dyn crate::investigation::Investigation,
) -> Result<(), String> {
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
                    let p = sources
                        .packages
                        .iter()
                        .find(|p| p.name == name)
                        .ok_or_else(|| format!("no package `{name}`"))?;
                    print!("{}", p.source);
                }
                None => {
                    for p in &sources.packages {
                        println!("{}\t{}", p.name, p.file);
                    }
                    if !sources.skipped.is_empty() {
                        println!(
                            "values left out of the values package (their names are taken): {}",
                            sources.skipped.join(", ")
                        );
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
            let (w, h) = other
                .split_once('x')
                .ok_or("expected WIDTHxHEIGHT or 720p, 1080p, 1440p, 2160p")?;
            [
                w.parse().map_err(|_| format!("bad width `{w}`"))?,
                h.parse().map_err(|_| format!("bad height `{h}`"))?,
            ]
        }
    };
    if size.iter().any(|&n| !(2..=8192).contains(&n) || n % 2 != 0) {
        return Err(format!(
            "{}x{}: width and height are even, from 2 to 8192",
            size[0], size[1]
        ));
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
    // `--page`'s help lists the lab's features' pages too (src/feature/).
    let pages = format!(
        "{}\n\nThe viewer's features' pages: {}; and the pages of the packages' features.",
        Cli::command()
            .find_subcommand("home")
            .and_then(|c| c.get_arguments().find(|a| a.get_id() == "page"))
            .and_then(|a| a.get_long_help().or(a.get_help()))
            .map(|h| h.to_string())
            .unwrap_or_default(),
        crate::feature::page_help()
    );
    let command =
        Cli::command().mut_subcommand("home", |c| c.mut_arg("page", |a| a.long_help(pages)));
    let cli = Cli::from_arg_matches(&command.get_matches()).unwrap_or_else(|e| e.exit());
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
            println!(
                "{:width$}  {} ({}, package {})",
                i.path(),
                i.title(),
                i.engine().name,
                i.package().unwrap_or("render_lab")
            );
        }
        for (i, why) in investigations::unavailable() {
            // An investigation whose package requires what this machine lacks: listed, with why it cannot run.
            println!("{:width$}  {} (unavailable: {why})", i.path(), i.title());
        }
        return Ok(());
    }
    let command = cli.command.unwrap_or(Command::Home {
        width: 1600,
        height: 900,
        ui_scale: DEFAULT_UI_SCALE,
        snapshot: None,
        page: "title".into(),
        autotest: None,
    });
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
    let chosen = choose(cli.investigation.as_deref())?;
    run(command, chosen, &manifest_dir)
}

/// The investigation `path` names (the first installed when None), made the current one.
fn choose(path: Option<&str>) -> Result<&'static dyn Investigation> {
    let known: Vec<String> = investigations::all().iter().map(|i| i.path()).collect();
    let path = path
        .map(str::to_string)
        .or_else(|| known.first().cloned())
        .context("no investigation is installed")?;
    // One that cannot run here says why, rather than being unknown.
    if let Some((_, why)) = investigations::unavailable()
        .into_iter()
        .find(|(i, _)| i.path() == path)
    {
        anyhow::bail!("investigation `{path}` cannot run on this machine: it {why}");
    }
    let chosen = investigations::find(&path).with_context(|| {
        format!(
            "unknown investigation `{path}`; known: {} (render-lab --list)",
            known.join(", ")
        )
    })?;
    investigation::set(chosen);
    Ok(chosen)
}

/// The `graph` command: print the frame's graph; fail if its declarations
/// do not compile or do not match the WGSL.
fn graph(
    chosen: &'static dyn investigation::Investigation,
    dot: Option<&std::path::Path>,
    json: bool,
) -> Result<()> {
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
            problems.push(format!(
                "source index: pass `{}` maps `{}`, which is no node of the graph",
                p.engine, p.lab
            ));
        }
        for k in p.sources.iter().filter(|k| !index.entries.contains_key(*k)) {
            problems.push(format!(
                "source index: pass `{}` cites `{k}`, which is no entry",
                p.engine
            ));
        }
    }
    if json {
        let mut value = graph.json();
        value["errors"] = serde_json::json!(problems);
        value["engine"] = serde_json::json!(
            index
                .passes
                .iter()
                .map(|p| serde_json::json!({"node": p.lab, "pass": p.engine, "sources": p.sources}))
                .collect::<Vec<_>>()
        );
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
        bail!(
            "{} problem(s):\n  {}",
            problems.len(),
            problems.join("\n  ")
        );
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
        println!(
            "{}.{}: {} ({} parts)",
            package.name,
            series.key,
            series.title_in("en"),
            series.parts.len()
        );
        // Each part's graph, and what the parts before it taught (by topic name).
        let mut taught: Vec<(usize, String)> = Vec::new();
        for (k, part) in series.parts.iter().enumerate() {
            let investigation = crate::investigations::all()
                .iter()
                .copied()
                .find(|i| i.package() == Some(part.as_str()));
            let timelines = crate::resources::timelines(part).len();
            match investigation {
                Some(i) => println!(
                    "  part {}: {part}: {} ({}), {timelines} narrated timeline(s)",
                    k + 1,
                    i.title(),
                    i.path()
                ),
                None => problems.push(format!(
                    "{}: part {} (`{part}`) provides no investigation",
                    series.key,
                    k + 1
                )),
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
                let Some(i) = graph.nodes.iter().position(|n| &n.id == topic) else {
                    continue;
                };
                if audience.relevant.get(i).copied().unwrap_or(false) && !audience.is_known(i) {
                    let why = if audience.teach.contains(&i) || audience.level.contains(&i) {
                        "teaches it again"
                    } else {
                        "does not know it"
                    };
                    problems.push(format!("{}: part {} (`{part}`) needs `{topic}`, which part {earlier} teaches, and {why} (add it to its audience's `known`)", series.key, k + 1));
                }
            }
            let teaches: Vec<usize> = audience
                .level
                .iter()
                .chain(&audience.teach)
                .copied()
                .collect();
            println!(
                "    teaches {} topic(s); its audience knows {} the parts before it taught",
                teaches.len(),
                taught
                    .iter()
                    .filter(|(_, t)| graph
                        .nodes
                        .iter()
                        .position(|n| &n.id == t)
                        .is_some_and(|i| audience.is_known(i)))
                    .count()
            );
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
    if problems.is_empty() {
        Ok(())
    } else {
        bail!("{} problem(s) in the series", problems.len())
    }
}

fn knowledge(
    chosen: &'static dyn investigation::Investigation,
    known: &[String],
    levels: bool,
) -> Result<()> {
    let package = chosen
        .package()
        .context("this investigation has no package")?;
    let graph = match crate::knowledge::load(package) {
        None => bail!("{package} has no knowledge graph (no `knowledge` resource of its own)"),
        Some(graph) => graph.map_err(|e| anyhow::anyhow!(e))?,
    };
    let names = |list: &[usize]| {
        list.iter()
            .map(|&i| graph.nodes[i].key.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };
    let edges: usize = graph.nodes.iter().map(|n| n.requires.len()).sum();
    println!(
        "{package}: {} topics, {edges} requirements, {} chapters; acyclic, transitively reduced, every topic reaches a base",
        graph.nodes.len(),
        graph.chapters.len()
    );
    let known: Vec<usize> = if known.is_empty() {
        graph.known.clone()
    } else {
        known
            .iter()
            .map(|k| graph.find(k).with_context(|| format!("no topic `{k}`")))
            .collect::<Result<_>>()?
    };
    if let Some(profile) = &graph.profile {
        println!("audience: {profile}");
    }
    let p = graph.partition_for(&known);
    let relevant = p.relevant.iter().filter(|r| **r).count();
    let known_count = p.known.iter().filter(|k| **k).count();
    println!(
        "relevant {relevant}, known {known_count}, level {}, to teach {}, carries {}",
        p.level.len(),
        p.teach.len(),
        p.carries.len()
    );
    println!("level: {}", names(&p.level));
    println!("to teach: {}", names(&p.teach));
    println!(
        "carried (known, used directly above the level): {}",
        names(&p.carried())
    );
    for c in &graph.chapters {
        let context = graph.context(&[c.number]);
        let own = |list: &[usize]| {
            list.iter()
                .copied()
                .filter(|&i| context[i])
                .collect::<Vec<_>>()
        };
        println!(
            "chapter {}: level {}; teaches {}",
            c.number,
            names(&own(&p.level)),
            names(&own(&p.teach))
        );
    }
    if levels {
        println!("nearby audiences (carries):");
        for n in crate::knowledge::neighbours(
            &graph.requires(),
            &graph.bases(),
            &graph.chapter_needs(),
            &known,
        ) {
            let q = graph.partition_for(&n.known);
            println!(
                "  {}{} ({}): level {}, to teach {}",
                if n.added {
                    "+"
                } else {
                    "-"
                },
                graph.nodes[n.node].key,
                n.carries,
                q.level.len(),
                q.teach.len()
            );
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
