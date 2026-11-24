//! Real receiver, mod dispatcher and window input regressions for debt 59.
#[allow(unused_imports)]
use super::*;
#[allow(unused_imports)]
use crate::game::WindowKeyInput;
#[allow(unused_imports)]
use mod_api::MouseScrollingEvent;
#[allow(unused_imports)]
use sunrise_client::{items::ItemStack, keybinds::VanillaMappingSlot, preedit, widgets};
#[allow(unused_imports)]
use winit::{dpi::PhysicalPosition, event::Ime, keyboard::PhysicalKey, window::WindowId};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(in crate::game) enum Cancel {
    #[default]
    None,
    Press,
    ReleasePre,
    ReleasePost,
    Drag,
    ScreenWheel,
    GameWheel,
}

#[derive(Debug, PartialEq)]
pub(in crate::game) enum Seen {
    Activation(&'static str),
    PressPre,
    PressPost(String, bool),
    ReleasePost(String, bool),
    PreeditPost(String, String),
    KeyPre(String),
    KeyPost(String),
    DragPre(i32, [f32; 2], [f32; 2]),
    DragPost,
    ScreenWheelPre([f64; 2]),
    ScreenWheelChild([f64; 2]),
    ScreenWheelPost([f64; 2]),
    GameWheel([f64; 2], [i32; 2], [bool; 3], [f32; 2], u32),
}

#[derive(Default)]
pub(in crate::game) struct State {
    pub(in crate::game) cancel: Cancel,
    pub(in crate::game) seen: Vec<Seen>,
}

pub(in crate::game) struct Declining(pub(in crate::game) Arc<Mutex<State>>);
impl ScreenWidget for Declining {
    fn bounds(&self) -> [f32; 4] {
        [10.0, 20.0, 90.0, 20.0]
    }

    fn mouse_clicked(
        &mut self,
        _: &mut dyn ClientHost,
        _: [f32; 2],
        _: i32,
        _: u32,
        _: bool,
    ) -> bool {
        self.0
            .lock()
            .unwrap()
            .seen
            .push(Seen::Activation("decline"));
        false
    }

    fn mouse_scrolled(
        &mut self,
        _: &mut dyn ClientHost,
        _: [f32; 2],
        scroll: [f64; 2],
        _: u32,
    ) -> bool {
        self.0
            .lock()
            .unwrap()
            .seen
            .push(Seen::ScreenWheelChild(scroll));
        false
    }
}

pub(in crate::game) struct Observe(pub(in crate::game) Arc<Mutex<State>>);

#[mod_api::__subscriptions]
impl Hooks for Observe {
    fn screen_init_post(&self, _: &dyn ClientHost, event: &mut ScreenInitPostEvent<'_>) {
        let button = |rect, name| {
            let state = self.0.clone();
            Button::new(rect, ApiText::literal(name), move |_, _| {
                state.lock().unwrap().seen.push(Seen::Activation(name));
            })
        };
        match event.screen.id {
            "paused" => {
                event
                    .widgets
                    .add_button(button(event.screen.rect, "over_resume"));
            }
            "mod" => {
                event
                    .widgets
                    .add_widget(Box::new(Declining(self.0.clone())));
                event
                    .widgets
                    .add_button(button([10.0, 20.0, 90.0, 20.0], "under_decline"));
                let focus = event
                    .widgets
                    .add_button(button([10.0, 90.0, 90.0, 20.0], "initial"));
                assert!(event.widgets.set_initial_focus(focus));
            }
            "dialog" => {
                event
                    .widgets
                    .add_button(button([20.0, 40.0, 90.0, 20.0], "after_dialog"));
            }
            _ => {}
        }
    }

    fn screen_mouse_button_pressed_pre(
        &self,
        _: &dyn ClientHost,
        _: &mut ScreenMouseButtonPressedPreEvent<'_>,
    ) -> Flow {
        let mut state = self.0.lock().unwrap();
        state.seen.push(Seen::PressPre);
        flow(state.cancel == Cancel::Press)
    }

    fn screen_mouse_button_pressed_post(
        &self,
        _: &dyn ClientHost,
        event: &mut ScreenMouseButtonPressedPostEvent<'_>,
    ) {
        self.0.lock().unwrap().seen.push(Seen::PressPost(
            event.screen.id.to_owned(),
            event.was_click_handled,
        ));
    }

    fn screen_mouse_button_released_post(
        &self,
        _: &dyn ClientHost,
        event: &mut ScreenMouseButtonReleasedPostEvent<'_>,
    ) {
        self.0.lock().unwrap().seen.push(Seen::ReleasePost(
            event.screen.id.to_owned(),
            event.was_release_handled,
        ));
    }

