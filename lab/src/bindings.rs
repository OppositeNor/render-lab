//! The viewer's bindings: the keys and pointer gestures it answers, by the context they act in, in one table.
//! The viewer's input (src/viewer/input.rs) matches the keys and buttons named here, and the help card (F1, the
//! toolbar's `?`, src/ui/help_card.rs) lists `BINDINGS`, so the card says what the code does. A gesture another
//! module reads by itself (the frame graph's views, src/graph_view.rs; a window's frame) is listed here as text.

use crate::i18n::tr;
use winit::event::MouseButton;
use winit::keyboard::KeyCode;

/// Play or pause (a timeline, or the demonstration's animation).
pub const PLAY: KeyCode = KeyCode::Space;
/// Step the animation one frame; on a timeline, Left and Right skip.
pub const STEP: KeyCode = KeyCode::ArrowRight;
pub const STEP_ALSO: KeyCode = KeyCode::Period;
pub const SKIP_BACK: KeyCode = KeyCode::ArrowLeft;
pub const SKIP_ON: KeyCode = KeyCode::ArrowRight;
pub const RESET: KeyCode = KeyCode::KeyR;
pub const TIMELINE: KeyCode = KeyCode::KeyT;
pub const FREE_CAMERA: KeyCode = KeyCode::KeyC;
/// Turn the camera to the selected entity.
pub const FOCUS: KeyCode = KeyCode::KeyF;
/// Close what covers the viewer (the help card, a full-screen graph), else back to the homepage.
pub const BACK: KeyCode = KeyCode::Escape;
pub const HELP: KeyCode = KeyCode::F1;
/// The shown scene's demonstrations, in order.
pub const DEMOS: [KeyCode; 12] = [
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
    KeyCode::Digit7,
    KeyCode::Digit8,
    KeyCode::Digit9,
    KeyCode::Digit0,
    KeyCode::Minus,
    KeyCode::Equal,
];
/// The fly camera's keys (the right button held): along the view, sideways, and down and up the world's Y.
pub const FLY_FORWARD: KeyCode = KeyCode::KeyW;
pub const FLY_BACK: KeyCode = KeyCode::KeyS;
pub const FLY_LEFT: KeyCode = KeyCode::KeyA;
pub const FLY_RIGHT: KeyCode = KeyCode::KeyD;
pub const FLY_DOWN: KeyCode = KeyCode::KeyQ;
pub const FLY_UP: KeyCode = KeyCode::KeyE;
pub const FLY_KEYS: [KeyCode; 6] = [FLY_FORWARD, FLY_LEFT, FLY_BACK, FLY_RIGHT, FLY_DOWN, FLY_UP];

/// The buttons a drag in a view moves the camera with: orbit (Shift: pan), pan, and fly.
pub const ORBIT: MouseButton = MouseButton::Left;
pub const PAN: MouseButton = MouseButton::Middle;
pub const FLY: MouseButton = MouseButton::Right;

/// Where a binding acts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Context {
    /// The viewer with a demonstration shown.
    Viewport,
    /// A timeline shown.
    Timeline,
    /// The right button held over a view.
    Flying,
    /// An entity selected in the Scene window.
    Entity,
    /// The frame graph's DAG or resource cards.
    Graph,
    /// An experiment's wipe shown (the Experiments tab).
    Wipe,
    /// The viewer's windows.
    Windows,
}

