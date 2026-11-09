//! The trace stepped: where the stepper is, the depth map as a height field, and the trace as a cross-section
//! along the ray.

use super::*;

pub(super) fn grey(v: f32) -> [u8; 4] {
    let g = (v.clamp(0.0, 1.0) * 255.0) as u8;
    [g, g, g, 255]
}

pub(super) fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Where the trace stepper is: `traceStep` = i + f, the iterations before i
/// done and iteration i f of the way, in fifths, one thing at a time (so a
/// story can say each in a sentence of its own): its cell lights as the last
/// iteration's marks fade (f 0 to 0.2); where the ray leaves the cell is
/// marked (0.2 to 0.4); where it reaches the cell's depth is marked (0.4 to
/// 0.6); the ray moves to where the iteration sends it (0.6 to 0.8); and the
/// level shown climbs or descends (0.8 to 1). A whole value shows the
/// iteration before it complete.
pub(super) struct Stepper<'a> {
    pub(super) steps: &'a [trace::Step],
    /// The iteration in progress (`steps.len()` once all are done).
    pub(super) i: usize,
    /// Each fifth's progress, 0 to 1: the cell, its leaving mark, its depth mark, the ray's move, the level's change.
    pub(super) appear: f32,
    pub(super) leave: f32,
    pub(super) reach: f32,
    pub(super) travel: f32,
    pub(super) climb: f32,
}

impl<'a> Stepper<'a> {
    pub(super) fn new(t: &'a trace::Trace, step: f32) -> Stepper<'a> {
        let len = t.steps.len();
        let x = step.max(0.0);
        let i = (x.floor() as usize).min(len);
        let f = if i < len {
            x - i as f32
        } else {
            0.0
        };
        let fifth = |k: f32| smooth(f * 5.0 - k);
        Stepper {
            steps: &t.steps,
            i,
            appear: fifth(0.0),
            leave: fifth(1.0),
            reach: fifth(2.0),
            travel: fifth(3.0),
            climb: fifth(4.0),
        }
    }

    pub(super) fn current(&self) -> Option<&'a trace::Step> {
        self.steps.get(self.i)
    }

    pub(super) fn previous(&self) -> Option<&'a trace::Step> {
        self.i.checked_sub(1).and_then(|k| self.steps.get(k))
    }

    /// The iterations whose comparison is on screen, each with its index and how strongly it shows.
    pub(super) fn fading(&self) -> Vec<(usize, &'a trace::Step, f32)> {
        let mut out = Vec::new();
        if let Some(s) = self.previous() {
            let a = if self.current().is_some() {
                1.0 - self.appear
            } else {
                1.0
            };
            if a > 0.0 {
                out.push((self.i - 1, s, a));
            }
        }
        if let Some(s) = self.current()
            && self.appear > 0.0
        {
            out.push((self.i, s, self.appear));
        }
        out
    }

    /// The iteration a still names (the readout's, the lit cell's): the stronger of the two fading.
    pub(super) fn tested(&self) -> Option<(usize, &'a trace::Step)> {
        self.fading()
            .into_iter()
            .max_by(|a, b| a.2.total_cmp(&b.2))
            .map(|(k, s, _)| (k, s))
    }

    /// Where along the ray (its t) the trace has got to.
    /// Where the chart's window is anchored along the ray: the start of the
    /// iteration in progress, moving on to the next start only while the level
    /// changes (so the ray visibly moves on a still chart, then the chart follows).
    pub(super) fn anchor(&self) -> f32 {
        match (self.current(), self.previous()) {
            (Some(s), _) => s.t + (s.next_t - s.t) * self.climb,
            (None, Some(s)) => s.next_t,
            (None, None) => 0.0,
        }
    }

    pub(super) fn tip(&self) -> f32 {
        match (self.current(), self.previous()) {
            (Some(s), _) => s.t + (s.next_t - s.t) * self.travel,
            (None, Some(s)) => s.next_t,
            (None, None) => 0.0,
        }
    }

    /// How strongly iteration `k`'s marks show (`j` 0 the leaving mark, 1 the
    /// depth mark): the iteration in progress's as their fifths come, the one
    /// before it as it fades.
    pub(super) fn mark_of(&self, k: usize, j: usize, a: f32) -> f32 {
        if k == self.i {
            a.min(if j == 0 {
                self.leave
            } else {
                self.reach
            })
        } else {
            a
        }
    }

    /// How strongly iteration `k`'s decision shows (its arrow): the iteration in progress's once the ray moves.
    pub(super) fn move_of(&self, k: usize, a: f32) -> f32 {
        if k == self.i {
            a.min(self.travel)
        } else {
            a
        }
    }

    /// Whether iteration `k`'s result can be named: both its marks are up (or it is the one before).
    pub(super) fn decided(&self, k: usize) -> bool {
        k != self.i || self.reach > 0.9
    }

    /// The pyramid level shown: between two levels while the ray climbs or descends (never below zero).
    pub(super) fn level(&self) -> f32 {
        match (self.current(), self.previous()) {
            (Some(s), _) => {
                let (a, b) = (s.level.max(0) as f32, s.next_level.max(0) as f32);
                a + (b - a) * self.climb
            }
            (None, Some(s)) => s.next_level.max(0) as f32,
            (None, None) => 0.0,
        }
    }

    /// The ray's path as values of t: where each finished iteration sent it, then the tip.
    pub(super) fn path(&self) -> Vec<f32> {
        let mut out: Vec<f32> = self.steps[..self.i].iter().map(|s| s.next_t).collect();
        if self.current().is_some() && self.travel > 0.0 {
            out.push(self.tip());
        }
        out
    }
}

