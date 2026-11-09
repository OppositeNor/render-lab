//! The homepage's pages: the title, the selection of an engine, a feature,
//! and what to do with it, the settings, and the leaving page.

use super::*;

impl Home {
    /// Draw the page on a canvas of `canvas` points; returns the choice made.
    /// `time`: the page's clock in seconds (the Config popup counts down on it).
    pub fn draw(&mut self, ctx: &egui::Context, canvas: Vec2, time: f32) -> Option<Choice> {
        self.widgets.clear();
        // Any enabled control: the glass buttons, the lists, the checkbox. Only set here
        // (a frame may take more than one pass, and only the first sees the click).
        self.clicked |= ctx.viewport(|v| v.interact_widgets.clicked.is_some());
        let painter = ctx.layer_painter(egui::LayerId::background());
        let ink = ui::Ink::new(&painter);
        let choice = match self.page {
            Page::Title => self.draw_title(ctx, &ink, canvas, time),
            Page::Select => self.draw_select(ctx, &ink, canvas),
            Page::Config => {
                self.draw_config(ctx, &ink, canvas, time);
                None
            }
        };
        if self.knowledge
            && self.page == Page::Select
            && crate::ui::draw_knowledge_overlay(
                ctx,
                canvas,
                &crate::ui::KnowledgeContext::default(),
            )
        {
            self.knowledge = false;
        }
        let choice = if self.pse {
            self.draw_pse(ctx, canvas).or(choice)
        } else {
            choice
        };
        self.popups.draw(ctx, canvas);
        self.min_text = self.min_text.min(ink.min_size.get());
        if choice.is_some() {
            self.error = None;
        }
        choice
    }

