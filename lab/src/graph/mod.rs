//! The frame as a render graph: nodes (passes) that read and write named
//! resources, built from an investigation's `SceneShading` and the lab's own
//! passes (the scene pass, the overlay, the composite). The renderer runs a
//! frame in the graph's order (src/renderer/), the viewer draws it with
//! each node's GPU time (the graph panel), and any resource can be shown as
//! it stood right after any node (the inspector, `<node>.<resource>`).
//!
//! The order comes from the declarations: for each resource, its accesses
//! keep their declared order (a writer before the readers after it, those
//! readers before the next writer), and nodes with no path between them keep
//! the order they are declared in. A node whose writes nothing live reads is
//! culled. `lint` checks the declarations against the WGSL: what each entry
//! point reads and writes, from naga's validator, must be what its pass
//! lists, so the graph's edges are the shaders' real data flow.

use crate::investigation::SceneShading;
use crate::pipelines::chain_binding;
use anyhow::Result;
use naga::valid::GlobalUse;
use std::collections::BTreeSet;
use std::fmt::Write as _;

mod scene;
mod show;
mod lint;
pub use scene::*;
pub use show::*;
pub use lint::*;

/// The names of the lab's own nodes (`Step::DepthViews`, `Overlay`,
/// `Composite`, and the scene pass of a frame without a shading).
pub const LAB_NODES: [&str; 4] = ["depthViews", "overlay", "composite", "scene"];

/// How long a resource lives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lifetime {
    /// Provided by the lab: `hdr` (what the frame shows), `depth` (the
    /// scene pass's), `overlay`, and `frame`.
    Imported,
    /// Made by its first writer this frame, read until its last reader.
    Transient,
    /// Kept across frames, rewritten only when what it is computed from
    /// changes (the update passes' volumes and buffers, the depth views).
    Persistent,
    /// A chain target or buffer the frame reads from the frames before it
    /// (`SceneShading::history`): kept from frame to frame, made again (zero)
    /// when the viewport's size changes.
    History,
}

impl Lifetime {
    pub fn name(self) -> &'static str {
        match self {
            Lifetime::Imported => "imported",
            Lifetime::Transient => "transient",
            Lifetime::Persistent => "persistent",
            Lifetime::History => "history",
        }
    }
}

/// What a resource is.
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    /// A 2D texture: the viewport's size divided by `divisor` (0: chosen by
    /// the state, `SceneShading::resolution`), with `samples` samples.
    Texture { format: wgpu::TextureFormat, divisor: u32, samples: u32 },
    /// A storage buffer of `words_per_tile` u32 per `tile` x `tile` pixels.
    Buffer { words_per_tile: u32, tile: u32 },
    /// An update pass's storage buffer (`UpdateBuffer`).
    UpdateBuffer,
    /// A square depth texture array (`DepthViews`).
    DepthArray { size: u32, layers: u32 },
    /// A 3D texture with mips (`ShadingVolume`).
    Volume { format: wgpu::TextureFormat, size: [u32; 3], mips: u32 },
    /// A program's image (`ProgramImage`): `count` textures of its kind and size.
    /// `viewport`: 0, `size`; otherwise the view's size divided by it. `mips`: its levels at most.
    Image { kind: crate::investigation::ImageKind, format: wgpu::TextureFormat, size: [u32; 3], count: u32, mips: u32, viewport: u32 },
    /// A program's buffer (`ProgramBuffer`), `bytes` long.
    Bytes { bytes: u64 },
}

