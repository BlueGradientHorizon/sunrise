

#[cfg(test)]
pub(in crate::game) mod tests {
    use super::super::native_entity_refs::tests::with_game;
    use super::*;
    use sunrise_core::geometry::Vec3;
    use sunrise_sim::mobs::Mob;

    #[test]
    #[ignore = "daily"]
    fn fox_enderman_and_zombified_piglin_take_targets_with_their_side_effects() {
        use sunrise_core::random::{LegacyRandom, RandomSource};
        with_game(|game| {
            for (id, name) in [
                (12, "minecraft:fox"),
                (13, "minecraft:enderman"),
                (14, "minecraft:zombified_piglin"),
                (15, "minecraft:zombie"),
            ] {
                let kind = game.res.mob_types().get(name).unwrap().clone();
                game.mobs.push(Mob::new(id, kind, Vec3::ZERO, 0.0));
            }
            let handle = |game: &crate::game::ServerGame<'_>, id| {
                game.mob(game.entity(id).unwrap()).unwrap()
            };
            let (fox, enderman, piglin) = (handle(game, 12), handle(game, 13), handle(game, 14));
            let other = game.living(game.entity(15).unwrap()).unwrap();
            let state = |game: &crate::game::ServerGame<'_>, id| {
                game.mobs.iter().find(|m| m.id == id).unwrap().clone()
            };

            // Fox.setTarget: a target leaves defending as it is; clearing ends it.
            game.mobs
                .iter_mut()
                .find(|m| m.id == 12)
                .unwrap()
                .extra
                .ints[0] |= mobs::fox::DEFENDING;
            mobs::fox::add_trusted_entity(
                game.mobs.iter_mut().find(|m| m.id == 12).unwrap(),
                [1, 2, 3, 4],
            );
            assert!(fox.set_target(game, Some(other)));
            assert_eq!(fox.target(game), Some(other));
            assert!(mobs::fox::flag(&state(game, 12), mobs::fox::DEFENDING));
            assert!(fox.set_target(game, None));
            assert_eq!(fox.target(game), None);
            assert!(!mobs::fox::flag(&state(game, 12), mobs::fox::DEFENDING));
            assert!(mobs::fox::trusts(&state(game, 12), [1, 2, 3, 4]));

            // Enderman.setTarget: the open mouth, targetChangeTime and the
            // attack speed modifier follow the target.
            let enderman_state = game.mobs.iter_mut().find(|m| m.id == 13).unwrap();
            enderman_state.age = 1234;
            enderman_state.extra.flag2 = true;
            game.start_audio_capture();
            assert!(enderman.set_target(game, Some(other)));
            let events =
                std::mem::take(&mut game.mobs.iter_mut().find(|m| m.id == 13).unwrap().outbox);
            game.handle_mob_events(events).unwrap();
            assert!(
                game.side
                    .pending_sounds
                    .iter()
                    .any(|sound| sound.event == "minecraft:entity.enderman.stare")
            );
            let m = state(game, 13);
            assert!(m.extra.flag, "DATA_CREEPY");
            assert_eq!(m.extra.counter, 1234, "targetChangeTime");
            assert!(mobs::enderman::attacking_speed(&m) > 0.0);
            assert!(m.extra.flag2, "retargeting preserves DATA_STARED_AT");
            assert!(m.movement_speed() > f64::from(0.3f32));
            assert!(enderman.set_target(game, None));
            let m = state(game, 13);
            assert!(!m.extra.flag);
            assert_eq!(m.extra.counter, 0);
            assert!(!m.extra.flag2, "clearing resets DATA_STARED_AT");
            assert_eq!(mobs::enderman::attacking_speed(&m), 0.0);

            // ZombifiedPiglin.setTarget samples both delays immediately;
            // changing a nonempty target does not resample either.
            let mut expected = LegacyRandom::new(2026);
            let sound_delay = expected.next_int_bound(21).unwrap();
            let alert_delay = 80 + expected.next_int_bound(41).unwrap();
            game.mobs.iter_mut().find(|m| m.id == 14).unwrap().random = LegacyRandom::new(2026);
            assert!(piglin.set_target(game, Some(other)));
            assert_eq!(piglin.target(game), Some(other));
            let first = state(game, 14);
            assert_eq!(first.extra.counter, sound_delay);
            assert_eq!(first.extra.counter2, alert_delay);
            assert!(piglin.set_target(game, Some(other)));
            let changed = state(game, 14);
            assert_eq!(changed.extra.counter, first.extra.counter);
            assert_eq!(changed.extra.counter2, first.extra.counter2);
            assert!(piglin.set_target(game, None));
            assert_eq!(piglin.target(game), None);
        });
    }

    /// `ZombifiedPiglin.setTarget`, `customServerAiStep`, and `alertOthers`:
    /// a host write is consumed by real mob ticks, audible to the player and
    /// propagated to another piglin. Seed selection fixes the first sound at
    /// one tick; advancing the alert timer avoids waiting four seconds.
    #[test]
    #[ignore = "daily"]
    fn piglin_target_write_drives_anger_sound_and_alert() {
        use sunrise_core::random::{LegacyRandom, RandomSource};
        with_game(|game| {
            for (x, z) in (-1..=1).flat_map(|x| (-1..=1).map(move |z| (x, z))) {
                let mut chunk = sunrise_level::chunk::Chunk::empty(x, z);
                chunk.lit = true;
                game.world.chunks.insert((x, z), std::sync::Arc::new(chunk));
            }
            for (id, kind, x) in [
                (12, "minecraft:zombified_piglin", 0.0),
                (13, "minecraft:zombified_piglin", 2.0),
                (14, "minecraft:iron_golem", 8.0),
            ] {
                let kind = game.res.mob_types().get(kind).unwrap().clone();
                let mut mob = Mob::new(id, kind, Vec3::new(x, 100.0, 0.0), 0.0);
                mobs::ai::register(&mut mob);
                game.mobs.push(mob);
            }
            let seed = (0..10_000)
                .find(|&seed| LegacyRandom::new(seed).next_int_bound(21) == Ok(1))
                .unwrap();
            game.mobs[0].random = LegacyRandom::new(seed);
            let actor = game.mob(game.entity(12).unwrap()).unwrap();
            let other = game.mob(game.entity(13).unwrap()).unwrap();
            let target = game.living(game.entity(14).unwrap()).unwrap();
            assert!(actor.set_target(game, Some(target)));
            assert_eq!(game.mobs[0].extra.counter, 1);
            game.mobs[0].extra.counter2 = 0;
            game.start_audio_capture();
            game.tick_mobs().unwrap();
            assert_eq!(other.target(game), Some(target));
            game.tick_mobs().unwrap();
            assert!(
                game.side
                    .pending_sounds
                    .iter()
                    .any(|sound| sound.event == "minecraft:entity.zombified_piglin.angry")
            );
        });
    }
}
use sunrise_gameplay_common::game::creatures::ModGameExt1 as _;

use sunrise_gameplay_common::game::native_entity_refs_common::NativeEntityRefsCommonGameExt1 as _;

use sunrise_gameplay_common::game::native_entity_refs_common::NativeEntityRefsCommonGameExt6 as _;
