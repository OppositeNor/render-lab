//! The Parameters tab: the investigation's own dock (`Investigation::dock`), the sliders of the demonstration
//! shown. The viewer opens on it.

use super::{Feature, FeatureState};
use crate::i18n::tr;
use crate::ui::DockTab;

/// The tab (the dock's first, and the one the viewer opens on).
pub const TAB: DockTab = DockTab("parameters");

pub struct ParametersFeature;

impl Feature for ParametersFeature {
    fn id(&self) -> &'static str {
        TAB.0
    }

    fn label(&self) -> Option<String> {
        Some(tr("controls-sliders").to_string())
    }

    fn order(&self) -> i32 {
        0
    }

    /// No dock of its own: a tab without one shows the investigation's.
    fn state(&self) -> Box<dyn FeatureState> {
        struct Parameters;
        impl FeatureState for Parameters {}
        Box::new(Parameters)
    }
}
