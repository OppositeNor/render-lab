//! The inspector: a resource drawn right after the pass that writes it, the
//! frame graph's images (its DAG, its resources' cards), and the graph and
//! performance panels' views.

use super::*;

/// The targets a frame's passes write and the inspector reads: the
/// chain's (a chained shading's), the shading's own (its program's
/// resources), and the viewport's.
#[derive(Clone, Copy)]
pub(super) struct FrameTargets<'a> {
    pub chain: Option<&'a ChainTargets>,
    pub shading: Option<&'a Shading>,
    pub targets: &'a ViewportTargets,
}

/// A story's (or the graph panel's) image of a resource of the frame's
/// graph (src/graph.rs, `ImageSource::Graph`): its texture, written right
/// after the node it names by the inspector (packages/render_lab/shaders/inspect.wgsl).
pub(super) struct Inspection {
    /// `<node>.<resource>`, and the node by its index in the graph.
    pub(super) key: String,
    pub(super) node: usize,
    pub(super) resource: &'static str,
    pub(super) entry: &'static str,
    pub(super) binding: u32,
    pub(super) _texture: wgpu::Texture,
    pub(super) storage: wgpu::TextureView,
    pub(super) size: [u32; 2],
    pub(super) params: wgpu::Buffer,
    pub(super) id: egui::TextureId,
    /// Which image of a program's binding array (`image.index`).
    pub(super) index: u32,
    /// The mip level shown of a 2D program image (`image.level`).
    pub(super) level: u32,
}

/// The most texels along an inspector texture's longer side.
pub(super) const INSPECT_SIDE: u32 = 640;

/// The DAG panel's images (`PanelMode::Dag`): one per node, named this
/// prefix and the node, smaller than the inspector's and not keeping their
/// writers alive (the view shows culled nodes as culled).
pub(super) const DAG_IMAGE: &str = "@dag/";
pub(super) const DAG_SIDE: u32 = 256;
/// The resources panel's images (`PanelMode::Resources`): one per resource,
/// its state at the end of the frame (`FrameGraph::last_user`).
pub(super) const RESOURCE_IMAGE: &str = "@resource/";

/// The frame graph scene's pictures (a 2D beat's `graph`): one per lit box,
/// its last member's result (src/diagram.rs `BeatDiagram::lit_groups`).
pub(super) const BOX_IMAGE: &str = "@box/";

/// An image of a graph panel's cards, not one a story or the inspector shows.
pub(super) fn card_image(name: &str) -> bool {
    name.starts_with(DAG_IMAGE) || name.starts_with(RESOURCE_IMAGE) || name.starts_with(BOX_IMAGE)
}

impl Renderer {
    /// What the graph panel shows (`PanelMode::Graph`): the frame's graph,
    /// each node's last GPU time, and the inspected resource.
    pub(super) fn graph_view(&mut self) -> ui::GraphView {
        let graph = self.frame_graph();
        let last: Vec<(String, f64)> = self
            .timing
            .as_ref()
            .map(|t| t.last().to_vec())
            .unwrap_or_default();
        let nodes = graph
            .order
            .iter()
            .map(|&i| self.graph_node_view(&graph, &last, i))
            .collect();
        // The DAG's arrows, between rows: what flows from one node to the next
        // (its resources), or an order without data (a later write).
        let edges = Self::dag_edges(&graph);
        let users = |r: &crate::graph::Resource, writes: bool| -> Vec<String> {
            graph
                .order
                .iter()
                .map(|&i| &graph.nodes[i])
                .filter(|n| {
                    if writes {
                        n.writes.contains(&r.name)
                    } else {
                        n.reads.contains(&r.name)
                    }
                })
                .map(|n| n.name.to_string())
                .collect()
        };
        let resources = graph
            .resources
            .iter()
            .map(|r| ui::GraphResourceView {
                name: r.name.to_string(),
                lifetime: r.lifetime,
                shape: r.shape.describe(),
                writers: users(r, true),
                readers: users(r, false),
                key: graph
                    .last_user(r.name)
                    .filter(|_| r.name != "frame")
                    .map(|n| format!("{}.{}", graph.nodes[n].name, r.name)),
                thumbnail: self
                    .inspections
                    .get(&format!("{RESOURCE_IMAGE}{}", r.name))
                    .map(|x| (x.id, x.size)),
            })
            .collect();
        let mut uses = Vec::new();
        for (row, &i) in graph.order.iter().enumerate() {
            let n = &graph.nodes[i];
            for (c, r) in graph.resources.iter().enumerate() {
                let (reads, writes) = (n.reads.contains(&r.name), n.writes.contains(&r.name));
                if reads || writes {
                    uses.push((row, c, reads, writes));
                }
            }
        }
        let inspected = self
            .inspections
            .get(crate::feature::graph::INSPECTOR)
            .map(|x| x.key.clone());
        ui::GraphView {
            nodes,
            resources,
            uses,
            edges,
            inspected,
            timestamps: self.timing.is_some(),
        }
    }

