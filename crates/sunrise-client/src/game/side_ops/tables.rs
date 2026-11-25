//! Role-dispatched table/parts side operations (rewritten to `SideOps` traits at switch).
use crate::game::{Game, WorldTables};

impl Game<'_> {
    #[inline(always)]
    pub(in crate::game) fn side_world_tables(&self) -> Option<&WorldTables> {
        if self.is_logical_server() {
            self.server_world_tables()
        } else {
            self.client_world_tables()
        }
    }

    #[inline(always)]
    pub(in crate::game) fn side_acting_player_uuid(&self) -> Option<u128> {
        if self.is_logical_server() {
            self.server_acting_player_uuid()
        } else {
            self.client_acting_player_uuid()
        }
    }

    #[inline(always)]
    pub(in crate::game) fn side_tags_ready(&self) -> bool {
        if self.is_logical_server() {
            self.server_tags_ready()
        } else {
            self.client_tags_ready()
        }
    }

    #[inline(always)]
    pub(in crate::game) fn side_resolve_enchant_particles(&self) {
        if !self.is_logical_server() {
            self.client_resolve_enchant_particles();
        }
    }

    #[inline(always)]
    pub(in crate::game) fn side_resolve_block_paths(&self) {
        if self.is_logical_server() {
            self.server_resolve_block_paths();
        }
    }

    #[inline(always)]
    pub(in crate::game) fn side_resolve_primary_items_server(
        &self,
        support: sunrise_world::enchantment_answers::Answers,
    ) {
        if self.is_logical_server() {
            self.server_resolve_primary_items_server(support);
        }
    }

    #[inline(always)]
    pub(in crate::game) fn side_clear_loot_enchantment_answers(&self) {
        if self.is_logical_server() {
            self.server_clear_loot_enchantment_answers();
        }
    }

    #[inline(always)]
    pub(in crate::game) fn side_clear_random_tick_sections(&mut self) {
        if self.is_logical_server() {
            self.server_clear_random_tick_sections();
        }
    }
}
