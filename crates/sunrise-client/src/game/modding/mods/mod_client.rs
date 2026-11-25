#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{
    Game, execution_role::ExecutionRole, owner_link::OwnerRequest,
    registration_capability_coverage as coverage,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::CodeSwitches;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::ContentOverlay;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::ContentRequest;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::DeclarationFailures;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::DisabledServingHost;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::ENCHANTMENT_DAMAGE_EFFECTS;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::ModsReload;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::ModsState;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::NativeDeclared;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::PreparedLoopContext;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::ServedContentLoadError;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::ServedContentStage;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::VERDICTS;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::Verdict;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::add_section_geometry;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::announced_entry;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::builtin_registries;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::builtin_registry_names;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::collect_cauldron_fluid_contents;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::collect_fluid_models;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::collect_key_mappings;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::collect_poi_extensions;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::content_layer_owner;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::content_overlay;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::content_overlay_with_refusals;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::content_owner_pairs;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::content_root;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::debug_renderers;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::extend_poi_types;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::freeze_declarations;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::install_registration_data;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::item_data_load::{
    LoadRole, load_item_data,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::live_switch_of;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::living_mod_types;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::mob_effect_entries;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::native_declared;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::native_mods_detail;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::not_built;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::read_contents;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::read_package_inputs;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::refuse_primary_namespace_claims;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::register_block_entity_types;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::register_block_tint_sources;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::register_cauldron_fluid_contents;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::register_client_presentations;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::register_client_tooltip_component_factories;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::register_color_resolvers;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::register_components;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::register_debug_entries;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::register_dimension_transition_screens;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::register_enchantment_damage_effects;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::register_entity_renderers;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::register_entity_spectator_shaders;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::register_entity_types;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::register_fluid_models;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::register_gui_layers;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::register_iterated_registries;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::register_layer_definitions;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::register_map_decoration_types;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::register_menu_types;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::register_meshing;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::register_mob_effects;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::register_particles;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::register_preset_editors;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::register_range_select_item_model_properties;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::register_render_pipelines;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::register_resource_sources;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::register_select_item_model_properties;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::register_special_model_renderers;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::register_standalone_models;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::remember_verdict;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::resolve_particle_providers;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::standalone::collect_standalone_models;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::validate_entity_type_families;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::validate_startup_packages;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::verdict;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::verdict_key;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::__private::{
    DeclarationKind, DeclarationRefusal, DeclarationSink, game::*,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{
    Answered, DeclarationError, LoadingMessage, RegistrationContext,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    lang,
    mobs::spawner::SpawnerTypeFacts,
    mod_content::DeclaredContent,
    mod_list::{self, Installed},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_mod_package::manifest::{
    MissingJar, collect_jar_requirements, missing_jars,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_world::{
    item_defaults::ItemData,
    native_load_context::{LoadContext, LoadInputs},
};

#[path = "package_files_client.rs"]
pub(in crate::game) mod package_files;

impl Game<'_, crate::game::client_side::ClientState> {
    /// The content mods the resources load with: the ones applied, which a
    /// switch that needs a restart doesn't change.
    pub(in crate::game) fn applied_mods(&self) -> std::collections::BTreeSet<String> {
        self.mods
            .applied
            .clone()
            .unwrap_or_else(|| self.settings.mods.enabled.clone())
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
        if let Some(server) = &self.integrated {
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
        let wanted = self.settings.mods.enabled.clone();
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
            .reconcile_code_switches(&self.settings.mods.disabled);
        for id in host_waiting {
            log::info!("Sunrise: serving host mod {id} needs a restart to switch");
            if !waiting.contains(&id) {
                waiting.push(id);
            }
        }
        self.ui.options.mods_note = (!waiting.is_empty())
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
}

impl ModsState {
    pub(in crate::game) fn startup_admission(&self) -> Option<&package_files::StartupAdmission> {
        self.startup_admission
            .as_deref()
            .filter(|admission| admission.matches_selection(&self.selected))
    }
}
