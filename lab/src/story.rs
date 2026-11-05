//! GPU-free story preflight and final timeline checks. These consume data;
//! they never execute an action, synthesize speech, or render a frame.

use crate::animation::{Ease, Motion};
use crate::shot::{Cue, CueChecks, CueProblem, Timeline, TimelineCueProblem};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Source {
    file: Option<String>,
    line: Option<u32>,
    column: Option<u32>,
    end_line: Option<u32>,
    end_column: Option<u32>,
    fallback: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Context {
    #[serde(default)]
    source: Source,
    beat: Option<String>,
    chapter: Option<String>,
    paragraph: Option<u32>,
    piece: Option<u32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Diagnostic {
    code: String,
    severity: &'static str,
    #[serde(flatten)]
    source: Source,
    beat: Option<String>,
    chapter: Option<String>,
    paragraph: Option<u32>,
    piece: Option<u32>,
    field: String,
    message: String,
}

impl Diagnostic {
    fn new(context: &Context, code: &str, field: &str, message: String) -> Self {
        Self { code: code.into(), severity: "error", source: context.source.clone(), beat: context.beat.clone(), chapter: context.chapter.clone(), paragraph: context.paragraph, piece: context.piece, field: field.into(), message }
    }

    fn problem(context: &Context, problem: CueProblem) -> Self {
        Self::new(context, problem.code, &problem.field, problem.message)
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Report {
    render_lab_story_check: u32,
    pub ok: bool,
    diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Boundary {
    Beat,
    Sentence,
}

/// A duration remains symbolic until measured speech is available.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Length {
    Seconds(f32),
    Boundary(Boundary),
}

impl Length {
    fn check(&self, context: &Context) -> Result<(), String> {
        match self {
            Self::Seconds(s) => crate::animation::seconds_problem(*s).map_or(Ok(()), |e| Err(e.into())),
            Self::Boundary(Boundary::Sentence) if context.paragraph.is_none() => Err("a beat's opening cue has no sentence boundary; use beat or numeric seconds".into()),
            _ => Ok(()),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UntimedKey {
    #[serde(default)]
    delay: f64,
    #[serde(default)]
    blend: f64,
    #[serde(default, rename = "ease")]
    _ease: Ease,
    #[serde(default)]
    animate: Vec<Motion<Length>>,
    #[serde(flatten)]
    cue: Cue,
}

#[derive(Deserialize)]
struct Entry {
    #[serde(flatten)]
    context: Context,
    key: serde_json::Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Story {
    render_lab_story: u32,
    cues: Vec<Entry>,
}

fn untimed(input: &str) -> Result<(), Diagnostic> {
    let story: Story = serde_json::from_str(input).map_err(|e| Diagnostic::new(&Context::default(), "story.schema", "story", e.to_string()))?;
    if story.render_lab_story != 1 {
        return Err(Diagnostic::new(&Context::default(), "story.version", "renderLabStory", "expected renderLabStory = 1".into()));
    }
    let mut checks = CueChecks::default();
    for entry in story.cues {
        let at = &entry.context;
        let key: UntimedKey = serde_json::from_value(entry.key).map_err(|e| Diagnostic::new(at, "story.key", "key", e.to_string()))?;
        for (field, value) in [("delay", key.delay), ("blend", key.blend)] {
            if !value.is_finite() || value < 0.0 {
                return Err(Diagnostic::new(at, "story.time", field, format!("{field} must be finite and nonnegative seconds")));
            }
        }
        checks.check(&key.cue).map_err(|e| Diagnostic::problem(at, e))?;
        for motion in &key.animate {
            key.cue.check_motion(motion).map_err(|e| Diagnostic::problem(at, e))?;
            if let Some(length) = motion.seconds() {
                length.check(at).map_err(|e| Diagnostic::new(at, "story.motion.length", "animate.seconds", e))?;
            }
        }
    }
    Ok(())
}

fn timeline(input: &str) -> Result<(), Diagnostic> {
    let mut data: serde_json::Value = serde_json::from_str(input).map_err(|e| Diagnostic::new(&Context::default(), "timeline.schema", "timeline", e.to_string()))?;
    // A raw timeline is also accepted, e.g. `story check --timeline timeline.json`.
    let provenance: Vec<Context> = if data.get("timeline").is_some() {
        let contexts = serde_json::from_value(data["provenance"].take()).map_err(|e| Diagnostic::new(&Context::default(), "timeline.provenance", "provenance", e.to_string()))?;
        data = data["timeline"].take();
        contexts
    } else {
        Vec::new()
    };
    let timeline: Timeline = serde_json::from_value(data).map_err(|e| Diagnostic::new(&Context::default(), "timeline.schema", "timeline", e.to_string()))?;
    // Keep original indices for the sidecar, even when accepting unsorted keys.
    let mut order: Vec<usize> = (0..timeline.keys.len()).collect();
    order.sort_by_key(|&i| timeline.keys[i].frame);
    let mut sorted = timeline.clone();
    sorted.keys.sort_by_key(|k| k.frame);
    sorted.validate().map_err(|error| {
        if let Some(key) = error.downcast_ref::<TimelineCueProblem>() {
            let mut context = provenance.get(order[key.index]).cloned().unwrap_or_default();
            if context.beat.is_none() {
                context.beat = key.source.clone();
            }
            Diagnostic::new(&context, key.problem.code, &key.problem.field, error.to_string())
        } else {
            Diagnostic::new(&Context::default(), "timeline.invalid", "timeline", error.to_string())
        }
    })
}

pub(crate) fn check(input: &str, full_timeline: bool) -> Report {
    let result = if full_timeline { timeline(input) } else { untimed(input) };
    Report { render_lab_story_check: 1, ok: result.is_ok(), diagnostics: result.err().into_iter().collect() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbolic_and_runtime_waves_can_omit_seconds() {
        let wave = r#"{"wave":{"param":"camera.yaw","amplitude":1,"period":2}}"#;
        assert!(serde_json::from_str::<Motion>(wave).unwrap().seconds().is_none());
        assert!(serde_json::from_str::<Motion<Length>>(wave).unwrap().seconds().is_none());
    }

    #[test]
    fn cue_and_timeline_fields_are_checked_after_flattening() {
        let key = r#"{"frame":3,"blend":2,"camera":{"yaw":4},"panel":"hidden"}"#;
        let parsed: crate::shot::Key = serde_json::from_str(key).unwrap();
        assert_eq!(parsed.frame, 3);
        assert_eq!(parsed.camera.as_ref().unwrap().yaw, Some(4.0));
        assert!(serde_json::from_str::<crate::shot::Key>(r#"{"frame":0,"typo":1}"#).is_err());
        assert!(serde_json::from_str::<crate::shot::Key>(r#"{"camera":{"yaw":4}}"#).is_err());
        assert!(serde_json::from_str::<UntimedKey>(r#"{"delay":0.5,"camera":{"yaw":4}}"#).is_ok());
        assert!(serde_json::from_str::<UntimedKey>(r#"{"frame":0}"#).is_err());
        assert!(serde_json::from_str::<UntimedKey>(r#"{"typo":0}"#).is_err());
    }
}
