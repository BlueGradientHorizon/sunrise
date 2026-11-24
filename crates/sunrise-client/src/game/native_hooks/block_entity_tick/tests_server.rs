#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::Dim;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_entity_refs::tests::{
    bind_local_player, run_fresh_cases, with_game, with_game_at,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::block_entity_tick::tests::AT;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::block_entity_tick::tests::CHUNK;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::block_entity_tick::tests::CONTAINER_MACHINE;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::block_entity_tick::tests::Effects;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::block_entity_tick::tests::KIND;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::block_entity_tick::tests::MACHINE;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::block_entity_tick::tests::POS;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::block_entity_tick::tests::Retake;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::block_entity_tick::tests::UNSUBSCRIBED;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::block_entity_tick::tests::UNSUBSCRIBED_MACHINE;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::block_entity_tick::tests::access;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::block_entity_tick::tests::check_edit_then_replace;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::block_entity_tick::tests::enable_block_ticks;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::block_entity_tick::tests::place_subject;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::block_entity_tick::tests::row;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::block_entity_tick::tests::with_ticker;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::block_entity_tick::tests::with_ticker_mods;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::block_entity_tick::tests::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::{
    BlockEdit, BlockEntityLoadAdditionalEvent, BlockEntityPart, BlockEntitySaveAdditionalEvent,
    BlockEntityTickEvent, BlockPart, BlockRun, BlockUpdateFlags, Direction, EntityJoinLevelEvent,
    Flow, Hooks, HostTransaction, ItemResource, ItemSpawnEvent, LevelHost, ModEntry,
    NeighborNotifyEvent, Registrar, STORAGE,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU32, Ordering},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    chunk::Chunk,
    containers::{ModBlockEntityFields, ModContainer},
    items::ItemStack,
    motion::PlayerMotion,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::geometry::Vec3;

#[test]
#[ignore = "daily"]
fn replacement_and_item_stack_journals_close_in_their_own_dimensions() {
    const NAME: &str = "game::server_side::native_hooks_server::block_entity_tick::tests::replacement_and_item_stack_journals_close_in_their_own_dimensions";
    if run_fresh_cases(NAME, "SUNRISE_RESIDENT_DIMENSION_JOURNALS", &["run"]) {
        return;
    }
    let scratch = tempfile::tempdir().unwrap();
    with_ticker(|game, _| {
        game.server.world_root = scratch.path().into();
        let overworld = place_subject(game, KIND);
        game.enter_level(Dim::Nether).unwrap();
        let nether = place_subject(game, KIND);
        let original = game.world.block(POS.0, POS.1, POS.2);
        let replacement = game
            .pack
            .assets
            .registry
            .default_state("minecraft:glass")
            .unwrap();
        let state = replacement.try_into_api().unwrap();
        // Keep distinct edited rows and stacks at exactly the same coordinates.
        for dim in [Dim::Nether, Dim::Overworld] {
            game.enter_level(dim).unwrap();
            game.world.block_entities.set_mod_containers(
                [(
                    original.0,
                    Arc::new(ModContainer {
                        slots: 2,
                        ..Default::default()
                    }),
                )]
                .into_iter()
                .collect(),
            );
            let BlockEntity::Mod(row) = game.world.block_entities.get_mut(&POS).unwrap() else {
                panic!("expected a mod container");
            };
            let item = if dim == Dim::Nether {
                "minecraft:gold_ingot"
            } else {
                "minecraft:iron_ingot"
            };
            row.items = vec![
                Some(ItemStack::new("minecraft:diamond", 1)),
                Some(ItemStack::new(item, 1)),
            ];
            game.spawn_item(ItemStack::new(item, 8), [1.0, 64.0, 1.0], [0.0; 3], 0);
        }
        let replace = |game: &mut Game<'_>, subject| {
            let item = game
                .item_entity(game.entity(game.dropped[0].id).unwrap())
                .unwrap();
            let mut access = access(game, subject);
            let mut tick = BlockEntityScope::new(&mut access);
            let (level, pos) = tick
                .with_view(|mut view| {
                    assert_eq!(view.set_item_slot(0, None), Ok(true));
                    (view.level(), view.position())
                })
                .unwrap();
            let host = tick.host();
            let stack = item
                .with_stack(host, |s| s.project())
                .flatten()
                .unwrap()
                .shrink(1)
                .unwrap();
            assert_eq!(item.set_stack(host, stack), Ok(true));
            assert_eq!(
                level.set_block(host, pos, state, BlockUpdateFlags::KNOWN_SHAPE),
                Ok(true)
            );
        };
        game.storage_scope(|game, _| {
            replace(game, overworld);
            game.enter_level(Dim::Nether).unwrap();
            game.storage_scope(|game, _| {
                replace(game, nether);
                ((), false)
            });
            let level = game.level("minecraft:the_nether").unwrap();
            assert_eq!(game.block_entity(level, AT), Some(nether));
            assert_eq!(game.world.block(POS.0, POS.1, POS.2), original);
            assert_eq!(game.dropped[0].stack.count, 8);
            assert_eq!(game.dropped.len(), 1);
            game.storage_scope(|game, _| {
                replace(game, nether);
                ((), true)
            });
            game.enter_level(Dim::Overworld).unwrap();
            ((), false)
        });
        let level = game.level("minecraft:overworld").unwrap();
        assert_eq!(game.block_entity(level, AT), Some(overworld));
        assert_eq!(game.world.block(POS.0, POS.1, POS.2), original);
        assert_eq!(game.dropped[0].stack.count, 8);
        assert_eq!(game.dropped.len(), 1);
        game.enter_level(Dim::Nether).unwrap();
        assert_eq!(game.world.block(POS.0, POS.1, POS.2), replacement);
        assert_eq!(
            game.block_entity(game.level("minecraft:the_nether").unwrap(), AT),
            None
        );
        assert_eq!(game.dropped[0].stack.count, 7);
        assert_eq!(
            game.dropped[1].stack,
            ItemStack::new("minecraft:gold_ingot", 1)
        );
        assert_eq!(game.dropped.len(), 2);
        game.enter_level(Dim::Overworld).unwrap();
    });
}
