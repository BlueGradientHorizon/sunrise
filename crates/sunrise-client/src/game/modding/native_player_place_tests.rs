//! `player.place_block` against real LAN guests, through the public player handle, with a
//! literal test for each refusal.

use super::native_entity_refs::tests::with_client_game;
use super::native_guest_turn_tests::with_guests;
use mod_api::{BlockFace, EntityRef, PlayerWorldRefusal as Refusal, WorldEpoch};

/// The API's value for a block state of the world.
fn api_state(game: &super::Game<'_>, name: &str) -> mod_api::BlockState {
    use super::api_values::TryIntoApi;
    game.pack
        .assets
        .registry
        .default_state(name)
        .unwrap()
        .try_into_api()
        .unwrap()
}

fn loaded(game: &mut super::Game<'_>) {
    // The write is `level_set_block`'s, which an authoritative server only takes.
    game.options.dedicated = Some(Vec::new());
    game.world.chunks.insert(
        (0, 0),
        std::sync::Arc::new(sunrise_client::chunk::Chunk::empty(0, 0)),
    );
}

/// A bed written for a player gets its head (`setPlacedBy`).
#[test]
#[ignore = "daily"]
fn a_block_placed_as_a_player_runs_set_placed_by() {
    with_guests(|game| {
        loaded(game);
        let b = game.player(13).unwrap();
        let level = game.level(game.dim.id()).unwrap();
        let bed = game
            .pack
            .assets
            .registry
            .default_state("minecraft:red_bed")
            .unwrap();
        let api_bed = api_state(game, "minecraft:red_bed");
        assert_eq!(
            b.place_block(game, level, [2, 70, 2], api_bed, BlockFace::Up, true),
            Ok(())
        );
        assert_eq!(game.world.block(2, 70, 2), bed);
        let head = game.world.block(2, 70, 1);
        let facts = game.res.facts();
        assert_eq!(
            facts.get(head).name,
            facts.get(bed).name,
            "the head was placed"
        );
    });
}

