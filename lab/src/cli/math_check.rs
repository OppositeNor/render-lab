//! `script math`: the motion check of the investigation's math scenes (src/math/check.rs), each played as the
//! viewer plays it: a demonstration's Play clip sampled 60 times a second, or every frame of a story's math beats.

use crate::clip::{self, Track};
use crate::frame::Statics;
use crate::math::check::{self, Finding, Limits, Sample};
use crate::scene::SceneId;
use crate::shot::ShotState;
use std::collections::HashMap;
use std::path::Path;

/// Samples a second of a demonstration's Play clip.
const RATE: f64 = 60.0;

/// Prepares each scene the scenes are called with once.
struct Prepared(HashMap<SceneId, Statics>);

impl Prepared {
    fn get(&mut self, id: SceneId) -> Result<&Statics, String> {
        if let std::collections::hash_map::Entry::Vacant(e) = self.0.entry(id) {
            e.insert(Statics::prepare(id).map_err(|e| format!("{e:#}"))?);
        }
        Ok(&self.0[&id])
    }
}

/// The scene `name` sampled at `at` (in the run's units) for `state` at `time` seconds.
fn sample(name: &str, state: &ShotState, statics: &Statics, at: f64, time: f32) -> Sample {
    match crate::script::runtime::math_probe(name, state, statics, time) {
        Ok((probes, frame)) => Sample {
            at,
            frame,
            probes: Ok(probes),
        },
        Err(e) => Sample {
            at,
            frame: (glam::Vec2::ZERO, crate::math::FRAME_HEIGHT),
            probes: Err(e),
        },
    }
}

/// One checked run: what was played, how its times read, and what it found.
struct Run {
    title: String,
    /// Frames a second when its times are a story's frames, else seconds.
    fps: Option<f64>,
    findings: Vec<Finding>,
}

/// The spans of a clip in which a progress channel's keyframes move it: from each keyframe to the next that
/// differs, the channel named.
fn progress_spans(clip: &clip::Clip, state: &ShotState) -> Vec<(String, f64, f64)> {
    let mut spans = Vec::new();
    for track in &clip.tracks {
        let Track::Keys { channel, keys } = track else {
            continue;
        };
        if !channel.starts_with(clip::PROGRESS) {
            continue;
        }
        let mut from = (0.0, state.try_get(channel).map_or(0.0, |v| v.f32()));
        for k in keys {
            // Each keyframe that changes the value ends a span from the one before.
            if (k.value - from.1).abs() > 1e-4 && k.at > from.0 {
                spans.push((channel.clone(), from.0 as f64, k.at as f64));
            }
            from = (k.at, k.value);
        }
    }
    spans
}

/// Each demonstration with a math scene, its Play clip sampled (its finished picture when it has none).
fn demos(
    only: Option<&str>,
    dump: Option<f64>,
    prepared: &mut Prepared,
    limits: &Limits,
) -> Result<Vec<Run>, String> {
    let Some(demos) = crate::investigation::current().demo_specs() else {
        return Ok(Vec::new());
    };
    let mut runs = Vec::new();
    for spec in &demos.specs {
        let Some(name) = spec.math.filter(|n| only.is_none_or(|o| o == *n)) else {
            continue;
        };
        let statics = prepared.get(spec.scene)?;
        let start = ShotState::for_demo(spec.id);
        let play = clip::demo_clips(spec.id).into_iter().next();
        let seconds = play.as_ref().map_or(0.0, |c| c.seconds() as f64);
        let times: Vec<f64> = match dump {
            Some(t) => vec![t],
            None => (0..=(seconds * RATE).round() as usize)
                .map(|i| i as f64 / RATE)
                .collect(),
        };
        let samples: Vec<Sample> = times
            .into_iter()
            .map(|t| {
                let mut state = start.clone();
                if let Some(c) = &play {
                    clip::apply(c, t as f32, &mut state);
                }
                sample(name, &state, statics, t, t as f32)
            })
            .collect();
        if dump.is_some() {
            print_probes(name, &samples[0]);
            continue;
        }
        let mut findings = check::check(&samples, limits);
        if let Some(c) = &play {
            // A span whose progress moves while nothing on the canvas does is a step that shows nothing.
            for (channel, from, to) in progress_spans(c, &start) {
                for mut f in check::dead_spans(&samples, &[(from, to)]) {
                    f.id = channel.trim_start_matches(clip::PROGRESS).to_string();
                    findings.push(f);
                }
            }
        }
        let clip_name = play
            .as_ref()
            .map_or("no Play clip".to_string(), |c| format!("clip {}", c.name));
        runs.push(Run {
            title: format!(
                "{name}: demonstration {}, {clip_name}, {seconds:.1} s",
                spec.id.key()
            ),
            fps: None,
            findings,
        });
    }
    Ok(runs)
}

