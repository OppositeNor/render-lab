//! The panels of the frame's passes: the performance panel (GPU time per
//! pass), and the frame graph as a grid, as a DAG with each pass's result,
//! as its resources' cards, and beside the WGSL a pass is compiled from.

use super::*;

/// The frame's graph as the graph panel shows it (src/graph.rs): its nodes
/// in execution order with their last GPU time, its resources, and what each
/// node does with each, with the inspected `<node>.<resource>`.
#[derive(Clone, Debug, Default)]
pub struct GraphView {
    pub nodes: Vec<GraphNodeView>,
    pub resources: Vec<GraphResourceView>,
    /// (node row, resource column, reads, writes).
    pub uses: Vec<(usize, usize, bool, bool)>,
    /// The DAG's arrows (`PanelMode::Dag`).
    pub edges: Vec<GraphEdgeView>,
    pub inspected: Option<String>,
    pub timestamps: bool,
}

#[derive(Clone, Debug)]
pub struct GraphNodeView {
    pub name: String,
    /// The engine pass it mirrors and where the source index cites it (`file:line`).
    pub engine: Option<(String, String)>,
    /// Its GPU time in the last measured frame, in milliseconds.
    pub ms: Option<f64>,
    pub live: bool,
    pub cached: bool,
    /// The resource its card shows (`FrameGraph::result_of`), and that image
    /// as it stood right after the node (`PanelMode::Dag` only).
    pub result: Option<String>,
    pub thumbnail: Option<(egui::TextureId, [u32; 2])>,
    /// What it does (`graph::about`).
    pub about: Option<&'static str>,
    pub reads: Vec<String>,
    pub writes: Vec<String>,
}

/// An arrow of the DAG, between two nodes by row (execution order): the
/// resources the first writes and the second reads (by column), or none
/// when the second only has to run later (it overwrites what the first used).
#[derive(Clone, Debug)]
pub struct GraphEdgeView {
    pub from: usize,
    pub to: usize,
    pub data: Vec<usize>,
}

#[derive(Clone, Debug)]
pub struct GraphResourceView {
    pub name: String,
    pub lifetime: crate::graph::Lifetime,
    pub shape: String,
    /// The nodes that write it and that read it, in execution order.
    pub writers: Vec<String>,
    pub readers: Vec<String>,
    /// `<node>.<resource>` of its state at the end of the frame
    /// (`FrameGraph::last_user`), and that image (`PanelMode::Resources` only).
    pub key: Option<String>,
    pub thumbnail: Option<(egui::TextureId, [u32; 2])>,
}

/// The frame's cost as the performance panel shows it (src/timing.rs): the
/// GPU, each pass's mean, 95th percentile, and last time in milliseconds,
/// and the recent frames' GPU times and frame times, oldest first.
#[derive(Clone, Debug, Default)]
pub struct PerformanceView {
    pub gpu: String,
    /// The device has timestamp queries.
    pub timestamps: bool,
    pub passes: Vec<(String, f64, f64, f64)>,
    pub totals: Vec<f64>,
    pub frames: Vec<f64>,
}

/// How an experiment's comparison shows (src/experiment.rs): its mode, the
/// reference's view (beside the live one, or the same), the wipe's line, the
/// difference's gain.
#[derive(Clone, Copy, Debug)]
pub struct CompareOverlay {
    pub mode: crate::experiment::Compare,
    pub reference: Rect,
    pub split: f32,
    pub gain: f32,
}

