

#[cfg(test)]
pub(crate) mod tests {

use sunrise_client_gameplay::game::client_side::native_client_session::tests::logged_client;

use sunrise_gameplay_common::game::client_side::container_hooks::tests::PASS;

use sunrise_gameplay_common::game::client_side::container_hooks::tests::PICK_NINE;

use sunrise_gameplay_common::game::client_side::container_hooks::tests::READ_FAR;

use sunrise_gameplay_common::game::client_side::container_hooks::tests::TAKE;

use sunrise_gameplay_common::game::client_side::container_hooks::tests::TAKE_RELEASE;

use super::*;

use crate::game::native_entity_refs::tests::with_client_game;

use sunrise_hooks::containers::{ContainerHooks, engine::with_registrants};

#[test]
    #[ignore = "daily"]
    fn a_hooks_click_goes_through_the_menu() {
        with_client_game(|game| {
            let _server = sunrise_client_gameplay::game::client_side::native_client_session::tests::logged_client(game);
            game.side.screen = Screen::Inventory;
            let stone = ItemStack::new("minecraft:stone", 5);
            game.inventory[9] = Some(stone.clone());
            with_registrants(&[&PICK_NINE], || {
                assert!(game.container_mouse(MouseButton::Left, true, false))
            });
            assert_eq!(game.carried, Some(stone));
            assert!(game.inventory[9].is_none());
        });
    }
#[test]
    #[ignore = "daily"]
    fn a_taken_press_keeps_its_release_from_vanilla() {
        with_client_game(|game| {
            game.side.screen = Screen::Inventory;
            with_registrants(&[&TAKE], || {
                assert!(game.container_mouse(MouseButton::Left, true, false));
                // Take takes no release: the button-up says the press was the hooks'.
                let owned = game.container_button_up(MouseButton::Left);
                assert!(owned);
                assert!(game.container_mouse(MouseButton::Left, false, owned));
                assert!(
                    !game.container_button_up(MouseButton::Left),
                    "the latch is cleared"
                );
            });
            with_registrants(&[&PASS], || {
                assert!(!game.container_mouse(MouseButton::Left, true, false));
                assert!(!game.container_button_up(MouseButton::Left));
            });
        });
    }
#[test]
    #[ignore = "daily"]
    fn a_release_taken_from_vanillas_press_ends_its_drag_unclicked() {
        with_client_game(|game| {
            game.side.screen = Screen::Inventory;
            game.side.inv_drag = Some((0, vec![9]));
            with_registrants(&[&TAKE_RELEASE], || {
                assert!(game.container_mouse(MouseButton::Left, false, false))
            });
            assert!(game.side.inv_drag.is_none());
        });
    }
#[test]
    #[ignore = "daily"]
    fn a_slot_outside_the_open_menu_reads_nothing() {
        with_client_game(|game| {
            game.side.screen = Screen::Inventory;
            let reads = || READ_FAR.0.load(std::sync::atomic::Ordering::Relaxed);
            // The press is taken only when the hook ran and every far read came back empty.
            with_registrants(&[&READ_FAR], || {
                assert!(game.container_mouse(MouseButton::Left, true, false))
            });
            assert_eq!(reads(), 1);
            let mut batch = HudBatch::new([854, 480], game.side.settings.gui_scale_inputs());
            with_registrants(&[&READ_FAR], || game.draw_container_hooks(&mut batch));
            assert_eq!(reads(), 2, "the draw view reads nothing there either");
        });
    }
#[test]
    #[ignore = "daily"]
    fn a_key_is_not_dispatched_while_the_recipe_search_is_focused() {
        with_client_game(|game| {
            game.side.screen = Screen::Inventory;
            with_registrants(&[&TAKE], || assert!(game.container_key(KeyCode::KeyR)));
            game.level.recipe_book.open[sunrise_sim::recipe_book::BookType::Crafting.index()] =
                true;
            game.ensure_recipe_search();
            game.recipe_focus_search();
            assert!(game.recipe_search_focused());
            BUILT.with(|n| n.set(0));
            with_registrants(&[&TAKE], || assert!(!game.container_key(KeyCode::KeyR)));
            assert_eq!(BUILT.with(|n| n.get()), 0);
        });
    }
#[test]
    #[ignore = "daily"]
    fn no_view_is_built_without_a_registrant_or_a_held_button() {
        with_client_game(|game| {
            game.side.screen = Screen::Inventory;
            BUILT.with(|n| n.set(0));
            with_registrants(&[], || {
                assert!(!game.container_mouse(MouseButton::Left, true, false))
            });
            assert_eq!(BUILT.with(|n| n.get()), 0);
            // No button is held: a pointer move is no drag, however a press went.
            with_registrants(&[&TAKE], || assert!(!game.container_drag()));
            assert_eq!(BUILT.with(|n| n.get()), 0);
            with_registrants(&[&TAKE], || {
                assert!(game.container_mouse(MouseButton::Left, true, false))
            });
            assert_eq!(BUILT.with(|n| n.get()), 1);
        });
    }
}
