//! Texel formats of the Surface Cache atlases, packed on the CPU exactly as
//! the GPU reads them back.
//!
//! PF_FloatR11G11B10 (wgpu `Rg11b10Ufloat`) stores three unsigned floats
//! with a 5-bit exponent (bias 15) and 6, 6, and 5 mantissa bits: red in
//! bits 0..11, green in 11..22, blue in 22..32.

use glam::Vec3;

/// One unsigned small float with `mantissa` bits, rounded to nearest.
fn small_float(x: f32, mantissa: u32) -> u32 {
    let max_code = (30u32 << mantissa) | ((1 << mantissa) - 1);
    if !(x > 0.0) {
        return 0;
    }
    let bits = x.to_bits();
    let exponent = ((bits >> 23) & 0xff) as i32 - 127 + 15;
    if exponent <= 0 {
        // Denormal: value = m / 2^mantissa * 2^-14.
        let m = (x * 2f32.powi(14) * (1u32 << mantissa) as f32).round() as u32;
        return m.min(max_code);
    }
    let shift = 23 - mantissa;
    let m = bits & 0x7f_ffff;
    let mut code = ((exponent as u32) << mantissa) | (m >> shift);
    // Round half up; a carry moves into the exponent, which is still correct.
    code += (m >> (shift - 1)) & 1;
    code.min(max_code)
}

fn small_float_value(code: u32, mantissa: u32) -> f32 {
    let exponent = code >> mantissa;
    let m = (code & ((1 << mantissa) - 1)) as f32 / (1u32 << mantissa) as f32;
    if exponent == 0 {
        m * 2f32.powi(-14)
    } else {
        (1.0 + m) * 2f32.powi(exponent as i32 - 15)
    }
}

pub fn pack_r11g11b10(c: Vec3) -> u32 {
    small_float(c.x, 6) | (small_float(c.y, 6) << 11) | (small_float(c.z, 5) << 22)
}

pub fn unpack_r11g11b10(v: u32) -> Vec3 {
    Vec3::new(small_float_value(v & 0x7ff, 6), small_float_value((v >> 11) & 0x7ff, 6), small_float_value(v >> 22, 5))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r11g11b10_round_trip() {
        for &x in &[0.0f32, 1e-6, 6.1e-5, 0.01, 0.5, 1.0, 1.7, 3.14159, 100.0, 60000.0] {
            let back = unpack_r11g11b10(pack_r11g11b10(Vec3::splat(x)));
            // 6 mantissa bits for red and green, 5 for blue: relative error ≤ 2^-7 and 2^-6.
            let tol = |bits: i32| (x * 2f32.powi(-bits)).max(2f32.powi(-20));
            assert!((back.x - x).abs() <= tol(7), "red {x} -> {}", back.x);
            assert!((back.z - x).abs() <= tol(6), "blue {x} -> {}", back.z);
        }
        assert_eq!(pack_r11g11b10(Vec3::new(1.0, 0.0, 0.0)), 15 << 6);
        assert_eq!(unpack_r11g11b10(pack_r11g11b10(Vec3::new(-1.0, f32::NAN, 1e9))), Vec3::new(0.0, 0.0, 64512.0));
    }
}
