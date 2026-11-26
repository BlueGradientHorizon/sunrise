//! Snapshot position-owned loot with clone's selected block entities.
use super::*;
use sunrise_core::nbt::Tag;
use sunrise_level::containers::BlockEntity;
use sunrise_sim::save::PendingContainerLoot;

pub(in crate::game) struct CloneBlockPayload {
    entity: Option<BlockEntity>,
    name: Option<Tag>,
    pending: Option<PendingContainerLoot>,
}
impl Game<'_, crate::game::server_side::ServerState> {
    pub(in crate::game::server_side) fn clone_block_payload(
        &self,
        key: (i32, i32, i32),
    ) -> CloneBlockPayload {
        CloneBlockPayload {
            entity: self.world.block_entities.get(&key).cloned(),
            name: self.world.block_names.get(&key).cloned(),
            pending: self.world.block_entity_extras(key).pending_loot,
        }
    }
    /// Replacements discard the previous marker before removal can access it.
    pub(in crate::game::server_side) fn discard_clone_loot(&mut self, pos: [i32; 3]) {
        crate::game::container_loot::take_block_pending(&mut self.world, (pos[0], pos[1], pos[2]));
    }
    pub(in crate::game::server_side) fn apply_clone_payload(
        &mut self,
        pos: [i32; 3],
        payload: CloneBlockPayload,
    ) {
        let key = (pos[0], pos[1], pos[2]);
        if let Some(entity) = payload.entity {
            self.world.replace_block_entity(key, entity);
            match payload.name {
                Some(name) => self.world.block_names.insert(key, name),
                None => self.world.block_names.remove(&key),
            };
            self.world.with_chunk_extras(key, |extras| {
                extras.pending_loot.remove(&key);
                extras
                    .pending_loot
                    .extend(payload.pending.map(|pending| (key, pending)));
            });
            self.notify_block_entity_load_additional(key);
        }
    }
}
