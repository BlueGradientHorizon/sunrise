#[allow(unused_imports)]
use crate::game::command_harness::spans;
#[allow(unused_imports)]
use crate::game::native_entity_refs::tests::{
    bind_local_player, prepare_entity_ticking_neighborhood, run_fresh_cases, with_game,
};
#[allow(unused_imports)]
use serde_json::{Value, json};
#[allow(unused_imports)]
use std::collections::HashSet;
#[allow(unused_imports)]
use sunrise_client::motion::PlayerMotion;
#[allow(unused_imports)]
use sunrise_core::geometry::Vec3;
#[allow(unused_imports)]
use sunrise_core::nbt::Tag;
#[allow(unused_imports)]
use sunrise_core::position::ChunkPos;
#[allow(unused_imports)]
use sunrise_core::random::RandomSource;

// `UUIDUtil.createOfflinePlayerUUID("Player")`, matching VanillaCommandExecOracle's single-player owner.
pub(in crate::game) const ORACLE_PLAYER_UUID: u128 = 0xa01e3843_e521_3998_958a_f459800e4d11;

pub(in crate::game) fn tag_contains_bolt(tag: &Tag) -> bool {
    if sunrise_client::entity_nbt::kind(tag).as_deref() == Some("minecraft:lightning_bolt") {
        return true;
    }
    if let Some(Tag::List(passengers)) = tag.get("Passengers") {
        return passengers.iter().any(tag_contains_bolt);
    }
    false
}

/// Replays the vanilla 26.3 witness for `/summon minecraft:lightning_bolt` with trailing NBT (#2690),
/// recorded into `.local` by `tools/test_summon_bolt_nbt_parity.py`.
#[test]
#[ignore = "run by tools/test_summon_bolt_nbt_parity.py"]
fn a_bolt_summoned_with_nbt_strikes_and_goes() {
    run_a_bolt_summoned_with_nbt_strikes_and_goes();
}

#[allow(unused_imports)]
pub(in crate::game) use crate::game::server_side::commands_server::summon_bolt_tests::run_a_bolt_summoned_with_nbt_strikes_and_goes;
