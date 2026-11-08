//! The Frame graph tab: the frame's render graph (src/graph/) in the panel (`PanelMode::Graph`, `Dag`, or
//! `Resources`, which a story shows too), the DAG or the cards full screen (with the selected pass's WGSL),
//! and the inspector (a resource right after a node, in an inset); the dock describes the hovered or
//! selected pass.

use super::{Feature, FeatureState, Host, Page, ShowTab, Snapshot, StoryPanel};
use crate::controls::Dock;
use crate::i18n::{args, tr, tr_with};
use crate::investigation;
use crate::palette;
use crate::shot::{PanelMode, SceneType, ShotState};
use crate::ui::{DockTab, GraphPanel, GraphPick, GraphView, Ink, ViewerAction};
use anyhow::{Result, anyhow, bail};
use egui::{CornerRadius, Pos2, Rect, RichText, Ui, pos2};

/// The tab (another tab's button may select it).
pub const TAB: DockTab = DockTab("graph");

pub struct GraphFeature;

/// The name of the inspector's image (a story's `{inset "graph"}` too).
pub const INSPECTOR: &str = "graph";

/// The inspector: the resource `<node>.<resource>` in the top-right inset (the image `graph`), its node
/// called out; none hides it.
pub fn inspect(state: &mut ShotState, key: Option<String>) {
    use crate::utility::inset::{Corner, ImageSource, StoryImage};
    state.images.retain(|i| i.name != INSPECTOR);
    state.highlight.clear();
    let Some(key) = key else { return };
    if let Some((node, _)) = key.rsplit_once('.') {
        state.highlight = vec![node.to_string()];
    }
    state.images.push(StoryImage {
        name: INSPECTOR.to_string(),
        source: ImageSource::Graph {
            key,
            show: None,
            params: Default::default(),
        },
        title: None,
        corner: Corner::TopRight,
        alpha: 1.0,
        click: None,
        mark: [0.0; 4],
        mark_alpha: 0.0,
        mark_text: None,
    });
}

/// The panels that show the DAG or the cards, which go full screen.
fn full_screen_panel(panel: PanelMode) -> bool {
    matches!(panel, PanelMode::Dag | PanelMode::Resources)
}

/// What a snapshot page shows of the graph.
#[derive(Clone, Debug, Default)]
struct PageArgs {
    /// The panel: the graph, its DAG, or the resources' cards.
    panel: Option<PanelMode>,
    /// The DAG (or the cards) full screen, and with it the selected pass's WGSL.
    full_screen: bool,
    source: bool,
    /// The nodes called out.
    highlight: Vec<String>,
    /// What the inspector shows (`<node>.<resource>[/<show>][?<param>=<value>&…]`); none: the first resource
    /// the most expensive pass writes.
    inspect: Option<String>,
}

impl PageArgs {
    /// `page`'s arguments: `viewer-graph[:<demo>]`, `viewer-inspect:<node>.<resource>[/<show>][?…]`,
    /// `viewer-source:<node>`, `viewer-dag[-full][:<demo>][@<node>,…]`, `viewer-resources[-full][:<demo>][@<node>,…]`.
    fn parse(page: &str, args: Option<&str>) -> Result<PageArgs> {
        Ok(match page {
            "viewer-graph" => PageArgs::default(),
            "viewer-inspect" => PageArgs {
                inspect: args.map(str::to_string),
                ..Default::default()
            },
            "viewer-source" => {
                let node = args.unwrap_or_default();
                let graph = crate::graph::FrameGraph::current(&[]);
                let i = graph
                    .node(node)
                    .ok_or_else(|| anyhow!("no node `{node}`"))?;
                let key = format!(
                    "{node}.{}",
                    graph
                        .result_of(i)
                        .ok_or_else(|| anyhow!("`{node}` has no result to select"))?
                );
                PageArgs {
                    panel: Some(PanelMode::Dag),
                    full_screen: true,
                    source: true,
                    inspect: Some(key),
                    ..Default::default()
                }
            }
            _ => {
                let panel = if page.starts_with("viewer-resources") {
                    PanelMode::Resources
                } else {
                    PanelMode::Dag
                };
                let highlight = args
                    .and_then(|a| a.split_once('@'))
                    .map(|(_, nodes)| nodes.split(',').map(str::to_string).collect())
                    .unwrap_or_default();
                PageArgs {
                    panel: Some(panel),
                    full_screen: page.ends_with("-full"),
                    highlight,
                    ..Default::default()
                }
            }
        })
    }
}

