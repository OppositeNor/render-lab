//! The shared shot state and the timeline that drives it.
//!
//! One `ShotState` decides everything a frame shows: the demonstration, the
//! camera, the selected record, the highlighted struct fields, the panel
//! mode, and every numeric emphasis parameter (overlay opacity, object
//! motion, roughness, ...). The recorder evaluates it from the timeline by
//! frame index; the viewer edits it interactively. Both render it the same way.
//!
//! Which demonstrations and parameters exist is the current investigation's
//! choice (src/lab/investigation.rs); the lab only names a demonstration by
//! its key and reads parameters by name.

use crate::lab::camera::OrbitCamera;
use crate::lab::investigation;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;

/// A demonstration of the current investigation, by its key (`"cards"`).
/// Investigations define their own demonstrations and hand out these ids.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DemoId(&'static str);

impl DemoId {
    pub const fn new(key: &'static str) -> DemoId {
        DemoId(key)
    }

    pub fn key(self) -> &'static str {
        self.0
    }

    /// The demonstration's title in the current locale.
    pub fn title(self) -> &'static str {
        investigation::current().demo_title(self)
    }

    /// The current investigation's demonstration with this key.
    pub fn find(key: &str) -> Option<DemoId> {
        investigation::current().demos().iter().copied().find(|d| d.key() == key)
    }
}

impl Serialize for DemoId {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(self.0)
    }
}

