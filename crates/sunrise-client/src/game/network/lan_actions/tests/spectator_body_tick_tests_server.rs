#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::lan_actions::tick::spectator_body_tick_tests::Log;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::lan_actions::tick::spectator_body_tick_tests::luck_left;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::lan_actions::tick::spectator_body_tick_tests::pearl_left;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::lan_actions::tick::spectator_body_tick_tests::placed;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::lan_actions::tick::spectator_body_tick_tests::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_entity_refs::tests::with_game;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_item_places::tests::{guest, open_to_lan};
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::HostSide;
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::sync::{Arc, Mutex};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{chunk::Chunk, effects::EffectInstance};

#[test]
#[ignore = "daily"]
fn leftover_sleep_countdown_waits_at_the_unloaded_edge_then_resumes() {
    mod_platform::testing::with_mods(Vec::new().into(), || {
        with_game(|game| {
            game.server.world_root = game.save.0.root.clone();
            open_to_lan(game);
            game.world
                .chunks
                .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
            let mut loaded = placed(1, 8.5, SPECTATOR);
            let mut edge = placed(2, 15.5, SPECTATOR);
            loaded.sleep_timer = 105;
            edge.sleep_timer = 105;
            assert!(loaded.bed.is_none() && edge.bed.is_none());
            game.lan.as_mut().unwrap().hub.guests.extend([loaded, edge]);
            game.tick_guests();
            let timers = |game: &Game<'_>| -> Vec<u32> {
                game.lan
                    .as_ref()
                    .unwrap()
                    .hub
                    .guests
                    .iter()
                    .map(|g| g.sleep_timer)
                    .collect()
            };
            assert_eq!(timers(game), [106, 105]);
            game.world
                .chunks
                .insert((1, 0), Arc::new(Chunk::empty(1, 0)));
            game.tick_guests();
            assert_eq!(timers(game), [107, 106]);
        });
    });
}

#[test]
#[ignore = "daily"]
fn pre_teleport_to_unloaded_edge_keeps_the_resolved_body_tick() {
    struct Move;
    #[mod_api::__subscriptions]
    impl mod_api::Hooks for Move {
        fn player_tick_pre(&self, host: HostSide<'_>, event: &mut mod_api::PlayerTickPreEvent) {
            let HostSide::Server(host) = host else {
                panic!("server fixture");
            };
            assert!(mod_api::EntityRef::from(event.player).teleport_to(
                host,
                [15.5, 64.0, 8.5],
                None
            ));
        }
    }
    mod_platform::testing::with_mods(
        vec![mod_api::ModEntry::new("move", Box::new(Move))].into(),
        || {
            with_game(|game| {
                game.server.world_root = game.save.0.root.clone();
                open_to_lan(game);
                // The teleport's target is lit land (an in-level `tp` to land not lit yet holds the
                // player and skips its body, #15727), yet chunk 1 is not loaded for the edge test.
                for x in -2..=2 {
                    for z in -2..=2 {
                        game.streaming
                            .insert_remote(&mut game.world, Chunk::empty(x, z));
                    }
                }
                game.world.chunks.remove(&(1, 0));
                assert!(!game.world.chunks.contains_key(&(1, 0)));
                let mut watcher = placed(1, 8.5, SPECTATOR);
                let mut motion = PlayerMotion::new(game.motion, Vec3::new(8.5, 64.0, 8.5)).unwrap();
                motion.spectator = true;
                motion.local_spectator = true;
                motion.health.food.exhaustion = 5.0;
                watcher.motion = Some(Box::new(motion));
                watcher.saved = Some(sunrise_core::nbt::Tag::compound([(
                    "ShoulderEntityLeft",
                    sunrise_core::nbt::Tag::compound([
                        (
                            "id",
                            sunrise_core::nbt::Tag::String("minecraft:parrot".into()),
                        ),
                        ("Silent", sunrise_core::nbt::Tag::Byte(1)),
                    ]),
                )]));
                game.lan.as_mut().unwrap().hub.guests.push(watcher);
                game.lan
                    .as_mut()
                    .unwrap()
                    .play
                    .entry(1)
                    .or_default()
                    .cooldowns
                    .add("minecraft:ender_pearl".into(), 5);
                game.tick_guests();
                let watcher = game.lan.as_ref().unwrap().hub.guest(1).unwrap();
                assert_eq!(watcher.position, [15.5, 64.0, 8.5]);
                assert!(
                    !game.guest_body_ticks(watcher),
                    "a fresh decision would skip the body"
                );
                assert_eq!(luck_left(game), [99]);
                assert_eq!(pearl_left(game), [4]);
                assert_eq!(watcher.motion.as_ref().unwrap().health.food.exhaustion, 1.0);
                assert!(
                    watcher
                        .saved
                        .as_ref()
                        .unwrap()
                        .get("ShoulderEntityLeft")
                        .is_none()
                );
                game.tick_guests();
                let watcher = game.lan.as_ref().unwrap().hub.guest(1).unwrap();
                assert_eq!(luck_left(game), [99]);
                assert_eq!(pearl_left(game), [4]);
                assert_eq!(watcher.motion.as_ref().unwrap().health.food.exhaustion, 1.0);
            });
        },
    );
}
