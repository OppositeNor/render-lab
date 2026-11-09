//! The Prerequisites tab: what the audience should know before the chapter, from the knowledge graph
//! (src/knowledge.rs), in the dock; the panel shows the graph (`PanelMode::Knowledge`, which a story shows too),
//! and its button shows it full screen over the viewer.

use super::{Feature, FeatureState, Host, Page, ShowTab, Snapshot, StoryPanel};
use crate::controls::Dock;
use crate::i18n::{args, tr, tr_with};
use crate::investigation::Investigation;
use crate::palette;
use crate::shot::{PanelMode, ShotState};
use crate::ui::{KnowledgeContext, ViewerAction};
use anyhow::Result;
use egui::{Color32, RichText, Ui};

pub struct PrerequisitesFeature;

impl Feature for PrerequisitesFeature {
    fn id(&self) -> &'static str {
        "prerequisites"
    }

    fn label(&self) -> Option<String> {
        Some(tr("controls-prerequisites").to_string())
    }

    fn order(&self) -> i32 {
        40
    }

    /// The current investigation's package has a knowledge graph (the viewer's features are the current one's).
    fn available(&self, _investigation: &dyn Investigation) -> bool {
        crate::knowledge::current().is_some()
    }

    /// `viewer-prerequisites[:<demo>]` (the graph in the panel, fitted to the demonstration's chapters),
    /// `viewer-knowledge[:<demo>]` (zoomed onto the first topic of the audience's level, selected),
    /// `viewer-knowledge-graph[:<demo>]` (the graph full screen), `viewer-knowledge-compact[:<demo>]` (full screen,
    /// only the topics its context needs).
    fn panels(&self) -> &'static [PanelMode] {
        &[PanelMode::Knowledge]
    }

    fn pages(&self) -> &'static [Page] {
        &[
            Page {
                name: "viewer-prerequisites",
                help: "[:<demo>] (the Prerequisites tab, the knowledge graph in the panel)",
            },
            Page {
                name: "viewer-knowledge",
                help: "[:<demo>] (that panel zoomed onto a topic of the audience's level, selected)",
            },
            Page {
                name: "viewer-knowledge-graph",
                help: "[:<demo>] (the knowledge graph full screen over the viewer)",
            },
            Page {
                name: "viewer-knowledge-compact",
                help: "[:<demo>] (full screen, only the topics its context needs)",
            },
        ]
    }

    fn snapshot_demo<'a>(&self, _page: &str, args: Option<&'a str>) -> Option<&'a str> {
        args
    }

    /// The tab shows the knowledge graph in the panel; leaving it gives the demonstration's panel back.
    fn show(&self, tab: &mut ShowTab) {
        if tab.shown {
            tab.state.panel = PanelMode::Knowledge;
        } else if tab.state.panel == PanelMode::Knowledge {
            tab.state.panel = tab.defaults.panel;
        }
    }

    fn state(&self) -> Box<dyn FeatureState> {
        Box::new(Prerequisites::default())
    }
}

#[derive(Default)]
struct Prerequisites {
    /// The chapters the tab and the panel are about, and a topic to focus once (a snapshot's).
    context: KnowledgeContext,
    /// The graph is full screen over the viewer.
    overlay: bool,
    /// A snapshot page's report line.
    report: String,
}

impl FeatureState for Prerequisites {
    /// On a timeline, the current chapter; else those whose demonstrations include this one.
    fn update(&mut self, host: &mut dyn Host) {
        self.context.chapters = match host.timeline_chapter() {
            Some(chapter) => vec![chapter],
            None => crate::knowledge::current()
                .map(|g| g.chapters_of_demo(host.state().demo.key()))
                .unwrap_or_default(),
        };
    }

    fn dock(&mut self, dock: &mut Dock, ui: &mut Ui) -> bool {
        self.prerequisites(dock, ui);
        true
    }

    /// The graph in the panel; its button shows it full screen. The wheel zooms it, not the camera.
    fn story_panel(&mut self, panel: &mut StoryPanel) {
        let rect = panel.input.layout.panel;
        let pressed = crate::ui::draw_knowledge_panel(panel.ctx, panel.ink, rect, &self.context);
        if panel.interactive {
            self.overlay |= pressed;
            // Its input area is in the background layer, which egui does not count as over the UI.
            panel.pointer |= panel
                .ctx
                .pointer_hover_pos()
                .is_some_and(|p| rect.contains(p));
        }
    }

    fn overlay(&mut self, panel: &mut StoryPanel) {
        if self.overlay {
            panel.pointer = true;
            if crate::ui::draw_knowledge_overlay(
                panel.ctx,
                panel.input.layout.canvas,
                &self.context,
            ) {
                self.overlay = false;
            }
        }
    }

    fn covers(&self, _state: &ShotState) -> bool {
        self.overlay
    }

    fn uncover(&mut self) {
        self.overlay = false;
    }