/// The cells of the level the field shows (`level`, between two levels while it
/// changes) that stand for an iteration's cell: the cell itself on its own
/// level, or the four it splits into while the ray descends from it.
pub(super) fn field_cells(s: &trace::Step, level: f32) -> Vec<[i32; 2]> {
    let l = level.max(0.0).floor() as i32;
    let own = s.level.max(0);
    if own <= l {
        return vec![[s.cell[0] >> (l - own), s.cell[1] >> (l - own)]];
    }
    let k = own - l;
    let n = 1 << k;
    (0..n * n)
        .map(|j| [(s.cell[0] << k) + j % n, (s.cell[1] << k) + j / n])
        .collect()
}

/// What the depth map's height field shows: P (`pixel`), around which it
/// zooms (`fieldZoom`: a window framing `focus`, a texel box x0, y0, x1, y1);
/// its columns, of pyramid level `level` (between two levels, each 2 x 2
/// rises to the tallest of its parent and merges); and `lit`, cells of that
/// level coloured (texel coordinates of the level, a colour).
#[derive(Clone, Copy)]
pub(super) struct FieldView<'a> {
    pub(super) pixel: [i32; 2],
    pub(super) focus: [f32; 4],
    pub(super) level: f32,
    pub(super) lit: &'a [([i32; 2], Color32)],
}

