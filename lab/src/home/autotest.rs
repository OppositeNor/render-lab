//! The homepage's autotest: clicks through its controls offscreen and checks what each opens.

use super::*;

/// The autotest's homepage, offscreen, and the checks made so far.
struct Autotest<'a> {
    home: Home,
    offscreen: Offscreen,
    options: &'a HomeOptions,
    /// The settings file of its own, beside the report.
    config_path: PathBuf,
    zh_cn: Locale,
    checks: Vec<serde_json::Value>,
}

/// What a click chose, for a check's detail.
fn describe(c: Option<Choice>) -> String {
    c.map(|c| c.describe()).unwrap_or_else(|| "nothing".into())
}

/// Click through the homepage offscreen and check what each control opens.
pub fn autotest(options: &HomeOptions, report: &Path) -> Result<()> {
    // A settings file of its own beside the report, so the user's is never touched.
    let config_path = report.with_file_name("home-autotest-config.toml");
    let zh_cn = Locale::from_code("zh-CN").expect("the registry has zh-CN");
    let _ = std::fs::remove_file(&config_path);
    let mut home = Home::new(options.manifest_dir.clone(), Some(config_path.clone()));
    let mut offscreen = Offscreen::new(options, true)?;
    offscreen.frame(&mut home, Vec::new());
    offscreen.frame(&mut home, Vec::new());
    let mut test = Autotest {
        home,
        offscreen,
        options,
        config_path,
        zh_cn,
        checks: Vec::new(),
    };
    test.config_page();
    test.config_trial();
    test.language();
    test.theme();
    test.volumes();
    test.title();
    test.fields();
    test.selection();
    let checks = test.checks;
    let passed = checks.iter().all(|c| c["passed"] == true);
    std::fs::write(
        report,
        serde_json::to_string_pretty(&serde_json::json!({ "passed": passed, "checks": checks }))?,
    )?;
    if !passed {
        anyhow::bail!("homepage autotest failed; see {}", report.display());
    }
    eprintln!("homepage autotest passed; report: {}", report.display());
    Ok(())
}

