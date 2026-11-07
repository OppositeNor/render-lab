//! The interactive viewer's windows: the Scene window (what the scene holds, and the selected entity's
//! transform, src/ui/scene_window.rs), the Controls window (the tabs of src/feature/ and their values,
//! src/controls.rs), and the Panel window (the frame's record, readout, code, or graph, src/ui/panel.rs).
//!
//! Each window is docked to an edge of the area between the header and the footer (left, right, or the
//! bottom), the viewport taking what the docked windows leave, or floats over the viewport where it was
//! dropped. Every window has the same frame: a title bar (drag it to move the window; drop it near an edge
//! to dock it there; double-click it to put the window back in its default place), a chevron that
//! collapses it to its title bar, and an edge or a corner that resizes it. The recorder never draws them:
//! a recording keeps its fixed 1920 × 1080 layout (`Layout::with_ui_scale`).

use egui::{pos2, vec2, Pos2, Rect};

/// The height of a window's title bar, in points.
pub const TITLE: f32 = 40.0;
/// The gap between docked windows, and between them and the viewport.
pub const GAP: f32 = 12.0;
/// How near an edge of the area a dropped title bar docks its window there.
pub const DOCK_REACH: f32 = 56.0;
/// The least a floating window keeps.
pub const MIN_FLOAT: egui::Vec2 = vec2(300.0, 160.0);
/// The width of a collapsed window's title bar.
pub const COLLAPSED_WIDTH: f32 = 360.0;

/// One of the viewer's windows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WindowId {
    Scene,
    Controls,
    Panel,
}

impl WindowId {
    pub const ALL: [WindowId; 3] = [WindowId::Scene, WindowId::Controls, WindowId::Panel];

    pub fn index(self) -> usize {
        self as usize
    }

    /// Its name in the settings file (`viewer_window_<key>`).
    pub fn key(self) -> &'static str {
        match self {
            WindowId::Scene => "scene",
            WindowId::Controls => "controls",
            WindowId::Panel => "panel",
        }
    }

    /// Where it is until the user moves it.
    pub fn default_place(self) -> WindowPlace {
        match self {
            WindowId::Scene => WindowPlace { dock: Dock::Float, float: [56.0, 128.0, 440.0, 640.0], size: 440.0, wide: 440.0, collapsed: true, open: true },
            WindowId::Controls => WindowPlace { dock: Dock::Bottom, float: [200.0, 560.0, 1200.0, 260.0], size: 212.0, wide: 212.0, collapsed: false, open: true },
            WindowId::Panel => WindowPlace { dock: Dock::Right, float: [1200.0, 140.0, 590.0, 640.0], size: 590.0, wide: crate::text::CODE_PANEL_WIDTH, collapsed: false, open: true },
        }
    }
}

/// The edge a window is docked to, or none (it floats).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dock {
    Left,
    Right,
    Bottom,
    Float,
}

impl Dock {
    pub fn key(self) -> &'static str {
        match self {
            Dock::Left => "left",
            Dock::Right => "right",
            Dock::Bottom => "bottom",
            Dock::Float => "float",
        }
    }

    pub fn parse(key: &str) -> Option<Dock> {
        [Dock::Left, Dock::Right, Dock::Bottom, Dock::Float].into_iter().find(|d| d.key() == key)
    }
}

/// Where a window is, as the user left it (the settings keep it, src/config.rs).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowPlace {
    pub dock: Dock,
    /// Its place while it floats: x, y, width, height in canvas points.
    pub float: [f32; 4],
    /// Docked, its width (left, right) or height (bottom).
    pub size: f32,
    /// Docked left or right while its content is wide (the pseudo-code, the knowledge graph), its width.
    pub wide: f32,
    /// Only its title bar shows; a docked window then gives its room to the viewport.
    pub collapsed: bool,
    /// Shown at all (the header's Windows menu opens and closes it).
    pub open: bool,
}

/// A window as this frame draws it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowFrame {
    pub id: WindowId,
    /// The whole window: its title bar, then its body.
    pub frame: Rect,
    pub title: Rect,
    /// What its content draws in (empty when collapsed).
    pub body: Rect,
    pub dock: Dock,
    pub collapsed: bool,
    /// Its content is wide (`WindowPlace::wide`).
    pub wide: bool,
}

/// Which windows the frame has, and how.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Shown {
    pub scene: bool,
    pub controls: bool,
    pub panel: bool,
    /// The panel's content is wide.
    pub wide: bool,
}

impl Shown {
    fn has(&self, id: WindowId) -> bool {
        match id {
            WindowId::Scene => self.scene,
            WindowId::Controls => self.controls,
            WindowId::Panel => self.panel,
        }
    }
}

