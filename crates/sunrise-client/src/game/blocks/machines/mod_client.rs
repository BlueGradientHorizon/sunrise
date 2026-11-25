#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{
    Game, composter::is_composter, spill, storage::is_shulker_box,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::machines::AutomationEntity;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::machines::ContainerTarget;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::machines::Containers;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::machines::Insertion;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::machines::Kind;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::machines::MoveIn;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::machines::PlacementAttempt;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::machines::PlacementEntity;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::machines::PlacementQuery;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::machines::PlacementSnapshot;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::machines::Screen;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::machines::Slots;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::machines::View;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::machines::container_signal;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::machines::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{
    self, BlockSide, Capability, ENTITY_AUTOMATION, EntityRef, ItemResource, ResourceHandler,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::{borrow::Cow, ops::ControlFlow, sync::Arc};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    Result,
    containers::{BlockEntity, HOPPER_SLOTS, ModContainer},
    item_data::cooking_fuel,
    items::{ItemStack, Items},
    mobs::Mob,
    save,
    world::World,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::nbt::Tag;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::{
    geometry::Direction, position::BlockPos, random::RandomSource,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_gameplay::hoppers::{
    HopperState, ItemColumns, StampedItemColumns, cat_sitting_above, dir_from_name, offset, room,
    shelf_signal,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_gameplay::hoppers::{Pos, hopper_cooldown_in};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_world::{
    level::{Behaviours, LevelAccess},
    registry::BlockStateId,
};

#[path = "crafter_client.rs"]
pub(in crate::game) mod crafter;
