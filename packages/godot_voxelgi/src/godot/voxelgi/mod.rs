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

pub mod actions;
pub mod bake;
pub mod cone;
pub mod controls;
pub mod demos;
pub mod dynamic;
pub mod light;
pub mod pseudocode;
pub mod sdf;
pub mod selftest;
pub mod shot;
pub mod sources;
pub mod storage;
pub mod text;
pub mod values;
pub mod world;

use crate::godot::voxelgi::text::tr;
use lab::camera::Projector;
use lab::controls::Dock;
use lab::demo::Demos;
use lab::frame::{FrameContent, Statics};
use lab::investigation::{Engine, Investigation, SceneShading, SelftestOptions, ShadingPass, ShadingTarget, Wording};
use lab::pseudocode::Listing;
use lab::shot::{DemoId, PanelMode, ShotState};
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

/// The scene shades with shaders/voxelgi_shading.wgsl, in Forward+'s order: a
/// depth pre-pass, the scene pass (the lab's direct light and the normal), the
/// WGSL port of Godot's VoxelGI screen pass once per pixel (reading the lit
/// texture of the CPU world, world.rs), then the opaque pass's mix.
const TARGETS: &[ShadingTarget] = &[
    ShadingTarget { name: "direct", format: wgpu::TextureFormat::Rgba16Float, divisor: 1 },
    ShadingTarget { name: "normal", format: wgpu::TextureFormat::Rgba16Float, divisor: 1 },
    ShadingTarget { name: "giAmbient", format: wgpu::TextureFormat::Rgba16Float, divisor: 1 },
    ShadingTarget { name: "giReflection", format: wgpu::TextureFormat::Rgba16Float, divisor: 1 },
];

const PASSES: &[ShadingPass] = &[
    ShadingPass { entry: "voxelgiGiCS", reads: &["direct", "normal", "depth"], writes: &["giAmbient", "giReflection"], workgroup: [8, 8] },
    ShadingPass { entry: "voxelgiComposeCS", reads: &["direct", "normal", "giAmbient", "giReflection", "depth"], writes: &["hdr"], workgroup: [8, 8] },
];

pub static SHADING: SceneShading = SceneShading {
    module: "godot_voxelgi.shader/voxelgi-shading",
    fragment: "voxelgiForwardFS",
    data: world::shading_data,
    verify: &["voxelgiProcessTests", "voxelgiLightTests", "voxelgiBounceTests"],
    targets: TARGETS,
    outputs: &["direct", "normal"],
    passes: PASSES,
    prepass: Some("voxelgiPrepassFS"),
    early: &[],
    buffers: &[],
    scene_reads: &[],
    depth_views: None,
};

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
    fn actions(&self) -> &'static lab::actions::Actions {
        &Self::ACTIONS
    }
    fn images(&self) -> &'static [ImageSpec] {
        IMAGES
    }
    fn annotations(&self) -> &'static [lab::utility::annotations::AnnotationSpec] {
        ANNOTATIONS
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

    fn animation(&self, demo: DemoId, t: f32) -> Vec<(&'static str, f32)> {
        shot::demo_animation(Demo::of(demo), t)
    }

    fn build_frame(&self, state: &ShotState, statics: &Statics, viewport: [f32; 2], pixel_scale: f32, time: f32) -> FrameContent {
        demos::build_frame(state, statics, viewport, pixel_scale, time)
    }

    fn scene_shading(&self) -> Option<&'static SceneShading> {
        Some(&SHADING)
    }

    fn shader_data(&self, block: &mut lab::shader_data::Block, state: &ShotState) {
        demos::shader_data(block, state)
    }

    fn dock(&self, dock: &mut Dock, ui: &mut egui::Ui) {
        controls::parameters(dock, ui);
    }

    /// A click on a surface picks the leaf cell under it (`cellX`, `cellY`, `cellZ`).
    fn click(&self, shown: &ShotState, state: &mut ShotState, content: &FrameContent, _statics: &Statics, projector: &Projector, pixel: Vec2) -> bool {
        let (ro, rd) = projector.ray(pixel);
        let hit = lab::scene::trace_scene(&content.objects, ro, rd, 100.0, u32::MAX);
        if !hit.is_hit() {
            return false;
        }
        let _ = shown;
        let w = world::world();
        let p = ro + rd * hit.t;
        let n = hit.normal;
        // The surface's own cell: just behind the point, along its normal.
        let cell = demos::leaf_of(&w.probe, p - n * 0.01);
        state.set("cellX", cell.x as f32);
        state.set("cellY", cell.y as f32);
        state.set("cellZ", cell.z as f32);
        true
    }

    fn selftest(&self, options: &SelftestOptions) -> anyhow::Result<bool> {
        selftest::run(options)
    }
}
