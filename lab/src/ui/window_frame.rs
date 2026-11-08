//! The viewer's windows as drawn (src/ui/windows.rs places them): each one's card under its content, and
//! over it, its title bar: the title, the chevron that collapses it, and the drags that move it, dock it to
//! an edge, and resize it. Every window has this one frame, whatever its content.

use super::*;
use windows::{Dock, WindowFrame, WindowId};

/// The size of a floating window's resize corner, and the width of a docked window's resize edge.
const GRIP: f32 = 18.0;

/// Window `id`'s card, which its content then draws in (its body; the title bar above it); none for a
/// collapsed one (`draw_window_titles` draws its title bar).
pub(super) fn draw_window_card(ink: &Ink, l: &Layout, id: WindowId) {
    if let Some(w) = l.window(id).filter(|w| !w.collapsed) {
        crate::paint::panel_card(ink.painter, w.frame, 12);
        ink.painter.hline(
            w.frame.x_range().shrink(1.0),
            w.title.max.y,
            Stroke::new(1.0, palette::border()),
        );
    }
}

/// What window `id`'s title bar says: the Panel window names what it shows.
fn title(id: WindowId, input: &UiInput, viewer: &ViewerChrome) -> String {
    match id {
        WindowId::Scene => tr("window-scene").to_string(),
        WindowId::Controls => tr("window-controls").to_string(),
        WindowId::Panel => {
            let feature = viewer
                .feature_panel()
                .and_then(|_| viewer.features.feature(viewer.tab.0))
                .or_else(|| viewer.features.panel_feature(input.state.panel));
            match (feature.and_then(|f| f.label()), input.state.panel) {
                (Some(label), _) => label,
                (None, PanelMode::Performance) => tr("window-performance").to_string(),
                _ => tr("window-data").to_string(),
            }
        }
    }
}

/// Every window's title bar, over its card: drawn last of the windows, so a drag reaches it.
pub(super) fn draw_window_titles(
    ctx: &egui::Context,
    ink: &Ink,
    input: &UiInput,
    viewer: &mut ViewerChrome,
) {
    let l = &input.layout;
    let pointer = ctx.pointer_hover_pos();
    for w in l.windows.iter().flatten().copied() {
        // The pointer over a window is the window's: no orbit, no pick, no zoom.
        viewer.ui_has_pointer |= pointer.is_some_and(|p| w.frame.contains(p));
        // A collapsed window has no content to draw its card: its title bar is all of it.
        if w.collapsed {
            crate::paint::panel_card(ink.painter, w.frame, 12);
        }
        let chevron = Rect::from_center_size(
            pos2(w.title.max.x - 24.0, w.title.center().y),
            vec2(32.0, 32.0),
        );
        let text = title(w.id, input, viewer);
        let text = ink.fit_right(&text, chevron.min.x - w.title.min.x - 28.0, 24.0, bold());
        ink.text(
            pos2(w.title.min.x + 18.0, w.title.center().y),
            Align2::LEFT_CENTER,
            text,
            24.0,
            bold(),
            palette::text(),
        );
        title_drag(
            ctx,
            ink,
            l,
            w,
            Rect::from_min_max(w.title.min, pos2(chevron.min.x - 4.0, w.title.max.y)),
            viewer,
        );
        // The chevron: down when open, right when collapsed.
        egui::Area::new(egui::Id::unique(("window-chevron", w.id.key())))
            .fixed_pos(chevron.min)
            .order(egui::Order::Background)
            .show(ctx, |ui| {
                let (_, response) = ui.allocate_exact_size(chevron.size(), egui::Sense::click());
                let color = if response.hovered() {
                    palette::accent()
                } else {
                    palette::text_dim()
                };
                let c = chevron.center();
                let points = if w.collapsed {
                    vec![
                        c + vec2(-4.0, -7.0),
                        c + vec2(6.0, 0.0),
                        c + vec2(-4.0, 7.0),
                    ]
                } else {
                    vec![
                        c + vec2(-7.0, -4.0),
                        c + vec2(7.0, -4.0),
                        c + vec2(0.0, 6.0),
                    ]
                };
                ink.painter
                    .add(egui::Shape::convex_polygon(points, color, Stroke::NONE));
                let hint = if w.collapsed {
                    tr("window-expand")
                } else {
                    tr("window-collapse")
                };
                if response.on_hover_text(hint).clicked() {
                    viewer.actions.push(ViewerAction::Window(
                        w.id,
                        WindowChange::Collapse(!w.collapsed),
                    ));
                    viewer.actions.push(ViewerAction::SaveLayout);
                }
            });
        if !w.collapsed {
            resize_grip(ctx, ink, w, viewer);
        }
    }
}

