//! The homepage's Config page: the settings by category, Save and Apply, Discard, and Restore Defaults; while
//! settings are tried, a popup counts down over it.

use super::*;

/// A setting's heading.
fn heading(ui: &mut egui::Ui, text: &str) {
    ui.label(
        RichText::new(text)
            .font(FontId::new(24.0, ui::bold()))
            .color(palette::accent()),
    );
    ui.add_space(6.0);
}

/// What a setting does, under it.
fn hint(ui: &mut egui::Ui, text: &str) {
    ui.add(egui::Label::new(RichText::new(text).color(palette::text_dim())).wrap());
    ui.add_space(24.0);
}

/// What the Config page's settings are edited with: the settings, the page's controls, whether a trial disables
/// them, and the Language option's outline's strength while it is outlined.
struct Editing<'a> {
    settings: &'a mut Settings,
    widgets: &'a mut Vec<(String, Rect)>,
    trying: bool,
    flash: Option<f32>,
}

impl Home {
    /// The Config page: the settings, Save and Apply, Discard, and Restore
    /// Defaults; while settings are tried, a popup counts down over it.
    pub(super) fn draw_config(
        &mut self,
        ctx: &egui::Context,
        ink: &ui::Ink,
        canvas: Vec2,
        time: f32,
    ) {
        if self
            .settings
            .trial
            .as_ref()
            .is_some_and(|trial| time - trial.started >= TRIAL_SECONDS)
        {
            self.restore_settings();
        }
        let trying = self.settings.trial.is_some();
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
            tr("home-config"),
            38.0,
            ui::bold(),
            palette::text(),
        );
        // The card's top; it grows to fit what it holds (the path at its foot may wrap).
        let card = Rect::from_min_size(
            pos2(canvas.x / 2.0 - 620.0, canvas.y * 0.53 - 410.0),
            vec2(1240.0, 820.0),
        );
        let inner = card.shrink2(vec2(48.0, 34.0));
        let flash = self
            .settings
            .flash
            .map(|start| 1.0 - (time - start) / FLASH_SECONDS)
            .filter(|a| *a > 0.0);
        let area = egui::Area::new(egui::Id::unique("home-config"))
            .fixed_pos(inner.min)
            .fade_in(false)
            .constrain(false)
            .show(ctx, |ui| {
                ui.set_width(inner.width());
                // A checkbox as large as the text beside it.
                let spacing = ui.spacing_mut();
                spacing.icon_width = 30.0;
                spacing.icon_width_inner = 18.0;
                spacing.icon_spacing = 14.0;
                ui.add_enabled_ui(!trying, |ui| {
                    let Home {
                        settings, widgets, ..
                    } = self;
                    let mut editing = Editing {
                        settings,
                        widgets,
                        trying,
                        flash,
                    };
                    editing.settings_card(ui, inner.width());
                    self.config_buttons(ui, inner.width(), trying, time);
                });
            });
        // The background layer is drawn under every area, so the card can follow its content.
        glass_panel(
            ink.painter,
            Rect::from_min_max(card.min, pos2(card.max.x, area.response.rect.max.y + 34.0)),
            24,
            true,
        );
        let footer = if trying {
            tr("home-esc_restores")
        } else {
            tr("home-esc_discards")
        };
        ink.text(
            pos2(40.0, canvas.y - 58.0),
            Align2::LEFT_TOP,
            footer,
            24.0,
            FontFamily::Proportional,
            palette::text_faint(),
        );

