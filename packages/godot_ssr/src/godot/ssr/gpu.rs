//! Godot's SSR passes as the lab program (shaders/ssr_passes.wgsl): their
//! images, the uniform buffer of the camera's matrices, the passes with what
//! each binds, and the plan that dispatches them each frame in Godot's order
//! (SSEffects::screen_space_reflection, ss_effects.cpp:1467-1722), the half-size
//! path (rendering/environment/screen_space_reflection/half_size, default on).

use glam::{Mat4, Vec4};
use lab::investigation::{
    Access::{self, Load, Sampled, Sampler, Uniform, Write},
    Element, ImageKind, PassKind, ProgramBuffer, ProgramCommand, ProgramFrame, ProgramImage, ProgramPass, ProgramStage, ProgramUse, ShadingProgram,
    Which::{self, First, Mip},
};
use wgpu::TextureFormat as F;

/// The levels one pass may write (a bind group per level): more than any view needs.
pub const LEVELS: u32 = 16;

/// The best-fit normal table's size (best_fit_normal_texture, render_forward_clustered.cpp:5171-5176).
pub const BEST_FIT_SIZE: u32 = 1024;

const fn image(name: &'static str, format: F, mips: u32, viewport: u32) -> ProgramImage {
    ProgramImage { name, kind: ImageKind::D2, size: [BEST_FIT_SIZE, BEST_FIT_SIZE, 1], format, count: 1, mips, viewport }
}

/// The SSR scope's textures (ssr_allocate_buffers, ss_effects.cpp:1428-1465), at
/// half the view's size, and the last frame (allocate_last_frame_buffer,
/// ss_effects.cpp:366-394: half size while SSIL is off).
pub static IMAGES: &[ProgramImage] = &[
    image("bestFit", F::R8Unorm, 1, 0),
    // RB_HIZ: R32_SFLOAT, the full chain.
    image("hiz", F::R32Float, u32::MAX, 2),
    // RB_NORMAL_ROUGHNESS (half size only): R8G8B8A8_UNORM.
    image("halfNormal", F::Rgba8Unorm, 1, 2),
    // RB_SSR: the base data format, R16G16B16A16_SFLOAT, the full chain.
    image("ssrColor", F::Rgba16Float, u32::MAX, 2),
    // RB_MIP_LEVEL: R8_UNORM, the blur level / 14.
    image("mipLevel", F::R8Unorm, 1, 2),
    // RB_FINAL (half size only): the internal size.
    image("final", F::Rgba16Float, 1, 1),
    // RB_LAST_FRAME: R16G16B16A16_SFLOAT.
    image("lastFrame", F::Rgba16Float, 1, 2),
];

/// The SSR UBO (ScreenSpaceReflectionSceneData): projection, its inverse, the
/// reprojection, and the eye offset, one view.
pub const SCENE_WORDS: usize = 16 * 3 + 4;

pub static BUFFERS: &[ProgramBuffer] = &[ProgramBuffer { name: "ssrScene", bytes: SCENE_WORDS as u64 * 4, indirect: false, count: 1 }];

const fn u(binding: u32, resource: &'static str, access: Access, which: Which) -> ProgramUse {
    ProgramUse { binding, resource, access, which }
}

const fn compute(name: &'static str, entry: &'static str, uses: &'static [ProgramUse], stage: ProgramStage, instances: u32) -> ProgramPass {
    ProgramPass { name, kind: PassKind::Compute { entry, workgroup: [8, 8, 1] }, uses, stage, instances }
}

