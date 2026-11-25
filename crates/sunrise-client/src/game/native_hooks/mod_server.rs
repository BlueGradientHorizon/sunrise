#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::block_entity_tick::ResidentBlockEntityAccess;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::build_creative_mode_tab_contents::CreativeCache;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::client_pick::ClientPick;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::fire::{
    ConnectsToCall, EntityChangeBlockPreCall, HotbarAttackIndicatorProgressCall,
    ItemAfterVerticalCollisionCall, PlayerPongCall, PreRemoveSideEffectsCall, UpdateShapeCall,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::{BlockEntityDataSource, BlockEntityRef, ClockAdjustment};
#[allow(unused_imports)]
pub(in crate::game::server_side) use super::native_data_maps::DataMapType;
#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{
    Game, GameLevel, LevelDimension, Screen,
    execution_role::{self, ExecutionRole},
    mod_api_host,
    native_block_entity_draw::BlockEntityDraw,
    native_configs::ConfigFile,
    native_mob_draw::{MobRender, PlayerRender},
    native_panel_recipes::RecipePage,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::__private::game::{
    BlockStateDefinitionRegistration, ItemModelRegistration, ItemTintSourceRegistration,
    JsonAnimationTypeRegistration, LayerDefinitionRegistration, ModelLoaderRegistration,
    ParticleGroupRegistration, PresetEditorFactoryRegistration, RangeSelectPropertyRegistration,
    SelectPropertyRegistration, SpriteSourceRegistration, StandaloneModelRegistration,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::GameMode;
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::GameTests;
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::game::{PhantomSpawnResult, Shake, SleepProblem};
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::{
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
pub(in crate::game::server_side) use mod_api::{
    ActiveEffect, AttachmentStore, BlockClick, BlockUpdateFlags, BonemealSource, ChunkRef,
    FluidStack, ItemClick, LevelRef, LogicalSide, NeighbourUpdate, TooltipElement, TriState,
    UsePhase,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::{
    BlockKey, DebugEntry, DebugRenderer, EffectKey, FrameModel, FrameModelEdit, MapDecorationType,
    SkullModel, TextureAtlasRegistration,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::{
    BlockRim, ClientPacket, ClientPayload, ColoredLight, ContainerLootFilled, ExplosionStart,
    ItemFished, LootDrops, PanelEvent, PortalShape, ScreenView,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::{
    CameraType, Despawn, Identifier, MobRef, PermissionLevel, RailShape, SoundPlay, SoundRef,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::block_contact::Pending;
#[allow(unused_imports)]
#[cfg(test)]
pub(in crate::game::server_side) use sunrise_client::mobs::Damage;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
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
pub(in crate::game::server_side) use sunrise_client_core::gamerules::Value as GameRuleValue;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client_core::suggestion::Suggestion;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::{
    connection_kind::ConnectionKind,
    geometry::Direction,
    position::BlockPos,
    ticks::{ScheduleResult, TickPriority},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_world::level::{BonemealCheck, PlaceContext};
#[allow(unused_imports)]
pub(in crate::game::server_side) use winit::{event::MouseButton, keyboard::KeyCode};
#[allow(unused_imports)]
pub(in crate::game::server_side) use {
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
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::add_attribute_tooltips::HookedAttributeTooltips;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::base::LevelView;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::bounce_restitution::install_block_contact_hooks;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::living_change_target::NativeHookHost;
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::ModBlockEntity;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::sound_and_shader_helpers::{finite, finite_params, local_sound_ok};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::sound_and_shader_helpers::{level_sound_playable, mod_sound_ok, sound_id};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::add_section_geometry::native_add_section_geometry;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::base::{another_mods, startup_setting};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::base::{
    native_block_entity_update_tag, native_blocks, native_component_types, native_items,
    native_menu_button, native_menu_quick_move, native_standalone_models,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::block_entity_client_tick::native_block_entity_client_tick;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::block_entity_data_received::native_block_entity_data_received;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::block_entity_on_load::native_block_entity_on_load;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::block_entity_on_load::{authoritative_world_for_mods, world_for_mods};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::block_entity_tick::native_block_entity_tick;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::block_entity_type_add_blocks::native_block_entity_type_add_blocks;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::can_place_item::native_can_place_item;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::client_pick::native_can_rider_interact;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::create_skull_models::native_create_skull_models;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::extend_poi_types::native_extend_poi_types;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::general_entity_tick::native_general_entity_tick;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::map_decoration_types::native_map_decoration_types;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::math::startup_math_profile;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::menu_slot_take::native_menu_slot_take;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::on_entity_item_update::native_on_entity_item_update;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::piston_push_reaction::native_piston_push_reaction;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_block_state_models::native_register_block_state_models;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_block_tint_sources::native_register_block_tint_sources;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_cauldron_fluid_content::native_register_cauldron_fluid_content;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_cauldron_interaction::native_register_cauldron_interaction;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_client_item_extensions::native_register_client_item_extensions;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_client_tooltip_component_factories::native_register_client_tooltip_component_factories;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_color_resolvers::native_register_color_resolvers;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_debug_entries::native_register_debug_entries;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_debug_renderers::native_register_debug_renderers;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_dimension_transition_screen::native_register_dimension_transition_screen;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_enchantment_damage_effects::native_register_enchantment_damage_effects;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_entity_layers::native_register_entity_layers;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_entity_renderers::native_register_entity_renderers;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_entity_spectator_shaders::native_register_entity_spectator_shaders;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_fluid_models::native_register_fluid_models;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_gui_layers::native_register_gui_layers;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_item_models::native_register_item_models;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_item_tint_sources::native_register_item_tint_sources;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_json_animation_types::native_register_json_animation_types;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_layer_definitions::native_register_layer_definitions;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_load_conditions::native_register_load_conditions;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_model_loaders::native_register_model_loaders;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_particle_groups::native_register_particle_groups;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_particle_providers::native_register_particle_providers;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_preset_editors::native_register_preset_editors;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_range_select_item_model_property::native_register_range_select_item_model_property;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_render_pipelines::native_register_render_pipelines;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_select_item_model_property::native_register_select_item_model_property;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_special_model_renderer::native_register_special_model_renderer;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_sprite_sources::native_register_sprite_sources;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_texture_atlases::native_register_texture_atlases;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_villager_professions::native_register_villager_professions;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::system_helpers::{mod_data_root, native_config_dir};
#[allow(unused_imports)]
#[cfg(test)]
pub(in crate::game::server_side) use crate::game::native_hooks::level_helpers::block_entity_for_block;
#[allow(unused_imports)]
pub(in crate::game::server_side) use super::screen_build::ObservedScreen;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::api_values::{TryIntoApi, TryIntoEngine};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::screen_chars::chars_to_type;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::containers::ModBlockEntity as EngineBlockEntity;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::screen_view::{screen_view_for_mod, screen_view_with_id};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_world::level::flags;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::camera_pose::bounded_shake;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::camera_pose::hand_pose_matrix;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::living_death::hurt_server_args;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::health::mob_source;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::ContentsChangedGuard;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::apply_menu_button_cursor;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::ScreenDecision;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::title_target_id;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::observed_overlay_id;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::SHAKE_MAX_DEGREES;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::SHAKE_MAX_OFFSET;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::shake_pose;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::OpenNativeScreen;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::OpenNativeMenu;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::seed_native_menu;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::SlotMoves;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::NativePlayer;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::NativeRecipeLayout;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::NativeRecipeResult;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::NativeRecipeView;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::NativeRecipeStack;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::NativeRecipeKind;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::NativeRecipe;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::NativeItem;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::NativeRecipes;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::ScreenMousePostKind;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::ScreenMousePost;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::ScreenMousePre;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::use_totem;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::HitLevel;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::invulnerability_check;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::engine_reduction;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::LiveDamageRead;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::LiveDamageContainer;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::incoming_damage;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::damage_pre;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::damage_post;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::shield_block;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::knockback;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::DAMAGE_HOOKS;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::BLOCK_CONTACT_HOOKS;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::mod_particles_ok;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::dimension_id;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::mob_kind;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::block_use_result;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::BlockClickAnswer;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::BonemealAsk;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::debug_takes;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::mod_menu;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::with_property;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::property_change;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::config_dir;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::NativeItemEntity;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::box_meets;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::mod_screen_opens;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::client_packet_view;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::serverbound_packet_view;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::packet_types;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::packet_view;

#[path = "base_server.rs"]
pub(in crate::game) mod base;

#[path = "block_entity_on_load_server.rs"]
pub(in crate::game) mod block_entity_on_load;

#[path = "block_entity_save_additional_tests_server.rs"]
pub(in crate::game) mod block_entity_save_additional_tests;

#[path = "block_entity_tick_server.rs"]
pub(in crate::game) mod block_entity_tick;

#[path = "build_creative_mode_tab_contents_server.rs"]
pub(in crate::game) mod build_creative_mode_tab_contents;

#[path = "can_start_swimming_server.rs"]
pub(in crate::game) mod can_start_swimming;

#[path = "client_information_updated_server.rs"]
pub(in crate::game) mod client_information_updated;

#[path = "contents_changed_server.rs"]
pub(in crate::game) mod contents_changed;

#[path = "damage_item_server.rs"]
pub(in crate::game) mod damage_item;

#[path = "enchanted_block_loot_server.rs"]
pub(in crate::game) mod enchanted_block_loot;

#[path = "entity_inside_server.rs"]
pub(in crate::game) mod entity_inside;

#[path = "exp_drop_server.rs"]
pub(in crate::game) mod exp_drop;

#[path = "explosion_notice_containment_server.rs"]
pub(in crate::game) mod explosion_notice_containment;

#[path = "farmland_trample_server.rs"]
pub(in crate::game) mod farmland_trample;

#[path = "fire_behaviour_server.rs"]
pub(in crate::game) mod fire_behaviour;

#[path = "general_entity_tick_server.rs"]
pub(in crate::game) mod general_entity_tick;

#[path = "incoming_tests_server.rs"]
pub(in crate::game) mod incoming_tests;

#[path = "item_attack_server.rs"]
pub(in crate::game) mod item_attack;

#[path = "item_picked_up_server.rs"]
pub(in crate::game) mod item_picked_up;

#[path = "item_toss_server.rs"]
pub(in crate::game) mod item_toss;

#[path = "level_world_server.rs"]
pub(in crate::game) mod level_world;

#[path = "living_breathe_server.rs"]
pub(in crate::game) mod living_breathe;

#[path = "living_death_server.rs"]
pub(in crate::game) mod living_death;

#[path = "living_death_post_server.rs"]
pub(in crate::game) mod living_death_post;

#[path = "mod_mismatch_server.rs"]
pub(in crate::game) mod mod_mismatch;

#[path = "on_chunk_unloaded_server.rs"]
pub(in crate::game) mod on_chunk_unloaded;

#[path = "on_entity_item_update_server.rs"]
pub(in crate::game) mod on_entity_item_update;

#[path = "player_bed_leave_server.rs"]
pub(in crate::game) mod player_bed_leave;

#[path = "player_leave_server.rs"]
pub(in crate::game) mod player_leave;

#[path = "register_enchantment_damage_effects_server.rs"]
pub(in crate::game) mod register_enchantment_damage_effects;

#[path = "remove_block_server.rs"]
pub(in crate::game) mod remove_block;

#[path = "server_tick_rate_changed_server.rs"]
pub(in crate::game) mod server_tick_rate_changed;

#[path = "shared/item_fallthrough_server.rs"]
pub(in crate::game) mod item_fallthrough;

#[path = "shared/loot_server.rs"]
pub(in crate::game) mod loot;

impl Drop for ContentsChangedGuard<'_, '_> {
    fn drop(&mut self) {
        self.game.server.contents_changing.pop();
    }
}

/// The hook points. Each does nothing in Sunrise.
impl Game<'_> {
    /// `AnimalTameEvent` for the mob at `index`, whose taming roll by the
    /// player `tamer` (`tamer_entity_id`) succeeded: true when a mod cancels
    /// it, and the taming fails.
    pub(in crate::game) fn animal_tame_cancelled(
        &mut self,
        index: usize,
        tamer: u128,
        tamer_entity_id: i32,
    ) -> bool {
        if !mod_api::__private::listened(mod_api::HookId::AnimalTame) {
            return false;
        }
        let m = &self.mobs[index];
        let (entity, kind) = (m.id, mob_kind(m));
        let mut cancel = false;
        self.native_animal_tame(tamer, tamer_entity_id, entity, &kind, &mut cancel);
        cancel
    }
    /// The [`Attack`] of `attacker` (an entity id, 0 for none) for the level a hit lands in
    /// ([`DamageLevel::attacked_by`]). While a native mod listens it also records what the
    /// attacker holds (the player's selected and off-hand stacks, a mob's main hand) for
    /// `living_knockback` and the weapon of `living_incoming_damage`; without one it is the
    /// id alone, two `None`s and no allocation. Taken before the level's borrows of the game.
    pub(in crate::game) fn attack_of(&self, attacker: i32) -> Attack {
        let held = if mod_api::__private::mods().is_empty() {
            [None, None]
        } else if self
            .player
            .as_ref()
            .is_some_and(|p| p.entity_id == attacker)
        {
            [self.stack_in(self.selected).cloned(), self.offhand.clone()]
        } else if let Some(mob) = self.mobs.iter().find(|m| m.id as i32 == attacker) {
            [mob.extra.main_hand_stack().cloned(), None]
        } else if let Some(guest) = self
            .lan
            .as_ref()
            .and_then(|host| host.hub.guests.iter().find(|g| g.entity_id == attacker))
        {
            [
                guest.inventory.get(guest.selected).cloned().flatten(),
                guest.offhand.clone(),
            ]
        } else {
            [None, None]
        };
        let player_uuid = if self
            .player
            .as_ref()
            .is_some_and(|p| p.entity_id == attacker)
        {
            self.player_uuid_of(self.server.lan_acting)
        } else {
            self.lan
                .as_ref()
                .and_then(|host| host.hub.guests.iter().find(|g| g.entity_id == attacker))
                .map(|g| sunrise_client::lan::playerdata::uuid_ints(g.uuid))
        };
        Attack {
            attacker,
            held,
            projectile: None,
            projectile_kind: sunrise_client::health::DirectEntity::Other,
            projectile_type: None,
            direct: None,
            direct_kind: None,
            direct_type: None,
            position: None,
            projectile_weapon: None,
            attacker_uuid: (player_uuid.is_none() && attacker != 0)
                .then(|| self.known_attacker_uuid(attacker as u32))
                .flatten(),
            player_uuid,
        }
    }
    /// Finish a lethal or totem-protected hit only once the owning level has settled it.
    pub(in crate::game::server_side) fn finish_damage_post(
        &mut self,
        entity: i32,
        kind: &str,
        post: std::sync::Arc<sunrise_client::health::DamagePost>,
    ) {
        use sunrise_client::health::{HitLend, LivingBody, PlayerBody, PlayerHealthRead};
        let (source, state, original_lend) = post.view();
        // The victim as the death or protection callback left it, beside the lent attacker:
        // the player in place, or a mob split from the attacker in the same list.
        let victim_mob = self.mobs.iter().position(|m| m.id as i32 == entity);
        // The Post record exists only for a Post subscriber, and names an attacker only if the hit
        // lent one: it carries the setup's decision.
        let lend_mob = original_lend
            .and_then(|id| self.mobs.iter().position(|m| m.id as i32 == id))
            .filter(|&index| Some(index) != victim_mob);
        // A parked guest the hit lent, lent again if its reads still answer from its record.
        let parked = original_lend
            .filter(|_| lend_mob.is_none())
            .and_then(|id| self.native_parked_causer(id));
        let (victim_mob, lend_mob) = match (victim_mob, lend_mob) {
            (Some(v), Some(l)) => {
                let [v, l] = self
                    .mobs
                    .get_disjoint_mut([v, l])
                    .expect("two different mobs");
                (Some(&*v), Some(l))
            }
            (None, Some(l)) => (None, Some(&mut self.mobs[l])),
            (Some(v), None) => (Some(&self.mobs[v]), None),
            (None, None) => (None, None),
        };
        let player_victim = self.player.as_ref().is_some_and(|p| p.entity_id == entity);
        let hub = self.lan.as_ref().map(|lan| &lan.hub);
        let parked = move || HitLend::parked_guest(hub, parked?);
        let (victim, lend) = if player_victim {
            let p = self.player.as_ref().expect("the player victim");
            let body = LivingBody::Player(PlayerBody {
                id: p.entity_id,
                uuid: None,
                health: PlayerHealthRead::Synced(&p.health.data, p.health.ids),
                attributes: Some(&p.attributes),
                effects: Some(&self.potions.effects),
                equipment: acting_equipment!(self),
                attachments: None,
            });
            (Some(body), lend_mob.and_then(HitLend::mob).or_else(parked))
        } else {
            let lend = match lend_mob {
                Some(mob) => HitLend::mob(mob),
                None => original_lend
                    .and_then(|id| {
                        HitLend::attacking_player(
                            self.player.as_mut(),
                            id,
                            &self.potions.effects,
                            acting_equipment!(self),
                        )
                    })
                    .or_else(parked),
            };
            (victim_mob.map(LivingBody::Mob), lend)
        };
        let level = crate::game::entity::damage_level_of!(
            self,
            &sunrise_client::health::NO_ATTACK,
            no_push,
            lend
        );
        let level = match victim {
            Some(victim) => level.reborrow().with_victim(victim),
            None => level.reborrow(),
        };
        let live = LiveDamageRead(state);
        HitLevel(level.caused_by(source.causing)).native_living_damage_post(
            entity,
            kind,
            &source,
            mod_api::DamageContainerView::new(&live),
        );
    }
    /// `ItemStack.onItemUseFirst` for a click on a block: the broadcast
    /// event's `ItemBeforeBlock` phase, then the held item's Part. Canceling skips that Part.
    /// Only `Pass` continues to the block.
    pub(in crate::game::server_side) fn item_use_first(
        &mut self,
        hit: &super::Hit,
        hand: Hand,
        stack: &ItemStack,
    ) -> Option<mod_api::InteractionResult> {
        if let Some(answer) =
            self.use_item_on_block(hit, hand, Some(stack), UsePhase::ItemBeforeBlock)
        {
            return Some(answer);
        }
        if !mod_api::__private::has_part_hook(mod_api::HookId::OnItemUseFirst) {
            return None;
        }
        let snapshot = stack.try_into_api().ok()?;
        let click = self.block_click(hit, hand, Some(&snapshot));
        let mut answer = None;
        self.native_on_item_use_first(&snapshot, &click, &mut answer);
        answer.filter(|answer| *answer != mod_api::InteractionResult::Pass)
    }
    /// `MobEvent::PlaceBlock`: a mob sets `state` at `pos` unless a mod
    /// cancels `entity_place`. Enderman attempts use `enderman_place_block`.
    pub(in crate::game) fn mob_place_block(
        &mut self,
        mob: u32,
        pos: [i32; 3],
        state: BlockStateId,
    ) -> crate::Result<()> {
        let at = BlockPos::new(pos[0], pos[1], pos[2]);
        let against = self.world.block(pos[0], pos[1] - 1, pos[2]);
        let kind = self
            .mobs
            .iter()
            .find(|m| m.id == mob)
            .map(|m| m.kind.id.clone());
        if self.place_canceled(mob, at, state, against, None) {
            return Ok(());
        }
        let facts = self.res.facts();
        let from = facts
            .get(self.world.block(pos[0], pos[1], pos[2]))
            .name
            .clone();
        let to = facts.get(state).name.clone();
        self.level_set(pos, state)?;
        if let Some(kind) = kind {
            self.native_entity_change_block(mob, &kind, pos, &from, &to);
        }
        Ok(())
    }
    /// Menu button `id` on a mod menu (`clickMenuButton`), with the acting
    /// player's cursor stack (`carried`, read and written). A block-backed
    /// menu lends its live resident block entity through `BlockEntityScope`;
    /// a blockless menu lends no view while keeping `host()` and
    /// `transaction(...)` usable.
    pub(in crate::game) fn mod_menu_button(
        &mut self,
        menu: &str,
        context: mod_api::item::menu::MenuButtonContext,
        block: Option<&str>,
        backing: Option<(i32, i32, i32)>,
        carried: &mut Option<ItemStack>,
    ) {
        if !mod_api::__private::has_part_hook(mod_api::HookId::MenuButton) {
            return;
        }
        let Ok(mut api_cursor) = carried.try_into_api() else {
            return;
        };
        let original_api = api_cursor.clone();
        let target_pos = backing.filter(|&pos| self.mod_menu_at(pos) == Some(menu));
        let target = target_pos.and_then(|pos| self.resident_block_entity_target(pos));
        let items_changed = {
            let mut access = match target {
                Some((handle, state)) => ResidentBlockEntityAccess::for_handle(self, handle, state),
                None => ResidentBlockEntityAccess::blockless(self),
            };
            crate::game::native_hooks::native_menu_button(
                menu,
                block,
                &context,
                mod_api::BlockEntityScope::new(&mut access),
                &mut api_cursor,
            );
            access.items_changed()
        };
        if items_changed
            && let Some((handle, _)) = target
            && let Some(pos) = self.native_block_entity_write_key(handle)
        {
            self.after_commit(crate::game::transactions::After::Changed(pos));
        }
        apply_menu_button_cursor(self.items(), &original_api, &api_cursor, carried);
    }
    /// The slot a player's carried stack is in, by `native_inventory_tick`'s
    /// numbers: none for another number.
    pub(in crate::game) fn native_carried_slot(
        &mut self,
        slot: i32,
    ) -> Option<&mut Option<ItemStack>> {
        match slot {
            0.. if (slot as usize) < self.inventory.len() => self.inventory.get_mut(slot as usize),
            100..=103 => self.armor.get_mut(103 - slot as usize),
            -106 => Some(&mut self.offhand),
            _ => None,
        }
    }
    /// The player clicking, as the block-use hooks name them: the acting
    /// player's UUID and entity id on a server, this client's own player on
    /// a client.
    pub(in crate::game) fn native_clicker(&mut self) -> (u128, i32) {
        if !self.is_logical_server() {
            return (self.own_uuid(), self.own_entity_id());
        }
        let entity = self
            .server
            .lan_acting_entity
            .map_or_else(|| self.own_entity_id(), |id| id as i32);
        (self.native_acting_id(), entity)
    }
    /// The containers of the mod blocks' block entities, by block state
    /// (`native_block_entity_container`), checked.
    pub(in crate::game) fn native_containers(&self) -> FxHashMap<u32, Arc<ModContainer>> {
        let registry = self.pack.assets.registry;
        let mut by_block: FxHashMap<String, Option<Arc<ModContainer>>> = FxHashMap::default();
        let mut out = FxHashMap::default();
        for i in 0..registry.len() as u32 {
            let Some(state) = registry.state(sunrise_world::registry::BlockStateId(i)) else {
                continue;
            };
            if state.name.starts_with("minecraft:") {
                continue;
            }
            let container = by_block.entry(state.name.clone()).or_insert_with(|| {
                let mut container = None;
                self.native_block_entity_container(&state.name, &mut container);
                container.map(|mut c| {
                    let n = c.slots;
                    for list in c.faces.iter_mut().chain([&mut c.inputs, &mut c.outputs]) {
                        list.retain(|&s| s < n);
                    }
                    c.insert_rules.truncate(n);
                    c.menu = c
                        .menu
                        .filter(|m| mod_menu(m).is_some_and(|(_, size)| size == n));
                    Arc::new(c)
                })
            });
            if let Some(c) = container {
                out.insert(i, Arc::clone(c));
            }
        }
        out
    }
    /// The energy buffers of the mod blocks' block entities, by block state
    /// (`native_block_entity_energy`), checked as the containers are.
    pub(in crate::game) fn native_energy_tables(&self) -> FxHashMap<u32, Arc<ModEnergyBuffer>> {
        let registry = self.pack.assets.registry;
        let mut by_block: FxHashMap<String, Option<Arc<ModEnergyBuffer>>> = FxHashMap::default();
        let mut out = FxHashMap::default();
        for i in 0..registry.len() as u32 {
            let Some(state) = registry.state(sunrise_world::registry::BlockStateId(i)) else {
                continue;
            };
            if state.name.starts_with("minecraft:") {
                continue;
            }
            let buffer = by_block.entry(state.name.clone()).or_insert_with(|| {
                let mut energy = None;
                self.native_block_entity_energy(&state.name, &mut energy);
                energy.filter(|e| e.capacity >= 1).map(|mut e| {
                    e.max_insert = e.max_insert.clamp(0, e.capacity);
                    e.max_extract = e.max_extract.clamp(0, e.capacity);
                    Arc::new(e)
                })
            });
            if let Some(b) = buffer {
                out.insert(i, Arc::clone(b));
            }
        }
        out
    }
    /// The fluid tanks of the mod blocks' block entities, by block state
    /// (`native_block_entity_fluids`), checked as the containers are.
    pub(in crate::game) fn native_fluid_tables(&self) -> FxHashMap<u32, Arc<ModFluidTanks>> {
        let registry = self.pack.assets.registry;
        let mut by_block: FxHashMap<String, Option<Arc<ModFluidTanks>>> = FxHashMap::default();
        let mut out = FxHashMap::default();
        for i in 0..registry.len() as u32 {
            let Some(state) = registry.state(sunrise_world::registry::BlockStateId(i)) else {
                continue;
            };
            if state.name.starts_with("minecraft:") {
                continue;
            }
            let tanks = by_block.entry(state.name.clone()).or_insert_with(|| {
                let mut fluids = None;
                self.native_block_entity_fluids(&state.name, &mut fluids);
                fluids.map(|mut f| {
                    f.tanks.truncate(16);
                    // Clamped, not dropped: the face and fill lists name
                    // tanks by number, which must not shift.
                    for c in &mut f.tanks {
                        *c = (*c).clamp(1, 1_000_000);
                    }
                    let n = f.tanks.len();
                    for list in f.faces.iter_mut().chain([&mut f.inputs, &mut f.outputs]) {
                        list.retain(|&t| t < n);
                    }
                    Arc::new(f)
                })
            });
            if let Some(t) = tanks {
                out.insert(i, Arc::clone(t));
            }
        }
        out
    }
    /// The acting player's hit landed on mob `index`, holding `stack`.
    pub(in crate::game) fn native_hurt_enemy_event(&mut self, stack: &ItemStack, index: usize) {
        let mob = &self.mobs[index];
        let kind = ["minecraft:", mob.kind.id.trim_start_matches("minecraft:")].concat();
        self.native_hurt_enemy(stack, mob.id, &kind);
    }
    /// `Inventory.tick` for the acting player: each carried stack goes through
    /// `native_inventory_tick`.
    pub(in crate::game) fn native_inventory_ticks(&mut self) {
        if self.player.is_none() {
            return;
        }
        for i in 0..self.inventory.len() {
            self.native_slot_tick(i as i32, i == self.selected);
        }
        // Head first here; `Inventory` saves feet as 100 and head as 103.
        for i in 0..self.armor.len() {
            self.native_slot_tick(103 - i as i32, false);
        }
        self.native_slot_tick(-106, false);
    }
    pub(in crate::game) fn native_slot_tick(&mut self, slot: i32, selected: bool) {
        if !mod_api::__private::has_part_hook(mod_api::HookId::InventoryTick) {
            return;
        }
        let Some(stack) = self.native_carried_slot(slot).and_then(|s| s.clone()) else {
            return;
        };
        self.native_inventory_tick(&stack, slot, selected);
        if let Some(stack) = self.native_carried_slot(slot)
            && stack.as_ref().is_some_and(|s| s.count == 0)
        {
            *stack = None;
        }
    }
    /// Whether a native mod's block `name` has a ticking block entity.
    pub(in crate::game) fn native_ticks(&self, name: &str) -> bool {
        let mut ticks = false;
        self.native_block_entity_ticks(name, &mut ticks);
        ticks
    }
    /// Runs `change` on the mod block entity at `pos` (a generator charging
    /// the machine beside it) and returns what it returns; none for any
    /// other block entity. Afterwards, as its `setChanged` would, its chunk is saved next
    /// time and, when its items changed, comparators read it again.
    #[allow(dead_code)]
    pub(in crate::game) fn native_with_mod_block_entity<R>(
        &mut self,
        pos: [i32; 3],
        change: impl FnOnce(&mut EngineBlockEntity) -> R,
    ) -> Option<R> {
        let key = (pos[0], pos[1], pos[2]);
        let Some(BlockEntity::Mod(m)) = self.world.block_entities.get_mut(&key) else {
            return None;
        };
        let items = m.items.clone();
        let out = change(m);
        let moved = m.items != items;
        self.world.modified.insert((key.0 >> 4, key.2 >> 4));
        if moved {
            self.container_changed(key);
        }
        Some(out)
    }
    /// `entity_tame` for the pet at `index`, just tamed by its owner.
    pub(in crate::game) fn pet_tamed(&mut self, index: usize) {
        let m = &self.mobs[index];
        let (entity, kind) = (m.id, mob_kind(m));
        let owner = m
            .extra
            .owner
            .map(sunrise_client::lan::playerdata::ints_uuid);
        self.native_entity_tame(entity, &kind, owner);
    }
    /// Whether a mod canceled `placer` writing `state` at `pos` (`entity_place`):
    /// `placer` is an entity id, `placed_against` the block it goes against and
    /// `companion` the second half of a bed, door or tall plant.
    pub(in crate::game) fn place_canceled(
        &mut self,
        placer: u32,
        pos: BlockPos,
        state: BlockStateId,
        placed_against: BlockStateId,
        companion: Option<(BlockPos, BlockStateId)>,
    ) -> bool {
        if !mod_api::__private::listened(mod_api::HookId::EntityPlace) {
            return false;
        }
        self.placement_canceled(self.entity(placer), pos, state, placed_against, companion)
    }
}

#[cfg(test)]
#[path = "block_entity_neighbors_tests_server.rs"]
pub(in crate::game) mod block_entity_neighbors_tests;

#[cfg(test)]
#[path = "death_source_tests_server.rs"]
pub(in crate::game) mod death_source_tests;

#[cfg(test)]
#[path = "on_block_placed_by_tests_server.rs"]
pub(in crate::game) mod on_block_placed_by_tests;

#[cfg(test)]
#[path = "piston_push_reaction_tests_server.rs"]
pub(in crate::game) mod piston_push_reaction_tests;

#[cfg(test)]
#[path = "placement_context_tests_server.rs"]
pub(in crate::game) mod placement_context_tests;

#[cfg(test)]
#[path = "trident_family_tests_server.rs"]
pub(in crate::game) mod trident_family_tests;

#[cfg(test)]
#[path = "shared/item_fall_on_tests_server.rs"]
pub(in crate::game) mod item_fall_on_tests;
