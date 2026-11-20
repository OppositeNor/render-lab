//! The ReSTIR investigation's verification suite: the WGSL passes against
//! their CPU mirror (cpu.rs) on the GPU's own inputs, read back after a frame;
//! the properties the paper proves (RIS is unbiased, Eq. 6; reservoir sampling
//! keeps each candidate in proportion to its weight, Eq. 8-10); and the
//! package's files. Every check records what it compared and the result.

use super::cpu::{self, Reservoir, Rng, Sample, Scene, Surface};
use super::lights::{self, Light};
use super::shot::{self, Demo};
use anyhow::{Result, anyhow};
use glam::{UVec2, Vec3};
use lab::shot::ShotState;
use lab::utility::selftest::{Report, request, request_at, still_timeline};
use serde_json::json;

mod sight;

/// The size the passes are checked at.
const SIZE: [u32; 2] = [320, 180];

pub fn run(options: &lab::investigation::SelftestOptions) -> Result<bool> {
    std::fs::create_dir_all(&options.work_dir)?;
    let mut report = Report::new();
    check_estimator(&mut report);
    check_alias(&mut report);
    check_reservoir(&mut report);
    check_bias(&mut report);
    check_toy(&mut report);
    check_passes(&mut report)?;
    check_unbiased_history(&mut report)?;
    check_listings(&mut report);
    sight::check(&mut report, options.timeline.as_deref())?;
    let states_ok = Demo::ALL.iter().all(|&d| shot::for_demo(d).demo == d.id());
    report.check(
        "every demonstration's file loads",
        states_ok,
        json!(Demo::ALL.iter().map(|d| d.key()).collect::<Vec<_>>()),
    );
    report.write(&options.report)
}

/// A program image read back as words.
struct Image {
    size: [u32; 2],
    data: Vec<u32>,
}

impl Image {
    fn texel(&self, q: [i32; 2]) -> [u32; 4] {
        let i = 4 * (q[1] as usize * self.size[0] as usize + q[0] as usize);
        [
            self.data[i],
            self.data[i + 1],
            self.data[i + 2],
            self.data[i + 3],
        ]
    }

    fn floats(&self, q: [i32; 2]) -> [f32; 4] {
        self.texel(q).map(f32::from_bits)
    }
}

