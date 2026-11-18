//! Clips: the scripts' static bindings of runtime.lam's `Clip`, read once when they load into the lab's clips
//! (src/clip.rs) and checked, and the runs of animations a clip's tracks make.

use super::*;
use crate::animation::{Ease, Motion, Orbit, Path, Repeat, Wave};
use crate::clip::{Clip, Keyframe, PROGRESS, Track};
use crate::values::Reflect;

crate::lamlet_record! {
    /// runtime.lam's `Keyframe`.
    struct KeyframeRecord {
        at: Num = "at",
        value: Num = "value",
        ease: String = "ease",
    }
}

crate::lamlet_record! {
    /// runtime.lam's `_Keys`.
    struct KeysRecord {
        channel: String = "channel",
        keys: Vec<KeyframeRecord> = "keys",
    }
}

crate::lamlet_record! {
    /// runtime.lam's `_PathTrack`.
    struct PathRecord {
        at: Num = "at",
        params: Vec<String> = "params",
        through: Vec<Vec<Num>> = "through",
        seconds: Num = "seconds",
        ease: String = "ease",
        closed: bool = "closed",
        repeat: String = "repeat",
    }
}

crate::lamlet_record! {
    /// runtime.lam's `_OrbitTrack`.
    struct OrbitRecord {
        at: Num = "at",
        params: Vec<String> = "params",
        center: Vec<Num> = "center",
        degrees: Num = "degrees",
        seconds: Num = "seconds",
        ease: String = "ease",
        repeat: String = "repeat",
    }
}

crate::lamlet_record! {
    /// runtime.lam's `_WaveTrack`.
    struct WaveRecord {
        at: Num = "at",
        param: String = "param",
        amplitude: Num = "amplitude",
        period: Num = "period",
        phase: Num = "phase",
        seconds: Num = "seconds",
        fade: Num = "fade",
    }
}

crate::lamlet_record! {
    /// runtime.lam's `_RunTrack`.
    struct RunRecord {
        at: Num = "at",
        seconds: Num = "seconds",
        animation: String = "animation",
    }
}

/// An ease by the name a clip writes (src/animation.rs `Ease`).
fn ease(name: &str) -> Result<Ease, String> {
    crate::clip::ease_named(name).ok_or_else(|| {
        format!("no ease `{name}` (the lab's: smooth, linear, in, out, inOut, sine)")
    })
}

/// A motion's repeat by its name (`once`, `loop`, `pingPong`).
fn repeat(name: &str) -> Result<Repeat, String> {
    match name {
        "once" => Ok(Repeat::Once),
        "loop" => Ok(Repeat::Loop),
        "pingPong" => Ok(Repeat::PingPong),
        other => Err(format!("no repeat `{other}` (once, loop, pingPong)")),
    }
}

fn floats(list: Vec<Num>) -> Vec<f32> {
    list.into_iter().map(|n| n.0).collect()
}

/// runtime.lam's `Track`, read by its type.
struct TrackRecord(Track);

