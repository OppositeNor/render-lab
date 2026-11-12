//! What the demonstrations draw from P to the lights, checked against the room:
//! every frame of a story's scene beats is rendered alone (so its overlay is built from its own
//! read-backs, lab/src/fetch.rs), and each line of that overlay that ends on a light traced
//! against the occluders (cpu.rs `Triangle`, the tracer the passes are checked
//! with). A line drawn as reaching its light must reach it; one drawn as
//! blocked (`demos::BLOCKED`) must be blocked.

use super::super::cpu::{self, Triangle};
use super::super::demos;
use super::super::lights::{self, Light};
use super::super::shot::Demo;
use anyhow::{Result, anyhow};
use glam::Vec3;
use lab::frame::{FrameContent, SegmentInstance, Statics};
use lab::shot::{ShotState, Timeline};
use lab::utility::selftest::{Report, request_at};
use serde_json::{Value, json};

/// The frames between two samples of a beat: half a second at the timeline's 30 fps.
const STEP: u64 = 15;

/// The demonstrations that draw lines from P to the lights.
const DRAWING: [Demo; 3] = [Demo::Ris, Demo::Spatial, Demo::Reuse];

/// One line from a surface to a point on a light, as drawn and as the room has it.
struct Sight {
    from: Vec3,
    to: Vec3,
    light: usize,
    drawn_blocked: bool,
    /// The occluder the line starts on, if any: the lab's shadow ray skips it (cpu.rs `visible`, the GPU's too).
    on: Option<u32>,
    /// The occluder the ray meets first, its own surface's skipped, if any.
    blocker: Option<u32>,
}

/// The renderer and the scene the frames are rendered and traced in, at one size.
struct Stage {
    renderer: lab::renderer::Renderer,
    statics: Statics,
}

impl Stage {
    fn new(size: [u32; 2]) -> Result<Stage> {
        let room = || -> Result<Statics> {
            Statics::prepare(
                lab::scene::SceneId::from_resource("paper_restir.scene/restir-room")
                    .ok_or_else(|| anyhow!("no ReSTIR room"))?,
            )
        };
        let gpu = lab::gpu::Gpu::new(lab::gpu::instance(None), None)?;
        let renderer = lab::renderer::Renderer::new(gpu, room()?, size[0], size[1])?;
        Ok(Stage {
            renderer,
            statics: room()?,
        })
    }

    /// The lines `state`'s demonstration draws to the lights at `frame` of `timeline`, each traced.
    fn sights(&mut self, state: &ShotState, timeline: &Timeline, frame: u64) -> Vec<Sight> {
        // A frame drawn alone is settled: its overlay is built from its own read-backs (lab/src/fetch.rs).
        let Some(content) = self
            .renderer
            .render_frame(request_at(state, timeline, frame), None)
            .content
        else {
            return Vec::new();
        };
        let placed = lights::place(state);
        let objects = demos::objects_for(state, &self.statics);
        let triangles = cpu::occluders(&self.statics.meshes, &objects);
        traced(&content, &placed, &triangles)
    }
}

/// The segments of `content` that end on one of `placed`, each traced against `triangles`.
fn traced(content: &FrameContent, placed: &[Light], triangles: &[Triangle]) -> Vec<Sight> {
    let blocked = lab::palette::to_f32(demos::BLOCKED, 1.0);
    let mut out = Vec::new();
    for s in &content.draw.segments {
        // A segment ends on a light when its end lies on the light's sphere.
        let (from, to) = (Vec3::from(s.p0), Vec3::from(s.p1));
        let Some(light) = placed
            .iter()
            .position(|l| ((to - l.center).length() - l.radius).abs() <= 1e-3 * l.radius.max(1.0))
        else {
            continue;
        };
        let on = object_at(from, triangles);
        out.push(Sight {
            from,
            to,
            light,
            drawn_blocked: same_colour(s, blocked),
            on,
            blocker: first_blocker(from, to, on, triangles),
        });
    }
    out
}

fn same_colour(s: &SegmentInstance, rgba: [f32; 4]) -> bool {
    (0..3).all(|i| (s.color[i] - rgba[i]).abs() < 0.02)
}

