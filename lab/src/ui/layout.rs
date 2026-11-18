//! Containers that share out the room they are given, as Godot's boxes and Qt's layouts do, on top of egui:
//! parts side by side or stacked, fixed or stretched by weight (`boxes`, `split`); a list that fills a column
//! and wraps into the next before it needs to scroll (`flow`); and a scroll area that fills its room (`fill_scroll`).
//! egui sizes a widget by its contents; these size the room first, and each part fills its share.

use egui::{Align, Layout, Rect, Ui, UiBuilder, Vec2, pos2, vec2};

/// How much of a box's length one part takes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Size {
    /// A fixed length, in points.
    Fixed(f32),
    /// A share of what the fixed parts and the gaps leave, by weight, but at least `min` points.
    Stretch { weight: f32, min: f32 },
}

impl Size {
    /// A stretched part of weight 1 with no minimum.
    pub const FILL: Size = Size::Stretch {
        weight: 1.0,
        min: 0.0,
    };
}

/// The lengths of `sizes` in `length`, `gap` apart: fixed parts keep theirs, stretched ones share the rest by
/// weight, each at least its minimum (so the parts may overflow `length`).
pub fn lengths(length: f32, sizes: &[Size], gap: f32) -> Vec<f32> {
    let gaps = gap * sizes.len().saturating_sub(1) as f32;
    let (mut fixed, mut weights) = (0.0, 0.0);
    for size in sizes {
        match *size {
            Size::Fixed(l) => fixed += l,
            Size::Stretch { weight, .. } => weights += weight,
        }
    }
    let left = (length - fixed - gaps).max(0.0);
    sizes
        .iter()
        .map(|size| match *size {
            Size::Fixed(l) => l,
            Size::Stretch { weight, min } if weights > 0.0 => (left * weight / weights).max(min),
            Size::Stretch { min, .. } => min,
        })
        .collect()
}

/// `rect` cut into the parts of `sizes`, side by side (`across`) or stacked, `gap` apart.
pub fn split(rect: Rect, sizes: &[Size], gap: f32, across: bool) -> Vec<Rect> {
    let axis = if across {
        0
    } else {
        1
    };
    let mut at = rect.min[axis];
    lengths(rect.size()[axis], sizes, gap)
        .into_iter()
        .map(|l| {
            let mut part = rect;
            part.min[axis] = at;
            part.max[axis] = at + l;
            at += l + gap;
            part
        })
        .collect()
}

/// Parts side by side (`across`) or stacked that fill what `ui` has left; `add` fills part `i` in a child Ui of
/// its own, laid out top-down and clipped to its rect. The parts' rects are allocated whatever their contents.
pub fn boxes(
    ui: &mut Ui,
    sizes: &[Size],
    gap: f32,
    across: bool,
    mut add: impl FnMut(usize, &mut Ui),
) {
    let room = ui.available_rect_before_wrap();
    let parts = split(room, sizes, gap, across);
    let mut used = Rect::from_min_size(room.min, Vec2::ZERO);
    for (i, part) in parts.into_iter().enumerate() {
        let mut child = ui.new_child(
            UiBuilder::new()
                .id_salt(("layout-box", i))
                .max_rect(part)
                .layout(Layout::top_down(Align::Min)),
        );
        child.shrink_clip_rect(part);
        add(i, &mut child);
        used = used.union(part);
    }
    ui.advance_cursor_after_rect(used);
}

/// How `count` cells of `cell` size, `gap` apart, fill `height`: as many rows as fit (at least one), then the
/// columns they need. Returns (rows, columns).
pub fn flow_grid(count: usize, cell: Vec2, gap: Vec2, height: f32) -> (usize, usize) {
    let rows = (((height + gap.y) / (cell.y + gap.y)).floor() as usize).max(1);
    (rows, count.div_ceil(rows))
}

/// The width `count` cells of `cell` size take when they flow down columns of `height` (`flow`).
pub fn flow_width(count: usize, cell: Vec2, gap: Vec2, height: f32) -> f32 {
    let (_, columns) = flow_grid(count, cell, gap, height);
    (columns as f32 * (cell.x + gap.x) - gap.x).max(0.0)
}