/// The performance panel: the GPU, the frame's GPU time and frame time,
/// their recent history against the frame-rate budgets, and a row per pass
/// (its share of the GPU time as a bar, its mean time), the most expensive
/// first. Passes named in `highlight` are called out (always shown), the
/// others dimmed.
pub(crate) fn draw_performance(
    ink: &Ink,
    rect: Rect,
    view: &PerformanceView,
    highlight: &[String],
) {
    let painter = ink.painter;
    let x = rect.min.x + 22.0;
    let right = rect.max.x - 22.0;
    let width = right - x;
    let mut y = rect.min.y + 16.0;
    ink.text(
        pos2(x, y),
        Align2::LEFT_TOP,
        tr("perf-title"),
        30.0,
        bold(),
        palette::text(),
    );
    y += 42.0;
    let gpu = ink.fit_right(&view.gpu, width, 24.0, FontFamily::Proportional);
    ink.text(
        pos2(x, y),
        Align2::LEFT_TOP,
        gpu,
        24.0,
        FontFamily::Proportional,
        palette::accent(),
    );
    y += 36.0;
    y = perf_headline(ink, (x, width), y, view) + 42.0;
    // The history: GPU time (accent) and frame time (faint), against the
    // budgets that fit its scale; smaller when the panel is short.
    let graph_height = ((rect.max.y - y) * 0.3).clamp(60.0, 160.0);
    let graph = Rect::from_min_size(pos2(x, y), vec2(width, graph_height));
    perf_history(ink, graph, view);
    y = graph.max.y + 8.0;
    let caption = tr_with(
        "perf-history",
        args!(frames = view.totals.len().max(view.frames.len())),
    );
    ink.text(
        pos2(x, y),
        Align2::LEFT_TOP,
        ink.fit_right(&caption, width, 24.0, FontFamily::Proportional),
        24.0,
        FontFamily::Proportional,
        palette::text_faint(),
    );
    y += 34.0;
    painter.line_segment(
        [pos2(x, y), pos2(right, y)],
        Stroke::new(1.0, palette::border()),
    );
    y += 10.0;
    perf_passes(ink, (x, right), y, rect.max.y, view, highlight);
}

/// The headline numbers at `y`: the GPU's time for the frame, and the time between frames (and the rate), each
/// the mean of the last 30. Where the numbers start.
fn perf_headline(ink: &Ink, (x, width): (f32, f32), y: f32, view: &PerformanceView) -> f32 {
    let mut y = y;
    let mean = |xs: &[f64]| {
        if xs.is_empty() {
            f64::NAN
        } else {
            xs.iter().sum::<f64>() / xs.len() as f64
        }
    };
    let ms = ms_label;
    // The headline numbers: the GPU's time for the frame, and the time between frames.
    let recent = |xs: &[f64]| mean(&xs[xs.len().saturating_sub(30)..]);
    let (gpu_ms, frame_ms) = (recent(&view.totals), recent(&view.frames));
    let half = width / 2.0;
    ink.text(
        pos2(x, y),
        Align2::LEFT_TOP,
        tr("perf-gpu"),
        24.0,
        FontFamily::Proportional,
        palette::text_dim(),
    );
    ink.text(
        pos2(x + half, y),
        Align2::LEFT_TOP,
        tr("perf-frame"),
        24.0,
        FontFamily::Proportional,
        palette::text_dim(),
    );
    y += 30.0;
    ink.text(
        pos2(x, y),
        Align2::LEFT_TOP,
        ms(gpu_ms),
        28.0,
        mono_bold(),
        palette::text(),
    );
    let fps = if frame_ms > 0.0 {
        format!(
            "{} · {:.0} fps",
            ms(frame_ms).replace(" ms", ""),
            1000.0 / frame_ms
        )
    } else {
        ms(frame_ms)
    };
    ink.text(
        pos2(x + half, y),
        Align2::LEFT_TOP,
        ink.fit_right(&fps, half, 28.0, mono_bold()),
        28.0,
        mono_bold(),
        palette::text(),
    );
    y
}

/// The history in `graph`: GPU time (accent) and frame time (faint), against the budgets that fit its scale.
fn perf_history(ink: &Ink, graph: Rect, view: &PerformanceView) {
    let painter = ink.painter;
    painter.rect_filled(graph, CornerRadius::same(6), palette::background());
    // The scale: the 95th percentile, so one slow frame (a scene loading) does not flatten the rest.
    let mut all: Vec<f64> = view
        .totals
        .iter()
        .chain(&view.frames)
        .copied()
        .filter(|v| v.is_finite())
        .collect();
    all.sort_by(|a, b| a.total_cmp(b));
    let p95 = all
        .get(((all.len() as f64 - 1.0) * 0.95).round().max(0.0) as usize)
        .copied()
        .unwrap_or(0.0);
    let top = p95.max(1000.0 / 120.0) * 1.25;
    let to_y = |v: f64| graph.max.y - (v / top).clamp(0.0, 1.0) as f32 * graph.height();
    let mut last_label: Option<f32> = None;
    for (budget, label) in [
        (1000.0 / 120.0, "120 fps"),
        (1000.0 / 60.0, "60 fps"),
        (1000.0 / 30.0, "30 fps"),
    ] {
        let gy = to_y(budget);
        if budget > top || gy < graph.min.y + 24.0 || last_label.is_some_and(|y| y - gy < 26.0) {
            continue;
        }
        last_label = Some(gy);
        painter.hline(graph.x_range(), gy, Stroke::new(1.0, palette::border()));
        ink.text(
            pos2(graph.max.x - 6.0, gy - 2.0),
            Align2::RIGHT_BOTTOM,
            label,
            24.0,
            FontFamily::Proportional,
            palette::text_faint(),
        );
    }
    for (series, color, thickness) in [
        (&view.frames, palette::text_faint(), 1.5),
        (&view.totals, palette::accent(), 2.5),
    ] {
        if series.len() < 2 {
            continue;
        }
        let n = series.len() - 1;
        let points: Vec<Pos2> = series
            .iter()
            .enumerate()
            .map(|(i, v)| pos2(graph.min.x + graph.width() * i as f32 / n as f32, to_y(*v)))
            .collect();
        painter.add(egui::Shape::line(points, Stroke::new(thickness, color)));
    }
}

