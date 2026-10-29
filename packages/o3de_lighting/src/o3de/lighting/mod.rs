//! The lighting model of O3DE's Atom renderer (O3DE 2.8.0, the commit in
//! source-index.json): how the StandardPBR material lights a pixel: direct
//! lighting from one simple point light (the metallic workflow, the GGX
//! specular BRDF and its D, G, and F, the Lambertian diffuse and its
//! response, the BRDF texture and multiscatter compensation, the light's
//! radius attenuation), then the global skylight's image-based lighting
//! (its prefiltered cubemaps, `ibl`, and the full-screen passes that apply it).
//!
//! The scene pass shades with shaders/standard_lighting.wgsl, a WGSL port of
//! O3DE's shader code (`SceneShading`); brdf.rs mirrors it on the CPU for the
//! readout and `selftest`. The demonstrations and parameters are in `shot`,
//! what each shot shows in `demos`, the dock in `controls`, the O3DE source
//! index in source-index.json (written by source_index.py), the
//! pseudo-code in pseudocode/, and the readout's formulas in `formulas`.

pub mod brdf;
pub mod cascades;
pub mod controls;
pub mod culling;
pub mod decals;
pub mod demos;
pub mod formulas;
pub mod ibl;
pub mod maps;
pub mod pseudocode;
pub mod reflections;
pub mod selftest;
pub mod shot;
pub mod ssao;
pub mod sources;
pub mod text;
pub mod transmission;
pub mod values;

use lab::camera::Projector;
use lab::controls::Dock;
use lab::demo::Demos;
use lab::frame::{FrameContent, Statics};
use crate::o3de::lighting::text::tr;
use lab::investigation::{Engine, Investigation, SceneShading, SelftestOptions, ShadingBuffer, ShadingPass, ShadingTarget, Wording};
use lab::pseudocode::Listing;
use lab::scene::{self, ids};
use lab::shot::{DemoId, PanelMode, ShotState};
use lab::sources::SourceIndex;
use glam::Vec2;
use shot::Demo;

pub struct Lighting;

pub static LIGHTING: Lighting = Lighting;

static WORDING: Wording = Wording {
    engine_short: "O3DE",
    engine_tag: "O3DE",
    checkout_dir: "o3de/",
};

/// O3DE's opaque passes, in the main pipeline's order (OpaqueParent.pass):
/// the forward pass into O3DE's targets (Forward.pass formats, where wgpu has
/// them; the lab adds a false-colour target for its views), then
/// DiffuseGlobalFullscreen, ReflectionGlobalFullscreen into its own
/// reflection target, ReflectionComposite, and DiffuseSpecularMerge. Compute
/// passes cannot blend, so each writes its sum to a new target.
const TARGETS: &[ShadingTarget] = &[
    ShadingTarget { name: "diffuse", format: wgpu::TextureFormat::Rgba16Float, divisor: 1 },
    ShadingTarget { name: "specular", format: wgpu::TextureFormat::Rgba16Float, divisor: 1 },
    ShadingTarget { name: "albedo", format: wgpu::TextureFormat::Rgba8Unorm, divisor: 1 },
    ShadingTarget { name: "specularF0", format: wgpu::TextureFormat::Rgba8Unorm, divisor: 1 },
    ShadingTarget { name: "normal", format: wgpu::TextureFormat::Rgb10a2Unorm, divisor: 1 },
    ShadingTarget { name: "debug", format: wgpu::TextureFormat::Rgba16Float, divisor: 1 },
    // The stencil reference each sample was drawn with (RenderCommon.h StencilRefs), / 255.
    ShadingTarget { name: "stencil", format: wgpu::TextureFormat::R8Unorm, divisor: 1 },
    ShadingTarget { name: "diffuseLit", format: wgpu::TextureFormat::Rgba16Float, divisor: 1 },
    ShadingTarget { name: "reflection", format: wgpu::TextureFormat::Rgba16Float, divisor: 1 },
    ShadingTarget { name: "specularLit", format: wgpu::TextureFormat::Rgba16Float, divisor: 1 },
    // The probe stencil (RGB) and blend-weight (A) views.
    ShadingTarget { name: "probeView", format: wgpu::TextureFormat::Rgba16Float, divisor: 1 },
];

/// Light culling (LightCullingParent.pass), after the depth pre-pass and
/// before the forward pass: one workgroup per 16 x 16 tile.
const EARLY: &[ShadingPass] = &[
    ShadingPass { entry: "o3deLightCullingTilePrepareCS", reads: &["depth"], writes: &["tileData"], workgroup: [16, 16] },
    ShadingPass { entry: "o3deLightCullingCS", reads: &[], writes: &["lightList", "tileData"], workgroup: [16, 16] },
    ShadingPass { entry: "o3deLightCullingRemapCS", reads: &[], writes: &["lightListRemapped", "lightList", "tileData"], workgroup: [16, 16] },
];

/// Per 16 x 16 tile: TileLightData, the light list, and the remapped list (8 bins of 256).
const BUFFERS: &[ShadingBuffer] = &[
    ShadingBuffer { name: "tileData", words_per_tile: 4, tile: 16 },
    ShadingBuffer { name: "lightList", words_per_tile: 256, tile: 16 },
    ShadingBuffer { name: "lightListRemapped", words_per_tile: 8 * 256, tile: 16 },
    // SSAO's stages, one f32 per pixel: the linear depth (R32_FLOAT), the
    // half-res depth (R32_FLOAT), SsaoCompute's output, the two blurs, and the
    // upsampled AO (R8_UNORM, rounded to 8 bits).
    ShadingBuffer { name: "linearDepth", words_per_tile: 1, tile: 1 },
    ShadingBuffer { name: "halfDepth", words_per_tile: 1, tile: 2 },
    ShadingBuffer { name: "ssao", words_per_tile: 1, tile: 2 },
    ShadingBuffer { name: "ssaoBlurV", words_per_tile: 1, tile: 2 },
    ShadingBuffer { name: "ssaoBlur", words_per_tile: 1, tile: 2 },
    ShadingBuffer { name: "ssaoFull", words_per_tile: 1, tile: 1 },
];

