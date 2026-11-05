//! VoxelGI, the baked voxel global illumination of Godot's Forward+ renderer
//! (Godot 4.7.2-stable, the commit in source-index.json): a box of the scene
//! baked in the editor into a sparse octree of cells and a distance field,
//! lit on the GPU by passes that run when a light changes (direct light,
//! a second bounce of six cones, the mips) into an RGBA8 3D texture, moving
//! objects plotted into it every frame, and a screen pass that traces cones
//! through it for every pixel.
//!
//! `bake`, `sdf`, `storage`, `light`, `dynamic`, and `cone` mirror the engine
//! on the CPU, at its sizes; `world` builds the lab's VoxelGI once. The
//! demonstrations and parameters are in `shot`, what each shot shows in
//! `demos`, the dock in `controls`, the Godot source index in
//! source-index.json (written by source_index.py), and the pseudo-code in
//! pseudocode/.

pub mod bake;
pub mod cone;
pub mod controls;
pub mod cost;
pub mod demos;
pub mod dynamic;
pub mod light;
pub mod pseudocode;
pub mod script;
pub mod sdf;
pub mod selftest;
pub mod shot;
pub mod sources;
pub mod storage;
pub mod text;
pub mod update;
pub mod values;
pub mod world;

use crate::godot::voxelgi::text::tr;
use lab::camera::Projector;
use lab::controls::Dock;
use lab::demo::Demos;
use lab::frame::{FrameContent, Statics};
use lab::investigation::{Engine, Investigation, SelftestOptions, Wording};
use lab::pseudocode::Listing;
use lab::shot::{PanelMode, ShotState};
use lab::sources::SourceIndex;
use lab::utility::inset::ImageSpec;
use glam::Vec2;
use shot::Demo;

pub struct VoxelGi;

pub static VOXELGI: VoxelGi = VoxelGi;

static WORDING: Wording = Wording {
    engine_short: "Godot",
    engine_tag: "Godot",
    checkout_dir: "godot/",
};

fn gi_divisor(state: &ShotState) -> u32 {
    if state.p("giHalfResolution") > 0.5 { 2 } else { 1 }
}

/// The images its story can show (`{@ image=key}`).
pub static IMAGES: &[ImageSpec] = &[
    ImageSpec { key: "fieldSlice", doc: "An x-z slice of the baked distance field at sliceHeight after edtPass passes, the worked cell marked." },
    ImageSpec { key: "mipSlice", doc: "An x-z slice of the light texture's mip mipLevel at sliceHeight (RGB x dynamic range; empty texels checkered)." },
    ImageSpec { key: "cellArray", doc: "The sorted cell array as a bar, one colour per level, the worked cell marked." },
];

/// The points its story's arrows and labels can name.
pub static ANNOTATIONS: &[lab::utility::annotations::AnnotationSpec] = &[
    lab::utility::annotations::AnnotationSpec { key: "cells.worked", kind: lab::utility::annotations::AnnotationKind::Point, doc: "The cells demonstration's worked leaf cell." },
    lab::utility::annotations::AnnotationSpec { key: "field.worked", kind: lab::utility::annotations::AnnotationKind::Point, doc: "The field demonstration's worked cell." },
    lab::utility::annotations::AnnotationSpec { key: "light.cell", kind: lab::utility::annotations::AnnotationKind::Point, doc: "The light demonstration's worked cell." },
    lab::utility::annotations::AnnotationSpec { key: "light.march", kind: lab::utility::annotations::AnnotationKind::Point, doc: "The last drawn step of its shadow march." },
    lab::utility::annotations::AnnotationSpec { key: "bounce.cell", kind: lab::utility::annotations::AnnotationKind::Point, doc: "The bounce's worked cell." },
    lab::utility::annotations::AnnotationSpec { key: "mips.parent", kind: lab::utility::annotations::AnnotationKind::Point, doc: "The mip pass's worked parent cell." },
    lab::utility::annotations::AnnotationSpec { key: "pixel.point", kind: lab::utility::annotations::AnnotationKind::Point, doc: "The worked pixel on the sphere." },
];