/// The occluder whose surface `p` lies on (within a few millimetres of one of its triangles), if any.
fn object_at(p: Vec3, triangles: &[Triangle]) -> Option<u32> {
    triangles
        .iter()
        .find(|t| on_triangle(p, t))
        .map(|t| t.object)
}

fn on_triangle(p: Vec3, t: &Triangle) -> bool {
    let (e1, e2) = (t.p[1] - t.p[0], t.p[2] - t.p[0]);
    let n = e1.cross(e2);
    let area = n.length();
    if area < 1e-12 || (p - t.p[0]).dot(n / area).abs() > 1e-2 {
        return false;
    }
    // Inside when the three sub-triangles' areas add up to the whole.
    let parts = [(t.p[0], t.p[1]), (t.p[1], t.p[2]), (t.p[2], t.p[0])]
        .iter()
        .map(|(a, b)| (*a - p).cross(*b - p).length())
        .sum::<f32>();
    parts <= area * 1.02
}

/// The occluder a ray from `from` to `to` meets first, skipping `own`, the one it starts on.
fn first_blocker(from: Vec3, to: Vec3, own: Option<u32>, triangles: &[Triangle]) -> Option<u32> {
    let d = to - from;
    let dist = d.length();
    let dir = d / dist.max(1e-6);
    let origin = from + dir * 1e-3;
    triangles
        .iter()
        .filter(|t| Some(t.object) != own)
        .find(|t| t.hit(origin, dir, dist * 0.999 - 1e-3))
        .map(|t| t.object)
}

/// The lines of `sights` that start on an occluder, by its index: where P (or a neighbour) is off the floor and walls.
fn starts(sights: &[Sight], counts: &mut std::collections::BTreeMap<String, usize>, beat: &str) {
    for s in sights {
        if let Some(o) = s.on {
            *counts.entry(format!("{beat}: object {o}")).or_default() += 1;
        }
    }
}

/// Where a frame's lines disagree with the room: each line drawn clear that is blocked, or drawn blocked that is clear.
fn problems(sights: &[Sight], at: Value) -> Vec<Value> {
    sights
        .iter()
        .filter(|s| s.drawn_blocked != s.blocker.is_some())
        .map(|s| {
            json!({"at": at, "light": s.light, "from": s.from.to_array(), "to": s.to.to_array(),
                "drawn": if s.drawn_blocked { "blocked" } else { "clear" }, "blocker": s.blocker})
        })
        .collect()
}

/// With a timeline, each of its scene beats whose demonstration draws to the lights, sampled every STEP frames.
pub fn check(report: &mut Report, timeline_path: Option<&std::path::Path>) -> Result<()> {
    let Some(path) = timeline_path else {
        return Ok(());
    };
    let timeline = Timeline::load(path)?;
    // P is the pixel at pickU, pickV, so its candidates are those of the timeline's own size.
    let mut stage = Stage::new([timeline.width, timeline.height])?;
    let (mut found, mut lines, mut frames) = (Vec::new(), 0, 0);
    let mut on = std::collections::BTreeMap::new();
    for beat in timeline
        .beats
        .iter()
        .filter(|b| b.kind == lab::shot::SceneType::ThreeD)
    {
        let mut frame = beat.start_frame;
        while frame < beat.start_frame + beat.frames {
            // Each sample of a beat whose demonstration draws to the lights.
            let state = timeline.state_at(frame);
            if DRAWING.contains(&Demo::of(state.demo)) {
                let sights = stage.sights(&state, &timeline, frame);
                frames += 1;
                lines += sights.len();
                starts(&sights, &mut on, &beat.id);
                let at = json!({"beat": beat.id, "frame": frame, "second": frame as f32 / timeline.fps as f32});
                found.extend(problems(&sights, at));
            }
            frame += STEP;
        }
    }
    report.check(
        "every line a demonstration draws from P to a light in the timeline's scene beats is clear where it is drawn clear, and blocked where it is drawn blocked",
        found.is_empty() && lines > 0,
        json!({"timeline": path, "frames": frames, "lines": lines, "startOnOccluders": on, "problems": found}),
    );
    Ok(())
}