    /// The title page: the name, Play and Quit on a glass card, the language, and the credits.
    pub(super) fn draw_title(
        &mut self,
        ctx: &egui::Context,
        ink: &ui::Ink,
        canvas: Vec2,
        time: f32,
    ) -> Option<Choice> {
        let center = canvas.x / 2.0;
        let card = Rect::from_center_size(pos2(center, canvas.y * 0.5), vec2(900.0, 710.0));
        glass_panel(ink.painter, card, 28, false);
        ink.text(
            pos2(center, card.min.y + 64.0),
            Align2::CENTER_TOP,
            "Render Lab",
            128.0,
            ui::bold(),
            palette::text(),
        );
        let rule = card.min.y + 230.0;
        ink.painter.line_segment(
            [pos2(center - 150.0, rule), pos2(center + 150.0, rule)],
            Stroke::new(2.0, palette::accent()),
        );
        ink.text(
            pos2(center, rule + 22.0),
            Align2::CENTER_TOP,
            tr("home-subtitle"),
            28.0,
            FontFamily::Proportional,
            palette::text_dim(),
        );
        let button = vec2(460.0, 84.0);
        let mut choice = None;
        egui::Area::new(egui::Id::unique("home-title"))
            .fixed_pos(pos2(center - button.x / 2.0, rule + 96.0))
            .fade_in(false)
            .constrain(false)
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 22.0);
                let play = glass_button(ui, button, tr("home-play"), 40.0, true);
                self.widgets.push(("play".into(), play.rect));
                if play.clicked() {
                    self.page = Page::Select;
                }
                let config = glass_button(ui, button, tr("home-config"), 40.0, true);
                self.widgets.push(("config".into(), config.rect));
                if config.clicked() {
                    self.open_config(Category::Interface);
                }
                let quit = glass_button(ui, button, tr("home-quit"), 40.0, true);
                self.widgets.push(("quit".into(), quit.rect));
                if quit.clicked() {
                    choice = Some(Choice::Quit);
                }
            });
        ink.text(
            pos2(40.0, canvas.y - 52.0),
            Align2::LEFT_TOP,
            credits(),
            24.0,
            FontFamily::Proportional,
            palette::text_faint(),
        );

        // The language, at the top right: the globe and the language's name. It opens the
        // Config page with the Language option outlined.
        let globe = self.globe(ctx);
        let name = self.applied.language.name();
        let icon = 44.0;
        let size = vec2(
            20.0 + icon + 14.0 + ink.measure(name, 28.0, ui::bold()).x + 24.0,
            64.0,
        );
        egui::Area::new(egui::Id::unique("home-language"))
            .fixed_pos(pos2(canvas.x - 40.0 - size.x, 30.0))
            .fade_in(false)
            .constrain(false)
            .show(ctx, |ui| {
                let r = glass_button(ui, size, "", 28.0, true);
                let color = if r.hovered() {
                    palette::accent()
                } else {
                    palette::text()
                };
                let icon_rect = Rect::from_min_size(
                    pos2(r.rect.min.x + 20.0, r.rect.center().y - icon / 2.0),
                    vec2(icon, icon),
                );
                if let Some(globe) = globe {
                    ui.painter().image(
                        globe,
                        icon_rect,
                        Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
                        color,
                    );
                }
                ui.painter().text(
                    pos2(icon_rect.max.x + 14.0, r.rect.center().y),
                    Align2::LEFT_CENTER,
                    name,
                    FontId::new(28.0, ui::bold()),
                    color,
                );
                self.widgets.push(("language".into(), r.rect));
                if r.on_hover_text(tr("home-language_hint")).clicked() {
                    self.open_config(Category::Interface);
                    self.settings.flash = Some(time);
                }
            });
        choice
    }

    /// The photosensitivity warning: full screen and opaque, so nothing moves
    /// behind it (the title's scene is moving too), and the page cannot be
    /// clicked, until its Continue.
    pub(super) fn draw_pse(&mut self, ctx: &egui::Context, canvas: Vec2) -> Option<Choice> {
        let mut choice = None;
        egui::Area::new(egui::Id::unique("home-pse"))
            .order(egui::Order::Foreground)
            .fixed_pos(Pos2::ZERO)
            .fade_in(false)
            .constrain(false)
            .show(ctx, |ui| {
                let full = Rect::from_min_size(Pos2::ZERO, canvas);
                ui.allocate_rect(full, egui::Sense::click_and_drag());
                let painter = ui.painter();
                painter.rect_filled(full, CornerRadius::ZERO, palette::background());
                // The title, the message, then Exit and Continue, centred as one column.
                let width = 1080.0f32.min(canvas.x - 160.0);
                let text = painter.layout(
                    tr("home-pse_message").to_string(),
                    FontId::new(30.0, FontFamily::Proportional),
                    palette::text(),
                    width,
                );
                let height = 70.0 + text.size().y + 60.0 + 72.0;
                let top = ((canvas.y - height) / 2.0).max(40.0);
                let x = (canvas.x - width) / 2.0;
                painter.text(
                    pos2(canvas.x / 2.0, top),
                    Align2::CENTER_TOP,
                    tr("home-pse_title"),
                    FontId::new(40.0, ui::bold()),
                    palette::accent(),
                );
                let below = top + 70.0 + text.size().y;
                painter.galley(pos2(x, top + 70.0), text, palette::text());
                let size = vec2(320.0, 72.0);
                let exit =
                    Rect::from_min_size(pos2(canvas.x / 2.0 - 20.0 - size.x, below + 60.0), size);
                let next = Rect::from_min_size(pos2(canvas.x / 2.0 + 20.0, below + 60.0), size);
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(exit)
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    |ui| {
                        // Exit: a red door with an arrow leaving it, read in any language.
                        let r = glass_button(ui, size, "", 32.0, true);
                        let label = tr("home-quit");
                        let font = FontId::new(32.0, ui::bold());
                        let icon = 34.0;
                        let label_width = ui
                            .painter()
                            .layout_no_wrap(label.to_string(), font.clone(), palette::text())
                            .size()
                            .x;
                        let left = r.rect.center().x - (icon + 16.0 + label_width) / 2.0;
                        exit_icon(
                            ui.painter(),
                            Rect::from_min_size(
                                pos2(left, r.rect.center().y - icon / 2.0),
                                vec2(icon, icon),
                            ),
                        );
                        ui.painter().text(
                            pos2(left + icon + 16.0, r.rect.center().y),
                            Align2::LEFT_CENTER,
                            label,
                            font,
                            palette::text(),
                        );
                        self.widgets.push(("pse:exit".into(), r.rect));
                        if r.clicked() {
                            choice = Some(Choice::Quit);
                        }
                    },
                );
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(next)
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    |ui| {
                        let r = glass_button(ui, size, tr("home-pse_continue"), 32.0, true);
                        self.widgets.push(("pse:continue".into(), r.rect));
                        if r.clicked() {
                            self.pse = false;
                        }
                    },
                );
            });
        choice
    }

    /// A column: its heading, then `add` in an area clipped to the column.
    pub(super) fn column_area(
        &mut self,
        ctx: &egui::Context,
        id: &str,
        rect: Rect,
        heading: &str,
        add: impl FnOnce(&mut Self, &mut egui::Ui),
    ) {
        let inner = rect.shrink2(vec2(22.0, 18.0));
        // No fade-in (a snapshot shows the page as it is) and no shifting to fit the screen.
        egui::Area::new(egui::Id::unique(id))
            .fixed_pos(inner.min)
            .fade_in(false)
            .constrain(false)
            .show(ctx, |ui| {
                ui.set_width(inner.width());
                ui.set_max_width(inner.width());
                ui.set_clip_rect(inner);
                ui.label(
                    RichText::new(heading)
                        .font(FontId::new(24.0, ui::bold()))
                        .color(palette::accent()),
                );
                ui.add_space(8.0);
                // What does not fit scrolls, so none of it is out of reach.
                let height = inner.max.y - ui.cursor().top();
                egui::ScrollArea::vertical()
                    .id_salt(id)
                    .max_height(height)
                    .min_scrolled_height(height)
                    .auto_shrink([false, true])
                    .show(ui, |ui| add(self, ui));
            });
    }
}
