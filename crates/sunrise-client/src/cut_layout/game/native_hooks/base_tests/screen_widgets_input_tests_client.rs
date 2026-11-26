use super::*;
use crate::game::native_hooks::base::screen_widgets_tests::input_routing::Cancel;
use crate::game::native_hooks::base::screen_widgets_tests::input_routing::Seen;
use crate::game::native_hooks::base::screen_widgets_tests::input_routing::key;
use crate::game::native_hooks::base::screen_widgets_tests::input_routing::send;
use crate::game::native_hooks::base::screen_widgets_tests::input_routing::take;
use crate::game::native_hooks::base::screen_widgets_tests::input_routing::with_input_game;
use crate::game::native_hooks::base::screen_widgets_tests::input_routing::with_input_game_as;
use sunrise_client::{keybinds::VanillaMappingSlot, preedit, widgets};
use sunrise_gameplay_common::game::Screen;
use sunrise_items::items::ItemStack;
use winit::{dpi::PhysicalPosition, event::Ime};

#[test]
#[ignore = "daily"]
fn inventory_release_post_distinguishes_quick_craft_from_swallowed_release() {
    if run_fresh_cases(
        "game::client_side::native_hooks_client::base::screen_widgets_tests::input_routing::inventory_release_post_distinguishes_quick_craft_from_swallowed_release",
        "SUNRISE_SCREEN_QUICK_CRAFT_CHILD",
        &["inventory"],
    ) {
        return;
    }
    with_input_game(|game, id, state| {
        use sunrise_level::{chunk::Chunk, containers::BlockEntity};
        let pos = (0, 0, 0);
        let saved_player = game.player.take();
        let _pipe =
            sunrise_client_gameplay::game::client_side::native_client_session::tests::logged_client(
                game,
            );
        game.player = saved_player;
        game.world
            .chunks
            .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
        let chest = game
            .pack
            .assets
            .registry
            .default_state("minecraft:chest")
            .unwrap();
        game.world.set(pos.0, pos.1, pos.2, chest).unwrap();
        game.world
            .block_entities
            .insert(pos, BlockEntity::Chest(vec![None; 27]));
        game.client
            .remote
            .as_mut()
            .unwrap()
            .menu
            .set(Some((1, "minecraft:generic_9x3".to_owned())));
        game.container = Some(pos);
        game.open_player_inventory();
        game.carried = Some(ItemStack::new("minecraft:stick", 8));
        let batch = game.gui_batch().unwrap();
        let layout = game.built_container(&batch).unwrap();
        let point = |slot| {
            let &(_, [x, y, ..]) = layout.slots.iter().find(|(i, _)| *i == slot).unwrap();
            (f64::from(x + 5.0), f64::from(y + 5.0))
        };
        let first = point(300);
        let second = point(301);
        let third = point(302);
        move_mouse(game, id, first.0, first.1);
        send_mouse_button(game, id, MouseButton::Left, true);
        move_mouse(game, id, second.0, second.1);
        take(state);
        send_mouse_button(game, id, MouseButton::Left, false);
        assert_eq!(take(state), [Seen::ReleasePost("inventory".into(), false)]);
        assert!(game.carried.is_none());
        assert_eq!(game.slot_view(300).unwrap().count, 4);
        assert_eq!(game.slot_view(301).unwrap().count, 4);

        game.carried = Some(ItemStack::new("minecraft:stick", 6));
        // Start on a fresh slot so a click within 250ms does not trigger vanilla PickupAll.
        move_mouse(game, id, third.0, third.1);
        send_mouse_button(game, id, MouseButton::Left, true);
        assert!(game.client.inv_drag.is_some());
        take(state);
        send_mouse_button(game, id, MouseButton::Right, false);
        assert_eq!(take(state), [Seen::ReleasePost("inventory".into(), true)]);
        assert!(game.client.inv_drag.is_none());
        assert_eq!(game.carried.as_ref().unwrap().count, 6);
        send_mouse_button(game, id, MouseButton::Left, false);
        assert_eq!(take(state), [Seen::ReleasePost("inventory".into(), true)]);
    });
}

