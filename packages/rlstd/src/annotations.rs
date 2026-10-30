//! Arrows and labels a story declares (`{@ arrow=a arrowFrom=… arrowTo=…}`,
//! `{@ label=l labelAt=… labelText=…}`, `{@ arrows=set}`), drawn over the
//! investigation's frame. A point is three numbers, or a point the
//! investigation publishes by name (`FrameContent::points`, declared in
//! `Investigation::annotations`), so an arrow can follow what the
//! investigation computes; a set is arrows the investigation computes and
//! publishes by name (`FrameContent::arrow_sets`) in the frames that show it.
//! The story says which, when, and in what colour; the timeline fades them
//! over `blend` and blends fixed points (src/shot.rs).

use crate::frame::{FrameContent, Label, FLAG_ARROW};
use egui::Color32;
use glam::Vec3;
use serde::Serialize;

/// What an investigation publishes for a story's annotations (`Investigation::annotations`).
pub struct AnnotationSpec {
    /// Its name in a story: a point's (`arrowFrom=<key>`) or a set's (`arrows=<key>`).
    pub key: &'static str,
    pub kind: AnnotationKind,
    pub doc: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnnotationKind {
    Point,
    ArrowSet,
}

/// An arrow of a set an investigation publishes (`FrameContent::arrow_sets`).
#[derive(Clone, Debug)]
pub struct Arrow {
    pub from: Vec3,
    pub to: Vec3,
    pub color: Color32,
    pub width: f32,
}

/// Where an arrow ends or a label sits: a fixed point, or a published one.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum Anchor {
    At([f32; 3]),
    Named(String),
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum NoteKind {
    Arrow { from: Anchor, to: Anchor, width: f32 },
    Label { at: Anchor, text: String },
    /// A set the investigation publishes, by its key.
    Arrows,
}

/// An arrow, a label, or a set of arrows the story shows.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct StoryNote {
    /// Its name in the story (a set's is the set's key).
    pub name: String,
    pub kind: NoteKind,
    /// Its colour; a set's arrows keep their own without one.
    pub color: Option<[u8; 3]>,
    /// How far it has faded in (0..1).
    pub alpha: f32,
}

/// A colour as a story writes it: `#rrggbb`.
pub fn parse_color(text: &str) -> Result<[u8; 3], String> {
    let hex = text.strip_prefix('#').filter(|h| h.len() == 6).ok_or_else(|| format!("`{text}` is not a colour (#rrggbb)"))?;
    let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|_| format!("`{text}` is not a colour (#rrggbb)"));
    Ok([byte(0)?, byte(2)?, byte(4)?])
}

fn resolve(anchor: &Anchor, content: &FrameContent) -> Option<Vec3> {
    match anchor {
        Anchor::At(p) => Some(Vec3::from(*p)),
        Anchor::Named(key) => content.points.iter().find(|(k, _)| k == key).map(|(_, p)| *p),
    }
}

/// Draws the story's `notes` into `content`: arrows as segments with heads,
/// labels as the frame's labels (`package`'s text), and the published sets
/// it shows. A note whose point the frame does not publish is left out.
pub fn draw(notes: &[StoryNote], content: &mut FrameContent, package: &str) {
    for note in notes.iter().filter(|n| n.alpha > 0.0) {
        let color = note.color.map(|[r, g, b]| Color32::from_rgb(r, g, b));
        match &note.kind {
            NoteKind::Arrow { from, to, width } => {
                if let (Some(a), Some(b)) = (resolve(from, content), resolve(to, content)) {
                    content.draw.segment(a, b, color.unwrap_or(crate::palette::text()), note.alpha, *width, FLAG_ARROW);
                }
            }
            NoteKind::Label { at, text } => {
                if let Some(anchor) = resolve(at, content).filter(|_| note.alpha >= 0.5) {
                    let text = crate::i18n::package_text(package, text).to_string();
                    content.labels.push(Label { anchor, text, color: color.unwrap_or(crate::palette::text()) });
                }
            }
            NoteKind::Arrows => {
                let arrows: Vec<Arrow> = content.arrow_sets.iter().find(|(k, _)| *k == note.name).map(|(_, a)| a.clone()).unwrap_or_default();
                for a in arrows {
                    content.draw.segment(a.from, a.to, color.unwrap_or(a.color), note.alpha, a.width, FLAG_ARROW);
                }
            }
        }
    }
}
