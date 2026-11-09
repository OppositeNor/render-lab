//! Reference namespaces shared by untimed stories and runtime timelines.
//! Only metadata is read: no frame, action, or script effect is evaluated.

use crate::shot::{Cue, CueProblem, Highlight, PanelMode, RecordRef, ShotState};
use std::collections::{BTreeMap, BTreeSet};

/// A stable spelling hint, without mixing different reference namespaces.
/// The known name nearest to `name`, when it is near enough to suggest.
pub(crate) fn nearest(name: &str, known: impl IntoIterator<Item = String>) -> Option<String> {
    let known: BTreeSet<String> = known.into_iter().collect();
    known
        .into_iter()
        .filter(|candidate| distance(name, candidate) <= 2.max(name.len() / 3))
        .min_by_key(|candidate| (distance(name, candidate), candidate.clone()))
}

pub(crate) fn unknown(
    category: &str,
    name: &str,
    known: impl IntoIterator<Item = String>,
    context: &str,
) -> String {
    let known: BTreeSet<String> = known.into_iter().collect();
    let nearest = known
        .iter()
        .min_by_key(|candidate| (distance(name, candidate), candidate.as_str()));
    let hint = nearest.filter(|candidate| distance(name, candidate) <= 2.max(name.len() / 3));
    let mut out = format!("unknown {category} `{name}`{context}");
    if let Some(candidate) = hint {
        out.push_str(&format!("; did you mean `{candidate}`?"));
    }
    let list = known
        .iter()
        .take(12)
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(", ");
    out.push_str(&format!(
        " (known: {}{})",
        if list.is_empty() {
            "none"
        } else {
            &list
        },
        if known.len() > 12 {
            ", …"
        } else {
            ""
        }
    ));
    out
}

/// Names for a message, comma-separated, or "none".
pub(crate) fn names_or_none<S: std::borrow::Borrow<str>>(names: &[S]) -> String {
    if names.is_empty() {
        "none".into()
    } else {
        names.join(", ")
    }
}

fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, a) in a.chars().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, b) in b.iter().enumerate() {
            let old = row[j + 1];
            row[j + 1] = (diagonal + usize::from(a != *b))
                .min(row[j] + 1)
                .min(old + 1);
            diagonal = old;
        }
    }
    row[b.len()]
}

pub(crate) fn value_problem(cue: &Cue, original: String) -> CueProblem {
    for (name, value) in &cue.params {
        if crate::values::resolve(name, value).is_ok() {
            continue;
        }
        let field = format!("params.{name}");
        if let (Some(entry), Some(option)) = (crate::values::entry(name), value.as_str())
            && let Some(choices) = entry.choices
        {
            return CueProblem::new(
                "story.value",
                &field,
                format!(
                    "{original}; {}",
                    unknown(
                        "option",
                        option,
                        choices.options.iter().map(|o| o.name.to_string()),
                        &format!(" for `{name}`")
                    )
                ),
            );
        }
        if crate::values::entry(name).is_none()
            && crate::investigation::current()
                .param_default(name)
                .is_none()
            && crate::shot::view_key(name).is_none()
        {
            let keys =
                crate::script::keys::keys(crate::investigation::current()).unwrap_or_default();
            let cue = keys.iter().filter(|k| k.get("on").is_none());
            let known = cue
                .filter_map(|k| {
                    k.get("key")
                        .or_else(|| k.get("name"))
                        .and_then(serde_json::Value::as_str)
                })
                .map(str::to_string);
            return CueProblem::new(
                "story.value",
                &field,
                format!(
                    "{original}; {}",
                    unknown("cue attribute or value", name, known, "")
                ),
            );
        }
        return CueProblem::new("story.value", &field, original);
    }
    CueProblem::new("story.value", "params", original)
}

