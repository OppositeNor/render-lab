//! The viewer's features: a tab of the dock with what it shows (its dock,
//! its panel, its state, the views it adds, the story panels it draws, its
//! homepage button, its snapshot pages), declared in one place.
//!
//! A feature is a `Feature` (what it is) and the `FeatureState` it makes for
//! each viewer and each recording (what it holds while they run). The viewer
//! asks the registry (`all`) for the features of its investigation, shows a
//! tab for each that has one, draws the current one's dock and panel, has
//! each draw the story panels it owns (`Feature::panels`, in a recording too),
//! and hands each its state at fixed points of a frame, with a `Host` (the
//! viewer, as a feature may change it). Nothing in the viewer names a feature:
//! a new one is a module of its own and an entry in `BUILT_IN`.

pub mod captions;
pub mod code;
pub mod experiments;
pub mod graph;
pub mod parameters;
pub mod performance;
pub mod prerequisites;
pub mod quiz;
pub mod sources;

use crate::controls::Dock;
use crate::investigation::Investigation;
use crate::renderer::{CompareRequest, Renderer};
use crate::shot::{DemoId, PanelMode, ShotState};
use crate::ui::{DockTab, Ink, UiInput, UiReport, ViewerAction};
use crate::view::ViewId;
use anyhow::Result;

/// A feature of the viewer: a tab of its dock (src/controls.rs), or panels only, and what it shows.
pub trait Feature: Sync {
    /// Its id, unique among the features: its tab's (`DockTab`).
    fn id(&self) -> &'static str;
    /// Its tab's label, in the viewer's language; none: it has no tab (only its panels and its pages).
    fn label(&self) -> Option<String>;
    /// Where its tab stands among the dock's: lower first.
    fn order(&self) -> i32;
    /// Its tab is a timeline's: listed while the timeline is shown, and shown when the timeline pauses;
    /// leaving the timeline leaves it for the first tab.
    fn timeline(&self) -> bool {
        false
    }
    /// The story panels it draws (`FeatureState::story_panel`) whenever the state shows one, its tab shown
    /// or not: in the viewer and in a recording.
    fn panels(&self) -> &'static [PanelMode] {
        &[]
    }
    /// Whether `investigation` has it (its tab, its homepage button).
    fn available(&self, _investigation: &dyn Investigation) -> bool {
        true
    }
    /// How its tab shares the screen.
    fn layout(&self) -> FeatureLayout {
        FeatureLayout::default()
    }
    /// Its homepage button's label, in the homepage's language: the button opens the viewer on its tab.
    fn home(&self) -> Option<String> {
        None
    }
    /// The snapshot pages that open the viewer on its tab (`home --snapshot … --page <name>[:<args>]`).
    fn pages(&self) -> &'static [Page] {
        &[]
    }
    /// The demonstration (its key) a snapshot page opens on, before the viewer loads its scene; none: the first.
    fn snapshot_demo<'a>(&self, _page: &str, _args: Option<&'a str>) -> Option<&'a str> {
        None
    }
    /// Its tab was shown, or another was: put its panel up, or give the panel back when it is its own.
    fn show(&self, _tab: &mut ShowTab) {}
    /// Its state for a new viewer.
    fn state(&self) -> Box<dyn FeatureState>;
}

/// How a feature's tab shares the screen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FeatureLayout {
    /// The tab draws its own panel right of the viewport (`FeatureState::panel`), in the value panel's place.
    pub panel: bool,
    /// Its panel gives way to the panels that show the frame (performance, knowledge, the frame graph).
    pub yields: bool,
    /// The tab draws beside the dock's buttons itself (`FeatureState::side`), not in the dock's scrolled groups.
    pub side: bool,
}

/// A snapshot page of a feature (`Feature::pages`).
#[derive(Clone, Copy, Debug)]
pub struct Page {
    /// What `--page` names it by; arguments follow a `:` (or an `@`).
    pub name: &'static str,
    /// Its line of `--page`'s help: its arguments, and what it shows.
    pub help: &'static str,
}