/// The depth map drawn as a height field (lab::utility::height_field), over a
/// dark ground that hides the room: each SSR texel a column as tall as its
/// reversed depth (near is tall), fading in flat, as the screen shows it
/// (`fieldShow`), then tilting back as the columns rise (`fieldMorph`).
/// What it shows is `shown`'s (`FieldView`).
pub(super) fn field(
    state: &ShotState,
    content: &mut FrameContent,
    rest: &lab::layout::CameraData,
    screen: &cpu::Screen,
    shown: &FieldView,
) -> HeightField {
    let FieldView {
        pixel,
        focus,
        level,
        lit,
    } = *shown;
    let show = 1.0;
    let morph = smooth(state.p("fieldMorph"));
    // On the plane the view's camera faces at rest; a little height even when flat, so the
    // tiles stand clear of the slab under them.
    let tilt = state.p("fieldTilt").clamp(0.0, 85.0);
    let placement = Placement::facing(
        rest,
        REST,
        (0.002 + 0.07 * morph) * REST / 0.25,
        tilt * morph,
        state.p("fieldTurn") * morph,
        1.0 - 0.4 * morph,
    );
    let hiz = &screen.hiz;
    let size = hiz[0].size;
    let (w, h) = (size[0] as f32, size[1] as f32);
    // The window: the whole image, or `focus` with a margin, at the image's shape (at least 40 texels across).
    let zoom = smooth(state.p("fieldZoom"));
    let across = ((focus[2] - focus[0]) * 1.3)
        .max((focus[3] - focus[1]) * 1.3 * w / h)
        .clamp(40.0, w);
    let close = [across, across * h / w];
    let cx = ((focus[0] + focus[2]) * 0.5).clamp(close[0] * 0.5, w - close[0] * 0.5);
    let cy = ((focus[1] + focus[3]) * 0.5).clamp(close[1] * 0.5, h - close[1] * 0.5);
    let near = [
        cx - close[0] * 0.5,
        cy - close[1] * 0.5,
        cx + close[0] * 0.5,
        cy + close[1] * 0.5,
    ];
    let window = [
        near[0] * zoom,
        near[1] * zoom,
        w + (near[2] - w) * zoom,
        h + (near[3] - h) * zoom,
    ];
    let field = HeightField {
        size,
        window,
        placement,
    };
    field.base(&mut content.draw, Color32::from_rgb(20, 24, 30), show);
    let top = field_top(screen, pixel);
    // Coloured on the depth map's ramp (its inset's), so the flat field is that picture.
    let ramp = hiz[0].data.iter().copied().fold(0.0f32, f32::max).max(1e-6);
    let last = hiz.len() - 1;
    let l = (level.max(0.0).floor() as usize).min(last);
    let f = if l < last {
        level - l as f32
    } else {
        0.0
    };
    let f = smooth(f.clamp(0.0, 1.0));
    let img = &hiz[l];
    let (sx, sy) = (w / img.size[0] as f32, h / img.size[1] as f32);
    for y in 0..img.size[1] as i32 {
        for x in 0..img.size[0] as i32 {
            let texels = [
                x as f32 * sx,
                y as f32 * sy,
                (x + 1) as f32 * sx,
                (y + 1) as f32 * sy,
            ];
            if !field.shows(texels) {
                continue;
            }
            let d = img.load([x, y]).x;
            // Between levels: rising to the parent's tallest.
            let d = if f > 0.0 {
                d + (hiz[l + 1]
                    .load([
                        (x >> 1).min(hiz[l + 1].size[0] as i32 - 1),
                        (y >> 1).min(hiz[l + 1].size[1] as i32 - 1),
                    ])
                    .x
                    - d)
                    * f
            } else {
                d
            };
            if d <= 0.0 {
                continue;
            }
            let color = lit
                .iter()
                .find(|(c, _)| *c == [x, y])
                .map_or_else(|| height_field::grey(d, ramp), |(_, c)| *c);
            field.column(&mut content.draw, texels, (d / top).min(1.0), color, show);
        }
    }
    // P, on its column.
    let p = Vec2::new(pixel[0] as f32 + 0.5, pixel[1] as f32 + 0.5);
    content.draw.point(
        field.point(p, (hiz[0].load(pixel).x / top).min(1.0)),
        6.0,
        palette::SELECTED,
        show,
    );
    field
}

/// A cell the ray crosses: the cell, the t where the ray enters and leaves
/// it, and its depth.
pub(super) type Crossing = ([i32; 2], f32, f32, f32);

/// The cells of `level` the ray crosses from t = 0 to `end`, in order.
pub(super) fn cells_along(
    screen: &cpu::Screen,
    t: &trace::Trace,
    level: usize,
    end: f32,
) -> Vec<Crossing> {
    let img = &screen.hiz[level];
    let size = Vec2::new(img.size[0] as f32, img.size[1] as f32);
    let mut out: Vec<Crossing> = Vec::new();
    const SAMPLES: usize = 3000;
    for k in 0..=SAMPLES {
        let tt = end * k as f32 / SAMPLES as f32;
        let p = (t.screen_pos + t.screen_ray * tt).truncate() * size;
        let cell = [p.x.floor() as i32, p.y.floor() as i32];
        if cell[0] < 0
            || cell[1] < 0
            || cell[0] >= img.size[0] as i32
            || cell[1] >= img.size[1] as i32
        {
            break;
        }
        match out.last_mut() {
            Some(last) if last.0 == cell => last.2 = tt,
            _ => out.push((cell, tt, tt, img.load(cell).x)),
        }
    }
    out
}

