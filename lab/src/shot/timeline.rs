//! The timeline a storyboard builds: its parts, chapters, and beats, and the keys
//! whose cues set the shot.

use super::*;

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
    pub locale: crate::i18n::Locale,
    /// Which part of a series it tells (src/packages.rs, `Series`), if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part: Option<TimelinePart>,
    pub chapters: Vec<TimelineChapter>,
    pub beats: Vec<TimelineBeat>,
    pub keys: Vec<Key>,
}

/// A timeline's place in its series: part `number` of `count`, of `series` (its title).
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TimelinePart {
    pub number: u32,
    pub count: u32,
    pub series: String,
}

impl Timeline {
    /// The tag over a chapter of it: `01 / 08   TITLE`, after its part's (`PART 1 / 3 · `) when it is one.
    pub fn chapter_tag(&self, chapter: &TimelineChapter) -> String {
        let tag = format!(
            "{:02} / {:02}   {}",
            chapter.number,
            self.chapters.len(),
            chapter.title.to_uppercase()
        );
        match &self.part {
            Some(p) => format!(
                "{}   {tag}",
                crate::i18n::tr_with(
                    "ui-part_tag",
                    crate::args!(number = p.number, count = p.count)
                )
            ),
            None => tag,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineChapter {
    pub number: u32,
    pub title: String,
    pub start_frame: u64,
    pub end_frame: u64,
}

/// The type of a beat's scene: the lab's 3D frame, or a 2D page in its
/// place, the beat's image (a diagram) or, without one, its sources card.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum SceneType {
    #[serde(rename = "3d")]
    ThreeD,
    #[serde(rename = "2d")]
    TwoD,
}

impl TimelineBeat {
    /// The caption shown at `frame`, if any.
    pub fn caption_at(&self, frame: u64) -> Option<&Caption> {
        self.captions
            .iter()
            .find(|c| c.start <= frame && frame < c.end)
    }

    /// A 2D scene that shows a diagram, not its sources card.
    pub fn shows_diagram(&self) -> bool {
        self.kind == SceneType::TwoD && (self.diagram.is_some() || self.graph.is_some())
    }

    /// The diagram's graph (made once): the beat's own, or the frame graph as its scene groups it.
    pub fn diagram_graph(&self) -> Option<Result<&crate::graph_view::Graph, &str>> {
        if self.diagram.is_none() && self.graph.is_none() {
            return None;
        }
        let made = self
            .diagram_graph
            .get_or_init(|| match (&self.graph, &self.diagram) {
                (Some(scene), _) => crate::graph::FrameGraph::current(&[]).scene_graph(scene),
                (None, Some(spec)) => spec.graph(),
                (None, None) => unreachable!("a beat with a diagram"),
            });
        Some(made.as_ref().map_err(|e| e.as_str()))
    }

    /// What the frame shows of its diagram, if it shows one (a diagram
    /// that cannot be made shows none; the timeline's check names it first),
    /// settled (no transition).
    pub fn beat_diagram(&self) -> Option<crate::diagram::BeatDiagram<'_>> {
        let graph = self.diagram_graph()?.ok()?;
        self.shows_diagram().then_some(crate::diagram::BeatDiagram {
            graph,
            focus: &self.focus,
            scene: self.graph.as_ref(),
            transition: crate::diagram::Transition::default(),
        })
    }

    /// Both 2D beats show one diagram (or one frame graph scene) in one chapter.
    pub fn same_diagram(&self, other: &TimelineBeat) -> bool {
        self.kind == SceneType::TwoD
            && other.kind == SceneType::TwoD
            && self.chapter == other.chapter
            && self.shows_diagram()
            && other.shows_diagram()
            && self.diagram == other.diagram
            && self.graph == other.graph
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineBeat {
    pub id: String,
    pub chapter: u32,
    pub kind: SceneType,
    pub start_frame: u64,
    pub frames: u64,
    pub headline: String,
    /// The diagram a 2D scene shows: its source (src/diagram.rs), and the
    /// nodes lit while it plays.
    #[serde(default)]
    pub diagram: Option<crate::graph_view::Spec>,
    /// A 2D scene that presents the frame graph, its sections (src/graph.rs):
    /// drawn as a diagram the lab makes from the live graph (once).
    #[serde(default)]
    pub graph: Option<crate::graph::GraphScene>,
    #[serde(skip)]
    diagram_graph: std::sync::OnceLock<Result<crate::graph_view::Graph, String>>,
    #[serde(default)]
    pub focus: Vec<String>,
    #[serde(default)]
    pub sources: Vec<String>,
    #[serde(default)]
    pub captions: Vec<Caption>,
    /// A 2D scene that shows the investigation's package's legal notices
    /// (`[[notice]]`; the video builder ends a video with one).
    #[serde(default)]
    pub notices: bool,
    /// A 2D scene that shows the works the story cites (the source index's
    /// `references`; a paper's story ends with one).
    #[serde(default)]
    pub references: bool,
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

/// What a cue highlights: a name in the namespace of the panel that shows it (a pass of the graph, DAG,
/// resources, or performance panel; a tag of a listing; a field of a record's struct).
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Highlight {
    Pass { name: String },
    Code { listing: String, tag: String },
    Field { record: String, field: String },
}

impl Highlight {
    /// The name the panel lights.
    pub fn name(&self) -> &str {
        match self {
            Self::Pass { name } => name,
            Self::Code { tag, .. } => tag,
            Self::Field { field, .. } => field,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Key {
    pub frame: u64,
    #[serde(default)]
    pub blend: u64,
    /// How the key's values move over its blend.
    #[serde(default)]
    pub ease: Ease,
    /// Motions that start at the key (src/animation.rs), each driving its
    /// channels until a later key sets them.
    #[serde(default)]
    pub animate: Vec<Motion>,
    /// Provenance for error messages (storyboard beat id).
    #[serde(default)]
    pub source: Option<String>,
    #[serde(flatten)]
    pub cue: Cue,
}

impl std::ops::Deref for Key {
    type Target = Cue;

    fn deref(&self) -> &Cue {
        &self.cue
    }
}

/// What a cue sets, independent of measured speech and frame numbers.
/// Both an untimed story and a runtime timeline check these same values.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Cue {
    #[serde(default)]
    pub demo: Option<DemoId>,
    #[serde(default)]
    pub camera: Option<CameraKey>,
    #[serde(default)]
    pub record: Option<RecordRef>,
    #[serde(default)]
    pub highlight: Option<Vec<Highlight>>,
    #[serde(default)]
    pub panel: Option<PanelMode>,
    /// The values it sets, as a story writes them (src/values.rs, `resolve`):
    /// a number, `true` or `false`, an option's name, or an array for a vector.
    #[serde(default)]
    pub params: BTreeMap<String, serde_json::Value>,
    /// `params` as the numbers of the channels they set, read on first use
    /// (`targets`).
    #[serde(skip)]
    resolved: std::sync::OnceLock<Result<Vec<(String, f32)>, String>>,
    /// `""` clears the callout.
    #[serde(default)]
    pub callout: Option<String>,
    /// Pseudo-code listing for PanelMode::Code; `""` clears it.
    #[serde(default)]
    pub code: Option<String>,
    /// The formula card: a formula, `{ FORMULA(…) }` (src/rich_text.rs); `""` clears it.
    #[serde(default)]
    pub formula: Option<String>,
    /// The card's lit term (0: none). It fades in over the key's `blend`.
    #[serde(default)]
    pub term: Option<usize>,
    /// An image to show in an inset (packages/render_lab/src/inset.rs), by name:
    /// one the investigation publishes, a PNG resource (`<package>.<key>`, or
    /// its package's own key), or, with `image_from`, an instance of an image
    /// kind (src/views.rs) of that name. It fades in over `blend`; `""` fades
    /// out every image.
    #[serde(default)]
    pub image: Option<String>,
    /// The image kind of the package that `image` is an instance of.
    #[serde(default)]
    pub image_from: Option<String>,
    /// The frame graph's resource that `image` shows, as it stood right
    /// after a node: `<node>.<resource>` (src/graph.rs, the inspector).
    #[serde(default)]
    pub image_graph: Option<String>,
    /// How a graph image draws its resource (`graph::Show`: color, unorm,
    /// normal, depth, ramp, signed, count, float, hash); its format's default.
    #[serde(default)]
    pub image_show: Option<String>,
    /// The instance's parameters it sets (`image.<param>=…`), blended over `blend`.
    #[serde(default)]
    pub image_params: BTreeMap<String, serde_json::Value>,
    /// The images it fades out.
    #[serde(default)]
    pub image_hide: Vec<String>,
    /// The click handler a click on `image`'s picture runs (src/actions.rs, `#[click]`).
    #[serde(default)]
    pub image_click: Option<String>,
    /// An arrow to show, by name (packages/render_lab/src/annotations.rs); `""`
    /// fades out every arrow and set. A new one names both ends.
    #[serde(default)]
    pub arrow: Option<String>,
    /// Its ends: three numbers (blended over `blend`) or a point the
    /// investigation publishes, by key.
    #[serde(default)]
    pub arrow_from: Option<serde_json::Value>,
    #[serde(default)]
    pub arrow_to: Option<serde_json::Value>,
    /// Its colour (`#rrggbb`; a set's arrows keep their own without one).
    #[serde(default)]
    pub arrow_color: Option<String>,
    /// Its width, in reference pixels (3 by default).
    #[serde(default)]
    pub arrow_width: Option<f32>,
    /// An arrow set the investigation publishes, to show.
    #[serde(default)]
    pub arrows: Option<String>,
    /// The arrows and sets it fades out.
    #[serde(default)]
    pub arrow_hide: Vec<String>,
    /// A label to show, by name; `""` fades out every label. A new one names
    /// its point and its text.
    #[serde(default)]
    pub label: Option<String>,
    /// Its point: three numbers or a published point's key.
    #[serde(default)]
    pub label_at: Option<serde_json::Value>,
    /// Its text: a key of the investigation's package's text.
    #[serde(default)]
    pub label_text: Option<String>,
    #[serde(default)]
    pub label_color: Option<String>,
    /// The labels it fades out.
    #[serde(default)]
    pub label_hide: Vec<String>,
    /// Its title: a text key of the investigation's package (the
    /// investigation's own title of it, or none, by default).
    #[serde(default)]
    pub image_title: Option<String>,
    /// Its corner (`top-right` by default).
    #[serde(default)]
    pub image_corner: Option<Corner>,
    /// A region of it to point out (`imageMark=u0,v0,u1,v1`, fractions of the
    /// picture from its top left): outlined, the rest dimmed, faded in over
    /// `blend` and moved there from a region already shown; empty: faded out.
    #[serde(default)]
    pub image_mark: Option<Vec<f32>>,
    /// The mark's label: a text key of the investigation's package (empty: none).
    #[serde(default)]
    pub image_mark_text: Option<String>,
    /// An action it starts (src/actions.rs), in place of a running one of the
    /// same name.
    #[serde(default)]
    pub run: Option<ActionCall>,
    /// The actions it stops.
    #[serde(default)]
    pub stop: Vec<String>,
}

impl Cue {
    /// The numbers its values set, by channel (a vector by component, a flag
    /// as 1 or 0, an option as its index); why not, for a value the
    /// investigation does not declare or one that does not fit its type.
    pub fn targets(&self) -> Result<&[(String, f32)], String> {
        self.resolved
            .get_or_init(|| {
                let mut out = Vec::new();
                for (name, json) in &self.params {
                    out.extend(values::resolve(name, json)?);
                }
                Ok(out)
            })
            .as_deref()
            .map_err(Clone::clone)
    }

    /// Whether it sets `channel` (by itself, or as a component of a value it sets).
    pub(super) fn sets(&self, channel: &str) -> bool {
        self.targets()
            .is_ok_and(|t| t.iter().any(|(c, _)| c == channel))
    }
}
