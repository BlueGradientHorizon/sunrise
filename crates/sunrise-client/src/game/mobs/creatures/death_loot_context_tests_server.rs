#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::death_loot_context_tests::BlockTool;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::death_loot_context_tests::CancelOnce;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::death_loot_context_tests::ChangeKillerHand;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::death_loot_context_tests::Nested;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::death_loot_context_tests::Observation;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::death_loot_context_tests::Probe;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::death_loot_context_tests::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::sync::{Arc, Mutex};

#[test]
#[ignore = "daily"]
fn death_loot_killer_is_not_the_acting_guest() {
    use crate::game::native_guest_turn_tests::with_guests;
    use sunrise_client::mobs::Damage;
    with_guests(|game| {
        game.side.next_entity = 1000;
        let loot = sunrise_world::loot::LootData::load(
            &game.options.resources,
            &Default::default(),
            game.res.item_defaults(),
        )
        .unwrap();
        game.side.server_loot.0.set(Some(Arc::new(loot)));
        game.build_registries();
        let mut hand = ItemStack::new("minecraft:diamond_sword", 1);
        hand.damage = 19;
        hand.enchant("minecraft:looting", 2);
        let mut changed = hand.clone();
        changed.damage = 27;
        let expected = changed.try_into_api().unwrap();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let mods = vec![
            mod_api::ModEntry::new("change_hand", Box::new(ChangeKillerHand(expected))),
            mod_api::ModEntry::new("guest_death_loot_probe", Box::new(Probe(seen.clone()))),
        ]
        .into();
        mod_platform::testing::with_mods(mods, || {
            let ty = game
                .res
                .mob_types()
                .get("minecraft:zombie")
                .unwrap()
                .clone();
            for (id, acting) in [(80, Some(13)), (81, None)] {
                game.as_guest(12, |g| {
                    g.inventory[g.selected] = Some(hand.clone());
                })
                .unwrap();
                let mut victim = Mob::new(id, ty.clone(), Vec3::ZERO, 0.0);
                victim.ai.last_hurt_by_player = 100;
                victim.last_damage = Some((Damage::player([0.0; 3]), 0));
                game.side
                    .last_hurt_by_player
                    .insert(id, crate::game::combat::PlayerCredit(12));
                if let Some(conn) = acting {
                    game.as_guest(conn, |g| g.mob_die(&victim)).unwrap();
                } else {
                    assert!(game.side.lan_acting.is_none());
                    game.mob_die(&victim);
                }
            }
        });
        assert_eq!(
            *seen.lock().unwrap(),
            [
                ("minecraft:player_attack".into(), 12, 12, Some(1)),
                ("minecraft:player_attack".into(), 12, 12, Some(1))
            ]
        );
    });
}

#[test]
#[ignore = "daily"]
fn death_loot_nested_death_keeps_outer_context() {
    use crate::game::native_guest_turn_tests::with_guests as with_game;
    use sunrise_client::mobs::{Damage, DamageKind, Who};
    with_game(|game| {
        let loot = sunrise_world::loot::LootData::load(
            &game.options.resources,
            &Default::default(),
            game.res.item_defaults(),
        )
        .unwrap();
        game.side.server_loot.0.set(Some(Arc::new(loot)));
        let ty = game
            .res
            .mob_types()
            .get("minecraft:zombie")
            .unwrap()
            .clone();
        game.build_registries();
        game.set_allow_commands(true);
        game.world
            .chunks
            .insert((0, 0), Arc::new(sunrise_client::chunk::Chunk::empty(0, 0)));
        assert!(game.run_command("summon pig 5.5 64 8.5 {NoAI:1b,NoGravity:1b}"));

        let mut killer = Mob::new(41, ty.clone(), Vec3::ZERO, 0.0);
        killer
            .extra
            .set_main_hand_stack(Some(ItemStack::new("minecraft:diamond_sword", 7)));
        game.mobs.push(killer);
        let mut victim = Mob::new(90, ty, Vec3::ZERO, 0.0);
        let mut damage = Damage::of(DamageKind::MobAttack);
        damage.attacker = Some(Who::Mob(41));
        victim.last_damage = Some((damage, 0));
        let seen = Arc::new(Mutex::new(Vec::new()));
        let mods = vec![
            mod_api::ModEntry::new("nested", Box::new(Nested)),
            mod_api::ModEntry::new("nested_probe", Box::new(Probe(seen.clone()))),
        ]
        .into();
        mod_platform::testing::with_mods(mods, || game.mob_die(&victim));
        assert_eq!(
            *seen.lock().unwrap(),
            [
                ("minecraft:generic_kill".into(), 0, 0, None),
                ("minecraft:mob_attack".into(), 41, 41, Some(7))
            ]
        );
    });
}

