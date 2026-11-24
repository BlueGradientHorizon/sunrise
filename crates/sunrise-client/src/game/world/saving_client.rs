#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{Game, loading_frame};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::saving::CLOSING_SNAPSHOT_BUDGET;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::saving::CLOSING_SNAPSHOTS_PER_FRAME;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::saving::ClientQuit;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::saving::ClosingSave;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::saving::SNAPSHOT_BUDGET;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::saving::SNAPSHOTS_PER_FRAME;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::saving::Stage;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::saving::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::{
    collections::{HashSet, VecDeque},
    sync::mpsc::{Receiver, TryRecvError},
    time::{Duration, Instant},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{Result, hud::HudBatch};

impl Game<'_> {
    pub(in crate::game) fn begin_closing_save(&mut self, quit: bool) -> Result<()> {
        if quit {
            self.client.client_quit.set(ClientQuit::Stopping);
            self.close_client_tasks();
        }
        if let Some(save) = &mut self.pending_save.0 {
            save.quit |= quit;
            return Ok(());
        }
        // A world still loading has no client in it yet: its server stops and is joined
        // through the same fallible path as a running one, before the loop may end.
        if quit && let Some(starting) = self.integrated_starting.take() {
            starting.handle.request_stop();
            self.integrated = Some(starting.handle);
            self.pending_save.0 = Some(ClosingSave {
                quit,
                close: true,
                reply: None,
                started: Instant::now(),
                stage: Stage::Server,
            });
            return Ok(());
        }
        if self.title.0.is_some() || self.client.saved_for_exit {
            // A quit from the title still needs the event loop to finish.
            self.pending_save.0 = Some(ClosingSave {
                quit,
                close: true,
                reply: None,
                started: Instant::now(),
                stage: Stage::Quit,
            });
            return Ok(());
        }
        if self.integrated.is_some() {
            // Minecraft.disconnect: the client logs out before its server halts.
            self.client_logging_out();
        }
        // Stop client-side terrain work before the integrated server is shut down. The
        // server's own stop path aborts its workers, but the window's streaming registration
        // is dropped only after the event loop exits and would otherwise wait for queued work.
        self.streaming.abort_generation();
        if let Some(integrated) = &self.integrated {
            integrated.request_stop();
            self.pending_save.0 = Some(ClosingSave {
                quit,
                close: true,
                reply: None,
                started: Instant::now(),
                stage: Stage::Server,
            });
        } else if self.remote.is_some() {
            if !quit {
                return self.leave_server(None);
            }
            self.connection_left(sunrise_hooks::connection::Left::Quit);
            self.client_logging_out();
            if let Some(mut remote) = self.remote.take() {
                remote.session.disconnect();
            }
            self.client.saved_for_exit = true;
            self.pending_save.0 = Some(ClosingSave {
                quit,
                close: true,
                reply: None,
                started: Instant::now(),
                stage: Stage::Quit,
            });
        } else {
            self.close_container();
            self.award_custom_stat("leave_game", 1);
            self.lan_close(false);
            self.free_player_beds()?;
            self.pending_save.0 = Some(ClosingSave {
                quit,
                close: true,
                reply: None,
                started: Instant::now(),
                stage: Stage::Generation,
            });
        }
        self.capture(false);
        Ok(())
    }
    /// Control saves acknowledge only after durable completion, while their
    /// snapshot work is spread across event-loop turns too.
    pub(in crate::game) fn begin_control_save(
        &mut self,
        all: bool,
        reply: std::sync::mpsc::Sender<String>,
    ) -> Result<()> {
        if self.pending_save.0.is_some() {
            return Err("a world save is already in progress".into());
        }
        if all {
            self.world
                .modified
                .extend(self.world.chunks.keys().copied());
        }
        self.clear_device_keys();
        self.client.physical_keys.clear();
        self.set_mapping_held("key.attack", false);
        self.set_mapping_held("key.use", false);
        self.pending_save.0 = Some(ClosingSave {
            quit: false,
            close: false,
            reply: Some(reply),
            started: Instant::now(),
            stage: Stage::Generation,
        });
        Ok(())
    }
    /// `true` means the window can exit. The session lock stays in Game until
    /// every snapshot has been queued and every writer has synced its files.
    pub(in crate::game) fn poll_closing_save(&mut self) -> Result<bool> {
        let defers = self.pending_save.0.as_ref().is_some_and(|save| {
            !matches!(save.stage, Stage::Server | Stage::Quit)
                && save.started.elapsed() < Duration::from_secs(30)
        });
        if defers && self.defer_chunk_disk_save() {
            return Ok(false);
        }
        let Some(mut save) = self.pending_save.0.take() else {
            return Ok(false);
        };
        let (per_frame, budget) = if save.close {
            (CLOSING_SNAPSHOTS_PER_FRAME, CLOSING_SNAPSHOT_BUDGET)
        } else {
            (SNAPSHOTS_PER_FRAME, SNAPSHOT_BUDGET)
        };
        let result = (|| -> Result<bool> {
            match &mut save.stage {
                Stage::Quit => Ok(true),
                Stage::Server => {
                    let server = self
                        .integrated
                        .as_ref()
                        .ok_or("the integrated server stopped with an error")?;
                    if !server.stopped() {
                        return Ok(false);
                    }
                    // Drop/join only after completion; leave_server's teardown
                    // can then run without blocking on the server's writes.
                    if save.quit {
                        self.integrated.take().unwrap().stop()?;
                    } else {
                        self.leave_server(None)?;
                    }
                    Ok(true)
                }
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
                    // A later shutdown barrier must force the files again if this one failed.
                    *flush = None;
                    result?;
                    if !save.close {
                        self.client.message = Some((
                            sunrise_client::lang::get("commands.save.success"),
                            Instant::now(),
                        ));
                    }
                    Ok(true)
                }
            }
        })();
        match result {
            Ok(true) => {
                if let Some(reply) = save.reply.take() {
                    let _ = reply.send(serde_json::json!({ "ok": true }).to_string());
                }
                if save.quit && !save.close {
                    // A window close during a control save still has to close
                    // containers and save the resulting inventory changes.
                    self.begin_closing_save(true)?;
                    return Ok(false);
                }
                if save.quit {
                    self.client.saved_for_exit = true;
                    Ok(true)
                } else {
                    if save.close && self.title.0.is_none() {
                        self.finish_perf();
                        self.finish_close_world()?;
                    }
                    self.clock.reset();
                    self.previous = Instant::now();
                    Ok(false)
                }
            }
            other => {
                if let Err(error) = &other
                    && let Some(reply) = save.reply.take()
                {
                    let _ =
                        reply.send(serde_json::json!({ "error": error.to_string() }).to_string());
                }
                // Keep ownership (and the lock) on errors too. Native shutdown must
                // finish a durable barrier before it can declare the save completed.
                self.pending_save.0 = Some(save);
                other
            }
        }
    }
    pub(in crate::game) fn render_closing_save(&mut self) -> Result<()> {
        if matches!(
            self.pending_save.0.as_ref().map(|save| &save.stage),
            Some(Stage::Quit)
        ) {
            return Ok(());
        }
        let waiting = matches!(
            self.pending_save.0.as_ref().map(|save| &save.stage),
            Some(Stage::Server | Stage::Writes(_))
        );
        if waiting {
            thread_local! {
                static LAST_FRAME: std::cell::Cell<Option<Instant>> =
                    const { std::cell::Cell::new(None) };
            }
            const FRAME: Duration = Duration::from_millis(33);
            if let Some(last) = LAST_FRAME.with(|c| c.get()) {
                let since = last.elapsed();
                if since < FRAME {
                    std::thread::sleep(FRAME - since);
                }
            }
            LAST_FRAME.with(|c| c.set(Some(Instant::now())));
        }
        // disconnectWithSavingScreen: a GenericMessageScreen, its title and
        // no navigation.
        sunrise_client::widgets::begin_screen(|| {
            sunrise_client::widgets::ScreenInfo::new(
                "saving",
                super::screen_info::tr("menu.savingLevel", &[]),
            )
            .without_navigation()
        });
        let Some(renderer) = &mut self.renderer.0 else {
            return Ok(());
        };
        let mut hud = HudBatch::new(
            [renderer.config.width, renderer.config.height],
            self.settings.gui_scale_inputs(),
        );
        let progress = match self.pending_save.0.as_ref().map(|save| &save.stage) {
            Some(Stage::Chunks(_)) => 0.25,
            Some(Stage::Entities(_)) => 0.5,
            Some(Stage::Writes(_)) => 0.9,
            _ => 0.0,
        };
        sunrise_client::level_loading::draw_progress(
            &mut hud,
            &self.gui.atlas,
            "Saving world...",
            progress,
        );
        renderer.render(&loading_frame(&hud))?;
        Ok(())
    }
    /// `Minecraft.stop`: record the request without reentering the client from a hook.
    pub(in crate::game) fn request_client_quit(&self) -> bool {
        if !self.native_window()
            || self.client.client_quit.get() != ClientQuit::Running
            || sunrise_threads::stop_requests() > 0
        {
            return false;
        }
        self.client.client_quit.set(ClientQuit::Requested);
        true
    }
    /// `GameRenderer.tryTakeScreenshotIfNeeded`: while a local world without a
    /// saved icon is on the screen, grab the rendered frame once a second until
    /// the save has one. The Select World list shows it as the world's preview.
    pub(in crate::game) fn tick_world_icon(&mut self) {
        // Its grab would take a sequence's frame.
        if self.screenshot_sequence.0.is_some() {
            return;
        }
        let Some(icon) = self.client.world_icon.as_ref() else {
            return;
        };
        if icon
            .last_attempt
            .is_some_and(|at| at.elapsed() < Duration::from_secs(1))
        {
            return;
        }
        if icon.path.is_file() {
            // tryTakeScreenshotIfNeeded's hasWorldScreenshot: it has one, stop asking.
            self.client.world_icon = None;
            return;
        }
        // takeAutoScreenshot only saves with every loaded section rendered, or
        // the icon would picture the holes the streamer has not filled yet.
        if self.streaming.pending() != 0
            || !self.pending_uploads.0.is_empty()
            || self.remote_backlog() != 0
        {
            return;
        }
        let Some(renderer) = self.renderer.0.as_mut() else {
            return;
        };
        // countRenderedSections() > 10: a client that joined its server
        // thread has nothing pending before the first chunks arrive, and the
        // icon would be only sky.
        if renderer.drawn_sections <= 10 {
            return;
        }
        if renderer.screenshot.is_some() {
            // F2's or an agent's screenshot owns the frame this time; ours
            // comes around again with the next attempt.
            return;
        }
        let path = icon.path.clone();
        if let Some(icon) = self.client.world_icon.as_mut() {
            icon.last_attempt = Some(Instant::now());
        }
        renderer.screenshot = Some(sunrise_client::gfx::ScreenshotRequest {
            path,
            size: Some([64, 64]),
            scaled: false,
            background: true,
        });
    }
}

#[cfg(test)]
pub(in crate::game) mod tests {
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use super::*;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::native_entity_refs::tests::with_game;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::saving::tests::*;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use std::sync::{Arc, mpsc};
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use sunrise_client::save::{LevelShape, SaveDir};
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use sunrise_core::nbt::{DISK_WRITE_NESTING_GUARD, Tag};

    pub(in crate::game) fn finish_staged_save(game: &mut Game<'_>) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let quit = game.poll_closing_save().unwrap();
            if game.pending_save.0.is_none() {
                return quit;
            }
            assert!(
                Instant::now() < deadline,
                "the region writer never acknowledged the save"
            );
            std::thread::yield_now();
        }
    }
}
