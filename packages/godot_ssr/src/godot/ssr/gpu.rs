//! Godot's SSR passes as the lab program (shaders/ssr_passes.wgsl): their
//! images, the uniform buffer of the camera's matrices, the passes with what
//! each binds, and the plan that dispatches them each frame in Godot's order
//! (SSEffects::screen_space_reflection, ss_effects.cpp:1467-1722), the half-size
//! path (rendering/environment/screen_space_reflection/half_size, default on).

use glam::{Mat4, Vec4};
use lab::investigation::{Element, ProgramCommand, ProgramFrame};

/// The best-fit normal table's size (best_fit_normal_texture, render_forward_clustered.cpp:5171-5176).
pub const BEST_FIT_SIZE: u32 = 1024;

/// The SSR UBO (ScreenSpaceReflectionSceneData): projection, its inverse, the
/// reprojection, and the eye offset, one view.
pub const SCENE_WORDS: usize = 16 * 3 + 4;

/// The functions shading.toml names.
pub static HOOKS: &[(&str, lab::investigation::ShadingHook)] = &[(
    "plan",
    lab::investigation::ShadingHook::Program {
        init: || Box::new(Plan::default()),
        plan: |state, frame| {
            state
                .downcast_mut::<Plan>()
                .expect("the SSR plan")
                .frame(frame)
        },
    },
)];

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
    ProgramCommand::Dispatch {
        pass,
        instance,
        groups: threads(size),
        params: params(words),
    }
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
    let c = state
        .camera
        .camera_data([viewport[0] as f32, viewport[1] as f32]);
    (
        godot_projection(Mat4::from_cols_array_2d(&c.view_to_clip)),
        Mat4::from_cols_array_2d(&c.world_to_view),
    )
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
            out.push(dispatch(
                "ssrBestFit",
                0,
                [BEST_FIT_SIZE, BEST_FIT_SIZE],
                &[],
            ));
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
        out.push(ProgramCommand::Upload {
            buffer: Element::of("ssrScene", 0),
            offset: 0,
            words,
        });
        let viewport = f.viewport;
        let size = ssr_size(viewport);
        let mips = mipmaps(size);
        // "SSR Downsample": odd when the full size is (the source depth's size).
        out.push(dispatch(
            "ssrDownsample",
            0,
            size,
            &[size[0], size[1], viewport[0] & 1, viewport[1] & 1],
        ));
        // "SSR HI-Z": level m from m - 1, odd when level m - 1 is.
        for m in 1..mips {
            let dest = level_size(size, m);
            let parent = level_size(size, m - 1);
            out.push(dispatch(
                "ssrHiz",
                m,
                dest,
                &[dest[0], dest[1], parent[0] & 1, parent[1] & 1],
            ));
        }
        // "SSR Main".
        let steps = state.p("ssrMaxSteps").round().clamp(1.0, 512.0) as u32;
        out.push(dispatch(
            "ssrTrace",
            0,
            size,
            &[
                size[0],
                size[1],
                mips,
                steps,
                state.p("ssrFadeOut").max(0.0).to_bits(),
                state.p("ssrFadeIn").max(0.0).to_bits(),
                state.p("ssrDepthTolerance").to_bits(),
                0,
                0,
            ],
        ));
        // "SSR Roughness Filter".
        for m in 1..mips {
            let dest = level_size(size, m);
            out.push(dispatch("ssrFilter", m, dest, &[dest[0], dest[1], m]));
        }
        // "SSR Resolve", at the internal size.
        out.push(dispatch(
            "ssrResolve",
            0,
            viewport,
            &[viewport[0], viewport[1]],
        ));
        // "Copy Framebuffer for SSIL/SSR", after the scene pass.
        out.push(dispatch("ssrCopyLastFrame", 0, size, &[size[0], size[1]]));
        out
    }
}
