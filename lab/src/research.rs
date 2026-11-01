//! Research runs: an experiment file (src/experiment.rs) rendered headless,
//! measured, and written down so that it can be reported and repeated
//! (`render-lab --investigation <id> experiment <file> --out <dir>`).
//!
//! The reference renders first (its own frame count), then each run: its
//! values set over the base, its scene data built, `frames` frames rendered
//! so a history converges, `warmup` frames more, then `repeats` frames back
//! to back, each timed on the GPU per pass (src/timing.rs) and on the wall
//! clock; the last one is read back and measured against the reference
//! (src/metrics.rs). With `convergence`, every frame of the first `frames`
//! is measured too.
//!
//! The output directory holds:
//! - `manifest.json`: what ran: the experiment (as given, and its SHA-256),
//!   the investigation, the lab's version, the checkout's commit (and
//!   whether it had changes), the GPU and driver, the system, the
//!   resolution, whether GPU timestamps were available, and when it ran;
//! - `experiment.toml`: the experiment as the lab read it;
//! - `results.csv`: a row per run and repeat: the values each run varies,
//!   then the metrics, then each pass's GPU milliseconds (`gpu:<pass>`);
//! - `results.json`: the same with each run's values and the mean, standard
//!   deviation, and 95th percentile of every metric;
//! - `convergence.csv` (with `convergence`): the image metrics per frame;
//! - `images/`: the reference and each run's first measured frame, as PNG
//!   (displayed) and PFM (linear HDR), and `flip/`: each run's ꟻLIP error map.
//!
//! `tools/experiments/report.py` turns the directory into tables, plots, and
//! an image grid.

use crate::experiment::{self, Experiment, Run};
use crate::gpu::{self, Gpu};
use crate::investigation;
use crate::loading::Loading;
use crate::metrics::{self, Image};
use crate::renderer::{Renderer, ViewKind};
use anyhow::{bail, Context, Result};
use serde_json::{json, Map, Value as Json};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

pub struct ResearchOptions {
    /// An experiment file, or a package's resource by full key.
    pub experiment: String,
    pub out: PathBuf,
    /// List the runs and stop.
    pub dry_run: bool,
    /// The lab's root (its checkout), for the manifest's commit.
    pub root: PathBuf,
}

/// One measured frame of a run.
struct Sample {
    metrics: Map<String, Json>,
    passes: Vec<(String, f64)>,
}

/// What a run's frames measured.
struct Measured {
    samples: Vec<Sample>,
    /// The first measured frame, displayed and linear.
    image: Option<(Image, Image)>,
    flip_map: Option<Vec<f32>>,
    /// Per frame (with `convergence`): its image metrics.
    convergence: Vec<Map<String, Json>>,
}