    fn screen_preedit_post(&self, _: &dyn ClientHost, event: &mut ScreenPreeditPostEvent<'_>) {
        self.0.lock().unwrap().seen.push(Seen::PreeditPost(
            event.screen.id.to_owned(),
            event.full_text.to_owned(),
        ));
    }

    fn screen_key_released_pre(
        &self,
        _: &dyn ClientHost,
        event: &mut ScreenKeyReleasedPreEvent<'_>,
    ) -> Flow {
        let mut state = self.0.lock().unwrap();
        state.seen.push(Seen::KeyPre(event.key.to_owned()));
        flow(state.cancel == Cancel::ReleasePre)
    }

    fn screen_key_released_post(
        &self,
        _: &dyn ClientHost,
        event: &mut ScreenKeyReleasedPostEvent<'_>,
    ) -> Flow {
        let mut state = self.0.lock().unwrap();
        state.seen.push(Seen::KeyPost(event.key.to_owned()));
        flow(state.cancel == Cancel::ReleasePost)
    }

    fn screen_mouse_dragged_pre(
        &self,
        _: &dyn ClientHost,
        event: &mut ScreenMouseDraggedPreEvent<'_>,
    ) -> Flow {
        let mut state = self.0.lock().unwrap();
        state
            .seen
            .push(Seen::DragPre(event.button, event.delta, event.screen.mouse));
        flow(state.cancel == Cancel::Drag)
    }

    fn screen_mouse_dragged_post(
        &self,
        _: &dyn ClientHost,
        _: &mut ScreenMouseDraggedPostEvent<'_>,
    ) {
        self.0.lock().unwrap().seen.push(Seen::DragPost);
    }

    fn screen_mouse_scrolled_pre(
        &self,
        _: &dyn ClientHost,
        event: &mut ScreenMouseScrolledPreEvent<'_>,
    ) -> Flow {
        let mut state = self.0.lock().unwrap();
        state.seen.push(Seen::ScreenWheelPre(event.scroll));
        flow(state.cancel == Cancel::ScreenWheel)
    }

    fn screen_mouse_scrolled_post(
        &self,
        _: &dyn ClientHost,
        event: &mut ScreenMouseScrolledPostEvent<'_>,
    ) {
        self.0
            .lock()
            .unwrap()
            .seen
            .push(Seen::ScreenWheelPost(event.scroll));
    }

    fn mouse_scrolling(&self, _: &mut dyn ClientHost, event: &mut MouseScrollingEvent) -> Flow {
        let mut state = self.0.lock().unwrap();
        state.seen.push(Seen::GameWheel(
            event.scroll,
            event.accumulated,
            event.buttons,
            event.cursor,
            event.modifiers,
        ));
        flow(state.cancel == Cancel::GameWheel)
    }
}

pub(in crate::game) fn flow(cancel: bool) -> Flow {
    if cancel { Flow::Cancel } else { Flow::Continue }
}

pub(in crate::game) fn with_input_game(test: impl FnOnce(&mut Game<'_>, WindowId, &Mutex<State>)) {
    let state = Arc::new(Mutex::new(State::default()));
    let mods = vec![ModEntry::new(
        "input_route",
        Box::new(Observe(state.clone())),
    )]
    .into();
    with_mods(mods, || {
        with_client_game(|game| {
            let (_window_loop, id) = attach_offscreen_window(game);
            test(game, id, &state);
            assert!(
                game.error.is_none(),
                "window event failed: {:?}",
                game.error
            );
        });
    });
}

pub(in crate::game) fn take(state: &Mutex<State>) -> Vec<Seen> {
    std::mem::take(&mut state.lock().unwrap().seen)
}

pub(in crate::game) fn send(game: &mut Game<'_>, id: WindowId, event: WindowEvent) {
    game.handle_window_event(&NoLoop, id, event, InputOrigin::User);
}

/// Winit's KeyEvent platform field is private. Feed its normalized input to
/// the production window-key adapter, not directly to the screen hooks.
pub(in crate::game) fn key(game: &mut Game<'_>, key: KeyCode, state: ElementState) {
    game.dispatch_window_key(
        &NoLoop,
        WindowKeyInput {
            physical: PhysicalKey::Code(key),
            state,
            repeat: false,
            text: None,
            shortcut: None,
        },
        |_| false,
    );
}

#[allow(unused_imports)]
pub(in crate::game) use crate::game::client_side::native_hooks_client::base::screen_widgets_tests::input_routing::publish_frame;