/// The windows placed in `area` (the room between the header and the footer): each shown window's frame
/// (by `WindowId::index`), and the viewport, what the docked ones leave. The bottom ones span the area's
/// width, the left and right ones the height above them; windows docked to one edge share it evenly.
pub fn arrange(area: Rect, places: &[WindowPlace; 3], shown: Shown) -> ([Option<WindowFrame>; 3], Rect) {
    let mut frames = [None; 3];
    let mut free = area;
    let visible = |id: WindowId| shown.has(id) && places[id.index()].open;
    let docked = |dock: Dock| WindowId::ALL.into_iter().filter(move |id| visible(*id) && places[id.index()].dock == dock && !places[id.index()].collapsed);
    let wide = |id: WindowId| id == WindowId::Panel && shown.wide;
    let size = |id: WindowId| {
        let p = &places[id.index()];
        if wide(id) { p.wide } else { p.size }
    };
    // The bottom: the tallest of its windows, side by side.
    let bottom: Vec<WindowId> = docked(Dock::Bottom).collect();
    if !bottom.is_empty() {
        let most = (free.height() - GAP - crate::ui::MIN_VIEWPORT.y).max(crate::ui::MIN_DOCK);
        let height = bottom.iter().map(|id| size(*id)).fold(0.0, f32::max).clamp(crate::ui::MIN_DOCK, most);
        let strip = Rect::from_min_max(pos2(free.min.x, free.max.y - height), free.max);
        for (id, rect) in bottom.iter().zip(split(strip, bottom.len(), true)) {
            frames[id.index()] = Some(frame(*id, rect, Dock::Bottom, false, wide(*id)));
        }
        free.max.y = strip.min.y - GAP;
    }
    // The sides: the widest of each side's windows, stacked.
    for dock in [Dock::Right, Dock::Left] {
        let ids: Vec<WindowId> = docked(dock).collect();
        if ids.is_empty() {
            continue;
        }
        let most = (free.width() - GAP - crate::ui::MIN_VIEWPORT.x).max(crate::ui::MIN_PANEL);
        let width = ids.iter().map(|id| size(*id)).fold(0.0, f32::max).clamp(crate::ui::MIN_PANEL, most);
        let column = match dock {
            Dock::Right => Rect::from_min_max(pos2(free.max.x - width, free.min.y), free.max),
            _ => Rect::from_min_max(free.min, pos2(free.min.x + width, free.max.y)),
        };
        for (id, rect) in ids.iter().zip(split(column, ids.len(), false)) {
            frames[id.index()] = Some(frame(*id, rect, dock, false, wide(*id)));
        }
        match dock {
            Dock::Right => free.max.x = column.min.x - GAP,
            _ => free.min.x = column.max.x + GAP,
        }
    }
    // Collapsed windows: their title bars at their edge's corner of the viewport, over it; floating windows
    // where they were dropped, kept on the viewport.
    let mut corners = [free.min.y; 2];
    for id in WindowId::ALL.into_iter().filter(|id| visible(*id)) {
        let p = &places[id.index()];
        if p.collapsed {
            let width = COLLAPSED_WIDTH.min(area.width());
            let (x, row) = match p.dock {
                Dock::Right => (free.max.x - width, &mut corners[1]),
                Dock::Float => (p.float[0].clamp(free.min.x, (free.max.x - width).max(free.min.x)), &mut corners[0]),
                _ => (free.min.x, &mut corners[0]),
            };
            let y = if p.dock == Dock::Float { p.float[1].clamp(free.min.y, (free.max.y - TITLE).max(free.min.y)) } else { *row };
            if p.dock != Dock::Float {
                *row += TITLE + GAP;
            }
            frames[id.index()] = Some(frame(id, Rect::from_min_size(pos2(x, y), vec2(width, TITLE)), p.dock, true, wide(id)));
        } else if p.dock == Dock::Float {
            let w = if wide(id) { p.float[2].max(p.wide) } else { p.float[2] };
            // Over the viewport, not under the docked windows (on the whole area if the viewport is too small).
            let room = if free.width() >= MIN_FLOAT.x && free.height() >= MIN_FLOAT.y { free } else { area };
            let size = vec2(w, p.float[3]).max(MIN_FLOAT).min(room.size());
            let min = pos2(p.float[0], p.float[1]).clamp(room.min, room.max - size);
            frames[id.index()] = Some(frame(id, Rect::from_min_size(min, size), Dock::Float, false, wide(id)));
        }
    }
    (frames, free)
}

/// `rect` cut into `n` equal parts, side by side (`across`) or stacked, `GAP` apart.
fn split(rect: Rect, n: usize, across: bool) -> Vec<Rect> {
    let n = n.max(1) as f32;
    if across {
        let w = (rect.width() - GAP * (n - 1.0)) / n;
        (0..n as usize).map(|i| Rect::from_min_size(pos2(rect.min.x + i as f32 * (w + GAP), rect.min.y), vec2(w, rect.height()))).collect()
    } else {
        let h = (rect.height() - GAP * (n - 1.0)) / n;
        (0..n as usize).map(|i| Rect::from_min_size(pos2(rect.min.x, rect.min.y + i as f32 * (h + GAP)), vec2(rect.width(), h))).collect()
    }
}

