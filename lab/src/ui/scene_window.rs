//! The Scene window (src/ui/windows.rs) and the gizmo it puts in the viewport: what the shown scene holds
//! (src/entity.rs), one row each, the selected one's values (the sliders that place it, as the dock draws
//! them), and the gizmo's mode; the gizmo (src/gizmo.rs) on the selected entity, its handle under the
//! pointer lit.

use super::*;
use crate::entity::{self, Entity, EntityKind};
use crate::gizmo::{self, Handle, Mode};
use egui::RichText;
use windows::WindowId;

/// A row's mark, painted at `at` (its centre): a sun for a light, a dot for a point, a filled diamond for an
/// object that moves, an outlined square for one that stays.
fn paint_icon(painter: &Painter, at: Pos2, e: &Entity) {
    let dim = palette::text_dim();
    match (e.kind, e.movable()) {
        (EntityKind::Light, _) => {
            let sun = Color32::from_rgb(240, 196, 92);
            painter.circle_filled(at, 4.5, sun);
            for k in 0..8 {
                let a = k as f32 * std::f32::consts::FRAC_PI_4;
                let d = vec2(a.cos(), a.sin());
                painter.line_segment([at + d * 7.0, at + d * 9.5], Stroke::new(1.5, sun));
            }
        }
        (EntityKind::Point, _) => {
            painter.circle_filled(at, 4.0, palette::accent());
        }
        (EntityKind::Object, true) => {
            let points = vec![at + vec2(0.0, -7.0), at + vec2(7.0, 0.0), at + vec2(0.0, 7.0), at + vec2(-7.0, 0.0)];
            painter.add(egui::Shape::convex_polygon(points, palette::accent(), Stroke::NONE));
        }
        (EntityKind::Object, false) => {
            painter.rect_stroke(Rect::from_center_size(at, vec2(11.0, 11.0)), CornerRadius::same(2), Stroke::new(1.5, dim), StrokeKind::Inside);
        }
    }
}

/// The Scene window's content, in its body.
pub(super) fn draw_scene_window(ctx: &egui::Context, ink: &Ink, input: &UiInput, viewer: &mut ViewerChrome) {
    let Some(window) = input.layout.window(WindowId::Scene).filter(|w| !w.collapsed) else { return };
    draw_window_card(ink, &input.layout, WindowId::Scene);
    let inner = window.body.shrink2(vec2(16.0, 10.0));
    let entities = entity::list(input.state, input.statics);
    let selected = viewer.selected.as_ref().and_then(|id| entities.iter().find(|e| &e.id == id)).cloned();
    let mut dock = crate::controls::Dock::new(input, viewer);
    let mode = viewer.gizmo.mode;
    let mut actions = Vec::new();
    egui::Area::new(egui::Id::unique("viewer-scene-window")).fixed_pos(inner.min).constrain(false).fade_in(false).show(ctx, |ui| {
        ui.set_width(inner.width());
        ui.set_clip_rect(inner);
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        let spacing = ui.spacing_mut();
        spacing.button_padding = vec2(10.0, 3.0);
        spacing.item_spacing = vec2(8.0, 4.0);
        spacing.slider_width = crate::controls::SLIDER_WIDTH;
        // The gizmo's mode: a turn and a size only for what has them.
        ui.horizontal(|ui| {
            let can = |m: Mode| m == Mode::Move || selected.as_ref().is_some_and(|e| e.yaw.is_some());
            for (m, label) in [(Mode::Move, tr("outliner-move")), (Mode::Rotate, tr("outliner-rotate")), (Mode::Scale, tr("outliner-scale"))] {
                let shown = if can(mode) { mode } else { Mode::Move };
                let r = ui.add_enabled(can(m), egui::Button::selectable(shown == m, label));
                if r.on_hover_text(tr("outliner-gizmo_hint")).clicked() {
                    actions.push(ViewerAction::SetGizmo(m));
                }
            }
        });
        ui.separator();
        // The list: as tall as its rows, up to 40 % of the window; the rest for the selected entity's values.
        let list_height = ((inner.height() - 60.0) * 0.4).max(90.0);
        egui::ScrollArea::vertical().id_salt("viewer-scene-list").max_height(list_height).min_scrolled_height(list_height).auto_shrink([false, true]).show(ui, |ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
            // Rows the window's width, their names from the left.
            ui.with_layout(egui::Layout::top_down_justified(egui::Align::Min), |ui| {
            for e in &entities {
                // Room for the icon, painted over the row.
                let mut text = format!("      {}", e.name);
                if !e.visible {
                    text.push_str(&format!("  · {}", tr("outliner-hidden")));
                }
                let color = if e.movable() && e.visible { palette::text() } else { palette::text_dim() };
                let on = selected.as_ref().is_some_and(|s| s.id == e.id);
                let row = ui.add(egui::Button::selectable(on, RichText::new(text).color(color)));
                paint_icon(ui.painter(), pos2(row.rect.min.x + 16.0, row.rect.center().y), e);
                dock.widgets.push((format!("entity:{}", e.id), row.rect));
                if row.clicked() {
                    actions.push(ViewerAction::Select((!on).then(|| e.id.clone())));
                }
            }
            });
        });
        ui.separator();
        let Some(e) = &selected else {
            ui.label(RichText::new(tr("outliner-none_selected")).color(palette::text_dim()));
            return;
        };
        ui.horizontal(|ui| {
            ui.label(RichText::new(&e.name).font(egui::FontId::new(24.0, bold())).color(palette::accent()));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if e.movable() && ui.button(tr("outliner-reset")).on_hover_text(tr("outliner-reset_hint")).clicked() {
                    for key in e.position.iter().flatten().chain(&e.yaw).chain(&e.scale) {
                        if let Some(default) = crate::values::entry(key).and_then(|en| en.default.map(|d| d.components()[crate::values::component(en, key)])) {
                            actions.push(ViewerAction::SetParam(key, default));
                        }
                    }
                }
                if ui.button(tr("outliner-focus")).on_hover_text(tr("outliner-focus_hint")).clicked() {
                    let mut camera = dock.state.camera;
                    camera.target = e.at.to_array();
                    actions.push(ViewerAction::SetCamera(camera));
                }
            });
        });
        if !e.movable() {
            let why = if e.kind == EntityKind::Object && input.statics.scene.objects.iter().any(|o| o.movable && entity::camel(o.name) == e.id["object.".len()..]) { tr("outliner-fixed_baked") } else { tr("outliner-fixed_object") };
            ui.add(egui::Label::new(RichText::new(why).color(palette::text_dim())).wrap());
            return;
        }
        // Its values: the toggle that shows it, where it is, its turn and its size (a vector's entry once).
        // An area grows with its content: the rows scroll in what the window has left.
        let room = (inner.max.y - ui.cursor().min.y).max(40.0);
        egui::ScrollArea::vertical().id_salt("viewer-scene-values").max_height(room).min_scrolled_height(room).auto_shrink([false, true]).show(ui, |ui| {
            dock.rows(ui, "viewer-scene-rows", |d, ui| {
                let mut done: Vec<&'static str> = Vec::new();
                for key in e.shown.iter().chain(e.position.iter().flatten()).chain(&e.yaw).chain(&e.scale) {
                    let Some(entry) = crate::values::entry(key) else { continue };
                    if !done.contains(&entry.key) {
                        done.push(entry.key);
                        d.value(ui, key);
                    }
                }
            });
        });
    });
    viewer.actions.extend(dock.actions);
    viewer.actions.extend(actions);
    viewer.widgets.extend(dock.widgets);
}