/// The trace as a cross-section along the ray (the usual picture of a Hi-Z
/// march), over the view (`profileShow`): across, the ray's t from P; up,
/// reversed depth (nearer is higher). The cells of the level shown stand as
/// bars as tall as their nearest depth (two levels cross-fading while the ray
/// climbs or descends), the true surface (level 0) drawn thin over them; the
/// ray is a straight line. For the iteration tested: its cell lit, where the
/// ray leaves the cell and where it reaches the cell's depth marked, whichever
/// comes first deciding (a miss climbs a level, a hit descends), and an arrow
/// to where the next iteration starts. The axes span the whole trace, so they
/// stay still while it is stepped.
pub(super) fn profile(
    content: &mut FrameContent,
    rest: &lab::layout::CameraData,
    screen: &cpu::Screen,
    t: &trace::Trace,
    stepper: &Stepper,
) {
    if t.steps.is_empty() {
        return;
    }
    let (chart, bars) = Chart::of(rest, screen, t, stepper);
    chart.axes(content);
    chart.cells(content, stepper, &bars);
    // The true surface, thin, over them: only while the bars are of level 0, where they are the surface's own
    // cells.
    let surface_alpha = (1.0 - stepper.level()).clamp(0.0, 1.0) * 0.7;
    if surface_alpha > 0.0 {
        for &(_, a, b, d) in &bars.surface {
            content.draw.segment(
                chart.on(chart.at(a, d)),
                chart.on(chart.at(b, d)),
                palette::SURFACE,
                surface_alpha,
                1.5,
                0,
            );
        }
    }
    chart.ray_line(content, stepper.tip());
    let named = stepper.tested().map(|(k, _)| k);
    for &(k, s, a) in &stepper.fading() {
        chart.comparison(content, stepper, (k, s, a), named);
    }
    chart.result(content, stepper);
}

/// The level cells along the ray a profile shows: those of each level the trace visits, the true surface's
/// (level 0), and the last level.
pub(super) struct Bars {
    levels: Vec<(usize, Vec<Crossing>)>,
    surface: Vec<Crossing>,
    last_level: usize,
}

impl Bars {
    /// The cells of level `l`, if the trace visits it.
    fn at(&self, l: usize) -> &[Crossing] {
        self.levels
            .iter()
            .find(|(k, _)| *k == l)
            .map(|(_, b)| b.as_slice())
            .unwrap_or(&[])
    }
}

/// The profile chart's frame: the plane it is on (the one the view's camera faces at rest), the trace, the
/// window of the march it shows (`t0`, `width` long), and the depths it spans (`lo` to `hi`).
pub(super) struct Chart<'a> {
    plane: Placement,
    lift: f32,
    t: &'a trace::Trace,
    t0: f32,
    width: f32,
    lo: f32,
    hi: f32,
}

/// The chart's corners in its plane: u across, v down.
pub(super) const CHART: (f32, f32, f32, f32) = (0.07, 0.93, 0.2, 0.86);

