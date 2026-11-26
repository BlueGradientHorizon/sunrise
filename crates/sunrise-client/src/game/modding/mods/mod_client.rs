use crate::game::mods::ModsReload;
use crate::game::mods::ModsState;
use crate::game::{Game, owner_link::OwnerRequest};
use crate::game::{execution_role::ExecutionRole, mods::standalone::collect_standalone_models};
#[allow(unused_imports)]
use mod_api::__private::game::*;
use mod_api::RegistrationContext;
use sunrise_game_data::lang;

#[path = "package_files_client.rs"]
pub(in crate::game) mod package_files;

#[cfg(feature = "client")]
impl Game<'_, crate::game::client_side::ClientState> {
    /// The content mods the resources load with: the ones applied, which a
    /// switch that needs a restart doesn't change.
    pub(in crate::game) fn applied_mods(&self) -> std::collections::BTreeSet<String> {
        self.mods
            .applied
            .clone()
            .unwrap_or_else(|| self.side.settings.mods.enabled.clone())
    }

    pub(in crate::game) fn client_set_audio_loop_context(
        &mut self,
        audio: Option<sunrise_world::native_load_context::LoopContext>,
    ) {
        if let Some(audio) = audio {
            self.side.audio.0.set_loop_context(audio);
        }
    }

    pub(in crate::game) fn client_retire_disabled_instance_groups(
        &self,
        effective: &std::collections::BTreeSet<String>,
    ) {
        self.side
            .instance_groups
            .borrow_mut()
            .retire_disabled(effective);
    }

    /// A live switch's resources applied: the server reloads its data packs
    /// from the new root, as `/reload` does (recipes, tags, loot tables,
    /// advancements, functions), and sends them to its players.
    pub(in crate::game) fn mods_reloaded(&mut self, applied: bool) {
        let Some(started) = self.mods.live.take() else {
            return;
        };
        let resources_ms = started.elapsed().as_millis();
        if !applied {
            return;
        }
        let reload = ModsReload {
            root: self.options.resources.clone(),
            data_source: self.pack.data_source.clone(),
            data_changes: self.pack.data_changes.clone(),
            additional_data: self.pack.additional_data.clone(),
            enabled: self.applied_mods(),
            selected: self.mods.selected.clone(),
            refused: self.mods.refused.clone(),
        };
        log::info!("Sunrise: content mods switched live: resources reloaded in {resources_ms} ms");
        if let Some(server) = &self.side.integrated {
            server.tell(OwnerRequest::ReloadMods(Box::new(reload)));
        }
    }
    /// The player turned content mods on or off (Options > Mods, or an
    /// install from Browse Mods): those with only assets and data apply now,
    /// through the resource reload and then the
    /// server's data reload (`mods_reloaded`); those that add blocks, items or
    /// worldgen wait for a restart, and the Mods page says so.
    pub(in crate::game::client_side) fn switch_mods(&mut self) {
        let Some(applied) = self.mods.applied.clone() else {
            return;
        };
        let wanted = self.side.settings.mods.enabled.clone();
        // Ordinary native mods' `mod.<id>` lines switch live (`sync_native_switches`);
        // a serving host bound to active or withheld `served_by` packages, and any content
        // mod with blocks, items, worldgen, or `served_by`, keep their startup state until
        // a restart.
        let content = &self.mods.content_ids;
        let mut next = wanted.clone();

        let mut waiting = Vec::new();
        for id in applied.symmetric_difference(&wanted) {
            if !content.contains(id) || self.mods.live_ids.contains(id) {
                continue;
            }
            log::info!("Sunrise: mod {id} needs a restart to switch");
            waiting.push(id.clone());
            if applied.contains(id) {
                next.insert(id.clone());
            } else {
                next.remove(id);
            }
        }
        let (_, host_waiting) = self
            .mods
            .reconcile_code_switches(&self.side.settings.mods.disabled);
        for id in host_waiting {
            log::info!("Sunrise: serving host mod {id} needs a restart to switch");
            if !waiting.contains(&id) {
                waiting.push(id);
            }
        }
        self.side.chat_ui.options.mods_note = (!waiting.is_empty())
            .then(|| lang::tr("sunrise.mods.switch.restart", &[&waiting.join(", ")]));
        let changed = applied
            .symmetric_difference(&next)
            .any(|id| content.contains(id));
        if !changed {
            self.mods.applied = Some(next);
            return;
        }
        log::info!(
            "Sunrise: content mods switched live: {:?} -> {:?}",
            applied,
            next
        );
        self.mods.applied = Some(next);
        self.mods.live = Some(std::time::Instant::now());
        self.reload_packs();
    }

    #[cfg(test)]
    pub(in crate::game) fn test_add_hook_panic_toasts(&mut self) {
        self.add_hook_panic_toasts();
    }

    #[cfg(test)]
    pub(in crate::game) fn test_open_title(&mut self, saves: std::path::PathBuf) {
        self.side.title.0 = Some(self.new_title(saves));
    }
}