const PASSES: &[ShadingPass] = &[
    ShadingPass { entry: "o3deDiffuseGlobalCS", reads: &["diffuse", "normal", "albedo", "stencil"], writes: &["diffuseLit"], workgroup: [8, 8] },
    ShadingPass { entry: "o3deReflectionsCS", reads: &["normal", "albedo", "specularF0", "stencil", "depth"], writes: &["reflection", "probeView"], workgroup: [8, 8] },
    ShadingPass { entry: "o3deReflectionCompositeCS", reads: &["specular", "reflection", "depth"], writes: &["specularLit"], workgroup: [8, 8] },
    // AoParent.pass (SsaoParent.pass), after the MSAA resolves; ModulateWithSsao is in the merge.
    ShadingPass { entry: "o3deLinearDepthCS", reads: &["depth"], writes: &["linearDepth"], workgroup: [16, 16] },
    ShadingPass { entry: "o3deDepthDownsampleCS", reads: &["depth"], writes: &["halfDepth", "linearDepth"], workgroup: [16, 16] },
    ShadingPass { entry: "o3deSsaoCS", reads: &["depth"], writes: &["ssao", "halfDepth"], workgroup: [16, 16] },
    ShadingPass { entry: "o3deSsaoBlurVerticalCS", reads: &["depth"], writes: &["ssaoBlurV", "ssao", "halfDepth"], workgroup: [16, 16] },
    ShadingPass { entry: "o3deSsaoBlurHorizontalCS", reads: &["depth"], writes: &["ssaoBlur", "ssaoBlurV", "ssao", "halfDepth"], workgroup: [16, 16] },
    ShadingPass { entry: "o3deSsaoUpsampleCS", reads: &["depth"], writes: &["ssaoFull", "ssaoBlur", "halfDepth", "linearDepth"], workgroup: [16, 16] },
    ShadingPass { entry: "o3deMergeCS", reads: &["diffuse", "specular", "debug", "diffuseLit", "specularLit", "reflection", "probeView", "depth"], writes: &["hdr", "ssaoFull"], workgroup: [8, 8] },
];

pub static SHADING: SceneShading = SceneShading {
    module: "o3de_lighting.shader/standard-lighting",
    fragment: "o3deForwardFS",
    data: ibl::shading_data,
    verify: &["o3deBrdfTests", "o3dePixelTests", "o3deBrdfMapTests", "o3deIblTests", "o3deResolveTests"],
    targets: TARGETS,
    outputs: &["diffuse", "specular", "albedo", "specularF0", "normal", "debug", "stencil"],
    passes: PASSES,
    prepass: Some("o3dePrepassFS"),
    early: EARLY,
    buffers: BUFFERS,
    scene_reads: &["tileData", "lightListRemapped"],
};

impl Investigation for Lighting {
    fn engine(&self) -> &'static Engine {
        &crate::o3de::ENGINE
    }

    fn id(&self) -> &'static str {
        "lighting"
    }

    fn title(&self) -> &'static str {
        tr("investigation.lighting_model")
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
        sources::index()
    }

    fn listings(&self) -> &'static [Listing] {
        pseudocode::all()
    }

    fn package(&self) -> Option<&'static str> {
        Some("o3de_lighting")
    }

    fn demo_specs(&self) -> Option<&'static Demos> {
        Some(shot::demos())
    }

    fn text(&self, key: &'static str) -> &'static str {
        text::tr_key(key)
    }

    fn backdrop(&self, t: f32) -> Option<ShotState> {
        // The room with the light circling it.
        let mut state = shot::for_demo(Demo::Room);
        state.panel = PanelMode::Hidden;
        state.set("labels", 0.0);
        for (name, value) in shot::demo_animation(Demo::Room, t) {
            state.set(name, value);
        }
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

    fn choices(&self) -> &'static [&'static lab::values::Choices] {
        use lab::values::Choice;
        static CHOICES: std::sync::OnceLock<[&'static lab::values::Choices; 2]> = std::sync::OnceLock::new();
        CHOICES.get_or_init(|| [brdf::ViewMode::choices(), brdf::DiffuseModel::choices()])
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

    /// A click on a surface makes its point the inspected one.
    fn click(&self, _shown: &ShotState, state: &mut ShotState, content: &FrameContent, _statics: &Statics, projector: &Projector, pixel: Vec2) -> bool {
        let (origin, dir) = projector.ray(pixel);
        let hit = scene::trace_scene(&content.objects, origin, dir, 100.0, ids::PANEL as u32);
        if !hit.is_hit() {
            return false;
        }
        let p = origin + dir * hit.t;
        let n = if hit.normal.dot(dir) > 0.0 { -hit.normal } else { hit.normal };
        for (name, value) in [("inspectX", p.x), ("inspectY", p.y), ("inspectZ", p.z), ("inspectNX", n.x), ("inspectNY", n.y), ("inspectNZ", n.z), ("inspectObject", hit.object as f32)] {
            state.set(name, value);
        }
        true
    }

    fn selftest(&self, options: &SelftestOptions) -> anyhow::Result<bool> {
        selftest::run(options)
    }
}
