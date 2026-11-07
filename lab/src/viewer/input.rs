//! Input: the keyboard's shortcuts, the pointer over the views (orbit, pan,
//! picks, clicks on images), and the events egui gets.

use super::*;

impl App {
    pub fn key(&mut self, code: KeyCode, event_loop: Option<&ActiveEventLoop>) {
        const DEMO_KEYS: [KeyCode; 12] = [
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
        // What a feature shows full screen (the DAG, the knowledge graph) takes the keys; Esc closes it.
        if self.features.covered(&self.state) {
            if code == KeyCode::Escape {
                self.features.uncover(&self.state);
            }
            return;
        }
        // The keys choose among the shown scene's demonstrations.
        let investigation = investigation::current();
        let demos = investigation.scene_demos(investigation.demo_scene(self.state.demo));
        if let Some(d) = DEMO_KEYS.iter().position(|k| *k == code).and_then(|i| demos.get(i)) {
            self.apply(ViewerAction::SelectDemo(*d));
            return;
        }
        match code {
            KeyCode::Space => self.apply(ViewerAction::TogglePlay),
            KeyCode::ArrowLeft if self.timeline_mode => self.skip(-SKIP_SECONDS),
            KeyCode::ArrowRight if self.timeline_mode => self.skip(SKIP_SECONDS),
            KeyCode::ArrowRight | KeyCode::Period => self.apply(ViewerAction::Step),
            KeyCode::KeyR => self.apply(ViewerAction::Reset),
            KeyCode::KeyT => self.apply(ViewerAction::ToggleTimeline),
            KeyCode::KeyC => self.apply(ViewerAction::ToggleFreeCamera),
            KeyCode::Escape => {
                if let Some(el) = event_loop {
                    self.exit = ViewerExit::Back;
                    el.exit();
                }
            }
            _ => {}
        }
    }

    /// The pixel of the scene under a window pixel: in the main view, or in a
    /// view beside it at the same place of the main one (the views share
    /// one size, and usually the camera), so a drag or a click in any view
    /// moves or picks through the main view's camera.
    /// A view stacked over the main one (src/view.rs): its name and its camera as last drawn.
    pub(super) fn over_camera(&self, view: ViewId) -> Option<(&'static str, crate::camera::OrbitCamera)> {
        self.renderer.as_ref()?.last_overs.iter().find(|(id, _, _)| *id == view).map(|(_, name, camera)| (*name, *camera))
    }

    /// The view on top under `window_px` (src/view.rs), and where it is (canvas points).
    pub(super) fn view_at(&self, window_px: Vec2) -> Option<(ViewId, egui::Rect)> {
        let point = self.to_egui(window_px);
        self.renderer().frame_views().iter().rev().find(|(_, rect)| rect.contains(point)).copied()
    }

    pub fn viewport_height(&self) -> f32 {
        self.renderer().frame_layout().viewport_size_px()[1] as f32
    }

    pub fn pointer_down(&mut self, button: MouseButton, at: Vec2) {
        if self.ui_has_pointer {
            return;
        }
        let Some((view, _)) = self.view_at(at) else { return };
        // A left press on a picture with a click handler picks there at once,
        // and dragging keeps picking under the cursor.
        let point = self.to_egui(at);
        let inset = (button == MouseButton::Left && !self.timeline_playing())
            .then(|| self.last_insets.iter().rev().find(|i| i.frame.contains(point)))
            .flatten()
            .and_then(|i| i.click.clone().filter(|_| i.picture.contains(point)).map(|h| (h, i.picture)));
        if let Some((handler, picture)) = &inset {
            self.run_inset_click(handler, *picture, point);
        }
        self.drag = Some(Drag { button, view, last: at, moved: 0.0, inset });
    }

    /// Run an inset's click handler at `point` (canvas points), clamped to its picture.
    pub(super) fn run_inset_click(&mut self, handler: &str, picture: egui::Rect, point: egui::Pos2) {
        let Some(content) = &self.last_content else { return };
        let shown = self.render_state();
        let Some(renderer) = &self.renderer else { return };
        let uv = ((point - picture.min) / picture.size()).clamp(egui::Vec2::ZERO, egui::vec2(1.0, 1.0));
        let mut frame = crate::actions::ClickFrame { shown: &shown, state: &mut self.state, uv: Vec2::new(uv.x, uv.y), content, statics: &renderer.statics };
        // The scripts' handler of that name, else the investigation's code's.
        if crate::script::runtime::click(handler, &mut frame).is_none() && let Err(e) = (investigation::current().actions().click)(handler, &mut frame) {
            eprintln!("click on an image: {e}");
        }
    }

    pub fn pointer_move(&mut self, at: Vec2) {
        self.cursor = at;
        let Some(mut drag) = self.drag.clone() else { return };
        let delta = at - drag.last;
        drag.moved += delta.length();
        drag.last = at;
        self.drag = Some(drag.clone());
        // A playing timeline holds its camera; the free camera stays the viewer's to move.
        if self.timeline_playing() && drag.view != ViewId::FREE {
            return;
        }
        if let Some((handler, picture)) = &drag.inset {
            let point = self.to_egui(at);
            self.run_inset_click(handler, *picture, point);
            return;
        }
        // A paused diagram takes the pointer itself (src/diagram.rs): the camera keeps its place.
        if self.paused_diagram() {
            return;
        }
        let height = self.viewport_height();
        // On the free camera's view, the pointer moves the free camera; on the main view (in its corner) the main one.
        if drag.view == ViewId::FREE {
            let free = self.free_view();
            let mut orbit = free.orbit();
            match drag.button {
                MouseButton::Left => orbit.orbit(delta),
                MouseButton::Right | MouseButton::Middle => orbit.pan(delta, height),
                _ => {}
            }
            free.place(&orbit);
            return;
        }
        self.end_intro();
        // A view stacked over the main one moves its own camera (src/view.rs); the main camera keeps its place.
        if let Some((name, mut camera)) = self.over_camera(drag.view) {
            match drag.button {
                MouseButton::Left => camera.orbit(delta),
                MouseButton::Right | MouseButton::Middle => camera.pan(delta, height),
                _ => {}
            }
            self.state.set_own_camera(name, &camera);
            return;
        }
        match drag.button {
            MouseButton::Left => self.state.camera.orbit(delta),
            MouseButton::Right | MouseButton::Middle => self.state.camera.pan(delta, height),
            _ => {}
        }
    }

    pub fn pointer_up(&mut self, button: MouseButton, at: Vec2) {
        let Some(drag) = self.drag.take() else { return };
        // A press on a picture already picked, and its drag kept picking.
        if drag.inset.is_some() {
            return;
        }
        if drag.button == button && button == MouseButton::Left && drag.moved < 4.0 {
            // A paused diagram takes its clicks itself (src/diagram.rs).
            if self.paused_diagram() {
                return;
            }
            // A click focuses the view it is on (when there are several): the dock acts on it.
            self.focus = drag.view;
            self.click(at, drag.view);
        }
    }

    pub(super) fn to_egui(&self, p: Vec2) -> egui::Pos2 {
        let scale = self.renderer().layout().scale;
        egui::pos2(p.x / scale, p.y / scale)
    }

    /// A cursor move in window pixels, for egui and the 3D view.
    pub fn cursor_moved(&mut self, p: Vec2) {
        self.egui_events.push(egui::Event::PointerMoved(self.to_egui(p)));
        self.pointer_move(p);
    }

    pub fn mouse_button(&mut self, button: MouseButton, pressed: bool) {
        if let Some(b) = crate::window::pointer_button(button) {
            self.egui_events.push(egui::Event::PointerButton { pos: self.to_egui(self.cursor), button: b, pressed, modifiers: self.modifiers });
        }
        if pressed {
            self.pointer_down(button, self.cursor);
        } else {
            self.pointer_up(button, self.cursor);
        }
    }

    /// Keys for a number field being typed into (a slider's value).
    pub(super) fn egui_key(&mut self, event: &winit::event::KeyEvent) {
        let pressed = event.state == ElementState::Pressed;
        let key = match event.physical_key {
            PhysicalKey::Code(KeyCode::Enter | KeyCode::NumpadEnter) => Some(egui::Key::Enter),
            PhysicalKey::Code(KeyCode::Escape) => Some(egui::Key::Escape),
            PhysicalKey::Code(KeyCode::Backspace) => Some(egui::Key::Backspace),
            PhysicalKey::Code(KeyCode::Delete) => Some(egui::Key::Delete),
            PhysicalKey::Code(KeyCode::ArrowLeft) => Some(egui::Key::ArrowLeft),
            PhysicalKey::Code(KeyCode::ArrowRight) => Some(egui::Key::ArrowRight),
            PhysicalKey::Code(KeyCode::Home) => Some(egui::Key::Home),
            PhysicalKey::Code(KeyCode::End) => Some(egui::Key::End),
            PhysicalKey::Code(KeyCode::Tab) => Some(egui::Key::Tab),
            PhysicalKey::Code(KeyCode::KeyA) if self.modifiers.command => Some(egui::Key::A),
            _ => None,
        };
        if let Some(key) = key {
            self.egui_events.push(egui::Event::Key { key, physical_key: None, pressed, repeat: event.repeat, modifiers: self.modifiers });
        }
        if let (true, Some(text)) = (pressed, &event.text) && !text.chars().any(char::is_control) {
            self.egui_events.push(egui::Event::Text(text.to_string()));
        }
    }

    pub fn wheel(&mut self, steps: f32) {
        // A paused diagram zooms itself (src/diagram.rs).
        if self.paused_diagram() {
            return;
        }
        let under = self.view_at(self.cursor).map(|(v, _)| v);
        if under == Some(ViewId::FREE) && !self.ui_has_pointer {
            let free = self.free_view();
            let mut orbit = free.orbit();
            orbit.zoom(steps);
            free.place(&orbit);
        } else if !self.timeline_playing() && !self.ui_has_pointer {
            self.end_intro();
            match under.and_then(|v| self.over_camera(v)) {
                Some((name, mut camera)) => {
                    camera.zoom(steps);
                    self.state.set_own_camera(name, &camera);
                }
                None => self.state.camera.zoom(steps),
            }
        }
    }

    pub(super) fn click(&mut self, at: Vec2, view: ViewId) {
        let Some((_, rect)) = self.view_at(at).filter(|(v, _)| *v == view) else { return };
        // The view's content, by field (so `self.state` stays free to change).
        let content = match view {
            ViewId::FREE => self.last_content.as_ref(),
            _ => self.last_main_content.as_ref().or(self.last_content.as_ref()),
        };
        let Some(content) = content else { return };
        // The pixel of the view's own frame (its picture may be shown smaller, as the main view in a corner).
        let point = self.to_egui(at);
        let uv = (point - rect.min) / rect.size();
        let size = content.uniforms.camera.viewport_size;
        let pixel = Vec2::new(uv.x * size[0], uv.y * size[1]);
        let shown = self.render_state();
        let Some(renderer) = &self.renderer else { return };
        // An inset covers the scene behind it; a click on the picture of one
        // with a click handler (`PictureInset::click`) runs the handler.
        if self.last_insets.iter().rev().any(|i| i.frame.contains(point)) {
            return;
        }
        // The scripts' `scene` click handler, else the investigation's code's.
        let mut frame = crate::actions::ClickFrame { shown: &shown, state: &mut self.state, uv: Vec2::new(uv.x, uv.y), content, statics: &renderer.statics };
        if crate::script::runtime::click("scene", &mut frame).is_some() {
            return;
        }
        let projector = Projector::new(&content.uniforms.camera);
        investigation::current().click(&shown, &mut self.state, content, &renderer.statics, &projector, pixel);
    }
}
