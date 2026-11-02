//! The frame as a render graph: nodes (passes) that read and write named
//! resources, built from an investigation's `SceneShading` and the lab's own
//! passes (the scene pass, the overlay, the composite). The renderer runs a
//! frame in the graph's order (src/renderer.rs), the viewer draws it with
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
    Image { kind: crate::investigation::ImageKind, format: wgpu::TextureFormat, size: [u32; 3], count: u32 },
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
            Shape::Image { kind, format, size, count } => {
                let extent = match kind {
                    crate::investigation::ImageKind::D2 => format!("{}x{}", size[0], size[1]),
                    crate::investigation::ImageKind::D2Array => format!("{}x{}, {} layers", size[0], size[1], size[2]),
                    crate::investigation::ImageKind::D3 => format!("{}x{}x{}", size[0], size[1], size[2]),
                };
                let count = if *count > 1 { format!(", {count} of them") } else { String::new() };
                format!("{format:?}, {extent}{count}")
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
    /// Compute pass `i` of `early` then `passes`, in one index.
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
        let chained = shading.filter(|s| s.chained());
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
                for (i, p) in u.passes.iter().enumerate() {
                    nodes.push(Node { name: p.entry, step: Step::Update(i), reads: p.reads.to_vec(), writes: p.writes.to_vec(), timing: p.entry, cached: true });
                }
            }
            if let Some(program) = s.program {
                for i in program.images {
                    resources.push(Resource { name: i.name, lifetime: Lifetime::Persistent, shape: Shape::Image { kind: i.kind, format: i.format, size: i.size, count: i.count } });
                }
                for b in program.buffers {
                    resources.push(Resource { name: b.name, lifetime: Lifetime::Persistent, shape: Shape::Bytes { bytes: b.bytes } });
                }
            }
        }
        // A program's passes of one stage, in its order.
        let program_nodes = |stage: crate::investigation::ProgramStage| -> Vec<Node> {
            let Some(program) = shading.and_then(|s| s.program) else { return Vec::new() };
            program
                .passes
                .iter()
                .enumerate()
                .filter(|(_, p)| p.stage == stage)
                .map(|(i, p)| Node { name: p.name, step: Step::Program(i), reads: program_reads(p.uses), writes: program_writes(p.uses), timing: p.name, cached: false })
                .collect()
        };
        use crate::investigation::ProgramStage;
        nodes.extend(program_nodes(ProgramStage::Start));
        // The program's scene group: what the scene pass reads of it, but what the chain's passes declare they read.
        let chain_reads: Vec<&str> = shading.map_or(Vec::new(), |s| s.early.iter().chain(s.passes).flat_map(|p| p.reads.iter().copied()).collect());
        let scene_uses: Vec<&'static str> = shading.and_then(|s| s.program).map_or(Vec::new(), |p| program_reads(p.scene_uses)).into_iter().filter(|r| !chain_reads.contains(r)).collect();
        resources.push(Resource { name: "overlay", lifetime: Lifetime::Imported, shape: Shape::Texture { format: crate::gpu::OVERLAY_FORMAT, divisor: 1, samples: 1 } });
        resources.push(Resource { name: "frame", lifetime: Lifetime::Imported, shape: Shape::Texture { format: crate::gpu::FRAME_FORMAT, divisor: 1, samples: 1 } });
        match chained {
            Some(s) => {
                if let Some(d) = s.depth_views {
                    nodes.push(Node { name: "depthViews", step: Step::DepthViews, reads: Vec::new(), writes: vec![d.name], timing: "depth views", cached: true });
                }
                if let Some(entry) = s.prepass {
                    let mut writes = vec!["depth"];
                    writes.extend(s.prepass_outputs.iter().copied());
                    nodes.push(Node { name: entry, step: Step::Prepass, reads: Vec::new(), writes, timing: "depth pre-pass", cached: false });
                }
                nodes.extend(program_nodes(ProgramStage::AfterPrepass));
                let mut named: Vec<&'static str> = Vec::new();
                let mut unique = |entry: &'static str| -> &'static str {
                    let k = named.iter().filter(|n| **n == entry).count();
                    named.push(entry);
                    if k == 0 { entry } else { Box::leak(format!("{entry}#{}", k + 1).into_boxed_str()) }
                };
                for (i, p) in s.early.iter().enumerate() {
                    nodes.push(Node { name: unique(p.entry), step: Step::Compute(i), reads: p.reads.to_vec(), writes: p.writes.to_vec(), timing: p.entry, cached: false });
                }
                nodes.extend(program_nodes(ProgramStage::BeforeScene));
                let mut reads = s.scene_reads.to_vec();
                reads.extend(scene_uses.iter().copied());
                // Without outputs, the scene pass writes the frame's HDR target, as without a chain.
                let mut writes = if s.outputs.is_empty() { vec!["hdr"] } else { s.outputs.to_vec() };
                if s.prepass.is_some() {
                    reads.push("depth");
                } else {
                    writes.push("depth");
                }
                nodes.push(Node { name: s.fragment, step: Step::Scene, reads, writes, timing: "scene pass", cached: false });
                for (i, p) in s.passes.iter().enumerate() {
                    nodes.push(Node { name: unique(p.entry), step: Step::Compute(s.early.len() + i), reads: p.reads.to_vec(), writes: p.writes.to_vec(), timing: p.entry, cached: false });
                }
            }
            None => {
                nodes.extend(program_nodes(ProgramStage::AfterPrepass));
                nodes.extend(program_nodes(ProgramStage::BeforeScene));
                let name = shading.map_or("scene", |s| s.fragment);
                nodes.push(Node { name, step: Step::Scene, reads: scene_uses.clone(), writes: vec!["hdr", "depth"], timing: "scene pass", cached: false });
            }
        }
        nodes.push(Node { name: "overlay", step: Step::Overlay, reads: vec!["depth"], writes: vec!["overlay"], timing: "overlay", cached: false });
        nodes.push(Node { name: "composite", step: Step::Composite, reads: vec!["hdr", "overlay"], writes: vec!["frame"], timing: "composite", cached: false });
        let shows = shading.map_or(Vec::new(), |s| s.shows.to_vec());
        let mut graph = FrameGraph { nodes, resources, edges: Vec::new(), order: Vec::new(), live: Vec::new(), errors: Vec::new(), shows };
        if let Some(s) = shading.filter(|s| !s.chained() && !(s.early.is_empty() && s.passes.is_empty())) {
            graph.errors.push(format!("{}: compute passes without scene or pre-pass outputs never run (SceneShading::outputs is empty)", s.fragment));
        }
        if let Some(s) = shading.filter(|s| s.outputs.is_empty() && !s.passes.is_empty()) {
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
                if n.reads.iter().filter(|r| *r == name).count() > 1 || n.writes.iter().filter(|w| *w == name).count() > 1 {
                    if seen.insert(*name) {
                        self.errors.push(format!("{}: lists `{name}` twice", n.name));
                    }
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
        type Cache = Mutex<Vec<((String, bool), Option<&'static GraphScene>)>>;
        static CACHE: OnceLock<Cache> = OnceLock::new();
        let package = crate::investigation::current().package()?;
        let key = (format!("{package}.graph/{name}"), crate::i18n::zh());
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
        let entry = |n: &str| n.split_once('#').map_or(n, |(e, _)| e).to_string();
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
            if let (Some(a), Some(b)) = (a, b) {
                if a != b && !edges.iter().any(|e| e.0 == *a && e.1 == *b) {
                    edges.push((a.clone(), b.clone()));
                }
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
    let entry = node.name.split_once('#').map_or(node.name, |(e, _)| e);
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

/// What node `name` does, in the interface's language: the lab's own nodes'
/// text, else the investigation's (`Investigation::pass_about`); a repeated
/// entry (`name#2`) as its first.
pub fn about(name: &str) -> Option<&'static str> {
    let name = name.split_once('#').map_or(name, |(n, _)| n);
    match name {
        "depthViews" => Some(crate::i18n::tr("graph.about_depth_views")),
        "scene" => Some(crate::i18n::tr("graph.about_scene")),
        "overlay" => Some(crate::i18n::tr("graph.about_overlay")),
        "composite" => Some(crate::i18n::tr("graph.about_composite")),
        _ => crate::investigation::current().pass_about(name),
    }
}

/// A view of a resource as text, `<show>[?<param>=<value>[,<value>]&…]`
/// (`SceneShading::shows`, the inspector's page): its visualizer, if named,
/// and its parameters, checked.
pub fn parse_view(spec: &str) -> Result<(Option<Show>, std::collections::BTreeMap<String, Vec<f32>>), String> {
    let (show, query) = spec.split_once('?').unwrap_or((spec, ""));
    let show = if show.is_empty() { None } else { Some(Show::from_key(show)?) };
    Ok((show, parse_params(query)?))
}

/// The inspector's parameters as text, `<param>=<value>[,<value>]&…`, checked.
pub fn parse_params(query: &str) -> Result<std::collections::BTreeMap<String, Vec<f32>>, String> {
    let mut params = std::collections::BTreeMap::new();
    for pair in query.split('&').filter(|p| !p.is_empty()) {
        let (name, value) = pair.split_once('=').ok_or_else(|| format!("`{pair}`: expected name=value"))?;
        let value: Vec<f32> = value.split(',').map(|v| v.trim().parse::<f32>()).collect::<Result<_, _>>().map_err(|e| format!("`{pair}`: {e}"))?;
        check_inspect_param(name, &value)?;
        params.insert(name.to_string(), value);
    }
    Ok(params)
}

/// How the inspector draws a resource (`imageShow=`; packages/rlstd/shaders/inspect.wgsl).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Show {
    /// HDR colour: `exposure` stops, then sRGB.
    Color,
    /// Stored values as they are (albedo, masks); `channel` picks one.
    Unorm,
    /// A normal: `encoding` 0 signed xyz, 1 unsigned xyz, 2 octahedral xy.
    Normal,
    /// Reversed depth on a logarithmic ramp over `range` (near bright).
    Depth,
    /// One channel (`channel`) on a ramp over `range`.
    Ramp,
    /// One channel, diverging around 0 over `range`.
    Signed,
    /// A buffer's word (`word` of each tile's) as a count on a ramp over `range`; 0 black.
    Count,
    /// A buffer's word read as a float, on a ramp over `range`.
    Float,
    /// Distinct colours per value (ids, stencil references); 0 black.
    Hash,
}

impl Show {
    pub const ALL: [Show; 9] = [Show::Color, Show::Unorm, Show::Normal, Show::Depth, Show::Ramp, Show::Signed, Show::Count, Show::Float, Show::Hash];

    pub fn key(self) -> &'static str {
        match self {
            Show::Color => "color",
            Show::Unorm => "unorm",
            Show::Normal => "normal",
            Show::Depth => "depth",
            Show::Ramp => "ramp",
            Show::Signed => "signed",
            Show::Count => "count",
            Show::Float => "float",
            Show::Hash => "hash",
        }
    }

    pub fn from_key(key: &str) -> Result<Show, String> {
        Show::ALL.into_iter().find(|s| s.key() == key).ok_or_else(|| format!("imageShow `{key}`: one of {}", Show::ALL.map(Show::key).join(", ")))
    }

    /// Its index in the shader (`InspectParams::mode`).
    pub fn mode(self) -> u32 {
        Show::ALL.iter().position(|s| *s == self).expect("a show") as u32
    }

    /// The range it maps to 0..1 unless `image.range` says.
    pub fn range(self) -> [f32; 2] {
        match self {
            Show::Depth => [1e-3, 1.0],
            Show::Signed => [-1.0, 1.0],
            Show::Count => [0.0, 64.0],
            _ => [0.0, 1.0],
        }
    }
}

/// The inspector's parameters (`image.<key>=` on a graph image): key,
/// components, default (`range`: the show's), and range.
pub const INSPECT_PARAMS: &[(&str, usize, f32, (f32, f32))] = &[
    ("exposure", 1, 0.0, (-16.0, 16.0)),
    ("channel", 1, 0.0, (0.0, 4.0)),
    ("encoding", 1, 0.0, (0.0, 3.0)),
    ("average", 1, 0.0, (0.0, 1.0)),
    ("range", 2, 0.0, (-1e9, 1e9)),
    ("slice", 1, 0.5, (0.0, 1.0)),
    ("level", 1, 0.0, (0.0, 64.0)),
    ("axis", 1, 1.0, (0.0, 2.0)),
    ("word", 1, 0.0, (0.0, 1e6)),
    // An integer image's bit field shown: its first bit and its width (0: the whole value).
    ("bits", 2, 0.0, (0.0, 32.0)),
    // Which image of a binding array (`ProgramImage::count`).
    ("index", 1, 0.0, (0.0, 64.0)),
];

/// Checks a graph image's parameter.
pub fn check_inspect_param(key: &str, value: &[f32]) -> Result<(), String> {
    let Some(&(_, components, _, (lo, hi))) = INSPECT_PARAMS.iter().find(|p| p.0 == key) else {
        return Err(format!("image.{key}: a graph image takes {}", INSPECT_PARAMS.iter().map(|p| p.0).collect::<Vec<_>>().join(", ")));
    };
    if value.len() != components {
        return Err(format!("image.{key} takes {components} number(s), not {}", value.len()));
    }
    if let Some(v) = value.iter().find(|v| !(lo..=hi).contains(*v)) {
        return Err(format!("image.{key} = {v}: outside {lo}..{hi}"));
    }
    Ok(())
}

/// The inspector's entry point for a resource of this shape (inspect.wgsl), and its source's binding.
pub fn inspect_entry(shape: &Shape) -> (&'static str, u32) {
    match shape {
        Shape::Texture { format: wgpu::TextureFormat::Depth32Float, samples, .. } if *samples > 1 => ("inspectDepthMsCS", 4),
        Shape::Texture { samples, .. } if *samples > 1 => ("inspectTextureMsCS", 3),
        Shape::Texture { .. } => ("inspectTextureCS", 2),
        Shape::Buffer { .. } | Shape::UpdateBuffer => ("inspectBufferCS", 6),
        Shape::DepthArray { .. } => ("inspectDepthArrayCS", 5),
        Shape::Volume { .. } => ("inspectVolumeCS", 7),
        Shape::Bytes { .. } => ("inspectBufferCS", 6),
        Shape::Image { kind, format, .. } => {
            use crate::investigation::ImageKind;
            let int = image_class(*format);
            match (kind, int) {
                (ImageKind::D2, 0) => ("inspectTextureCS", 2),
                (ImageKind::D3, 0) => ("inspectVolumeCS", 7),
                (ImageKind::D3, 1) => ("inspectVolumeUintCS", 8),
                (ImageKind::D3, _) => ("inspectVolumeSintCS", 9),
                (ImageKind::D2Array, 0) => ("inspectArrayCS", 10),
                (ImageKind::D2Array, 1) | (ImageKind::D2, 1) => ("inspectArrayUintCS", 11),
                (ImageKind::D2Array, _) | (ImageKind::D2, _) => ("inspectArraySintCS", 12),
            }
        }
    }
}

/// A format's class for the inspector: 0 float (unorm, snorm, float), 1 unsigned integer, 2 signed integer.
pub fn image_class(format: wgpu::TextureFormat) -> u32 {
    match format.sample_type(None, None) {
        Some(wgpu::TextureSampleType::Uint) => 1,
        Some(wgpu::TextureSampleType::Sint) => 2,
        _ => 0,
    }
}

/// The resources a program pass's uses read and write (samplers are neither).
pub fn program_reads(uses: &[crate::investigation::ProgramUse]) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for u in uses.iter().filter(|u| u.access.reads()) {
        if !out.contains(&u.resource) {
            out.push(u.resource);
        }
    }
    out
}

