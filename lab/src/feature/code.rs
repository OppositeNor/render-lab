//! The Code tab: the demonstration's pseudo-code in the panel (`PanelMode::Code`, which a story shows too); the
//! dock picks a listing and lights its steps.

use super::{Feature, FeatureState, Host, Page, ShowTab, Snapshot, StoryPanel};
use crate::controls::Dock;
use crate::i18n::{args, tr, tr_with};
use crate::investigation;
use crate::palette;
use crate::shot::PanelMode;
use crate::ui::ViewerAction;
use anyhow::{Result, anyhow};
use egui::{RichText, Ui};

pub struct CodeFeature;

impl Feature for CodeFeature {
    fn id(&self) -> &'static str {
        "code"
    }

    fn label(&self) -> Option<String> {
        Some(tr("controls-code").to_string())
    }

    fn order(&self) -> i32 {
        20
    }

    fn panels(&self) -> &'static [PanelMode] {
        &[PanelMode::Code]
    }

    /// `viewer-code:<listing>[@<tag>]`: the listing on its demonstration, the lines of the tag lit (default:
    /// its first tag), in the listing style in use (`--listing-style`).
    fn pages(&self) -> &'static [Page] {
        &[Page {
            name: "viewer-code",
            help: ":<listing>[@<tag>] (the Code tab on that listing, the tag's lines lit; see --listing-style)",
        }]
    }

    fn snapshot_demo<'a>(&self, _page: &str, args: Option<&'a str>) -> Option<&'a str> {
        let listing = args?.split_once('@').map_or(args?, |(l, _)| l);
        investigation::current()
            .listing(listing)
            .and_then(|l| l.demo.as_deref())
    }

    /// The tab shows the demonstration's first listing in the panel; leaving it gives the demonstration's
    /// panel and call-outs back.
    fn show(&self, tab: &mut ShowTab) {
        let state = &mut *tab.state;
        if tab.shown {
            if state.panel != PanelMode::Code {
                state.panel = PanelMode::Code;
                state.code = investigation::current()
                    .listing_for_demo(state.demo)
                    .map(|l| l.name.to_string());
                state.highlight.clear();
            }
        } else if state.panel == PanelMode::Code {
            state.panel = tab.defaults.panel;
            state.highlight = tab.defaults.highlight.clone();
            state.code = None;
        }
    }

    fn state(&self) -> Box<dyn FeatureState> {
        Box::new(Code::default())
    }
}

#[derive(Default)]
struct Code {
    /// The listing the dock picked, to show.
    picked: Option<&'static str>,
    /// A snapshot page's report line.
    report: String,
}

impl FeatureState for Code {
    /// The listing picked shows, unlit.
    fn update(&mut self, host: &mut dyn Host) {
        if let Some(name) = self.picked.take().filter(|_| host.interactive()) {
            let state = host.state_mut();
            state.code = Some(name.to_string());
            state.highlight.clear();
        }
    }

    /// This demonstration's listings, and one button per step (tag) that lights its lines in the panel.
    fn dock(&mut self, dock: &mut Dock, ui: &mut Ui) -> bool {
        let demo = dock.state.demo;
        let listings: Vec<_> = investigation::current()
            .listings()
            .iter()
            .filter(|l| l.demo.as_deref() == Some(demo.key()))
            .collect();
        if listings.is_empty() {
            ui.label(RichText::new(tr("controls-no_listing")).color(palette::text_dim()));
            return true;
        }
        let engine = investigation::current().wording().engine_short;
        let hint = tr_with("controls-listing_hint", args!(engine = engine));
        dock.group(ui, tr("controls-listing"), Some(&hint), |d, ui| {
            for listing in &listings {
                let selected = d.state.code.as_deref() == Some(listing.name);
                if ui.selectable_label(selected, listing.title()).clicked() {
                    self.picked = Some(listing.name);
                }
            }
        });
        let Some(listing) = dock
            .state
            .code
            .as_deref()
            .and_then(|c| investigation::current().listing(c))
        else {
            return true;
        };
        let mut tags: Vec<&str> = Vec::new();
        for line in &listing.lines {
            for tag in &line.tags {
                if !tags.contains(&tag.as_str()) {
                    tags.push(tag);
                }
            }
        }
        dock.group(
            ui,
            tr("controls-steps"),
            Some(tr("controls-steps_hint")),
            |d, ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.set_max_width(520.0);
                    for tag in tags {
                        let lit = d.state.highlight.iter().any(|h| h == tag);
                        if ui.selectable_label(lit, tag).clicked() {
                            d.actions.push(ViewerAction::SetHighlight(if lit {
                                Vec::new()
                            } else {
                                vec![tag.to_string()]
                            }));
                        }
                    }
                });
            },
        );
        true
    }

    /// The listing (`ShotState::code`), the lines of `highlight`'s tags lit.
    fn story_panel(&mut self, panel: &mut StoryPanel) {
        let input = panel.input;
        if !input.layout.panel_shown {
            return;
        }
        let rect = input.layout.panel;
        crate::paint::panel_card(panel.ink.painter, rect, 12);
        if let Some(listing) = input
            .state
            .code
            .as_deref()
            .and_then(|c| investigation::current().listing(c))
        {
            crate::ui::draw_code(
                panel.ink,
                rect,
                listing,
                &input.state.highlight,
                panel.report,
            );
        }
    }

    /// The listing's demonstration at its defaults on the tab, the listing and its tag shown, settled over
    /// sixteen frames.
    fn snapshot(
        &mut self,
        _page: &str,
        args: Option<&str>,
        step: usize,
        host: &mut dyn Host,
    ) -> Result<Snapshot> {
        match step {
            0 => {
                let args = args.ok_or_else(|| {
                    anyhow!("a code page names its listing: viewer-code:<listing>[@<tag>]")
                })?;
                let (listing, tag) = args
                    .split_once('@')
                    .map_or((args, None), |(l, t)| (l, Some(t)));
                let current = investigation::current();
                let found = current.listing(listing).ok_or_else(|| {
                    let names: Vec<&str> = current.listings().iter().map(|l| l.name).collect();
                    anyhow!("no listing `{listing}`; known: {}", names.join(", "))
                })?;
                let defaults = host.default_state(host.state().demo);
                host.show(defaults);
                let tag = tag.map(str::to_string).or_else(|| {
                    found
                        .lines
                        .iter()
                        .flat_map(|l| l.tags.iter())
                        .next()
                        .cloned()
                });
                let state = host.state_mut();
                state.code = Some(found.name.to_string());
                state.highlight = tag.iter().cloned().collect();
                self.report = format!(
                    "listing {listing} as {}, lit: {}",
                    crate::pseudocode::style().key(),
                    tag.unwrap_or_else(|| "nothing".into())
                );
                Ok(Snapshot::Settle)
            }
            1..16 => Ok(Snapshot::Settle),
            _ => Ok(Snapshot::Done(self.report.clone())),
        }
    }
}
