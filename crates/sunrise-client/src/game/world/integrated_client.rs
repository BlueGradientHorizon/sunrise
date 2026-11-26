use super::{Game, WorldIcon, execution_role::ExecutionRole};
use crate::game::server_side::integrated::Finished;
use crate::game::server_side::integrated::Flags;
use crate::game::server_side::integrated::Integrated;
use crate::game::server_side::integrated::Launch;
use crate::game::server_side::integrated::Starting;
use crate::game::server_side::integrated::serve;
use crate::game::server_side::integrated::server_thread_stack;
use std::{
    sync::{Arc, atomic::Ordering, mpsc::channel},
    time::{Duration, Instant},
};
use sunrise_client::{Result, lan::OwnerProfile};
use sunrise_protocol::crypto::offline_uuid;
use sunrise_sim::world_options::WorldOptions;

impl Game<'_, crate::game::client_side::ClientState> {
    /// Only the owner window changes its integrated server's command tree.
    pub(in crate::game::client_side) fn sync_pregen_option(&self) {
        let Some(server) = &self.integrated else {
            return;
        };
        let enabled = self.options.pregen || self.side.settings.extras.pregen;
        // Check on the server after earlier updates so a later toggle wins without blocking.
        server.queue_pregen_option(enabled);
    }
    pub(in crate::game) fn client_has_singleplayer_server(&self) -> bool {
        self.integrated.is_some()
    }
    pub(in crate::game) fn client_is_published(&self) -> bool {
        self.integrated
            .as_ref()
            .is_some_and(|s| s.published().is_some())
    }
    #[inline(always)]
    #[allow(dead_code)]
    pub(in crate::game) fn integrated_tick_loop_client(&mut self, started: Instant) -> f32 {
        self.side.debug_charts.log_tick(started, started.elapsed());
        self.side.debug_charts.smoothed_tick_ms
    }
    /// `Minecraft.doWorldLoad`: start the server thread on the world; the
    /// window joins it through memory once it is ready (`poll_integrated_start`).
    pub(in crate::game) fn open_integrated(
        &mut self,
        folder: &str,
        name: Option<&str>,
        seed: Option<i64>,
        creative: bool,
        create: Option<&WorldOptions>,
    ) -> Result<()> {
        self.client_logging_out();
        sunrise_client::command::vanilla::set_sunrise_commands(
            self.options.pregen || self.side.settings.extras.pregen,
        );
        let auth = sunrise_client::auth::current();
        let player = auth
            .as_ref()
            .map_or_else(|| self.options.player_name(), |a| a.name.clone());
        let new_world = create.is_some();
        let rule_registry = create.map_or_else(
            || Arc::new(crate::game::api_values::game_rule_registry()),
            |options| options.rules.registry().clone(),
        );
        // `IntegratedServer`'s constructor: `setSingleplayerProfile(minecraft.getGameProfile())`
        // — the signed-in account's own UUID. Only an offline start names its player by the
        // offline profile. The owner is the world's player, so a world without
        // `singleplayer_uuid` (a Realms download, `renameAndDropPlayer`) finds the owner's
        // `players/data/<uuid>.dat` instead of starting a new player.
        let owner = auth
            .as_ref()
            .map_or_else(|| offline_uuid(&player), |a| a.uuid);
        let launch = Launch {
            online_auth: auth.is_some(),
            rule_registry: rule_registry.clone(),
            folder: folder.to_owned(),
            name: name.map(str::to_owned),
            seed,
            creative,
            create: create.cloned(),
            owner,
            player: player.clone(),
            owner_profile: auth.as_ref().map(|a| {
                let id = a.uuid;
                Arc::new(OwnerProfile::new(move || {
                    sunrise_client::skins::fetch_own_properties(id)
                        .into_iter()
                        .map(
                            |(name, value, signature)| sunrise_core::profile::AccountProperty {
                                name,
                                value,
                                signature,
                            },
                        )
                        .collect()
                }))
            }),
        };
        let mut options = self.options.clone();
        // A server without a console: no local player, no window, no control socket.
        // Compatibility for gameplay checks outside startup/world loading.
        options.dedicated = Some(Vec::new());
        options.execution_role = ExecutionRole::IntegratedServer;
        options.control = None;
        options.benchmark = None;
        options.frames = None;
        options.world_frames = None;
        options.report = None;
        options.server = None;
        options.world = None;
        self.join_world_rest()?;
        let (res, pack, settings) = (
            self.res.clone(),
            self.pack.clone(),
            self.side.settings.sim_settings(),
        );
        sunrise_client::parallel::set_setting(settings.parallel_tick);
        let flags = Arc::new(Flags {
            reports: self.side.client_reports.0.clone(),
            ..Flags::default()
        });
        flags
            .render_distance
            .store(settings.render_distance, Ordering::Relaxed);
        flags
            .simulation_distance
            .store(settings.simulation_distance, Ordering::Relaxed);
        flags
            .entity_distance
            .store(settings.entity_distance.to_bits(), Ordering::Relaxed);
        let (ready_tx, ready) = channel();
        let (tasks, server_tasks) = channel();
        let server_flags = flags.clone();
        let run_server = move |_: &()| {
            let _finished = Finished(server_flags.clone());
            // MinecraftServer.spin's Thread.setPriority(8).
            sunrise_threads::favour_current();
            let result = serve(
                &res,
                pack,
                options,
                settings,
                launch,
                server_flags.clone(),
                server_tasks,
                &ready_tx,
            );
            if let Err(e) = result {
                log::error!("Sunrise: integrated server: {e}");
                *server_flags.failure.lock().unwrap() = Some(e.to_string());
                let _ = ready_tx.send(Err(e.to_string()));
            }
        };
        // Production installations are process-wide. A test installation is scoped
        // to its owning thread; the real server thread must inherit that snapshot.
        #[cfg(test)]
        let run_server = mod_api::testing::inherit_mods(run_server);
        let thread = std::thread::Builder::new()
            .name(sunrise_client::crash::INTEGRATED_SERVER_THREAD.into())
            .stack_size(server_thread_stack())
            .spawn(move || run_server(&()))?;
        let emergency_flags = flags.clone();
        let server_id = thread.thread().id();
        let emergency_save = sunrise_client::crash::on_emergency_save(move || {
            emergency_flags.emergency_save(server_id, Duration::from_secs(30));
        });
        let handle = Integrated {
            emergency_save: Some(emergency_save),
            flags,
            thread: Some(thread),
            tasks,
            world_name: String::new(),
            save_folder: folder.to_owned(),
            rule_registry,
        };
        self.integrated_starting = Some(Starting {
            handle,
            ready,
            player,
            folder: folder.to_owned(),
            new_world,
        });
        // Harnesses, control scripts and runs without a window go on with the world open.
        if self.side.window.0.is_none() || super::synchronous_startup(&self.options) {
            while self.integrated_starting.is_some() {
                self.poll_integrated_start()?;
                if self.integrated_starting.is_some() {
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        }
        Ok(())
    }
    /// The starting server's answer, polled each frame: joined once it is ready.
    pub(in crate::game::client_side) fn poll_integrated_start(&mut self) -> Result<()> {
        if self.integrated_starting.is_none() {
            return Ok(());
        }
        self.tick_load_progress();
        let Some(starting) = &self.integrated_starting else {
            return Ok(());
        };
        let (pipe, world_name) = match starting.ready.try_recv() {
            Ok(Ok(ready)) => ready,
            Err(std::sync::mpsc::TryRecvError::Empty) => return Ok(()),
            // FileFixerProgressScreen's Cancel: the upgrade stopped, back to the title.
            Ok(Err(_)) if starting.handle.upgrade().is_canceled() => {
                self.integrated_starting = None;
                self.side.perf.cancel_world_opening();
                return Ok(());
            }
            Ok(Err(e)) => {
                self.integrated_starting = None;
                return Err(e.into());
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.integrated_starting = None;
                return Err("the integrated server stopped".into());
            }
        };
        let Some(Starting {
            mut handle,
            player,
            folder,
            new_world,
            ..
        }) = self.integrated_starting.take()
        else {
            return Ok(());
        };
        handle.world_name = world_name;
        self.server_packs_world_pack(&folder);
        self.connect_local(pipe, &player)?;
        if let Some(r) = self.remote.as_mut() {
            // `new LevelLoadTracker(newWorld ? 500L : 0L)`.
            r.level_load = sunrise_client::level_loading::LevelLoadTracker::new(if new_world {
                sunrise_client::level_loading::LEVEL_LOAD_CLOSE_DELAY_MS
            } else {
                0
            });
        }
        // IntegratedServer.getWorldScreenshotFile: the client grabs the frame
        // the Select World list shows, and needs the save folder for it.
        self.side.world_icon = Some(WorldIcon {
            path: std::env::current_dir()?
                .join("saves")
                .join(&folder)
                .join("icon.png"),
            last_attempt: None,
        });
        self.integrated = Some(handle);
        Ok(())
    }
    /// `LevelLoadingScreen.tick`'s easing of the server's progress, every 50 ms
    /// of wall time (the starting server has no client tick yet).
    pub(in crate::game::client_side) fn tick_load_progress(&mut self) {
        let view = match (&self.integrated_starting, &self.integrated) {
            (Some(s), _) => s.handle.load_view().clone(),
            (None, Some(i)) => i.load_view().clone(),
            _ => return,
        };
        let now = Instant::now();
        let at = self.side.loading_progress_at.get_or_insert(now);
        let steps = (now.duration_since(*at).as_millis() / 50).min(20) as u32;
        *at += Duration::from_millis(u64::from(steps) * 50);
        if now.duration_since(*at) > Duration::from_secs(1) {
            *at = now;
        }
        for _ in 0..steps {
            self.side.loading_progress = sunrise_client::level_loading::smooth(
                self.side.loading_progress,
                view.server_progress(),
            );
        }
    }
}