#[test]
#[ignore = "daily"]
fn native_and_mounted_press_tab_and_preedit_use_the_receiving_screen() {
    if run_fresh_cases(
        "game::client_side::native_hooks_client::base::screen_widgets_tests::input_routing::native_and_mounted_press_tab_and_preedit_use_the_receiving_screen",
        "SUNRISE_SCREEN_INPUT_ROUTING_CHILD",
        &["routing"],
    ) {
        return;
    }
    with_input_game(|game, id, state| {
        game.client.pause_menu = true;
        game.set_screen(Screen::Paused);
        publish_frame(game, 1);
        assert_eq!(game.mounted_widget_targets().len(), 1);
        let (w, h) = game.gui_size();
        let (x, y, _, _, _, _) = game.pause_layout(w, h).buttons[0];
        move_mouse(game, id, f64::from(x + 10.0), f64::from(y + 10.0));
        send_mouse_button(game, id, MouseButton::Left, true);
        assert_eq!(game.client.screen, Screen::Playing);
        assert_eq!(
            take(state),
            [Seen::PressPre, Seen::PressPost("paused".into(), true)]
        );
        send_mouse_button(game, id, MouseButton::Left, false);

        assert!(game.mod_screen_open("input_route"));
        let focused = game.client.screen_state.0.widgets.focused_id().unwrap();
        move_mouse(game, id, 24.0, 30.0);
        send_mouse_button(game, id, MouseButton::Left, true);
        assert_eq!(
            take(state),
            [
                Seen::PressPre,
                Seen::Activation("decline"),
                Seen::PressPost("mod".into(), true)
            ]
        );
        assert_eq!(
            game.client.screen_state.0.widgets.focused_id(),
            Some(focused)
        );
        send_mouse_button(game, id, MouseButton::Left, false);
        take(state);
        move_mouse(game, id, 300.0, 100.0);
        send_mouse_button(game, id, MouseButton::Left, true);
        assert_eq!(
            take(state),
            [Seen::PressPre, Seen::PressPost("mod".into(), false)]
        );
        assert_eq!(
            game.client.screen_state.0.widgets.focused_id(),
            Some(focused)
        );
        send_mouse_button(game, id, MouseButton::Left, false);
        game.mod_screen_close("input_route");
        assert_eq!(game.client.screen, Screen::Playing);
        take(state);

        let dialog = sunrise_command::snbt::parse_fully(
            r#"{type:"minecraft:notice",title:"Input",inputs:[{type:"minecraft:text",key:"name",label:"Name"}]}"#,
        ).unwrap();
        game.show_dialog(&dialog);
        assert!(game.dialog_open());
        publish_frame(game, 2);
        let tab = |game: &mut sunrise_client_gameplay::game::ClientGame<'_>| {
            key(game, KeyCode::Tab, ElementState::Pressed)
        };
        tab(game); // Warning, tab group -10.
        tab(game); // Native field.
        assert!(game.dialog_typing());
        assert!(!game.client.screen_state.0.widgets.has_focus());
        tab(game); // Native notice button.
        tab(game); // Appended mod button.
        assert!(!game.dialog_typing());
        assert!(game.client.screen_state.0.widgets.has_focus());
        key(game, KeyCode::Enter, ElementState::Pressed);
        assert_eq!(take(state), [Seen::Activation("after_dialog")]);
        game.clear_dialog();

        game.begin_chat_screen("native draft", false);
        send(
            game,
            id,
            WindowEvent::Ime(Ime::Preedit("ni".into(), Some((0, 2)))),
        );
        assert!(preedit::should_suppress_key());
        assert!(take(state).is_empty());
        assert!(game.open_screen_overlay("link_prompt", |game| {
            game.client.link_prompt = Some("https://example.com".into());
        }));
        assert!(game.mounted_widget_targets().is_empty());
        send(
            game,
            id,
            WindowEvent::Ime(Ime::Preedit("ka".into(), Some((0, 2)))),
        );
        assert_eq!(
            take(state),
            [Seen::PreeditPost("link_prompt".into(), "ka".into())]
        );
        assert!(!preedit::should_suppress_key());
        assert_eq!(
            game.client.chat_ui.chat_screen.as_ref().unwrap().value(),
            "native draft"
        );
    });
}

