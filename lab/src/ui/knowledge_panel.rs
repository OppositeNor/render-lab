//! The knowledge graph: the Prerequisites tab's panel, its full-screen
//! overlay, and the topics a story's audience knows, learns, and needs.

use super::*;

/// The knowledge panel's view, kept in egui's memory between frames: the
/// graph's view (src/graph_view.rs: its scale, 0 until fitted, its centre,
/// the topics moved by hand), the selected topic, and the chapters it was
/// last fitted to.
#[derive(Clone, Debug, Default)]
pub(super) struct KnowledgeView {
    /// The view showed only the context's topics when last drawn.
    pub(super) compact: bool,
    pub(super) view: crate::graph_view::ViewState,
    pub(super) selected: Option<usize>,
    pub(super) fitted: Option<Vec<u32>>,
    /// The chrome's focus last applied (`KnowledgeContext::focus`).
    pub(super) focused: Option<String>,
    /// A topic to centre on the next frame (`focus_topic`).
    pub(super) centre_on: Option<usize>,
}

/// Where the knowledge graph is drawn: the side panel, or full screen over everything.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum KnowledgePlace {
    Panel,
    Overlay,
}

impl KnowledgePlace {
    pub(super) fn view_id(self) -> egui::Id {
        egui::Id::unique(if self == KnowledgePlace::Panel {
            "knowledge-panel-view"
        } else {
            "knowledge-overlay-view"
        })
    }

    /// The order of its input areas: the overlay's above everything else.
    pub(super) fn order(self) -> egui::Order {
        if self == KnowledgePlace::Panel {
            egui::Order::Background
        } else {
            egui::Order::Tooltip
        }
    }
}

/// Select topic `node` in the knowledge panel and the full-screen graph, and
/// centre it there (a click in the Prerequisites tab).
pub fn focus_topic(ctx: &egui::Context, node: usize) {
    ctx.data_mut(|d| {
        for place in [KnowledgePlace::Panel, KnowledgePlace::Overlay] {
            let view = d.get_temp_mut_or_default::<KnowledgeView>(place.view_id());
            view.selected = Some(node);
            view.centre_on = Some(node);
        }
    });
}

/// The knowledge graph full screen, over the viewer or the homepage, for
/// `chrome`'s chapters (none: the whole story). Returns true when it is
/// closed (its button; Esc is the caller's).
pub fn draw_knowledge_overlay(
    ctx: &egui::Context,
    canvas: Vec2,
    chrome: &KnowledgeContext,
) -> bool {
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::unique("knowledge-overlay"),
    ));
    let ink = Ink::new(&painter);
    let screen = Rect::from_min_size(Pos2::ZERO, canvas);
    painter.rect_filled(screen, CornerRadius::ZERO, palette::theme().dim);
    // What is under the overlay takes no pointer input.
    egui::Area::new(egui::Id::unique("knowledge-overlay-block"))
        .fixed_pos(Pos2::ZERO)
        .order(egui::Order::Foreground)
        .fade_in(false)
        .show(ctx, |ui| {
            ui.allocate_rect(screen, egui::Sense::click_and_drag());
        });
    draw_knowledge_graph(
        ctx,
        &ink,
        screen.shrink(40.0),
        chrome,
        KnowledgePlace::Overlay,
    )
}

/// A topic's role for the audience, as the panel colours it.
pub(super) fn topic_color(p: &crate::knowledge::Partition, i: usize) -> Color32 {
    if p.is_known(i) {
        palette::code_string()
    } else if p.level.contains(&i) {
        palette::accent()
    } else if p.teach.contains(&i) {
        palette::code_number()
    } else {
        palette::text_faint()
    }
}

/// The chapters the viewer's context names that the graph declares (none: every chapter).
pub fn knowledge_chapters(graph: &crate::knowledge::Graph, chrome: &KnowledgeContext) -> Vec<u32> {
    chrome
        .chapters
        .iter()
        .copied()
        .filter(|c| graph.chapters.iter().any(|g| g.number == *c))
        .collect()
}