impl<'a> Chart<'a> {
    /// The chart of `t` on `screen`, with the bars it shows. The axes span the whole trace, so they stay still
    /// while it is stepped; across, a window around the ray's tip, eight cells of the level shown wide: it
    /// follows the tip and widens as the ray climbs (both move smoothly), so a step on any level reads at the
    /// same size.
    fn of(
        rest: &lab::layout::CameraData,
        screen: &cpu::Screen,
        t: &'a trace::Trace,
        stepper: &Stepper,
    ) -> (Chart<'a>, Bars) {
        let last_level = screen.hiz.len() - 1;
        // The whole march, with a little room after where it ends.
        let reach = t
            .steps
            .iter()
            .map(|s| s.edge_t.max(s.next_t))
            .fold(0.0f32, f32::max)
            .min(t.t_max.max(1e-6));
        let end = (reach.max(t.steps.last().map_or(0.0, |s| s.next_t)) * 1.08).max(1e-6);
        // Every level the trace visits, once: the bars of each, and the range of depths over all of them.
        let mut levels: Vec<usize> = t
            .steps
            .iter()
            .map(|s| (s.level.max(0) as usize).min(last_level))
            .collect();
        levels.sort_unstable();
        levels.dedup();
        let bars = Bars {
            levels: levels
                .iter()
                .map(|&l| (l, cells_along(screen, t, l, end)))
                .collect(),
            surface: cells_along(screen, t, 0, end),
            last_level,
        };
        let ray = |tt: f32| (t.screen_pos + t.screen_ray * tt).z;
        let mut lo = ray(0.0).min(ray(end));
        let mut hi = ray(0.0).max(ray(end));
        for &(_, _, _, d) in bars.levels.iter().flat_map(|(_, b)| b).chain(&bars.surface) {
            if d > 0.0 {
                lo = lo.min(d);
                hi = hi.max(d);
            }
        }
        let pad = (hi - lo).max(1e-6) * 0.15;
        let size = Vec2::new(screen.size[0] as f32, screen.size[1] as f32);
        let per_texel = 1.0
            / (t.screen_ray.x.abs() * size.x)
                .max(t.screen_ray.y.abs() * size.y)
                .max(1e-9);
        let width = (8.0 * per_texel * 2f32.powf(stepper.level())).min(end);
        let chart = Chart {
            // On the plane the view's camera faces at rest.
            plane: Placement::facing(rest, REST, 0.0, 0.0, 0.0, 1.0),
            lift: REST / 0.25,
            t,
            t0: (stepper.anchor() - 0.3 * width).clamp(0.0, end - width),
            width,
            lo: lo - pad,
            hi: hi + pad,
        };
        (chart, bars)
    }

    /// The ray's depth at `tt` along it.
    fn ray(&self, tt: f32) -> f32 {
        (self.t.screen_pos + self.t.screen_ray * tt).z
    }

    /// Where `tt` along the ray, at depth `d`, is on the chart.
    fn at(&self, tt: f32, d: f32) -> Vec2 {
        let (u0, u1, v0, v1) = CHART;
        Vec2::new(
            u0 + ((tt - self.t0) / self.width).clamp(0.0, 1.0) * (u1 - u0),
            v1 - ((d - self.lo) / (self.hi - self.lo)).clamp(0.0, 1.0) * (v1 - v0),
        )
    }

    /// Whether `tt` along the ray is in the window shown.
    fn inside(&self, tt: f32) -> bool {
        tt >= self.t0 && tt <= self.t0 + self.width
    }

    /// A point of the chart in the scene, just in front of its plane.
    fn on(&self, uv: Vec2) -> Vec3 {
        self.plane.on(uv, 2e-3 * self.lift)
    }

    /// The chart's background and axes, their names pinned where they end (so they never move).
    fn axes(&self, content: &mut FrameContent) {
        let (u0, u1, v0, v1) = CHART;
        self.plane.rect(
            &mut content.draw,
            Vec2::new(u0, v0),
            Vec2::new(u1, v1),
            -2e-3 * self.lift,
            Color32::from_rgb(16, 21, 28),
            1.0,
        );
        for end in [Vec2::new(u1, v1), Vec2::new(u0, v0)] {
            content.draw.segment(
                self.on(Vec2::new(u0, v1)),
                self.on(end),
                palette::text_dim(),
                1.0,
                1.5,
                frame::FLAG_ARROW,
            );
        }
        content.draw.text(
            self.on(Vec2::new(u1, v1 + 0.015)),
            tr("demos-profile_t"),
            palette::text_dim(),
            [1.0, 0.0],
        );
        content.draw.text(
            self.on(Vec2::new(u0, v0 - 0.012)),
            tr("demos-profile_depth"),
            palette::text_dim(),
            [0.0, 1.0],
        );
    }