/// The native mods' standalone models (`native_standalone_models`), set once
/// before the resources first load.
pub(in crate::game) fn register_standalone_models(role: ExecutionRole) {
    if let Some(registrations) = collect_standalone_models(role) {
        sunrise_client::models::register_standalone(registrations);
    }
    register_fluid_models(role);
}

/// Recollect before each resource bake, retaining registration-phase attribution.
pub(in crate::game) fn register_fluid_models(role: ExecutionRole) {
    if let Some(models) = collect_fluid_models(role) {
        sunrise_client::fluid_models::register(models);
    }
}

pub(in crate::game) fn collect_fluid_models(
    role: ExecutionRole,
) -> Option<Vec<sunrise_mod_package::content::FluidModel>> {
    if !role.is_client() {
        return None;
    }
    let mut models = Vec::new();
    crate::game::native_hooks::native_register_fluid_models(
        &RegistrationContext::new(),
        &mut models,
    );
    Some(
        models
            .into_iter()
            .map(|model| sunrise_mod_package::content::FluidModel {
                fluid: model.fluid,
                still: model.still,
                flowing: model.flowing,
                overlay: model.overlay,
            })
            .collect(),
    )
}

/// Freeze the native mods' entity model layer definitions at startup, then bake their skull
/// types (`native_create_skull_models`) from those definitions or vanilla's under `vanilla_root`.
pub(in crate::game) fn register_layer_definitions(
    role: ExecutionRole,
    vanilla_root: &std::path::Path,
) -> sunrise_game_data::Result<()> {
    if !role.is_client() {
        return Ok(());
    }
    let mut suppliers = Vec::new();
    crate::game::native_hooks::native_register_layer_definitions(
        &RegistrationContext::new(),
        &mut suppliers,
    );
    sunrise_client::mobs::model_layers::register(suppliers, vanilla_root)?;
    let mut models = Vec::new();
    crate::game::native_hooks::native_create_skull_models(&RegistrationContext::new(), &mut models);
    sunrise_client::skull_models::register(models, vanilla_root);
    Ok(())
}

/// Freeze the native mods' F3 text callbacks and renderer toggles at client startup.
pub(in crate::game) fn register_debug_entries(role: ExecutionRole) {
    if !role.is_client() {
        return;
    }
    let mut entries = Vec::new();
    crate::game::native_hooks::native_register_debug_entries(
        &RegistrationContext::new(),
        &mut entries,
    );
    sunrise_client::debug_entries::register(entries);
}

/// Install the native mods' HUD draw callbacks among the vanilla anchors once at client startup.
pub(in crate::game) fn register_gui_layers(role: ExecutionRole) {
    if !role.is_client() {
        return;
    }
    let mut layers = sunrise_client::gui_layers::GuiLayers::default();
    crate::game::client_side::native_hooks_client::register_gui_layers::native_register_gui_layers(
        &RegistrationContext::new(),
        &mut layers,
    );
    sunrise_client::gui_layers::freeze(layers);
}

