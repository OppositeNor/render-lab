//! A sound's volume as the user sets it: a level from 0 to 100 % and a mute
//! switch that keeps the level, so unmuting brings it back. The settings
//! file keeps one for the homepage's music and one for the narration
//! (src/config.rs); `slider` is the control both are set with, on the Config
//! page and in the viewer's dock. Players take it as a gain (src/audio.rs).

use crate::i18n::tr;
use crate::palette;
use egui::{pos2, vec2, Color32, Rect, Sense, Shape, Stroke, Ui};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Volume {
    /// The level, 0 to 100.
    pub percent: u8,
    pub muted: bool,
}

impl Volume {
    pub const fn new(percent: u8) -> Volume {
        Volume { percent: if percent > 100 { 100 } else { percent }, muted: false }
    }

    /// The factor a player scales the sound by: 0 when muted.
    pub fn gain(self) -> f32 {
        if self.muted { 0.0 } else { self.percent as f32 / 100.0 }
    }

    /// Whether nothing is heard (muted, or at 0 %).
    pub fn silent(self) -> bool {
        self.gain() == 0.0
    }
}

/// What a volume slider did this frame.
#[derive(Clone, Copy, Debug)]
pub struct SliderResponse {
    /// The volume changed.
    pub changed: bool,
    /// A change is finished (the mute button, a click on the rail, the end of
    /// a drag): a player that restarts to take a new volume does it now,
    /// rather than at every step of a drag.
    pub committed: bool,
    /// The mute button and the rail, for tests.
    pub mute: Rect,
    pub rail: Rect,
}

/// The mute button (a speaker, crossed out when nothing is heard) and a
/// slider of the level, `rail` points long, on one row of `ui`. The button is
/// as tall as `size`; dragging the slider of a muted volume unmutes it.
pub fn slider(ui: &mut Ui, volume: &mut Volume, size: f32, rail: f32) -> SliderResponse {
    let before = *volume;
    ui.horizontal(|ui| {
        let (mute_rect, mute) = ui.allocate_exact_size(vec2(size, size), Sense::click());
        let mute = mute.on_hover_text(if volume.muted { tr("volume.unmute") } else { tr("volume.mute") });
        if mute.clicked() {
            volume.muted = !volume.muted;
        }
        let color = if mute.hovered() { palette::accent() } else if volume.silent() { palette::text_faint() } else { palette::text() };
        speaker(ui.painter(), mute_rect, *volume, color);
        ui.spacing_mut().slider_width = rail;
        let mut percent = volume.percent;
        let level = ui.add(egui::Slider::new(&mut percent, 0..=100).suffix(" %").trailing_fill(true));
        if level.changed() {
            volume.percent = percent;
            volume.muted = false;
        }
        let committed = mute.clicked() || (level.changed() && !level.dragged()) || level.drag_stopped();
        SliderResponse { changed: *volume != before, committed, mute: mute_rect, rail: level.rect }
    })
    .inner
}

/// A speaker in `rect`: its waves follow the level; a cross when silent.
fn speaker(painter: &egui::Painter, rect: Rect, volume: Volume, color: Color32) {
    let s = rect.height() / 24.0;
    let at = |x: f32, y: f32| pos2(rect.min.x + x * s, rect.min.y + y * s);
    painter.add(Shape::convex_polygon(vec![at(3.0, 9.0), at(7.0, 9.0), at(12.0, 4.5), at(12.0, 19.5), at(7.0, 15.0), at(3.0, 15.0)], color, Stroke::NONE));
    let stroke = Stroke::new(2.0 * s, color);
    if volume.silent() {
        painter.line_segment([at(15.5, 9.0), at(21.5, 15.0)], stroke);
        painter.line_segment([at(21.5, 9.0), at(15.5, 15.0)], stroke);
        return;
    }
    let waves: &[f32] = match volume.percent {
        0..=33 => &[4.0],
        34..=66 => &[4.0, 7.5],
        _ => &[4.0, 7.5, 11.0],
    };
    for radius in waves {
        let points = (0..=12).map(|i| {
            let angle = (i as f32 / 12.0 - 0.5) * 1.6;
            at(12.0 + radius * angle.cos(), 12.0 + radius * angle.sin())
        });
        painter.add(Shape::line(points.collect(), stroke));
    }
}
