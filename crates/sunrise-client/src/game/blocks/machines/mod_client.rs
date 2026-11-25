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
#[path = "screen.rs"]
mod screen;

impl Game<'_> {
    pub(in crate::game) fn machine_layout_selector(&self) -> Option<bool> {
        self.machine_screen().map(|s| s == Screen::Hopper)
    }
    pub(in crate::game) fn machine_screen(&self) -> Option<Screen> {
        let key = self.container?;
        // A mod's screen over a hopper or dispenser menu lays out its own slots.
        if self.native_screen_inventory_y().is_some() {
            return None;
        }
        // A server's menu shows by its type, whatever block (or minecart) holds it.
        if let Some(menu) = self.remote_menu_type() {
            return match menu {
                "minecraft:hopper" => Some(Screen::Hopper),
                "minecraft:generic_3x3" => Some(Screen::Dispenser),
                _ => None,
            };
        }
        if key.1 == super::minecarts::CART_KEY_Y {
            return (self.open_cart_kind() == Some(super::minecarts::CartKind::Hopper))
                .then_some(Screen::Hopper);
        }
        let state = self.world.block(key.0, key.1, key.2);
        match &*self.res.facts().get(state).name {
            "minecraft:hopper" => Some(Screen::Hopper),
            "minecraft:dispenser" => Some(Screen::Dispenser),
            "minecraft:dropper" => Some(Screen::Dropper),
            _ => None,
        }
    }
    /// `imageHeight` of the open screen (HopperScreen is 133 tall).
    pub(in crate::game) fn screen_height(&self) -> f32 {
        if let Some(height) = self.native_screen_height() {
            return height;
        }
        if self
            .station
            .as_ref()
            .is_some_and(|s| s.kind == super::stations::Kind::Beacon)
        {
            return super::beacon::HEIGHT;
        }
        if let Some(rows) = self.chest_rows() {
            // ContainerScreen: 114 + 18 per row.
            return 114.0 + rows as f32 * 18.0;
        }
        if self.container_block_is_shulker() {
            return 167.0;
        }
        if self.machine_screen() == Some(Screen::Hopper) {
            133.0
        } else {
            166.0
        }
    }
    /// HopperMenu / DispenserMenu slot layouts; None for other screens.
    pub(in crate::game) fn machine_slot_position(&self, i: usize) -> Option<(f32, f32)> {
        let screen = self.machine_screen()?;
        let inventory_y = if screen == Screen::Hopper { 51.0 } else { 84.0 };
        Some(match i {
            0..9 => (8.0 + i as f32 * 18.0, inventory_y + 58.0),
            9..36 => (
                8.0 + ((i - 9) % 9) as f32 * 18.0,
                inventory_y + ((i - 9) / 9) as f32 * 18.0,
            ),
            300..500 if screen == Screen::Hopper => (44.0 + (i - 300) as f32 * 18.0, 20.0),
            300..500 => (
                62.0 + ((i - 300) % 3) as f32 * 18.0,
                17.0 + ((i - 300) / 3) as f32 * 18.0,
            ),
            _ => return None,
        })
    }
}
