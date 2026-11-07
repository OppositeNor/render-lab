//! The Quiz tab: a package's quiz (src/quiz.rs) in the panel right of the viewport, the question, its choices,
//! and once one is picked, whether it was right and why; at the end, the score. The viewport at the left shows
//! the question's view and stays interactive. The homepage's Take the quiz button opens it.

use super::{Feature, FeatureLayout, FeatureState, Host, Page, PanelUi, Snapshot};
use crate::i18n::{args, tr, tr_with};
use crate::investigation;
use crate::palette;
use crate::quiz::{ids, load, Quiz, QuizView};
use crate::ui::ViewerAction;
use anyhow::{anyhow, bail, Result};
use egui::{vec2, Color32, RichText, Ui};

/// The Quiz tab (src/feature/): the investigation's first quiz in the panel, each question's view in the viewport.
pub struct QuizFeature;

impl Feature for QuizFeature {
    fn id(&self) -> &'static str {
        "quiz"
    }

    fn label(&self) -> Option<String> {
        Some(tr("controls-quiz").to_string())
    }

    fn order(&self) -> i32 {
        60
    }

    fn available(&self, investigation: &dyn investigation::Investigation) -> bool {
        !ids(investigation).is_empty()
    }

    fn layout(&self) -> FeatureLayout {
        FeatureLayout { panel: true, ..Default::default() }
    }

    fn home(&self) -> Option<String> {
        Some(tr("home-take_the_quiz").to_string())
    }

    fn pages(&self) -> &'static [Page] {
        &[Page { name: "viewer-quiz", help: "[:<question>[@<choice>]] (the Quiz button's viewer on that question, 1 first, with that choice picked, A first; none: unanswered; past the last question: the score)" }]
    }

    fn state(&self) -> Box<dyn FeatureState> {
        Box::new(Session::default())
    }
}

/// What the quiz's panel asks of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Asked {
    /// Answer the question shown with this choice.
    Answer(usize),
    /// Go to this question, its view set up again (the number of questions: the score).
    Go(usize),
    /// Take the quiz again from its first question.
    Restart,
}

/// The viewer's quiz: the one opened, how far it has been taken, and what its panel asked last.
#[derive(Clone, Debug, Default)]
pub struct Session {
    pub quiz: Option<Quiz>,
    pub view: QuizView,
    asked: Option<Asked>,
    /// A snapshot page's report line.
    report: String,
}

impl Session {
    /// Open the investigation's first quiz at its first question; a quiz that does not read is said in the panel.
    fn open(&mut self, host: &mut dyn Host) {
        let current = investigation::current();
        let package = current.package().unwrap_or("render_lab");
        let opened = ids(current).first().cloned().ok_or_else(|| anyhow!("{}", tr("quiz-none"))).and_then(|id| load(&id));
        match opened {
            Ok(quiz) => {
                self.view = QuizView::of(&quiz, package);
                self.quiz = Some(quiz);
                self.go(0, host);
            }
            Err(e) => self.view = QuizView { title: current.title().to_string(), error: Some(format!("{e:#}")), ..Default::default() },
        }
    }

    /// Show question `i`: its demonstration at its defaults, then its camera and values (the score past the last
    /// question keeps the view).
    fn go(&mut self, i: usize, host: &mut dyn Host) {
        let Some(quiz) = self.quiz.clone() else { return };
        self.view.index = i.min(quiz.questions.len());
        let Some(question) = quiz.questions.get(i) else { return };
        let shown = question.start_demo().and_then(|demo| {
            let mut state = host.default_state(demo);
            question.apply(&mut state)?;
            Ok(state)
        });
        match shown {
            Ok(state) => host.show(state),
            Err(e) => self.view.error = Some(format!("{e:#}")),
        }
    }

    /// Answer the question shown with `choice`, unless it is answered.
    fn answer(&mut self, choice: usize) {
        let i = self.view.index;
        if let Some(answer) = self.view.answers.get_mut(i).filter(|a| a.is_none()) {
            *answer = Some(choice);
        }
    }
}

impl FeatureState for Session {
    fn selected(&mut self, host: &mut dyn Host) {
        if self.quiz.is_none() {
            self.open(host);
        }
    }

    fn update(&mut self, host: &mut dyn Host) {
        let Some(asked) = self.asked.take() else { return };
        if !host.interactive() {
            return;
        }
        match asked {
            Asked::Answer(choice) => self.answer(choice),
            Asked::Go(i) => self.go(i, host),
            Asked::Restart => {
                self.view.answers.iter_mut().for_each(|a| *a = None);
                self.go(0, host);
            }
        }
    }

