//! Behavioral tests for mounted screen widgets (`ScreenEvent.Init.Post` / `ScreenWidgets`),
//! synchronous transitions from widget callbacks, text field editing & IME integration, custom
//! `ScreenWidget` dispatch & panic isolation, and separated screen input Pre/Post hooks.

#[allow(unused_imports)]
use super::*;
#[allow(unused_imports)]
use crate::game::event_loop::EventLoopCtl;
#[allow(unused_imports)]
use crate::game::input_trace::InputOrigin;
#[allow(unused_imports)]
use crate::game::native_entity_refs::tests::{run_fresh_cases, with_client_game};
#[allow(unused_imports)]
use mod_api::client::widgets::{Button, TextField};
#[allow(unused_imports)]
use mod_api::{
    ClientHost, DrawContext, Flow, Hooks, ModEntry, ScreenCharTypedPostEvent,
    ScreenCharTypedPreEvent, ScreenClosingEvent, ScreenInitPostEvent, ScreenKeyPressedPostEvent,
    ScreenKeyPressedPreEvent, ScreenKeyReleasedPostEvent, ScreenKeyReleasedPreEvent,
    ScreenMouseButtonPressedPostEvent, ScreenMouseButtonPressedPreEvent,
    ScreenMouseButtonReleasedPostEvent, ScreenMouseButtonReleasedPreEvent,
    ScreenMouseDraggedPostEvent, ScreenMouseDraggedPreEvent, ScreenMouseScrolledPostEvent,
    ScreenMouseScrolledPreEvent, ScreenOpeningEvent, ScreenPreeditPostEvent, ScreenPreeditPreEvent,
    ScreenWidget, ScreenWidgetId, Text as ApiText,
};
#[allow(unused_imports)]
use mod_platform::testing::with_mods;
#[allow(unused_imports)]
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
#[allow(unused_imports)]
use winit::dpi::PhysicalSize;
#[allow(unused_imports)]
use winit::event::{
    DeviceId, ElementState, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent,
};
#[allow(unused_imports)]
use winit::event_loop::{ControlFlow, EventLoop};
#[allow(unused_imports)]
use winit::keyboard::KeyCode;

#[path = "screen_widgets_input_tests.rs"]
pub(in crate::game) mod input_routing;

pub(in crate::game) struct NoLoop;
impl EventLoopCtl for NoLoop {
    fn exit(&self) {}
    fn set_control_flow(&self, _: ControlFlow) {}
}

pub(in crate::game) struct DropCounter(pub(in crate::game) Arc<AtomicUsize>);
impl Drop for DropCounter {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[allow(unused_imports)]
pub(in crate::game) use crate::game::client_side::native_hooks_client::base::screen_widgets_tests::attach_offscreen_window;

pub(in crate::game) fn move_mouse(
    game: &mut crate::game::ClientGame<'_>,
    id: winit::window::WindowId,
    x: f64,
    y: f64,
) {
    game.handle_window_event(
        &NoLoop,
        id,
        WindowEvent::CursorMoved {
            device_id: DeviceId::dummy(),
            position: winit::dpi::PhysicalPosition::new(x, y),
        },
        InputOrigin::User,
    );
}

pub(in crate::game) fn send_mouse_button(
    game: &mut crate::game::ClientGame<'_>,
    id: winit::window::WindowId,
    button: MouseButton,
    pressed: bool,
) {
    game.handle_window_event(
        &NoLoop,
        id,
        WindowEvent::MouseInput {
            device_id: DeviceId::dummy(),
            state: if pressed {
                ElementState::Pressed
            } else {
                ElementState::Released
            },
            button,
        },
        InputOrigin::User,
    );
}

pub(in crate::game) fn send_scroll(
    game: &mut crate::game::ClientGame<'_>,
    id: winit::window::WindowId,
    x: f32,
    y: f32,
) {
    game.handle_window_event(
        &NoLoop,
        id,
        WindowEvent::MouseWheel {
            device_id: DeviceId::dummy(),
            delta: MouseScrollDelta::LineDelta(x, y),
            phase: TouchPhase::Moved,
        },
        InputOrigin::User,
    );
}

pub(in crate::game) fn send_key_control(game: &mut crate::game::ClientGame<'_>, line: &str) {
    game.replay_line(line, &NoLoop);
}

pub(in crate::game) fn take_log(log: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
    std::mem::take(&mut *log.lock().unwrap())
}

// ---------------------------------------------------------------------------
// Fast pure unit tests (run in `verify`)
// ---------------------------------------------------------------------------

/// `ScreenState::is_double_click` matches vanilla's 250ms window on the same screen
/// instance and mouse button (`MouseHandler.java:101-106`), surviving same-instance
/// widget rebuilds (`widgets.clear()` on resize) while clearing on replacement screens
/// (`bump_instance()`).
#[test]
fn double_click_window_checks_250ms_button_screen_and_generation() {
    let mut state = crate::game::screen_build::ScreenState::default();
    let t0 = 1_000;
    let inst = state.screen_instance;
    assert!(!state.is_double_click(t0, inst, None, 0));

    state.record_click(t0, inst, None, 0);
    assert!(state.is_double_click(t0 + 100, inst, None, 0));
    assert!(state.is_double_click(t0 + 249, inst, None, 0));
    assert!(!state.is_double_click(t0 + 250, inst, None, 0));
    assert!(!state.is_double_click(t0 + 50, inst, None, 1));
    assert!(!state.is_double_click(t0 + 50, inst + 1, None, 0));
    assert!(!state.is_double_click(t0 + 50, inst, Some(1), 0));

    // Same-instance widget rebuild (e.g. resize) preserves `last_click`.
    state.widgets.clear();
    assert!(state.is_double_click(t0 + 50, inst, None, 0));

    // Replacing the screen bumps `screen_instance` and clears `last_click`.
    let next_inst = state.bump_instance();
    assert_ne!(next_inst, inst);
    assert!(!state.is_double_click(t0 + 50, next_inst, None, 0));
    assert!(!state.is_double_click(t0 + 50, inst, None, 0));
}

/// `ObservedScreen::view` preserves `mod_screen` so Post listeners can identify
/// which mod's screen received an input event even after A->B or A->close transitions.
#[test]
fn observed_screen_view_carries_mod_screen_owner() {
    let observed = crate::game::screen_build::ObservedScreen {
        id: "mod",
        overlay: false,
        mod_screen: Some("mod_a".to_owned()),
        menu: None,
        menu_generation: None,
        dialog_key: None,
        title: None,
        rect: [0.0, 0.0, 320.0, 240.0],
        screen_size: [320.0, 240.0],
        slots: 0,
        mouse: [10.0, 20.0],
        partial_tick: 0.5,
        regions: Vec::new(),
    };
    let view = observed.view();
    assert_eq!(view.id, "mod");
    assert_eq!(view.mod_screen, Some("mod_a"));
    let other = screen_view_for_mod(&view, "mod_b");
    assert_eq!(other.mod_screen, Some("mod_b"));
}

// ---------------------------------------------------------------------------
// Daily tests (prepared resources + offscreen renderer, isolated per process)
// ---------------------------------------------------------------------------