pub(crate) fn publish_frame(game: &mut sunrise_client_gameplay::game::ClientGame<'_>, frame: u64) {
    let renderer = game.client.renderer.0.as_ref().unwrap();
    let size = [renderer.config.width, renderer.config.height];
    game.hud(size);
    widgets::set_action_screen(game.key_screen());
    widgets::end_frame(frame);
}

#[test]
#[ignore = "daily"]
fn release_drag_and_wheel_cancellation_preserve_input_bookkeeping() {
    if run_fresh_cases(
        "game::client_side::native_hooks_client::base::screen_widgets_tests::input_routing::release_drag_and_wheel_cancellation_preserve_input_bookkeeping",
        "SUNRISE_SCREEN_PHYSICAL_INPUT_CHILD",
        &["input"],
    ) {
        return;
    }
    with_input_game(|game, id, state| {
        assert!(game.mod_screen_open("input_route"));
        let cancel = |cancel| state.lock().unwrap().cancel = cancel;
        let forward = VanillaMappingSlot::by_default_key(KeyCode::KeyW).unwrap();
        let jump = VanillaMappingSlot::by_default_key(KeyCode::Space).unwrap();
        game.client.settings.keys.set("key.jump", "key.keyboard.w");
        game.insert_device_key(KeyCode::KeyW);
        game.insert_device_key(KeyCode::Space);
        assert!(
            game.client.vanilla_mappings.is_down(forward)
                && game.client.vanilla_mappings.is_down(jump)
        );
        cancel(Cancel::ReleasePre);
        key(game, KeyCode::KeyW, ElementState::Released);
        assert_eq!(take(state), [Seen::KeyPre("key.keyboard.w".into())]);
        assert!(!game.client.keys.raw(&KeyCode::KeyW) && !game.client.keys.raw(&KeyCode::Space));
        assert!(
            !game.client.vanilla_mappings.is_down(forward)
                && !game.client.vanilla_mappings.is_down(jump)
        );

        game.insert_device_key(KeyCode::KeyW);
        send_key_control(game, "key w up");
        assert_eq!(take(state), [Seen::KeyPre("key.keyboard.w".into())]);
        assert!(game.client.keys.raw(&KeyCode::KeyW));
        cancel(Cancel::ReleasePost);
        key(game, KeyCode::KeyW, ElementState::Released);
        assert_eq!(
            take(state),
            [
                Seen::KeyPre("key.keyboard.w".into()),
                Seen::KeyPost("key.keyboard.w".into())
            ]
        );
        assert!(game.client.keys.raw(&KeyCode::KeyW));
        cancel(Cancel::None);
        send_key_control(game, "key w up");
        assert!(!game.client.keys.raw(&KeyCode::KeyW));
        take(state);

        move_mouse(game, id, 24.0, 30.0);
        cancel(Cancel::Press);
        send_mouse_button(game, id, MouseButton::Left, true);
        assert_eq!(take(state), [Seen::PressPre]);
        assert!(game.client.physical_buttons.contains(&MouseButton::Left));
        send_mouse_button(game, id, MouseButton::Left, false);
        assert!(!game.client.physical_buttons.contains(&MouseButton::Left));
        take(state);

        move_mouse(game, id, 300.0, 100.0);
        cancel(Cancel::None);
        send_mouse_button(game, id, MouseButton::Left, true);
        send_mouse_button(game, id, MouseButton::Right, true);
        take(state);
        move_mouse(game, id, 310.0, 120.0);
        assert_eq!(
            take(state),
            [
                Seen::DragPre(1, [10.0, 20.0], [310.0, 120.0]),
                Seen::DragPost
            ]
        );
        cancel(Cancel::Drag);
        move_mouse(game, id, 312.0, 123.0);
        assert_eq!(take(state), [Seen::DragPre(1, [2.0, 3.0], [312.0, 123.0])]);
        assert_eq!(game.cursor(), [312.0, 123.0]);
        send_mouse_button(game, id, MouseButton::Left, false);
        take(state);
        move_mouse(game, id, 314.0, 126.0);
        assert!(take(state).is_empty());
        assert!(game.client.physical_buttons.contains(&MouseButton::Right));
        send_mouse_button(game, id, MouseButton::Left, true);
        game.client.physical_keys.insert(KeyCode::ShiftLeft);
        game.client.modifiers = winit::keyboard::ModifiersState::SHIFT;
        take(state);
        send(game, id, WindowEvent::Focused(false));
        move_mouse(game, id, 316.0, 128.0);
        assert!(take(state).is_empty());
        assert!(game.client.physical_buttons.is_empty() && game.client.physical_keys.is_empty());
        assert!(game.client.modifiers.is_empty() && game.client.drag_button.is_none());
        send(game, id, WindowEvent::Focused(true));
        move_mouse(game, id, 24.0, 30.0);

        cancel(Cancel::ScreenWheel);
        send_scroll(game, id, 0.25, -0.75);
        assert_eq!(take(state), [Seen::ScreenWheelPre([0.25, -0.75])]);
        cancel(Cancel::None);
        send_scroll(game, id, 0.25, -0.75);
        assert_eq!(
            take(state),
            [
                Seen::ScreenWheelPre([0.25, -0.75]),
                Seen::ScreenWheelChild([0.25, -0.75]),
                Seen::ScreenWheelPost([0.25, -0.75])
            ]
        );
        game.client.pad.0.menu.synthetic = true;
        send_scroll(game, id, 0.25, -0.75);
        game.client.pad.0.menu.synthetic = false;
        assert!(take(state).is_empty());
        game.mod_screen_close("input_route");
        assert_eq!(game.client.screen, Screen::Playing);
        game.selected = 4;
        game.client.settings.mouse_wheel_sensitivity = 1.0;
        game.client.settings.discrete_scroll = false;
        move_mouse(game, id, 100.0, 110.0);
        send_scroll(game, id, 0.25, 0.0);
        assert_eq!(game.selected, 4);
        assert_eq!(
            take(state),
            [Seen::GameWheel(
                [0.25, 0.0],
                [0, 0],
                [false; 3],
                [100.0, 110.0],
                0
            )]
        );
        send_scroll(game, id, 0.75, 0.0);
        assert_eq!(game.selected, 5);
        assert_eq!(
            take(state),
            [Seen::GameWheel(
                [0.75, 0.0],
                [1, 0],
                [false; 3],
                [100.0, 110.0],
                0
            )]
        );
        cancel(Cancel::GameWheel);
        send_scroll(game, id, 0.0, 1.0);
        assert_eq!(game.selected, 5);
        assert_eq!(
            take(state),
            [Seen::GameWheel(
                [0.0, 1.0],
                [0, 1],
                [false; 3],
                [100.0, 110.0],
                0
            )]
        );
        cancel(Cancel::None);
        send_scroll(game, id, 0.0, 0.25);
        assert_eq!(game.selected, 5);
        send_scroll(game, id, 0.0, 0.75);
        assert_eq!(game.selected, 4);
        assert_eq!(
            take(state),
            [
                Seen::GameWheel([0.0, 0.25], [0, 0], [false; 3], [100.0, 110.0], 0),
                Seen::GameWheel([0.0, 0.75], [0, 1], [false; 3], [100.0, 110.0], 0),
            ]
        );
        game.client.settings.mouse_wheel_sensitivity = 2.0;
        let pixel_wheel = |game: &mut sunrise_client_gameplay::game::ClientGame<'_>| {
            send(
                game,
                id,
                WindowEvent::MouseWheel {
                    device_id: DeviceId::dummy(),
                    delta: MouseScrollDelta::PixelDelta(PhysicalPosition::new(60.0, -30.0)),
                    phase: TouchPhase::Moved,
                },
            );
        };
        pixel_wheel(game);
        assert_eq!(
            take(state),
            [Seen::GameWheel(
                [4.0, -2.0],
                [4, -2],
                [false; 3],
                [100.0, 110.0],
                0
            )]
        );
        game.client.settings.discrete_scroll = true;
        pixel_wheel(game);
        assert_eq!(
            take(state),
            [Seen::GameWheel(
                [2.0, -2.0],
                [2, -2],
                [false; 3],
                [100.0, 110.0],
                0
            )]
        );

        let selected = game.selected;
        let player = game.player.take();
        send_scroll(game, id, 0.0, 1.0);
        game.player = player;
        game.sleeping = Some([0, 64, 0]);
        send_scroll(game, id, 0.0, 1.0);
        game.sleeping = None;
        assert_eq!(game.selected, selected);
        // A skipped gameplay wheel can belong to a screen, but never the hotbar hook.
        assert!(
            take(state)
                .iter()
                .all(|event| !matches!(event, Seen::GameWheel(..)))
        );
    });
}

