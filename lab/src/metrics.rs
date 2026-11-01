//! Image metrics for experiments (src/experiment.rs): how far a rendered
//! image is from a reference, as rendering papers measure it.
//!
//! - MSE, RMSE and PSNR over the display image's RGB (0..1, sRGB-encoded,
//!   peak 1).
//! - Relative MSE over the linear HDR image: the mean of (a − b)² / (b² + ε),
//!   per channel, ε = 0.01 (the denoising literature's relMSE).
//! - SSIM: Wang, Bovik, Sheikh and Simoncelli, "Image Quality Assessment:
//!   From Error Visibility to Structural Similarity", IEEE TIP 13(4), 2004.
//!   On the display image's luma, with their 11 × 11 Gaussian window
//!   (σ = 1.5), K1 = 0.01, K2 = 0.03, L = 1, averaged over the pixels whose
//!   window lies inside the image.
//! - ꟻLIP (LDR): Andersson, Nilsson, Akenine-Möller, Oskarsson, Åström and
//!   Fairchild, "FLIP: A Difference Evaluator for Alternating Images", Proc.
//!   ACM CGIT 3(2), 2020: the colour pipeline (contrast sensitivity filters
//!   in YCxCz, Hunt-adjusted L*a*b*, HyAB distance, the paper's remapping
//!   with qc = 0.7, pc = 0.4, pt = 0.95) and the feature pipeline (edges and
//!   points from Gaussian derivatives of luminance, qf = 0.5), combined as
//!   ΔE_c^(1 − ΔE_f). Viewing conditions: the paper's default of 0.7 m from a
//!   0.7 m wide 3840-pixel monitor (about 67 pixels per degree).
//! - Mean luminance of the linear image (Rec. 709 weights).

/// An image of RGB pixels, row by row from the top.
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<[f32; 3]>,
}

impl Image {
    pub fn new(width: usize, height: usize, pixels: Vec<[f32; 3]>) -> Image {
        assert_eq!(pixels.len(), width * height, "an image of {width} × {height} has that many pixels");
        Image { width, height, pixels }
    }

    /// From RGBA8 bytes (sRGB-encoded): values 0..1, still encoded.
    pub fn from_rgba8(width: usize, height: usize, rgba: &[u8]) -> Image {
        Image::new(width, height, rgba.chunks_exact(4).map(|p| [p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0]).collect())
    }

    /// From RGBA half floats (a linear HDR target's texels).
    pub fn from_rgba16f(width: usize, height: usize, words: &[u16]) -> Image {
        Image::new(width, height, words.chunks_exact(4).map(|p| [f16_to_f32(p[0]), f16_to_f32(p[1]), f16_to_f32(p[2])]).collect())
    }

    /// RGBA8 bytes of the image, clamped to 0..1.
    pub fn to_rgba8(&self) -> Vec<u8> {
        self.pixels.iter().flat_map(|p| [byte(p[0]), byte(p[1]), byte(p[2]), 255]).collect()
    }
}

