//! ReSTIR's passes as the lab program (shaders/restir_passes.wgsl): the plan
//! that dispatches them each frame in the order of the paper's Alg. 5 (148:6):
//! candidates, visibility, temporal reuse, n rounds of spatial reuse, the
//! frame's reservoirs kept, and the shading; with the cameras the passes
//! reproject with, the frame's seed, and the count of frames averaged.

use glam::Mat4;
use lab::investigation::{Element, ProgramCommand, ProgramFrame};
use lab::shot::ShotState;

/// The cameras' uniform (RestirScene): clip-to-world, last frame's world-to-clip, last frame's eye.
pub const SCENE_WORDS: usize = 16 * 2 + 4;

/// The functions shading.toml names.
pub static HOOKS: &[(&str, lab::investigation::ShadingHook)] = &[(
    "plan",
    lab::investigation::ShadingHook::Program {
        init: || Box::new(Plan::default()),
        plan: |state, frame| {
            state
                .downcast_mut::<Plan>()
                .expect("the ReSTIR plan")
                .frame(frame)
        },
    },
)];

/// The values a frame's image depends on: while none of them changes, the frames are averaged.
const STILL_KEYS: &[&str] = &[
    "restirCandidates",
    "restirTemporal",
    "restirSpatialPasses",
    "restirNeighbours",
    "restirRadius",
    "restirVisibilityReuse",
    "restirUnbiased",
    "restirGeometryTest",
    "restirTemporalClamp",
    "restirView",
    "restirStage",
    "lightCount",
    "lightPhase",
    "lightIntensity",
];

/// What the plan keeps from frame to frame: the frame's number (the seed of its random numbers), last frame's
/// camera, and what the averaged frames were drawn with and how many there are.
#[derive(Default)]
pub struct Plan {
    frame: u32,
    last: Option<(Mat4, [f32; 3])>,
    still: Option<Vec<u32>>,
    averaged: u32,
}

fn params(words: &[u32]) -> [u32; 16] {
    let mut p = [0u32; 16];
    p[..words.len()].copy_from_slice(words);
    p
}

fn dispatch(pass: &'static str, size: [u32; 2], words: &[u32]) -> ProgramCommand {
    ProgramCommand::Dispatch {
        pass,
        instance: 0,
        groups: [size[0].div_ceil(8), size[1].div_ceil(8), 1],
        params: params(words),
    }
}

/// The camera a frame of `state` is drawn with at `viewport`: world-to-clip, and the eye.
pub fn camera(state: &ShotState, viewport: [u32; 2]) -> (Mat4, [f32; 3]) {
    let c = state
        .camera
        .camera_data([viewport[0] as f32, viewport[1] as f32]);
    (Mat4::from_cols_array_2d(&c.world_to_clip), c.position)
}

/// The spatial rounds in order: the first reads the temporal pass's reservoirs into resA, then they alternate
/// between resA and resB; and where the frame's reservoirs end up (0 temporal, 1 resA, 2 resB).
pub fn spatial_rounds(n: u32) -> (Vec<&'static str>, u32) {
    let mut rounds = Vec::new();
    for i in 0..n {
        // Round one from the temporal image, then each round from the image the one before wrote.
        rounds.push(match i {
            0 => "restirSpatialTA",
            i if i % 2 == 1 => "restirSpatialAB",
            _ => "restirSpatialBA",
        });
    }
    let last = match n {
        0 => 0,
        n if n % 2 == 1 => 1,
        _ => 2,
    };
    (rounds, last)
}

/// What a frame of `state` is drawn with, for the averaging: its camera and the values the image depends on.
fn signature(state: &ShotState, camera: Mat4) -> Vec<u32> {
    let mut out: Vec<u32> = camera.to_cols_array().iter().map(|x| x.to_bits()).collect();
    out.extend(STILL_KEYS.iter().map(|k| state.p(k).to_bits()));
    out
}

impl Plan {
    fn frame(&mut self, f: &ProgramFrame) -> Vec<ProgramCommand> {
        let state = f.state;
        let size = f.viewport;
        let current = camera(state, size);
        let mut out = Vec::new();
        // A frame that stands alone starts from nothing: no last frame, and the first seed.
        if !f.history {
            for image in ["prev", "prevP", "prevM", "prevA"] {
                out.push(ProgramCommand::Clear(Element::all(image)));
            }
            self.last = None;
            self.frame = 0;
            self.still = None;
        }
        let has_last = self.last.is_some() as u32;
        let last = self.last.unwrap_or(current);
        self.last = Some(current);
        // A fixed seed when the shot names one (a pixel's candidates followed), else the frame's number.
        let seed = if state.p("restirSeed") >= 0.0 {
            state.p("restirSeed") as u32
        } else {
            self.frame
        };
        self.frame = self.frame.wrapping_add(1);
        // The averaging goes on while nothing changes, and starts again when anything does.
        let sig = signature(state, current.0);
        self.averaged = if self.still.as_ref() == Some(&sig) && state.p("restirAccumulate") > 0.5 {
            self.averaged.saturating_add(1)
        } else {
            0
        };
        self.still = Some(sig);
        let mut words: Vec<u32> = Vec::with_capacity(SCENE_WORDS);
        for m in [current.0.inverse(), last.0] {
            // The uniform: this frame's clip-to-world, last frame's world-to-clip, column by column, then its eye.
            words.extend(m.to_cols_array().iter().map(|x| x.to_bits()));
        }
        words.extend(last.1.iter().map(|x| x.to_bits()));
        words.push(0);
        out.push(ProgramCommand::Upload {
            buffer: Element::of("restirScene", 0),
            offset: 0,
            words,
        });
        let base = [size[0], size[1], seed];
        out.push(dispatch("restirCandidates", size, &base));
        out.push(dispatch("restirVisibility", size, &base));
        out.push(dispatch(
            "restirTemporal",
            size,
            &[size[0], size[1], seed, has_last],
        ));
        let n = state.p("restirSpatialPasses").round().clamp(0.0, 4.0) as u32;
        let (rounds, last_image) = spatial_rounds(n);
        for (i, pass) in rounds.into_iter().enumerate() {
            out.push(dispatch(pass, size, &[size[0], size[1], seed, i as u32]));
        }
        out.push(dispatch(
            "restirKeep",
            size,
            &[size[0], size[1], seed, last_image],
        ));
        out.push(dispatch(
            "restirShade",
            size,
            &[size[0], size[1], seed, self.averaged],
        ));
        out
    }
}