fn unix_seconds() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// `git` in the lab's root: its output, or None.
fn git(root: &Path, args: &[&str]) -> Option<String> {
    let out = std::process::Command::new("git").arg("-C").arg(root).args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// A linear image as a Portable Float Map (rows from the bottom, little-endian).
fn write_pfm(path: &Path, image: &Image) -> Result<()> {
    let mut bytes = format!("PF\n{} {}\n-1.0\n", image.width, image.height).into_bytes();
    for y in (0..image.height).rev() {
        for p in &image.pixels[y * image.width..(y + 1) * image.width] {
            for c in p {
                bytes.extend_from_slice(&c.to_le_bytes());
            }
        }
    }
    std::fs::write(path, bytes).with_context(|| format!("write {}", path.display()))
}

fn write_png(path: &Path, image: &Image) -> Result<()> {
    crate::readback::write_png(path, image.width as u32, image.height as u32, &image.to_rgba8())
}

/// A run's name as a file name, after its index (`03-probeRayCount=8`).
fn slug(index: usize, run: &str) -> String {
    format!("{index:02}-{}", experiment::file_name(run))
}

fn number(x: f64) -> Json {
    serde_json::Number::from_f64(x).map_or(Json::Null, Json::Number)
}

/// The image metrics of `test` against `reference`, those `wanted` asks for.
fn image_metrics(reference: &(Image, Image), test: &(Image, Image), wanted: &[String], with_map: bool) -> (Map<String, Json>, Option<Vec<f32>>) {
    let has = |m: &str| wanted.iter().any(|w| w == m);
    let mut out = Map::new();
    let mse = metrics::mse(&reference.0, &test.0);
    if has("mse") {
        out.insert("mse".into(), number(mse));
    }
    if has("rmse") {
        out.insert("rmse".into(), number(mse.sqrt()));
    }
    if has("psnr") {
        // A JSON number cannot be infinite: identical images record 999.
        out.insert("psnr".into(), number(metrics::psnr(mse).min(999.0)));
    }
    if has("ssim") {
        out.insert("ssim".into(), number(metrics::ssim(&reference.0, &test.0)));
    }
    let mut map = None;
    if has("flip") {
        let (mean, m) = metrics::flip(&reference.0, &test.0, metrics::DEFAULT_PPD);
        out.insert("flip".into(), number(mean));
        if with_map {
            map = Some(m);
        }
    }
    if has("relmse") {
        out.insert("relmse".into(), number(metrics::relmse(&reference.1, &test.1)));
    }
    (out, map)
}

/// Render `values` over the experiment's demonstration as one run, measuring
/// it against `reference` when given.
fn measure_run(renderer: &mut Renderer, gpu: &Gpu, e: &Experiment, values: &Map<String, Json>, frames: u32, reference: Option<&(Image, Image)>) -> Result<Measured> {
    let mut state = renderer.default_state(e.start_demo()?);
    e.apply(&mut state, values)?;
    renderer.reset_history();
    if let Some(t) = renderer.timing.as_mut() {
        t.clear();
    }
    renderer.prepare_scene_data(&state);
    let wants_images = e.metrics.iter().any(|m| !matches!(m.as_str(), "gpu_ms" | "cpu_ms"));
    let mut convergence = Vec::new();
    for _ in 0..frames {
        renderer.render_experiment(&state);
        if let (true, Some(r)) = (e.convergence, reference) {
            let view = renderer.read_view(ViewKind::Live).context("the live view")?;
            convergence.push(image_metrics(r, &view, &e.metrics, false).0);
        }
    }
    for _ in 0..e.warmup {
        renderer.render_experiment(&state);
    }
    let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
    // The timed frames back to back (a readback between them would let the
    // GPU's clocks drop); the image is read and measured after the last one.
    let mut samples = Vec::new();
    for _ in 0..e.repeats {
        let started = Instant::now();
        renderer.render_experiment(&state);
        let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
        let cpu_ms = started.elapsed().as_secs_f64() * 1000.0;
        let passes = match renderer.timing.as_mut() {
            Some(t) => {
                t.finish(&gpu.device);
                t.last().to_vec()
            }
            None => Vec::new(),
        };
        let mut m = Map::new();
        if e.metrics.iter().any(|x| x == "gpu_ms") && !passes.is_empty() {
            m.insert("gpu_ms".into(), number(passes.iter().map(|p| p.1).sum()));
        }
        if e.metrics.iter().any(|x| x == "cpu_ms") {
            m.insert("cpu_ms".into(), number(cpu_ms));
        }
        samples.push(Sample { metrics: m, passes });
    }
    let mut image = None;
    let mut flip_map = None;
    if wants_images {
        let view = renderer.read_view(ViewKind::Live).context("the live view")?;
        let last = samples.last_mut().expect("a measured frame");
        if e.metrics.iter().any(|x| x == "luminance") {
            last.metrics.insert("luminance".into(), number(metrics::mean_luminance(&view.1)));
        }
        if let Some(r) = reference {
            let (more, map) = image_metrics(r, &view, &e.metrics, true);
            last.metrics.extend(more);
            flip_map = map;
        }
        image = Some(view);
    }
    Ok(Measured { samples, image, flip_map, convergence })
}

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s.to_string() }
}

fn csv_value(j: Option<&Json>) -> String {
    match j {
        None | Some(Json::Null) => String::new(),
        Some(Json::String(s)) => csv_field(s),
        Some(other) => csv_field(&other.to_string()),
    }
}