fn read(renderer: &lab::renderer::Renderer, name: &str) -> Result<Image> {
    let (bytes, size) = renderer
        .read_program(name, 0)
        .ok_or_else(|| anyhow!("no image {name}"))?;
    let data = bytes
        .chunks(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    Ok(Image {
        size: [size[0], size[1]],
        data,
    })
}

/// One surface near the middle of the room: the floor in front of the teapot, facing up.
fn floor_surface() -> Surface {
    Surface {
        position: Vec3::new(0.3, 0.0, -0.6),
        distance: 4.0,
        normal: Vec3::Y,
        roughness: 1.0,
        albedo: Vec3::splat(0.5),
        object: 0,
        valid: true,
    }
}

/// Eq. 6 is unbiased (148:3 §2.1): the mean of f(y) · W over many runs of streaming RIS at one surface matches
/// the unshadowed integral of ρ · Le · G over the lights' surfaces, computed by quadrature.
fn check_estimator(report: &mut Report) {
    let state = shot::for_demo(Demo::Room);
    let lights = lights::place(&state);
    let (shares, table) = (lights::shares(&lights), lights::table(&lights));
    let s = floor_surface();
    let mut scene = Scene {
        lights: &lights,
        shares: &shares,
        table: &table,
        sky: 0.0,
        eye: Vec3::new(0.0, 1.6, 3.0),
        triangles: &[],
        planes: &[],
        candidates: 4,
        unbiased: false,
        visibility_reuse: false,
    };
    let lit = quadrature(&scene, &s);
    let sky = sky_quadrature(&s, scene.eye);
    let mut results = Vec::new();
    for (m, level) in [(1u32, 0.0f32), (4, 0.0), (32, 0.0), (1, 1.0), (32, 1.0)] {
        // For each M, with and without the sky, many runs from different seeds, averaged.
        scene.candidates = m;
        scene.sky = level;
        let exact = lit + f64::from(level) * sky;
        let runs = 40_000;
        let mean: f64 = (0..runs)
            .map(|k| {
                let mut rng = Rng::new(UVec2::new(k, 7), 3, 0);
                let r = scene.candidates(&s, &mut rng);
                (lights::luminance(scene.contribution(&s, r.y)) * r.w) as f64
            })
            .sum::<f64>()
            / runs as f64;
        results.push(json!({"M": m, "sky": level, "integral": exact, "mean": mean, "relativeError": (mean - exact) / exact}));
    }
    let ok = results
        .iter()
        .all(|r| r["relativeError"].as_f64().is_some_and(|e| e.abs() < 0.03));
    report.check(
        "streaming RIS is unbiased (Eq. 6): f(y)·W averages to the integral for M = 1, 4, 32, and with a quarter of the candidates from the sky for M = 1, 32",
        ok,
        json!({"lights": lit, "skyAtLevelOne": sky, "runs": results}),
    );
}

/// ∫ ρ · Le · cos θ dω over the sky's directions at level 1: in each texel, the integrand at 16 of its cells, each
/// standing for a sixteenth of the texel's cells (the cells of one band of the sphere share a solid angle).
fn sky_quadrature(s: &Surface, eye: Vec3) -> f64 {
    use super::environment::{cell_solid_angle, direction, table};
    let t = table();
    let [w, h] = t.size;
    let (across, down) = (65536 / w, 65536 / h);
    let cells = (across * down) as f64 / 16.0;
    let v = (eye - s.position).normalize();
    let mut total = 0.0f64;
    for (i, le) in t.radiance.iter().enumerate() {
        let (x, y) = (i % w, i / w);
        for (a, b) in (0..16).map(|k| (k % 4, k / 4)) {
            // The cell at this point of the texel's 4 x 4 grid.
            let uv = ((x * across + (2 * a + 1) * across / 8)
                | ((y * down + (2 * b + 1) * down / 8) << 16)) as u32;
            let wi = direction(uv);
            let cos = s.normal.dot(wi);
            if cos > 0.0 {
                let f = lights::luminance(cpu::brdf(s, wi, v) * *le * cos);
                total += f64::from(f) * f64::from(cell_solid_angle(uv)) * cells;
            }
        }
    }
    total
}

/// The alias tables the candidates are drawn from (§5): each gives every light its share of the power, and every
/// texel of the environment map its share of luminance times solid angle.
fn check_alias(report: &mut Report) {
    use super::alias::{probabilities, table};
    let worst = |weights: &[f32]| {
        // The largest difference between an entry's probability and its weight's share, relative to the share.
        let total: f64 = weights.iter().map(|&w| f64::from(w)).sum();
        let p = probabilities(&table(weights));
        weights
            .iter()
            .zip(&p)
            .map(|(&w, p)| (p - f64::from(w) / total).abs() * weights.len() as f64)
            .fold(0.0f64, f64::max)
    };
    let lights = lights::place(&shot::for_demo(Demo::Room));
    let powers: Vec<f32> = lights.iter().map(Light::power).collect();
    let sky = super::environment::table();
    let texels: Vec<f32> = sky
        .radiance
        .iter()
        .enumerate()
        .map(|(i, c)| {
            lights::luminance(*c) * super::environment::texel_solid_angle(sky.size, i / sky.size[0])
        })
        .collect();
    let (l, e) = (worst(&powers), worst(&texels));
    // A sample's direction, mapped back to the image as lab/src/environment.rs maps directions, lands in its texel.
    let [w, h] = sky.size;
    let strays = (0..4096u32)
        .filter(|&k| {
            let mut rng = Rng::new(UVec2::new(k, 3), 1, 5);
            let uv = sky.draw([rng.rand(), rng.rand(), rng.rand(), rng.rand()]);
            let d = super::environment::direction(uv);
            let across = 0.5 + d.x.atan2(-d.z) / std::f32::consts::TAU + lab::environment::TURN;
            let x = (across.rem_euclid(1.0) * w as f32) as usize % w;
            let y = ((d.y.clamp(-1.0, 1.0).acos() / std::f32::consts::PI * h as f32) as usize)
                .min(h - 1);
            sky.texel(uv) != [x, y]
        })
        .count();
    report.check(
        "the alias tables (§5) give each light its share of the power and each texel of the sky its share of luminance times solid angle; a sky sample's direction lies in its texel",
        l < 1e-5 && e < 1e-3 && strays == 0,
        json!({"lights": powers.len(), "worstLight": l, "texels": texels.len(), "worstTexel": e, "directionsOutsideTheirTexel": strays}),
    );
}

/// ∫ ρ · Le · G dA over every light's sphere, on a stratified grid of its surface.
fn quadrature(scene: &Scene, s: &Surface) -> f64 {
    let n = 256u32;
    let mut total = 0.0f64;
    for (i, l) in scene.lights.iter().enumerate() {
        let area = 4.0 * std::f64::consts::PI * (l.radius as f64).powi(2);
        let mut sum = 0.0f64;
        for a in 0..n {
            for b in 0..n {
                // The cell's centre as the sample's two 16-bit coordinates.
                let u = ((a as f32 + 0.5) / n as f32 * 65536.0) as u32;
                let v = ((b as f32 + 0.5) / n as f32 * 65536.0) as u32;
                let x = Sample {
                    light: i as u32,
                    uv: u | (v << 16),
                };
                sum += lights::luminance(scene.contribution(s, x)) as f64;
            }
        }
        total += sum / (n * n) as f64 * area;
    }
    total
}

/// Eq. 8-10 (148:4 §2.2): after a stream, each element is in the reservoir with probability w_i / Σ w.
fn check_reservoir(report: &mut Report) {
    let weights = [1.0f32, 3.0, 0.5, 6.0, 2.5];
    let total: f32 = weights.iter().sum();
    let runs = 200_000;
    let mut counts = [0u32; 5];
    for k in 0..runs {
        let mut rng = Rng::new(UVec2::new(k, 1), 0, 9);
        let mut r = Reservoir::empty();
        for (i, w) in weights.iter().enumerate() {
            r.update(
                Sample {
                    light: i as u32,
                    uv: 0,
                },
                *w,
                &mut rng,
            );
        }
        counts[r.y.light as usize] += 1;
    }
    let worst = (0..5)
        .map(|i| (counts[i] as f32 / runs as f32 - weights[i] / total).abs())
        .fold(0.0f32, f32::max);
    report.check(
        "weighted reservoir sampling keeps each element with probability w_i / Σw (Eq. 8-10)",
        worst < 0.005,
        json!({"weights": weights, "counts": counts, "worstDifference": worst}),
    );
}

/// The 1D example (Fig. 7, Eq. 19-22): where only half the candidates' PDFs reach (x > 1/2), the biased W averages
/// half the true inverse PDF (|Z|/M = 1/2); the naive 1/|Z| and the balance heuristic average all of it.
fn check_bias(report: &mut Report) {
    use super::demos::bias::{BINS, Weight, run};
    let ratio = |w: Weight| {
        // The mean of W over the true inverse PDF, over the bins past one half.
        let c = run(4, w, false, 400_000);
        let right: Vec<f64> = (BINS / 2 + 2..BINS - 2)
            .map(|b| c.mean_w[b] / c.truth[b])
            .collect();
        right.iter().sum::<f64>() / right.len() as f64
    };
    let (biased, naive, balance) = (
        ratio(Weight::Biased),
        ratio(Weight::Naive),
        ratio(Weight::Balance),
    );
    let ok =
        (biased - 0.5).abs() < 0.05 && (naive - 1.0).abs() < 0.05 && (balance - 1.0).abs() < 0.05;
    report.check("the 1D example: biased W is half the inverse PDF where one PDF is zero (Eq. 19); 1/|Z| and the balance heuristic are unbiased (Eq. 20-22)", ok, json!({"biased": biased, "naive": naive, "balance": balance}));
}

/// The worked example the story narrates (toy.rs): each list's estimate (4/3, 20/3, 12), their average the true total
/// 4, and the two parts removed (1.5 without dividing by p, 1 without the average); and RIS itself, run on that world
/// many times (two candidates drawn, one kept by weight, f(y) · W), averaging 4 too.
fn check_toy(report: &mut Report) {
    use super::demos::toy::{Drop, average, estimate, kept, lists, weight};
    let each: Vec<f64> = lists().iter().map(|l| estimate(l, Drop::None)).collect();
    let means = [Drop::None, Drop::DivideByP, Drop::Average].map(|d| average(d).0);
    let mut state = 0x9e37_79b9_7f4a_7c15u64;
    let mut rand = move || {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((state >> 11) as f64) / ((1u64 << 53) as f64)
    };
    let runs = 400_000;
    let mut sum = 0.0;
    for _ in 0..runs {
        // Two candidates by the cheap choice (A with 3/4), one kept in proportion to its weight; f(y) / p̂(y) is 1.
        let x = [rand(), rand()].map(|u| usize::from(u >= 0.75));
        let w = x.map(|i| weight(i, Drop::None));
        sum += (w[0] + w[1]) / 2.0;
    }
    let simulated = sum / f64::from(runs);
    // RIS keeps A 6 times in 10 and B 4 times in 10, and W averages one over those chances.
    let k = kept();
    let near = |a: f64, b: f64| (a - b).abs() < 1e-9;
    let ok = near(each[0], 4.0 / 3.0)
        && near(each[1], 20.0 / 3.0)
        && near(each[2], 12.0)
        && near(means[0], 4.0)
        && near(means[1], 1.5)
        && near(means[2], 1.0)
        && (simulated - 4.0).abs() < 0.03
        && near(k[0].0, 0.6)
        && near(k[1].0, 0.4)
        && near(k[0].1, 1.0 / 0.6)
        && near(k[1].1, 2.5);
    report.check(
        "the worked example: estimates 4/3, 20/3, 12 average to 4; without dividing by p 1.5, without the average 1; RIS run on it averages 4; it keeps B 4 times in 10, W then averaging 2.5",
        ok,
        json!({"estimates": each, "averages": means, "simulated": simulated, "kept": k}),
    );
}

/// The frame's inputs the passes read, read back.
struct Frame {
    surf: [Image; 3],
    lights: Vec<Light>,
    shares: Vec<f32>,
    table: Vec<super::alias::AliasColumn>,
    triangles: Vec<cpu::Triangle>,
    planes: Vec<cpu::Triangle>,
    eye: Vec3,
}

impl Frame {
    fn surface(&self, q: [i32; 2]) -> Surface {
        Surface::from_texels(
            self.surf[0].floats(q),
            self.surf[1].floats(q),
            self.surf[2].floats(q),
        )
    }

    fn scene(&self, state: &ShotState) -> Scene<'_> {
        Scene {
            lights: &self.lights,
            shares: &self.shares,
            table: &self.table,
            sky: super::demos::sky(state),
            eye: self.eye,
            triangles: &self.triangles,
            planes: &self.planes,
            candidates: state.p("restirCandidates") as u32,
            unbiased: state.p("restirUnbiased") > 0.5,
            visibility_reuse: state.p("restirVisibilityReuse") > 0.5,
        }
    }
}

