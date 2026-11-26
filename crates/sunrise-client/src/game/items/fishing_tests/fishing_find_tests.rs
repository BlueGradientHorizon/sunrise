//! A cast bobber is an entity of its level (#6100): selectors and `entities_in` find it, and
//! `fishing_hook` narrows it to a `FishingHookRef`, as vanilla's `FishingHook` is found.
// Included as fishing's child so fixtures can construct its private hook state.
use super::*;
use crate::game::native_entity_refs::tests::{collect_game_entities_in, with_game};
use std::sync::Arc;
use sunrise_client::motion::PlayerMotion;

/// The caster's bobber at (5.5, 66, 0.5) over air, after one tick.
#[test]
#[ignore = "daily"]
fn a_cast_bobber_is_found_by_selectors_and_entities_in() {
    mod_platform::testing::with_mods(Default::default(), || {
        with_game(|game| {
            game.world
                .chunks
                .insert((0, 0), Arc::new(sunrise_client::chunk::Chunk::empty(0, 0)));
            game.player = Some(PlayerMotion::new(game.motion, Vec3::new(0.5, 64.0, 0.5)).unwrap());
            game.inventory[game.selected] = Some(ItemStack::new("minecraft:fishing_rod", 1));
            crate::game::native_entity_refs::tests::bind_local_player(game, 20, 7);
            game.set_allow_commands(true);
            game.build_registries();
            let id = game.take_entity_id();
            let mut bobber = Hook::remote(id, Vec3::new(5.5, 66.0, 0.5), [0; 4]);
            bobber.random = LegacyRandom::new(1);
            game.install_fishing_hook(bobber);
            game.tick_fishing();
            let at = game.hook.as_ref().expect("the bobber stays out").position;
            assert!(
                game.level_candidates(false)
                    .contains(&super::super::commands::Target::Hook(id)),
                "a selector's candidates"
            );
            assert!(game.run_command("execute if entity @e[type=minecraft:fishing_bobber]"));
            let level = game.level(game.dim.id()).unwrap();
            let found = collect_game_entities_in(
                game,
                level,
                [at.x - 1.0, at.y - 1.0, at.z - 1.0],
                [at.x + 1.0, at.y + 1.0, at.z + 1.0],
            )
            .unwrap();
            let hooks: Vec<_> = found
                .into_iter()
                .filter_map(|entity| game.fishing_hook(entity))
                .collect();
            assert_eq!(hooks.len(), 1, "entities_in finds the bobber");
            assert_eq!(hooks[0].entity().id(), id);
        });
    });
}

/// #6100 review: the in-place player's bobber is a candidate of that player's level only. With
/// the Nether loaded and put in place for a command, the Overworld player's bobber is no
/// candidate there, so an unbounded selector walking both levels finds it once.
#[test]
#[ignore = "daily"]
fn a_bobber_is_a_candidate_of_its_casters_level_only() {
    mod_platform::testing::with_mods(Default::default(), || {
        with_game(|game| {
            game.world
                .chunks
                .insert((0, 0), Arc::new(sunrise_client::chunk::Chunk::empty(0, 0)));
            game.player = Some(PlayerMotion::new(game.motion, Vec3::new(0.5, 64.0, 0.5)).unwrap());
            game.inventory[game.selected] = Some(ItemStack::new("minecraft:fishing_rod", 1));
            crate::game::native_entity_refs::tests::bind_local_player(game, 20, 7);
            game.set_allow_commands(true);
            game.build_registries();
            let id = game.take_entity_id();
            let mut bobber = Hook::remote(id, Vec3::new(5.5, 66.0, 0.5), [0; 4]);
            bobber.random = LegacyRandom::new(1);
            game.install_fishing_hook(bobber);
            game.tick_fishing();
            let hook = super::super::commands::Target::Hook(id);
            let home = game.dim;
            assert!(
                game.level_candidates(false).contains(&hook),
                "its own level"
            );
            game.enter_command_level(crate::game::Dim::Nether);
            assert_ne!(game.dim, home);
            assert!(
                !game.level_candidates(false).contains(&hook),
                "another loaded level"
            );
            game.enter_command_level(home);
            assert!(game.level_candidates(false).contains(&hook), "back home");
        });
    });
}
