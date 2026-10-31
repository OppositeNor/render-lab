//! The dock's Sliders tab for each demonstration of the VoxelGI
//! investigation, built from the lab's dock kit (lab/src/controls.rs).

use super::shot::Demo;
use crate::godot::voxelgi::text::tr;
use egui::Ui;
use lab::controls::Dock;

pub fn parameters(dock: &mut Dock, ui: &mut Ui) {
    let demo = Demo::of(dock.state.demo);
    dock.camera(ui);
    let mut toggles: Vec<(&'static str, &str)> = Vec::new();
    match demo {
        Demo::Room | Demo::Compose => dock.values(ui, tr("controls.gi"), None, "gi"),
        Demo::Bounds | Demo::Octree => {
            dock.values(ui, tr("controls.bake"), None, "bake");
            toggles.extend([("workedTriangle", tr("controls.worked_triangle")), ("workedLeaf", tr("controls.worked_leaf"))]);
        }
        Demo::Cells | Demo::Field => {
            dock.values(ui, tr("controls.cells"), None, "cells");
            dock.values(ui, tr("controls.field"), None, "field");
            toggles.push(("cellArray", tr("controls.cell_array")));
        }
        Demo::Light => {
            dock.values(ui, tr("controls.light"), None, "light");
            toggles.extend([("extraLight", tr("controls.extra_light")), ("workedLight", tr("controls.worked_light"))]);
        }
        Demo::Bounce | Demo::Mips => {
            dock.values(ui, tr("controls.bounce"), None, "bounce");
            dock.values(ui, tr("controls.cones"), None, "cones");
            toggles.extend([("workedBounce", tr("controls.worked_bounce")), ("workedParent", tr("controls.worked_parent"))]);
        }
        Demo::Dynamic => {
            dock.values(ui, tr("controls.dynamic"), None, "dynamic");
            toggles.push(("dynamicVoxels", tr("controls.dynamic_voxels")));
        }
        Demo::Pixel => {
            dock.values(ui, tr("controls.cones"), None, "cones");
            dock.values(ui, tr("controls.gi"), None, "gi");
            toggles.extend([("workedPixel", tr("controls.worked_pixel")), ("specCone", tr("controls.spec_cone"))]);
        }
    }
    toggles.push(("labels", tr("controls.labels")));
    dock.show(ui, &toggles);
}