/// Install the native mods' client tooltip component factories once at client startup.
pub(in crate::game) fn register_client_tooltip_component_factories(role: ExecutionRole) {
    if !role.is_client() {
        return;
    }
    let mut factories = sunrise_client::tooltip_components::TooltipComponentFactories::default();
    crate::game::client_side::native_hooks_client::register_client_tooltip_component_factories::native_register_client_tooltip_component_factories(
        &RegistrationContext::new(),
        &mut factories,
    );
    sunrise_client::tooltip_components::freeze(factories);
}

/// Collect the native mods' owned debug renderers and their owners on each list refresh.
pub(in crate::game) fn debug_renderers(
    role: ExecutionRole,
    entries: &dyn mod_api::__private::DebugEntryStates,
) -> Vec<mod_api::Answered<mod_api::DebugRenderer>> {
    let mut renderers = Vec::new();
    if role.is_client() {
        crate::game::native_hooks::native_register_debug_renderers(
            &RegistrationContext::new(),
            entries,
            &mut renderers,
        );
    }
    renderers
}

/// The native mods' numeric item model properties
/// (`native_register_range_select_item_model_property`), kept once at start, before
/// resources load, so `range_dispatch` models on them read the mod's number.
pub(in crate::game) fn register_range_select_item_model_properties(role: ExecutionRole) {
    if !role.is_client() {
        return;
    }
    let mut properties = Vec::new();
    crate::game::native_hooks::native_register_range_select_item_model_property(
        &RegistrationContext::new(),
        &mut properties,
    );
    sunrise_client::item_render::install_range_property_factories(properties);
}

/// The native mods' categorical item model properties
/// (`native_register_select_item_model_property`), kept once at start, before resources
/// load, so `select` models on them read the mod's value.
pub(in crate::game) fn register_select_item_model_properties(role: ExecutionRole) {
    if !role.is_client() {
        return;
    }
    let mut properties = Vec::new();
    crate::game::native_hooks::native_register_select_item_model_property(
        &RegistrationContext::new(),
        &mut properties,
    );
    sunrise_client::item_render::install_select_property_factories(properties);
}

/// The types native mods register for resource definitions, kept once at start, before
/// resources load: item tint sources (`native_register_item_tint_sources`), so `tints` entries on
/// them read the mod's colour; atlas sprite sources (`native_register_sprite_sources`), so atlas
/// definitions' entries on them list the mod's sprites; model loaders
/// (`native_register_model_loaders`), so models naming one get the mod's quads; item model node
/// types (`native_register_item_models`), so item definitions' nodes of them draw what the mod
/// answers; and animation targets and interpolations (`native_register_json_animation_types`),
/// bound directly to their callbacks so animation files resolve and convert them at parse time.
/// Block-state
/// definition types (`native_register_block_state_models`), so blockstate files naming one get
/// the models the mod picks; and special item model factories
/// (`native_register_special_model_renderer`), so each special model node of a registered type
/// bakes its own renderer.
pub(in crate::game) fn register_resource_sources(role: ExecutionRole) {
    if !role.is_client() {
        return;
    }
    let mut sources = Vec::new();
    crate::game::native_hooks::native_register_item_tint_sources(
        &RegistrationContext::new(),
        &mut sources,
    );
    sunrise_client::item_render::install_item_tint_factories(sources);
    let mut sources = Vec::new();
    crate::game::native_hooks::native_register_sprite_sources(
        &RegistrationContext::new(),
        &mut sources,
    );
    sunrise_client_render::mod_sprite_sources::register(sources);
    let mut loaders = Vec::new();
    crate::game::native_hooks::native_register_model_loaders(
        &RegistrationContext::new(),
        &mut loaders,
    );
    sunrise_client_render::mod_model_loaders::register(loaders);
    let mut models = Vec::new();
    crate::game::native_hooks::native_register_item_models(
        &RegistrationContext::new(),
        &mut models,
    );
    sunrise_client::item_render::install_item_model_factories(models);
    let mut types = Vec::new();
    crate::game::native_hooks::native_register_json_animation_types(
        &RegistrationContext::new(),
        &mut types,
    );
    sunrise_client_render::mod_animations::register(types);
    let mut definitions = Vec::new();
    crate::game::native_hooks::native_register_block_state_models(
        &RegistrationContext::new(),
        &mut definitions,
    );
    sunrise_client_render::mod_block_state_models::register(definitions);
    register_special_model_renderers();
}

