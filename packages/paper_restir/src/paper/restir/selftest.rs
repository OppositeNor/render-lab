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
use lab::utility::selftest::{Report, request, still_timeline};
use serde_json::json;

mod sight;

/// The size the passes are checked at.
const SIZE: [u32; 2] = [320, 180];

pub fn run(options: &lab::investigation::SelftestOptions) -> Result<bool> {
    std::fs::create_dir_all(&options.work_dir)?;
    let mut report = Report::new();
    check_estimator(&mut report);
    check_reservoir(&mut report);
    check_bias(&mut report);
    check_passes(&mut report)?;
    check_listings(&mut report);
    sight::check(&mut report, options.timeline.as_deref())?;
    let states_ok = Demo::ALL.iter().all(|&d| shot::for_demo(d).demo == d.id());
    report.check(
        "every demonstration's file loads",
        states_ok,
        json!(Demo::ALL.iter().map(|d| d.key()).collect::<Vec<_>>()),
    );
    let passed = report.passed();
    std::fs::write(
        &options.report,
        serde_json::to_string_pretty(&json!({"passed": passed, "checks": report.checks}))?,
    )?;
    Ok(passed)
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
fn floor_surface(state: &ShotState) -> Surface {
    let _ = state;
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
    let cdf = lights::cdf(&lights);
    let s = floor_surface(&state);
    let mut scene = Scene {
        lights: &lights,
        cdf: &cdf,
        eye: Vec3::new(0.0, 1.6, 3.0),
        triangles: &[],
        candidates: 4,
        unbiased: false,
        visibility_reuse: false,
    };
    let exact = quadrature(&scene, &s);
    let mut results = Vec::new();
    for m in [1u32, 4, 32] {
        // For each M, many runs from different seeds, averaged.
        scene.candidates = m;
        let runs = 40_000;
        let mean: f64 = (0..runs)
            .map(|k| {
                let mut rng = Rng::new(UVec2::new(k, 7), 3, 0);
                let r = scene.candidates(&s, &mut rng);
                (lights::luminance(scene.contribution(&s, r.y)) * r.w) as f64
            })
            .sum::<f64>()
            / runs as f64;
        results.push(json!({"M": m, "mean": mean, "relativeError": (mean - exact) / exact}));
    }
    let ok = results
        .iter()
        .all(|r| r["relativeError"].as_f64().is_some_and(|e| e.abs() < 0.03));
    report.check(
        "streaming RIS is unbiased (Eq. 6): f(y)·W averages to the integral for M = 1, 4, 32",
        ok,
        json!({"integral": exact, "runs": results}),
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

/// The frame's inputs the passes read, read back.
struct Frame {
    surf: [Image; 3],
    lights: Vec<Light>,
    cdf: Vec<f32>,
    triangles: Vec<cpu::Triangle>,
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
            cdf: &self.cdf,
            eye: self.eye,
            triangles: &self.triangles,
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
        cdf: lights::cdf(&lights),
        lights,
        triangles,
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
/// biased (Alg. 4) and unbiased (Alg. 6), each from the GPU's own inputs.
fn check_passes(report: &mut Report) -> Result<()> {
    let mut state = shot::for_demo(Demo::Room);
    state.set("restirTemporal", 0.0);
    state.set("restirSpatialPasses", 1.0);
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
            check_candidates(report, &scene, &frame, &initial, &qs);
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
            report.check("visibility reuse: an occluded sample's W becomes 0 and its M stays (Alg. 5 lines 7-9), as the CPU's shadow ray says", agree * 100 >= qs.len() * 99, json!({"pixels": qs.len(), "agree": agree}));
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
                "a spatial round, {name}: the GPU's reservoir is the CPU's from the same neighbours"
            ),
            agree * 100 >= qs.len() * 98,
            json!({"pixels": qs.len(), "agree": agree, "seed": seed}),
        );
    }
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
) {
    let seed = find_seed(scene, frame, initial, qs);
    let agree = qs
        .iter()
        .filter(|q| {
            let mut rng = Rng::new(UVec2::new(q[0] as u32, q[1] as u32), seed, 0);
            same(
                &scene.candidates(&frame.surface(**q), &mut rng),
                &Reservoir::unpack(initial.texel(**q)),
            )
        })
        .count();
    report.check(
        "streaming RIS (Alg. 3): each pixel's reservoir is the CPU's from the same candidates",
        agree * 100 >= qs.len() * 98,
        json!({"pixels": qs.len(), "agree": agree, "seed": seed}),
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
