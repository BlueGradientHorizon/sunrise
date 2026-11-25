//! Native hook implementations, split by catalog area.
//!
//! Hook documentation and contract versions live in the catalog, not a shared changelog.
//!
//! Native mods are built against an exact game revision, rather than a hook API revision.
//! Bodies have no `cfg` and no macros: horizon-mixins refuses a target with
//! either (`tests::hook_points_stay_hookable`).

/// Generated dispatch entry points, not hand-written hook implementations.
pub(in crate::game) mod fire;
#[allow(unused_imports)]
pub(in crate::game) use block_entity_tick::ResidentBlockEntityAccess;
#[allow(unused_imports)]
pub(in crate::game) use build_creative_mode_tab_contents::CreativeCache;
#[allow(unused_imports)]
pub(in crate::game) use client_pick::ClientPick;
#[allow(unused_imports)]
pub(in crate::game) use fire::{
    ConnectsToCall, EntityChangeBlockPreCall, HotbarAttackIndicatorProgressCall,
    ItemAfterVerticalCollisionCall, PlayerPongCall, PreRemoveSideEffectsCall, UpdateShapeCall,
};
#[allow(unused_imports)]
use mod_api::{BlockEntityDataSource, BlockEntityRef, ClockAdjustment};

#[allow(unused_imports)]
use super::native_data_maps::DataMapType;
#[allow(unused_imports)]
use super::{
    Game, GameLevel, LevelDimension, Screen,
    execution_role::{self, ExecutionRole},
    mod_api_host,
    native_block_entity_draw::BlockEntityDraw,
    native_configs::ConfigFile,
    native_mob_draw::{MobRender, PlayerRender},
    native_panel_recipes::RecipePage,
};
#[allow(unused_imports)]
use mod_api::__private::game::{
    BlockStateDefinitionRegistration, ItemModelRegistration, ItemTintSourceRegistration,
    JsonAnimationTypeRegistration, LayerDefinitionRegistration, ModelLoaderRegistration,
    ParticleGroupRegistration, PresetEditorFactoryRegistration, RangeSelectPropertyRegistration,
    SelectPropertyRegistration, SpriteSourceRegistration, StandaloneModelRegistration,
};
#[allow(unused_imports)]
use mod_api::GameMode;
#[allow(unused_imports)]
use mod_api::GameTests;
#[allow(unused_imports)]
pub use mod_api::game::{PhantomSpawnResult, Shake, SleepProblem};
#[allow(unused_imports)]
use mod_api::{
    self, Answered, BlockPlacement, BlockTintSources, CauldronFluidContents, CauldronInteractions,
    ClientAudioOptions, ClientChatOptions, ClientGraphicsOptions, ClientGuiOptions,
    ClientInputOptions, ClientMiningPrediction, ClientModifiers, ClientReloadListeners,
    ColorResolvers, EnchantmentDamageEffects, EnchantmentInfo, EntityLayers, EntityRef,
    EntityRenderers, GameRuleActor, GuiLayerRegistrar, HandDrawContext, HandRender, InsideEffects,
    InteractionAttempt, InteractionKey, ItemEntityRef, LifecycleContext, LivingRef, LoadConditions,
    NoteBlockChange, ParticleProviders, PathComputationType, Placement, PlayerRef,
    PoiTypeExtensions, RecipeBookCategory, ScreenId, ScreenOpening, ScreenTarget, ScreenWidgets,
    ScreenshotImage, ServerReloadListeners, ServerTickPhase, SpecialModelRenderers,
    TicketController, ToolUse, VillagerProfessions, WorldPhase,
};
#[allow(unused_imports)]
use mod_api::{
    ActiveEffect, AttachmentStore, BlockClick, BlockUpdateFlags, BonemealSource, ChunkRef,
    FluidStack, ItemClick, LevelRef, LogicalSide, NeighbourUpdate, TooltipElement, TriState,
    UsePhase,
};
#[allow(unused_imports)]
use mod_api::{
    BlockKey, DebugEntry, DebugRenderer, EffectKey, FrameModel, FrameModelEdit, MapDecorationType,
    SkullModel, TextureAtlasRegistration,
};
#[allow(unused_imports)]
pub(crate) use mod_api::{
    BlockRim, ClientPacket, ClientPayload, ColoredLight, ContainerLootFilled, ExplosionStart,
    ItemFished, LootDrops, PanelEvent, PortalShape, ScreenView,
};
#[allow(unused_imports)]
use mod_api::{
    CameraType, Despawn, Identifier, MobRef, PermissionLevel, RailShape, SoundPlay, SoundRef,
};
#[allow(unused_imports)]
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
#[allow(unused_imports)]
use sunrise_client::block_contact::Pending;
#[allow(unused_imports)]
#[cfg(test)]
use sunrise_client::mobs::Damage;
#[allow(unused_imports)]
use sunrise_client::{
    atlas_sources::qualified,
    biome::{BiomeColors, SpawnEntry},
    block_entities::Pos,
    chat::ChatSource,
    containers::{
        BlockEntity, MOD_MENUS, ModBlockEntityFields, ModContainer, ModEnergyBuffer, ModFluidTanks,
    },
    crafting::{Ingredient, recipe_result},
    extras::Corner,
    extras_hud::Corners,
    fast_hash::FxHashMap,
    first_person::Hand,
    health::{
        Attack, DamageContainer, DamageHooks, DamageLevel, HurtSource, Hurting, Knock, ShieldBlock,
        Victim,
    },
    hud::{HudBatch, rgba},
    items::ItemStack,
    keybinds,
    merchant::MerchantOffer,
    mobs::Who,
    mod_list::{self, Installed, KeyMapping, NativePage, NativeSetting, NativeSettingValue},
    music::{Music, MusicLayer},
    particles::{ParticleOptions, ParticleType},
    text::Text,
};
#[allow(unused_imports)]
use sunrise_client_core::gamerules::Value as GameRuleValue;
#[allow(unused_imports)]
use sunrise_client_core::suggestion::Suggestion;
#[allow(unused_imports)]
use sunrise_core::{
    connection_kind::ConnectionKind,
    geometry::Direction,
    position::BlockPos,
    ticks::{ScheduleResult, TickPriority},
};
#[allow(unused_imports)]
use sunrise_world::level::{BonemealCheck, PlaceContext};
#[allow(unused_imports)]
use winit::{event::MouseButton, keyboard::KeyCode};
#[allow(unused_imports)]
use {
    sunrise_core::{
        nbt::{Compound, Tag},
        use_animation::UseAnimation,
    },
    sunrise_world::{
        fluid::FluidKind,
        level::{LevelAccess, PlacementContext, Ticks},
        registry::{BlockRegistry, BlockStateId},
    },
};

