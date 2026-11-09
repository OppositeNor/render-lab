//! The knowledge graph: what understanding an investigation's chapters needs,
//! worked back to three bases, and the audience an investigation is aimed
//! at (the `knowledge-graph` agent skill).
//!
//! A package's `knowledge` resource (a `kind = "file"` TOML file, conventionally
//! `knowledge.toml`) has:
//!
//! - `[[topic]]` tables, the nodes: `key`, `title-text` and `summary-text`
//!   (keys of the package's text tables), `area` (the panel's grouping), the
//!   `requires` list, optional `reading` (published references), optional
//!   `group` (a group of its area it belongs to), and `base = true` for the
//!   three bases (render_lab's alone);
//! - `[[group]]` tables, subsections of an area that gather related topics
//!   (the panel boxes them): `key`, `title-text`, `area`;
//! - `[[chapter]]` tables, an investigation's chapters: `number`, `demos` (the
//!   demonstrations that show it), and `requires`, the nodes it uses directly;
//! - an `[audience]` table: `profile` (a note, not shown) and `known`, the
//!   nodes the audience knows.
//!
//! A requirement is a node of the same file (`geo-sdf`), or of a package it
//! imports whose `knowledge` it exports (`render_lab.knowledge#geo-sdf`). The graph must
//! be acyclic, its only sources the bases, and transitively reduced (an edge
//! another path already implies is an error), so that "directly requires"
//! means an edge.
//!
//! The audience is a separator of the graph. With the chapters as sinks and
//! the graph restricted to their ancestors, the known set A is `known` with
//! everything it requires (and the bases). The level S is the nodes outside A
//! whose requirements are all in A: an antichain. An edge from A to a node
//! beyond the level is a carry: a known topic used directly above the level.
//! In the graph with every level-skipping edge subdivided, the level and the
//! carries together are an antichain vertex separator between A and the rest
//! (the proof is in the `knowledge-graph` agent skill). The rest is what the story teaches.

use crate::packages::{self, Package};
use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

/// The panel's groupings, bottom to top.
pub const AREAS: &[&str] = &["gpu", "rt", "geo", "cg", "light", "gi"];

/// One node of the merged graph.
#[derive(Clone, Debug)]
pub struct Node {
    /// Its full name, `<package>.knowledge#<key>`.
    pub id: String,
    pub package: String,
    pub key: String,
    pub title: String,
    pub summary: String,
    pub area: String,
    pub base: bool,
    /// The nodes it requires directly (indices).
    pub requires: Vec<usize>,
    pub reading: Vec<String>,
    /// The group it belongs to (`Graph::groups`).
    pub group: Option<usize>,
}

/// A subsection of an area: related topics the panel draws in a box.
#[derive(Clone, Debug)]
pub struct Group {
    /// Its full name, `<package>.knowledge#<key>`.
    pub id: String,
    pub key: String,
    pub title: String,
    pub area: String,
}

/// One chapter of an investigation.
#[derive(Clone, Debug)]
pub struct Chapter {
    pub number: u32,
    pub demos: Vec<String>,
    pub requires: Vec<usize>,
}

/// An investigation package's merged graph: its nodes and those of the
/// packages it imports, its chapters, and its audience.
#[derive(Clone, Debug, Default)]
pub struct Graph {
    pub package: String,
    pub nodes: Vec<Node>,
    pub groups: Vec<Group>,
    pub chapters: Vec<Chapter>,
    pub profile: Option<String>,
    /// The audience's known nodes, as written (without their closure).
    pub known: Vec<usize>,
}

/// The audience split of a graph's chapter subgraph.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Partition {
    /// Nodes some chapter needs (a chapter's requirement or its ancestor).
    pub relevant: Vec<bool>,
    /// A: the known set.
    pub known: Vec<bool>,
    /// S: the level (relevant nodes outside A whose requirements are all in A).
    pub level: Vec<usize>,
    /// Beyond the level: what the story teaches.
    pub teach: Vec<usize>,
    /// Edges from a known node to a node beyond the level, or to a chapter.
    pub carries: Vec<(usize, Carry)>,
    /// Chapters whose requirements are all known.
    pub known_chapters: Vec<u32>,
}

/// Where a carry goes: a node beyond the level, or a chapter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Carry {
    Node(usize),
    Chapter(u32),
}

impl Partition {
    pub fn is_known(&self, i: usize) -> bool {
        self.known.get(i).copied().unwrap_or(false)
    }

    /// The known nodes with a carry: used directly above the level.
    pub fn carried(&self) -> Vec<usize> {
        let mut list: Vec<usize> = self.carries.iter().map(|c| c.0).collect();
        list.sort();
        list.dedup();
        list
    }
}

// The graph's mathematics, on requirement lists alone (unit-tested).

/// Every ancestor of `start` (not `start` itself unless on a cycle).
pub fn ancestors(requires: &[Vec<usize>], start: &[usize]) -> Vec<bool> {
    let mut seen = vec![false; requires.len()];
    let mut stack: Vec<usize> = start
        .iter()
        .flat_map(|&s| requires[s].iter().copied())
        .collect();
    while let Some(n) = stack.pop() {
        if !seen[n] {
            seen[n] = true;
            stack.extend(requires[n].iter().copied());
        }
    }
    seen
}

