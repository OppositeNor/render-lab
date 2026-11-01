//! The knowledge graph: what understanding an investigation's chapters needs,
//! worked back to three bases, and the audience an investigation is aimed
//! at (todo/knowledge-graph.md; the `knowledge-graph` agent skill).
//!
//! A package's `knowledge` resource (a `kind = "file"` TOML file, conventionally
//! `knowledge.toml`) has:
//!
//! - `[[topic]]` tables, the nodes: `key`, `title-text` and `summary-text`
//!   (keys of the package's text tables), `area` (the panel's grouping), the
//!   `requires` list, optional `reading` (published references), and
//!   `base = true` for the three bases (rlstd's alone);
//! - `[[chapter]]` tables, an investigation's chapters: `number`, `demos` (the
//!   demonstrations that show it), and `requires`, the nodes it uses directly;
//! - an `[audience]` table: `profile` (a note, not shown) and `known`, the
//!   nodes the audience knows.
//!
//! A requirement is a node of the same file (`geo-sdf`), or of a package it
//! imports whose `knowledge` it exports (`rlstd.knowledge#geo-sdf`). The graph must
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
//! (the proof is in todo/knowledge-graph.md). The rest is what the story teaches.

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
    pub chapters: Vec<Chapter>,
    pub profile: Option<String>,
    /// The audience's known nodes, as written (without their closure).
    pub known: Vec<usize>,
}

