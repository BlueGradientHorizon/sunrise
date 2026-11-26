

#[cfg(test)]
mod tests {
    use super::super::native_entity_refs::tests::with_game;
    use super::*;
    use mod_api::{EntityRef, WorldEpoch};
    use sunrise_core::geometry::Vec3;
    use sunrise_items::items::ItemStack;
    use sunrise_sim::motion::PlayerMotion;

    /// The acting player with a body, as a hook's turn finds them.
    fn acting_player(game: &mut sunrise_gameplay::game::ServerGame<'_>) -> PlayerRef {
        game.player = Some(PlayerMotion::new(game.motion, Vec3::ZERO).unwrap());
        sunrise_gameplay::game::native_entity_refs::tests::bind_local_player(game, 20, 123);
        game.player(123).unwrap()
    }

    #[test]
    #[ignore = "daily"]
    fn points_repair_a_damaged_mending_item_before_experience() {
        with_game(|game| {
            let player = acting_player(game);
            let mut sword = ItemStack::new("minecraft:diamond_sword", 1);
            sword.damage = 1000;
            sword.enchant("minecraft:mending", 1);
            game.inventory[game.selected] = Some(sword);
            assert!(game.player_give_experience_points(player, 10));
            let sword = game.inventory[game.selected].as_ref().unwrap();
            assert!(
                sword.damage < 1000,
                "Mending repaired the item before any point became experience"
            );
            assert_eq!(game.experience.total, 0);
            game.inventory[game.selected] = None;
            assert!(game.player_give_experience_points(player, 10));
            assert_eq!(game.experience.total, 10);
        });
    }

    #[test]
    #[ignore = "daily"]
    fn levels_are_added_taken_and_clamped_at_zero() {
        with_game(|game| {
            let player = acting_player(game);
            game.experience.progress = 0.25;
            assert!(game.player_give_levels(player, 3));
            assert_eq!(game.experience.level, 3);
            assert_eq!(
                game.experience.progress, 0.25,
                "a level add keeps the progress"
            );
            game.experience.total = 40;
            assert!(game.player_give_levels(player, -5));
            assert_eq!(game.experience.level, 0, "below zero clamps to zero");
            assert_eq!(game.experience.total, 0, "the clamp clears the total");
            assert_eq!(
                game.experience.progress, 0.0,
                "the clamp clears the progress"
            );
        });
    }

    #[test]
    #[ignore = "daily"]
    fn the_total_is_set_alone_and_a_negative_one_is_refused() {
        with_game(|game| {
            let player = acting_player(game);
            game.experience.level = 5;
            game.experience.progress = 0.5;
            assert!(game.player_set_total_experience(player, 1234));
            assert_eq!(game.player_experience(player).unwrap().total, 1234);
            assert_eq!(game.experience.level, 5, "the level is kept");
            assert_eq!(game.experience.progress, 0.5, "the progress is kept");
            assert!(!game.player_set_total_experience(player, -1));
            assert_eq!(game.experience.total, 1234, "a refusal writes nothing");
        });
    }

    #[test]
    #[ignore = "daily"]
    fn more_points_than_an_int_are_refused_untouched() {
        with_game(|game| {
            let player = acting_player(game);
            assert!(game.player_give_experience_points(player, i32::MAX as u32));
            assert_eq!(game.experience.total, i32::MAX);
            let mut sword = ItemStack::new("minecraft:diamond_sword", 1);
            sword.damage = 1000;
            sword.enchant("minecraft:mending", 1);
            game.inventory[game.selected] = Some(sword);
            assert!(!game.player_give_experience_points(player, i32::MAX as u32 + 1));
            let sword = game.inventory[game.selected].as_ref().unwrap();
            assert_eq!(sword.damage, 1000, "a refused give repairs nothing");
            assert_eq!(
                game.experience.total,
                i32::MAX,
                "a refused give gives nothing"
            );
        });
    }

    #[test]
    #[ignore = "daily"]
    fn a_stale_handle_gives_nothing() {
        with_game(|game| {
            let player = acting_player(game);
            let stale = mod_api::__private::game::player_ref(
                EntityRef::new(WorldEpoch::new(u64::MAX), 7),
                player.uuid(),
            );
            assert!(!game.player_give_experience_points(stale, 10));
            assert!(!game.player_give_levels(stale, 1));
            assert!(!game.player_set_total_experience(stale, 10));
            assert_eq!(game.experience.total, 0);
            assert_eq!(game.experience.level, 0);
        });
    }
}
use sunrise_gameplay_common::game::native_player_food::NativePlayerFoodGameExt1 as _;

use sunrise_gameplay_common::game::native_server::NativeServerGameExt1 as _;
