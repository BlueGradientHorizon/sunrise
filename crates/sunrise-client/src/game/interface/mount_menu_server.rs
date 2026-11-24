#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{
    Game, Screen,
    transactions::{After, SaveChunkScope},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::mount_menu::CHEST_SIZE;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::mount_menu::MountMenu;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::mount_menu::Open;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::mount_menu::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    hud::{HudBatch, rgba},
    items::ItemStack,
    mobs::{Mob, Species, llama, nautilus},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_gameplay::mount_rules::{
    ARMOR, CHEST, SADDLE, body_armor_fits, has_mount_menu, mount_columns, shows_body, shows_saddle,
};

impl Game<'_> {
    pub(in crate::game) fn show_mount_menu(&mut self) {
        if self.remote.is_none() && self.server.lan_acting.is_none() {
            self.carried = None;
        }
        self.station = None;
        self.container = None;
        self.craft_size = 2;
        self.set_screen(Screen::Inventory);
        self.capture(false);
    }
}