    /// The level's cells along the ray (between two levels, the lower fading into the upper), and the cell each
    /// iteration on screen tests, lit on its own level, as strongly as it shows.
    fn cells(&self, content: &mut FrameContent, stepper: &Stepper, bars: &Bars) {
        let (lo, last_level) = (self.lo, bars.last_level);
        let level = stepper.level();
        let below = (level.floor() as usize).min(last_level);
        let above = (below + 1).min(last_level);
        let mix = if above > below {
            level - below as f32
        } else {
            0.0
        };
        for (l, a) in [(below, 1.0 - mix), (above, mix)] {
            if a <= 0.0 || (l == above && above == below) {
                continue;
            }
            for &(_, x0, x1, d) in bars.at(l) {
                self.plane.rect(
                    &mut content.draw,
                    self.at(x0, lo),
                    self.at(x1, d.max(lo)),
                    0.0,
                    Color32::from_rgb(92, 100, 112),
                    0.85 * a,
                );
            }
        }
        for &(_, s, a) in &stepper.fading() {
            let l = (s.level.max(0) as usize).min(last_level);
            let color = if s.hit {
                palette::INCIDENT
            } else {
                palette::SELECTED
            };
            if let Some(&(_, x0, x1, d)) = bars.at(l).iter().find(|(c, ..)| *c == s.cell) {
                self.plane.rect(
                    &mut content.draw,
                    self.at(x0, lo),
                    self.at(x1, d.max(lo)),
                    1e-3 * self.lift,
                    color,
                    a,
                );
            }
        }
    }

    /// The ray as far as the trace has marched it (`reached`), and where it is; P at its start.
    fn ray_line(&self, content: &mut FrameContent, reached: f32) {
        let (start, tip) = (
            self.on(self.at(0.0, self.ray(0.0))),
            self.on(self.at(reached, self.ray(reached))),
        );
        content
            .draw
            .segment(start, tip, palette::REFLECTED, 1.0, 3.0, 0);
        content.draw.point(tip, 6.0, palette::REFLECTED, 1.0);
        if self.inside(0.0) {
            content.draw.text(
                self.on(self.at(0.0, self.ray(0.0)) + Vec2::new(0.008, -0.01)),
                "P",
                palette::SELECTED,
                [0.0, 1.0],
            );
        }
    }

    /// Iteration `k`'s comparison (`s`, shown at `a`), fading: where the ray leaves the cell and where it reaches
    /// the cell's depth, each in its own fifth (one thing at a time), named on the iteration `named`; then the
    /// arrow to where the next iteration starts.
    fn comparison(
        &self,
        content: &mut FrameContent,
        stepper: &Stepper,
        (k, s, a): (usize, &trace::Step, f32),
        named: Option<usize>,
    ) {
        let marks = [
            (
                s.edge_t,
                palette::AXIS_Z,
                tr("demos-profile_leaves"),
                stepper.mark_of(k, 0, a),
            ),
            (
                s.depth_t,
                palette::INCIDENT,
                tr("demos-profile_reaches"),
                stepper.mark_of(k, 1, a),
            ),
        ];
        for (j, (tt, color, text, a)) in marks.into_iter().enumerate() {
            if a <= 0.0 {
                continue;
            }
            // Names do not fade: only the iteration named carries them.
            let text = (named == Some(k)).then_some(text);
            if self.inside(tt) {
                self.mark(content, (tt, j), (color, a), text);
            } else {
                self.off_chart(content, (tt, j), (color, a), text);
            }
        }
        let moving = stepper.move_of(k, a);
        if s.next_t > s.t && moving > 0.0 {
            content.draw.segment(
                self.on(self.at(s.t, self.ray(s.t))),
                self.on(self.at(s.next_t, self.ray(s.next_t))),
                palette::SELECTED,
                moving,
                3.0,
                frame::FLAG_ARROW,
            );
        }
    }

