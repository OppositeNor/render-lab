//! The Experiments panel's changed values: what A and B set differently, and what differs from the
//! demonstration's defaults.

use crate::controls::{Dock, readout};
use crate::i18n::{args, tr, tr_with};
use crate::palette;
use crate::shot::ShotState;
use crate::ui::ViewerAction;
use crate::values::{self, Entry};
use egui::{RichText, Ui};

/// A value A and B set differently: its entry and channel (a vector's component), A's and B's.
pub(super) struct AbRow {
    entry: &'static Entry,
    channel: &'static str,
    a: f32,
    b: f32,
}

/// Every value `reference` (A) and `live` (B) set differently, in the values' order.
pub(super) fn ab_rows(reference: &ShotState, live: &ShotState) -> Vec<AbRow> {
    values::list()
        .iter()
        .flat_map(|e| e.channels().into_iter().map(move |c| (*e, c)))
        .filter_map(|(entry, channel)| {
            let a = reference.try_get(channel)?.f32();
            let b = live.try_get(channel)?.f32();
            (a != b).then_some(AbRow {
                entry,
                channel,
                a,
                b,
            })
        })
        .collect()
}

/// The A vs B group: a row per value A and B set differently, with A's, B's, and B − A (a choice's options,
/// no difference); a click on a name shows only its control in the dock (`dock::show_only`).
pub(super) fn ab_group(ui: &mut Ui, rows: &[AbRow]) {
    ui.set_max_width(360.0);
    if rows.is_empty() {
        ui.label(RichText::new(tr("experiments-ab_same")).color(palette::text_faint()));
        return;
    }
    for row in rows {
        if ui
            .link(channel_name(row.entry, row.channel))
            .on_hover_text(tr("experiments-ab_show_hint"))
            .clicked()
        {
            super::dock::show_only(ui, row.entry.key);
        }
        let text = |v: f32| match row.entry.choices {
            Some(c) if (v as usize) < c.options.len() => c.option_text(v as usize).to_string(),
            _ => readout(v as f64),
        };
        // Under the name: A's, B's, and (for a number) B − A.
        let mut parts = vec![("A", text(row.a)), ("B", text(row.b))];
        if row.entry.choices.is_none() {
            let d = row.b - row.a;
            let sign = if d > 0.0 {
                "+"
            } else {
                ""
            };
            parts.push(("B − A", format!("{sign}{}", readout(d as f64))));
        }
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            for (name, value) in parts {
                ui.label(RichText::new(name).color(palette::text_faint()));
                ui.label(RichText::new(value).color(palette::text()));
                ui.add_space(6.0);
            }
        });
    }
}

/// The experiment group: every value changed from the demonstration's defaults, each with its reset, and
/// Reset all.
pub(super) fn changed_group(d: &mut Dock, ui: &mut Ui, changed: &[(&'static Entry, &'static str)]) {
    ui.set_max_width(330.0);
    ui.label(
        RichText::new(tr_with("experiments-changed", args!(count = changed.len())))
            .color(palette::text_dim()),
    );
    if changed.is_empty() {
        ui.label(RichText::new(tr("experiments-no_changes")).color(palette::text_faint()));
    }
    for (entry, channel) in changed {
        ui.horizontal(|ui| {
            let name = channel_name(entry, channel);
            let value = d
                .state
                .try_get(channel)
                .map(|v| readout(v.f32() as f64))
                .unwrap_or_default();
            if ui.small_button("↺").clicked() {
                d.reset(channel);
            }
            ui.label(RichText::new(format!("{name} = {value}")).color(palette::text()));
        });
    }
    if ui
        .add_enabled(
            !changed.is_empty(),
            egui::Button::new(tr("experiments-reset_all")),
        )
        .clicked()
    {
        d.actions.push(ViewerAction::ResetParameters);
    }
}

/// A value's name on screen: its entry's, with a vector's component after it.
fn channel_name(entry: &Entry, channel: &str) -> String {
    if entry.components.is_empty() {
        entry.label_text().to_string()
    } else {
        format!(
            "{} {}",
            entry.label_text(),
            channel.rsplit('.').next().unwrap_or(channel)
        )
    }
}
