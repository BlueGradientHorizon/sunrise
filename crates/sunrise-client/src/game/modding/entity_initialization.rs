//! `entity_constructing` (`EntityEvent.EntityConstructing`): vanilla's `Entity.<init>`, once per
//! entity on the side that makes it. Sunrise has no one constructor, so the hook is told where
//! each kind of entity starts: a spawned mob (`spawn_mob`), an entity read from disk or a
//! summon's NBT (`remember_uuid`, before its saved attachments are read), every other new
//! entity in the join pass just before `entity_join_level`, a LAN guest's player as it is
//! loaded or respawned, and on a client of a server the entities and players the server adds.
//! The first attachment access of an entity not yet told tells it first, so no mod reads an
//! attachment before the listeners have set it.

#[allow(unused_imports)]
use super::Game;
pub(in crate::game) use crate::game::entity_initialization_common::EntityInitialization;
#[allow(unused_imports)]
use mod_api::Holder;
#[allow(unused_imports)]
use std::collections::{HashMap, HashSet};

#[cfg(test)]
#[path = "entity_initialization/tests.rs"]
pub(in crate::game) mod tests;

impl Game<'_, crate::game::server_side::ServerState> {
    pub(in crate::game) fn server_bind_constructed_entity_attachments(&mut self, id: u32) {
        if self.native_authoritative()
            && self
                .entity_attachments
                .get(&id)
                .is_some_and(|map| map.has_index_entries())
            && let Err(error) = self.bind_resident_entity_attachments(id)
        {
            log::warn!("constructor attachment binding failed for entity {id}: {error:?}");
        }
    }

    /// Forget destroyed objects in this level before its UUID metadata is pruned.
    pub(in crate::game::server_side) fn prune_entity_initializations(
        &mut self,
        retained: &HashSet<u32>,
    ) {
        self.entity_initialization
            .initialized
            .retain(|holder| match holder {
                Holder::Entity(id) => !self.persist.uuids.contains_key(id) || retained.contains(id),
                _ => true,
            });
    }

    /// A LAN guest's player, its file just read for connection `conn`.
    pub(in crate::game::server_side) fn initialize_server_player(&mut self, uuid: u128, conn: u32) {
        if self.entity_initialization.players.insert(uuid, conn) != Some(conn) {
            self.forget_entity_initialization(Holder::Player(uuid));
        }
        self.initialize_guest_player_under_data(uuid, conn);
    }

    /// `PlayerList.respawn`: the guest gets a new `ServerPlayer`, told again, and the data it
    /// keeps (`copy_on_death`, already in place) is restored over it.
    pub(in crate::game::server_side) fn reinitialize_server_player(
        &mut self,
        uuid: u128,
        conn: u32,
    ) {
        self.forget_entity_initialization(Holder::Player(uuid));
        self.initialize_guest_player_under_data(uuid, conn);
    }

    /// Tell a guest's player whose attachments already hold its data, then read that data
    /// back over what the listeners set: the end state of vanilla's order, where the player is
    /// constructed before its data is read (`Entity.load`, `restoreFrom`).
    pub(in crate::game) fn initialize_guest_player_under_data(&mut self, uuid: u128, conn: u32) {
        if !mod_api::__private::listened(mod_api::HookId::EntityConstructing) {
            return;
        }
        let data = self
            .lan
            .as_mut()
            .and_then(|host| host.hub.guests.attachments_mut(conn))
            .map(|attachments| attachments.to_nbt());
        self.initialize_entity(Holder::Player(uuid), "minecraft:player");
        if let Some(data) = data
            && let Some(attachments) = self
                .lan
                .as_mut()
                .and_then(|host| host.hub.guests.attachments_mut(conn))
        {
            attachments.read_saved_over(data);
        }
    }
}

