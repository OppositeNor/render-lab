//! The dock's Sliders tab for each demonstration of the ReSTIR investigation,
//! built from the lab's dock kit (lab/src/controls.rs).

use super::shot::Demo;
use crate::paper::restir::text::tr;
use egui::Ui;
use lab::controls::Dock;

pub fn parameters(dock: &mut Dock, ui: &mut Ui) {
    let demo = Demo::of(dock.state.demo);
    dock.camera(ui);
    match demo {
        Demo::Ris => {
            // RIS at P: the pixel, the stream's step, the seed; then the method's parameters.
            dock.values(ui, tr("controls-pick"), Some(tr("hints-pick")), "pick");
            dock.values(
                ui,
                tr("controls-restir"),
                Some(tr("hints-restir")),
                "restir",
            );
        }
        Demo::Spatial => {
            // Spatial reuse at P.
            dock.values(ui, tr("controls-pick"), Some(tr("hints-pick")), "pick");
            dock.values(
                ui,
                tr("controls-restir"),
                Some(tr("hints-restir")),
                "restir",
            );
            dock.values(
                ui,
                tr("controls-view_group"),
                Some(tr("hints-view")),
                "view",
            );
        }
        Demo::Bias => {
            // Bias in RIS.
            dock.values(ui, tr("controls-bias"), Some(tr("hints-bias")), "bias");
            dock.values(
                ui,
                tr("controls-restir"),
                Some(tr("hints-restir")),
                "restir",
            );
            dock.values(
                ui,
                tr("controls-view_group"),
                Some(tr("hints-view")),
                "view",
            );
        }
        Demo::Room | Demo::Stage0 | Demo::Accum => {
            // The room: the method's parameters, what the scene shows, and the lights.
            dock.values(
                ui,
                tr("controls-restir"),
                Some(tr("hints-restir")),
                "restir",
            );
            dock.values(
                ui,
                tr("controls-view_group"),
                Some(tr("hints-view")),
                "view",
            );
            dock.values(
                ui,
                tr("controls-lights"),
                Some(tr("hints-lights")),
                "lights",
            );
        }
    }
}
