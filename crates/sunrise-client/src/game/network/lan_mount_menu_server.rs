#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{
    Dim, Game,
    lan::LanHost,
    lan_actions::guest_within_entity_range,
    lan_crafting::GuestMenuEffects,
    mount_menu::{mount_columns, shows_body},
    transactions::After,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::lan_mount_menu::GuestMount;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::lan_mount_menu::MountClickWindow;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::lan_mount_menu::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    items::ItemStack,
    menu::{Kind, Menu, QuickCraftDrag},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_protocol::packets::play::{self, Clientbound as C};

impl Game<'_, crate::game::server_side::ServerState> {
    /// `AbstractMountInventoryMenu.stillValid`: the mount is alive and the
    /// guest's eyes are within the entity interaction range (3, 5 in creative)
    /// plus 4 of its box (`Player.isWithinEntityInteractionRange(mount, 4.0)`).
    pub(in crate::game) fn lan_mount_valid(
        &self,
        g: &sunrise_client::lan::guest::Guest,
        mob: u32,
    ) -> bool {
        self.mobs
            .iter()
            .find(|m| m.id == mob)
            .is_some_and(|m| m.alive() && guest_within_entity_range(g, &m.bounds(), 4.0))
    }
}
