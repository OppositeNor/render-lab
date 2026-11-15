//! The dock's Sliders tab for each demonstration of the ReSTIR investigation,
//! built from the lab's dock kit (lab/src/controls.rs).

use super::shot::Demo;
use crate::paper::restir::text::tr;
use egui::Ui;
use lab::controls::Dock;

/// The value groups a demonstration shows, in order: the per-pixel ones start with P, the 1D example with its own.
fn groups(demo: Demo) -> &'static [&'static str] {
    match demo {
        Demo::Ris => &["pick", "restir", "view", "lights"],
        Demo::Spatial => &["pick", "restir", "view", "lights"],
        Demo::Reuse => &["pick", "restir", "view", "lights"],
        Demo::Bias => &["bias", "restir", "view", "lights"],
        Demo::Toy => &["toy"],
        Demo::Wrs => &["wrs", "pick", "restir"],
        Demo::Lights => &["sampling", "pick", "lights"],
        Demo::Weights => &["weights"],
        Demo::Params => &["params"],
        Demo::Combine => &["combine"],
        Demo::Rounds => &["rounds", "restir"],
        Demo::Methods => &["restir", "view", "lights"],
        Demo::Room | Demo::Reservoirs | Demo::Reference => &["restir", "view", "lights"],
    }
}

/// One group of values, with its title and hint.
fn group(dock: &mut Dock, ui: &mut Ui, label: &str) {
    let (title, hint) = match label {
        "pick" => (tr("controls-pick"), tr("hints-pick")),
        "restir" => (tr("controls-restir"), tr("hints-restir")),
        "view" => (tr("controls-view_group"), tr("hints-view")),
        "lights" => (tr("controls-lights"), tr("hints-lights")),
        "toy" => (tr("controls-toy"), tr("hints-toy")),
        "wrs" => (tr("controls-wrs"), tr("hints-wrs")),
        "sampling" => (tr("controls-sampling"), tr("hints-sampling")),
        "weights" => (tr("controls-weights"), tr("hints-weights")),
        "params" => (tr("controls-params"), tr("hints-params")),
        "combine" => (tr("controls-combine"), tr("hints-combine")),
        "rounds" => (tr("controls-rounds"), tr("hints-rounds")),
        _ => (tr("controls-bias"), tr("hints-bias")),
    };
    dock.values(ui, title, Some(hint), label);
}

pub fn parameters(dock: &mut Dock, ui: &mut Ui) {
    let demo = Demo::of(dock.state.demo);
    dock.camera(ui);
    for label in groups(demo) {
        group(dock, ui, label);
    }
}