#[test]
#[ignore = "daily"]
fn death_loot_uses_call_scoped_source_and_hand() {
    use crate::game::native_entity_refs::tests::{run_fresh_cases, with_game};
    use sunrise_client::mobs::{Damage, DamageKind, Who};
    if run_fresh_cases(
        "game::server_side::creatures_server::death_loot_context_tests::death_loot_uses_call_scoped_source_and_hand",
        "SUNRISE_DEATH_LOOT_CONTEXT_CHILD",
        &["damage_sources"],
    ) {
        return;
    }
    with_game(|game| {
        let loot = sunrise_world::loot::LootData::load(
            &game.options.resources,
            &Default::default(),
            game.res.item_defaults(),
        )
        .unwrap();
        game.side.server_loot.0.set(Some(Arc::new(loot)));
        let seen = Arc::new(Mutex::new(Vec::new()));
        let mods = vec![mod_api::ModEntry::new(
            "death_loot_probe",
            Box::new(Probe(seen.clone())) as Box<dyn mod_api::Hooks>,
        )]
        .into();
        mod_platform::testing::with_mods(mods, || {
            let ty = game
                .res
                .mob_types()
                .get("minecraft:zombie")
                .unwrap()
                .clone();
            game.player =
                Some(sunrise_client::motion::PlayerMotion::new(game.motion, Vec3::ZERO).unwrap());
            crate::game::native_entity_refs::tests::bind_local_player(game, 11, 123);
            // Reserve the fixture's explicit IDs before spawning experience orbs.
            game.side.next_entity = 1000;
            game.inventory[game.selected] = Some(ItemStack::new("minecraft:iron_sword", 2));
            for (id, environmental) in [(68, false), (69, true)] {
                let mut victim = Mob::new(id, ty.clone(), Vec3::ZERO, 0.0);
                victim.ai.last_hurt_by_player = 100;
                game.remember_mob_player_credit(id);
                victim.last_damage = Some((
                    if environmental {
                        Damage::fall()
                    } else {
                        Damage::player([0.0; 3])
                    },
                    0,
                ));
                game.mob_die(&victim);
            }
            let mut killer = Mob::new(41, ty.clone(), Vec3::ZERO, 0.0);
            killer
                .extra
                .set_main_hand_stack(Some(ItemStack::new("minecraft:diamond_sword", 7)));
            game.mobs.push(killer);
            for (id, kind, attacker) in [
                (70, DamageKind::MobAttack, Some(Who::Mob(41))),
                (71, DamageKind::Arrow, Some(Who::Mob(41))),
                (72, DamageKind::Fall, None),
            ] {
                let mut victim = Mob::new(id, ty.clone(), Vec3::ZERO, 0.0);
                let mut damage = Damage::of(kind);
                damage.attacker = attacker;
                victim.last_damage = Some((damage, 0));
                game.mob_die(&victim);
            }
            // Predicate attempts cannot become the source of a later unrelated death.
            for (id, refused) in [(76, false), (77, true)] {
                let mut victim = Mob::new(id, ty.clone(), Vec3::ZERO, 0.0);
                victim.invulnerable_flag = refused;
                victim.last_damage = Some((Damage::fall(), 0));
                game.mobs.push(victim);
                let landed = game.mob_hits_mob(
                    id,
                    if refused { 5.0 } else { 0.0 },
                    Damage::player([0.0; 3]),
                );
                if refused {
                    assert!(!landed);
                }
                let victim = game.mobs.iter_mut().find(|m| m.id == id).unwrap();
                victim.invulnerable_flag = false;
                victim.invulnerable = 0;
                let before = seen.lock().unwrap().len();
                assert!(game.mob_hits_mob(id, 1000.0, Damage::fall()));
                assert_eq!(
                    seen.lock().unwrap()[before..],
                    [("minecraft:fall".to_owned(), 0, 0, None)],
                );
            }
            // IDs belong to the killing blow, not a lookup of a surviving mob.
            game.mobs.retain(|m| m.id != 41);
            let mut victim = Mob::new(73, ty.clone(), Vec3::ZERO, 0.0);
            let mut damage = Damage::of(DamageKind::Arrow);
            damage.attacker = Some(Who::Mob(41));
            victim.last_damage = Some((damage, 0));
            game.mob_die(&victim);
            let mods = vec![
                mod_api::ModEntry::new(
                    "cancel_once",
                    Box::new(CancelOnce(std::sync::atomic::AtomicBool::new(true)))
                        as Box<dyn mod_api::Hooks>,
                ),
                mod_api::ModEntry::new(
                    "cancellation_probe",
                    Box::new(Probe(seen.clone())) as Box<dyn mod_api::Hooks>,
                ),
            ]
            .into();
            mod_platform::testing::with_mods(mods, || {
                let before = seen.lock().unwrap().len();
                game.mobs.push(Mob::new(78, ty.clone(), Vec3::ZERO, 0.0));
                assert!(game.mob_hits_mob(78, 1000.0, Damage::fall()));
                assert_eq!(seen.lock().unwrap().len(), before);
                game.mobs.push(Mob::new(79, ty, Vec3::ZERO, 0.0));
                assert!(game.mob_hits_mob(79, 1000.0, Damage::fall()));
                assert_eq!(seen.lock().unwrap().len(), before + 1);
            });
        });
        assert_eq!(
            *seen.lock().unwrap(),
            [
                ("minecraft:player_attack".into(), 11, 11, Some(2)),
                ("minecraft:fall".into(), 0, 0, None),
                ("minecraft:mob_attack".into(), 41, 41, Some(7)),
                ("minecraft:arrow".into(), 41, 0, Some(7)),
                ("minecraft:fall".into(), 0, 0, None),
                ("minecraft:fall".into(), 0, 0, None),
                ("minecraft:fall".into(), 0, 0, None),
                ("minecraft:arrow".into(), 41, 0, None),
                ("minecraft:fall".into(), 0, 0, None),
            ]
        );
    });
}
