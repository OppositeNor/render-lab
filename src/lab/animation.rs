//! How values move over time: the easing of a blend, and motions (a path
//! through waypoints, an orbit, a wave) that drive named channels. The
//! timeline (src/lab/shot.rs) runs a key's blend with an [`Ease`] and its
//! `animate` motions; an investigation's Play animation is a [`Clip`] of
//! looping motions; the camera's sway is a set of waves.
//!
//! A channel is a shot parameter by name, or the camera's `camera.x`,
//! `camera.y`, `camera.z` (its target), `camera.yaw`, `camera.pitch`,
//! `camera.distance`, or `camera.fov`. A motion that does not give its start
//! takes the channels' values when it starts, so it continues from wherever
//! they are.

use crate::lab::camera::OrbitCamera;
use serde::{Deserialize, Serialize};
use std::f32::consts::{PI, TAU};

/// The easing of a blend: how far from the start value to the target it is
/// at each fraction of the blend.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Ease {
    /// Smootherstep: zero velocity and acceleration at both ends.
    #[default]
    Smooth,
    /// Constant speed, so keys in a row join into one steady motion.
    Linear,
    /// Cubic: starts at rest and arrives at full speed.
    In,
    /// Cubic: leaves at full speed and comes to rest.
    Out,
    /// Cubic: starts and ends at rest, faster in the middle than `Smooth`.
    InOut,
    /// Half a cosine: starts and ends at rest; a ping-pong of it is a pure sine.
    Sine,
}

impl Ease {
    pub fn apply(self, x: f32) -> f32 {
        match self {
            Ease::Smooth => x * x * x * (x * (x * 6.0 - 15.0) + 10.0),
            Ease::Linear => x,
            Ease::In => x * x * x,
            Ease::Out => 1.0 - (1.0 - x).powi(3),
            Ease::InOut if x < 0.5 => 4.0 * x * x * x,
            Ease::InOut => 1.0 - 4.0 * (1.0 - x).powi(3),
            Ease::Sine => 0.5 - 0.5 * (PI * x).cos(),
        }
    }
}

/// What a motion does after its `seconds`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Repeat {
    /// Stops at its end.
    #[default]
    Once,
    /// Starts again from its start.
    Loop,
    /// Runs back to its start, then forward again.
    PingPong,
}

impl Repeat {
    /// The fraction of the motion done `t` seconds after it started.
    fn progress(self, t: f32, seconds: f32) -> f32 {
        if seconds <= 0.0 {
            return 1.0;
        }
        let x = t / seconds;
        match self {
            Repeat::Once => x.clamp(0.0, 1.0),
            Repeat::Loop => x.rem_euclid(1.0),
            Repeat::PingPong => 1.0 - (x.rem_euclid(2.0) - 1.0).abs(),
        }
    }
}

/// A motion of one or more channels.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Motion {
    Path(Path),
    Orbit(Orbit),
    Wave(Wave),
}

/// Through waypoints on a smooth curve (a Catmull-Rom spline timed by the
/// distance between waypoints), so the channels pass each waypoint without
/// stopping. The curve starts at the channels' values when it starts.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Path {
    pub params: Vec<String>,
    /// The waypoints after the start, one value per channel.
    pub through: Vec<Vec<f32>>,
    pub seconds: f32,
    /// The pace along the whole curve: `smooth` starts and ends at rest.
    #[serde(default)]
    pub ease: Ease,
    /// Back to the start at the end, so a `loop` goes round without a seam.
    #[serde(default)]
    pub closed: bool,
    #[serde(default)]
    pub repeat: Repeat,
}

/// A point turned about a centre: two channels turn in their plane (from the
/// first towards the second), three about `axis` (right-handed).
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Orbit {
    pub params: Vec<String>,
    pub center: Vec<f32>,
    /// The axis of a three-channel orbit; up (+y) by default.
    #[serde(default)]
    pub axis: Option<[f32; 3]>,
    /// The point it starts from; by default, the channels' values.
    #[serde(default)]
    pub start: Option<Vec<f32>>,
    pub degrees: f32,
    pub seconds: f32,
    #[serde(default)]
    pub ease: Ease,
    #[serde(default)]
    pub repeat: Repeat,
}

/// `center + amplitude · sin(360° · t / period + phase)`.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Wave {
    pub param: String,
    /// By default, the channel's value when the wave starts.
    #[serde(default)]
    pub center: Option<f32>,
    pub amplitude: f32,
    pub period: f32,
    /// Degrees; 90 makes it a cosine.
    #[serde(default)]
    pub phase: f32,
    /// How long it waves; it waves for ever without.
    #[serde(default)]
    pub seconds: Option<f32>,
    /// Seconds over which the amplitude grows at the start and shrinks at the
    /// end (of `seconds`), so a wave that stops ends on its centre.
    #[serde(default)]
    pub fade: f32,
}