/// Without timestamps, or before the first measurement, why there are no rows; else one row per pass from
/// `y`, the most expensive first, as many as fit above `bottom`: its name, a bar of its
/// share of the frame's GPU time, and its mean. The passes called out come first among equals: they always show.
fn perf_passes(
    ink: &Ink,
    (x, right): (f32, f32),
    mut y: f32,
    bottom: f32,
    view: &PerformanceView,
    highlight: &[String],
) {
    let width = right - x;
    if !view.timestamps {
        ink.text(
            pos2(x, y),
            Align2::LEFT_TOP,
            ink.fit_right(
                tr("perf-no_timestamps"),
                width,
                24.0,
                FontFamily::Proportional,
            ),
            24.0,
            FontFamily::Proportional,
            palette::text_dim(),
        );
        return;
    }
    if view.passes.is_empty() {
        ink.text(
            pos2(x, y),
            Align2::LEFT_TOP,
            tr("perf-measuring"),
            24.0,
            FontFamily::Proportional,
            palette::text_dim(),
        );
        return;
    }
    // One row per pass, the most expensive first, as many as fit: its name,
    // a bar of its share of the frame's GPU time, and its mean. The passes
    // called out come first among equals: they always show.
    let total: f64 = view.passes.iter().map(|p| p.1).sum::<f64>().max(1e-6);
    let any_lit = view.passes.iter().any(|p| highlight.contains(&p.0));
    let mut order: Vec<&(String, f64, f64, f64)> = view.passes.iter().collect();
    order.sort_by(|a, b| {
        highlight
            .contains(&b.0)
            .cmp(&highlight.contains(&a.0))
            .then(b.1.total_cmp(&a.1))
    });
    let row = 32.0;
    let room = ((bottom - 12.0 - y) / row).floor().max(1.0) as usize;
    let shown = if order.len() > room {
        room.saturating_sub(1).max(1)
    } else {
        order.len()
    };
    // Called-out passes first, then by cost: the rows drawn are re-sorted by cost.
    let mut rows: Vec<&(String, f64, f64, f64)> = order[..shown].to_vec();
    rows.sort_by(|a, b| b.1.total_cmp(&a.1));
    for (name, mean_ms, _, _) in rows {
        pass_row(
            ink,
            (x, width),
            y,
            (name, *mean_ms, total),
            (highlight.contains(name), any_lit),
        );
        y += row;
    }
    if shown < order.len() {
        let rest: f64 = order[shown..].iter().map(|p| p.1).sum();
        let more = tr_with(
            "perf-more",
            args!(count = order.len() - shown, time = format!("{rest:.2}")),
        );
        ink.text(
            pos2(x, y),
            Align2::LEFT_TOP,
            ink.fit_right(&more, width, 24.0, FontFamily::Proportional),
            24.0,
            FontFamily::Proportional,
            palette::text_faint(),
        );
    }
}

