//! The cards the lab's panels and viewport overlays are drawn on.

use crate::palette;
use egui::{Color32, CornerRadius, Painter, Rect, Stroke, StrokeKind};

/// The dark fill of a card over the scene, at `alpha` (premultiplied).
pub const fn overlay_fill(alpha: u8) -> Color32 {
    Color32::from_rgba_premultiplied(7, 18, 31, alpha)
}

/// A panel's card: the theme's panel colour, a thin border.
pub fn panel_card(painter: &Painter, rect: Rect, radius: u8) {
    painter.rect_filled(rect, CornerRadius::same(radius), palette::panel());
    painter.rect_stroke(rect, CornerRadius::same(radius), Stroke::new(1.0, palette::border()), StrokeKind::Inside);
}

/// A card over the scene (a formula, a pane's title, a callout): the dark
/// fill, and a border of `stroke`.
pub fn overlay_card(painter: &Painter, rect: Rect, radius: u8, stroke: Color32) {
    painter.rect_filled(rect, CornerRadius::same(radius), overlay_fill(235));
    painter.rect_stroke(rect, CornerRadius::same(radius), Stroke::new(1.5, stroke), StrokeKind::Inside);
}