    /// Node `i`'s row in the graph panel: its engine pass, its share of its entry's last GPU time (`last`), its
    /// result and that result's card.
    fn graph_node_view(
        &self,
        graph: &crate::graph::FrameGraph,
        last: &[(String, f64)],
        i: usize,
    ) -> ui::GraphNodeView {
        let n = &graph.nodes[i];
        // Two nodes of one entry share its row in the timing: each shows half.
        let sharing = graph
            .nodes
            .iter()
            .filter(|m| m.timing == n.timing)
            .count()
            .max(1) as f64;
        let ms = last
            .iter()
            .find(|(name, _)| name == n.timing)
            .map(|(_, ms)| ms / sharing);
        let index = crate::sources::index();
        let engine = index.pass(n.name).map(|p| {
            let at =
                p.sources
                    .first()
                    .and_then(|k| index.entries.get(k))
                    .map_or(String::new(), |e| {
                        format!(
                            "{}:{}",
                            e.path.rsplit('/').next().unwrap_or(&e.path),
                            e.line
                        )
                    });
            (p.engine.clone(), at)
        });
        let result = graph.result_of(i).map(str::to_string);
        let thumbnail = self
            .inspections
            .get(&format!("{DAG_IMAGE}{}", n.name))
            .map(|x| (x.id, x.size));
        ui::GraphNodeView {
            name: n.name.to_string(),
            engine,
            ms,
            live: graph.live[i],
            cached: n.cached,
            result,
            thumbnail,
            about: crate::graph::about(n.name),
            reads: n.reads.iter().map(|r| r.to_string()).collect(),
            writes: n.writes.iter().map(|w| w.to_string()).collect(),
        }
    }

    /// The DAG's arrows, between rows: what flows from one node to the next
    /// (its resources), or an order without data (a later write).
    fn dag_edges(graph: &crate::graph::FrameGraph) -> Vec<ui::GraphEdgeView> {
        let row_of = |i: usize| {
            graph
                .order
                .iter()
                .position(|&o| o == i)
                .expect("every node is in the order")
        };
        let mut edges: Vec<ui::GraphEdgeView> = Vec::new();
        for &(a, b, kind) in &graph.edges {
            // Edges between the same two rows merge into one, which lists the resources each data edge carries.
            let (from, to) = (row_of(a), row_of(b));
            let at = match edges.iter().position(|e| e.from == from && e.to == to) {
                Some(k) => k,
                None => {
                    edges.push(ui::GraphEdgeView {
                        from,
                        to,
                        data: Vec::new(),
                    });
                    edges.len() - 1
                }
            };
            if let crate::graph::Edge::Data(r) = kind {
                edges[at].data.push(r);
            }
        }
        edges
    }