fn frame(id: WindowId, rect: Rect, dock: Dock, collapsed: bool, wide: bool) -> WindowFrame {
    let title = Rect::from_min_size(rect.min, vec2(rect.width(), TITLE));
    let body = if collapsed { Rect::from_min_size(title.left_bottom(), vec2(rect.width(), 0.0)) } else { Rect::from_min_max(title.left_bottom(), rect.max) };
    WindowFrame { id, frame: rect, title, body, dock, collapsed, wide }
}

/// The edge a title bar dropped at `pointer` docks its window to, if it is near one of `area`'s.
pub fn dock_at(area: Rect, pointer: Pos2) -> Option<Dock> {
    if !area.expand(DOCK_REACH).contains(pointer) {
        return None;
    }
    let (left, right, bottom) = (pointer.x - area.min.x, area.max.x - pointer.x, area.max.y - pointer.y);
    let nearest = left.min(right).min(bottom);
    (nearest < DOCK_REACH).then_some(if nearest == bottom { Dock::Bottom } else if nearest == left { Dock::Left } else { Dock::Right })
}

/// Where a window docked to `dock` would be, for the hint a drag shows before the drop.
pub fn dock_hint(area: Rect, dock: Dock) -> Rect {
    match dock {
        Dock::Left => Rect::from_min_size(area.min, vec2(DOCK_REACH * 3.0, area.height())),
        Dock::Right => Rect::from_min_max(pos2(area.max.x - DOCK_REACH * 3.0, area.min.y), area.max),
        _ => Rect::from_min_max(pos2(area.min.x, area.max.y - DOCK_REACH * 3.0), area.max),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> [WindowPlace; 3] {
        WindowId::ALL.map(WindowId::default_place)
    }

    const AREA: Rect = Rect { min: pos2(40.0, 112.0), max: pos2(1880.0, 1014.0) };

    #[test]
    fn docked_windows_take_their_edges_room() {
        let shown = Shown { scene: true, controls: true, panel: true, wide: false };
        let (frames, viewport) = arrange(AREA, &defaults(), shown);
        let controls = frames[WindowId::Controls.index()].unwrap();
        let panel = frames[WindowId::Panel.index()].unwrap();
        // The controls along the bottom, the whole width; the panel on the right above them.
        assert_eq!((controls.frame.min.x, controls.frame.max.x, controls.frame.max.y), (AREA.min.x, AREA.max.x, AREA.max.y));
        assert_eq!(controls.frame.height(), 212.0);
        assert_eq!((panel.frame.max.x, panel.frame.width(), panel.frame.max.y), (AREA.max.x, 590.0, controls.frame.min.y - GAP));
        assert_eq!(panel.body.min.y, panel.frame.min.y + TITLE);
        // The viewport: what they leave; the floating scene window does not take any of it.
        assert_eq!(viewport, Rect::from_min_max(AREA.min, pos2(panel.frame.min.x - GAP, controls.frame.min.y - GAP)));
        assert!(viewport.contains_rect(frames[WindowId::Scene.index()].unwrap().frame));
    }

    #[test]
    fn collapsed_closed_and_hidden_windows_give_their_room() {
        let mut places = defaults();
        places[WindowId::Panel.index()].collapsed = true;
        places[WindowId::Controls.index()].open = false;
        let shown = Shown { scene: false, controls: true, panel: true, wide: false };
        let (frames, viewport) = arrange(AREA, &places, shown);
        assert_eq!(viewport, AREA);
        assert!(frames[WindowId::Controls.index()].is_none() && frames[WindowId::Scene.index()].is_none());
        let panel = frames[WindowId::Panel.index()].unwrap();
        assert!(panel.collapsed && panel.body.height() == 0.0 && panel.frame.max.x == AREA.max.x);
    }

    #[test]
    fn wide_content_and_floating_windows_stay_on_the_area() {
        let mut places = defaults();
        let wide = Shown { scene: false, controls: false, panel: true, wide: true };
        assert_eq!(arrange(AREA, &places, wide).0[WindowId::Panel.index()].unwrap().frame.width(), crate::text::CODE_PANEL_WIDTH);
        places[WindowId::Panel.index()].dock = Dock::Float;
        places[WindowId::Panel.index()].float = [5000.0, -40.0, 10.0, 10.0];
        let panel = arrange(AREA, &places, Shown { wide: false, ..wide }).0[WindowId::Panel.index()].unwrap().frame;
        assert!(AREA.contains_rect(panel) && panel.width() >= MIN_FLOAT.x && panel.max.x == AREA.max.x && panel.min.y == AREA.min.y);
    }

    #[test]
    fn a_title_dropped_near_an_edge_docks_there() {
        assert_eq!(dock_at(AREA, pos2(60.0, 500.0)), Some(Dock::Left));
        assert_eq!(dock_at(AREA, pos2(1870.0, 500.0)), Some(Dock::Right));
        assert_eq!(dock_at(AREA, pos2(900.0, 1000.0)), Some(Dock::Bottom));
        assert_eq!(dock_at(AREA, pos2(900.0, 500.0)), None);
    }
}
