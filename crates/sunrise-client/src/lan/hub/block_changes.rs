//! `ChunkHolder.broadcastChanges`: the light and blocks that changed in the level in place, to
//! the players sent those chunks, and, wrapped with the level, to the players whose client views
//! it (`viewed_levels`).
use super::{Guest, Hub};
use crate::{chunk::Chunk, lan::viewed_levels::send::Runs, world::World};
use sunrise_protocol::{
    connection_groups::{self, ChunkEntry, UpdateEntry},
    packets::{
        custom_payload::CustomPayload,
        play::{self, Clientbound as C},
    },
    viewed_levels::Level,
};

fn encode_payload(msg: &connection_groups::Message) -> Vec<u8> {
    let data = connection_groups::encode(msg)
        .expect("connection_groups::Message batches are capped at MAX_ENTRIES");
    C::CustomPayload(CustomPayload {
        channel: connection_groups::CHANNEL.into(),
        data,
    })
    .encode()
}

/// Encode one chunk's connection-group map into one or more `sunrise:connection_groups` play
/// clientbound custom-payload packet bodies. When `include_empty` is false (initial chunk send),
/// an empty chunk produces nothing; when true (bulk chunk resend), an empty chunk emits an
/// authoritative empty `replace: true` snapshot so clients clear any stale groups in unchanged
/// sections.
pub(in crate::lan) fn encode_chunk_connection_groups(
    chunk: &Chunk,
    include_empty: bool,
) -> Vec<Vec<u8>> {
    if chunk.connection_groups.is_empty() {
        if !include_empty {
            return Vec::new();
        }
        return vec![encode_payload(&connection_groups::Message::Chunk {
            cx: chunk.x,
            cz: chunk.z,
            replace: true,
            entries: Vec::new(),
        })];
    }
    let mut entries: Vec<ChunkEntry> = chunk
        .connection_groups
        .iter()
        .map(|(&cell, &group)| {
            let (local_x, y, local_z) = chunk.dynamic_cell_coords(cell);
            ChunkEntry::from_packed((local_x << 4) | local_z, y, group)
        })
        .collect();
    entries.sort_unstable_by_key(|e| (e.y(), e.local_z(), e.local_x()));
    entries
        .chunks(connection_groups::MAX_ENTRIES)
        .enumerate()
        .map(|(part, slice)| {
            encode_payload(&connection_groups::Message::Chunk {
                cx: chunk.x,
                cz: chunk.z,
                replace: part == 0,
                entries: slice.to_vec(),
            })
        })
        .collect()
}

