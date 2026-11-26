

#[cfg(test)]
pub(crate) mod tests {

use crate::game::damage_item_tests::{Rule, with_wear_part};

use sunrise_core::geometry::Vec3;

use sunrise_sim::mobs::{Mob, finalize};

use sunrise_items::items::ItemStack;

#[test]
    #[ignore = "daily"]
    fn mob_armour_obeys_negative_cancel_and_breaking_answers() {
        with_wear_part("minecraft:iron_helmet", Rule::Set(-1), |game, wear| {
            let kind = game
                .res
                .mob_types()
                .get("minecraft:zombie")
                .unwrap()
                .clone();
            let mut mob = Mob::new(901, kind, Vec3::ZERO, 0.0);
            finalize::set_item_stack(&mut mob, 5, ItemStack::new("minecraft:iron_helmet", 1));
            game.mobs.push(mob);
            for rule in [Rule::Set(i32::MIN), Rule::Cancel] {
                wear.set(rule);
                game.wear_mob_piece(901, 5, 1);
                assert_eq!(
                    finalize::item_stack(game.mobs.last().unwrap(), 5)
                        .unwrap()
                        .damage,
                    0
                );
                assert_eq!(wear.take_asked(), [1]);
            }
            wear.set(Rule::Set(164));
            game.wear_mob_piece(901, 5, 1);
            assert_eq!(
                finalize::item_stack(game.mobs.last().unwrap(), 5)
                    .unwrap()
                    .damage,
                164
            );
            wear.set(Rule::Set(i32::MAX));
            game.wear_mob_piece(901, 5, 1);
            assert!(finalize::item_stack(game.mobs.last().unwrap(), 5).is_none());
            assert_eq!(wear.take_asked(), [1, 1]);
            game.wear_mob_piece(901, 5, 1);
            assert!(wear.take_asked().is_empty());
        });
    }
#[test]
    #[ignore = "daily"]
    fn thorns_and_boots_wear_break_the_equipped_piece() {
        use sunrise_core::geometry::Vec3;
        use sunrise_items::items::ItemStack;
        use sunrise_sim::motion::PlayerMotion;
        super::super::native_entity_refs::tests::with_game(|game| {
            game.player = Some(PlayerMotion::new(game.motion, Vec3::ZERO).unwrap());
            let mut chest = ItemStack::new("minecraft:diamond_chestplate", 1);
            chest.damage = game.items().get(&chest.item).unwrap().max_damage - 1;
            chest.data_mut().enchantments = vec![("minecraft:thorns".into(), 255)];
            game.armor[1] = Some(chest);
            assert!(!game.thorns_damage().is_empty());
            assert!(game.armor[1].is_none());
            let mut boots = ItemStack::new("minecraft:diamond_boots", 1);
            boots.damage = game.items().get(&boots.item).unwrap().max_damage - 1;
            game.armor[3] = Some(boots);
            game.wear_boots(1);
            assert!(game.armor[3].is_none());
        });
    }
}
