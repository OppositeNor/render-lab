use super::*;

/// A frame of `ctx` with `events`: a panel in the window (painted on the background) with one place at `window`,
/// and the full-screen one (painted on an overlay that covers the screen and takes the pointer, as the frame
/// graph's does) with one place at `full`, both named alike; whether each place was clicked.
fn frame(ctx: &egui::Context, events: Vec<egui::Event>, window: Rect, full: Rect) -> (bool, bool) {
    let screen = Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0));
    let view = GraphView::default();
    let mut clicked = (false, false);
    let _ = ctx.run_ui(
        egui::RawInput {
            events,
            screen_rect: Some(screen),
            ..Default::default()
        },
        |ui| {
            let ctx = ui.ctx();
            let painter = ctx.layer_painter(egui::LayerId::background());
            let ink = Ink::new(&painter);
            let panel = GraphPanel {
                ctx,
                ink: &ink,
                rect: screen,
                view: &view,
                highlight: &[],
                full: false,
                interactive: true,
                source: None,
            };
            clicked.0 = interact_rects(&panel, "test-places", &[window])[0].clicked();
            egui::Area::new(egui::Id::unique("test-overlay"))
                .order(egui::Order::Foreground)
                .fixed_pos(Pos2::ZERO)
                .show(ctx, |ui| {
                    ui.allocate_rect(screen, egui::Sense::click_and_drag());
                    let ink = Ink::new(ui.painter());
                    let panel = GraphPanel {
                        ink: &ink,
                        full: true,
                        ..panel
                    };
                    clicked.1 = interact_rects(&panel, "test-places", &[full])[0].clicked();
                });
        },
    );
    clicked
}

/// A click at `at`: the pointer there, pressed, and let go, a frame each; what the last frame clicked.
fn click(ctx: &egui::Context, at: Pos2, window: Rect, full: Rect) -> (bool, bool) {
    let button = |pressed: bool| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    frame(ctx, vec![egui::Event::PointerMoved(at)], window, full);
    frame(ctx, vec![button(true)], window, full);
    frame(ctx, vec![button(false)], window, full)
}

#[test]
fn a_click_on_a_panel_is_never_the_full_screen_ones_too() {
    let ctx = egui::Context::default();
    // The window's place, under the overlay that is drawn later in the same frame (Full screen opening it), and
    // the overlay's own (Close) at another place.
    let window = Rect::from_min_size(pos2(40.0, 40.0), vec2(80.0, 30.0));
    let full = Rect::from_min_size(pos2(600.0, 20.0), vec2(80.0, 30.0));
    frame(&ctx, Vec::new(), window, full);
    assert_eq!(click(&ctx, full.center(), window, full), (false, true));
    // A press on the overlay itself brings it to the top; its places stay above it.
    click(&ctx, pos2(400.0, 300.0), window, full);
    assert_eq!(click(&ctx, full.center(), window, full), (false, true));
}