/// A pass's row at `y`: its name, a bar of its `share` of the frame's GPU time, and its mean; lit when it is
/// called out, dimmed when another is.
fn pass_row(
    ink: &Ink,
    (x, width): (f32, f32),
    y: f32,
    (name, mean_ms, total): (&str, f64, f64),
    (lit, any_lit): (bool, bool),
) {
    let painter = ink.painter;
    let (right, row) = (x + width, 32.0);
    let name_width = width * 0.45;
    let bar_x = x + name_width + 10.0;
    let bar_width = width * 0.28;
    let mean_ms = &mean_ms;
    if lit {
        painter.rect_filled(
            Rect::from_min_size(pos2(x - 10.0, y - 2.0), vec2(width + 20.0, row)),
            CornerRadius::same(5),
            palette::highlight_row(),
        );
    }
    let text_color = if lit || !any_lit {
        palette::text()
    } else {
        palette::text_faint()
    };
    ink.text(
        pos2(x, y),
        Align2::LEFT_TOP,
        ink.fit_right(pass_label(name), name_width, 24.0, FontFamily::Monospace),
        24.0,
        FontFamily::Monospace,
        text_color,
    );
    let track = Rect::from_min_size(pos2(bar_x, y + 9.0), vec2(bar_width, 11.0));
    painter.rect_filled(track, CornerRadius::same(3), palette::background());
    let share = (*mean_ms / total).clamp(0.0, 1.0) as f32;
    let bar_color = if lit || !any_lit {
        palette::accent()
    } else {
        palette::border()
    };
    painter.rect_filled(
        Rect::from_min_size(
            track.min,
            vec2((track.width() * share).max(2.0), track.height()),
        ),
        CornerRadius::same(3),
        bar_color,
    );
    let value = ms_label(*mean_ms);
    ink.text(
        pos2(right, y),
        Align2::RIGHT_TOP,
        value,
        24.0,
        FontFamily::Monospace,
        text_color,
    );
}

/// A cell of the graph panel's grid (its node's row, its resource's column), if there is one.
type Cell = Option<(usize, usize)>;

/// Where the graph panel's grid is: its left and top, a row's height, a column's width, and how many rows.
#[derive(Clone, Copy)]
struct Grid {
    x: f32,
    y: f32,
    row: f32,
    column: f32,
    rows: f32,
}

/// The graph panel (`PanelMode::Graph`): the frame's nodes top to bottom in
/// execution order, its resources as columns, a filled square where a node
/// writes a resource, a ring where it reads one, and each resource's
/// lifetime as a line from its first use to its last; each node's GPU time
/// on the right. Hovering a cell names it; a click on one returns its
/// `<node>.<resource>` (the inspector shows it).
pub(crate) fn draw_graph_panel(panel: &GraphPanel) -> Option<GraphPick> {
    let GraphPanel {
        ctx,
        ink,
        rect,
        view,
        highlight,
        ..
    } = *panel;
    let painter = ink.painter;
    crate::paint::panel_card(painter, rect, 12);
    let x = rect.min.x + 22.0;
    let right = rect.max.x - 22.0;
    let width = right - x;
    let (y, switched) = graph_header(
        &GraphPanel {
            full: false,
            source: None,
            ..*panel
        },
        GraphPlace::Grid,
    );
    if view.nodes.is_empty() || switched.is_some() {
        return switched;
    }
    // The legend's two rows and the hovered cell's line stay at the bottom.
    let footer = 4.0 * 30.0 + 12.0;
    let rows = view.nodes.len() as f32;
    let row = ((rect.max.y - 16.0 - footer - y) / rows).clamp(14.0, 32.0);
    let name_width = width * 0.40;
    let time_width = if view.timestamps {
        84.0
    } else {
        0.0
    };
    let grid_x = x + name_width + 8.0;
    let grid_width = (right - time_width - 8.0 - grid_x).max(40.0);
    let column = grid_width / view.resources.len().max(1) as f32;
    let pointer = ctx.input(|i| i.pointer.hover_pos());
    let clicked = ctx.input(|i| i.pointer.primary_clicked());
    let cell_of = |p: Pos2| -> Option<(usize, usize)> {
        if p.x < grid_x || p.x >= grid_x + grid_width || p.y < y || p.y >= y + rows * row {
            return None;
        }
        Some((
            ((p.y - y) / row) as usize,
            ((p.x - grid_x) / column) as usize,
        ))
    };
    let hovered = pointer
        .and_then(cell_of)
        .filter(|&(r, c)| view.uses.iter().any(|u| u.0 == r && u.1 == c));
    let inspected = view
        .inspected
        .as_deref()
        .and_then(|k| k.rsplit_once('.'))
        .and_then(|(n, r)| {
            Some((
                view.nodes.iter().position(|x| x.name == n)?,
                view.resources.iter().position(|x| x.name == r)?,
            ))
        });
    let grid = Grid {
        x: grid_x,
        y,
        row,
        column,
        rows,
    };
    // Rows: a band on every other one, the called-out and inspected ones lit.
    grid_rows(
        ink,
        view,
        highlight,
        &grid,
        (x, right),
        (hovered, inspected),
    );
    // Columns: each resource's lifetime, then its uses.
    grid_columns(painter, view, &grid, (hovered, inspected));
    // The footer: the cell hovered (or inspected), then the legend.
    let fy = rect.max.y - 16.0 - footer + 8.0;
    grid_footer(ink, view, hovered.or(inspected), (x, right), fy);
    let pick = pointer
        .and_then(cell_of)
        .filter(|_| clicked)
        .filter(|&(r, c)| view.uses.iter().any(|u| u.0 == r && u.1 == c));
    pick.map(|(r, c)| {
        GraphPick::Inspect(format!("{}.{}", view.nodes[r].name, view.resources[c].name))
    })
}