    /// What the performance panel shows (`PanelMode::Performance`): the GPU,
    /// each pass's times, and the recent frames' GPU and frame times.
    pub(super) fn performance_view(&self) -> ui::PerformanceView {
        let starts: Vec<&std::time::Instant> = self.frame_starts.iter().collect();
        let frames = starts
            .windows(2)
            .map(|w| (*w[1] - *w[0]).as_secs_f64() * 1000.0)
            .collect();
        let (passes, totals) = match &self.timing {
            Some(t) => {
                let last = t.last();
                // The passes the last measured frame ran: a pass that runs only
                // on a change (an update, the depth views) shows while it runs.
                let passes = t
                    .summary_last(30)
                    .into_iter()
                    .filter_map(|(name, mean, p95)| {
                        let now = last.iter().find(|(n, _)| *n == name)?.1;
                        Some((name, mean, p95, now))
                    })
                    .collect();
                (passes, t.totals())
            }
            None => (Vec::new(), Vec::new()),
        };
        ui::PerformanceView {
            gpu: self.gpu.info.name.clone(),
            timestamps: self.timing.is_some(),
            passes,
            totals,
            frames,
        }
    }

    /// Makes (or keeps) the texture of each graph image the state shows,
    /// and writes its parameters (src/graph.rs `Show`, `INSPECT_PARAMS`).
    pub(super) fn prepare_inspections(&mut self, state: &ShotState, boxes: &[(String, String)]) {
        use crate::graph::FrameGraph;
        use crate::utility::inset::view_texture;
        let shown = shown_images(state, boxes);
        let gone: Vec<String> = self
            .inspections
            .iter()
            .filter(|(name, x)| !shown.iter().any(|i| i.name == **name && i.key == x.key))
            .map(|(n, _)| n.clone())
            .collect();
        for name in gone {
            let x = self.inspections.remove(&name).expect("an inspection");
            self.egui_renderer.free_texture(&x.id);
            self.textures.remove(&view_texture(&name));
        }
        if shown.is_empty() {
            return;
        }
        if self.inspect_pipelines.is_empty() {
            self.make_inspect_pipelines();
        }
        let source = investigation::current().scene_shading();
        let graph = FrameGraph::new(source, &[]);
        for image in shown {
            self.prepare_inspection(&graph, image);
        }
    }

    /// The inspector's compute programs (packages/render_lab/shaders/inspect.wgsl): one per kind of source.
    fn make_inspect_pipelines(&mut self) {
        let device = &self.gpu.device;
        let source =
            crate::shaders::module(crate::shaders::INSPECT).unwrap_or_else(|e| panic!("{e:#}"));
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("inspect"),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
        for (entry, binding, ty) in inspect_programs() {
            let pipeline = inspect_pipeline(device, &module, entry, binding, ty);
            self.inspect_pipelines.insert(entry, pipeline);
        }
    }

    /// Makes (or keeps) the texture of the graph image `image` names, at the size its source has in the
    /// inspector, and writes its parameters.
    fn prepare_inspection(&mut self, graph: &crate::graph::FrameGraph, image: Shown) {
        use crate::graph::inspect_entry;
        use crate::utility::inset::view_texture;
        let source = investigation::current().scene_shading();
        let viewport = self
            .viewport
            .as_ref()
            .map_or([self.width, self.height], |v| v.size);
        let Ok((node, r)) = graph.inspected(&image.key) else {
            return;
        };
        let resource = graph.resources[r].clone();
        let viewing = Viewing::of(graph, r, image.show.as_deref(), image.params);
        let update_words = |name: &str| {
            source
                .and_then(|s| s.update)
                .and_then(|u| u.buffers.iter().find(|b| b.name == name))
                .map_or(1, |b| (b.words)().max(1))
        };
        let at = Sizing {
            viewport,
            scale: source
                .and_then(|s| self.shadings.iter().find(|x| std::ptr::eq(x.source, s)))
                .map_or(1, |s| s.scale.max(1)),
            level: viewing.level,
            axis: viewing.axis,
        };
        // The source's size in texels (a buffer: the viewport's pixels, or its elements row by row).
        let src = at.source_size(&resource, &update_words);
        let name = image.name;
        let long = src[0].max(src[1]).max(1);
        let side = if card_image(&name) {
            DAG_SIDE
        } else {
            INSPECT_SIDE.min(long.max(256))
        };
        let size = [(src[0] * side / long).max(1), (src[1] * side / long).max(1)];
        if self.inspections.get(&name).is_some_and(|x| x.size != size) {
            let x = self.inspections.remove(&name).expect("an inspection");
            self.egui_renderer.free_texture(&x.id);
            self.textures.remove(&view_texture(&name));
        }
        if !self.inspections.contains_key(&name) {
            // A new inspection (or one whose size changed) gets its own targets and texture, drawn after its node.
            let (texture, storage, id, buffer) = self.inspection_targets(&name, size);
            let (entry, binding) = inspect_entry(&resource.shape);
            self.textures.insert(view_texture(&name), (id, size));
            self.inspections.insert(
                name.clone(),
                Inspection {
                    key: image.key.clone(),
                    node,
                    resource: resource.name,
                    entry,
                    binding,
                    _texture: texture,
                    storage,
                    size,
                    params: buffer,
                    id,
                    index: viewing.param("index", 0).unwrap_or(0.0) as u32,
                    level: viewing.level,
                },
            );
        }
        let words_out = viewing.words(&resource, viewport, src, size);
        let x = &self.inspections[&name];
        self.gpu
            .queue
            .write_buffer(&x.params, 0, bytemuck::cast_slice(&words_out));
    }

