//! Popups: short messages over a window (the homepage or the viewer), in
//! cards stacked at its bottom-right corner, newest at the bottom. A popup
//! has a title, a message, and buttons; it closes when one is clicked, or on
//! its own after `seconds` (while the pointer is on it, it stays). A button's
//! action other than closing goes back to the window (`Popups::take_actions`),
//! which carries it out, so drawing a popup has no effects of its own.

use crate::i18n::tr;
use crate::palette;
use crate::ui;
use egui::{vec2, Align2, CornerRadius, FontFamily, FontId, RichText, Stroke, Vec2};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// What a button does besides closing its popup.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Close,
    /// Show a folder in the system's file manager.
    OpenFolder(PathBuf),
}

pub struct Popup {
    pub title: String,
    pub message: String,
    pub buttons: Vec<(String, Action)>,
    /// Closes on its own this long after it opens, or after the pointer
    /// leaves it; `None` waits for a button.
    pub seconds: Option<f32>,
    /// When it opened, or when the pointer last left it.
    since: Instant,
}

/// The last half second of a popup that closes on its own fades it out.
const FADE: f32 = 0.5;
const WIDTH: f32 = 640.0;

impl Popup {
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Popup {
        Popup { title: title.into(), message: message.into(), buttons: Vec::new(), seconds: None, since: Instant::now() }
    }

    pub fn button(mut self, label: impl Into<String>, action: Action) -> Popup {
        self.buttons.push((label.into(), action));
        self
    }

    pub fn closing_after(mut self, seconds: f32) -> Popup {
        self.seconds = Some(seconds);
        self
    }

    /// Where a screenshot (F11, `readback::screenshot`) was saved, or why it
    /// was not.
    pub fn screenshot(result: &anyhow::Result<PathBuf>) -> Popup {
        match result {
            Ok(path) => {
                let popup = Popup::new(tr("popup.screenshot_saved"), path.display().to_string());
                let popup = match path.parent() {
                    Some(dir) => popup.button(tr("popup.open_folder"), Action::OpenFolder(dir.to_path_buf())),
                    None => popup,
                };
                popup.button(tr("popup.close"), Action::Close).closing_after(8.0)
            }
            Err(e) => Popup::new(tr("popup.screenshot_failed"), format!("{e:#}")).button(tr("popup.close"), Action::Close),
        }
    }
}

#[derive(Default)]
pub struct Popups {
    shown: Vec<Popup>,
    actions: Vec<Action>,
    /// Draw nothing this frame (the frame a screenshot is taken of).
    pub hidden: bool,
}

impl Popups {
    pub fn push(&mut self, popup: Popup) {
        self.shown.push(popup);
    }

    /// The actions of the buttons clicked since the last call, for the window
    /// to carry out.
    pub fn take_actions(&mut self) -> Vec<Action> {
        std::mem::take(&mut self.actions)
    }

    /// Draws the popups above everything else on a canvas of `canvas` points,
    /// and closes those whose time is up or whose button was clicked.
    pub fn draw(&mut self, ctx: &egui::Context, canvas: Vec2) {
        let now = Instant::now();
        self.shown.retain(|p| p.seconds.is_none_or(|s| now < p.since + Duration::from_secs_f32(s)));
        if self.shown.is_empty() || std::mem::take(&mut self.hidden) {
            return;
        }
        // Above the viewer's footer and the homepage's hint line.
        let corner = canvas - vec2(40.0, 110.0);
        let mut closed = Vec::new();
        egui::Area::new(egui::Id::unique("popups")).order(egui::Order::Foreground).pivot(Align2::RIGHT_BOTTOM).fixed_pos(corner.to_pos2()).fade_in(false).constrain(false).show(ctx, |area| {
            area.spacing_mut().item_spacing.y = 16.0;
            for (i, popup) in self.shown.iter_mut().enumerate() {
                let left = popup.seconds.map(|s| s - (now - popup.since).as_secs_f32());
                let opacity = left.map_or(1.0, |left| (left / FADE).clamp(0.0, 1.0));
                let frame = egui::Frame::new()
                    .fill(palette::PANEL.gamma_multiply(0.95 * opacity))
                    .stroke(Stroke::new(2.0, palette::ACCENT.gamma_multiply(opacity)))
                    .corner_radius(CornerRadius::same(16))
                    .inner_margin(egui::Margin::symmetric(28, 22));
                let card = frame.show(area, |ui| {
                    ui.set_width(WIDTH);
                    ui.multiply_opacity(opacity);
                    ui.label(RichText::new(&popup.title).font(FontId::new(30.0, ui::bold())).color(palette::TEXT));
                    ui.add_space(6.0);
                    ui.add(egui::Label::new(RichText::new(&popup.message).font(FontId::new(24.0, FontFamily::Proportional)).color(palette::TEXT_DIM)).wrap());
                    if !popup.buttons.is_empty() {
                        ui.add_space(14.0);
                        // A row as tall as its buttons, laid out from the right edge.
                        ui.allocate_ui_with_layout(vec2(WIDTH, 48.0), egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.spacing_mut().item_spacing.x = 14.0;
                            // Right to left: the last button sits at the right edge.
                            for (label, action) in popup.buttons.iter().rev() {
                                let button = egui::Button::new(RichText::new(label).font(FontId::new(24.0, ui::bold()))).min_size(vec2(150.0, 48.0));
                                if ui.add(button).clicked() {
                                    if *action != Action::Close {
                                        self.actions.push(action.clone());
                                    }
                                    closed.push(i);
                                }
                            }
                        });
                    }
                });
                // The pointer on a popup holds it open; its time starts again when it leaves.
                if card.response.contains_pointer() {
                    popup.since = now;
                }
            }
        });
        for i in closed.into_iter().rev() {
            self.shown.remove(i);
        }
    }
}

/// Carries out an action a popup's button asked for.
pub fn perform(action: Action) {
    match action {
        Action::Close => {}
        Action::OpenFolder(dir) => {
            let opener = if cfg!(target_os = "windows") {
                "explorer"
            } else if cfg!(target_os = "macos") {
                "open"
            } else {
                "xdg-open"
            };
            if let Err(e) = std::process::Command::new(opener).arg(&dir).spawn() {
                eprintln!("could not open {}: {e}", dir.display());
            }
        }
    }
}
