use super::*;
use crate::game::native_entity_refs::tests::{bind_local_player, run_fresh_cases, with_game_at};
use mod_api::{
    BlockUpdateFlags, CauldronDecision, CauldronUse, GameEventContext, Hooks, ItemPlace,
    ItemResource, ServerHost, SoundSource, StatType,
};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use sunrise_client::cauldron::{Outcome, interactions::Entry};

const POS: [i32; 3] = [0, 64, 0];
#[derive(Default)]
struct Seen {
    fail_preparation: AtomicBool,
    calls: AtomicUsize,
    actor: AtomicUsize,
    drops: AtomicUsize,
}
struct Syrup {
    seen: Arc<Seen>,
}
impl mod_api::Interaction for Syrup {
    fn interact(&self, host: &mut dyn ServerHost, input: &CauldronUse) -> CauldronDecision {
        use mod_api::HostTransaction as _;
        self.seen.calls.fetch_add(1, Ordering::SeqCst);
        self.seen
            .actor
            .store(input.player.entity().id() as usize, Ordering::SeqCst);
        if input.player.entity().id() == 7 {
            assert_eq!(host.entity(7), Some(input.player.entity()));
        }
        let block = host.block_state_id("test:syrup_cauldron[level=1]").unwrap();
        let bottle = mod_api::ItemStack::new(
            mod_api::__private::item_key("minecraft:glass_bottle").unwrap(),
            1,
        )
        .unwrap();
        let hand = ItemPlace::PlayerHand {
            player: input.player.uuid(),
            hand: input.hand,
        };
        let resource = ItemResource::of(&bottle);
        host.transaction(|host, tx| {
            input
                .level
                .set_block(
                    host,
                    input.pos,
                    block,
                    BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS,
                )
                .map_err(|_| ())?;
            if hand.exchange(host, tx, &resource, 1) != 1 {
                return Err(());
            }
            Ok(())
        })
        .unwrap();
        input
            .player
            .award_stat(host, StatType::Custom, "minecraft:use_cauldron", 1);
        input
            .player
            .award_stat(host, StatType::Used, "test:syrup_bottle", 1);
        let at = input.pos.map(|v| f64::from(v) + 0.5);
        input.level.play_sound(
            host,
            "minecraft:item.bottle.empty",
            at,
            SoundSource::Blocks,
            1.0,
            1.0,
            None,
        );
        input.level.game_event(
            host,
            "minecraft:fluid_place",
            at,
            GameEventContext::default(),
        );
        CauldronDecision::Success
    }
}
struct TestMod {
    seen: Arc<Seen>,
    dye: bool,
}
#[mod_api::__subscriptions]
impl Hooks for TestMod {
    fn blocks(&self, _: &mod_api::RegistrationContext, event: &mut mod_api::BlocksEvent<'_>) {
        event
            .register(mod_api::Block::new(
                "test:syrup_cauldron",
                "minecraft:water_cauldron",
            ))
            .unwrap();
    }
    fn items(&self, _: &mod_api::RegistrationContext, event: &mut mod_api::ItemsEvent<'_>) {
        event
            .register(mod_api::Item::new(
                "test:syrup_bottle",
                "minecraft:glass_bottle",
            ))
            .unwrap();
    }
    fn register_cauldron_interaction(
        &self,
        _: &mod_api::RegistrationContext,
        event: &mut mod_api::RegisterCauldronInteractionEvent<'_>,
    ) {
        let handler: Arc<dyn mod_api::Interaction> = Arc::new(Syrup {
            seen: self.seen.clone(),
        });
        let item = mod_api::__private::item_key(if self.dye {
            "minecraft:leather_helmet"
        } else {
            "test:syrup_bottle"
        })
        .unwrap();
        let block = mod_api::__private::block_key(if self.dye {
            "minecraft:water_cauldron"
        } else {
            "minecraft:cauldron"
        })
        .unwrap();
        event
            .interactions
            .register(block, item, handler.clone())
            .unwrap();
        if self.seen.fail_preparation.load(Ordering::SeqCst) {
            let _ = event
                .interactions
                .register_tag_to_all("invalid tag", handler.clone());
        }
        event
            .interactions
            .register(
                mod_api::__private::block_key("test:syrup_cauldron").unwrap(),
                mod_api::__private::item_key("test:syrup_bottle").unwrap(),
                handler,
            )
            .unwrap();
    }
    fn item_spawn(&self, _: &mut dyn ServerHost, _: &mut mod_api::ItemSpawnEvent<'_>) {
        self.seen.drops.fetch_add(1, Ordering::SeqCst);
    }
}
#[test]
fn a_panicking_cauldron_registrar_leaves_no_exact_tag_or_all_entries() {
    use mod_api::{ModEntry, RegistrationContext};
    use sunrise_mod_runtime::block_definitions::BlockDefinition;
    use sunrise_world::{loot::PackOverlay, registry::BlockRegistry};

    struct Registrant(Option<Arc<AtomicBool>>);
    #[mod_api::__subscriptions]
    impl Hooks for Registrant {
        fn register_cauldron_interaction(
            &self,
            _: &RegistrationContext,
            event: &mut mod_api::RegisterCauldronInteractionEvent<'_>,
        ) {
            let block = mod_api::__private::block_key("minecraft:water_cauldron").unwrap();
            let item = |id| mod_api::__private::item_key(id).unwrap();
            let handler: Arc<dyn mod_api::Interaction> = Arc::new(Syrup {
                seen: Arc::new(Seen::default()),
            });
            let interactions = &mut *event.interactions;
            let Some(added) = &self.0 else {
                interactions
                    .register(block, item("test:winner"), handler)
                    .unwrap();
                return;
            };
            interactions
                .register(block, item("test:exact"), handler.clone())
                .unwrap();
            interactions
                .register_tag(block, "test:tag", handler.clone())
                .unwrap();
            interactions
                .register_to_all(item("test:all"), handler.clone())
                .unwrap();
            interactions
                .register_tag_to_all("test:all_tag", handler.clone())
                .unwrap();
            // Overwriting a built-in key or an earlier tag in the same batch succeeds in place.
            assert_eq!(
                interactions.register(block, item("minecraft:water_bucket"), handler.clone()),
                Ok(()),
            );
            assert_eq!(
                interactions.register_tag(block, "#test:tag", handler),
                Ok(())
            );
            added.store(true, Ordering::Relaxed);
            panic!("after registering exact, tag and all-dispatcher entries");
        }
    }

    let added = Arc::new(AtomicBool::new(false));
    mod_platform::testing::with_mods(
        vec![
            ModEntry::new("failed", Box::new(Registrant(Some(added.clone())))),
            ModEntry::new("healthy", Box::new(Registrant(None))),
        ]
        .into(),
        || {
            crate::game::api_values::install_parts(&[]).unwrap();
            mod_api::__private::resolve_block_keys(["minecraft:water_cauldron"], |_| {
                BlockDefinition::new(&[&Default::default()], 0).map_err(|err| format!("{err:?}"))
            })
            .unwrap();
            let ids = [
                "test:exact",
                "test:all",
                "test:winner",
                "minecraft:water_bucket",
            ];
            mod_api::__private::resolve_items(ids).unwrap();
            let root = tempfile::tempdir().unwrap();
            let item_json = ids
                .map(|id| (id, serde_json::json!({})))
                .into_iter()
                .collect::<std::collections::BTreeMap<_, _>>();
            std::fs::write(
                root.path().join("items.json"),
                serde_json::to_vec(&item_json).unwrap(),
            )
            .unwrap();
            let defaults = sunrise_world::item_defaults::read(root.path()).unwrap();
            let items = Items::catalog(root.path(), &PackOverlay::NONE, &defaults).unwrap();
            let registry = BlockRegistry::from_facts_json(
                &br#"{
                "defaults":{"minecraft:water_cauldron":0},
                "states":[["water_cauldron",false,false,"","minecraft:water_cauldron"]]
            }"#[..],
            )
            .unwrap();
            let mut registrar = Registrar::new(&items, &registry);
            Game::native_register_cauldron_interaction(&RegistrationContext::new(), &mut registrar);
            assert!(
                added.load(Ordering::Relaxed),
                "the failed listener reached the intended panic"
            );
            let table = registrar.finish().unwrap();
            let water = "minecraft:water_cauldron";
            assert!(
                table
                    .select(water, "test:exact", false, |_| false)
                    .is_none()
            );
            assert!(
                table
                    .select(water, "test:other", false, |tag| tag == "test:tag")
                    .is_none()
            );
            for block in ["minecraft:cauldron", water] {
                assert!(table.select(block, "test:all", false, |_| false).is_none());
                assert!(
                    table
                        .select(block, "test:other", false, |tag| tag == "test:all_tag")
                        .is_none()
                );
            }
            assert!(
                matches!(table.select(water, "test:winner", false, |_| false), Some(Entry::Mod(found)) if found.owner == "healthy")
            );
            assert!(matches!(
                table.select(water, "minecraft:water_bucket", false, |_| false),
                Some(Entry::Vanilla(sunrise_client::cauldron::Action::EmptyWater))
            ));
        },
    );
}

