#[allow(unused_imports)]
use super::*;

#[test]
#[ignore = "daily"]
fn default_part_answer_preserves_vanilla_shield_use() {
    fn run<S: InteractionSide>(game: &mut Game<'_, S>) {
        game.inventory[game.selected] = Some(ItemStack::new("minecraft:shield", 1));
        game.swing = Default::default();
        assert!(S::interaction_use_item_as_player(game));
        assert!(game.using.is_some());
        assert!(!game.swing.swinging());
    }

    mod_platform::testing::with_mods(Vec::<mod_api::ModEntry>::new().into(), || {
        with_interaction_server_game(run);
        with_interaction_client_game(run);
    });
}