impl<'de> Deserialize<'de> for DemoId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<DemoId, D::Error> {
        let key = String::deserialize(deserializer)?;
        DemoId::find(&key).ok_or_else(|| serde::de::Error::custom(format!("unknown demonstration `{key}` for {}", investigation::current().title())))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum RecordRef {
    None,
    Object { index: u32 },
    Voxel { index: u32 },
    Brick { index: u32 },
    Card { index: u32 },
    Page { index: u32 },
    Ray,
    Camera,
    Probe { index: u32 },
    Radiance { index: u32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PanelMode {
    Declaration,
    Values,
    Memory,
    /// A Python pseudo-code listing (`ShotState::code`) in a wider panel.
    Code,
    Hidden,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ShotState {
    pub demo: DemoId,
    pub camera: OrbitCamera,
    pub record: RecordRef,
    pub highlight: Vec<String>,
    pub panel: PanelMode,
    pub params: BTreeMap<String, f32>,
    pub callout: Option<String>,
    /// The pseudo-code listing (src/lab/pseudocode.rs) shown in PanelMode::Code;
    /// `highlight` then names the tags of the lit lines.
    pub code: Option<String>,
}

impl ShotState {
    /// The current investigation's starting state for a demonstration.
    pub fn for_demo(demo: impl Into<DemoId>) -> ShotState {
        investigation::current().shot_state(demo.into())
    }

    pub fn p(&self, name: &str) -> f32 {
        *self.params.get(name).unwrap_or_else(|| panic!("unknown shot parameter {name}"))
    }

    pub fn set(&mut self, name: &str, value: f32) {
        assert!(investigation::current().param_default(name).is_some(), "unknown shot parameter {name}");
        self.params.insert(name.to_string(), value);
    }
}

/// The fields a newly selected record highlights in the struct panel.
pub fn default_highlight(record: RecordRef) -> Vec<String> {
    let fields: &[&str] = match record {
        RecordRef::Object { .. } => &["localToWorld", "worldExtent"],
        RecordRef::Voxel { .. } => &["signedDistance", "encoded"],
        RecordRef::Brick { .. } => &["virtualBrick", "physicalBrick"],
        RecordRef::Card { .. } => &["tangentFrame", "pageIndex"],
        RecordRef::Page { .. } => &["physicalPage", "cardUVRect"],
        RecordRef::Ray => &["incidentDir", "normal", "reflectedDir"],
        RecordRef::Probe { .. } => &["position", "framesAccumulated"],
        _ => &[],
    };
    fields.iter().map(|s| s.to_string()).collect()
}

/// A slow sway of the orbit camera, so that held shots show the scene's
/// depth through parallax: a few degrees of yaw and pitch and a small change
/// of distance, on periods that do not line up. It is the change of the sway
/// between two times (seconds), scaled by `amount`. The video sways from
/// time 0; the viewer sways from where Play animation started.
pub fn camera_drift(camera: OrbitCamera, from: f32, to: f32, amount: f32) -> OrbitCamera {
    if amount == 0.0 || from == to {
        return camera;
    }
    let sway = |t: f32| {
        let wave = |period: f32| (std::f32::consts::TAU * t / period).sin();
        [3.0 * wave(17.0) + 1.0 * wave(7.3), 1.5 * wave(11.0), 0.03 * wave(13.0)]
    };
    let ([yaw0, pitch0, dist0], [yaw1, pitch1, dist1]) = (sway(from), sway(to));
    OrbitCamera {
        yaw: camera.yaw + amount * (yaw1 - yaw0),
        pitch: camera.pitch + amount * (pitch1 - pitch0),
        distance: camera.distance * (1.0 + amount * dist1) / (1.0 + amount * dist0),
        ..camera
    }
}

// ---------------------------------------------------------------------------
// Timeline produced by scripts/build_enhanced.py from the storyboard and
// measured narration. All times are absolute frame indices.

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Timeline {
    pub fps: u32,
    pub width: u32,
    pub height: u32,
    pub total_frames: u64,
    pub title: String,
    pub engine_label: String,
    /// Language of the captions and on-screen text.
    #[serde(default)]
    pub locale: crate::lab::i18n::Locale,
    pub chapters: Vec<TimelineChapter>,
    pub beats: Vec<TimelineBeat>,
    pub keys: Vec<Key>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)] // endFrame is part of the timeline schema; frames are looked up by start.
pub struct TimelineChapter {
    pub number: u32,
    pub title: String,
    pub start_frame: u64,
    pub end_frame: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BeatKind {
    Scene,
    Mermaid,
    Sources,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineBeat {
    pub id: String,
    pub chapter: u32,
    pub kind: BeatKind,
    pub start_frame: u64,
    pub frames: u64,
    pub headline: String,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub sources: Vec<String>,
    #[serde(default)]
    pub captions: Vec<Caption>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Caption {
    pub start: u64,
    pub end: u64,
    pub text: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CameraKey {
    pub target: Option<[f32; 3]>,
    pub yaw: Option<f32>,
    pub pitch: Option<f32>,
    pub distance: Option<f32>,
    pub fov: Option<f32>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Key {
    pub frame: u64,
    #[serde(default)]
    pub blend: u64,
    #[serde(default)]
    pub demo: Option<DemoId>,
    #[serde(default)]
    pub camera: Option<CameraKey>,
    #[serde(default)]
    pub record: Option<RecordRef>,
    #[serde(default)]
    pub highlight: Option<Vec<String>>,
    #[serde(default)]
    pub panel: Option<PanelMode>,
    #[serde(default)]
    pub params: BTreeMap<String, f32>,
    /// `""` clears the callout.
    #[serde(default)]
    pub callout: Option<String>,
    /// Pseudo-code listing for PanelMode::Code; `""` clears it.
    #[serde(default)]
    pub code: Option<String>,
    /// Provenance for error messages (storyboard beat id).
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Clone, Copy, Debug)]
struct Segment {
    from: u64,
    blend: u64,
    start: f32,
    target: f32,
}

impl Segment {
    fn value(&self, frame: u64) -> f32 {
        if self.blend == 0 || frame >= self.from + self.blend {
            return self.target;
        }
        if frame <= self.from {
            return self.start;
        }
        let x = (frame - self.from) as f32 / self.blend as f32;
        // Smootherstep: zero velocity and acceleration at both ends.
        let s = x * x * x * (x * (x * 6.0 - 15.0) + 10.0);
        self.start + (self.target - self.start) * s
    }
}

const CAMERA_CHANNELS: [&str; 7] = ["cam.tx", "cam.ty", "cam.tz", "cam.yaw", "cam.pitch", "cam.distance", "cam.fov"];

fn camera_channels(c: &OrbitCamera) -> [f32; 7] {
    [c.target[0], c.target[1], c.target[2], c.yaw, c.pitch, c.distance, c.fov]
}

impl Timeline {
    pub fn load(path: &std::path::Path) -> Result<Timeline> {
        let text = std::fs::read_to_string(path).with_context(|| format!("read timeline {}", path.display()))?;
        let mut timeline: Timeline = serde_json::from_str(&text).with_context(|| format!("parse timeline {}", path.display()))?;
        timeline.keys.sort_by_key(|k| k.frame);
        timeline.validate()?;
        Ok(timeline)
    }

    pub fn validate(&self) -> Result<()> {
        for key in &self.keys {
            for name in key.params.keys() {
                if investigation::current().param_default(name).is_none() {
                    bail!("unknown shot parameter `{name}` in key at frame {} ({:?})", key.frame, key.source);
                }
            }
            if let Some(code) = key.code.as_deref().filter(|c| !c.is_empty()) {
                if investigation::current().listing(code).is_none() {
                    bail!("unknown pseudo-code listing `{code}` in key at frame {} ({:?})", key.frame, key.source);
                }
            }
        }
        let mut frame = 0;
        for beat in &self.beats {
            if beat.start_frame != frame {
                bail!("beat {} starts at {} but the previous beat ended at {frame}", beat.id, beat.start_frame);
            }
            frame += beat.frames;
        }
        if frame != self.total_frames {
            bail!("beats cover {frame} frames, timeline declares {}", self.total_frames);
        }
        Ok(())
    }

    pub fn beat_at(&self, frame: u64) -> (usize, &TimelineBeat) {
        let i = self.beats.partition_point(|b| b.start_frame + b.frames <= frame).min(self.beats.len() - 1);
        (i, &self.beats[i])
    }

    pub fn chapter_at(&self, frame: u64) -> &TimelineChapter {
        self.chapters.iter().rev().find(|c| c.start_frame <= frame).unwrap_or(&self.chapters[0])
    }

    /// Evaluate the shot state at a frame. Pure function of (timeline, frame).
    pub fn state_at(&self, frame: u64) -> ShotState {
        // Before the first key, the investigation's first demonstration.
        let mut state = ShotState::for_demo(investigation::current().demos()[0]);
        let mut segments: BTreeMap<String, Segment> = BTreeMap::new();
        let reset = |segments: &mut BTreeMap<String, Segment>, state: &ShotState, at: u64| {
            for (name, value) in &state.params {
                segments.insert(name.clone(), Segment { from: at, blend: 0, start: *value, target: *value });
            }
            for (name, value) in CAMERA_CHANNELS.iter().zip(camera_channels(&state.camera)) {
                segments.insert(name.to_string(), Segment { from: at, blend: 0, start: value, target: value });
            }
        };
        reset(&mut segments, &state, 0);
        for key in self.keys.iter().take_while(|k| k.frame <= frame) {
            if let Some(demo) = key.demo {
                let callout = state.callout.clone();
                let camera_now: Vec<f32> = CAMERA_CHANNELS.iter().map(|c| segments[*c].value(key.frame)).collect();
                state = ShotState::for_demo(demo);
                state.callout = callout;
                reset(&mut segments, &state, key.frame);
                // A demo switch cuts parameters but keeps the camera continuous
                // when the key itself moves the camera.
                if key.camera.is_some() {
                    for (i, c) in CAMERA_CHANNELS.iter().enumerate() {
                        segments.insert(c.to_string(), Segment { from: key.frame, blend: 0, start: camera_now[i], target: camera_now[i] });
                    }
                }
            }
            if let Some(record) = key.record {
                state.record = record;
            }
            if let Some(highlight) = &key.highlight {
                state.highlight = highlight.clone();
            }
            if let Some(panel) = key.panel {
                state.panel = panel;
            }
            if let Some(callout) = &key.callout {
                state.callout = if callout.is_empty() { None } else { Some(callout.clone()) };
            }
            if let Some(code) = &key.code {
                state.code = if code.is_empty() { None } else { Some(code.clone()) };
            }
            let mut targets: Vec<(String, f32)> = key.params.iter().map(|(k, v)| (k.clone(), *v)).collect();
            if let Some(c) = &key.camera {
                let names = CAMERA_CHANNELS;
                if let Some(t) = c.target {
                    targets.extend([(names[0].into(), t[0]), (names[1].into(), t[1]), (names[2].into(), t[2])]);
                }
                for (i, v) in [(3, c.yaw), (4, c.pitch), (5, c.distance), (6, c.fov)] {
                    if let Some(v) = v {
                        targets.push((names[i].into(), v));
                    }
                }
            }
            for (name, target) in targets {
                let start = segments.get(&name).map(|s| s.value(key.frame)).unwrap_or(target);
                segments.insert(name, Segment { from: key.frame, blend: key.blend, start, target });
            }
        }
        let value = |name: &str| segments[name].value(frame);
        let keyed = OrbitCamera {
            target: [value("cam.tx"), value("cam.ty"), value("cam.tz")],
            yaw: value("cam.yaw"),
            pitch: value("cam.pitch"),
            distance: value("cam.distance"),
            fov: value("cam.fov"),
        };
        state.camera = camera_drift(keyed, 0.0, frame as f32 / self.fps as f32, segments.get("cameraDrift").map_or(0.0, |s| s.value(frame)));
        let names: Vec<String> = state.params.keys().cloned().collect();
        for name in names {
            state.params.insert(name.clone(), value(&name));
        }
        state
    }
}
