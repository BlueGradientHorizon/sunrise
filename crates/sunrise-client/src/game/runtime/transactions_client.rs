use super::Game;
use crate::game::transactions::{
    After, ClosingPhase, JournaledPlayer, Kept, KeptPlayer, Level, StorageSettlement, entities,
};
use mod_api::TransactionLifecycle;
use sunrise_client::{block_entities::BlockEntityIdentity, containers::Pos};

impl Game<'_> {
    pub(in crate::game) fn client_tx_close_storage_transaction(
        &mut self,
        commit: bool,
    ) -> Option<StorageSettlement> {
        let mut level = self.transactions.pop()?;
        let refused = commit
            && level.packets.is_some()
            && sunrise_client::lan::conn::transaction_packets::Scope::refused();
        let commit = commit && !refused;
        let owned = level.owned_scratch.take().and_then(|id| {
            level.scratch_kept.remove(&id);
            self.transactions.scratch.remove(&id)
        });
        self.transactions
            .attachment_query_scopes
            .leave(level.parent_attachment_query_scope);
        let closing = if self.transactions.is_empty() {
            TransactionLifecycle::RootClosing
        } else {
            TransactionLifecycle::Closing
        };
        let outer = self.transactions.closing.replace(closing);
        let retire = !self.has_level_transaction()
            && !entities::is_committing(&self.transactions.committing, self.dim);
        let guard = ClosingPhase { game: self, outer };
        if let Some(scope) = level.packets.take()
            && commit
            && let Some(packets) = scope.finish()
        {
            level.captured_packets.push(packets);
        }
        guard
            .game
            .transactions
            .close_retained_originals(level.frame, commit);
        match (commit, guard.game.transactions.levels.last_mut()) {
            (true, Some(outer)) => {
                outer.merge_level_writes(&mut level);
                for (id, kept) in level.scratch_kept {
                    if guard.game.transactions.scratch.contains_key(&id) {
                        outer.scratch_kept.entry(id).or_insert(kept);
                    }
                }
                for (owner, kept) in level.players {
                    outer.players.entry(owner).or_insert(kept);
                }
                for (holder, kept) in level.attachments {
                    outer.attachments.entry(holder).or_insert(kept);
                }
                outer.captured_packets.extend(level.captured_packets);
                outer.after.extend(level.after);
            }
            (true, None) => guard.game.client_tx_root_commit(level),
            (false, _) => guard.game.client_tx_roll_back(level),
        }
        if retire {
            guard.game.world.block_entities.publish_retirement();
        }
        Some(if commit {
            StorageSettlement::Committed(owned)
        } else if refused {
            StorageSettlement::Refused
        } else {
            StorageSettlement::Aborted
        })
    }

    pub(in crate::game::client_side) fn client_tx_root_commit(&mut self, level: Level) {
        for packets in level.captured_packets {
            packets.publish();
        }
        for after in level.after {
            self.client_tx_run_after(after);
        }
    }

    pub(in crate::game::client_side) fn client_tx_roll_back(&mut self, mut level: Level) {
        self.scratch_roll_back(std::mem::take(&mut level.scratch_kept));
        level.after.clear();
        level.captured_packets.clear();
        let removed = std::mem::take(&mut level.removed_block_entities);
        for (pos, kept) in level.block_entities {
            let was_removed = removed.contains_key(&pos);
            kept.restore_client(&mut self.world, was_removed, pos);
        }
        for ((pos, slot), kept) in level.slots {
            if self.world.block_entities.generation(&pos) != kept.generation {
                continue;
            }
            if let Some(target) = self
                .world
                .block_entities
                .get_mut(&pos)
                .and_then(|entity| entity.slots_mut().get_mut(slot))
            {
                *target = kept.stack;
            }
        }
        for (pos, block) in level.blocks {
            self.world.set_stored(pos.0, pos.1, pos.2, block.state);
        }
        for ((x, y, z), group) in level.connection_groups {
            self.world.set_connection_group_stored(x, y, z, group);
        }
        for (owner, kept) in level.players {
            if matches!(owner, JournaledPlayer::Local(_)) {
                self.inventory = kept.inventory;
                self.armor = kept.armor;
                self.offhand = kept.offhand;
                self.equipment = kept.equipment;
                self.carried = kept.carried;
            }
        }
    }

    pub(in crate::game) fn client_tx_run_after(&mut self, after: After) {
        match after {
            After::Payload(payload) => payload.publish(),
            After::Packet(packet, body) => self.publish_mod_held(packet, body),
            After::Block(_)
            | After::BulkBlocks(..)
            | After::ConnectionGroup(..)
            | After::Changed(_)
            | After::ChangedWithoutHook(_)
            | After::ChangedHolder { .. } => {}
            other => {
                debug_assert!(
                    false,
                    "server-only After variant replayed on Client role: {:?}",
                    std::mem::discriminant(&other)
                );
            }
        }
    }

    pub(in crate::game) fn client_tx_container_change_effect(
        &self,
        pos: (i32, i32, i32),
        holder: Option<BlockEntityIdentity>,
    ) -> After {
        match holder {
            Some(holder) => After::ChangedHolder {
                holder,
                silent: false,
            },
            None => After::Changed(pos),
        }
    }

    pub(in crate::game) fn client_tx_journal_player(&mut self, uuid: u128) {
        let Some(level) = self.transactions.player_write() else {
            return;
        };
        level.keep_player(
            JournaledPlayer::Local(uuid),
            &[],
            self.level.player_extra.as_ref(),
            || KeptPlayer {
                inventory: self.inventory.clone(),
                armor: self.armor.clone(),
                offhand: self.offhand.clone(),
                equipment: self.equipment.clone(),
                carried: self.carried.clone(),
            },
        );
    }

    pub(in crate::game) fn client_tx_journal_block_entity(&mut self, pos: Pos) {
        self.transactions.writes = self.transactions.writes.wrapping_add(1);
        let Some(level) = entities::open_in(&mut self.transactions, self.dim) else {
            return;
        };
        level
            .block_entities
            .entry(pos)
            .or_insert_with(|| Kept::capture_client(&self.world, pos));
    }

    pub(in crate::game) fn client_tx_journal_menus(&mut self, _player: mod_api::PlayerRef) {
        self.transactions.player_writes = self.transactions.player_writes.wrapping_add(1);
        self.transactions.writes = self.transactions.writes.wrapping_add(1);
    }
}