/// A cycle's nodes, if there is one.
pub fn cycle(requires: &[Vec<usize>]) -> Option<Vec<usize>> {
    // 0 unvisited, 1 on the stack, 2 done.
    fn visit(
        n: usize,
        requires: &[Vec<usize>],
        state: &mut [u8],
        path: &mut Vec<usize>,
    ) -> Option<Vec<usize>> {
        match state[n] {
            2 => return None,
            1 => return Some(path[path.iter().position(|&p| p == n).unwrap_or(0)..].to_vec()),
            _ => {}
        }
        state[n] = 1;
        path.push(n);
        for &r in &requires[n] {
            if let Some(c) = visit(r, requires, state, path) {
                return Some(c);
            }
        }
        path.pop();
        state[n] = 2;
        None
    }
    let mut state = vec![0u8; requires.len()];
    (0..requires.len()).find_map(|n| visit(n, requires, &mut state, &mut Vec::new()))
}

/// The redundant edges: `(node, requirement)` where another requirement of
/// the node already requires it (directly or not), or it is listed twice.
pub fn redundant(requires: &[Vec<usize>]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    for (n, reqs) in requires.iter().enumerate() {
        for (k, &r) in reqs.iter().enumerate() {
            let others: Vec<usize> = reqs
                .iter()
                .enumerate()
                .filter(|&(j, _)| j != k)
                .map(|(_, &o)| o)
                .collect();
            if others.contains(&r) || ancestors(requires, &others)[r] {
                out.push((n, r));
            }
        }
    }
    out
}

/// The audience split: `requires` the nodes' requirements, `bases` the
/// bases, `chapters` each chapter's number and requirements, `known` the
/// audience's nodes (closed under requirements here).
pub fn partition(
    requires: &[Vec<usize>],
    bases: &[usize],
    chapters: &[(u32, Vec<usize>)],
    known: &[usize],
) -> Partition {
    let n = requires.len();
    let needed: Vec<usize> = chapters.iter().flat_map(|c| c.1.iter().copied()).collect();
    let relevant = context(requires, &needed);
    let closure = ancestors(requires, known);
    let a: Vec<bool> = (0..n)
        .map(|i| relevant[i] && (closure[i] || known.contains(&i) || bases.contains(&i)))
        .collect();
    let level: Vec<usize> = (0..n)
        .filter(|&i| relevant[i] && !a[i] && requires[i].iter().all(|&r| a[r]))
        .collect();
    let teach: Vec<usize> = (0..n)
        .filter(|&i| relevant[i] && !a[i] && !level.contains(&i))
        .collect();
    let mut carries = Vec::new();
    for &b in &teach {
        for &r in &requires[b] {
            if a[r] {
                carries.push((r, Carry::Node(b)));
            }
        }
    }
    let mut known_chapters = Vec::new();
    for (number, reqs) in chapters {
        // A chapter all of whose requirements are known sits at the level itself.
        if reqs.iter().all(|&r| a[r]) {
            known_chapters.push(*number);
            continue;
        }
        for &r in reqs {
            if a[r] {
                carries.push((r, Carry::Chapter(*number)));
            }
        }
    }
    Partition {
        relevant,
        known: a,
        level,
        teach,
        carries,
        known_chapters,
    }
}

/// The nodes a context needs: the given requirements and their ancestors.
pub fn context(requires: &[Vec<usize>], needs: &[usize]) -> Vec<bool> {
    let mut seen = ancestors(requires, needs);
    for &r in needs {
        seen[r] = true;
    }
    seen
}

/// A nearby audience: the node added (`added`) or removed, its known list,
/// and its partition's carry count.
pub struct Neighbour {
    pub added: bool,
    pub node: usize,
    pub known: Vec<usize>,
    pub carries: usize,
}

/// Nearby audiences: the known list with one level node added, or one
/// maximal known node removed, fewest carries first.
pub fn neighbours(
    requires: &[Vec<usize>],
    bases: &[usize],
    chapters: &[(u32, Vec<usize>)],
    known: &[usize],
) -> Vec<Neighbour> {
    let p = partition(requires, bases, chapters, known);
    let n = requires.len();
    let mut out = Vec::new();
    for &s in &p.level {
        let mut k = known.to_vec();
        k.push(s);
        let carries = partition(requires, bases, chapters, &k).carries.len();
        out.push(Neighbour {
            added: true,
            node: s,
            known: k,
            carries,
        });
    }
    // A maximal known node: known, not a base, and no known node requires it.
    for i in 0..n {
        if !p.known[i]
            || bases.contains(&i)
            || (0..n).any(|j| p.known[j] && requires[j].contains(&i))
        {
            continue;
        }
        let k: Vec<usize> = (0..n)
            .filter(|&j| p.known[j] && j != i && !bases.contains(&j))
            .collect();
        let carries = partition(requires, bases, chapters, &k).carries.len();
        out.push(Neighbour {
            added: false,
            node: i,
            known: k,
            carries,
        });
    }
    out.sort_by_key(|o| o.carries);
    out
}

// Reading a package's graph.

/// One file's records, before the merge resolves their references.
struct RawNode {
    key: String,
    title: String,
    summary: String,
    area: String,
    base: bool,
    requires: Vec<String>,
    reading: Vec<String>,
    group: Option<String>,
}