fn fixture(dye: bool, test: impl FnOnce(&mut Game<'_>, &Arc<Seen>)) {
    let seen = Arc::new(Seen::default());
    let mods: mod_api::Mods = vec![mod_api::ModEntry::new(
        "test",
        Box::new(TestMod {
            seen: seen.clone(),
            dye,
        }),
    )]
    .into();
    mod_platform::__private::install(mods.clone());
    crate::game::api_values::install_parts(&mods).unwrap();
    let base = std::path::PathBuf::from(std::env::var_os("SUNRISE_TEST_RESOURCES").unwrap());
    let root = crate::game::mods::content_root(&base, &Default::default())
        .unwrap()
        .prepared()
        .to_path_buf();
    with_game_at(root, |game| {
        setup(game, "minecraft:cauldron", None, "test:syrup_bottle", 2);
        test(game, &seen);
    });
}
fn setup(game: &mut Game<'_>, block: &str, level: Option<&str>, item: &str, count: u32) {
    game.world
        .chunks
        .insert((0, 0), Arc::new(sunrise_client::chunk::Chunk::empty(0, 0)));
    let state = sunrise_world::chunk::BlockState {
        name: block.into(),
        properties: level
            .map(|v| ("level".into(), v.into()))
            .into_iter()
            .collect(),
    };
    let id = game.pack.assets.registry.resolve(&state).unwrap();
    game.world.set(0, 64, 0, id).unwrap();
    game.player = Some(
        sunrise_client::motion::PlayerMotion::new(game.motion, sunrise_core::geometry::Vec3::ZERO)
            .unwrap(),
    );
    if game.remote.is_none() {
        bind_local_player(game, 7, 123);
    } else {
        game.lan_acting_entity = None;
        game.lan_acting_uuid = None;
    }
    game.creative = false;
    game.selected = 0;
    game.hand = Hand::Main;
    game.inventory.fill(None);
    game.inventory[0] = Some(ItemStack::new(item, count));
    game.creatures.lan_sounds.clear();
    game.audio.capture = Some(Vec::new());
    crate::game::sculk::TEST_POSTED_EVENTS.with(|events| *events.borrow_mut() = Some(Vec::new()));
}
fn state(game: &Game<'_>) -> (String, Option<String>) {
    let state = game
        .pack
        .assets
        .registry
        .state(game.world.block(0, 64, 0))
        .unwrap();
    (state.name.clone(), state.properties.get("level").cloned())
}
fn events() -> usize {
    crate::game::sculk::TEST_POSTED_EVENTS.with(|events| events.borrow().as_ref().unwrap().len())
}
fn unchanged(game: &Game<'_>, seen: &Seen) {
    assert_eq!(state(game), ("minecraft:cauldron".into(), None));
    let held = game.inventory[0].as_ref().unwrap();
    assert_eq!((&*held.item, held.count), ("test:syrup_bottle", 2));
    assert_eq!(seen.drops.load(Ordering::SeqCst), 0);
    assert_eq!(game.dropped.len(), 0);
    assert_eq!(
        game.inventory
            .iter()
            .flatten()
            .filter(|stack| &*stack.item == "minecraft:glass_bottle")
            .count(),
        0
    );
}
#[test]
#[ignore = "daily"]
fn cauldron_syrup_applies_once() {
    if run_fresh_cases(
        "game::cauldron_interactions::tests::cauldron_syrup_applies_once",
        "SUNRISE_CAULDRON_CASE",
        &["run"],
    ) {
        return;
    }
    fixture(false, |game, seen| {
        crate::game::native_item_places::tests::open_to_lan(game);
        assert_eq!(game.use_on_cauldron(POS).unwrap(), Outcome::Success);
        assert_eq!(seen.calls.load(Ordering::SeqCst), 1);
        assert_eq!(seen.actor.load(Ordering::SeqCst), 7);
        assert_eq!(game.inventory[0].as_ref().unwrap().count, 1);
        assert_eq!(
            state(game),
            ("test:syrup_cauldron".into(), Some("1".into()))
        );
        assert_eq!(
            game.inventory
                .iter()
                .flatten()
                .filter(|s| &*s.item == "minecraft:glass_bottle")
                .map(|s| s.count)
                .sum::<u32>(),
            1
        );
        assert!(
            game.creatures
                .lan_sounds
                .iter()
                .any(|sound| sound.sound == "minecraft:item.bottle.empty")
        );
        assert_eq!(events(), 1);
        crate::game::sculk::TEST_POSTED_EVENTS.with(|events| {
            assert_eq!(
                events.borrow().as_ref().unwrap()[0].0,
                "minecraft:fluid_place"
            )
        });
        assert_eq!(seen.drops.load(Ordering::SeqCst), 0);
    });
}
#[test]
#[ignore = "daily"]
fn cauldron_outer_abort_discards_effects_and_drops() {
    if run_fresh_cases(
        "game::cauldron_interactions::tests::cauldron_outer_abort_discards_effects_and_drops",
        "SUNRISE_CAULDRON_CASE",
        &["run"],
    ) {
        return;
    }
    fixture(false, |game, seen| {
        for slot in game.inventory.iter_mut().skip(1) {
            *slot = Some(ItemStack::new("minecraft:stone", 64));
        }
        game.storage_scope(|game, _| {
            assert_eq!(game.use_on_cauldron(POS).unwrap(), Outcome::Success);
            ((), false)
        });
        unchanged(game, seen);
    });
}
#[test]
#[ignore = "daily"]
fn cauldron_dye_tag_shadows_exact_handler_even_on_pass() {
    if run_fresh_cases(
        "game::cauldron_interactions::tests::cauldron_dye_tag_shadows_exact_handler_even_on_pass",
        "SUNRISE_CAULDRON_CASE",
        &["run"],
    ) {
        return;
    }
    fixture(true, |game, seen| {
        setup(
            game,
            "minecraft:water_cauldron",
            Some("2"),
            "minecraft:leather_helmet",
            1,
        );
        game.inventory[0].as_mut().unwrap().data_mut().dyed_color = Some(0x123456);
        assert_eq!(game.use_on_cauldron(POS).unwrap(), Outcome::Success);
        assert_eq!(
            state(game),
            ("minecraft:water_cauldron".into(), Some("1".into()))
        );
        assert_eq!(
            game.inventory[0]
                .as_ref()
                .unwrap()
                .data()
                .and_then(|d| d.dyed_color),
            None
        );
        assert_eq!(game.use_on_cauldron(POS).unwrap(), Outcome::Pass);
        assert_eq!(seen.calls.load(Ordering::SeqCst), 0);
    });
}
#[test]
#[ignore = "daily"]
fn cauldron_transfer_awards_two_statistics_once() {
    if run_fresh_cases(
        "game::cauldron_interactions::tests::cauldron_transfer_awards_two_statistics_once",
        "SUNRISE_CAULDRON_CASE",
        &["run"],
    ) {
        return;
    }
    fixture(false, |game, _| {
        assert_eq!(game.use_on_cauldron(POS).unwrap(), Outcome::Success);
        assert_eq!(game.stats.get("custom", "minecraft:use_cauldron"), 1);
        assert_eq!(game.stats.get("used", "test:syrup_bottle"), 1);
    });
}
#[test]
#[ignore = "daily"]
fn cauldron_client_skips_custom_prediction() {
    if run_fresh_cases(
        "game::cauldron_interactions::tests::cauldron_client_skips_custom_prediction",
        "SUNRISE_CAULDRON_CASE",
        &["run"],
    ) {
        return;
    }
    fixture(false, |game, seen| {
        game.options.execution_role = crate::game::execution_role::ExecutionRole::Client;
        let _server = crate::game::native_client_session::tests::logged_client(game);
        setup(game, "minecraft:cauldron", None, "test:syrup_bottle", 2);
        let held = game.inventory[0].clone();
        let (outcome, proposed) = game.client_cauldron_prediction(POS, held.as_ref());
        assert_eq!(outcome, Outcome::Consume);
        assert!(proposed.0.is_empty());
        assert!(proposed.1.is_empty());
        assert_eq!(seen.calls.load(Ordering::SeqCst), 0);
        assert_eq!(game.cauldron_outcome(POS, held.as_ref()), Outcome::Consume);
        assert_eq!(seen.calls.load(Ordering::SeqCst), 0);
        let hit = Hit {
            pos: POS,
            face: [0, 1, 0],
            point: [0.5, 65.0, 0.5],
            inside: false,
        };
        let (answer, _) = game.remote_cauldron_reacts(&hit, held.as_ref()).unwrap();
        assert!(answer.consumes_action());
        assert_eq!(seen.calls.load(Ordering::SeqCst), 0);
        assert_eq!(seen.actor.load(Ordering::SeqCst), 0);
        unchanged(game, seen);
        assert_eq!(game.audio.capture.as_ref().unwrap().len(), 0);
        assert_eq!(events(), 0);
        setup(
            game,
            "minecraft:cauldron",
            None,
            "minecraft:water_bucket",
            1,
        );
        let held = game.inventory[0].clone();
        let (outcome, proposed) = game.client_cauldron_prediction(POS, held.as_ref());
        assert_eq!(outcome, Outcome::Success);
        assert_eq!(proposed.0.len(), 1);
        assert_eq!(seen.calls.load(Ordering::SeqCst), 0);
    });
}

#[test]
#[ignore = "daily"]
fn cauldron_register_to_all_and_admission_errors() {
    if run_fresh_cases(
        "game::cauldron_interactions::tests::cauldron_register_to_all_and_admission_errors",
        "SUNRISE_CAULDRON_CASE",
        &["run"],
    ) {
        return;
    }
    fixture(false, |game, seen| {
        use mod_api::CauldronInteractions as _;
        let handler: Arc<dyn mod_api::Interaction> = Arc::new(Syrup { seen: seen.clone() });
        let item = mod_api::__private::item_key("test:syrup_bottle").unwrap();
        let custom = mod_api::__private::block_key("test:syrup_cauldron").unwrap();
        let make = || Registrar::new(game.items(), game.pack.assets.registry);
        let _owner = mod_api::__private::enter("all_mod", "register_cauldron_interaction");
        let mut registrar = make();
        registrar.register_to_all(item, handler.clone()).unwrap();
        // The custom dispatcher is declared after register_to_all was queued.
        registrar
            .register(
                custom,
                mod_api::__private::item_key("minecraft:stick").unwrap(),
                handler.clone(),
            )
            .unwrap();
        let table = registrar.finish().unwrap();
        assert_eq!(table.blocks().len(), 5);
        for block in [
            "minecraft:cauldron",
            "minecraft:water_cauldron",
            "minecraft:lava_cauldron",
            "minecraft:powder_snow_cauldron",
            "test:syrup_cauldron",
        ] {
            assert!(
                matches!(table.select(block,"test:syrup_bottle",false,|_| false),Some(Entry::Mod(callback)) if callback.owner=="all_mod")
            );
        }
        let mut registrar = make();
        assert_eq!(
            registrar.register_to_all(
                mod_api::__private::item_key("minecraft:water_bucket").unwrap(),
                handler.clone()
            ),
            Ok(())
        );
        let table = registrar.finish().unwrap();
        for block in [
            "minecraft:cauldron",
            "minecraft:water_cauldron",
            "minecraft:lava_cauldron",
            "minecraft:powder_snow_cauldron",
        ] {
            assert!(
                matches!(
                    table.select(block, "minecraft:water_bucket", false, |_| false),
                    Some(Entry::Mod(callback)) if callback.owner == "all_mod"
                ),
                "{block}"
            );
        }
        let mut items = game.items().clone();
        items.items.remove("test:syrup_bottle");
        let mut registrar = Registrar::new(&items, game.pack.assets.registry);
        assert_eq!(
            registrar.register_to_all(item, handler.clone()),
            Err(mod_api::CauldronRegistrationError::UnknownItem)
        );
        assert!(
            registrar
                .finish()
                .err()
                .unwrap()
                .to_string()
                .contains("UnknownItem")
        );
        let mut registrar = Registrar::new(&items, game.pack.assets.registry);
        assert_eq!(
            registrar.register(custom, item, handler.clone()),
            Err(mod_api::CauldronRegistrationError::UnknownItem)
        );
        assert!(
            registrar
                .finish()
                .err()
                .unwrap()
                .to_string()
                .contains("UnknownItem")
        );
        let registry = sunrise_world::registry::BlockRegistry::from_facts_json(
            &br#"{"defaults":{"minecraft:air":0},"states":[["air",true,false,"","minecraft:air"]]}"#[..],
        )
        .unwrap();
        let mut registrar = Registrar::new(game.items(), &registry);
        assert_eq!(
            registrar.register(custom, item, handler),
            Err(mod_api::CauldronRegistrationError::UnknownBlock)
        );
        assert!(
            registrar
                .finish()
                .err()
                .unwrap()
                .to_string()
                .contains("UnknownBlock")
        );
    });
}

#[test]
#[ignore = "daily"]
fn cauldron_world_close_discards_callbacks_before_failed_preparation() {
    if run_fresh_cases(
        "game::cauldron_interactions::tests::cauldron_world_close_discards_callbacks_before_failed_preparation",
        "SUNRISE_CAULDRON_CASE",
        &["run"],
    ) {
        return;
    }
    fixture(false, |game, seen| {
        let old = snapshot();
        assert!(matches!(
            old.select("minecraft:cauldron", "test:syrup_bottle", false, |_| false),
            Some(Entry::Mod(_))
        ));
        seen.fail_preparation.store(true, Ordering::SeqCst);
        assert!(
            game.resolve_parts()
                .unwrap_err()
                .to_string()
                .contains("InvalidTag")
        );
        assert!(
            Arc::ptr_eq(&old, &snapshot()),
            "failed active reload retains its snapshot"
        );
        game.native_lifecycle_world_closed();
        game.native_lifecycle
            .begin_open(mod_api::Authority::IntegratedServer);
        assert!(
            game.resolve_parts()
                .unwrap_err()
                .to_string()
                .contains("InvalidTag")
        );
        assert_eq!(snapshot().blocks().len(), 4);
        for block in [
            "minecraft:cauldron",
            "minecraft:water_cauldron",
            "minecraft:lava_cauldron",
            "minecraft:powder_snow_cauldron",
            "test:syrup_cauldron",
        ] {
            assert!(
                snapshot()
                    .select(block, "test:syrup_bottle", false, |_| false)
                    .is_none()
            );
        }
        game.inventory[0] = Some(ItemStack::new("minecraft:water_bucket", 1));
        assert_eq!(game.use_on_cauldron(POS).unwrap(), Outcome::Success);
        assert_eq!(
            state(game),
            ("minecraft:water_cauldron".into(), Some("3".into()))
        );
        assert_eq!(
            &*game.inventory[0].as_ref().unwrap().item,
            "minecraft:bucket"
        );
        assert_eq!(seen.calls.load(Ordering::SeqCst), 0);
    });
}

#[test]
#[ignore = "daily"]
fn cauldron_tag_registration_normalizes_admits_and_expands_over_the_complete_set() {
    if run_fresh_cases(
        "game::cauldron_interactions::tests::cauldron_tag_registration_normalizes_admits_and_expands_over_the_complete_set",
        "SUNRISE_CAULDRON_CASE",
        &["run"],
    ) {
        return;
    }
    fixture(false, |game, seen| {
        use mod_api::CauldronInteractions as _;
        use mod_api::CauldronRegistrationError as Error;
        let handler: Arc<dyn mod_api::Interaction> = Arc::new(Syrup { seen: seen.clone() });
        let custom = mod_api::__private::block_key("test:syrup_cauldron").unwrap();
        let make = || Registrar::new(game.items(), game.pack.assets.registry);
        let _owner = mod_api::__private::enter("tag_mod", "register_cauldron_interaction");
        // The leading `#` is dropped, and the custom dispatcher is declared after the
        // register_tag_to_all that expands over it.
        let mut registrar = make();
        registrar
            .register_tag_to_all("#c:buckets/milk", handler.clone())
            .unwrap();
        registrar
            .register_tag(custom, "c:foods", handler.clone())
            .unwrap();
        let table = registrar.finish().unwrap();
        assert_eq!(table.blocks().len(), 5);
        for block in [
            "minecraft:cauldron",
            "minecraft:water_cauldron",
            "minecraft:lava_cauldron",
            "minecraft:powder_snow_cauldron",
            "test:syrup_cauldron",
        ] {
            assert!(
                matches!(
                    table.select(block, "test:anything", false, |tag| tag == "c:buckets/milk"),
                    Some(Entry::Mod(callback)) if callback.owner == "tag_mod"
                ),
                "{block}"
            );
            assert!(
                table
                    .select(block, "test:anything", false, |_| false)
                    .is_none()
            );
        }
        assert!(matches!(
            table.select("test:syrup_cauldron", "test:anything", false, |tag| tag
                == "c:foods"),
            Some(Entry::Mod(_))
        ));
        assert!(
            table
                .select("minecraft:cauldron", "test:anything", false, |tag| tag
                    == "c:foods")
                .is_none()
        );
        // Admission: a string that is no resource location is refused, and fails preparation.
        let mut registrar = make();
        assert_eq!(
            registrar.register_tag(custom, "no tag here", handler.clone()),
            Err(Error::InvalidTag)
        );
        assert_eq!(
            registrar.register_tag_to_all("#no tag here", handler.clone()),
            Err(Error::InvalidTag)
        );
        assert!(
            registrar
                .finish()
                .err()
                .unwrap()
                .to_string()
                .contains("InvalidTag")
        );
        // Registering the same tag again (with or without `#`) overwrites in place.
        let mut registrar = make();
        registrar
            .register_tag(custom, "c:foods", handler.clone())
            .unwrap();
        {
            let _second = mod_api::__private::enter("second_mod", "register_cauldron_interaction");
            registrar
                .register_tag(custom, "#c:foods", handler.clone())
                .unwrap();
        }
        let table = registrar.finish().unwrap();
        assert!(matches!(
            table.select("test:syrup_cauldron", "test:anything", false, |tag| tag
                == "c:foods"),
            Some(Entry::Mod(callback)) if callback.owner == "second_mod"
        ));
        // Expansion via register_tag_to_all also overwrites an existing tag entry in place.
        let mut registrar = make();
        registrar
            .register_tag(custom, "c:foods", handler.clone())
            .unwrap();
        {
            let _second = mod_api::__private::enter("second_mod", "register_cauldron_interaction");
            registrar.register_tag_to_all("c:foods", handler).unwrap();
        }
        let table = registrar.finish().unwrap();
        assert!(matches!(
            table.select("test:syrup_cauldron", "test:anything", false, |tag| tag
                == "c:foods"),
            Some(Entry::Mod(callback)) if callback.owner == "second_mod"
        ));
    });
}