impl Wave {
    pub fn new(param: &str, center: f32, amplitude: f32, period: f32) -> Wave {
        Wave { param: param.to_string(), center: Some(center), amplitude, period, phase: 0.0, seconds: None, fade: 0.0 }
    }

    /// The same wave a quarter period earlier: `center + amplitude · cos(...)`.
    pub fn cosine(self) -> Wave {
        Wave { phase: 90.0, ..self }
    }

    fn value(&self, t: f32, start: f32) -> f32 {
        let mut envelope = 1.0;
        if self.fade > 0.0 {
            envelope = (t / self.fade).min(1.0);
            if let Some(seconds) = self.seconds {
                envelope = envelope.min((seconds - t) / self.fade);
            }
            envelope = Ease::Smooth.apply(envelope.clamp(0.0, 1.0));
        }
        if self.seconds.is_some_and(|s| t >= s) {
            envelope = 0.0;
        }
        self.center.unwrap_or(start) + self.amplitude * envelope * (TAU * t / self.period + self.phase.to_radians()).sin()
    }
}

impl Path {
    fn sample(&self, t: f32, start: &[f32]) -> Vec<f32> {
        let mut points: Vec<&[f32]> = std::iter::once(start).chain(self.through.iter().map(|p| p.as_slice())).collect();
        if self.closed {
            points.push(start);
        }
        let distance = |a: &[f32], b: &[f32]| a.iter().zip(b).map(|(x, y)| (y - x) * (y - x)).sum::<f32>().sqrt();
        let spans: Vec<f32> = points.windows(2).map(|w| distance(w[0], w[1])).collect();
        let total: f32 = spans.iter().sum();
        if total == 0.0 {
            return start.to_vec();
        }
        // The velocity at each waypoint: the chords on either side, each
        // weighted by the other's length; at an open end, the chord itself.
        let n = points.len();
        let chord = |i: usize, c: usize| (points[i + 1][c] - points[i][c]) / spans[i].max(1e-6);
        let velocity = |i: usize, c: usize| {
            let (before, after) = match (i, self.closed) {
                (0, true) => (Some(spans.len() - 1), Some(0)),
                (_, true) if i == n - 1 => (Some(spans.len() - 1), Some(0)),
                (0, false) => (None, Some(0)),
                _ if i == n - 1 => (Some(i - 1), None),
                _ => (Some(i - 1), Some(i)),
            };
            match (before, after) {
                (Some(b), Some(a)) => (chord(b, c) * spans[a] + chord(a, c) * spans[b]) / (spans[a] + spans[b]).max(1e-6),
                (Some(b), None) => chord(b, c),
                (None, Some(a)) => chord(a, c),
                (None, None) => 0.0,
            }
        };
        let mut along = self.ease.apply(self.repeat.progress(t, self.seconds)) * total;
        let mut i = 0;
        while i + 1 < spans.len() && along > spans[i] {
            along -= spans[i];
            i += 1;
        }
        let h = spans[i];
        let s = if h > 0.0 { (along / h).clamp(0.0, 1.0) } else { 0.0 };
        let (s2, s3) = (s * s, s * s * s);
        (0..start.len())
            .map(|c| {
                let (p0, p1) = (points[i][c], points[i + 1][c]);
                (2.0 * s3 - 3.0 * s2 + 1.0) * p0 + (s3 - 2.0 * s2 + s) * h * velocity(i, c) + (-2.0 * s3 + 3.0 * s2) * p1 + (s3 - s2) * h * velocity(i + 1, c)
            })
            .collect()
    }
}

impl Orbit {
    fn sample(&self, t: f32, start: &[f32]) -> Vec<f32> {
        let p = self.start.as_deref().unwrap_or(start);
        let angle = (self.degrees * self.ease.apply(self.repeat.progress(t, self.seconds))).to_radians();
        let (sin, cos) = angle.sin_cos();
        let r: Vec<f32> = p.iter().zip(&self.center).map(|(p, c)| p - c).collect();
        let turned = if r.len() == 2 {
            vec![r[0] * cos - r[1] * sin, r[0] * sin + r[1] * cos]
        } else {
            // Rodrigues' rotation of r about the unit axis k.
            let k = glam::Vec3::from(self.axis.unwrap_or([0.0, 1.0, 0.0])).normalize();
            let v = glam::Vec3::new(r[0], r[1], r[2]);
            (v * cos + k.cross(v) * sin + k * k.dot(v) * (1.0 - cos)).to_array().to_vec()
        };
        turned.iter().zip(&self.center).map(|(r, c)| c + r).collect()
    }
}