        // The countdown popup, above everything else on the page.
        self.trial_popup(ctx, canvas, time);
    }

    /// Save and Apply (or Return), Discard, and Restore Defaults, `width` wide, and what the page says under them.
    fn config_buttons(&mut self, ui: &mut egui::Ui, width: f32, trying: bool, time: f32) {
        ui.add_space(12.0);
        let changed = self.settings.pending != self.settings.saved;
        let [first, discard, reset] = ui
            .horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 18.0;
                let size = vec2((width - 36.0) / 3.0, 72.0);
                // Save and Apply only when something changed; otherwise Return.
                let first = if changed {
                    ("save", tr("home-save_and_apply"))
                } else {
                    ("return", tr("home-return"))
                };
                [
                    first,
                    ("discard", tr("home-discard")),
                    ("defaults", tr("home-restore_defaults")),
                ]
                .map(|(name, label)| {
                    let r = glass_button(ui, size, label, 30.0, !trying);
                    self.widgets.push((name.into(), r.rect));
                    r.clicked()
                })
            })
            .inner;
        if first && changed {
            self.try_settings(time);
        }
        if first && !changed {
            self.page = Page::Title;
            self.settings.message = None;
            self.settings.flash = None;
        }
        if discard {
            self.discard_settings();
        }
        if reset {
            self.settings.pending = Config::default();
            self.settings.message = Some(tr("home-defaults_shown").to_string());
        }
        ui.add_space(24.0);
        if self.settings.pending != self.settings.saved {
            ui.label(RichText::new(tr("home-unsaved_changes")).color(palette::accent()));
        }
        if let Some(message) = &self.settings.message {
            ui.add(egui::Label::new(RichText::new(message).color(palette::text_dim())).wrap());
        }
        let stored = match &self.settings.path {
            Some(path) => format!("{} {}", tr("home-stored_in"), path.display()),
            None => tr("home-no_config_dir").to_string(),
        };
        ui.add(egui::Label::new(RichText::new(stored).color(palette::text_faint())).wrap());
    }

    /// The countdown popup over a trial of settings: Apply keeps them, Restore puts the previous ones back.
    fn trial_popup(&mut self, ctx: &egui::Context, canvas: Vec2, time: f32) {
        let Some(started) = self.settings.trial.as_ref().map(|t| t.started) else {
            return;
        };
        let left = (TRIAL_SECONDS - (time - started)).ceil().max(1.0) as u32;
        egui::Area::new(egui::Id::unique("home-config-popup"))
            .order(egui::Order::Foreground)
            .fixed_pos(Pos2::ZERO)
            .fade_in(false)
            .constrain(false)
            .show(ctx, |ui| {
                let painter = ui.painter();
                painter.rect_filled(
                    Rect::from_min_size(Pos2::ZERO, canvas),
                    CornerRadius::ZERO,
                    palette::theme().dim,
                );
                let popup = Rect::from_center_size(
                    pos2(canvas.x / 2.0, canvas.y * 0.5),
                    vec2(820.0, 360.0),
                );
                painter.rect_filled(popup, CornerRadius::same(22), palette::theme().popup);
                painter.rect_stroke(
                    popup,
                    CornerRadius::same(22),
                    Stroke::new(2.0, palette::accent()),
                    StrokeKind::Inside,
                );
                painter.text(
                    pos2(popup.center().x, popup.min.y + 44.0),
                    Align2::CENTER_TOP,
                    tr("home-keep_these_settings"),
                    FontId::new(38.0, ui::bold()),
                    palette::text(),
                );
                let countdown = tr_with("home-countdown", args!(seconds = left));
                painter.text(
                    pos2(popup.center().x, popup.min.y + 110.0),
                    Align2::CENTER_TOP,
                    countdown,
                    FontId::new(28.0, FontFamily::Proportional),
                    palette::text_dim(),
                );
                let size = vec2(300.0, 72.0);
                let row = Rect::from_center_size(
                    pos2(popup.center().x, popup.max.y - 80.0),
                    vec2(size.x * 2.0 + 24.0, size.y),
                );
                let mut keep = false;
                let mut restore = false;
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(row)
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    |ui| {
                        ui.spacing_mut().item_spacing.x = 24.0;
                        let apply = glass_button(ui, size, tr("home-apply"), 32.0, true);
                        self.widgets.push(("trial:apply".into(), apply.rect));
                        keep = apply.clicked();
                        let back = glass_button(ui, size, tr("home-restore"), 32.0, true);
                        self.widgets.push(("trial:restore".into(), back.rect));
                        restore = back.clicked();
                    },
                );
                if keep {
                    self.keep_settings();
                } else if restore {
                    self.restore_settings();
                }
            });
    }
}