    /// An inspector image's texture `size` texels large, its storage view, its egui texture, and its parameters'
    /// buffer.
    fn inspection_targets(
        &mut self,
        name: &str,
        size: [u32; 2],
    ) -> (
        wgpu::Texture,
        wgpu::TextureView,
        egui::TextureId,
        wgpu::Buffer,
    ) {
        let device = &self.gpu.device;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(name),
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let storage = texture.create_view(&Default::default());
        let id = self.egui_renderer.register_native_texture(
            device,
            &texture.create_view(&Default::default()),
            wgpu::FilterMode::Linear,
        );
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(name),
            size: 80,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        (texture, storage, id, buffer)
    }

    /// Draws each graph image that names node `node` from its resource (in
    /// `frame`'s targets) as it
    /// stands now (right after the node): packages/render_lab/shaders/inspect.wgsl.
    pub(super) fn capture(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        node: usize,
        frame: &FrameTargets,
    ) {
        let FrameTargets {
            chain,
            shading,
            targets,
        } = *frame;
        for x in self.inspections.values().filter(|x| x.node == node) {
            let Some((pipeline, layout)) = self.inspect_pipelines.get(x.entry) else {
                continue;
            };
            let volume_view;
            let program_handle;
            let resource: wgpu::BindingResource = match x.resource {
                "hdr" => wgpu::BindingResource::TextureView(&targets.hdr),
                "depth" => wgpu::BindingResource::TextureView(&targets.depth),
                "overlay" => wgpu::BindingResource::TextureView(&targets.overlay),
                name => {
                    // The chain's buffer, depth views, or target; else the program's, an update's, or a volume.
                    let chained = chain.and_then(|c| {
                        if let Some((_, b)) = c.buffers.iter().find(|(n, _)| *n == name) {
                            return Some(b.as_entire_binding());
                        }
                        if let (Some(d), Some((all, _))) = (c.source.depth_views, &c.depth_views)
                            && d.name == name
                        {
                            return Some(wgpu::BindingResource::TextureView(all));
                        }
                        c.views
                            .iter()
                            .find(|(n, _)| *n == name)
                            .map(|(_, v)| wgpu::BindingResource::TextureView(v))
                    });
                    match chained {
                        Some(r) => r,
                        None => {
                            let Some(s) = shading else { continue };
                            if let Some(h) = s.program_handle(name) {
                                program_handle = h;
                                match &program_handle {
                                    crate::program::ProgramHandle::Buffer(b) => {
                                        let Some(b) = b.get(x.index as usize).or(b.first()) else {
                                            continue;
                                        };
                                        b.as_entire_binding()
                                    }
                                    image => {
                                        let Some(view) = image.view(x.index, x.level) else {
                                            continue;
                                        };
                                        volume_view = view;
                                        wgpu::BindingResource::TextureView(&volume_view)
                                    }
                                }
                            } else if let Some(k) = s
                                .source
                                .update
                                .and_then(|u| u.buffers.iter().position(|b| b.name == name))
                            {
                                s.update_buffers[k].as_entire_binding()
                            } else if let (Some(v), Some(set)) = (
                                s.source.volumes.iter().position(|v| v.name == name),
                                s.volume_sets.get(s.volume_set),
                            ) {
                                volume_view = set.textures[v].create_view(&Default::default());
                                wgpu::BindingResource::TextureView(&volume_view)
                            } else {
                                continue;
                            }
                        }
                    }
                }
            };
            let group = self
                .gpu
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("inspect"),
                    layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: x.params.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(&x.storage),
                        },
                        wgpu::BindGroupEntry {
                            binding: x.binding,
                            resource,
                        },
                    ],
                });
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("inspect"),
                timestamp_writes: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups(x.size[0].div_ceil(8), x.size[1].div_ceil(8), 1);
        }
    }

    /// The graph images shown: their names and `<node>.<resource>` keys.
    pub fn inspected(&self) -> Vec<(String, String)> {
        self.inspections
            .iter()
            .filter(|(n, _)| !card_image(n))
            .map(|(n, x)| (n.clone(), x.key.clone()))
            .collect()
    }

    /// A graph image's texture read back (RGBA8, its size).
    pub fn read_inspection(&self, name: &str) -> Option<(Vec<u8>, [u32; 2])> {
        let x = self.inspections.get(name)?;
        Some((
            crate::readback::read_texture_rgba(
                &self.gpu.device,
                &self.gpu.queue,
                &x._texture,
                x.size,
            ),
            x.size,
        ))
    }

    /// The current frame's graph (src/graph.rs), kept alive for what the
    /// image kinds read: compiled again when the shading or those change.
    pub(super) fn frame_graph(&mut self) -> std::sync::Arc<crate::graph::FrameGraph> {
        let source = investigation::current().scene_shading();
        let mut sinks: Vec<String> = self
            .views
            .values()
            .flat_map(|v| v.kind.reads.iter().cloned())
            .collect();
        sinks.extend(
            self.inspections
                .iter()
                .filter(|(n, _)| !card_image(n))
                .map(|(_, x)| x.resource.to_string()),
        );
        sinks.sort();
        sinks.dedup();
        let key = (
            source.map_or(0, |s| s as *const SceneShading as usize),
            sinks,
        );
        if let Some((k, graph)) = &self.graph
            && *k == key
        {
            return graph.clone();
        }
        let names: Vec<&str> = key.1.iter().map(String::as_str).collect();
        let graph = std::sync::Arc::new(crate::graph::FrameGraph::new(source, &names));
        if !graph.errors.is_empty() {
            panic!(
                "the frame's graph does not compile (render-lab graph):\n  {}",
                graph.errors.join("\n  ")
            );
        }
        self.graph = Some((key, graph.clone()));
        graph
    }
}