pub fn run(options: &ResearchOptions) -> Result<()> {
    let source = match Path::new(&options.experiment).exists() {
        true => std::fs::read_to_string(&options.experiment).with_context(|| format!("read {}", options.experiment))?,
        false => crate::resources::find(&options.experiment).map(|r| r.text().to_string()).with_context(|| format!("no experiment file or resource `{}`", options.experiment))?,
    };
    let e = Experiment::parse(&source).with_context(|| options.experiment.clone())?;
    e.check()?;
    let runs = e.runs();
    if options.dry_run {
        println!("{} ({}): {} runs at {}×{}, {} frames + {} warm-up + {} measured each", e.name, investigation::current().path(), runs.len(), e.resolution[0], e.resolution[1], e.frames, e.warmup, e.repeats);
        if let Some(r) = &e.reference {
            println!("  reference: {} frames, {}", r.frames.unwrap_or(e.frames), Json::Object(r.values.clone()));
        }
        for (i, run) in runs.iter().enumerate() {
            println!("  {i:2} {}: {}", run.name, Json::Object(run.values.clone()));
        }
        return Ok(());
    }
    let images = options.out.join("images");
    std::fs::create_dir_all(&images).with_context(|| format!("create {}", images.display()))?;
    let wants_flip = e.metrics.iter().any(|m| m == "flip") && e.reference.is_some();
    if wants_flip {
        std::fs::create_dir_all(options.out.join("flip"))?;
    }
    let started = unix_seconds();
    let gpu = Gpu::new(gpu::instance(None), None)?;
    let demo = e.start_demo()?;
    let mut renderer = Loading::start(investigation::current(), gpu.clone(), investigation::current().demo_scene(demo), e.resolution).wait()?;
    let timestamps = renderer.set_timing(e.metrics.iter().any(|m| m == "gpu_ms"));
    if e.metrics.iter().any(|m| m == "gpu_ms") && !timestamps {
        eprintln!("this GPU has no timestamp queries: gpu_ms is left out");
    }

    let reference = match e.reference_values() {
        Some(values) => {
            let frames = e.reference.as_ref().and_then(|r| r.frames).unwrap_or(e.frames);
            eprintln!("reference: {frames} frames");
            let mut reference_options = e.clone();
            reference_options.repeats = 1;
            reference_options.convergence = false;
            reference_options.metrics = vec!["luminance".into()];
            let measured = measure_run(&mut renderer, &gpu, &reference_options, &values, frames, None)?;
            let image = measured.image.context("the reference's image")?;
            write_png(&images.join("reference.png"), &image.0)?;
            write_pfm(&images.join("reference.pfm"), &image.1)?;
            Some(image)
        }
        None => None,
    };

    let mut rows: Vec<(usize, &Run, Measured)> = Vec::new();
    for (i, run) in runs.iter().enumerate() {
        let t = Instant::now();
        let measured = measure_run(&mut renderer, &gpu, &e, &run.values, e.frames, reference.as_ref())?;
        let name = slug(i, &run.name);
        if let Some((display, linear)) = &measured.image {
            write_png(&images.join(format!("{name}.png")), display)?;
            write_pfm(&images.join(format!("{name}.pfm")), linear)?;
        }
        if let (Some(map), Some((display, _))) = (&measured.flip_map, &measured.image) {
            write_png(&options.out.join("flip").join(format!("{name}.png")), &Image::new(display.width, display.height, metrics::heat(map)))?;
        }
        let summary: Vec<String> = ["psnr", "flip", "gpu_ms"].iter().filter_map(|m| {
            let xs: Vec<f64> = measured.samples.iter().filter_map(|s| s.metrics.get(*m).and_then(Json::as_f64)).collect();
            (!xs.is_empty()).then(|| format!("{m} {:.4}", metrics::summary(&xs).0))
        }).collect();
        eprintln!("[{}/{}] {} ({:.1} s){}", i + 1, runs.len(), run.name, t.elapsed().as_secs_f64(), if summary.is_empty() { String::new() } else { format!(": {}", summary.join(", ")) });
        rows.push((i, run, measured));
    }

    // results.csv: the varied keys, the metrics, then each pass.
    let mut varied: Vec<String> = Vec::new();
    for run in &runs {
        for (k, _) in &run.varied {
            if !varied.contains(k) {
                varied.push(k.clone());
            }
        }
    }
    let metric_columns: Vec<&str> = experiment::METRICS.iter().copied().filter(|m| rows.iter().any(|r| r.2.samples.iter().any(|s| s.metrics.contains_key(*m)))).collect();
    let mut passes: Vec<String> = Vec::new();
    for (_, _, m) in &rows {
        for s in &m.samples {
            for (p, _) in &s.passes {
                if !passes.contains(p) {
                    passes.push(p.clone());
                }
            }
        }
    }
    let mut csv = String::new();
    let header: Vec<String> = ["run", "repeat"].iter().map(|s| s.to_string()).chain(varied.iter().cloned()).chain(metric_columns.iter().map(|s| s.to_string())).chain(passes.iter().map(|p| format!("gpu:{p}"))).map(|h| csv_field(&h)).collect();
    csv.push_str(&header.join(","));
    csv.push('\n');
    for (_, run, m) in &rows {
        for (r, s) in m.samples.iter().enumerate() {
            let mut fields = vec![csv_field(&run.name), r.to_string()];
            fields.extend(varied.iter().map(|k| csv_value(run.values.get(k))));
            fields.extend(metric_columns.iter().map(|c| csv_value(s.metrics.get(*c))));
            fields.extend(passes.iter().map(|p| s.passes.iter().find(|(n, _)| n == p).map_or(String::new(), |(_, ms)| format!("{ms:.6}"))));
            csv.push_str(&fields.join(","));
            csv.push('\n');
        }
    }
    std::fs::write(options.out.join("results.csv"), csv)?;

    if e.convergence {
        let mut conv = String::from("run,frame");
        for c in metric_columns.iter().filter(|c| !matches!(**c, "gpu_ms" | "cpu_ms" | "luminance")) {
            conv.push_str(&format!(",{c}"));
        }
        conv.push('\n');
        for (_, run, m) in &rows {
            for (f, values) in m.convergence.iter().enumerate() {
                let mut fields = vec![csv_field(&run.name), (f + 1).to_string()];
                fields.extend(metric_columns.iter().filter(|c| !matches!(**c, "gpu_ms" | "cpu_ms" | "luminance")).map(|c| csv_value(values.get(*c))));
                conv.push_str(&fields.join(","));
                conv.push('\n');
            }
        }
        std::fs::write(options.out.join("convergence.csv"), conv)?;
    }

    let results: Vec<Json> = rows
        .iter()
        .map(|(i, run, m)| {
            let mut summary = Map::new();
            for c in &metric_columns {
                let xs: Vec<f64> = m.samples.iter().filter_map(|s| s.metrics.get(*c).and_then(Json::as_f64)).collect();
                if !xs.is_empty() {
                    let (mean, sd, p95) = metrics::summary(&xs);
                    summary.insert(c.to_string(), json!({"mean": number(mean), "sd": number(sd), "p95": number(p95)}));
                }
            }
            for p in &passes {
                let xs: Vec<f64> = m.samples.iter().filter_map(|s| s.passes.iter().find(|(n, _)| n == p).map(|x| x.1)).collect();
                if !xs.is_empty() {
                    let (mean, sd, p95) = metrics::summary(&xs);
                    summary.insert(format!("gpu:{p}"), json!({"mean": number(mean), "sd": number(sd), "p95": number(p95)}));
                }
            }
            let name = slug(*i, &run.name);
            json!({
                "name": run.name,
                "image": m.image.as_ref().map(|_| format!("images/{name}.png")),
                "flip": m.flip_map.as_ref().map(|_| format!("flip/{name}.png")),
                "varied": Json::Object(run.varied.iter().cloned().collect()),
                "values": Json::Object(run.values.clone()),
                "samples": m.samples.iter().map(|s| json!({"metrics": Json::Object(s.metrics.clone()), "passes": Json::Object(s.passes.iter().map(|(p, ms)| (p.clone(), number(*ms))).collect())})).collect::<Vec<_>>(),
                "summary": Json::Object(summary),
            })
        })
        .collect();
    let results = json!({
        "experiment": e.name,
        "description": e.description,
        "investigation": investigation::current().path(),
        "reference": reference.as_ref().map(|_| "images/reference.png"),
        "varied": varied,
        "metrics": metric_columns,
        "passes": passes,
        "runs": results,
    });
    std::fs::write(options.out.join("results.json"), serde_json::to_string_pretty(&results)?)?;

    use sha2::Digest;
    let digest: String = sha2::Sha256::digest(source.as_bytes()).iter().map(|b| format!("{b:02x}")).collect();
    let manifest = json!({
        "experiment": options.experiment,
        "sha256": digest,
        "source": source,
        "investigation": investigation::current().path(),
        "package": investigation::current().package(),
        "lab": env!("CARGO_PKG_VERSION"),
        "commit": git(&options.root, &["rev-parse", "HEAD"]),
        "describe": git(&options.root, &["describe", "--always", "--dirty"]),
        "gpu": crate::renderer::gpu_summary(&gpu),
        "timestamps": timestamps,
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "resolution": e.resolution,
        "frames": e.frames,
        "warmup": e.warmup,
        "repeats": e.repeats,
        "runs": runs.len(),
        "command": std::env::args().collect::<Vec<_>>(),
        "started": started,
        "finished": unix_seconds(),
    });
    std::fs::write(options.out.join("manifest.json"), serde_json::to_string_pretty(&manifest)?)?;
    std::fs::write(options.out.join("experiment.toml"), e.to_toml())?;
    if rows.is_empty() {
        bail!("the experiment has no runs");
    }
    eprintln!("results: {} (report: python3 tools/experiments/report.py {})", options.out.display(), options.out.display());
    Ok(())
}
