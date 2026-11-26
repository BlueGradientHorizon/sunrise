//! Resumable borrowed mob-tick region; the caller owns final restoration.
#[cfg(test)]
use super::*;

#[cfg(test)]
pub(in crate::game) mod off_level_tests {
    use super::*;

    #[test]
    #[ignore = "daily"]
    fn borrowed_tick_resolves_the_retained_controller() {
        crate::game::native_entity_refs::tests::with_game(|game| {
            use sunrise_sim::mobs::goals::{JUMP, LOOK, MOVE};
            game.options.dedicated = Some(Vec::new());
            game.world.chunks.insert(
                (0, 0),
                std::sync::Arc::new(sunrise_level::chunk::Chunk::empty(0, 0)),
            );
            let spider = game
                .res
                .mob_types()
                .get("minecraft:spider")
                .unwrap()
                .clone();
            let skeleton = game
                .res
                .mob_types()
                .get("minecraft:skeleton")
                .unwrap()
                .clone();
            let at = Vec3::new(1.0, 80.0, 0.5);
            let mut vehicle = Mob::new(71, spider, at, 0.0);
            vehicle.extra.rider = Some(72);
            let mut rider = Mob::new(72, skeleton, at, 0.0);
            rider.extra.mount = Some(71);
            rider.age = 19;
            game.mobs.push(vehicle);
            game.persist
                .off_level
                .insert(72, crate::game::entity_memory::OffLevel::Mob(rider));
            game.tick_creature_entities().unwrap();
            assert_eq!(
                game.mob_object(71).unwrap().ai.goals.disabled & (MOVE | JUMP | LOOK),
                MOVE | JUMP | LOOK
            );
            assert_eq!(game.mob_object(72).unwrap().age, 19);
            assert_eq!(game.mobs.len(), 1);
        });
    }

    #[test]
    #[ignore = "daily"]
    fn resident_descendant_does_not_tick_until_detached() {
        crate::game::native_entity_refs::tests::with_game(|game| {
            game.options.dedicated = Some(Vec::new());
            game.world.chunks.insert(
                (0, 0),
                std::sync::Arc::new(sunrise_level::chunk::Chunk::empty(0, 0)),
            );
            let pig = game.res.mob_types().get("minecraft:pig").unwrap().clone();
            let mut parent = Mob::new(72, pig.clone(), Vec3::new(1.0, 80.0, 0.5), 0.0);
            parent.extra.mount = Some(71);
            let mut child = Mob::new(73, pig, parent.position, 0.0);
            child.extra.mount = Some(72);
            child.no_ai = true;
            child.age = 19;
            child.fire_ticks = 20;
            game.persist
                .off_level
                .insert(72, crate::game::entity_memory::OffLevel::Mob(parent));
            game.mobs.push(child);
            game.tick_creature_entities().unwrap();
            let child = game.mob_object(73).unwrap();
            assert_eq!(child.age, 19);
            assert_eq!(child.fire_ticks, 20);
            game.mob_object_mut(73).unwrap().extra.mount = None;
            game.tick_creature_entities().unwrap();
            assert_eq!(game.mob_object(73).unwrap().age, 20);
        });
    }
}

#[cfg(test)]
pub(in crate::game) mod hoglin_routing_tests {
    use super::*;
    use crate::game::server_side::creatures_server::borrowed_tick::route_player_events;

    #[test]
    fn broadcast_keeps_source_player_connection() {
        for (seen_guest, tag, expected) in [
            (Some(12), None, Some(12)),
            (None, None, None),
            (Some(12), Some(Some(13)), Some(13)),
            (Some(12), Some(None), None),
        ] {
            let event = MobEvent::HoglinAttackTarget {
                mob: 9001,
                target: Who::Player,
                player: None,
            };
            let mut events = vec![match tag {
                Some(conn) => MobEvent::ForPlayer {
                    conn,
                    event: Box::new(event),
                },
                None => event,
            }];
            route_player_events(&mut events, 0, seen_guest, &[], &mut Vec::new(), 9000);
            assert!(matches!(events[0], MobEvent::HoglinAttackTarget {
                player, ..
            } if player == expected));
        }
    }
}

#[cfg(test)]
#[path = "mob_tick_access_tests.rs"]
pub(in crate::game) mod mob_tick_access_tests;
