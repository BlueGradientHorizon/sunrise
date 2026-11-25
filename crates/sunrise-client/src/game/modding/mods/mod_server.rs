#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{
    Game, execution_role::ExecutionRole, owner_link::OwnerRequest,
    registration_capability_coverage as coverage,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::CodeSwitches;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::ContentOverlay;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::ContentRequest;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::DeclarationFailures;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::DisabledServingHost;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::ENCHANTMENT_DAMAGE_EFFECTS;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::ModsReload;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::ModsState;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::NativeDeclared;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::PreparedLoopContext;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::ServedContentLoadError;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::ServedContentStage;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::VERDICTS;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::Verdict;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::add_section_geometry;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::announced_entry;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::builtin_registries;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::builtin_registry_names;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::collect_cauldron_fluid_contents;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::collect_fluid_models;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::collect_key_mappings;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::collect_poi_extensions;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::content_layer_owner;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::content_overlay;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::content_overlay_with_refusals;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::content_owner_pairs;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::content_root;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::debug_renderers;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::extend_poi_types;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::freeze_declarations;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::install_registration_data;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::item_data_load::{
    LoadRole, load_item_data,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::live_switch_of;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::living_mod_types;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::mob_effect_entries;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::native_declared;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::native_mods_detail;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::not_built;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::read_contents;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::read_package_inputs;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::refuse_primary_namespace_claims;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::register_block_entity_types;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::register_block_tint_sources;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::register_cauldron_fluid_contents;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::register_client_presentations;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::register_client_tooltip_component_factories;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::register_color_resolvers;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::register_components;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::register_debug_entries;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::register_dimension_transition_screens;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::register_enchantment_damage_effects;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::register_entity_renderers;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::register_entity_spectator_shaders;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::register_entity_types;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::register_fluid_models;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::register_gui_layers;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::register_iterated_registries;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::register_layer_definitions;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::register_map_decoration_types;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::register_menu_types;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::register_meshing;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::register_mob_effects;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::register_particles;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::register_preset_editors;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::register_range_select_item_model_properties;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::register_render_pipelines;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::register_resource_sources;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::register_select_item_model_properties;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::register_special_model_renderers;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::register_standalone_models;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::remember_verdict;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::resolve_particle_providers;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::standalone::collect_standalone_models;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::validate_entity_type_families;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::validate_startup_packages;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::verdict;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::verdict_key;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::mods::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::__private::{
    DeclarationKind, DeclarationRefusal, DeclarationSink, game::*,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::{
    Answered, DeclarationError, LoadingMessage, RegistrationContext,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    lang,
    mobs::spawner::SpawnerTypeFacts,
    mod_content::DeclaredContent,
    mod_list::{self, Installed},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_mod_package::manifest::{
    MissingJar, collect_jar_requirements, missing_jars,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_world::{
    item_defaults::ItemData,
    native_load_context::{LoadContext, LoadInputs},
};

impl Game<'_> {
    pub(in crate::game) fn begin_condition_reload(
        &mut self,
        stack: &crate::game::server_side::functions::PackStack,
        resources: &std::path::Path,
    ) -> Result<
        crate::game::server_side::server_reload::PreparedServerReload,
        mod_api::__private::reload::ReloadStageError,
    > {
        use mod_api::__private::reload::ReloadStageError;
        if self.mods.load_condition_candidate.is_none() {
            let inventory = sunrise_mod_package::enabled::EnabledMods::read(resources)
                .map_err(|error| ReloadStageError::DataSource(error.to_string()))?;
            self.mods.load_condition_candidate = Some(
                crate::game::load_conditions::conditions_for_mods(&self.mods, &inventory)
                    .map_err(ReloadStageError::DataSource)?,
            );
        }
        self.prepare_server_reload(stack)
    }

    pub(in crate::game) fn prepare_pack_condition_reload(
        &mut self,
        stack: &crate::game::server_side::functions::PackStack,
    ) -> Result<
        crate::game::server_side::server_reload::PreparedServerReload,
        mod_api::__private::reload::ReloadStageError,
    > {
        self.mods.load_condition_candidate = None;
        self.begin_condition_reload(stack, &self.options.resources.clone())
    }

    pub(in crate::game) fn server_set_audio_loop_context(
        &mut self,
        audio: Option<sunrise_world::native_load_context::LoopContext>,
    ) {
        debug_assert!(audio.is_none());
    }

    pub(in crate::game) fn server_retire_disabled_instance_groups(
        &self,
        _effective: &std::collections::BTreeSet<String>,
    ) {
    }

    /// The server's side of a live switch: the data packs reload from the new root.
    pub(in crate::game) fn apply_mods_reload(&mut self, reload: ModsReload) {
        let at = std::time::Instant::now();
        self.options.resources = reload.root;
        self.pack.data_source = reload.data_source;
        self.pack.data_changes = reload.data_changes;
        self.pack.additional_data = reload.additional_data;
        self.settings.mods.enabled = reload.enabled.clone();
        self.mods.applied = Some(reload.enabled);
        self.mods.selected = reload.selected;
        self.mods.refused = reload.refused;
        self.mods.data_switched = true;
        if self.server.world_root.as_os_str().is_empty() {
            return;
        }
        if let Err(error) = self.reload_datapacks(true, &self.level.enabled_features.clone()) {
            log::warn!(
                "Sunrise: content mods data reload failed: {}",
                error.diagnostic()
            );
            return;
        }
        log::info!(
            "Sunrise: content mods switched live: server data reloaded in {} ms",
            at.elapsed().as_millis()
        );
    }
}

#[cfg(test)]
pub(in crate::game) mod tests {
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use super::*;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::mods::tests::BUILT;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::mods::tests::Fixture;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::mods::tests::run_in_child;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::mods::tests::smod;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::mods::tests::*;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use mod_api::__private::native::BuiltMod;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use sunrise_mod_package::runtime::Runtime;

    pub(in crate::game) mod inventory {
        include!("mod_inventory_tests_server.rs");
    }
}
