//! Non-blocking server chunk load/generation requests (`LevelRef::request_chunk`, #5234).
//!
//! Each active request holds a transient loading-only ticket at `ticket_levels::FULL` (level 33 —
//! accessible, not ticking) via `Streaming::add_async_request` and `Sources::viewed` (`ticking:
//! false, radius: 0`). Completions are captured on the tick thread during spare tick time
//! (`server_has_time()`) into immutable `ChunkSnapshot`s and queued per `(mod_id, level)` for
//! `LevelRef::drain_chunk_requests`. Pending and undrained requests owned by a disabled mod or a
//! closing level are dropped immediately and their tickets released.
use super::native_chunk_snapshot::EngineChunkSnapshot;
use super::{Dim, Game};
use mod_api::{ChunkLoadError, ChunkRequestRef, CompletedChunkRequest, DimensionKey, LevelRef};
use std::collections::VecDeque;
use sunrise_client::fast_hash::FxHashMap;
use sunrise_client::streaming::{FullColumn, Streaming, full_column_ready};

fn calling_mod() -> Option<&'static str> {
    mod_api::content::crash::current_native_mod()
}

fn mod_is_disabled(mod_id: &str) -> bool {
    mod_api::__private::mod_index(mod_id)
        .is_some_and(|index| !mod_api::__private::mod_index_on(index))
}

fn dim_of(key: DimensionKey) -> Dim {
    match key {
        DimensionKey::OVERWORLD => Dim::Overworld,
        DimensionKey::NETHER => Dim::Nether,
        DimensionKey::END => Dim::End,
        other => Dim::Extra(other),
    }
}

struct PendingChunkRequest {
    id: u64,
    mod_id: &'static str,
    level: LevelRef,
    pos: (i32, i32),
}

