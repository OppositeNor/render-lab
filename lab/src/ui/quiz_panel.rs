//! The Quiz tab's panel, right of the viewport (src/quiz.rs): the question, its choices, and once one is picked,
//! whether it was right and why; at the end, the score. The viewport at the left shows the question's view and
//! stays interactive.

use super::{Ink, UiInput, ViewerAction, ViewerChrome};
use crate::i18n::{args, tr, tr_with};
use crate::palette;
use egui::{vec2, Color32, RichText, Ui};

/// The letter before a choice: A, B, C, …
fn letter(i: usize) -> char {
    (b'A' + i as u8) as char
}

pub fn draw(ctx: &egui::Context, ink: &Ink, input: &UiInput, viewer: &mut ViewerChrome) {
    let rect = input.layout.panel;
    crate::paint::panel_card(ink.painter, rect, 10);
    let inner = rect.shrink2(vec2(20.0, 16.0));
    let quiz = viewer.quiz.clone();
    let mut actions = Vec::new();
    let mut widgets = Vec::new();
    egui::Area::new(egui::Id::unique("viewer-quiz-panel")).fixed_pos(inner.min).constrain(false).fade_in(false).show(ctx, |ui| {
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
                score(ui, &quiz, &mut actions, &mut widgets);
            } else {
                question(ui, &quiz, &mut actions, &mut widgets);
            }
        });
    });
    viewer.actions.extend(actions);
    viewer.widgets.extend(widgets);
}

fn question(ui: &mut Ui, quiz: &crate::quiz::QuizView, actions: &mut Vec<ViewerAction>, widgets: &mut Vec<(String, egui::Rect)>) {
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
            actions.push(ViewerAction::QuizAnswer(k));
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
                actions.push(ViewerAction::QuizGo(i - 1));
            }
        }
        let view = ui.button(RichText::new(tr("quiz-show_view")).size(21.0)).on_hover_text(tr("quiz-show_view_hint"));
        widgets.push(("quiz-view".into(), view.rect));
        if view.clicked() {
            actions.push(ViewerAction::QuizGo(i));
        }
        let last = i + 1 == quiz.questions.len();
        let label = if last { tr("quiz-see_score") } else { tr("quiz-next") };
        let next = ui.add_enabled(answer.is_some(), egui::Button::new(RichText::new(label).size(21.0)));
        widgets.push(("quiz-next".into(), next.rect));
        if next.clicked() {
            actions.push(ViewerAction::QuizGo(i + 1));
        }
    });
}

fn score(ui: &mut Ui, quiz: &crate::quiz::QuizView, actions: &mut Vec<ViewerAction>, widgets: &mut Vec<(String, egui::Rect)>) {
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
            actions.push(ViewerAction::QuizRestart);
        }
        let home = ui.button(RichText::new(tr("quiz-home")).size(21.0));
        widgets.push(("quiz-home".into(), home.rect));
        if home.clicked() {
            actions.push(ViewerAction::Home);
        }
    });
}