impl Motion {
    /// The channels it drives, in the order of its values.
    pub fn params(&self) -> Vec<&str> {
        match self {
            Motion::Path(m) => m.params.iter().map(|s| s.as_str()).collect(),
            Motion::Orbit(m) => m.params.iter().map(|s| s.as_str()).collect(),
            Motion::Wave(m) => vec![m.param.as_str()],
        }
    }

    /// Why it cannot run, if its values do not fit its channels.
    pub fn problem(&self) -> Option<String> {
        let n = self.params().len();
        match self {
            Motion::Path(m) if m.through.is_empty() => Some("a path needs at least one waypoint".into()),
            Motion::Path(m) if m.through.iter().any(|p| p.len() != n) => Some(format!("each waypoint needs {n} values")),
            Motion::Orbit(m) if n != 2 && n != 3 => Some("an orbit turns two or three channels".into()),
            Motion::Orbit(m) if m.center.len() != n || m.start.as_ref().is_some_and(|s| s.len() != n) => Some(format!("the orbit's center and start need {n} values")),
            Motion::Orbit(m) if m.axis.is_some() && n != 3 => Some("only a three-channel orbit has an axis".into()),
            Motion::Wave(m) if m.period <= 0.0 => Some("a wave needs a positive period".into()),
            _ => None,
        }
    }

    /// Its values `t` seconds after it started from `start` (the channels'
    /// values then, in the order of `params`).
    pub fn sample(&self, t: f32, start: &[f32]) -> Vec<f32> {
        match self {
            Motion::Path(m) => m.sample(t, start),
            Motion::Orbit(m) => m.sample(t, start),
            Motion::Wave(m) => vec![m.value(t, start[0])],
        }
    }
}

/// Motions that run together from the same start: an investigation's Play
/// animation.
#[derive(Clone, Debug, Default)]
pub struct Clip(pub Vec<Motion>);

impl Clip {
    /// Every channel's value `t` seconds after the clip started; `start` gives
    /// a channel's value when it started.
    pub fn sample(&self, t: f32, start: impl Fn(&str) -> f32) -> Vec<(String, f32)> {
        let mut values = Vec::new();
        for motion in &self.0 {
            let params = motion.params();
            let from: Vec<f32> = params.iter().map(|p| start(p)).collect();
            values.extend(params.iter().map(|p| p.to_string()).zip(motion.sample(t, &from)));
        }
        values
    }
}

/// The camera's sway, as waves of yaw and pitch (degrees) and of the
/// distance (a fraction of it), on periods that do not line up.
fn sway() -> [Vec<Wave>; 3] {
    [
        vec![Wave::new("camera.yaw", 0.0, 3.0, 17.0), Wave::new("camera.yaw", 0.0, 1.0, 7.3)],
        vec![Wave::new("camera.pitch", 0.0, 1.5, 11.0)],
        vec![Wave::new("camera.distance", 0.0, 0.03, 13.0)],
    ]
}

/// A slow sway of the orbit camera, so that held shots show the scene's
/// depth through parallax: a few degrees of yaw and pitch and a small change
/// of distance. It is the change of the sway between two times (seconds),
/// scaled by `amount`. The video sways from time 0; the viewer sways from
/// where Play animation started.
pub fn camera_drift(camera: OrbitCamera, from: f32, to: f32, amount: f32) -> OrbitCamera {
    if amount == 0.0 || from == to {
        return camera;
    }
    let at = |t: f32| sway().map(|waves| waves.iter().map(|w| w.value(t, 0.0)).sum::<f32>());
    let ([yaw0, pitch0, dist0], [yaw1, pitch1, dist1]) = (at(from), at(to));
    OrbitCamera {
        yaw: camera.yaw + amount * (yaw1 - yaw0),
        pitch: camera.pitch + amount * (pitch1 - pitch0),
        distance: camera.distance * (1.0 + amount * dist1) / (1.0 + amount * dist0),
        ..camera
    }
}

/// The camera's move into the shot when the viewer opens on a demonstration or
/// a narrated video: it starts away from the shot's camera and eases into it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Intro {
    /// Swings round to the shot from one side.
    Swing,
    /// Comes down onto the shot from above.
    Crane,
    /// Moves in from further back, turning as it comes.
    PushIn,
}

/// The intro chosen last, so the next one is a different move (3: none yet).
static LAST_INTRO: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(3);

impl Intro {
    pub const ALL: [Intro; 3] = [Intro::Swing, Intro::Crane, Intro::PushIn];
    /// Seconds from the start pose to the shot.
    pub const SECONDS: f32 = 2.6;

