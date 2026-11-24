#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{Game, Screen};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::game_mode::inventory_after_infinite_materials;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::game_mode::relative;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::game_mode::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    collisions::SceneCollisions, command::args::GameMode, items::ItemStack,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_gameplay::game_modes::{
    loads_chunks, mode_from_id, projectile_predicate_supported,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_world::world_movement::in_range_of_ground;

impl Game<'_> {
    /// Whether `Screen::Inventory` currently shows the player's own `InventoryScreen`
    /// rather than a container, crafting table, workstation, mount, or server/mod menu.
    pub(in crate::game) fn is_player_inventory_screen(&self) -> bool {
        self.client.screen == Screen::Inventory
            && self.container.is_none()
            && self.station.is_none()
            && self.craft_size == 2
            && !self.mount_active()
            && self.mod_screen.0.is_none()
            && self.remote_menu_type().is_none()
    }
    /// Reconcile an open player inventory on its screen tick (`InventoryScreen.containerTick`
    /// and `CreativeModeInventoryScreen.containerTick`) against live `instabuild`, keeping
    /// the shared cursor (`carried`) stack across the switch.
    pub(in crate::game) fn tick_inventory_screen(&mut self) {
        if self.player.is_none() {
            return;
        }
        let next = inventory_after_infinite_materials(
            self.client.screen,
            self.is_player_inventory_screen(),
            self.instabuild(),
        );
        if next == self.client.screen {
            return;
        }
        match next {
            Screen::Creative => {
                self.set_screen(Screen::Creative);
                if self.client.screen == Screen::Creative {
                    self.refresh_creative_tabs();
                    self.creative_screen.0.reopen();
                    self.refresh_saved_hotbars();
                }
            }
            Screen::Inventory => {
                self.set_screen(Screen::Inventory);
            }
            _ => {}
        }
    }
}