pub fn program_writes(uses: &[crate::investigation::ProgramUse]) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for u in uses.iter().filter(|u| u.access.writes()) {
        if !out.contains(&u.resource) {
            out.push(u.resource);
        }
    }
    out
}

/// How a resource shows unless `imageShow` says.
pub fn default_show(shape: &Shape) -> Show {
    match shape {
        Shape::Texture { format, .. } => match format {
            wgpu::TextureFormat::Depth32Float => Show::Depth,
            wgpu::TextureFormat::Rgba16Float | wgpu::TextureFormat::Rgba32Float | wgpu::TextureFormat::Rg11b10Ufloat => Show::Color,
            wgpu::TextureFormat::R8Unorm | wgpu::TextureFormat::R16Float | wgpu::TextureFormat::R32Float => Show::Ramp,
            _ => Show::Unorm,
        },
        // One word a pixel holds a value (a depth, a factor), read as f32; more, counts and indices.
        Shape::Buffer { words_per_tile: 1, .. } => Show::Float,
        Shape::Buffer { .. } | Shape::UpdateBuffer => Show::Count,
        Shape::DepthArray { .. } => Show::Depth,
        Shape::Volume { .. } => Show::Color,
        Shape::Bytes { .. } => Show::Count,
        Shape::Image { format, .. } => match image_class(*format) {
            0 => default_show(&Shape::Texture { format: *format, divisor: 1, samples: 1 }),
            1 => Show::Hash,
            _ => Show::Signed,
        },
    }
}

