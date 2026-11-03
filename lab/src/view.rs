//! Views: each picture of the frame on screen is a view of its own, with its
//! own GPU slot in the renderer (its targets, its chain, its program's images
//! and history, how long it has held still), so no view reads or overwrites
//! another's. The main view; the views of a split scene area
//! (`Investigation::panes`); the experiment's reference; the free camera's.

/// A view, by what it is in the frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ViewId(pub u32);

impl ViewId {
    /// The main view: the one the dock, the insets, and the panel belong to.
    pub const MAIN: ViewId = ViewId(0);
    /// The experiment's reference (src/experiment.rs).
    pub const REFERENCE: ViewId = ViewId(1);
    /// The free camera's view of the main frame (src/free_view.rs).
    pub const FREE: ViewId = ViewId(2);

    /// The `n`th view beside the main one in a split scene area.
    pub const fn pane(n: u32) -> ViewId {
        ViewId(16 + n)
    }

    /// A view an investigation stacks over the main one (`Investigation::views`).
    pub const fn over(n: u32) -> ViewId {
        ViewId(64 + n)
    }
}

std::thread_local! {
    static CURRENT: std::cell::Cell<ViewId> = const { std::cell::Cell::new(ViewId::MAIN) };
}

/// The view being built and drawn now (the renderer sets it as it enters a
/// view): what a view's data is scoped by (`lab::fetch`, an investigation's
/// caches), so no view reads another's unless it names it.
pub fn current() -> ViewId {
    CURRENT.with(|c| c.get())
}

/// Sets the view being built and drawn (`Renderer::enter`).
pub(crate) fn set_current(view: ViewId) {
    CURRENT.with(|c| c.set(view));
}

std::thread_local! {
    static NAME: std::cell::Cell<Option<&'static str>> = const { std::cell::Cell::new(None) };
}

/// The name of the view stacked over the main one being built now
/// (`Investigation::views`), or None for the main view and its kin.
pub fn name() -> Option<&'static str> {
    NAME.with(|n| n.get())
}

/// Sets the name of the view being built (the renderer, around an over view's build).
pub(crate) fn set_name(name: Option<&'static str>) {
    NAME.with(|n| n.set(name));
}
