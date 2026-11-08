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

    /// The selection page: engines, the chosen engine's features, and the
    /// chosen feature's timelines and demonstrations, on glass panels.
    pub(super) fn draw_select(
        &mut self,
        ctx: &egui::Context,
        ink: &ui::Ink,
        canvas: Vec2,
    ) -> Option<Choice> {
        let mut choice = None;
        let chosen = self.selected();
        // The chrome's strings follow the chosen feature.
        investigation::set(chosen);
        let painter = ink.painter;

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
            tr("home-choose"),
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
                    self.page = Page::Title;
                }
            });
        let top = 130.0;
        let bottom = canvas.y - 90.0;
        let column = |x0: f32, x1: f32| Rect::from_min_max(pos2(x0, top), pos2(x1, bottom));
        let engines_rect = column(40.0, 420.0);
        let features_rect = column(444.0, 824.0);
        let detail_rect = column(848.0, canvas.x - 40.0);
        for r in [engines_rect, features_rect, detail_rect] {
            glass_panel(painter, r, 18, true);
        }

        // Engines.
        let engines = investigations::engines();
        let mut new_engine = None;
        self.column_area(
            ctx,
            "home-engines",
            engines_rect,
            tr("home-engine"),
            |home, ui| {
                for (i, engine) in engines.iter().enumerate() {
                    let r = ui.add(
                        egui::Button::selectable(
                            i == home.engine,
                            RichText::new(engine.name).font(FontId::new(30.0, ui::bold())),
                        )
                        .min_size(vec2(ui.available_width(), 56.0)),
                    );
                    home.widgets.push((format!("engine:{}", engine.id), r.rect));
                    if r.clicked() {
                        new_engine = Some(i);
                    }
                    if i == home.engine {
                        ui.add_space(4.0);
                        ui.add(
                            egui::Label::new(
                                RichText::new(engine.summary).color(palette::text_dim()),
                            )
                            .wrap(),
                        );
                        ui.add_space(12.0);
                    }
                }
            },
        );

        // Features of the chosen engine, with the version each was read from.
        let features = self.features();
        let mut new_feature = None;
        self.column_area(
            ctx,
            "home-features",
            features_rect,
            tr("home-feature"),
            |home, ui| {
                for (i, feature) in features.iter().enumerate() {
                    // An investigation, or a series (its title, and how many parts it tells).
                    let (title, widget, first) = match feature {
                        Feature::One(f) => {
                            (f.title().to_string(), format!("feature:{}", f.path()), *f)
                        }
                        Feature::Series(s) => (
                            s.title_in(home.applied.language.code()).to_string(),
                            format!("series:{}", s.key),
                            parts_of(s)[0],
                        ),
                    };
                    let r = ui.add(
                        egui::Button::selectable(
                            i == home.feature,
                            RichText::new(title).font(FontId::new(30.0, ui::bold())),
                        )
                        .min_size(vec2(ui.available_width(), 56.0)),
                    );
                    home.widgets.push((widget, r.rect));
                    if r.clicked() {
                        new_feature = Some(i);
                    }
                    if let Feature::Series(s) = feature {
                        ui.label(
                            RichText::new(tr_with(
                                "home-series_parts",
                                args!(count = s.parts.len()),
                            ))
                            .color(palette::text_dim()),
                        );
                    }
                    let engine = &first.sources().engine;
                    let commit = engine.commit.get(..8).unwrap_or(&engine.commit);
                    ui.label(
                        RichText::new(format!("{} {} · {commit}", engine.name, engine.version))
                            .color(palette::text_faint()),
                    );
                    ui.add_space(12.0);
                }
            },
        );

        // The chosen feature: what it covers, how to open it, its chapters and demonstrations.
        let rows = self.timeline_rows(chosen);
        let edition = self
            .edition
            .unwrap_or_else(|| preferred_edition(&rows, self.applied.language))
            .min(rows.len().saturating_sub(1));
        let mut new_edition = None;
        let mut new_part: Option<&'static dyn Investigation> = None;
        let error = self.error.clone();
        self.column_area(
            ctx,
            "home-detail",
            detail_rect,
            chosen.title(),
            |home, ui| {
                // For a part of a series, its parts first (engine, story, part, left to right); beside them, what it covers and its notices.
                let part = chosen.package().and_then(crate::resources::part_of);
                let full = ui.available_width();
                ui.horizontal_top(|ui| {
                    // A part of a series (src/packages.rs, `Series`): which, its parts in order, and whether to go on.
                    if let Some((series, index)) = part {
                        let language = home.applied.language.code();
                        // The story's parts, one under another, on darker glass.
                        let width = full * 0.4 - 24.0;
                        egui::Frame::new()
                            .fill(Color32::from_black_alpha(110))
                            .corner_radius(CornerRadius::same(12))
                            .inner_margin(egui::Margin::same(12))
                            .show(ui, |ui| {
                                ui.vertical(|ui| {
                                    ui.set_width(width);
                                    ui.set_max_width(width);
                                    ui.spacing_mut().item_spacing.y = 2.0;
                                    let heading = tr_with(
                                        "home-part_of",
                                        args!(
                                            number = index + 1,
                                            count = series.parts.len(),
                                            series = series.title_in(language)
                                        ),
                                    );
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(heading)
                                                .font(FontId::new(24.0, ui::bold()))
                                                .color(palette::accent()),
                                        )
                                        .wrap(),
                                    )
                                    .on_hover_text(series.summary_in(language));
                                    ui.add_space(4.0);
                                    for (k, investigation) in
                                        parts_of(series).into_iter().enumerate()
                                    {
                                        let label = RichText::new(format!(
                                            "{}  {}",
                                            k + 1,
                                            investigation.title()
                                        ))
                                        .font(FontId::new(24.0, ui::bold()));
                                        let r = ui.add(
                                            egui::Button::selectable(k == index, label)
                                                .min_size(vec2(width, 38.0)),
                                        );
                                        home.widgets.push((
                                            format!("part:{}", investigation.path()),
                                            r.rect,
                                        ));
                                        if r.clicked() {
                                            new_part = Some(investigation);
                                        }
                                    }
                                })
                            });
                        ui.add_space(12.0);
                    }
                    ui.vertical(|ui| {
                        ui.set_width(if part.is_some() {
                            full * 0.6 - 12.0
                        } else {
                            full
                        });
                        ui.add(
                            egui::Label::new(
                                RichText::new(chosen.summary()).color(palette::text()),
                            )
                            .wrap(),
                        );
                        // Its package's legal notices (`[[notice]]`): its engine's copyright, license, and trademarks.
                        for (text, _) in
                            crate::resources::notices(chosen.package().unwrap_or("render_lab"))
                        {
                            ui.add_space(6.0);
                            ui.add(
                                egui::Label::new(
                                    RichText::new(text).size(24.0).color(palette::text_faint()),
                                )
                                .wrap(),
                            );
                        }
                    });
                });
                ui.add_space(14.0);
                if let Some(e) = &error {
                    ui.add(
                        egui::Label::new(RichText::new(e).color(palette::HISTORY_REJECT)).wrap(),
                    );
                    ui.add_space(10.0);
                }
                // Rows of two: watch and explore, then the features' buttons (src/feature/: experiment, the quiz).
                let first = rows.get(edition).filter(|r| r.summary.is_ok());
                // Short rows, so the chapter list below keeps its room.
                let button = vec2((ui.available_width() - 24.0) / 2.0, 46.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 16.0;
                    let watch = glass_button(
                        ui,
                        button,
                        tr("home-watch_the_narrated_video"),
                        26.0,
                        first.is_some(),
                    );
                    home.widgets.push(("watch".into(), watch.rect));
                    if let (true, Some(row)) = (watch.clicked(), first) {
                        choice = Some(Choice::Watch {
                            investigation: chosen,
                            timeline: row.info,
                            start_frame: 0,
                        });
                    }
                    let explore = glass_button(
                        ui,
                        button,
                        tr("home-explore_the_demonstrations"),
                        26.0,
                        true,
                    );
                    home.widgets.push(("explore".into(), explore.rect));
                    if explore.clicked() {
                        choice = Some(Choice::Explore {
                            investigation: chosen,
                        });
                    }
                });
                let buttons: Vec<(&'static dyn crate::feature::Feature, String)> =
                    crate::feature::all()
                        .into_iter()
                        .filter_map(|f| f.home().map(|label| (f, label)))
                        .collect();
                for row in buttons.chunks(2) {
                    ui.add_space(2.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 16.0;
                        for (feature, label) in row {
                            let pressed =
                                glass_button(ui, button, label, 26.0, feature.available(chosen));
                            home.widgets.push((feature.id().into(), pressed.rect));
                            if pressed.clicked() {
                                choice = Some(Choice::Feature {
                                    investigation: chosen,
                                    feature: *feature,
                                });
                            }
                        }
                    });
                }
                // What the story assumes (src/knowledge.rs): the level it starts from, the known
                // topics it uses directly, and the graph full screen.
                if let Some(graph) = crate::knowledge::current() {
                    let p = graph.audience();
                    // The first few of a list; the knowledge graph has the rest.
                    const SHOWN: usize = 8;
                    let names = |list: &[usize]| {
                        let mut text = list
                            .iter()
                            .take(SHOWN)
                            .map(|&i| graph.nodes[i].title.as_str())
                            .collect::<Vec<_>>()
                            .join(tr("knowledge-separator"));
                        if list.len() > SHOWN {
                            text += &tr_with(
                                "home-prerequisites_more",
                                args!(count = list.len() - SHOWN),
                            );
                        }
                        text
                    };
                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(tr("home-prerequisites"))
                                .font(FontId::new(24.0, ui::bold()))
                                .color(palette::accent()),
                        );
                        let open = ui
                            .button(RichText::new(tr("home-knowledge_graph")).size(24.0))
                            .on_hover_text(tr("home-knowledge_graph_hint"));
                        home.widgets.push(("knowledge".into(), open.rect));
                        if open.clicked() {
                            home.knowledge = true;
                        }
                    });
                    ui.add(
                        egui::Label::new(
                            RichText::new(tr_with(
                                "home-prerequisites_level",
                                args!(topics = names(&p.level)),
                            ))
                            .color(palette::text()),
                        )
                        .wrap(),
                    );
                    let known = p.known.iter().filter(|k| **k).count();
                    ui.add(
                        egui::Label::new(
                            RichText::new(tr_with(
                                "home-prerequisites_known",
                                args!(count = known, topics = names(&p.carried())),
                            ))
                            .color(palette::text_dim()),
                        )
                        .wrap(),
                    );
                }
                ui.add_space(18.0);
                let width = ui.available_width();
                ui.horizontal_top(|ui| {
                    // The narrated timelines and their chapters.
                    ui.vertical(|ui| {
                        ui.set_width(width * 0.58);
                        // With several editions, a picker; the chapters and Watch follow it.
                        if rows.len() > 1 {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(tr("home-edition")).color(palette::text_dim()),
                                );
                                for (i, row) in rows.iter().enumerate() {
                                    // An edition is named by its language (the registry's name), once its timeline is read.
                                    let name =
                                        row.locale.map_or(row.info.id.as_str(), Locale::name);
                                    let r = ui.selectable_label(
                                        i == edition,
                                        RichText::new(name).font(FontId::new(24.0, ui::bold())),
                                    );
                                    home.widgets
                                        .push((format!("edition:{}", row.info.id), r.rect));
                                    if r.clicked() {
                                        new_edition = Some(i);
                                    }
                                }
                            });
                            ui.add_space(6.0);
                        }
                        if let Some(row) = rows.get(edition) {
                            match &row.summary {
                                Ok((title, length, chapters, narrated)) => {
                                    ui.label(
                                        RichText::new(format!("{title} · {length}"))
                                            .font(FontId::new(24.0, ui::bold()))
                                            .color(palette::accent()),
                                    );
                                    if !narrated {
                                        ui.label(
                                            RichText::new(tr("home-no_narration"))
                                                .color(palette::text_faint()),
                                        );
                                    }
                                    ui.label(
                                        RichText::new(tr("home-start_from_a_chapter"))
                                            .color(palette::text_dim()),
                                    );
                                    for (number, label, frame) in chapters {
                                        let r = ui.add(
                                            egui::Button::new(RichText::new(format!(
                                                "{number:02}  {label}"
                                            )))
                                            .frame(false)
                                            .truncate(),
                                        );
                                        home.widgets.push((format!("chapter:{number}"), r.rect));
                                        if r.clicked() {
                                            choice = Some(Choice::Watch {
                                                investigation: chosen,
                                                timeline: row.info,
                                                start_frame: *frame,
                                            });
                                        }
                                    }
                                }
                                Err(e) => {
                                    ui.label(
                                        RichText::new(&row.info.title)
                                            .font(FontId::new(24.0, ui::bold()))
                                            .color(palette::accent()),
                                    );
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(format!(
                                                "{} {e}",
                                                tr("home-not_built_yet")
                                            ))
                                            .color(palette::text_faint()),
                                        )
                                        .wrap(),
                                    );
                                }
                            }
                            ui.add_space(10.0);
                        }
                    });
                    // The demonstrations, with their keys in the viewer.
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(tr("home-demonstrations"))
                                .font(FontId::new(24.0, ui::bold()))
                                .color(palette::accent()),
                        );
                        const KEYS: [&str; 12] =
                            ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "−", "="];
                        for (i, demo) in chosen.demos().iter().enumerate() {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(format!(
                                        "{}  {}",
                                        KEYS.get(i).unwrap_or(&" "),
                                        chosen.demo_title(*demo)
                                    ))
                                    .color(palette::text_dim()),
                                )
                                .truncate(),
                            );
                        }
                    });
                });
            },
        );
        let footer = tr("home-esc_hint");
        ink.text(
            pos2(40.0, canvas.y - 58.0),
            Align2::LEFT_TOP,
            footer,
            24.0,
            FontFamily::Proportional,
            palette::text_faint(),
        );

        if let Some(e) = new_engine {
            self.engine = e;
            self.feature = 0;
            self.part = 0;
            self.edition = None;
        }
        if let Some(f) = new_feature {
            if f != self.feature {
                self.part = 0;
            }
            self.feature = f;
            self.edition = None;
        }
        if let Some(e) = new_edition {
            self.edition = Some(e);
        }
        if let Some(part) = new_part {
            self.select(part);
        }
        choice
    }

    /// Select `investigation` on the selection page (its engine and feature).
    pub(super) fn select(&mut self, investigation: &'static dyn Investigation) {
        self.engine = investigations::engines()
            .iter()
            .position(|e| e.id == investigation.engine().id)
            .unwrap_or(0);
        let path = investigation.path();
        let features = self.features();
        let (feature, part) = features
            .iter()
            .enumerate()
            .find_map(|(n, f)| match f {
                Feature::One(i) => (i.path() == path).then_some((n, 0)),
                Feature::Series(s) => parts_of(s)
                    .iter()
                    .position(|p| p.path() == path)
                    .map(|k| (n, k)),
            })
            .unwrap_or((0, 0));
        (self.feature, self.part) = (feature, part);
        self.edition = None;
    }

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
                    let defaults = Config::default();
                    let Home {
                        settings, widgets, ..
                    } = self;
                    let heading = |ui: &mut egui::Ui, text: &str| {
                        ui.label(
                            RichText::new(text)
                                .font(FontId::new(24.0, ui::bold()))
                                .color(palette::accent()),
                        );
                        ui.add_space(6.0);
                    };
                    let hint = |ui: &mut egui::Ui, text: &str| {
                        ui.add(
                            egui::Label::new(RichText::new(text).color(palette::text_dim())).wrap(),
                        );
                        ui.add_space(24.0);
                    };

                    // A tab per category, on the left; a tab whose settings were edited is marked.
                    let tabs = 250.0;
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = 36.0;
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 14.0;
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
                            ui.set_width(inner.width() - tabs - 36.0);
                            ui.set_min_height(420.0);
                            let pending = &mut settings.pending;
                            match settings.category {
                                Category::Interface => {
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
                                                        RichText::new(locale.name())
                                                            .font(FontId::new(30.0, ui::bold())),
                                                    )
                                                    .min_size(vec2(180.0, 48.0)),
                                                );
                                                widgets.push((
                                                    format!("option:language:{}", locale.code()),
                                                    r.rect,
                                                ));
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
                                            Stroke::new(
                                                3.0,
                                                palette::accent().gamma_multiply(alpha),
                                            ),
                                            StrokeKind::Outside,
                                        );
                                    }
                                    hint(ui, tr("home-language_hint"));

                                    heading(ui, tr("home-listing_style"));
                                    let style_name = |style: crate::pseudocode::Style| match style {
                                        crate::pseudocode::Style::Text => {
                                            tr("home-listing_style_text")
                                        }
                                        crate::pseudocode::Style::Blocks => {
                                            tr("home-listing_style_blocks")
                                        }
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
                                                        RichText::new(style_name(style))
                                                            .font(FontId::new(30.0, ui::bold())),
                                                    )
                                                    .min_size(vec2(180.0, 48.0)),
                                                );
                                                widgets.push((
                                                    format!("option:listing_style:{}", style.key()),
                                                    r.rect,
                                                ));
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
                                                            RichText::new(&entry.name).font(
                                                                FontId::new(26.0, ui::bold()),
                                                            ),
                                                        )
                                                        .min_size(vec2(280.0, 44.0)),
                                                    );
                                                    widgets.push((
                                                        format!("option:theme:{}", entry.id),
                                                        r.rect,
                                                    ));
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
                                        egui::Label::new(
                                            RichText::new(tr("home-theme_hint"))
                                                .color(palette::text_dim()),
                                        )
                                        .wrap(),
                                    );
                                    let folders = theme::dirs()
                                        .iter()
                                        .map(|d| d.display().to_string())
                                        .collect::<Vec<_>>()
                                        .join("\n");
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(folders).color(palette::text_faint()),
                                        )
                                        .wrap(),
                                    );
                                    ui.add_space(24.0);
                                    for error in &settings.theme_errors {
                                        ui.add(
                                            egui::Label::new(
                                                RichText::new(error).color(palette::text_faint()),
                                            )
                                            .wrap(),
                                        );
                                    }
                                }
                                Category::Render => {
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
                                                RichText::new(tr("home-full_screen"))
                                                    .font(FontId::new(30.0, ui::bold())),
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
                                Category::Audio => {
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
                                                let (rect, _) = ui.allocate_exact_size(
                                                    vec2(240.0, 48.0),
                                                    Sense::hover(),
                                                );
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
                        });
                    });

                    ui.add_space(12.0);
                    let changed = settings.pending != settings.saved;
                    let [first, discard, reset] = ui
                        .horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 18.0;
                            let size = vec2((inner.width() - 36.0) / 3.0, 72.0);
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
                                widgets.push((name.into(), r.rect));
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
                        ui.label(
                            RichText::new(tr("home-unsaved_changes")).color(palette::accent()),
                        );
                    }
                    if let Some(message) = &self.settings.message {
                        ui.add(
                            egui::Label::new(RichText::new(message).color(palette::text_dim()))
                                .wrap(),
                        );
                    }
                    let stored = match &self.settings.path {
                        Some(path) => format!("{} {}", tr("home-stored_in"), path.display()),
                        None => tr("home-no_config_dir").to_string(),
                    };
                    ui.add(
                        egui::Label::new(RichText::new(stored).color(palette::text_faint())).wrap(),
                    );
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