/// What the user does.
#[derive(Clone, Copy, Debug)]
pub enum Input {
    /// One of these keys.
    Keys(&'static [KeyCode]),
    /// One of these keys with Shift held.
    ShiftKeys(&'static [KeyCode]),
    /// A pointer gesture, as its text names it.
    Gesture(Text),
}

/// A text of render_lab's table, read when shown (a function, so that the table names its key in a `tr` call).
pub type Text = fn() -> &'static str;

/// A binding: where it acts, what the user does, and what it does.
#[derive(Clone, Copy, Debug)]
pub struct Binding {
    pub context: Context,
    pub input: Input,
    pub does: Text,
}

const fn key(context: Context, keys: &'static [KeyCode], does: Text) -> Binding {
    Binding {
        context,
        input: Input::Keys(keys),
        does,
    }
}

const fn gesture(context: Context, gesture: Text, does: Text) -> Binding {
    Binding {
        context,
        input: Input::Gesture(gesture),
        does,
    }
}

/// Every binding, in the help card's order.
pub static BINDINGS: &[Binding] = &[
    key(Context::Viewport, &DEMOS, || tr("help-demos")),
    key(Context::Viewport, &[PLAY], || tr("help-play")),
    key(Context::Viewport, &[STEP, STEP_ALSO], || tr("help-step")),
    key(Context::Viewport, &[RESET], || tr("help-reset")),
    key(Context::Viewport, &[TIMELINE], || tr("help-timeline")),
    key(Context::Viewport, &[FREE_CAMERA], || tr("help-free_camera")),
    key(Context::Viewport, &[HELP], || tr("help-help")),
    key(Context::Viewport, &[BACK], || tr("help-back")),
    gesture(
        Context::Viewport,
        || tr("help-gesture_drag"),
        || tr("help-orbit"),
    ),
    gesture(
        Context::Viewport,
        || tr("help-gesture_shift_drag"),
        || tr("help-pan"),
    ),
    gesture(
        Context::Viewport,
        || tr("help-gesture_middle_drag"),
        || tr("help-pan"),
    ),
    gesture(
        Context::Viewport,
        || tr("help-gesture_wheel"),
        || tr("help-zoom"),
    ),
    gesture(
        Context::Viewport,
        || tr("help-gesture_click"),
        || tr("help-pick"),
    ),
    key(Context::Timeline, &[PLAY], || tr("help-play_timeline")),
    key(Context::Timeline, &[SKIP_BACK, SKIP_ON], || tr("help-skip")),
    key(Context::Timeline, &[STEP_ALSO], || tr("help-step_timeline")),
    gesture(
        Context::Flying,
        || tr("help-gesture_right_hold"),
        || tr("help-look"),
    ),
    key(
        Context::Flying,
        &[FLY_FORWARD, FLY_LEFT, FLY_BACK, FLY_RIGHT],
        || tr("help-fly"),
    ),
    key(Context::Flying, &[FLY_DOWN, FLY_UP], || {
        tr("help-fly_vertical")
    }),
    gesture(
        Context::Flying,
        || tr("help-gesture_shift"),
        || tr("help-fly_faster"),
    ),
    gesture(
        Context::Flying,
        || tr("help-gesture_wheel"),
        || tr("help-fly_speed"),
    ),
    gesture(
        Context::Entity,
        || tr("help-gesture_drag_handle"),
        || tr("help-gizmo"),
    ),
    key(Context::Entity, &[FOCUS], || tr("help-focus")),
    gesture(
        Context::Graph,
        || tr("help-gesture_wheel"),
        || tr("help-graph_zoom"),
    ),
    gesture(
        Context::Graph,
        || tr("help-gesture_drag"),
        || tr("help-graph_drag"),
    ),
    gesture(
        Context::Graph,
        || tr("help-gesture_double_click"),
        || tr("help-graph_fit"),
    ),
    gesture(
        Context::Graph,
        || tr("help-gesture_click_card"),
        || tr("help-graph_inspect"),
    ),
    gesture(
        Context::Wipe,
        || tr("help-gesture_drag_line"),
        || tr("help-wipe_drag"),
    ),
    Binding {
        context: Context::Wipe,
        input: Input::ShiftKeys(&[SKIP_BACK, SKIP_ON]),
        does: || tr("help-wipe_nudge"),
    },
    gesture(
        Context::Windows,
        || tr("help-gesture_drag_title"),
        || tr("help-window_move"),
    ),
    gesture(
        Context::Windows,
        || tr("help-gesture_drag_edge"),
        || tr("help-window_resize"),
    ),
];

/// A key's name on the card: a digit, a sign, a letter, an arrow; Space, Esc, and F1 as their text says.
pub fn key_name(code: KeyCode) -> String {
    let name = format!("{code:?}");
    match code {
        KeyCode::Space => tr("help-key_space").to_string(),
        KeyCode::Escape => tr("help-key_esc").to_string(),
        KeyCode::ArrowLeft => "←".into(),
        KeyCode::ArrowRight => "→".into(),
        KeyCode::Period => ".".into(),
        KeyCode::Minus => "-".into(),
        KeyCode::Equal => "=".into(),
        _ => name
            .strip_prefix("Digit")
            .or_else(|| name.strip_prefix("Key"))
            .unwrap_or(&name)
            .to_string(),
    }
}

/// What the user does, as the card writes it: keys joined by " / ", the demonstrations' as a range.
pub fn input_text(input: Input) -> String {
    let keys = |codes: &[KeyCode]| {
        if codes.len() > 4 {
            let first = key_name(codes[0]);
            let last = key_name(codes[codes.len() - 1]);
            format!("{first} … {last}")
        } else {
            codes
                .iter()
                .map(|c| key_name(*c))
                .collect::<Vec<_>>()
                .join(" / ")
        }
    };
    match input {
        Input::Keys(codes) => keys(codes),
        Input::ShiftKeys(codes) => format!("Shift + {}", keys(codes)),
        Input::Gesture(text) => text().to_string(),
    }
}