impl Autotest<'_> {
    /// A check, printed and kept for the report.
    fn check(&mut self, name: &str, passed: bool, detail: String) {
        eprintln!(
            "home autotest {name}: {} {detail}",
            if passed {
                "ok"
            } else {
                "FAILED"
            }
        );
        self.checks
            .push(serde_json::json!({ "name": name, "passed": passed, "detail": detail }));
    }

    /// A click on the control `name`: None when there is no such control, else what it chose.
    fn click(&mut self, name: &str) -> Option<Option<Choice>> {
        self.offscreen.click(&mut self.home, name)
    }

    /// Esc, then a pass, so the page lists the controls of where it went: whether Esc went anywhere.
    fn esc(&mut self) -> bool {
        let went = self.home.back();
        self.offscreen.frame(&mut self.home, Vec::new());
        went
    }

    /// Whether the page has the control `name`.
    fn has(&self, name: &str) -> bool {
        self.home.widgets.iter().any(|w| w.0 == name)
    }

    /// The settings file as written (render settings are tried, then kept or put back; only Apply writes them).
    fn stored(&self) -> Option<Config> {
        Config::load(&self.config_path)
            .ok()
            .filter(|_| self.config_path.exists())
    }

    /// The colours of the theme `id` the Config page offers.
    fn colors(&self, id: &str) -> Option<palette::Theme> {
        self.home
            .settings
            .themes
            .iter()
            .find(|t| t.id == id)
            .map(|t| t.theme)
    }

    /// Whether the video builder has written `timeline` (not in a fresh checkout, nor on CI).
    fn built(&self, timeline: &TimelineInfo) -> bool {
        self.options.manifest_dir.join(&timeline.path).is_file()
    }

    /// The photosensitivity warning, the Config page's tabs, Save and Apply, and its popup's Restore.
    fn config_page(&mut self) {
        // The photosensitivity warning first, over the title, until Continue.
        self.check(
            "the photosensitivity warning is up at start, in the configured language",
            self.home.pse
                && self.has("pse:continue")
                && tr("home-pse_title") == "Photosensitivity warning",
            tr("home-pse_title").to_string(),
        );
        let exit = self.click("pse:exit").flatten();
        self.check(
            "the warning's Exit quits",
            matches!(exit, Some(Choice::Quit)),
            describe(exit),
        );
        self.click("pse:continue");
        self.check(
            "Continue closes the warning",
            !self.home.pse && !self.has("pse:continue"),
            String::new(),
        );
        self.click("config");
        self.check(
            "the title's Config opens the Config page at the Interface tab",
            self.home.page == Page::Config
                && self.home.settings.category == Category::Interface
                && self.has("option:language:zh-CN")
                && !self.has("option:fullscreen"),
            format!("{:?}, {:?}", self.home.page, self.home.settings.category),
        );
        self.check(
            "with nothing changed, the first button is Return, and no option is marked",
            self.has("return") && !self.has("save") && !self.has("restore:language"),
            String::new(),
        );
        self.click("category:render");
        self.check(
            "the Render tab shows Full Screen",
            self.home.settings.category == Category::Render && self.has("option:fullscreen"),
            format!("{:?}", self.home.settings.category),
        );
        self.click("option:fullscreen");
        self.check(
            "a change turns Return into Save and Apply, and marks the option with its Restore",
            self.has("save") && !self.has("return") && self.has("restore:fullscreen"),
            String::new(),
        );
        self.check(
            "ticking Full Screen changes nothing until it is applied",
            self.home.settings.pending.fullscreen
                && !self.home.applied.fullscreen
                && self.stored().is_none(),
            format!(
                "pending {:?}, fullscreen {}",
                self.home.settings.pending, self.home.applied.fullscreen
            ),
        );
        self.click("save");
        let trying = self.home.settings.trial.is_some() && self.home.applied.fullscreen;
        self.check(
            "Save and Apply tries a render setting without writing it",
            trying && self.stored().is_none(),
            format!(
                "trial {}, fullscreen {}, file {:?}",
                self.home.settings.trial.is_some(),
                self.home.applied.fullscreen,
                self.stored()
            ),
        );
        self.click("trial:restore");
        self.check(
            "the popup's Restore puts the previous settings back",
            self.home.settings.trial.is_none()
                && !self.home.applied.fullscreen
                && !self.home.settings.pending.fullscreen
                && self.stored().is_none(),
            format!(
                "fullscreen {}, pending {:?}",
                self.home.applied.fullscreen, self.home.settings.pending
            ),
        );
    }

    /// A trial running out, the popup's Apply, an option's Restore, Restore Defaults, Discard, and Return.
    fn config_trial(&mut self) {
        self.click("option:fullscreen");
        self.click("save");
        self.offscreen.frames += (TRIAL_SECONDS * 60.0) as u64 + 30;
        self.offscreen.frame(&mut self.home, Vec::new());
        self.check(
            "after ten seconds without Apply, the previous settings come back",
            self.home.settings.trial.is_none()
                && !self.home.applied.fullscreen
                && self.stored().is_none(),
            format!(
                "fullscreen {}, file {:?}",
                self.home.applied.fullscreen,
                self.stored()
            ),
        );
        self.click("option:fullscreen");
        self.click("save");
        self.click("trial:apply");
        self.check(
            "the popup's Apply keeps the settings and writes the file",
            self.home.applied.fullscreen
                && self.stored()
                    == Some(Config {
                        fullscreen: true,
                        ..Config::default()
                    }),
            format!(
                "fullscreen {}, file {:?}",
                self.home.applied.fullscreen,
                self.stored()
            ),
        );
        self.check(
            "right after saving, the first button is Return again",
            self.has("return") && !self.has("save"),
            String::new(),
        );
        self.click("restore:fullscreen");
        self.check(
            "an option's Restore shows its default without applying it",
            !self.home.settings.pending.fullscreen
                && self.home.applied.fullscreen
                && self.has("save")
                && !self.has("restore:fullscreen")
                && self.stored()
                    == Some(Config {
                        fullscreen: true,
                        ..Config::default()
                    }),
            format!(
                "pending {:?}, fullscreen {}",
                self.home.settings.pending, self.home.applied.fullscreen
            ),
        );
        self.click("option:fullscreen");
        self.click("defaults");
        self.check(
            "Restore Defaults shows the defaults without applying them",
            self.home.settings.pending == Config::default()
                && self.home.applied.fullscreen
                && self.stored()
                    == Some(Config {
                        fullscreen: true,
                        ..Config::default()
                    }),
            format!(
                "pending {:?}, fullscreen {}",
                self.home.settings.pending, self.home.applied.fullscreen
            ),
        );
        self.click("discard");
        self.check(
            "Discard drops the edits and returns to the title",
            self.home.page == Page::Title && self.home.settings.pending == self.home.settings.saved,
            format!(
                "{:?}, pending {:?}",
                self.home.page, self.home.settings.pending
            ),
        );
        self.click("config");
        self.click("return");
        self.check(
            "Return goes back to the title",
            self.home.page == Page::Title,
            format!("{:?}", self.home.page),
        );
    }

    /// The language: the title's globe opens the Config page at it; saved, it changes the text at once.
    fn language(&mut self) {
        // The language: the title's globe opens the Config page at it; saved, it changes the text at once.
        self.click("language");
        self.check(
            "the title's globe opens the Config page with the Language option outlined",
            self.home.page == Page::Config
                && self.home.settings.flash.is_some()
                && self.has("option:language:zh-CN"),
            format!("{:?}", self.home.page),
        );
        self.click("option:language:zh-CN");
        self.check(
            "picking a language changes nothing until it is saved",
            self.home.settings.pending.language == self.zh_cn
                && i18n::current() == Locale::EN
                && self.has("restore:language"),
            format!(
                "pending {:?}, text {:?}",
                self.home.settings.pending.language,
                i18n::current()
            ),
        );
        self.click("save");
        self.check(
            "a saved language applies and is written at once, without the popup",
            self.home.settings.trial.is_none()
                && i18n::current() == self.zh_cn
                && tr("home-config") != "Config"
                && self
                    .stored()
                    .is_some_and(|c| c.language == self.zh_cn && c.fullscreen),
            format!(
                "text {:?}: {}, file {:?}",
                i18n::current(),
                tr("home-config"),
                self.stored()
            ),
        );
        self.check(
            "a new language shows the photosensitivity warning again, in that language",
            self.home.pse
                && self.has("pse:continue")
                && tr("home-pse_title") != "Photosensitivity warning",
            tr("home-pse_title").to_string(),
        );
        self.click("pse:continue");
        // The edition offered first is in the language, else English.
        for i in investigations::all()
            .iter()
            .copied()
            .filter(|i| !i.timelines().is_empty())
        {
            let rows = self.home.timeline_rows(i);
            let offered = &rows[preferred_edition(&rows, self.zh_cn)];
            let wanted = rows
                .iter()
                .find(|r| r.locale == Some(self.zh_cn))
                .or_else(|| rows.iter().find(|r| r.locale == Some(Locale::EN)))
                .unwrap_or(&rows[0]);
            self.check(
                &format!(
                    "{}: in Chinese, the edition offered first is the Chinese one, else the English one",
                    i.path()
                ),
                offered.info.id == wanted.info.id,
                format!("{} ({:?})", offered.info.id, offered.locale),
            );
        }
    }

    /// The theme: offered from the themes beside the binary, the default first in use.
    fn theme(&mut self) {
        // The theme: offered from the themes beside the binary, the default first in use.
        let ids: Vec<String> = self
            .home
            .settings
            .themes
            .iter()
            .map(|t| t.id.clone())
            .collect();
        self.check(
            "the Interface tab offers the built-in theme and the default one, which is in use",
            self.has(&format!("option:theme:{}", theme::BUILT_IN))
                && self.has(&format!("option:theme:{}", theme::DEFAULT))
                && self.colors(theme::DEFAULT) == Some(palette::theme()),
            format!("{ids:?}"),
        );
        let other = ids
            .iter()
            .find(|id| *id != theme::DEFAULT && *id != theme::BUILT_IN)
            .cloned()
            .unwrap_or_else(|| theme::BUILT_IN.into());
        // A theme and a render setting at once: the theme applies and is written; the render setting is tried.
        self.click(&format!("option:theme:{other}"));
        self.click("category:render");
        self.click("option:fullscreen");
        self.click("save");
        self.check(
            "a saved theme applies and is written at once, while a render change beside it is tried",
            self.home.settings.trial.is_some()
                && self.colors(&other) == Some(palette::theme())
                && self.stored().is_some_and(|c| c.theme == other && c.fullscreen),
            format!(
                "trial {}, file {:?}",
                self.home.settings.trial.is_some(),
                self.stored()
            ),
        );
        self.click("trial:restore");
        self.check(
            "Restore puts back the render setting and keeps the theme",
            self.home.applied.fullscreen
                && self.home.applied.theme == other
                && self.colors(&other) == Some(palette::theme())
                && self.home.settings.pending == self.home.settings.saved,
            format!("applied {:?}", self.home.applied),
        );
    }

    /// The volumes: a slider and its mute button, applied and written at once.
    fn volumes(&mut self) {
        // The volumes: a slider and its mute button, applied and written at once.
        self.click("category:audio");
        self.click("mute:music");
        self.check(
            "the music's mute button mutes it, and keeps its level",
            self.home.settings.pending.music.muted
                && self.home.settings.pending.music.percent == Config::default().music.percent
                && self.has("restore:music"),
            format!("{:?}", self.home.settings.pending.music),
        );
        let rail = self
            .home
            .widgets
            .iter()
            .find(|w| w.0 == "option:narration")
            .map(|w| w.1);
        if let Some(rail) = rail {
            // A click a quarter along the rail.
            let at = pos2(rail.min.x + rail.width() * 0.25, rail.center().y);
            let press = |pressed| egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            };
            self.offscreen
                .frame(&mut self.home, vec![egui::Event::PointerMoved(at)]);
            self.offscreen.frame(&mut self.home, vec![press(true)]);
            self.offscreen.frame(&mut self.home, vec![press(false)]);
            self.offscreen
                .frame(&mut self.home, vec![egui::Event::PointerGone]);
            self.home.clicked = false;
        }
        let narration = self.home.settings.pending.narration;
        self.check(
            "a click on the narration's slider sets its level",
            rail.is_some() && narration.percent < 60 && !narration.muted,
            format!("{narration:?}"),
        );
        self.click("save");
        self.check(
            "saved volumes are in use and written at once, without the popup",
            self.home.settings.trial.is_none()
                && self.home.applied.music.muted
                && self.home.applied.narration == narration
                && self
                    .stored()
                    .is_some_and(|c| c.music.muted && c.narration == narration),
            format!(
                "music {:?}, file {:?}",
                self.home.applied.music,
                self.stored()
            ),
        );
        self.home.keep_narration(Volume::new(80));
        self.check(
            "the narration's volume as the viewer leaves it is kept and written",
            self.home.applied.narration == Volume::new(80)
                && self
                    .stored()
                    .is_some_and(|c| c.narration == Volume::new(80) && c.language == self.zh_cn),
            format!("file {:?}", self.stored()),
        );
        self.click("config");
        self.click("defaults");
        self.click("save");
        self.click("trial:apply");
        self.check(
            "back in English, the photosensitivity warning shows again, in English",
            self.home.pse && tr("home-pse_title") == "Photosensitivity warning",
            tr("home-pse_title").to_string(),
        );
        self.click("pse:continue");
        self.check(
            "Restore Defaults, applied, puts English, the default theme, and the default volumes back",
            self.home.applied == Config::default()
                && i18n::current() == Locale::EN
                && self.colors(theme::DEFAULT) == Some(palette::theme())
                && self.stored() == Some(Config::default()),
            format!("file {:?}", self.stored()),
        );
        self.click("return");
        let _ = std::fs::remove_file(&self.config_path);
    }

    /// The title page's Quit and Play.
    fn title(&mut self) {
        // The title page.
        let quit = self.click("quit").flatten();
        self.check(
            "the title's Quit quits",
            matches!(quit, Some(Choice::Quit)),
            describe(quit),
        );
        // The investigation the command line names is preselected, in its field.
        let current = investigation::current();
        let field = investigations::field_of(current);
        self.check(
            "the command line's investigation is preselected in its field",
            self.home.field().is(field) && self.home.selected().path() == current.path(),
            format!(
                "{} in {}",
                self.home.selected().path(),
                self.home.field().id()
            ),
        );
        let play = self.click("play");
        self.check(
            "the title's Play opens the fields page",
            play.is_some_and(|c| c.is_none()) && self.home.page == Page::Fields,
            format!("{:?}", self.home.page),
        );
    }

    /// The fields page: every field listed, each opening the selection page of its engines; Back and Esc from the
    /// selection page to the fields and from the fields to the title.
    fn fields(&mut self) {
        let fields = investigations::fields();
        let listed = fields
            .iter()
            .all(|f| self.has(&format!("field:{}", f.id())));
        let ids: Vec<String> = fields.iter().map(|f| f.id()).collect();
        self.check(
            "every field with an investigation is listed",
            listed && !fields.is_empty(),
            ids.join(", "),
        );
        for field in &fields {
            let opened = self.click(&format!("field:{}", field.id())).is_some()
                && self.home.page == Page::Select
                && self.home.field().is(*field);
            // Its engines, and only its, are listed.
            let engines = investigations::engines(*field);
            let shown = engines
                .iter()
                .all(|e| self.has(&format!("engine:{}", e.id)))
                && self
                    .home
                    .widgets
                    .iter()
                    .filter(|w| w.0.starts_with("engine:"))
                    .count()
                    == engines.len();
            self.check(
                &format!(
                    "the folder {} opens the selection page of its engines",
                    field.id()
                ),
                opened && shown,
                engines.iter().map(|e| e.id).collect::<Vec<_>>().join(", "),
            );
            // Back closes an open series first (the command line's investigation may be a part), then leaves.
            let open = self.home.open;
            let mut back = self.click("back").is_some();
            if open {
                back &= self.home.page == Page::Select && !self.home.open;
                back &= self.click("back").is_some();
            }
            self.check(
                &format!("{}: Back returns to the fields", field.id()),
                back && self.home.page == Page::Fields,
                format!("{:?}, a series open: {open}", self.home.page),
            );
        }
        // Esc goes back the same way: the selection page to the fields, the fields to the title.
        self.click(&format!("field:{}", ids[0]));
        let to_fields = self.esc() && self.home.page == Page::Fields;
        let to_title = self.esc() && self.home.page == Page::Title;
        self.check(
            "Esc returns from the selection page to the fields, then to the title",
            to_fields && to_title,
            format!("{:?}", self.home.page),
        );
        let back = self.click("play").is_some() && self.click("back").is_some();
        self.check(
            "the fields page's Back returns to the title",
            back && self.home.page == Page::Title,
            format!("{:?}", self.home.page),
        );
        self.click("play");
    }

    /// The selection page: each investigation selected in its field and opened, and Back.
    fn selection(&mut self) {
        for i in investigations::all() {
            self.open_field_of(*i);
            self.investigation(*i);
        }
        // Back closes an open series first, then returns to the fields.
        let mut back = self.click("back").is_some();
        if self.home.page == Page::Select {
            back &= self.click("back").is_some();
        }
        self.check(
            "Back returns to the fields",
            back && self.home.page == Page::Fields,
            format!("{:?}", self.home.page),
        );
        self.check(
            "text is at least 24 pt at 1080p",
            self.home.min_text >= ui::MIN_TEXT_PT,
            format!("{:.1} pt", self.home.min_text),
        );
        self.check(
            "every click on a control plays the click sound, and hovering plays none",
            self.offscreen.heard == self.offscreen.clicks && self.offscreen.hovers_heard == 0,
            format!(
                "{} of {} clicks, {} hovers",
                self.offscreen.heard, self.offscreen.clicks, self.offscreen.hovers_heard
            ),
        );
    }

    /// The selection page of `i`'s field, from wherever the homepage is.
    fn open_field_of(&mut self, i: &'static dyn Investigation) {
        let field = investigations::field_of(i);
        if self.home.page == Page::Select && self.home.field().is(field) {
            return;
        }
        // Back to the fields page (closing a series on the way), then into the folder.
        for _ in 0..3 {
            if self.home.page == Page::Fields {
                break;
            }
            self.click("back");
        }
        self.click(&format!("field:{}", field.id()));
    }

    /// The investigation `i` selected and opened: Explore, its features' buttons, Watch, its series' parts, a
    /// chapter, and its other editions.
    fn investigation(&mut self, i: &'static dyn Investigation) {
        let engine = format!("engine:{}", i.engine().id);
        // A part of a series: the series in the Feature column, a folder that opens on its parts, then the part.
        let series = i.package().and_then(crate::resources::part_of);
        let feature = series.map_or_else(
            || format!("feature:{}", i.path()),
            |(s, _)| format!("series:{}", s.key),
        );
        let mut selected = self.click(&engine).is_some() && self.click(&feature).is_some();
        if series.is_some() {
            selected &= self.click(&format!("part:{}", i.path())).is_some();
        }
        let selected = selected && self.home.selected().path() == i.path();
        self.check(
            &format!("{} can be selected", i.path()),
            selected,
            feature.clone(),
        );
        let explore = self.click("explore").flatten();
        let ok = matches!(explore, Some(Choice::Explore { investigation }) if investigation.path() == i.path());
        self.check(
            &format!("{}: Explore opens the demonstrations", i.path()),
            ok,
            describe(explore),
        );
        // Each feature's button (src/feature/: experiment, the quiz) opens its tab when the investigation has it, else does nothing.
        for feature in crate::feature::all()
            .into_iter()
            .filter(|f| f.home().is_some())
        {
            let opened = self.click(feature.id()).flatten();
            let has = feature.available(i);
            let ok = if has {
                matches!(opened, Some(Choice::Feature { investigation, feature: f }) if investigation.path() == i.path() && f.id() == feature.id())
            } else {
                opened.is_none()
            };
            self.check(
                &format!(
                    "{}: its {} button opens its tab (has it: {has})",
                    i.path(),
                    feature.id()
                ),
                ok,
                describe(opened),
            );
        }
        let Some(timeline) = i.timelines().first() else {
            return;
        };
        self.watch(i, timeline);
        self.parts(i);
        self.chapters(i, timeline);
        self.editions(i);
    }

    /// Watch opens the timeline from its start; while it is not built, it does nothing.
    fn watch(&mut self, i: &'static dyn Investigation, timeline: &TimelineInfo) {
        // A timeline the video builder has not written (a fresh checkout, CI): its Watch does nothing.
        let watch = self.click("watch").flatten();
        if self.built(timeline) {
            let ok = matches!(watch, Some(Choice::Watch { investigation, timeline: t, start_frame: 0, .. }) if investigation.path() == i.path() && t.id == timeline.id);
            self.check(
                &format!("{}: Watch opens the timeline from its start", i.path()),
                ok,
                describe(watch),
            );
        } else {
            self.check(
                &format!(
                    "{}: Watch does nothing while its timeline is not built",
                    i.path()
                ),
                watch.is_none(),
                describe(watch),
            );
        }
    }

    /// A part of a series: its parts can be picked.
    fn parts(&mut self, i: &'static dyn Investigation) {
        if let Some((series, index)) = i.package().and_then(crate::resources::part_of) {
            let parts: Vec<&'static dyn Investigation> = series
                .parts
                .iter()
                .filter_map(|p| {
                    investigations::all()
                        .iter()
                        .copied()
                        .find(|x| x.package() == Some(p.as_str()))
                })
                .collect();
            let listed = parts.iter().all(|p| {
                self.home
                    .widgets
                    .iter()
                    .any(|w| w.0 == format!("part:{}", p.path()))
            });
            self.check(
                &format!(
                    "{}: part {} of {} of {}, every part listed",
                    i.path(),
                    index + 1,
                    series.parts.len(),
                    series.key
                ),
                listed && parts.len() == series.parts.len(),
                parts
                    .iter()
                    .map(|p| p.path())
                    .collect::<Vec<_>>()
                    .join(" -> "),
            );
            // Picking another part selects its investigation; then back to this one.
            if let Some(other) = parts.iter().find(|p| p.path() != i.path()) {
                let picked = self.click(&format!("part:{}", other.path())).is_some()
                    && self.home.selected().path() == other.path();
                self.check(
                    &format!("{}: picking part {} selects it", i.path(), other.path()),
                    picked,
                    self.home.selected().path(),
                );
                self.click(&format!("part:{}", i.path()));
            }
            // Closing the folder lists the engine's features again, the series among them; Esc closes it too.
            let closed = self.click("series-close").is_some()
                && self.has(&format!("series:{}", series.key))
                && !self.has(&format!("part:{}", i.path()));
            let reopened = self.click(&format!("series:{}", series.key)).is_some()
                && self.has(&format!("part:{}", i.path()));
            let escaped = self.esc() && self.home.page == Page::Select && !self.home.open;
            self.check(
                &format!(
                    "{}: its series folder closes and opens again, and Esc closes it",
                    i.path()
                ),
                closed && reopened && escaped,
                format!("closed {closed}, reopened {reopened}, Esc {escaped}"),
            );
            self.click(&format!("series:{}", series.key));
            self.click(&format!("part:{}", i.path()));
        }
    }

    /// A chapter opens the timeline at its start.
    fn chapters(&mut self, i: &'static dyn Investigation, timeline: &TimelineInfo) {
        match TimelineSummary::load(&self.options.manifest_dir, timeline) {
            _ if !self.built(timeline) => {}
            Ok(summary) => {
                if let Some((number, _, frame)) = summary.chapters.last().cloned() {
                    let chosen = self.click(&format!("chapter:{number}")).flatten();
                    let ok = matches!(chosen, Some(Choice::Watch { start_frame, .. }) if start_frame == frame);
                    self.check(
                        &format!("{}: a chapter opens the timeline at its start", i.path()),
                        ok,
                        format!("chapter {number} (frame {frame}) → {}", describe(chosen)),
                    );
                }
            }
            Err(e) => self.check(
                &format!("{}: its timeline loads", i.path()),
                false,
                format!("{e:#}"),
            ),
        }
    }

    /// Every other edition: picking it makes Watch play that timeline.
    fn editions(&mut self, i: &'static dyn Investigation) {
        for other in i.timelines().iter().skip(1) {
            let picked = self.click(&format!("edition:{}", other.id)).is_some();
            let watch = self.click("watch").flatten();
            if self.built(other) {
                let ok = picked
                    && matches!(watch, Some(Choice::Watch { timeline: t, start_frame: 0, .. }) if t.id == other.id);
                self.check(
                    &format!(
                        "{}: the {} edition can be picked and watched",
                        i.path(),
                        other.id
                    ),
                    ok,
                    describe(watch),
                );
            } else {
                self.check(
                    &format!(
                        "{}: the {} edition, not built, can be picked but not watched",
                        i.path(),
                        other.id
                    ),
                    picked && watch.is_none(),
                    describe(watch),
                );
            }
        }
        if let Some(first) = i.timelines().first() {
            self.click(&format!("edition:{}", first.id));
        }
    }
}
