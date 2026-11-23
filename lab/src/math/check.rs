//! The motion check of math scenes: a scene sampled densely (a demonstration's Play clip, a story's frames) and
//! its mobs followed by name from sample to sample, so what a still never shows is found without rendering:
//!
//! - **pop**: a mob that appears or vanishes while plainly visible, or whose opacity, place, size, or colour
//!   jumps between neighbouring samples;
//! - **fast** (in real time, a story's frames): a fade by half or more within a tenth of a second, which reads as
//!   a cut;
//! - **double**: one name drawn twice in one place (an emphasis appended beside its figure);
//! - **dead**: a span of a clip where a progress moves and nothing on the canvas does;
//! - **small**: text under the lab's sizes (an em of 0.32 canvas units, 0.42 with fractions, sub- and
//!   superscripts), at the frame's zoom;
//! - **overlap**: two texts over each other, a text partly over a shape it does not sit inside, or a line drawn
//!   through a text (a strike aside);
//! - **outside**: a mob beyond the part of the canvas the frame shows;
//! - **error**: a sample where the scene fails.
//!
//! Mobs without a name are drawn but not followed. A mob is paired across samples with the nearest mob of its name
//! and kind; a name drawn more than once (a plot cut into runs, two forms cross-fading in a transform) fades and
//! comes or goes as a group, by its most opaque mob. A copy leaving from or landing on a mob drawn where it is
//! does not count as appearing or vanishing.

use glam::Vec2;
use std::collections::HashMap;

/// What the check measures of one mob a scene draws (src/script/runtime/math.rs reads it with the mob).
#[derive(Clone, Debug)]
pub struct Probe {
    pub id: String,
    pub kind: &'static str,
    /// Its ink box on the canvas.
    pub lo: Vec2,
    pub hi: Vec2,
    pub alpha: f32,
    /// How much of it is written or drawn (a formula's wipe, a path's drawn length), 0 to 1.
    pub coverage: f32,
    pub rgb: [f32; 3],
    /// A text's or formula's em, in canvas units.
    pub em: Option<f32>,
    /// A formula with fractions, sub- or superscripts, whose smaller glyphs need a larger em.
    pub fine_print: bool,
    /// The formula a part belongs to (its parts may overlap each other).
    pub formula: Option<String>,
    /// A path's points, as far as it is drawn (none for other mobs).
    pub points: Vec<Vec2>,
    /// The rest of a formula (its ink outside the named terms), whose box spans the whole formula.
    pub rest: bool,
    /// How opaque its filled inside is (a rectangle's fill, a dot), 0 for what has none.
    pub fill: f32,
    /// The theme's colour it is drawn in, which follows the viewer's theme (`rgb` is the current theme's).
    pub role: Option<Role>,
}

/// A colour of the theme (runtime.lam's `textColor`, `textDimColor`, `textFaintColor`, `accentColor`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Text,
    Dim,
    Faint,
    Accent,
}

/// What the check needs of a theme: its name, the canvas's colour, and the colour of each `Role`.
#[derive(Clone, Debug)]
pub struct Palette {
    pub name: String,
    pub background: [f32; 3],
    pub roles: [[f32; 3]; 4],
}

impl Palette {
    pub fn of(name: &str, t: &crate::palette::Theme) -> Palette {
        let f = |c: egui::Color32| [c.r(), c.g(), c.b()].map(|v| v as f32 / 255.0);
        Palette {
            name: name.into(),
            background: f(t.background),
            roles: [t.text, t.text_dim, t.text_faint, t.accent].map(f),
        }
    }

    /// A probe's colour in this theme.
    fn color(&self, p: &Probe) -> [f32; 3] {
        p.role.map_or(p.rgb, |r| self.roles[r as usize])
    }
}

impl Probe {
    fn size(&self) -> Vec2 {
        (self.hi - self.lo).max(Vec2::ZERO)
    }

    fn center(&self) -> Vec2 {
        (self.lo + self.hi) * 0.5
    }

    /// How much of it shows: its opacity, by how much is written, by how large it is (a figure grown from a point
    /// shows little at first, a bar grown from its base by its thinner side).
    fn presence(&self) -> f32 {
        let extent = match self.kind {
            "rect" | "band" => (self.size().min_element() / 0.1).min(1.0),
            _ => (self.size().max_element() / 0.25).min(1.0),
        };
        self.alpha.clamp(0.0, 1.0) * self.coverage * extent
    }

    /// Whether `other` is drawn where this is, the same kind and size: a copy that leaves it, or lands on it.
    fn coincides(&self, other: &Probe) -> bool {
        self.kind == other.kind
            && self.center().distance(other.center()) < 0.1
            && self.size().distance(other.size()) < 0.1
    }

    fn is_text(&self) -> bool {
        self.kind == "tex" || self.kind == "text"
    }
}