/// The grid's rows: a band on every other one, the called-out and inspected ones lit; each node's name (the
/// entries' shared prefix left out) and its GPU time.
fn grid_rows(
    ink: &Ink,
    view: &GraphView,
    highlight: &[String],
    grid: &Grid,
    (x, right): (f32, f32),
    (hovered, inspected): (Cell, Cell),
) {
    let (painter, width, y, row) = (ink.painter, right - x, grid.y, grid.row);
    let name_width = width * 0.40;
    let prefix = graph_prefix(view.nodes.iter().map(|n| n.name.as_str()));
    let any_lit = view.nodes.iter().any(|n| highlight.contains(&n.name));
    for (i, node) in view.nodes.iter().enumerate() {
        let top = y + i as f32 * row;
        let band = Rect::from_min_size(pos2(x - 10.0, top), vec2(width + 20.0, row));
        if highlight.contains(&node.name)
            || inspected.is_some_and(|(r, _)| r == i)
            || hovered.is_some_and(|(r, _)| r == i)
        {
            painter.rect_filled(band, CornerRadius::same(4), palette::highlight_row());
        } else if i % 2 == 1 {
            painter.rect_filled(
                band,
                CornerRadius::ZERO,
                palette::background().gamma_multiply(0.35),
            );
        }
        let color = if !node.live {
            palette::text_faint()
        } else if any_lit && !highlight.contains(&node.name) {
            palette::text_dim()
        } else {
            palette::text()
        };
        let size = (row - 6.0).clamp(18.0, 24.0);
        let label = node_label(node, prefix);
        ink.text(
            pos2(x, top + row / 2.0),
            Align2::LEFT_CENTER,
            ink.fit_right(&label, name_width, size, FontFamily::Monospace),
            size,
            FontFamily::Monospace,
            color,
        );
        if view.timestamps {
            let time = node.ms.map_or("–".to_string(), |ms| format!("{ms:.2}"));
            ink.text(
                pos2(right, top + row / 2.0),
                Align2::RIGHT_CENTER,
                time,
                size,
                FontFamily::Monospace,
                palette::text_dim(),
            );
        }
    }
}

/// The grid's columns: each resource's lifetime as a line from its first use to its last, then its uses (a filled
/// square where a node writes it, a ring where it reads it).
fn grid_columns(
    painter: &Painter,
    view: &GraphView,
    grid: &Grid,
    (hovered, inspected): (Cell, Cell),
) {
    let Grid {
        x: grid_x,
        y,
        row,
        column,
        rows,
    } = *grid;
    let lifetime_color = graph_lifetime_color;
    for (c, resource) in view.resources.iter().enumerate() {
        let cx = grid_x + (c as f32 + 0.5) * column;
        let used: Vec<usize> = view.uses.iter().filter(|u| u.1 == c).map(|u| u.0).collect();
        let color = lifetime_color(resource.lifetime);
        if hovered.is_some_and(|(_, h)| h == c) || inspected.is_some_and(|(_, h)| h == c) {
            painter.rect_filled(
                Rect::from_min_size(pos2(cx - column / 2.0, y), vec2(column, rows * row)),
                CornerRadius::same(2),
                palette::highlight_row(),
            );
        }
        if let (Some(&a), Some(&b)) = (used.iter().min(), used.iter().max()) {
            painter.line_segment(
                [
                    pos2(cx, y + (a as f32 + 0.5) * row),
                    pos2(cx, y + (b as f32 + 0.5) * row),
                ],
                Stroke::new(2.0, color.gamma_multiply(0.7)),
            );
        }
    }
    let mark = (column * 0.36).clamp(2.5, row * 0.32);
    for &(r, c, reads, writes) in &view.uses {
        let center = pos2(
            grid_x + (c as f32 + 0.5) * column,
            y + (r as f32 + 0.5) * row,
        );
        let color = lifetime_color(view.resources[c].lifetime);
        if writes {
            painter.rect_filled(
                Rect::from_center_size(center, vec2(mark * 2.0, mark * 2.0)),
                CornerRadius::same(1),
                color,
            );
        } else if reads {
            painter.circle_stroke(center, mark, Stroke::new(1.5, color));
        }
        if inspected == Some((r, c)) {
            painter.circle_stroke(center, mark + 4.0, Stroke::new(2.0, palette::text()));
        }
    }
}