/// Explicit ownership passed alongside one travelling object's NBT, never encoded in NBT.
/// Not Clone: a transfer cannot install the same runtime state twice.
#[derive(Debug)]
pub(in crate::game) struct TransferredEntityState {
    pub(in crate::game) attachments: Option<sunrise_mod_runtime::attachments::Attachments>,
    pub(in crate::game) initialized: bool,
    pub(in crate::game) accepted_hit: Option<std::sync::Arc<sunrise_client::health::AcceptedHit>>,
    pub(in crate::game) died: bool,
    pub(in crate::game) transient: bool,
    /// The source id whose attachment map a transaction open at the take kept: the
    /// arrival moves that keeping to the new id.
    pub(in crate::game) journaled: Option<u32>,
    /// Filled by an abort that comes before the arrival: the contents the arrival puts back.
    pub(in crate::game) rollback: Option<super::transactions::ArrivalRollback>,
    /// The id the entity had before it travelled, which a kill credit may still name.
    pub(in crate::game) source: u32,
}

impl TransferredEntityState {
    /// Move the single accepted-hit owner onto a reconstructed living body.
    pub(in crate::game) fn restore_mob(&mut self, mob: &mut sunrise_client::mobs::Mob) {
        if mob.health > 0.0 {
            self.died = false;
        }
        if let Some(hit) = self.accepted_hit.take() {
            mob.last_damage = hit.committed;
            mob.accepted_hit = Some(hit);
        }
    }
}

impl Game<'_, crate::game::server_side::ServerState> {
    /// Consume the source object's state before forgetting its persistence metadata.
    /// Retiring the publisher immediately makes retained query write tokens stale.
    /// Inside a transaction the take is journaled: the map's writes in the action roll back
    /// wherever the map is, and the source level's index keeps its rows until the root closes
    /// (`defer_departure`). Travel itself is not undone, so the map stays with the entity.
    pub(in crate::game) fn take_transferred_entity_state(
        &mut self,
        id: u32,
    ) -> TransferredEntityState {
        let journaled = (!self.transactions.is_empty()).then(|| {
            self.journal_attachments(Holder::Entity(id));
            id
        });
        let rollback = journaled.map(|id| self.journal_in_flight(id));
        let mut attachments = self.entity_attachments.remove(&id);
        if journaled.is_some() {
            self.defer_departure(id, attachments.as_mut());
        } else if let Some(map) = &mut attachments {
            map.unbind_index();
        }
        let accepted_hit = self
            .mob_object_mut(id)
            .and_then(|mob| mob.accepted_hit.take());
        let died = self.side.died.remove(&id);
        TransferredEntityState {
            attachments,
            accepted_hit,
            died,
            transient: self.persist.transient.remove(&id),
            journaled,
            rollback,
            source: id,
            initialized: self
                .entity_initialization
                .initialized
                .remove(&Holder::Entity(id)),
        }
    }

    /// Install before `remember_uuid`: the destination is the same logical entity,
    /// so neither construction nor the saved overlay may replace its runtime values.
    pub(in crate::game) fn adopt_transferred_entity_state(
        &mut self,
        id: u32,
        state: TransferredEntityState,
    ) {
        self.journal_arrival(id);
        let level = self.dim;
        self.entity_attachments.move_collider(id, level);
        if let Some(source) = state.journaled {
            self.journal_travel(source, id);
        }
        let now = self.world.game_time;
        self.side.levels.0.note_travel(state.source, id, now);
        if state.died {
            self.side.died.insert(id);
        }
        if state.transient {
            self.persist.transient.insert(id);
        }
        // Even an empty map marks this as runtime state for `remember_uuid`.
        let mut map = state.attachments.unwrap_or_default();
        // 🤖 Agent: an action that took this entity and was rolled back before it arrived left
        // the contents it first found here: the arrival carries those, not the action's writes.
        if let Some(kept) = state.rollback.and_then(|slot| {
            slot.lock()
                .unwrap_or_else(|error| error.into_inner())
                .take()
        }) {
            map.replace_contents(kept);
        }
        self.entity_attachments.insert(id, map);
        if state.initialized {
            self.entity_initialization
                .initialized
                .insert(Holder::Entity(id));
        }
    }
}