/// The scene at one sample: when (a clip's seconds, a story's frame), the frame it shows (centre, height), and
/// its mobs, or why it failed.
pub struct Sample {
    pub at: f64,
    pub frame: (Vec2, f32),
    pub probes: Result<Vec<Probe>, String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Rule {
    Error,
    Pop,
    Fast,
    Double,
    Dead,
    Small,
    Overlap,
    Outside,
    Contrast,
}

impl Rule {
    pub fn name(self) -> &'static str {
        match self {
            Rule::Error => "error",
            Rule::Pop => "pop",
            Rule::Fast => "fast",
            Rule::Double => "double",
            Rule::Dead => "dead",
            Rule::Small => "small",
            Rule::Overlap => "overlap",
            Rule::Outside => "outside",
            Rule::Contrast => "contrast",
        }
    }

    /// How long (in the samples' units) a condition must last to count: a pop is one sample; a double draw, an
    /// overlap, or a mob outside the frame shorter than this is a motion passing through.
    fn lasting(self) -> f64 {
        match self {
            Rule::Error | Rule::Pop | Rule::Fast | Rule::Dead => 0.0,
            Rule::Outside => 0.25,
            Rule::Double | Rule::Overlap | Rule::Small | Rule::Contrast => 0.5,
        }
    }
}

/// A finding: a rule broken by the mob `id` (or a pair, `a / b`) from `from` to `to`, with what was measured.
#[derive(Clone, Debug)]
pub struct Finding {
    pub rule: Rule,
    pub id: String,
    pub from: f64,
    pub to: f64,
    pub detail: String,
}

/// The bounds of the rules, in canvas units and fractions.
pub struct Limits {
    /// The frame's width over its height (the canvas's narrowest: 16:9).
    pub aspect: f32,
    /// The least em of plain text and of a formula with fine print, at a frame 8 high.
    pub min_em: f32,
    pub min_fine_em: f32,
    /// In real time (a story's frames): the least seconds over which a mob may change by half its presence.
    pub min_fade: Option<f64>,
    /// The themes a viewer may choose, in each of which every text must read.
    pub themes: Vec<Palette>,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            aspect: 16.0 / 9.0,
            min_em: 0.32,
            min_fine_em: 0.42,
            min_fade: None,
            themes: vec![Palette::of("classic", &crate::palette::Theme::CLASSIC)],
        }
    }
}

/// A presence that shows plainly: appearing or vanishing above it is a pop.
const POP: f32 = 0.35;
/// The least change between two samples that can be a jump: in presence, in canvas units moved, in an em's log
/// ratio, in a colour channel. A change is a jump when it is also `SUDDEN` times the larger of the changes in the
/// samples on either side: a fast motion changes as much from sample to sample, a cut once.
const LEAST: Change = Change {
    fade: 0.3,
    moved: 0.3,
    grown: 0.14,
    colour: 0.25,
};
const SUDDEN: f32 = 3.0;
/// The area (square canvas units, by how much each fades) that fading within the window reads as a cut.
const FAST_AREA: f32 = 0.5;
/// The least room between a text's ink and a line drawn past it, in canvas units.
const CLEARANCE: f32 = 0.04;

/// How much one mob changed between two samples.
#[derive(Clone, Copy, Default)]
struct Change {
    fade: f32,
    moved: f32,
    grown: f32,
    colour: f32,
}

/// A mob's name and kind: mobs are followed across samples within them.
type Key = (String, &'static str);

/// The mobs of two samples paired by name and kind, each with the nearest of its group in the other (by place,
/// size, and presence), and those left over alone. Unnamed mobs are left out.
fn pairs<'a>(a: &'a [Probe], b: &'a [Probe]) -> Vec<(Key, Option<&'a Probe>, Option<&'a Probe>)> {
    let mut groups: HashMap<Key, (Vec<&Probe>, Vec<&Probe>)> = HashMap::new();
    for p in a.iter().filter(|p| !p.id.is_empty()) {
        groups.entry((p.id.clone(), p.kind)).or_default().0.push(p);
    }
    for q in b.iter().filter(|q| !q.id.is_empty()) {
        groups.entry((q.id.clone(), q.kind)).or_default().1.push(q);
    }
    let mut out = Vec::new();
    for (key, (xs, ys)) in groups {
        // The closest pairs are taken first, greedily; the rest have no partner.
        let mut costs: Vec<(f32, usize, usize)> = Vec::new();
        for (i, p) in xs.iter().enumerate() {
            for (j, q) in ys.iter().enumerate() {
                let em = match (p.em, q.em) {
                    (Some(e), Some(f)) if e.min(f) > 0.0 => (f / e).ln().abs(),
                    _ => 0.0,
                };
                let colour = (0..3).map(|i| (p.rgb[i] - q.rgb[i]).abs()).sum::<f32>();
                let cost = p.center().distance(q.center())
                    + p.size().distance(q.size())
                    + 4.0 * (p.presence() - q.presence()).abs()
                    + 4.0 * em
                    + 2.0 * colour;
                costs.push((cost, i, j));
            }
        }
        costs.sort_by(|x, y| x.0.total_cmp(&y.0));
        let (mut used_x, mut used_y) = (vec![false; xs.len()], vec![false; ys.len()]);
        for (_, i, j) in costs {
            if !used_x[i] && !used_y[j] {
                used_x[i] = true;
                used_y[j] = true;
                out.push((key.clone(), Some(xs[i]), Some(ys[j])));
            }
        }
        out.extend(
            xs.iter()
                .zip(&used_x)
                .filter(|(_, u)| !**u)
                .map(|(p, _)| (key.clone(), Some(*p), None)),
        );
        out.extend(
            ys.iter()
                .zip(&used_y)
                .filter(|(_, u)| !**u)
                .map(|(q, _)| (key.clone(), None, Some(*q))),
        );
    }
    out
}

