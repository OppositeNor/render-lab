//! The wipe's line grabbed in the viewport: a band around it that a drag moves, and Shift+Left/Right nudging
//! it, through the same split the panel's slider sets.

use super::{Asked, Experiments};
use crate::experiment::Compare;
use crate::feature::StoryPanel;
use egui::{CursorIcon, Key, Rect, Sense, pos2, vec2};

/// How far either side of the line the pointer grabs it, in points.
const GRAB: f32 = 6.0;
/// How far Shift+Left/Right moves the line, as a share of the view's width.
const NUDGE: f32 = 0.01;

impl Experiments {
    /// While a wipe is shown (not on a timeline): its line's band, which shows the resize cursor and takes the
    /// pointer from the camera, a drag of it, and the keys.
    pub(super) fn wipe(&mut self, panel: &mut StoryPanel) {
        let Some(compare) = panel.input.compare.filter(|c| c.mode == Compare::Wipe) else {
            return;
        };
        if !panel.interactive {
            return;
        }
        let view = panel.input.layout.viewport;
        let x = view.min.x + view.width() * compare.split.clamp(0.0, 1.0);
        let band = Rect::from_min_max(pos2(x - GRAB, view.min.y), pos2(x + GRAB, view.max.y));
        let response = egui::Area::new(egui::Id::unique("experiments-wipe"))
            .fixed_pos(band.min)
            .constrain(false)
            .fade_in(false)
            .show(panel.ctx, |ui| {
                ui.allocate_exact_size(vec2(band.width(), band.height()), Sense::drag())
                    .1
            })
            .inner;
        if response.hovered() || response.dragged() {
            panel.ctx.set_cursor_icon(CursorIcon::ResizeHorizontal);
            panel.pointer = true;
        }
        if response.dragged()
            && let Some(p) = response.interact_pointer_pos()
        {
            self.asked
                .push(Asked::SetSplit((p.x - view.min.x) / view.width()));
        }
        // Shift+Left/Right nudge it, while no widget has the keys (plain Right steps the animation).
        let typing = panel.ctx.memory(|m| m.focused().is_some());
        let nudge = panel.ctx.input(|i| {
            let shift = i.modifiers.shift;
            (shift && i.key_pressed(Key::ArrowRight)) as i32 as f32
                - (shift && i.key_pressed(Key::ArrowLeft)) as i32 as f32
        });
        if !typing && nudge != 0.0 {
            self.asked
                .push(Asked::SetSplit(compare.split + nudge * NUDGE));
        }
    }
}
