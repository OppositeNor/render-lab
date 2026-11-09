use super::*;

fn spec(nodes: &[(&str, &[&str])], edges: &[(&str, &str)]) -> Spec {
    Spec {
        nodes: nodes
            .iter()
            .map(|(id, rows)| SpecNode {
                id: id.to_string(),
                title: vec![id.to_string()],
                rows: rows
                    .iter()
                    .map(|r| SpecRow {
                        id: r.to_string(),
                        label: r.to_string(),
                    })
                    .collect(),
                dashed: false,
                image: None,
            })
            .collect(),
        edges: edges
            .iter()
            .map(|(a, b)| SpecEdge {
                from: a.to_string(),
                to: b.to_string(),
                ..Default::default()
            })
            .collect(),
    }
}

#[test]
fn a_spec_names_its_nodes_and_rows() {
    let g = spec(
        &[("CULL", &["tilesCS", "cullCS"]), ("FWD", &[])],
        &[("CULL", "FWD")],
    )
    .graph()
    .unwrap();
    assert_eq!(g.named("cullCS"), Some(0));
    assert_eq!(g.named("FWD"), Some(1));
    assert!(g.check_focus(&["tilesCS".into(), "FWD".into()]).is_ok());
    assert!(g.check_focus(&["nothing".into()]).is_err());
    assert!(spec(&[("A", &[])], &[("A", "B")]).graph().is_err());
    assert!(spec(&[("A", &["A"])], &[]).graph().is_err());
}

#[test]
fn lays_out_along_its_flow() {
    let g = spec(
        &[("A", &[]), ("B", &["b1", "b2"]), ("C", &[])],
        &[("A", "B"), ("B", "C"), ("C", "B")],
    )
    .graph()
    .unwrap();
    let width = |s: &str| s.len() as f32 * 0.6;
    let right = lay_out(&g, &width, &[], 0.5625, &Options::default());
    assert!(
        right.rects[0].center().x < right.rects[1].center().x
            && right.rects[1].center().x < right.rects[2].center().x
    );
    // The edge written last closes the cycle: an arc back.
    assert_eq!(
        right.routes.iter().map(|r| r.back).collect::<Vec<_>>(),
        [false, false, true]
    );
    // A box of rows is taller; a picture makes it taller still.
    assert!(right.rects[1].height() > right.rects[0].height());
    let pictured = lay_out(&g, &width, &[1], 0.5625, &Options::default());
    assert!(pictured.rects[1].height() > right.rects[1].height() && pictured.pictures[1] > 0.0);
    // Top to bottom: the same layers, down the page.
    let down = lay_out(
        &g,
        &width,
        &[],
        0.5625,
        &Options {
            flow: Flow::Down,
            ..Default::default()
        },
    );
    assert!(
        down.rects[0].center().y < down.rects[1].center().y
            && down.rects[1].center().y < down.rects[2].center().y
    );
    assert!(down.bounds.contains_rect(down.rects[1]));
}

#[test]
fn an_image_node_takes_its_pictures_shape() {
    let mut g = spec(&[("A", &[]), ("I", &[])], &[("A", "I")])
        .graph()
        .unwrap();
    g.nodes[1].image = Some(("p.image".into(), 0.5));
    let laid = lay_out(
        &g,
        &|s| s.len() as f32 * 0.6,
        &[],
        0.5625,
        &Options::default(),
    );
    // At least `IMAGE` wide (with its padding), its picture as high as half its width, at most `IMAGE`.
    assert!(laid.rects[1].width() >= IMAGE);
    assert!(laid.pictures[1] > 0.0 && laid.pictures[1] <= IMAGE && laid.pictures[0] == 0.0);
}

