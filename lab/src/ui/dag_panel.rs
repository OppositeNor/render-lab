//! The frame graph as a DAG, each pass's result on its card, and the WGSL a pass is compiled from beside it.

use super::*;

/// The DAG panel (`PanelMode::Dag`), or the DAG full screen
/// (src/graph_view.rs): the frame's nodes as cards, laid out by the
/// longest path to them, top to bottom in the panel and left to right full
/// screen, boxed by the package's sections (`graph/frame`, the story's frame
/// graph scene's; src/graph.rs `GraphScene::current`); each card the
/// node's name, its GPU time, and its result as it stood right after it
/// (the inspector's image). An arrow runs from a node to each that reads
/// what it wrote, coloured by the resource's lifetime (a faint one: a node
/// that must run later because it overwrites what the first used). In the
/// viewer the view pans with a drag (or the wheel), a card moves with one,
/// and a double click puts both back; a story's `highlight` centres the
/// nodes it names. Hovering a card names its arrows' resources; a click on
/// one inspects its result.
pub(crate) fn draw_dag_panel(panel: &GraphPanel) -> Option<GraphPick> {
    let GraphPanel {
        ctx,
        ink,
        rect,
        view,
        full,
        ..
    } = *panel;
    use crate::graph_view::{self, Flow};
    let painter = ink.painter;
    crate::paint::panel_card(painter, rect, 12);
    let x = rect.min.x + 22.0;
    let right = rect.max.x - 22.0;
    let (y, switched) = graph_header(panel, GraphPlace::Dag);
    let id = egui::Id::unique(if full {
        "frame-graph-dag-overlay"
    } else {
        "frame-graph-dag-panel"
    });
    if switched == Some(GraphPick::Reset) {
        crate::graph_view::ask_reset(ctx, id);
    } else if view.nodes.is_empty() || switched.is_some() {
        return switched;
    }
    // The footer: the hovered (or inspected) card's lines, or the hint.
    let footer = 4.0 * 30.0 + 12.0;
    let area = Rect::from_min_max(pos2(x, y), pos2(right, rect.max.y - 16.0 - footer));
    let flow = if full {
        Flow::Right
    } else {
        Flow::Down
    };
    // The graph: a card per node, its sections boxed around their passes (in the order they first run).
    let aspect = frame_aspect(view);
    let card = (8.5, 1.33 + 8.5 * aspect + 0.34);
    let graph = dag_graph(view, card);
    let laid = dag_layout(ink, &graph, flow, aspect);
    let (state, picks, lit) = dag_view(panel, id, area, laid);
    let hovered = picks.hovered;
    let inspected = view
        .inspected
        .as_deref()
        .and_then(|k| k.rsplit_once('.'))
        .and_then(|(n, r)| {
            view.nodes
                .iter()
                .position(|x| x.name == n && x.result.as_deref() == Some(r))
        });
    let focus = hovered.or(inspected);
    let any_lit = !lit.is_empty();
    let looks = dag_node_looks(view, &lit, inspected, hovered);
    let edge_looks = dag_edge_looks(view, focus);
    let clusters: Vec<bool> = graph
        .clusters
        .iter()
        .map(|c| !any_lit || c.members.iter().any(|m| lit.contains(m)))
        .collect();
    let t = state.transform(area);
    // The card being dragged on top: drawn last.
    graph_view::draw(
        ink,
        area.expand(4.0),
        &t,
        &graph,
        laid,
        &graph_view::Look {
            nodes: looks,
            edges: edge_looks,
            clusters,
            moved: &state.moved,
            edge_labels: false,
            tracked: true,
        },
    );
    // The hovered card's arrows named by their resources, where they leave or enter it.
    dag_arrow_labels(ink, area, view, laid, &state, &t, focus);
    // The footer.
    let fy = rect.max.y - 16.0 - footer + 8.0;
    dag_footer(ink, view, focus, (x, right), fy);
    // The dock's Frame graph tab describes the card under the pointer.
    ctx.data_mut(|d| {
        d.insert_temp(
            egui::Id::unique("frame-graph-hovered"),
            hovered.map(|i| view.nodes[i].name.clone()),
        )
    });
    picks.clicked.and_then(|i| {
        Some(GraphPick::Inspect(format!(
            "{}.{}",
            view.nodes[i].name,
            view.nodes[i].result.as_ref()?
        )))
    })
}

