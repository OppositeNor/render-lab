//! The frame graph as a 2D scene (a timeline beat's `graph`, the graph panel's
//! DAG), and what a node is compiled from.

use super::*;

/// A 2D scene that presents the frame graph (a timeline beat's `graph`,
/// from its package's `graphs/<name>.toml`, the resource `graph/<name>`; tools/video/frame_graph.py): its
/// sections, each a box of passes (or resources) a story's `focus` names.
#[derive(Clone, Debug, Default, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct GraphScene {
    pub sections: Vec<Section>,
    /// Passes in no section (the lab's overlay and composite need not be listed).
    #[serde(default)]
    pub omit: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Section {
    pub id: String,
    /// Its title in the edition's language, and a note under it (a box without members).
    pub title: String,
    #[serde(default)]
    pub note: Option<String>,
    /// Its passes (entry points) or resources, in the order they run.
    #[serde(default)]
    pub passes: Vec<String>,
    #[serde(default)]
    pub resources: Vec<String>,
    /// Not a pass of the lab's frame (built offline or once, or not ported):
    /// dashed, joined by the edges it declares.
    #[serde(default)]
    pub outside: bool,
    /// What its box shows while lit (`<node>.<resource>`); by default its last
    /// pass's result, or its last resource as the frame leaves it.
    #[serde(default)]
    pub picture: Option<String>,
    #[serde(default)]
    pub to: Vec<String>,
    #[serde(default)]
    pub from: Vec<String>,
}

impl GraphScene {
    /// The sections of a package's `graphs/<name>.toml` (tools/video/frame_graph.py
    /// has the format), `say` giving a text key's text.
    pub fn parse(text: &str, at: &str, say: &dyn Fn(&str) -> Option<String>) -> Result<GraphScene, String> {
        let fail = |m: String| format!("{at}: {m}");
        let document = toml_edit::Document::parse(text.to_string()).map_err(|e| fail(format!("parse: {e}")))?;
        let table = document.as_table();
        let strings = |t: &toml_edit::Table, field: &str| -> Result<Vec<String>, String> {
            match t.get(field) {
                None => Ok(Vec::new()),
                Some(item) => item.as_array().ok_or_else(|| fail(format!("`{field}` is a list of strings")))?.iter().map(|v| v.as_str().map(str::to_string).ok_or_else(|| fail(format!("`{field}` is a list of strings")))).collect(),
            }
        };
        let string = |t: &toml_edit::Table, field: &str| t.get(field).and_then(|v| v.as_str()).map(str::to_string);
        let text = |key: String| say(&key).ok_or_else(|| fail(format!("no text `{key}`")));
        let mut sections = Vec::new();
        for t in table.get("section").and_then(|i| i.as_array_of_tables()).map(|a| a.iter().collect::<Vec<_>>()).unwrap_or_default() {
            let id = string(t, "id").ok_or_else(|| fail("a section needs `id`".into()))?;
            sections.push(Section {
                title: text(string(t, "title-text").ok_or_else(|| fail(format!("{id}: needs `title-text`")))?)?,
                note: string(t, "note-text").map(text).transpose()?,
                passes: strings(t, "passes")?,
                resources: strings(t, "resources")?,
                outside: t.get("outside").and_then(|v| v.as_bool()).unwrap_or(false),
                picture: string(t, "picture"),
                to: strings(t, "to")?,
                from: strings(t, "from")?,
                id,
            });
        }
        Ok(GraphScene { sections, omit: strings(table, "omit")? })
    }

    /// The current investigation's sections `graph/<name>`, in the
    /// interface's language (once per language); none without the resource.
    pub fn current(name: &str) -> Option<&'static GraphScene> {
        use std::sync::{Mutex, OnceLock};
        type Cache = Mutex<Vec<((String, crate::i18n::Locale), Option<&'static GraphScene>)>>;
        static CACHE: OnceLock<Cache> = OnceLock::new();
        let package = crate::investigation::current().package()?;
        let key = (format!("{package}.graph/{name}"), crate::i18n::current());
        let mut cache = CACHE.get_or_init(Default::default).lock().unwrap();
        if let Some((_, scene)) = cache.iter().find(|(k, _)| *k == key) {
            return *scene;
        }
        let scene = crate::resources::find(&key.0).and_then(|r| match GraphScene::parse(r.text(), r.source(), &|k| crate::i18n::try_package_text(package, k).map(str::to_string)) {
            Ok(s) => Some(&*Box::leak(Box::new(s))),
            Err(e) => {
                eprintln!("{e}");
                None
            }
        });
        cache.push((key, scene));
        scene
    }
}

