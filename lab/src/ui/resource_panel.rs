//! The resources panel: every resource of the frame as a card, as the frame leaves it.

use super::*;

/// The resources panel (`PanelMode::Resources`), or full screen: every
/// resource of the frame as a card, in the order the graph lists them
/// (the lab's, then the investigation's targets, buffers, volumes): its
/// name, its state at the end of the frame (the inspector's image right
/// after its last writer, `FrameGraph::last_user`), its lifetime and shape. A story's
/// `highlight` lights the resources its nodes read and write. Hovering a
/// card names its writers and readers; a click inspects it. The view pans
/// like the DAG's.
pub(crate) fn draw_resource_panel(panel: &GraphPanel) -> Option<GraphPick> {
    let GraphPanel {
        ctx,
        ink,
        rect,
        view,
        highlight,
        full,
        interactive,
        ..
    } = *panel;
    let painter = ink.painter;
    crate::paint::panel_card(painter, rect, 12);
    let x = rect.min.x + 22.0;
    let right = rect.max.x - 22.0;
    let (y, switched) = graph_header(
        &GraphPanel {
            source: None,
            ..*panel
        },
        GraphPlace::Resources,
    );
    let shown: Vec<usize> = (0..view.resources.len())
        .filter(|&r| view.resources[r].name != "frame")
        .collect();
    let id = egui::Id::unique(if full {
        "frame-graph-resources-overlay"
    } else {
        "frame-graph-resources-panel"
    });
    if switched == Some(GraphPick::Reset) {
        crate::graph_view::ask_reset(ctx, id);
    } else if shown.is_empty() || switched.is_some() {
        return switched;
    }
    let footer = 3.0 * 30.0 + 12.0;
    let area = Rect::from_min_max(pos2(x, y), pos2(right, rect.max.y - 16.0 - footer));
    // The cards in a grid (graph units: a label's font is 1 high), drawn and dragged as any graph's.
    use crate::graph_view;
    let scale = MIN_TEXT_PT;
    let (graph, laid, card, columns) = resource_cards(view, &shown, area.width() / scale);
    // Lit: what the called-out nodes read and write.
    let lit = lit_resources(view, &shown, highlight);
    let key = cards_key(columns, shown.len(), card, highlight);
    let setup = |v: &mut graph_view::ViewState, same: bool| {
        v.scale = scale;
        v.centre = graph_view::centre_on(&laid, area, scale, &lit).unwrap_or(if same {
            v.centre
        } else {
            graph_view::start(&laid, area, scale)
        });
    };
    let (mut state, picks) = graph_view::view(
        ctx,
        interactive.then_some(id),
        area,
        &laid,
        key,
        &graph_view::Controls {
            zoom: Some(graph_view::zoom_range(&laid, area, scale)),
            move_nodes: true,
        },
        &setup,
    );
    graph_view::keep(&mut state, &laid, area);
    if interactive {
        ctx.data_mut(|d| d.insert_temp(id, state.clone()));
    }
    let hovered = picks.hovered;
    let inspected = inspected_card(view, &shown);
    let looks = resource_looks(view, &shown, &lit, hovered.or(inspected));
    graph_view::draw(
        ink,
        area.expand(4.0),
        &state.transform(area),
        &graph,
        &laid,
        &graph_view::Look {
            nodes: looks,
            edges: Vec::new(),
            clusters: Vec::new(),
            moved: &state.moved,
            edge_labels: false,
            tracked: true,
        },
    );
    // The footer: the hovered (or inspected) resource's writers and readers, or the hint.
    let fy = rect.max.y - 16.0 - footer + 8.0;
    resource_footer(
        ink,
        view,
        hovered.or(inspected).map(|k| shown[k]),
        (x, right),
        fy,
    );
    picks
        .clicked
        .and_then(|k| view.resources[shown[k]].key.clone())
        .map(GraphPick::Inspect)
}

/// The cards lit: the resources the called-out nodes read and write.
fn lit_resources(view: &GraphView, shown: &[usize], highlight: &[String]) -> Vec<usize> {
    let lit: Vec<usize> = (0..shown.len())
        .filter(|&k| {
            view.uses
                .iter()
                .any(|u| u.1 == shown[k] && highlight.contains(&view.nodes[u.0].name))
        })
        .collect();
    lit
}