/// The knowledge graph in the panel (`PanelMode::Knowledge`). Returns true when
/// its full-screen button is pressed.
pub(crate) fn draw_knowledge_panel(
    ctx: &egui::Context,
    ink: &Ink,
    rect: Rect,
    chrome: &KnowledgeContext,
) -> bool {
    draw_knowledge_graph(ctx, ink, rect, chrome, KnowledgePlace::Panel)
}

/// The knowledge graph shows only what its context needs (the panel, the
/// full-screen view, and the homepage's share it; `set_knowledge_compact`).
pub(super) static KNOWLEDGE_COMPACT: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Show only the topics the graph's context needs (a snapshot sets it).
pub fn set_knowledge_compact(on: bool) {
    KNOWLEDGE_COMPACT.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// The knowledge graph in `rect`: every topic, coloured by what the
/// investigation's audience knows (src/knowledge.rs), the context's chapters at
/// full strength; dragged to pan, scrolled to zoom, a click selects a topic
/// (its card below), a double click fits the context again. Its button at
/// the top right (full screen in the panel, close in the overlay) returns
/// true when pressed. Node titles follow the zoom, so they are drawn with
/// the painter, not `Ink` (whose smallest-size record is the recorder's check).
pub(super) fn draw_knowledge_graph(
    ctx: &egui::Context,
    ink: &Ink,
    rect: Rect,
    chrome: &KnowledgeContext,
    place: KnowledgePlace,
) -> bool {
    crate::paint::panel_card(ink.painter, rect, 12);
    let x = rect.min.x + 22.0;
    let pressed = knowledge_bar(ctx, ink, rect, place);
    let y = knowledge_legend(ink, x, rect.min.y + 16.0 + 42.0);
    let measure = |t: &str| ink.measure(t, 100.0, FontFamily::Proportional).x / 100.0;
    let Some(graph) = crate::knowledge::current() else {
        ink.text(
            pos2(x, y),
            Align2::LEFT_TOP,
            tr("knowledge-none"),
            24.0,
            FontFamily::Proportional,
            palette::text_dim(),
        );
        return pressed;
    };
    let chapters = knowledge_chapters(graph, chrome);
    let context = graph.context(&chapters);
    // Compact: only the context's topics, laid out again on their own.
    let compact = KNOWLEDGE_COMPACT.load(std::sync::atomic::Ordering::Relaxed);
    let Some((_, kv)) =
        crate::knowledge::current_view(&measure, compact.then_some(context.as_slice()))
    else {
        return pressed;
    };
    // The card under the graph describes the selected topic; with none, the graph has its room.
    let selected = ctx
        .data(|d| d.get_temp::<KnowledgeView>(place.view_id()))
        .and_then(|v| v.selected);
    let card_height = if selected.is_some() {
        150.0
    } else {
        0.0
    };
    let area = Rect::from_min_max(
        pos2(rect.min.x + 8.0, y),
        pos2(rect.max.x - 8.0, rect.max.y - card_height - 8.0),
    );
    let hint = ink.fit_right(
        tr("knowledge-hint"),
        area.width() - 28.0,
        24.0,
        FontFamily::Proportional,
    );
    ink.text(
        pos2(x, area.max.y - 4.0),
        Align2::LEFT_BOTTOM,
        hint,
        24.0,
        FontFamily::Proportional,
        palette::text_faint(),
    );
    let topics = Topics {
        graph,
        kv,
        audience: graph.audience(),
        context,
        area: Rect::from_min_max(area.min, pos2(area.max.x, area.max.y - 34.0)),
    };
    let id = place.view_id();
    let mut view = ctx
        .data(|d| d.get_temp::<KnowledgeView>(id))
        .unwrap_or_default();
    topics.fit_view(&mut view, compact, chrome, &chapters);
    let hovered = topics.input(ctx, place, &mut view);
    topics.keep_on_graph(&mut view.view);
    ctx.data_mut(|d| d.insert_temp(id, view.clone()));
    if view.selected.is_some() != selected.is_some() {
        // The card comes or goes: lay the graph out again at once.
        ctx.request_repaint();
    }
    topics.draw(ink, &view, hovered);
    // The selected topic's card under the graph.
    let card = Rect::from_min_max(
        pos2(rect.min.x + 8.0, rect.max.y - card_height),
        pos2(rect.max.x - 8.0, rect.max.y - 8.0),
    );
    // A topic selected this frame gets its card from the next one, when the graph has made room.
    if let Some(s) = view.selected.filter(|_| card_height > 0.0) {
        topic_card(ink, card, graph, &topics.audience, s);
    }
    pressed
}

/// A button of the knowledge graph's bar, its right edge at `right` (its top at `top`), in its place's area
/// `id`, lit when `selected`: whether it was clicked, and its width.
fn bar_button(
    ctx: &egui::Context,
    ink: &Ink,
    id: &str,
    (right, top): (f32, f32),
    label: &str,
    order: egui::Order,
    selected: bool,
) -> (bool, f32) {
    let size = vec2(
        ink.measure(label, 24.0, FontFamily::Proportional).x + 32.0,
        40.0,
    );
    let mut clicked = false;
    egui::Area::new(egui::Id::unique(id))
        .fixed_pos(pos2(right - size.x, top))
        .order(order)
        .fade_in(false)
        .show(ctx, |ui| {
            clicked = ui
                .add(
                    egui::Button::new(egui::RichText::new(label).size(24.0))
                        .min_size(size)
                        .selected(selected),
                )
                .clicked();
        });
    (clicked, size.x)
}

/// The knowledge graph's title and its bar: its button at the top right (full screen in the panel, close in the
/// overlay), the toggle that hides the topics the context does not need (or shows them all again), and Reset,
/// which puts the moved topics back. Whether its button was pressed.
fn knowledge_bar(ctx: &egui::Context, ink: &Ink, rect: Rect, place: KnowledgePlace) -> bool {
    ink.text(
        pos2(rect.min.x + 22.0, rect.min.y + 16.0),
        Align2::LEFT_TOP,
        tr("knowledge-title"),
        30.0,
        bold(),
        palette::text(),
    );
    let panel = place == KnowledgePlace::Panel;
    let (order, ids) = if panel {
        (
            egui::Order::Middle,
            [
                "knowledge-panel-button",
                "knowledge-panel-compact",
                "knowledge-panel-reset",
            ],
        )
    } else {
        (
            egui::Order::Tooltip,
            [
                "knowledge-overlay-button",
                "knowledge-overlay-compact",
                "knowledge-overlay-reset",
            ],
        )
    };
    let label = if panel {
        tr("knowledge-full_screen")
    } else {
        tr("knowledge-close")
    };
    let top = rect.min.y + 12.0;
    let (pressed, button) = bar_button(
        ctx,
        ink,
        ids[0],
        (rect.max.x - 16.0, top),
        label,
        order,
        false,
    );
    let compact = KNOWLEDGE_COMPACT.load(std::sync::atomic::Ordering::Relaxed);
    let toggle = if compact {
        tr("knowledge-show_all")
    } else {
        tr("knowledge-hide_unused")
    };
    let right = rect.max.x - 28.0 - button;
    let (toggled, toggle_width) =
        bar_button(ctx, ink, ids[1], (right, top), toggle, order, compact);
    if toggled {
        set_knowledge_compact(!compact);
    }
    let right = rect.max.x - 40.0 - button - toggle_width;
    let (reset, _) = bar_button(
        ctx,
        ink,
        ids[2],
        (right, top),
        tr("graph-reset"),
        order,
        false,
    );
    if reset {
        crate::graph_view::ask_reset(ctx, place.view_id());
    }
    pressed
}

/// The legend at (`x`, `y`): one swatch per role. Where what follows it starts.
fn knowledge_legend(ink: &Ink, x: f32, y: f32) -> f32 {
    let mut lx = x;
    for (label, color) in [
        (tr("knowledge-known"), palette::code_string()),
        (tr("knowledge-level"), palette::accent()),
        (tr("knowledge-teach"), palette::code_number()),
        (tr("knowledge-outside"), palette::text_faint()),
    ] {
        ink.painter.rect_filled(
            Rect::from_min_size(pos2(lx, y + 6.0), vec2(16.0, 16.0)),
            CornerRadius::same(3),
            color,
        );
        let r = ink.text(
            pos2(lx + 24.0, y),
            Align2::LEFT_TOP,
            label,
            24.0,
            FontFamily::Proportional,
            palette::text_dim(),
        );
        lx = r.max.x + 22.0;
    }
    y + 34.0
}

/// The knowledge graph as drawn: the graph, its laid-out view, the audience's partition of it, the topics its
/// context needs, and the area it is drawn in.
struct Topics {
    graph: &'static crate::knowledge::Graph,
    kv: &'static crate::knowledge::View,
    audience: crate::knowledge::Partition,
    context: Vec<bool>,
    area: Rect,
}

impl Topics {
    /// Topic `i`'s place in the laid-out view, if it is shown.
    fn local(&self, i: usize) -> Option<usize> {
        self.kv.local[i]
    }

    /// The bounding box of the context's topics (where they were laid out), in graph units; the whole graph's when
    /// none is shown.
    fn context_bounds(&self) -> Rect {
        (0..self.graph.nodes.len())
            .filter(|&i| self.context[i])
            .filter_map(|i| self.local(i))
            .map(|k| self.kv.laid.rects[k])
            .reduce(|a, b| a.union(b))
            .unwrap_or(self.kv.laid.bounds)
    }

    /// `v` fitted to `b`, but no smaller than titles stay readable at (the rest is a drag away); where it does not
    /// fit, from its top-left (the bases and the first topics).
    fn fit(&self, b: Rect, v: &mut crate::graph_view::ViewState) {
        let area = self.area;
        let b = b.expand(2.0);
        v.scale = (area.width() / b.width())
            .min(area.height() / b.height())
            .clamp(17.0, 30.0);
        let half = area.size() / v.scale / 2.0;
        let x = if b.width() > 2.0 * half.x {
            b.min.x + half.x
        } else {
            b.center().x
        };
        let y = if b.height() > 2.0 * half.y {
            b.min.y + half.y
        } else {
            b.center().y
        };
        v.centre = vec2(x, y);
    }

    /// The view set up for this frame: fitted again after a change of view (all topics or the context's) or of
    /// chapters, on the chrome's focus, and centred on a topic focused from the dock.
    fn fit_view(
        &self,
        view: &mut KnowledgeView,
        compact: bool,
        chrome: &KnowledgeContext,
        chapters: &[u32],
    ) {
        let laid = &self.kv.laid;
        if view.compact != compact {
            view.compact = compact;
            view.fitted = None;
            view.view.moved.clear();
            if view.selected.is_some_and(|s| self.local(s).is_none()) {
                view.selected = None;
            }
        }
        if let Some(key) = &chrome.focus
            && view.focused.as_ref() != Some(key)
        {
            if let Some(node) = self.graph.find(key) {
                view.selected = Some(node);
                if let Some(k) = self.local(node) {
                    view.view.centre = laid.rects[k].center().to_vec2();
                    view.view.scale = 18.0;
                }
            }
            view.focused = Some(key.clone());
            view.fitted = Some(chapters.to_vec());
        }
        if view.view.scale == 0.0 || view.fitted.as_deref() != Some(chapters) {
            self.fit(self.context_bounds(), &mut view.view);
            view.fitted = Some(chapters.to_vec());
        }
        // A topic focused from the dock: centred, at a readable scale.
        if let Some(k) = view
            .centre_on
            .take()
            .filter(|&n| n < self.graph.nodes.len())
            .and_then(|n| self.local(n))
        {
            view.view.centre = crate::graph_view::placed(laid, &view.view.moved, k)
                .center()
                .to_vec2();
            view.view.scale = view.view.scale.max(18.0);
        }
    }

    /// Input: an interactive area over the graph (egui then has the pointer, not the camera): drags, the wheel,
    /// clicks, and a hovered topic's summary. The topic hovered.
    fn input(
        &self,
        ctx: &egui::Context,
        place: KnowledgePlace,
        view: &mut KnowledgeView,
    ) -> Option<usize> {
        let (area, laid, kv, graph) = (self.area, &self.kv.laid, self.kv, self.graph);
        let whole = laid.bounds;
        let min_scale = ((area.width() / (whole.width() + 4.0))
            .min(area.height() / (whole.height() + 4.0))
            * 0.8)
            .min(1.5);
        let mut hovered = None;
        let input_id = egui::Id::unique(if place == KnowledgePlace::Panel {
            "knowledge-panel-input"
        } else {
            "knowledge-overlay-input"
        });
        egui::Area::new(input_id)
            .fixed_pos(area.min)
            .order(place.order())
            .fade_in(false)
            .show(ctx, |ui| {
                let response = ui.allocate_rect(
                    Rect::from_min_size(area.min, area.size()),
                    egui::Sense::click_and_drag(),
                );
                let picks = crate::graph_view::interact(
                    ctx,
                    area,
                    laid,
                    &mut view.view,
                    &crate::graph_view::Controls {
                        zoom: Some((min_scale, 40.0)),
                        move_nodes: true,
                    },
                    &|_| true,
                );
                hovered = picks.hovered.map(|k| kv.shown[k]);
                if picks.reset || crate::graph_view::take_reset(ctx, place.view_id()) {
                    view.view.moved.clear();
                    self.fit(self.context_bounds(), &mut view.view);
                } else if picks.clicked.is_some() || picks.clicked_empty {
                    view.selected = picks.clicked.map(|k| kv.shown[k]);
                }
                if let Some(i) = hovered {
                    response.on_hover_ui_at_pointer(|ui| {
                        ui.set_max_width(420.0);
                        ui.label(
                            egui::RichText::new(&graph.nodes[i].title)
                                .strong()
                                .color(topic_color(&self.audience, i)),
                        );
                        ui.label(&graph.nodes[i].summary);
                    });
                }
            });
        hovered
    }

    /// The view stays on the graph: along an axis where the graph is larger
    /// than the view, no empty space past its edge; where it is smaller, centred.
    fn keep_on_graph(&self, view: &mut crate::graph_view::ViewState) {
        let whole = self.kv.laid.bounds;
        let half = self.area.size() / view.scale / 2.0;
        let keep = |c: f32, lo: f32, hi: f32, h: f32| {
            if hi - lo > 2.0 * h {
                c.clamp(lo + h, hi - h)
            } else {
                (lo + hi) / 2.0
            }
        };
        let margin = 2.0;
        view.centre = vec2(
            keep(
                view.centre.x,
                whole.min.x - margin,
                whole.max.x + margin,
                half.x,
            ),
            keep(
                view.centre.y,
                whole.min.y - margin,
                whole.max.y + margin,
                half.y,
            ),
        );
    }

    /// The graph drawn: the context's topics at full strength, the selected topic's requirements and what
    /// requires it lit, the selected and `hovered` topics outlined.
    fn draw(&self, ink: &Ink, view: &KnowledgeView, hovered: Option<usize>) {
        let (graph, kv, p, context) = (self.graph, self.kv, &self.audience, &self.context);
        let requires = graph.requires();
        let lit: Vec<bool> = match view.selected {
            Some(s) => {
                let below = crate::knowledge::context(&requires, &[s]);
                (0..graph.nodes.len())
                    .map(|i| below[i] || crate::knowledge::context(&requires, &[i])[s])
                    .collect()
            }
            None => vec![false; graph.nodes.len()],
        };
        let fade = |c: Color32, strong: bool| {
            if strong {
                c
            } else {
                c.gamma_multiply(0.3)
            }
        };
        let nodes = kv
            .shown
            .iter()
            .map(|&i| {
                let strong = context[i] && (view.selected.is_none() || lit[i]);
                let color = fade(topic_color(p, i), strong);
                let outline = if view.selected == Some(i) || hovered == Some(i) {
                    Stroke::new(3.0, palette::SELECTED)
                } else {
                    Stroke::new(
                        if graph.nodes[i].base {
                            3.0
                        } else {
                            1.5
                        },
                        color,
                    )
                };
                crate::graph_view::NodeLook {
                    fill: palette::panel_raised(),
                    outline,
                    text: fade(palette::text(), strong),
                    rows: Vec::new(),
                    picture: None,
                    tint: Color32::WHITE,
                    badge: None,
                    note: None,
                    hidden: false,
                    alpha: 1.0,
                }
            })
            .collect();
        let edges = kv
            .graph
            .edges
            .iter()
            .map(|e| {
                let (r, i) = (kv.shown[e.from], kv.shown[e.to]);
                let on_path = view.selected.is_some() && lit[r] && lit[i];
                crate::graph_view::EdgeLook {
                    color: if on_path {
                        palette::SELECTED
                    } else {
                        fade(palette::border(), context[r] && context[i])
                    },
                    width: if on_path {
                        2.5
                    } else {
                        1.2
                    },
                    hidden: false,
                    drawn: 1.0,
                }
            })
            .collect();
        let clusters = kv
            .graph
            .clusters
            .iter()
            .map(|c| c.members.iter().any(|&k| context[kv.shown[k]]))
            .collect();
        let t = view.view.transform(self.area);
        crate::graph_view::draw(
            ink,
            self.area,
            &t,
            &kv.graph,
            &kv.laid,
            &crate::graph_view::Look {
                nodes,
                edges,
                clusters,
                moved: &view.view.moved,
                edge_labels: false,
                tracked: false,
            },
        );
    }
}

/// The selected topic `s`'s card in `card`: its title and role, its summary, and the chapters that use it.
fn topic_card(
    ink: &Ink,
    card: Rect,
    graph: &crate::knowledge::Graph,
    p: &crate::knowledge::Partition,
    s: usize,
) {
    let painter = ink.painter;
    let cx = card.min.x + 14.0;
    let width = card.width() - 28.0;
    painter.line_segment(
        [
            pos2(card.min.x + 14.0, card.min.y),
            pos2(card.max.x - 14.0, card.min.y),
        ],
        Stroke::new(1.0, palette::border()),
    );
    let node = &graph.nodes[s];
    let role = if p.is_known(s) {
        tr("knowledge-known")
    } else if p.level.contains(&s) {
        tr("knowledge-level")
    } else if p.teach.contains(&s) {
        tr("knowledge-teach")
    } else {
        tr("knowledge-outside")
    };
    let title = ink.fit_right(&format!("{} · {role}", node.title), width, 24.0, bold());
    ink.text(
        pos2(cx, card.min.y + 8.0),
        Align2::LEFT_TOP,
        title,
        24.0,
        bold(),
        topic_color(p, s),
    );
    let bottom = ink.wrapped(
        pos2(cx, card.min.y + 40.0),
        width,
        &node.summary,
        24.0,
        FontFamily::Proportional,
        palette::text(),
    );
    let used = graph.chapters_using(s);
    let used = if used.is_empty() {
        tr("knowledge-no_chapter").to_string()
    } else {
        tr_with(
            "knowledge-used_by",
            args!(
                chapters = used
                    .iter()
                    .map(|c| c.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        )
    };
    let used = ink.fit_right(&used, width, 24.0, FontFamily::Proportional);
    ink.text(
        pos2(cx, (bottom + 4.0).min(card.max.y - 30.0)),
        Align2::LEFT_TOP,
        used,
        24.0,
        FontFamily::Proportional,
        palette::text_dim(),
    );
}
