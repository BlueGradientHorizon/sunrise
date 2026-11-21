use super::Game;
use crate::game::native_guest_turn_tests::with_guests;
use mod_api::{
    CriterionContext, CriterionTest, CriterionTestError, Identifier, TriggerCriterionError,
};
use serde_json::json;
use std::sync::Arc;
use sunrise_client::{advancements as a, chunk::Chunk, items::ItemStack as EngineStack};

fn install(game: &mut Game<'_>) {
    game.build_registries();
    game.world
        .chunks
        .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
    let mut tree = a::Tree::default();
    for (id, trigger, conditions) in [
        (
            "default",
            "default_block_use",
            json!({"location":{"type":"minecraft:location_check","predicate":{"position":{"x":0.5,"y":64.5,"z":0.5}}}}),
        ),
        (
            "item2",
            "item_used_on_block",
            json!({"location":{"type":"minecraft:match_tool","predicate":{"items":"minecraft:stone","count":2}}}),
        ),
        (
            "position_requires_chunk",
            "default_block_use",
            json!({"location":{"condition":"minecraft:location_check","predicate":{"block":{"blocks":"minecraft:air"}}}}),
        ),
        ("default_unconditional", "default_block_use", json!({})),
        (
            "any2",
            "any_block_use",
            json!({"location":{"type":"minecraft:match_tool","predicate":{"items":"minecraft:stone","count":2}}}),
        ),
        (
            "any1",
            "any_block_use",
            json!({"location":{"type":"minecraft:match_tool","predicate":{"items":"minecraft:stone","count":1}}}),
        ),
        ("any", "any_block_use", json!({})),
        (
            "consume",
            "consume_item",
            json!({"item":{"items":"minecraft:stone","count":2}}),
        ),
    ] {
        let trigger = format!("minecraft:{trigger}");
        let index = tree.list.len();
        let id = format!("test:{id}");
        tree.by_id.insert(id.clone(), index);
        tree.by_trigger
            .entry(trigger.clone())
            .or_default()
            .push((index, 0));
        tree.list.push(a::Advancement {
            id,
            parent: None,
            display: None,
            criteria: vec![a::Criterion {
                name: "seen".into(),
                trigger,
                conditions: conditions.into(),
            }],
            requirements: vec![vec!["seen".into()]],
            rewards: Default::default(),
        });
    }
    game.adv.tree = Arc::new(tree);
    game.lan
        .as_mut()
        .unwrap()
        .hub
        .guests
        .update_all(|g| g.advancements_loaded = true);
}
fn has(game: &Game<'_>, id: &str) -> bool {
    game.lan
        .as_ref()
        .unwrap()
        .hub
        .guest(12)
        .unwrap()
        .advancements
        .has(&format!("test:{id}"), "seen")
}

#[test]
#[ignore = "daily"]
fn default_block_use_awards_only_default() {
    with_guests(|g| {
        install(g);
        let p = g.player(12).unwrap();
        assert_eq!(p.trigger_block_use(g, [0, 64, 0]), Ok(()));
        assert!(has(g, "default"));
        assert!(!has(g, "any"));
        assert!(!has(g, "item2"));
        assert!(!has(g, "any1"));
        assert!(!has(g, "any2"));
    });
}

#[test]
#[ignore = "daily"]
fn item_block_use_distinguishes_snapshots() {
    with_guests(|g| {
        install(g);
        let p = g.player(12).unwrap();
        let key = mod_api::__private::item_key("minecraft:stone").unwrap();
        let original = mod_api::ItemStack::new(key, 2).unwrap();
        g.lan
            .as_mut()
            .unwrap()
            .hub
            .update_guest(12, |g| {
                g.inventory[0] = Some(EngineStack::new("minecraft:dirt", 1))
            })
            .unwrap();
        assert_eq!(
            p.trigger_item_used_on_block(g, [0, 64, 0], &original),
            Ok(())
        );
        assert!(has(g, "item2"));
        assert!(!has(g, "any1"));
        assert!(!has(g, "any"));
        assert!(!has(g, "any2"));
        assert!(!has(g, "default"));
    });
}

