//! A package's quizzes: questions about its story, each asked over a live view of a demonstration
//! (the viewer's Quiz tab, opened by the homepage's Quiz button). The question and its choices are in the panel
//! at the right; the viewport at the left shows the demonstration the question is about, set as the question says,
//! and stays interactive, so the answer can be found in it.
//!
//! A quiz is a package's `quiz/<name>` resource of kind `file`, TOML (or Lamlet, read as its TOML binding,
//! lab/src/script/data.rs). Its text is keys of the package's text table, in fields ending in `-text`, so the
//! package's build checks them in every language:
//!
//! ```toml
//! title-text = "quiz-title"
//!
//! [[question]]
//! demo = "samples"                     # the demonstration (its key); none: the first
//! question-text = "quiz-tracers"
//! explanation-text = "quiz-tracers-why"   # shown once answered; optional
//! [question.camera]                    # the camera's keys (`render-lab values`); optional
//! yaw = 28.0
//! [question.values]                    # values set over the demonstration's defaults; optional
//! light-count = 8.0
//! [[question.choice]]
//! choice-text = "quiz-tracers-a"
//! [[question.choice]]
//! choice-text = "quiz-tracers-b"
//! correct = true                       # exactly one choice is
//! ```
//!
//! Multiple choice only for now. Formulas and code names (rich text) are not drawn in the panel yet, so a quiz's
//! text may not hold them (`check` says so).

use crate::experiment::{json_of, values_of};
use crate::investigation;
use crate::shot::ShotState;
use anyhow::{anyhow, bail, Context, Result};
use serde_json::{Map, Value as Json};

const FIELDS: &[&str] = &["title-text", "question"];
const QUESTION_FIELDS: &[&str] = &["demo", "question-text", "explanation-text", "camera", "values", "choice"];
const CHOICE_FIELDS: &[&str] = &["choice-text", "correct"];
/// How many choices a question may have.
pub const CHOICES: std::ops::RangeInclusive<usize> = 2..=6;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Quiz {
    /// Its title's key; none: the investigation's title.
    pub title_text: Option<String>,
    pub questions: Vec<Question>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Question {
    pub demo: Option<String>,
    pub question_text: String,
    pub explanation_text: Option<String>,
    /// Camera keys (`camera.yaw`, …) and values, as an experiment's (src/experiment.rs).
    pub camera: Map<String, Json>,
    pub values: Map<String, Json>,
    pub choices: Vec<Choice>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Choice {
    pub text: String,
    pub correct: bool,
}

fn table<'a>(json: &'a Json, at: &str, fields: &[&str]) -> Result<&'a Map<String, Json>> {
    let Json::Object(t) = json else { bail!("{at} is a table") };
    if let Some(unknown) = t.keys().find(|k| !fields.contains(&k.as_str())) {
        bail!("unknown field `{unknown}` in {at} (it has {})", fields.join(", "));
    }
    Ok(t)
}

fn key(t: &Map<String, Json>, field: &str, at: &str) -> Result<Option<String>> {
    match t.get(field) {
        None => Ok(None),
        Some(Json::String(s)) if !s.is_empty() => Ok(Some(s.clone())),
        Some(other) => bail!("{at}: `{field}` is a text key, not {other}"),
    }
}

fn list<'a>(t: &'a Map<String, Json>, field: &str, at: &str) -> Result<&'a [Json]> {
    match t.get(field) {
        None => Ok(&[]),
        Some(Json::Array(a)) => Ok(a),
        Some(other) => bail!("{at}: `{field}` is a list of tables (`[[{field}]]`), not {other}"),
    }
}