impl FrameGraph {
    /// The frame graph as `scene` groups it, as a graph to draw
    /// (src/graph_view.rs: one box per section, a row per pass by its name
    /// without the investigation's prefix and the stage's suffix, each row's
    /// id its pass or resource; dashed, a section outside the frame): an edge from a section to another
    /// wherever a pass of the first writes what a pass of the second reads
    /// (through the section that holds the resource, if one does), but those
    /// a longer path implies.
    pub fn scene_graph(&self, scene: &GraphScene) -> Result<crate::graph_view::Graph, String> {
        let entry = |n: &str| entry_of(n).to_string();
        let ids: Vec<&str> = scene.sections.iter().map(|s| s.id.as_str()).collect();
        let mut errors = Vec::new();
        let mut section_of = std::collections::HashMap::new();
        let mut holder = std::collections::HashMap::new();
        for s in &scene.sections {
            for p in &s.passes {
                if !self.nodes.iter().any(|n| entry(n.name) == *p) {
                    errors.push(format!("{}: no pass `{p}` in the frame", s.id));
                }
                section_of.insert(p.clone(), s.id.clone());
            }
            for r in &s.resources {
                if self.resource(r).is_none() {
                    errors.push(format!("{}: no resource `{r}`", s.id));
                }
                holder.insert(r.clone(), s.id.clone());
            }
            for other in s.to.iter().chain(&s.from) {
                if !ids.contains(&other.as_str()) {
                    errors.push(format!("{}: no section `{other}`", s.id));
                }
            }
            if let Some(Err(e)) = s.picture.as_deref().map(|p| self.inspected(p)) {
                errors.push(format!("{}: its picture {e}", s.id));
            }
        }
        let missing: Vec<&str> = self.nodes.iter().map(|n| n.name).filter(|n| !section_of.contains_key(&entry(n)) && !["overlay", "composite", "scene"].contains(n) && !scene.omit.contains(&entry(n))).collect();
        if !missing.is_empty() {
            errors.push(format!("passes in no section: {} (or list them in `omit`)", missing.join(", ")));
        }
        if !errors.is_empty() {
            return Err(errors.join("; "));
        }
        let mut edges: Vec<(String, String)> = Vec::new();
        let mut add = |a: Option<&String>, b: Option<&String>| {
            if let (Some(a), Some(b)) = (a, b) && a != b && !edges.iter().any(|e| e.0 == *a && e.1 == *b) {
                edges.push((a.clone(), b.clone()));
            }
        };
        for &(a, b, e) in &self.edges {
            let Edge::Data(r) = e else { continue };
            let (from, to) = (section_of.get(&entry(self.nodes[a].name)), section_of.get(&entry(self.nodes[b].name)));
            match holder.get(self.resources[r].name) {
                Some(h) => {
                    add(from, Some(h));
                    add(Some(h), to);
                }
                None => add(from, to),
            }
        }
        for s in &scene.sections {
            for t in &s.to {
                add(Some(&s.id), Some(t));
            }
            for f in &s.from {
                add(Some(f), Some(&s.id));
            }
        }
        // Only the edges no longer path implies, dropped one at a time (so of
        // two that imply each other around a cycle, one stays).
        let reaches = |edges: &[(String, String)], a: &str, b: &str, skip: usize| {
            let mut seen = BTreeSet::new();
            let mut stack = vec![a.to_string()];
            while let Some(x) = stack.pop() {
                if x == b {
                    return true;
                }
                if seen.insert(x.clone()) {
                    stack.extend(edges.iter().enumerate().filter(|(k, e)| *k != skip && e.0 == x).map(|(_, e)| e.1.clone()));
                }
            }
            false
        };
        let mut kept = edges.clone();
        let mut k = 0;
        while k < kept.len() {
            if reaches(&kept, &kept[k].0, &kept[k].1, k) {
                kept.remove(k);
            } else {
                k += 1;
            }
        }
        kept.sort_by_key(|e| (ids.iter().position(|i| *i == e.0), ids.iter().position(|i| *i == e.1)));
        let prefix = crate::ui::graph_prefix(self.nodes.iter().map(|n| n.name));
        let label = |p: &str| {
            let name = p.strip_prefix(prefix).filter(|r| !r.is_empty()).unwrap_or(p);
            let name = ["CS", "FS", "VS"].iter().find_map(|s| name.strip_suffix(s)).filter(|r| !r.is_empty()).unwrap_or(name);
            name.to_string()
        };
        // A title or note may break its line (a newline in its text).
        let lines = |t: &str| t.split('\n').map(str::to_string).collect::<Vec<_>>();
        let nodes = scene
            .sections
            .iter()
            .map(|s| {
                let mut title = lines(&s.title);
                if s.passes.is_empty() && s.resources.is_empty() {
                    title.extend(s.note.iter().flat_map(|n| lines(n)));
                }
                let rows = s.passes.iter().map(|p| crate::graph_view::Row { id: p.clone(), label: label(p) }).chain(s.resources.iter().map(|r| crate::graph_view::Row { id: r.clone(), label: r.clone() })).collect();
                crate::graph_view::Node { id: s.id.clone(), title, rows, dashed: s.outside, card: None, image: None }
            })
            .collect();
        let index = |id: &str| ids.iter().position(|i| *i == id).expect("a section");
        let edges = kept.iter().map(|(a, b)| crate::graph_view::Edge { from: index(a), to: index(b), dotted: false, label: None }).collect();
        Ok(crate::graph_view::Graph { nodes, edges, clusters: Vec::new() })
    }
}

