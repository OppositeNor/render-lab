//! The dock's Sliders tab for each demonstration of the SDFGI investigation,
//! built from the lab's dock kit (lab/src/controls.rs).

use super::shot::Demo;
use crate::godot::sdfgi::text::tr;
use egui::Ui;
use lab::controls::Dock;

pub fn parameters(dock: &mut Dock, ui: &mut Ui) {
    let demo = Demo::of(dock.state.demo);
    dock.camera(ui);
    let mut toggles: Vec<(&'static str, &str)> = vec![("probes", tr("controls.probes"))];
    match demo {
        Demo::Room => dock.values(ui, tr("controls.gi"), None, "gi"),
        Demo::Cascades | Demo::Regions => {
            dock.values(ui, tr("controls.cascades"), None, "cascades");
            toggles.extend([("regions", tr("controls.regions")), ("scrollView", tr("controls.scroll_view"))]);
            if demo == Demo::Regions {
                dock.values(ui, tr("controls.light"), None, "light");
                toggles.push(("extraLight", tr("controls.extra_light")));
            }
        }
        Demo::Voxels => {
            dock.values(ui, tr("controls.voxels"), None, "voxels");
            toggles.push(("voxels", tr("controls.voxels")));
        }
        Demo::Field | Demo::Occlusion => dock.values(ui, tr("controls.field"), None, "field"),
        Demo::Light => {
            dock.values(ui, tr("controls.light"), None, "light");
            toggles.extend([("cellLight", tr("controls.cell_light")), ("extraLight", tr("controls.extra_light")), ("shadowRay", tr("controls.shadow_ray"))]);
        }
        Demo::Probes | Demo::Maps => {
            dock.values(ui, tr("controls.probe_group"), None, "probes");
            toggles.extend([("probeRays", tr("controls.probe_rays")), ("probeMaps", tr("controls.probe_maps"))]);
        }
        Demo::Pixel => {
            dock.values(ui, tr("controls.pixel"), None, "pixel");
            dock.values(ui, tr("controls.gi"), None, "gi");
            toggles.push(("pixelProbes", tr("controls.pixel_probes")));
        }
    }
    toggles.push(("labels", tr("controls.labels")));
    dock.show(ui, &toggles);
}
