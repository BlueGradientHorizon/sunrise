//! Role-dispatched UI, chat, block-entity NBT, item-splash, and level-bound side operations.
use crate::game::Game;
use sunrise_client::entities::ItemSplash;
use sunrise_core::nbt::Tag;

impl Game<'_> {
    #[inline(always)]
    pub(in crate::game) fn side_block_entity_nbt(&self, pos: [i32; 3]) -> Option<Tag> {
        if self.is_logical_server() {
            self.server_block_entity_nbt(pos)
        } else {
            self.client_block_entity_nbt(pos)
        }
    }

    #[inline(always)]
    pub(in crate::game) fn side_send_chat_command(&mut self, command: &str) {
        if self.is_logical_server() {
            self.server_send_chat_command(command)
        } else {
            self.client_send_chat_command(command)
        }
    }

    #[inline(always)]
    pub(in crate::game) fn side_apply_item_splash(&mut self, splash: ItemSplash) {
        if self.is_logical_server() {
            self.server_apply_item_splash(splash)
        } else {
            self.client_apply_item_splash(splash)
        }
    }

    #[inline(always)]
    pub(in crate::game) fn side_level_min_y(&self) -> i32 {
        if self.is_logical_server() {
            self.server_level_min_y()
        } else {
            self.client_level_min_y()
        }
    }
}