#[test]
#[ignore = "daily"]
fn chosen_placement_attributes_the_player_and_rolls_back_on_refusal() {
    use super::native_entity_refs::tests::run_fresh_cases;
    use mod_api::{
        BlockEntityRef, BlockState, BlockUpdateFlags, EntityPlaceEvent, Flow, Hooks, LivingRef,
        ModEntry, Placement, ServerHost, run_transaction,
    };
    use mod_platform::testing::with_mods;
    use std::sync::{Arc, Mutex};
    use sunrise_client::{containers::BlockEntity, items::ItemStack};
    use sunrise_core::{nbt::Tag, position::BlockPos};

    const FLAG: &str = "SUNRISE_TEST_PLAYER_PLACE";
    const AT: [i32; 3] = [2, 70, 2];
    const HEAD: [i32; 3] = [2, 70, 1];
    const AGAINST: [i32; 3] = [1, 70, 2];
    const WRITE: [i32; 3] = [4, 70, 2];
    const OUTER: [i32; 3] = [5, 70, 2];
    const NAME: &str = "The placed chest";
    if run_fresh_cases(
        "game::native_player_place_tests::chosen_placement_attributes_the_player_and_rolls_back_on_refusal",
        FLAG,
        &[
            "chest-accept",
            "chest-veto",
            "chest-replace",
            "bed-veto",
            "bed-stale",
            "bed-against",
            "bed-unwind",
            "bed-death",
        ],
    ) {
        return;
    }
    type PlacementSeen = (
        Option<EntityRef>,
        BlockPos,
        BlockState,
        BlockState,
        Placement,
    );
    struct Probe {
        case: String,
        seen: Arc<Mutex<Vec<PlacementSeen>>>,
        entity: Arc<Mutex<Option<BlockEntityRef>>>,
    }
    #[mod_api::__subscriptions]
    impl Hooks for Probe {
        fn entity_place(&self, host: &mut dyn ServerHost, event: &mut EntityPlaceEvent) -> Flow {
            let level = host.level("minecraft:overworld").unwrap();
            assert_eq!(level.block_state(host, AT), Some(event.state));
            self.seen.lock().unwrap().push((
                event.placer,
                event.pos,
                event.state,
                event.placed_against,
                event.placement.clone(),
            ));
            if self.case.starts_with("chest-") {
                let entity = host.block_entity(level, AT).expect("tentative chest");
                assert!(entity.data(host).is_some());
                assert_eq!(entity.set_name(host, Some(NAME)), Ok(true));
                *self.entity.lock().unwrap() = Some(entity);
            } else {
                assert!(host.block_entity(level, AT).is_none());
                let Placement::Multi(blocks) = &event.placement else {
                    panic!("one event for both halves")
                };
                assert_eq!(level.block_state(host, HEAD), Some(blocks[1].state));
                let write = match self.case.as_str() {
                    "bed-stale" => Some(HEAD),
                    "bed-against" => Some(AGAINST),
                    "bed-veto" | "bed-unwind" => Some(WRITE),
                    _ => None,
                };
                if let Some(write) = write {
                    let edit = if self.case == "bed-against" {
                        level.block_state(host, WRITE).unwrap()
                    } else {
                        event.state
                    };
                    assert_eq!(
                        level.set_block(host, write, edit, BlockUpdateFlags::NONE),
                        Ok(true)
                    );
                }
                if self.case == "bed-unwind" {
                    panic!("placement listener unwound");
                }
            }
            if self.case.ends_with("-veto") {
                Flow::Cancel
            } else {
                Flow::Continue
            }
        }
    }
    let case = std::env::var(FLAG).unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let entity = Arc::new(Mutex::new(None));
    let mods = vec![ModEntry::new(
        "placement_probe",
        Box::new(Probe {
            case: case.clone(),
            seen: seen.clone(),
            entity: entity.clone(),
        }),
    )];
    with_mods(mods.into(), || {
        with_guests(|game| {
            use super::api_values::TryIntoApi;
            loaded(game);
            let place_as = game.player(13).unwrap();
            let level = game.level(game.dim.id()).unwrap();
            let foot_id = game
                .pack
                .assets
                .registry
                .default_state("minecraft:red_bed")
                .unwrap();
            let foot: BlockState = foot_id.try_into_api().unwrap();
            let head_id = game
                .with_props(foot_id, &[("part", "head"), ("facing", "north")])
                .unwrap();
            let head: BlockState = head_id.try_into_api().unwrap();
            let air: BlockState = game.world.air.try_into_api().unwrap();
            let stone = api_state(game, "minecraft:stone");
            let chest = api_state(game, "minecraft:chest");
            let barrel = api_state(game, "minecraft:barrel");
            let is_bed = case.starts_with("bed-");
            let (before, chosen) = if is_bed {
                (chest, foot)
            } else if case == "chest-replace" {
                (barrel, chest)
            } else {
                (air, chest)
            };
            if before != air {
                assert_eq!(
                    level.set_block(game, AT, before, BlockUpdateFlags::NONE),
                    Ok(true)
                );
            }
            assert_eq!(
                level.set_block(game, AGAINST, stone, BlockUpdateFlags::NONE),
                Ok(true)
            );
            let stack = ItemStack::new("minecraft:dirt", 5);
            game.as_guest(13, |game| {
                game.inventory[game.selected] = Some(stack.clone())
            })
            .unwrap();
            if is_bed {
                game.world.block_entities.insert(
                    (2, 70, 2),
                    BlockEntity::Chest(vec![Some(stack.clone()); 27]),
                );
            }
            let original_entity = game.block_entity(level, AT);
            game.world.block_log = Some(Vec::new());
            let place = |game: &mut super::Game<'_>| {
                game.as_guest(12, |game| {
                    place_as.place_block(game, level, AT, chosen, BlockFace::East, false)
                })
                .unwrap()
            };
            let result = match case.as_str() {
                "bed-unwind" => run_transaction(game, |game, outer| {
                    assert_eq!(
                        level.set_block(game, OUTER, stone, BlockUpdateFlags::NONE),
                        Ok(true)
                    );
                    assert!(
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| place(game)))
                            .is_err()
                    );
                    assert_eq!(game.transaction_current().as_ref(), Some(outer));
                    assert_eq!(level.block_state(game, OUTER), Some(stone));
                    assert_eq!(level.block_state(game, AT), Some(before));
                    assert_eq!(level.block_state(game, WRITE), Some(air));
                    Ok(())
                }),
                "bed-death" => run_transaction(game, |game, _| {
                    place(game)?;
                    assert_eq!(level.block_state(game, HEAD), Some(head));
                    let living = LivingRef::new(place_as.entity());
                    assert!(living.set_health(game, 0.0));
                    assert_eq!(living.health(game), Some(0.0));
                    Ok(())
                }),
                _ => place(game),
            };
            let seen = seen.lock().unwrap();
            let [(placer, pos, state, against, footprint)] = seen.as_slice() else {
                panic!("one placement event")
            };
            assert_eq!(
                (*placer, *pos, *state, *against),
                (
                    Some(place_as.entity()),
                    BlockPos::new(2, 70, 2),
                    chosen,
                    stone
                )
            );
            if is_bed {
                let Placement::Multi(blocks) = footprint else {
                    panic!("both bed halves")
                };
                assert_eq!(
                    blocks
                        .iter()
                        .map(|cell| (cell.pos, cell.state))
                        .collect::<Vec<_>>(),
                    [
                        (BlockPos::new(2, 70, 2), foot),
                        (BlockPos::new(2, 70, 1), head)
                    ]
                );
            } else {
                assert!(matches!(footprint, Placement::Single));
            }
            if case != "bed-death" {
                game.as_guest(13, |game| assert_eq!(game.held(), Some(&stack)))
                    .unwrap();
            }
            match case.as_str() {
                "bed-death" | "bed-against" => {
                    assert_eq!(result, Ok(()));
                    assert_eq!(level.block_state(game, AT), Some(foot));
                    assert_eq!(level.block_state(game, HEAD), Some(head));
                    assert_eq!(
                        level.block_state(game, AGAINST),
                        Some(if case == "bed-against" { air } else { stone })
                    );
                }
                "chest-accept" | "chest-replace" => {
                    assert_eq!(result, Ok(()));
                    assert_eq!(level.block_state(game, AT), Some(chest));
                    assert_eq!(game.block_entity(level, AT), *entity.lock().unwrap());
                    assert_eq!(
                        game.world.block_names.get(&(2, 70, 2)),
                        Some(&Tag::String(NAME.into()))
                    );
                }
                "chest-veto" | "bed-veto" | "bed-stale" | "bed-unwind" => {
                    assert_eq!(
                        result,
                        match case.as_str() {
                            "bed-unwind" => Ok(()),
                            "bed-stale" => Err(Refusal::PlacementChanged),
                            _ => Err(Refusal::Canceled),
                        }
                    );
                    assert_eq!(level.block_state(game, AT), Some(before));
                    assert_eq!(level.block_state(game, AGAINST), Some(stone));
                    assert_eq!(game.block_entity(level, AT), original_entity);
                    assert!(!game.world.block_names.contains_key(&(2, 70, 2)));
                    if is_bed {
                        assert!(matches!(game.world.block_entities.get(&(2, 70, 2)),
                            Some(BlockEntity::Chest(slots)) if slots[0].as_ref() == Some(&stack)));
                    }
                    assert_eq!(level.block_state(game, WRITE), Some(air));
                    assert_eq!(level.block_state(game, HEAD), Some(air));
                    if case == "bed-unwind" {
                        assert_eq!(level.block_state(game, OUTER), Some(stone));
                        assert!(game.transaction_current().is_none());
                    } else {
                        assert!(game.world.block_log.as_ref().unwrap().is_empty());
                    }
                }
                _ => unreachable!(),
            }
        })
    });
}