struct RawFile {
    nodes: Vec<RawNode>,
    /// key, title text key, area.
    groups: Vec<(String, String, String)>,
    chapters: Vec<(u32, Vec<String>, Vec<String>)>,
    profile: Option<String>,
    known: Vec<String>,
}

fn strings(
    table: &toml_edit::Table,
    field: &str,
    fail: &dyn Fn(String) -> String,
) -> Result<Vec<String>, String> {
    match table.get(field) {
        None => Ok(Vec::new()),
        Some(item) => {
            let list = item
                .as_array()
                .ok_or_else(|| fail(format!("`{field}` is a list of strings")))?;
            list.iter()
                .map(|v| {
                    v.as_str()
                        .map(String::from)
                        .ok_or_else(|| fail(format!("`{field}` is a list of strings")))
                })
                .collect()
        }
    }
}

fn string(
    table: &toml_edit::Table,
    field: &str,
    fail: &dyn Fn(String) -> String,
) -> Result<Option<String>, String> {
    match table.get(field) {
        None => Ok(None),
        Some(item) => item
            .as_str()
            .map(|s| Some(s.to_string()))
            .ok_or_else(|| fail(format!("`{field}` is a string"))),
    }
}

fn parse(text: &str, at: &str) -> Result<RawFile, String> {
    let fail = |m: String| format!("knowledge: {at}: {m}");
    let document =
        toml_edit::Document::parse(text.to_string()).map_err(|e| fail(format!("parse: {e}")))?;
    let table = document.as_table();
    if let Some((key, _)) = table
        .iter()
        .find(|(k, _)| !["topic", "group", "chapter", "audience"].contains(k))
    {
        return Err(fail(format!(
            "unknown key `{key}` (a knowledge file has topic, group, chapter, audience)"
        )));
    }
    let tables = |name: &str| -> Result<Vec<&toml_edit::Table>, String> {
        match table.get(name) {
            None => Ok(Vec::new()),
            Some(item) => Ok(item
                .as_array_of_tables()
                .ok_or_else(|| fail(format!("`{name}` is a list of `[[{name}]]` tables")))?
                .iter()
                .collect()),
        }
    };
    let mut nodes = Vec::new();
    for t in tables("topic")? {
        nodes.push(parse_topic(t, &fail)?);
    }
    let mut groups = Vec::new();
    for t in tables("group")? {
        groups.push(parse_group(t, &fail)?);
    }
    let mut chapters = Vec::new();
    for t in tables("chapter")? {
        if let Some((key, _)) = t
            .iter()
            .find(|(k, _)| !["number", "demos", "requires"].contains(k))
        {
            return Err(fail(format!(
                "a chapter has no field `{key}` (it has number, demos, requires)"
            )));
        }
        let number = t
            .get("number")
            .and_then(|n| n.as_integer())
            .filter(|n| *n > 0)
            .ok_or_else(|| fail("a chapter needs `number`, a positive integer".into()))?
            as u32;
        let f = |m: String| fail(format!("chapter {number}: {m}"));
        chapters.push((
            number,
            strings(t, "demos", &f)?,
            strings(t, "requires", &f)?,
        ));
    }
    let (profile, known) = match table.get("audience") {
        None => (None, Vec::new()),
        Some(item) => {
            let t = item
                .as_table()
                .ok_or_else(|| fail("`audience` is a table".into()))?;
            if let Some((key, _)) = t.iter().find(|(k, _)| !["profile", "known"].contains(k)) {
                return Err(fail(format!(
                    "the audience has no field `{key}` (it has profile, known)"
                )));
            }
            (string(t, "profile", &fail)?, strings(t, "known", &fail)?)
        }
    };
    Ok(RawFile {
        nodes,
        groups,
        chapters,
        profile,
        known,
    })
}

/// A `[[topic]]` of a knowledge file, its fields checked.
fn parse_topic(t: &toml_edit::Table, fail: &dyn Fn(String) -> String) -> Result<RawNode, String> {
    const FIELDS: &[&str] = &[
        "key",
        "title-text",
        "summary-text",
        "area",
        "base",
        "requires",
        "reading",
        "group",
    ];
    if let Some((key, _)) = t.iter().find(|(k, _)| !FIELDS.contains(k)) {
        return Err(fail(format!(
            "a topic has no field `{key}` (it has {})",
            FIELDS.join(", ")
        )));
    }
    let key = string(t, "key", &fail)?.ok_or_else(|| fail("a topic needs `key`".into()))?;
    let f = |m: String| fail(format!("topic `{key}`: {m}"));
    if key.is_empty()
        || !key
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err(f("a key is lower-case letters, digits, and `-`".into()));
    }
    let area = string(t, "area", &f)?.ok_or_else(|| f("needs `area`".into()))?;
    if !AREAS.contains(&area.as_str()) {
        return Err(f(format!(
            "area `{area}` is not one of {}",
            AREAS.join(", ")
        )));
    }
    Ok(RawNode {
        title: string(t, "title-text", &f)?.ok_or_else(|| f("needs `title-text`".into()))?,
        summary: string(t, "summary-text", &f)?.ok_or_else(|| f("needs `summary-text`".into()))?,
        area,
        base: t
            .get("base")
            .map(|b| {
                b.as_bool()
                    .ok_or_else(|| f("`base` is true or false".into()))
            })
            .transpose()?
            .unwrap_or(false),
        requires: strings(t, "requires", &f)?,
        reading: strings(t, "reading", &f)?,
        group: string(t, "group", &f)?,
        key,
    })
}