/// The DAG's graph: a card per node, its sections boxed around their passes (in the order they first run).
fn dag_graph(view: &GraphView, card: (f32, f32)) -> crate::graph_view::Graph {
    use crate::graph_view;
    let prefix = graph_prefix(view.nodes.iter().map(|n| n.name.as_str()));
    let entry = |name: &str| crate::graph::entry_of(name).to_string();
    let nodes: Vec<graph_view::Node> = view
        .nodes
        .iter()
        .map(|n| {
            let name = node_label(n, prefix);
            graph_view::Node {
                id: n.name.clone(),
                title: vec![name],
                card: Some(card),
                ..Default::default()
            }
        })
        .collect();
    let scene = crate::graph::GraphScene::current("frame");
    let mut clusters: Vec<graph_view::Cluster> = Vec::new();
    let mut placed = vec![false; nodes.len()];
    for i in 0..nodes.len() {
        // Each node not yet placed starts a cluster: its section's passes boxed together, else the node alone.
        if placed[i] {
            continue;
        }
        let section = scene.and_then(|s| {
            s.sections
                .iter()
                .find(|sec| sec.passes.contains(&entry(&view.nodes[i].name)))
        });
        let members: Vec<usize> = match section {
            Some(sec) => (0..nodes.len())
                .filter(|&k| !placed[k] && sec.passes.contains(&entry(&view.nodes[k].name)))
                .collect(),
            None => vec![i],
        };
        for &k in &members {
            placed[k] = true;
        }
        clusters.push(graph_view::Cluster {
            title: section.map(|s| s.title.replace('\n', " ")),
            members,
            boxed: section.is_some(),
        });
    }
    let edges: Vec<graph_view::Edge> = view
        .edges
        .iter()
        .map(|e| graph_view::Edge {
            from: e.from,
            to: e.to,
            dotted: e.data.is_empty(),
            label: None,
        })
        .collect();

    graph_view::Graph {
        nodes,
        edges,
        clusters,
    }
}

/// The DAG laid out, once per graph and flow (`aspect`: its cards' pictures' shape).
fn dag_layout(
    ink: &Ink,
    graph: &crate::graph_view::Graph,
    flow: crate::graph_view::Flow,
    aspect: f32,
) -> &'static crate::graph_view::Laid {
    use crate::graph_view;
    // Between layers, room for a box's padding on both sides and (top to bottom) its title.
    let column = if flow == graph_view::Flow::Down {
        3.2
    } else {
        2.2
    };
    let options = graph_view::Options {
        flow,
        spacing: Some(crate::layered::Spacing {
            column,
            row: 0.7,
            dummy: 0.5,
        }),
        pad: 0.5,
        title: 1.5,
        gap: 1.0,
    };

    ({
        use std::hash::{Hash, Hasher};
        static CACHE: std::sync::OnceLock<
            std::sync::Mutex<std::collections::HashMap<u64, &'static graph_view::Laid>>,
        > = std::sync::OnceLock::new();
        let mut h = std::collections::hash_map::DefaultHasher::new();
        format!("{graph:?}{flow:?}").hash(&mut h);
        let width = |t: &str| ink.measure(t, 100.0, FontFamily::Proportional).x / 100.0;
        let mut cache = CACHE.get_or_init(Default::default).lock().unwrap();
        *cache.entry(h.finish()).or_insert_with(|| {
            Box::leak(Box::new(graph_view::lay_out(
                graph,
                &width,
                &[],
                aspect,
                &options,
            )))
        })
    }) as _
}