pub(in crate::game) mod add_attribute_tooltips;
pub(in crate::game) mod add_client_reload_listeners;
pub(in crate::game) mod add_pack_finders;
pub(in crate::game) mod add_section_geometry;
pub(in crate::game) mod add_server_reload_listeners;
pub(in crate::game) mod adjacent_block_path_type;
pub(in crate::game) mod affect_neighbors_after_removal;
pub(in crate::game) mod animal_tame;
pub(in crate::game) mod animate_tick;
#[cfg(test)]
pub(in crate::game) mod animate_tick_tests;
pub(in crate::game) mod anvil_update;
pub(in crate::game) mod apply_enchantments;
pub(in crate::game) mod arm_pose_transform;
pub(in crate::game) mod armor_hurt;
pub(in crate::game) mod arrow_nock;
pub(in crate::game) mod baby_entity_spawn;
pub(in crate::game) mod base;
pub(in crate::game) mod beacon_color_multiplier;
pub(in crate::game) mod block_drops;
pub(in crate::game) mod block_entity_client_tick;
#[cfg(test)]
pub(in crate::game) mod block_entity_client_tick_tests;
pub(in crate::game) mod block_entity_data_received;
pub(in crate::game) mod block_entity_load_additional;
pub(in crate::game) mod block_entity_neighbors;
#[cfg(test)]
pub(in crate::game) mod block_entity_neighbors_tests;
pub(in crate::game) mod block_entity_on_load;
pub(in crate::game) mod block_entity_on_load_routes_tests;
pub(in crate::game) mod block_entity_on_load_tests;
pub(in crate::game) mod block_entity_save_additional;
pub(in crate::game) mod block_entity_save_additional_tests;
pub(in crate::game) mod block_entity_tick;
pub(in crate::game) mod block_entity_type_add_blocks;
pub(in crate::game) mod block_entity_view_distance;
pub(in crate::game) mod block_grow_feature;
pub(in crate::game) mod block_landing;
pub(in crate::game) mod block_particle_effects;
#[cfg(test)]
pub(in crate::game) mod block_particle_effects_tests;
pub(in crate::game) mod block_path_type;
pub(in crate::game) mod block_render_visibility;
pub(in crate::game) mod block_sticky;
pub(in crate::game) mod block_tool_modification;
pub(in crate::game) mod block_use_item_on;
pub(in crate::game) mod block_use_without_item;
pub(in crate::game) mod bonemeal;
pub(in crate::game) mod bounce_restitution;
pub(in crate::game) mod break_progress_positions;
pub(in crate::game) mod build_creative_mode_tab_contents;
pub(in crate::game) mod calculate_player_turn;
pub(in crate::game) mod camera_pose;
#[cfg(test)]
pub(in crate::game) mod camera_pose_tests;
pub(in crate::game) mod camera_read;
pub(in crate::game) mod camera_submersion;
pub(in crate::game) mod camera_type;
pub(in crate::game) mod can_be_connected_to;
#[cfg(test)]
pub(in crate::game) mod can_be_connected_to_tests;
pub(in crate::game) mod can_be_hydrated;
pub(in crate::game) mod can_continue_sleeping;
pub(in crate::game) mod can_drop_from_explosion;
pub(in crate::game) mod can_entity_destroy;
pub(in crate::game) mod can_equip;
pub(in crate::game) mod can_harvest_block;
pub(in crate::game) mod can_perform_action;
pub(in crate::game) mod can_place_item;
pub(in crate::game) mod can_player_sleep;
pub(in crate::game) mod can_start_swimming;
pub(in crate::game) mod can_sustain_plant;
pub(in crate::game) mod chat_recipient;
pub(in crate::game) mod chat_suggestion_applied;
pub(in crate::game) mod chunk_ticket_level_updated;
pub(in crate::game) mod chunk_watch;
pub(in crate::game) mod client_attack_target;
pub(in crate::game) mod client_block_changed;
pub(in crate::game) mod client_chat;
pub(in crate::game) mod client_chat_received;
#[cfg(test)]
pub(in crate::game) mod client_chunk_hooks_tests;
pub(in crate::game) mod client_chunk_load;
pub(in crate::game) mod client_chunk_unload;
pub(in crate::game) mod client_gui_state;
pub(in crate::game) mod client_information_updated;
pub(in crate::game) mod client_input_options;
pub(in crate::game) mod client_input_reads;
pub(in crate::game) mod client_input_reads_tests;
pub(in crate::game) mod client_mining_predict;
pub(in crate::game) mod client_narrator;
#[cfg(test)]
pub(in crate::game) mod client_narrator_tests;
pub(in crate::game) mod client_options;
pub(in crate::game) mod client_packet;
pub(in crate::game) mod client_packet_send;
pub(in crate::game) mod client_pause_change;
pub(in crate::game) mod client_pause_read;
pub(in crate::game) mod client_pick;
pub(in crate::game) mod client_player_change_game_type;
pub(in crate::game) mod client_player_clone;
pub(in crate::game) mod client_player_logging_in;
pub(in crate::game) mod client_player_logging_out;
pub(in crate::game) mod client_services;
pub(in crate::game) mod client_setup;
pub(in crate::game) mod client_stopped;
pub(in crate::game) mod client_stopping;
pub(in crate::game) mod client_text;
pub(in crate::game) mod client_tick_post;
pub(in crate::game) mod client_tick_pre;
pub(in crate::game) mod client_visual_text;
pub(in crate::game) mod clone_item_stack;
pub(in crate::game) mod component_hover;
pub(in crate::game) mod compute_fov_modifier;
pub(in crate::game) mod connects_to;
pub(in crate::game) mod console_line;
pub(in crate::game) mod contained_fluid;
pub(in crate::game) mod container_loot_filled;
pub(in crate::game) mod contents_changed;
pub(in crate::game) mod crafting_remainder;
pub(in crate::game) mod create_fluid_source;
pub(in crate::game) mod create_skull_models;
pub(in crate::game) mod create_spawn_position;
pub(in crate::game) mod creative_tabs_read;
#[cfg(test)]
pub(in crate::game) mod creative_tabs_read_tests;
pub(in crate::game) mod creator_mod_id;
pub(in crate::game) mod crop_grow;
pub(in crate::game) mod custom_click_action;
pub(in crate::game) mod custom_spawners;
pub(in crate::game) mod customize_boss_event_progress;
pub(in crate::game) mod customize_chat_overlay;
pub(in crate::game) mod damage_item;
pub(in crate::game) mod data_maps_updated;
#[cfg(test)]
pub(in crate::game) mod death_source_tests;
pub(in crate::game) mod default_components_bound;
pub(in crate::game) mod dispense_method;
pub(in crate::game) mod draw_measures;
pub(in crate::game) mod effect_particle_modification;
pub(in crate::game) mod empty_contents;
pub(in crate::game) mod enchant_power_bonus;
pub(in crate::game) mod enchanted_block_loot;
pub(in crate::game) mod enchanted_entity_loot;
#[cfg(test)]
pub(in crate::game) mod enchantment_and_lang_tests;
pub(in crate::game) mod enchantment_level_set;
pub(in crate::game) mod enchantments;
pub(in crate::game) mod enderman_anger;
pub(in crate::game) mod entity_change_block_pre;
pub(in crate::game) mod entity_constructing;
pub(in crate::game) mod entity_entering_section;
pub(in crate::game) mod entity_inside;
pub(in crate::game) mod entity_interact_specific;
pub(in crate::game) mod entity_invulnerability_check;
pub(in crate::game) mod entity_mob_griefing;
pub(in crate::game) mod entity_outline;
pub(in crate::game) mod entity_place;
pub(in crate::game) mod entity_size;
pub(in crate::game) mod entity_struck_by_lightning;
pub(in crate::game) mod entity_travel_to_dimension;
pub(in crate::game) mod exp_drop;
pub(in crate::game) mod explosion_detonate;
pub(in crate::game) mod explosion_knockback;
pub(in crate::game) mod explosion_notice_containment;
pub(in crate::game) mod explosion_resistance;
pub(in crate::game) mod explosion_start;
pub(in crate::game) mod extend_poi_types;
pub(in crate::game) mod extract_block_screen_effect;
pub(in crate::game) mod falling_tick;
pub(in crate::game) mod farmland_trample;
pub(in crate::game) mod fence_gate_in_wall;
#[cfg(test)]
pub(in crate::game) mod fence_gate_in_wall_tests;
pub(in crate::game) mod finalize_spawn;
pub(in crate::game) mod fire_behaviour;
pub(in crate::game) mod fire_odds;
pub(in crate::game) mod fishing_medium;
pub(in crate::game) mod flip_frame;
pub(in crate::game) mod fluid_draw;
pub(in crate::game) mod fluid_place_block;
pub(in crate::game) mod fluid_rules;
pub(in crate::game) mod fluid_tick;
pub(in crate::game) mod fluid_tooltip;
pub(in crate::game) mod friction;
pub(in crate::game) mod game_rule_changed;
pub(in crate::game) mod game_shutting_down;
pub(in crate::game) mod gather_effect_screen_tooltips;
pub(in crate::game) mod gather_skipped_attribute_tooltips;
pub(in crate::game) mod general_entity_tick;
pub(in crate::game) mod get_arm_pose;
pub(in crate::game) mod grindstone_place;
pub(in crate::game) mod grindstone_take;
#[cfg(test)]
pub(in crate::game) mod guest_death_lifecycle_tests;
pub(in crate::game) mod gui_render_post;
pub(in crate::game) mod gui_ticks;
pub(in crate::game) mod held_item_tint;
pub(in crate::game) mod highlight_tip;
pub(in crate::game) mod hotbar_attack_indicator_progress;
pub(in crate::game) mod hud_mode;
pub(in crate::game) mod humanoid_setup_anim;
pub(in crate::game) mod ignited_by_lava;
pub(in crate::game) mod incoming_tests;
pub(in crate::game) mod ingredient_test;
pub(in crate::game) mod input_key;
pub(in crate::game) mod interaction;
pub(in crate::game) mod is_fertile;
pub(in crate::game) mod is_fire_source;
pub(in crate::game) mod is_not_replaceable_by_pick_action;
pub(in crate::game) mod is_pathfindable;
pub(in crate::game) mod is_piglin_currency;
pub(in crate::game) mod is_possible_to_respawn_in;
pub(in crate::game) mod is_primary_item_for;
pub(in crate::game) mod item_after_vertical_collision;
pub(in crate::game) mod item_attack;
pub(in crate::game) mod item_container;
pub(in crate::game) mod item_crafted;
pub(in crate::game) mod item_destroyed;
pub(in crate::game) mod item_fished;
pub(in crate::game) mod item_interact_mob;
pub(in crate::game) mod item_model_property;
pub(in crate::game) mod item_picked_up;
#[cfg(test)]
pub(in crate::game) mod item_picked_up_tests;
pub(in crate::game) mod item_smelted;
pub(in crate::game) mod item_stacked_on_other;
pub(in crate::game) mod item_tooltip;
pub(in crate::game) mod item_toss;
pub(in crate::game) mod key_mapping_pre;
pub(in crate::game) mod left_click_empty;
pub(in crate::game) mod level_save;
pub(in crate::game) mod level_sounds;
pub(in crate::game) mod level_tick_tests;
pub(in crate::game) mod level_world;
pub(in crate::game) mod light_emission;
pub(in crate::game) mod living_breathe;
pub(in crate::game) mod living_change_target;
pub(in crate::game) mod living_conversion_post;
pub(in crate::game) mod living_conversion_pre;
pub(in crate::game) mod living_damage_post;
pub(in crate::game) mod living_damage_pre;
pub(in crate::game) mod living_death;
pub(in crate::game) mod living_death_message;
pub(in crate::game) mod living_death_post;
pub(in crate::game) mod living_destroy_block;
pub(in crate::game) mod living_drops;
pub(in crate::game) mod living_experience_drop;
pub(in crate::game) mod living_get_projectile;
pub(in crate::game) mod living_shield_block;
pub(in crate::game) mod living_use_totem;
pub(in crate::game) mod living_visibility;
pub(in crate::game) mod makes_piglins_neutral;
pub(in crate::game) mod map_decoration_types;
pub(in crate::game) mod math;
pub(in crate::game) mod menu_extra_data;
pub(in crate::game) mod menu_links;
pub(in crate::game) mod menu_slot_changed;
pub(in crate::game) mod menu_slot_rules;
pub(in crate::game) mod menu_slot_take;
pub(in crate::game) mod menu_transfer;
pub(in crate::game) mod minecart_pass;
pub(in crate::game) mod mob_effect_events;
pub(in crate::game) mod mob_spawn_checks;
pub(in crate::game) mod mob_split;
#[cfg(test)]
pub(in crate::game) mod mod_block_entity_payload_tests;
pub(in crate::game) mod mod_configs;
pub(in crate::game) mod mod_mismatch;
pub(in crate::game) mod mod_settings;
pub(in crate::game) mod model_baking_completed;
pub(in crate::game) mod movement_input;
pub(in crate::game) mod muffle_sounds;
pub(in crate::game) mod must_survive;
pub(in crate::game) mod native_note_block_change;
pub(in crate::game) mod native_potential_spawns;
pub(in crate::game) mod native_village_siege;
pub(in crate::game) mod neighbor_notify;
pub(in crate::game) mod note_block_play;
pub(in crate::game) mod on_block_exploded;
pub(in crate::game) mod on_block_placed_by;
#[cfg(test)]
pub(in crate::game) mod on_block_placed_by_tests;
pub(in crate::game) mod on_block_state_change;
#[cfg(test)]
pub(in crate::game) mod on_block_state_change_tests;
pub(in crate::game) mod on_caught_fire;
pub(in crate::game) mod on_chunk_unloaded;
#[cfg(test)]
pub(in crate::game) mod on_chunk_unloaded_tests;
pub(in crate::game) mod on_destroyed_by_player;
pub(in crate::game) mod on_destroyed_by_push_reaction;
#[cfg(test)]
pub(in crate::game) mod on_destroyed_by_push_reaction_tests;
pub(in crate::game) mod on_dropped_by_player;
pub(in crate::game) mod on_entity_item_update;
pub(in crate::game) mod on_item_use_first;
pub(in crate::game) mod panel_recipes;
#[cfg(test)]
pub(in crate::game) mod panel_recipes_tests;
pub(in crate::game) mod permissions_changed;
pub(in crate::game) mod piston_carries_block_entity;
pub(in crate::game) mod piston_push_reaction;
#[cfg(test)]
pub(in crate::game) mod piston_push_reaction_tests;
pub(in crate::game) mod place_liquid;
pub(in crate::game) mod placement_context;
#[cfg(test)]
pub(in crate::game) mod placement_context_tests;
pub(in crate::game) mod play_hit_sound;
#[cfg(test)]
pub(in crate::game) mod play_hit_sound_tests;
pub(in crate::game) mod player_bed_leave;
pub(in crate::game) mod player_break_speed;
pub(in crate::game) mod player_brewed_potion;
pub(in crate::game) mod player_destroy_item;
pub(in crate::game) mod player_destroyed_block;
pub(in crate::game) mod player_enchant_item;
pub(in crate::game) mod player_flyable_fall;
pub(in crate::game) mod player_game_mode_changed;
pub(in crate::game) mod player_harvest_check;
pub(in crate::game) mod player_heart_type;
pub(in crate::game) mod player_inventory_changed;
pub(in crate::game) mod player_item_smithed;
pub(in crate::game) mod player_leave;
pub(in crate::game) mod player_move;
pub(in crate::game) mod player_passage_cross;
pub(in crate::game) mod player_pong;
pub(in crate::game) mod player_set_spawn;
pub(in crate::game) mod player_spawn_phantoms;
pub(in crate::game) mod player_start_spectating_entity;
pub(in crate::game) mod player_swap_hands;
pub(in crate::game) mod player_xp_change;
pub(in crate::game) mod player_xp_level_change;
pub(in crate::game) mod portal_spawn;
pub(in crate::game) mod potion_brew_post;
pub(in crate::game) mod potion_brew_pre;
pub(in crate::game) mod pre_remove_side_effects;
pub(in crate::game) mod preset_title;
pub(in crate::game) mod rails;
pub(in crate::game) mod recipe_book_categories;
pub(in crate::game) mod recipe_book_ghost;
pub(in crate::game) mod recipe_book_place;
pub(in crate::game) mod recipe_sources;
pub(in crate::game) mod register_block_state_models;
pub(in crate::game) mod register_block_tint_sources;
pub(in crate::game) mod register_cauldron_fluid_content;
pub(in crate::game) mod register_cauldron_interaction;
pub(in crate::game) mod register_client_item_extensions;
pub(in crate::game) mod register_client_tooltip_component_factories;
pub(in crate::game) mod register_color_resolvers;
pub(in crate::game) mod register_configuration_tasks;
pub(in crate::game) mod register_debug_entries;
pub(in crate::game) mod register_debug_renderers;
pub(in crate::game) mod register_dimension_transition_screen;
pub(in crate::game) mod register_enchantment_damage_effects;
pub(in crate::game) mod register_entity_layers;
pub(in crate::game) mod register_entity_renderers;
pub(in crate::game) mod register_entity_spectator_shaders;
pub(in crate::game) mod register_fluid_models;
pub(in crate::game) mod register_game_tests;
pub(in crate::game) mod register_gui_layers;
pub(in crate::game) mod register_item_models;
pub(in crate::game) mod register_item_tint_sources;
pub(in crate::game) mod register_json_animation_types;
pub(in crate::game) mod register_layer_definitions;
pub(in crate::game) mod register_load_conditions;
pub(in crate::game) mod register_model_loaders;
pub(in crate::game) mod register_particle_groups;
pub(in crate::game) mod register_particle_providers;
pub(in crate::game) mod register_preset_editors;
pub(in crate::game) mod register_range_select_item_model_property;
pub(in crate::game) mod register_render_pipelines;
pub(in crate::game) mod register_select_item_model_property;
pub(in crate::game) mod register_special_model_renderer;
pub(in crate::game) mod register_sprite_sources;
pub(in crate::game) mod register_texture_atlases;
pub(in crate::game) mod register_villager_professions;
pub(in crate::game) mod registered_potions;
#[cfg(test)]
pub(in crate::game) mod registered_potions_tests;
pub(in crate::game) mod remove_block;
#[cfg(test)]
pub(in crate::game) mod remove_block_tests;
pub(in crate::game) mod render_arm;
pub(in crate::game) mod render_frame_post;
pub(in crate::game) mod render_frame_pre;
pub(in crate::game) mod render_gui_post;
pub(in crate::game) mod render_gui_pre;
pub(in crate::game) mod render_hand;
pub(in crate::game) mod render_item_in_frame;
pub(in crate::game) mod render_level_stage;
pub(in crate::game) mod render_living_post;
pub(in crate::game) mod render_living_pre;
pub(in crate::game) mod render_name_tag;
pub(in crate::game) mod render_player_post;
pub(in crate::game) mod render_player_pre;
pub(in crate::game) mod render_tooltip_gather_components;
pub(in crate::game) mod render_tooltip_pre;
pub(in crate::game) mod render_tooltip_texture;
pub(in crate::game) mod resolve_player_skin;
pub(in crate::game) mod respawn_position;
pub(in crate::game) mod right_click_empty;
pub(in crate::game) mod right_click_item;
pub(in crate::game) mod runtime_environment;
pub(in crate::game) mod screen_chars;
pub(in crate::game) mod screen_closing;
pub(in crate::game) mod screen_exclusion_areas;
pub(in crate::game) mod screen_hovered_stack;
pub(in crate::game) mod screen_init_post;
pub(in crate::game) mod screen_input_post;
#[cfg(test)]
pub(in crate::game) mod screen_input_tests;
pub(in crate::game) mod screen_opening;
pub(in crate::game) mod screen_preedit_effects;
pub(in crate::game) mod screen_render_stages;
pub(in crate::game) mod screen_room;
#[cfg(test)]
pub(in crate::game) mod screen_slot_tests;
pub(in crate::game) mod screen_transparent_background;
pub(in crate::game) mod screenshot;
pub(in crate::game) mod section_visibility;
pub(in crate::game) mod server_chat;
pub(in crate::game) mod server_tick_rate_changed;
pub(in crate::game) mod set_placed_by;
pub(in crate::game) mod should_cause_block_break_reset;
pub(in crate::game) mod should_cause_reequip_animation;
pub(in crate::game) mod should_check_weak_power;
pub(in crate::game) mod should_display_fluid_overlay;
pub(in crate::game) mod should_hide_adjacent_fluid_face;
pub(in crate::game) mod should_redstone_wire_connect_to;
pub(in crate::game) mod sound_and_shader_helpers;
pub(in crate::game) mod spread_players_teleport;
pub(in crate::game) mod state_at_viewpoint;
pub(in crate::game) mod stateful_block_entity;
pub(in crate::game) mod submit_custom_geometry;
pub(in crate::game) mod substitute_inline_images;
pub(in crate::game) mod supports_enchantment;
pub(in crate::game) mod sweep_attack;
pub(in crate::game) mod tags_updated;
pub(in crate::game) mod teleport_command;
pub(in crate::game) mod texture_atlas_stitched;
pub(in crate::game) mod tick_target_millis;
pub(in crate::game) mod toast_add;
pub(in crate::game) mod tool_modified_state;
pub(in crate::game) mod translate;
#[cfg(test)]
pub(in crate::game) mod trident_family_tests;
pub(in crate::game) mod use_item_hand;
pub(in crate::game) mod use_item_on_block;
pub(in crate::game) mod validate_tickets;
pub(in crate::game) mod vanilla_game_event;
pub(in crate::game) mod viewed_level_sound;
pub(in crate::game) mod wind_burst;
pub(in crate::game) mod window_resize;
pub(in crate::game) mod xp_orb_targeting;
pub(in crate::game) mod xp_repair_ratio;
#[allow(unused_imports)]
pub(in crate::game) use add_attribute_tooltips::HookedAttributeTooltips;
#[allow(unused_imports)]
pub(in crate::game) use base::LevelView;
#[allow(unused_imports)]
pub(in crate::game) use bounce_restitution::install_block_contact_hooks;
#[allow(unused_imports)]
pub(in crate::game) use living_change_target::NativeHookHost;
#[allow(unused_imports)]
use mod_api::ModBlockEntity;
#[allow(unused_imports)]
use sound_and_shader_helpers::{finite, finite_params, local_sound_ok};
#[allow(unused_imports)]
pub(in crate::game) use sound_and_shader_helpers::{level_sound_playable, mod_sound_ok, sound_id};
include!("shared.rs");

