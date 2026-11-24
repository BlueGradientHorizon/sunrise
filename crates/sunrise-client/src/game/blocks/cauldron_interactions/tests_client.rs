#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_entity_refs::tests::{
    bind_local_player, run_fresh_cases, with_game_at,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::cauldron_interactions::tests::POS;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::cauldron_interactions::tests::Seen;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::cauldron_interactions::tests::Syrup;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::cauldron_interactions::tests::TestMod;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::cauldron_interactions::tests::bottles;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::cauldron_interactions::tests::events;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::cauldron_interactions::tests::fixture;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::cauldron_interactions::tests::fixture_mods;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::cauldron_interactions::tests::setup;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::cauldron_interactions::tests::state;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::cauldron_interactions::tests::unchanged;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::cauldron_interactions::tests::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{
    BlockUpdateFlags, CauldronDecision, CauldronUse, Hooks, ItemPlace, ItemResource, ServerHost,
    StatType,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::cauldron::{Outcome, interactions::Entry};

#[test]
#[ignore = "daily"]
fn cauldron_client_skips_custom_prediction() {
    if run_fresh_cases(
        "game::client_side::cauldron_interactions_client::tests::cauldron_client_skips_custom_prediction",
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
        assert_eq!(outcome, Outcome::Pass);
        assert!(proposed.0.is_empty());
        assert!(proposed.1.is_empty());
        assert_eq!(seen.calls.load(Ordering::SeqCst), 0);
        assert_eq!(game.cauldron_outcome(POS, held.as_ref()), Outcome::Pass);
        assert_eq!(seen.calls.load(Ordering::SeqCst), 0);
        let hit = Hit {
            pos: POS,
            face: [0, 1, 0],
            point: [0.5, 65.0, 0.5],
            inside: false,
        };
        assert!(game.remote_cauldron_reacts(&hit, held.as_ref()).is_none());
        assert_eq!(seen.calls.load(Ordering::SeqCst), 0);
        assert_eq!(seen.actor.load(Ordering::SeqCst), 0);
        unchanged(game, seen);
        assert_eq!(game.audio.0.capture.as_ref().unwrap().len(), 0);
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
fn cauldron_outer_abort_discards_effects_and_drops() {
    if run_fresh_cases(
        "game::client_side::cauldron_interactions_client::tests::cauldron_outer_abort_discards_effects_and_drops",
        "SUNRISE_CAULDRON_CASE",
        &["run"],
    ) {
        return;
    }
    fixture(false, |game, seen| {
        crate::game::native_item_places::tests::open_to_lan(game);
        for slot in game.inventory.iter_mut().skip(1) {
            *slot = Some(ItemStack::new("minecraft:stone", 64));
        }
        game.storage_scope(|game, _| {
            assert_eq!(game.use_on_cauldron(POS).unwrap(), Outcome::Success);
            assert_eq!(events(), 0);
            assert!(game.creatures.lan_sounds.is_empty());
            assert!(game.audio.0.capture.as_ref().unwrap().is_empty());
            ((), false)
        });
        unchanged(game, seen);
        assert_eq!(events(), 0);
        assert!(game.creatures.lan_sounds.is_empty());
        assert!(game.audio.0.capture.as_ref().unwrap().is_empty());
    });
}
