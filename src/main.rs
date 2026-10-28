//! Render Lab: illustrated investigations of game-engine features, rendered
//! with wgpu and WGSL. Without a command it opens the homepage, where an
//! engine and a feature are chosen and the interactive viewer is opened. `render` records a timeline
//! deterministically, `selftest` verifies the renderer and the
//! investigation's content, and `viewer-autotest` tests the viewer's controls.
//!
//! `src/lab` holds the shared renderer, recorder, viewer, and dock;
//! `src/investigations.rs` lists the investigations; the public ones live in
//! `src/<engine>/<feature>` (`src/o3de/lighting`).
//!
//! Investigations of commercial engines live in `src/commercial`, which is
//! not published (see .gitignore). Without it, build.rs leaves `commercial`
//! unset and the lab builds with the public investigations alone: the shared
//! code the commercial ones use is then partly unused.
#![cfg_attr(not(commercial), allow(dead_code))]

mod investigations;
mod lab;
mod o3de;
#[cfg(commercial)]
mod commercial;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use lab::home;
use lab::text::DEFAULT_UI_SCALE;
use lab::{investigation, layout, record, shaders, viewer};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "render-lab", version, about = "Illustrated investigations of game-engine features: homepage, viewer, recorder, and verification")]
struct Cli {
    /// The investigation, as engine/feature (default: the first one).
    #[arg(long, global = true)]
    investigation: Option<String>,
    /// The lab's root, where its packages and their content are (default:
    /// RENDER_LAB_ROOT, or the crate's directory).
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
        /// The page the snapshot shows: title, select, config, trial (the Config page's countdown), or
        /// loading (the viewer's loading page, while the current investigation's first scene loads).
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
    /// List the resources the packages declare (their `resources.toml`
    /// manifests): id, kind, cached size, license, and source.
    Resources {
        #[arg(long)]
        json: bool,
    },
    /// Process every resource that is not cached yet (build/resources), and
    /// remove cached files no resource names any more.
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

fn main() -> Result<()> {
    let cli = Cli::parse();
    if let Some(root) = &cli.root {
        lab::resources::set_root(root.clone());
    }
    let manifest_dir = lab::resources::root().to_path_buf();
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
            let options = home::HomeOptions { manifest_dir: manifest_dir.clone(), width, height, ui_scale: ui_scale.clamp(0.4, 1.5), config_path: lab::config::Config::default_path() };
            match (snapshot, autotest) {
                (Some(png), _) if page == "loading" => lab::loading::snapshot(&png, [width.max(64), height.max(64)], options.ui_scale),
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
            viewer::run(viewer::ViewerOptions { timeline, audio: None, manifest_dir: manifest_dir.clone(), ui_scale: ui_scale.clamp(0.4, 1.5), autotest: Some(report), width, height, locale: None, start_frame: 0, fullscreen: false })
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
            let report = lab::formula::check_files(&check)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
            if report.iter().any(|r| r["error"] != serde_json::Value::Null) {
                bail!("a formula failed its check");
            }
            Ok(())
        }
        Command::Resources { json } => {
            let all = lab::resources::all();
            let cache = lab::resources::cache_dir();
            // The size of its cached output, if it is cooked; without cooking it.
            let cached = |id: &str| -> Option<u64> {
                let prefix = format!("{}-", id.replace('/', "-"));
                let file = std::fs::read_dir(&cache).ok()?.flatten().find(|e| {
                    let name = e.file_name().to_string_lossy().into_owned();
                    name.strip_prefix(&prefix).is_some_and(|rest| rest.len() == 28 && rest.ends_with(".bin"))
                })?;
                Some(file.metadata().ok()?.len())
            };
            if json {
                let list: Vec<serde_json::Value> = all
                    .iter()
                    .map(|r| serde_json::json!({"id": r.id(), "kind": r.kind(), "cachedBytes": cached(r.id()), "license": r.license(), "manifest": r.manifest(), "source": r.source()}))
                    .collect();
                println!("{}", serde_json::to_string_pretty(&list)?);
            } else {
                for r in all {
                    let size = cached(r.id()).map(|b| format!("{b:>10} B")).unwrap_or_else(|| format!("{:>12}", "not cooked"));
                    println!("{:<28} {:<9} {size}  {:<36} {}", r.id(), r.kind(), r.license(), r.source());
                }
            }
            Ok(())
        }
        Command::Cook => {
            for c in lab::resources::cook()? {
                println!("{:<28} {:<9} {:>10} B  {}", c.resource.id(), c.resource.kind(), c.bytes, if c.cached { "cached" } else { "cooked" });
            }
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
            let statics = lab::frame::Statics::prepare_cpu(lab::scene::SceneId::STUDY_ROOM)?;
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
                    "pages": fx.surface_cache.pages.len(), "atlasTexels": fx.surface_cache.atlas_size, "pageTexels": lab::fixtures::PHYSICAL_PAGE_SIZE,
                },
                "probes": {
                    "count": fx.probes.records.len(), "samples": fx.probes.samples.len(), "selected": fx.probes.selected,
                    "selectedPosition": fx.probes.records[fx.probes.selected].position, "selectedNormal": fx.probes.records[fx.probes.selected].normal,
                    "historySteps": fx.probes.history.len(), "historyMoveStep": lab::fixtures::HISTORY_MOVE_STEP, "spacingPx": lab::fixtures::PROBE_SPACING_PX,
                    "history": fx.probes.history.iter().map(|h| serde_json::json!({"frames": h.frames, "weight": h.weight, "fastUpdate": h.fast_update})).collect::<Vec<_>>(),
                },
            });
            std::fs::write(&output, serde_json::to_string_pretty(&summary)?)?;
            if let Some(png) = atlas_png {
                // One PNG per layer: <stem>-<layer>.png.
                let s = fx.surface_cache.atlas_size;
                for layer in crate::lab::surface_cache::Layer::ALL {
                    let name = format!("{}-{}.png", png.file_stem().and_then(|n| n.to_str()).unwrap_or("atlas"), layer.name_and_format().0);
                    record::write_png(&png.with_file_name(name), s[0], s[1], &fx.surface_cache.display_rgba(layer, statics.scene.lighting.exposure))?;
                }
            }
            Ok(())
        }
    }
}