impl Shape {
    /// A short description: format and size.
    pub fn describe(&self) -> String {
        match self {
            Shape::Texture { format, divisor, samples } => {
                let size = match divisor {
                    0 => "viewport / state".to_string(),
                    1 => "viewport".to_string(),
                    d => format!("viewport / {d}"),
                };
                let samples = if *samples > 1 { format!(", {samples}x MSAA") } else { String::new() };
                format!("{format:?}, {size}{samples}")
            }
            Shape::Buffer { words_per_tile, tile } => format!("{words_per_tile} u32 per {tile}x{tile} pixels"),
            Shape::UpdateBuffer => "u32 buffer".to_string(),
            Shape::DepthArray { size, layers } => format!("Depth32Float, {size}x{size}, {layers} layers"),
            Shape::Volume { format, size, mips } => format!("{format:?}, {}x{}x{}, {mips} mips", size[0], size[1], size[2]),
            Shape::Image { kind, format, size, count, mips, viewport } => {
                let extent = match kind {
                    crate::investigation::ImageKind::D2 if *viewport > 0 => if *viewport == 1 { "viewport".to_string() } else { format!("viewport / {viewport}") },
                    crate::investigation::ImageKind::D2 => format!("{}x{}", size[0], size[1]),
                    crate::investigation::ImageKind::D2Array => format!("{}x{}, {} layers", size[0], size[1], size[2]),
                    crate::investigation::ImageKind::D3 => format!("{}x{}x{}", size[0], size[1], size[2]),
                };
                let count = if *count > 1 { format!(", {count} of them") } else { String::new() };
                let mips = if *mips > 1 { ", mips".to_string() } else { String::new() };
                format!("{format:?}, {extent}{mips}{count}")
            }
            Shape::Bytes { bytes } => format!("{bytes} bytes"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Resource {
    pub name: &'static str,
    pub lifetime: Lifetime,
    pub shape: Shape,
}

/// What the renderer runs for a node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Update pass `i` of `SceneShading::update` (run when its plan's key changes).
    Update(usize),
    /// The depth views, one render pass per layer (drawn when what they see changes).
    DepthViews,
    /// The depth pre-pass.
    Prepass,
    /// The scene pass: the scene's meshes, MSAA, into the outputs (or `hdr`).
    Scene,
    /// Compute pass `i` of `SceneShading::passes`.
    Compute(usize),
    /// Pass `i` of the program (`ShadingProgram::passes`): its commands this frame.
    Program(usize),
    /// The lab's overlay (lines, boxes, points), depth-tested against the scene.
    Overlay,
    /// The lab's composite: `hdr` tone-mapped, under the overlay, into the frame.
    Composite,
}

#[derive(Clone, Debug)]
pub struct Node {
    /// Its name in the inspector (`<node>.<resource>`): its entry point, or
    /// the lab's name for its own passes.
    pub name: &'static str,
    pub step: Step,
    pub reads: Vec<&'static str>,
    pub writes: Vec<&'static str>,
    /// Its row in the GPU timing (src/timing.rs).
    pub timing: &'static str,
    /// It runs only when what it computes from changes.
    pub cached: bool,
}

/// A frame's graph: its nodes in declaration order and its resources, with
/// the order, the culled nodes, and each resource's lifetime once compiled.
#[derive(Clone, Debug)]
pub struct FrameGraph {
    pub nodes: Vec<Node>,
    pub resources: Vec<Resource>,
    /// Edges between nodes (from, to), by index into `nodes`: data (a writer
    /// to a reader of what it wrote) and order (a reader to the next writer,
    /// a writer to the next writer).
    pub edges: Vec<(usize, usize, Edge)>,
    /// The nodes in execution order.
    pub order: Vec<usize>,
    /// Whether each node runs (indexed as `nodes`): false when nothing live
    /// reads what it writes (a node that writes a history resource, or one
    /// that is cached, always runs).
    pub live: Vec<bool>,
    /// What is wrong with the declarations.
    pub errors: Vec<String>,
    /// How the investigation shows its resources (`SceneShading::shows`).
    pub shows: Vec<(&'static str, &'static str)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    /// What the first node writes, the second reads: the resource by index.
    Data(usize),
    /// The second overwrites what the first read or wrote.
    Order(usize),
}

impl FrameGraph {
    /// The graph of a frame drawn with `shading` (none: the lab's own scene
    /// pass), keeping alive the nodes that write `sinks` (an image kind's
    /// chain reads; the inspected resource) besides what the frame shows.
    pub fn new(shading: Option<&'static SceneShading>, sinks: &[&str]) -> FrameGraph {
        let mut nodes = Vec::new();
        let mut resources: Vec<Resource> = vec![
            Resource { name: "hdr", lifetime: Lifetime::Imported, shape: Shape::Texture { format: crate::gpu::HDR_FORMAT, divisor: 1, samples: 1 } },
            Resource { name: "depth", lifetime: Lifetime::Imported, shape: Shape::Texture { format: crate::gpu::DEPTH_FORMAT, divisor: 1, samples: crate::gpu::MSAA } },
        ];
        if let Some(s) = shading {
            for t in s.targets {
                let divisor = if s.resolution.is_some_and(|r| r.targets.contains(&t.name)) { 0 } else { t.divisor };
                let samples = if s.attachment(t.name) { crate::gpu::MSAA } else { 1 };
                resources.push(Resource { name: t.name, lifetime: if s.history.contains(&t.name) { Lifetime::History } else { Lifetime::Transient }, shape: Shape::Texture { format: t.format, divisor, samples } });
            }
            let life = |name: &str| if s.history.contains(&name) { Lifetime::History } else { Lifetime::Transient };
            for b in s.buffers {
                resources.push(Resource { name: b.name, lifetime: life(b.name), shape: Shape::Buffer { words_per_tile: b.words_per_tile, tile: b.tile } });
            }
            if let Some(d) = s.depth_views {
                resources.push(Resource { name: d.name, lifetime: Lifetime::Persistent, shape: Shape::DepthArray { size: d.size, layers: d.vertices.len() as u32 } });
            }
            for v in s.volumes {
                resources.push(Resource { name: v.name, lifetime: Lifetime::Persistent, shape: Shape::Volume { format: v.format, size: v.size, mips: v.mips } });
            }
            if let Some(u) = s.update {
                for b in u.buffers {
                    resources.push(Resource { name: b.name, lifetime: Lifetime::Persistent, shape: Shape::UpdateBuffer });
                }
            }
            if let Some(program) = s.program {
                for i in program.images {
                    resources.push(Resource { name: i.name, lifetime: Lifetime::Persistent, shape: Shape::Image { kind: i.kind, format: i.format, size: i.size, count: i.count, mips: i.mips, viewport: i.viewport } });
                }
                for b in program.buffers {
                    resources.push(Resource { name: b.name, lifetime: Lifetime::Persistent, shape: Shape::Bytes { bytes: b.bytes } });
                }
            }
        }
        // The program's scene group: what the scene pass reads of it, but what the chain's passes declare they read.
        let chain_reads: Vec<&str> = shading.map_or(Vec::new(), |s| s.passes.iter().flat_map(|p| p.reads.iter().copied()).collect());
        let scene_uses: Vec<&'static str> = shading.and_then(|s| s.program).map_or(Vec::new(), |p| program_reads(p.scene_uses)).into_iter().filter(|r| !chain_reads.contains(r)).collect();
        resources.push(Resource { name: "overlay", lifetime: Lifetime::Imported, shape: Shape::Texture { format: crate::gpu::OVERLAY_FORMAT, divisor: 1, samples: 1 } });
        resources.push(Resource { name: "frame", lifetime: Lifetime::Imported, shape: Shape::Texture { format: crate::gpu::FRAME_FORMAT, divisor: 1, samples: 1 } });
        let mut errors = Vec::new();
        match shading {
            Some(s) => Self::shading_nodes(s, &scene_uses, &mut nodes, &mut errors),
            None => nodes.push(Node { name: "scene", step: Step::Scene, reads: Vec::new(), writes: vec!["hdr", "depth"], timing: "scene pass", cached: false }),
        }
        nodes.push(Node { name: "overlay", step: Step::Overlay, reads: vec!["depth"], writes: vec!["overlay"], timing: "overlay", cached: false });
        nodes.push(Node { name: "composite", step: Step::Composite, reads: vec!["hdr", "overlay"], writes: vec!["frame"], timing: "composite", cached: false });
        let shows = shading.map_or(Vec::new(), |s| s.shows.to_vec());
        let mut graph = FrameGraph { nodes, resources, edges: Vec::new(), order: Vec::new(), live: Vec::new(), errors, shows };
        if let Some(s) = shading.filter(|s| !s.chained() && !s.passes.is_empty()) {
            graph.errors.push(format!("{}: compute passes without scene or pre-pass outputs never run (SceneShading::outputs is empty)", s.fragment));
        }
        let after_scene = |s: &SceneShading| s.nodes.iter().skip_while(|n| **n != crate::investigation::FrameNode::Scene).any(|n| matches!(n, crate::investigation::FrameNode::Compute(_)));
        if let Some(s) = shading.filter(|s| s.outputs.is_empty() && after_scene(s)) {
            graph.errors.push(format!("{}: passes after a scene pass that writes the frame (no outputs) have nothing to read", s.fragment));
        }
        if let Some(s) = shading {
            for h in s.history {
                if !s.targets.iter().any(|t| t.name == *h) && !s.buffers.iter().any(|b| b.name == *h) {
                    graph.errors.push(format!("{}: history `{h}` is none of its targets or buffers", s.fragment));
                }
            }
            for (name, spec) in s.shows {
                if graph.resource(name).is_none() {
                    graph.errors.push(format!("{}: shows `{name}`, which is no resource", s.fragment));
                }
                if let Err(e) = parse_view(spec) {
                    graph.errors.push(format!("{}: shows `{name}` as `{spec}`: {e}", s.fragment));
                }
            }
        }
        graph.compile(sinks);
        graph
    }

    /// The graph of the current investigation's frame.
    pub fn current(sinks: &[&str]) -> FrameGraph {
        FrameGraph::new(crate::investigation::current().scene_shading(), sinks)
    }

    /// The nodes of a shading's frame, in its order (`SceneShading::nodes`),
    /// and what is wrong with that order.
    fn shading_nodes(s: &'static SceneShading, scene_uses: &[&'static str], nodes: &mut Vec<Node>, errors: &mut Vec<String>) {
        use crate::investigation::FrameNode;
        let chained = s.chained();
        let frame = s.nodes;
        let mut named: Vec<&'static str> = Vec::new();
        let mut unique = |entry: &'static str| -> &'static str {
            let k = named.iter().filter(|n| **n == entry).count();
            named.push(entry);
            if k == 0 { entry } else { Box::leak(format!("{entry}#{}", k + 1).into_boxed_str()) }
        };
        let computes = s.passes;
        let updates = s.update.map_or(&[][..], |u| u.passes);
        let programs = s.program.map_or(&[][..], |p| p.passes);
        // How many times each declared node is placed.
        let mut placed = std::collections::BTreeMap::<FrameNode, usize>::new();
        let mut leading = true;
        for node in frame.iter() {
            *placed.entry(*node).or_default() += 1;
            let update = matches!(node, FrameNode::Update(_));
            if update && !leading {
                errors.push(format!("{}: update passes run before the frame; list them first", s.fragment));
            }
            leading &= update;
            match *node {
                FrameNode::Update(i) => match updates.get(i) {
                    Some(p) => nodes.push(Node { name: p.entry, step: Step::Update(i), reads: p.reads.to_vec(), writes: p.writes.to_vec(), timing: p.entry, cached: true }),
                    None => errors.push(format!("{}: no update pass {i}", s.fragment)),
                },
                FrameNode::DepthViews => match s.depth_views {
                    Some(d) if chained => nodes.push(Node { name: "depthViews", step: Step::DepthViews, reads: Vec::new(), writes: vec![d.name], timing: "depth views", cached: true }),
                    Some(_) => {}
                    None => errors.push(format!("{}: a depth views node without depth views", s.fragment)),
                },
                FrameNode::Prepass => match s.prepass {
                    Some(entry) if chained => {
                        let mut writes = vec!["depth"];
                        writes.extend(s.prepass_outputs.iter().copied());
                        nodes.push(Node { name: entry, step: Step::Prepass, reads: Vec::new(), writes, timing: "depth pre-pass", cached: false });
                    }
                    Some(_) => {}
                    None => errors.push(format!("{}: a pre-pass node without a pre-pass", s.fragment)),
                },
                FrameNode::Compute(i) => match computes.get(i) {
                    // Without a chain they never run (an error below).
                    Some(p) if chained => nodes.push(Node { name: unique(p.entry), step: Step::Compute(i), reads: p.reads.to_vec(), writes: p.writes.to_vec(), timing: p.entry, cached: false }),
                    Some(_) => {}
                    None => errors.push(format!("{}: no compute pass {i}", s.fragment)),
                },
                FrameNode::Program(i) => match programs.get(i) {
                    Some(p) => nodes.push(Node { name: p.name, step: Step::Program(i), reads: program_reads(p.uses), writes: program_writes(p.uses), timing: p.name, cached: false }),
                    None => errors.push(format!("{}: no program pass {i}", s.fragment)),
                },
                FrameNode::Scene => {
                    let (reads, writes) = if chained {
                        let mut reads = s.scene_reads.to_vec();
                        reads.extend(scene_uses.iter().copied().filter(|r| !s.scene_reads.contains(r)));
                        // Without outputs, the scene pass writes the frame's HDR target, as without a chain.
                        let mut writes = if s.outputs.is_empty() { vec!["hdr"] } else { s.outputs.to_vec() };
                        if s.prepass.is_some() {
                            reads.push("depth");
                        } else {
                            writes.push("depth");
                        }
                        (reads, writes)
                    } else {
                        (scene_uses.to_vec(), vec!["hdr", "depth"])
                    };
                    nodes.push(Node { name: s.fragment, step: Step::Scene, reads, writes, timing: "scene pass", cached: false });
                }
            }
        }
        // Each declared node once, the scene pass once, the pre-pass before it.
        let count = |n: FrameNode| placed.get(&n).copied().unwrap_or(0);
        let mut wanted: Vec<(FrameNode, String)> = vec![(FrameNode::Scene, format!("the scene pass `{}`", s.fragment))];
        wanted.extend(updates.iter().enumerate().map(|(i, p)| (FrameNode::Update(i), format!("update pass `{}`", p.entry))));
        wanted.extend(s.depth_views.map(|d| (FrameNode::DepthViews, format!("the depth views `{}`", d.name))));
        wanted.extend(s.prepass.map(|e| (FrameNode::Prepass, format!("the pre-pass `{e}`"))));
        wanted.extend(computes.iter().enumerate().map(|(i, p)| (FrameNode::Compute(i), format!("compute pass {i} `{}`", p.entry))));
        wanted.extend(programs.iter().enumerate().map(|(i, p)| (FrameNode::Program(i), format!("program pass `{}`", p.name))));
        for (node, what) in wanted {
            match count(node) {
                1 => {}
                0 => errors.push(format!("{}: {what} is in no node of the frame", s.fragment)),
                n => errors.push(format!("{}: {what} is in {n} nodes of the frame", s.fragment)),
            }
        }
        let at = |n: FrameNode| frame.iter().position(|x| *x == n);
        if let (Some(p), Some(scene)) = (at(FrameNode::Prepass), at(FrameNode::Scene)) && p > scene {
            errors.push(format!("{}: the pre-pass comes after the scene pass, which tests against its depth", s.fragment));
        }
    }

    pub fn resource(&self, name: &str) -> Option<usize> {
        self.resources.iter().position(|r| r.name == name)
    }

    pub fn node(&self, name: &str) -> Option<usize> {
        self.nodes.iter().position(|n| n.name == name)
    }

    fn compile(&mut self, sinks: &[&str]) {
        // Names: every one a node uses is a resource; a node names each once.
        for n in &self.nodes {
            for name in n.reads.iter().chain(&n.writes) {
                if self.resources.iter().all(|r| r.name != *name) {
                    self.errors.push(format!("{}: no resource `{name}`", n.name));
                }
            }
            let mut seen = BTreeSet::new();
            for name in n.reads.iter().chain(&n.writes).filter(|name| self.resources.iter().any(|r| r.name == **name)) {
                if (n.reads.iter().filter(|r| *r == name).count() > 1 || n.writes.iter().filter(|w| *w == name).count() > 1) && seen.insert(*name) {
                    self.errors.push(format!("{}: lists `{name}` twice", n.name));
                }
            }
        }
        if self.nodes.iter().map(|n| n.name).collect::<BTreeSet<_>>().len() != self.nodes.len() {
            let mut seen = BTreeSet::new();
            for n in &self.nodes {
                if !seen.insert(n.name) {
                    self.errors.push(format!("two nodes are named `{}`", n.name));
                }
            }
        }
        // Edges, per resource, in declaration order.
        let mut edges = Vec::new();
        for (r, resource) in self.resources.iter().enumerate() {
            let mut writer: Option<usize> = None;
            let mut readers: Vec<usize> = Vec::new();
            for (i, n) in self.nodes.iter().enumerate() {
                let reads = n.reads.contains(&resource.name);
                let writes = n.writes.contains(&resource.name);
                if reads {
                    match writer {
                        Some(w) if w != i => edges.push((w, i, Edge::Data(r))),
                        Some(_) => {}
                        None if matches!(resource.lifetime, Lifetime::Persistent | Lifetime::History) => {}
                        // A node that reads and writes it first reads what it writes itself.
                        None if writes => {}
                        None => self.errors.push(format!("{} reads `{}` before any node writes it", n.name, resource.name)),
                    }
                    readers.push(i);
                }
                if writes {
                    for &reader in readers.iter().filter(|&&x| x != i) {
                        edges.push((reader, i, Edge::Order(r)));
                    }
                    if let Some(w) = writer.filter(|&w| w != i && !readers.contains(&w)) {
                        edges.push((w, i, Edge::Order(r)));
                    }
                    writer = Some(i);
                    readers.clear();
                }
            }
        }
        edges.sort_by_key(|e| (e.0, e.1));
        edges.dedup_by_key(|e| (e.0, e.1, e.2));
        self.edges = edges;
        // The order: ready nodes, the first declared first.
        let count = self.nodes.len();
        let mut incoming = vec![0usize; count];
        for e in &self.edges {
            incoming[e.1] += 1;
        }
        let mut done = vec![false; count];
        while self.order.len() < count {
            let Some(next) = (0..count).find(|&i| !done[i] && incoming[i] == 0) else {
                self.errors.push("the nodes' accesses form a cycle".to_string());
                self.order.extend((0..count).filter(|&i| !done[i]));
                break;
            };
            done[next] = true;
            self.order.push(next);
            for e in self.edges.iter().filter(|e| e.0 == next) {
                incoming[e.1] -= 1;
            }
        }
        // Live: the composite, the cached nodes (their results outlive the
        // frame), the writers of the sinks, and every writer what is live reads.
        let mut live = vec![false; count];
        let history = |n: &Node| n.writes.iter().any(|w| self.resources.iter().any(|r| r.name == *w && r.lifetime == Lifetime::History));
        // A program's passes run when its plan says, writing what outlives the frame.
        let mut stack: Vec<usize> = (0..count).filter(|&i| self.nodes[i].cached || matches!(self.nodes[i].step, Step::Composite | Step::Program(_)) || history(&self.nodes[i])).collect();
        for sink in sinks {
            if let Some(i) = self.order.iter().rev().copied().find(|&i| self.nodes[i].writes.contains(sink)) {
                stack.push(i);
            }
        }
        while let Some(i) = stack.pop() {
            if std::mem::replace(&mut live[i], true) {
                continue;
            }
            stack.extend(self.edges.iter().filter(|e| e.1 == i && matches!(e.2, Edge::Data(_))).map(|e| e.0));
        }
        self.live = live;
    }

    /// Writes nothing reads: `(node, resource)`, for the graph command's notes.
    pub fn unread(&self) -> Vec<(&'static str, &'static str)> {
        let mut out = Vec::new();
        for (i, n) in self.nodes.iter().enumerate() {
            for w in &n.writes {
                let Some(r) = self.resource(w) else { continue };
                if self.resources[r].lifetime != Lifetime::Transient {
                    continue;
                }
                if !self.edges.iter().any(|e| e.0 == i && e.2 == Edge::Data(r)) {
                    out.push((n.name, *w));
                }
            }
        }
        out
    }

    /// The first and last position in `order` of the live nodes that use
    /// resource `r` (none if no live node does).
    pub fn lifetime(&self, r: usize) -> Option<(usize, usize)> {
        let name = self.resources[r].name;
        let uses: Vec<usize> = self.order.iter().enumerate().filter(|&(_, &i)| self.live[i] && (self.nodes[i].reads.contains(&name) || self.nodes[i].writes.contains(&name))).map(|(p, _)| p).collect();
        Some((*uses.first()?, *uses.last()?))
    }

    /// How resource `r` is shown unless told otherwise: the investigation's
    /// choice (`SceneShading::shows`), or what its format suggests.
    pub fn view_of(&self, r: usize) -> (Show, std::collections::BTreeMap<String, Vec<f32>>) {
        let resource = &self.resources[r];
        self.shows
            .iter()
            .find(|(name, _)| *name == resource.name)
            .and_then(|(_, spec)| parse_view(spec).ok())
            .map(|(show, params)| (show.unwrap_or_else(|| default_show(&resource.shape)), params))
            .unwrap_or_else(|| (default_show(&resource.shape), Default::default()))
    }

    /// The node `name` and resource `resource` of an inspector key
    /// (`<node>.<resource>`): the node must write or read it.
    pub fn inspected(&self, key: &str) -> Result<(usize, usize), String> {
        let (node, resource) = key.rsplit_once('.').ok_or_else(|| format!("`{key}`: expected <node>.<resource>"))?;
        let n = self.node(node).ok_or_else(|| format!("`{key}`: no node `{node}` (nodes: {})", self.nodes.iter().map(|n| n.name).collect::<Vec<_>>().join(", ")))?;
        let r = self.resource(resource).ok_or_else(|| format!("`{key}`: no resource `{resource}`"))?;
        if resource == "frame" {
            return Err(format!("`{key}`: the frame is what the viewer shows; inspect `hdr` or `overlay`"));
        }
        if !self.nodes[n].writes.contains(&self.resources[r].name) && !self.nodes[n].reads.contains(&self.resources[r].name) {
            return Err(format!("`{key}`: {node} neither reads nor writes {resource} (it writes {})", self.nodes[n].writes.join(", ")));
        }
        Ok((n, r))
    }

    /// The node right after which resource `name` holds what the frame made
    /// of it: its last live writer that runs every frame (later readers may
    /// discard it, as the overlay does the depth), else (a resource only a
    /// cached node writes) its last live reader.
    pub fn last_user(&self, name: &str) -> Option<usize> {
        let last = |pick: &dyn Fn(&Node) -> bool| self.order.iter().rev().copied().find(|&i| self.live[i] && pick(&self.nodes[i]));
        last(&|n: &Node| !n.cached && n.writes.contains(&name)).or_else(|| last(&|n: &Node| n.reads.contains(&name) || n.writes.contains(&name)))
    }

    /// What the DAG view shows on node `i`: the first image it writes (a
    /// texture or a volume: an update pass's volume, not its bookkeeping
    /// buffer), else its first write, else (a node that writes only the
    /// frame) what it reads.
    pub fn result_of(&self, i: usize) -> Option<&'static str> {
        let n = &self.nodes[i];
        let image = |name: &&'static str| self.resource(name).is_some_and(|r| !matches!(self.resources[r].shape, Shape::Buffer { .. } | Shape::UpdateBuffer | Shape::Bytes { .. }));
        n.writes.iter().copied().filter(|r| *r != "frame").find(|r| image(r)).or_else(|| n.writes.iter().chain(&n.reads).copied().find(|r| *r != "frame"))
    }

    /// The graph as text: the nodes in order, then the resources.
    pub fn text(&self) -> String {
        let mut out = String::new();
        for (p, &i) in self.order.iter().enumerate() {
            let n = &self.nodes[i];
            let state = if !self.live[i] { " (culled)" } else if n.cached { " (when its inputs change)" } else { "" };
            let _ = writeln!(out, "{p:2} {}{state}", n.name);
            if !n.reads.is_empty() {
                let _ = writeln!(out, "     reads  {}", n.reads.join(", "));
            }
            if !n.writes.is_empty() {
                let _ = writeln!(out, "     writes {}", n.writes.join(", "));
            }
        }
        let _ = writeln!(out, "resources:");
        let width = self.resources.iter().map(|r| r.name.len()).max().unwrap_or(0);
        for (r, resource) in self.resources.iter().enumerate() {
            let life = match self.lifetime(r) {
                Some((a, b)) => format!("nodes {a}..{b}"),
                None => "unused".to_string(),
            };
            let _ = writeln!(out, "  {:width$}  {:10} {}; {life}", resource.name, resource.lifetime.name(), resource.shape.describe());
        }
        for (node, resource) in self.unread() {
            let _ = writeln!(out, "note: {node} writes `{resource}`, which nothing after it reads (an image kind or the inspector may)");
        }
        out
    }

    /// The graph in Graphviz's language.
    pub fn dot(&self) -> String {
        let mut out = String::from("digraph frame {\n  rankdir=LR;\n");
        for &i in &self.order {
            let style = if self.live[i] { "" } else { ", style=dashed" };
            let _ = writeln!(out, "  n{i} [label=\"{}\", shape=box{style}];", self.nodes[i].name);
        }
        for (r, resource) in self.resources.iter().enumerate() {
            let _ = writeln!(out, "  r{r} [label=\"{}\\n{}\", shape=ellipse];", resource.name, resource.lifetime.name());
        }
        for (i, n) in self.nodes.iter().enumerate() {
            for name in &n.reads {
                if let Some(r) = self.resource(name) {
                    let _ = writeln!(out, "  r{r} -> n{i};");
                }
            }
            for name in &n.writes {
                if let Some(r) = self.resource(name) {
                    let _ = writeln!(out, "  n{i} -> r{r};");
                }
            }
        }
        out.push_str("}\n");
        out
    }

    pub fn json(&self) -> serde_json::Value {
        serde_json::json!({
            "nodes": self.order.iter().map(|&i| {
                let n = &self.nodes[i];
                serde_json::json!({"name": n.name, "reads": n.reads, "writes": n.writes, "timing": n.timing, "cached": n.cached, "live": self.live[i]})
            }).collect::<Vec<_>>(),
            "resources": self.resources.iter().enumerate().map(|(r, res)| serde_json::json!({"name": res.name, "lifetime": res.lifetime.name(), "shape": res.shape.describe(), "uses": self.lifetime(r)})).collect::<Vec<_>>(),
            // What flows from node to node: the writer, the reader, the resource.
            "edges": self.edges.iter().filter_map(|&(a, b, e)| match e {
                Edge::Data(r) => Some(serde_json::json!({"from": self.nodes[a].name, "to": self.nodes[b].name, "resource": self.resources[r].name})),
                Edge::Order(_) => None,
            }).collect::<Vec<_>>(),
            "errors": self.errors,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::investigation::{ShadingBuffer, ShadingPass, ShadingTarget};

    /// A chain: the pre-pass, `early`, the scene pass, then `after`.
    fn shading(early: &'static [ShadingPass], after: &'static [ShadingPass]) -> &'static SceneShading {
        use crate::investigation::FrameNode;
        static TARGETS: &[ShadingTarget] = &[
            ShadingTarget { name: "color", format: wgpu::TextureFormat::Rgba16Float, divisor: 1 },
            ShadingTarget { name: "blur", format: wgpu::TextureFormat::Rgba16Float, divisor: 1 },
            ShadingTarget { name: "unused", format: wgpu::TextureFormat::Rgba16Float, divisor: 1 },
        ];
        static BUFFERS: &[ShadingBuffer] = &[ShadingBuffer { name: "tiles", words_per_tile: 4, tile: 16 }];
        let passes: &'static [ShadingPass] = Box::leak(early.iter().chain(after).map(|p| ShadingPass { ..*p }).collect::<Vec<_>>().into_boxed_slice());
        let mut nodes = vec![FrameNode::Prepass];
        nodes.extend((0..early.len()).map(FrameNode::Compute));
        nodes.push(FrameNode::Scene);
        nodes.extend((early.len()..passes.len()).map(FrameNode::Compute));
        Box::leak(Box::new(SceneShading {
            module: "test",
            fragment: "sceneFS",
            data: Vec::new,
            verify: &[],
            targets: TARGETS,
            outputs: &["color"],
            passes,
            prepass: Some("prepassFS"),
            prepass_outputs: &[],
            buffers: BUFFERS,
            scene_reads: &["tiles"],
            depth_views: None,
            volumes: &[],
            update: None,
            resolution: None,
            history: &[],
            shows: &[],
            program: None,
            nodes: Box::leak(nodes.into_boxed_slice()),
        }))
    }

    #[test]
    fn a_program_places_its_passes_where_its_nodes_are() {
        use crate::investigation::{Access, FrameNode, ImageKind, PassKind, ProgramBuffer, ProgramImage, ProgramPass, ProgramUse, ShadingProgram, Which};
        static IMAGES: &[ProgramImage] = &[ProgramImage { name: "grid", kind: ImageKind::D3, size: [8, 8, 8], format: wgpu::TextureFormat::R32Uint, count: 2, mips: 1, viewport: 0 }];
        static BUFFERS: &[ProgramBuffer] = &[ProgramBuffer { name: "list", bytes: 64, indirect: true, count: 1 }];
        static PASSES: &[ProgramPass] = &[
            ProgramPass {
                name: "buildCS",
                kind: PassKind::Compute { entry: "buildCS", workgroup: [4, 4, 4] },
                uses: &[ProgramUse { binding: 1, resource: "grid", access: Access::Write, which: Which::Instance }, ProgramUse { binding: 2, resource: "list", access: Access::Atomic, which: Which::First }],
                instances: 2,
            },
            ProgramPass {
                name: "gatherCS",
                kind: PassKind::Compute { entry: "gatherCS", workgroup: [8, 8, 1] },
                uses: &[ProgramUse { binding: 1, resource: "grid", access: Access::Load, which: Which::All }],
                instances: 1,
            },
            // This frame's colour, once the scene pass has written it.
            ProgramPass {
                name: "keepCS",
                kind: PassKind::Compute { entry: "keepCS", workgroup: [8, 8, 1] },
                uses: &[ProgramUse { binding: 1, resource: "hdr", access: Access::Load, which: Which::First }, ProgramUse { binding: 2, resource: "grid", access: Access::Write, which: Which::Mip(0) }],
                instances: 1,
            },
        ];
        static PROGRAM: ShadingProgram = ShadingProgram { images: IMAGES, buffers: BUFFERS, passes: PASSES, scene_uses: &[], init: || Box::new(()), plan: |_, _| Vec::new(), warmup: 0 };
        let s = shading(&[], &[]);
        // No chain (the scene pass writes the frame), as a program's investigation may have.
        // Its pre-pass does not run without a chain.
        static NODES: &[FrameNode] = &[FrameNode::Program(0), FrameNode::Prepass, FrameNode::Program(1), FrameNode::Scene, FrameNode::Program(2)];
        let with = |nodes: &'static [FrameNode]| -> &'static SceneShading { Box::leak(Box::new(SceneShading { program: Some(&PROGRAM), outputs: &[], scene_reads: &[], nodes, ..*s })) };
        let g = FrameGraph::new(Some(with(NODES)), &[]);
        assert!(g.errors.is_empty(), "{:?}", g.errors);
        // Its commands are recorded in three runs: before the pre-pass, between it and the scene pass, after the scene pass.
        assert_eq!(with(NODES).program_runs(), vec![0, 1, 2]);
        // A pass in no node, or in two, is an error; so is a pre-pass after the scene pass.
        let missing = FrameGraph::new(Some(with(&[FrameNode::Prepass, FrameNode::Program(0), FrameNode::Program(1), FrameNode::Scene])), &[]);
        assert!(missing.errors.iter().any(|e| e.contains("program pass `keepCS` is in no node")), "{:?}", missing.errors);
        let twice = FrameGraph::new(Some(with(&[FrameNode::Program(0), FrameNode::Program(1), FrameNode::Scene, FrameNode::Program(2), FrameNode::Program(2), FrameNode::Prepass])), &[]);
        assert!(twice.errors.iter().any(|e| e.contains("`keepCS` is in 2 nodes")), "{:?}", twice.errors);
        assert!(twice.errors.iter().any(|e| e.contains("pre-pass comes after the scene pass")), "{:?}", twice.errors);
        let names: Vec<&str> = g.order.iter().map(|&i| g.nodes[i].name).collect();
        assert_eq!(names, ["buildCS", "gatherCS", "sceneFS", "keepCS", "overlay", "composite"]);
        // It reads `hdr` after the scene pass writes it.
        let keep = g.nodes.iter().position(|n| n.name == "keepCS").unwrap();
        let scene = g.nodes.iter().position(|n| n.name == "sceneFS").unwrap();
        assert!(g.edges.iter().any(|e| (e.0, e.1) == (scene, keep)));
        // A program's passes write what outlives the frame: they run though nothing reads their writes this frame.
        assert!(g.live.iter().all(|l| *l));
        let build = &g.nodes[g.node("buildCS").unwrap()];
        assert_eq!((build.reads.clone(), build.writes.clone()), (vec!["list"], vec!["grid", "list"]));
        let grid = &g.resources[g.resource("grid").unwrap()];
        assert_eq!(grid.lifetime, Lifetime::Persistent);
        assert_eq!(grid.shape.describe(), "R32Uint, 8x8x8, 2 of them");
        assert_eq!(inspect_entry(&grid.shape).0, "inspectVolumeUintCS");
        assert_eq!(default_show(&grid.shape), Show::Hash);
    }

    #[test]
    fn order_follows_accesses_and_keeps_declaration_order() {
        let s = shading(
            &[ShadingPass { entry: "tilesCS", reads: &["depth"], writes: &["tiles"], workgroup: [16, 16] }],
            &[
                ShadingPass { entry: "blurCS", reads: &["color"], writes: &["blur"], workgroup: [8, 8] },
                ShadingPass { entry: "mergeCS", reads: &["blur", "depth"], writes: &["hdr"], workgroup: [8, 8] },
            ],
        );
        let g = FrameGraph::new(Some(s), &[]);
        assert!(g.errors.is_empty(), "{:?}", g.errors);
        let names: Vec<&str> = g.order.iter().map(|&i| g.nodes[i].name).collect();
        assert_eq!(names, ["prepassFS", "tilesCS", "sceneFS", "blurCS", "mergeCS", "overlay", "composite"]);
        assert!(g.live.iter().all(|l| *l));
        // The scene pass reads the pre-pass's depth and the tiles.
        let scene = g.node("sceneFS").unwrap();
        assert!(g.edges.contains(&(g.node("tilesCS").unwrap(), scene, Edge::Data(g.resource("tiles").unwrap()))));
        assert!(g.inspected("blurCS.blur").is_ok());
        assert!(g.inspected("blurCS.hdr").is_err());
        assert_eq!(g.result_of(g.node("composite").unwrap()), Some("hdr"));
        assert_eq!(g.result_of(g.node("blurCS").unwrap()), Some("blur"));
        assert_eq!(g.last_user("blur"), g.node("blurCS"));
        // What each node is compiled from.
        assert_eq!(source_of(Some(s), &g.nodes[g.node("blurCS").unwrap()]), (s.module, vec!["blurCS"]));
        assert_eq!(source_of(Some(s), &g.nodes[g.node("sceneFS").unwrap()]), (s.module, vec!["sceneVS", "sceneFS"]));
        assert_eq!(source_of(Some(s), &g.nodes[g.node("composite").unwrap()]).0, crate::shaders::SCENE);
        // The frame graph as a 2D scene: its sections' boxes and the edges between them.
        let section = |id: &str, passes: &[&str]| Section { id: id.into(), title: id.into(), passes: passes.iter().map(|p| p.to_string()).collect(), ..Default::default() };
        let scene = GraphScene { sections: vec![section("DEPTH", &["prepassFS", "tilesCS"]), section("SCENE", &["sceneFS"]), section("POST", &["blurCS", "mergeCS"])], omit: Vec::new() };
        let d = g.scene_graph(&scene).unwrap();
        assert_eq!(d.nodes.iter().map(|n| n.rows.len()).collect::<Vec<_>>(), [2, 1, 2]);
        assert_eq!(d.nodes[0].rows[1].label, "tiles");
        // DEPTH reaches POST through SCENE (the blur reads the scene's colour): no edge of its own.
        assert_eq!(d.edges.iter().map(|e| (e.from, e.to)).collect::<Vec<_>>(), [(0, 1), (1, 2)]);
        assert_eq!(d.named("tilesCS"), Some(0));
        let parsed = GraphScene::parse("[[section]]\nid = \"DEPTH\"\ntitle-text = \"t\"\npasses = [\"prepassFS\"]\npicture = \"prepassFS.depth\"\n", "test", &|k| (k == "t").then(|| "Depth".to_string())).unwrap();
        assert_eq!((parsed.sections[0].title.as_str(), parsed.sections[0].picture.as_deref()), ("Depth", Some("prepassFS.depth")));
        assert!(GraphScene::parse("[[section]]\nid = \"A\"\ntitle-text = \"missing\"\n", "test", &|_| None).is_err());
        let short = GraphScene { sections: vec![section("DEPTH", &["prepassFS"])], omit: Vec::new() };
        assert!(g.scene_graph(&short).unwrap_err().contains("passes in no section: tilesCS"));
        assert_eq!(g.last_user("hdr"), g.node("mergeCS"));
        // A resource's view: the investigation's, else its format's.
        let shown = FrameGraph::new(Some(Box::leak(Box::new(SceneShading { shows: &[("tiles", "count?word=3&range=0,8")], ..*s }))), &[]);
        let (show, params) = shown.view_of(shown.resource("tiles").unwrap());
        assert_eq!((show, params["word"].clone(), params["range"].clone()), (Show::Count, vec![3.0], vec![0.0, 8.0]));
        assert_eq!(shown.view_of(shown.resource("depth").unwrap()).0, Show::Depth);
        let bad = FrameGraph::new(Some(Box::leak(Box::new(SceneShading { shows: &[("tiles", "count?wurd=3"), ("nothing", "hash")], ..*s }))), &[]);
        assert_eq!(bad.errors.len(), 2, "{:?}", bad.errors);
    }

    #[test]
    fn reads_before_writes_and_culling() {
        let s = shading(
            &[],
            &[
                ShadingPass { entry: "mergeCS", reads: &["blur"], writes: &["hdr"], workgroup: [8, 8] },
                ShadingPass { entry: "blurCS", reads: &["color"], writes: &["blur"], workgroup: [8, 8] },
                ShadingPass { entry: "debugCS", reads: &["color"], writes: &["unused"], workgroup: [8, 8] },
            ],
        );
        let g = FrameGraph::new(Some(s), &[]);
        assert!(g.errors.iter().any(|e| e.contains("mergeCS reads `blur` before")), "{:?}", g.errors);
        assert!(!g.live[g.node("debugCS").unwrap()]);
        assert_eq!(g.unread(), vec![("blurCS", "blur"), ("debugCS", "unused")]);
        // An inspected resource keeps its writer alive.
        let g = FrameGraph::new(Some(s), &["unused"]);
        assert!(g.live[g.node("debugCS").unwrap()]);
    }

    #[test]
    fn history_and_repeated_entries() {
        let s = shading(
            &[ShadingPass { entry: "tilesCS", reads: &["depth"], writes: &["tiles"], workgroup: [16, 16] }],
            &[
                ShadingPass { entry: "mergeCS", reads: &["blur", "color"], writes: &["hdr"], workgroup: [8, 8] },
                ShadingPass { entry: "blurCS", reads: &["color"], writes: &["blur"], workgroup: [8, 8] },
                ShadingPass { entry: "blurCS", reads: &["color"], writes: &["blur"], workgroup: [8, 8] },
            ],
        );
        let g = FrameGraph::new(Some(Box::leak(Box::new(SceneShading { history: &["blur"], ..*s }))), &[]);
        assert!(g.errors.is_empty(), "{:?}", g.errors);
        assert_eq!(g.resources[g.resource("blur").unwrap()].lifetime, Lifetime::History);
        // The next frame reads what blurCS writes.
        assert!(g.live[g.node("blurCS").unwrap()]);
        assert!(g.node("blurCS#2").is_some());
    }

    #[test]
    fn without_a_chain() {
        let g = FrameGraph::new(None, &[]);
        assert!(g.errors.is_empty());
        let names: Vec<&str> = g.order.iter().map(|&i| g.nodes[i].name).collect();
        assert_eq!(names, ["scene", "overlay", "composite"]);
    }
}