pub static PASSES: &[ProgramPass] = &[
    compute("ssrBestFit", "ssrBestFitCS", &[u(1, "bestFit", Write, First)], ProgramStage::Start, 1),
    compute(
        "ssrDownsample",
        "ssrDownsampleCS",
        &[u(1, "depth", Load, First), u(2, "normalRoughness", Load, First), u(3, "hiz", Write, Mip(0)), u(4, "halfNormal", Write, First)],
        ProgramStage::AfterPrepass,
        1,
    ),
    compute("ssrHiz", "ssrHizCS", &[u(1, "hiz", Load, Mip(-1)), u(2, "hiz", Write, Mip(0))], ProgramStage::AfterPrepass, LEVELS),
    compute(
        "ssrTrace",
        "ssrTraceCS",
        &[
            u(1, "lastFrame", Sampled, First),
            u(2, "hiz", Load, First),
            u(3, "halfNormal", Load, First),
            u(4, "ssrColor", Write, Mip(0)),
            u(5, "mipLevel", Write, First),
            u(6, "ssrScene", Uniform, First),
            u(7, "linearSampler", Sampler, First),
        ],
        ProgramStage::AfterPrepass,
        1,
    ),
    compute("ssrFilter", "ssrFilterCS", &[u(1, "ssrColor", Sampled, Mip(-1)), u(2, "ssrColor", Write, Mip(0)), u(3, "linearSampler", Sampler, First)], ProgramStage::AfterPrepass, LEVELS),
    compute(
        "ssrResolve",
        "ssrResolveCS",
        &[
            u(1, "depth", Load, First),
            u(2, "normalRoughness", Load, First),
            u(3, "hiz", Load, First),
            u(4, "halfNormal", Load, First),
            u(5, "ssrColor", Sampled, First),
            u(6, "mipLevel", Load, First),
            u(7, "final", Write, First),
            u(8, "trilinearSampler", Sampler, First),
        ],
        ProgramStage::AfterPrepass,
        1,
    ),
    compute("ssrCopyLastFrame", "ssrCopyLastFrameCS", &[u(1, "hdr", Load, First), u(2, "lastFrame", Write, First)], ProgramStage::AfterScene, 1),
];

pub static PROGRAM: ShadingProgram = ShadingProgram {
    images: IMAGES,
    buffers: BUFFERS,
    passes: PASSES,
    scene_uses: &[u(1, "final", Load, First), u(2, "bestFit", Load, First)],
    init: || Box::new(Plan::default()),
    plan: |state, frame| state.downcast_mut::<Plan>().expect("the SSR plan").frame(frame),
    // A frame drawn alone first draws the shot four times: the trace reads the
    // frame before it, so a reflection of a reflection needs one more frame.
    warmup: 4,
};

/// What the plan keeps from frame to frame: the camera of the frame before
/// (`ssr_last_frame_projections`, `ssr_last_frame_transform`), and whether the
/// best-fit table is made.
#[derive(Default)]
pub struct Plan {
    last: Option<(Mat4, Mat4)>,
    best_fit: bool,
}

fn params(words: &[u32]) -> [u32; 16] {
    let mut p = [0u32; 16];
    p[..words.len()].copy_from_slice(words);
    p
}

fn threads(size: [u32; 2]) -> [u32; 3] {
    [size[0].div_ceil(8), size[1].div_ceil(8), 1]
}

fn dispatch(pass: &'static str, instance: u32, size: [u32; 2], words: &[u32]) -> ProgramCommand {
    ProgramCommand::Dispatch { pass, instance, groups: threads(size), params: params(words) }
}

/// The SSR size for a view (`internal_size / 2`, at least one texel).
pub fn ssr_size(viewport: [u32; 2]) -> [u32; 2] {
    [(viewport[0] / 2).max(1), (viewport[1] / 2).max(1)]
}

/// Godot's mip count: halve both sides while both are above one (ss_effects.cpp:1436-1451).
pub fn mipmaps(size: [u32; 2]) -> u32 {
    let (mut w, mut h, mut m) = (size[0], size[1], 1);
    while w > 1 && h > 1 {
        w /= 2;
        h /= 2;
        m += 1;
    }
    m
}

/// Level `m`'s size.
pub fn level_size(size: [u32; 2], m: u32) -> [u32; 2] {
    [(size[0] >> m).max(1), (size[1] >> m).max(1)]
}

