//! The performance panel (`PanelMode::Performance`, which a story shows, and the Experiments tab's button):
//! the GPU time of each pass, measured as the frame renders (src/timing.rs), the passes `highlight` names
//! called out. No tab of its own.

use super::{Feature, FeatureState, Host, Page, Snapshot, StoryPanel};
use crate::shot::PanelMode;
use anyhow::Result;

pub struct PerformanceFeature;

impl Feature for PerformanceFeature {
    fn id(&self) -> &'static str {
        "performance"
    }

    fn label(&self) -> Option<String> {
        None
    }

    fn order(&self) -> i32 {
        0
    }

    fn panels(&self) -> &'static [PanelMode] {
        &[PanelMode::Performance]
    }

    fn pages(&self) -> &'static [Page] {
        &[Page {
            name: "viewer-performance",
            help: "[:<demo>] (the performance panel, its most expensive pass called out)",
        }]
    }

    fn snapshot_demo<'a>(&self, _page: &str, args: Option<&'a str>) -> Option<&'a str> {
        args
    }

    fn state(&self) -> Box<dyn FeatureState> {
        Box::new(Performance::default())
    }
}

#[derive(Default)]
struct Performance {
    /// A snapshot page's report line.
    report: String,
}

impl FeatureState for Performance {
    fn story_panel(&mut self, panel: &mut StoryPanel) {
        let input = panel.input;
        if !input.layout.panel_shown {
            return;
        }
        let rect = input.layout.panel;
        crate::paint::panel_card(panel.ink.painter, rect, 12);
        if let Some(view) = input.performance {
            crate::ui::draw_performance(panel.ink, rect, view, &input.state.highlight);
        }
    }

    /// The demonstration at its defaults with the panel, measured over 60 frames; then the most expensive pass
    /// called out, as a story's `highlight` would, settled over four.
    fn snapshot(
        &mut self,
        _page: &str,
        _args: Option<&str>,
        step: usize,
        host: &mut dyn Host,
    ) -> Result<Snapshot> {
        match step {
            0 => {
                let defaults = host.default_state(host.state().demo);
                *host.state_mut() = defaults;
                host.state_mut().panel = PanelMode::Performance;
                Ok(Snapshot::Settle)
            }
            1..60 => Ok(Snapshot::Settle),
            60 => {
                let passes = host
                    .renderer()
                    .and_then(|r| r.timing.as_ref().map(|t| t.summary()))
                    .unwrap_or_default();
                let top = passes
                    .iter()
                    .max_by(|a, b| a.1.total_cmp(&b.1))
                    .map(|p| p.0.clone());
                if let Some(name) = &top {
                    host.state_mut().highlight = vec![name.clone()];
                }
                let list: Vec<String> = passes
                    .iter()
                    .map(|(n, mean, _)| format!("{n} {mean:.2} ms"))
                    .collect();
                self.report = format!(
                    "{}; called out: {}",
                    if list.is_empty() {
                        "no GPU timestamps".into()
                    } else {
                        list.join(", ")
                    },
                    top.unwrap_or_else(|| "none".into())
                );
                Ok(Snapshot::Settle)
            }
            61..64 => Ok(Snapshot::Settle),
            _ => Ok(Snapshot::Done(self.report.clone())),
        }
    }
}