/// What a feature holds while a viewer (or a recording) runs; `Send`, as a renderer is made on a worker
/// thread with the states its recordings draw with. Every hook has a default that does nothing.
pub trait FeatureState: Send {
    /// Its tab was selected (or the viewer opened on it, once the scene can set up a view).
    fn selected(&mut self, _host: &mut dyn Host) {}
    /// Once a frame, before the frame's state is read, and after the UI's actions: what the UI asked of the
    /// feature, done with the viewer.
    fn update(&mut self, _host: &mut dyn Host) {}
    /// Its tab's dock, right of the buttons; false: the investigation's (`Investigation::dock`).
    fn dock(&mut self, _dock: &mut Dock, _ui: &mut egui::Ui) -> bool {
        false
    }
    /// Its tab's dock beside the buttons (with `FeatureLayout::side`), in `rect`.
    fn side(&mut self, _ink: &Ink, _rect: egui::Rect, _input: &UiInput) {}
    /// Its panel (with `FeatureLayout::panel`), while its tab shows outside a playing timeline.
    fn panel(&mut self, _ui: &mut PanelUi) {}
    /// One of its story panels (`Feature::panels`), the one the state shows, in the panel's place.
    fn story_panel(&mut self, _panel: &mut StoryPanel) {}
    /// In the viewer, over everything but the popups: what it shows full screen (`covers`).
    fn overlay(&mut self, _panel: &mut StoryPanel) {}
    /// Something of it covers the viewer, `state` shown: it takes the pointer and the keys.
    fn covers(&self, _state: &ShotState) -> bool {
        false
    }
    /// Esc, while it covers the viewer: close what covers it.
    fn uncover(&mut self) {}
    /// What it asks of the frame about to render, `state` as it renders.
    fn frame_request(&self, _state: &ShotState, _frame: &FrameInfo) -> FrameAsk {
        FrameAsk::default()
    }
    /// What it reads of the renderer once the frame's requests are made (its timings).
    fn frame_readout(&mut self, _renderer: &Renderer, _frame: &FrameInfo) {}
    /// The state of a view it adds (src/view.rs), from the live one; none: not its view, or not shown.
    fn view_state(&self, _view: ViewId, _live: &ShotState) -> Option<ShotState> {
        None
    }
    /// A dock slider moved while its view `view` is focused: set the view's value. False: not its view.
    fn set_value(&mut self, _view: ViewId, _key: &'static str, _value: f32) -> bool {
        false
    }
    /// The viewer opened on its tab changed the demonstration: `host.state()` is the new one's defaults.
    fn demo_selected(&mut self, _change: &DemoChange, _host: &mut dyn Host) {}
    /// One step of a snapshot page (`Feature::pages`): `page` its name, `args` what follows its `:`, `step`
    /// the step's number (0 first). The viewer does what the step returns, then asks for the next.
    fn snapshot(
        &mut self,
        _page: &str,
        _args: Option<&str>,
        _step: usize,
        _host: &mut dyn Host,
    ) -> Result<Snapshot> {
        Ok(Snapshot::Done(String::new()))
    }
}

/// What a snapshot page's step asks of the viewer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Snapshot {
    /// Render one frame.
    Render,
    /// Render one frame, half a second after the one before (at 60 fps), so what eases in settles.
    Settle,
    /// The page is drawn: what it shows, for its report line (empty: nothing to add).
    Done(String),
}

/// A frame about to render, as the features see it.
#[derive(Clone, Copy, Debug)]
pub struct FrameInfo {
    /// The tab shown, and the tab the viewer opened on.
    pub tab: DockTab,
    pub opened: Option<DockTab>,
    pub timeline_mode: bool,
    /// The viewer's frame time: mean and 95th percentile in milliseconds.
    pub frame_ms: (f64, f64),
}

/// What a feature asks of a frame.
#[derive(Default)]
pub struct FrameAsk {
    /// A reference view compared with the live one.
    pub compare: Option<CompareRequest>,
    /// GPU time per pass measured (src/timing.rs).
    pub timing: bool,
}

