#![cfg(test)]
use sunrise_core::nbt::Tag;
use sunrise_sim::lan::entity::EntityState;
#[cfg(test)]
pub(crate) mod tests {
    macro_rules! pending {
        ($list:expr) => {{
            let list = &mut $list;
            let mut owner = list.remove(list.len() - 1);
            owner.id = 0;
            list.push(owner);
        }};
    }

    use super::*;

    #[test]
    #[ignore = "daily"]
    fn tracking_ids_preserve_all_family_interleaving() {
        sunrise_gameplay::game::native_entity_refs::tests::with_game(|game| {
            for (kind, nbt) in [
                ("minecraft:tnt", "{fuse:80s}"),
                (
                    "minecraft:falling_block",
                    "{BlockState:{Name:\"minecraft:sand\"},Time:1}",
                ),
                ("minecraft:arrow", "{}"),
                ("minecraft:trident", "{}"),
                ("minecraft:firework_rocket", "{LifeTime:20}"),
                ("minecraft:end_crystal", "{}"),
                (
                    "minecraft:ominous_item_spawner",
                    "{item:{id:\"minecraft:stone\",count:1}}",
                ),
                ("minecraft:area_effect_cloud", "{Duration:200,Radius:3.0f}"),
            ] {
                game.summon_with_nbt(kind, [1.0, 70.0, 1.0], nbt).unwrap();
            }
            pending!(game.tnts);
            pending!(game.falling);
            pending!(game.creatures.projectiles);
            pending!(game.tridents);
            pending!(game.rockets);
            pending!(game.crystals);
            pending!(game.ominous_spawners);
            pending!(game.breath_clouds);
            game.bolts.push(
                sunrise_gameplay_common::game::lightning_common::Bolt::new(
                    0,
                    [1.0, 70.0, 1.0],
                    true,
                )
                .0,
            );
            sunrise_client_gameplay::game::client_side::potions::add_remote_thrown(
                &mut game.potions,
                0,
                "minecraft:splash_potion",
                Default::default(),
                Default::default(),
            );
            game.end.add_remote_eye(0, [1.0, 70.0, 1.0], [0.0; 3]);
            let base = game.server.next_entity;
            let joined = game.persist.joined_seq;
            let announced = game.persist.live.clone();
            let mut slots = crate::game::entity_server::taken_pass_tests::SlotIdentities::default();
            for _ in 0..2 {
                game.lan_assign_ids();
                let ids = [
                    game.tnts[0].id,
                    game.falling[0].id,
                    game.creatures.projectiles[0].id,
                    game.tridents[0].id,
                    game.rockets[0].id,
                    game.crystals[0].id,
                    game.bolts[0].id,
                    game.ominous_spawners[0].id,
                    *sunrise_gameplay::game::server_side::potions_server::thrown_ids(
                        &mut game.common.potions,
                    )
                    .next()
                    .unwrap(),
                    *game.common.end.eye_ids().next().unwrap(),
                    game.common.breath_clouds[0].id,
                ];
                assert_eq!(
                    ids,
                    std::array::from_fn::<_, 11, _>(|i| base + i as u32 + 1)
                );
                assert_eq!(game.server.next_entity, base + 11);
                assert_eq!(game.persist.joined_seq, joined);
                assert_eq!(game.persist.live, announced);
                let epoch = game.native_lifecycle.epoch().unwrap();
                slots.observe(&mut game.falling, epoch, |f| f.id);
                game.creatures.projectiles.for_each_mut(|p| {
                    let id = p.id;
                    slots.observe_one(p, epoch, id);
                });
            }
        });
    }
    #[test]
    #[ignore = "daily"]
    fn tracking_ids_refuse_bound_zero_owners() {
        for kind in ["minecraft:falling_block", "minecraft:arrow"] {
            sunrise_gameplay::game::native_entity_refs::tests::with_game(|game| {
                let nbt = if kind == "minecraft:arrow" {
                    "{}"
                } else {
                    "{BlockState:{Name:\"minecraft:sand\"},Time:1}"
                };
                game.summon_with_nbt(kind, [1.0, 70.0, 1.0], nbt).unwrap();
                let epoch = game.native_lifecycle.epoch().unwrap();
                let mut slots =
                    crate::game::entity_server::taken_pass_tests::SlotIdentities::default();
                if kind == "minecraft:arrow" {
                    pending!(game.creatures.projectiles);
                    game.creatures
                        .projectiles
                        .for_each_mut(|p| slots.observe_one(p, epoch, 0));
                } else {
                    pending!(game.falling);
                    slots.observe(&mut game.falling, epoch, |f| f.id);
                }
                let before = game.server.next_entity;
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    game.lan_assign_ids()
                }));
                assert!(result.is_err());
                assert_eq!(game.server.next_entity, before);
            });
        }
    }
    #[test]
    #[ignore = "daily"]
    fn tracking_ids_only_number_the_spawn_buffer_during_a_taken_pass() {
        macro_rules! check {
            ($kind:expr, $nbt:expr, $game:ident, $list:expr) => {
                crate::game::native_entity_refs::tests::with_game(|$game| {
                    $game
                        .summon_with_nbt($kind, [1.0, 70.0, 1.0], $nbt)
                        .unwrap();
                    pending!($list);
                    let taken = $list.lift();
                    $game
                        .summon_with_nbt($kind, [2.0, 70.0, 1.0], $nbt)
                        .unwrap();
                    pending!($list);
                    let base = $game.server.next_entity;
                    $game.lan_assign_ids();
                    assert_eq!(taken[0].id, 0);
                    assert_eq!($list[0].id, base + 1);
                    $list.restore(taken, sunrise_sim::join_seq::RestoreOrder::SurvivorsFirst);
                    $game.lan_assign_ids();
                    assert_eq!(
                        $list.iter().map(|p| p.id).collect::<Vec<_>>(),
                        [base + 2, base + 1]
                    );
                    assert_eq!($game.server.next_entity, base + 2);
                });
            };
        }
        sunrise_gameplay::game::native_entity_refs::tests::with_game(|game| {
            game.summon_with_nbt("minecraft:arrow", [1.0, 70.0, 1.0], "{}")
                .unwrap();
            pending!(game.creatures.projectiles);
            let taken = game.take_entities();
            game.summon_with_nbt("minecraft:arrow", [2.0, 70.0, 1.0], "{}")
                .unwrap();
            pending!(game.creatures.projectiles);
            let base = game.server.next_entity;
            game.lan_assign_ids();
            assert_eq!(game.creatures.projectiles[0].id, base + 1);
            game.restore_entities(taken);
            assert_eq!(game.creatures.projectiles[0].id, 0);
            game.lan_assign_ids();
            assert_eq!(
                game.creatures
                    .projectiles
                    .iter()
                    .map(|p| p.id)
                    .collect::<Vec<_>>(),
                [base + 2, base + 1]
            );
            assert_eq!(game.server.next_entity, base + 2);
        });
        check!(
            "minecraft:area_effect_cloud",
            "{Duration:200,Radius:3.0f}",
            game,
            game.breath_clouds
        );
        check!(
            "minecraft:ominous_item_spawner",
            "{}",
            game,
            game.ominous_spawners
        );
    }
    #[test]
    #[ignore = "daily"]
    fn late_dragon_ids_reserve_parts_before_crystals() {
        sunrise_gameplay::game::native_entity_refs::tests::with_game(|game| {
            game.summon_with_nbt("minecraft:ender_dragon", [1.0, 70.0, 1.0], "{}")
                .unwrap();
            let mut dragon = game.dragon.extract().unwrap();
            dragon.id = 0;
            assert!(game.dragon.install(dragon).is_ok());
            for _ in 0..2 {
                game.summon_with_nbt("minecraft:end_crystal", [1.0, 70.0, 1.0], "{}")
                    .unwrap();
                pending!(game.crystals);
            }
            let base = game.server.next_entity;
            for _ in 0..2 {
                game.assign_dragon_and_crystal_ids();
                assert_eq!(game.dragon.as_ref().unwrap().id, base + 1);
                assert_eq!(
                    game.crystals.iter().map(|c| c.id).collect::<Vec<_>>(),
                    [base + 10, base + 11]
                );
                assert_eq!(game.server.next_entity, base + 11);
                game.lan_assign_ids();
                assert_eq!(game.server.next_entity, base + 11);
            }
        });
    }
    #[test]
    #[ignore = "daily"]
    fn kept_display_data_is_what_a_build_gives() {
        sunrise_gameplay::game::native_entity_refs::tests::with_game(|game| {
            let snbt = |s: &str| sunrise_command::snbt::parse_fully(s).unwrap();
            let displays: [(&str, Tag, Tag); 3] = [
                (
                    "minecraft:text_display",
                    snbt(r#"{text:{text:"Shop",color:"gold"},billboard:"center",line_width:120}"#),
                    snbt(r#"{text:{text:"Sold out",bold:1b},billboard:"center",background:0}"#),
                ),
                (
                    "minecraft:block_display",
                    snbt(
                        r#"{block_state:{Name:"minecraft:oak_stairs",Properties:{facing:"east"}}}"#,
                    ),
                    snbt(r#"{block_state:{Name:"minecraft:glass"},brightness:{sky:15,block:3}}"#),
                ),
                (
                    "minecraft:item_display",
                    snbt(r#"{item:{id:"minecraft:diamond",count:3},item_display:"gui"}"#),
                    snbt(
                        r#"{item:{id:"minecraft:apple",count:1},item_display:"head",glow_color_override:255}"#,
                    ),
                ),
            ];
            let state = |kind: &str| EntityState {
                kind: kind.into(),
                ..Default::default()
            };
            for (id, (kind, first, second)) in displays.iter().enumerate() {
                let id = 900 + id as u32;
                // Ticks with the first compound, then the second, then the first
                // again: each answer is a fresh build's.
                for tag in [first, first, first, second, second, first] {
                    let mut built = state(kind);
                    let built_item = game.lan_display_data(tag, &mut built);
                    assert!(!built.meta.is_empty(), "{kind}");
                    let mut kept = state(kind);
                    let kept_item = game.lan_display_data_kept(id, tag, &mut kept);
                    assert_eq!(format!("{kept_item:?}"), format!("{built_item:?}"));
                    assert_eq!(
                        format!("{:?}", kept.meta),
                        format!("{:?}", built.meta),
                        "{kind}"
                    );
                    assert_eq!(
                        format!("{:?}", kept.defaults),
                        format!("{:?}", built.defaults),
                        "{kind}"
                    );
                }
            }
        });
    }
}
