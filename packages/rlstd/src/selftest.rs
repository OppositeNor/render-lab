//! What an investigation's selftest (`Investigation::selftest`) is built
//! from: its report of checks, frames of single states, and test inputs.

use super::sources::PackageIndex;
use crate::renderer::FrameRequest;
use crate::shot::{SceneType, ShotState, Timeline};
use crate::ui::Chrome;
use glam::Vec3;
use serde_json::{json, Value};
use std::path::Path;

/// The checks run so far, each printed as it is recorded (PASS or FAIL and its name).
#[derive(Default)]
pub struct Report {
    /// `{name, passed, detail}`, in the order they ran, for the JSON report.
    pub checks: Vec<Value>,
}

impl Report {
    pub fn new() -> Report {
        Report::default()
    }

    pub fn check(&mut self, name: &str, passed: bool, detail: Value) {
        eprintln!("{} {name}", if passed { "PASS" } else { "FAIL" });
        self.checks.push(json!({"name": name, "passed": passed, "detail": detail}));
    }

    /// Every check passed.
    pub fn passed(&self) -> bool {
        self.checks.iter().all(|c| c["passed"] == true)
    }

    /// Checks every line `index` cites against `engine`'s checkout, which is
    /// there when it holds `marker` (a file at its root); without it the check
    /// is recorded as skipped.
    pub fn cited_lines(&mut self, engine: &str, index: &PackageIndex, checkout: &Path, marker: &str) -> anyhow::Result<()> {
        let name = format!("every cited {engine} line is unchanged in the checkout");
        if !checkout.join(marker).is_file() {
            self.check(&format!("{name} (skipped: no checkout)"), true, json!({"checkout": checkout}));
            return Ok(());
        }
        let (cited, changed) = index.changed_lines(checkout)?;
        self.check(&name, changed.is_empty(), json!({"checkout": checkout, "linesCited": cited, "mismatches": changed}));
        Ok(())
    }
}

/// A one-beat timeline of `width` x `height`, to render single states.
pub fn still_timeline(width: u32, height: u32) -> Timeline {
    serde_json::from_value(json!({
        "fps": 30, "width": width, "height": height, "totalFrames": 1, "title": "selftest", "engineLabel": "",
        "chapters": [{"number": 1, "title": "Selftest", "startFrame": 0, "endFrame": 1}],
        "beats": [{"id": "still", "chapter": 1, "kind": "scene", "startFrame": 0, "frames": 1, "headline": "Selftest frame", "captions": []}],
        "keys": []
    }))
    .expect("the still timeline is valid")
}

/// A recorder's request for `state` as a scene beat of `timeline`, at its first frame.
pub fn request<'a>(state: &'a ShotState, timeline: &'a Timeline) -> FrameRequest<'a> {
    request_at(state, timeline, 0)
}

/// A recorder's request for `state` as a scene beat of `timeline`, at `frame`.
pub fn request_at<'a>(state: &'a ShotState, timeline: &'a Timeline, frame: u64) -> FrameRequest<'a> {
    FrameRequest {
        state,
        scene_type: SceneType::ThreeD,
        image: None,
        beat_sources: &[],
        notices: false,
        chapter_number: 1,
        fade: 0.0,
        time: 0.0,
        raw_input: egui::RawInput { time: Some(0.0), ..Default::default() },
        chrome: Chrome::Recording { timeline, frame },
        stamp: None,
        history: false,
    }
}

/// Deterministic pseudo-random numbers (xorshift) for test inputs.
pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }

    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next()
    }

    /// A direction, uniform over the sphere.
    pub fn unit(&mut self) -> Vec3 {
        loop {
            let v = Vec3::new(self.range(-1.0, 1.0), self.range(-1.0, 1.0), self.range(-1.0, 1.0));
            if v.length_squared() > 0.01 && v.length_squared() <= 1.0 {
                return v.normalize();
            }
        }
    }

    /// A direction in the hemisphere around `n`.
    pub fn above(&mut self, n: Vec3) -> Vec3 {
        let v = self.unit();
        if v.dot(n) < 0.05 { (v - n * (v.dot(n) - 0.05 - self.next())).normalize() } else { v }
    }
}