/// The DAG's view: kept in the viewer, set up again for a new graph; a story's highlight centres its nodes (the
/// lit ones, returned with the view and the pointer's picks).
fn dag_view(
    panel: &GraphPanel,
    id: egui::Id,
    area: Rect,
    laid: &'static crate::graph_view::Laid,
) -> (
    crate::graph_view::ViewState,
    crate::graph_view::Picks,
    Vec<usize>,
) {
    use crate::graph_view;
    let GraphPanel {
        ctx,
        view,
        highlight,
        interactive,
        ..
    } = *panel;
    let scale = MIN_TEXT_PT;
    // The view: kept in the viewer; set up again for a new graph; a story's highlight centres its nodes.
    let lit: Vec<usize> = view
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| highlight.contains(&n.name))
        .map(|(i, _)| i)
        .collect();
    let key = {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (laid as *const graph_view::Laid as usize, highlight).hash(&mut h);
        h.finish()
    };
    let setup = |v: &mut graph_view::ViewState, same: bool| {
        v.scale = scale;
        v.centre = graph_view::centre_on(laid, area, scale, &lit).unwrap_or(if same {
            v.centre
        } else {
            graph_view::start(laid, area, scale)
        });
    };
    // The wheel zooms (out to the whole graph, in to four times the readable scale); a drag on the background pans.
    let (mut state, picks) = graph_view::view(
        ctx,
        interactive.then_some(id),
        area,
        laid,
        key,
        &graph_view::Controls {
            zoom: Some(graph_view::zoom_range(laid, area, scale)),
            move_nodes: true,
        },
        &setup,
    );
    graph_view::keep(&mut state, laid, area);
    if interactive {
        ctx.data_mut(|d| d.insert_temp(id, state.clone()));
    }
    (state, picks, lit)
}

/// The DAG's cards: the `lit` ones outlined, the others faded beside them; the inspected and hovered ones
/// outlined; each its result, and its GPU time as a badge.
fn dag_node_looks(
    view: &GraphView,
    lit: &[usize],
    inspected: Option<usize>,
    hovered: Option<usize>,
) -> Vec<crate::graph_view::NodeLook> {
    use crate::graph_view;
    let any_lit = !lit.is_empty();
    let looks: Vec<graph_view::NodeLook> = view
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let is_lit = lit.contains(&i);
            let (outline, text) = if is_lit {
                (Stroke::new(3.0, palette::accent()), palette::text())
            } else if Some(i) == inspected || Some(i) == hovered {
                (Stroke::new(2.0, palette::text()), palette::text())
            } else if any_lit || !n.live {
                (Stroke::new(1.0, palette::border()), palette::text_faint())
            } else {
                (Stroke::new(1.0, palette::border()), palette::text_dim())
            };
            graph_view::NodeLook {
                fill: palette::background(),
                outline,
                text,
                rows: Vec::new(),
                picture: n.thumbnail.filter(|_| n.live),
                tint: if any_lit && !is_lit {
                    Color32::from_gray(140)
                } else {
                    Color32::WHITE
                },
                badge: n
                    .ms
                    .filter(|_| view.timestamps)
                    .map(|ms| format!("{ms:.2} ms")),
                note: Some(if n.live {
                    "–".to_string()
                } else {
                    tr("graph-culled").to_string()
                }),
                hidden: false,
                alpha: 1.0,
            }
        })
        .collect();
    looks
}

/// The DAG's arrows, coloured by their resources' lifetimes; those of the `focus`ed card stronger.
fn dag_edge_looks(view: &GraphView, focus: Option<usize>) -> Vec<crate::graph_view::EdgeLook> {
    use crate::graph_view;
    let edge_looks: Vec<graph_view::EdgeLook> = view
        .edges
        .iter()
        .map(|e| {
            let touches = focus.is_some_and(|f| f == e.from || f == e.to);
            let color = e.data.first().map_or(palette::text_faint(), |&r| {
                graph_lifetime_color(view.resources[r].lifetime)
            });
            let alpha = if focus.is_none() || touches {
                1.0
            } else {
                0.25
            };
            let width = if e.data.is_empty() {
                1.0
            } else if touches {
                3.0
            } else {
                2.0
            };
            graph_view::EdgeLook {
                color: if e.data.is_empty() {
                    color.gamma_multiply(0.6 * alpha)
                } else {
                    color.gamma_multiply(alpha)
                },
                width,
                hidden: false,
                drawn: 1.0,
            }
        })
        .collect();
    edge_looks
}

