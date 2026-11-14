//! The Experiments panel's status: what A and B are, which of them the dock edits, whether the measurement is
//! current, whether it is saved, and whether the changes carry over to the next demonstration.

use super::{Asked, ExperimentView};
use crate::i18n::{args, tr, tr_with};
use crate::palette;
use egui::{RichText, Ui};

/// The status above the panel's groups, and the carry's checkbox; what it asks goes to `asked`.
pub(super) fn status_strip(ui: &mut Ui, view: &ExperimentView, asked: &mut Vec<Asked>) {
    ui.set_max_width(330.0);
    let sides = if view.reference {
        tr_with("experiments-status_reference", args!(count = view.ab.len()))
    } else {
        tr("experiments-status_no_reference").to_string()
    };
    ui.label(RichText::new(sides).color(palette::text()));
    // A paused reference is not rendered; one rendering again builds its history up.
    if view.paused && view.reference {
        ui.label(RichText::new(tr("experiments-status_paused")).color(palette::accent()));
    } else if view.warming {
        ui.label(RichText::new(tr("experiments-status_warming")).color(palette::accent()));
    }
    let (editing, color) = if view.editing_reference {
        (
            tr("experiments-status_editing_reference"),
            palette::accent(),
        )
    } else {
        (tr("experiments-status_editing_live"), palette::text_dim())
    };
    ui.label(RichText::new(editing).color(color));
    let (measured, color) = match (view.measurement.is_some(), view.stale) {
        (false, _) => (tr("experiments-status_measure_none"), palette::text_dim()),
        (true, false) => (
            tr("experiments-status_measure_current"),
            palette::text_dim(),
        ),
        (true, true) => (tr("experiments-status_measure_stale"), palette::accent()),
    };
    ui.label(RichText::new(measured).color(color));
    let (saved, color) = match (&view.saved, view.unsaved) {
        (_, true) => (
            tr("experiments-status_unsaved").to_string(),
            palette::accent(),
        ),
        (Some(name), false) => (
            tr_with("experiments-status_saved", args!(name = name.clone())),
            palette::text_dim(),
        ),
        (None, false) => (
            tr("experiments-status_nothing_to_save").to_string(),
            palette::text_dim(),
        ),
    };
    ui.label(RichText::new(saved).color(color));
    let mut carry = view.carry;
    if ui
        .checkbox(&mut carry, tr("experiments-carry"))
        .on_hover_text(tr("experiments-carry_hint"))
        .changed()
    {
        asked.push(Asked::SetCarry(carry));
    }
    ui.separator();
}
