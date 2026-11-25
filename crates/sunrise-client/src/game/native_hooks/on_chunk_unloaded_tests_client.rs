#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_entity_refs::tests::{
    run_fresh_cases, with_game_at,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::on_chunk_unloaded_tests::CALLS;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::on_chunk_unloaded_tests::CHEST;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::on_chunk_unloaded_tests::CHUNK;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::on_chunk_unloaded_tests::POT;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::on_chunk_unloaded_tests::POTS;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::on_chunk_unloaded_tests::Pot;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::on_chunk_unloaded_tests::Probe;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::on_chunk_unloaded_tests::prepared_root;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::on_chunk_unloaded_tests::pump_until;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::on_chunk_unloaded_tests::request;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::on_chunk_unloaded_tests::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::world_loading::WorldRequest;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{
    BlockEntityPart, BlockPart, BlocksEvent, Hooks, HostSide, OnChunkUnloadedEvent, Registrar,
    RegistrationContext,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::path::Path;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::sync::Mutex;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::time::{Duration, Instant};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::containers::BlockEntity;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::nbt::{Compound, Tag};

#[test]
#[ignore = "daily"]
fn each_mod_block_entity_of_an_unloading_chunk_is_told_once() {
    if run_fresh_cases(
        "game::client_side::native_hooks_client::on_chunk_unloaded_tests::each_mod_block_entity_of_an_unloading_chunk_is_told_once",
        "SUNRISE_ON_CHUNK_UNLOADED_CHILD",
        &["game"],
    ) {
        return;
    }
    let mods: mod_api::Mods = vec![mod_api::ModEntry::new("unload_probe", Box::new(Probe))].into();
    mod_platform::__private::install(mods.clone());
    crate::game::api_values::install_parts(&mods).unwrap();
    let root = prepared_root();
    let saves = std::env::current_dir()
        .unwrap()
        .join("target")
        .join(format!("on-chunk-unloaded-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&saves);

    with_game_at(root, |game| {
        game.load_world(request(&saves, true)).unwrap();
        game.set_allow_commands(true);
        assert!(game.run_command("forceload add 1600 1600"));
        pump_until(game, true);
        game.tick_world(true).unwrap();
        for [x, y, z] in POTS {
            assert!(game.run_command(&format!("setblock {x} {y} {z} {POT}")));
        }
        let [x, y, z] = CHEST;
        assert!(game.run_command(&format!("setblock {x} {y} {z} minecraft:chest")));
        game.tick_world(true).unwrap();
        assert!(CALLS.lock().unwrap().is_empty(), "told before any unload");

        // The ticket goes: the chunk is saved, its parts are told, its block entities dropped.
        assert!(game.run_command("forceload remove 1600 1600"));
        pump_until(game, false);
        let mut calls = std::mem::take(&mut *CALLS.lock().unwrap());
        calls.sort();
        assert_eq!(
            calls,
            POTS.map(|pos| ("server", pos, POT.to_owned(), true))
                .to_vec(),
            "one call for each pot, none for the chest"
        );
        for [x, y, z] in POTS {
            assert!(game.world.block_entities.get(&(x, y, z)).is_none());
        }
        // A few more streaming rounds tell nothing again.
        for _ in 0..20 {
            game.streaming.wait(Duration::from_millis(5));
            game.server_update_world();
        }
        assert!(CALLS.lock().unwrap().is_empty(), "told twice");

        // Back from its save: both pots, without what was written while it unloaded.
        assert!(game.run_command("forceload add 1600 1600"));
        pump_until(game, true);
        for [x, y, z] in POTS {
            let Some(BlockEntity::Mod(entity)) = game.world.block_entities.get(&(x, y, z)) else {
                panic!("pot at {x} {y} {z} was not saved");
            };
            assert_eq!(entity.id(), POT);
            assert!(!entity.nbt.contains_key(&"written_on_unload".into()));
        }

        // Closing the world, its chunks with it, tells nothing.
        game.begin_closing_save(false).unwrap();
        let deadline = Instant::now() + Duration::from_secs(120);
        while game.pending_save.0.is_some() {
            assert!(Instant::now() < deadline, "the world did not close");
            game.poll_closing_save().unwrap();
        }
        assert!(
            game.world.chunks.is_empty(),
            "the world's chunks went with it"
        );
        assert!(
            CALLS.lock().unwrap().is_empty(),
            "told when the world closed"
        );
    });
    let _ = std::fs::remove_dir_all(&saves);
}