/// The focused card's arrows named by their resources, where they leave or enter it.
fn dag_arrow_labels(
    ink: &Ink,
    area: Rect,
    view: &GraphView,
    laid: &crate::graph_view::Laid,
    state: &crate::graph_view::ViewState,
    t: &crate::graph_view::Transform,
    focus: Option<usize>,
) {
    use crate::graph_view::{self, Flow};
    let painter = ink.painter;
    let clipped = painter.with_clip_rect(area);
    let cink = Ink::new(&clipped);
    let mut placed_labels: Vec<Rect> = Vec::new();
    if let Some(f) = focus {
        for e in view
            .edges
            .iter()
            .filter(|e| (e.from == f || e.to == f) && !e.data.is_empty())
        {
            let (a, z) = graph_view::ends(
                graph_view::placed(laid, &state.moved, e.from),
                graph_view::placed(laid, &state.moved, e.to),
                laid.flow,
            );
            let along = if laid.flow == Flow::Down {
                vec2(0.0, 0.9)
            } else {
                vec2(1.6, 0.0)
            };
            let at = t.at(if e.from == f {
                a + along
            } else {
                z - along
            });
            let names: Vec<&str> = e
                .data
                .iter()
                .map(|&r| view.resources[r].name.as_str())
                .collect();
            let text = names.join(", ");
            let color = graph_lifetime_color(view.resources[e.data[0]].lifetime);
            let size = cink.measure(&text, 24.0, FontFamily::Monospace);
            let half_w = (size.x + 12.0) / 2.0;
            let at = pos2(
                at.x.clamp(
                    area.min.x + half_w,
                    (area.max.x - half_w).max(area.min.x + half_w),
                ),
                at.y,
            );
            let bg = Rect::from_center_size(at, size + vec2(12.0, 4.0));
            if placed_labels.iter().any(|p| p.intersects(bg)) {
                continue;
            }
            placed_labels.push(bg);
            clipped.rect_filled(bg, CornerRadius::same(4), palette::panel());
            cink.text(
                at,
                Align2::CENTER_CENTER,
                text,
                24.0,
                FontFamily::Monospace,
                color,
            );
        }
    }
    ink.min_size
        .set(ink.min_size.get().min(cink.min_size.get()));
}

/// The DAG's footer at `fy`: the focused card's pass, its engine pass, and what it does; or the hint.
fn dag_footer(
    ink: &Ink,
    view: &GraphView,
    focus: Option<usize>,
    (x, right): (f32, f32),
    mut fy: f32,
) {
    let painter = ink.painter;
    let width = right - x;
    painter.line_segment(
        [pos2(x, fy - 6.0), pos2(right, fy - 6.0)],
        Stroke::new(1.0, palette::border()),
    );
    if let Some(i) = focus {
        let node = &view.nodes[i];
        let line = match &node.result {
            Some(r) => format!("{} → {r}", node.name),
            None => node.name.clone(),
        };
        ink.text(
            pos2(x, fy),
            Align2::LEFT_TOP,
            ink.fit_right(&line, width, 24.0, FontFamily::Monospace),
            24.0,
            FontFamily::Monospace,
            palette::text(),
        );
        fy += 30.0;
        if let Some((pass, at)) = &node.engine {
            let text = format!(
                "≈ {}: {pass} · {at}",
                investigation::current().wording().engine_short
            );
            ink.text(
                pos2(x, fy),
                Align2::LEFT_TOP,
                ink.fit_right(&text, width, 24.0, FontFamily::Proportional),
                24.0,
                FontFamily::Proportional,
                palette::accent(),
            );
        }
        fy += 30.0;
        if let Some(about) = node.about {
            footer_text(ink, pos2(x, fy), width, about, 2);
        }
    } else {
        footer_text(ink, pos2(x, fy), width, tr("graph-dag_hint"), 3);
    }
}