/// The graph panel's footer at `fy`: the cell hovered (or inspected), then the legend, a swatch per kind of
/// lifetime.
fn grid_footer(
    ink: &Ink,
    view: &GraphView,
    cell: Option<(usize, usize)>,
    (x, right): (f32, f32),
    mut fy: f32,
) {
    let (painter, width, lifetime_color) = (ink.painter, right - x, graph_lifetime_color);
    painter.line_segment(
        [pos2(x, fy - 6.0), pos2(right, fy - 6.0)],
        Stroke::new(1.0, palette::border()),
    );
    if let Some((r, c)) = cell {
        let (_, _, reads, writes) = *view
            .uses
            .iter()
            .find(|u| u.0 == r && u.1 == c)
            .expect("a use");
        let resource = &view.resources[c];
        let verb = match (reads, writes) {
            (true, true) => tr("graph-reads_writes"),
            (false, true) => tr("graph-writes"),
            _ => tr("graph-reads"),
        };
        let line = format!("{} {verb} {}", view.nodes[r].name, resource.name);
        ink.text(
            pos2(x, fy),
            Align2::LEFT_TOP,
            ink.fit_right(&line, width, 24.0, FontFamily::Monospace),
            24.0,
            FontFamily::Monospace,
            palette::text(),
        );
        fy += 30.0;
        // The engine's pass it mirrors, where the source index cites it.
        if let Some((pass, at)) = &view.nodes[r].engine {
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
        let detail = format!(
            "{}: {}",
            graph_lifetime_label(resource.lifetime),
            resource.shape
        );
        ink.text(
            pos2(x, fy),
            Align2::LEFT_TOP,
            ink.fit_right(&detail, width, 24.0, FontFamily::Proportional),
            24.0,
            FontFamily::Proportional,
            palette::text_dim(),
        );
        fy += 30.0;
    } else {
        ink.text(
            pos2(x, fy),
            Align2::LEFT_TOP,
            ink.fit_right(tr("graph-hint"), width, 24.0, FontFamily::Proportional),
            24.0,
            FontFamily::Proportional,
            palette::text_dim(),
        );
        fy += 90.0;
    }
    // The legend: one swatch per kind of lifetime.
    let mut lx = x;
    for lifetime in [
        crate::graph::Lifetime::Transient,
        crate::graph::Lifetime::Imported,
        crate::graph::Lifetime::Persistent,
        crate::graph::Lifetime::History,
    ] {
        if !view.resources.iter().any(|r| r.lifetime == lifetime) {
            continue;
        }
        let label = graph_lifetime_label(lifetime);
        painter.rect_filled(
            Rect::from_center_size(pos2(lx + 7.0, fy + 12.0), vec2(12.0, 12.0)),
            CornerRadius::same(2),
            lifetime_color(lifetime),
        );
        let galley = ink.text(
            pos2(lx + 20.0, fy),
            Align2::LEFT_TOP,
            label,
            24.0,
            FontFamily::Proportional,
            palette::text_dim(),
        );
        lx = galley.max.x + 18.0;
        if lx > right - 60.0 {
            break;
        }
    }
}

/// What a click in the frame graph's panels asks for.
#[derive(Clone, Debug, PartialEq)]
pub enum GraphPick {
    /// Inspect `<node>.<resource>` (the inspector's inset).
    Inspect(String),
    /// Show the graph as a grid, a DAG, or its resources (`PanelMode::Graph`, `Dag`, `Resources`).
    Panel(PanelMode),
    /// Open or close the DAG (or the resources) full screen.
    Overlay(bool),
    /// Put the view's nodes back where the layout placed them.
    Reset,
    /// Show (or hide) the selected pass's WGSL beside the full-screen DAG.
    Source(bool),
}

/// A frame graph panel to draw: the context and ink it draws with, its
/// rect, the graph, the story's highlight, whether it is full screen
/// (`full`), whether the pointer works (`interactive`: the viewer), and, for
/// the full-screen DAG, whether the source pane beside it is open (`source`;
/// None: no source button).
#[derive(Clone, Copy)]
pub(crate) struct GraphPanel<'a> {
    pub ctx: &'a egui::Context,
    pub ink: &'a Ink<'a>,
    pub rect: Rect,
    pub view: &'a GraphView,
    pub highlight: &'a [String],
    pub full: bool,
    pub interactive: bool,
    pub source: Option<bool>,
}