/// Use the enums' own deserializers and serde names; this adds location and
/// spelling help to their existing checks rather than a second option table.
pub(crate) fn check_options(data: &serde_json::Value) -> Result<(), CueProblem> {
    fn option<T: serde::de::DeserializeOwned + serde::Serialize>(
        data: &serde_json::Value,
        field: &str,
        all: &[T],
    ) -> Result<(), CueProblem> {
        let Some(value) = data.get(field).filter(|v| !v.is_null()) else {
            return Ok(());
        };
        if let Err(error) = serde_json::from_value::<T>(value.clone()) {
            let known = all
                .iter()
                .filter_map(|o| serde_json::to_value(o).ok()?.as_str().map(str::to_string));
            let message = value
                .as_str()
                .map(|name| unknown(&format!("{field} option"), name, known, ""))
                .unwrap_or_else(|| error.to_string());
            return Err(CueProblem::new("story.option", field, message));
        }
        Ok(())
    }
    option(data, "panel", &PanelMode::ALL)?;
    option(data, "demo", crate::investigation::current().demos())?;
    option(data, "imageCorner", &crate::utility::inset::Corner::ALL)?;
    option(data, "ease", &crate::animation::Ease::ALL)
}

#[derive(Clone)]
struct Context {
    demo: crate::shot::DemoId,
    panel: PanelMode,
    code: Option<String>,
    record: RecordRef,
    highlight: Vec<Highlight>,
}

impl Context {
    fn for_demo(demo: crate::shot::DemoId) -> Self {
        let state = ShotState::for_demo(demo);
        let mut context = Self {
            demo,
            panel: state.panel,
            code: state.code,
            record: state.record,
            highlight: Vec::new(),
        };
        // The demonstration's own highlight names things of the panel it starts with.
        context.highlight = state
            .highlight
            .into_iter()
            .filter_map(|name| context.scoped(name))
            .collect();
        context
    }

    /// `name` in the namespace of the panel this context shows; `None` when that panel shows no highlight.
    fn scoped(&self, name: String) -> Option<Highlight> {
        match self.panel {
            PanelMode::Code => self
                .code
                .clone()
                .map(|listing| Highlight::Code { listing, tag: name }),
            PanelMode::Performance | PanelMode::Graph | PanelMode::Dag | PanelMode::Resources => {
                Some(Highlight::Pass { name })
            }
            PanelMode::Declaration | PanelMode::Values | PanelMode::Memory => {
                self.record_name().map(|record| Highlight::Field {
                    record: record.to_string(),
                    field: name,
                })
            }
            PanelMode::Hidden | PanelMode::Knowledge => None,
        }
    }

    fn apply(&mut self, cue: &Cue) {
        // Match Timeline::state_at: demo defaults first, then this cue's
        // optional fields. A record/listing change does not clear highlight.
        if let Some(demo) = cue.demo {
            *self = Self::for_demo(demo);
        }
        if let Some(record) = cue.record {
            self.record = record;
        }
        if let Some(panel) = cue.panel {
            self.panel = panel;
        }
        if let Some(code) = &cue.code {
            self.code = Some(code.clone()).filter(|c| !c.is_empty());
        }
        if let Some(highlight) = &cue.highlight {
            self.highlight = highlight.clone();
        }
    }

    fn record_name(&self) -> Option<&'static str> {
        match self.record {
            RecordRef::Camera => Some("CameraData"),
            RecordRef::Object { .. } => Some("ObjectData"),
            RecordRef::None => None,
        }
    }

    fn description(&self) -> String {
        format!(
            " in demo `{}`, {:?} panel, listing {:?}, record {:?}",
            self.demo.key(),
            self.panel,
            self.code,
            self.record
        )
    }
}

#[derive(Default)]
pub(crate) struct References {
    context: Option<Context>,
    passes: Option<Vec<(String, String)>>,
    layouts: Option<BTreeMap<String, crate::layout::ShaderStruct>>,
    images: BTreeSet<String>,
    arrows: BTreeSet<String>,
    labels: BTreeSet<String>,
    warnings: Vec<CueProblem>,
}