/// A graph image a frame shows: its name, its `<node>.<resource>`, the visualizer it names (none: the
/// investigation's), and its parameters.
struct Shown {
    name: String,
    key: String,
    show: Option<String>,
    params: crate::utility::inset::ImageParams,
}

impl Shown {
    /// An image the lab shows itself (a DAG card's, a resource card's, a lit box's): the investigation's view.
    fn of(name: String, key: String) -> Shown {
        Shown {
            name,
            key,
            show: None,
            params: Default::default(),
        }
    }
}

/// The graph images `state` shows: its images, the DAG's cards, the resources' cards, and the frame graph scene's
/// lit `boxes` (a pass's result, or a resource as the frame leaves it).
fn shown_images(state: &ShotState, boxes: &[(String, String)]) -> Vec<Shown> {
    use crate::graph::FrameGraph;
    use crate::utility::inset::ImageSource;
    let mut shown: Vec<Shown> = state
        .images
        .iter()
        .filter_map(|i| match &i.source {
            ImageSource::Graph { key, show, params } => Some(Shown {
                name: i.name.clone(),
                key: key.clone(),
                show: show.clone(),
                params: params.clone(),
            }),
            _ => None,
        })
        .collect();
    if state.panel == crate::shot::PanelMode::Dag {
        // The DAG's cards: each pass's result, as the pass leaves it.
        let graph = FrameGraph::new(investigation::current().scene_shading(), &[]);
        for (i, n) in graph.nodes.iter().enumerate() {
            if let Some(r) = graph.result_of(i) {
                shown.push(Shown::of(
                    format!("{DAG_IMAGE}{}", n.name),
                    format!("{}.{r}", n.name),
                ));
            }
        }
    }
    // The frame graph scene's lit boxes: a pass's result, or a resource as the frame leaves it.
    if !boxes.is_empty() {
        let graph = FrameGraph::new(investigation::current().scene_shading(), &[]);
        for (group, member) in boxes {
            // A member is `node.resource`, a pass (its result), or a resource (after its last user).
            let key = match graph.node(member) {
                _ if member.contains('.') => Some(member.clone()),
                Some(n) => graph.result_of(n).map(|r| format!("{member}.{r}")),
                None => graph
                    .last_user(member)
                    .map(|n| format!("{}.{member}", graph.nodes[n].name)),
            };
            if let Some(key) = key {
                shown.push(Shown::of(format!("{BOX_IMAGE}{group}"), key));
            }
        }
    }
    if state.panel == crate::shot::PanelMode::Resources {
        // The resource cards: each resource (but the frame) as its last user leaves it.
        let graph = FrameGraph::new(investigation::current().scene_shading(), &[]);
        for r in graph.resources.iter().filter(|r| r.name != "frame") {
            if let Some(node) = graph.last_user(r.name) {
                shown.push(Shown::of(
                    format!("{RESOURCE_IMAGE}{}", r.name),
                    format!("{}.{}", graph.nodes[node].name, r.name),
                ));
            }
        }
    }
    shown
}