#[test]
#[ignore = "daily"]
fn hover_moves_fire_before_drag_and_a_screen_grab_ends_with_the_screen() {
    if run_fresh_cases(
        concat!(
            module_path!(),
            "::hover_moves_fire_before_drag_and_a_screen_grab_ends_with_the_screen"
        )
        .split_once("::")
        .unwrap()
        .1,
        "SUNRISE_SCREEN_POINTER_GRAB_CHILD",
        &["grab"],
    ) {
        return;
    }
    with_input_game_as("input_route", |game, id, state| {
        state.lock().unwrap().record_pointer = true;
        // No screen: nothing fires.
        move_mouse(game, id, 20.0, 20.0);
        assert!(take(state).is_empty());
        assert!(game.mod_screen_open("input_route"));

        // A hover move, then a held-button move: the moved hook comes first both times.
        move_mouse(game, id, 300.0, 100.0);
        assert_eq!(take(state), [Seen::Moved([280.0, 80.0], [300.0, 100.0])]);
        move_mouse(game, id, 300.0, 100.0);
        assert!(take(state).is_empty(), "a zero delta fires nothing");
        send_mouse_button(game, id, MouseButton::Left, true);
        take(state);
        move_mouse(game, id, 310.0, 120.0);
        assert_eq!(
            take(state),
            [
                Seen::Moved([10.0, 20.0], [310.0, 120.0]),
                Seen::DragPre(0, [10.0, 20.0], [310.0, 120.0]),
                Seen::DragPost
            ]
        );
        send_mouse_button(game, id, MouseButton::Left, false);
        take(state);

        // Cancel skips the drag too, and the cursor still moves.
        state.lock().unwrap().cancel = Cancel::Move;
        send_mouse_button(game, id, MouseButton::Left, true);
        take(state);
        move_mouse(game, id, 312.0, 123.0);
        assert_eq!(take(state), [Seen::Moved([2.0, 3.0], [312.0, 123.0])]);
        assert_eq!(game.cursor(), [312.0, 123.0]);
        send_mouse_button(game, id, MouseButton::Left, false);
        state.lock().unwrap().cancel = Cancel::None;
        take(state);

        // The pad's pointer is not the mouse.
        game.client.pad.0.menu.synthetic = true;
        move_mouse(game, id, 330.0, 130.0);
        game.client.pad.0.menu.synthetic = false;
        assert!(take(state).is_empty());

        // The grab: raw motion goes to the screen, not the camera or the moved hook.
        assert!(game.test_set_pointer_grab(true));
        assert!(game.test_pointer_grabbed(), "reported at once");
        game.test_apply_pointer_grab_request();
        assert!(
            game.client.captured
                && game.test_mod_screen_grabs_pointer()
                && game.test_pointer_grabbed()
        );
        let turn = game.test_pointer_accumulated();
        game.test_grabbed_pointer_motion([3.5, -2.0]);
        game.test_grabbed_pointer_motion([0.0, 0.0]);
        assert_eq!(
            take(state),
            [Seen::Motion("input_route".into(), [3.5, -2.0])]
        );
        assert_eq!(
            game.test_pointer_accumulated(),
            turn,
            "the camera does not turn"
        );
        let held = game.cursor();
        move_mouse(game, id, 340.0, 140.0);
        send_mouse_button(game, id, MouseButton::Left, true);
        take(state);
        move_mouse(game, id, 350.0, 150.0);
        send_mouse_button(game, id, MouseButton::Left, false);
        assert!(
            !take(state)
                .iter()
                .any(|seen| matches!(seen, Seen::Moved(..) | Seen::DragPre(..))),
            "the grab's cursor moves are neither hover nor drag"
        );
        assert_eq!(game.cursor(), held);
        // Focus loss releases it, and refocus does not take it back on a screen.
        send(game, id, WindowEvent::Focused(false));
        assert!(!game.client.captured && !game.test_screen_grab() && !game.test_pointer_grabbed());
        send(game, id, WindowEvent::Focused(true));
        assert!(!game.client.captured && !game.test_pointer_grabbed());
        // The mod's own call, both ways.
        assert!(game.test_set_pointer_grab(true));
        game.test_apply_pointer_grab_request();
        assert!(game.test_pointer_grabbed());
        assert!(game.test_set_pointer_grab(false));
        assert!(!game.test_pointer_grabbed(), "reported at once");
        game.test_apply_pointer_grab_request();
        assert!(!game.client.captured && !game.test_screen_grab());
        game.test_screen_pointer_motion([1.0, 1.0]);
        assert!(take(state).is_empty(), "motion is the grab's");
        // Switching the mod off releases it at the next event.
        assert!(game.test_set_pointer_grab(true));
        game.test_apply_pointer_grab_request();
        assert!(game.test_screen_grab());
        assert!(mod_api::__private::set_mod_enabled("input_route", false));
        game.test_apply_pointer_grab_request();
        assert!(!game.client.captured && !game.test_screen_grab());
        assert!(mod_api::__private::set_mod_enabled("input_route", true));
        // Closing the screen releases it.
        assert!(game.test_set_pointer_grab(true));
        game.test_apply_pointer_grab_request();
        game.mod_screen_close("input_route");
        assert_eq!(game.client.screen, Screen::Playing);
        assert!(!game.test_screen_grab() && !game.test_pointer_grabbed());
        assert!(!game.test_set_pointer_grab(true), "no screen is open");
    });
}