impl Quiz {
    /// Parse a quiz file (its form only; `check` checks it against the investigation).
    pub fn parse(source: &str) -> Result<Quiz> {
        let document = toml_edit::Document::parse(source).context("parse the quiz (TOML)")?;
        let json = json_of(document.as_item());
        let top = table(&json, "a quiz", FIELDS)?;
        let mut questions = Vec::new();
        for (i, q) in list(top, "question", "a quiz")?.iter().enumerate() {
            let at = format!("question {}", i + 1);
            let q = table(q, &at, QUESTION_FIELDS)?;
            let mut choices = Vec::new();
            for (j, c) in list(q, "choice", &at)?.iter().enumerate() {
                let at = format!("{at}, choice {}", j + 1);
                let c = table(c, &at, CHOICE_FIELDS)?;
                let correct = match c.get("correct") {
                    None => false,
                    Some(Json::Bool(b)) => *b,
                    Some(other) => bail!("{at}: `correct` is true or false, not {other}"),
                };
                choices.push(Choice { text: key(c, "choice-text", &at)?.ok_or_else(|| anyhow!("{at} has no `choice-text`"))?, correct });
            }
            if !CHOICES.contains(&choices.len()) {
                bail!("{at} has {} choices; a question has {} to {}", choices.len(), CHOICES.start(), CHOICES.end());
            }
            let correct = choices.iter().filter(|c| c.correct).count();
            if correct != 1 {
                bail!("{at} has {correct} correct choices; exactly one is (`correct = true`)");
            }
            let mut camera = Map::new();
            for (k, v) in values_of(q.get("camera"), &format!("{at}: camera"))? {
                camera.insert(if k.starts_with("camera.") { k } else { format!("camera.{k}") }, v);
            }
            let demo = match q.get("demo") {
                None => None,
                Some(Json::String(s)) => Some(s.clone()),
                Some(other) => bail!("{at}: `demo` is a demonstration's key, not {other}"),
            };
            questions.push(Question {
                demo,
                question_text: key(q, "question-text", &at)?.ok_or_else(|| anyhow!("{at} has no `question-text`"))?,
                explanation_text: key(q, "explanation-text", &at)?,
                camera,
                values: values_of(q.get("values"), &format!("{at}: values"))?,
                choices,
            });
        }
        if questions.is_empty() {
            bail!("a quiz has at least one `[[question]]`");
        }
        Ok(Quiz { title_text: key(top, "title-text", "a quiz")?, questions })
    }

    /// Check it against the current investigation: its demonstrations, values, and text keys (in every language
    /// the package has, without formulas or code names).
    pub fn check(&self, package: &str) -> Result<()> {
        let mut problems = Vec::new();
        let text = |key: &str, problems: &mut Vec<String>| match crate::i18n::try_package_text(package, key) {
            None => problems.push(format!("no text `{key}` in {package}'s table")),
            Some(t) if crate::rich_text::is_rich(t) => problems.push(format!("`{key}` holds a formula or a code name, which a quiz does not draw yet")),
            Some(_) => {}
        };
        if let Some(k) = &self.title_text {
            text(k, &mut problems);
        }
        for (i, q) in self.questions.iter().enumerate() {
            let at = format!("question {}", i + 1);
            if let Some(d) = &q.demo {
                if let Err(e) = crate::experiment::demo_named(d) {
                    problems.push(format!("{at}: {e}"));
                }
            }
            for (k, v) in q.camera.iter().chain(&q.values) {
                if let Err(e) = crate::values::resolve(k, v) {
                    problems.push(format!("{at}: {e}"));
                }
            }
            text(&q.question_text, &mut problems);
            for k in q.explanation_text.iter().chain(q.choices.iter().map(|c| &c.text)) {
                text(k, &mut problems);
            }
        }
        if problems.is_empty() {
            Ok(())
        } else {
            bail!("{}", problems.join("\n"))
        }
    }
}

impl Question {
    /// The demonstration it is asked over.
    pub fn start_demo(&self) -> Result<crate::shot::DemoId> {
        match &self.demo {
            Some(d) => crate::experiment::demo_named(d),
            None => Ok(investigation::current().demos()[0]),
        }
    }

    /// Set its camera and then its values on `state` (the demonstration's defaults).
    pub fn apply(&self, state: &mut ShotState) -> Result<()> {
        crate::experiment::apply(state, &self.camera)?;
        crate::experiment::apply(state, &self.values)
    }

    /// The index of its correct choice.
    pub fn correct(&self) -> usize {
        self.choices.iter().position(|c| c.correct).unwrap_or(0)
    }
}

