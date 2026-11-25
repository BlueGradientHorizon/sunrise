#[allow(unused_imports)]
pub(in crate::game::client_side) use super::super::{
    Game, Hit, execution_role::ExecutionRole, lan::LanSettings,
    native_entity_refs::tests::with_game,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::interaction_event_tests::Controls;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::interaction_event_tests::Observer;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::interaction_event_tests::Step;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::interaction_event_tests::hit;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::interaction_event_tests::side;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::interaction_event_tests::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::InteractionResult as Answer;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{
    self, BlockPart, ENTITY_ITEMS, Flow, Hooks, HostSide, HostTransaction, ItemPart, ItemResource,
    LivingEntityUseItemFinishEvent, LogicalSide, Registrar, RightClickBlockEvent,
    RightClickItemEvent, ServerHost, TriState, UseItemEvent, UseItemOnBlockEvent, UseItemOnEvent,
    UseOnEvent, UsePhase,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::sync::{
    Arc, Mutex,
    atomic::{AtomicI32, AtomicUsize},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    chunk::Chunk,
    first_person::Hand,
    items::ItemStack,
    lan::{Joined, NetConfig, PlayerSender, guest::Guest, hub::HubConfig},
    motion::PlayerMotion,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::geometry::Vec3;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::nbt::Tag;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_protocol::{
    connection::Pipe, packets::configuration::ClientInformation,
};

#[path = "mapping_press_tests_client.rs"]
pub(in crate::game) mod mapping_press_tests;

impl InteractionSide for crate::game::client_side::ClientState {
    const IS_CLIENT: bool = true;
    const LOGICAL_SIDE: LogicalSide = LogicalSide::Client;
    fn set_interaction_mode(
        game: &mut Game<'_, Self>,
        mode: sunrise_client::command::args::GameMode,
    ) {
        game.set_local_mode(mode);
    }
    fn interaction_use_block_hand(game: &mut Game<'_, Self>, hand: Hand) -> bool {
        game.remote_use_item_on(hand, mod_api::InteractionAttempt::Press, None)
    }
    fn interaction_use_air_hand(game: &mut Game<'_, Self>, hand: Hand) -> bool {
        game.remote_use_item(hand)
    }
    fn interaction_use_item_as_player(game: &mut Game<'_, Self>) -> bool {
        game.remote_use_item(Hand::Main)
    }
    fn interaction_set_sneak(game: &mut Game<'_, Self>, sneak: bool) {
        game.set_mapping_held("key.sneak", sneak);
        if let Some(remote) = game.remote.as_mut() {
            if sneak {
                remote.last_input |= crate::game::remote::SHIFT_INPUT;
            } else {
                remote.last_input &= !crate::game::remote::SHIFT_INPUT;
            }
        }
    }
    fn interaction_clear_guest_use(_game: &mut Game<'_, Self>) {}
    fn interaction_reset_use_failed(_game: &mut Game<'_, Self>) {}
}