/// Per-server state for non-blocking mod chunk requests across all open dimensions.
#[derive(Default)]
pub(super) struct AsyncChunkRequests {
    next_id: u64,
    last_mods_switched: u64,
    pending: FxHashMap<Dim, VecDeque<PendingChunkRequest>>,
    /// Index of active pending requests by `(dim, request)` -> `(mod_id, pos)` so `cancel` is
    /// O(1) (lazy tombstoning on `pending` queues avoids O(N) linear scan and shift).
    pending_by_handle: FxHashMap<(Dim, ChunkRequestRef), (&'static str, (i32, i32))>,
    /// Active pending request count per `(dim, chunk)` so overlapping requests from multiple mods
    /// (or repeated requests from one mod) release the level-33 ticket only when the last holder
    /// completes or cancels.
    holders: FxHashMap<(Dim, (i32, i32)), usize>,
    /// Count of active (non-tombstoned) pending requests per dimension.
    active_counts: FxHashMap<Dim, usize>,
    completed: FxHashMap<(&'static str, LevelRef), Vec<CompletedChunkRequest>>,
}

impl AsyncChunkRequests {
    pub(super) fn has_pending_in(&self, dim: Dim) -> bool {
        self.active_counts.get(&dim).copied().unwrap_or(0) > 0
    }

    fn dec_active(&mut self, dim: Dim) {
        if let Some(count) = self.active_counts.get_mut(&dim) {
            if *count <= 1 {
                self.active_counts.remove(&dim);
            } else {
                *count -= 1;
            }
        }
    }

    fn add_holder(&mut self, dim: Dim, pos: (i32, i32)) -> bool {
        let count = self.holders.entry((dim, pos)).or_insert(0);
        *count += 1;
        *count == 1
    }

    fn release_holder(&mut self, dim: Dim, pos: (i32, i32)) -> bool {
        let Some(count) = self.holders.get_mut(&(dim, pos)) else {
            return false;
        };
        if *count <= 1 {
            self.holders.remove(&(dim, pos));
            true
        } else {
            *count -= 1;
            false
        }
    }
}

impl Game<'_> {
    fn native_server_level_dim(&self, level: LevelRef) -> Option<Dim> {
        self.native_level_world(level)?;
        Some(dim_of(level.dimension()))
    }

    fn level_streaming_and_world(
        &self,
        dim: Dim,
    ) -> Option<(&Streaming, &sunrise_client::world::World)> {
        if dim == self.dim {
            Some((&self.streaming, &self.world))
        } else {
            self.levels
                .parked
                .get(&dim)
                .map(|p| (&p.streaming, &p.world))
        }
    }

    fn level_streaming_mut(&mut self, dim: Dim) -> Option<&mut Streaming> {
        if dim == self.dim {
            Some(&mut self.streaming)
        } else {
            self.levels.parked.get_mut(&dim).map(|p| &mut p.streaming)
        }
    }

    /// Whether `dim` has pending non-blocking mod chunk requests that need a streaming pass.
    pub(super) fn has_async_chunk_requests(&self, dim: Dim) -> bool {
        self.async_chunk_requests.has_pending_in(dim)
    }

    /// Enqueue a non-blocking server chunk request (`LevelRef::request_chunk`).
    pub(super) fn level_request_chunk(
        &mut self,
        level: LevelRef,
        pos: [i32; 2],
    ) -> Result<ChunkRequestRef, ChunkLoadError> {
        let Some(mod_id) = calling_mod() else {
            return Err(ChunkLoadError::Unloaded);
        };
        if mod_is_disabled(mod_id) {
            return Err(ChunkLoadError::Unloaded);
        }
        let Some(dim) = self.native_server_level_dim(level) else {
            return Err(ChunkLoadError::Unloaded);
        };
        let chunk = (pos[0], pos[1]);
        let status = {
            let Some((streaming, world)) = self.level_streaming_and_world(dim) else {
                return Err(ChunkLoadError::Unloaded);
            };
            streaming.check_async_request(world, chunk)
        };
        match status {
            FullColumn::Refused => return Err(ChunkLoadError::Unloaded),
            FullColumn::Failed => return Err(ChunkLoadError::Failed),
            FullColumn::Ready | FullColumn::Pending => {}
        }

        self.async_chunk_requests.next_id =
            self.async_chunk_requests.next_id.wrapping_add(1).max(1);
        let id = self.async_chunk_requests.next_id;
        let first_holder = self.async_chunk_requests.add_holder(dim, chunk);
        if first_holder && let Some(streaming) = self.level_streaming_mut(dim) {
            streaming.add_async_request(chunk);
        }
        let handle = mod_api::__private::game::chunk_request_ref(level, id);
        self.async_chunk_requests
            .pending_by_handle
            .insert((dim, handle), (mod_id, chunk));
        *self
            .async_chunk_requests
            .active_counts
            .entry(dim)
            .or_insert(0) += 1;
        self.async_chunk_requests
            .pending
            .entry(dim)
            .or_default()
            .push_back(PendingChunkRequest {
                id,
                mod_id,
                level,
                pos: chunk,
            });
        Ok(handle)
    }

    /// Drain all completed or failed chunk requests owned by the calling mod on `level`.
    pub(super) fn level_drain_chunk_requests(
        &mut self,
        level: LevelRef,
    ) -> Option<Vec<CompletedChunkRequest>> {
        let mod_id = calling_mod()?;
        self.native_server_level_dim(level)?;
        self.prune_disabled_async_chunk_requests();
        if mod_is_disabled(mod_id) {
            return None;
        }
        Some(
            self.async_chunk_requests
                .completed
                .remove(&(mod_id, level))
                .unwrap_or_default(),
        )
    }

    /// Cancel a pending or undrained chunk request owned by the calling mod and release its
    /// level-33 ticket immediately if no other request still holds that chunk.
    pub(super) fn chunk_request_cancel(&mut self, request: ChunkRequestRef) -> bool {
        let Some(mod_id) = calling_mod() else {
            return false;
        };
        let level = request.level();
        let Some(dim) = self.native_server_level_dim(level) else {
            return false;
        };
        self.prune_disabled_async_chunk_requests();
        if let Some(&(owner, pos)) = self
            .async_chunk_requests
            .pending_by_handle
            .get(&(dim, request))
            && owner == mod_id
        {
            self.async_chunk_requests
                .pending_by_handle
                .remove(&(dim, request));
            self.async_chunk_requests.dec_active(dim);
            if self.async_chunk_requests.release_holder(dim, pos) {
                if let Some(streaming) = self.level_streaming_mut(dim) {
                    streaming.remove_async_request(pos);
                }
                if dim == self.dim {
                    self.rebuild_chunk_tickets(false);
                }
            }
            if !self.async_chunk_requests.has_pending_in(dim) {
                self.async_chunk_requests.pending.remove(&dim);
            }
            return true;
        }
        if let Some(done) = self
            .async_chunk_requests
            .completed
            .get_mut(&(mod_id, level))
            && let Some(idx) = done.iter().position(|entry| entry.request == request)
        {
            done.swap_remove(idx);
            if done.is_empty() {
                self.async_chunk_requests.completed.remove(&(mod_id, level));
            }
            return true;
        }
        false
    }

    /// Drop pending and undrained requests owned by any mod that has been disabled, releasing
    /// their level-33 tickets immediately. O(1) when no mod was toggled since the last check.
    pub(super) fn prune_disabled_async_chunk_requests(&mut self) {
        let switched = mod_api::__private::mods_switched();
        if self.async_chunk_requests.last_mods_switched == switched {
            return;
        }
        self.async_chunk_requests.last_mods_switched = switched;
        self.prune_async_chunk_requests_where(mod_is_disabled);
    }

    fn prune_async_chunk_requests_where(&mut self, is_disabled: impl Fn(&str) -> bool) {
        if self.async_chunk_requests.pending.is_empty()
            && self.async_chunk_requests.completed.is_empty()
        {
            return;
        }
        let mut released: Vec<(Dim, (i32, i32))> = Vec::new();
        let mut empty_dims: Vec<Dim> = Vec::new();
        let pending_by_handle = &mut self.async_chunk_requests.pending_by_handle;
        let active_counts = &mut self.async_chunk_requests.active_counts;
        for (&dim, queue) in &mut self.async_chunk_requests.pending {
            queue.retain(|req| {
                let handle = mod_api::__private::game::chunk_request_ref(req.level, req.id);
                if !pending_by_handle.contains_key(&(dim, handle)) {
                    return false;
                }
                if is_disabled(req.mod_id) {
                    pending_by_handle.remove(&(dim, handle));
                    if let Some(count) = active_counts.get_mut(&dim) {
                        if *count <= 1 {
                            active_counts.remove(&dim);
                        } else {
                            *count -= 1;
                        }
                    }
                    released.push((dim, req.pos));
                    false
                } else {
                    true
                }
            });
            if queue.is_empty() {
                empty_dims.push(dim);
            }
        }
        for dim in empty_dims {
            self.async_chunk_requests.pending.remove(&dim);
        }
        for (dim, pos) in released {
            if self.async_chunk_requests.release_holder(dim, pos)
                && let Some(streaming) = self.level_streaming_mut(dim)
            {
                streaming.remove_async_request(pos);
            }
        }
        self.async_chunk_requests
            .completed
            .retain(|&(mod_id, _), _| !is_disabled(mod_id));
    }

    /// Drop all pending and undrained requests for a closing level and clear its streaming tickets.
    pub(super) fn clear_async_chunk_requests_for_dim(&mut self, dim: Dim) {
        self.async_chunk_requests.pending.remove(&dim);
        self.async_chunk_requests
            .pending_by_handle
            .retain(|&(d, _), _| d != dim);
        self.async_chunk_requests.active_counts.remove(&dim);
        self.async_chunk_requests
            .holders
            .retain(|&(holder_dim, _), _| holder_dim != dim);
        let key = dim.key();
        self.async_chunk_requests
            .completed
            .retain(|&(_, level), _| level.dimension() != key);
        if let Some(streaming) = self.level_streaming_mut(dim) {
            streaming.clear_async_requests();
        }
    }

    /// Drop all pending and undrained async chunk requests across every dimension.
    pub(super) fn clear_all_async_chunk_requests(&mut self) {
        self.async_chunk_requests.pending.clear();
        self.async_chunk_requests.pending_by_handle.clear();
        self.async_chunk_requests.active_counts.clear();
        self.async_chunk_requests.holders.clear();
        self.async_chunk_requests.completed.clear();
        self.streaming.clear_async_requests();
        for parked in self.levels.parked.values_mut() {
            parked.streaming.clear_async_requests();
        }
    }

    /// Capture ready (or failed) pending async chunk requests for the level in place (`self.dim`)
    /// during spare tick time (`server_has_time()`), releasing their level-33 tickets once
    /// captured so the chunks unload and save through vanilla's ordinary path.
    pub(super) fn process_async_chunk_requests(&mut self) {
        self.prune_disabled_async_chunk_requests();
        let dim = self.dim;
        if !self.async_chunk_requests.has_pending_in(dim) {
            return;
        }
        let Some(states) = super::api_values::block_state::current().ok() else {
            return;
        };
        let biomes = world_biomes!(self).clone();
        let properties = self.pack.assets.properties.clone();

        let Some(mut queue) = self.async_chunk_requests.pending.remove(&dim) else {
            return;
        };
        let total = queue.len();
        let mut processed = 0usize;
        let mut scanned = 0usize;
        let mut released_chunks: Vec<(i32, i32)> = Vec::new();

        while scanned < total {
            let Some(req) = queue.pop_front() else {
                break;
            };
            scanned += 1;
            let handle = mod_api::__private::game::chunk_request_ref(req.level, req.id);
            if !self
                .async_chunk_requests
                .pending_by_handle
                .contains_key(&(dim, handle))
            {
                // Cancelled lazily via `chunk_request_cancel`.
                continue;
            }
            if self.streaming.full_column_failed(req.pos) {
                self.async_chunk_requests
                    .pending_by_handle
                    .remove(&(dim, handle));
                self.async_chunk_requests.dec_active(dim);
                released_chunks.push(req.pos);
                self.async_chunk_requests
                    .completed
                    .entry((req.mod_id, req.level))
                    .or_default()
                    .push(mod_api::__private::game::completed_chunk_request(
                        handle,
                        [req.pos.0, req.pos.1],
                        Err(ChunkLoadError::Failed),
                    ));
                processed += 1;
                if !self.server_has_time() {
                    break;
                }
            } else if full_column_ready(&self.world, req.pos)
                && let Some(column) = self.world.chunks.get(&req.pos)
            {
                self.async_chunk_requests
                    .pending_by_handle
                    .remove(&(dim, handle));
                self.async_chunk_requests.dec_active(dim);
                let has_light = column.lit || column.light_loaded;
                let snapshot = EngineChunkSnapshot::from_chunk(
                    column,
                    self.world.shape,
                    states.clone(),
                    biomes.clone(),
                    properties.clone(),
                    has_light,
                );
                released_chunks.push(req.pos);
                self.async_chunk_requests
                    .completed
                    .entry((req.mod_id, req.level))
                    .or_default()
                    .push(mod_api::__private::game::completed_chunk_request(
                        handle,
                        [req.pos.0, req.pos.1],
                        Ok(snapshot),
                    ));
                processed += 1;
                if !self.server_has_time() {
                    break;
                }
            } else {
                queue.push_back(req);
                if scanned.is_multiple_of(64) && !self.server_has_time() {
                    break;
                }
            }
        }

        if !queue.is_empty() {
            self.async_chunk_requests.pending.insert(dim, queue);
        }
        let mut released_any = false;
        for pos in released_chunks {
            if self.async_chunk_requests.release_holder(dim, pos) {
                self.streaming.remove_async_request(pos);
                released_any = true;
            }
        }
        if released_any {
            self.rebuild_chunk_tickets(false);
        }
        let _ = processed;
    }
}

#[cfg(test)]
mod tests {
    use crate::game::native_entity_refs::tests::with_game;
    use mod_api::ChunkLoadError;
    use std::sync::Arc;
    use std::time::{Duration, Instant};
    use sunrise_client::chunk::Chunk;

