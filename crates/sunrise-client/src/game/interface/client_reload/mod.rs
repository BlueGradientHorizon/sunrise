//! Prepare client listeners against the new pack selection before publishing
//! any assets, and apply them on the thread that owns the game. The initial
//! load runs the same three steps: the loader thread registers and stages,
//! and the game applies once it exists (`start_game`).
use mod_api::__private::game::*;

mod texture_atlases;
pub(in crate::game) use texture_atlases::{collect_texture_atlases, stage_texture_atlases};

#[cfg(test)]
#[path = "client_reload_tests.rs"]
mod tests;
use super::Game;
use super::packs::reload_files;
use mod_api::{
    __private::reload::{PreparedStep, ReloadStageError, prepare_listeners},
    ClientHost, ClientReloadContext, ClientReloadListener, HookId, Host,
};
use sunrise_client::datapack::DataPack;

/// One client reload step: a mod's apply action or model publication.
pub(super) type PreparedClientReload = PreparedStep<()>;

/// The registration host of the first load: no game exists yet, so it reads
/// nothing — a host read during registration panics, as its contract forbids
/// one. The reload itself reads through the game's host.
pub(super) struct StartupClientHost;
impl Host for StartupClientHost {}
mod_api::impl_host_areas!(StartupClientHost);
impl ClientHost for StartupClientHost {}
mod_api::impl_client_host_areas!(StartupClientHost, except(audio));
// A startup reload opens no audio stream.
impl mod_api::prelude::AudioClientHost for StartupClientHost {
    fn open_audio_stream(
        &self,
        _: &mod_api::StreamSpec,
        _: Box<dyn mod_api::AudioStream>,
    ) -> Result<mod_api::AudioStreamRef, mod_api::AudioRefusal> {
        Err(mod_api::AudioRefusal::Unavailable)
    }

    fn open_push_audio_stream(
        &self,
        _: &mod_api::StreamSpec,
    ) -> Result<mod_api::AudioStreamRef, mod_api::AudioRefusal> {
        Err(mod_api::AudioRefusal::Unavailable)
    }
}

impl Game<'_> {
    /// Registers a later reload's client listeners through the game
    /// (`native_add_client_reload_listeners`), gated on the hook being
    /// listened to.
    pub(super) fn register_client_reload_listeners(&self) -> mod_api::ClientListenerRegistry {
        let _reload = self
            .mod_phase
            .reloading()
            .expect("client listener registration on its owning thread");
        let mut listeners = <mod_api::ClientListenerRegistry as mod_api::__private::game::ClientReloadListenersOrder>::new();
        if mod_api::__private::listened(HookId::AddClientReloadListeners) {
            self.native_add_client_reload_listeners(&mut listeners);
        }
        listeners
    }
}

/// Registers the first load's client listeners: no game exists yet, so
/// [`StartupClientHost`] stands in, before any resources are loaded.
pub(super) fn register_startup_client_reload_listeners() -> mod_api::ClientListenerRegistry {
    let phase = mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::Setup);
    let _reload = phase.reloading().expect("initial client registration");
    let mut listeners = <mod_api::ClientListenerRegistry as mod_api::__private::game::ClientReloadListenersOrder>::new();
    if mod_api::__private::listened(HookId::AddClientReloadListeners) {
        mod_api::dispatch_add_client_reload_listeners(&StartupClientHost, |listen| {
            listeners.register_listener(listen);
        });
    }
    listeners
}

/// Prepare listeners against the merged asset view, running the model build
/// at the model manager's position in their order. The models travel separately
/// so renderer preflight can use them without publishing them.
pub(super) fn stage_client_reload<V, E: From<ReloadStageError>>(
    root: &sunrise_assets::resource_source::ResourceSource,
    listeners: mod_api::ClientListenerRegistry,
    entering: mod_api::__private::ModPhase,
    build_models: impl FnOnce() -> Result<V, E>,
) -> Result<(V, Vec<PreparedClientReload>), E> {
    let phase = mod_api::__private::PhaseOwner::scope(entering);
    let pack = DataPack::from_source("vanilla".into(), root.clone());
    let ordered = listeners.finish().map_err(ReloadStageError::Order)?;
    let mut build_models = Some(build_models);
    let mut models = None;
    let steps = prepare_listeners(
        ordered,
        HookId::AddClientReloadListeners,
        "client_reload_prepare",
        || -> Result<(), E> {
            models = Some(build_models.take().expect("one model manager")()?);
            Ok(())
        },
        |id, converter| ClientReloadContext::new(reload_files("assets", &[&pack], id, converter)),
        |listener, context| {
            let _reload = phase.reloading().expect("client listener prepare");
            ClientReloadListener::prepare(listener, context)
        },
    )?;
    Ok((models.expect("model manager is always registered"), steps))
}

#[cfg(test)]
thread_local! {
    static APPLY_GENERATIONS: std::cell::RefCell<Option<Vec<u64>>> = const { std::cell::RefCell::new(None) };
}

impl Game<'_> {
    /// Publish the models in order with the mod listener actions on the game-owning
    /// thread, before the resources-reloaded notice.
    pub(super) fn apply_client_reload(
        &mut self,
        prepared: Vec<PreparedClientReload>,
        publish_models: impl FnOnce(&mut Self),
    ) {
        let phase = self.mod_phase.clone();
        let mut publish_models = Some(publish_models);
        for step in prepared {
            #[cfg(test)]
            if matches!(step, PreparedStep::Mod { .. }) {
                APPLY_GENERATIONS.with_borrow_mut(|trace| {
                    if let Some(trace) = trace {
                        trace.push(self.pack.render.generation);
                    }
                });
            }
            step.apply(
                &phase,
                HookId::AddClientReloadListeners,
                "client_reload_apply",
                |()| {
                    publish_models.take().expect("one model manager")(self);
                    Ok::<_, std::convert::Infallible>(())
                },
            )
            .unwrap();
        }
    }
}