impl Feature for GraphFeature {
    fn id(&self) -> &'static str {
        TAB.0
    }

    fn label(&self) -> Option<String> {
        Some(tr("controls-frame_graph").to_string())
    }

    fn order(&self) -> i32 {
        50
    }

    fn panels(&self) -> &'static [PanelMode] {
        &[PanelMode::Graph, PanelMode::Dag, PanelMode::Resources]
    }

    fn pages(&self) -> &'static [Page] {
        &[
            Page {
                name: "viewer-graph",
                help: "[:<demo>] (the frame graph in the panel, the inspector on what its most expensive pass writes)",
            },
            Page {
                name: "viewer-inspect",
                help: ":<node>.<resource>[/<show>][?<param>=<value>&…] (the frame graph, the inspector on that resource after that node)",
            },
            Page {
                name: "viewer-source",
                help: ":<node> (the full-screen DAG, that node selected, the WGSL it is compiled from beside it)",
            },
            Page {
                name: "viewer-dag",
                help: "[:<demo>][@<node>,…] (the frame graph as a DAG in the panel, each pass's result on its card, those nodes called out)",
            },
            Page {
                name: "viewer-dag-full",
                help: "[:<demo>][@<node>,…] (that DAG full screen)",
            },
            Page {
                name: "viewer-resources",
                help: "[:<demo>][@<node>,…] (every resource as a card, its state at the end of the frame)",
            },
            Page {
                name: "viewer-resources-full",
                help: "[:<demo>][@<node>,…] (those cards full screen)",
            },
        ]
    }

    fn snapshot_demo<'a>(&self, page: &str, args: Option<&'a str>) -> Option<&'a str> {
        match page {
            "viewer-inspect" | "viewer-source" => None,
            "viewer-graph" => args,
            _ => args
                .map(|a| a.split_once('@').map_or(a, |(demo, _)| demo))
                .filter(|d| !d.is_empty()),
        }
    }

    /// The tab shows the frame graph (its DAG, unless a view of it shows); leaving it gives the demonstration's
    /// panel back (and closes the full-screen DAG, `Graph::update`).
    fn show(&self, tab: &mut ShowTab) {
        let graph = tab.state.panel.shows_graph();
        if tab.shown {
            if !graph {
                tab.state.panel = PanelMode::Dag;
            }
        } else if graph {
            tab.state.panel = tab.defaults.panel;
        }
    }

    fn state(&self) -> Box<dyn FeatureState> {
        Box::new(Graph::default())
    }
}

#[derive(Default)]
struct Graph {
    /// The DAG (or the cards) full screen, and with the DAG, the selected pass's WGSL beside it.
    full_screen: bool,
    source: bool,
    /// What a click asked the inspector to show (none: hide it).
    inspect: Option<Option<String>>,
    /// The tab was shown at the last update.
    on_tab: bool,
    /// A snapshot page's, from its first step.
    page: PageArgs,
    report: String,
}

impl Graph {
    /// A pick in a graph panel: a click on what is inspected hides the inspector.
    fn pick(&mut self, pick: GraphPick, view: &GraphView, actions: &mut Vec<ViewerAction>) {
        match pick {
            GraphPick::Inspect(key) => {
                let same = view.inspected.as_deref() == Some(key.as_str());
                self.inspect = Some((!same).then_some(key));
            }
            GraphPick::Panel(mode) => actions.push(ViewerAction::SetPanel(mode)),
            GraphPick::Overlay(on) => self.full_screen = on,
            // A view's own: its panel asks it (`graph_view::ask_reset`) and never returns it.
            GraphPick::Reset => unreachable!("a panel resets its own view"),
            GraphPick::Source(on) => self.source = on,
        }
    }

