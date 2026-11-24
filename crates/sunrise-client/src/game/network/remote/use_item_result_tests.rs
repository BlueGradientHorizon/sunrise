#[allow(unused_imports)]
use super::*;

#[test]
#[ignore = "daily"]
fn default_part_answer_preserves_vanilla_shield_use() {
    mod_platform::testing::with_mods(Vec::<mod_api::ModEntry>::new().into(), || {
        for client in [false, true] {
            with_interaction_game(client, |game| {
                game.inventory[game.selected] = Some(ItemStack::new("minecraft:shield", 1));
                game.swing = Default::default();
                let handled = if client {
                    game.remote_use_item(Hand::Main)
                } else {
                    game.use_item_as_player().unwrap().is_some()
                };
                assert!(handled);
                assert!(game.using.is_some());
                assert!(!game.swing.swinging());
            });
        }
    });
}