    fn panel(&mut self, ui: &mut PanelUi) {
        if let Some(asked) = draw(ui, &self.view) {
            self.asked = Some(asked);
        }
    }

    /// `viewer-quiz[:<question>[@<choice>]]`: question 1 first, choice A first; past the last question, the
    /// score, every question answered with its first choice. Rendered once (the quiz opens), set up, then
    /// settled over twelve frames.
    fn snapshot(&mut self, _page: &str, args: Option<&str>, step: usize, host: &mut dyn Host) -> Result<Snapshot> {
        match step {
            0 => return Ok(Snapshot::Render),
            2..=12 => return Ok(Snapshot::Settle),
            13.. => return Ok(Snapshot::Done(self.report.clone())),
            _ => {}
        }
        let (question, choice) = args.map_or((None, None), |s| s.split_once('@').map_or((Some(s), None), |(q, c)| (Some(q), Some(c))));
        let question = question.map(|q| q.parse::<usize>().ok().filter(|n| *n >= 1).ok_or_else(|| anyhow!("a quiz page's question is a number from 1: `{q}`"))).transpose()?.map_or(0, |q| q - 1);
        let choice = choice
            .map(|c| {
                let c = c.trim().to_ascii_uppercase();
                c.chars().next().filter(|l| c.len() == 1 && l.is_ascii_uppercase()).map(|l| (l as u8 - b'A') as usize).ok_or_else(|| anyhow!("a quiz page's choice is a letter: `{c}`"))
            })
            .transpose()?;
        if let Some(e) = &self.view.error {
            bail!("the quiz did not open: {e}");
        }
        let count = self.view.questions.len();
        if question >= count {
            for i in 0..count {
                self.go(i, host);
                self.answer(0);
            }
        }
        self.go(question.min(count), host);
        if let Some(c) = choice {
            self.answer(c);
        }
        let at = if question >= count { "the score".to_string() } else { format!("question {} of {count}", question + 1) };
        self.report = format!("{at}{}", self.view.error.as_ref().map(|e| format!("; {e}")).unwrap_or_default());
        Ok(Snapshot::Settle)
    }
}

/// The letter before a choice: A, B, C, …
fn letter(i: usize) -> char {
    (b'A' + i as u8) as char
}

/// The panel; what it asks of the quiz (a choice picked, a button pressed), if anything.
fn draw(panel: &mut PanelUi, quiz: &QuizView) -> Option<Asked> {
    let rect = panel.input.layout.panel;
    crate::paint::panel_card(panel.ink.painter, rect, 10);
    let inner = rect.shrink2(vec2(20.0, 16.0));
    let mut asked = None;
    let mut widgets = Vec::new();
    let mut home = false;
    egui::Area::new(egui::Id::unique("viewer-quiz-panel")).fixed_pos(inner.min).constrain(false).fade_in(false).show(panel.ctx, |ui| {
        ui.set_width(inner.width());
        ui.set_clip_rect(inner);
        ui.spacing_mut().item_spacing = vec2(10.0, 10.0);
        ui.spacing_mut().button_padding = vec2(14.0, 10.0);
        egui::ScrollArea::vertical().id_salt("viewer-quiz-scroll").min_scrolled_height(inner.height()).max_height(inner.height()).auto_shrink([false, true]).show(ui, |ui| {
            ui.set_width(inner.width());
            ui.label(RichText::new(&quiz.title).size(30.0).strong().color(palette::text()));
            if let Some(error) = &quiz.error {
                ui.add(egui::Label::new(RichText::new(error).size(22.0).color(palette::MISMATCH)).wrap());
            } else if quiz.questions.is_empty() {
                ui.label(RichText::new(tr("quiz-loading")).size(22.0).color(palette::text_dim()));
            } else if quiz.finished() {
                home = score(ui, quiz, &mut asked, &mut widgets);
            } else {
                question(ui, quiz, &mut asked, &mut widgets);
            }
        });
    });
    if home {
        panel.dock.actions.push(ViewerAction::Home);
    }
    panel.dock.widgets.extend(widgets);
    asked
}

