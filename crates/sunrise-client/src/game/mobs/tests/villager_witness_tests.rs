//! Actual murder notification, with no server or world tick.
use super::*;
use sunrise_client::mobs::{Damage, Who};

#[test]
#[ignore = "daily"]
fn murder_uses_only_the_victims_sensor_memory() {
    crate::game::kill_die_vibration_tests::game(|game| {
        game.mobs.clear();
        let player = [0, 0, 0, 101];
        game.level.player_uuid = Some(player);
        let kind = game
            .res
            .mob_types()
            .get("minecraft:villager")
            .unwrap()
            .clone();
        let mut victim = Mob::new(91, kind.clone(), Vec3::new(1.0, 100.0, 1.0), 0.0);
        game.mobs
            .push(Mob::new(92, kind, Vec3::new(2.0, 100.0, 1.0), 0.0));
        game.villager_murdered(&victim, true, None);
        assert_eq!(
            game.villager_reputation_of(92, player),
            0,
            "a nearby villager absent from the last scan is not a witness"
        );
        let snapshot: Vec<_> = game
            .mobs
            .iter()
            .map(|m| m.seen(None, game.items()))
            .collect();
        let world = game.mob_world_base(&snapshot);
        let mut level = sunrise_client::health::TestLevel::new(0);
        let mut events = Vec::new();
        for _ in 0..20 {
            villager_brain::tick(&mut victim, &world, &mut level.level(), &mut events);
        }
        assert_eq!(
            villager_brain::murder_witnesses(&mut victim, &world),
            vec![92]
        );
        game.villager_murdered(&victim, true, None);
        assert_eq!(game.villager_reputation_of(92, player), -125);
    });
}

/// Hits through `mob_hits_mob`: a mob (removed right after), two players in one tick, a
/// refused hit and a lethal one. Vanilla's `Villager.setLastHurtByMob` adds 25 minor_negative
/// per accepted hit and broadcasts event 13 only for a living villager hit by a player.
#[test]
#[ignore = "daily"]
fn every_accepted_hurt_counts_for_its_own_attacker() {
    crate::game::native_entity_refs::tests::with_game(|game| {
        game.mobs.clear();
        let kind = game
            .res
            .mob_types()
            .get("minecraft:villager")
            .unwrap()
            .clone();
        for (id, x) in [(90, 2.0), (91, 1.0), (92, 3.0)] {
            game.mobs
                .push(Mob::new(id, kind.clone(), Vec3::new(x, 100.0, 1.0), 0.0));
        }
        game.persist.uuids.insert(92, [0, 0, 0, 4]);
        // A player's hit names its player through the player entity.
        game.player = Some(
            sunrise_client::motion::PlayerMotion::new(game.motion, Vec3::new(1.0, 100.0, 1.0))
                .unwrap(),
        );
        let (host, guest) = ([0, 0, 0, 2], 0x3u128);
        game.level.player_uuid = Some(host);
        let hit = |game: &mut Game<'_>, attacker: Who, amount| {
            game.mobs
                .iter_mut()
                .find(|m| m.id == 91)
                .unwrap()
                .invulnerable = 0;
            let damage = match attacker {
                Who::Player => Damage::player([1.0, 100.0, 1.0]),
                Who::Mob(id) => Damage::mob(id, [1.0, 100.0, 1.0]),
            };
            game.mob_hits_mob(91, amount, damage)
        };
        let guest_hit = |game: &mut Game<'_>, uuid: u128, amount| {
            game.lan_acting = Some(7);
            game.lan_acting_uuid = Some(uuid);
            let hurt = hit(game, Who::Player, amount);
            game.lan_acting = None;
            hurt
        };
        assert!(hit(game, Who::Mob(90), 1.0));
        // The attacker is gone before anything ticks; the gossip is in already.
        let zombie = game.mobs.remove(0);
        assert_eq!(game.mobs[0].id, 91);
        assert!(hit(game, Who::Player, 1.0));
        assert!(guest_hit(game, guest, 1.0));
        assert!(
            !game.mob_hits_mob(91, 1.0, Damage::mob(92, [1.0, 100.0, 1.0])),
            "the cooldown refuses a hit this weak"
        );
        assert!(guest_hit(game, 0x5, 1000.0));
        let mob_uuid = *game
            .persist
            .uuids
            .get(&zombie.id)
            .expect("the zombie's made-up UUID is kept at the hit");
        let got = [
            mob_uuid,
            host,
            sunrise_client::lan::playerdata::uuid_ints(guest),
        ]
        .map(|uuid| game.villager_reputation_of(91, uuid));
        let expected = match std::env::var_os("SUNRISE_REPUTATION_ORACLE") {
            // The harness's vanilla `hurt` row: a mob and two players.
            Some(oracle) => std::fs::read_to_string(oracle)
                .unwrap()
                .lines()
                .find_map(|l| l.strip_prefix("hurt "))
                .unwrap()
                .trim_matches(['[', ']'])
                .split(',')
                .map(|s| s.parse().unwrap())
                .collect(),
            None => vec![-25, -25, -25],
        };
        assert_eq!(got.to_vec(), expected);
        let lethal = sunrise_client::lan::playerdata::uuid_ints(0x5);
        assert_eq!(game.villager_reputation_of(91, lethal), -25);
        assert_eq!(game.villager_reputation_of(91, [0, 0, 0, 4]), 0);
        // A site that pushes nobody (`no_push`, as a no_knockback damage type is) counts too, with
        // the UUID of the hit's own source: a player, then a mob that already has one.
        game.mobs
            .push(Mob::new(93, kind.clone(), Vec3::new(1.0, 100.0, 2.0), 0.0));
        let victim = game.mobs.len() - 1;
        for (attacker, uuid) in [(Who::Player, host), (Who::Mob(92), [0, 0, 0, 4])] {
            let mut attack = game.attack_of_who(Some(attacker));
            attack.player_uuid = (attacker == Who::Player).then_some(host);
            game.mobs[victim].invulnerable = 0;
            let damage = match attacker {
                Who::Player => Damage::player([1.0, 100.0, 2.0]),
                Who::Mob(id) => Damage::mob(id, [1.0, 100.0, 2.0]),
            };
            assert!(game.mobs[victim].damage(
                crate::game::entity::damage_level_of!(game, &attack, no_push, None),
                1.0,
                damage
            ));
            assert_eq!(game.villager_reputation_of(93, uuid), -25);
        }
    });
}