/// A colour lit for a handle under the pointer or dragged.
fn lit(color: Color32, on: bool) -> Color32 {
    if on { color.lerp_to_gamma(Color32::WHITE, 0.55) } else { color }
}

/// The selected entity's gizmo over the view on screen (`content` its frame), as the viewer's input finds
/// its handles (src/viewer/input.rs).
pub(super) fn draw_gizmo(ink: &Ink, input: &UiInput, content: &FrameContent, viewer: &ViewerChrome) {
    if (viewer.timeline_mode && viewer.playing) || !input.panes.is_empty() || !input.overs.is_empty() || input.compare.is_some() {
        return;
    }
    let Some(id) = &viewer.selected else { return };
    let Some(e) = entity::list(input.state, input.statics).into_iter().find(|e| &e.id == id && e.movable() && e.visible) else { return };
    let l = &input.layout;
    let projector = crate::camera::Projector::new(&content.uniforms.camera);
    let mode = if e.yaw.is_some() { viewer.gizmo.mode } else { Mode::Move };
    let Some(shape) = gizmo::shape(&projector, e.at, mode, gizmo::ARM * l.scale) else { return };
    let painter = ink.painter.with_clip_rect(l.viewport);
    let at = |p: glam::Vec2| l.viewport_pixel_to_canvas(p);
    let on = |h: Handle| viewer.gizmo.active == Some(h) || (viewer.gizmo.active.is_none() && viewer.gizmo.hover == Some(h));
    let axes = [palette::AXIS_X, palette::AXIS_Y, palette::AXIS_Z];
    let center = at(shape.center);
    match mode {
        Mode::Move => {
            for n in 0..3 {
                if let Some(q) = shape.squares[n] {
                    let color = lit(axes[n], on(Handle::Plane(n)));
                    let points: Vec<Pos2> = q.iter().map(|p| at(*p)).collect();
                    painter.add(egui::Shape::convex_polygon(points.clone(), color.gamma_multiply(0.35), Stroke::new(1.5, color)));
                }
            }
            for i in 0..3 {
                let Some(tip) = shape.tips[i] else { continue };
                let (color, width) = if on(Handle::Axis(i)) { (lit(axes[i], true), 5.0) } else { (axes[i], 3.0) };
                let tip = at(tip);
                let dir = (tip - center).normalized();
                painter.line_segment([center, tip - dir * 12.0], Stroke::new(width, color));
                let side = vec2(-dir.y, dir.x) * 7.0;
                painter.add(egui::Shape::convex_polygon(vec![tip, tip - dir * 16.0 + side, tip - dir * 16.0 - side], color, Stroke::NONE));
            }
        }
        Mode::Rotate => {
            let (color, width) = if on(Handle::Ring) { (lit(palette::AXIS_Y, true), 5.0) } else { (palette::AXIS_Y, 3.0) };
            painter.add(egui::Shape::line(shape.ring.iter().map(|p| at(*p)).collect(), Stroke::new(width, color)));
        }
        Mode::Scale => {
            let color = lit(palette::text(), on(Handle::Scale));
            let handle = at(shape.scale);
            painter.line_segment([center, handle], Stroke::new(3.0, color));
            painter.rect_filled(Rect::from_center_size(handle, vec2(16.0, 16.0)), CornerRadius::same(3), color);
        }
    }
    painter.circle(center, 5.0, palette::text(), Stroke::new(1.5, palette::background()));
}
