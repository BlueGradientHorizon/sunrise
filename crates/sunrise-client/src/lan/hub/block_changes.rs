//! `ChunkHolder.broadcastChanges`: the light and blocks that changed in the level in place, to
//! the players sent those chunks, and, wrapped with the level, to the players whose client views
//! it (`viewed_levels`).
use super::{Guest, Hub};
use crate::{
    chunk::Chunk,
    fast_hash::FxHashSet,
    lan::viewed_levels::send::Runs,
    world::{ChunkResend, World},
};
use sunrise_protocol::{
    packets::play::{self, Clientbound as C},
    viewed_levels::Level,
};

impl Hub {
    /// Whether any player currently holds `key` in `dimension` for an explicit resend:
    /// a tracked chunk in their own level (`Guest::tracks_chunk`) or a sent chunk in a viewed one.
    pub fn has_chunk_resend_target(&self, dimension: &str, key: (i32, i32)) -> bool {
        self.resend_holders(dimension, key).next().is_some()
    }

    /// The players eligible for an explicit chunk resend of `key` in `dimension`:
    /// same-level players for whom `Guest::tracks_chunk` holds (`ChunkMap.isChunkTracked`),
    /// plus players viewing `dimension` who have been sent `key`.
    fn resend_holders<'a>(
        &'a self,
        dimension: &'a str,
        key: (i32, i32),
    ) -> impl Iterator<Item = (usize, Option<Level>)> + 'a {
        let viewed = self.viewed.active();
        self.guests
            .simulation_indexed_iter()
            .filter_map(move |(i, g)| {
                if g.watches_level(dimension) {
                    g.tracks_chunk(dimension, key).then_some((i, None))
                } else if viewed && g.views_level(dimension) {
                    g.viewed
                        .view(dimension)
                        .filter(|v| v.sent.contains(&key))
                        .map(|v| (i, Some(v.level)))
                } else {
                    None
                }
            })
    }

    /// The players sent chunk `key` in `dimension` (`ChunkMap.getPlayers`): those in it (None),
    /// and those whose client views it, with the level to wrap with and the chunks each was sent.
    fn chunk_holders<'a>(
        &'a self,
        dimension: &'a str,
        key: (i32, i32),
    ) -> impl Iterator<Item = (usize, Option<Level>, &'a FxHashSet<(i32, i32)>)> + 'a {
        let viewed = self.viewed.active();
        self.guests
            .simulation_indexed_iter()
            .filter_map(move |(i, g)| {
                if g.watches_level(dimension) {
                    g.sent_chunks
                        .contains(&key)
                        .then_some((i, None, &g.sent_chunks))
                } else if viewed && g.views_level(dimension) {
                    g.viewed
                        .view(dimension)
                        .filter(|v| v.sent.contains(&key))
                        .map(|v| (i, Some(v.level), &v.sent))
                } else {
                    None
                }
            })
    }

    /// `body`, a play clientbound packet, to each of `holders`.
    fn send_to_holders(&self, holders: &[(usize, Option<Level>)], body: &[u8]) {
        for &(i, level) in holders {
            let g = &self.guests[i];
            match level {
                None => g.sender.send(body.to_vec()),
                Some(level) => {
                    let mut runs = Runs::new(level);
                    runs.push(body);
                    g.sender.send_many(runs.finish());
                }
            }
        }
    }

    /// `ChunkHolder.broadcastChanges`' light part: the light sections that
    /// changed in a column go to the players for whom it is on the border of
    /// what they track (`ChunkMap.isChunkOnTrackedBorder`); the others light
    /// the blocks they are sent themselves.
    pub(in crate::lan) fn broadcast_light_changes(&mut self, world: &mut World) {
        let Some(log) = world.light_log.as_mut() else {
            return;
        };
        if log.is_empty() {
            return;
        }
        let mut changes: Vec<_> = log.drain().collect();
        // One entry per column, so the order is the columns'.
        changes.sort_unstable_by_key(|&(column, _)| column);
        for ((x, z), sections) in changes {
            let viewers: Vec<(usize, Option<Level>)> = self
                .chunk_holders(&self.config.dimension, (x, z))
                .filter(|(_, _, sent)| {
                    (-1..=1).any(|dz| (-1..=1).any(|dx| !sent.contains(&(x + dx, z + dz))))
                })
                .map(|(i, level, _)| (i, level))
                .collect();
            if viewers.is_empty() {
                continue;
            }
            let Some(chunk) = world.chunks.get(&(x, z)) else {
                continue;
            };
            let body = self.encoder.light_update(chunk, sections);
            self.send_to_holders(&viewers, &body);
        }
    }

    pub(in crate::lan) fn broadcast_block_changes(&mut self, world: &mut World) {
        // Chunks changed in bulk (`World::edit_runs`) go whole to those who see them: one packet each, where a
        // change of each block would be thousands (the client lights and meshes them once, on its workers).
        let mut bulk = Vec::new();
        if !world.bulk_log.is_empty() {
            bulk.extend(world.bulk_log.drain());
            bulk.sort_unstable();
            // The single-player window's far terrain draws a column it isn't sent as the window last saw it: it is
            // handed the column to summarize again (`handoff::changed`).
            let unsent = |g: &Guest, key: &(i32, i32)| {
                self.here(g)
                    && g.address() == crate::lan::conn::MEMORY_ADDRESS
                    && !g.sent_chunks.contains(key)
            };
            for key in &bulk {
                if let Some(chunk) = world.chunks.get(key)
                    && self.guests.iter().any(|g| unsent(g, key))
                {
                    crate::lan::handoff::changed(chunk);
                }
            }
        }
        let mut resends = Vec::new();
        if !world.chunk_resends.is_empty() {
            // Release only resends whose preceding light completion marker reached `Ready`
            // (`ChunkMap.waitForLightBeforeSending` / `ThreadedLevelLightEngine.waitForPendingTasks`).
            world.chunk_resends.retain(|&key, state| {
                if !world.chunks.contains_key(&key) {
                    return false;
                }
                match *state {
                    ChunkResend::Ready => {
                        resends.push(key);
                        false
                    }
                    ChunkResend::WaitingEdits | ChunkResend::WaitingLight => {
                        self.has_chunk_resend_target(&self.config.dimension, key)
                    }
                }
            });
            resends.sort_unstable();
        }
        let mut sent_whole = Vec::new();
        if !bulk.is_empty() || !resends.is_empty() {
            let mut whole = bulk.clone();
            whole.extend_from_slice(&resends);
            whole.sort_unstable();
            whole.dedup();
            let columns: Vec<&Chunk> = whole
                .iter()
                .filter_map(|k| world.chunks.get(k).map(|c| &**c))
                .collect();
            self.encoder.warm(&columns);
            for chunk in columns {
                let key = (chunk.x, chunk.z);
                let is_bulk = bulk.binary_search(&key).is_ok();
                let holders: Vec<_> = if is_bulk {
                    self.holders(key)
                } else {
                    self.resend_holders(&self.config.dimension, key).collect()
                };
                if holders.is_empty() {
                    continue;
                }
                let body = self.encoder.body_in(chunk, world);
                self.send_to_holders(&holders, &body);
                sent_whole.push(key);
            }
        }
        let Some(log) = world.block_log.as_mut() else {
            return;
        };
        if !sent_whole.is_empty() {
            // (what changed in them one at a time went with them, but for one-shot fields: a chunk has none)
            log.retain(|p| {
                sent_whole.binary_search(&(p.0 >> 4, p.2 >> 4)).is_err()
                    || matches!(
                        world.block_entities.get(p),
                        Some(crate::containers::BlockEntity::Mod(m)) if m.has_client_update_fields()
                    )
            });
        }
        if log.is_empty() {
            return;
        }
        let mut changes = std::mem::take(log);
        changes.sort_unstable();
        changes.dedup();
        // Group by section.
        let mut sections: std::collections::BTreeMap<(i32, i32, i32), Vec<(i32, i32, i32)>> =
            Default::default();
        for p in changes {
            sections
                .entry((p.0 >> 4, p.1 >> 4, p.2 >> 4))
                .or_default()
                .push(p);
        }
        for ((sx, sy, sz), positions) in sections {
            let holders = self.holders((sx, sz));
            if holders.is_empty() {
                continue;
            }
            let packet = if positions.len() == 1 {
                let (x, y, z) = positions[0];
                C::BlockUpdate(play::BlockUpdate {
                    pos: [x, y, z],
                    state: world.block(x, y, z).0 as i32,
                })
            } else {
                C::SectionBlocksUpdate(play::SectionBlocksUpdate {
                    section: [sx, sy, sz],
                    changes: positions
                        .iter()
                        .map(|&(x, y, z)| {
                            let state = i64::from(world.block(x, y, z).0);
                            state << 12
                                | i64::from(x & 15) << 8
                                | i64::from(z & 15) << 4
                                | i64::from(y & 15)
                        })
                        .collect(),
                })
            };
            self.send_to_holders(&holders, &packet.encode());
            // Block entities with data the client shows.
            for &(x, y, z) in &positions {
                let state = world.block(x, y, z).0;
                if !self
                    .encoder
                    .entity_matches(state, world.block_entities.get(&(x, y, z)))
                {
                    continue;
                }
                // A decorated pot is no block entity: its tag is the chunk's saved one.
                let state = world.block(x, y, z).0;
                let tag = match world.block_entities.get(&(x, y, z)) {
                    Some(entity) => self.encoder.change_tag((x, y, z), state, entity),
                    None => world
                        .chunks
                        .get(&(x >> 4, z >> 4))
                        .and_then(|c| crate::lan::chunks::pot_tag(c, (x, y, z))),
                };
                let Some(tag) = tag else {
                    continue;
                };
                let Some(kind) = self.encoder.entity_type(state) else {
                    continue;
                };
                let body = C::BlockEntityData(play::BlockEntityData {
                    pos: [x, y, z],
                    kind,
                    tag,
                })
                .encode();
                self.send_to_holders(&holders, &body);
            }
        }
    }

    /// [`Self::chunk_holders`] without the chunks they were sent.
    fn holders(&self, key: (i32, i32)) -> Vec<(usize, Option<Level>)> {
        self.chunk_holders(&self.config.dimension, key)
            .map(|(i, level, _)| (i, level))
            .collect()
    }
}