impl References {
    pub fn take_warnings(&mut self) -> Vec<CueProblem> {
        std::mem::take(&mut self.warnings)
    }
    /// Declarations from the whole document allow idempotent cleanup even
    /// before first activation. Creating/updating an instance still follows
    /// runtime order, as checked by check_image/check_notes.
    pub fn declare(&mut self, cue: &Cue) {
        for (name, names) in [
            (&cue.image, &mut self.images),
            (&cue.arrow, &mut self.arrows),
            (&cue.label, &mut self.labels),
        ] {
            if let Some(name) = name.as_ref().filter(|n| !n.is_empty()) {
                names.insert(name.clone());
            }
        }
    }

    pub fn check(&mut self, cue: &Cue) -> Result<(), CueProblem> {
        let current = crate::investigation::current();
        if let Some(code) = cue.code.as_deref().filter(|c| !c.is_empty())
            && current.listing(code).is_none()
        {
            return Err(CueProblem::new(
                "story.listing",
                "code",
                unknown(
                    "listing",
                    code,
                    current.listings().iter().map(|l| l.name.to_string()),
                    "",
                ),
            ));
        }
        let actions: BTreeSet<String> = current
            .actions()
            .specs
            .iter()
            .map(|a| a.key.to_string())
            .chain(
                crate::script::runtime::actions()
                    .into_iter()
                    .map(|a| a.name),
            )
            .collect();
        for name in &cue.stop {
            if !actions.contains(name) {
                return Err(CueProblem::new(
                    "story.action",
                    "stop",
                    unknown("action", name, actions.iter().cloned(), ""),
                ));
            }
        }
        if let Some(call) = &cue.run
            && !actions.contains(&call.name)
        {
            return Err(CueProblem::new(
                "story.action",
                "run",
                unknown("action", &call.name, actions.iter().cloned(), ""),
            ));
        }
        Self::check_image(cue)?;
        self.check_cleanup(cue)?;
        self.check_text(cue)?;
        let context = self
            .context
            .get_or_insert_with(|| Context::for_demo(current.demos()[0]));
        context.apply(cue);
        let context = context.clone();
        for target in &context.highlight {
            self.check_highlight(target, &context)?;
        }
        // A highlight is set only where it shows: a panel that is hidden, or a readout without a record, draws none,
        // and the next highlight would replace it unseen.
        let no_consumer = matches!(context.panel, PanelMode::Hidden | PanelMode::Knowledge)
            || (matches!(
                context.panel,
                PanelMode::Values | PanelMode::Declaration | PanelMode::Memory
            ) && context.record_name().is_none());
        if no_consumer && cue.highlight.as_ref().is_some_and(|h| !h.is_empty()) {
            return Err(CueProblem::new(
                "story.highlight.unseen",
                "highlight",
                format!(
                    "nothing shows this highlight{}: show its panel in the same cue (panelCode with its listing, a graph or performance panel, or panelValues with a record), or leave the highlight out",
                    context.description()
                ),
            ));
        }
        Ok(())
    }

