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