impl Investigation for VoxelGi {
    fn engine(&self) -> &'static Engine {
        &crate::godot::ENGINE
    }
    fn images(&self) -> &'static [ImageSpec] {
        IMAGES
    }
    fn annotations(&self) -> &'static [lab::utility::annotations::AnnotationSpec] {
        ANNOTATIONS
    }
    fn script_natives(&self, natives: &mut lab::script::lamlet::embed::NativeRegistry) -> Result<(), lab::script::lamlet::embed::Error> {
        cost::natives(natives)?;
        script::register(natives)
    }

    fn id(&self) -> &'static str {
        "voxelgi"
    }

    fn title(&self) -> &'static str {
        tr("investigation.title")
    }

    fn summary(&self) -> &'static str {
        tr("investigation.summary")
    }

    fn wording(&self) -> &'static Wording {
        &WORDING
    }

    fn footer(&self) -> &'static str {
        tr("investigation.footer")
    }

    fn sources(&self) -> &'static SourceIndex {
        sources::INDEX.get()
    }

    fn listings(&self) -> &'static [Listing] {
        pseudocode::all()
    }

    fn package(&self) -> Option<&'static str> {
        Some("godot_voxelgi")
    }

    fn demo_specs(&self) -> Option<&'static Demos> {
        Some(shot::demos())
    }

    fn text(&self, key: &'static str) -> &'static str {
        text::tr_key(key)
    }

    fn backdrop(&self, t: f32) -> Option<ShotState> {
        let mut state = shot::for_demo(Demo::Room);
        state.panel = PanelMode::Hidden;
        state.camera.yaw += 3.0 * t;
        state.camera = lab::shot::camera_drift(state.camera, 0.0, t, 1.0);
        Some(state)
    }

    fn build_frame(&self, state: &ShotState, statics: &Statics, viewport: [f32; 2], pixel_scale: f32, time: f32) -> FrameContent {
        demos::build_frame(state, statics, viewport, pixel_scale, time)
    }

    fn panes(&self, state: &ShotState) -> Vec<lab::investigation::Pane> {
        demos::panes(state)
    }

    fn pass_about(&self, node: &str) -> Option<&'static str> {
        Some(match node {
            "voxelgiLightCS" => tr("pass.voxelgiLightCS"),
            "voxelgiBounceCS" => tr("pass.voxelgiBounceCS"),
            "voxelgiMipmapsCS" => tr("pass.voxelgiMipmapsCS"),
            "voxelgiWriteCS" => tr("pass.voxelgiWriteCS"),
            "voxelgiPrepassFS" => tr("pass.voxelgiPrepassFS"),
            "voxelgiForwardFS" => tr("pass.voxelgiForwardFS"),
            "voxelgiGiCS" => tr("pass.voxelgiGiCS"),
            "voxelgiComposeCS" => tr("pass.voxelgiComposeCS"),
            _ => return None,
        })
    }

    fn shading_hooks(&self) -> &'static [(&'static str, lab::investigation::ShadingHook)] {
        use lab::investigation::ShadingHook;
        static HOOKS: &[(&str, ShadingHook)] = &[
            ("data", ShadingHook::Data(world::shading_data)),
            ("gi-divisor", ShadingHook::Divisor(gi_divisor)),
            ("light-update", ShadingHook::UpdatePlan(update::plan)),
            ("outputs-words", ShadingHook::Words(update::outputs_words)),
        ];
        HOOKS
    }

    fn shader_data(&self, block: &mut lab::shader_data::Block, state: &ShotState) {
        demos::shader_data(block, state)
    }

    fn dock(&self, dock: &mut Dock, ui: &mut egui::Ui) {
        controls::parameters(dock, ui);
    }

    /// A click on a surface is its script's (script/logic.lam `scene`: the leaf cell under it).
    fn click(&self, _shown: &ShotState, _state: &mut ShotState, _content: &FrameContent, _statics: &Statics, _projector: &Projector, _pixel: Vec2) -> bool {
        false
    }

    fn selftest(&self, options: &SelftestOptions) -> anyhow::Result<bool> {
        selftest::run(options)
    }
}
