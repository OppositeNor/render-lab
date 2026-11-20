//! The homepage's fields page: a folder per field of study (src/investigations.rs `fields`), each a square glass
//! card with its icon and its name, the cards centred on the page in rows. A folder opens the selection page of its
//! field.

use super::*;
use crate::investigations::Field;

/// A card's size, the room between cards, and its icon's size, at the 1080p reference.
const CARD: Vec2 = vec2(360.0, 360.0);
const GAP: f32 = 48.0;
const ICON: f32 = 160.0;

impl Home {
    /// The fields page: its heading and Back, then the folders, centred in rows.
    pub(super) fn draw_fields(&mut self, ctx: &egui::Context, ink: &ui::Ink, canvas: Vec2) {
        self.page_header(ctx, ink, canvas, tr("home-choose_field"));
        let fields = investigations::fields();
        let per_row = (((canvas.x - 80.0 + GAP) / (CARD.x + GAP)).floor() as usize)
            .clamp(1, fields.len().max(1));
        let rows = fields.len().div_ceil(per_row);
        let height = rows as f32 * (CARD.y + GAP) - GAP;
        let top = ((canvas.y - height) / 2.0).max(130.0);
        let mut picked = None;
        for (n, field) in fields.iter().enumerate() {
            // The cards of a row, the last one perhaps shorter, are centred across the page.
            let row = n / per_row;
            let in_row = (fields.len() - row * per_row).min(per_row);
            let width = in_row as f32 * (CARD.x + GAP) - GAP;
            let min = pos2(
                (canvas.x - width) / 2.0 + (n % per_row) as f32 * (CARD.x + GAP),
                top + row as f32 * (CARD.y + GAP),
            );
            if self.field_card(ctx, ink, Rect::from_min_size(min, CARD), *field) {
                picked = Some(n);
            }
        }
        ink.text(
            pos2(40.0, canvas.y - 58.0),
            Align2::LEFT_TOP,
            tr("home-fields_esc_hint"),
            24.0,
            FontFamily::Proportional,
            palette::text_faint(),
        );
        if let Some(n) = picked {
            self.open_field(n);
        }
    }

    /// The selection page of field `n` (of `investigations::fields()`), its first engine and feature chosen, unless
    /// it is the field chosen already.
    pub(super) fn open_field(&mut self, n: usize) {
        if n != self.field {
            self.field = n;
            (self.engine, self.feature, self.part, self.open) = (0, 0, 0, false);
            self.edition = None;
        }
        self.page = Page::Select;
    }

    /// "RENDER LAB" and the page's `heading` at the top left, and Back at the top right (as Esc: back one page).
    pub(super) fn page_header(
        &mut self,
        ctx: &egui::Context,
        ink: &ui::Ink,
        canvas: Vec2,
        heading: &str,
    ) {
        ink.text(
            pos2(40.0, 20.0),
            Align2::LEFT_TOP,
            "RENDER LAB",
            24.0,
            ui::bold(),
            palette::accent(),
        );
        ink.text(
            pos2(38.0, 52.0),
            Align2::LEFT_TOP,
            heading,
            38.0,
            ui::bold(),
            palette::text(),
        );
        let back_size = vec2(200.0, 60.0);
        egui::Area::new(egui::Id::unique("home-back"))
            .fixed_pos(pos2(canvas.x - 40.0 - back_size.x, 30.0))
            .fade_in(false)
            .constrain(false)
            .show(ctx, |ui| {
                let back = glass_button(ui, back_size, tr("home-back"), 28.0, true);
                self.widgets.push(("back".into(), back.rect));
                if back.clicked() {
                    self.back();
                }
            });
    }

    /// One field's folder in `rect`: its icon above its name and its number of investigations, outlined and tinted
    /// under the pointer. True when it is clicked.
    fn field_card(&mut self, ctx: &egui::Context, ink: &ui::Ink, rect: Rect, field: Field) -> bool {
        let id = field.id();
        let mut response = None;
        egui::Area::new(egui::Id::unique(("home-field", &id)))
            .fixed_pos(rect.min)
            .fade_in(false)
            .constrain(false)
            .show(ctx, |ui| {
                let (_, r) = ui.allocate_exact_size(rect.size(), Sense::click());
                response = Some(r);
            });
        let response = response.expect("the area shows its contents");
        self.widgets.push((format!("field:{id}"), rect));
        let hot = response.hovered();
        let painter = ink.painter;
        glass_panel(painter, rect, 28, true);
        if hot {
            painter.rect_stroke(
                rect,
                CornerRadius::same(28),
                Stroke::new(2.5, palette::accent()),
                StrokeKind::Inside,
            );
        }
        let color = if hot {
            palette::accent()
        } else {
            palette::text()
        };
        // The name, bold and centred, wrapping within the card; the icon above it and the count under it, the three
        // centred in the card as one block.
        let mut job = egui::text::LayoutJob::simple(
            field.title().to_string(),
            ink.font(36.0, ui::bold()),
            palette::text(),
            CARD.x - 48.0,
        );
        job.halign = egui::Align::Center;
        let name = painter.layout_job(job);
        let count_height = ink.measure("0", 24.0, FontFamily::Proportional).y;
        let block = ICON + 20.0 + name.size().y + 10.0 + count_height;
        let icon = Rect::from_center_size(
            pos2(
                rect.center().x,
                rect.min.y + (CARD.y - block) / 2.0 + ICON / 2.0,
            ),
            vec2(ICON, ICON),
        );
        if let Some(texture) = self.icon(ctx, field.icon()) {
            painter.image(
                texture,
                icon,
                Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
                color,
            );
        }
        let name_top = icon.max.y + 20.0;
        let below = name_top + name.size().y;
        painter.galley(pos2(rect.center().x, name_top), name, palette::text());
        let count = investigations::all()
            .iter()
            .filter(|i| investigations::field_of(**i).is(field))
            .count();
        ink.text(
            pos2(rect.center().x, below + 10.0),
            Align2::CENTER_TOP,
            tr_with("home-field_count", args!(count = count)),
            24.0,
            FontFamily::Proportional,
            palette::text_faint(),
        );
        let response = match field.summary() {
            Some(summary) => response.on_hover_text(summary),
            None => response,
        };
        response.clicked()
    }
}