/// Whether `name` is one of `shading`'s volumes (a chain pass lists the
/// volumes it samples in its reads; they are bound at group 3, not group 2).
pub fn is_volume(shading: &SceneShading, name: &str) -> bool {
    shading.volumes.iter().any(|v| v.name == name)
}

/// Whether a chain pass binds `name` at group 3: a volume, or a resource of
/// the program's scene group (a shading without volumes).
pub fn is_group3(shading: &SceneShading, name: &str) -> bool {
    is_volume(shading, name) || (shading.volumes.is_empty() && shading.program.is_some_and(|p| p.scene_uses.iter().any(|u| u.resource == name)))
}

/// Checks `shading`'s declarations against its WGSL: every resource an
/// entry point reads is in its pass's `reads`, every one it writes in its
/// `writes`, and every one listed is used so. A buffer is bound
/// `read_write` whichever list names it; the lists say which it does.
/// Returns what is wrong, one message each.
pub fn lint(shading: &SceneShading) -> Result<Vec<String>> {
    let (module, info) = crate::shaders::analyse(shading.module)?;
    let mut problems = Vec::new();
    let uses = |entry: &str| -> Option<Vec<(u32, u32, GlobalUse)>> {
        let index = module.entry_points.iter().position(|e| e.name == entry)?;
        let function = info.get_entry_point(index);
        Some(
            module
                .global_variables
                .iter()
                .filter_map(|(handle, var)| {
                    let binding = var.binding.as_ref()?;
                    let used = function[handle];
                    (!used.is_empty()).then_some((binding.group, binding.binding, used))
                })
                .collect(),
        )
    };
    // Group 2 of the chain: what each binding is.
    let mut chain: Vec<(u32, &str, bool)> = Vec::new();
    for t in shading.targets {
        chain.push((chain_binding(shading, t.name, false), t.name, false));
        chain.push((chain_binding(shading, t.name, true), t.name, true));
    }
    chain.push((chain_binding(shading, "depth", false), "depth", false));
    chain.push((chain_binding(shading, "hdr", true), "hdr", true));
    let buffers: Vec<&str> = shading.buffers.iter().map(|b| b.name).collect();
    for b in &buffers {
        chain.push((chain_binding(shading, b, false), b, false));
    }
    if let Some(d) = shading.depth_views {
        chain.push((chain_binding(shading, d.name, false), d.name, false));
    }
    // Group 3 of a chain pass: the volumes, or (a shading without volumes) the program's scene group.
    let volume = |binding: u32| {
        if shading.volumes.is_empty() {
            return shading.program.and_then(|p| p.scene_uses.iter().find(|u| u.binding == binding)).map(|u| u.resource);
        }
        shading.volumes.get(binding.wrapping_sub(1) as usize).map(|v| v.name)
    };
    // A chain pass, the scene pass (`writes` None: it may only read), or the pre-pass (`subset`: only what it uses is checked).
    let mut check_chain = |entry: &str, reads: &[&str], writes: Option<&[&str]>, subset: bool| {
        let Some(used) = uses(entry) else {
            problems.push(format!("{entry}: no such entry point in {}", shading.module));
            return;
        };
        let mut seen: Vec<(&str, bool)> = Vec::new();
        for &(group, binding, flags) in &used {
            match group {
                2 => {
                    let Some(&(_, name, written)) = chain.iter().find(|c| c.0 == binding) else {
                        problems.push(format!("{entry}: binds group 2 binding {binding}, which is none of the chain's resources"));
                        continue;
                    };
                    if buffers.contains(&name) {
                        if flags.intersects(GlobalUse::READ | GlobalUse::QUERY) {
                            seen.push((name, false));
                            if !reads.contains(&name) && flags.contains(GlobalUse::READ) {
                                problems.push(format!("{entry}: reads buffer `{name}` but does not list it in its reads"));
                            }
                        }
                        if flags.contains(GlobalUse::WRITE) {
                            seen.push((name, true));
                            match writes {
                                Some(w) if !w.contains(&name) => problems.push(format!("{entry}: writes buffer `{name}` but does not list it in its writes")),
                                None => problems.push(format!("{entry}: writes buffer `{name}`, which a scene pass cannot list (it only reads)")),
                                _ => {}
                            }
                        }
                    } else {
                        seen.push((name, written));
                        let listed = if written { writes.is_some_and(|w| w.contains(&name)) } else { reads.contains(&name) };
                        if !listed {
                            problems.push(format!("{entry}: {} `{name}` but does not list it in its {}", if written { "writes" } else { "reads" }, if written { "writes" } else { "reads" }));
                        }
                    }
                }
                3 if binding > 0 => {
                    let Some(name) = volume(binding) else { continue };
                    seen.push((name, false));
                    if !reads.contains(&name) {
                        problems.push(format!("{entry}: samples volume `{name}` but does not list it in its reads"));
                    }
                }
                _ => {}
            }
        }
        if subset {
            return;
        }
        for name in reads {
            if !seen.contains(&(name, false)) {
                problems.push(format!("{entry}: lists `{name}` in its reads but does not read it"));
            }
        }
        for name in writes.unwrap_or(&[]) {
            if !seen.contains(&(name, true)) {
                problems.push(format!("{entry}: lists `{name}` in its writes but does not write it"));
            }
        }
    };
    for pass in shading.early.iter().chain(shading.passes) {
        check_chain(pass.entry, pass.reads, Some(pass.writes), false);
    }
    if shading.chained() {
        check_chain(shading.fragment, shading.scene_reads, None, false);
        // The pre-pass runs before the passes that fill the scene's reads: it reads none of them.
        if let Some(entry) = shading.prepass {
            check_chain(entry, &[], None, true);
        }
    }
    drop(check_chain);
    for pass in shading.early.iter().chain(shading.passes) {
        if pass.writes.iter().any(|w| is_volume(shading, w)) {
            problems.push(format!("{}: a chain pass cannot write a volume (an update pass does)", pass.entry));
        }
    }
    // The update passes: group 2 their parameters, the update's buffers, the volumes written; group 3 the volumes read.
    if let Some(update) = shading.update {
        let nb = update.buffers.len() as u32;
        for pass in update.passes {
            let Some(used) = uses(pass.entry) else {
                problems.push(format!("{}: no such entry point in {}", pass.entry, shading.module));
                continue;
            };
            let mut seen: Vec<(&str, bool)> = Vec::new();
            for &(group, binding, flags) in &used {
                match (group, binding) {
                    (2, 0) => {}
                    (2, b) if b <= nb => {
                        let name = update.buffers[(b - 1) as usize].name;
                        if flags.contains(GlobalUse::READ) {
                            seen.push((name, false));
                            if !pass.reads.contains(&name) {
                                problems.push(format!("{}: reads buffer `{name}` but does not list it in its reads", pass.entry));
                            }
                        }
                        if flags.contains(GlobalUse::QUERY) && pass.reads.contains(&name) {
                            seen.push((name, false));
                        }
                        if flags.contains(GlobalUse::WRITE) {
                            seen.push((name, true));
                            if !pass.writes.contains(&name) {
                                problems.push(format!("{}: writes buffer `{name}` but does not list it in its writes", pass.entry));
                            }
                        }
                    }
                    (2, b) => {
                        if let Some(v) = shading.volumes.get((b - 1 - nb) as usize) {
                            seen.push((v.name, true));
                            if !pass.writes.contains(&v.name) {
                                problems.push(format!("{}: writes volume `{}` but does not list it in its writes", pass.entry, v.name));
                            }
                        }
                    }
                    (3, b) if b > 0 => {
                        if let Some(name) = volume(b) {
                            seen.push((name, false));
                            if !pass.reads.contains(&name) {
                                problems.push(format!("{}: samples volume `{name}` but does not list it in its reads", pass.entry));
                            }
                        }
                    }
                    _ => {}
                }
            }
            for name in pass.reads {
                if !seen.contains(&(name, false)) {
                    problems.push(format!("{}: lists `{name}` in its reads but does not read it", pass.entry));
                }
            }
            for name in pass.writes {
                if !seen.contains(&(name, true)) {
                    problems.push(format!("{}: lists `{name}` in its writes but does not write it", pass.entry));
                }
            }
        }
    }
    if let Some(program) = shading.program {
        let chain_entries: Vec<&str> = if shading.volumes.is_empty() { shading.early.iter().chain(shading.passes).map(|p| p.entry).collect() } else { Vec::new() };
        lint_program(program, &uses, &mut problems, shading.module, shading.fragment, &chain_entries);
    }
    Ok(problems)
}

