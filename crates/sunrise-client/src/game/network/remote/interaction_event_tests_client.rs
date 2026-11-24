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
pub(in crate::game::client_side) use crate::game::remote::interaction_event_tests::with_interaction_game;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::interaction_event_tests::with_interaction_game_at;
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
