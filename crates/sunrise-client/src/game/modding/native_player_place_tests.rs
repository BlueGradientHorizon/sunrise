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
        BlockState, BlockUpdateFlags, EntityPlaceEvent, Flow, Hooks, ModEntry, ServerHost,
    };
    use mod_platform::testing::with_mods;
    use std::sync::{Arc, Mutex};
    use sunrise_client::{containers::BlockEntity, items::ItemStack};
    use sunrise_core::position::BlockPos;

    const FLAG: &str = "SUNRISE_TEST_PLAYER_PLACE";
    const AT: [i32; 3] = [2, 70, 2];
    const AGAINST: [i32; 3] = [1, 70, 2];
    const WRITE: [i32; 3] = [4, 70, 2];
    if run_fresh_cases(
        "game::native_player_place_tests::chosen_placement_attributes_the_player_and_rolls_back_on_refusal",
        FLAG,
        &["accept", "cancel", "change"],
    ) {
        return;
    }
    type PlacementSeen = (Option<EntityRef>, BlockPos, BlockState, BlockState);
    struct Probe {
        case: String,
        seen: Arc<Mutex<Vec<PlacementSeen>>>,
    }
    #[mod_api::__subscriptions]
    impl Hooks for Probe {
        fn entity_place(&self, host: &mut dyn ServerHost, event: &mut EntityPlaceEvent) -> Flow {
            let level = host.level("minecraft:overworld").unwrap();
            assert!(matches!(event.placement, mod_api::Placement::Single));
            assert_eq!(level.block_state(host, AT), Some(event.state));
            assert!(host.block_entity(level, AT).is_none());
            self.seen.lock().unwrap().push((
                event.placer,
                event.pos,
                event.state,
                event.placed_against,
            ));
            if self.case != "accept" {
                let pos = if self.case == "change" {
                    AGAINST
                } else {
                    WRITE
                };
                assert_eq!(
                    level.set_block(host, pos, event.state, BlockUpdateFlags::NONE),
                    Ok(true)
                );
            }
            if self.case == "cancel" {
                Flow::Cancel
            } else {
                Flow::Continue
            }
        }
    }
    let case = std::env::var(FLAG).unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let mods = vec![ModEntry::new(
        "placement_probe",
        Box::new(Probe {
            case: case.clone(),
            seen: seen.clone(),
        }),
    )];
    with_mods(mods.into(), || {
        with_guests(|game| {
            loaded(game);
            let place_as = game.player(13).unwrap();
            let level = game.level(game.dim.id()).unwrap();
            let bed = api_state(game, "minecraft:red_bed");
            let stone = api_state(game, "minecraft:stone");
            let chest = api_state(game, "minecraft:chest");
            assert_eq!(
                level.set_block(game, AT, chest, BlockUpdateFlags::NONE),
                Ok(true)
            );
            assert_eq!(
                level.set_block(game, AGAINST, stone, BlockUpdateFlags::NONE),
                Ok(true)
            );
            let stack = ItemStack::new("minecraft:dirt", 5);
            game.as_guest(13, |game| {
                game.inventory[game.selected] = Some(stack.clone());
            })
            .unwrap();
            game.world.block_entities.insert(
                (2, 70, 2),
                BlockEntity::Chest(vec![Some(stack.clone()); 27]),
            );
            game.world.block_log = Some(Vec::new());
            let air = game.world.air;
            let result = game
                .as_guest(12, |game| {
                    place_as.place_block(game, level, AT, bed, BlockFace::East, false)
                })
                .unwrap();
            assert_eq!(
                *seen.lock().unwrap(),
                [(Some(place_as.entity()), BlockPos::new(2, 70, 2), bed, stone)]
            );
            game.as_guest(13, |game| assert_eq!(game.held(), Some(&stack)))
                .unwrap();
            match case.as_str() {
                "accept" => {
                    assert_eq!(result, Ok(()));
                    assert_eq!(level.block_state(game, AT), Some(bed));
                    assert_ne!(game.world.block(2, 70, 1), air, "the bed's head was placed");
                }
                "cancel" | "change" => {
                    assert_eq!(
                        result,
                        Err(if case == "cancel" {
                            Refusal::Canceled
                        } else {
                            Refusal::PlacementChanged
                        })
                    );
                    assert_eq!(level.block_state(game, AT), Some(chest));
                    assert_eq!(level.block_state(game, AGAINST), Some(stone));
                    assert!(
                        matches!(game.world.block_entities.get(&(2, 70, 2)), Some(BlockEntity::Chest(slots)) if slots[0].as_ref() == Some(&stack))
                    );
                    assert_eq!(game.world.block(4, 70, 2), air, "listener write was undone");
                    assert_eq!(game.world.block(2, 70, 1), air, "no companion was placed");
                    assert!(game.world.block_log.as_ref().unwrap().is_empty());
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
