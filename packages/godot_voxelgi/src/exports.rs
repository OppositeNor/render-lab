//! The C functions this package's library exports to other packages' (the
//! lab's src/plugin/abi.rs), as its interface crate (interface/) declares
//! them: the CPU mirror's area-light integral and omni-light falloff.

use crate::godot::voxelgi::light;
use glam::Vec3;
use godot_voxelgi_interface::LtcQuad;

lab::export! {
    /// `light::ltc_diffuse` of the quad, written to `out`.
    pub fn godot_voxelgi_ltc_diffuse_v1(quad: *const LtcQuad, out: *mut f32) -> Result<(), String> {
        // SAFETY: the caller's quad, or null.
        let quad = unsafe { quad.as_ref() }.ok_or("no quad")?;
        let points = quad.points.map(Vec3::from_array);
        // SAFETY: the caller's float.
        unsafe { *out = light::ltc_diffuse(Vec3::from_array(quad.normal), points) };
        Ok(())
    }

    /// `light::omni_attenuation`, written to `out`.
    pub fn godot_voxelgi_omni_attenuation_v1(distance: f32, inv_range: f32, decay: f32, out: *mut f32) -> Result<(), String> {
        // SAFETY: the caller's float.
        unsafe { *out = light::omni_attenuation(distance, inv_range, decay) };
        Ok(())
    }
}