/// A program's passes against their WGSL: every group-3 binding an entry
/// point uses is one of its `uses`, used as declared (a write where it
/// writes, atomics where `Atomic`), and every use is used.
fn lint_program(program: &crate::investigation::ShadingProgram, uses: &dyn Fn(&str) -> Option<Vec<(u32, u32, GlobalUse)>>, problems: &mut Vec<String>, module: &str, fragment: &str, chain_entries: &[&str]) {
    use crate::investigation::{Access, PassKind};
    let known = |name: &str| program.images.iter().any(|i| i.name == name) || program.buffers.iter().any(|b| b.name == name) || matches!(name, "linearSampler" | "nearestSampler");
    let mut check = |label: &str, entries: &[&str], declared: &[crate::investigation::ProgramUse], read_only: bool| {
        let mut used: Vec<(u32, GlobalUse)> = Vec::new();
        for entry in entries {
            let Some(u) = uses(entry) else {
                problems.push(format!("{label}: no entry point `{entry}` in {module}"));
                return;
            };
            for (group, binding, flags) in u {
                if group != 3 {
                    continue;
                }
                match used.iter_mut().find(|x| x.0 == binding) {
                    Some(x) => x.1 |= flags,
                    None => used.push((binding, flags)),
                }
            }
        }
        for d in declared {
            if !known(d.resource) {
                problems.push(format!("{label}: uses `{}`, which is none of the program's images, buffers, or samplers", d.resource));
            }
            if read_only && d.access.writes() {
                problems.push(format!("{label}: the scene pass may only read `{}`", d.resource));
            }
        }
        for (binding, flags) in &used {
            let Some(d) = declared.iter().find(|d| d.binding == *binding) else {
                problems.push(format!("{label}: binds group 3 binding {binding}, which none of its uses declares"));
                continue;
            };
            if flags.contains(GlobalUse::WRITE) && !d.access.writes() {
                problems.push(format!("{label}: writes `{}` (binding {binding}), declared {:?}", d.resource, d.access));
            }
            // naga marks texture atomics; a buffer's atomics are reads and writes.
            let image = program.images.iter().any(|i| i.name == d.resource);
            if image && flags.contains(GlobalUse::ATOMIC) != (d.access == Access::Atomic) && !matches!(d.access, Access::Sampler | Access::Uniform) {
                problems.push(format!("{label}: `{}` (binding {binding}) is {}used with atomics, declared {:?}", d.resource, if flags.contains(GlobalUse::ATOMIC) { "" } else { "not " }, d.access));
            }
            if d.access.writes() && !flags.intersects(GlobalUse::WRITE | GlobalUse::ATOMIC) {
                problems.push(format!("{label}: declares `{}` {:?} but only reads it", d.resource, d.access));
            }
        }
        for d in declared {
            if !used.iter().any(|(b, _)| *b == d.binding) {
                problems.push(format!("{label}: declares `{}` at binding {} but does not use it", d.resource, d.binding));
            }
        }
    };
    for pass in program.passes {
        match pass.kind {
            PassKind::Compute { entry, .. } => check(pass.name, &[entry], pass.uses, false),
            PassKind::Raster { vertex, fragment } => check(pass.name, &[vertex, fragment], pass.uses, false),
        }
    }
    // The scene group: read by the scene pass and the chain's compute passes (`chain`).
    if !program.scene_uses.is_empty() {
        let mut entries = vec![fragment];
        entries.extend(chain_entries.iter().copied());
        check(fragment, &entries, program.scene_uses, true);
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::investigation::{ShadingBuffer, ShadingPass, ShadingTarget};

    fn shading(early: &'static [ShadingPass], passes: &'static [ShadingPass]) -> &'static SceneShading {
        static TARGETS: &[ShadingTarget] = &[
            ShadingTarget { name: "color", format: wgpu::TextureFormat::Rgba16Float, divisor: 1 },
            ShadingTarget { name: "blur", format: wgpu::TextureFormat::Rgba16Float, divisor: 1 },
            ShadingTarget { name: "unused", format: wgpu::TextureFormat::Rgba16Float, divisor: 1 },
        ];
        static BUFFERS: &[ShadingBuffer] = &[ShadingBuffer { name: "tiles", words_per_tile: 4, tile: 16 }];
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
            early,
            buffers: BUFFERS,
            scene_reads: &["tiles"],
            depth_views: None,
            volumes: &[],
            update: None,
            resolution: None,
            history: &[],
            shows: &[],
            program: None,
        }))
    }

    #[test]
    fn a_program_places_its_passes_at_their_stages() {
        use crate::investigation::{Access, ImageKind, PassKind, ProgramBuffer, ProgramImage, ProgramPass, ProgramStage, ProgramUse, ShadingProgram, Which};
        static IMAGES: &[ProgramImage] = &[ProgramImage { name: "grid", kind: ImageKind::D3, size: [8, 8, 8], format: wgpu::TextureFormat::R32Uint, count: 2 }];
        static BUFFERS: &[ProgramBuffer] = &[ProgramBuffer { name: "list", bytes: 64, indirect: true, count: 1 }];
        static PASSES: &[ProgramPass] = &[
            ProgramPass {
                name: "buildCS",
                kind: PassKind::Compute { entry: "buildCS", workgroup: [4, 4, 4] },
                uses: &[ProgramUse { binding: 1, resource: "grid", access: Access::Write, which: Which::Instance }, ProgramUse { binding: 2, resource: "list", access: Access::Atomic, which: Which::First }],
                stage: ProgramStage::Start,
                instances: 2,
            },
            ProgramPass {
                name: "gatherCS",
                kind: PassKind::Compute { entry: "gatherCS", workgroup: [8, 8, 1] },
                uses: &[ProgramUse { binding: 1, resource: "grid", access: Access::Load, which: Which::All }],
                stage: ProgramStage::BeforeScene,
                instances: 1,
            },
        ];
        static PROGRAM: ShadingProgram = ShadingProgram { images: IMAGES, buffers: BUFFERS, passes: PASSES, scene_uses: &[], init: || Box::new(()), plan: |_, _| Vec::new() };
        let s = shading(&[], &[]);
        // No chain (the scene pass writes the frame), as a program's investigation may have.
        let g = FrameGraph::new(Some(Box::leak(Box::new(SceneShading { program: Some(&PROGRAM), outputs: &[], scene_reads: &[], ..*s }))), &[]);
        assert!(g.errors.is_empty(), "{:?}", g.errors);
        let names: Vec<&str> = g.order.iter().map(|&i| g.nodes[i].name).collect();
        assert_eq!(names, ["buildCS", "gatherCS", "sceneFS", "overlay", "composite"]);
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