    fn insert_lit_stone_chunk(game: &mut crate::game::Game<'_>, pos: (i32, i32)) {
        let stone_key = mod_api::__private::block_key("minecraft:stone").unwrap();
        let stone_id = super::super::api_values::default_state(stone_key)
            .unwrap()
            .0;
        let mut column = Chunk::empty(pos.0, pos.1);
        column.lit = true;
        Arc::make_mut(&mut column.sections[4]).set(0, stone_id);
        game.world.chunks.insert(pos, Arc::new(column));
    }

    #[test]
    #[ignore = "daily"]
    fn request_chunk_is_non_blocking_level_33_and_respects_spare_time_budget() {
        with_game(|game| {
            game.streaming.paused = false;
            let level = game.level("minecraft:overworld").unwrap();
            let stone_key = mod_api::__private::block_key("minecraft:stone").unwrap();

            // 1. Non-blocking request queues level-33 ticket (accessible, NOT ticking).
            let req = {
                let _g = mod_api::__private::enter("mod_a", "test");
                level
                    .request_chunk(game, [12, -8])
                    .expect("request admitted")
            };
            assert!(game.streaming.has_async_requests());
            game.rebuild_chunk_tickets(false);
            let levels = game.tickets.levels();
            assert!(!levels.entity_ticking(12, -8));
            assert!(!levels.block_ticking(12, -8));
            assert!(!levels.random_ticking(12, -8));
            assert!(!levels.loaded_ticking(12, -8));

            // 2. While not ready, drain returns empty Some(vec![]).
            {
                let _g = mod_api::__private::enter("mod_a", "test");
                assert!(level.drain_chunk_requests(game).unwrap().is_empty());
            }

            // 3. Once resident & lit, process captures ChunkSnapshot and releases the ticket.
            insert_lit_stone_chunk(game, (12, -8));
            game.process_async_chunk_requests();
            assert!(!game.streaming.has_async_requests());
            {
                let _g = mod_api::__private::enter("mod_a", "test");
                let done = level.drain_chunk_requests(game).unwrap();
                assert_eq!(done.len(), 1);
                assert_eq!(done[0].request, req);
                assert_eq!(done[0].pos, [12, -8]);
                let snap = done[0].result.as_ref().expect("snapshot ok");
                assert_eq!(snap.block_state(0, 0, 0).unwrap().block(), stone_key);
                assert!(snap.has_light());
                // Draining again is empty.
                assert!(level.drain_chunk_requests(game).unwrap().is_empty());
            }

            // 4. Spare-time budget: when deadline already passed, a burst of 5 ready chunks
            // completes exactly 1 per tick (guaranteed forward progress without stalling).
            for x in 20..25 {
                insert_lit_stone_chunk(game, (x, 0));
                let _g = mod_api::__private::enter("mod_a", "test");
                level.request_chunk(game, [x, 0]).unwrap();
            }
            game.server_tick_deadline = Some(Instant::now() - Duration::from_millis(1));
            game.process_async_chunk_requests();
            {
                let _g = mod_api::__private::enter("mod_a", "test");
                assert_eq!(level.drain_chunk_requests(game).unwrap().len(), 1);
            }
            game.process_async_chunk_requests();
            {
                let _g = mod_api::__private::enter("mod_a", "test");
                assert_eq!(level.drain_chunk_requests(game).unwrap().len(), 1);
            }
            // Restoring spare time completes the remaining 3 in one tick.
            game.server_tick_deadline = None;
            game.process_async_chunk_requests();
            {
                let _g = mod_api::__private::enter("mod_a", "test");
                assert_eq!(level.drain_chunk_requests(game).unwrap().len(), 3);
            }
            assert!(!game.streaming.has_async_requests());

            // 5. Out-of-bounds chunk position refused cleanly.
            {
                let _g = mod_api::__private::enter("mod_a", "test");
                assert_eq!(
                    level.request_chunk(game, [i32::MAX, 0]),
                    Err(ChunkLoadError::Unloaded)
                );
            }
        });
    }