/// A tab shown (`Feature::show`).
pub struct ShowTab<'a> {
    /// The live state, and its demonstration's defaults.
    pub state: &'a mut ShotState,
    pub defaults: &'a ShotState,
    /// The feature's tab is the one shown.
    pub shown: bool,
    /// Actions for the viewer, applied at once.
    pub actions: &'a mut Vec<ViewerAction>,
}

/// A change of demonstration in a viewer opened on a feature's tab.
pub struct DemoChange<'a> {
    /// The state before, and its demonstration's defaults.
    pub old: &'a ShotState,
    pub old_defaults: &'a ShotState,
    /// The new demonstration's defaults.
    pub defaults: &'a ShotState,
}

/// What a feature's panel draws with: the dock's kit (`Dock`: its rows, its actions, its widgets).
pub struct PanelUi<'a, 'd> {
    pub ctx: &'a egui::Context,
    pub ink: &'a Ink<'a>,
    pub input: &'a UiInput<'a>,
    pub dock: &'a mut Dock<'d>,
}

/// What a story panel (`FeatureState::story_panel`, `overlay`) draws with, and what it gives back.
pub struct StoryPanel<'a> {
    pub ctx: &'a egui::Context,
    pub ink: &'a Ink<'a>,
    pub input: &'a UiInput<'a>,
    /// In the viewer, where its clicks act; a recording only shows it.
    pub interactive: bool,
    /// The last live reload of the shaders failed: what it says.
    pub shader_error: Option<&'a str>,
    /// What the recorder checks of the frame (the code's lines).
    pub report: &'a mut UiReport,
    /// Actions for the viewer.
    pub actions: Vec<ViewerAction>,
    /// The pointer is over it: the wheel and drags are its, not the camera's.
    pub pointer: bool,
}

/// The viewer, as a feature sees it.
pub trait Host {
    /// The interactive state (the live view's).
    fn state(&self) -> &ShotState;
    fn state_mut(&mut self) -> &mut ShotState;
    /// Not while a timeline plays (it owns the state).
    fn interactive(&self) -> bool;
    /// The scene can set up a view: the renderer is there, and no timeline is shown.
    fn ready(&self) -> bool;
    /// `demo` at its defaults (its own values and camera).
    fn default_state(&mut self, demo: DemoId) -> ShotState;
    /// Make `state` the live one: the animation stopped and the camera's intro cut.
    fn set_state(&mut self, state: ShotState);
    /// Show `state`: `set_state`, the clock at the start, and the tab's panel kept.
    fn show(&mut self, state: ShotState);
    /// Set a value of the live state, as its dock slider does.
    fn set_value(&mut self, key: &'static str, value: f32);
    /// The renderer, once it is there.
    fn renderer(&mut self) -> Option<&mut Renderer>;
    /// The tab shown.
    fn tab(&self) -> DockTab;
    /// The chapter at the timeline's frame, while the timeline is shown.
    fn timeline_chapter(&self) -> Option<u32>;
    /// While the timeline is shown: the caption at or last before its frame, and the one before that.
    fn captions(&self) -> (Option<String>, Option<String>);
    /// An action for the viewer, applied once the features are done.
    fn act(&mut self, action: ViewerAction);
}

/// The lab's own features, in no order (`Feature::order` orders their tabs).
static BUILT_IN: &[&dyn Feature] = &[
    &parameters::ParametersFeature,
    &experiments::ExperimentsFeature,
    &code::CodeFeature,
    &sources::SourcesFeature,
    &prerequisites::PrerequisitesFeature,
    &graph::GraphFeature,
    &quiz::QuizFeature,
    &captions::CaptionsFeature,
    &performance::PerformanceFeature,
];

/// The lines `--page`'s help has for the lab's features' pages (the packages' are known once they load).
pub fn page_help() -> String {
    BUILT_IN
        .iter()
        .flat_map(|f| f.pages())
        .map(|p| format!("{}{}", p.name, p.help))
        .collect::<Vec<_>>()
        .join(", or\n")
}

/// Every feature, in the order of their tabs: the lab's, then the packages' (their libraries' `features`
/// handle, src/plugin.rs).
pub fn all() -> Vec<&'static dyn Feature> {
    static ALL: std::sync::OnceLock<Vec<&'static dyn Feature>> = std::sync::OnceLock::new();
    ALL.get_or_init(|| {
        let (all, refused) = merge(BUILT_IN, &crate::investigations::features());
        for problem in refused {
            eprintln!("{problem}");
        }
        all
    })
    .clone()
}