/// `count` cells of `cell` size, down a column until `ui`'s height runs out, then on in the next column: a list
/// that fills its room before it scrolls. `add` fills cell `i` in a child Ui of the cell's rect.
pub fn flow(ui: &mut Ui, count: usize, cell: Vec2, gap: Vec2, mut add: impl FnMut(usize, &mut Ui)) {
    let room = ui.available_rect_before_wrap();
    let (rows, _) = flow_grid(count, cell, gap, room.height());
    let mut used = Rect::from_min_size(room.min, Vec2::ZERO);
    for i in 0..count {
        // Down the column first, then across.
        let (column, row) = (i / rows, i % rows);
        let at = pos2(
            room.min.x + column as f32 * (cell.x + gap.x),
            room.min.y + row as f32 * (cell.y + gap.y),
        );
        let rect = Rect::from_min_size(at, cell);
        let mut child = ui.new_child(
            UiBuilder::new()
                .id_salt(("layout-flow", i))
                .max_rect(rect)
                .layout(Layout::left_to_right(Align::Center)),
        );
        add(i, &mut child);
        used = used.union(rect);
    }
    ui.advance_cursor_after_rect(used);
}

/// A vertical scroll area that fills the room `ui` has left (not shrunk to its contents), its offset kept under
/// `id_salt`.
pub fn fill_scroll(ui: &mut Ui, id_salt: impl egui::AsIdSalt, add: impl FnOnce(&mut Ui)) {
    egui::ScrollArea::vertical()
        .id_salt(id_salt)
        .auto_shrink([false, false])
        .show(ui, add);
}

/// The size a cell needs for the widest of `texts` in `font` as a button draws it (its padding around the text).
pub fn text_cell(ui: &Ui, texts: impl IntoIterator<Item = String>, font: &egui::FontId) -> Vec2 {
    let pad = ui.spacing().button_padding;
    let width = texts.into_iter().fold(0.0f32, |w, text| {
        let galley = ui
            .ctx()
            .fonts_mut(|f| f.layout_no_wrap(text, font.clone(), egui::Color32::WHITE));
        w.max(galley.size().x)
    });
    let height = ui.ctx().fonts_mut(|f| f.row_height(font));
    vec2(width, height) + 2.0 * pad
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_parts_keep_their_length_and_stretched_ones_share_the_rest() {
        let sizes = [
            Size::Fixed(100.0),
            Size::FILL,
            Size::Stretch {
                weight: 3.0,
                min: 0.0,
            },
        ];
        assert_eq!(lengths(520.0, &sizes, 10.0), vec![100.0, 100.0, 300.0]);
    }

    #[test]
    fn a_stretched_part_keeps_its_minimum() {
        let sizes = [
            Size::Fixed(400.0),
            Size::Stretch {
                weight: 1.0,
                min: 200.0,
            },
        ];
        assert_eq!(lengths(500.0, &sizes, 0.0), vec![400.0, 200.0]);
    }

    #[test]
    fn split_places_the_parts_gap_apart() {
        let rect = Rect::from_min_size(pos2(10.0, 0.0), vec2(210.0, 50.0));
        let parts = split(rect, &[Size::Fixed(100.0), Size::FILL], 10.0, true);
        assert_eq!(
            parts[0],
            Rect::from_min_max(pos2(10.0, 0.0), pos2(110.0, 50.0))
        );
        assert_eq!(
            parts[1],
            Rect::from_min_max(pos2(120.0, 0.0), pos2(220.0, 50.0))
        );
    }

    #[test]
    fn a_list_wraps_into_the_columns_it_needs() {
        let (cell, gap) = (vec2(100.0, 20.0), vec2(10.0, 4.0));
        // 100 points hold four rows of 20 with gaps of 4: 22 cells take six columns.
        assert_eq!(flow_grid(22, cell, gap, 100.0), (4, 6));
        assert_eq!(flow_width(22, cell, gap, 100.0), 650.0);
        // Too low for one row still lays out one.
        assert_eq!(flow_grid(3, cell, gap, 5.0), (1, 3));
    }
}