/// How a paired mob changed between two samples.
fn change(p: &Probe, q: &Probe) -> Change {
    let shows = p.presence().min(q.presence()) > 0.2;
    let grown = match (p.em, q.em) {
        (Some(e), Some(f)) if shows && e.min(f) > 0.0 => (f / e).ln().abs(),
        _ => 0.0,
    };
    let colour = if p.presence().min(q.presence()) > 0.3 {
        (0..3)
            .map(|i| (p.rgb[i] - q.rgb[i]).abs())
            .fold(0.0, f32::max)
    } else {
        0.0
    };
    // Opacity by how much is written: a figure growing or shrinking is a motion, not a fade.
    let opacity = |m: &Probe| m.alpha.clamp(0.0, 1.0) * m.coverage;
    // A path is followed by its first point: one being drawn grows from it (its box swells, it does not move).
    let at = |m: &Probe| match (m.kind, m.points.first()) {
        ("path", Some(first)) => *first,
        _ => m.center(),
    };
    Change {
        fade: (opacity(p) - opacity(q)).abs(),
        moved: if shows {
            at(p).distance(at(q))
        } else {
            0.0
        },
        grown,
        colour,
    }
}

/// How the mobs paired between two samples changed, by name and kind. A name drawn more than once (two forms
/// cross-fading in a transform, which trade partners as they fade) fades as a group: by its most opaque mob.
fn changes(a: &[Probe], b: &[Probe]) -> HashMap<Key, Vec<Change>> {
    let mut out: HashMap<Key, Vec<Change>> = HashMap::new();
    let mut groups: HashMap<Key, (f32, f32, usize)> = HashMap::new();
    let opacity = |m: &Probe| m.alpha.clamp(0.0, 1.0) * m.coverage;
    for (key, p, q) in pairs(a, b) {
        let g = groups.entry(key.clone()).or_insert((0.0, 0.0, 0));
        // A copy leaving from or landing on a mob drawn where it is adds nothing that shows.
        let copy = match (p, q) {
            (None, Some(q)) => a.iter().any(|o| o.presence() > POP && o.coincides(q)),
            (Some(p), None) => b.iter().any(|o| o.presence() > POP && o.coincides(p)),
            _ => false,
        };
        if !copy {
            // A mob in both samples by its opacity; one coming or going by how much of it shows.
            let (vp, vq) = match (p, q) {
                (Some(p), Some(q)) => (opacity(p), opacity(q)),
                (p, q) => (
                    p.map_or(0.0, Probe::presence),
                    q.map_or(0.0, Probe::presence),
                ),
            };
            g.0 = g.0.max(vp);
            g.1 = g.1.max(vq);
        }
        g.2 += 1;
        if let (Some(p), Some(q)) = (p, q) {
            out.entry(key).or_default().push(change(p, q));
        }
    }
    for (key, (before, after, count)) in groups {
        if count > 1 {
            // The group's members keep their motion; their fades are the group's.
            if let Some(cs) = out.get_mut(&key) {
                for c in cs.iter_mut() {
                    c.fade = 0.0;
                }
                cs.push(Change {
                    fade: (before - after).abs(),
                    ..Change::default()
                });
            }
        }
    }
    out
}

/// The mobs that appear or vanish while plainly shown between two samples, but for a copy that leaves a mob drawn
/// in the same place before (`a`), or lands on one drawn there after (`b`).
fn cuts(a: &[Probe], b: &[Probe]) -> Vec<(String, String)> {
    // A name drawn more than once in either sample (a plot cut into runs at a jump, two forms cross-fading) changes
    // as a group: its fades are judged by the group (`changes`), not each mob coming or going.
    let mut count: HashMap<Key, (usize, usize)> = HashMap::new();
    for p in a.iter().filter(|p| !p.id.is_empty()) {
        count.entry((p.id.clone(), p.kind)).or_default().0 += 1;
    }
    for q in b.iter().filter(|q| !q.id.is_empty()) {
        count.entry((q.id.clone(), q.kind)).or_default().1 += 1;
    }
    let grouped = |key: &Key| {
        count.get(key).is_some_and(|c| c.0 > 1 || c.1 > 1) && count[key].0 > 0 && count[key].1 > 0
    };
    let mut out = Vec::new();
    for (key, p, q) in pairs(a, b) {
        if grouped(&key) {
            continue;
        }
        match (p, q) {
            // A mob that was not there before shows at once.
            (None, Some(q))
                if q.presence() > POP
                    && !a.iter().any(|p| p.presence() > POP && p.coincides(q)) =>
            {
                out.push((
                    key.0,
                    format!("{} appears at {:.0}%", q.kind, q.presence() * 100.0),
                ));
            }
            // A mob that is gone was shown until then.
            (Some(p), None)
                if p.presence() > POP
                    && !b.iter().any(|q| q.presence() > POP && q.coincides(p)) =>
            {
                out.push((
                    key.0,
                    format!("{} vanishes from {:.0}%", p.kind, p.presence() * 100.0),
                ));
            }
            _ => {}
        }
    }
    out
}

