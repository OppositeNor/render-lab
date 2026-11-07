//! Why a cue cannot play: its notes, its image, its motions, as problems a
//! story's check reports.

use super::*;

/// Why `key`'s arrows and labels cannot show: a point, a set, or a colour
/// the investigation does not have, or a new one without its ends, point, or
/// text. `made` are the notes the keys before it made.
pub(super) fn check_notes(key: &Cue, made: &mut BTreeMap<String, &'static str>) -> Result<(), String> {
    let mut names = std::collections::BTreeSet::new();
    for name in [&key.arrow, &key.arrows, &key.label].into_iter().flatten().filter(|n| !n.is_empty()) {
        if !names.insert(name) {
            return Err(format!("note `{name}` is defined more than once in one cue (arrow, arrows, label share instance names)"));
        }
    }
    for (field, value) in [("arrowFrom", &key.arrow_from), ("arrowTo", &key.arrow_to), ("labelAt", &key.label_at)] {
        if let Some(v) = value {
            point_value(v).map_err(|e| format!("{field}: {e}"))?;
        }
    }
    for color in [&key.arrow_color, &key.label_color].into_iter().flatten() {
        crate::utility::annotations::parse_color(color)?;
    }
    if let Some(name) = key.arrow.as_deref().filter(|n| !n.is_empty()) {
        if made.get(name) != Some(&"arrow") && (key.arrow_from.is_none() || key.arrow_to.is_none()) {
            return Err(format!("the new arrow `{name}` names both its ends (arrowFrom, arrowTo)"));
        }
        made.insert(name.to_string(), "arrow");
    } else if key.arrow_from.is_some() || key.arrow_to.is_some() || key.arrow_width.is_some() || (key.arrow_color.is_some() && key.arrows.is_none()) {
        return Err("arrowFrom, arrowTo, arrowColor, and arrowWidth go with the arrow they set (`arrow=<name>`)".into());
    }
    if let Some(set) = &key.arrows {
        let declared = investigation::current().annotations().iter().any(|a| a.key == set && a.kind == AnnotationKind::ArrowSet);
        if !declared {
            let known: Vec<&str> = investigation::current().annotations().iter().filter(|a| a.kind == AnnotationKind::ArrowSet).map(|a| a.key).collect();
            return Err(format!("no arrow set `{set}` (the investigation's: {})", crate::story_refs::names_or_none(&known)));
        }
        made.insert(set.clone(), "arrows");
    }
    if let Some(name) = key.label.as_deref().filter(|n| !n.is_empty()) {
        if made.get(name) != Some(&"label") && (key.label_at.is_none() || key.label_text.is_none()) {
            return Err(format!("the new label `{name}` names its point and its text (labelAt, labelText)"));
        }
        made.insert(name.to_string(), "label");
    } else if key.label_at.is_some() || key.label_text.is_some() || key.label_color.is_some() {
        return Err("labelAt, labelText, and labelColor go with the label they set (`label=<name>`)".into());
    }
    Ok(())
}

/// Why `key`'s image cannot show: an image or a kind the investigation does
/// not have, or parameters its kind does not take. `instances` are the
/// instances made by the keys before it, by name.
pub(super) fn check_image(key: &Cue, instances: &mut BTreeMap<String, &'static crate::views::ViewKind>, graphs: &mut std::collections::BTreeSet<String>) -> Result<(), String> {
    let Some(name) = key.image.as_deref().filter(|n| !n.is_empty()) else {
        return if key.image_from.is_some() || key.image_graph.is_some() || key.image_show.is_some() || !key.image_params.is_empty() || key.image_click.is_some() { Err("imageFrom, imageGraph, imageShow, imageClick, and image.<param> go with the image they make or set (`image=<name>`)".into()) } else { Ok(()) };
    };
    if key.image_from.is_some() && key.image_graph.is_some() {
        return Err(format!("image `{name}`: imageFrom (an image kind) or imageGraph (a resource of the frame), not both"));
    }
    if let Some(graph) = &key.image_graph {
        crate::graph::FrameGraph::current(&[]).inspected(graph)?;
        instances.remove(name);
        graphs.insert(name.to_string());
    } else if key.image_from.is_some() {
        graphs.remove(name);
    }
    if graphs.contains(name) {
        if let Some(show) = &key.image_show {
            crate::graph::Show::from_key(show)?;
        }
        return key.image_params.iter().try_for_each(|(param, value)| crate::graph::check_inspect_param(param, &components(value)?));
    }
    if key.image_show.is_some() {
        return Err(format!("imageShow draws a graph image; `{name}` is not one (make it with imageGraph=<node>.<resource>)"));
    }
    if let Some(click) = &key.image_click && !crate::script::runtime::click_handlers().iter().any(|h| h == click) {
        investigation::current().actions().check_click(click)?;
    }
    let package = investigation::current().package().unwrap_or("render_lab");
    if let Some(from) = &key.image_from {
        instances.insert(name.to_string(), crate::views::kind(package, from)?);
    }
    match instances.get(name) {
        Some(kind) => key.image_params.iter().try_for_each(|(param, value)| kind.check(param, &components(value)?)),
        None if !key.image_params.is_empty() => Err(format!("image.<param> sets the parameters of an image kind's instance; `{name}` is not one (make it with imageFrom=<kind>)")),
        None => image_source(name).map(|_| ()),
    }
}