    /// The Frame graph tab (src/graph/; the panel shows the graph): the pass
    /// under the pointer in the panel, else the selected one (the inspected
    /// result's): its engine pass, what it does, its GPU time, and what it
    /// reads and writes (a click inspects that resource right after it); then
    /// every pass in order, a click selecting it.
    fn frame_graph(&mut self, dock: &mut Dock, ui: &mut Ui) {
        let Some(view) = dock.input.graph else { return };
        let hovered: Option<String> = ui
            .ctx()
            .data(|d| d.get_temp(egui::Id::unique("frame-graph-hovered")))
            .flatten();
        let inspected = dock.state.images.iter().find_map(|i| match &i.source {
            crate::utility::inset::ImageSource::Graph { key, .. } if i.name == INSPECTOR => {
                key.rsplit_once('.').map(|(n, _)| n.to_string())
            }
            _ => None,
        });
        let focus = hovered
            .or(inspected)
            .and_then(|name| view.nodes.iter().find(|n| n.name == name));
        const COLUMN: f32 = 640.0;
        let mut asked = None;
        dock.group(
            ui,
            &focus.map_or(tr("graph-pass").to_string(), |n| n.name.clone()),
            None,
            |_, ui| {
                ui.set_width(COLUMN);
                let Some(node) = focus else {
                    ui.add(
                        egui::Label::new(
                            RichText::new(tr("graph-pick_hint")).color(palette::text_dim()),
                        )
                        .wrap(),
                    );
                    return;
                };
                if let Some((pass, at)) = &node.engine {
                    ui.label(
                        RichText::new(format!(
                            "≈ {}: {pass} · {at}",
                            investigation::current().wording().engine_short
                        ))
                        .color(palette::accent()),
                    );
                }
                if let Some(about) = node.about {
                    ui.add(egui::Label::new(RichText::new(about).color(palette::text())).wrap());
                }
                if let Some(ms) = node.ms {
                    ui.label(
                        RichText::new(tr_with("graph-gpu_time", args!(time = format!("{ms:.3}"))))
                            .color(palette::text_dim()),
                    );
                }
                // What it reads and writes: a click shows that resource right after it.
                for (label, names) in [
                    (tr("graph-reads"), &node.reads),
                    (tr("graph-writes"), &node.writes),
                ] {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(label).color(palette::text_dim()));
                        for name in names.iter().filter(|n| n.as_str() != "frame") {
                            if ui
                                .selectable_label(false, RichText::new(name).monospace())
                                .clicked()
                            {
                                asked = Some(format!("{}.{name}", node.name));
                            }
                        }
                    });
                }
            },
        );
        dock.group(
            ui,
            tr("graph-passes"),
            Some(tr("graph-passes_hint")),
            |_, ui| {
                ui.set_width(COLUMN * 0.6);
                for node in &view.nodes {
                    let selected = focus.is_some_and(|f| f.name == node.name);
                    let label = RichText::new(&node.name).monospace().color(if node.live {
                        palette::text()
                    } else {
                        palette::text_faint()
                    });
                    if ui.selectable_label(selected, label).clicked()
                        && let Some(result) = &node.result
                    {
                        asked = Some(format!("{}.{result}", node.name));
                    }
                }
            },
        );
        if asked.is_some() {
            self.inspect = Some(asked);
        }
    }
}

impl FeatureState for Graph {
    /// What a click asked the inspector; leaving the tab closes the full-screen DAG.
    fn update(&mut self, host: &mut dyn Host) {
        if let Some(key) = self.inspect.take().filter(|_| host.interactive()) {
            inspect(host.state_mut(), key);
        }
        let on_tab = host.tab() == TAB;
        if self.on_tab && !on_tab {
            self.full_screen = false;
        }
        self.on_tab = on_tab;
    }