impl Editing<'_> {
    /// A tab per category, on the left (a tab whose settings were edited is marked); the category's settings
    /// beside them, in a card `width` wide.
    fn settings_card(&mut self, ui: &mut egui::Ui, width: f32) {
        let tabs = 250.0;
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 36.0;
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 14.0;
                let Editing {
                    settings,
                    widgets,
                    trying,
                    ..
                } = self;
                let trying = *trying;
                for category in Category::ALL {
                    let edited = settings.pending.differs_in(&settings.saved, category);
                    let r = tab_button(
                        ui,
                        vec2(tabs, 64.0),
                        category_name(category),
                        settings.category == category,
                        edited,
                        !trying,
                    );
                    widgets.push((format!("category:{}", category.key()), r.rect));
                    if r.clicked() {
                        settings.category = category;
                    }
                }
            });
            ui.vertical(|ui| {
                ui.set_width(width - tabs - 36.0);
                ui.set_min_height(420.0);
                match self.settings.category {
                    Category::Interface => {
                        self.language(ui);
                        self.listing_style(ui);
                        self.theme(ui);
                    }
                    Category::Render => self.render(ui),
                    Category::Audio => self.audio(ui),
                }
            });
        });
    }

    /// The language, outlined while `flash` is on.
    fn language(&mut self, ui: &mut egui::Ui) {
        let defaults = Config::default();
        let Editing {
            settings,
            widgets,
            trying,
            flash,
        } = self;
        let (trying, flash) = (*trying, *flash);
        let pending = &mut settings.pending;
        heading(ui, tr("home-language"));
        let (row, restore) = option_row(
            ui,
            widgets,
            "language",
            pending.language != defaults.language,
            defaults.language.name(),
            !trying,
            |ui, widgets| {
                ui.spacing_mut().item_spacing.x = 12.0;
                for locale in Locale::offered() {
                    let r = ui.add(
                        egui::Button::selectable(
                            pending.language == locale,
                            RichText::new(locale.name()).font(FontId::new(30.0, ui::bold())),
                        )
                        .min_size(vec2(180.0, 48.0)),
                    );
                    widgets.push((format!("option:language:{}", locale.code()), r.rect));
                    if r.clicked() {
                        pending.language = locale;
                    }
                }
            },
        );
        if restore {
            pending.language = defaults.language;
        }
        if let Some(alpha) = flash {
            ui.painter().rect_stroke(
                row.expand2(vec2(28.0, 8.0)),
                CornerRadius::same(12),
                Stroke::new(3.0, palette::accent().gamma_multiply(alpha)),
                StrokeKind::Outside,
            );
        }
        hint(ui, tr("home-language_hint"));
    }

    /// The listing style: text or blocks.
    fn listing_style(&mut self, ui: &mut egui::Ui) {
        let defaults = Config::default();
        let trying = self.trying;
        let widgets = &mut *self.widgets;
        let pending = &mut self.settings.pending;
        heading(ui, tr("home-listing_style"));
        let style_name = |style: crate::pseudocode::Style| match style {
            crate::pseudocode::Style::Text => tr("home-listing_style_text"),
            crate::pseudocode::Style::Blocks => tr("home-listing_style_blocks"),
        };
        let (_, restore) = option_row(
            ui,
            widgets,
            "listing_style",
            pending.listing_style != defaults.listing_style,
            style_name(defaults.listing_style),
            !trying,
            |ui, widgets| {
                ui.spacing_mut().item_spacing.x = 12.0;
                for style in [
                    crate::pseudocode::Style::Blocks,
                    crate::pseudocode::Style::Text,
                ] {
                    let r = ui.add(
                        egui::Button::selectable(
                            pending.listing_style == style,
                            RichText::new(style_name(style)).font(FontId::new(30.0, ui::bold())),
                        )
                        .min_size(vec2(180.0, 48.0)),
                    );
                    widgets.push((format!("option:listing_style:{}", style.key()), r.rect));
                    if r.clicked() {
                        pending.listing_style = style;
                    }
                }
            },
        );
        if restore {
            pending.listing_style = defaults.listing_style;
        }
        hint(ui, tr("home-listing_style_hint"));
    }

    /// The theme: the themes beside the binary, the folders they are read from, and the errors of those that
    /// did not read.
    fn theme(&mut self, ui: &mut egui::Ui) {
        let defaults = Config::default();
        let trying = self.trying;
        let widgets = &mut *self.widgets;
        let settings = &mut *self.settings;
        let pending = &mut settings.pending;
        heading(ui, tr("home-theme"));
        let themes = &settings.themes;
        let name = |id: &str| {
            themes
                .iter()
                .find(|t| t.id == id)
                .map_or(id.to_string(), |t| t.name.clone())
        };
        let (_, restore) = option_row(
            ui,
            widgets,
            "theme",
            pending.theme != defaults.theme,
            &name(&defaults.theme),
            !trying,
            |ui, widgets| {
                // A list, one to a row, so the default and Restore fit beside it.
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 8.0;
                    for entry in themes {
                        let r = ui.add(
                            egui::Button::selectable(
                                pending.theme == entry.id,
                                RichText::new(&entry.name).font(FontId::new(26.0, ui::bold())),
                            )
                            .min_size(vec2(280.0, 44.0)),
                        );
                        widgets.push((format!("option:theme:{}", entry.id), r.rect));
                        if r.clicked() {
                            pending.theme = entry.id.clone();
                        }
                    }
                });
            },
        );
        if restore {
            pending.theme = defaults.theme.clone();
        }
        ui.add(
            egui::Label::new(RichText::new(tr("home-theme_hint")).color(palette::text_dim()))
                .wrap(),
        );
        let folders = theme::dirs()
            .iter()
            .map(|d| d.display().to_string())
            .collect::<Vec<_>>()
            .join("\n");
        ui.add(egui::Label::new(RichText::new(folders).color(palette::text_faint())).wrap());
        ui.add_space(24.0);
        for error in &settings.theme_errors {
            ui.add(egui::Label::new(RichText::new(error).color(palette::text_faint())).wrap());
        }
    }

    /// The Render tab: full screen.
    fn render(&mut self, ui: &mut egui::Ui) {
        let defaults = Config::default();
        let trying = self.trying;
        let widgets = &mut *self.widgets;
        let pending = &mut self.settings.pending;
        heading(ui, tr("home-display"));
        let (_, restore) = option_row(
            ui,
            widgets,
            "fullscreen",
            pending.fullscreen != defaults.fullscreen,
            on_off(defaults.fullscreen),
            !trying,
            |ui, widgets| {
                let r = ui.checkbox(
                    &mut pending.fullscreen,
                    RichText::new(tr("home-full_screen")).font(FontId::new(30.0, ui::bold())),
                );
                widgets.push(("option:fullscreen".into(), r.rect));
            },
        );
        if restore {
            pending.fullscreen = defaults.fullscreen;
        }
        hint(ui, tr("home-full_screen_hint"));
        hint(ui, tr("home-render_hint"));
    }

    /// The Audio tab: the music's and the narration's volumes.
    fn audio(&mut self, ui: &mut egui::Ui) {
        let defaults = Config::default();
        let trying = self.trying;
        let widgets = &mut *self.widgets;
        let pending = &mut self.settings.pending;
        heading(ui, tr("home-volume"));
        for (name, label, volume, default) in [
            (
                "music",
                tr("home-music"),
                &mut pending.music,
                defaults.music,
            ),
            (
                "narration",
                tr("home-narration"),
                &mut pending.narration,
                defaults.narration,
            ),
        ] {
            let (_, restore) = option_row(
                ui,
                widgets,
                name,
                *volume != default,
                &format!("{} %", default.percent),
                !trying,
                |ui, widgets| {
                    let (rect, _) = ui.allocate_exact_size(vec2(240.0, 48.0), Sense::hover());
                    ui.painter().text(
                        rect.left_center(),
                        Align2::LEFT_CENTER,
                        label,
                        FontId::new(30.0, ui::bold()),
                        palette::text(),
                    );
                    let r = volume::slider(ui, volume, 40.0, 360.0);
                    widgets.push((format!("option:{name}"), r.rail));
                    widgets.push((format!("mute:{name}"), r.mute));
                },
            );
            if restore {
                *volume = default;
            }
        }
        hint(ui, tr("home-audio_hint"));
    }
}