/// What an inspected source's size depends on: the viewport's size and the shading's scale, and the level and
/// axis an image shows.
struct Sizing {
    viewport: [u32; 2],
    scale: u32,
    level: u32,
    axis: u32,
}

impl Sizing {
    /// The size in texels of `resource` as the inspector reads it (a buffer: the viewport's pixels, or its
    /// elements row by row; `update_words` counts an update buffer's).
    fn source_size(
        &self,
        resource: &crate::graph::Resource,
        update_words: &dyn Fn(&str) -> u64,
    ) -> [u32; 2] {
        use crate::graph::Shape;
        let Sizing {
            viewport,
            scale,
            level,
            axis,
        } = *self;
        match &resource.shape {
            Shape::Texture { divisor, .. } => {
                // Divisor 0 takes the shading's scale, which the state chooses.
                let d = if *divisor == 0 {
                    scale
                } else {
                    *divisor
                };
                [(viewport[0] / d).max(1), (viewport[1] / d).max(1)]
            }
            Shape::Buffer { .. } => viewport,
            Shape::UpdateBuffer => [
                256,
                (update_words(resource.name) as u32).div_ceil(256).max(1),
            ],
            Shape::DepthArray { size, .. } => [*size, *size],
            Shape::Volume { size, mips, .. } => {
                let m = level.min(mips.saturating_sub(1));
                let s = size.map(|v| (v >> m).max(1));
                match axis {
                    0 => [s[2], s[1]],
                    2 => [s[0], s[1]],
                    _ => [s[0], s[2]],
                }
            }
            Shape::Bytes { bytes } => [256, (bytes.div_ceil(4) as u32).div_ceil(256).max(1)],
            Shape::Image {
                kind: crate::investigation::ImageKind::D3,
                size: s,
                ..
            } => match axis {
                0 => [s[2], s[1]],
                2 => [s[0], s[1]],
                _ => [s[0], s[2]],
            },
            // A level of an image (the view's size divided, when it follows the view).
            Shape::Image {
                size, viewport: d, ..
            } => {
                let s = if *d == 0 {
                    [size[0], size[1]]
                } else {
                    [(viewport[0] / d).max(1), (viewport[1] / d).max(1)]
                };
                [(s[0] >> level).max(1), (s[1] >> level).max(1)]
            }
        }
    }
}