    fn dock(&mut self, dock: &mut Dock, ui: &mut Ui) -> bool {
        self.frame_graph(dock, ui);
        true
    }

    /// The graph, its DAG, or its resources' cards; the wheel scrolls it, not the camera.
    fn story_panel(&mut self, panel: &mut StoryPanel) {
        let input = panel.input;
        let l = &input.layout;
        if !l.panel_shown {
            return;
        }
        let view = input.graph.cloned().unwrap_or_default();
        let graph = GraphPanel {
            ctx: panel.ctx,
            ink: panel.ink,
            rect: l.panel,
            view: &view,
            highlight: &input.state.highlight,
            full: false,
            interactive: panel.interactive,
            source: None,
        };
        let pick = match input.state.panel {
            PanelMode::Graph => crate::ui::draw_graph_panel(&graph),
            PanelMode::Dag => crate::ui::draw_dag_panel(&graph),
            _ => crate::ui::draw_resource_panel(&graph),
        };
        if panel.interactive {
            // Its input is read by hand in the background layer.
            panel.pointer |= panel
                .ctx
                .pointer_hover_pos()
                .is_some_and(|p| l.panel.contains(p));
            if let Some(pick) = pick {
                self.pick(pick, &view, &mut panel.actions);
            }
        }
    }

    /// The DAG (with the selected pass's WGSL beside it) or the cards full screen, over everything (Esc
    /// closes it).
    fn overlay(&mut self, panel: &mut StoryPanel) {
        let input = panel.input;
        if !(self.covers(input.state) && input.scene_type == SceneType::ThreeD) {
            return;
        }
        let (ctx, ink) = (panel.ctx, panel.ink);
        let view = input.graph.cloned().unwrap_or_default();
        let canvas = Rect::from_min_size(Pos2::ZERO, input.layout.canvas);
        let mut picked = None;
        egui::Area::new(egui::Id::unique("frame-graph-overlay"))
            .order(egui::Order::Foreground)
            .fixed_pos(Pos2::ZERO)
            .constrain(false)
            .show(ctx, |ui| {
                // Covers the viewer, so nothing under it takes the pointer.
                ui.allocate_rect(canvas, egui::Sense::click_and_drag());
                let overlay_ink = Ink::new(ui.painter());
                ui.painter().rect_filled(
                    canvas,
                    CornerRadius::ZERO,
                    palette::background().gamma_multiply(0.92),
                );
                let area = canvas.shrink(24.0);
                let source = self.source && input.state.panel == PanelMode::Dag;
                picked = if input.state.panel == PanelMode::Dag {
                    // With the source: the graph on the left, the selected pass's WGSL on the right.
                    let graph_area = if source {
                        Rect::from_min_max(
                            area.min,
                            pos2(area.min.x + area.width() * 0.5 - 8.0, area.max.y),
                        )
                    } else {
                        area
                    };
                    let pick = crate::ui::draw_dag_panel(&GraphPanel {
                        ctx,
                        ink: &overlay_ink,
                        rect: graph_area,
                        view: &view,
                        highlight: &input.state.highlight,
                        full: true,
                        interactive: true,
                        source: Some(source),
                    });
                    if source {
                        let pane =
                            Rect::from_min_max(pos2(graph_area.max.x + 16.0, area.min.y), area.max);
                        crate::ui::draw_source_pane(
                            ctx,
                            &overlay_ink,
                            pane,
                            &view,
                            panel.shader_error,
                        );
                    }
                    pick
                } else {
                    crate::ui::draw_resource_panel(&GraphPanel {
                        ctx,
                        ink: &overlay_ink,
                        rect: area,
                        view: &view,
                        highlight: &input.state.highlight,
                        full: true,
                        interactive: true,
                        source: None,
                    })
                };
                ink.min_size
                    .set(ink.min_size.get().min(overlay_ink.min_size.get()));
            });
        if let Some(pick) = picked {
            self.pick(pick, &view, &mut panel.actions);
        }
        panel.pointer = true;
    }