    /// A cue's image: its click handler (the code's or a script's), the image kind it makes, and how it shows a
    /// graph resource, each one that exists.
    fn check_image(cue: &Cue) -> Result<(), CueProblem> {
        let current = crate::investigation::current();
        if let Some(click) = &cue.image_click {
            let script_clicks = crate::script::runtime::click_handlers();
            if !script_clicks.contains(click) {
                current.actions().check_click(click).map_err(|e| {
                    let known = current
                        .actions()
                        .clicks
                        .iter()
                        .map(|c| c.key.to_string())
                        .chain(script_clicks);
                    CueProblem::new(
                        "story.image",
                        "imageClick",
                        format!("{e}; {}", unknown("click handler", click, known, "")),
                    )
                })?;
            }
        }
        if let Some(from) = &cue.image_from {
            let package = current.package().unwrap_or("render_lab");
            if crate::views::kind(package, from).is_err() {
                let known = crate::views::kinds(package)
                    .map_err(|e| CueProblem::new("story.image", "imageFrom", e.to_string()))?;
                return Err(CueProblem::new(
                    "story.image",
                    "imageFrom",
                    unknown("image kind", from, known.iter().map(|k| k.key.clone()), ""),
                ));
            }
        }
        if let Some(show) = &cue.image_show
            && crate::graph::Show::from_key(show).is_err()
        {
            return Err(CueProblem::new(
                "story.image",
                "imageShow",
                unknown(
                    "graph image display",
                    show,
                    crate::graph::Show::ALL.map(|s| s.key().to_string()),
                    "",
                ),
            ));
        }
        Ok(())
    }

    fn check_cleanup(&self, cue: &Cue) -> Result<(), CueProblem> {
        let current = crate::investigation::current();
        let sets: BTreeSet<String> = current
            .annotations()
            .iter()
            .filter(|a| a.kind == crate::utility::annotations::AnnotationKind::ArrowSet)
            .map(|a| a.key.to_string())
            .collect();
        for name in &cue.image_hide {
            if !self.images.contains(name) && crate::shot::image_source(name).is_err() {
                let known = self
                    .images
                    .iter()
                    .cloned()
                    .chain(current.images().iter().map(|i| i.key.to_string()));
                return Err(CueProblem::new(
                    "story.image",
                    "imageHide",
                    unknown("image instance or resource", name, known, ""),
                ));
            }
        }
        for (field, hidden, names) in [
            ("arrowHide", &cue.arrow_hide, &self.arrows),
            ("labelHide", &cue.label_hide, &self.labels),
        ] {
            for name in hidden {
                if !names.contains(name) && !(field == "arrowHide" && sets.contains(name)) {
                    let known = names
                        .iter()
                        .cloned()
                        .chain(sets.iter().filter(|_| field == "arrowHide").cloned());
                    return Err(CueProblem::new(
                        "story.note",
                        field,
                        unknown(
                            if field == "arrowHide" {
                                "arrow or arrow set"
                            } else {
                                "label"
                            },
                            name,
                            known,
                            "",
                        ),
                    ));
                }
            }
        }
        Ok(())
    }

    fn check_text(&self, cue: &Cue) -> Result<(), CueProblem> {
        let current = crate::investigation::current();
        if let Some(key) = cue.callout.as_deref().filter(|k| !k.is_empty())
            && !current.sources().entries.contains_key(key)
        {
            return Err(CueProblem::new(
                "story.source",
                "callout",
                unknown(
                    "source key",
                    key,
                    current.sources().entries.keys().cloned(),
                    "",
                ),
            ));
        }
        let package = current.package().unwrap_or("render_lab");
        for (field, key) in [
            ("imageTitle", &cue.image_title),
            ("imageMarkText", &cue.image_mark_text),
            ("labelText", &cue.label_text),
        ] {
            if let Some(key) = key.as_deref().filter(|k| !k.is_empty())
                && crate::i18n::try_package_text(package, key).is_none()
                && crate::i18n::try_package_text("render_lab", key).is_none()
            {
                let known = [package, "render_lab"]
                    .into_iter()
                    .flat_map(|p| crate::i18n::package_keys(p).into_iter().map(str::to_string));
                return Err(CueProblem::new(
                    "story.text",
                    field,
                    unknown("text key", key, known, ""),
                ));
            }
        }
        Ok(())
    }