/// A `[[group]]` of a knowledge file: its key, its title's text key, its area.
fn parse_group(
    t: &toml_edit::Table,
    fail: &dyn Fn(String) -> String,
) -> Result<(String, String, String), String> {
    if let Some((key, _)) = t
        .iter()
        .find(|(k, _)| !["key", "title-text", "area"].contains(k))
    {
        return Err(fail(format!(
            "a group has no field `{key}` (it has key, title-text, area)"
        )));
    }
    let key = string(t, "key", &fail)?.ok_or_else(|| fail("a group needs `key`".into()))?;
    let f = |m: String| fail(format!("group `{key}`: {m}"));
    let area = string(t, "area", &f)?.ok_or_else(|| f("needs `area`".into()))?;
    if !AREAS.contains(&area.as_str()) {
        return Err(f(format!(
            "area `{area}` is not one of {}",
            AREAS.join(", ")
        )));
    }
    Ok((
        key.clone(),
        string(t, "title-text", &f)?.ok_or_else(|| f("needs `title-text`".into()))?,
        area,
    ))
}

/// The full node name a reference in `package`'s topics names.
fn resolve(package: &Package, imported: &[Package], reference: &str) -> Result<String, String> {
    match reference.split_once('#') {
        None => Ok(format!("{}.knowledge#{reference}", package.name)),
        Some((resource, key)) => {
            let full = packages::resolve(package, imported, resource)?;
            if !full.ends_with(".knowledge") {
                return Err(format!(
                    "`{reference}` names `{full}`, not a package's `knowledge`"
                ));
            }
            Ok(format!("{full}#{key}"))
        }
    }
}

/// Merges parsed files (each with its package, in import order) into the
/// graph of `package`, checking it. `text` turns a package's text key into text.
fn merge(
    package: &str,
    files: Vec<(&Package, RawFile)>,
    all: &[Package],
    text: &dyn Fn(&str, &str) -> String,
) -> Result<Graph, String> {
    let mut graph = Graph {
        package: package.to_string(),
        ..Default::default()
    };
    let (index, group_index) = index_topics(&mut graph, &files, text)?;
    let mut start = 0;
    for (p, file) in &files {
        let at = all
            .iter()
            .position(|o| o.name == p.name)
            .unwrap_or(all.len());
        for (i, node) in file.nodes.iter().enumerate() {
            let Some(reference) = &node.group else {
                continue;
            };
            let id = resolve(p, &all[..at], reference)
                .map_err(|e| format!("knowledge: {}: {e}", p.name))?;
            let g = *group_index
                .get(&id)
                .ok_or_else(|| format!("knowledge: {}: `{}`: no group `{id}`", p.name, node.key))?;
            if graph.groups[g].area != node.area {
                return Err(format!(
                    "knowledge: {}: `{}` is in area {}, its group `{}` in {}",
                    p.name, node.key, node.area, graph.groups[g].key, graph.groups[g].area
                ));
            }
            graph.nodes[start + i].group = Some(g);
        }
        start += file.nodes.len();
    }
    let lookup = |p: &Package, reference: &str| -> Result<usize, String> {
        let at = all
            .iter()
            .position(|o| o.name == p.name)
            .unwrap_or(all.len());
        let id =
            resolve(p, &all[..at], reference).map_err(|e| format!("knowledge: {}: {e}", p.name))?;
        index
            .get(&id)
            .copied()
            .ok_or_else(|| format!("knowledge: {}: no topic `{id}`", p.name))
    };
    let mut start = 0;
    for (p, file) in &files {
        for (i, node) in file.nodes.iter().enumerate() {
            let requires = node
                .requires
                .iter()
                .map(|r| lookup(p, r))
                .collect::<Result<Vec<_>, _>>()?;
            if node.base != requires.is_empty() {
                let why = if node.base {
                    "a base requires nothing"
                } else {
                    "a topic that is not a base requires something"
                };
                return Err(format!("knowledge: {}: `{}`: {why}", p.name, node.key));
            }
            graph.nodes[start + i].requires = requires;
        }
        start += file.nodes.len();
        // An imported package's chapters and audience are its own investigation's.
        if p.name == package {
            for (number, demos, requires) in &file.chapters {
                if graph.chapters.iter().any(|c| c.number == *number) {
                    return Err(format!(
                        "knowledge: {package}: chapter {number} is declared twice"
                    ));
                }
                if requires.is_empty() {
                    return Err(format!(
                        "knowledge: {package}: chapter {number} requires nothing (leave a recap chapter out)"
                    ));
                }
                let requires = requires
                    .iter()
                    .map(|r| lookup(p, r))
                    .collect::<Result<Vec<_>, _>>()?;
                graph.chapters.push(Chapter {
                    number: *number,
                    demos: demos.clone(),
                    requires,
                });
            }
            graph.profile = file.profile.clone();
            graph.known = file
                .known
                .iter()
                .map(|r| lookup(p, r))
                .collect::<Result<Vec<_>, _>>()?;
        }
    }
    graph.chapters.sort_by_key(|c| c.number);
    check_graph(&graph, package)?;
    Ok(graph)
}

/// The files' topics and groups added to `graph` (each declared once; only render_lab declares the bases):
/// Where each topic (or each group) is in the graph, by its full name.
type Places = BTreeMap<String, usize>;

