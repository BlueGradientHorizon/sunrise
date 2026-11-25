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
pub(in crate::game::server_side) use crate::game::native_hooks::add_section_geometry::{native_add_section_geometry};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::base::{native_block_entity_update_tag, native_blocks, native_component_types, native_items, native_menu_button, native_menu_quick_move, native_standalone_models};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::block_entity_client_tick::{native_block_entity_client_tick};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::block_entity_data_received::{native_block_entity_data_received};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::block_entity_on_load::{native_block_entity_on_load};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::block_entity_tick::{native_block_entity_tick};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::block_entity_type_add_blocks::{native_block_entity_type_add_blocks};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::can_place_item::{native_can_place_item};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::client_pick::{native_can_rider_interact};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::create_skull_models::{native_create_skull_models};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::extend_poi_types::{native_extend_poi_types};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::general_entity_tick::{native_general_entity_tick};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::map_decoration_types::{native_map_decoration_types};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::menu_slot_take::{native_menu_slot_take};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::on_entity_item_update::{native_on_entity_item_update};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::piston_push_reaction::{native_piston_push_reaction};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_block_state_models::{native_register_block_state_models};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_block_tint_sources::{native_register_block_tint_sources};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_cauldron_fluid_content::{native_register_cauldron_fluid_content};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_cauldron_interaction::{native_register_cauldron_interaction};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_client_item_extensions::{native_register_client_item_extensions};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_client_tooltip_component_factories::{native_register_client_tooltip_component_factories};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_color_resolvers::{native_register_color_resolvers};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_debug_entries::{native_register_debug_entries};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_debug_renderers::{native_register_debug_renderers};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_dimension_transition_screen::{native_register_dimension_transition_screen};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_enchantment_damage_effects::{native_register_enchantment_damage_effects};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_entity_layers::{native_register_entity_layers};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_entity_renderers::{native_register_entity_renderers};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_entity_spectator_shaders::{native_register_entity_spectator_shaders};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_fluid_models::{native_register_fluid_models};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_gui_layers::{native_register_gui_layers};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_item_models::{native_register_item_models};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_item_tint_sources::{native_register_item_tint_sources};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_json_animation_types::{native_register_json_animation_types};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_layer_definitions::{native_register_layer_definitions};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_load_conditions::{native_register_load_conditions};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_model_loaders::{native_register_model_loaders};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_particle_groups::{native_register_particle_groups};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_particle_providers::{native_register_particle_providers};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_preset_editors::{native_register_preset_editors};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_range_select_item_model_property::{native_register_range_select_item_model_property};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_render_pipelines::{native_register_render_pipelines};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_select_item_model_property::{native_register_select_item_model_property};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_special_model_renderer::{native_register_special_model_renderer};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_sprite_sources::{native_register_sprite_sources};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_texture_atlases::{native_register_texture_atlases};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::register_villager_professions::{native_register_villager_professions};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::block_entity_on_load::{authoritative_world_for_mods, world_for_mods};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::math::startup_math_profile;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::system_helpers::{mod_data_root, native_config_dir};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::base::{another_mods, startup_setting};
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
pub(in crate::game::server_side) use crate::game::native_hooks::posed_camera;
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

#[path = "block_entity_save_additional_tests_server.rs"]
pub(in crate::game) mod block_entity_save_additional_tests;

#[path = "block_entity_tick_server.rs"]
pub(in crate::game) mod block_entity_tick;

#[path = "build_creative_mode_tab_contents_server.rs"]
pub(in crate::game) mod build_creative_mode_tab_contents;

#[path = "can_start_swimming_server.rs"]
pub(in crate::game) mod can_start_swimming;

#[path = "contents_changed_server.rs"]
pub(in crate::game) mod contents_changed;

#[path = "enchanted_block_loot_server.rs"]
pub(in crate::game) mod enchanted_block_loot;

#[path = "entity_inside_server.rs"]
pub(in crate::game) mod entity_inside;

#[path = "farmland_trample_server.rs"]
pub(in crate::game) mod farmland_trample;

#[path = "general_entity_tick_server.rs"]
pub(in crate::game) mod general_entity_tick;

#[path = "incoming_tests_server.rs"]
pub(in crate::game) mod incoming_tests;

#[path = "item_toss_server.rs"]
pub(in crate::game) mod item_toss;