/// The jumps of the change from sample i-1 to i (`now`), against the largest change of the same name and kind in
/// the samples on either side.
fn jumps(
    before: Option<&HashMap<Key, Vec<Change>>>,
    now: &HashMap<Key, Vec<Change>>,
    after: Option<&HashMap<Key, Vec<Change>>>,
) -> Vec<(String, String)> {
    let near = |key: &Key, f: fn(&Change) -> f32| {
        let side = |m: Option<&HashMap<Key, Vec<Change>>>| {
            m.and_then(|m| m.get(key))
                .map_or(0.0, |cs| cs.iter().map(f).fold(0.0, f32::max))
        };
        side(before).max(side(after))
    };
    let mut out = Vec::new();
    for (key, cs) in now {
        // Two forms of one name cross-fading (a transform) trade partners as they fade: their size and colour are
        // followed only for a name drawn once.
        let alone = cs.len() == 1;
        for c in cs {
            let sudden = |f: fn(&Change) -> f32| f(c) > f(&LEAST) && f(c) > SUDDEN * near(key, f);
            let why = if sudden(|c| c.fade) {
                format!(
                    "{} opacity changes by {:.0}% at once",
                    key.1,
                    c.fade * 100.0
                )
            } else if sudden(|c| c.moved) {
                format!("{} moves {:.2} units at once", key.1, c.moved)
            } else if alone && sudden(|c| c.grown) {
                format!(
                    "{} em changes {:.0}% at once",
                    key.1,
                    (c.grown.exp() - 1.0) * 100.0
                )
            } else if alone && sudden(|c| c.colour) {
                format!("{} changes colour at once", key.1)
            } else {
                continue;
            };
            out.push((key.0.clone(), why));
        }
    }
    out
}

/// The mobs whose opacity changes by half or more between two samples `seconds` apart, when together they cover
/// more than `FAST_AREA` (a block of the picture fading that fast reads as a cut; glyphs written one after another,
/// each as fast, do not; a figure growing fast is a motion).
fn hurried(a: &[Probe], b: &[Probe], seconds: f64) -> Vec<(String, String)> {
    // Each name's opacity before and after, by its most opaque mob (two forms cross-fading trade partners), and
    // the largest area it covers.
    let mut groups: HashMap<Key, (f32, f32, f32)> = HashMap::new();
    for (key, p, q) in pairs(a, b) {
        let g = groups.entry(key).or_insert((0.0, 0.0, 0.0));
        // A mob in both samples by its alpha (a wipe across glyphs is writing); one coming or going by how much of
        // it shows, its size counted (a bar growing from nothing shows little at first).
        let (vp, vq) = match (p, q) {
            (Some(p), Some(q)) => (p.alpha.clamp(0.0, 1.0), q.alpha.clamp(0.0, 1.0)),
            (p, q) => (
                p.map_or(0.0, Probe::presence),
                q.map_or(0.0, Probe::presence),
            ),
        };
        g.0 = g.0.max(vp);
        g.1 = g.1.max(vq);
        g.2 = g.2.max(p.map_or(0.0, area)).max(q.map_or(0.0, area));
    }
    let changed: Vec<(Key, f32)> = groups
        .into_iter()
        .filter(|(_, (before, after, _))| (before - after).abs() >= 0.5)
        .map(|(key, (before, after, covered))| {
            let fade = (before - after).abs().min(1.0);
            (key, fade * covered)
        })
        .collect();
    if changed.iter().map(|(_, c)| c).sum::<f32>() < FAST_AREA {
        return Vec::new();
    }
    changed
        .into_iter()
        .map(|(key, _)| {
            (
                key.0,
                format!("{} fades by half or more within {seconds:.2} s", key.1),
            )
        })
        .collect()
}

/// Whether anything changed visibly between two samples (for dead spans): a mob appearing or vanishing while it
/// shows, or one changing past a hair.
fn moved(a: &[Probe], b: &[Probe]) -> bool {
    pairs(a, b).into_iter().any(|(_, p, q)| match (p, q) {
        (Some(p), Some(q)) => {
            // A path also changes by its points, inside a box that stays (a curve redrawn).
            let mean =
                |m: &Probe| m.points.iter().copied().sum::<Vec2>() / m.points.len().max(1) as f32;
            (p.presence() - q.presence()).abs() > 0.01
                || p.center().distance(q.center()) > 0.005
                || p.size().distance(q.size()) > 0.005
                || mean(p).distance(mean(q)) > 0.005
                || p.points.len() != q.points.len()
                || (0..3).any(|i| (p.rgb[i] - q.rgb[i]).abs() > 0.01)
        }
        (Some(m), None) | (None, Some(m)) => m.presence() > 0.01,
        (None, None) => false,
    })
}

/// The area two boxes share.
fn shared(a: (Vec2, Vec2), b: (Vec2, Vec2)) -> f32 {
    let size = (a.1.min(b.1) - a.0.max(b.0)).max(Vec2::ZERO);
    size.x * size.y
}