/// where each topic is, and where each group is, by full name.
fn index_topics(
    graph: &mut Graph,
    files: &[(&Package, RawFile)],
    text: &dyn Fn(&str, &str) -> String,
) -> Result<(Places, Places), String> {
    let mut index: BTreeMap<String, usize> = BTreeMap::new();
    for (p, file) in files {
        for node in &file.nodes {
            let id = format!("{}.knowledge#{}", p.name, node.key);
            if index.contains_key(&id) {
                return Err(format!(
                    "knowledge: {}: the topic `{}` is declared twice",
                    p.name, node.key
                ));
            }
            if node.base && p.name != "render_lab" {
                return Err(format!(
                    "knowledge: {}: `{}` is a base; only render_lab declares the bases",
                    p.name, node.key
                ));
            }
            index.insert(id.clone(), graph.nodes.len());
            graph.nodes.push(Node {
                id,
                package: p.name.clone(),
                key: node.key.clone(),
                title: text(&p.name, &node.title),
                summary: text(&p.name, &node.summary),
                area: node.area.clone(),
                base: node.base,
                requires: Vec::new(),
                reading: node.reading.clone(),
                group: None,
            });
        }
    }
    let mut group_index: BTreeMap<String, usize> = BTreeMap::new();
    for (p, file) in files {
        for (key, title, area) in &file.groups {
            let id = format!("{}.knowledge#{}", p.name, key);
            if group_index.contains_key(&id) || index.contains_key(&id) {
                return Err(format!(
                    "knowledge: {}: `{key}` is declared twice (a group's key is not a topic's)",
                    p.name
                ));
            }
            group_index.insert(id.clone(), graph.groups.len());
            graph.groups.push(Group {
                id,
                key: key.clone(),
                title: text(&p.name, title),
                area: area.clone(),
            });
        }
    }
    Ok((index, group_index))
}

/// Refuses a merged graph whose requirements form a cycle or repeat what another requirement implies, whose
/// bases are not three, or a chapter requirement another one implies.
fn check_graph(graph: &Graph, package: &str) -> Result<(), String> {
    let requires = graph.requires();
    if let Some(c) = cycle(&requires) {
        return Err(format!(
            "knowledge: {package}: the requirements form a cycle: {}",
            c.iter()
                .map(|&i| graph.nodes[i].id.as_str())
                .collect::<Vec<_>>()
                .join(" → ")
        ));
    }
    if let Some((n, r)) = redundant(&requires).first() {
        return Err(format!(
            "knowledge: {package}: `{}` requires `{}`, which another of its requirements already does (remove it)",
            graph.nodes[*n].id, graph.nodes[*r].id
        ));
    }
    let bases = graph.bases();
    if graph.nodes.iter().any(|n| n.package == "render_lab") && bases.len() != 3 {
        return Err(format!(
            "knowledge: render_lab declares {} bases (it declares three)",
            bases.len()
        ));
    }
    for chapter in &graph.chapters {
        let mut with = requires.clone();
        with.push(chapter.requires.clone());
        if let Some((_, r)) = redundant(&with)
            .into_iter()
            .find(|(n, _)| *n == with.len() - 1)
        {
            return Err(format!(
                "knowledge: {package}: chapter {} requires `{}`, which another of its requirements already does (remove it)",
                chapter.number, graph.nodes[r].id
            ));
        }
    }
    Ok(())
}

impl Graph {
    pub fn requires(&self) -> Vec<Vec<usize>> {
        self.nodes.iter().map(|n| n.requires.clone()).collect()
    }

    pub fn bases(&self) -> Vec<usize> {
        (0..self.nodes.len())
            .filter(|&i| self.nodes[i].base)
            .collect()
    }

    pub fn chapter_needs(&self) -> Vec<(u32, Vec<usize>)> {
        self.chapters
            .iter()
            .map(|c| (c.number, c.requires.clone()))
            .collect()
    }

    /// The node `name`: a full name, or a key (of the graph's package, else of any).
    pub fn find(&self, name: &str) -> Option<usize> {
        let nodes = &self.nodes;
        nodes
            .iter()
            .position(|n| n.id == name)
            .or_else(|| {
                nodes
                    .iter()
                    .position(|n| n.key == name && n.package == self.package)
            })
            .or_else(|| nodes.iter().position(|n| n.key == name))
    }

    /// The stored audience's split.
    pub fn audience(&self) -> Partition {
        self.partition_for(&self.known)
    }

    pub fn partition_for(&self, known: &[usize]) -> Partition {
        partition(
            &self.requires(),
            &self.bases(),
            &self.chapter_needs(),
            known,
        )
    }

    /// What the given chapters need (none given: every chapter).
    pub fn context(&self, chapters: &[u32]) -> Vec<bool> {
        let needs: Vec<usize> = self
            .chapters
            .iter()
            .filter(|c| chapters.is_empty() || chapters.contains(&c.number))
            .flat_map(|c| c.requires.iter().copied())
            .collect();
        context(&self.requires(), &needs)
    }

    /// Each node's rank: one more than its highest requirement's (the bases 0).
    pub fn ranks(&self) -> Vec<usize> {
        let requires = self.requires();
        let mut rank = vec![0usize; requires.len()];
        for i in topological(&requires) {
            rank[i] = requires[i].iter().map(|&r| rank[r] + 1).max().unwrap_or(0);
        }
        rank
    }

