//! Behavioral tests for mounted screen widgets (`ScreenEvent.Init.Post` / `ScreenWidgets`),
//! synchronous transitions from widget callbacks, text field editing & IME integration, custom
//! `ScreenWidget` dispatch & panic isolation, and separated screen input Pre/Post hooks.

use super::*;
use crate::game::event_loop::EventLoopCtl;
use crate::game::input_trace::InputOrigin;
use crate::game::native_entity_refs::tests::{run_fresh_cases, with_client_game};
use mod_api::client::widgets::{Button, TextField};
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
use mod_platform::testing::with_mods;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};
use winit::dpi::PhysicalSize;
use winit::event::{
    DeviceId, ElementState, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent,
};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::keyboard::KeyCode;
use winit::window::Window;

struct NoLoop;
impl EventLoopCtl for NoLoop {
    fn exit(&self) {}
    fn set_control_flow(&self, _: ControlFlow) {}
}

struct DropCounter(Arc<AtomicUsize>);
impl Drop for DropCounter {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

/// Attach a hidden offscreen window and renderer to `game` so screen hooks, `mod_screen_open`,
/// and real `WindowEvent` input paths execute end to end. Returns the `EventLoop` (kept alive
/// for the test's duration) and `WindowId`.
fn attach_offscreen_window(game: &mut Game<'_>) -> (EventLoop<()>, winit::window::WindowId) {
    let mut builder = EventLoop::builder();
    #[cfg(target_os = "linux")]
    {
        use winit::platform::x11::EventLoopBuilderExtX11;
        builder.with_x11().with_any_thread(true);
    }
    let event_loop = builder.build().unwrap();
    #[allow(deprecated)]
    let window = Arc::new(
        event_loop
            .create_window(
                Window::default_attributes()
                    .with_visible(false)
                    .with_inner_size(PhysicalSize::new(640, 480)),
            )
            .unwrap(),
    );
    let id = window.id();
    game.options.offscreen = true;
    game.settings.gui_scale = 1;
    game.window = Some(window.clone());
    game.renderer = Some(game.new_renderer(window).unwrap());
    game.title = None;
    game.focused = true;
    (event_loop, id)
}

fn move_mouse(game: &mut Game<'_>, id: winit::window::WindowId, x: f64, y: f64) {
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

fn send_mouse_button(
    game: &mut Game<'_>,
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

fn send_scroll(game: &mut Game<'_>, id: winit::window::WindowId, x: f32, y: f32) {
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

fn send_key_control(game: &mut Game<'_>, line: &str) {
    game.replay_line(line, &NoLoop);
}

fn take_log(log: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
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
    let t0 = Instant::now();
    let inst = state.screen_instance;
    assert!(!state.is_double_click(t0, inst, 0));

    state.record_click(t0, inst, 0);
    assert!(state.is_double_click(t0 + Duration::from_millis(100), inst, 0));
    assert!(state.is_double_click(t0 + Duration::from_millis(249), inst, 0));
    assert!(!state.is_double_click(t0 + Duration::from_millis(250), inst, 0));
    assert!(!state.is_double_click(t0 + Duration::from_millis(50), inst, 1));
    assert!(!state.is_double_click(t0 + Duration::from_millis(50), inst + 1, 0));

    // Same-instance widget rebuild (e.g. resize) preserves `last_click`.
    state.widgets.clear();
    assert!(state.is_double_click(t0 + Duration::from_millis(50), inst, 0));

    // Replacing the screen bumps `screen_instance` and clears `last_click`.
    let next_inst = state.bump_instance();
    assert_ne!(next_inst, inst);
    assert!(!state.is_double_click(t0 + Duration::from_millis(50), next_inst, 0));
    assert!(!state.is_double_click(t0 + Duration::from_millis(50), inst, 0));
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
        title: None,
        rect: [0.0, 0.0, 320.0, 240.0],
        screen_size: [320.0, 240.0],
        slots: 0,
        mouse: [10.0, 20.0],
        partial_tick: 0.5,
        instance: 7,
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

/// Mounting at Init & lifecycle: `add_button`, `add_text_field`, `add_widget` assign distinct
/// `ScreenWidgetId`s; `set_initial_focus` focuses the requested widget; `remove` drops removed
/// widgets during init; closing or resizing disposes mounted widgets via `Drop` and clears
/// focus/drag state.
#[test]
#[ignore = "daily"]
fn mounting_lifecycle_focus_remove_resize_and_close_dispose() {
    if run_fresh_cases(
        "game::native_hooks::base::screen_widgets_tests::mounting_lifecycle_focus_remove_resize_and_close_dispose",
        "SUNRISE_WIDGETS_LIFECYCLE_CHILD",
        &["window"],
    ) {
        return;
    }

    struct DummyCustom {
        bounds: [f32; 4],
        _drop: DropCounter,
    }
    impl ScreenWidget for DummyCustom {
        fn bounds(&self) -> [f32; 4] {
            self.bounds
        }
    }

    struct MountMod {
        drops: Arc<AtomicUsize>,
        ids: Arc<Mutex<Vec<ScreenWidgetId>>>,
        focus_results: Arc<Mutex<Vec<bool>>>,
        remove_results: Arc<Mutex<Vec<bool>>>,
        init_count: Arc<AtomicUsize>,
    }

    #[mod_api::__subscriptions]
    impl Hooks for MountMod {
        fn screen_init_post(&self, host: &dyn ClientHost, event: &mut ScreenInitPostEvent<'_>) {
            if host.mod_screen() != Some("mount_mod") {
                return;
            }
            self.init_count.fetch_add(1, Ordering::SeqCst);
            let btn_drop = DropCounter(self.drops.clone());
            let btn_id = event.widgets.add_button(Button::new(
                [10.0, 10.0, 80.0, 20.0],
                ApiText::literal("Btn"),
                move |_state, _host| {
                    let _ = &btn_drop;
                },
            ));
            let tf_drop = DropCounter(self.drops.clone());
            let tf_id = event.widgets.add_text_field(
                TextField::new([10.0, 40.0, 120.0, 20.0], 32).on_change(move |_text, _host| {
                    let _ = &tf_drop;
                }),
            );
            let removed_drop = DropCounter(self.drops.clone());
            let removed_id = event.widgets.add_widget(Box::new(DummyCustom {
                bounds: [10.0, 70.0, 40.0, 20.0],
                _drop: removed_drop,
            }));
            let kept_id = event.widgets.add_widget(Box::new(DummyCustom {
                bounds: [60.0, 70.0, 40.0, 20.0],
                _drop: DropCounter(self.drops.clone()),
            }));
            let inactive_id = event.widgets.add_button(Button::inactive(
                [10.0, 100.0, 80.0, 20.0],
                ApiText::literal("Off"),
            ));

            self.ids
                .lock()
                .unwrap()
                .extend([btn_id, tf_id, removed_id, kept_id, inactive_id]);

            let removed_first = event.widgets.remove(removed_id);
            let removed_again = event.widgets.remove(removed_id);
            self.remove_results
                .lock()
                .unwrap()
                .extend([removed_first, removed_again]);

            let focus_removed = event.widgets.set_initial_focus(removed_id);
            let focus_inactive = event.widgets.set_initial_focus(inactive_id);
            let focus_tf = event.widgets.set_initial_focus(tf_id);
            self.focus_results
                .lock()
                .unwrap()
                .extend([focus_removed, focus_inactive, focus_tf]);
        }
    }

    let drops = Arc::new(AtomicUsize::new(0));
    let ids = Arc::new(Mutex::new(Vec::new()));
    let focus_results = Arc::new(Mutex::new(Vec::new()));
    let remove_results = Arc::new(Mutex::new(Vec::new()));
    let init_count = Arc::new(AtomicUsize::new(0));
    let mods: Arc<[ModEntry]> = vec![ModEntry::new(
        "mount_mod",
        Box::new(MountMod {
            drops: drops.clone(),
            ids: ids.clone(),
            focus_results: focus_results.clone(),
            remove_results: remove_results.clone(),
            init_count: init_count.clone(),
        }),
    )]
    .into();

    with_mods(mods, || {
        with_client_game(|game| {
            let (_event_loop, win_id) = attach_offscreen_window(game);

            assert!(game.mod_screen_open("mount_mod"));
            assert_eq!(init_count.load(Ordering::SeqCst), 1);

            // All 5 mounted widgets received distinct ScreenWidgetIds.
            let assigned = std::mem::take(&mut *ids.lock().unwrap());
            assert_eq!(assigned.len(), 5);
            for i in 0..assigned.len() {
                for j in (i + 1)..assigned.len() {
                    assert_ne!(assigned[i], assigned[j]);
                }
            }
            assert_eq!(
                std::mem::take(&mut *remove_results.lock().unwrap()),
                vec![true, false]
            );
            assert_eq!(
                std::mem::take(&mut *focus_results.lock().unwrap()),
                vec![false, false, true]
            );
            // The removed custom widget was dropped immediately during init; 3 live drop counters remain.
            assert_eq!(drops.load(Ordering::SeqCst), 1);
            // 4 entries stay mounted (btn, tf, kept custom, inactive btn) and tf (index 1) has focus.
            assert_eq!(game.screen_state.widgets.len(), 4);
            assert_eq!(game.screen_state.widgets.focused_index(), Some(1));

            // Click into the text field to establish drag state, then resize the window.
            move_mouse(game, win_id, 30.0, 50.0);
            send_mouse_button(game, win_id, MouseButton::Left, true);
            assert_eq!(game.screen_state.widgets.dragging(), Some((1, 0)));

            let gen_before_resize = game.screen_state.widgets.generation();
            game.handle_window_event(
                &NoLoop,
                win_id,
                WindowEvent::Resized(PhysicalSize::new(800, 600)),
                InputOrigin::User,
            );
            game.observe_screen();
            assert_eq!(init_count.load(Ordering::SeqCst), 2);
            assert_ne!(game.screen_state.widgets.generation(), gen_before_resize);
            // Prior 3 live drop counters + 1 newly removed drop counter = 5 total drops.
            assert_eq!(drops.load(Ordering::SeqCst), 5);
            // Drag state cleared on rebuild; initial focus re-applied to the newly mounted text field.
            assert_eq!(game.screen_state.widgets.dragging(), None);
            assert_eq!(game.screen_state.widgets.focused_index(), Some(1));

            // Closing the screen disposes the remaining 3 widgets and clears focus/drag state.
            assert!(game.close_screen());
            assert_eq!(game.screen, Screen::Playing);
            assert_eq!(drops.load(Ordering::SeqCst), 8);
            assert!(game.screen_state.widgets.is_empty());
            assert_eq!(game.screen_state.widgets.focused_index(), None);
            assert_eq!(game.screen_state.widgets.dragging(), None);
        });
    });
}

/// Button `on_press` & synchronous screen transitions (Audit [86]):
/// - Pressing a button mutates its `ButtonState` (`label`, `active`).
/// - Enter and Space activate a focused button.
/// - `host.mod_screen_open("mod_b")` inside a `mod_a` button `on_press` runs synchronously with
///   no borrow panic, switches to `mod_b`, mounts `mod_b`'s widgets, disposes the extracted
///   `mod_a` button, and never overwrites `mod_b`'s widget table.
/// - `host.close_screen()` inside a button `on_press` closes synchronously to `Playing`.
#[test]
#[ignore = "daily"]
fn button_on_press_state_mutation_keyboard_and_synchronous_transitions() {
    if run_fresh_cases(
        "game::native_hooks::base::screen_widgets_tests::button_on_press_state_mutation_keyboard_and_synchronous_transitions",
        "SUNRISE_WIDGETS_BUTTON_TRANSITION_CHILD",
        &["window"],
    ) {
        return;
    }

    struct ModA {
        log: Arc<Mutex<Vec<String>>>,
        mod_a_btn_drops: Arc<AtomicUsize>,
    }

    #[mod_api::__subscriptions]
    impl Hooks for ModA {
        fn screen_init_post(&self, host: &dyn ClientHost, event: &mut ScreenInitPostEvent<'_>) {
            if host.mod_screen() != Some("mod_a") {
                return;
            }
            let log_toggle = self.log.clone();
            let toggle_id = event.widgets.add_button(Button::new(
                [20.0, 20.0, 100.0, 20.0],
                ApiText::literal("Count 0"),
                move |state, _host| {
                    let was_initial = state.label == ApiText::literal("Count 0");
                    log_toggle
                        .lock()
                        .unwrap()
                        .push(format!("toggle:{was_initial}"));
                    state.label = ApiText::literal("Count 1");
                    state.active = false;
                },
            ));
            let log_kb = self.log.clone();
            let kb_id = event.widgets.add_button(Button::new(
                [20.0, 50.0, 100.0, 20.0],
                ApiText::literal("Keyboard"),
                move |state, _host| {
                    log_kb.lock().unwrap().push("kb_pressed".into());
                    state.label = ApiText::literal("KB Done");
                },
            ));
            let log_switch = self.log.clone();
            let switch_drop = DropCounter(self.mod_a_btn_drops.clone());
            event.widgets.add_button(Button::new(
                [20.0, 80.0, 100.0, 20.0],
                ApiText::literal("To B"),
                move |_state, host| {
                    let _ = &switch_drop;
                    let opened = host.mod_screen_open("mod_b");
                    log_switch
                        .lock()
                        .unwrap()
                        .push(format!("switch_to_b:{opened}:{:?}", host.mod_screen()));
                },
            ));
            let _ = toggle_id;
            assert!(event.widgets.set_initial_focus(kb_id));
        }
    }

    struct ModB {
        log: Arc<Mutex<Vec<String>>>,
        mod_b_btn_drops: Arc<AtomicUsize>,
    }

    #[mod_api::__subscriptions]
    impl Hooks for ModB {
        fn screen_init_post(&self, host: &dyn ClientHost, event: &mut ScreenInitPostEvent<'_>) {
            if host.mod_screen() != Some("mod_b") {
                return;
            }
            self.log.lock().unwrap().push("init_mod_b".into());
            let log_b = self.log.clone();
            let drop_b = DropCounter(self.mod_b_btn_drops.clone());
            let close_id = event.widgets.add_button(Button::new(
                [20.0, 80.0, 100.0, 20.0],
                ApiText::literal("Close B"),
                move |_state, host| {
                    let _ = &drop_b;
                    let closed = host.close_screen();
                    log_b
                        .lock()
                        .unwrap()
                        .push(format!("close_b:{closed}:{:?}", host.mod_screen()));
                },
            ));
            assert!(event.widgets.set_initial_focus(close_id));
        }
    }

    let log = Arc::new(Mutex::new(Vec::new()));
    let mod_a_btn_drops = Arc::new(AtomicUsize::new(0));
    let mod_b_btn_drops = Arc::new(AtomicUsize::new(0));
    let mods: Arc<[ModEntry]> = vec![
        ModEntry::new(
            "mod_a",
            Box::new(ModA {
                log: log.clone(),
                mod_a_btn_drops: mod_a_btn_drops.clone(),
            }),
        ),
        ModEntry::new(
            "mod_b",
            Box::new(ModB {
                log: log.clone(),
                mod_b_btn_drops: mod_b_btn_drops.clone(),
            }),
        ),
    ]
    .into();

    with_mods(mods, || {
        with_client_game(|game| {
            let (_event_loop, win_id) = attach_offscreen_window(game);
            assert!(game.mod_screen_open("mod_a"));

            // Enter and Space activate the initially focused button.
            assert!(game.mounted_widget_key_pressed(KeyCode::Enter, false, 0));
            assert!(game.mounted_widget_key_pressed(KeyCode::Space, false, 0));
            assert_eq!(take_log(&log), vec!["kb_pressed", "kb_pressed"]);

            // Clicking the toggle button mutates its ButtonState (label + active = false).
            move_mouse(game, win_id, 50.0, 30.0);
            send_mouse_button(game, win_id, MouseButton::Left, true);
            send_mouse_button(game, win_id, MouseButton::Left, false);
            assert_eq!(take_log(&log), vec!["toggle:true"]);
            // Second click does nothing because the button disabled itself.
            send_mouse_button(game, win_id, MouseButton::Left, true);
            send_mouse_button(game, win_id, MouseButton::Left, false);
            assert!(take_log(&log).is_empty());

            // Clicking "To B" calls host.mod_screen_open("mod_b") synchronously inside on_press.
            move_mouse(game, win_id, 50.0, 90.0);
            send_mouse_button(game, win_id, MouseButton::Left, true);
            send_mouse_button(game, win_id, MouseButton::Left, false);
            assert_eq!(game.screen, Screen::Mod);
            assert_eq!(game.mod_screen(), Some("mod_b"));
            assert_eq!(
                take_log(&log),
                vec!["init_mod_b", "switch_to_b:true:Some(\"mod_b\")"]
            );
            // Extracted mod_a button was disposed rather than written back into mod_b's table.
            assert_eq!(mod_a_btn_drops.load(Ordering::SeqCst), 1);
            assert_eq!(game.screen_state.widgets.len(), 1);
            assert_eq!(mod_b_btn_drops.load(Ordering::SeqCst), 0);

            // Pressing Enter on mod_b's focused button calls host.close_screen() synchronously.
            assert!(game.mounted_widget_key_pressed(KeyCode::Enter, false, 0));
            assert_eq!(game.screen, Screen::Playing);
            assert_eq!(game.mod_screen(), None);
            assert_eq!(take_log(&log), vec!["close_b:true:None"]);
            assert_eq!(mod_b_btn_drops.load(Ordering::SeqCst), 1);
            assert!(game.screen_state.widgets.is_empty());
        });
    });
}

/// TextField local state, IME & double-click:
/// - Typing, Backspace, Delete, Left/Right/Home/End, Ctrl+A, and Enter update local `EditBox`
///   and fire `on_change` / `on_submit` with no `DrawContext` round-trip.
/// - Focused text field reports `text_field_focused() == true`; `ime_preedit` is taken with no
///   `screen_preedit_post`; `ime_commit` inserts text and fires `on_change`.
/// - Unfocusing (or focusing a non-capturing custom widget) flips `text_field_focused() == false`
///   and routes `ime_preedit` to `screen_preedit_post`.
/// - Double-clicking inside the text field within 250ms selects the word under the cursor; a
///   click outside the window does not set `double_click`.
#[test]
#[ignore = "daily"]
fn text_field_editing_ime_and_double_click_word_selection() {
    if run_fresh_cases(
        "game::native_hooks::base::screen_widgets_tests::text_field_editing_ime_and_double_click_word_selection",
        "SUNRISE_WIDGETS_TEXTFIELD_CHILD",
        &["window"],
    ) {
        return;
    }

    struct NonCapturingCustom;
    impl ScreenWidget for NonCapturingCustom {
        fn bounds(&self) -> [f32; 4] {
            [20.0, 60.0, 80.0, 20.0]
        }
        fn mouse_clicked(
            &mut self,
            _host: &mut dyn ClientHost,
            _mouse: [f32; 2],
            _button: i32,
            _modifiers: u32,
            _double_click: bool,
        ) -> bool {
            true
        }
    }

    struct TextFieldMod {
        changes: Arc<Mutex<Vec<String>>>,
        submits: Arc<Mutex<Vec<String>>>,
        preedit_posts: Arc<Mutex<Vec<String>>>,
        pressed_double_clicks: Arc<Mutex<Vec<bool>>>,
    }

    #[mod_api::__subscriptions]
    impl Hooks for TextFieldMod {
        fn screen_init_post(&self, host: &dyn ClientHost, event: &mut ScreenInitPostEvent<'_>) {
            if host.mod_screen() != Some("tf_mod") {
                return;
            }
            let changes = self.changes.clone();
            let submits = self.submits.clone();
            let tf_id = event.widgets.add_text_field(
                TextField::new([20.0, 20.0, 200.0, 20.0], 64)
                    .hint(ApiText::literal("Hint"))
                    .on_change(move |text, _host| {
                        changes.lock().unwrap().push(text.to_owned());
                    })
                    .on_submit(move |text, _host| {
                        submits.lock().unwrap().push(text.to_owned());
                    }),
            );
            event.widgets.add_widget(Box::new(NonCapturingCustom));
            assert!(event.widgets.set_initial_focus(tf_id));
        }

        fn screen_preedit_post(
            &self,
            _host: &dyn ClientHost,
            event: &mut ScreenPreeditPostEvent<'_>,
        ) {
            self.preedit_posts
                .lock()
                .unwrap()
                .push(format!("{}:{}", event.screen.id, event.full_text));
        }

        fn screen_mouse_button_pressed_pre(
            &self,
            _host: &dyn ClientHost,
            event: &mut ScreenMouseButtonPressedPreEvent<'_>,
        ) -> Flow {
            self.pressed_double_clicks
                .lock()
                .unwrap()
                .push(event.double_click);
            Flow::Continue
        }
    }

    let changes = Arc::new(Mutex::new(Vec::new()));
    let submits = Arc::new(Mutex::new(Vec::new()));
    let preedit_posts = Arc::new(Mutex::new(Vec::new()));
    let pressed_double_clicks = Arc::new(Mutex::new(Vec::new()));
    let mods: Arc<[ModEntry]> = vec![ModEntry::new(
        "tf_mod",
        Box::new(TextFieldMod {
            changes: changes.clone(),
            submits: submits.clone(),
            preedit_posts: preedit_posts.clone(),
            pressed_double_clicks: pressed_double_clicks.clone(),
        }),
    )]
    .into();

    with_mods(mods, || {
        with_client_game(|game| {
            let (_event_loop, win_id) = attach_offscreen_window(game);
            assert!(game.mod_screen_open("tf_mod"));

            // Typing directly into the mounted TextField without any render / DrawContext round-trip.
            for ch in "hello world".chars() {
                assert!(game.mounted_widget_char_typed(ch, 0));
            }
            assert_eq!(
                changes.lock().unwrap().last().map(String::as_str),
                Some("hello world")
            );
            changes.lock().unwrap().clear();

            // Backspace deletes 'd', Home moves to start, Delete removes 'h', Right moves past 'e',
            // typing '!' inserts in place, End moves to end.
            assert!(game.mounted_widget_key_pressed(KeyCode::Backspace, false, 0));
            assert!(game.mounted_widget_key_pressed(KeyCode::Home, false, 0));
            assert!(game.mounted_widget_key_pressed(KeyCode::Delete, false, 0));
            assert!(game.mounted_widget_key_pressed(KeyCode::ArrowRight, false, 0));
            assert!(game.mounted_widget_char_typed('!', 0));
            assert!(game.mounted_widget_key_pressed(KeyCode::End, false, 0));
            assert!(game.mounted_widget_char_typed('?', 0));
            assert_eq!(
                take_log(&changes),
                vec!["hello worl", "ello worl", "e!llo worl", "e!llo worl?"]
            );

            // Ctrl+A selects all, typing replaces the whole selection, Enter fires on_submit.
            let select_mod = if cfg!(target_os = "macos") { 8 } else { 2 };
            assert!(game.mounted_widget_key_pressed(KeyCode::KeyA, false, select_mod));
            for ch in "alpha beta".chars() {
                assert!(game.mounted_widget_char_typed(ch, 0));
            }
            assert!(game.mounted_widget_key_pressed(KeyCode::Enter, false, 0));
            assert_eq!(take_log(&submits), vec!["alpha beta"]);
            changes.lock().unwrap().clear();

            // Focused text field captures IME preedit and commit.
            assert!(game.text_field_focused());
            game.ime_preedit("ka", Some((0, 2)));
            assert!(take_log(&preedit_posts).is_empty());
            game.ime_commit("!");
            assert_eq!(take_log(&changes), vec!["alpha beta!"]);

            // Double-click inside the text field selects the first word ("alpha").
            pressed_double_clicks.lock().unwrap().clear();
            move_mouse(game, win_id, 30.0, 30.0);
            send_mouse_button(game, win_id, MouseButton::Left, true);
            send_mouse_button(game, win_id, MouseButton::Left, false);
            send_mouse_button(game, win_id, MouseButton::Left, true);
            send_mouse_button(game, win_id, MouseButton::Left, false);
            assert_eq!(
                std::mem::take(&mut *pressed_double_clicks.lock().unwrap()),
                vec![false, true]
            );
            // Replacing the double-click selection replaces "alpha", keeping " beta!".
            assert!(game.mounted_widget_char_typed('X', 0));
            assert_eq!(take_log(&changes), vec!["X beta!"]);

            // Clicking the non-capturing custom widget shifts focus off the text field.
            move_mouse(game, win_id, 40.0, 70.0);
            send_mouse_button(game, win_id, MouseButton::Left, true);
            send_mouse_button(game, win_id, MouseButton::Left, false);
            assert!(!game.text_field_focused());
            game.ime_preedit("ni", Some((0, 2)));
            assert_eq!(take_log(&preedit_posts), vec!["mod:ni"]);

            // Clicking outside all widgets clears focus too.
            move_mouse(game, win_id, 400.0, 400.0);
            send_mouse_button(game, win_id, MouseButton::Left, true);
            send_mouse_button(game, win_id, MouseButton::Left, false);
            assert_eq!(game.screen_state.widgets.focused_index(), None);
            assert!(!game.text_field_focused());
            game.ime_preedit("go", Some((0, 2)));
            assert_eq!(take_log(&preedit_posts), vec!["mod:go"]);
        });
    });
}

/// Open `ScreenWidget` full callback surface & per-widget panic isolation:
/// - Verifies `render`, `mouse_clicked` (including `double_click`), `mouse_dragged`,
///   `mouse_released`, `mouse_scrolled`, `key_pressed`, `key_released`, `char_typed`, and
///   `set_focused`.
/// - Panics in a button `on_press`, a text field `on_change`, and a custom `ScreenWidget`
///   disable ONLY the panicking widget, queue `CallbackPanicNotice`s, and leave sibling widgets
///   working.
/// - A non-panic refusal (`host.mod_screen_open(...)` returning `false`) does NOT disable the
///   widget.
#[test]
#[ignore = "daily"]
fn custom_screen_widget_callbacks_and_per_widget_panic_isolation() {
    if run_fresh_cases(
        "game::native_hooks::base::screen_widgets_tests::custom_screen_widget_callbacks_and_per_widget_panic_isolation",
        "SUNRISE_WIDGETS_CUSTOM_PANIC_CHILD",
        &["window"],
    ) {
        return;
    }

    struct RecordingWidget {
        bounds: [f32; 4],
        log: Arc<Mutex<Vec<String>>>,
        panic_on_click: Arc<AtomicBool>,
    }

    impl ScreenWidget for RecordingWidget {
        fn bounds(&self) -> [f32; 4] {
            self.bounds
        }
        fn render(
            &mut self,
            _host: &dyn ClientHost,
            draw: &mut dyn DrawContext,
            mouse: [f32; 2],
            hovered: bool,
            focused: bool,
            _partial_tick: f32,
        ) {
            draw.fill(self.bounds, 0xFF22_4466);
            self.log.lock().unwrap().push(format!(
                "render:{:.0},{:.0}:{hovered}:{focused}",
                mouse[0], mouse[1]
            ));
        }
        fn mouse_clicked(
            &mut self,
            _host: &mut dyn ClientHost,
            _mouse: [f32; 2],
            button: i32,
            _modifiers: u32,
            double_click: bool,
        ) -> bool {
            if self.panic_on_click.load(Ordering::SeqCst) {
                panic!("custom widget boom");
            }
            self.log
                .lock()
                .unwrap()
                .push(format!("click:{button}:{double_click}"));
            true
        }
        fn mouse_dragged(
            &mut self,
            _host: &mut dyn ClientHost,
            _mouse: [f32; 2],
            button: i32,
            delta: [f32; 2],
            _modifiers: u32,
        ) -> bool {
            self.log
                .lock()
                .unwrap()
                .push(format!("drag:{button}:{:.0},{:.0}", delta[0], delta[1]));
            true
        }
        fn mouse_released(
            &mut self,
            _host: &mut dyn ClientHost,
            _mouse: [f32; 2],
            button: i32,
            _modifiers: u32,
        ) -> bool {
            self.log.lock().unwrap().push(format!("release:{button}"));
            true
        }
        fn mouse_scrolled(
            &mut self,
            _host: &mut dyn ClientHost,
            _mouse: [f32; 2],
            scroll: [f64; 2],
            _modifiers: u32,
        ) -> bool {
            self.log
                .lock()
                .unwrap()
                .push(format!("scroll:{:.0},{:.0}", scroll[0], scroll[1]));
            true
        }
        fn key_pressed(
            &mut self,
            _host: &mut dyn ClientHost,
            key: &str,
            repeat: bool,
            _modifiers: u32,
        ) -> bool {
            self.log
                .lock()
                .unwrap()
                .push(format!("key_down:{key}:{repeat}"));
            true
        }
        fn key_released(&mut self, _host: &mut dyn ClientHost, key: &str, _modifiers: u32) -> bool {
            self.log.lock().unwrap().push(format!("key_up:{key}"));
            true
        }
        fn char_typed(&mut self, _host: &mut dyn ClientHost, c: char, _modifiers: u32) -> bool {
            self.log.lock().unwrap().push(format!("char:{c}"));
            true
        }
        fn set_focused(&mut self, focused: bool) {
            self.log.lock().unwrap().push(format!("focus:{focused}"));
        }
    }

    struct PanicMod {
        log: Arc<Mutex<Vec<String>>>,
        panic_custom: Arc<AtomicBool>,
        block_open: Arc<AtomicBool>,
    }

    #[mod_api::__subscriptions]
    impl Hooks for PanicMod {
        fn screen_opening(
            &self,
            _host: &dyn ClientHost,
            event: &mut ScreenOpeningEvent<'_>,
        ) -> Option<ScreenOpening> {
            if self.block_open.load(Ordering::SeqCst) && event.new.id == "mod" {
                Some(ScreenOpening::Cancel)
            } else {
                None
            }
        }

        fn screen_init_post(&self, host: &dyn ClientHost, event: &mut ScreenInitPostEvent<'_>) {
            if host.mod_screen() != Some("panic_mod") {
                return;
            }
            // 0: Recording custom widget (can panic on click once armed)
            event.widgets.add_widget(Box::new(RecordingWidget {
                bounds: [20.0, 20.0, 80.0, 20.0],
                log: self.log.clone(),
                panic_on_click: self.panic_custom.clone(),
            }));
            // 1: Panicking button
            event.widgets.add_button(Button::new(
                [20.0, 50.0, 80.0, 20.0],
                ApiText::literal("BoomBtn"),
                |_state, _host| {
                    panic!("button boom");
                },
            ));
            // 2: Panicking text field
            event
                .widgets
                .add_text_field(TextField::new([20.0, 80.0, 80.0, 20.0], 16).on_change(
                    |_text, _host| {
                        panic!("textfield boom");
                    },
                ));
            // 3: Healthy sibling button that also exercises a non-panic refusal (`mod_screen_open` -> false)
            let log_sibling = self.log.clone();
            event.widgets.add_button(Button::new(
                [20.0, 110.0, 80.0, 20.0],
                ApiText::literal("Sibling"),
                move |_state, host| {
                    let opened = host.mod_screen_open("other_mod");
                    log_sibling
                        .lock()
                        .unwrap()
                        .push(format!("sibling_open:{opened}"));
                },
            ));
        }
    }

    let log = Arc::new(Mutex::new(Vec::new()));
    let panic_custom = Arc::new(AtomicBool::new(false));
    let block_open = Arc::new(AtomicBool::new(false));
    let mods: Arc<[ModEntry]> = vec![ModEntry::new(
        "panic_mod",
        Box::new(PanicMod {
            log: log.clone(),
            panic_custom: panic_custom.clone(),
            block_open: block_open.clone(),
        }),
    )]
    .into();

    with_mods(mods, || {
        with_client_game(|game| {
            let (_event_loop, win_id) = attach_offscreen_window(game);
            let _ = mod_api::__private::game::take_callback_panic_notices();
            assert!(game.mod_screen_open("panic_mod"));

            // Exercise every ScreenWidget callback on the recording widget.
            move_mouse(game, win_id, 40.0, 30.0);
            send_mouse_button(game, win_id, MouseButton::Left, true);
            move_mouse(game, win_id, 55.0, 30.0);
            send_mouse_button(game, win_id, MouseButton::Left, false);
            // Second click within 250ms delivers double_click = true.
            send_mouse_button(game, win_id, MouseButton::Left, true);
            send_mouse_button(game, win_id, MouseButton::Left, false);
            send_scroll(game, win_id, 0.0, 1.0);
            assert!(game.mounted_widget_key_pressed(KeyCode::KeyG, false, 0));
            assert!(game.mounted_widget_key_released(KeyCode::KeyG, 0));
            assert!(game.mounted_widget_char_typed('g', 0));
            let mut batch = HudBatch::scratch([640, 480]);
            game.draw_mounted_screen_widgets(&mut batch);

            let events = take_log(&log);
            assert!(events.iter().any(|e| e == "click:0:false"), "{events:?}");
            assert!(events.iter().any(|e| e == "focus:true"), "{events:?}");
            assert!(events.iter().any(|e| e == "drag:0:15,0"), "{events:?}");
            assert!(events.iter().any(|e| e == "release:0"), "{events:?}");
            assert!(events.iter().any(|e| e == "click:0:true"), "{events:?}");
            assert!(
                events.iter().any(|e| e.starts_with("scroll:0,")),
                "{events:?}"
            );
            assert!(
                events.iter().any(|e| e == "key_down:key.keyboard.g:false"),
                "{events:?}"
            );
            assert!(
                events.iter().any(|e| e == "key_up:key.keyboard.g"),
                "{events:?}"
            );
            assert!(events.iter().any(|e| e == "char:g"), "{events:?}");
            assert!(
                events.iter().any(|e| e == "render:55,30:true:true"),
                "{events:?}"
            );

            // 1) Panic in Button on_press disables only the button and queues a panic notice.
            move_mouse(game, win_id, 40.0, 60.0);
            send_mouse_button(game, win_id, MouseButton::Left, true);
            send_mouse_button(game, win_id, MouseButton::Left, false);
            assert!(game.screen_state.widgets.is_disabled(1));

            // 2) Panic in TextField on_change disables only the text field.
            move_mouse(game, win_id, 40.0, 90.0);
            send_mouse_button(game, win_id, MouseButton::Left, true);
            send_mouse_button(game, win_id, MouseButton::Left, false);
            assert!(game.mounted_widget_char_typed('a', 0));
            assert!(game.screen_state.widgets.is_disabled(2));

            // 3) Panic in custom ScreenWidget mouse_clicked disables only the custom widget.
            panic_custom.store(true, Ordering::SeqCst);
            move_mouse(game, win_id, 40.0, 30.0);
            send_mouse_button(game, win_id, MouseButton::Left, true);
            send_mouse_button(game, win_id, MouseButton::Left, false);
            assert!(game.screen_state.widgets.is_disabled(0));

            let notices = mod_api::__private::game::take_callback_panic_notices();
            assert_eq!(notices.len(), 3, "{notices:?}");
            assert!(notices.iter().any(|n| n.operation == "button_press"));
            assert!(notices.iter().any(|n| n.operation == "text_field_change"));
            assert!(
                notices
                    .iter()
                    .any(|n| n.operation == "screen_widget_mouse_clicked")
            );

            // 4) Healthy sibling keeps working; non-panic refusal (`mod_screen_open` returning false)
            // does NOT disable the widget and queues no panic notice.
            block_open.store(true, Ordering::SeqCst);
            take_log(&log);
            move_mouse(game, win_id, 40.0, 120.0);
            send_mouse_button(game, win_id, MouseButton::Left, true);
            send_mouse_button(game, win_id, MouseButton::Left, false);
            send_mouse_button(game, win_id, MouseButton::Left, true);
            send_mouse_button(game, win_id, MouseButton::Left, false);
            assert_eq!(
                take_log(&log),
                vec!["sibling_open:false", "sibling_open:false"]
            );
            assert!(!game.screen_state.widgets.is_disabled(3));
            assert!(mod_api::__private::game::take_callback_panic_notices().is_empty());
        });
    });
}

/// Separate input Post & Pre parity + receiving screen:
/// - `MouseButtonPressed.Pre` & `.Post` receive `double_click: bool`.
/// - `MouseButtonPressed.Post` & `MouseButtonReleased.Post` receive `event.handled: bool`
///   reflecting whether a widget/screen handled the input; overriding `event.handled = false`
///   prevents `last_click` recording while `event.handled = true` records it.
/// - A button press transitioning screen A -> screen B (or closing to `Playing`) still fires
///   `MouseButtonPressed.Post` with screen A's `event.screen.id`.
/// - `MouseDragged.Post`, `MouseScrolled.Post`, `CharacterTyped.Post`, `Preedit.Post` fire ONLY
///   when unhandled on the receiving screen (and are non-cancellable notices).
/// - `KeyPressed.Post` and `KeyReleased.Post` fire ONLY when unhandled on the receiving screen,
///   and returning `Flow::Cancel` blocks `mapped_key_event`.
#[test]
#[ignore = "daily"]
fn separate_input_pre_post_parity_and_receiving_screen() {
    if run_fresh_cases(
        "game::native_hooks::base::screen_widgets_tests::separate_input_pre_post_parity_and_receiving_screen",
        "SUNRISE_WIDGETS_INPUT_POST_CHILD",
        &["window"],
    ) {
        return;
    }

    struct DragScrollWidget {
        bounds: [f32; 4],
        handle_drag: Arc<AtomicBool>,
        handle_scroll: Arc<AtomicBool>,
    }

    impl ScreenWidget for DragScrollWidget {
        fn bounds(&self) -> [f32; 4] {
            self.bounds
        }
        fn mouse_clicked(
            &mut self,
            _host: &mut dyn ClientHost,
            _mouse: [f32; 2],
            _button: i32,
            _modifiers: u32,
            _double_click: bool,
        ) -> bool {
            true
        }
        fn mouse_dragged(
            &mut self,
            _host: &mut dyn ClientHost,
            _mouse: [f32; 2],
            _button: i32,
            _delta: [f32; 2],
            _modifiers: u32,
        ) -> bool {
            self.handle_drag.load(Ordering::SeqCst)
        }
        fn mouse_released(
            &mut self,
            _host: &mut dyn ClientHost,
            _mouse: [f32; 2],
            _button: i32,
            _modifiers: u32,
        ) -> bool {
            true
        }
        fn mouse_scrolled(
            &mut self,
            _host: &mut dyn ClientHost,
            _mouse: [f32; 2],
            _scroll: [f64; 2],
            _modifiers: u32,
        ) -> bool {
            self.handle_scroll.load(Ordering::SeqCst)
        }
    }

    struct InputMod {
        log: Arc<Mutex<Vec<String>>>,
        override_press_handled: Arc<Mutex<Option<bool>>>,
        override_release_handled: Arc<Mutex<Option<bool>>>,
        cancel_key_post: Arc<AtomicBool>,
        handle_drag: Arc<AtomicBool>,
        handle_scroll: Arc<AtomicBool>,
    }

    #[mod_api::__subscriptions]
    impl Hooks for InputMod {
        fn screen_init_post(&self, host: &dyn ClientHost, event: &mut ScreenInitPostEvent<'_>) {
            match event.screen.id {
                "paused" => {
                    // Button on Paused that transitions to mod screen "target_mod".
                    event.widgets.add_button(Button::new(
                        [20.0, 20.0, 80.0, 20.0],
                        ApiText::literal("To Mod"),
                        |_state, host| {
                            host.mod_screen_open("target_mod");
                        },
                    ));
                }
                "mod" if host.mod_screen() == Some("target_mod") => {
                    // Button on target_mod that closes back to Playing.
                    event.widgets.add_button(Button::new(
                        [20.0, 20.0, 80.0, 20.0],
                        ApiText::literal("Close"),
                        |_state, host| {
                            host.close_screen();
                        },
                    ));
                    // Custom widget for drag/scroll handled vs unhandled checks.
                    event.widgets.add_widget(Box::new(DragScrollWidget {
                        bounds: [20.0, 60.0, 80.0, 20.0],
                        handle_drag: self.handle_drag.clone(),
                        handle_scroll: self.handle_scroll.clone(),
                    }));
                }
                _ => {}
            }
        }

        fn screen_mouse_button_pressed_pre(
            &self,
            _host: &dyn ClientHost,
            event: &mut ScreenMouseButtonPressedPreEvent<'_>,
        ) -> Flow {
            self.log.lock().unwrap().push(format!(
                "press_pre:{}:{}:{}",
                event.screen.id, event.button, event.double_click
            ));
            Flow::Continue
        }

        fn screen_mouse_button_pressed_post(
            &self,
            _host: &dyn ClientHost,
            event: &mut ScreenMouseButtonPressedPostEvent<'_>,
        ) {
            self.log.lock().unwrap().push(format!(
                "press_post:{}:{}:{}:{}",
                event.screen.id, event.button, event.double_click, event.handled
            ));
            if let Some(forced) = *self.override_press_handled.lock().unwrap() {
                event.handled = forced;
            }
        }

        fn screen_mouse_button_released_pre(
            &self,
            _host: &dyn ClientHost,
            event: &mut ScreenMouseButtonReleasedPreEvent<'_>,
        ) -> Flow {
            self.log
                .lock()
                .unwrap()
                .push(format!("release_pre:{}:{}", event.screen.id, event.button));
            Flow::Continue
        }

        fn screen_mouse_button_released_post(
            &self,
            _host: &dyn ClientHost,
            event: &mut ScreenMouseButtonReleasedPostEvent<'_>,
        ) {
            self.log.lock().unwrap().push(format!(
                "release_post:{}:{}:{}",
                event.screen.id, event.button, event.handled
            ));
            if let Some(forced) = *self.override_release_handled.lock().unwrap() {
                event.handled = forced;
            }
        }

        fn screen_mouse_dragged_pre(
            &self,
            _host: &dyn ClientHost,
            event: &mut ScreenMouseDraggedPreEvent<'_>,
        ) -> Flow {
            self.log
                .lock()
                .unwrap()
                .push(format!("drag_pre:{}", event.screen.id));
            Flow::Continue
        }

        fn screen_mouse_dragged_post(
            &self,
            _host: &dyn ClientHost,
            event: &mut ScreenMouseDraggedPostEvent<'_>,
        ) {
            self.log
                .lock()
                .unwrap()
                .push(format!("drag_post:{}", event.screen.id));
        }

        fn screen_mouse_scrolled_pre(
            &self,
            _host: &dyn ClientHost,
            event: &mut ScreenMouseScrolledPreEvent<'_>,
        ) -> Flow {
            self.log
                .lock()
                .unwrap()
                .push(format!("scroll_pre:{}", event.screen.id));
            Flow::Continue
        }

        fn screen_mouse_scrolled_post(
            &self,
            _host: &dyn ClientHost,
            event: &mut ScreenMouseScrolledPostEvent<'_>,
        ) {
            self.log
                .lock()
                .unwrap()
                .push(format!("scroll_post:{}", event.screen.id));
        }

        fn screen_key_pressed_pre(
            &self,
            _host: &dyn ClientHost,
            event: &mut ScreenKeyPressedPreEvent<'_>,
        ) -> Flow {
            self.log
                .lock()
                .unwrap()
                .push(format!("key_press_pre:{}:{}", event.screen.id, event.key));
            Flow::Continue
        }

        fn screen_key_pressed_post(
            &self,
            _host: &dyn ClientHost,
            event: &mut ScreenKeyPressedPostEvent<'_>,
        ) -> Flow {
            self.log
                .lock()
                .unwrap()
                .push(format!("key_press_post:{}:{}", event.screen.id, event.key));
            if self.cancel_key_post.load(Ordering::SeqCst) {
                Flow::Cancel
            } else {
                Flow::Continue
            }
        }

        fn screen_key_released_pre(
            &self,
            _host: &dyn ClientHost,
            event: &mut ScreenKeyReleasedPreEvent<'_>,
        ) -> Flow {
            self.log
                .lock()
                .unwrap()
                .push(format!("key_rel_pre:{}:{}", event.screen.id, event.key));
            Flow::Continue
        }

        fn screen_key_released_post(
            &self,
            _host: &dyn ClientHost,
            event: &mut ScreenKeyReleasedPostEvent<'_>,
        ) -> Flow {
            self.log
                .lock()
                .unwrap()
                .push(format!("key_rel_post:{}:{}", event.screen.id, event.key));
            if self.cancel_key_post.load(Ordering::SeqCst) {
                Flow::Cancel
            } else {
                Flow::Continue
            }
        }

        fn screen_char_typed_pre(
            &self,
            _host: &dyn ClientHost,
            event: &mut ScreenCharTypedPreEvent<'_>,
        ) -> Flow {
            self.log
                .lock()
                .unwrap()
                .push(format!("char_pre:{}:{}", event.screen.id, event.c));
            Flow::Continue
        }

        fn screen_char_typed_post(
            &self,
            _host: &dyn ClientHost,
            event: &mut ScreenCharTypedPostEvent<'_>,
        ) {
            self.log
                .lock()
                .unwrap()
                .push(format!("char_post:{}:{}", event.screen.id, event.c));
        }

        fn screen_preedit_pre(
            &self,
            _host: &dyn ClientHost,
            event: &mut ScreenPreeditPreEvent<'_>,
        ) -> Flow {
            self.log.lock().unwrap().push(format!(
                "preedit_pre:{}:{}",
                event.screen.id, event.full_text
            ));
            Flow::Continue
        }

        fn screen_preedit_post(
            &self,
            _host: &dyn ClientHost,
            event: &mut ScreenPreeditPostEvent<'_>,
        ) {
            self.log.lock().unwrap().push(format!(
                "preedit_post:{}:{}",
                event.screen.id, event.full_text
            ));
        }

        fn screen_opening(
            &self,
            _host: &dyn ClientHost,
            _event: &mut ScreenOpeningEvent<'_>,
        ) -> Option<ScreenOpening> {
            None
        }

        fn screen_closing(&self, _host: &dyn ClientHost, _event: &mut ScreenClosingEvent<'_>) {}
    }

    let log = Arc::new(Mutex::new(Vec::new()));
    let override_press_handled = Arc::new(Mutex::new(None));
    let override_release_handled = Arc::new(Mutex::new(None));
    let cancel_key_post = Arc::new(AtomicBool::new(false));
    let handle_drag = Arc::new(AtomicBool::new(true));
    let handle_scroll = Arc::new(AtomicBool::new(true));
    let mods: Arc<[ModEntry]> = vec![ModEntry::new(
        "target_mod",
        Box::new(InputMod {
            log: log.clone(),
            override_press_handled: override_press_handled.clone(),
            override_release_handled: override_release_handled.clone(),
            cancel_key_post: cancel_key_post.clone(),
            handle_drag: handle_drag.clone(),
            handle_scroll: handle_scroll.clone(),
        }),
    )]
    .into();

    with_mods(mods, || {
        with_client_game(|game| {
            let (_event_loop, win_id) = attach_offscreen_window(game);

            // 1) Open Paused and click the button that transitions paused -> mod ("target_mod"):
            //    MouseButtonPressed.Post MUST still carry screen A's id ("paused"), not "mod".
            game.set_screen(Screen::Paused);
            game.observe_screen();
            take_log(&log);
            move_mouse(game, win_id, 40.0, 30.0);
            send_mouse_button(game, win_id, MouseButton::Left, true);
            assert_eq!(game.screen, Screen::Mod);
            assert_eq!(game.mod_screen(), Some("target_mod"));
            assert_eq!(
                take_log(&log),
                vec!["press_pre:paused:0:false", "press_post:paused:0:false:true"]
            );
            // Release now happens on "mod".
            send_mouse_button(game, win_id, MouseButton::Left, false);
            assert_eq!(
                take_log(&log),
                vec!["release_pre:mod:0", "release_post:mod:0:false"]
            );

            // 2) Override event.handled = false on a handled click suppresses last_click recording,
            //    so an immediate second click does NOT see double_click = true.
            *override_press_handled.lock().unwrap() = Some(false);
            move_mouse(game, win_id, 40.0, 70.0);
            send_mouse_button(game, win_id, MouseButton::Left, true);
            send_mouse_button(game, win_id, MouseButton::Left, false);
            assert!(game.screen_state.last_click.is_none());
            take_log(&log);
            // Next click immediately after sees double_click = false.
            *override_press_handled.lock().unwrap() = None;
            send_mouse_button(game, win_id, MouseButton::Left, true);
            send_mouse_button(game, win_id, MouseButton::Left, false);
            let entries = take_log(&log);
            assert_eq!(
                entries,
                vec![
                    "press_pre:mod:0:false",
                    "press_post:mod:0:false:true",
                    "release_pre:mod:0",
                    "release_post:mod:0:true",
                ]
            );
            assert!(game.screen_state.last_click.is_some());

            // 3) Override event.handled = true on an UNHANDLED click records last_click, so an
            //    immediate second click receives double_click = true on both Pre and Post.
            game.screen_state.last_click = None;
            *override_press_handled.lock().unwrap() = Some(true);
            move_mouse(game, win_id, 400.0, 400.0);
            send_mouse_button(game, win_id, MouseButton::Left, true);
            send_mouse_button(game, win_id, MouseButton::Left, false);
            assert!(game.screen_state.last_click.is_some());
            take_log(&log);
            *override_press_handled.lock().unwrap() = None;
            send_mouse_button(game, win_id, MouseButton::Left, true);
            send_mouse_button(game, win_id, MouseButton::Left, false);
            assert_eq!(
                take_log(&log),
                vec![
                    "press_pre:mod:0:true",
                    "press_post:mod:0:true:false",
                    "release_pre:mod:0",
                    "release_post:mod:0:false",
                ]
            );

            // 4) Drag & Scroll Post fire ONLY when unhandled.
            //    Handled drag/scroll -> Pre fires, Post does NOT fire.
            handle_drag.store(true, Ordering::SeqCst);
            handle_scroll.store(true, Ordering::SeqCst);
            move_mouse(game, win_id, 40.0, 70.0);
            send_mouse_button(game, win_id, MouseButton::Left, true);
            take_log(&log);
            move_mouse(game, win_id, 55.0, 70.0);
            send_scroll(game, win_id, 0.0, 1.0);
            assert_eq!(take_log(&log), vec!["drag_pre:mod", "scroll_pre:mod"]);
            send_mouse_button(game, win_id, MouseButton::Left, false);

            //    Unhandled drag/scroll -> Pre AND Post both fire on "mod".
            handle_drag.store(false, Ordering::SeqCst);
            handle_scroll.store(false, Ordering::SeqCst);
            move_mouse(game, win_id, 40.0, 70.0);
            send_mouse_button(game, win_id, MouseButton::Left, true);
            take_log(&log);
            move_mouse(game, win_id, 50.0, 70.0);
            send_scroll(game, win_id, 0.0, -1.0);
            assert_eq!(
                take_log(&log),
                vec![
                    "drag_pre:mod",
                    "drag_post:mod",
                    "scroll_pre:mod",
                    "scroll_post:mod"
                ]
            );
            send_mouse_button(game, win_id, MouseButton::Left, false);

            // 5) CharacterTyped.Post and Preedit.Post fire when unhandled on "mod".
            take_log(&log);
            game.screen_chars_typed(Some("z"));
            game.ime_preedit("hi", Some((0, 2)));
            assert_eq!(
                take_log(&log),
                vec![
                    "char_pre:mod:z",
                    "char_post:mod:z",
                    "preedit_pre:mod:hi",
                    "preedit_post:mod:hi",
                ]
            );

            // 6) KeyPressed.Post & KeyReleased.Post: returning Flow::Cancel blocks mapped_key_event.
            //    First with cancel_key_post = true: pressing W does NOT mark W down in game.keys.
            game.keys.clear();
            cancel_key_post.store(true, Ordering::SeqCst);
            take_log(&log);
            send_key_control(game, "key w down");
            assert!(!game.keys.raw(&KeyCode::KeyW));
            assert_eq!(
                take_log(&log),
                vec![
                    "key_press_pre:mod:key.keyboard.w",
                    "key_press_post:mod:key.keyboard.w",
                ]
            );
            //    Now with cancel_key_post = false: pressing W reaches mapped_key_event and sets W down.
            cancel_key_post.store(false, Ordering::SeqCst);
            send_key_control(game, "key w down");
            assert!(game.keys.raw(&KeyCode::KeyW));
            //    Releasing through the control socket (`key w up`) runs KeyReleased.Pre and
            //    KeyReleased.Post on the receiving screen; cancelling Post blocks mapped_key_event.
            take_log(&log);
            cancel_key_post.store(true, Ordering::SeqCst);
            send_key_control(game, "key w up");
            assert!(game.keys.raw(&KeyCode::KeyW));
            assert_eq!(
                take_log(&log),
                vec![
                    "key_rel_pre:mod:key.keyboard.w",
                    "key_rel_post:mod:key.keyboard.w",
                ]
            );
            cancel_key_post.store(false, Ordering::SeqCst);
            send_key_control(game, "key w up");
            assert!(!game.keys.raw(&KeyCode::KeyW));
            assert_eq!(
                take_log(&log),
                vec![
                    "key_rel_pre:mod:key.keyboard.w",
                    "key_rel_post:mod:key.keyboard.w",
                ]
            );
            //    `key w tap` runs both press and release phases through Pre and Post.
            take_log(&log);
            send_key_control(game, "key w tap");
            assert!(!game.keys.raw(&KeyCode::KeyW));
            assert_eq!(
                take_log(&log),
                vec![
                    "key_press_pre:mod:key.keyboard.w",
                    "key_press_post:mod:key.keyboard.w",
                    "key_rel_pre:mod:key.keyboard.w",
                    "key_rel_post:mod:key.keyboard.w",
                ]
            );

            // 7) Closing screen from button press still fires MouseButtonPressed.Post with "mod"
            //    even though the screen is now Playing.
            move_mouse(game, win_id, 40.0, 30.0);
            take_log(&log);
            send_mouse_button(game, win_id, MouseButton::Left, true);
            assert_eq!(game.screen, Screen::Playing);
            assert_eq!(
                take_log(&log),
                vec!["press_pre:mod:0:false", "press_post:mod:0:false:true"]
            );

            // 8) Advancements blank-space press/release and no-tab scroll report unhandled.
            game.set_screen(Screen::Advancements);
            take_log(&log);
            move_mouse(game, win_id, 5.0, 5.0);
            send_mouse_button(game, win_id, MouseButton::Left, true);
            send_mouse_button(game, win_id, MouseButton::Left, false);
            send_scroll(game, win_id, 0.0, 1.0);
            assert_eq!(
                take_log(&log),
                vec![
                    "press_pre:advancements:0:false",
                    "press_post:advancements:0:false:false",
                    "release_pre:advancements:0",
                    "release_post:advancements:0:false",
                    "scroll_pre:advancements",
                    "scroll_post:advancements",
                ]
            );
        });
    });
}