/// A mob with no UUID hitting a villager twice before the end of the tick keeps one UUID, and
/// the gossip is under the one the mob keeps.
#[test]
#[ignore = "daily"]
fn two_hits_of_a_mob_without_uuid_share_one() {
    crate::game::native_entity_refs::tests::with_game(|game| {
        game.mobs.clear();
        let kind = game
            .res
            .mob_types()
            .get("minecraft:villager")
            .unwrap()
            .clone();
        game.mobs
            .push(Mob::new(91, kind, Vec3::new(1.0, 100.0, 1.0), 0.0));
        let first = game.attacker_uuid_of(90).0;
        let second = game.attacker_uuid_of(90).0;
        assert_eq!(first, second, "two hits before the drain mint one UUID");
        game.persist.uuids.insert(90, first);
        assert_eq!(game.attacker_uuid_of(90), (first, false));
    });
}

/// `Villager.onReputationEventFrom` refreshes only the trader the hit came from, and a hit's
/// player is the hit's own, not the player acting now.
#[test]
#[ignore = "daily"]
fn hurt_gossip_belongs_to_the_hits_own_player() {
    crate::game::native_entity_refs::tests::with_game(|game| {
        game.mobs.clear();
        let kind = game
            .res
            .mob_types()
            .get("minecraft:villager")
            .unwrap()
            .clone();
        game.mobs
            .push(Mob::new(91, kind, Vec3::new(1.0, 100.0, 1.0), 0.0));
        let (host, guest) = ([0, 0, 0, 2], [0, 0, 0, 9]);
        game.level.player_uuid = Some(host);
        let attack = sunrise_client::health::Attack {
            player_uuid: Some(guest),
            ..Default::default()
        };
        let plan = game.prepare_knockback_credit(91, Some(Who::Player), &attack);
        let credit = plan.lend(&mut game.last_hurt_by_player);
        assert_eq!(
            sunrise_client::health::KnockbackCredit::hurt_by_uuid(
                &credit,
                &Damage::player([1.0; 3])
            ),
            Some((guest, false))
        );
    });
}
