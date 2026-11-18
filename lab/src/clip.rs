//! Clips: named animations with a length of their own, made of tracks. A story's cue plays one at a word
//! (`play`), at its own speed or stretched over a range of words (`playOver`); a demonstration's Play plays its
//! first (src/viewer/); the Animation panel shows them as a dopesheet.
//!
//! A track drives one thing from the clip's start:
//!
//! - **keyframes** of a channel: a value's (`restirCandidates`, a vector's component, `camera.yaw`) or a
//!   progress of the package's own (`progress.<name>`, 0 to 1, which scenes, overlays, and readouts read), each
//!   keyframe a value `at` seconds, eased from the one before; before the first, the channel eases from its value
//!   when the clip started;
//! - a **motion** (src/animation.rs: a path, an orbit, a wave), from `at` seconds;
//! - a **run** of an animation, for what keyframes cannot say: a script's `Animation` hook (runtime.lam) given the
//!   seconds since its track began, for `seconds` (0: for ever).
//!
//! A played clip holds its end: its keyframes their last values and its motions where they stopped, until a
//! later key or clip takes a channel over; its runs their last writes (set after every channel), until the
//! demonstration changes.
//!
//! Clips are declared in a package's scripts (runtime.lam's `Clip`, src/script/runtime/clips.rs), checked when
//! they load, and listed by `render-lab --investigation <id> clips`. A clip is data: playing it is a pure
//! function of the frame, as every other channel of a timeline is (src/shot/eval.rs).

use crate::animation::{Ease, Motion};

/// The channel a progress named `name` is kept in (`progress.<name>`).
pub fn progress_channel(name: &str) -> String {
    format!("{PROGRESS}{name}")
}

/// The prefix of the progress channels.
pub const PROGRESS: &str = "progress.";

/// A value of a keyframe track at a time: `value` at `at` seconds, reached by `ease` from the keyframe before.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Keyframe {
    pub at: f32,
    pub value: f32,
    pub ease: Ease,
}

/// What one track of a clip drives.
#[derive(Clone, Debug)]
pub enum Track {
    /// A channel through keyframes, in time order.
    Keys {
        channel: String,
        keys: Vec<Keyframe>,
    },
    /// A motion that starts `at` seconds in.
    Motion { at: f32, motion: Motion },
    /// The script's animation hook `animation`, from `at` for `seconds` (0: for ever).
    Run {
        at: f32,
        seconds: f32,
        animation: String,
    },
}

impl Track {
    /// The seconds into its clip when it starts and ends.
    pub fn span(&self) -> (f32, f32) {
        match self {
            Track::Keys { keys, .. } => (
                keys.first().map_or(0.0, |k| k.at.min(0.0)),
                keys.last().map_or(0.0, |k| k.at),
            ),
            Track::Motion { at, motion } => (*at, at + motion.seconds().copied().unwrap_or(0.0)),
            Track::Run { at, seconds, .. } => (*at, at + seconds),
        }
    }

    /// What it drives, as the panel and the listing name it: a channel, a motion's channels, or an animation.
    pub fn label(&self) -> String {
        match self {
            Track::Keys { channel, .. } => channel.clone(),
            Track::Motion { motion, .. } => motion.params().join(", "),
            Track::Run { animation, .. } => format!("run {animation}"),
        }
    }

    /// The seconds into the clip of its marks: a keyframe track's keyframes, else where it starts and ends.
    pub fn marks(&self) -> Vec<f32> {
        match self {
            Track::Keys { keys, .. } => keys.iter().map(|k| k.at).collect(),
            _ => {
                let (a, b) = self.span();
                vec![a, b]
            }
        }
    }
}

/// A named animation: its tracks, which start together at its start.
#[derive(Clone, Debug)]
pub struct Clip {
    pub name: String,
    pub tracks: Vec<Track>,
}

impl Clip {
    /// Its length in seconds: where its last track ends.
    pub fn seconds(&self) -> f32 {
        self.tracks.iter().map(|t| t.span().1).fold(0.0, f32::max)
    }
}

/// A keyframe track's value `t` seconds into its clip, `start` the channel's value when the clip started: held
/// at `start` before 0 and at its last keyframe after it; between two keyframes, eased by the later one's ease,
/// as a timeline key blends a value (src/shot/eval.rs `Segment`).
pub fn sample_keys(keys: &[Keyframe], t: f32, start: f32) -> f32 {
    // Before the first keyframe the track runs from the channel's value at 0 to it.
    let mut from = Keyframe {
        at: 0.0,
        value: start,
        ease: Ease::Linear,
    };
    if t < 0.0 {
        return start;
    }
    for k in keys {
        if t < k.at {
            let span = k.at - from.at;
            if span <= 0.0 {
                return k.value;
            }
            let x = (t - from.at) / span;
            return from.value + (k.value - from.value) * k.ease.apply(x);
        }
        from = *k;
    }
    from.value
}

/// The name of an ease as a clip writes it, and the ease (src/animation.rs `Ease`, by its serde name).
pub fn ease_named(name: &str) -> Option<Ease> {
    Ease::ALL.into_iter().find(|e| {
        serde_json::to_value(e)
            .ok()
            .and_then(|v| v.as_str().map(|s| s == name))
            .unwrap_or(false)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(at: f32, value: f32, ease: Ease) -> Keyframe {
        Keyframe { at, value, ease }
    }

    #[test]
    fn keyframes_ease_from_the_start_value_then_between_and_hold() {
        let keys = [k(1.0, 1.0, Ease::Linear), k(3.0, 0.0, Ease::Linear)];
        assert_eq!(sample_keys(&keys, -1.0, 0.5), 0.5);
        assert_eq!(sample_keys(&keys, 0.0, 0.5), 0.5);
        assert_eq!(sample_keys(&keys, 0.5, 0.5), 0.75);
        assert_eq!(sample_keys(&keys, 2.0, 0.5), 0.5);
        assert_eq!(sample_keys(&keys, 9.0, 0.5), 0.0);
    }

    #[test]
    fn a_keyframe_at_zero_jumps() {
        let keys = [k(0.0, 2.0, Ease::Smooth), k(1.0, 4.0, Ease::Smooth)];
        assert_eq!(sample_keys(&keys, 0.0, 0.0), 2.0);
        assert_eq!(sample_keys(&keys, 0.5, 0.0), 3.0);
    }

    #[test]
    fn eases_by_name_and_a_clip_s_length() {
        assert_eq!(ease_named("inOut"), Some(Ease::InOut));
        assert_eq!(ease_named("bounce"), None);
        let clip = Clip {
            name: "a".into(),
            tracks: vec![
                Track::Keys {
                    channel: progress_channel("x"),
                    keys: vec![k(0.5, 1.0, Ease::Smooth)],
                },
                Track::Run {
                    at: 0.25,
                    seconds: 1.0,
                    animation: "play".into(),
                },
            ],
        };
        assert_eq!(clip.seconds(), 1.25);
    }
}
