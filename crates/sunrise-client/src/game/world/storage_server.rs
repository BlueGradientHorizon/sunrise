#[allow(unused_imports)]
pub(in crate::game::server_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::storage::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    containers::{BlockEntity, is_chest_block},
    items::ItemStack,
    text::Text,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::random::RandomSource;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_gameplay::chests::{
    chest_connected_pos, is_shulker_box,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_gameplay::chests::{chest_half, facing_step};

impl Game<'_> {
    /// Closing it copies the items back and keeps the block entity, as vanilla's
    /// `EnderChestBlockEntity` persists across open/close (`close_ender_chest`
    /// must not remove it, or `/data get block` loses it after a use).
    pub(in crate::game) fn close_ender_chest(&mut self) {
        let Some(key) = self.server.ender_open.take() else {
            return;
        };
        // The items go back to the player: an `EnderChestBlockEntity` holds none, so
        // nothing of theirs stays in the chunk or in `/data get block`.
        if let Some(BlockEntity::Chest(items)) = self.world.block_entities.get_mut(&key) {
            self.server.ender_items = std::mem::replace(items, vec![None; 27]);
        }
    }
    /// Opening an ender chest shows the player's ender inventory in its place.
    pub(in crate::game) fn open_ender_chest(&mut self, key: (i32, i32, i32)) {
        self.world
            .block_entities
            .insert(key, BlockEntity::Chest(self.server.ender_items.clone()));
        self.container = Some(key);
        self.server.ender_open = Some(key);
    }
    /// `ServerPlayerGameMode.useItemOn` for a spectator: the block's menu
    /// (`BlockState.getMenuProvider`) opens, and nothing else happens. The
    /// lid, its sound and the `container_open` game event come from
    /// `ContainerOpenersCounter`, which spectators never join
    /// (`ChestBlockEntity.startOpen`); the stat, the piglins' anger and the
    /// shulker box's room check are `useWithoutItem`'s, which is not called.
    /// Only the block containers whose clicks the server takes back from a
    /// spectator (`handleContainerClick`) open here: storage, the furnaces,
    /// the brewing stand and the crafter.
    pub(in crate::game) fn spectator_open_menu(&mut self, pos: [i32; 3]) {
        if self.fake_blocks(mod_api::FakeEffect::Menu) {
            return;
        }
        let key = (pos[0], pos[1], pos[2]);
        let name = self
            .res
            .facts()
            .get(self.world.block(pos[0], pos[1], pos[2]))
            .name
            .clone();
        let container = is_shulker_box(&name)
            || matches!(
                name.trim_start_matches("minecraft:"),
                "chest"
                    | "trapped_chest"
                    | "barrel"
                    | "hopper"
                    | "dispenser"
                    | "dropper"
                    | "furnace"
                    | "blast_furnace"
                    | "smoker"
                    | "brewing_stand"
                    | "crafter"
            );
        // ChestBlock.getMenuProvider: none while either half is blocked.
        if !container || self.chest_menu_blocked(key, &name) {
            return;
        }
        // BaseContainerBlockEntity.canOpen: a spectator opens no container that still
        // has a loot table to unpack (it clears while it rolls, on first open).
        // ServerPlayer.openMenu sends the refused spectator a red action bar.
        if self
            .world
            .chunks
            .get(&(key.0 >> 4, key.2 >> 4))
            .and_then(|c| c.saved.as_ref())
            .is_some_and(|s| s.pending_loot.contains_key(&key))
        {
            self.send_overlay_message(
                Text::translate("container.spectatorCantOpen", &[])
                    .with_color(sunrise_client::text::RED),
            );
            return;
        }
        let Some(entity) = BlockEntity::for_block(&name) else {
            return;
        };
        self.world.block_entities.get_or_insert_with(key, || entity);
        self.container = Some(key);
        self.server.open_container_slots = Some((key, self.open_container_size()));
        self.set_screen(super::Screen::Inventory);
        self.capture(false);
        self.set_mapping_held("key.use", false);
    }
    /// `Container.stillValidBlockEntity`: the open block container closes once
    /// the player's eyes are out of reach of any of its blocks (a 4-block buffer).
    pub(in crate::game) fn tick_container_reach(&mut self) {
        // A server's menu ends when the server closes it.
        if self.remote.is_some() {
            return;
        }
        let Some(eye) = self.eye_position() else {
            return;
        };
        if self
            .container
            .is_none_or(|k| k.1 == super::minecarts::CART_KEY_Y)
        {
            return;
        }
        // `Container.stillValidBlockEntity`: the menu holds a block entity per
        // part, and a part that went away (`getBlockEntity(pos) != this`) ends
        // it. A double chest whose other half broke would otherwise show a
        // shorter container.
        if self
            .server
            .open_container_slots
            .is_some_and(|(open, slots)| {
                self.container == Some(open) && self.open_container_size() < slots
            })
        {
            self.close_container();
            self.tick_container_reach_client();
            return;
        }
        let range = self.reach() + 4.0;
        let near = self.container_parts().iter().all(|&((x, y, z), _)| {
            sunrise_client::lan::guest::within_block_range(eye, range, [x, y, z])
        });
        if !near {
            self.close_container();
            self.set_screen(super::Screen::Playing);
            self.capture(true);
        }
    }
    /// `CraftingMenu.stillValid`: the open crafting table closes once its
    /// block is no longer a crafting table or the player's eyes are out of
    /// reach (a 4-block buffer).
    pub(in crate::game) fn tick_craft_table(&mut self) {
        // A server's menu ends when the server closes it.
        if self.remote.is_some() || self.craft_size != 3 {
            return;
        }
        let (Some(pos), Some(eye)) = (self.server.craft_table, self.eye_position()) else {
            return;
        };
        let state = self.world.block(pos[0], pos[1], pos[2]);
        let table = &*self.res.facts().get(state).name == "minecraft:crafting_table";
        let range = self.reach() + 4.0;
        if !(table && sunrise_client::lan::guest::within_block_range(eye, range, pos)) {
            self.close_container();
            self.set_screen(super::Screen::Playing);
            self.capture(true);
        }
    }
}
