//! The homepage's selection page: engines, the chosen engine's features, and the chosen feature's timelines and
//! demonstrations, on glass panels.

use super::*;
use crate::investigation::Engine;

/// What the selection page's detail column chose: a choice to open, an edition, or another part of the series.
#[derive(Default)]
struct DetailPicks {
    choice: Option<Choice>,
    edition: Option<usize>,
    part: Option<&'static dyn Investigation>,
}

impl Home {
    /// The selection page: engines, the chosen engine's features, and the
    /// chosen feature's timelines and demonstrations, on glass panels.
    pub(super) fn draw_select(
        &mut self,
        ctx: &egui::Context,
        ink: &ui::Ink,
        canvas: Vec2,
    ) -> Option<Choice> {
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
            |home, ui| new_engine = home.engines_column(ui, &engines),
        );

        // Features of the chosen engine, with the version each was read from.
        let features = self.features();
        let mut new_feature = None;
        self.column_area(
            ctx,
            "home-features",
            features_rect,
            tr("home-feature"),
            |home, ui| new_feature = home.features_column(ui, &features),
        );

        // The chosen feature: what it covers, how to open it, its chapters and demonstrations.
        let rows = self.timeline_rows(chosen);
        let edition = self
            .edition
            .unwrap_or_else(|| preferred_edition(&rows, self.applied.language))
            .min(rows.len().saturating_sub(1));
        let mut picks = DetailPicks::default();
        self.column_area(
            ctx,
            "home-detail",
            detail_rect,
            chosen.title(),
            |home, ui| picks = home.detail_column(ui, chosen, &rows, edition),
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
        if let Some(e) = picks.edition {
            self.edition = Some(e);
        }
        if let Some(part) = picks.part {
            self.select(part);
        }
        picks.choice
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

    /// The engines, the chosen one's summary under it: the engine clicked.
    fn engines_column(&mut self, ui: &mut egui::Ui, engines: &[&'static Engine]) -> Option<usize> {
        let mut new_engine = None;
        for (i, engine) in engines.iter().enumerate() {
            let r = ui.add(
                egui::Button::selectable(
                    i == self.engine,
                    RichText::new(engine.name).font(FontId::new(30.0, ui::bold())),
                )
                .min_size(vec2(ui.available_width(), 56.0)),
            );
            self.widgets.push((format!("engine:{}", engine.id), r.rect));
            if r.clicked() {
                new_engine = Some(i);
            }
            if i == self.engine {
                ui.add_space(4.0);
                ui.add(
                    egui::Label::new(RichText::new(engine.summary).color(palette::text_dim()))
                        .wrap(),
                );
                ui.add_space(12.0);
            }
        }
        new_engine
    }

    /// The chosen engine's features (an investigation, or a series), each with the version it was read from: the
    /// feature clicked.
    fn features_column(&mut self, ui: &mut egui::Ui, features: &[Feature]) -> Option<usize> {
        let mut new_feature = None;
        for (i, feature) in features.iter().enumerate() {
            // An investigation, or a series (its title, and how many parts it tells).
            let (title, widget, first) = match feature {
                Feature::One(f) => (f.title().to_string(), format!("feature:{}", f.path()), *f),
                Feature::Series(s) => (
                    s.title_in(self.applied.language.code()).to_string(),
                    format!("series:{}", s.key),
                    parts_of(s)[0],
                ),
            };
            let r = ui.add(
                egui::Button::selectable(
                    i == self.feature,
                    RichText::new(title).font(FontId::new(30.0, ui::bold())),
                )
                .min_size(vec2(ui.available_width(), 56.0)),
            );
            self.widgets.push((widget, r.rect));
            if r.clicked() {
                new_feature = Some(i);
            }
            if let Feature::Series(s) = feature {
                ui.label(
                    RichText::new(tr_with("home-series_parts", args!(count = s.parts.len())))
                        .color(palette::text_dim()),
                );
            }
            ui.label(RichText::new(first.sources().edition()).color(palette::text_faint()));
            ui.add_space(12.0);
        }
        new_feature
    }

    /// The chosen feature's column: its series' parts and what it covers, how to open it, what it assumes, its
    /// timelines and chapters, and its demonstrations.
    fn detail_column(
        &mut self,
        ui: &mut egui::Ui,
        chosen: &'static dyn Investigation,
        rows: &[TimelineRow],
        edition: usize,
    ) -> DetailPicks {
        let mut picks = DetailPicks {
            part: self.detail_header(ui, chosen),
            ..DetailPicks::default()
        };
        ui.add_space(14.0);
        let error = self.error.clone();
        if let Some(e) = &error {
            ui.add(egui::Label::new(RichText::new(e).color(palette::HISTORY_REJECT)).wrap());
            ui.add_space(10.0);
        }
        picks.choice = self.open_buttons(ui, chosen, rows.get(edition));
        self.prerequisites(ui);
        ui.add_space(18.0);
        let width = ui.available_width();
        ui.horizontal_top(|ui| {
            // The narrated timelines and their chapters.
            ui.vertical(|ui| {
                ui.set_width(width * 0.58);
                // With several editions, a picker; the chapters and Watch follow it.
                if rows.len() > 1 {
                    picks.edition = self.edition_picker(ui, rows, edition);
                }
                if let Some(row) = rows.get(edition)
                    && let Some(choice) = self.timeline(ui, chosen, row)
                {
                    picks.choice = Some(choice);
                }
            });
            // The demonstrations, with their keys in the viewer.
            ui.vertical(|ui| demonstrations(ui, chosen));
        });
        picks
    }

    /// For a part of a series, its parts first (engine, story, part, left to right); beside them, what it covers
    /// and its notices: the part clicked.
    fn detail_header(
        &mut self,
        ui: &mut egui::Ui,
        chosen: &'static dyn Investigation,
    ) -> Option<&'static dyn Investigation> {
        let part = chosen.package().and_then(crate::resources::part_of);
        let full = ui.available_width();
        let mut picked = None;
        ui.horizontal_top(|ui| {
            // A part of a series (src/packages.rs, `Series`): which, its parts in order, and whether to go on.
            if let Some((series, index)) = part {
                picked = self.series_parts(ui, series, index, full * 0.4 - 24.0);
                ui.add_space(12.0);
            }
            ui.vertical(|ui| {
                ui.set_width(if part.is_some() {
                    full * 0.6 - 12.0
                } else {
                    full
                });
                ui.add(
                    egui::Label::new(RichText::new(chosen.summary()).color(palette::text())).wrap(),
                );
                // Its package's legal notices (`[[notice]]`): its engine's copyright, license, and trademarks.
                for (text, _) in crate::resources::notices(chosen.package().unwrap_or("render_lab"))
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
        picked
    }

    /// The story's parts, one under another, on darker glass, `width` wide, part `index` chosen: the part clicked.
    fn series_parts(
        &mut self,
        ui: &mut egui::Ui,
        series: &'static crate::packages::Series,
        index: usize,
        width: f32,
    ) -> Option<&'static dyn Investigation> {
        let language = self.applied.language.code();
        let mut picked = None;
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
                    for (k, investigation) in parts_of(series).into_iter().enumerate() {
                        let label = RichText::new(format!("{}  {}", k + 1, investigation.title()))
                            .font(FontId::new(24.0, ui::bold()));
                        let r = ui.add(
                            egui::Button::selectable(k == index, label).min_size(vec2(width, 38.0)),
                        );
                        self.widgets
                            .push((format!("part:{}", investigation.path()), r.rect));
                        if r.clicked() {
                            picked = Some(investigation);
                        }
                    }
                })
            });
        picked
    }