#[cfg(any(unix, windows))]
#[test]
#[ignore = "daily"]
fn dropped_files_reach_a_mod_screen_whole_and_in_order() {
    use std::path::PathBuf;
    if run_fresh_cases(
        concat!(
            module_path!(),
            "::dropped_files_reach_a_mod_screen_whole_and_in_order"
        )
        .split_once("::")
        .unwrap()
        .1,
        "SUNRISE_SCREEN_FILES_DROPPED_CHILD",
        &["drop"],
    ) {
        return;
    }
    with_input_game_as("input_route", |game, id, state| {
        let first = PathBuf::from("/tmp/a.png");
        let second = not_utf8_path();
        // No screen open: the drop fires nothing.
        send(game, id, WindowEvent::DroppedFile(first.clone()));
        game.idle(&NoLoop);
        assert!(take(state).is_empty());

        assert!(game.mod_screen_open("input_route"));
        send(game, id, WindowEvent::DroppedFile(first.clone()));
        send(game, id, WindowEvent::DroppedFile(second.clone()));
        assert!(
            take(state).is_empty(),
            "the list is fired when the pass ends"
        );
        game.idle(&NoLoop);
        let seen = take(state);
        assert_eq!(seen, [Seen::Dropped("mod".into(), vec![first, second])]);
        let Seen::Dropped(_, paths) = &seen[0] else {
            unreachable!()
        };
        assert_eq!(
            paths[1].to_str(),
            None,
            "the second path is still not UTF-8"
        );
        assert!(
            game.mod_screen_open("input_route"),
            "observe only: the screen stays"
        );
        assert_eq!(game.mod_screen(), Some("input_route"));
        game.idle(&NoLoop);
        assert!(take(state).is_empty(), "a drop fires once");
    });
}