/// Renders `state` once at SIZE and reads back what the checks compare.
fn render(state: &ShotState) -> Result<(lab::renderer::Renderer, Frame)> {
    let gpu = lab::gpu::Gpu::new(lab::gpu::instance(None), None)?;
    let statics = lab::frame::Statics::prepare(
        lab::scene::SceneId::from_resource("paper_restir.scene/restir-room")
            .ok_or_else(|| anyhow!("no ReSTIR room"))?,
    )?;
    let objects = super::demos::objects_for(state, &statics);
    let triangles = cpu::occluders(&statics.meshes, &objects);
    let planes = cpu::planes(&statics.meshes, &objects);
    let mut renderer = lab::renderer::Renderer::new(gpu, statics, SIZE[0], SIZE[1])?;
    let still = still_timeline(SIZE[0], SIZE[1]);
    renderer.render_frame(request(state, &still), None);
    let lights = lights::place(state);
    let frame = Frame {
        surf: [
            read(&renderer, "surfP")?,
            read(&renderer, "surfM")?,
            read(&renderer, "surfA")?,
        ],
        shares: lights::shares(&lights),
        table: lights::table(&lights),
        lights,
        triangles,
        planes,
        eye: Vec3::from(
            state
                .camera
                .camera_data([SIZE[0] as f32, SIZE[1] as f32])
                .position,
        ),
    };
    Ok((renderer, frame))
}

