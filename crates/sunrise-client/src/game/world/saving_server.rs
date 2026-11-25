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

impl Game<'_, crate::game::server_side::ServerState> {
    #[inline(always)]
    #[allow(dead_code)]
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
            self.side
                .save
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
    #[allow(dead_code)]
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
            self.side.save.0.save_entities_later(chunks)?;
            keys.pop_front();
            if started.elapsed() >= budget {
                break;
            }
        }
        if keys.is_empty() {
            self.prune_entity_memory();
            self.save_raids();
            self.side.save.0.save_attachments(&self.world.attachments)?;
            self.save_chunk_tickets()?;
            self.save_game_metadata()?;
            Ok(Some(self.side.save.0.flush_later()?))
        } else {
            Ok(None)
        }
    }
    #[inline(always)]
    #[allow(dead_code)]
    pub(in crate::game) fn poll_closing_save_flush_server(
        &self,
    ) -> Result<Receiver<std::io::Result<()>>> {
        self.side.save.0.flush_later()
    }
    /// Save the level's chunk tickets: `/forceload`'s in vanilla's `chunk_tickets.dat`, the
    /// mods' in Sunrise's own `mod_chunk_tickets.dat`.
    pub(in crate::game::server_side) fn save_chunk_tickets(&self) -> Result<()> {
        if self.side.startup_ticket_restore_pending {
            return Ok(());
        }
        self.side.save.0.save_chunk_tickets(
            self.streaming.commanded(),
            self.side.tickets.0.portal_tickets(),
        )?;
        self.side
            .save
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
        self.side.save.0.save_attachments(&self.world.attachments)?;
        // ChunkMap.save calls tryMarkSaved before creating the snapshot. A record failure
        // finishes this attempt and does not put the mark back.
        for key in &keys {
            self.world.modified.remove(key);
        }
        let chunks = self.with_entities(keys.iter().filter_map(|k| self.world.chunks.get(k)));
        self.native_full_chunk_snapshots(&chunks);
        self.side
            .save
            .0
            .save_later(&chunks, self.pack.assets.registry, world_biomes!(self))?;
        // Prepare entities while the chunk writer runs, then use one barrier.
        if !wait {
            self.world.modified.autosaved(&keys, Instant::now());
        }
        self.save_entities(false)?;
        self.save_raids();
        if wait {
            self.side.save.0.flush()?;
        }
        if mod_api::__private::listened(mod_api::HookId::LevelSave) {
            self.native_level_save(self.dim.id());
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub(in crate::game) fn begin_server_control_save(
        &mut self,
        all: bool,
        reply: std::sync::mpsc::Sender<String>,
    ) -> Result<ClosingSave> {
        if all {
            self.world
                .modified
                .extend(self.world.chunks.keys().copied());
        }
        Ok(ClosingSave {
            quit: false,
            close: false,
            reply: Some(reply),
            started: Instant::now(),
            stage: Stage::Generation,
        })
    }

    /// Advance a server-owned staged save by one step. Returns `Ok(true)` once finished.
    #[allow(dead_code)]
    pub(in crate::game) fn poll_server_closing_save(
        &mut self,
        save: &mut ClosingSave,
    ) -> Result<bool> {
        let defers = !matches!(save.stage, Stage::Server | Stage::Quit)
            && save.started.elapsed() < Duration::from_secs(30);
        if defers && self.defer_chunk_disk_save() {
            return Ok(false);
        }
        let (per_frame, budget) = if save.close {
            (CLOSING_SNAPSHOTS_PER_FRAME, CLOSING_SNAPSHOT_BUDGET)
        } else {
            (SNAPSHOTS_PER_FRAME, SNAPSHOT_BUDGET)
        };
        let result = (|| -> Result<bool> {
            match &mut save.stage {
                Stage::Quit | Stage::Server => Ok(true),
                Stage::Generation => {
                    if save.started.elapsed() > Duration::from_secs(30) {
                        return Err("chunk generation did not finish before saving".into());
                    }
                    let deadline = Instant::now() + budget;
                    loop {
                        match self.streaming.prepare_poll_save(&mut self.world, deadline) {
                            sunrise_client::streaming::SaveStep::Pending => break,
                            sunrise_client::streaming::SaveStep::Full(chunk) => {
                                self.native_late_full_chunk_snapshot(&chunk);
                                self.streaming.save_finished(chunk);
                            }
                            sunrise_client::streaming::SaveStep::Protos(protos) => {
                                self.native_proto_snapshots(&protos);
                                if self.streaming.submit_protos(protos) {
                                    save.stage = Stage::Chunks(
                                        self.world.modified.iter().copied().collect(),
                                    );
                                }
                                break;
                            }
                        }
                    }
                    Ok(false)
                }
                Stage::Chunks(keys) => {
                    if let Some(entities) =
                        self.poll_closing_save_chunks_server(keys, per_frame, budget)?
                    {
                        save.stage = Stage::Entities(entities);
                    }
                    Ok(false)
                }
                Stage::Entities(keys) => {
                    if let Some(flush) =
                        self.poll_closing_save_entities_server(keys, per_frame, budget)?
                    {
                        save.stage = Stage::Writes(Some(flush));
                    }
                    Ok(false)
                }
                Stage::Writes(flush) => {
                    let Some(receive) = flush else {
                        *flush = Some(self.poll_closing_save_flush_server()?);
                        return Ok(false);
                    };
                    if !self.finish_player_autosaves(false)? {
                        return Ok(false);
                    }
                    let result = match receive.try_recv() {
                        Ok(result) => result,
                        Err(TryRecvError::Empty) => return Ok(false),
                        Err(TryRecvError::Disconnected) => {
                            Err(std::io::Error::other("the region writer stopped"))
                        }
                    };
                    *flush = None;
                    result?;
                    Ok(true)
                }
            }
        })();
        match result {
            Ok(true) => {
                if let Some(reply) = save.reply.take() {
                    let _ = reply.send(serde_json::json!({ "ok": true }).to_string());
                }
                Ok(true)
            }
            Err(error) => {
                if let Some(reply) = save.reply.take() {
                    let _ =
                        reply.send(serde_json::json!({ "error": error.to_string() }).to_string());
                }
                Err(error)
            }
            Ok(false) => Ok(false),
        }
    }

    #[cfg(test)]
    pub(in crate::game) fn test_begin_server_control_save(
        &mut self,
        all: bool,
        reply: std::sync::mpsc::Sender<String>,
    ) -> Result<ClosingSave> {
        self.begin_server_control_save(all, reply)
    }

    #[cfg(test)]
    pub(in crate::game) fn test_poll_server_closing_save(
        &mut self,
        save: &mut ClosingSave,
    ) -> Result<bool> {
        self.poll_server_closing_save(save)
    }
}

#[cfg(test)]
pub(in crate::game) mod tests {
    use super::*;
    use crate::game::native_entity_refs::tests::with_game;
    use std::sync::{Arc, mpsc};
    use sunrise_client::save::{LevelShape, SaveDir};
    use sunrise_core::nbt::{DISK_WRITE_NESTING_GUARD, Tag};

    pub(in crate::game) fn finish_server_staged_save(
        game: &mut crate::game::ServerGame<'_>,
        save: &mut ClosingSave,
    ) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if game.poll_server_closing_save(save).unwrap() {
                return save.quit;
            }
            assert!(
                Instant::now() < deadline,
                "the region writer never acknowledged the save"
            );
            std::thread::yield_now();
        }
    }

    #[test]
    #[ignore = "daily"]
    fn both_successful_save_paths_resolve_the_banner_in_the_active_language() {
        use sunrise_client::lang::{Language, scoped};

        mod_platform::testing::with_mods(Vec::new().into(), || {
            with_game(|game| {
                let dir = tempfile::tempdir().unwrap();
                game.side.world_root = dir.path().to_path_buf();
                game.side.save.0 = Arc::new(SaveDir::new(
                    game.side.world_root.clone(),
                    LevelShape::OVERWORLD,
                    Some(Default::default()),
                ));
                let translated = "[test locale]";
                let language = Language::of(
                    "en_test",
                    [("commands.save.success".into(), translated.into())],
                );
                scoped(language, || {
                    game.save_game().unwrap();
                    let explicit = sunrise_client::lang::get("commands.save.success");
                    let mut save = ClosingSave {
                        quit: false,
                        close: false,
                        reply: None,
                        started: Instant::now(),
                        stage: Stage::Writes(Some(game.side.save.0.flush_later().unwrap())),
                    };
                    assert!(!finish_server_staged_save(game, &mut save));
                    let incremental = sunrise_client::lang::get("commands.save.success");
                    assert_eq!(
                        (explicit.as_str(), incremental.as_str()),
                        (translated, translated)
                    );
                });
            });
        });
    }

    #[test]
    #[ignore = "daily"]
    fn closing_and_control_saves_complete_after_reporting_a_record_failure() {
        mod_platform::testing::with_mods(Vec::new().into(), || {
            with_game(|game| {
                for quit in [false, true] {
                    let dir = tempfile::tempdir().unwrap();
                    game.side.save.0 = Arc::new(SaveDir::new(
                        dir.path().to_path_buf(),
                        LevelShape::OVERWORLD,
                        None,
                    ));
                    let invalid = (0..=DISK_WRITE_NESTING_GUARD)
                        .fold(Tag::Int(0), |nested, _| Tag::List(vec![nested]));
                    game.side
                        .save
                        .0
                        .save_entities(vec![((2, 0), vec![invalid])])
                        .unwrap();
                    assert_eq!(game.side.save.0.record_save_failures(), 1);
                    let (send, receive) = mpsc::channel();
                    let mut save = ClosingSave {
                        quit,
                        close: quit,
                        reply: (!quit).then_some(send),
                        started: Instant::now(),
                        stage: Stage::Writes(Some(game.side.save.0.flush_later().unwrap())),
                    };
                    assert_eq!(finish_server_staged_save(game, &mut save), quit);
                    if !quit {
                        let reply: serde_json::Value =
                            serde_json::from_str(&receive.try_recv().unwrap()).unwrap();
                        assert_eq!(reply, serde_json::json!({"ok": true}));
                    }
                }
            });
        });
    }
}