/// The area a mob covers: its box's, but a path's is its length by a hairline (a ring or an axis covers little).
fn area(p: &Probe) -> f32 {
    if p.kind == "path" {
        let length: f32 = p.points.windows(2).map(|w| w[0].distance(w[1])).sum();
        return length * 0.03;
    }
    p.size().x * p.size().y
}

/// One name drawn twice in one place in a sample.
fn doubles(probes: &[Probe]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let shown: Vec<&Probe> = probes
        .iter()
        .filter(|p| !p.id.is_empty() && p.presence() > 0.3)
        .collect();
    for (i, p) in shown.iter().enumerate() {
        for q in &shown[i + 1..] {
            // A tint over its figure (`indicate`) differs in colour; a double draw is the same mob again.
            let tint = (0..3).any(|i| (p.rgb[i] - q.rgb[i]).abs() > 0.05);
            if p.id != q.id || p.kind != q.kind || p.formula != q.formula || tint {
                continue;
            }
            let both = shared((p.lo, p.hi), (q.lo, q.hi));
            let either = area(p) + area(q) - both;
            // Nearly the same box (or the same point, for a box of no size) is the same mob drawn again.
            let same = if either > 1e-6 {
                both / either > 0.9
            } else {
                p.center().distance(q.center()) < 0.01
            };
            if same {
                out.push((p.id.clone(), format!("{} drawn twice", p.kind)));
            }
        }
    }
    out.dedup();
    out
}

/// Texts under the size limits, at the frame's zoom.
fn small(probes: &[Probe], frame_height: f32, limits: &Limits) -> Vec<(String, String)> {
    let zoom = super::FRAME_HEIGHT / frame_height.max(0.01);
    probes
        .iter()
        .filter(|p| p.alpha * p.coverage > 0.5)
        .filter_map(|p| {
            let em = p.em? * zoom;
            let least = if p.fine_print {
                limits.min_fine_em
            } else {
                limits.min_em
            };
            (em + 0.005 < least)
                .then(|| (p.id.clone(), format!("{} em {em:.2} < {least:.2}", p.kind)))
        })
        .collect()
}

/// Whether the segment from `a` to `b` passes through the box `lo`, `hi` (clipped against it, Liang-Barsky).
fn crosses(a: Vec2, b: Vec2, lo: Vec2, hi: Vec2) -> bool {
    let d = b - a;
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for (p, q) in [
        (-d.x, a.x - lo.x),
        (d.x, hi.x - a.x),
        (-d.y, a.y - lo.y),
        (d.y, hi.y - a.y),
    ] {
        if p == 0.0 {
            // Parallel to this side: inside its slab or never.
            if q < 0.0 {
                return false;
            }
        } else if p < 0.0 {
            t0 = t0.max(q / p);
        } else {
            t1 = t1.min(q / p);
        }
    }
    t0 <= t1
}