/// Pixels spread over the image where a surface is lit.
fn pixels(frame: &Frame) -> Vec<[i32; 2]> {
    let mut out = Vec::new();
    for k in 0..400u32 {
        // A scatter of pixels from the hash, kept where there is a surface.
        let h = cpu::hash(k * 7919 + 13);
        let q = [(h % SIZE[0]) as i32, ((h / SIZE[0]) % SIZE[1]) as i32];
        if frame.surface(q).valid {
            out.push(q);
        }
    }
    out
}

fn same(a: &Reservoir, b: &Reservoir) -> bool {
    let close = |x: f32, y: f32| (x - y).abs() <= 1e-3 * x.abs().max(y.abs()).max(1e-6);
    a.y == b.y && a.m == b.m && close(a.w, b.w)
}

/// The passes against cpu.rs: candidates (Alg. 3) for the frame's seed, visibility, and one spatial round,
/// biased (Alg. 4) and unbiased (Alg. 6), each from the GPU's own inputs; without the sky and with it.
fn check_passes(report: &mut Report) -> Result<()> {
    for sky in [0.0, 1.0] {
        check_passes_under(report, sky)?;
    }
    Ok(())
}

fn check_passes_under(report: &mut Report, sky: f32) -> Result<()> {
    let mut state = shot::for_demo(Demo::Room);
    state.set("restirTemporal", 0.0);
    state.set("restirSpatialPasses", 1.0);
    state.set("restirEnvironment", sky);
    let under = if sky > 0.0 {
        ", under the sky"
    } else {
        ""
    };
    for unbiased in [0.0, 1.0] {
        // One frame for each combination weight; the candidate and visibility checks need only the first.
        state.set("restirUnbiased", unbiased);
        let (renderer, frame) = render(&state)?;
        let scene = frame.scene(&state);
        let qs = pixels(&frame);
        let initial = read(&renderer, "initial")?;
        let visible = read(&renderer, "visible")?;
        let temporal = read(&renderer, "temporal")?;
        let spatial = read(&renderer, "resA")?;
        if unbiased == 0.0 {
            check_candidates(report, &scene, &frame, &initial, &qs, under);
            let agree = qs
                .iter()
                .filter(|q| {
                    let r = Reservoir::unpack(initial.texel(**q));
                    let v = Reservoir::unpack(visible.texel(**q));
                    // Visible (or nothing to test): W as it was; occluded: W is 0.
                    let expect =
                        if r.y.light == cpu::NONE || scene.visible(&frame.surface(**q), r.y) {
                            r.w
                        } else {
                            0.0
                        };
                    v.w == expect && v.m == r.m
                })
                .count();
            report.check(&format!("visibility reuse: an occluded sample's W becomes 0 and its M stays (Alg. 5 lines 7-9), as the CPU's shadow ray says{under}"), agree * 100 >= qs.len() * 99, json!({"pixels": qs.len(), "agree": agree}));
        }
        let seed = find_seed(&scene, &frame, &initial, &qs);
        let agree = qs
            .iter()
            .filter(|q| {
                let cpu = spatial_round(&scene, &frame, &state, &temporal, **q, seed);
                same(&cpu, &Reservoir::unpack(spatial.texel(**q)))
            })
            .count();
        let name = if unbiased > 0.5 {
            "unbiased (Alg. 6)"
        } else {
            "biased (Alg. 4)"
        };
        report.check(
            &format!(
                "a spatial round, {name}: the GPU's reservoir is the CPU's from the same neighbours{under}"
            ),
            agree * 100 >= qs.len() * 98,
            json!({"pixels": qs.len(), "agree": agree, "seed": seed}),
        );
    }
    Ok(())
}