/// The native mods' special item model renderers (`native_register_special_model_renderer`),
/// kept once at start, before resources load: their types are what a `minecraft:special` model
/// may name besides vanilla's, or in place of one. A later registration of a type replaces the
/// earlier one, and the log names both mods.
pub(in crate::game) fn register_special_model_renderers() {
    let mut registry = mod_api::SpecialModelRendererRegistry::default();
    crate::game::native_hooks::native_register_special_model_renderer(
        &RegistrationContext::new(),
        &mut registry,
    );
    for (kind, owner) in registry.initial_owners() {
        if sunrise_assets::item_model::SpecialModel::is_builtin_type(&kind.to_string()) {
            log::warn!(
                "Sunrise: special model type {kind} registered by the game is replaced by {}'s registration",
                owner.unwrap_or("the game")
            );
        }
    }
    let (factories, replaced) = registry.into_renderers();
    for replaced in replaced {
        let name = |id: Option<&str>| id.unwrap_or("the game").to_owned();
        log::warn!(
            "Sunrise: special model type {} registered by {} is replaced by {}'s registration",
            replaced.kind,
            name(replaced.earlier),
            name(replaced.later)
        );
    }
    sunrise_client::item_render::install_special_model_factories(
        factories
            .into_iter()
            .filter_map(|(kind, (owner, factory))| {
                let owner = owner?;
                Some((
                    kind,
                    sunrise_client::item_render::OwnedSpecialModelFactory::new(owner, factory),
                ))
            })
            .collect(),
    );
}

/// The client presentation registrations, kept once at start, before resources load: the
/// dimension transition screens, the entity spectator shaders, the render pipelines and the entity
/// renderers.
pub(in crate::game) fn register_client_presentations(
    role: ExecutionRole,
) -> sunrise_game_data::Result<()> {
    register_preset_editors(role);
    register_dimension_transition_screens(role);
    register_entity_spectator_shaders(role);
    register_render_pipelines(role)?;
    register_entity_renderers(role);
    Ok(())
}

/// Resolve client dimension-transition registrations once, before the first level opens.
pub(in crate::game) fn register_dimension_transition_screens(role: ExecutionRole) {
    if !role.is_client() {
        return;
    }
    let mut screens = Vec::new();
    crate::game::native_hooks::native_register_dimension_transition_screen(
        &RegistrationContext::new(),
        &mut screens,
    );
    sunrise_client_render::dimension_transition::register(screens);
}

/// Resolve client spectator post-effect registrations once, before the first level opens.
pub(in crate::game) fn register_entity_spectator_shaders(role: ExecutionRole) {
    if !role.is_client() {
        return;
    }
    let mut shaders = Vec::new();
    crate::game::native_hooks::native_register_entity_spectator_shaders(
        &RegistrationContext::new(),
        &mut shaders,
    );
    sunrise_client_render::spectator_shaders::register(shaders);
}

pub(in crate::game) fn register_preset_editors(role: ExecutionRole) {
    if !role.is_client() {
        return;
    }
    let mut editors = Vec::new();
    crate::game::native_hooks::register_preset_editors::native_register_preset_editors(
        &RegistrationContext::new(),
        &mut editors,
    );
    sunrise_client::title::preset_editors::register(editors);
}

/// Keep the mods' render pipeline registrations once at startup, before resources load: every
/// load builds them from its own shader files.
pub(in crate::game) fn register_render_pipelines(
    role: ExecutionRole,
) -> sunrise_game_data::Result<()> {
    if !role.is_client() {
        return Ok(());
    }
    let mut pipelines = Vec::new();
    crate::game::native_hooks::native_register_render_pipelines(
        &RegistrationContext::new(),
        &mut pipelines,
    );
    sunrise_client_render::mod_pipelines::register(pipelines)?;
    Ok(())
}

