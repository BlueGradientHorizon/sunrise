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
    __private::reload::{PreparedStep, ReloadStageError, apply_listeners, prepare_listeners},
    ClientReloadContext, ClientReloadHost, ClientReloadListener, HookId, ReloadContext,
};
use sunrise_client::datapack::DataPack;

/// One client reload step to run later on the game-owning thread: a mod
/// listener's prepared apply.
pub(super) type PreparedClientReload = PreparedStep<()>;

/// The registration context of the first load: no game exists yet, so
/// [`ClientReloadHost`] defaults answer empty/unavailable while listeners
/// register. Later reloads pass `&Game` directly.
pub(super) struct StartupReloadContext;
impl mod_api::prelude::BlockStatesHost for StartupReloadContext {}
impl mod_api::prelude::KeysHost for StartupReloadContext {}
impl mod_api::prelude::TagsHost for StartupReloadContext {}
impl mod_api::prelude::TextHost for StartupReloadContext {
    fn text_string(&self, text: &mod_api::Text) -> String {
        sunrise_client::lang::scoped(sunrise_client::lang::Language::default(), || {
            sunrise_client_lib::api_text::to_engine_inprocess(text).plain()
        })
        .0
    }
}
impl mod_api::prelude::RecipesHost for StartupReloadContext {}
impl mod_api::prelude::RecipePropertiesHost for StartupReloadContext {}
impl mod_api::prelude::ResourcesHost for StartupReloadContext {}
impl mod_api::prelude::MathHost for StartupReloadContext {}
impl mod_api::prelude::PayloadTokensHost for StartupReloadContext {}
impl mod_api::prelude::PayloadDropsHost for StartupReloadContext {}
impl mod_api::prelude::ItemParticlesClientHost for StartupReloadContext {
    fn item_particle_options(&self, _: &mod_api::ItemStack) -> Option<mod_api::ParticleOptions> {
        None
    }
}
impl ReloadContext for StartupReloadContext {}
impl ClientReloadHost for StartupReloadContext {}

impl Game<'_> {
    /// Registers a later reload's client listeners through the game
    /// (`native_add_client_reload_listeners`), gated on the hook being
    /// listened to.
    pub(super) fn register_client_reload_listeners(&self) -> mod_api::ClientListenerRegistry {
        let mut listeners = <mod_api::ClientListenerRegistry as mod_api::__private::game::ClientReloadListenersOrder>::new();
        if mod_api::__private::listened(HookId::AddClientReloadListeners) {
            self.native_add_client_reload_listeners(&mut listeners);
        }
        listeners
    }
}

/// Registers the first load's client listeners: no game exists yet, so
/// [`StartupReloadContext`] stands in, before any resources are loaded.
pub(super) fn register_startup_client_reload_listeners() -> mod_api::ClientListenerRegistry {
    let mut listeners = <mod_api::ClientListenerRegistry as mod_api::__private::game::ClientReloadListenersOrder>::new();
    if mod_api::__private::listened(HookId::AddClientReloadListeners) {
        mod_api::dispatch_add_client_reload_listeners(&StartupReloadContext, &mut listeners);
    }
    listeners
}

/// Orders the registered listeners, then prepares each against `root`, the
/// merged resource root the reload will publish; runs on the reload's worker,
/// off the game-owning thread.
pub(super) fn stage_client_reload(
    root: &sunrise_assets::resource_source::ResourceSource,
    listeners: mod_api::ClientListenerRegistry,
) -> Result<Vec<PreparedClientReload>, ReloadStageError> {
    let pack = DataPack::from_source("vanilla".into(), root.clone());
    let ordered = listeners.finish().map_err(ReloadStageError::Order)?;
    prepare_listeners(
        ordered,
        "client_reload_prepare",
        |_| None,
        |id, converter| ClientReloadContext::new(reload_files("assets", &[&pack], id, converter)),
        ClientReloadListener::prepare,
    )
}

/// Applies the prepared listeners in order on the game-owning thread, after
/// the rest of the staged pack is published and before the
/// resources-reloaded notice.
pub(super) fn apply_client_reload(prepared: Vec<PreparedClientReload>) {
    apply_listeners(prepared, "client_reload_apply", |()| {});
}
