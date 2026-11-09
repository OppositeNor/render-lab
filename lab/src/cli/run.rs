//! The commands that open or record: the homepage and its offscreen pages, the viewer's test, a recording, the
//! selftest, Lamlet, scripts, and experiments.

use super::*;

/// Runs `command` for the investigation `chosen`; `manifest_dir` is the lab's root.
pub(super) fn run(
    command: Command,
    chosen: &'static dyn Investigation,
    manifest_dir: &Path,
) -> Result<()> {
    match command {
        Command::Home {
            width,
            height,
            ui_scale,
            snapshot,
            page,
            autotest,
        } => {
            let options = home::HomeOptions {
                manifest_dir: manifest_dir.to_path_buf(),
                width,
                height,
                ui_scale: ui_scale.clamp(0.4, 1.5),
                config_path: crate::config::Config::default_path(),
            };
            match (snapshot, autotest) {
                (Some(png), _) => home_snapshot(&options, &page, &png),
                (None, Some(report)) => home::autotest(&options, &report),
                (None, None) => home::run(options),
            }
        }
        Command::ViewerAutotest {
            report,
            timeline,
            ui_scale,
            width,
            height,
        } => {
            // Silent: the test plays the timeline without its narration.
            viewer::run(viewer::ViewerOptions {
                timeline,
                audio: None,
                manifest_dir: manifest_dir.to_path_buf(),
                ui_scale: ui_scale.clamp(0.4, 1.5),
                autotest: Some(report),
                width,
                height,
                locale: None,
                start_frame: 0,
                fullscreen: false,
                narration: crate::config::Config::default().narration,
                tab: None,
                layout: Default::default(),
                settings: None,
            })
        }
        command @ Command::Render { .. } => render(command, manifest_dir),
        Command::Selftest {
            report,
            timeline,
            work_dir,
            ffmpeg,
        } => {
            crate::script::runtime::set_strict(true);
            let work_dir = work_dir.unwrap_or_else(|| manifest_dir.join("target/selftest"));
            let passed = chosen.selftest(&investigation::SelftestOptions {
                report: report.clone(),
                timeline,
                ffmpeg: ffmpeg.unwrap_or_else(|| record::default_ffmpeg(manifest_dir)),
                work_dir,
            })?;
            if !passed {
                bail!("selftest failed; see {}", report.display());
            }
            eprintln!("selftest passed; report: {}", report.display());
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
        Command::Experiment { file, out, dry_run } => {
            let out = match (out, dry_run) {
                (Some(out), _) => out,
                (None, true) => PathBuf::new(),
                (None, false) => bail!("give --out, the directory for the results (or --dry-run)"),
            };
            crate::research::run(&crate::research::ResearchOptions {
                experiment: file,
                out,
                dry_run,
                root: manifest_dir.to_path_buf(),
            })
        }
        command => report(command, chosen),
    }
}

/// `render`: records a timeline, frame by frame, to a video and/or snapshots.
fn render(command: Command, manifest_dir: &Path) -> Result<()> {
    let Command::Render {
        timeline,
        output,
        frames,
        warmup,
        resolution,
        snapshot,
        snapshot_dir,
        report,
        codec,
        crf,
        preset,
        ffmpeg,
        frame_stamp,
    } = command
    else {
        unreachable!("`run` passes `render` only its command");
    };
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
        ffmpeg: ffmpeg.unwrap_or_else(|| record::default_ffmpeg(manifest_dir)),
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
    eprintln!(
        "rendered {} frames in {:.1} s ({:.1} fps); smallest text {:.1} px",
        summary.frames,
        summary.seconds,
        summary.frames as f64 / summary.seconds,
        summary.min_text_pt
    );
    Ok(())
}

/// `home --snapshot`: the homepage's `page` rendered offscreen to `png`.
fn home_snapshot(options: &home::HomeOptions, page: &str, png: &Path) -> Result<()> {
    match page {
        "loading" => crate::loading::snapshot(
            png,
            [options.width.max(64), options.height.max(64)],
            options.ui_scale,
        ),
        "starting" => home::snapshot_starting(options, png),
        "viewer" | "viewer-timeline" => {
            home::snapshot_viewer(options, page == "viewer-timeline", None, png)
        }
        "viewer-diagram" => home::snapshot_viewer_page(
            options,
            true,
            None,
            Some(crate::viewer::PausedPage::Diagram),
            png,
        ),
        "viewer-sources" => home::snapshot_viewer_page(
            options,
            true,
            None,
            Some(crate::viewer::PausedPage::Sources),
            png,
        ),
        "viewer-layout" => home::snapshot_layout(options, png),
        "viewer-camera" => home::snapshot_camera(options, png),
        _ if page == "viewer-scene"
            || page.starts_with("viewer-scene:")
            || page.starts_with("viewer-scene@") =>
        {
            let rest = page.strip_prefix("viewer-scene").unwrap_or_default();
            let (entity, mode) = rest.split_once('@').map_or((rest, ""), |(e, m)| (e, m));
            let mode = match mode {
                "" | "move" => crate::gizmo::Mode::Move,
                "rotate" => crate::gizmo::Mode::Rotate,
                "scale" => crate::gizmo::Mode::Scale,
                other => {
                    anyhow::bail!("viewer-scene@{other}: the gizmo's modes are move, rotate, scale")
                }
            };
            home::snapshot_scene(
                options,
                entity.strip_prefix(':').filter(|e| !e.is_empty()),
                mode,
                png,
            )
        }
        _ if page == "viewer-free" || page.starts_with("viewer-free:") => {
            home::snapshot_free(options, page.strip_prefix("viewer-free:"), png)
        }
        // A feature's page (src/feature/: viewer-experiments, viewer-quiz, viewer-prerequisites…): the viewer on its tab, set up as the page says.
        _ if crate::feature::page(page).is_some() => {
            let (feature, name, args) = crate::feature::page(page).expect("a feature's page");
            home::snapshot_feature(options, feature, name, args, png)
        }
        "select-knowledge" => home::snapshot_knowledge(options, png),
        _ if page.starts_with("viewer:") => {
            home::snapshot_viewer(options, false, page.strip_prefix("viewer:"), png)
        }
        _ => {
            let trial = page == "trial";
            home::snapshot(
                options,
                if trial {
                    home::Page::Config
                } else {
                    home::Page::parse(page)?
                },
                trial,
                png,
            )
        }
    }
}