/// The native mods' meshing registrations, kept once at start, before resources load: colour
/// resolvers, then the block tint sources that read them, then section geometry renderers.
pub(in crate::game) fn register_meshing(role: ExecutionRole) {
    register_color_resolvers(role);
    register_block_tint_sources(role);
    add_section_geometry(role);
}

/// The native mods' colour resolvers (`native_register_color_resolvers`), kept once at start,
/// before the block tint sources that read them and before resources load.
pub(in crate::game) fn register_color_resolvers(role: ExecutionRole) {
    if !role.is_client() {
        return;
    }
    let mut registry = mod_api::ColorResolverRegistry::default();
    crate::game::native_hooks::native_register_color_resolvers(
        &RegistrationContext::new(),
        &mut registry,
    );
    sunrise_client::mesher::register_color_resolvers(registry.into_resolvers());
}

/// The native mods' block tint sources (`native_register_block_tint_sources`), kept once at
/// start, before resources load; every resource load applies them to its mesh tables.
pub(in crate::game) fn register_block_tint_sources(role: ExecutionRole) {
    if !role.is_client() {
        return;
    }
    let mut registry = mod_api::BlockTintRegistry::default();
    crate::game::native_hooks::native_register_block_tint_sources(
        &RegistrationContext::new(),
        &mut registry,
    );
    sunrise_client::mesher::register_block_tint_sources(registry.into_layers());
}

/// The native mods' section geometry renderers (`native_add_section_geometry`), kept once at
/// start, before resources load; every mesh of a section asks them to add their quads.
pub(in crate::game) fn add_section_geometry(role: ExecutionRole) {
    if !role.is_client() {
        return;
    }
    let mut registry = mod_api::SectionGeometryRegistry::default();
    crate::game::native_hooks::native_add_section_geometry(
        &RegistrationContext::new(),
        &mut registry,
    );
    sunrise_client::mesher::register_section_geometry(registry.into_renderers());
}

/// The native mods' entity renderers (`native_register_entity_renderers`), kept once at start,
/// before resources load; every resource load resolves each type's renderer.
pub(in crate::game) fn register_entity_renderers(role: ExecutionRole) {
    if !role.is_client() {
        return;
    }
    let mut registry = mod_api::EntityRendererRegistry::default();
    crate::game::native_hooks::native_register_entity_renderers(
        &RegistrationContext::new(),
        &mut registry,
    );
    #[cfg(feature = "client")]
    crate::game::client_side::native_entity_renderers::register(registry.into_renderers());
}

/// The native mods' particle providers (`native_register_particle_providers`) and render groups
/// (`native_register_particle_groups`) are registered once at startup. Each particle engine
/// constructs its groups once; sprite-set loads associate types with those groups and providers.
pub(in crate::game) fn register_particles(role: ExecutionRole) -> sunrise_game_data::Result<()> {
    if !role.is_client() {
        return Ok(());
    }
    let mut registry = mod_api::ParticleProviderRegistry::default();
    crate::game::native_hooks::native_register_particle_providers(
        &RegistrationContext::new(),
        &mut registry,
    );
    sunrise_client::particles::register_particle_providers(registry.into_providers());
    let mut group_factories = Vec::new();
    crate::game::native_hooks::native_register_particle_groups(
        &RegistrationContext::new(),
        &mut group_factories,
    );
    sunrise_client::particles::register_particle_groups(group_factories)?;
    Ok(())
}

pub(in crate::game) fn content_root(
    root: &std::path::Path,
    sources: &ModsState,
) -> sunrise_game_data::Result<sunrise_assets::resource_source::ResourceRoot> {
    let set_fluid_looks =
        if sources.installation_side == sunrise_mod_package::manifest::Side::Server {
            sunrise_sim::mod_content::set_fluid_looks
        } else {
            sunrise_client::mod_content_client::set_fluid_looks
        };
    crate::game::mods::content_root_with(root, sources, set_fluid_looks)
}
