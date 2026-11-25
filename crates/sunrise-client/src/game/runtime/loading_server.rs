//! Dedicated server startup (`start_dedicated_game`) and server-side agent control socket setup.
use super::*;
use sunrise_client::control;

/// The post-load setup for a dedicated server run: builds `ServerGame` with no window,
/// renderer, client reload, or audio device, runs `run_dedicated`, and shuts down.
#[allow(clippy::too_many_arguments)]
pub(in crate::game) fn start_dedicated_game(
    res: &Arc<Resources>,
    pack: PackResources,
    gui: UiResources,
    options: Options,
    mut settings: Settings,
    started: Instant,
    prepared: Option<GameCpuResources>,
) -> Result<()> {
    if let Some(d) = options.distance_override {
        settings.render_distance = d;
    }
    let prepared = match prepared {
        Some(prepared) => Some(prepared),
        None => Some(GameCpuResources::load(
            &sunrise_assets::resource_source::ResourceRoot::new(
                options.resources.clone(),
                packs::cache_dir(),
                pack.source.clone(),
            ),
            None,
            pack.render.model_library.as_deref().ok(),
            false,
        )?),
    };
    let mut game = crate::game::ServerGame::new(res, pack, gui, options, settings, prepared)?;
    if game.options.benchmark.is_some() {
        game.settings.vsync = false;
        game.settings.max_fps = 260;
        game.settings.inactivity_fps_limit = "minimized".into();
        game.options.no_pause = true;
    }
    if game.options.benchmark.is_none() {
        game.start_mods();
    }
    game.refresh_loop_context()?;
    game.start_server_agent_control()?;
    if let Some(scenario) = game.options.benchmark.clone() {
        let control = game
            .control
            .get_or_insert_with(sunrise_client::control::Server::local);
        sunrise_client::benchmark::start(
            control.sender(),
            sunrise_client::benchmark::Setup {
                scenario,
                started,
                app: sunrise_bench::App {
                    name: "sunrise-client".into(),
                    version: sunrise_client::crash::version(),
                    commit: option_env!("SUNRISE_COMMIT").map(str::to_owned),
                    mods: Vec::new(),
                },
                simulation_distance: game.settings.simulation_distance as u32,
                vsync: false,
                fps_cap: None,
                offscreen: game.options.offscreen,
                game_dir: std::env::current_dir()?,
                set: game.options.benchmark_set.clone(),
                profile: game.options.benchmark_profile,
            },
        );
    }
    let result = game.run_dedicated();
    game.native_game_shutting_down();
    result
}

impl Game<'_, crate::game::server_side::ServerState> {
    /// Server-side startup control socket setup: preserves `--control`, benchmark, No AI, and
    /// `settings.extras.agent_control` without touching client state.
    pub(in crate::game::server_side) fn start_server_agent_control(&mut self) -> Result<()> {
        if self.options.benchmark.is_some() && self.options.control.is_none() {
            return Ok(());
        }
        if self.settings.extras.no_ai {
            if self.options.control.is_some() {
                log::warn!(
                    "Sunrise: control socket not opened: {}",
                    sunrise_core::no_ai::REFUSAL
                );
            }
            return Ok(());
        }
        if let Some(path) = &self.options.control {
            let server = control::Server::start(path)
                .map_err(|e| format!("control socket {}: {e}", path.display()))?;
            log::info!("Sunrise: control socket {}", server.path().display());
            self.control = Some(server);
        } else if self.settings.extras.agent_control {
            let path = control::default_path();
            match control::Server::start(&path) {
                Ok(server) => {
                    log::info!("Sunrise: control socket {}", server.path().display());
                    self.control = Some(server);
                }
                Err(e) => log::warn!("Sunrise: control socket {}: {e}", path.display()),
            }
        }
        Ok(())
    }
}
