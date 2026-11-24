#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::lan_actions::pickups::pickup_post_tests::CASE;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::lan_actions::pickups::pickup_post_tests::Case;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::lan_actions::pickups::pickup_post_tests::Notice;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::lan_actions::pickups::pickup_post_tests::Outcome;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::lan_actions::pickups::pickup_post_tests::Probe;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::lan_actions::pickups::pickup_post_tests::State;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::lan_actions::pickups::pickup_post_tests::apples;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::lan_actions::pickups::pickup_post_tests::arm;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::lan_actions::pickups::pickup_post_tests::clear_ground;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::lan_actions::pickups::pickup_post_tests::ground;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::lan_actions::pickups::pickup_post_tests::guest_case;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::lan_actions::pickups::pickup_post_tests::host_case;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::lan_actions::pickups::pickup_post_tests::host_stat;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::lan_actions::pickups::pickup_post_tests::spawn;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::lan_actions::pickups::pickup_post_tests::with_probe;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::lan_actions::pickups::pickup_post_tests::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::{
    Game, native_entity_refs::tests::collect_all_entities,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{
    Hooks, ItemPickedUpEvent, ItemPickedUpPostEvent, ModEntry, ServerHost, TriState,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_platform::testing::with_mods;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::sync::{Arc, Mutex};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::items::ItemStack;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::motion::PlayerMotion;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::geometry::Vec3;

pub(in crate::game) fn host_animations(game: &Game<'_>) -> u64 {
    game.pickups.0.made().get("item").copied().unwrap_or(0)
}