impl lamlet::embed::FromLamlet for TrackRecord {
    fn from_lamlet(v: &Value) -> Result<Self, String> {
        use super::typed::payload;
        let (tag, p) = super::typed::variant(v)?;
        Ok(TrackRecord(match tag {
            "keys" => {
                let r: KeysRecord = payload(tag, p)?;
                let keys = r
                    .keys
                    .into_iter()
                    .map(|k| {
                        Ok(Keyframe {
                            at: k.at.0,
                            value: k.value.0,
                            ease: ease(&k.ease)?,
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                // The keyframes are in time order, so a track's value between two is the one it eases between.
                if keys.windows(2).any(|w| w[1].at < w[0].at) {
                    return Err(format!(
                        "the keyframes of `{}` are not in time order",
                        r.channel
                    ));
                }
                Track::Keys {
                    channel: r.channel,
                    keys,
                }
            }
            "path" => {
                let r: PathRecord = payload(tag, p)?;
                Track::Motion {
                    at: r.at.0,
                    motion: Motion::Path(Path {
                        params: r.params,
                        through: r.through.into_iter().map(floats).collect(),
                        seconds: r.seconds.0,
                        ease: ease(&r.ease)?,
                        closed: r.closed,
                        repeat: repeat(&r.repeat)?,
                    }),
                }
            }
            "orbit" => {
                let r: OrbitRecord = payload(tag, p)?;
                Track::Motion {
                    at: r.at.0,
                    motion: Motion::Orbit(Orbit {
                        params: r.params,
                        center: floats(r.center),
                        axis: None,
                        start: None,
                        degrees: r.degrees.0,
                        seconds: r.seconds.0,
                        ease: ease(&r.ease)?,
                        repeat: repeat(&r.repeat)?,
                    }),
                }
            }
            "wave" => {
                let r: WaveRecord = payload(tag, p)?;
                Track::Motion {
                    at: r.at.0,
                    motion: Motion::Wave(Wave {
                        param: r.param,
                        center: None,
                        amplitude: r.amplitude.0,
                        period: r.period.0,
                        phase: r.phase.0,
                        seconds: Some(r.seconds.0),
                        fade: r.fade.0,
                    }),
                }
            }
            "run" => {
                let r: RunRecord = payload(tag, p)?;
                Track::Run {
                    at: r.at.0,
                    seconds: r.seconds.0,
                    animation: r.animation,
                }
            }
            other => return Err(format!("no track `{other}`")),
        }))
    }
}

/// The clip `name` a static binding holds.
pub(super) fn clip_of(name: &str, value: &Value) -> Result<Clip, String> {
    let tracks = value
        .field("tracks")
        .ok_or_else(|| format!("the clip `{name}` has no field `tracks`"))?;
    let tracks = super::typed::items::<TrackRecord>(tracks, "track")
        .map_err(|e| format!("the clip `{name}`: {e}"))?;
    Ok(Clip {
        name: name.to_string(),
        tracks: tracks.into_iter().map(|t| t.0).collect(),
    })
}

/// Why `channel` is not one a clip's keyframes may drive with `value`: a progress, the camera's, or a value's
/// (checked against its type, src/values.rs `resolve`).
fn channel_problem(channel: &str, value: f32) -> Option<String> {
    let camera = crate::camera::OrbitCamera::schema()
        .iter()
        .any(|e| e.channels().contains(&channel));
    if channel.starts_with(PROGRESS) || camera {
        return None;
    }
    let json = serde_json::json!(value);
    match crate::values::resolve(channel, &json) {
        Ok(channels) if channels.iter().any(|(c, _)| c == channel) => None,
        Ok(_) => Some(format!(
            "`{channel}` is a vector: a track drives one of its components"
        )),
        Err(e) => Some(e),
    }
}

impl Runtime {
    /// Refuses a clip whose channel is not a value's, the camera's, or a progress, whose keyframe a value does not
    /// take, whose motion does not fit its channels, or whose run names no animation of the scripts.
    pub(super) fn check_clips(&self) -> Result<(), String> {
        for clip in self.clips.values() {
            for track in &clip.tracks {
                let problem = match track {
                    Track::Keys { channel, keys } => {
                        keys.iter().find_map(|k| channel_problem(channel, k.value))
                    }
                    Track::Motion { motion, .. } => motion.problem().or_else(|| {
                        motion
                            .params()
                            .into_iter()
                            .find_map(|p| channel_problem(p, 0.0))
                    }),
                    Track::Run { animation, .. } => (!self.animations.contains_key(animation))
                        .then(|| {
                            let known: Vec<&str> =
                                self.animations.keys().map(String::as_str).collect();
                            format!(
                                "no animation `{animation}` in the scripts (theirs: {})",
                                crate::story_refs::names_or_none(&known)
                            )
                        }),
                };
                if let Some(p) = problem {
                    return Err(format!("the clip `{}`: {}: {p}", clip.name, track.label()));
                }
            }
        }
        Ok(())
    }
}

/// The scripts' clips, by name.
pub fn clips() -> Vec<Arc<Clip>> {
    with_runtime(|rt| rt.clips.values().cloned().collect())
        .ok()
        .flatten()
        .unwrap_or_default()
}

#[cfg(test)]
thread_local! {
    /// The clips a unit test plays, whose investigation has no scripts.
    pub(crate) static TEST_CLIPS: RefCell<Vec<Arc<Clip>>> = const { RefCell::new(Vec::new()) };
}

/// The scripts' clip `name`, if they have one.
pub fn clip(name: &str) -> Option<Arc<Clip>> {
    #[cfg(test)]
    if let Some(c) = TEST_CLIPS.with(|t| t.borrow().iter().find(|c| c.name == name).cloned()) {
        return Some(c);
    }
    with_runtime(|rt| rt.clips.get(name).cloned())
        .ok()
        .flatten()
        .flatten()
}

/// Applies the scripts' animation hook `name` at `t` seconds to `state`: the keys it set, or None when the
/// scripts have no such hook. Its writes apply all or none; a failure is reported and sets nothing.
pub fn run_animation(name: &str, t: f32, state: &mut ShotState) -> Option<Vec<String>> {
    let snapshot = state.clone();
    with_runtime(|rt| {
        let kind = Kind::Animation(name.to_string());
        rt.hook(&kind)?;
        let written = rt.call_writes(&kind, vec![Value::Float(t as f64)], &snapshot, None, None);
        Some(match written {
            Ok(w) => {
                rt.succeeded(&kind.to_string());
                apply(&w, state);
                w.into_iter().map(|(k, _)| k).collect()
            }
            Err(e) => {
                rt.failed(&kind.to_string(), e);
                Vec::new()
            }
        })
    })
    .ok()
    .flatten()
    .flatten()
}
