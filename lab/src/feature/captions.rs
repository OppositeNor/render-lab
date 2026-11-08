//! The Captions tab: a paused timeline's caption, to read again, with the one before it, beside the dock's
//! buttons. A timeline's tab: it shows when the timeline pauses.

use super::{Feature, FeatureLayout, FeatureState, Host};
use crate::i18n::tr;
use crate::ui::{Ink, UiInput};

pub struct CaptionsFeature;

impl Feature for CaptionsFeature {
    fn id(&self) -> &'static str {
        "captions"
    }

    fn label(&self) -> Option<String> {
        Some(tr("controls-captions").to_string())
    }

    fn order(&self) -> i32 {
        1000
    }

    fn timeline(&self) -> bool {
        true
    }

    fn layout(&self) -> FeatureLayout {
        FeatureLayout {
            side: true,
            ..Default::default()
        }
    }

    fn state(&self) -> Box<dyn FeatureState> {
        Box::new(Captions::default())
    }
}

/// The caption at or last before the timeline's frame, and the one before that.
#[derive(Default)]
struct Captions {
    previous: Option<String>,
    current: Option<String>,
}

impl FeatureState for Captions {
    fn update(&mut self, host: &mut dyn Host) {
        (self.previous, self.current) = host.captions();
    }

    fn side(&mut self, ink: &Ink, rect: egui::Rect, _input: &UiInput) {
        crate::ui::draw_caption_tab(
            ink,
            rect.shrink2(egui::vec2(16.0, 0.0)),
            self.previous.as_deref(),
            self.current.as_deref(),
        );
    }
}
