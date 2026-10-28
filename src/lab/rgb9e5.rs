//! RGB9E5, EXT_texture_shared_exponent's encoding: nine-bit mantissas under
//! one five-bit exponent (bias 15). The environment's texels are kept in it,
//! the format of the GPU texture (gEnvironment). The resource pipeline
//! (pipeline/radiance.rs) encodes with this file, and environment.rs's tests
//! check it against `decode_rgb9e5`.

use glam::Vec3;

pub fn encode_rgb9e5(c: Vec3) -> u32 {
    let c = c.clamp(Vec3::ZERO, Vec3::splat(65408.0));
    let max = c.max_element();
    if max < 2f32.powi(-24) {
        return 0;
    }
    // floor(log2(max)), from the float's exponent bits.
    let log2 = ((max.to_bits() >> 23) & 0xff) as i32 - 127;
    let mut exponent = log2.max(-16) + 16;
    let mut step = 2f32.powi(exponent - 24);
    if (max / step + 0.5).floor() >= 512.0 {
        step *= 2.0;
        exponent += 1;
    }
    let m = (c / step + 0.5).floor();
    m.x as u32 | (m.y as u32) << 9 | (m.z as u32) << 18 | (exponent as u32) << 27
}
