//! The dock's Sliders tab for each demonstration of the SSR investigation,
//! built from the lab's dock kit (lab/src/controls.rs).

use super::shot::Demo;
use crate::godot::ssr::text::tr;
use egui::Ui;
use lab::controls::Dock;

pub fn parameters(dock: &mut Dock, ui: &mut Ui) {
    let demo = Demo::of(dock.state.demo);
    dock.camera(ui);
    match demo {
        Demo::Room => {
            dock.values(ui, tr("controls.ssr"), Some(tr("hints.ssr")), "ssr");
            dock.values(ui, tr("controls.surface"), Some(tr("hints.surface")), "surface");
        }
        Demo::Pyramid | Demo::Trace => {
            dock.values(ui, tr("controls.pick"), Some(tr("hints.pick")), "pick");
            dock.values(ui, tr("controls.field"), Some(tr("hints.field")), "field");
            dock.values(ui, tr("controls.ssr"), Some(tr("hints.ssr")), "ssr");
            dock.values(ui, tr("controls.surface"), Some(tr("hints.surface")), "surface");
        }
        Demo::Inputs | Demo::Hit | Demo::Blur => {
            dock.values(ui, tr("controls.pick"), Some(tr("hints.pick")), "pick");
            dock.values(ui, tr("controls.ssr"), Some(tr("hints.ssr")), "ssr");
            dock.values(ui, tr("controls.surface"), Some(tr("hints.surface")), "surface");
        }
        Demo::Compose => {
            dock.values(ui, tr("controls.compose"), Some(tr("hints.compose")), "compose");
            dock.values(ui, tr("controls.surface"), Some(tr("hints.surface")), "surface");
        }
    }
}