#[test]
#[ignore = "daily"]
fn block_use_refusals_grant_nothing() {
    with_guests(|g| {
        install(g);
        let p = g.player(12).unwrap();
        let invalid = mod_api::ItemStack::new(
            mod_api::__private::item_key("minecraft:stone").unwrap(),
            999,
        )
        .unwrap();
        assert_eq!(
            p.trigger_item_used_on_block(g, [0, 64, 0], &invalid),
            Err(TriggerCriterionError::Stack(
                mod_api::StackWriteError::CountExceeded { maximum: 64 }
            ))
        );
        struct Never;
        impl CriterionTest for Never {
            fn test(&mut self, _: &dyn CriterionContext) -> Result<bool, CriterionTestError> {
                panic!("vanilla guard must precede callback");
            }
        }
        assert_eq!(
            p.trigger_criterion(
                g,
                &Identifier::parse("minecraft:any_block_use").unwrap(),
                &mut Never
            ),
            Err(TriggerCriterionError::BuiltinTrigger)
        );
        let b = g.player(13).unwrap();
        g.as_guest(12, |g| {
            g.as_guest(13, |g| {
                assert_eq!(
                    p.trigger_block_use(g, [0, 64, 0]),
                    Err(TriggerCriterionError::PlayerUnavailable)
                );
                assert_eq!(b.trigger_block_use(g, [0, 64, 0]), Ok(()));
            })
            .unwrap();
        })
        .unwrap();
        let stale = mod_api::__private::game::player_ref(p.entity(), 999);
        assert_eq!(
            stale.trigger_block_use(g, [0, 64, 0]),
            Err(TriggerCriterionError::PlayerUnavailable)
        );
        for id in ["default", "item2", "any2", "any1", "any"] {
            assert!(!has(g, id));
        }
    });
}

#[test]
#[ignore = "daily"]
fn unloaded_block_use_only_fails_predicates_that_need_the_world() {
    with_guests(|g| {
        install(g);
        let p = g.player(12).unwrap();
        let at = [100000, 64, 100000];
        let key = mod_api::__private::item_key("minecraft:stone").unwrap();
        let original = mod_api::ItemStack::new(key, 2).unwrap();
        assert_eq!(p.trigger_block_use(g, at), Ok(()));
        assert!(has(g, "default_unconditional"));
        assert!(!has(g, "position_requires_chunk"));
        assert_eq!(p.trigger_item_used_on_block(g, at, &original), Ok(()));
        assert!(has(g, "item2"));
    });
}

#[test]
#[ignore = "daily"]
fn custom_trigger_borrows_conditions_and_uses_the_target_players_predicate() {
    use mod_api::ItemStack;

    struct Pipe<'a> {
        stack: &'a ItemStack,
        counts: &'a mut Vec<u64>,
    }
    impl CriterionTest for Pipe<'_> {
        fn test(&mut self, criterion: &dyn CriterionContext) -> Result<bool, CriterionTestError> {
            let count = criterion.conditions()["minimum"].as_u64().unwrap();
            self.counts.push(count);
            criterion.item("pipe", self.stack)
        }
    }

    with_guests(|g| {
        install(g);
        let dir = tempfile::tempdir().unwrap();
        let advancements = dir.path().join("test/advancement");
        std::fs::create_dir_all(&advancements).unwrap();
        for (id, player) in [
            ("pipe", serde_json::Value::Null),
            (
                "pipe_burning",
                json!({
                    "condition": "minecraft:entity_properties", "entity": "this",
                    "predicate": {"flags": {"is_on_fire": true}}
                }),
            ),
        ] {
            std::fs::write(
                advancements.join(format!("{id}.json")),
                json!({"criteria":{"seen":{"trigger":"minecraft:test_pipe_connected",
                    "conditions":{"minimum":2,"pipe":{"items":"minecraft:stone","count":{"min":2}},"player":player}}}}).to_string(),
            ).unwrap();
        }
        g.adv.tree = Arc::new(a::Tree::load(dir.path()));
        let p = g.player(12).unwrap();
        let trigger = Identifier::parse("minecraft:test_pipe_connected").unwrap();
        let stone = mod_api::__private::item_key("minecraft:stone").unwrap();
        let mut counts = Vec::new();
        assert_eq!(
            p.trigger_criterion(
                g,
                &trigger,
                &mut Pipe {
                    stack: &ItemStack::new(stone, 1).unwrap(),
                    counts: &mut counts,
                }
            ),
            Ok(())
        );
        assert!(!has(g, "pipe"));
        assert_eq!(
            p.trigger_criterion(
                g,
                &trigger,
                &mut Pipe {
                    stack: &ItemStack::new(stone, 2).unwrap(),
                    counts: &mut counts,
                }
            ),
            Ok(())
        );
        assert!(has(g, "pipe"));
        assert!(!has(g, "pipe_burning"));
        assert_eq!(counts, [2, 2, 2, 2]);
    });
}