/// Frames of history the unbiased check runs.
const HISTORY_FRAMES: u64 = 200;
/// The largest W the check allows: the paper's setting stays near 1e2 in this room.
const W_BOUND: f32 = 1e6;

/// Unbiased reuse with temporal history, as the story runs it: Alg. 6 with the paper's unbiased setting (one
/// round of k = 3, 148:10 §5) and visibility reuse. After many frames every reservoir's W must stay finite and
/// below W_BOUND; two rounds of five neighbours instead let W run away (past 1e32 here, to infinity in a larger
/// image), which the story therefore never shows.
fn check_unbiased_history(report: &mut Report) -> Result<()> {
    let mut state = shot::for_demo(Demo::Room);
    for (key, value) in [
        ("restirTemporal", 1.0),
        ("restirSpatialPasses", 1.0),
        ("restirNeighbours", 3.0),
        ("restirUnbiased", 1.0),
        ("restirVisibilityReuse", 1.0),
        ("restirSeed", -1.0),
    ] {
        state.set(key, value);
    }
    let (mut renderer, _) = render(&state)?;
    let still = still_timeline(SIZE[0], SIZE[1]);
    for frame in 1..=HISTORY_FRAMES {
        // Each frame reuses the last one's reservoirs, with a new seed.
        let request = lab::renderer::FrameRequest {
            history: true,
            ..request_at(&state, &still, frame)
        };
        renderer.render_frame(request, None);
    }
    let reservoirs = read(&renderer, "resA")?;
    let mut finite = true;
    let mut largest = 0.0f32;
    for y in 0..SIZE[1] as i32 {
        for x in 0..SIZE[0] as i32 {
            // Every pixel's W after the last frame: finite, and its largest value recorded.
            let w = Reservoir::unpack(reservoirs.texel([x, y])).w;
            finite &= w.is_finite();
            largest = largest.max(w);
        }
    }
    report.check(
        &format!("unbiased reuse over {HISTORY_FRAMES} frames of history (Alg. 6, one round of three neighbours): every W stays finite and below {W_BOUND:e}"),
        finite && largest < W_BOUND,
        json!({"frames": HISTORY_FRAMES, "largestW": largest}),
    );
    Ok(())
}