    /// The chapters a demonstration shows.
    pub fn chapters_of_demo(&self, demo: &str) -> Vec<u32> {
        self.chapters
            .iter()
            .filter(|c| c.demos.iter().any(|d| d == demo))
            .map(|c| c.number)
            .collect()
    }

    /// The chapters that use a node (it is one of their needs or an ancestor of one).
    pub fn chapters_using(&self, node: usize) -> Vec<u32> {
        let requires = self.requires();
        self.chapters
            .iter()
            .filter(|c| context(&requires, &c.requires)[node])
            .map(|c| c.number)
            .collect()
    }

    /// Problems with the audience: missing, or leaving a chapter nothing to
    /// teach, or naming a node no chapter needs. `demos`: the investigation's
    /// demonstration keys, which the chapters' `demos` must name.
    pub fn check(&self, demos: &[&str]) -> Vec<String> {
        let mut problems = Vec::new();
        for c in &self.chapters {
            for d in &c.demos {
                if !demos.is_empty() && !demos.contains(&d.as_str()) {
                    problems.push(format!(
                        "{}: chapter {}: no demonstration `{d}`",
                        self.package, c.number
                    ));
                }
            }
        }
        if self.chapters.is_empty() {
            return problems;
        }
        if self.known.is_empty() {
            problems.push(format!(
                "{}: no `[audience] known` (the knowledge-graph agent skill chooses it)",
                self.package
            ));
            return problems;
        }
        let p = self.audience();
        for number in &p.known_chapters {
            problems.push(format!(
                "{}: chapter {number} needs nothing beyond what the audience knows",
                self.package
            ));
        }
        for &k in &self.known {
            if !p.relevant[k] {
                problems.push(format!(
                    "{}: the audience's `{}` is not needed by any chapter",
                    self.package, self.nodes[k].id
                ));
            }
        }
        problems
    }

