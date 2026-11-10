//! The homepage's controls: glass panels and buttons, the exit icon, a
//! setting's row, and the settings' tabs.

use super::*;

/// An option's value as the Config page names it.
pub(super) fn on_off(on: bool) -> &'static str {
    if on {
        tr("home-on")
    } else {
        tr("home-off")
    }
}

/// Translucent white: the glass the page's panels and buttons are made of.
pub(super) fn glass(alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(255, 255, 255, alpha)
}

/// A frosted-glass panel; `smoked` darkens it, for panels full of text.
pub(super) fn glass_panel(painter: &egui::Painter, rect: Rect, radius: u8, smoked: bool) {
    let fill = if smoked {
        palette::theme().smoke
    } else {
        glass(14)
    };
    painter.rect_filled(rect, CornerRadius::same(radius), fill);
    painter.rect_stroke(
        rect,
        CornerRadius::same(radius),
        Stroke::new(1.5, glass(48)),
        StrokeKind::Inside,
    );
}

/// A glass button with its label centred; it brightens under the pointer.
pub(super) fn glass_button(
    ui: &mut egui::Ui,
    size: Vec2,
    label: &str,
    font: f32,
    enabled: bool,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(
        size,
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    let hot = enabled && response.hovered();
    let fill = if enabled && response.is_pointer_button_down_on() {
        52
    } else if hot {
        38
    } else {
        20
    };
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(16), glass(fill));
    painter.rect_stroke(
        rect,
        CornerRadius::same(16),
        Stroke::new(
            if hot {
                2.0
            } else {
                1.5
            },
            if hot {
                palette::accent()
            } else {
                glass(70)
            },
        ),
        StrokeKind::Inside,
    );
    let color = if enabled {
        palette::text()
    } else {
        palette::text_faint()
    };
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::new(font, ui::bold()),
        color,
    );
    response
}

/// The exit icon in `rect`: a door frame, open on the right, and an arrow
/// leaving through it, in red whatever the theme.
pub(super) fn exit_icon(painter: &egui::Painter, rect: Rect) {
    const RED: Color32 = Color32::from_rgb(0xe5, 0x48, 0x4d);
    let stroke = Stroke::new(rect.width() * 0.1, RED);
    let (l, r, t, b) = (rect.min.x, rect.max.x, rect.min.y, rect.max.y);
    let door = r - rect.width() * 0.4;
    painter.line(
        vec![
            pos2(door, t + rect.height() * 0.25),
            pos2(door, t),
            pos2(l, t),
            pos2(l, b),
            pos2(door, b),
            pos2(door, b - rect.height() * 0.25),
        ],
        stroke,
    );
    let y = rect.center().y;
    let head = rect.width() * 0.22;
    painter.line_segment([pos2(l + rect.width() * 0.3, y), pos2(r, y)], stroke);
    painter.line(
        vec![
            pos2(r - head, y - head),
            pos2(r, y),
            pos2(r - head, y + head),
        ],
        stroke,
    );
}

/// An option's row on the Config page: `add` draws its control, naming its
/// widgets. An option that differs from its default is marked, names the
/// default, and has a Restore; returns the row's rectangle and whether
/// Restore was clicked.
pub(super) fn option_row(
    ui: &mut egui::Ui,
    widgets: &mut Vec<(String, Rect)>,
    name: &str,
    changed: bool,
    default: &str,
    enabled: bool,
    add: impl FnOnce(&mut egui::Ui, &mut Vec<(String, Rect)>),
) -> (Rect, bool) {
    let mut restore = false;
    let row = ui.horizontal(|ui| {
        ui.set_min_height(48.0);
        add(ui, widgets);
        if changed {
            ui.add_space(12.0);
            ui.label(
                RichText::new(format!("{} {default}", tr("home-default"))).color(palette::accent()),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let r = glass_button(ui, vec2(190.0, 48.0), tr("home-restore"), 26.0, enabled);
                widgets.push((format!("restore:{name}"), r.rect));
                restore = r.clicked();
            });
        }
    });
    let r = row.response.rect;
    if changed {
        // A changed setting is marked by an accent bar to the left of its row.
        ui.painter().rect_filled(
            Rect::from_min_max(
                pos2(r.min.x - 20.0, r.min.y + 4.0),
                pos2(r.min.x - 14.0, r.max.y - 4.0),
            ),
            CornerRadius::same(3),
            palette::accent(),
        );
    }
    (r, restore)
}

/// A category's name on its tab.
pub(super) fn category_name(category: Category) -> &'static str {
    match category {
        Category::Interface => tr("home-interface"),
        Category::Render => tr("home-render"),
        Category::Audio => tr("home-audio"),
    }
}

/// A Config page tab: a glass button, outlined when it is the tab shown,
/// with a dot when its settings were edited.
pub(super) fn tab_button(
    ui: &mut egui::Ui,
    size: Vec2,
    label: &str,
    selected: bool,
    edited: bool,
    enabled: bool,
) -> egui::Response {
    let r = glass_button(ui, size, label, 30.0, enabled);
    let painter = ui.painter();
    if selected {
        painter.rect_filled(
            r.rect,
            CornerRadius::same(16),
            palette::accent().gamma_multiply(0.18),
        );
        painter.rect_stroke(
            r.rect,
            CornerRadius::same(16),
            Stroke::new(2.0, palette::accent()),
            StrokeKind::Inside,
        );
    }
    if edited {
        painter.circle_filled(
            pos2(r.rect.max.x - 22.0, r.rect.center().y),
            6.0,
            palette::accent(),
        );
    }
    r
}
