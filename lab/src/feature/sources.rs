//! The Sources tab: the engine source the demonstration's claims rest on, listed beside the dock's buttons.

use super::{Feature, FeatureLayout, FeatureState};
use crate::i18n::tr;
use crate::ui::{Ink, UiInput};

pub struct SourcesFeature;

impl Feature for SourcesFeature {
    fn id(&self) -> &'static str {
        "sources"
    }

    fn label(&self) -> Option<String> {
        Some(tr("controls-source").to_string())
    }

    fn order(&self) -> i32 {
        30
    }

    fn layout(&self) -> FeatureLayout {
        FeatureLayout { side: true, ..Default::default() }
    }

    fn state(&self) -> Box<dyn FeatureState> {
        Box::new(Sources)
    }
}

struct Sources;

impl FeatureState for Sources {
    fn side(&mut self, ink: &Ink, rect: egui::Rect, input: &UiInput) {
        crate::ui::draw_source_list(ink, rect, input.state.demo);
    }
}