#[path = "level_world_server.rs"]
pub(in crate::game) mod level_world;

#[path = "living_death_server.rs"]
pub(in crate::game) mod living_death;

#[path = "living_death_post_server.rs"]
pub(in crate::game) mod living_death_post;

#[path = "mod_mismatch_server.rs"]
pub(in crate::game) mod mod_mismatch;

#[path = "on_entity_item_update_server.rs"]
pub(in crate::game) mod on_entity_item_update;

#[path = "player_bed_leave_server.rs"]
pub(in crate::game) mod player_bed_leave;

#[path = "player_leave_server.rs"]
pub(in crate::game) mod player_leave;

#[path = "register_enchantment_damage_effects_server.rs"]
pub(in crate::game) mod register_enchantment_damage_effects;

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

impl Game<'_> {
    /// A write an `entity_inside` part asked for, applied as a landing write is: at the
    /// touched block, once the entity's batch has ticked.
    pub(in crate::game) fn apply_inside_write(
        &mut self,
        level: mod_api::LevelRef,
        pos: BlockPos,
        write: sunrise_world::level::InsideWrite,
    ) {
        let at = [pos.x, pos.y, pos.z];
        match write {
            sunrise_world::level::InsideWrite::SetBlock(state) => {
                let flags = BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS;
                let _ = self.level_set_block(level, at, state, flags);
            }
            sunrise_world::level::InsideWrite::DestroyBlock { drop } => {
                self.level_destroy_block(level, at, drop, None, false);
            }
            // A write a later API adds that this game does not know is not applied.
            _ => {}
        }
    }
    /// Apply one already-validated engine write, at the block the entity landed on.
    /// The original-state guard runs once before the ordered batch, not between writes.
    pub(in crate::game) fn apply_landing_write(
        &mut self,
        level: mod_api::LevelRef,
        pos: BlockPos,
        write: sunrise_world::level::LandingWrite,
    ) {
        let at = [pos.x, pos.y, pos.z];
        match write {
            // `Level.setBlockAndUpdate`: flag 3, the neighbours and the clients.
            sunrise_world::level::LandingWrite::SetBlock(state) => {
                let flags = BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS;
                let _ = self.level_set_block(level, at, state, flags);
            }
            sunrise_world::level::LandingWrite::DestroyBlock { drop } => {
                self.level_destroy_block(level, at, drop, None, false);
            }
            // A write a later API adds that this game does not know is not applied.
            _ => {}
        }
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
    /// [`Self::attack_of`] for the attacker a damage source names: the player, a mob, or (none)
    /// the attack the enclosing [`Self::with_attack`] lends.
    pub(in crate::game) fn attack_of_who(&self, attacker: Option<Who>) -> Attack {
        match attacker {
            Some(Who::Player) => self.attack_of(self.player.as_ref().map_or(0, |p| p.entity_id)),
            Some(Who::Mob(id)) => self.attack_of(id as i32),
            None => self.current_attack(),
        }
    }
    /// `BonemealEvent` for bone meal from `stack` on `pos`, used by the
    /// acting player (`by_player`) or by a dispenser or a villager:
    /// `Some(successful)` when a mod canceled it, the answer `growCrop`
    /// gives in vanilla's place. The caller keeps what the mods did to
    /// `stack`.
    pub(in crate::game) fn bonemeal(
        &mut self,
        pos: BlockPos,
        by_player: bool,
        stack: &mut ItemStack,
    ) -> BonemealAsk {
        if !mod_api::__private::listened(mod_api::HookId::Bonemeal) {
            return BonemealAsk {
                crop: None,
                check: None,
            };
        }
        let client = self.remote.is_some();
        let check = self.with_level(|b, level, _| b.is_valid_bonemeal_target(level, pos, client));
        let valid = check.is_valid();
        let state = self.world.block(pos.x, pos.y, pos.z);
        let (player, player_entity_id) = if by_player {
            let (uuid, entity) = self.native_clicker();
            (Some(uuid), Some(entity))
        } else {
            (None, None)
        };
        let (mut successful, mut canceled) = (false, false);
        let mut snapshot = Some(stack.clone());
        self.native_bonemeal(crate::game::native_hooks::fire::BonemealCall {
            dimension: self.dim.level(),
            player,
            player_entity_id,
            pos,
            state,
            source: BonemealSource::Interaction,
            stack: &mut snapshot,
            valid_bonemeal_target: valid,
            successful: &mut successful,
            canceled: &mut canceled,
        });
        match snapshot {
            Some(next) => *stack = next,
            None => stack.count = 0,
        }
        BonemealAsk {
            crop: canceled.then_some(successful),
            check: Some(check),
        }
    }
    /// Deliver what the batch just ticked recorded, in order: `minecart_pass`
    /// to the parts, and the writes a landing, step, or mid-walk `entity_inside`
    /// contact asked for to the level. A write is skipped when the block at
    /// its position is no longer the state it touched, as vanilla never sees
    /// a stale one. The parts' own `entity_inside` contacts fire mid-walk now,
    /// on the shared inside sweep, not here.
    pub(in crate::game) fn drain_entity_inside(&mut self) {
        for contact in sunrise_client::block_contact::take_pending() {
            let (state, pos) = match &contact {
                Pending::Landed(landed) => (landed.state, landed.position),
                Pending::MinecartPass(pass) => (pass.state, pass.position),
                Pending::InsideWritten(written) => (written.state, written.position),
            };
            let Some(level) = self.level(self.dim.id()) else {
                continue;
            };
            if self.level_block_state(level, [pos.x, pos.y, pos.z]) != Some(state) {
                continue;
            }
            match contact {
                Pending::MinecartPass(pass) => {
                    if let Some(entity) = u32::try_from(pass.entity)
                        .ok()
                        .and_then(|id| self.entity(id))
                    {
                        self.native_minecart_pass(state, pos, entity);
                    }
                }
                Pending::Landed(landed) => {
                    for write in landed.writes {
                        self.apply_landing_write(level, pos, write);
                    }
                }
                // The mid-walk contacts' own level writes, guarded on the touched
                // state above, as a landing's are.
                Pending::InsideWritten(written) => {
                    for write in written.writes {
                        self.apply_inside_write(level, pos, write);
                    }
                }
            }
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
    pub(in crate::game) fn item_use_first(
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
    /// The acting player as the hook points name one: the guest whose turn
    /// it is, else the host.
    pub(in crate::game) fn native_acting_id(&mut self) -> u128 {
        sunrise_client::lan::playerdata::ints_uuid(self.acting_player_uuid())
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
    /// The mobs from index `from` on are new in the world.
    pub(in crate::game) fn native_mobs_joined(&mut self, from: usize, natural: bool) {
        // Preserve saved/constructed UUIDs. Natural spawns built with provisional
        // IDs get their persistent identity when admitted, not on their first hit.
        for mob in self.mobs.iter().skip(from) {
            self.persist
                .uuids
                .get_or_insert_with(mob.id, sunrise_client::save::random_uuid);
        }
        self.join_spawn_tags(from);
        // A mod can keep a mob out; it is then taken out and not told of.
        self.native_joins_flush(false, 0);
        for i in from..self.mobs.len() {
            let m = &self.mobs[i];
            let (kind, pos) = (mob_kind(m), [m.position.x, m.position.y, m.position.z]);
            self.native_mob_spawned(self.dim.id(), &kind, pos, natural);
        }
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
    /// `PlayerInteractEvent.RightClickBlock` for a click on `hit` with
    /// `hand` holding `stack`: what the mods let it use.
    pub(in crate::game) fn right_click_block(
        &mut self,
        hit: &super::Hit,
        hand: Hand,
        stack: Option<&ItemStack>,
    ) -> BlockClickAnswer {
        let (mut use_block, mut use_item) = (TriState::Default, TriState::Default);
        if !mod_api::__private::listened(mod_api::HookId::RightClickBlock) {
            return BlockClickAnswer::Go {
                use_block,
                use_item,
            };
        }
        let Ok(snapshot) = stack.try_into_api() else {
            return BlockClickAnswer::Go {
                use_block,
                use_item,
            };
        };
        let click = self.block_click(hit, hand, snapshot.as_ref());
        let (mut result, mut canceled) = (mod_api::InteractionResult::Pass, false);
        self.native_right_click_block(
            &click,
            &mut use_block,
            &mut use_item,
            &mut result,
            &mut canceled,
        );
        if canceled {
            BlockClickAnswer::Canceled(result)
        } else {
            BlockClickAnswer::Go {
                use_block,
                use_item,
            }
        }
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
