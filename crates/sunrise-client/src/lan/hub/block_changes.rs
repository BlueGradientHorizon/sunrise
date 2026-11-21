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

/// Snapshot prefix of a column packet. An unprefixed column carries no groups.
pub(in crate::lan) fn encode_chunk_connection_groups(chunk: &Chunk) -> Vec<Vec<u8>> {
    if chunk.connection_groups.is_empty() {
        return Vec::new();
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

    /// Send a world-state correction directly to one guest, with its group at the same
    /// receive boundary. This does not publish the replacement to its other holders.
    pub fn send_authoritative_block(&self, conn: u32, world: &World, pos: [i32; 3]) {
        let Some(guest) = self.guest(conn) else {
            return;
        };
        let [x, y, z] = pos;
        let block = C::BlockUpdate(play::BlockUpdate {
            pos,
            state: world.block(x, y, z).0 as i32,
        });
        if guest.negotiated_connection_groups()
            && world
                .chunks
                .get(&(x >> 4, z >> 4))
                .is_some_and(|chunk| chunk.has_held_connection_groups)
        {
            let prefix = encode_payload(&connection_groups::Message::Blocks {
                entries: vec![UpdateEntry {
                    pos,
                    group: world.connection_group(x, y, z),
                }],
            });
            guest.sender.send_many([prefix, block.encode()]);
        } else {
            guest.send(&block);
        }
    }

    /// Publish the body to each holder; only negotiated peers get its metadata, and only if
    /// that scope accepts the body. Returns whether all negotiated holders got the body.
    fn send_to_holders_prefixed(
        &self,
        holders: &[(usize, Option<Level>)],
        prefix: &[Vec<u8>],
        body: &[u8],
    ) -> bool {
        let mut published = true;
        for &(i, level) in holders {
            let guest = &self.guests[i];
            let negotiated = !prefix.is_empty() && guest.negotiated_connection_groups();
            let prefix = if negotiated { prefix } else { &[] };
            if let Some(level) = level {
                let mut runs = Runs::new(level);
                if runs.push_prefixed(prefix, body) {
                    guest.sender.send_many(runs.finish());
                } else if guest.negotiated_connection_groups() {
                    published = false;
                }
            } else {
                guest
                    .sender
                    .send_many(prefix.iter().cloned().chain(std::iter::once(body.to_vec())));
            }
        }
        published
    }

    fn send_to_holders(&self, holders: &[(usize, Option<Level>)], body: &[u8]) {
        self.send_to_holders_prefixed(holders, &[], body);
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
            let mut published = Vec::new();
            let mut missed = Vec::new();
            for chunk in columns {
                let key = (chunk.x, chunk.z);
                let holders = self.holders(key);
                if holders.is_empty() {
                    published.push(key);
                    continue;
                }
                let body = self.encoder.body_in(chunk, world);
                let prefix = if holders
                    .iter()
                    .any(|&(i, _)| self.guests[i].negotiated_connection_groups())
                {
                    encode_chunk_connection_groups(chunk)
                } else {
                    Vec::new()
                };
                let mut all_published = true;
                let mut accepted = holders;
                accepted.retain(|&holder @ (i, _)| {
                    let sent = self.send_to_holders_prefixed(&[holder], &prefix, &body);
                    all_published &= sent;
                    sent && self.guests[i].negotiated_connection_groups()
                });
                if all_published {
                    published.push(key);
                } else {
                    missed.push((key, accepted));
                }
            }
            for key in published {
                world.group_blocks_sent(key, None);
            }
            for (key, accepted) in missed {
                let revision = world.group_column_missed(key);
                let dimension = &self.config.dimension;
                for (i, level) in accepted {
                    self.guests.update(i, |g| {
                        let snapshots = if level.is_some() {
                            &mut g
                                .viewed
                                .views
                                .get_mut(dimension)
                                .expect("holder views this level")
                                .group_snapshots
                        } else {
                            &mut g.group_snapshots
                        };
                        snapshots.insert(key, revision);
                    });
                }
            }
        }
        let Some(log) = world.block_log.as_mut() else {
            self.broadcast_connection_group_changes(world, &bulk);
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
                let key = (sx, sz);
                let holders = self.holders(key);
                if let Some(log) = &mut world.connection_group_log {
                    for pos in &positions {
                        log.remove(pos);
                    }
                }
                if holders.is_empty() {
                    world.group_blocks_sent(key, Some(&positions));
                    continue;
                }
                let prefix = if world
                    .chunks
                    .get(&key)
                    .is_some_and(|c| c.has_held_connection_groups)
                    && holders
                        .iter()
                        .any(|&(i, _)| self.guests[i].negotiated_connection_groups())
                {
                    vec![encode_payload(&connection_groups::Message::Blocks {
                        entries: positions
                            .iter()
                            .map(|&(x, y, z)| UpdateEntry {
                                pos: [x, y, z],
                                group: world.connection_group(x, y, z),
                            })
                            .collect(),
                    })]
                } else {
                    Vec::new()
                };
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
                if self.send_to_holders_prefixed(&holders, &prefix, &packet.encode()) {
                    world.group_blocks_sent(key, Some(&positions));
                }
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
        self.broadcast_connection_group_changes(world, &bulk);
    }

    /// Explicit writes with no companion block packet may only describe state the holders
    /// already have. A deferred replacement will publish the current group when its block
    /// is sent, not as a standalone delta to an older block.
    fn broadcast_connection_group_changes(&self, world: &mut World, bulk: &[(i32, i32)]) {
        let Some(log) = world.connection_group_log.as_mut() else {
            return;
        };
        if log.is_empty() {
            return;
        }
        let mut positions: Vec<_> = log.drain().collect();
        positions.sort_unstable();
        let mut by_chunk: std::collections::BTreeMap<(i32, i32), Vec<UpdateEntry>> =
            Default::default();
        for (x, y, z) in positions {
            let key = (x >> 4, z >> 4);
            if bulk.binary_search(&key).is_ok() || !world.chunks.contains_key(&key) {
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
                let guest = &self.guests[i];
                if !guest.negotiated_connection_groups() {
                    continue;
                }
                let snapshots = if level.is_some() {
                    &self
                        .view_here(guest)
                        .expect("holder views this level")
                        .group_snapshots
                } else {
                    &guest.group_snapshots
                };
                let sent_revision = snapshots.get(&key).copied().unwrap_or(0);
                per_guest
                    .entry(i)
                    .or_insert_with(|| (level, Vec::new()))
                    .1
                    .extend(entries.iter().copied().filter(|entry| {
                        let [x, y, z] = entry.pos;
                        world
                            .group_block_pending((x, y, z))
                            .is_none_or(|revision| revision <= sent_revision)
                    }));
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