    /// The demonstration at its defaults on the tab, settled over sixteen frames.
    fn snapshot(
        &mut self,
        page: &str,
        _args: Option<&str>,
        step: usize,
        host: &mut dyn Host,
    ) -> Result<Snapshot> {
        match step {
            0 => {
                if page == "viewer-knowledge-compact" {
                    crate::ui::set_knowledge_compact(true);
                }
                let defaults = host.default_state(host.state().demo);
                host.show(defaults);
                self.overlay =
                    matches!(page, "viewer-knowledge-graph" | "viewer-knowledge-compact");
                // The chapters whose demonstrations include this one (no timeline is shown).
                let graph = crate::knowledge::current().expect("the tab has a graph");
                let chapters = graph.chapters_of_demo(host.state().demo.key());
                let focus = (page == "viewer-knowledge").then(|| {
                    let context = graph.context(&chapters);
                    let p = graph.audience();
                    p.level
                        .iter()
                        .find(|&&i| context[i])
                        .or(p.level.first())
                        .map(|&i| graph.nodes[i].id.clone())
                });
                let focus = focus.flatten();
                self.report = format!(
                    "demonstration {}, chapters {chapters:?}{}",
                    host.state().demo.key(),
                    focus
                        .as_ref()
                        .map(|f| format!(", focused on {f}"))
                        .unwrap_or_default()
                );
                if focus.is_some() {
                    self.context.focus = focus;
                }
                Ok(Snapshot::Settle)
            }
            1..16 => Ok(Snapshot::Settle),
            _ => Ok(Snapshot::Done(self.report.clone())),
        }
    }
}

impl Prerequisites {
    /// The Prerequisites tab (src/knowledge.rs): for the context's chapters,
    /// the level the investigation's audience starts from, what it should
    /// know (by area, the nearest the level first; a mark on what the
    /// chapters use directly), and what they teach beyond the level. A click
    /// on a topic selects it in the knowledge panel.
    fn prerequisites(&mut self, dock: &mut Dock, ui: &mut Ui) {
        let Some(graph) = crate::knowledge::current() else {
            ui.label(RichText::new(tr("knowledge-none")).color(palette::text_dim()));
            return;
        };
        let chapters = crate::ui::knowledge_chapters(graph, &self.context);
        let p = graph.audience();
        let context = graph.context(&chapters);
        let carried = p.carried();
        let ranks = graph.ranks();
        let heading = if chapters.is_empty() {
            tr("knowledge-for_all").to_string()
        } else {
            tr_with(
                "knowledge-for_chapters",
                args!(
                    chapters = chapters
                        .iter()
                        .map(|c| c.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            )
        };
        // A topic: its title (a click selects it in the graph) and its summary under it.
        let topic = |d: &mut Dock, ui: &mut Ui, i: usize, color: Color32| {
            topic_row(d, ui, (graph, &carried), (i, color))
        };
        // Each group a column of this width, its summaries wrapped in it.
        const COLUMN: f32 = 360.0;
        let level: Vec<usize> = p.level.iter().copied().filter(|&i| context[i]).collect();
        let mut full_screen = false;
        dock.group(
            ui,
            tr("prerequisites-level"),
            Some(&format!("{heading}\n\n{}", tr("prerequisites-level_hint"))),
            |d, ui| {
                ui.set_width(COLUMN);
                let full = ui
                    .button(tr("knowledge-full_screen"))
                    .on_hover_text(tr("prerequisites-full_screen_hint"));
                full_screen = full.clicked();
                ui.add_space(6.0);
                for &i in &level {
                    topic(d, ui, i, palette::accent());
                }
            },
        );
        // What the audience should know, by area, the nearest the level first.
        for area in crate::knowledge::AREAS {
            let mut known: Vec<usize> = (0..graph.nodes.len())
                .filter(|&i| p.is_known(i) && context[i] && graph.nodes[i].area == *area)
                .collect();
            if known.is_empty() {
                continue;
            }
            known.sort_by_key(|&i| std::cmp::Reverse(ranks[i]));
            let title = match *area {
                "gpu" => tr("knowledge-area-gpu"),
                "rt" => tr("knowledge-area-rt"),
                "geo" => tr("knowledge-area-geo"),
                "cg" => tr("knowledge-area-cg"),
                "light" => tr("knowledge-area-light"),
                _ => tr("knowledge-area-gi"),
            };
            let title = tr_with("prerequisites-known", args!(area = title));
            dock.group(ui, &title, Some(tr("prerequisites-known_hint")), |d, ui| {
                ui.set_width(COLUMN);
                for &i in &known {
                    topic(d, ui, i, palette::code_string());
                }
            });
        }
        let teach: Vec<usize> = p.teach.iter().copied().filter(|&i| context[i]).collect();
        if !teach.is_empty() {
            dock.group(
                ui,
                tr("prerequisites-teach"),
                Some(tr("prerequisites-teach_hint")),
                |d, ui| {
                    ui.set_width(COLUMN);
                    for &i in &teach {
                        topic(d, ui, i, palette::code_number());
                    }
                },
            );
        }
        self.overlay |= full_screen;
    }
}

/// A topic's row: its title (a click selects it in the graph; a topic carried from an earlier part is marked)
/// and its summary under it.
fn topic_row(
    d: &mut Dock,
    ui: &mut Ui,
    (graph, carried): (&crate::knowledge::Graph, &[usize]),
    (i, color): (usize, Color32),
) {
    let node = &graph.nodes[i];
    let mark = if carried.contains(&i) {
        " ↗"
    } else {
        ""
    };
    let r = ui.selectable_label(
        false,
        RichText::new(format!("{}{mark}", node.title)).color(color),
    );
    let r = if mark.is_empty() {
        r
    } else {
        r.on_hover_text(tr("prerequisites-carried_hint"))
    };
    if r.clicked() {
        crate::ui::focus_topic(ui.ctx(), i);
        if d.state.panel != PanelMode::Knowledge {
            d.actions.push(ViewerAction::SetPanel(PanelMode::Knowledge));
        }
    }
    ui.add(
        egui::Label::new(
            RichText::new(&node.summary)
                .size(20.0)
                .color(palette::text_dim()),
        )
        .wrap(),
    );
    ui.add_space(6.0);
}