/// The frame's seed: the one whose candidates the GPU drew (a frame drawn alone is drawn several times first).
fn find_seed(scene: &Scene, frame: &Frame, initial: &Image, qs: &[[i32; 2]]) -> u32 {
    (0..64)
        .max_by_key(|seed| {
            qs.iter()
                .take(24)
                .filter(|q| {
                    let mut rng = Rng::new(UVec2::new(q[0] as u32, q[1] as u32), *seed, 0);
                    same(
                        &scene.candidates(&frame.surface(**q), &mut rng),
                        &Reservoir::unpack(initial.texel(**q)),
                    )
                })
                .count()
        })
        .unwrap_or(0)
}

fn check_candidates(
    report: &mut Report,
    scene: &Scene,
    frame: &Frame,
    initial: &Image,
    qs: &[[i32; 2]],
    under: &str,
) {
    let seed = find_seed(scene, frame, initial, qs);
    let pairs: Vec<(Reservoir, Reservoir)> = qs
        .iter()
        .map(|q| {
            let mut rng = Rng::new(UVec2::new(q[0] as u32, q[1] as u32), seed, 0);
            let cpu = scene.candidates(&frame.surface(*q), &mut rng);
            (cpu, Reservoir::unpack(initial.texel(*q)))
        })
        .collect();
    let agree = pairs.iter().filter(|(c, g)| same(c, g)).count();
    // The first few pixels where they differ, for the report.
    let differ: Vec<String> = pairs
        .iter()
        .filter(|(c, g)| !same(c, g))
        .take(4)
        .map(|(c, g)| format!("cpu {c:?} gpu {g:?}"))
        .collect();
    report.check(
        &format!(
            "streaming RIS (Alg. 3): each pixel's reservoir is the CPU's from the same candidates{under}"
        ),
        agree * 100 >= qs.len() * 98,
        json!({"pixels": qs.len(), "agree": agree, "seed": seed, "fromTheSky": qs.iter().filter(|q| Reservoir::unpack(initial.texel(**q)).y.light == super::environment::MARKER).count(), "differ": differ}),
    );
}

/// One spatial round at q on the CPU (`restirSpatialCS`, round 0).
fn spatial_round(
    scene: &Scene,
    frame: &Frame,
    state: &ShotState,
    input: &Image,
    q: [i32; 2],
    seed: u32,
) -> Reservoir {
    let s = frame.surface(q);
    let mut rng = Rng::new(UVec2::new(q[0] as u32, q[1] as u32), seed, 2);
    let rotation = [rng.rand(), rng.rand()];
    let mut sources = vec![(Reservoir::unpack(input.texel(q)), s)];
    let k = (state.p("restirNeighbours") as u32).min(16);
    for j in 0..k {
        // Each neighbour that passes the geometry test brings its reservoir and surface.
        let qn = cpu::neighbour(q, j, rotation, state.p("restirRadius"), SIZE);
        let sn = frame.surface(qn);
        if qn == q || !similar(state, &s, &sn) {
            continue;
        }
        sources.push((Reservoir::unpack(input.texel(qn)), sn));
    }
    scene.combine(&s, &sources, &mut rng)
}

/// `restirSimilar`, with the shot's test.
fn similar(state: &ShotState, s: &Surface, n: &Surface) -> bool {
    cpu::similar(state.p("restirGeometryTest") > 0.5, s, n)
}

fn check_listings(report: &mut Report) {
    let index = super::sources::INDEX.get();
    let listings = super::pseudocode::all();
    let bad: Vec<&str> = listings
        .iter()
        .filter(|l| {
            !index.entries.contains_key(&l.source)
                || l.demo
                    .as_deref()
                    .is_none_or(|d| Demo::ALL.iter().all(|x| x.key() != d))
        })
        .map(|l| l.name)
        .collect();
    report.check(
        "every listing cites an index entry and a demonstration",
        bad.is_empty(),
        json!({"listings": listings.len(), "bad": bad}),
    );
}
