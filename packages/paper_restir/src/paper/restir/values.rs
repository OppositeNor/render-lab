//! The investigation's values: every number a demonstration, the dock, a
//! timeline, or a story sets, its default, and what it is, as a struct the lab
//! reaches by key (`#[derive(Values)]`, lab/src/values.rs). A field's `label`
//! names the dock group that shows it (controls.rs). The method's parameters
//! (restirCandidates, restirTemporal, restirSpatialPasses, …) are declared by
//! their tags in shaders/restir_shading.wgsl.

use lab::palette;

#[derive(Clone, Debug, PartialEq, lab::Values, serde::Serialize, serde::Deserialize)]
#[values(package = "paper_restir", rename_all = "camelCase")]
#[serde(rename_all = "camelCase")]
pub struct RestirValues {
    /// How many of the room's lights are on (lights.rs `place`).
    #[value(default = 64.0, range = 1.0..=64.0, unit = index, text = "controls-light_count", color = palette::THEME_ACCENT, label(lights), help = "help-lightCount")]
    pub light_count: f32,
    /// Where the lights are along their loops, 0 to 1 (lights.rs `place`): the paper's lights are dynamic.
    #[value(default = 0.0, range = 0.0..=1.0, unit = plain, text = "controls-light_phase", color = palette::THEME_ACCENT, label(lights), help = "help-lightPhase")]
    pub light_phase: f32,
    /// A scale on every light's radiance.
    #[value(default = 1.0, range = 0.0..=4.0, unit = plain, text = "controls-light_intensity", color = palette::THEME_ACCENT, label(lights), help = "help-lightIntensity")]
    pub light_intensity: f32,
    /// P, the pixel the per-pixel demonstrations follow, as a fraction of the viewport's width.
    #[value(default = 0.88, range = 0.0..=1.0, unit = plain, text = "controls-pick_u", color = palette::SELECTED, label(pick), help = "help-pickU")]
    pub pick_u: f32,
    /// P, as a fraction of the viewport's height, down.
    #[value(default = 0.56, range = 0.0..=1.0, unit = plain, text = "controls-pick_v", color = palette::SELECTED, label(pick), help = "help-pickV")]
    pub pick_v: f32,
    /// How many of P's candidates have been streamed into its reservoir (Alg. 2-3), from none to M.
    #[value(default = 32.0, range = 0.0..=64.0, unit = index, text = "controls-ris_step", color = palette::SELECTED, label(pick), help = "help-risStep")]
    pub ris_step: f32,
    /// How P's candidate lines show their weights (ris.rs): 0 all alike, 1 each as bright as its weight w = p̂ / p.
    #[value(default = 1.0, range = 0.0..=1.0, unit = plain, text = "controls-ris_weigh", color = palette::SELECTED, label(pick), help = "help-risWeigh")]
    pub ris_weigh: f32,
    /// A marker along P's streamed candidates laid end to end by weight (ris.rs): at 0 off, between 0 and 1 the
    /// candidate under it lit, so each is lit for a share of the sweep equal to its share of the weight sum; at 1 the
    /// kept one.
    #[value(default = 0.0, range = 0.0..=1.0, unit = plain, text = "controls-ris_pick", color = palette::SELECTED, label(pick), help = "help-risPick")]
    pub ris_pick: f32,
    /// The seed of the frame's random numbers; below 0, a new one each frame (the frame's number). A fixed seed
    /// draws the same candidates every frame, so a pixel's can be followed.
    #[value(default = -1.0, range = -1.0..=1000.0, unit = index, text = "controls-seed", color = palette::THEME_ACCENT, label(pick), help = "help-restirSeed")]
    pub restir_seed: f32,
    /// How many of P's k neighbours its first spatial round has combined so far (spatial.rs), in order; 16 is all.
    #[value(default = 16.0, range = 0.0..=16.0, unit = index, text = "controls-spatial_step", color = palette::SELECTED, label(pick), help = "help-spatialStep")]
    pub spatial_step: f32,
    /// The 1D example's weight m (Fig. 7): 0 the biased 1/M, 1 the naive 1/|Z|, 2 the balance heuristic (Eq. 22).
    #[value(default = 0.0, range = 0.0..=2.0, unit = index, text = "controls-bias_weight", color = palette::SELECTED, label(bias), help = "help-biasWeight")]
    pub bias_weight: f32,
    /// The 1D example's candidates, half from each PDF (Fig. 7 shows 2, 4, 10, and 20).
    #[value(default = 4.0, range = 2.0..=20.0, unit = index, text = "controls-bias_candidates", color = palette::SELECTED, label(bias), help = "help-biasCandidates")]
    pub bias_candidates: f32,
    /// How much of the 1D plot is drawn (bias.rs), each curve sweeping in from the left as it passes the next whole
    /// number: 1 the target p̂, 2 the flat PDF, 3 the step PDF, 4 the true inverse PDF, 5 the mean of W.
    #[value(default = 5.0, range = 0.0..=5.0, unit = plain, text = "controls-bias_reveal", color = palette::SELECTED, label(bias), help = "help-biasReveal")]
    pub bias_reveal: f32,
    /// A marker across the 1D plot at this place, 0 to 1 (bias.rs); below 0, none.
    #[value(default = -1.0, range = -1.0..=1.0, unit = plain, text = "controls-bias_mark", color = palette::SELECTED, label(bias), help = "help-biasMark")]
    pub bias_mark: f32,
    /// Whether the step PDF p₂ is nearly zero past one half instead of zero (Fig. 7c).
    #[value(default = 0.0, range = 0.0..=1.0, unit = plain, text = "controls-bias_near_zero", color = palette::SELECTED, label(bias), help = "help-biasNearZero")]
    pub bias_near_zero: f32,
}
