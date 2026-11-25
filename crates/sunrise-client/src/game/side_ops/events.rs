use crate::game::{Game, Result, transactions};
use sunrise_client::{
    block_entities::BlockEntityIdentity, mobs::Who, sculk::Context as SculkContext,
};
use sunrise_world::registry::BlockStateId;

#[cfg(test)]
thread_local! {
    pub(in crate::game) static TEST_POSTED_EVENTS: std::cell::RefCell<
        Option<Vec<(&'static str, [f64; 3], Option<Who>)>>,
    > = const { std::cell::RefCell::new(None) };
    pub(in crate::game) static TEST_POSTED_EVENT_ORDER: std::cell::RefCell<
        Option<Vec<(&'static str, Option<Who>, usize, usize)>>,
    > = const { std::cell::RefCell::new(None) };
}

impl Game<'_> {
    // Level events and world update.

    pub(in crate::game) fn side_process_level_events(&mut self) -> Result<()> {
        if self.is_logical_server() {
            self.server_process_level_events()
        } else {
            self.client_process_level_events()
        }
    }

    pub(in crate::game) fn side_level_event_at(&mut self, id: i32, pos: [i32; 3], data: i32) {
        if self.is_logical_server() {
            self.server_level_event_at(id, pos, data);
        } else {
            self.client_level_event_at(id, pos, data);
        }
    }

    pub(in crate::game) fn side_update_world(&mut self) {
        if self.is_logical_server() {
            self.server_update_world();
        } else {
            self.client_update_world();
        }
    }

    // Sculk and game events.

    pub(in crate::game) fn side_post_block_game_event(
        &mut self,
        event: &'static str,
        pos: [i32; 3],
        actor: Option<Who>,
        state: Option<BlockStateId>,
    ) {
        if self.is_logical_server() {
            self.server_post_block_game_event(event, pos, actor, state);
        } else {
            self.client_post_block_game_event(event, pos, actor, state);
        }
    }

    pub(in crate::game) fn side_post_game_event(
        &mut self,
        event: &'static str,
        pos: [f64; 3],
        ctx: SculkContext,
    ) {
        if self.is_logical_server() {
            self.server_post_game_event(event, pos, ctx);
        } else {
            self.client_post_game_event(event, pos, ctx);
        }
    }

    pub(in crate::game) fn side_sculk_actor(&self) -> Option<Who> {
        if self.is_logical_server() {
            self.server_sculk_actor()
        } else {
            self.client_sculk_actor()
        }
    }

    // Animated block-entity bell hooks.

    pub(in crate::game) fn side_bell_gather(&self, pos: [i32; 3]) -> Vec<u64> {
        if self.is_logical_server() {
            self.server_bell_gather(pos)
        } else {
            self.client_bell_gather(pos)
        }
    }

    pub(in crate::game) fn side_bell_heard(&mut self, pos: [i32; 3], time: i64) {
        if self.is_logical_server() {
            self.server_bell_heard(pos, time);
        } else {
            self.client_bell_heard(pos, time);
        }
    }

    #[allow(dead_code)]
    pub(in crate::game) fn side_tick_bell(
        &mut self,
        pos: [i32; 3],
        tags: &sunrise_client::items::EntityTypeTags,
    ) {
        if self.is_logical_server() {
            self.server_tick_bell(pos, tags);
        } else {
            self.client_tick_bell(pos, tags);
        }
    }

    // Stats and container notifications for common callers.

    pub(in crate::game) fn side_award_stat(&mut self, category: &str, item: &str, amount: i32) {
        if self.is_logical_server() {
            self.server_award_stat(category, item, amount);
        } else {
            self.client_award_stat(category, item, amount);
        }
    }

    pub(in crate::game) fn side_award_custom_stat(&mut self, key: &str, amount: i32) {
        if self.is_logical_server() {
            self.server_award_custom_stat(key, amount);
        } else {
            self.client_award_custom_stat(key, amount);
        }
    }

    pub(in crate::game) fn side_container_changed(&mut self, key: (i32, i32, i32)) {
        if self.is_logical_server() {
            self.server_container_changed(key);
        } else {
            self.client_container_changed(key);
        }
    }

    // Transactions and journals.

    pub(in crate::game) fn side_tx_close_storage_transaction(
        &mut self,
        commit: bool,
    ) -> Option<transactions::StorageSettlement> {
        if self.is_logical_server() {
            self.server_tx_close_storage_transaction(commit)
        } else {
            self.client_tx_close_storage_transaction(commit)
        }
    }

    pub(in crate::game) fn side_tx_run_after(&mut self, after: transactions::After) {
        if self.is_logical_server() {
            self.server_tx_run_after(after);
        } else {
            self.client_tx_run_after(after);
        }
    }

    pub(in crate::game) fn side_tx_container_change_effect(
        &self,
        pos: (i32, i32, i32),
        holder: Option<BlockEntityIdentity>,
    ) -> transactions::After {
        if self.is_logical_server() {
            self.server_tx_container_change_effect(pos, holder)
        } else {
            self.client_tx_container_change_effect(pos, holder)
        }
    }

    pub(in crate::game) fn side_tx_journal_player(&mut self, uuid: u128) {
        if self.is_logical_server() {
            self.server_tx_journal_player(uuid);
        } else {
            self.client_tx_journal_player(uuid);
        }
    }

    pub(in crate::game) fn side_tx_journal_block_entity(&mut self, pos: (i32, i32, i32)) {
        if self.is_logical_server() {
            self.server_tx_journal_block_entity(pos);
        } else {
            self.client_tx_journal_block_entity(pos);
        }
    }

    pub(in crate::game) fn side_tx_journal_menus(&mut self, player: mod_api::PlayerRef) {
        if self.is_logical_server() {
            self.server_tx_journal_menus(player);
        } else {
            self.client_tx_journal_menus(player);
        }
    }
}