/// What a story's `image=` names: an image the investigation publishes, else
/// a PNG resource its package names; why not, for neither.
pub(crate) fn image_source(reference: &str) -> Result<ImageSource, String> {
    let current = investigation::current();
    if current.images().iter().any(|i| i.key == reference) {
        return Ok(ImageSource::Published(reference.to_string()));
    }
    let package = current.package().unwrap_or("render_lab");
    let published: Vec<&str> = current.images().iter().map(|i| i.key).collect();
    let full = crate::resources::resolve(package, reference).map_err(|e| format!("image `{reference}`: {e}"))?;
    let Some(resource) = crate::resources::find(&full) else {
        return Err(format!("image `{reference}`: neither an image the investigation publishes ({}) nor a resource (`{full}`)", crate::story_refs::names_or_none(&published)));
    };
    if !resource.path().extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("png")) {
        return Err(format!("image `{reference}`: resource `{full}` is not a PNG image"));
    }
    Ok(ImageSource::Resource(full))
}

/// A semantic failure, before adding a story location or a runtime frame.
#[derive(Debug)]
pub(crate) struct CueProblem {
    pub code: &'static str,
    pub field: String,
    pub message: String,
}

#[derive(Debug)]
pub(crate) struct TimelineCueProblem {
    pub index: usize,
    pub frame: u64,
    pub source: Option<String>,
    pub problem: CueProblem,
}

impl std::fmt::Display for TimelineCueProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}, in key at frame {} ({:?})", self.problem.message, self.frame, self.source)
    }
}

impl std::error::Error for TimelineCueProblem {}

impl CueProblem {
    pub(crate) fn new(code: &'static str, field: &str, message: String) -> Self {
        Self { code, field: field.to_string(), message }
    }
}

/// The declarations made by earlier cues. Checking never executes actions
/// or evaluates a frame; the same state is used by preflight and playback.
#[derive(Default)]
pub(crate) struct CueChecks {
    instances: BTreeMap<String, &'static crate::views::ViewKind>,
    graphs: std::collections::BTreeSet<String>,
    notes: BTreeMap<String, &'static str>,
    references: crate::story_refs::References,
}

impl CueChecks {
    pub fn take_warnings(&mut self) -> Vec<CueProblem> {
        self.references.take_warnings()
    }
    pub fn declare(&mut self, cue: &Cue) {
        self.references.declare(cue);
    }

    pub fn check(&mut self, cue: &Cue) -> Result<(), CueProblem> {
        cue.targets().map_err(|e| crate::story_refs::value_problem(cue, e))?;
        check_notes(cue, &mut self.notes).map_err(|e| CueProblem::new("story.note", "annotations", e))?;
        self.references.check(cue)?;
        check_image(cue, &mut self.instances, &mut self.graphs).map_err(|e| CueProblem::new("story.image", "image", e))?;
        if let Some(call) = &cue.run {
            let checked = crate::script::runtime::check_action(call).unwrap_or_else(|| investigation::current().actions().check(call));
            checked.map_err(|e| CueProblem::new("story.action", "run", e))?;
        }
        Ok(())
    }
}

impl Cue {
    pub(crate) fn check_motion<D>(&self, motion: &Motion<D>) -> Result<(), CueProblem> {
        if let Some(problem) = motion.problem() {
            return Err(CueProblem::new("story.motion", "animate", problem));
        }
        for name in motion.params() {
            if investigation::current().param_default(name).is_none() && !camera_channels().contains(&name) && !free_camera_channels().contains(&name) && !frame_channels().contains(&name) {
                return Err(CueProblem::new("story.motion.channel", "animate", format!("unknown channel `{name}` in a motion")));
            }
            if self.sets(name) {
                return Err(CueProblem::new("story.motion.conflict", "animate", format!("`{name}` is both set and animated by the key")));
            }
        }
        Ok(())
    }

    /// A key's motions together: two motions of one key may not drive the same channel (the later would silently
    /// replace the earlier); a later key taking a channel over is ordinary.
    pub(crate) fn check_motions<D>(&self, motions: &[Motion<D>]) -> Result<(), CueProblem> {
        let mut seen: Vec<(&str, usize)> = Vec::new();
        for (i, motion) in motions.iter().enumerate() {
            self.check_motion(motion)?;
            for name in motion.params() {
                if let Some((_, first)) = seen.iter().find(|(n, _)| *n == name) {
                    return Err(CueProblem::new("story.motion.duplicate", "animate", format!("`{name}` is driven by two motions of the key (motions {first} and {i}); give each channel one motion, or move one to a later key")));
                }
                seen.push((name, i));
            }
        }
        Ok(())
    }
}