/// The panel's layout (graph units: a node is one unit high; x grows from
/// the bases' column on the left, y downward).
pub use crate::layered::Layout;

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
    let mut stack: Vec<usize> = start.iter().flat_map(|&s| requires[s].iter().copied()).collect();
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
    fn visit(n: usize, requires: &[Vec<usize>], state: &mut [u8], path: &mut Vec<usize>) -> Option<Vec<usize>> {
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
            let others: Vec<usize> = reqs.iter().enumerate().filter(|&(j, _)| j != k).map(|(_, &o)| o).collect();
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
pub fn partition(requires: &[Vec<usize>], bases: &[usize], chapters: &[(u32, Vec<usize>)], known: &[usize]) -> Partition {
    let n = requires.len();
    let needed: Vec<usize> = chapters.iter().flat_map(|c| c.1.iter().copied()).collect();
    let relevant = context(requires, &needed);
    let closure = ancestors(requires, known);
    let a: Vec<bool> = (0..n).map(|i| relevant[i] && (closure[i] || known.contains(&i) || bases.contains(&i))).collect();
    let level: Vec<usize> = (0..n).filter(|&i| relevant[i] && !a[i] && requires[i].iter().all(|&r| a[r])).collect();
    let teach: Vec<usize> = (0..n).filter(|&i| relevant[i] && !a[i] && !level.contains(&i)).collect();
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
    Partition { relevant, known: a, level, teach, carries, known_chapters }
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
pub fn neighbours(requires: &[Vec<usize>], bases: &[usize], chapters: &[(u32, Vec<usize>)], known: &[usize]) -> Vec<Neighbour> {
    let p = partition(requires, bases, chapters, known);
    let n = requires.len();
    let mut out = Vec::new();
    for &s in &p.level {
        let mut k = known.to_vec();
        k.push(s);
        let carries = partition(requires, bases, chapters, &k).carries.len();
        out.push(Neighbour { added: true, node: s, known: k, carries });
    }
    // A maximal known node: known, not a base, and no known node requires it.
    for i in 0..n {
        if !p.known[i] || bases.contains(&i) || (0..n).any(|j| p.known[j] && requires[j].contains(&i)) {
            continue;
        }
        let k: Vec<usize> = (0..n).filter(|&j| p.known[j] && j != i && !bases.contains(&j)).collect();
        let carries = partition(requires, bases, chapters, &k).carries.len();
        out.push(Neighbour { added: false, node: i, known: k, carries });
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
}

struct RawFile {
    nodes: Vec<RawNode>,
    chapters: Vec<(u32, Vec<String>, Vec<String>)>,
    profile: Option<String>,
    known: Vec<String>,
}

fn strings(table: &toml_edit::Table, field: &str, fail: &dyn Fn(String) -> String) -> Result<Vec<String>, String> {
    match table.get(field) {
        None => Ok(Vec::new()),
        Some(item) => {
            let list = item.as_array().ok_or_else(|| fail(format!("`{field}` is a list of strings")))?;
            list.iter().map(|v| v.as_str().map(String::from).ok_or_else(|| fail(format!("`{field}` is a list of strings")))).collect()
        }
    }
}

fn string(table: &toml_edit::Table, field: &str, fail: &dyn Fn(String) -> String) -> Result<Option<String>, String> {
    match table.get(field) {
        None => Ok(None),
        Some(item) => item.as_str().map(|s| Some(s.to_string())).ok_or_else(|| fail(format!("`{field}` is a string"))),
    }
}

fn parse(text: &str, at: &str) -> Result<RawFile, String> {
    let fail = |m: String| format!("knowledge: {at}: {m}");
    let document = toml_edit::Document::parse(text.to_string()).map_err(|e| fail(format!("parse: {e}")))?;
    let table = document.as_table();
    if let Some((key, _)) = table.iter().find(|(k, _)| !["topic", "chapter", "audience"].contains(k)) {
        return Err(fail(format!("unknown key `{key}` (a knowledge file has topic, chapter, audience)")));
    }
    let tables = |name: &str| -> Result<Vec<&toml_edit::Table>, String> {
        match table.get(name) {
            None => Ok(Vec::new()),
            Some(item) => Ok(item.as_array_of_tables().ok_or_else(|| fail(format!("`{name}` is a list of `[[{name}]]` tables")))?.iter().collect()),
        }
    };
    let mut nodes = Vec::new();
    for t in tables("topic")? {
        const FIELDS: &[&str] = &["key", "title-text", "summary-text", "area", "base", "requires", "reading"];
        if let Some((key, _)) = t.iter().find(|(k, _)| !FIELDS.contains(k)) {
            return Err(fail(format!("a topic has no field `{key}` (it has {})", FIELDS.join(", "))));
        }
        let key = string(t, "key", &fail)?.ok_or_else(|| fail("a topic needs `key`".into()))?;
        let f = |m: String| fail(format!("topic `{key}`: {m}"));
        if key.is_empty() || !key.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-') {
            return Err(f("a key is lower-case letters, digits, and `-`".into()));
        }
        let area = string(t, "area", &f)?.ok_or_else(|| f("needs `area`".into()))?;
        if !AREAS.contains(&area.as_str()) {
            return Err(f(format!("area `{area}` is not one of {}", AREAS.join(", "))));
        }
        nodes.push(RawNode {
            title: string(t, "title-text", &f)?.ok_or_else(|| f("needs `title-text`".into()))?,
            summary: string(t, "summary-text", &f)?.ok_or_else(|| f("needs `summary-text`".into()))?,
            area,
            base: t.get("base").map(|b| b.as_bool().ok_or_else(|| f("`base` is true or false".into()))).transpose()?.unwrap_or(false),
            requires: strings(t, "requires", &f)?,
            reading: strings(t, "reading", &f)?,
            key,
        });
    }
    let mut chapters = Vec::new();
    for t in tables("chapter")? {
        if let Some((key, _)) = t.iter().find(|(k, _)| !["number", "demos", "requires"].contains(k)) {
            return Err(fail(format!("a chapter has no field `{key}` (it has number, demos, requires)")));
        }
        let number = t.get("number").and_then(|n| n.as_integer()).filter(|n| *n > 0).ok_or_else(|| fail("a chapter needs `number`, a positive integer".into()))? as u32;
        let f = |m: String| fail(format!("chapter {number}: {m}"));
        chapters.push((number, strings(t, "demos", &f)?, strings(t, "requires", &f)?));
    }
    let (profile, known) = match table.get("audience") {
        None => (None, Vec::new()),
        Some(item) => {
            let t = item.as_table().ok_or_else(|| fail("`audience` is a table".into()))?;
            if let Some((key, _)) = t.iter().find(|(k, _)| !["profile", "known"].contains(k)) {
                return Err(fail(format!("the audience has no field `{key}` (it has profile, known)")));
            }
            (string(t, "profile", &fail)?, strings(t, "known", &fail)?)
        }
    };
    Ok(RawFile { nodes, chapters, profile, known })
}

/// The full node name a reference in `package`'s topics names.
fn resolve(package: &Package, imported: &[Package], reference: &str) -> Result<String, String> {
    match reference.split_once('#') {
        None => Ok(format!("{}.knowledge#{reference}", package.name)),
        Some((resource, key)) => {
            let full = packages::resolve(package, imported, resource)?;
            if !full.ends_with(".knowledge") {
                return Err(format!("`{reference}` names `{full}`, not a package's `knowledge`"));
            }
            Ok(format!("{full}#{key}"))
        }
    }
}

/// Merges parsed files (each with its package, in import order) into the
/// graph of `package`, checking it. `text` turns a package's text key into text.
fn merge(package: &str, files: Vec<(&Package, RawFile)>, all: &[Package], text: &dyn Fn(&str, &str) -> String) -> Result<Graph, String> {
    let mut graph = Graph { package: package.to_string(), ..Default::default() };
    let mut index: BTreeMap<String, usize> = BTreeMap::new();
    for (p, file) in &files {
        for node in &file.nodes {
            let id = format!("{}.knowledge#{}", p.name, node.key);
            if index.contains_key(&id) {
                return Err(format!("knowledge: {}: the topic `{}` is declared twice", p.name, node.key));
            }
            if node.base && p.name != "rlstd" {
                return Err(format!("knowledge: {}: `{}` is a base; only rlstd declares the bases", p.name, node.key));
            }
            index.insert(id.clone(), graph.nodes.len());
            graph.nodes.push(Node { id, package: p.name.clone(), key: node.key.clone(), title: text(&p.name, &node.title), summary: text(&p.name, &node.summary), area: node.area.clone(), base: node.base, requires: Vec::new(), reading: node.reading.clone() });
        }
    }
    let lookup = |p: &Package, reference: &str| -> Result<usize, String> {
        let at = all.iter().position(|o| o.name == p.name).unwrap_or(all.len());
        let id = resolve(p, &all[..at], reference).map_err(|e| format!("knowledge: {}: {e}", p.name))?;
        index.get(&id).copied().ok_or_else(|| format!("knowledge: {}: no topic `{id}`", p.name))
    };
    let mut start = 0;
    for (p, file) in &files {
        for (i, node) in file.nodes.iter().enumerate() {
            let requires = node.requires.iter().map(|r| lookup(p, r)).collect::<Result<Vec<_>, _>>()?;
            if node.base != requires.is_empty() {
                let why = if node.base { "a base requires nothing" } else { "a topic that is not a base requires something" };
                return Err(format!("knowledge: {}: `{}`: {why}", p.name, node.key));
            }
            graph.nodes[start + i].requires = requires;
        }
        start += file.nodes.len();
        // An imported package's chapters and audience are its own investigation's.
        if p.name == package {
            for (number, demos, requires) in &file.chapters {
                if graph.chapters.iter().any(|c| c.number == *number) {
                    return Err(format!("knowledge: {package}: chapter {number} is declared twice"));
                }
                if requires.is_empty() {
                    return Err(format!("knowledge: {package}: chapter {number} requires nothing (leave a recap chapter out)"));
                }
                let requires = requires.iter().map(|r| lookup(p, r)).collect::<Result<Vec<_>, _>>()?;
                graph.chapters.push(Chapter { number: *number, demos: demos.clone(), requires });
            }
            graph.profile = file.profile.clone();
            graph.known = file.known.iter().map(|r| lookup(p, r)).collect::<Result<Vec<_>, _>>()?;
        }
    }
    graph.chapters.sort_by_key(|c| c.number);
    let requires = graph.requires();
    if let Some(c) = cycle(&requires) {
        return Err(format!("knowledge: {package}: the requirements form a cycle: {}", c.iter().map(|&i| graph.nodes[i].id.as_str()).collect::<Vec<_>>().join(" → ")));
    }
    if let Some((n, r)) = redundant(&requires).first() {
        return Err(format!("knowledge: {package}: `{}` requires `{}`, which another of its requirements already does (remove it)", graph.nodes[*n].id, graph.nodes[*r].id));
    }
    let bases = graph.bases();
    if graph.nodes.iter().any(|n| n.package == "rlstd") && bases.len() != 3 {
        return Err(format!("knowledge: rlstd declares {} bases (it declares three)", bases.len()));
    }
    for chapter in &graph.chapters {
        let mut with = requires.clone();
        with.push(chapter.requires.clone());
        if let Some((_, r)) = redundant(&with).into_iter().find(|(n, _)| *n == with.len() - 1) {
            return Err(format!("knowledge: {package}: chapter {} requires `{}`, which another of its requirements already does (remove it)", chapter.number, graph.nodes[r].id));
        }
    }
    Ok(graph)
}

impl Graph {
    pub fn requires(&self) -> Vec<Vec<usize>> {
        self.nodes.iter().map(|n| n.requires.clone()).collect()
    }

    pub fn bases(&self) -> Vec<usize> {
        (0..self.nodes.len()).filter(|&i| self.nodes[i].base).collect()
    }

    pub fn chapter_needs(&self) -> Vec<(u32, Vec<usize>)> {
        self.chapters.iter().map(|c| (c.number, c.requires.clone())).collect()
    }

    /// The node `name`: a full name, or a key (of the graph's package, else of any).
    pub fn find(&self, name: &str) -> Option<usize> {
        let nodes = &self.nodes;
        nodes.iter().position(|n| n.id == name).or_else(|| nodes.iter().position(|n| n.key == name && n.package == self.package)).or_else(|| nodes.iter().position(|n| n.key == name))
    }

    /// The stored audience's split.
    pub fn audience(&self) -> Partition {
        self.partition_for(&self.known)
    }

    pub fn partition_for(&self, known: &[usize]) -> Partition {
        partition(&self.requires(), &self.bases(), &self.chapter_needs(), known)
    }

    /// What the given chapters need (none given: every chapter).
    pub fn context(&self, chapters: &[u32]) -> Vec<bool> {
        let needs: Vec<usize> = self.chapters.iter().filter(|c| chapters.is_empty() || chapters.contains(&c.number)).flat_map(|c| c.requires.iter().copied()).collect();
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
        self.chapters.iter().filter(|c| c.demos.iter().any(|d| d == demo)).map(|c| c.number).collect()
    }

    /// The chapters that use a node (it is one of their needs or an ancestor of one).
    pub fn chapters_using(&self, node: usize) -> Vec<u32> {
        let requires = self.requires();
        self.chapters.iter().filter(|c| context(&requires, &c.requires)[node]).map(|c| c.number).collect()
    }

    /// Problems with the audience: missing, or leaving a chapter nothing to
    /// teach, or naming a node no chapter needs. `demos`: the investigation's
    /// demonstration keys, which the chapters' `demos` must name.
    pub fn check(&self, demos: &[&str]) -> Vec<String> {
        let mut problems = Vec::new();
        for c in &self.chapters {
            for d in &c.demos {
                if !demos.is_empty() && !demos.contains(&d.as_str()) {
                    problems.push(format!("{}: chapter {}: no demonstration `{d}`", self.package, c.number));
                }
            }
        }
        if self.chapters.is_empty() {
            return problems;
        }
        if self.known.is_empty() {
            problems.push(format!("{}: no `[audience] known` (the knowledge-graph agent skill chooses it)", self.package));
            return problems;
        }
        let p = self.audience();
        for number in &p.known_chapters {
            problems.push(format!("{}: chapter {number} needs nothing beyond what the audience knows", self.package));
        }
        for &k in &self.known {
            if !p.relevant[k] {
                problems.push(format!("{}: the audience's `{}` is not needed by any chapter", self.package, self.nodes[k].id));
            }
        }
        problems
    }

    /// The layered layout for the panel: ranks (columns, left to right) by
    /// the longest path from the bases, dummy points on edges that skip
    /// ranks, a few barycentric sweeps to order each rank. `width` measures a
    /// title in node-height units.
    pub fn layout(&self, width: &dyn Fn(&str) -> f32) -> Layout {
        let n = self.nodes.len();
        let requires = self.requires();
        let edges: Vec<(usize, usize)> = (0..n).flat_map(|i| requires[i].iter().map(move |&r| (r, i))).collect();
        // The first order groups the areas.
        let mut by_area: Vec<usize> = (0..n).collect();
        by_area.sort_by_key(|&i| (AREAS.iter().position(|a| *a == self.nodes[i].area).unwrap_or(AREAS.len()), self.nodes[i].key.clone()));
        let sizes: Vec<(f32, f32)> = self.nodes.iter().map(|node| (width(&node.title) + 0.8, 1.0)).collect();
        crate::layered::layout(&sizes, &edges, &by_area, crate::layered::Spacing { column: 2.4, row: 0.5, dummy: 0.35 })
    }
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
        let Some(resource) = crate::resources::find(&format!("{}.knowledge", p.name)) else { continue };
        match parse(resource.text(), resource.source()) {
            Ok(file) => files.push((p, file)),
            Err(e) => return Some(Err(e)),
        }
    }
    if files.is_empty() {
        return None;
    }
    Some(merge(package, files, all, &|p, key| crate::i18n::package_text(p, key).to_string()))
}

/// The current investigation's graph, read once per language (its titles
/// are translated). An error is printed once and leaves no graph.
pub fn current() -> Option<&'static Graph> {
    static CACHE: OnceLock<Mutex<BTreeMap<(String, bool), Option<&'static Graph>>>> = OnceLock::new();
    let package = crate::investigation::current().package()?;
    let key = (package.to_string(), crate::i18n::zh());
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

/// The current graph and its panel layout, computed once per graph.
pub fn current_layout(width: &dyn Fn(&str) -> f32) -> Option<(&'static Graph, &'static Layout)> {
    static CACHE: OnceLock<Mutex<Vec<(usize, &'static Layout)>>> = OnceLock::new();
    let graph = current()?;
    let id = graph as *const Graph as usize;
    let mut cache = CACHE.get_or_init(Default::default).lock().unwrap();
    if let Some((_, l)) = cache.iter().find(|(g, _)| *g == id) {
        return Some((graph, l));
    }
    let l: &'static Layout = Box::leak(Box::new(graph.layout(width)));
    cache.push((id, l));
    Some((graph, l))
}

/// A Mermaid flowchart of a graph's nodes (bases at the bottom), by area.
pub fn mermaid(graph: &Graph) -> String {
    let id = |i: usize| graph.nodes[i].key.replace('-', "_");
    let label = |i: usize| graph.nodes[i].title.replace('"', "'");
    let names = [("gpu", "GPU"), ("rt", "Real-time rendering"), ("geo", "Geometry and spatial structures"), ("cg", "Light and materials"), ("light", "Real-time lighting techniques"), ("gi", "Global illumination")];
    let mut out = String::from("flowchart BT\n  subgraph base_area[\"Bases\"]\n");
    for i in graph.bases() {
        out.push_str(&format!("    {}[\"{}\"]\n", id(i), label(i)));
    }
    out.push_str("  end\n");
    for (area, name) in names {
        let members: Vec<usize> = (0..graph.nodes.len()).filter(|&i| graph.nodes[i].area == area && !graph.nodes[i].base).collect();
        if members.is_empty() {
            continue;
        }
        out.push_str(&format!("  subgraph {area}_area[\"{name}\"]\n"));
        for i in members {
            out.push_str(&format!("    {}[\"{}\"]\n", id(i), label(i)));
        }
        out.push_str("  end\n");
    }
    for (i, node) in graph.nodes.iter().enumerate() {
        for &r in &node.requires {
            out.push_str(&format!("  {} --> {}\n", id(r), id(i)));
        }
    }
    out.push_str("  classDef base stroke-width:3px\n");
    out.push_str(&format!("  class {} base\n", graph.bases().iter().map(|&i| id(i)).collect::<Vec<_>>().join(",")));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 0, 1 bases; 2 ← 0; 3 ← 2; 4 ← 1; 5 ← 3, 4; 6 ← 0; 7 ← 5, 6 (a skip edge from 6).
    fn sample() -> Vec<Vec<usize>> {
        vec![vec![], vec![], vec![0], vec![2], vec![1], vec![3, 4], vec![0], vec![5, 6]]
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
        assert_eq!((0..8).filter(|&i| p.known[i]).collect::<Vec<_>>(), [0, 1, 2]);
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
                    assert!(a == b || !anc[a][b], "level not an antichain: {a} requires {b}");
                }
            }
            for (i, reqs) in g.iter().enumerate() {
                for &r in reqs {
                    if p.known[r] && p.teach.contains(&i) {
                        assert!(p.carries.contains(&(r, Carry::Node(i))), "{r} → {i} crosses without a carry");
                    }
                    assert!(!(p.known[i] && !p.known[r]), "known {i} requires unknown {r}");
                }
            }
        }
    }

    #[test]
    fn layout_places_every_node() {
        let mut graph = Graph::default();
        for (i, reqs) in sample().into_iter().enumerate() {
            graph.nodes.push(Node { id: format!("t#{i}"), package: "t".into(), key: i.to_string(), title: format!("node {i}"), summary: String::new(), area: "gpu".into(), base: reqs.is_empty(), requires: reqs, reading: Vec::new() });
        }
        let l = graph.layout(&|t| t.len() as f32 * 0.3);
        assert_eq!(l.positions.len(), 8);
        for (i, node) in graph.nodes.iter().enumerate() {
            for &r in &node.requires {
                assert!(l.positions[r].0 < l.positions[i].0, "{r} is not left of {i}");
            }
        }
        // 6 (rank 1) → 7 (rank 4) bends through ranks 2 and 3.
        let bend = l.bends.iter().find(|b| b.0 == 6 && b.1 == 7).unwrap();
        assert_eq!(bend.2.len(), 2);
    }

    /// The public packages' graphs merge, and their audiences check, from the
    /// checkout's sources (the run time reads the built copies).
    #[test]
    fn public_graphs_check() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let all = packages::scan(root, &root.join("packages")).unwrap();
        for package in ["o3de_lighting", "godot_sdfgi", "godot_voxelgi"] {
            let mut files = Vec::new();
            for p in all.iter().filter(|p| p.name == package || p.name == "rlstd") {
                let path = p.dir.join("knowledge.toml");
                files.push((p, parse(&std::fs::read_to_string(&path).unwrap(), &path.display().to_string()).unwrap()));
            }
            let graph = merge(package, files, &all, &|_, key| key.to_string()).unwrap_or_else(|e| panic!("{e}"));
            assert!(!graph.chapters.is_empty(), "{package} has no chapters");
            let problems = graph.check(&[]);
            assert!(problems.is_empty(), "{}", problems.join("\n"));
        }
    }
}
