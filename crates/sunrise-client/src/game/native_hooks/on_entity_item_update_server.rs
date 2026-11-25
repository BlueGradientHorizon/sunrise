#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::on_entity_item_update::ItemEntityTickAccess;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::on_entity_item_update::ResidentDroppedItem;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::on_entity_item_update::native_on_entity_item_update;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::on_entity_item_update::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_ticks;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::geometry::Vec3;

#[cfg(test)]
pub(in crate::game) mod on_entity_item_update_tests {
    #[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::{
        api_values::{STACK_PROJECTIONS, test_snapshot},
        native_entity_refs::tests::with_game,
    };
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::{Flow, Hooks, ItemPart, OnEntityItemUpdateEvent, Registrar, RegistrationContext};
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::sync::{Arc, Mutex};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::chunk::Chunk;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::on_entity_item_update::on_entity_item_update_tests::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::on_entity_item_update::on_entity_item_update_tests::Update;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::on_entity_item_update::on_entity_item_update_tests::Mod;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::on_entity_item_update::on_entity_item_update_tests::ORDER;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::on_entity_item_update::on_entity_item_update_tests::told;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::on_entity_item_update::on_entity_item_update_tests::floor;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::on_entity_item_update::on_entity_item_update_tests::counts;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::on_entity_item_update::on_entity_item_update_tests::a_canceling_part_holds_its_own_item_and_nothing_else_isolated;

    /// One stack of `item` at rest on the floor, a pickup delay of 20 ticks.
    pub(in crate::game) fn rest(game: &mut Game<'_>, item: &str, x: f64, z: f64) -> u32 {
        game.spawn_item(ItemStack::new(item, 1), [x, 64.0, z], [0.0; 3], 20);
        game.server.ticked_entities_through = game.server.next_entity;
        game.dropped.last().unwrap().id
    }
}
