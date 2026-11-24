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

impl Game<'_> {
    pub(in crate::game) fn install_loop_context(&mut self, context: PreparedLoopContext) {
        self.protect_condition_root();
        self.loop_context = context.game;
        self.refresh_recipe_property_context();
        self.setting_overrides.borrow_mut().clear();
        if let Some(audio) = context.audio {
            self.audio.0.set_loop_context(audio);
        }
        if let Some(worldgen) = context.worldgen {
            sunrise_world::native_worldgen_hooks::publish_context(Arc::new(worldgen));
        }
    }
    /// Apply the player's switches at the canonical native dispatch boundary.
    pub(in crate::game) fn sync_native_switches(&self) {
        let (effective, _) = self
            .mods
            .reconcile_code_switches(&self.settings.mods.disabled);
        self.client
            .instance_groups
            .borrow_mut()
            .retire_disabled(&effective);
        self.mods.apply_code_switches(&effective);
    }
}