/// The shader module node `node` is compiled from (src/shaders.rs) and the
/// entry points its pipelines use, with `shading` the frame's (none: the
/// lab's own scene pass): what the frame graph's source view shows.
pub fn source_of(shading: Option<&SceneShading>, node: &Node) -> (&'static str, Vec<&'static str>) {
    let entry = entry_of(node.name);
    match (node.step, shading) {
        (Step::Overlay, _) => (crate::shaders::OVERLAY, vec!["ghostVS", "ghostFS", "segmentVS", "boxVS", "lineFS", "pointVS", "pointFS"]),
        (Step::Composite, _) => (crate::shaders::SCENE, vec!["fullscreenVS", "compositeFS"]),
        (Step::Scene, None) => (crate::shaders::SCENE, vec!["sceneVS", "sceneFS"]),
        (Step::Scene | Step::Prepass, Some(s)) => (s.module, vec!["sceneVS", entry]),
        (Step::DepthViews, Some(s)) => (s.module, s.depth_views.map_or_else(Vec::new, |d| d.vertices.to_vec())),
        (_, Some(s)) => (s.module, vec![entry]),
        (_, None) => (crate::shaders::SCENE, vec![entry]),
    }
}

/// The entry a node's name runs (`name` of a repeated entry's `name#2`).
pub fn entry_of(name: &str) -> &str {
    name.split_once('#').map_or(name, |(entry, _)| entry)
}

/// What node `name` does, in the interface's language: the lab's own nodes'
/// text, else the investigation's (`Investigation::pass_about`); a repeated
/// entry (`name#2`) as its first.
pub fn about(name: &str) -> Option<&'static str> {
    let name = entry_of(name);
    match name {
        "depthViews" => Some(crate::i18n::tr("graph-about_depth_views")),
        "scene" => Some(crate::i18n::tr("graph-about_scene")),
        "overlay" => Some(crate::i18n::tr("graph-about_overlay")),
        "composite" => Some(crate::i18n::tr("graph-about_composite")),
        _ => crate::investigation::current().pass_about(name),
    }
}