#[test]
#[ignore = "daily"]
fn remote_block_use_refuses() {
    crate::game::native_entity_refs::tests::with_client_game(|g| {
        let (pipe, _server) = sunrise_protocol::connection::Pipe::pair();
        g.connect_local(pipe, "CriteriaClient").unwrap();
        let p = mod_api::__private::game::player_ref(
            mod_api::EntityRef::new(mod_api::WorldEpoch::new(1), 1),
            12,
        );
        assert_eq!(
            p.trigger_block_use(g, [0, 64, 0]),
            Err(TriggerCriterionError::NotServer)
        );
    });
}

#[test]
#[ignore = "daily"]
fn consume_item_uses_supplied_snapshot_and_refuses_invalid_inputs() {
    crate::game::native_entity_refs::tests::with_client_game(|g| {
        let _server = crate::game::native_client_session::tests::logged_client(g);
        let p = mod_api::__private::game::player_ref(
            mod_api::EntityRef::new(mod_api::WorldEpoch::new(1), 7),
            12,
        );
        let key = mod_api::__private::item_key("minecraft:stone").unwrap();
        let stack = mod_api::ItemStack::new(key, 1).unwrap();
        assert_eq!(
            p.trigger_consume_item(g, &stack),
            Err(TriggerCriterionError::NotServer)
        );
    });
    mod_platform::testing::with_mods(Vec::new().into(), || {
        with_guests(|g| {
            install(g);
            let p = g.player(12).unwrap();
            let key = mod_api::__private::item_key("minecraft:stone").unwrap();
            let one = mod_api::ItemStack::new(key, 1).unwrap();
            assert_eq!(p.trigger_consume_item(g, &one), Ok(()));
            assert!(!has(g, "consume"));
            let invalid = mod_api::ItemStack::new(key, 999).unwrap();
            assert_eq!(
                p.trigger_consume_item(g, &invalid),
                Err(TriggerCriterionError::Stack(
                    mod_api::StackWriteError::CountExceeded { maximum: 64 }
                ))
            );
            assert!(!has(g, "consume"));
            let two = mod_api::ItemStack::new(key, 2).unwrap();
            assert_eq!(p.trigger_consume_item(g, &two), Ok(()));
            assert!(has(g, "consume"));
            assert!(!has(g, "any"));
            g.as_guest(12, |g| {
                g.as_guest(13, |g| {
                    assert_eq!(
                        p.trigger_consume_item(g, &two),
                        Err(TriggerCriterionError::PlayerUnavailable)
                    );
                })
                .unwrap();
            })
            .unwrap();
            let stale = mod_api::__private::game::player_ref(p.entity(), 999);
            assert_eq!(
                stale.trigger_consume_item(g, &two),
                Err(TriggerCriterionError::PlayerUnavailable)
            );
        });
    });
}