/// Godot's projection: the camera's, then the depth correction (y flipped; the
/// lab's depth is already reversed in [0, 1]), so screen uv = ndc * 0.5 + 0.5
/// with v down, as `compute_screen_pos` reads it.
pub fn godot_projection(view_to_clip: Mat4) -> Mat4 {
    Mat4::from_diagonal(Vec4::new(1.0, -1.0, 1.0, 1.0)) * view_to_clip
}

/// The camera a frame of `state` is drawn with, at `viewport`.
pub fn camera(state: &lab::shot::ShotState, viewport: [u32; 2]) -> (Mat4, Mat4) {
    let c = state.camera.camera_data([viewport[0] as f32, viewport[1] as f32]);
    (godot_projection(Mat4::from_cols_array_2d(&c.view_to_clip)), Mat4::from_cols_array_2d(&c.world_to_view))
}

/// The reprojection matrix (rfc.cpp:1498-1504): this frame's screen to view
/// space, to the world, to the last frame's view, to its screen.
pub fn reprojection(current: (Mat4, Mat4), last: (Mat4, Mat4)) -> Mat4 {
    last.0 * last.1 * current.1.inverse() * current.0.inverse()
}

impl Plan {
    fn frame(&mut self, f: &ProgramFrame) -> Vec<ProgramCommand> {
        let mut out = Vec::new();
        // Made once in Godot; here also on every frame without history, as a plan may be
        // replaced before it is recorded (a frame drawn alone is planned again for its warm-up).
        if !self.best_fit || !f.history {
            out.push(dispatch("ssrBestFit", 0, [BEST_FIT_SIZE, BEST_FIT_SIZE], &[]));
            self.best_fit = true;
        }
        let state = f.state;
        let current = camera(state, f.viewport);
        // A frame that stands alone starts from nothing: no last frame, its own camera.
        if !f.history {
            out.push(ProgramCommand::Clear(Element::all("lastFrame")));
            self.last = None;
        }
        let last = self.last.unwrap_or(current);
        self.last = Some(current);
        if state.p("ssrEnabled") < 0.5 {
            return out;
        }
        let mut words: Vec<u32> = Vec::with_capacity(SCENE_WORDS);
        for m in [current.0, current.0.inverse(), reprojection(current, last)] {
            words.extend(m.to_cols_array().iter().map(|x| x.to_bits()));
        }
        words.extend([0u32; 4]);
        out.push(ProgramCommand::Upload { buffer: Element::of("ssrScene", 0), offset: 0, words });
        let viewport = f.viewport;
        let size = ssr_size(viewport);
        let mips = mipmaps(size);
        // "SSR Downsample": odd when the full size is (the source depth's size).
        out.push(dispatch("ssrDownsample", 0, size, &[size[0], size[1], viewport[0] & 1, viewport[1] & 1]));
        // "SSR HI-Z": level m from m - 1, odd when level m - 1 is.
        for m in 1..mips {
            let dest = level_size(size, m);
            let parent = level_size(size, m - 1);
            out.push(dispatch("ssrHiz", m, dest, &[dest[0], dest[1], parent[0] & 1, parent[1] & 1]));
        }
        // "SSR Main".
        let steps = state.p("ssrMaxSteps").round().clamp(1.0, 512.0) as u32;
        out.push(dispatch("ssrTrace", 0, size, &[size[0], size[1], mips, steps, state.p("ssrFadeOut").max(0.0).to_bits(), state.p("ssrFadeIn").max(0.0).to_bits(), state.p("ssrDepthTolerance").to_bits(), 0, 0]));
        // "SSR Roughness Filter".
        for m in 1..mips {
            let dest = level_size(size, m);
            out.push(dispatch("ssrFilter", m, dest, &[dest[0], dest[1], m]));
        }
        // "SSR Resolve", at the internal size.
        out.push(dispatch("ssrResolve", 0, viewport, &[viewport[0], viewport[1]]));
        // "Copy Framebuffer for SSIL/SSR", after the scene pass.
        out.push(dispatch("ssrCopyLastFrame", 0, size, &[size[0], size[1]]));
        out
    }
}
