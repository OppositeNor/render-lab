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
    /// The investigation, as engine/feature (default: the first one).
    #[arg(long, global = true)]
    investigation: Option<String>,
    /// The lab's root, where its packages are (default:
    /// RENDER_LAB_ROOT, or the repository's directory).
    #[arg(long, global = true)]
    root: Option<PathBuf>,
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
        /// the investigation's first timeline, with its bar), or viewer-diagram (that timeline paused on its first diagram
        /// beat, the diagram panel zoomed in).
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
    /// List the current investigation's choices (`#[derive(Choice)]` enums),
    /// their options, and the WGSL constants the lab writes for its shaders
    /// (its package's `build/generated/<name>.wgsl`) whenever it loads it.
    Choices,
    /// List the current investigation's actions (lab/src/actions.rs): what a
    /// story starts with `run=<key>` and stops with `stop=<key>`, and the
    /// arguments it gives them (`run.<argument>=…`).
    Actions,
    /// List the images the current investigation publishes for a story to
    /// show (`{@ image=<key>}`), and the image kinds its package's shaders
    /// define (`{@ image=<name> imageFrom=<kind> image.<param>=…}`, GPU
    /// images); a story can also show a PNG resource by its key.
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
    #[cfg(commercial)]
    /// Write a summary of the prepared teaching data (and the atlas as PNG).
    Fixtures {
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        atlas_png: Option<PathBuf>,
    },
}

/// `--resolution`: a 16:9 preset by its height, or WIDTHxHEIGHT (even, for
/// FFmpeg's yuv420p).
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
    investigations::set_title(app.title);
    let manifest_dir = crate::resources::root().to_path_buf();
    let command = cli.command.unwrap_or(Command::Home { width: 1600, height: 900, ui_scale: DEFAULT_UI_SCALE, snapshot: None, page: "title".into(), autotest: None });
    let chosen = {
        let known: Vec<String> = investigations::all().iter().map(|i| i.path()).collect();
        let path = cli.investigation.clone().or_else(|| known.first().cloned()).context("no investigation is installed")?;
        let chosen = investigations::find(&path).with_context(|| format!("unknown investigation `{path}`; known: {}", known.join(", ")))?;
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
                (Some(png), _) if page == "viewer-diagram" => home::snapshot_viewer_page(&options, true, None, true, &png),
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
            viewer::run(viewer::ViewerOptions { timeline, audio: None, manifest_dir: manifest_dir.clone(), ui_scale: ui_scale.clamp(0.4, 1.5), autotest: Some(report), width, height, locale: None, start_frame: 0, fullscreen: false, narration: crate::config::Config::default().narration })
        }
        Command::Render { timeline, output, frames, resolution, snapshot, snapshot_dir, report, codec, crf, preset, ffmpeg, frame_stamp } => {
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
                            "step": e.step, "unit": format!("{:?}", e.unit), "wrap": e.wrap, "labels": e.labels, "text": e.text.map(|_| e.label_text()), "doc": e.doc, "components": e.components, "choices": e.choices.map(|c| c.options.iter().map(|o| o.name).collect::<Vec<_>>())})
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
                    println!("{}{channels}: {}{default}{range} {:?}{labels}{choice}", e.key, e.ty.name(), e.unit);
                    if !e.doc.is_empty() {
                        println!("    {}", e.doc);
                    }
                }
            }
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
            for spec in investigation::current().actions().specs {
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
            let package = investigation::current().package().unwrap_or("rlstd");
            for kind in crate::views::kinds(package)? {
                let params: Vec<String> = kind.params.iter().map(|p| format!("{} = {}", p.key, p.default.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(","))).collect();
                println!("kind {} ({}x{}, {} · {}): {}", kind.key, kind.size[0], kind.size[1], kind.module, kind.entry, params.join(", "));
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
        #[cfg(commercial)]
        Command::Fixtures { output, atlas_png } => {
            // The CPU reference build: no GPU needed.
            let statics = crate::frame::Statics::prepare_cpu(crate::scene::SceneId::STUDY_ROOM)?;
            let fx = &statics.fixtures;
            let df = &fx.distance_field;
            let summary = serde_json::json!({
                "note": "Prepared teaching data, generated deterministically by src/commercial/lab/fixtures.rs; not produced by an engine's builders.",
                "distanceField": {
                    "object": "sphere", "radius": df.radius, "volumeMin": df.volume_min.to_array(), "volumeSize": df.volume_size,
                    "mips": df.mips.iter().enumerate().map(|(i, m)| serde_json::json!({"mip": i, "bricksPerAxis": m.dims, "voxelSize": m.voxel_size, "band": m.band, "scaleBias": m.scale_bias, "stored": m.stored, "virtual": m.dims.pow(3)})).collect::<Vec<_>>(),
                    "atlasBricks": df.atlas_bricks, "denseDisplaySamples": df.voxels.len(),
                },
                "surfaceCache": {
                    "cards": fx.surface_cache.cards.iter().map(|c| serde_json::json!({"name": c.name, "object": c.object, "sizeInPages": c.size_in_pages, "resLevel": c.res_level, "pageIndex": c.page_index})).collect::<Vec<_>>(),
                    "pages": fx.surface_cache.pages.len(), "atlasTexels": fx.surface_cache.atlas_size, "pageTexels": crate::fixtures::PHYSICAL_PAGE_SIZE,
                },
                "probes": {
                    "count": fx.probes.records.len(), "samples": fx.probes.samples.len(), "selected": fx.probes.selected,
                    "selectedPosition": fx.probes.records[fx.probes.selected].position, "selectedNormal": fx.probes.records[fx.probes.selected].normal,
                    "historySteps": fx.probes.history.len(), "historyMoveStep": crate::fixtures::HISTORY_MOVE_STEP, "spacingPx": crate::fixtures::PROBE_SPACING_PX,
                    "history": fx.probes.history.iter().map(|h| serde_json::json!({"frames": h.frames, "weight": h.weight, "fastUpdate": h.fast_update})).collect::<Vec<_>>(),
                },
            });
            std::fs::write(&output, serde_json::to_string_pretty(&summary)?)?;
            if let Some(png) = atlas_png {
                // One PNG per layer: <stem>-<layer>.png.
                let s = fx.surface_cache.atlas_size;
                for layer in crate::surface_cache::Layer::ALL {
                    let name = format!("{}-{}.png", png.file_stem().and_then(|n| n.to_str()).unwrap_or("atlas"), layer.name_and_format().0);
                    record::write_png(&png.with_file_name(name), s[0], s[1], &fx.surface_cache.display_rgba(layer, statics.scene.lighting.exposure))?;
                }
            }
            Ok(())
        }
    }
}