    /// Rows of two: watch and explore, then the features' buttons (src/feature/: experiment, the quiz); `first` the
    /// edition to watch: the choice clicked.
    fn open_buttons(
        &mut self,
        ui: &mut egui::Ui,
        chosen: &'static dyn Investigation,
        first: Option<&TimelineRow>,
    ) -> Option<Choice> {
        let mut choice = None;
        // Rows of two: watch and explore, then the features' buttons (src/feature/: experiment, the quiz).
        let first = first.filter(|r| r.summary.is_ok());
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
            self.widgets.push(("watch".into(), watch.rect));
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
            self.widgets.push(("explore".into(), explore.rect));
            if explore.clicked() {
                choice = Some(Choice::Explore {
                    investigation: chosen,
                });
            }
        });
        let buttons: Vec<(&'static dyn crate::feature::Feature, String)> = crate::feature::all()
            .into_iter()
            .filter_map(|f| f.home().map(|label| (f, label)))
            .collect();
        for row in buttons.chunks(2) {
            // Two feature buttons to a row; one the chosen investigation lacks is drawn but cannot be pressed.
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 16.0;
                for (feature, label) in row {
                    let pressed = glass_button(ui, button, label, 26.0, feature.available(chosen));
                    self.widgets.push((feature.id().into(), pressed.rect));
                    if pressed.clicked() {
                        choice = Some(Choice::Feature {
                            investigation: chosen,
                            feature: *feature,
                        });
                    }
                }
            });
        }
        choice
    }

    /// What the story assumes (src/knowledge.rs): the level it starts from, the known topics it uses directly, and
    /// the graph full screen.
    fn prerequisites(&mut self, ui: &mut egui::Ui) {
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
                    text += &tr_with("home-prerequisites_more", args!(count = list.len() - SHOWN));
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
                self.widgets.push(("knowledge".into(), open.rect));
                if open.clicked() {
                    self.knowledge = true;
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
    }

    /// The edition picker: the edition clicked.
    fn edition_picker(
        &mut self,
        ui: &mut egui::Ui,
        rows: &[TimelineRow],
        edition: usize,
    ) -> Option<usize> {
        let mut picked = None;
        ui.horizontal(|ui| {
            ui.label(RichText::new(tr("home-edition")).color(palette::text_dim()));
            for (i, row) in rows.iter().enumerate() {
                // An edition is named by its language (the registry's name), once its timeline is read.
                let name = row.locale.map_or(row.info.id.as_str(), Locale::name);
                let r = ui.selectable_label(
                    i == edition,
                    RichText::new(name).font(FontId::new(24.0, ui::bold())),
                );
                self.widgets
                    .push((format!("edition:{}", row.info.id), r.rect));
                if r.clicked() {
                    picked = Some(i);
                }
            }
        });
        ui.add_space(6.0);
        picked
    }

    /// An edition's title, length, and chapters (or why it is not built): the chapter clicked.
    fn timeline(
        &mut self,
        ui: &mut egui::Ui,
        chosen: &'static dyn Investigation,
        row: &TimelineRow,
    ) -> Option<Choice> {
        let mut choice = None;
        match &row.summary {
            Ok((title, length, chapters, narrated)) => {
                ui.label(
                    RichText::new(format!("{title} · {length}"))
                        .font(FontId::new(24.0, ui::bold()))
                        .color(palette::accent()),
                );
                if !narrated {
                    ui.label(RichText::new(tr("home-no_narration")).color(palette::text_faint()));
                }
                ui.label(RichText::new(tr("home-start_from_a_chapter")).color(palette::text_dim()));
                for (number, label, frame) in chapters {
                    let r = ui.add(
                        egui::Button::new(RichText::new(format!("{number:02}  {label}")))
                            .frame(false)
                            .truncate(),
                    );
                    self.widgets.push((format!("chapter:{number}"), r.rect));
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
                        RichText::new(format!("{} {e}", tr("home-not_built_yet")))
                            .color(palette::text_faint()),
                    )
                    .wrap(),
                );
            }
        }
        ui.add_space(10.0);
        choice
    }
}

/// The demonstrations, with their keys in the viewer.
fn demonstrations(ui: &mut egui::Ui, chosen: &'static dyn Investigation) {
    ui.label(
        RichText::new(tr("home-demonstrations"))
            .font(FontId::new(24.0, ui::bold()))
            .color(palette::accent()),
    );
    const KEYS: [&str; 12] = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "−", "="];
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
}
