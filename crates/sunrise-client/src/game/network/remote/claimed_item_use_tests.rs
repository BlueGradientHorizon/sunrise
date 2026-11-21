use super::{S, play};
use crate::game::{
    Hit, Screen,
    native_claimed_items::ModEvent,
    native_entity_refs::tests::with_client_game,
    native_hotbar_slot::hotbar_slot_tests::{logged_in, sent},
};
use sunrise_client::{interaction_input::Button, items::ItemStack};

#[test]
#[ignore = "daily"]
fn interception_does_not_send_item_use_or_release_packets() {
    with_client_game(|game| {
        let mut reader = logged_in(game);
        game.screen = Screen::Playing;
        game.remote.as_mut().unwrap().loaded_sent = true;
        game.claim_items("claim_fixture", &["minecraft:shield".to_owned()]);

        // With the claimed kind in the off hand only, input still starts and releases real use.
        game.offhand = Some(ItemStack::new("minecraft:shield", 1));
        game.right = true;
        game.remote.as_mut().unwrap().input.press(Button::Use);
        game.remote_handle_input();
        assert!(game.using.is_some());
        assert!(
            sent(&mut reader)
                .iter()
                .any(|packet| matches!(packet, S::UseItem(play::UseItem { hand: 1, .. })))
        );
        game.right = false;
        game.remote_handle_input();
        assert!(game.using.is_none());
        assert!(sent(&mut reader).iter().any(|packet| matches!(
            packet,
            S::PlayerAction(play::PlayerAction { action: 6, .. })
        )));
        assert!(game.poll("claim_fixture").is_empty());

        // Now target a block with the claimed item in the main hand. Two presses in one
        // input drain both reach start-use handling, but only the first can capture it.
        game.inventory[game.selected] = game.offhand.take();
        let stone = game
            .pack
            .assets
            .registry
            .default_state("minecraft:stone")
            .unwrap();
        game.world.set(0, 64, 0, stone).unwrap();
        game.forced_hit = Some(Hit {
            pos: [0, 64, 0],
            face: [0, 1, 0],
            point: [0.5, 65.0, 0.5],
            inside: false,
        });
        game.right = true;
        let input = &mut game.remote.as_mut().unwrap().input;
        input.press(Button::Use);
        input.press(Button::Use);
        game.remote_handle_input();
        assert!(game.client_item_use.is_some());
        assert!(game.using.is_none());

        game.use_delay = 0;
        game.remote_handle_input();
        game.remote_tick_using();
        assert!(game.using.is_none());
        assert!(game.remote.as_ref().unwrap().use_item.is_none());

        game.right = false;
        game.remote_handle_input();
        assert!(game.client_item_use.is_none());
        assert!(game.using.is_none());
        assert!(matches!(
            game.poll("claim_fixture").as_slice(),
            [ModEvent::UseStart(_, 0), ModEvent::UseStop(_, 1)]
        ));
        let packets = sent(&mut reader);
        assert!(
            !packets.iter().any(|packet| matches!(
                packet,
                S::UseItem(_)
                    | S::UseItemOn(_)
                    | S::Interact(_)
                    | S::PlayerAction(play::PlayerAction { action: 6, .. })
            )),
            "interception must never send use or release packets: {packets:?}"
        );
    });
}
