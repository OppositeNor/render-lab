//! The Experiments panel's status: what A and B are, which of them the dock edits, and whether the changes
//! carry over to the next demonstration.

use super::{Asked, ExperimentView};
use crate::i18n::{args, tr, tr_with};
use crate::palette;
use egui::{RichText, Ui};

/// The status above the panel's groups, and the carry's checkbox; what it asks goes to `asked`.
pub(super) fn status_strip(ui: &mut Ui, view: &ExperimentView, asked: &mut Vec<Asked>) {
    ui.set_max_width(330.0);
    let sides = if view.reference {
        tr_with("experiments-status_reference", args!(count = view.differ))
    } else {
        tr("experiments-status_no_reference").to_string()
    };
    ui.label(RichText::new(sides).color(palette::text()));
    let (editing, color) = if view.editing_reference {
        (
            tr("experiments-status_editing_reference"),
            palette::accent(),
        )
    } else {
        (tr("experiments-status_editing_live"), palette::text_dim())
    };
    ui.label(RichText::new(editing).color(color));
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