/// The lab's features and the packages' (each with its package), in the order of their tabs; and why any
/// was left out: an id another feature has.
fn merge(
    built_in: &[&'static dyn Feature],
    packages: &[(&str, &'static dyn Feature)],
) -> (Vec<&'static dyn Feature>, Vec<String>) {
    let mut all: Vec<&'static dyn Feature> = built_in.to_vec();
    let mut refused = Vec::new();
    for (package, feature) in packages {
        let id = feature.id();
        if all.iter().any(|f| f.id() == id) {
            refused.push(format!(
                "package {package}: its feature `{id}` is left out: another feature has that id"
            ));
        } else {
            all.push(*feature);
        }
    }
    // A stable sort: features of one order keep the lab's, then the packages', order.
    all.sort_by_key(|f| f.order());
    (all, refused)
}

/// The feature whose tab is `id`.
pub fn find(id: &str) -> Option<&'static dyn Feature> {
    all().into_iter().find(|f| f.id() == id)
}

/// The feature a snapshot page belongs to, the page's name, and its arguments: what follows its `:`, or
/// everything after its name from an `@` (`viewer-dag@<node>`).
pub fn page(page: &str) -> Option<(&'static dyn Feature, &'static str, Option<&str>)> {
    all().into_iter().find_map(|f| {
        f.pages()
            .iter()
            .find_map(|p| match page.strip_prefix(p.name) {
                Some("") => Some((f, p.name, None)),
                Some(rest) if rest.starts_with('@') => Some((f, p.name, Some(rest))),
                Some(rest) => rest.strip_prefix(':').map(|args| (f, p.name, Some(args))),
                None => None,
            })
    })
}

/// One feature in a viewer or a recording: whether its investigation has it, and its state.
struct Held {
    feature: &'static dyn Feature,
    available: bool,
    state: Box<dyn FeatureState>,
}

/// A viewer's (or a recording's) features: each one's state. Every feature has one, so a story panel is drawn
/// by its feature in any investigation; only those the investigation has (`Feature::available`) show a tab.
#[derive(Default)]
pub struct Features {
    held: Vec<Held>,
}

impl Features {
    /// The features, each with a new state, for `investigation`.
    pub fn new(investigation: &dyn Investigation) -> Features {
        Features {
            held: all()
                .into_iter()
                .map(|feature| Held {
                    feature,
                    available: feature.available(investigation),
                    state: feature.state(),
                })
                .collect(),
        }
    }

    /// The features the investigation has, in the order of their tabs.
    pub fn features(&self) -> impl Iterator<Item = &'static dyn Feature> + '_ {
        self.held.iter().filter(|h| h.available).map(|h| h.feature)
    }

    /// The feature whose id is `id`, if the investigation has it.
    pub fn feature(&self, id: &str) -> Option<&'static dyn Feature> {
        self.features().find(|f| f.id() == id)
    }

    /// The dock's tabs, in their order: the features' that have one, a timeline's only while it is shown.
    pub fn tabs(&self, timeline: bool) -> Vec<(DockTab, String)> {
        self.features()
            .filter(|f| timeline || !f.timeline())
            .filter_map(|f| Some((DockTab(f.id()), f.label()?)))
            .collect()
    }

    /// The state of the feature whose id is `id`.
    pub fn state_mut(&mut self, id: &str) -> Option<&mut (dyn FeatureState + 'static)> {
        self.held
            .iter_mut()
            .find(|h| h.feature.id() == id)
            .map(|h| h.state.as_mut())
    }

    /// The feature that draws the story panel `panel` (`Feature::panels`).
    pub fn panel_feature(&self, panel: PanelMode) -> Option<&'static dyn Feature> {
        self.held
            .iter()
            .map(|h| h.feature)
            .find(|f| f.panels().contains(&panel))
    }

    /// The state of the feature that draws the story panel `panel` (`Feature::panels`).
    pub fn panel_mut(&mut self, panel: PanelMode) -> Option<&mut (dyn FeatureState + 'static)> {
        self.held
            .iter_mut()
            .find(|h| h.feature.panels().contains(&panel))
            .map(|h| h.state.as_mut())
    }

    pub fn states(&self) -> impl Iterator<Item = &(dyn FeatureState + 'static)> + '_ {
        self.held.iter().map(|h| h.state.as_ref())
    }

    pub fn states_mut(&mut self) -> impl Iterator<Item = &mut (dyn FeatureState + 'static)> + '_ {
        self.held.iter_mut().map(|h| h.state.as_mut())
    }

    /// Something of a feature covers the viewer (`FeatureState::covers`).
    pub fn covered(&self, state: &ShotState) -> bool {
        self.states().any(|s| s.covers(state))
    }

    /// Esc: close what covers the viewer.
    pub fn uncover(&mut self, state: &ShotState) {
        for s in self.states_mut().filter(|s| s.covers(state)) {
            s.uncover();
        }
    }

    /// The state of view `view`, from the feature that adds it.
    pub fn view_state(&self, view: ViewId, live: &ShotState) -> Option<ShotState> {
        self.states().find_map(|s| s.view_state(view, live))
    }

    /// What the features ask of the frame about to render: the first comparison asked for, and timing if any asks.
    pub fn frame_request(&self, state: &ShotState, frame: &FrameInfo) -> FrameAsk {
        self.states()
            .map(|s| s.frame_request(state, frame))
            .fold(FrameAsk::default(), |a, b| FrameAsk {
                compare: a.compare.or(b.compare),
                timing: a.timing || b.timing,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Tab(&'static str, i32);

    impl Feature for Tab {
        fn id(&self) -> &'static str {
            self.0
        }
        fn label(&self) -> Option<String> {
            Some(self.0.to_string())
        }
        fn order(&self) -> i32 {
            self.1
        }
        fn state(&self) -> Box<dyn FeatureState> {
            struct Nothing;
            impl FeatureState for Nothing {}
            Box::new(Nothing)
        }
    }

    static A: Tab = Tab("a", 30);
    static B: Tab = Tab("b", 10);
    static TWIN: Tab = Tab("a", 5);
    static PARAMETERS: Tab = Tab("parameters", 1);
    static C: Tab = Tab("c", 30);

    #[test]
    fn packages_features_join_the_lab_s_in_tab_order() {
        let (all, refused) = merge(
            &[&A, &B, &PARAMETERS],
            &[("p", &C), ("p", &TWIN), ("q", &PARAMETERS)],
        );
        assert_eq!(
            all.iter().map(|f| f.id()).collect::<Vec<_>>(),
            ["parameters", "b", "a", "c"]
        );
        assert_eq!(refused.len(), 2);
        assert!(refused[0].contains("package p") && refused[0].contains("`a`"));
        assert!(refused[1].contains("package q") && refused[1].contains("`parameters`"));
    }

    #[test]
    fn a_page_s_arguments() {
        let (f, name, args) = page("viewer-quiz:2@B").unwrap();
        assert_eq!((f.id(), name, args), ("quiz", "viewer-quiz", Some("2@B")));
        let (f, name, args) = page("viewer-dag-full@ssrTrace").unwrap();
        assert_eq!(
            (f.id(), name, args),
            ("graph", "viewer-dag-full", Some("@ssrTrace"))
        );
        assert_eq!(
            page("viewer-knowledge-graph").map(|(f, n, a)| (f.id(), n, a)),
            Some(("prerequisites", "viewer-knowledge-graph", None))
        );
        assert!(page("viewer-quizzes").is_none());
        assert!(page("viewer-layout").is_none());
        let (f, name, args) = page("viewer-performance:room").unwrap();
        assert_eq!(
            (f.id(), name, args),
            ("performance", "viewer-performance", Some("room"))
        );
    }
}