    fn pass_names(&mut self, panel: PanelMode) -> Vec<String> {
        let passes = self.passes.get_or_insert_with(|| {
            crate::graph::FrameGraph::current(&[])
                .nodes
                .iter()
                .map(|n| (n.name.to_string(), n.timing.to_string()))
                .collect()
        });
        passes
            .iter()
            .flat_map(|(node, timing)| match panel {
                PanelMode::Performance => vec![timing.clone()],
                PanelMode::Graph | PanelMode::Dag | PanelMode::Resources => vec![node.clone()],
                // No pass consumer yet: both identities are known, and the exact
                // consumer will be checked if a later cue activates one.
                _ => vec![node.clone(), timing.clone()],
            })
            .collect()
    }

    fn record_fields(&mut self, name: &str) -> Result<Vec<String>, CueProblem> {
        if self.layouts.is_none() {
            let layouts = crate::shaders::layout_json()
                .and_then(|json| crate::layout::parse_layouts(&json))
                .map_err(|e| {
                    CueProblem::new("story.highlight.schema", "highlight", e.to_string())
                })?;
            self.layouts = Some(layouts);
        }
        let layouts = self.layouts.as_ref().expect("layouts just loaded");
        let layout = layouts.get(name).ok_or_else(|| {
            CueProblem::new(
                "story.highlight",
                "highlight",
                unknown("record", name, layouts.keys().cloned(), ""),
            )
        })?;
        Ok(layout.fields.iter().map(|f| f.name.clone()).collect())
    }

    fn check_highlight(&mut self, target: &Highlight, context: &Context) -> Result<(), CueProblem> {
        let current = crate::investigation::current();
        let details = context.description();
        let tags = |listing: &str| -> Result<Vec<String>, CueProblem> {
            let listing = current.listing(listing).ok_or_else(|| {
                CueProblem::new(
                    "story.highlight",
                    "highlight",
                    unknown(
                        "listing",
                        listing,
                        current.listings().iter().map(|l| l.name.to_string()),
                        &details,
                    ),
                )
            })?;
            Ok(listing
                .lines
                .iter()
                .flat_map(|l| l.tags.iter().cloned())
                .collect())
        };
        let check = |category: &str, name: &str, known: Vec<String>| -> Result<(), CueProblem> {
            if known.iter().any(|k| k == name) {
                Ok(())
            } else {
                Err(CueProblem::new(
                    "story.highlight",
                    "highlight",
                    unknown(category, name, known, &details),
                ))
            }
        };
        // Each name is checked in its own namespace, even while no panel shows it (pass names are the panel's:
        // the performance panel's are the timing names).
        match target {
            Highlight::Pass { name } => check("pass", name, self.pass_names(context.panel))?,
            Highlight::Code { listing, tag } => {
                check(&format!("tag in listing `{listing}`"), tag, tags(listing)?)?
            }
            Highlight::Field { record, field } => check(
                &format!("field in `{record}`"),
                field,
                self.record_fields(record)?,
            )?,
        }
        let mismatch = || {
            CueProblem::new(
                "story.highlight.context",
                "highlight",
                format!("highlight namespace does not match its consumer{details}"),
            )
        };
        let matches = match context.panel {
            PanelMode::Code => {
                let listing = context.code.as_deref().ok_or_else(|| {
                    CueProblem::new(
                        "story.highlight.context",
                        "highlight",
                        format!("code highlight needs a selected listing{details}"),
                    )
                })?;
                matches!(target, Highlight::Code { listing: wanted, .. } if wanted == listing)
            }
            PanelMode::Performance | PanelMode::Graph | PanelMode::Dag | PanelMode::Resources => {
                matches!(target, Highlight::Pass { .. })
            }
            PanelMode::Declaration | PanelMode::Values | PanelMode::Memory => {
                match context.record_name() {
                    Some(record) => {
                        matches!(target, Highlight::Field { record: wanted, .. } if wanted == record)
                    }
                    None => true, // a readout shows no highlight; a cue setting one there is refused in `check`
                }
            }
            PanelMode::Hidden | PanelMode::Knowledge => true, // kept until a panel shows it
        };
        if matches {
            Ok(())
        } else {
            Err(mismatch())
        }
    }
}
