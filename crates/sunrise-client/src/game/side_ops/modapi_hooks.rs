use std::sync::Arc;

use crate::game::Game;
use mod_api::{
    CreativeTabChange, CreativeTabEdit, CreativeTabEditError, CreativeTabList, PlayerRef,
    SwimmingCheck,
};
use sunrise_client::{chunk::Chunk, items::ItemStack};
use sunrise_items::creative_catalog::Holders;

impl Game<'_> {
    pub(in crate::game) fn side_fire_pending_block_entity_loads(&mut self) {
        if self.is_logical_server() {
            self.server_fire_pending_block_entity_loads();
        } else {
            self.client_fire_pending_block_entity_loads();
        }
    }

    pub(in crate::game) fn side_creative_holders(&self) -> Option<Holders> {
        if self.is_logical_server() {
            self.server_creative_holders()
        } else {
            self.client_creative_holders()
        }
    }

    pub(in crate::game) fn side_creative_tab_entries(
        &self,
        edit: CreativeTabEdit,
        list: CreativeTabList,
    ) -> Result<Vec<ItemStack>, CreativeTabEditError> {
        if self.is_logical_server() {
            self.server_creative_tab_entries(edit, list)
        } else {
            self.client_creative_tab_entries(edit, list)
        }
    }

    pub(in crate::game) fn side_creative_tab_change(
        &self,
        edit: CreativeTabEdit,
        change: CreativeTabChange,
    ) -> Result<(), CreativeTabEditError> {
        if self.is_logical_server() {
            self.server_creative_tab_change(edit, change)
        } else {
            self.client_creative_tab_change(edit, change)
        }
    }

    pub(in crate::game) fn side_ask_swim_eligibility(&self, check: SwimmingCheck) -> Option<bool> {
        if self.is_logical_server() {
            self.server_ask_swim_eligibility(check)
        } else {
            self.client_ask_swim_eligibility(check)
        }
    }

    pub(in crate::game) fn side_acting_id(&mut self) -> u128 {
        if self.is_logical_server() {
            self.server_acting_id()
        } else {
            self.client_acting_id()
        }
    }

    pub(in crate::game) fn side_clicker(&mut self) -> (u128, i32) {
        if self.is_logical_server() {
            self.server_clicker()
        } else {
            self.client_clicker()
        }
    }

    pub(in crate::game) fn side_acting_or_local_player(&self) -> Option<PlayerRef> {
        if self.is_logical_server() {
            self.server_acting_or_local_player()
        } else {
            self.client_acting_or_local_player()
        }
    }

    pub(in crate::game) fn side_chunks_loaded(&mut self) {
        if self.is_logical_server() {
            self.server_chunks_loaded();
        } else {
            self.client_chunks_loaded();
        }
    }

    pub(in crate::game) fn side_chunks_unloaded(&mut self, unloaded: &[Arc<Chunk>]) {
        if self.is_logical_server() {
            self.server_chunks_unloaded(unloaded);
        } else {
            self.client_chunks_unloaded(unloaded);
        }
    }

    pub(in crate::game) fn side_apply_after_use_components(&mut self, stack: &ItemStack) {
        if self.is_logical_server() {
            self.server_apply_after_use_components(stack);
        } else {
            self.client_apply_after_use_components(stack);
        }
    }
}
