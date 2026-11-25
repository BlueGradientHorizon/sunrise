#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{Game, loading_frame};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::saving::CLOSING_SNAPSHOT_BUDGET;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::saving::CLOSING_SNAPSHOTS_PER_FRAME;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::saving::ClientQuit;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::saving::ClosingSave;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::saving::SNAPSHOT_BUDGET;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::saving::SNAPSHOTS_PER_FRAME;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::saving::Stage;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::saving::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::{
    collections::{HashSet, VecDeque},
    sync::mpsc::{Receiver, TryRecvError},
    time::{Duration, Instant},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{Result, hud::HudBatch};

impl Game<'_> {
    #[inline(always)]
    pub(in crate::game) fn poll_closing_save_chunks_server(
        &mut self,
        keys: &mut VecDeque<(i32, i32)>,
        per_frame: usize,
        budget: Duration,
    ) -> Result<Option<VecDeque<(i32, i32)>>> {
        let started = Instant::now();
        for _ in 0..per_frame {
            let Some(&key) = keys.front() else { break };
            // ChunkMap.save consumes the dirty mark before capturing the snapshot.
            self.world.modified.remove(&key);
            let chunks = self.with_entities(self.world.chunks.get(&key));
            self.native_full_chunk_snapshots(&chunks);
            self.save
                .0
                .save_later(&chunks, self.pack.assets.registry, world_biomes!(self))?;
            keys.pop_front();
            if started.elapsed() >= budget {
                break;
            }
        }
        if keys.is_empty() {
            self.spawn_loaded_entities();
            Ok(Some(self.world.chunks.keys().copied().collect()))
        } else {
            Ok(None)
        }
    }
    #[inline(always)]
    pub(in crate::game) fn poll_closing_save_entities_server(
        &mut self,
        keys: &mut VecDeque<(i32, i32)>,
        per_frame: usize,
        budget: Duration,
    ) -> Result<Option<Receiver<std::io::Result<()>>>> {
        let started = Instant::now();
        for _ in 0..per_frame {
            let Some(&key) = keys.front() else { break };
            let chunks = self.entity_chunks(&HashSet::from([key]), false);
            self.save.0.save_entities_later(chunks)?;
            keys.pop_front();
            if started.elapsed() >= budget {
                break;
            }
        }
        if keys.is_empty() {
            self.prune_entity_memory();
            self.save_raids();
            self.save.0.save_attachments(&self.world.attachments)?;
            self.save_chunk_tickets()?;
            self.save_game_metadata()?;
            Ok(Some(self.save.0.flush_later()?))
        } else {
            Ok(None)
        }
    }
    #[inline(always)]
    pub(in crate::game) fn poll_closing_save_flush_server(
        &self,
    ) -> Result<Receiver<std::io::Result<()>>> {
        self.save.0.flush_later()
    }
    /// Save the level's chunk tickets: `/forceload`'s in vanilla's `chunk_tickets.dat`, the
    /// mods' in Sunrise's own `mod_chunk_tickets.dat`.
    pub(in crate::game::server_side) fn save_chunk_tickets(&self) -> Result<()> {
        if self.server.startup_ticket_restore_pending {
            return Ok(());
        }
        self.save
            .0
            .save_chunk_tickets(self.streaming.commanded(), self.tickets.0.portal_tickets())?;
        self.save
            .0
            .save_mod_tickets(self.streaming.mod_tickets(), |slot| {
                mod_api::TicketController::from_slot(slot).map(|c| c.id())
            })
    }
    /// Save the level in place: its changed chunks, its entities and its raids.
    pub(in crate::game) fn save_level(&mut self, wait: bool) -> Result<()> {
        self.admit_level_save()?;
        // No save holds a player vehicle: a mob a mod seated on a player gets off first.
        self.eject_player_riders(None);
        self.save_border();
        self.save_chunk_tickets()?;
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            match self.streaming.prepare_save(&mut self.world, wait, deadline) {
                sunrise_client::streaming::SaveStep::Full(chunk) => {
                    self.native_late_full_chunk_snapshot(&chunk);
                    self.streaming.save_finished(chunk);
                }
                sunrise_client::streaming::SaveStep::Protos(protos) => {
                    self.native_proto_snapshots(&protos);
                    self.streaming.finish_save(protos, wait);
                    break;
                }
                sunrise_client::streaming::SaveStep::Pending => unreachable!(),
            }
        }
        let keys: Vec<_> = self.world.modified.iter().copied().collect();
        self.save.0.save_attachments(&self.world.attachments)?;
        // ChunkMap.save calls tryMarkSaved before creating the snapshot. A record failure
        // finishes this attempt and does not put the mark back.
        for key in &keys {
            self.world.modified.remove(key);
        }
        let chunks = self.with_entities(keys.iter().filter_map(|k| self.world.chunks.get(k)));
        self.native_full_chunk_snapshots(&chunks);
        self.save
            .0
            .save_later(&chunks, self.pack.assets.registry, world_biomes!(self))?;
        // Prepare entities while the chunk writer runs, then use one barrier.
        if !wait {
            self.world.modified.autosaved(&keys, Instant::now());
        }
        self.save_entities(false)?;
        self.save_raids();
        if wait {
            self.save.0.flush()?;
        }
        if mod_api::__private::listened(mod_api::HookId::LevelSave) {
            self.native_level_save(self.dim.id());
        }
        Ok(())
    }
}