/// Each math beat of the timeline at `path`, every frame.
fn beats(
    path: &Path,
    only: Option<&str>,
    dump: Option<f64>,
    prepared: &mut Prepared,
    limits: &Limits,
) -> Result<Vec<Run>, String> {
    let timeline =
        crate::shot::Timeline::load(path).map_err(|e| format!("{}: {e:#}", path.display()))?;
    // Its texts are measured in the timeline's language, as its recording shows them.
    crate::i18n::set(timeline.locale);
    let statics = prepared.get(crate::investigation::current().first_scene())?;
    let fps = timeline.fps.max(1) as f64;
    let mut runs = Vec::new();
    for beat in &timeline.beats {
        let Some(name) = beat
            .math
            .as_deref()
            .filter(|n| only.is_none_or(|o| o == *n))
        else {
            continue;
        };
        let frames = beat.start_frame..beat.start_frame + beat.frames;
        if let Some(f) = dump.map(|f| f as u64).filter(|f| frames.contains(f)) {
            let state = timeline.state_at(f);
            print_probes(
                name,
                &sample(
                    name,
                    &state,
                    statics,
                    f as f64 / fps,
                    (f as f64 / fps) as f32,
                ),
            );
            continue;
        }
        if dump.is_some() {
            continue;
        }
        let samples: Vec<Sample> = frames
            .clone()
            .map(|f| {
                let state = timeline.state_at(f);
                sample(
                    name,
                    &state,
                    statics,
                    f as f64 / fps,
                    (f as f64 / fps) as f32,
                )
            })
            .collect();
        runs.push(Run {
            title: format!(
                "{name}: beat {}, frames {}-{}",
                beat.id,
                frames.start,
                frames.end - 1
            ),
            fps: Some(fps),
            findings: check::check(&samples, limits),
        });
    }
    Ok(runs)
}

/// The findings of one rule over one span, of mobs of one kind changing alike, together (a grid laid out again
/// pops every cell at once).
fn grouped(findings: &[Finding]) -> Vec<Vec<&Finding>> {
    let shape = |f: &Finding| {
        let words: String = f
            .detail
            .chars()
            .filter(|c| !c.is_ascii_digit() && *c != '.')
            .collect();
        (
            f.rule,
            (f.from * 1e6).round() as i64,
            (f.to * 1e6).round() as i64,
            words,
        )
    };
    let mut groups: Vec<Vec<&Finding>> = Vec::new();
    for f in findings {
        match groups.iter_mut().find(|g| shape(g[0]) == shape(f)) {
            Some(g) => g.push(f),
            None => groups.push(vec![f]),
        }
    }
    groups
}

/// A time of a run as it reads: seconds of a clip, or a story's frame.
fn when(at: f64, fps: Option<f64>) -> String {
    match fps {
        Some(fps) => format!("frame {}", (at * fps).round() as u64),
        None => format!("{at:.2} s"),
    }
}

/// The probes of one sample, one a line: what the check sees there.
fn print_probes(name: &str, s: &Sample) {
    println!(
        "{name} at {:.3} (frame centre {:.2},{:.2}, height {:.2}):",
        s.at, s.frame.0.x, s.frame.0.y, s.frame.1
    );
    match &s.probes {
        Err(e) => println!("  error: {e}"),
        Ok(probes) => {
            for p in probes {
                println!(
                    "  {:<6} {:<16} [{:6.2},{:6.2}]..[{:6.2},{:6.2}] alpha {:.2} drawn {:.2}{}",
                    p.kind,
                    if p.id.is_empty() {
                        "(unnamed)"
                    } else {
                        &p.id
                    },
                    p.lo.x,
                    p.lo.y,
                    p.hi.x,
                    p.hi.y,
                    p.alpha,
                    p.coverage,
                    p.em.map_or(String::new(), |e| format!(" em {e:.2}"))
                );
            }
        }
    }
}

/// Runs the check and prints its findings by scene; fails when it finds any. With `dump` (a demonstration's
/// seconds, a story's frame), prints what each scene draws there instead.
pub fn run(only: Option<&str>, timeline: Option<&Path>, dump: Option<f64>) -> Result<(), String> {
    let limits = Limits {
        // A story plays in real time: a fade over fewer than six frames reads as a cut.
        min_fade: timeline.map(|_| 0.1),
        // Every theme a viewer may choose: the built-in one and the installed files'.
        themes: crate::theme::list()
            .0
            .iter()
            .map(|e| check::Palette::of(&e.id, &e.theme))
            .collect(),
        ..Limits::default()
    };
    let mut prepared = Prepared(HashMap::new());
    let runs = match timeline {
        Some(path) => beats(path, only, dump, &mut prepared, &limits)?,
        None => demos(only, dump, &mut prepared, &limits)?,
    };
    if dump.is_some() {
        return Ok(());
    }
    if runs.is_empty() {
        println!("no math scene to check");
        return Ok(());
    }
    let mut total = 0;
    for run in &runs {
        let found = match run.findings.len() {
            0 => "clean".to_string(),
            n => format!("{n} findings"),
        };
        println!("{}: {found}", run.title);
        for group in grouped(&run.findings) {
            let f = group[0];
            let span = if (f.to - f.from).abs() < 1e-9 {
                when(f.from, run.fps)
            } else {
                format!("{} to {}", when(f.from, run.fps), when(f.to, run.fps))
            };
            let ids: Vec<&str> = group
                .iter()
                .map(|f| f.id.as_str())
                .filter(|i| !i.is_empty())
                .collect();
            let who = match ids.len() {
                0 => String::new(),
                1..=4 => format!("{}: ", ids.join(", ")),
                n => format!("{}, and {} more: ", ids[..4].join(", "), n - 4),
            };
            println!("  {:<8} {span}  {who}{}", f.rule.name(), f.detail);
        }
        total += run.findings.len();
    }
    if total > 0 {
        return Err(format!("{total} findings in {} runs", runs.len()));
    }
    Ok(())
}