fn byte(x: f32) -> u8 {
    (x.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

/// An IEEE half float as f32.
pub fn f16_to_f32(h: u16) -> f32 {
    let sign = if h & 0x8000 != 0 { -1.0 } else { 1.0 };
    let exponent = ((h >> 10) & 0x1f) as i32;
    let mantissa = (h & 0x3ff) as f32;
    match exponent {
        0 => sign * mantissa * 2f32.powi(-24),
        31 if mantissa == 0.0 => sign * f32::INFINITY,
        31 => f32::NAN,
        e => sign * (1.0 + mantissa / 1024.0) * 2f32.powi(e - 15),
    }
}

/// How a test image compares with a reference.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize)]
pub struct Measurement {
    pub mse: f64,
    pub rmse: f64,
    /// Infinite for identical images.
    pub psnr: f64,
    pub ssim: f64,
    pub flip: f64,
    pub relmse: f64,
    /// Mean linear luminance of the reference and the test.
    pub luminance_reference: f64,
    pub luminance: f64,
}

/// Every metric of `test` against `reference`: display images (sRGB-encoded
/// 0..1) and, when given, their linear HDR images.
pub fn measure(reference: &Image, test: &Image, linear: Option<(&Image, &Image)>) -> Measurement {
    let mse = mse(reference, test);
    let (relmse, luminance_reference, luminance) = match linear {
        Some((r, t)) => (relmse(r, t), mean_luminance(r), mean_luminance(t)),
        None => (f64::NAN, f64::NAN, f64::NAN),
    };
    Measurement { mse, rmse: mse.sqrt(), psnr: psnr(mse), ssim: ssim(reference, test), flip: flip(reference, test, DEFAULT_PPD).0, relmse, luminance_reference, luminance }
}

fn same_size(a: &Image, b: &Image) {
    assert!(a.width == b.width && a.height == b.height, "images of one size ({}×{} and {}×{})", a.width, a.height, b.width, b.height);
}

/// Mean squared error over every channel.
pub fn mse(a: &Image, b: &Image) -> f64 {
    same_size(a, b);
    let sum: f64 = a.pixels.iter().zip(&b.pixels).map(|(p, q)| (0..3).map(|c| ((p[c] - q[c]) as f64).powi(2)).sum::<f64>()).sum();
    sum / (a.pixels.len() * 3).max(1) as f64
}

/// Peak signal-to-noise ratio in dB for a peak of 1.
pub fn psnr(mse: f64) -> f64 {
    if mse <= 0.0 { f64::INFINITY } else { 10.0 * (1.0 / mse).log10() }
}

/// Relative MSE of `test` against `reference` (linear images).
pub fn relmse(reference: &Image, test: &Image) -> f64 {
    same_size(reference, test);
    let sum: f64 = reference.pixels.iter().zip(&test.pixels).map(|(r, t)| (0..3).map(|c| ((t[c] - r[c]) as f64).powi(2) / ((r[c] as f64).powi(2) + 0.01)).sum::<f64>()).sum();
    sum / (reference.pixels.len() * 3).max(1) as f64
}

/// Mean Rec. 709 luminance.
pub fn mean_luminance(image: &Image) -> f64 {
    let sum: f64 = image.pixels.iter().map(|p| 0.2126 * p[0] as f64 + 0.7152 * p[1] as f64 + 0.0722 * p[2] as f64).sum();
    sum / image.pixels.len().max(1) as f64
}

/// A single-channel image, row by row.
#[derive(Clone)]
struct Plane {
    width: usize,
    height: usize,
    v: Vec<f64>,
}

impl Plane {
    fn map(&self, f: impl Fn(f64) -> f64) -> Plane {
        Plane { width: self.width, height: self.height, v: self.v.iter().map(|&x| f(x)).collect() }
    }

    fn zip(&self, other: &Plane, f: impl Fn(f64, f64) -> f64) -> Plane {
        Plane { width: self.width, height: self.height, v: self.v.iter().zip(&other.v).map(|(&a, &b)| f(a, b)).collect() }
    }

    /// Convolved with `kx` along rows, then `ky` along columns (odd lengths,
    /// centred), the edges clamped.
    fn separable(&self, kx: &[f64], ky: &[f64]) -> Plane {
        let (w, h) = (self.width, self.height);
        let (rx, ry) = (kx.len() as isize / 2, ky.len() as isize / 2);
        let mut rows = vec![0.0; w * h];
        for y in 0..h {
            for x in 0..w {
                let mut s = 0.0;
                for (i, k) in kx.iter().enumerate() {
                    let sx = (x as isize + i as isize - rx).clamp(0, w as isize - 1) as usize;
                    s += k * self.v[y * w + sx];
                }
                rows[y * w + x] = s;
            }
        }
        let mut out = vec![0.0; w * h];
        for y in 0..h {
            for x in 0..w {
                let mut s = 0.0;
                for (i, k) in ky.iter().enumerate() {
                    let sy = (y as isize + i as isize - ry).clamp(0, h as isize - 1) as usize;
                    s += k * rows[sy * w + x];
                }
                out[y * w + x] = s;
            }
        }
        Plane { width: w, height: h, v: out }
    }
}

/// Luma of a display image (Rec. 709 weights on the encoded values).
fn luma(image: &Image) -> Plane {
    Plane { width: image.width, height: image.height, v: image.pixels.iter().map(|p| 0.2126 * p[0] as f64 + 0.7152 * p[1] as f64 + 0.0722 * p[2] as f64).collect() }
}

/// Mean SSIM of two display images.
pub fn ssim(a: &Image, b: &Image) -> f64 {
    same_size(a, b);
    let (x, y) = (luma(a), luma(b));
    let window: Vec<f64> = {
        let g: Vec<f64> = (-5..=5).map(|i: i32| (-(i * i) as f64 / (2.0 * 1.5 * 1.5)).exp()).collect();
        let sum: f64 = g.iter().sum();
        g.iter().map(|v| v / sum).collect()
    };
    let blur = |p: &Plane| p.separable(&window, &window);
    let (mx, my) = (blur(&x), blur(&y));
    let sxx = blur(&x.zip(&x, |a, b| a * b));
    let syy = blur(&y.zip(&y, |a, b| a * b));
    let sxy = blur(&x.zip(&y, |a, b| a * b));
    let (c1, c2) = ((0.01f64).powi(2), (0.03f64).powi(2));
    let (w, h) = (x.width, x.height);
    let mut sum = 0.0;
    let mut n = 0usize;
    // The pixels whose whole window is inside the image (all of them for a tiny image).
    let (lo_x, hi_x, lo_y, hi_y) = if w > 10 && h > 10 { (5, w - 5, 5, h - 5) } else { (0, w, 0, h) };
    for py in lo_y..hi_y {
        for px in lo_x..hi_x {
            let i = py * w + px;
            let (ux, uy) = (mx.v[i], my.v[i]);
            let vx = sxx.v[i] - ux * ux;
            let vy = syy.v[i] - uy * uy;
            let cxy = sxy.v[i] - ux * uy;
            sum += ((2.0 * ux * uy + c1) * (2.0 * cxy + c2)) / ((ux * ux + uy * uy + c1) * (vx + vy + c2));
            n += 1;
        }
    }
    sum / n.max(1) as f64
}

/// Pixels per degree of the paper's default viewing conditions:
/// 0.7 m from a 0.7 m wide monitor of 3840 pixels.
pub const DEFAULT_PPD: f64 = 0.7 * (3840.0 / 0.7) * std::f64::consts::PI / 180.0;

fn srgb_to_linear(c: f64) -> f64 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

/// Linear sRGB to XYZ (D65).
fn xyz(rgb: [f64; 3]) -> [f64; 3] {
    [
        0.4124564 * rgb[0] + 0.3575761 * rgb[1] + 0.1804375 * rgb[2],
        0.2126729 * rgb[0] + 0.7151522 * rgb[1] + 0.0721750 * rgb[2],
        0.0193339 * rgb[0] + 0.1191920 * rgb[1] + 0.9503041 * rgb[2],
    ]
}

/// XYZ (D65) to linear sRGB.
fn rgb_of_xyz(x: [f64; 3]) -> [f64; 3] {
    [
        3.2404542 * x[0] - 1.5371385 * x[1] - 0.4985314 * x[2],
        -0.9692660 * x[0] + 1.8760108 * x[1] + 0.0415560 * x[2],
        0.0556434 * x[0] - 0.2040259 * x[1] + 1.0572252 * x[2],
    ]
}

/// The reference white: XYZ of linear RGB (1, 1, 1).
fn white() -> [f64; 3] {
    xyz([1.0, 1.0, 1.0])
}

fn ycxcz(x: [f64; 3]) -> [f64; 3] {
    let w = white();
    let (xn, yn, zn) = (x[0] / w[0], x[1] / w[1], x[2] / w[2]);
    [116.0 * yn - 16.0, 500.0 * (xn - yn), 200.0 * (yn - zn)]
}

fn xyz_of_ycxcz(c: [f64; 3]) -> [f64; 3] {
    let w = white();
    let yn = (c[0] + 16.0) / 116.0;
    let xn = c[1] / 500.0 + yn;
    let zn = yn - c[2] / 200.0;
    [xn * w[0], yn * w[1], zn * w[2]]
}

/// Hunt-adjusted CIELAB of XYZ: a* and b* scaled by 0.01 L*.
fn hunt_lab(x: [f64; 3]) -> [f64; 3] {
    let w = white();
    let f = |t: f64| if t > (6.0f64 / 29.0).powi(3) { t.cbrt() } else { t / (3.0 * (6.0f64 / 29.0).powi(2)) + 4.0 / 29.0 };
    let (fx, fy, fz) = (f(x[0] / w[0]), f(x[1] / w[1]), f(x[2] / w[2]));
    let l = 116.0 * fy - 16.0;
    [l, 0.01 * l * 500.0 * (fx - fy), 0.01 * l * 200.0 * (fy - fz)]
}

fn hyab(a: [f64; 3], b: [f64; 3]) -> f64 {
    (a[0] - b[0]).abs() + ((a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// One Gaussian term of a contrast sensitivity function along one axis,
/// exp(−π²x²/b) with x in degrees, at `radius` pixels each side.
fn csf_term(b: f64, ppd: f64, radius: isize) -> Vec<f64> {
    use std::f64::consts::PI;
    (-radius..=radius).map(|i| {
        let x = i as f64 / ppd;
        (-PI * PI * x * x / b).exp()
    }).collect()
}

/// The ꟻLIP error of `test` against `reference` (display images): its mean,
/// and the per-pixel error map (0..1).
pub fn flip(reference: &Image, test: &Image, ppd: f64) -> (f64, Vec<f32>) {
    same_size(reference, test);
    let (w, h) = (reference.width, reference.height);
    let (qc, qf, pc, pt) = (0.7, 0.5, 0.4, 0.95);
    let planes = |image: &Image| -> [Plane; 3] {
        let c: Vec<[f64; 3]> = image.pixels.iter().map(|p| ycxcz(xyz([srgb_to_linear(p[0] as f64), srgb_to_linear(p[1] as f64), srgb_to_linear(p[2] as f64)]))).collect();
        [0, 1, 2].map(|k| Plane { width: w, height: h, v: c.iter().map(|p| p[k]).collect() })
    };
    let (r, t) = (planes(reference), planes(test));

    // Colour: each opponent channel filtered by its contrast sensitivity
    // function (a sum of two Gaussians), normalized to sum 1 in 2D.
    let params = [(1.0, 0.0047, 0.0, 1e-5), (1.0, 0.0053, 0.0, 1e-5), (34.1, 0.04, 13.5, 0.025)];
    let radius = (3.0 * (0.04f64 / (2.0 * std::f64::consts::PI.powi(2))).sqrt() * ppd).ceil() as isize;
    // The 2D kernel a1·√(π/b1)·exp(−π²r²/b1) + a2·√(π/b2)·exp(−π²r²/b2) is
    // a sum of separable Gaussians, divided by its sum over the window.
    let filter = |p: &Plane, (a1, b1, a2, b2): (f64, f64, f64, f64)| -> Plane {
        use std::f64::consts::PI;
        let (g1, g2) = (csf_term(b1, ppd, radius), csf_term(b2, ppd, radius));
        let (w1, w2) = (a1 * (PI / b1).sqrt(), a2 * (PI / b2).sqrt());
        let total = w1 * g1.iter().sum::<f64>().powi(2) + w2 * g2.iter().sum::<f64>().powi(2);
        let f1 = p.separable(&g1, &g1);
        if a2 == 0.0 {
            return f1.map(|v| v * w1 / total);
        }
        let f2 = p.separable(&g2, &g2);
        f1.zip(&f2, |x, y| (w1 * x + w2 * y) / total)
    };
    let filtered = |c: &[Plane; 3]| -> Vec<[f64; 3]> {
        let f: Vec<Plane> = (0..3).map(|k| filter(&c[k], params[k])).collect();
        (0..w * h).map(|i| {
            let rgb = rgb_of_xyz(xyz_of_ycxcz([f[0].v[i], f[1].v[i], f[2].v[i]]));
            hunt_lab(xyz(rgb.map(|v| v.clamp(0.0, 1.0))))
        }).collect()
    };
    let (lr, lt) = (filtered(&r), filtered(&t));
    let cmax = hyab(hunt_lab(xyz([0.0, 1.0, 0.0])), hunt_lab(xyz([0.0, 0.0, 1.0]))).powf(qc);
    let pccmax = pc * cmax;
    let color: Vec<f64> = lr.iter().zip(&lt).map(|(a, b)| {
        let d = hyab(*a, *b).powf(qc);
        if d < pccmax { d * pt / pccmax } else { pt + (d - pccmax) / (cmax - pccmax) * (1.0 - pt) }
    }).collect();

    // Features: edges and points of the normalized luminance, from the first
    // and second derivatives of a Gaussian of σ = 0.5 · 0.082 · ppd (each
    // kernel's positive and negative weights normalized to sum to ±1).
    let sd = 0.5 * 0.082 * ppd;
    let fr = (3.0 * sd).ceil() as isize;
    let g: Vec<f64> = (-fr..=fr).map(|i| (-((i * i) as f64) / (2.0 * sd * sd)).exp()).collect();
    let g_sum: f64 = g.iter().sum();
    let normalized = |k: Vec<f64>| -> Vec<f64> {
        let pos: f64 = k.iter().filter(|v| **v > 0.0).sum::<f64>() * g_sum;
        let neg: f64 = -k.iter().filter(|v| **v < 0.0).sum::<f64>() * g_sum;
        k.iter().map(|v| if *v > 0.0 { v / pos } else if *v < 0.0 { v / neg } else { 0.0 }).collect()
    };
    let edge = normalized((-fr..=fr).zip(&g).map(|(i, gv)| -(i as f64) * gv).collect());
    let point = normalized((-fr..=fr).zip(&g).map(|(i, gv)| ((i * i) as f64 / (sd * sd) - 1.0) * gv).collect());
    let features = |c: &[Plane; 3]| -> (Plane, Plane) {
        let y = c[0].map(|v| (v + 16.0) / 116.0);
        let (ex, ey) = (y.separable(&edge, &g), y.separable(&g, &edge));
        let (px, py) = (y.separable(&point, &g), y.separable(&g, &point));
        (ex.zip(&ey, |a, b| (a * a + b * b).sqrt()), px.zip(&py, |a, b| (a * a + b * b).sqrt()))
    };
    let ((er, pr), (et, ptt)) = (features(&r), features(&t));
    let map: Vec<f32> = (0..w * h).map(|i| {
        let df = ((er.v[i] - et.v[i]).abs().max((pr.v[i] - ptt.v[i]).abs()) / std::f64::consts::SQRT_2).powf(qf);
        color[i].powf(1.0 - df) as f32
    }).collect();
    let mean = map.iter().map(|&v| v as f64).sum::<f64>() / map.len().max(1) as f64;
    (mean, map)
}

/// An error map (0..1) in a magma-like ramp (black, purple, orange, pale
/// yellow), as ꟻLIP's error maps are shown.
pub fn heat(map: &[f32]) -> Vec<[f32; 3]> {
    const STOPS: [[f32; 3]; 5] = [[0.0, 0.0, 0.016], [0.316, 0.071, 0.485], [0.716, 0.215, 0.475], [0.987, 0.535, 0.382], [0.987, 0.991, 0.750]];
    map.iter().map(|&v| {
        let t = v.clamp(0.0, 1.0) * (STOPS.len() - 1) as f32;
        let i = (t.floor() as usize).min(STOPS.len() - 2);
        let f = t - i as f32;
        [0, 1, 2].map(|c| STOPS[i][c] + (STOPS[i + 1][c] - STOPS[i][c]) * f)
    }).collect()
}

/// Mean, sample standard deviation, and 95th percentile of `xs` (NaN for none).
pub fn summary(xs: &[f64]) -> (f64, f64, f64) {
    if xs.is_empty() {
        return (f64::NAN, f64::NAN, f64::NAN);
    }
    let n = xs.len() as f64;
    let mean = xs.iter().sum::<f64>() / n;
    let sd = if xs.len() > 1 { (xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0)).sqrt() } else { 0.0 };
    let mut sorted = xs.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let p95 = sorted[((0.95 * (n - 1.0)).round() as usize).min(sorted.len() - 1)];
    (mean, sd, p95)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gradient(w: usize, h: usize) -> Image {
        Image::new(w, h, (0..w * h).map(|i| {
            let (x, y) = ((i % w) as f32 / w as f32, (i / w) as f32 / h as f32);
            [x, y, 0.5 * (x + y)]
        }).collect())
    }

    #[test]
    fn identical_images() {
        let a = gradient(48, 32);
        let m = measure(&a, &a, Some((&a, &a)));
        assert_eq!((m.mse, m.relmse), (0.0, 0.0));
        assert!(m.psnr.is_infinite());
        assert!((m.ssim - 1.0).abs() < 1e-9, "{}", m.ssim);
        assert!(m.flip.abs() < 1e-9, "{}", m.flip);
    }

    #[test]
    fn known_offset() {
        let a = gradient(40, 40);
        let b = Image::new(40, 40, a.pixels.iter().map(|p| [p[0] + 0.1, p[1] + 0.1, p[2] + 0.1]).collect());
        // MSE 0.01 exactly, so PSNR 20 dB.
        assert!((mse(&a, &b) - 0.01).abs() < 1e-6);
        assert!((psnr(mse(&a, &b)) - 20.0).abs() < 1e-3);
        // A brightness offset keeps structure: SSIM below 1 but high; ꟻLIP small but positive.
        let s = ssim(&a, &b);
        assert!(s < 1.0 && s > 0.8, "{s}");
        let (f, _) = flip(&a, &b, DEFAULT_PPD);
        assert!(f > 0.0 && f < 0.5, "{f}");
    }

    #[test]
    fn flip_extremes() {
        // Black against white is close to the largest colour difference.
        let black = Image::new(24, 24, vec![[0.0; 3]; 576]);
        let white = Image::new(24, 24, vec![[1.0; 3]; 576]);
        let (f, map) = flip(&black, &white, DEFAULT_PPD);
        assert!(f > 0.8 && f <= 1.0, "{f}");
        assert!(map.iter().all(|v| (0.0..=1.0).contains(v)));
        // Noise lowers SSIM more than an offset of the same MSE does.
        let a = gradient(40, 40);
        let noisy = Image::new(40, 40, a.pixels.iter().enumerate().map(|(i, p)| {
            let n = if (i * 7919) % 2 == 0 { 0.1 } else { -0.1 };
            [p[0] + n, p[1] + n, p[2] + n]
        }).collect());
        let offset = Image::new(40, 40, a.pixels.iter().map(|p| [p[0] + 0.1, p[1] + 0.1, p[2] + 0.1]).collect());
        assert!((mse(&a, &noisy) - mse(&a, &offset)).abs() < 1e-3);
        assert!(ssim(&a, &noisy) < ssim(&a, &offset));
    }

    #[test]
    fn halves_and_summary() {
        assert_eq!(f16_to_f32(0x3c00), 1.0);
        assert_eq!(f16_to_f32(0xc000), -2.0);
        assert_eq!(f16_to_f32(0x0000), 0.0);
        assert!((f16_to_f32(0x3555) - 0.333).abs() < 1e-3);
        let (mean, sd, p95) = summary(&[1.0, 2.0, 3.0, 4.0]);
        assert_eq!(mean, 2.5);
        assert!((sd - 1.290994).abs() < 1e-5);
        assert_eq!(p95, 4.0);
    }
}
