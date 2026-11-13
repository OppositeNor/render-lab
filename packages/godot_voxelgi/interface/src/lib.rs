//! The C interface of the godot_voxelgi package's library (src/exports.rs
//! exports it): Godot's area-light and omni-light falloff (area_lights_inc.glsl,
//! `get_omni_attenuation`), which the package's CPU mirror ports and other
//! Godot packages share.
//!
//! An importer imports `godot_voxelgi` in its manifest, names `LTC_DIFFUSE`
//! and `OMNI_ATTENUATION` in its `negotiate!` (`imports [...]`), and calls
//! `ltc_diffuse` and `omni_attenuation`. Each importing library has its own
//! copy of these statics, resolved by its own negotiation.

use glam::Vec3;
use lab::plugin::abi::{self, Import, RlBuffer, RlStatus};

/// The package that exports this interface.
pub const PACKAGE: &str = "godot_voxelgi";

lab::c_struct! {
    /// A light's quad seen from a point: the normal there, and the quad's
    /// corners relative to the point, in order around it.
    pub struct LtcQuad {
        pub normal: [f32; 3],
        pub points: [[f32; 3]; 4],
    }
}

/// `godot_voxelgi_ltc_diffuse_v1`: writes `ltc_evaluate_diff`'s integral over the quad.
pub type LtcDiffuseV1 = unsafe extern "C" fn(*const LtcQuad, *mut f32, *mut RlBuffer) -> RlStatus;

/// `godot_voxelgi_omni_attenuation_v1`: writes `get_omni_attenuation(distance, inv_range, decay)`.
pub type OmniAttenuationV1 =
    unsafe extern "C" fn(f32, f32, f32, *mut f32, *mut RlBuffer) -> RlStatus;

pub static LTC_DIFFUSE: Import<LtcDiffuseV1> = Import::new(PACKAGE, "ltc_diffuse_v1");
pub static OMNI_ATTENUATION: Import<OmniAttenuationV1> =
    Import::new(PACKAGE, "omni_attenuation_v1");

/// `ltc_evaluate_diff` with no projector texture: the integral over the
/// light's quad (`points`, relative to the point lit), clipped to the horizon
/// of `normal`, of the cosine to `normal`, not divided by 2 pi.
pub fn ltc_diffuse(normal: Vec3, points: [Vec3; 4]) -> f32 {
    let quad = LtcQuad {
        normal: normal.to_array(),
        points: points.map(|p| p.to_array()),
    };
    let mut out = 0.0;
    // SAFETY: the exporter's function, resolved by the negotiation, with a quad and a float of the caller's.
    abi::call(|message| unsafe { LTC_DIFFUSE.get()(&quad, &mut out, message) })
        .unwrap_or_else(|e| panic!("godot_voxelgi_ltc_diffuse_v1: {e}"));
    out
}

/// `get_omni_attenuation`: (1 - (d/r)^4)^2 * d^-decay.
pub fn omni_attenuation(distance: f32, inv_range: f32, decay: f32) -> f32 {
    let mut out = 0.0;
    // SAFETY: as `ltc_diffuse`'s.
    abi::call(|message| unsafe {
        OMNI_ATTENUATION.get()(distance, inv_range, decay, &mut out, message)
    })
    .unwrap_or_else(|e| panic!("godot_voxelgi_omni_attenuation_v1: {e}"));
    out
}
