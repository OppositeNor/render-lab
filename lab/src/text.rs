//! Text: the bundled fonts, the minimum text size the lab draws, and `Ink`,
//! which draws text and records the smallest size it used.

use crate::{palette, resources};
use egui::{vec2, Align2, Color32, FontFamily, FontId, Painter, Pos2, Rect, Vec2};
use std::cell::Cell;
use std::sync::Arc;

pub const MIN_TEXT_PT: f32 = 24.0;
/// The homepage's and the viewer's default size of text and panels relative to the video's.
pub const DEFAULT_UI_SCALE: f32 = 0.75;
/// Width of the pseudo-code panel: 60 monospace columns at 24 px.
pub const CODE_PANEL_WIDTH: f32 = 930.0;
pub const REFERENCE: Vec2 = vec2(1920.0, 1080.0);

pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    // Chinese text falls back to Source Han Sans (SIL OFL 1.1) in every family.
    for (name, id) in [
        ("DejaVuSans", "rlstd.font/dejavu-sans"),
        ("DejaVuSans-Bold", "rlstd.font/dejavu-sans-bold"),
        ("DejaVuSansMono", "rlstd.font/dejavu-sans-mono"),
        ("DejaVuSansMono-Bold", "rlstd.font/dejavu-sans-mono-bold"),
        ("SourceHanSans", "rlstd.font/source-han-sans"),
        ("SourceHanSans-Bold", "rlstd.font/source-han-sans-bold"),
    ] {
        let mut data = egui::FontData::from_static(resources::get(id).bytes());
        // Source Han Sans' metrics put its glyphs lower than DejaVu's on a shared
        // line (by about 0.145 em, measured on the lab's 24 px text): raised, a
        // mixed line ("MIT 许可证") keeps one baseline.
        if name.starts_with("SourceHanSans") {
            data.tweak.y_offset_factor = -0.145;
        }
        fonts.font_data.insert(name.to_owned(), Arc::new(data));
    }
    for (family, first) in [(FontFamily::Proportional, "DejaVuSans"), (FontFamily::Monospace, "DejaVuSansMono")] {
        let list = fonts.families.entry(family).or_default();
        list.insert(0, first.into());
        list.insert(1, "SourceHanSans".into());
    }
    fonts.families.insert(FontFamily::Name("Bold".into()), vec!["DejaVuSans-Bold".into(), "DejaVuSans".into(), "SourceHanSans-Bold".into()]);
    fonts.families.insert(FontFamily::Name("MonoBold".into()), vec!["DejaVuSansMono-Bold".into(), "DejaVuSansMono".into(), "SourceHanSans-Bold".into()]);
    ctx.set_fonts(fonts);
    ctx.all_styles_mut(|style| {
        use egui::TextStyle::*;
        for (text_style, size, family) in [(Small, 24.0, FontFamily::Proportional), (Body, 24.0, FontFamily::Proportional), (Button, 24.0, FontFamily::Proportional), (Monospace, 24.0, FontFamily::Monospace), (Heading, 30.0, FontFamily::Name("Bold".into()))] {
            style.text_styles.insert(text_style, FontId::new(size, family));
        }
        style.spacing.button_padding = vec2(12.0, 6.0);
        style.spacing.item_spacing = vec2(8.0, 6.0);
    });
    restyle(ctx);
}

/// Gives `ctx` the theme's colours (src/palette.rs) when they changed since
/// it last had them; every egui pass of the lab starts with it.
pub fn restyle(ctx: &egui::Context) {
    let id = egui::Id::unique("render-lab-theme-generation");
    let generation = palette::theme_generation();
    if ctx.data(|d| d.get_temp::<u64>(id)) == Some(generation) {
        return;
    }
    ctx.data_mut(|d| d.insert_temp(id, generation));
    let theme = palette::theme();
    ctx.all_styles_mut(|style| {
        style.visuals = egui::Visuals::dark();
        style.visuals.widgets.inactive.weak_bg_fill = theme.panel_raised;
        style.visuals.widgets.hovered.weak_bg_fill = theme.hover;
        style.visuals.selection.bg_fill = theme.selection;
    });
}

pub fn bold() -> FontFamily {
    FontFamily::Name("Bold".into())
}
pub fn mono_bold() -> FontFamily {
    FontFamily::Name("MonoBold".into())
}

/// Text drawing that records the smallest font size used.
pub struct Ink<'a> {
    pub painter: &'a Painter,
    pub min_size: Cell<f32>,
}

impl<'a> Ink<'a> {
    pub fn new(painter: &'a Painter) -> Self {
        Ink { painter, min_size: Cell::new(f32::INFINITY) }
    }

    pub fn font(&self, size: f32, family: FontFamily) -> FontId {
        self.min_size.set(self.min_size.get().min(size));
        FontId::new(size, family)
    }

    pub fn text(&self, pos: Pos2, align: Align2, text: impl ToString, size: f32, family: FontFamily, color: Color32) -> Rect {
        let font = self.font(size, family);
        self.painter.text(pos, align, text, font, color)
    }

    pub fn measure(&self, text: &str, size: f32, family: FontFamily) -> Vec2 {
        self.painter.layout_no_wrap(text.to_string(), FontId::new(size, family), Color32::WHITE).size()
    }

    /// Wrapped text inside `width`; returns the bottom y.
    pub fn wrapped(&self, pos: Pos2, width: f32, text: &str, size: f32, family: FontFamily, color: Color32) -> f32 {
        let font = self.font(size, family);
        let galley = self.painter.layout(text.to_string(), font, color, width);
        let h = galley.size().y;
        self.painter.galley(pos, galley, color);
        pos.y + h
    }

    /// Truncate from the left with an ellipsis so the text fits `width`.
    pub fn fit_left(&self, text: &str, width: f32, size: f32, family: FontFamily) -> String {
        if self.measure(text, size, family.clone()).x <= width {
            return text.to_string();
        }
        let chars: Vec<char> = text.chars().collect();
        for start in 1..chars.len() {
            let candidate: String = std::iter::once('…').chain(chars[start..].iter().copied()).collect();
            if self.measure(&candidate, size, family.clone()).x <= width {
                return candidate;
            }
        }
        text.to_string()
    }

    pub fn fit_right(&self, text: &str, width: f32, size: f32, family: FontFamily) -> String {
        if self.measure(text, size, family.clone()).x <= width {
            return text.to_string();
        }
        let chars: Vec<char> = text.chars().collect();
        for end in (1..chars.len()).rev() {
            let candidate: String = chars[..end].iter().copied().chain(std::iter::once('…')).collect();
            if self.measure(&candidate, size, family.clone()).x <= width {
                return candidate;
            }
        }
        text.to_string()
    }
}