// Free-function associated native hooks re-exported at native_hooks::*
#[allow(unused_imports)]
pub(in crate::game) use add_section_geometry::{native_add_section_geometry};
#[allow(unused_imports)]
pub(in crate::game) use base::{native_block_entity_update_tag, native_blocks, native_component_types, native_items, native_menu_button, native_menu_quick_move, native_standalone_models};
#[allow(unused_imports)]
pub(in crate::game) use block_entity_client_tick::{native_block_entity_client_tick};
#[allow(unused_imports)]
pub(in crate::game) use block_entity_data_received::{native_block_entity_data_received};
#[allow(unused_imports)]
pub(in crate::game) use block_entity_on_load::{native_block_entity_on_load};
#[allow(unused_imports)]
pub(in crate::game) use block_entity_tick::{native_block_entity_tick};
#[allow(unused_imports)]
pub(in crate::game) use block_entity_type_add_blocks::{native_block_entity_type_add_blocks};
#[allow(unused_imports)]
pub(in crate::game) use can_place_item::{native_can_place_item};
#[allow(unused_imports)]
pub(in crate::game) use client_pick::{native_can_rider_interact};
#[allow(unused_imports)]
pub(in crate::game) use create_skull_models::{native_create_skull_models};
#[allow(unused_imports)]
pub(in crate::game) use extend_poi_types::{native_extend_poi_types};
#[allow(unused_imports)]
pub(in crate::game) use general_entity_tick::{native_general_entity_tick};
#[allow(unused_imports)]
pub(in crate::game) use map_decoration_types::{native_map_decoration_types};
#[allow(unused_imports)]
pub(in crate::game) use menu_slot_take::{native_menu_slot_take};
#[allow(unused_imports)]
pub(in crate::game) use on_entity_item_update::{native_on_entity_item_update};
#[allow(unused_imports)]
pub(in crate::game) use piston_push_reaction::{native_piston_push_reaction};
#[allow(unused_imports)]
pub(in crate::game) use register_block_state_models::{native_register_block_state_models};
#[allow(unused_imports)]
pub(in crate::game) use register_block_tint_sources::{native_register_block_tint_sources};
#[allow(unused_imports)]
pub(in crate::game) use register_cauldron_fluid_content::{native_register_cauldron_fluid_content};
#[allow(unused_imports)]
pub(in crate::game) use register_cauldron_interaction::{native_register_cauldron_interaction};
#[allow(unused_imports)]
pub(in crate::game) use register_client_item_extensions::{native_register_client_item_extensions};
#[allow(unused_imports)]
pub(in crate::game) use register_client_tooltip_component_factories::{native_register_client_tooltip_component_factories};
#[allow(unused_imports)]
pub(in crate::game) use register_color_resolvers::{native_register_color_resolvers};
#[allow(unused_imports)]
pub(in crate::game) use register_debug_entries::{native_register_debug_entries};
#[allow(unused_imports)]
pub(in crate::game) use register_debug_renderers::{native_register_debug_renderers};
#[allow(unused_imports)]
pub(in crate::game) use register_dimension_transition_screen::{native_register_dimension_transition_screen};
#[allow(unused_imports)]
pub(in crate::game) use register_enchantment_damage_effects::{native_register_enchantment_damage_effects};
#[allow(unused_imports)]
pub(in crate::game) use register_entity_layers::{native_register_entity_layers};
#[allow(unused_imports)]
pub(in crate::game) use register_entity_renderers::{native_register_entity_renderers};
#[allow(unused_imports)]
pub(in crate::game) use register_entity_spectator_shaders::{native_register_entity_spectator_shaders};
#[allow(unused_imports)]
pub(in crate::game) use register_fluid_models::{native_register_fluid_models};
#[allow(unused_imports)]
pub(in crate::game) use register_gui_layers::{native_register_gui_layers};
#[allow(unused_imports)]
pub(in crate::game) use register_item_models::{native_register_item_models};
#[allow(unused_imports)]
pub(in crate::game) use register_item_tint_sources::{native_register_item_tint_sources};
#[allow(unused_imports)]
pub(in crate::game) use register_json_animation_types::{native_register_json_animation_types};
#[allow(unused_imports)]
pub(in crate::game) use register_layer_definitions::{native_register_layer_definitions};
#[allow(unused_imports)]
pub(in crate::game) use register_load_conditions::{native_register_load_conditions};
#[allow(unused_imports)]
pub(in crate::game) use register_model_loaders::{native_register_model_loaders};
#[allow(unused_imports)]
pub(in crate::game) use register_particle_groups::{native_register_particle_groups};
#[allow(unused_imports)]
pub(in crate::game) use register_particle_providers::{native_register_particle_providers};
#[allow(unused_imports)]
pub(in crate::game) use register_preset_editors::{native_register_preset_editors};
#[allow(unused_imports)]
pub(in crate::game) use register_range_select_item_model_property::{native_register_range_select_item_model_property};
#[allow(unused_imports)]
pub(in crate::game) use register_render_pipelines::{native_register_render_pipelines};
#[allow(unused_imports)]
pub(in crate::game) use register_select_item_model_property::{native_register_select_item_model_property};
#[allow(unused_imports)]
pub(in crate::game) use register_special_model_renderer::{native_register_special_model_renderer};
#[allow(unused_imports)]
pub(in crate::game) use register_sprite_sources::{native_register_sprite_sources};
#[allow(unused_imports)]
pub(in crate::game) use register_texture_atlases::{native_register_texture_atlases};
#[allow(unused_imports)]
pub(in crate::game) use register_villager_professions::{native_register_villager_professions};
#[allow(unused_imports)]
pub(in crate::game) use block_entity_on_load::{authoritative_world_for_mods, world_for_mods};
#[allow(unused_imports)]
pub(in crate::game) use math::startup_math_profile;
#[allow(unused_imports)]
pub(in crate::game) use system_helpers::{mod_data_root, native_config_dir};
#[allow(unused_imports)]
pub(in crate::game) use base::{another_mods, startup_setting};