/// The title bar's drag: it moves the window (a docked one floats), shows where a drop near an edge would
/// dock it, and docks or leaves it there when let go; a double-click puts the window back in its place.
fn title_drag(
    ctx: &egui::Context,
    ink: &Ink,
    l: &Layout,
    w: WindowFrame,
    grip: Rect,
    viewer: &mut ViewerChrome,
) {
    egui::Area::new(egui::Id::unique(("window-title", w.id.key())))
        .fixed_pos(grip.min)
        .order(egui::Order::Background)
        .show(ctx, |ui| {
            let (_, response) = ui.allocate_exact_size(grip.size(), egui::Sense::click_and_drag());
            if response.dragged() {
                ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
            } else if response.hovered() {
                ctx.set_cursor_icon(egui::CursorIcon::Grab);
            }
            let response = response.on_hover_text(tr("window-move_hint"));
            if response.double_clicked() {
                viewer
                    .actions
                    .push(ViewerAction::Window(w.id, WindowChange::Default));
                viewer.actions.push(ViewerAction::SaveLayout);
                return;
            }
            // On the frame it is let go, the pointer is no longer the drag's: where it was last seen.
            let dock = response
                .interact_pointer_pos()
                .or_else(|| ctx.input(|i| i.pointer.latest_pos()))
                .and_then(|p| windows::dock_at(l.area, p));
            if response.dragged() {
                let delta = response.drag_delta();
                if delta != Vec2::ZERO {
                    viewer
                        .actions
                        .push(ViewerAction::Window(w.id, WindowChange::Move(delta)));
                }
                // Where it would dock, lit while the drop would dock it there.
                if let Some(dock) = dock {
                    let hint = windows::dock_hint(l.area, dock);
                    ink.painter.rect_filled(
                        hint,
                        CornerRadius::same(12),
                        palette::accent().gamma_multiply(0.18),
                    );
                    ink.painter.rect_stroke(
                        hint,
                        CornerRadius::same(12),
                        Stroke::new(2.0, palette::accent()),
                        StrokeKind::Inside,
                    );
                }
            }
            if response.drag_stopped() {
                viewer.actions.push(ViewerAction::Window(
                    w.id,
                    WindowChange::Drop(dock.unwrap_or(Dock::Float)),
                ));
                viewer.actions.push(ViewerAction::SaveLayout);
            }
        });
}

/// What resizes a window: a docked one's edge toward the viewport, a floating one's lower right corner.
fn resize_grip(ctx: &egui::Context, ink: &Ink, w: WindowFrame, viewer: &mut ViewerChrome) {
    let f = w.frame;
    let half = windows::GAP / 2.0;
    let (rect, cursor) = match w.dock {
        Dock::Left => (
            Rect::from_min_max(
                pos2(f.max.x - 2.0, f.min.y),
                pos2(f.max.x + half + 2.0, f.max.y),
            ),
            egui::CursorIcon::ResizeHorizontal,
        ),
        Dock::Right => (
            Rect::from_min_max(
                pos2(f.min.x - half - 2.0, f.min.y),
                pos2(f.min.x + 2.0, f.max.y),
            ),
            egui::CursorIcon::ResizeHorizontal,
        ),
        Dock::Bottom => (
            Rect::from_min_max(
                pos2(f.min.x, f.min.y - half - 2.0),
                pos2(f.max.x, f.min.y + 2.0),
            ),
            egui::CursorIcon::ResizeVertical,
        ),
        Dock::Float => (
            Rect::from_min_max(f.max - vec2(GRIP, GRIP), f.max),
            egui::CursorIcon::ResizeNwSe,
        ),
    };
    egui::Area::new(egui::Id::unique(("window-resize", w.id.key())))
        .fixed_pos(rect.min)
        .order(egui::Order::Background)
        .show(ctx, |ui| {
            let (_, response) = ui.allocate_exact_size(rect.size(), egui::Sense::drag());
            let active = response.hovered() || response.dragged();
            if active {
                ctx.set_cursor_icon(cursor);
            }
            let stroke = Stroke::new(
                3.0,
                if response.dragged() {
                    palette::accent()
                } else if active {
                    palette::text_dim()
                } else {
                    palette::border()
                },
            );
            match w.dock {
                Dock::Left | Dock::Right if active => {
                    ink.painter
                        .vline(rect.center().x, rect.y_range().shrink(24.0), stroke);
                }
                Dock::Bottom if active => {
                    ink.painter
                        .hline(rect.x_range().shrink(24.0), rect.center().y, stroke);
                }
                // A floating window always shows its corner: two short diagonals.
                Dock::Float => {
                    for k in [5.0, 10.0] {
                        ink.painter.line_segment(
                            [
                                pos2(f.max.x - k - 3.0, f.max.y - 3.0),
                                pos2(f.max.x - 3.0, f.max.y - k - 3.0),
                            ],
                            Stroke::new(1.5, stroke.color),
                        );
                    }
                }
                _ => {}
            }
            if response.dragged()
                && let Some(p) = response.interact_pointer_pos()
            {
                viewer
                    .actions
                    .push(ViewerAction::Window(w.id, WindowChange::Resize(p)));
            }
            if response.drag_stopped() {
                viewer.actions.push(ViewerAction::SaveLayout);
            }
        });
}