    fn covers(&self, state: &ShotState) -> bool {
        self.full_screen && full_screen_panel(state.panel)
    }

    fn uncover(&mut self) {
        self.full_screen = false;
    }

    /// The demonstration at its defaults with the graph in the panel, settled over thirty frames; then the
    /// inspector on the page's resource (or what the most expensive pass writes), settled over four.
    fn snapshot(
        &mut self,
        page: &str,
        args: Option<&str>,
        step: usize,
        host: &mut dyn Host,
    ) -> Result<Snapshot> {
        let graph = crate::graph::FrameGraph::current(&[]);
        match step {
            0 => {
                self.page = PageArgs::parse(page, args)?;
                let defaults = host.default_state(host.state().demo);
                *host.state_mut() = defaults;
                host.state_mut().panel = self.page.panel.unwrap_or(PanelMode::Graph);
                self.full_screen = self.page.full_screen;
                self.source = self.page.source;
                if let Some(unknown) = self.page.highlight.iter().find(|h| graph.node(h).is_none())
                {
                    bail!(
                        "no node `{unknown}` (nodes: {})",
                        graph
                            .nodes
                            .iter()
                            .map(|n| n.name)
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
                Ok(Snapshot::Settle)
            }
            1..30 => Ok(Snapshot::Settle),
            30 => {
                // `?name=v,v&…`: the inspector's parameters (`graph::INSPECT_PARAMS`).
                let inspect = self.page.inspect.as_deref();
                let (inspect, query) = match inspect.and_then(|k| k.split_once('?')) {
                    Some((k, q)) => (Some(k), q),
                    None => (inspect, ""),
                };
                let params = crate::graph::parse_params(query).map_err(|e| anyhow!(e))?;
                let (key, show) = match inspect {
                    Some(k) => match k.split_once('/') {
                        Some((key, show)) => (key.to_string(), Some(show.to_string())),
                        None => (k.to_string(), None),
                    },
                    None => {
                        // The most expensive live pass that writes something of its own.
                        let passes = host
                            .renderer()
                            .and_then(|r| r.timing.as_ref().map(|t| t.summary()))
                            .unwrap_or_default();
                        let cost = |n: &crate::graph::Node| {
                            passes.iter().find(|p| p.0 == n.timing).map_or(0.0, |p| p.1)
                        };
                        let node = graph
                            .order
                            .iter()
                            .map(|&i| &graph.nodes[i])
                            .filter(|n| !n.writes.is_empty() && n.writes[0] != "frame")
                            .max_by(|a, b| cost(a).total_cmp(&cost(b)))
                            .ok_or_else(|| anyhow!("no node writes anything"))?;
                        (format!("{}.{}", node.name, node.writes[0]), None)
                    }
                };
                graph.inspected(&key).map_err(|e| anyhow!(e))?;
                if let Some(show) = &show {
                    crate::graph::Show::from_key(show).map_err(|e| anyhow!(e))?;
                }
                let state = host.state_mut();
                self::inspect(state, Some(key.clone()));
                // The page's call-outs over the inspector's.
                if !self.page.highlight.is_empty() {
                    state.highlight = self.page.highlight.clone();
                }
                if let Some(crate::utility::inset::ImageSource::Graph {
                    show: s, params: p, ..
                }) = state
                    .images
                    .iter_mut()
                    .find(|i| i.name == INSPECTOR)
                    .map(|i| &mut i.source)
                {
                    *s = show;
                    *p = params;
                }
                self.report = format!(
                    "{} nodes, {} resources; inspecting {key}",
                    graph.nodes.len(),
                    graph.resources.len()
                );
                Ok(Snapshot::Settle)
            }
            31..34 => Ok(Snapshot::Settle),
            _ => Ok(Snapshot::Done(self.report.clone())),
        }
    }
}