#[cfg(any(unix, windows))]
fn not_utf8_path() -> std::path::PathBuf {
    #[cfg(unix)]
    let path = {
        use std::os::unix::ffi::OsStrExt;
        std::ffi::OsStr::from_bytes(b"/tmp/a\xff.png").to_owned()
    };
    #[cfg(windows)]
    let path = {
        use std::os::windows::ffi::OsStringExt;
        let mut units: Vec<u16> = "/tmp/a".encode_utf16().collect();
        units.push(0xd800);
        units.extend(".png".encode_utf16());
        std::ffi::OsString::from_wide(&units)
    };
    path.into()
}

#[test]
#[ignore = "daily"]
fn a_drop_reaches_its_screen_before_a_click_that_closes_it() {
    if run_fresh_cases(
        concat!(
            module_path!(),
            "::a_drop_reaches_its_screen_before_a_click_that_closes_it"
        )
        .split_once("::")
        .unwrap()
        .1,
        "SUNRISE_SCREEN_FILES_DROPPED_CLOSE_CHILD",
        &["drop"],
    ) {
        return;
    }
    with_input_game_as("input_route", |game, id, state| {
        game.client.pause_menu = true;
        game.set_screen(Screen::Paused);
        publish_frame(game, 1);
        let (w, h) = game.gui_size();
        let (x, y, _, _, _, _) = game.pause_layout(w, h).buttons[0];
        move_mouse(game, id, f64::from(x + 10.0), f64::from(y + 10.0));
        take(state);
        let files = vec![
            std::path::PathBuf::from("pack.zip"),
            std::path::PathBuf::from("shot.png"),
        ];
        for file in &files {
            send(game, id, WindowEvent::DroppedFile(file.clone()));
        }
        // Back to Game: the click closes the pause menu in this pass.
        send_mouse_button(game, id, MouseButton::Left, true);
        assert_eq!(game.client.screen, Screen::Playing);
        assert_eq!(
            take(state),
            [
                Seen::Dropped("paused".into(), files),
                Seen::PressPre,
                Seen::PressPost("paused".into(), true)
            ]
        );
        game.idle(&NoLoop);
        assert!(take(state).is_empty(), "a drop fires once");
    });
}