impl Hub {
    /// The players sent chunk `key` of the level in place: those in it (None), and those whose
    /// client views it, with the level to wrap with. The chunks each was sent come with it.
    fn chunk_holders(
        &self,
        key: (i32, i32),
    ) -> Vec<(
        usize,
        Option<Level>,
        &crate::fast_hash::FxHashSet<(i32, i32)>,
    )> {
        let viewed = self.viewed.active();
        (0..self.guests.len())
            .filter_map(|i| {
                let g = &self.guests[i];
                if self.watches(g) {
                    g.sent_chunks
                        .contains(&key)
                        .then_some((i, None, &g.sent_chunks))
                } else if viewed {
                    self.view_here(g)
                        .filter(|v| v.sent.contains(&key))
                        .map(|v| (i, Some(v.level), &v.sent))
                } else {
                    None
                }
            })
            .collect()
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
                .chunk_holders((x, z))
                .into_iter()
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
        let bulk: Vec<(i32, i32)> = if world.bulk_log.is_empty() {
            Vec::new()
        } else {
            let mut keys: Vec<_> = world.bulk_log.drain().collect();
            keys.sort_unstable();
            keys
        };
        if !bulk.is_empty() {
            // The single-player window's far terrain draws a column it isn't sent as the window last saw it: it is
            // handed the column to summarize again (`handoff::changed`).
            let unsent = |g: &Guest, key: &(i32, i32)| {
                self.here(g)
                    && g.address == crate::lan::conn::MEMORY_ADDRESS
                    && !g.sent_chunks.contains(key)
            };
            for key in &bulk {
                if let Some(chunk) = world.chunks.get(key)
                    && self.guests.iter().any(|g| unsent(g, key))
                {
                    crate::lan::handoff::changed(chunk);
                }
            }
            let columns: Vec<&Chunk> = bulk
                .iter()
                .filter_map(|k| world.chunks.get(k).map(|c| &**c))
                .collect();
            self.encoder.warm(&columns);
            for chunk in columns {
                let key = (chunk.x, chunk.z);
                let holders = self.holders(key);
                if holders.is_empty() {
                    continue;
                }
                let body = self.encoder.body_in(chunk, world);
                self.send_to_holders(&holders, &body);
                if chunk.has_held_connection_groups {
                    let cg_holders: Vec<_> = holders
                        .iter()
                        .copied()
                        .filter(|&(i, _)| self.guests[i].negotiated_connection_groups())
                        .collect();
                    if !cg_holders.is_empty() {
                        for packet in encode_chunk_connection_groups(chunk, true) {
                            self.send_to_holders(&cg_holders, &packet);
                        }
                    }
                }
            }
        }
        let mut block_group_positions: Vec<(i32, i32, i32)> = Vec::new();
        let Some(log) = world.block_log.as_mut() else {
            self.broadcast_connection_group_changes(world, &bulk, block_group_positions);
            return;
        };
        if !bulk.is_empty() {
            // (what changed in them one at a time went with them, but for one-shot fields: a chunk has none)
            log.retain(|p| {
                bulk.binary_search(&(p.0 >> 4, p.2 >> 4)).is_err()
                    || matches!(
                        world.block_entities.get(p),
                        Some(crate::containers::BlockEntity::Mod(m)) if m.has_client_update_fields()
                    )
            });
        }
        if !log.is_empty() {
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
                if world
                    .chunks
                    .get(&(sx, sz))
                    .is_some_and(|c| c.has_held_connection_groups)
                    && holders
                        .iter()
                        .any(|&(i, _)| self.guests[i].negotiated_connection_groups())
                {
                    block_group_positions.extend(positions.iter().copied());
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
        self.broadcast_connection_group_changes(world, &bulk, block_group_positions);
    }

    /// Broadcast incremental connection-group updates for explicitly logged writes in
    /// `world.connection_group_log` and for sent block changes in chunks that have held a group
    /// (`block_group_positions`), reading each position's authoritative group from `world` at send
    /// time. Routed to negotiated holders (both in-level guests and viewed-level viewers).
    fn broadcast_connection_group_changes(
        &self,
        world: &mut World,
        bulk: &[(i32, i32)],
        mut positions: Vec<(i32, i32, i32)>,
    ) {
        if let Some(log) = world.connection_group_log.as_mut()
            && !log.is_empty()
        {
            positions.extend(log.drain(..));
        }
        if positions.is_empty() {
            return;
        }
        positions.sort_unstable();
        positions.dedup();
        // Group by chunk so we only send to negotiated holders that hold that chunk. Positions in
        // bulk-resent chunks were already covered by the authoritative chunk snapshot above.
        let mut by_chunk: std::collections::BTreeMap<(i32, i32), Vec<UpdateEntry>> =
            Default::default();
        for (x, y, z) in positions {
            let key = (x >> 4, z >> 4);
            if bulk.binary_search(&key).is_ok() {
                continue;
            }
            let group = world.connection_group(x, y, z);
            by_chunk.entry(key).or_default().push(UpdateEntry {
                pos: [x, y, z],
                group,
            });
        }
        if by_chunk.is_empty() {
            return;
        }
        // Batch per guest so one packet (or `sunrise:levels` run) covers all chunks that guest
        // holds in this level. (`Level` is not `Ord`, so key by guest index; each guest holds at
        // most one view of the level in place.)
        let mut per_guest: std::collections::BTreeMap<usize, (Option<Level>, Vec<UpdateEntry>)> =
            Default::default();
        for (key, entries) in by_chunk {
            for (i, level) in self.holders(key) {
                if self.guests[i].negotiated_connection_groups() {
                    per_guest
                        .entry(i)
                        .or_insert_with(|| (level, Vec::new()))
                        .1
                        .extend(entries.iter().copied());
                }
            }
        }
        for (i, (level, entries)) in per_guest {
            for slice in entries.chunks(connection_groups::MAX_ENTRIES) {
                let body = encode_payload(&connection_groups::Message::Update {
                    entries: slice.to_vec(),
                });
                self.send_to_holders(&[(i, level)], &body);
            }
        }
    }

    /// [`Self::chunk_holders`] without the chunks they were sent.
    fn holders(&self, key: (i32, i32)) -> Vec<(usize, Option<Level>)> {
        self.chunk_holders(key)
            .into_iter()
            .map(|(i, level, _)| (i, level))
            .collect()
    }
}