    /// The panel's graph (src/graph_view.rs) and its layout: ranks (columns,
    /// left to right) by the longest path from the bases, each group in a
    /// band of its own across the columns it spans (a boxed cluster), an
    /// area's loose topics together, the areas in order, top to bottom.
    /// `width` measures a title in font units. With `visible`, only those
    /// nodes (a compact view of a set closed under requirements, such as what
    /// some chapters need).
    pub fn view(&self, width: &dyn Fn(&str) -> f32, visible: Option<&[bool]>) -> View {
        let all = self.nodes.len();
        let shown: Vec<usize> = (0..all)
            .filter(|&i| visible.is_none_or(|v| v.get(i).copied().unwrap_or(false)))
            .collect();
        let mut local = vec![None; all];
        for (k, &i) in shown.iter().enumerate() {
            local[i] = Some(k);
        }
        let requires = self.requires();
        let edges: Vec<crate::graph_view::Edge> = shown
            .iter()
            .flat_map(|&i| {
                requires[i]
                    .iter()
                    .filter_map(|&r| {
                        Some(crate::graph_view::Edge {
                            from: local[r]?,
                            to: local[i]?,
                            dotted: false,
                            label: None,
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        let nodes: Vec<crate::graph_view::Node> = shown
            .iter()
            .map(|&i| crate::graph_view::Node {
                id: self.nodes[i].id.clone(),
                title: vec![self.nodes[i].title.clone()],
                ..Default::default()
            })
            .collect();
        let node = |k: usize| &self.nodes[shown[k]];
        let mut clusters: Vec<crate::graph_view::Cluster> = Vec::new();
        let mut groups: Vec<Option<usize>> = Vec::new();
        let mut areas: Vec<String> = AREAS.iter().map(|a| a.to_string()).collect();
        for k in 0..shown.len() {
            if !areas.contains(&node(k).area) {
                areas.push(node(k).area.clone());
            }
        }
        for area in areas {
            let loose: Vec<usize> = (0..shown.len())
                .filter(|&k| node(k).area == area && node(k).group.is_none())
                .collect();
            if !loose.is_empty() {
                clusters.push(crate::graph_view::Cluster {
                    title: None,
                    members: loose,
                    boxed: false,
                });
                groups.push(None);
            }
            for (g, group) in self
                .groups
                .iter()
                .enumerate()
                .filter(|(_, g)| g.area == area)
            {
                let members: Vec<usize> = (0..shown.len())
                    .filter(|&k| node(k).group == Some(g))
                    .collect();
                if !members.is_empty() {
                    clusters.push(crate::graph_view::Cluster {
                        title: Some(group.title.clone()),
                        members,
                        boxed: true,
                    });
                    groups.push(Some(g));
                }
            }
        }
        let graph = crate::graph_view::Graph {
            nodes,
            edges,
            clusters,
        };
        let options = crate::graph_view::Options {
            flow: crate::graph_view::Flow::Right,
            spacing: Some(crate::layered::Spacing {
                column: 4.8,
                row: 1.0,
                dummy: 0.7,
            }),
            pad: 0.7,
            title: 1.5,
            gap: 1.8,
        };
        let laid = crate::graph_view::lay_out(&graph, width, &[], 0.5625, &options);
        View {
            graph,
            laid,
            shown,
            local,
            groups,
        }
    }
}

/// The knowledge panel's graph (`Graph::view`): its nodes are the shown
/// topics (`shown[k]` is node k's topic; `local` the other way), its clusters
/// the areas' loose topics and the groups (`groups`, by cluster).
pub struct View {
    pub graph: crate::graph_view::Graph,
    pub laid: crate::graph_view::Laid,
    pub shown: Vec<usize>,
    pub local: Vec<Option<usize>>,
    pub groups: Vec<Option<usize>>,
}

/// The nodes in an order where every node comes after its requirements.
fn topological(requires: &[Vec<usize>]) -> Vec<usize> {
    fn visit(i: usize, requires: &[Vec<usize>], done: &mut [bool], out: &mut Vec<usize>) {
        if done[i] {
            return;
        }
        done[i] = true;
        for &r in &requires[i] {
            visit(r, requires, done, out);
        }
        out.push(i);
    }
    let mut done = vec![false; requires.len()];
    let mut out = Vec::new();
    for i in 0..requires.len() {
        visit(i, requires, &mut done, &mut out);
    }
    out
}

/// Reads the merged graph of `package` from the built packages: its own
/// `knowledge` and those of the packages it imports (directly or not). `None`
/// when the package has no `knowledge` of its own (no chapters to be about).
pub fn load(package: &str) -> Option<Result<Graph, String>> {
    let all = crate::resources::packages();
    let at = all.iter().position(|p| p.name == package)?;
    crate::resources::find(&format!("{package}.knowledge"))?;
    let mut wanted = vec![package.to_string()];
    let mut k = 0;
    while k < wanted.len() {
        if let Some(p) = all.iter().find(|p| p.name == wanted[k]) {
            for i in &p.imports {
                if !wanted.contains(i) {
                    wanted.push(i.clone());
                }
            }
        }
        k += 1;
    }
    let mut files = Vec::new();
    for p in all[..=at].iter().filter(|p| wanted.contains(&p.name)) {
        let Some(resource) = crate::resources::find(&format!("{}.knowledge", p.name)) else {
            continue;
        };
        match parse(resource.text(), resource.source()) {
            Ok(file) => files.push((p, file)),
            Err(e) => return Some(Err(e)),
        }
    }
    if files.is_empty() {
        return None;
    }
    Some(merge(package, files, all, &|p, key| {
        crate::i18n::package_text(p, key).to_string()
    }))
}

/// The current investigation's graph, read once per language (its titles
/// are translated). An error is printed once and leaves no graph.
pub fn current() -> Option<&'static Graph> {
    /// By package and language.
    type Cache = BTreeMap<(String, crate::i18n::Locale), Option<&'static Graph>>;
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    let package = crate::investigation::current().package()?;
    let key = (package.to_string(), crate::i18n::current());
    let mut cache = CACHE.get_or_init(Default::default).lock().unwrap();
    *cache.entry(key).or_insert_with(|| match load(package) {
        Some(Ok(g)) => Some(Box::leak(Box::new(g))),
        Some(Err(e)) => {
            eprintln!("{e}");
            None
        }
        None => None,
    })
}

/// The current graph and its panel's view, computed once per graph.
/// With `visible`, the compact view of those nodes alone (kept for the
/// last few sets).
pub fn current_view(
    width: &dyn Fn(&str) -> f32,
    visible: Option<&[bool]>,
) -> Option<(&'static Graph, &'static View)> {
    type Key = (usize, Option<Vec<bool>>);
    static CACHE: OnceLock<Mutex<Vec<(Key, &'static View)>>> = OnceLock::new();
    let graph = current()?;
    let key: Key = (graph as *const Graph as usize, visible.map(|v| v.to_vec()));
    let mut cache = CACHE.get_or_init(Default::default).lock().unwrap();
    if let Some((_, l)) = cache.iter().find(|(k, _)| *k == key) {
        return Some((graph, l));
    }
    // A layout is small; the compact ones are few (one per set of chapters).
    let l: &'static View = Box::leak(Box::new(graph.view(width, visible)));
    cache.push((key, l));
    Some((graph, l))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 0, 1 bases; 2 ← 0; 3 ← 2; 4 ← 1; 5 ← 3, 4; 6 ← 0; 7 ← 5, 6 (a skip edge from 6).
    fn sample() -> Vec<Vec<usize>> {
        vec![
            vec![],
            vec![],
            vec![0],
            vec![2],
            vec![1],
            vec![3, 4],
            vec![0],
            vec![5, 6],
        ]
    }

    #[test]
    fn cycles_and_redundant_edges() {
        assert!(cycle(&sample()).is_none());
        assert!(cycle(&[vec![1], vec![0]]).is_some());
        assert!(redundant(&sample()).is_empty());
        // 3 ← 0 is implied by 3 ← 2 ← 0.
        let mut g = sample();
        g[3].push(0);
        assert_eq!(redundant(&g), vec![(3, 0)]);
    }

    #[test]
    fn partition_of_a_skip_edge() {
        let g = sample();
        let p = partition(&g, &[0, 1], &[(1, vec![7])], &[2]);
        // Known: 0, 1, 2; the level: 3, 4, 6 (requirements all known); taught: 5, 7.
        assert_eq!(
            (0..8).filter(|&i| p.known[i]).collect::<Vec<_>>(),
            [0, 1, 2]
        );
        assert_eq!(p.level, [3, 4, 6]);
        assert_eq!(p.teach, [5, 7]);
        assert!(p.carries.is_empty());
        // With 3 known, 5 has one requirement known (3) and one not (4): a carry.
        let p = partition(&g, &[0, 1], &[(1, vec![7])], &[3]);
        assert_eq!(p.level, [4, 6]);
        assert_eq!(p.carries, [(3, Carry::Node(5))]);
    }

    /// For every known set, the level is an antichain, and in the subdivided
    /// graph (one carry vertex per edge from a known node to one beyond the
    /// level) the level and the carries separate the known set from the
    /// rest: no other edge joins them.
    #[test]
    fn level_and_carries_separate() {
        let g = sample();
        let n = g.len();
        let anc: Vec<Vec<bool>> = (0..n).map(|i| ancestors(&g, &[i])).collect();
        for mask in 0u32..(1 << n) {
            let known: Vec<usize> = (0..n).filter(|&i| mask & (1 << i) != 0).collect();
            let p = partition(&g, &[0, 1], &[(1, vec![7])], &known);
            for &a in &p.level {
                for &b in &p.level {
                    assert!(
                        a == b || !anc[a][b],
                        "level not an antichain: {a} requires {b}"
                    );
                }
            }
            for (i, reqs) in g.iter().enumerate() {
                for &r in reqs {
                    if p.known[r] && p.teach.contains(&i) {
                        assert!(
                            p.carries.contains(&(r, Carry::Node(i))),
                            "{r} → {i} crosses without a carry"
                        );
                    }
                    assert!(
                        !(p.known[i] && !p.known[r]),
                        "known {i} requires unknown {r}"
                    );
                }
            }
        }
    }

    #[test]
    fn layout_places_every_node() {
        let mut graph = Graph::default();
        for (i, reqs) in sample().into_iter().enumerate() {
            graph.nodes.push(Node {
                id: format!("t#{i}"),
                package: "t".into(),
                key: i.to_string(),
                title: format!("node {i}"),
                summary: String::new(),
                area: "gpu".into(),
                base: reqs.is_empty(),
                requires: reqs,
                reading: Vec::new(),
                group: None,
            });
        }
        // Two groups: 2, 3 (ranks 1-2) and 5, 7 (ranks 3-4); 4 and 6 loose.
        for (key, members) in [("a", [2, 3]), ("b", [5, 7])] {
            graph.groups.push(Group {
                id: format!("t#{key}"),
                key: key.into(),
                title: key.into(),
                area: "gpu".into(),
            });
            for m in members {
                graph.nodes[m].group = Some(graph.groups.len() - 1);
            }
        }
        let v = graph.view(&|t| t.len() as f32 * 0.6, None);
        let x = |i: usize| v.laid.rects[v.local[i].unwrap()].center().x;
        assert_eq!(v.laid.rects.len(), 8);
        for (i, node) in graph.nodes.iter().enumerate() {
            for &r in &node.requires {
                assert!(x(r) < x(i), "{r} is not left of {i}");
            }
        }
        // Each group's box holds its members and no other node.
        assert_eq!(v.laid.boxes.len(), 2);
        for &(c, b) in &v.laid.boxes {
            let g = v.groups[c].expect("a boxed cluster is a group");
            for (i, node) in graph.nodes.iter().enumerate() {
                let inside = b.contains(v.laid.rects[v.local[i].unwrap()].center());
                assert_eq!(inside, node.group == Some(g), "node {i} and group {g}");
            }
        }
        // The compact view of 0, 2, 3 (closed under requirements): their edges only, group a's box alone.
        let visible: Vec<bool> = (0..8).map(|i| [0, 2, 3].contains(&i)).collect();
        let v = graph.view(&|t| t.len() as f32 * 0.6, Some(&visible));
        assert_eq!(v.shown, [0, 2, 3]);
        assert_eq!(
            v.graph
                .edges
                .iter()
                .map(|e| (v.shown[e.from], v.shown[e.to]))
                .collect::<Vec<_>>(),
            [(0, 2), (2, 3)]
        );
        assert_eq!(
            v.laid
                .boxes
                .iter()
                .map(|b| v.groups[b.0])
                .collect::<Vec<_>>(),
            [Some(0)]
        );
        assert!(v.laid.rects[1].center().x < v.laid.rects[2].center().x);
    }

    /// The public packages' graphs merge, and their audiences check, from the
    /// checkout's sources (the run time reads the built copies).
    #[test]
    fn public_graphs_check() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap();
        let all = packages::scan(root, &root.join("packages")).unwrap();
        for package in ["o3de_lighting", "godot_sdfgi", "godot_voxelgi"] {
            let mut files = Vec::new();
            for p in all
                .iter()
                .filter(|p| p.name == package || p.name == "render_lab")
            {
                let path = p.dir.join("knowledge.toml");
                files.push((
                    p,
                    parse(
                        &std::fs::read_to_string(&path).unwrap(),
                        &path.display().to_string(),
                    )
                    .unwrap(),
                ));
            }
            let graph = merge(package, files, &all, &|_, key| key.to_string())
                .unwrap_or_else(|e| panic!("{e}"));
            assert!(!graph.chapters.is_empty(), "{package} has no chapters");
            let problems = graph.check(&[]);
            assert!(problems.is_empty(), "{}", problems.join("\n"));
        }
    }
}
