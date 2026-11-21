//! Actual murder notification, with no server or world tick.
use super::*;

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

#[test]
#[ignore = "daily"]
fn every_accepted_hurt_counts_for_its_own_attacker() {
    crate::game::kill_die_vibration_tests::game(|game| {
        game.mobs.clear();
        let kind = game
            .res
            .mob_types()
            .get("minecraft:villager")
            .unwrap()
            .clone();
        let zombie = Mob::new(90, kind.clone(), Vec3::new(2.0, 100.0, 1.0), 0.0);
        game.mobs.push(zombie);
        game.persist.uuids.insert(90, [0, 0, 0, 1]);
        game.mobs
            .push(Mob::new(91, kind, Vec3::new(1.0, 100.0, 1.0), 0.0));
        let (a, b) = ([0, 0, 0, 2], [0, 0, 0, 3]);
        // A mob's hit, and two hits by different players in one tick.
        let hurts = &mut villager::ext_mut(&mut game.mobs[1]).hurts;
        hurts.push((Who::Mob(90), None, true));
        hurts.push((Who::Player, Some(a), true));
        hurts.push((Who::Player, Some(b), true));
        game.villager_reputation_tick();
        let oracle = std::fs::read_to_string(
            std::env::var_os("SUNRISE_REPUTATION_ORACLE").expect("oracle output"),
        )
        .unwrap();
        let expected: Vec<i32> = oracle
            .lines()
            .find_map(|l| l.strip_prefix("hurt "))
            .unwrap()
            .trim_matches(['[', ']'])
            .split(',')
            .map(|s| s.parse().unwrap())
            .collect();
        let got: Vec<i32> = [[0, 0, 0, 1], a, b]
            .map(|uuid| game.villager_reputation_of(91, uuid))
            .into();
        assert_eq!(got, expected);
        assert!(villager::ext(&game.mobs[1]).unwrap().hurts.is_empty());
        // A credit commit binds the latest unbound player hit only.
        let hurts = &mut villager::ext_mut(&mut game.mobs[1]).hurts;
        hurts.push((Who::Player, None, true));
        hurts.push((Who::Player, None, true));
        villager::bind_hurt_player(&mut game.mobs[1], a);
        let hurts = &villager::ext(&game.mobs[1]).unwrap().hurts;
        assert_eq!((hurts[0].1, hurts[1].1), (None, Some(a)));
    });
}