/// Which view of the frame graph a panel draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum GraphPlace {
    Grid,
    Dag,
    Resources,
}

/// The graph panels' header: the title, the summary, and (in the viewer)
/// the buttons that switch between the grid, the DAG, and the resources, and
/// the full-screen view (`full`: drawn there). Returns where the content
/// starts, and a button's pick.
pub(super) fn graph_header(panel: &GraphPanel, place: GraphPlace) -> (f32, Option<GraphPick>) {
    let GraphPanel {
        ctx,
        ink,
        rect,
        view,
        interactive,
        ..
    } = *panel;
    let x = rect.min.x + 22.0;
    let right = rect.max.x - 22.0;
    let mut y = rect.min.y + 16.0;
    let mut pick = None;
    let mut buttons_left = right;
    if interactive {
        let pointer = ctx.input(|i| i.pointer.hover_pos());
        let clicked = ctx.input(|i| i.pointer.primary_clicked());
        let buttons = header_buttons(panel, place);
        for (label, active, action) in buttons {
            let size = ink.measure(label, 24.0, FontFamily::Proportional) + vec2(20.0, 8.0);
            let button = Rect::from_min_size(pos2(buttons_left - size.x, y + 2.0), size);
            let hovered = pointer.is_some_and(|p| button.contains(p));
            let fill = if active {
                palette::highlight_row()
            } else if hovered {
                palette::background()
            } else {
                Color32::TRANSPARENT
            };
            ink.painter.rect_filled(button, CornerRadius::same(6), fill);
            ink.painter.rect_stroke(
                button,
                CornerRadius::same(6),
                Stroke::new(1.0, palette::border()),
                StrokeKind::Inside,
            );
            ink.text(
                button.center(),
                Align2::CENTER_CENTER,
                label,
                24.0,
                FontFamily::Proportional,
                if active {
                    palette::text()
                } else {
                    palette::text_dim()
                },
            );
            if hovered && clicked && !active {
                pick = Some(action);
            }
            buttons_left = button.min.x - 8.0;
        }
    }
    ink.text(
        pos2(x, y),
        Align2::LEFT_TOP,
        ink.fit_right(tr("graph-title"), (buttons_left - x).max(0.0), 30.0, bold()),
        30.0,
        bold(),
        palette::text(),
    );
    y += 42.0;
    let summary = tr_with(
        "graph-summary",
        args!(nodes = view.nodes.len(), resources = view.resources.len()),
    );
    ink.text(
        pos2(x, y),
        Align2::LEFT_TOP,
        ink.fit_right(&summary, right - x, 24.0, FontFamily::Proportional),
        24.0,
        FontFamily::Proportional,
        palette::text_dim(),
    );
    (y + 36.0, pick)
}

/// The header's buttons, right to left: full screen (or close), the resources, the DAG, the grid, the WGSL
/// beside the full-screen DAG, and Reset where the nodes move; each its label, whether it is the place shown, and
/// its pick.
fn header_buttons(panel: &GraphPanel, place: GraphPlace) -> Vec<(&'static str, bool, GraphPick)> {
    let GraphPanel {
        view, full, source, ..
    } = *panel;
    let mut buttons: Vec<(&str, bool, GraphPick)> = vec![
        if full {
            (tr("graph-close"), false, GraphPick::Overlay(false))
        } else {
            (tr("graph-full_screen"), false, GraphPick::Overlay(true))
        },
        (
            tr("graph-resources"),
            place == GraphPlace::Resources,
            GraphPick::Panel(PanelMode::Resources),
        ),
        (
            tr("graph-dag"),
            place == GraphPlace::Dag,
            GraphPick::Panel(PanelMode::Dag),
        ),
    ];
    if !full {
        buttons.push((
            tr("graph-grid"),
            place == GraphPlace::Grid,
            GraphPick::Panel(PanelMode::Graph),
        ));
    }
    // The full-screen DAG, a pass selected: its WGSL beside the graph, or not.
    if let (Some(open), true) = (source, view.inspected.is_some() || source == Some(true)) {
        buttons.push((
            if open {
                tr("graph-hide_source")
            } else {
                tr("graph-show_source")
            },
            false,
            GraphPick::Source(!open),
        ));
    }
    // The views whose nodes move: a button to put them back.
    if place != GraphPlace::Grid {
        buttons.push((tr("graph-reset"), false, GraphPick::Reset));
    }
    buttons
}