/// Each refusal is its own name and writes nothing.
#[test]
#[ignore = "daily"]
fn each_place_block_refusal_has_its_own_name_and_writes_nothing() {
    with_guests(|game| {
        loaded(game);
        let b = game.player(13).unwrap();
        let level = game.level(game.dim.id()).unwrap();
        let stone = api_state(game, "minecraft:stone");
        let empty = game.world.block(3, 70, 3);
        let at = [2, 70, 2];
        assert_eq!(
            b.place_block(game, level, at, stone, BlockFace::Up, false),
            Ok(())
        );
        assert_eq!(
            b.place_block(game, level, at, stone, BlockFace::Up, false),
            Err(Refusal::Unchanged),
            "stone already stands there"
        );
        for pos in [[2, 400, 2], [2, -100, 2], [30_000_000, 70, 2]] {
            assert_eq!(
                b.place_block(game, level, pos, stone, BlockFace::Up, true),
                Err(Refusal::OutOfWorld { pos })
            );
        }
        let far = [1600, 70, 1600];
        assert_eq!(
            b.place_block(game, level, far, stone, BlockFace::Up, true),
            Err(Refusal::NotLoaded { pos: far })
        );
        // A mod cannot make a state the world lacks; the game's own check names one.
        let nothing = sunrise_world::registry::BlockStateId(u32::MAX);
        assert_eq!(
            game.player_place_block(b, level, [3, 70, 3], nothing, BlockFace::Up, true),
            Err(Refusal::UnknownState)
        );
        let old =
            mod_api::LevelRef::new(WorldEpoch::new(level.epoch().get() + 1), level.dimension());
        assert_eq!(
            b.place_block(game, old, [3, 70, 3], stone, BlockFace::Up, true),
            Err(Refusal::StaleLevel)
        );
        let gone = mod_api::__private::game::player_ref(
            EntityRef::new(
                WorldEpoch::new(b.entity().epoch().get() + 1),
                b.entity().id(),
            ),
            b.uuid(),
        );
        assert_eq!(
            gone.place_block(game, level, [3, 70, 3], stone, BlockFace::Up, true),
            Err(Refusal::Stale)
        );
        assert_eq!(game.world.block(3, 70, 3), empty, "nothing written");
        game.lan
            .as_mut()
            .unwrap()
            .hub
            .update_guest(13, |g| g.dead = true)
            .unwrap();
        assert_eq!(
            b.place_block(game, level, [3, 70, 3], stone, BlockFace::Up, true),
            Err(Refusal::Dead)
        );
        assert_eq!(game.world.block(3, 70, 3), empty, "nothing written");
    });
}

/// A level that is open but is not the one in place is `OtherLevel`: the check compares the
/// supplied level with the installed one, not with the player's dimension.
#[test]
#[ignore = "daily"]
fn a_level_not_in_place_is_other_level() {
    with_guests(|game| {
        loaded(game);
        let b = game.player(13).unwrap();
        let overworld = game.level("minecraft:overworld").unwrap();
        let stone = api_state(game, "minecraft:stone");
        game.enter_level(super::Dim::Nether).unwrap();
        assert_eq!(
            b.place_block(game, overworld, [2, 70, 2], stone, BlockFace::Up, true),
            Err(Refusal::OtherLevel)
        );
    });
}

/// On a client it is `Client`.
#[test]
#[ignore = "daily"]
fn a_client_cannot_place_a_block_as_a_player() {
    with_client_game(|game| {
        let level = mod_api::LevelRef::new(WorldEpoch::new(0), game.dim.key());
        let ghost = mod_api::__private::game::player_ref(EntityRef::new(WorldEpoch::new(0), 1), 1);
        let stone = api_state(game, "minecraft:stone");
        assert_eq!(
            ghost.place_block(game, level, [2, 70, 2], stone, BlockFace::Up, true),
            Err(Refusal::Client)
        );
    });
}