fn question(ui: &mut Ui, quiz: &QuizView, asked: &mut Option<Asked>, widgets: &mut Vec<(String, egui::Rect)>) {
    let i = quiz.index;
    let q = &quiz.questions[i];
    let answer = quiz.answers[i];
    ui.label(RichText::new(tr_with("quiz-question_of", args!(number = i + 1, count = quiz.questions.len()))).size(22.0).color(palette::accent()));
    ui.add(egui::Label::new(RichText::new(&q.question).size(26.0).color(palette::text())).wrap());
    ui.add(egui::Label::new(RichText::new(tr("quiz-live_view")).size(19.0).color(palette::text_faint())).wrap());
    ui.add_space(4.0);
    let width = ui.available_width();
    for (k, choice) in q.choices.iter().enumerate() {
        // Once answered: the right choice green, a wrong pick red, the rest dimmed.
        let (fill, text) = match answer {
            None => (palette::panel_raised(), palette::text()),
            Some(_) if k == q.correct => (palette::RIGHT.gamma_multiply(0.35), palette::text()),
            Some(a) if a == k => (palette::WRONG.gamma_multiply(0.35), palette::text()),
            Some(_) => (palette::panel(), palette::text_faint()),
        };
        let label = RichText::new(format!("{}.  {choice}", letter(k))).size(23.0).color(text);
        let button = egui::Button::new(label).wrap().fill(fill).min_size(vec2(width, 52.0));
        let response = ui.add_enabled(answer.is_none(), button);
        widgets.push((format!("quiz-choice-{k}"), response.rect));
        if response.clicked() {
            *asked = Some(Asked::Answer(k));
        }
    }
    if let Some(a) = answer {
        ui.add_space(4.0);
        let (verdict, color) = if a == q.correct { (tr("quiz-right").to_string(), palette::RIGHT) } else { (tr_with("quiz-wrong", args!(letter = letter(q.correct).to_string())), palette::WRONG) };
        ui.label(RichText::new(verdict).size(24.0).strong().color(color));
        if let Some(why) = &q.explanation {
            ui.add(egui::Label::new(RichText::new(why).size(22.0).color(palette::text_dim())).wrap());
        }
    }
    ui.add_space(8.0);
    // Wrapped: three buttons of a long language need not fit one line.
    ui.horizontal_wrapped(|ui| {
        if i > 0 {
            let back = ui.button(RichText::new(tr("quiz-previous")).size(21.0));
            widgets.push(("quiz-previous".into(), back.rect));
            if back.clicked() {
                *asked = Some(Asked::Go(i - 1));
            }
        }
        let view = ui.button(RichText::new(tr("quiz-show_view")).size(21.0)).on_hover_text(tr("quiz-show_view_hint"));
        widgets.push(("quiz-view".into(), view.rect));
        if view.clicked() {
            *asked = Some(Asked::Go(i));
        }
        let last = i + 1 == quiz.questions.len();
        let label = if last { tr("quiz-see_score") } else { tr("quiz-next") };
        let next = ui.add_enabled(answer.is_some(), egui::Button::new(RichText::new(label).size(21.0)));
        widgets.push(("quiz-next".into(), next.rect));
        if next.clicked() {
            *asked = Some(Asked::Go(i + 1));
        }
    });
}

/// The score; true when Home is pressed.
fn score(ui: &mut Ui, quiz: &QuizView, asked: &mut Option<Asked>, widgets: &mut Vec<(String, egui::Rect)>) -> bool {
    let mut home = false;
    let right = quiz.score();
    ui.label(RichText::new(tr_with("quiz-score", args!(right = right, count = quiz.questions.len()))).size(28.0).strong().color(palette::accent()));
    for (k, (q, a)) in quiz.questions.iter().zip(&quiz.answers).enumerate() {
        let ok = *a == Some(q.correct);
        let (mark, color): (&str, Color32) = if ok { ("✓", palette::RIGHT) } else { ("✗", palette::WRONG) };
        ui.horizontal_top(|ui| {
            ui.label(RichText::new(format!("{mark} {}.", k + 1)).size(22.0).color(color));
            ui.add(egui::Label::new(RichText::new(&q.question).size(21.0).color(palette::text_dim())).wrap());
        });
    }
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        let again = ui.button(RichText::new(tr("quiz-again")).size(21.0));
        widgets.push(("quiz-again".into(), again.rect));
        if again.clicked() {
            *asked = Some(Asked::Restart);
        }
        let home_button = ui.button(RichText::new(tr("quiz-home")).size(21.0));
        widgets.push(("quiz-home".into(), home_button.rect));
        if home_button.clicked() {
            home = true;
        }
    });
    home
}