#[test]
fn a_drag_moves_a_node_and_a_reset_puts_it_back() {
    let g = spec(&[("A", &[]), ("B", &[])], &[("A", "B")])
        .graph()
        .unwrap();
    let laid = lay_out(
        &g,
        &|s| s.len() as f32 * 0.6,
        &[],
        0.5625,
        &Options::default(),
    );
    let area = Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0));
    let ctx = egui::Context::default();
    let id = egui::Id::unique("test-graph-view");
    let setup = |v: &mut ViewState, _: bool| {
        v.scale = 24.0;
        v.centre = laid.bounds.center().to_vec2();
    };
    let controls = Controls {
        zoom: None,
        move_nodes: true,
    };
    let frame = |events: Vec<egui::Event>| -> ViewState {
        let mut out = None;
        let _ = ctx.run_ui(
            egui::RawInput {
                events,
                screen_rect: Some(area),
                ..Default::default()
            },
            |ui| {
                out = Some(view(ui.ctx(), Some(id), area, &laid, 1, &controls, &setup).0);
            },
        );
        out.expect("a frame")
    };
    let a = Transform::centred(area, laid.bounds.center(), 24.0)
        .rect(laid.rects[0])
        .center();
    let button = |pos: Pos2, pressed: bool| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    frame(vec![egui::Event::PointerMoved(a)]);
    frame(vec![button(a, true)]);
    frame(vec![egui::Event::PointerMoved(a + vec2(30.0, 0.0))]);
    let moved = frame(vec![egui::Event::PointerMoved(a + vec2(60.0, 0.0))]);
    assert!(
        moved.moved.iter().any(|m| m.0 == 0 && m.1.x > 1.0),
        "{:?}",
        moved.moved
    );
    frame(vec![button(a + vec2(60.0, 0.0), false)]);
    ask_reset(&ctx, id);
    let back = frame(Vec::new());
    assert!(back.moved.is_empty());
    assert_eq!(back.centre, laid.bounds.center().to_vec2());
}

#[test]
fn the_wheel_zooms_a_panel_and_a_drag_pans_it() {
    let g = spec(&[("A", &[]), ("B", &[])], &[("A", "B")])
        .graph()
        .unwrap();
    let laid = lay_out(
        &g,
        &|s| s.len() as f32 * 0.6,
        &[],
        0.5625,
        &Options::default(),
    );
    let area = Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0));
    let ctx = egui::Context::default();
    let id = egui::Id::unique("test-graph-zoom");
    let setup = |v: &mut ViewState, _: bool| {
        v.scale = 24.0;
        v.centre = laid.bounds.center().to_vec2();
    };
    let range = zoom_range(&laid, area, 24.0);
    assert!(range.0 <= 24.0 && range.1 == 96.0);
    let controls = Controls {
        zoom: Some(range),
        move_nodes: true,
    };
    let frame = |events: Vec<egui::Event>| -> ViewState {
        let mut out = None;
        let _ = ctx.run_ui(
            egui::RawInput {
                events,
                screen_rect: Some(area),
                ..Default::default()
            },
            |ui| {
                out = Some(view(ui.ctx(), Some(id), area, &laid, 1, &controls, &setup).0);
            },
        );
        out.expect("a frame")
    };
    let at = area.center();
    frame(vec![egui::Event::PointerMoved(at)]);
    let wheel = egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: vec2(0.0, 120.0),
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Unknown,
        modifiers: Default::default(),
    };
    let mut zoomed = frame(vec![wheel]);
    for _ in 0..30 {
        zoomed = frame(Vec::new());
    }
    // The wheel zooms about the pointer and does not pan.
    assert!(
        zoomed.scale > 24.0 && zoomed.scale <= 96.0,
        "{}",
        zoomed.scale
    );
    assert!(
        (zoomed.centre - laid.bounds.center().to_vec2()).length() < 1e-3,
        "{:?}",
        zoomed.centre
    );
    // A drag on the background pans.
    let empty = area.min + vec2(5.0, 5.0);
    let button = |pos: Pos2, pressed: bool| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    frame(vec![egui::Event::PointerMoved(empty)]);
    frame(vec![button(empty, true)]);
    let panned = frame(vec![egui::Event::PointerMoved(empty + vec2(40.0, 0.0))]);
    assert!(
        panned.centre.x < zoomed.centre.x - 0.1,
        "{:?} {:?}",
        panned.centre,
        zoomed.centre
    );
    assert_eq!(panned.scale, zoomed.scale);
}

#[test]
fn clusters_are_boxed_together() {
    let mut g = spec(
        &[("A", &[]), ("B", &[]), ("C", &[])],
        &[("A", "B"), ("A", "C")],
    )
    .graph()
    .unwrap();
    g.clusters = vec![
        Cluster {
            title: Some("one".into()),
            members: vec![0, 1],
            boxed: true,
        },
        Cluster {
            title: None,
            members: vec![2],
            boxed: false,
        },
    ];
    let laid = lay_out(
        &g,
        &|s| s.len() as f32 * 0.6,
        &[],
        0.5625,
        &Options::default(),
    );
    assert_eq!(laid.boxes.len(), 1);
    let (_, b) = laid.boxes[0];
    assert!(
        b.contains_rect(laid.rects[0])
            && b.contains_rect(laid.rects[1])
            && !b.contains_rect(laid.rects[2])
    );
}
