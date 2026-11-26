//! Level player visits and chunk/entity tracking queries. The level visit
//! resolves UUIDs from the spawn code's player enumeration so a guest's turn
//! names its player once, excluding anyone who cannot resolve.
use crate::game::ServerGame;
use core::ops::ControlFlow;
use mod_api::{ChunkRef, EntityRef, LevelReadError, LevelRef, PlayerRef};

impl ServerGame<'_> {
    /// `ServerLevel.players`: the online players of an open server level, each once.
    pub(in crate::game) fn level_visit_online_players(
        &self,
        level: LevelRef,
        visit: &mut dyn FnMut(PlayerRef) -> ControlFlow<()>,
    ) -> Result<ControlFlow<()>, LevelReadError> {
        self.common.native_level_world_result(level)?;
        Ok(self
            .level_player_uuids(level.dimension_id())
            .filter_map(|uuid| self.common.player(uuid))
            .try_for_each(visit))
    }

    /// `ChunkMap.TrackedEntity.seenBy`: the players of the level in place whose tracker holds
    /// the entity, other than the entity itself.
    pub(in crate::game) fn entity_trackers(&self, entity: EntityRef) -> Option<Vec<PlayerRef>> {
        let target = self.common.native_entity_target(entity)?;
        if self.target_level(target).key() != self.common.dim.key() {
            return None;
        }
        let id = entity.id() as i32;
        Some(
            self.server
                .lan
                .iter()
                .flat_map(|h| h.hub.guests.iter().map(move |g| (&h.hub, g)))
                .filter(|(hub, g)| hub.here(g) && g.entity_id != id && g.tracker.is_tracking(id))
                .filter_map(|(_, g)| self.common.player(g.uuid))
                .collect(),
        )
    }

    /// `ChunkMap.getPlayers(pos, false)`: the players the chunk was sent to and is
    /// inside the view of.
    pub(in crate::game) fn chunk_watchers(&self, chunk: ChunkRef) -> Option<Vec<PlayerRef>> {
        self.common.native_level_world(chunk.level())?;
        let dimension = chunk.level().dimension_id();
        let [x, z] = chunk.pos();
        Some(
            self.server
                .lan
                .iter()
                .flat_map(|h| h.hub.guests.iter())
                .filter(|g| g.tracks_chunk(dimension, (x, z)))
                .filter_map(|g| self.common.player(g.uuid))
                .collect(),
        )
    }
}