    #[test]
    #[ignore = "daily"]
    fn cancel_isolation_mod_disable_and_level_close_release_tickets() {
        with_game(|game| {
            game.streaming.paused = false;
            let level = game.level("minecraft:overworld").unwrap();

            // Per-mod isolation & overlapping chunk refcount: both mod_a and mod_b request [5, 5].
            let req_a = {
                let _g = mod_api::__private::enter("mod_a", "test");
                level.request_chunk(game, [5, 5]).unwrap()
            };
            let req_b = {
                let _g = mod_api::__private::enter("mod_b", "test");
                level.request_chunk(game, [5, 5]).unwrap()
            };
            // mod_b cannot cancel mod_a's request.
            {
                let _g = mod_api::__private::enter("mod_b", "test");
                assert!(!req_a.cancel(game));
            }
            // mod_a cancels its request: ticket stays alive for mod_b!
            {
                let _g = mod_api::__private::enter("mod_a", "test");
                assert!(req_a.cancel(game));
                assert!(!req_a.cancel(game));
            }
            assert!(game.streaming.has_async_requests());

            // Complete [5, 5]: only mod_b receives it, not mod_a.
            insert_lit_stone_chunk(game, (5, 5));
            game.process_async_chunk_requests();
            assert!(!game.streaming.has_async_requests());
            {
                let _g = mod_api::__private::enter("mod_a", "test");
                assert!(level.drain_chunk_requests(game).unwrap().is_empty());
            }
            // Cancel an undrained completed request before draining: removes it from queue.
            {
                let _g = mod_api::__private::enter("mod_b", "test");
                assert!(req_b.cancel(game));
                assert!(level.drain_chunk_requests(game).unwrap().is_empty());
            }

            // Mod disable drops both pending and undrained completed requests for that mod only
            // and releases its streaming tickets immediately without touching mod_b.
            {
                let _g = mod_api::__private::enter("mod_a", "test");
                level.request_chunk(game, [7, 7]).unwrap();
                level.request_chunk(game, [8, 8]).unwrap();
            }
            {
                let _g = mod_api::__private::enter("mod_b", "test");
                level.request_chunk(game, [10, 10]).unwrap();
            }
            insert_lit_stone_chunk(game, (7, 7));
            game.process_async_chunk_requests();
            // mod_a now has 1 completed ([7, 7]) and 1 pending ([8, 8]); mod_b has 1 pending ([10, 10]).
            game.prune_async_chunk_requests_where(|id| id == "mod_a");
            {
                let _g = mod_api::__private::enter("mod_a", "test");
                assert!(level.drain_chunk_requests(game).unwrap().is_empty());
            }
            let remaining: Vec<_> = game.streaming.async_requests().collect();
            assert_eq!(remaining, vec![(10, 10)]);

            // Level close drops pending requests and releases tickets.
            let dim = game.dim;
            game.clear_async_chunk_requests_for_dim(dim);
            assert!(!game.streaming.has_async_requests());
            assert!(!game.has_async_chunk_requests(crate::game::Dim::Overworld));
        });
    }
}