/// The resources' cards in a grid `width` graph units wide (a label's font is 1 high): the graph, laid out, a
/// card's size, and the grid's columns.
fn resource_cards(
    view: &GraphView,
    shown: &[usize],
    width: f32,
) -> (
    crate::graph_view::Graph,
    crate::graph_view::Laid,
    Vec2,
    usize,
) {
    use crate::graph_view;
    let gap = 0.7;
    let columns = (((width + gap) / (9.2 + gap)).floor() as usize).clamp(1, shown.len());
    let card_w = ((width - (columns - 1) as f32 * gap) / columns as f32).min(14.0);
    let card = vec2(
        card_w,
        1.33 + card_w * frame_aspect(view) + graph_view::ROW + 0.34,
    );
    let left = (width - (columns as f32 * (card.x + gap) - gap)) / 2.0;
    let rects: Vec<Rect> = (0..shown.len())
        .map(|k| {
            Rect::from_min_size(
                pos2(
                    left + (k % columns) as f32 * (card.x + gap),
                    (k / columns) as f32 * (card.y + gap),
                ),
                card,
            )
        })
        .collect();
    let bounds = rects
        .iter()
        .copied()
        .reduce(|a, r| a.union(r))
        .expect("a card");
    let laid = graph_view::Laid {
        pictures: vec![0.0; rects.len()],
        routes: Vec::new(),
        boxes: Vec::new(),
        flow: graph_view::Flow::Down,
        bounds,
        rects,
    };
    let nodes: Vec<graph_view::Node> = shown
        .iter()
        .map(|&r| {
            let res = &view.resources[r];
            let detail = format!("{} · {}", graph_lifetime_label(res.lifetime), res.shape);
            graph_view::Node {
                id: res.name.clone(),
                title: vec![res.name.clone()],
                rows: vec![graph_view::Row {
                    id: res.name.clone(),
                    label: detail,
                }],
                card: Some((card.x, card.y)),
                ..Default::default()
            }
        })
        .collect();
    let graph = graph_view::Graph {
        nodes,
        edges: Vec::new(),
        clusters: Vec::new(),
    };
    (graph, laid, card, columns)
}

/// The resources' cards: the `lit` ones outlined, the others faded beside them, the focused one outlined; each
/// its state as the frame leaves it.
fn resource_looks(
    view: &GraphView,
    shown: &[usize],
    lit: &[usize],
    focus: Option<usize>,
) -> Vec<crate::graph_view::NodeLook> {
    use crate::graph_view;

    shown
        .iter()
        .enumerate()
        .map(|(k, &r)| {
            let res = &view.resources[r];
            let is_lit = lit.contains(&k);
            let dim = !lit.is_empty() && !is_lit;
            let outline = if is_lit {
                Stroke::new(3.0, palette::accent())
            } else if Some(k) == focus {
                Stroke::new(2.0, palette::text())
            } else {
                Stroke::new(1.0, palette::border())
            };
            graph_view::NodeLook {
                fill: palette::background(),
                outline,
                text: if dim {
                    palette::text_faint()
                } else {
                    palette::text()
                },
                rows: vec![(graph_lifetime_color(res.lifetime), false)],
                picture: res.thumbnail,
                tint: if dim {
                    Color32::from_gray(140)
                } else {
                    Color32::WHITE
                },
                badge: None,
                note: Some("–".to_string()),
                hidden: false,
                alpha: 1.0,
            }
        })
        .collect()
}

/// The resources panel's footer at `fy`: the focused resource's last writer, its writers and readers; or the
/// hint.
fn resource_footer(
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
    if let Some(k) = focus {
        let resource = &view.resources[k];
        let head = match &resource.key {
            Some(key) => format!(
                "{} · {}",
                resource.name,
                tr_with(
                    "graph-after",
                    args!(node = key.rsplit_once('.').map_or("", |(n, _)| n))
                )
            ),
            None => resource.name.clone(),
        };
        ink.text(
            pos2(x, fy),
            Align2::LEFT_TOP,
            ink.fit_right(&head, width, 24.0, FontFamily::Monospace),
            24.0,
            FontFamily::Monospace,
            palette::text(),
        );
        fy += 30.0;
        for (label, names) in [
            (tr("graph-written_by"), &resource.writers),
            (tr("graph-read_by"), &resource.readers),
        ] {
            let list = if names.is_empty() {
                "–".to_string()
            } else {
                names.join(", ")
            };
            let text = format!("{label}: {list}");
            ink.text(
                pos2(x, fy),
                Align2::LEFT_TOP,
                ink.fit_right(&text, width, 24.0, FontFamily::Proportional),
                24.0,
                FontFamily::Proportional,
                palette::text_dim(),
            );
            fy += 30.0;
        }
    } else {
        ink.text(
            pos2(x, fy),
            Align2::LEFT_TOP,
            ink.fit_right(
                tr("graph-resources_hint"),
                width,
                24.0,
                FontFamily::Proportional,
            ),
            24.0,
            FontFamily::Proportional,
            palette::text_dim(),
        );
    }
}

/// The card of the inspected resource, if one is shown.
fn inspected_card(view: &GraphView, shown: &[usize]) -> Option<usize> {
    view.inspected.as_deref().and_then(|key| {
        shown
            .iter()
            .position(|&r| view.resources[r].key.as_deref() == Some(key))
    })
}

/// What the cards' view is set up for: their grid, their size, and the highlight.
fn cards_key(columns: usize, count: usize, card: Vec2, highlight: &[String]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (columns, count, (card.x * 100.0) as i32, highlight).hash(&mut h);
    h.finish()
}