#[test]
#[ignore = "daily"]
fn a_pointer_grab_is_refused_on_another_mods_screen() {
    if run_fresh_cases(
        concat!(
            module_path!(),
            "::a_pointer_grab_is_refused_on_another_mods_screen"
        )
        .split_once("::")
        .unwrap()
        .1,
        "SUNRISE_SCREEN_GRAB_OTHER_MOD_CHILD",
        &["grab"],
    ) {
        return;
    }
    with_input_game_as("someone_else", |game, _, _| {
        assert!(game.mod_screen_open("input_route"));
        assert!(!game.test_set_pointer_grab(true));
        assert!(!game.test_pointer_grabbed());
        game.test_apply_pointer_grab_request();
        assert!(
            !game.test_screen_grab()
                && !game.client.captured
                && game.mod_screen() == Some("input_route")
        );
    });
}

#[test]
#[ignore = "daily"]
fn a_pointer_grab_is_refused_with_no_screen_open() {
    if run_fresh_cases(
        concat!(
            module_path!(),
            "::a_pointer_grab_is_refused_with_no_screen_open"
        )
        .split_once("::")
        .unwrap()
        .1,
        "SUNRISE_SCREEN_GRAB_NO_SCREEN_CHILD",
        &["grab"],
    ) {
        return;
    }
    with_input_game_as("input_route", |game, _, _| {
        assert_eq!(game.mod_screen(), None);
        assert!(!game.test_set_pointer_grab(true));
        assert!(!game.test_pointer_grabbed());
        game.test_apply_pointer_grab_request();
        assert!(!game.test_screen_grab());
    });
}

#[test]
#[ignore = "daily"]
fn a_pointer_grab_is_refused_while_the_window_is_unfocused() {
    if run_fresh_cases(
        concat!(
            module_path!(),
            "::a_pointer_grab_is_refused_while_the_window_is_unfocused"
        )
        .split_once("::")
        .unwrap()
        .1,
        "SUNRISE_SCREEN_GRAB_UNFOCUSED_CHILD",
        &["grab"],
    ) {
        return;
    }
    with_input_game_as("input_route", |game, id, _| {
        assert!(game.mod_screen_open("input_route"));
        send(game, id, WindowEvent::Focused(false));
        assert!(!game.test_set_pointer_grab(true));
        assert!(!game.test_pointer_grabbed());
        game.test_apply_pointer_grab_request();
        assert!(!game.test_screen_grab() && !game.client.captured);
    });
}