/// The inspector's programs: the entry point, the binding of its source, and the source's binding type.
fn inspect_programs() -> Vec<(&'static str, u32, wgpu::BindingType)> {
    use crate::graph::{Shape, inspect_entry};
    let programs = array_programs();
    let sources: [(&str, wgpu::BindingType); 6] = [
        (
            "inspectTextureCS",
            wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
        ),
        (
            "inspectTextureMsCS",
            wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: true,
            },
        ),
        (
            "inspectDepthMsCS",
            wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Depth,
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: true,
            },
        ),
        (
            "inspectDepthArrayCS",
            wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Depth,
                view_dimension: wgpu::TextureViewDimension::D2Array,
                multisampled: false,
            },
        ),
        (
            "inspectBufferCS",
            wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
        ),
        (
            "inspectVolumeCS",
            wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                view_dimension: wgpu::TextureViewDimension::D3,
                multisampled: false,
            },
        ),
    ];
    let sources = sources.into_iter().map(|(entry, ty)| {
        let (_, binding) = [
            Shape::Texture {
                format: wgpu::TextureFormat::Rgba8Unorm,
                divisor: 1,
                samples: 1,
            },
            Shape::Texture {
                format: wgpu::TextureFormat::Rgba8Unorm,
                divisor: 1,
                samples: 4,
            },
            Shape::Texture {
                format: wgpu::TextureFormat::Depth32Float,
                divisor: 1,
                samples: 4,
            },
            Shape::DepthArray { size: 1, layers: 1 },
            Shape::UpdateBuffer,
            Shape::Volume {
                format: wgpu::TextureFormat::Rgba8Unorm,
                size: [1; 3],
                mips: 1,
            },
        ]
        .iter()
        .map(inspect_entry)
        .find(|(e, _)| *e == entry)
        .expect("an entry per kind");
        (entry, binding, ty)
    });
    sources.chain(programs).collect()
}

/// The inspector's programs of integer volumes and of texture arrays, each with the binding of its source.
fn array_programs() -> [(&'static str, u32, wgpu::BindingType); 5] {
    let tex = |sample_type, view_dimension| wgpu::BindingType::Texture {
        sample_type,
        view_dimension,
        multisampled: false,
    };
    let (float, uint, sint) = (
        wgpu::TextureSampleType::Float { filterable: false },
        wgpu::TextureSampleType::Uint,
        wgpu::TextureSampleType::Sint,
    );
    let programs: [(&str, u32, wgpu::BindingType); 5] = [
        (
            "inspectVolumeUintCS",
            8,
            tex(uint, wgpu::TextureViewDimension::D3),
        ),
        (
            "inspectVolumeSintCS",
            9,
            tex(sint, wgpu::TextureViewDimension::D3),
        ),
        (
            "inspectArrayCS",
            10,
            tex(float, wgpu::TextureViewDimension::D2Array),
        ),
        (
            "inspectArrayUintCS",
            11,
            tex(uint, wgpu::TextureViewDimension::D2Array),
        ),
        (
            "inspectArraySintCS",
            12,
            tex(sint, wgpu::TextureViewDimension::D2Array),
        ),
    ];
    programs
}

/// An inspector program's pipeline and bind group layout: its parameters (0), its output (1), and its source
/// (`binding`, of type `ty`).
fn inspect_pipeline(
    device: &wgpu::Device,
    module: &wgpu::ShaderModule,
    entry: &'static str,
    binding: u32,
    ty: wgpu::BindingType,
) -> (wgpu::ComputePipeline, wgpu::BindGroupLayout) {
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(entry),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::StorageTexture {
                    access: wgpu::StorageTextureAccess::WriteOnly,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    view_dimension: wgpu::TextureViewDimension::D2,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty,
                count: None,
            },
        ],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(entry),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some(entry),
        layout: Some(&pipeline_layout),
        module,
        entry_point: Some(entry),
        compilation_options: Default::default(),
        cache: None,
    });
    (pipeline, layout)
}