/// The selected pass's WGSL beside the full-screen DAG (src/shader_source.rs):
/// the module its pipelines are compiled from, as it is now (a live reload
/// shows here at once), cut to the pass's entry points and what they use,
/// each piece under the file and line it comes from; `failed`, why the last
/// live reload did not compile. Read-only; it scrolls both ways.
pub(crate) fn draw_source_pane(
    ctx: &egui::Context,
    ink: &Ink,
    rect: Rect,
    view: &GraphView,
    failed: Option<&str>,
) {
    let painter = ink.painter;
    crate::paint::panel_card(painter, rect, 12);
    let x = rect.min.x + 22.0;
    let width = rect.width() - 44.0;
    let mut y = rect.min.y + 16.0;
    let node = view
        .inspected
        .as_deref()
        .and_then(|k| k.rsplit_once('.'))
        .map(|(n, _)| n.to_string());
    let Some(node) = node else {
        ink.text(
            pos2(x, y),
            Align2::LEFT_TOP,
            ink.fit_right(
                tr("graph-source_pick"),
                width,
                24.0,
                FontFamily::Proportional,
            ),
            24.0,
            FontFamily::Proportional,
            palette::text_dim(),
        );
        return;
    };
    let shading = investigation::current().scene_shading();
    let graph = crate::graph::FrameGraph::current(&[]);
    let Some(n) = graph.node(&node) else { return };
    let (module, entries) = crate::graph::source_of(shading, &graph.nodes[n]);
    ink.text(
        pos2(x, y),
        Align2::LEFT_TOP,
        ink.fit_right(&format!("WGSL · {node}"), width, 30.0, bold()),
        30.0,
        bold(),
        palette::text(),
    );
    y += 42.0;
    let from = tr_with(
        "graph-source_module",
        args!(module = module, entries = entries.join(", ")),
    );
    ink.text(
        pos2(x, y),
        Align2::LEFT_TOP,
        ink.fit_right(&from, width, 24.0, FontFamily::Proportional),
        24.0,
        FontFamily::Proportional,
        palette::text_dim(),
    );
    y += 36.0;
    if let Some(error) = failed {
        let galley = painter.layout(
            error.to_string(),
            egui::FontId::monospace(24.0),
            palette::MISMATCH,
            width,
        );
        let shown = galley.size().y.min(6.0 * 30.0);
        painter
            .with_clip_rect(Rect::from_min_size(pos2(x, y), vec2(width, shown)))
            .galley(pos2(x, y), galley, palette::MISMATCH);
        y += shown + 10.0;
    }
    // The code view (src/code_view.rs): each piece under its file and line, the entry points
    // strong, lines as they are (the view scrolls both ways; another pass starts at its top).
    use crate::code_view;
    let body = Rect::from_min_max(pos2(rect.min.x + 22.0, y), rect.max - vec2(16.0, 12.0));
    let sections = source_sections(module, &entries);
    let scroll = code_view::Scroll {
        ctx,
        id: egui::Id::unique("frame-graph-source-scroll"),
        key: &node,
    };
    code_view::draw(
        ink,
        body,
        code_view::Language::Wgsl,
        &sections,
        false,
        Some(scroll),
    );
}

/// The code view's sections of a pass's WGSL: each piece of `module` that `entries` use under its file and line,
/// the entry points strong; or the error that kept it from being read.
fn source_sections(module: &str, entries: &[&str]) -> Vec<crate::code_view::Section> {
    use crate::code_view;
    let sections: Vec<code_view::Section> = match crate::shader_source::cached(module, entries) {
        Err(e) => vec![code_view::Section {
            header: None,
            lines: e
                .lines()
                .map(|l| code_view::Line {
                    text: l.to_string(),
                    lit: false,
                })
                .collect(),
            strong: true,
        }],
        Ok(items) => items
            .iter()
            .map(|item| code_view::Section {
                header: Some(format!("{}:{}", item.file, item.line)),
                lines: item
                    .text
                    .lines()
                    .map(|l| code_view::Line {
                        text: l.to_string(),
                        lit: false,
                    })
                    .collect(),
                strong: entries.contains(&item.name.as_str()),
            })
            .collect(),
    };
    sections
}

/// `text` wrapped to `width` from `at`, at most `lines` lines of 30 points
/// (the last ends in "…" when it is cut).
pub(super) fn footer_text(ink: &Ink, at: Pos2, width: f32, text: &str, lines: usize) {
    let font = egui::FontId::proportional(24.0);
    let galley = ink
        .painter
        .layout(text.to_string(), font.clone(), palette::text_dim(), width);
    let rows: Vec<String> = galley.rows.iter().map(|r| r.text()).collect();
    for (k, row) in rows.iter().take(lines).enumerate() {
        let row = if k + 1 == lines && rows.len() > lines {
            ink.fit_right(
                &format!("{}…", row.trim_end()),
                width,
                24.0,
                FontFamily::Proportional,
            )
        } else {
            row.trim_end().to_string()
        };
        ink.text(
            at + vec2(0.0, k as f32 * 30.0),
            Align2::LEFT_TOP,
            row,
            24.0,
            FontFamily::Proportional,
            palette::text_dim(),
        );
    }
}