    /// A mark at `tt` along the ray (the `j`th of its iteration): a dashed line across the chart and a point on
    /// the ray, in `color` at `a`; its `text` at the top of its line, in a row of its own, right of the line (or
    /// left of it in the chart's right half, so it stays inside).
    fn mark(
        &self,
        content: &mut FrameContent,
        (tt, j): (f32, usize),
        (color, a): (Color32, f32),
        text: Option<&str>,
    ) {
        let (_, _, v0, v1) = CHART;
        let x = self.at(tt, self.lo).x;
        content.draw.segment(
            self.on(Vec2::new(x, v1)),
            self.on(Vec2::new(x, v0)),
            color,
            0.9 * a,
            2.0,
            frame::FLAG_DASHED,
        );
        content
            .draw
            .point(self.on(self.at(tt, self.ray(tt))), 7.0, color, a);
        let Some(text) = text else {
            return;
        };
        let left = x > 0.55;
        content.draw.text(
            self.on(Vec2::new(
                if left {
                    x - 0.006
                } else {
                    x + 0.006
                },
                v0 + 0.06 + 0.05 * j as f32,
            )),
            text,
            color,
            [
                if left {
                    1.0
                } else {
                    0.0
                },
                0.0,
            ],
        );
    }

    /// A mark past the window's edge (a far wall's depth, or one already behind the ray): an arrow at that edge
    /// pointing to it, in the mark's own row, so the comparison still reads; its `text` said to be off the chart,
    /// so the arrow does not read as pointing at nothing.
    fn off_chart(
        &self,
        content: &mut FrameContent,
        (tt, j): (f32, usize),
        (color, a): (Color32, f32),
        text: Option<&str>,
    ) {
        let (u0, u1, v0, _) = CHART;
        let right = tt > self.t0 + self.width;
        let (edge, back) = if right {
            (u1, u1 - 0.05)
        } else {
            (u0, u0 + 0.05)
        };
        let y = v0 + 0.075 + 0.05 * j as f32;
        content.draw.segment(
            self.on(Vec2::new(back, y)),
            self.on(Vec2::new(edge, y)),
            color,
            0.9 * a,
            2.0,
            frame::FLAG_ARROW,
        );
        let Some(text) = text else {
            return;
        };
        let x = if right {
            back - 0.006
        } else {
            back + 0.006
        };
        let text = tr_with("demos-profile_off", lab::args!(mark = text));
        content.draw.text(
            self.on(Vec2::new(x, y - 0.015)),
            text,
            color,
            [
                if right {
                    1.0
                } else {
                    0.0
                },
                0.0,
            ],
        );
    }

    /// The tested iteration's result, once both its marks are up: held, a hit (down a level), or a miss (up one).
    fn result(&self, content: &mut FrameContent, stepper: &Stepper) {
        let Some((k, s)) = stepper.tested() else {
            return;
        };
        if !stepper.decided(k) {
            return;
        }
        let result = if s.held {
            tr("demos-held")
        } else if s.hit {
            tr("demos-hit_down")
        } else {
            tr("demos-miss_up")
        };
        let text = tr_with(
            "demos-profile_level",
            lab::args!(from = s.level, to = s.next_level, result = result),
        );
        content.draw.text(
            self.on(Vec2::new(0.5, CHART.2 - 0.075)),
            text,
            if s.hit {
                palette::INCIDENT
            } else {
                palette::SELECTED
            },
            [0.5, 1.0],
        );
    }
}

/// The reversed depth a field column of full height stands for: P's, twice
/// over, so P's column is half the height. Reversed depth grows
/// fast near the camera; the floor nearer than P is capped there (the ray
/// climbs away from it, over farther texels).
pub(super) fn field_top(screen: &cpu::Screen, pixel: [i32; 2]) -> f32 {
    let max = screen.hiz[0]
        .data
        .iter()
        .copied()
        .fold(0.0f32, f32::max)
        .max(1e-6);
    (screen.hiz[0].load(pixel).x * 2.0).clamp(1e-6, max)
}

/// A Hi-Z level as a picture: its depths on a ramp over the room's range.
pub(super) fn level_picture(level: &Image, key: String) -> Picture {
    let max = level.data.iter().copied().fold(0.0f32, f32::max).max(1e-6);
    let rgba: Vec<u8> = level
        .data
        .iter()
        .flat_map(|d| grey((d / max).powf(0.5)))
        .collect();
    Picture::Pixels {
        key,
        size: [level.size[0] as usize, level.size[1] as usize],
        rgba: Arc::new(rgba),
    }
}