/// The frame's shape (the composite's `hdr`), which the cards' pictures take.
pub(super) fn frame_aspect(view: &GraphView) -> f32 {
    view.nodes
        .iter()
        .filter(|n| n.result.as_deref() == Some("hdr"))
        .find_map(|n| n.thumbnail)
        .map_or(9.0 / 16.0, |(_, s)| s[1] as f32 / s[0].max(1) as f32)
        .clamp(0.3, 1.5)
}

/// A node's name in a panel: without the investigation's common `prefix`,
/// marked ↻ while its result is cached.
pub(super) fn node_label(node: &GraphNodeView, prefix: &str) -> String {
    let mut label = node
        .name
        .strip_prefix(prefix)
        .filter(|rest| !rest.is_empty())
        .unwrap_or(&node.name)
        .to_string();
    if node.cached {
        label.push_str(" ↻");
    }
    label
}

pub(super) fn graph_lifetime_color(lifetime: crate::graph::Lifetime) -> Color32 {
    match lifetime {
        crate::graph::Lifetime::Imported => palette::text_dim(),
        crate::graph::Lifetime::Transient => palette::accent(),
        crate::graph::Lifetime::Persistent => palette::VIEW,
        crate::graph::Lifetime::History => palette::OBB,
    }
}

/// The lower-case prefix every camel-case name with one shares (`o3de` of
/// `o3deMergeCS`, `o3dePrepassFS`); the lab's own names (`overlay`) have none.
pub(crate) fn graph_prefix<'a>(names: impl Iterator<Item = &'a str>) -> &'a str {
    let mut prefix: Option<&str> = None;
    for name in names.filter(|n| !crate::graph::LAB_NODES.contains(n)) {
        let head = name
            .find(|c: char| c.is_ascii_uppercase())
            .map_or(name, |i| &name[..i]);
        if head == name {
            continue;
        }
        match prefix {
            None => prefix = Some(head),
            Some(p) if p != head => return "",
            _ => {}
        }
    }
    prefix.unwrap_or("")
}

pub(super) fn graph_lifetime_label(lifetime: crate::graph::Lifetime) -> &'static str {
    match lifetime {
        crate::graph::Lifetime::Imported => tr("graph-imported"),
        crate::graph::Lifetime::Transient => tr("graph-transient"),
        crate::graph::Lifetime::Persistent => tr("graph-persistent"),
        crate::graph::Lifetime::History => tr("graph-history"),
    }
}

/// A pass's name on screen: the lab's own passes (src/renderer.rs) in the
/// interface's language; an investigation's chain passes by their entry point.
/// A story's `highlight` names them by `name`, as measured.
pub fn pass_label(name: &str) -> &str {
    match name {
        "depth views" => tr("perf-pass_depth_views"),
        "depth pre-pass" => tr("perf-pass_prepass"),
        "scene pass" => tr("perf-pass_scene"),
        "overlay" => tr("perf-pass_overlay"),
        "composite" => tr("perf-pass_composite"),
        other => other,
    }
}

pub(super) fn swatch(painter: &Painter, rect: Rect, colors: &[Color32]) {
    let n = colors.len().max(1) as f32;
    for (i, c) in colors.iter().enumerate() {
        let x0 = rect.min.x + rect.width() * i as f32 / n;
        let x1 = rect.min.x + rect.width() * (i + 1) as f32 / n;
        painter.rect_filled(
            Rect::from_min_max(pos2(x0, rect.min.y), pos2(x1, rect.max.y)),
            CornerRadius::ZERO,
            *c,
        );
    }
}

/// A time in milliseconds as the performance panel shows it ("–" when it is not a number).
fn ms_label(v: f64) -> String {
    if v.is_finite() {
        format!("{v:.2} ms")
    } else {
        "–".into()
    }
}