/// An inspected image's visualizer and parameters: the investigation's view of its resource (its parameters under
/// the image's), unless the image names another visualizer; and the level and axis it shows.
struct Viewing {
    show: crate::graph::Show,
    params: crate::utility::inset::ImageParams,
    level: u32,
    axis: u32,
}

impl Viewing {
    /// How resource `r` of `graph` is shown, by the visualizer `show` names (none: the investigation's), with
    /// `params` over its own.
    fn of(
        graph: &crate::graph::FrameGraph,
        r: usize,
        show: Option<&str>,
        params: crate::utility::inset::ImageParams,
    ) -> Viewing {
        use crate::graph::Show;
        let (declared, mut defaults) = graph.view_of(r);
        let show = show.map_or(declared, |s| {
            Show::from_key(s).unwrap_or_else(|e| panic!("{e}"))
        });
        if show != declared {
            defaults.clear();
        }
        defaults.extend(params);
        let mut viewing = Viewing {
            show,
            params: defaults,
            level: 0,
            axis: 0,
        };
        viewing.level = viewing.param("level", 0).unwrap_or(0.0).max(0.0) as u32;
        viewing.axis = viewing
            .param("axis", 0)
            .unwrap_or(inspect_default("axis"))
            .round()
            .clamp(0.0, 2.0) as u32;
        viewing
    }

    /// Component `i` of the parameter `k`, if it is set.
    fn param(&self, k: &str, i: usize) -> Option<f32> {
        self.params.get(k).and_then(|v| v.get(i)).copied()
    }

    /// The inspector program's parameters (packages/render_lab/shaders/inspect.wgsl) for `resource`, read at
    /// `src` texels and drawn at `size`, in a viewport of `viewport` pixels.
    fn words(
        &self,
        resource: &crate::graph::Resource,
        viewport: [u32; 2],
        src: [u32; 2],
        size: [u32; 2],
    ) -> [u32; 20] {
        use crate::graph::Shape;
        let (show, level, axis) = (self.show, self.level, self.axis);
        let param = |k: &str, i: usize| self.param(k, i);
        let default = inspect_default;
        let range = [
            param("range", 0).unwrap_or(show.range()[0]),
            param("range", 1).unwrap_or(show.range()[1]),
        ];
        let (tile, words, tiles_x, samples) = match &resource.shape {
            Shape::Buffer {
                words_per_tile,
                tile,
            } => (
                *tile,
                *words_per_tile,
                viewport[0].div_ceil((*tile).max(1)),
                1,
            ),
            Shape::UpdateBuffer | Shape::Bytes { .. } => (0, 1, 0, 1),
            // An integer image: its value is the word (a hash of it, a count).
            Shape::Image { format, .. } if crate::graph::image_class(*format) != 0 => (1, 1, 0, 1),
            Shape::Texture { samples, .. } => (1, 0, 0, *samples),
            _ => (1, 0, 0, 1),
        };
        let u = |v: f32| v.max(0.0).round() as u32;
        [
            show.mode(),
            u(param("channel", 0).unwrap_or(0.0)),
            u(param("encoding", 0).unwrap_or(0.0)),
            u(param("average", 0).unwrap_or(0.0)),
            range[0].to_bits(),
            range[1].to_bits(),
            param("exposure", 0).unwrap_or(0.0).to_bits(),
            param("slice", 0).unwrap_or(default("slice")).to_bits(),
            size[0],
            size[1],
            src[0],
            src[1],
            tile,
            words,
            u(param("word", 0).unwrap_or(0.0)),
            tiles_x,
            level,
            axis,
            samples,
            u(param("bits", 0).unwrap_or(0.0)) | (u(param("bits", 1).unwrap_or(0.0)) << 8),
        ]
    }
}

/// An inspector parameter's default (`INSPECT_PARAMS`).
fn inspect_default(k: &str) -> f32 {
    crate::graph::INSPECT_PARAMS
        .iter()
        .find(|p| p.0 == k)
        .map_or(0.0, |p| p.2)
}
