//! The Experiments panel's A/B comparison: the reference, how it is shown, and the measurement.

use super::{Asked, ExperimentView};
use crate::controls::{Dock, SliderRow, Unit, readout};
use crate::experiment::Compare;
use crate::i18n::tr;
use crate::palette;
use egui::{RichText, Ui, vec2};

/// The A/B comparison group: the reference set, swapped, or cleared; how it is shown (a wipe's split, a
/// difference's gain); and Measure.
pub(super) fn compare_group(
    d: &mut Dock,
    ui: &mut Ui,
    view: &ExperimentView,
    asked: &mut Vec<Asked>,
) {
    ui.set_max_width(360.0);
    ui.horizontal(|ui| {
        if ui
            .button(tr("experiments-set_reference"))
            .on_hover_text(tr("experiments-set_reference_hint"))
            .clicked()
        {
            asked.push(Asked::SetReference);
        }
        if ui
            .add_enabled(view.reference, egui::Button::new(tr("experiments-swap")))
            .on_hover_text(tr("experiments-swap_hint"))
            .clicked()
        {
            asked.push(Asked::SwapReference);
        }
        if ui
            .add_enabled(view.reference, egui::Button::new(tr("experiments-clear")))
            .clicked()
        {
            asked.push(Asked::ClearReference);
        }
    });
    ui.add_enabled_ui(view.reference, |ui| {
        ui.horizontal_wrapped(|ui| {
            for mode in Compare::ALL {
                let label = match mode {
                    Compare::Off => tr("experiments-compare_off"),
                    Compare::Wipe => tr("experiments-compare_wipe"),
                    Compare::SideBySide => tr("experiments-compare_side"),
                    Compare::Difference => tr("experiments-compare_difference"),
                    Compare::Reference => tr("experiments-compare_reference"),
                };
                if ui.selectable_label(view.compare == mode, label).clicked() {
                    asked.push(Asked::SetCompare(mode));
                }
            }
        });
        d.rows(ui, "experiment-compare", |d, ui| {
            if view.compare == Compare::Wipe
                && let Some(v) = d.number(
                    ui,
                    "experiment.split",
                    SliderRow {
                        label: tr("experiments-split"),
                        color: palette::text(),
                        range: 0.0..=1.0,
                        unit: Unit::Plain,
                    },
                    view.split,
                )
            {
                asked.push(Asked::SetSplit(v));
            }
            if view.compare == Compare::Difference
                && let Some(v) = d.number(
                    ui,
                    "experiment.gain",
                    SliderRow {
                        label: tr("experiments-gain"),
                        color: palette::text(),
                        range: 1.0..=64.0,
                        unit: Unit::Factor,
                    },
                    view.gain,
                )
            {
                asked.push(Asked::SetGain(v));
            }
        });
        // A stale measurement calls for another: the button is drawn in the accent.
        let measure = if view.stale {
            egui::Button::new(RichText::new(tr("experiments-measure")).color(palette::accent()))
                .stroke(egui::Stroke::new(1.5, palette::accent()))
        } else {
            egui::Button::new(tr("experiments-measure"))
        };
        if ui
            .add(measure)
            .on_hover_text(tr("experiments-measure_hint"))
            .clicked()
        {
            asked.push(Asked::Measure);
        }
    });
}

/// The last measurement against the reference, if there is one: PSNR, SSIM, FLIP, MSE, relMSE, the mean
/// luminances; dimmed under a note when the states or the size changed since.
pub(super) fn measurement_grid(ui: &mut Ui, view: &ExperimentView) {
    if let Some(m) = view.measurement {
        let (names, values) = if view.stale {
            ui.label(RichText::new(tr("experiments-measure_stale")).color(palette::accent()));
            (palette::text_faint(), palette::text_faint())
        } else {
            (palette::text_dim(), palette::text())
        };
        egui::Grid::new("experiment-measurement")
            .num_columns(2)
            .spacing(vec2(10.0, 2.0))
            .show(ui, |ui| {
                for (name, value) in [
                    ("PSNR (dB)", m.psnr),
                    ("SSIM", m.ssim),
                    ("ꟻLIP", m.flip),
                    ("MSE", m.mse),
                    ("relMSE", m.relmse),
                    ("L̄ A", m.luminance_reference),
                    ("L̄ B", m.luminance),
                ] {
                    ui.label(RichText::new(name).color(names));
                    ui.label(RichText::new(readout(value)).color(values));
                    ui.end_row();
                }
            });
    }
}