/// Texts over each other, a text partly over a shape it does not sit inside, or a line through a text (but for a
/// strike, which crosses its text by design).
fn overlaps(probes: &[Probe]) -> Vec<(String, String)> {
    // Only what shows whole counts: two forms of one piece cross while a transform fades one into the other.
    let shown: Vec<&Probe> = probes.iter().filter(|p| p.presence() > 0.9).collect();
    let mut out = Vec::new();
    for t in shown.iter().filter(|p| p.is_text()) {
        for o in &shown {
            // A formula's rest has a box as wide as the formula, which says little about where its ink is; one name drawn
            // twice is a double draw, not an overlap.
            let same = o.is_text() && o.formula.is_some() && o.formula == t.formula;
            if std::ptr::eq(*t, *o) || same || t.rest || o.rest || t.id == o.id {
                continue;
            }
            let both = shared((t.lo, t.hi), (o.lo, o.hi));
            let hit = if o.is_text() {
                // Two texts touching at all (a twentieth of the smaller) count once, from the earlier one.
                std::ptr::from_ref(*t) < std::ptr::from_ref(*o)
                    && both > 0.05 * area(t).min(area(o))
            } else if o.kind == "path" {
                // The text's box with a hair of clearance (a line is drawn a few pixels wide) crossed by a segment.
                let clear = Vec2::splat(CLEARANCE);
                o.id != "cross"
                    && o.points
                        .windows(2)
                        .any(|w| crosses(w[0], w[1], t.lo - clear, t.hi + clear))
            } else if o.kind == "rect" || o.kind == "dot" {
                // A text on a shape is a label inside it, or beside it; partly over it, it covers or is covered.
                let inside = both > 0.9 * area(t);
                !inside && both > 0.2 * area(t)
            } else {
                false
            };
            if hit {
                out.push((
                    format!("{} / {}", t.id, o.id),
                    format!("{} over {}", t.kind, o.kind),
                ));
            }
        }
    }
    // A fill drawn after a label tints and hides it, however see-through: a label goes over its shape.
    for (i, t) in probes.iter().enumerate() {
        if !t.is_text() || t.rest || t.presence() <= 0.9 {
            continue;
        }
        for o in probes[i + 1..]
            .iter()
            .filter(|o| o.fill >= 0.3 && o.coverage > 0.9)
        {
            if o.id != t.id && shared((t.lo, t.hi), (o.lo, o.hi)) > 0.5 * area(t) {
                out.push((
                    format!("{} / {}", t.id, o.id),
                    format!("{} under a filled {}", t.kind, o.kind),
                ));
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// The least contrast of a text with what is under it (WCAG's for large text).
const MIN_CONTRAST: f32 = 4.5;

/// The relative luminance of an sRGB colour (WCAG).
fn luminance(c: [f32; 3]) -> f32 {
    let lin = |v: f32| {
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2])
}

/// Texts too close in brightness to what is under them, in the theme where they read worst: the fills drawn before
/// a text at its centre, laid over the canvas in order, against the text's colour.
fn contrasts(probes: &[Probe], limits: &Limits) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (i, t) in probes.iter().enumerate() {
        if !t.is_text() || t.rest || t.presence() <= 0.9 {
            continue;
        }
        let c = t.center();
        let fills: Vec<&Probe> = probes[..i]
            .iter()
            .filter(|o| o.kind == "rect" || o.kind == "dot")
            .filter(|o| o.fill > 0.0 && c.cmpge(o.lo).all() && c.cmple(o.hi).all())
            .collect();
        let ratio = |theme: &Palette| {
            // Each fill around the text's centre covers what is under it by its opacity.
            let under = fills.iter().fold(theme.background, |under, o| {
                let (a, rgb) = (o.fill.min(1.0), theme.color(o));
                [0, 1, 2].map(|k| under[k] * (1.0 - a) + rgb[k] * a)
            });
            let (a, b) = (luminance(theme.color(t)), luminance(under));
            (a.max(b) + 0.05) / (a.min(b) + 0.05)
        };
        let worst = limits
            .themes
            .iter()
            .map(|theme| (ratio(theme), theme))
            .min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((r, theme)) = worst.filter(|(r, _)| *r < MIN_CONTRAST) {
            out.push((
                t.id.clone(),
                format!(
                    "{} at a contrast of {r:.1} with what is under it (theme {})",
                    t.kind, theme.name
                ),
            ));
        }
    }
    out
}

/// Mobs beyond the part of the canvas the frame shows.
fn outside(
    probes: &[Probe],
    (center, height): (Vec2, f32),
    limits: &Limits,
) -> Vec<(String, String)> {
    let half = Vec2::new(height * limits.aspect, height) * 0.5 + Vec2::splat(0.05);
    probes
        .iter()
        .filter(|p| p.presence() > 0.3)
        .filter(|p| (p.lo - center).cmplt(-half).any() || (p.hi - center).cmpgt(half).any())
        .map(|p| (p.id.clone(), format!("{} beyond the frame", p.kind)))
        .collect()
}

/// Runs of one rule's per-sample conditions merged into findings, those shorter than the rule's lasting left out.
struct Runs {
    open: HashMap<(Rule, String), Finding>,
    done: Vec<Finding>,
}

impl Runs {
    fn new() -> Runs {
        Runs {
            open: HashMap::new(),
            done: Vec::new(),
        }
    }

    /// The conditions at `at` (the sample after `before`): a run goes on, starts, or ends.
    fn add(&mut self, at: f64, before: f64, found: Vec<(Rule, String, String)>) {
        let mut now: HashMap<(Rule, String), String> = HashMap::new();
        for (rule, id, detail) in found {
            now.entry((rule, id)).or_insert(detail);
        }
        let ended: Vec<(Rule, String)> = self
            .open
            .keys()
            .filter(|k| !now.contains_key(*k))
            .cloned()
            .collect();
        for key in ended {
            let f = self.open.remove(&key).unwrap();
            self.close(f);
        }
        for (key, detail) in now {
            // A condition still holding extends its run; a new one starts at this sample.
            match self.open.get_mut(&key) {
                Some(f) => f.to = at,
                None => {
                    let from = if key.0 == Rule::Pop {
                        before
                    } else {
                        at
                    };
                    self.open.insert(
                        key.clone(),
                        Finding {
                            rule: key.0,
                            id: key.1,
                            from,
                            to: at,
                            detail,
                        },
                    );
                }
            }
        }
    }

    fn close(&mut self, f: Finding) {
        if f.to - f.from + 1e-9 >= f.rule.lasting() {
            self.done.push(f);
        }
    }

    fn finish(mut self) -> Vec<Finding> {
        let open: Vec<Finding> = self.open.drain().map(|(_, f)| f).collect();
        for f in open {
            self.close(f);
        }
        self.done.sort_by(|a, b| {
            (a.rule, a.from, &a.id)
                .partial_cmp(&(b.rule, b.from, &b.id))
                .unwrap()
        });
        self.done
    }
}

/// The findings of one run of samples, in time order (a demonstration's Play, one beat of a story): cuts and jumps
/// between neighbours, and the conditions of each sample merged into runs.
pub fn check(samples: &[Sample], limits: &Limits) -> Vec<Finding> {
    let ok: Vec<Option<&Vec<Probe>>> = samples.iter().map(|s| s.probes.as_ref().ok()).collect();
    // The change into each sample from the one before (none into the first, or around a failed one).
    let into: Vec<Option<HashMap<Key, Vec<Change>>>> = (0..samples.len())
        .map(|i| match (i.checked_sub(1).and_then(|j| ok[j]), ok[i]) {
            (Some(a), Some(b)) => Some(changes(a, b)),
            _ => None,
        })
        .collect();
    let mut runs = Runs::new();
    for (i, s) in samples.iter().enumerate() {
        let mut found: Vec<(Rule, String, String)> = Vec::new();
        match &s.probes {
            Err(e) => found.push((Rule::Error, String::new(), e.clone())),
            Ok(probes) => {
                if let (Some(a), Some(now)) = (i.checked_sub(1).and_then(|j| ok[j]), &into[i]) {
                    let before = i.checked_sub(1).and_then(|j| into[j].as_ref());
                    let after = into.get(i + 1).and_then(Option::as_ref);
                    let popped = cuts(a, probes).into_iter().chain(jumps(before, now, after));
                    found.extend(popped.map(|(id, d)| (Rule::Pop, id, d)));
                }
                if let Some(window) = limits.min_fade {
                    // In real time, the sample `window` seconds before shows what a fade did in that time.
                    let back = samples[..i]
                        .iter()
                        .rposition(|e| s.at - e.at >= window - 1e-6);
                    if let Some(Ok(earlier)) = back.map(|j| &samples[j].probes) {
                        let hurried = hurried(earlier, probes, window);
                        found.extend(hurried.into_iter().map(|(id, d)| (Rule::Fast, id, d)));
                    }
                }
                found.extend(
                    doubles(probes)
                        .into_iter()
                        .map(|(i, d)| (Rule::Double, i, d)),
                );
                found.extend(
                    small(probes, s.frame.1, limits)
                        .into_iter()
                        .map(|(i, d)| (Rule::Small, i, d)),
                );
                found.extend(
                    overlaps(probes)
                        .into_iter()
                        .map(|(i, d)| (Rule::Overlap, i, d)),
                );
                found.extend(
                    contrasts(probes, limits)
                        .into_iter()
                        .map(|(i, d)| (Rule::Contrast, i, d)),
                );
                found.extend(
                    outside(probes, s.frame, limits)
                        .into_iter()
                        .map(|(i, d)| (Rule::Outside, i, d)),
                );
            }
        }
        let before = i.checked_sub(1).map_or(s.at, |j| samples[j].at);
        runs.add(s.at, before, found);
    }
    runs.finish()
}

/// The spans (`from`, `to` in the samples' units) in which a clip moves a progress, each found dead when no two
/// neighbouring samples inside it differ.
pub fn dead_spans(samples: &[Sample], spans: &[(f64, f64)]) -> Vec<Finding> {
    spans
        .iter()
        .filter(|(from, to)| {
            let inside: Vec<&Vec<Probe>> = samples
                .iter()
                .filter(|s| s.at >= from - 1e-6 && s.at <= to + 1e-6)
                .filter_map(|s| s.probes.as_ref().ok())
                .collect();
            // A slow change moves little from sample to sample, but the span's ends differ.
            let ends = inside.len() > 1 && moved(inside[0], inside[inside.len() - 1]);
            inside.len() > 1 && !ends && !inside.windows(2).any(|w| moved(w[0], w[1]))
        })
        .map(|&(from, to)| Finding {
            rule: Rule::Dead,
            id: String::new(),
            from,
            to,
            detail: "a progress moves and nothing on the canvas does".into(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(id: &str, x: f32, alpha: f32) -> Probe {
        Probe {
            id: id.into(),
            kind: "text",
            lo: Vec2::new(x - 0.5, -0.2),
            hi: Vec2::new(x + 0.5, 0.2),
            alpha,
            coverage: 1.0,
            rgb: [1.0; 3],
            em: Some(0.4),
            fine_print: false,
            formula: None,
            points: Vec::new(),
            rest: false,
            fill: 0.0,
            role: None,
        }
    }

    fn sample(at: f64, probes: Vec<Probe>) -> Sample {
        Sample {
            at,
            frame: (Vec2::ZERO, 8.0),
            probes: Ok(probes),
        }
    }

    fn rules(f: &[Finding]) -> Vec<(Rule, String)> {
        f.iter().map(|f| (f.rule, f.id.clone())).collect()
    }

    #[test]
    fn a_fade_passes_and_a_cut_pops() {
        let faded: Vec<Sample> = (0..=30)
            .map(|i| sample(i as f64 / 30.0, vec![text("a", 0.0, i as f32 / 30.0)]))
            .collect();
        assert!(check(&faded, &Limits::default()).is_empty());
        let cut = vec![sample(0.0, vec![text("a", 0.0, 1.0)]), sample(0.1, vec![])];
        assert_eq!(
            rules(&check(&cut, &Limits::default())),
            vec![(Rule::Pop, "a".into())]
        );
    }

    #[test]
    fn a_jump_in_place_pops_and_a_glide_does_not() {
        let jumped = vec![
            sample(0.0, vec![text("a", 0.0, 1.0)]),
            sample(0.1, vec![text("a", 2.0, 1.0)]),
        ];
        assert_eq!(
            rules(&check(&jumped, &Limits::default())),
            vec![(Rule::Pop, "a".into())]
        );
        let glide: Vec<Sample> = (0..=30)
            .map(|i| sample(i as f64 / 30.0, vec![text("a", i as f32 / 15.0, 1.0)]))
            .collect();
        assert!(check(&glide, &Limits::default()).is_empty());
    }

    #[test]
    fn a_double_draw_counts_once_it_lasts() {
        let twice = |at| sample(at, vec![text("a", 0.0, 1.0), text("a", 0.0, 1.0)]);
        assert!(check(&[twice(0.0), twice(0.1)], &Limits::default()).is_empty());
        let found = check(&[twice(0.0), twice(0.3), twice(0.6)], &Limits::default());
        assert_eq!(rules(&found), vec![(Rule::Double, "a".into())]);
    }

    #[test]
    fn small_text_overlaps_and_the_frame() {
        let mut tiny = text("t", 0.0, 1.0);
        tiny.em = Some(0.2);
        let probes = vec![tiny, text("b", 0.3, 1.0), text("far", 9.0, 1.0)];
        let found = check(
            &[sample(0.0, probes.clone()), sample(1.0, probes)],
            &Limits::default(),
        );
        assert_eq!(
            rules(&found),
            vec![
                (Rule::Small, "t".into()),
                (Rule::Overlap, "t / b".into()),
                (Rule::Outside, "far".into()),
            ]
        );
    }

    #[test]
    fn a_label_must_be_drawn_over_its_cell() {
        let cell = Probe {
            kind: "rect",
            lo: Vec2::new(-1.0, -1.0),
            hi: Vec2::new(1.0, 1.0),
            em: None,
            fill: 0.6,
            rgb: [0.2, 0.2, 0.3],
            ..text("cell", 0.0, 0.6)
        };
        let found = |probes: Vec<Probe>| {
            let s = [sample(0.0, probes.clone()), sample(1.0, probes)];
            rules(&check(&s, &Limits::default()))
        };
        // A label on its cell reads; the cell's fill drawn over the label hides it.
        assert!(found(vec![cell.clone(), text("label", 0.0, 1.0)]).is_empty());
        assert_eq!(
            found(vec![text("label", 0.0, 1.0), cell.clone()]),
            vec![(Rule::Overlap, "label / cell".into())]
        );
        // White on a light fill does not read.
        let pale = Probe {
            rgb: [0.9, 0.8, 0.85],
            fill: 1.0,
            ..cell
        };
        assert_eq!(
            found(vec![pale, text("label", 0.0, 1.0)]),
            vec![(Rule::Contrast, "label".into())]
        );
    }

    #[test]
    fn a_span_where_nothing_moves_is_dead() {
        let still: Vec<Sample> = (0..=10)
            .map(|i| sample(i as f64 / 10.0, vec![text("a", 0.0, 1.0)]))
            .collect();
        assert_eq!(dead_spans(&still, &[(0.0, 1.0)]).len(), 1);
        let moving: Vec<Sample> = (0..=10)
            .map(|i| sample(i as f64 / 10.0, vec![text("a", i as f32 * 0.01, 1.0)]))
            .collect();
        assert!(dead_spans(&moving, &[(0.0, 1.0)]).is_empty());
    }

    #[test]
    fn a_cross_fade_and_a_copy_leaving_are_not_pops() {
        // Two forms of one name, the old fading out as the new fades in, at its place.
        let faded: Vec<Sample> = (0..=30)
            .map(|i| {
                let k = i as f32 / 30.0;
                sample(k as f64, vec![text("a", 0.0, 1.0 - k), text("a", 0.1, k)])
            })
            .collect();
        assert!(check(&faded, &Limits::default()).is_empty());
        // A copy that appears where its source is drawn, then flies off.
        let flown: Vec<Sample> = (0..=30)
            .map(|i| {
                let mut probes = vec![text("source", 0.0, 1.0)];
                if i > 0 {
                    probes.push(text("copy", i as f32 * 0.05, 1.0));
                }
                sample(i as f64 / 30.0, probes)
            })
            .collect();
        let found = check(&flown, &Limits::default());
        assert!(found.iter().all(|f| f.rule != Rule::Pop), "{found:?}");
    }

    #[test]
    fn a_path_being_drawn_grows_and_does_not_move() {
        let drawn: Vec<Sample> = (1..=30)
            .map(|i| {
                let end = Vec2::new(
                    i as f32 * 0.2,
                    if i > 15 {
                        -2.0
                    } else {
                        0.0
                    },
                );
                let p = Probe {
                    kind: "path",
                    lo: Vec2::new(0.0, end.y.min(0.0)),
                    hi: Vec2::new(end.x, 0.0),
                    points: vec![Vec2::ZERO, Vec2::new(end.x, 0.0), end],
                    em: None,
                    ..text("curve", 0.0, 1.0)
                };
                sample(i as f64 / 30.0, vec![p])
            })
            .collect();
        let found = check(&drawn, &Limits::default());
        assert!(found.iter().all(|f| f.rule != Rule::Pop), "{found:?}");
    }
}