/// The quizzes of `investigation`'s package: its `quiz/<name>` resources' ids, by name.
pub fn ids(investigation: &dyn investigation::Investigation) -> Vec<String> {
    let Some(package) = investigation.package() else { return Vec::new() };
    let prefix = format!("{package}.quiz/");
    let mut ids: Vec<String> = crate::resources::all().iter().map(|r| r.id()).filter(|id| id.starts_with(&prefix)).map(str::to_string).collect();
    ids.sort();
    ids
}

/// Read and check the quiz resource `id` of the current investigation's package.
pub fn load(id: &str) -> Result<Quiz> {
    let package = investigation::current().package().context("the investigation has no package")?;
    let source = crate::script::data::resource_toml(id).map_err(anyhow::Error::msg)?;
    let quiz = Quiz::parse(&source).with_context(|| id.to_string())?;
    quiz.check(package).with_context(|| id.to_string())?;
    Ok(quiz)
}

/// What the Quiz tab shows (the viewer's state of a quiz being taken), its text in the viewer's language.
#[derive(Clone, Debug, Default)]
pub struct QuizView {
    pub title: String,
    pub questions: Vec<QuestionView>,
    /// The question shown; `questions.len()`: the score.
    pub index: usize,
    /// The choice picked for each question, once answered.
    pub answers: Vec<Option<usize>>,
    /// Why no quiz could be opened.
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct QuestionView {
    pub question: String,
    pub choices: Vec<String>,
    pub correct: usize,
    pub explanation: Option<String>,
}

impl QuizView {
    /// `quiz`'s text in the viewer's language, at its first question.
    pub fn of(quiz: &Quiz, package: &str) -> QuizView {
        let text = |k: &str| crate::i18n::package_text(package, k).to_string();
        QuizView {
            title: quiz.title_text.as_deref().map_or_else(|| investigation::current().title().to_string(), text),
            questions: quiz
                .questions
                .iter()
                .map(|q| QuestionView { question: text(&q.question_text), choices: q.choices.iter().map(|c| text(&c.text)).collect(), correct: q.correct(), explanation: q.explanation_text.as_deref().map(text) })
                .collect(),
            index: 0,
            answers: vec![None; quiz.questions.len()],
            error: None,
        }
    }

    /// How many questions were answered right.
    pub fn score(&self) -> usize {
        self.answers.iter().zip(&self.questions).filter(|(a, q)| **a == Some(q.correct)).count()
    }

    pub fn finished(&self) -> bool {
        !self.questions.is_empty() && self.index >= self.questions.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const QUIZ: &str = r#"
title-text = "quiz-title"

[[question]]
demo = "room"
question-text = "q1"
explanation-text = "q1-why"
[question.camera]
yaw = 20.0
[question.values]
light-count = 8.0
[[question.choice]]
choice-text = "q1-a"
[[question.choice]]
choice-text = "q1-b"
correct = true
"#;

    #[test]
    fn parse_a_quiz() {
        let quiz = Quiz::parse(QUIZ).unwrap();
        assert_eq!(quiz.title_text.as_deref(), Some("quiz-title"));
        let q = &quiz.questions[0];
        assert_eq!((q.demo.as_deref(), q.question_text.as_str(), q.explanation_text.as_deref()), (Some("room"), "q1", Some("q1-why")));
        assert_eq!(q.camera.get("camera.yaw"), Some(&Json::from(20.0)));
        assert_eq!(q.values.get("lightCount"), Some(&Json::from(8.0)));
        assert_eq!((q.choices.len(), q.correct()), (2, 1));
    }

    #[test]
    fn refuse_a_malformed_quiz() {
        let error = |source: &str| Quiz::parse(source).unwrap_err().to_string();
        assert!(error("").contains("at least one"));
        assert!(error("colour = 1\n[[question]]").contains("unknown field `colour`"));
        let no_correct = QUIZ.replace("correct = true", "");
        assert!(error(&no_correct).contains("0 correct choices"));
        let two_correct = QUIZ.replace("choice-text = \"q1-a\"", "choice-text = \"q1-a\"\ncorrect = true");
        assert!(error(&two_correct).contains("2 correct choices"));
        let one_choice = "[[question]]\nquestion-text = \"q\"\n[[question.choice]]\nchoice-text = \"a\"\ncorrect = true\n";
        assert!(error(one_choice).contains("1 choices"));
    }
}