    /// One of the three at random, never the one chosen last.
    pub fn random() -> Intro {
        let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.subsec_nanos() as usize);
        let last = LAST_INTRO.load(std::sync::atomic::Ordering::Relaxed) as usize;
        let choices: Vec<usize> = (0..3).filter(|&i| i != last).collect();
        let i = choices[seed % choices.len()];
        LAST_INTRO.store(i as u8, std::sync::atomic::Ordering::Relaxed);
        Intro::ALL[i]
    }

    /// The camera `t` seconds into the intro, on its way to `shot`; `shot` itself at the end.
    pub fn camera(self, shot: OrbitCamera, t: f32) -> OrbitCamera {
        if t >= Intro::SECONDS {
            return shot;
        }
        // Offsets kept small, so the camera stays inside the room.
        let (yaw, pitch, distance) = match self {
            Intro::Swing => (40.0, 8.0, 1.2),
            Intro::Crane => (-12.0, (75.0 - shot.pitch).clamp(0.0, 35.0), 1.15),
            Intro::PushIn => (-22.0, -(shot.pitch - 2.0).clamp(0.0, 5.0), 1.35),
        };
        // From the offset pose (k = 1) to the shot (k = 0): leaves at rest, settles gently.
        let k = 1.0 - Ease::InOut.apply((t / Intro::SECONDS).clamp(0.0, 1.0));
        OrbitCamera {
            yaw: shot.yaw + yaw * k,
            pitch: (shot.pitch + pitch * k).clamp(-85.0, 85.0),
            distance: shot.distance * (1.0 + (distance - 1.0) * k),
            ..shot
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eases_run_from_zero_to_one() {
        for ease in [Ease::Smooth, Ease::Linear, Ease::In, Ease::Out, Ease::InOut, Ease::Sine] {
            assert!(ease.apply(0.0).abs() < 1e-6 && (ease.apply(1.0) - 1.0).abs() < 1e-6, "{ease:?}");
        }
    }

    #[test]
    fn a_path_passes_its_waypoints() {
        let path = Path { params: vec!["a".into(), "b".into()], through: vec![vec![1.0, 0.0], vec![1.0, 1.0]], seconds: 2.0, ease: Ease::Linear, closed: false, repeat: Repeat::Once };
        let at = |t: f32| path.sample(t, &[0.0, 0.0]);
        assert_eq!(at(0.0), vec![0.0, 0.0]);
        assert!(at(1.0).iter().zip([1.0, 0.0]).all(|(a, b)| (a - b).abs() < 1e-5), "{:?}", at(1.0));
        assert!(at(2.0).iter().zip([1.0, 1.0]).all(|(a, b)| (a - b).abs() < 1e-5));
    }

    #[test]
    fn an_orbit_keeps_its_radius() {
        let orbit = Orbit { params: vec!["x".into(), "y".into(), "z".into()], center: vec![1.0, 2.0, 3.0], axis: None, start: Some(vec![2.0, 2.5, 3.0]), degrees: 90.0, seconds: 1.0, ease: Ease::Linear, repeat: Repeat::Once };
        let p = orbit.sample(1.0, &[0.0; 3]);
        // A right-handed quarter turn about +y takes +x to -z, at the same height.
        assert!((p[0] - 1.0).abs() < 1e-5 && (p[1] - 2.5).abs() < 1e-5 && (p[2] - 2.0).abs() < 1e-5, "{p:?}");
    }

    #[test]
    fn every_intro_ends_on_the_shot_and_never_repeats() {
        let shot = OrbitCamera { target: [0.0, 1.0, 0.0], yaw: 10.0, pitch: 20.0, distance: 3.0, fov: 50.0 };
        for intro in Intro::ALL {
            let end = intro.camera(shot, Intro::SECONDS);
            assert!(end.yaw == shot.yaw && end.pitch == shot.pitch && end.distance == shot.distance, "{intro:?}");
            assert!(intro.camera(shot, 0.0).yaw != shot.yaw, "{intro:?} starts on the shot");
        }
        let mut last = Intro::random();
        for _ in 0..20 {
            let next = Intro::random();
            assert_ne!(next, last);
            last = next;
        }
    }

    #[test]
    fn a_ping_pong_of_a_sine_ease_is_a_cosine_wave() {
        for t in [0.0, 1.3, 4.0, 7.7, 11.0] {
            let swing = Ease::Sine.apply(Repeat::PingPong.progress(t, 6.0));
            assert!((swing - (0.5 - 0.5 * (TAU * t / 12.0).cos())).abs() < 1e-5);
        }
    }
}
