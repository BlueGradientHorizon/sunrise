#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::block_entity_tick::ResidentBlockEntityAccess;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::build_creative_mode_tab_contents::CreativeCache;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::client_pick::ClientPick;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::fire::{
    ConnectsToCall, EntityChangeBlockPreCall, HotbarAttackIndicatorProgressCall,
    ItemAfterVerticalCollisionCall, PlayerPongCall, PreRemoveSideEffectsCall, UpdateShapeCall,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{BlockEntityDataSource, BlockEntityRef, ClockAdjustment};
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::native_data_maps::DataMapType;
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{
    Game, GameLevel, LevelDimension, Screen,
    execution_role::{self, ExecutionRole},
    mod_api_host,
    native_block_entity_draw::BlockEntityDraw,
    native_configs::ConfigFile,
    native_mob_draw::{MobRender, PlayerRender},
    native_panel_recipes::RecipePage,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::__private::game::{
    BlockStateDefinitionRegistration, ItemModelRegistration, ItemTintSourceRegistration,
    JsonAnimationTypeRegistration, LayerDefinitionRegistration, ModelLoaderRegistration,
    ParticleGroupRegistration, PresetEditorFactoryRegistration, RangeSelectPropertyRegistration,
    SelectPropertyRegistration, SpriteSourceRegistration, StandaloneModelRegistration,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::GameMode;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::GameTests;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::game::{PhantomSpawnResult, Shake, SleepProblem};
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{
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
pub(in crate::game::client_side) use mod_api::{
    ActiveEffect, AttachmentStore, BlockClick, BlockUpdateFlags, BonemealSource, ChunkRef,
    FluidStack, ItemClick, LevelRef, LogicalSide, NeighbourUpdate, TooltipElement, TriState,
    UsePhase,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{
    BlockKey, DebugEntry, DebugRenderer, EffectKey, FrameModel, FrameModelEdit, MapDecorationType,
    SkullModel, TextureAtlasRegistration,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{
    BlockRim, ClientPacket, ClientPayload, ColoredLight, ContainerLootFilled, ExplosionStart,
    ItemFished, LootDrops, PanelEvent, PortalShape, ScreenView,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{
    CameraType, Despawn, Identifier, MobRef, PermissionLevel, RailShape, SoundPlay, SoundRef,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::block_contact::Pending;
#[allow(unused_imports)]
#[cfg(test)]
pub(in crate::game::client_side) use sunrise_client::mobs::Damage;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
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
pub(in crate::game::client_side) use sunrise_client_core::gamerules::Value as GameRuleValue;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client_core::suggestion::Suggestion;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::{
    connection_kind::ConnectionKind,
    geometry::Direction,
    position::BlockPos,
    ticks::{ScheduleResult, TickPriority},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_world::level::{BonemealCheck, PlaceContext};
#[allow(unused_imports)]
pub(in crate::game::client_side) use winit::{event::MouseButton, keyboard::KeyCode};
#[allow(unused_imports)]
pub(in crate::game::client_side) use {
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
pub(in crate::game::client_side) use crate::game::native_hooks::add_attribute_tooltips::HookedAttributeTooltips;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::base::LevelView;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::bounce_restitution::install_block_contact_hooks;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::living_change_target::NativeHookHost;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::ModBlockEntity;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::sound_and_shader_helpers::{finite, finite_params, local_sound_ok};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::sound_and_shader_helpers::{level_sound_playable, mod_sound_ok, sound_id};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::add_section_geometry::{native_add_section_geometry};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::base::{native_block_entity_update_tag, native_blocks, native_component_types, native_items, native_menu_button, native_menu_quick_move, native_standalone_models};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::block_entity_client_tick::{native_block_entity_client_tick};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::block_entity_data_received::{native_block_entity_data_received};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::block_entity_on_load::{native_block_entity_on_load};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::block_entity_tick::{native_block_entity_tick};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::block_entity_type_add_blocks::{native_block_entity_type_add_blocks};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::can_place_item::{native_can_place_item};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::client_pick::{native_can_rider_interact};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::create_skull_models::{native_create_skull_models};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::extend_poi_types::{native_extend_poi_types};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::general_entity_tick::{native_general_entity_tick};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::map_decoration_types::{native_map_decoration_types};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::menu_slot_take::{native_menu_slot_take};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::on_entity_item_update::{native_on_entity_item_update};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::piston_push_reaction::{native_piston_push_reaction};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::register_block_state_models::{native_register_block_state_models};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::register_block_tint_sources::{native_register_block_tint_sources};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::register_cauldron_fluid_content::{native_register_cauldron_fluid_content};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::register_cauldron_interaction::{native_register_cauldron_interaction};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::register_client_item_extensions::{native_register_client_item_extensions};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::register_client_tooltip_component_factories::{native_register_client_tooltip_component_factories};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::register_color_resolvers::{native_register_color_resolvers};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::register_debug_entries::{native_register_debug_entries};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::register_debug_renderers::{native_register_debug_renderers};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::register_dimension_transition_screen::{native_register_dimension_transition_screen};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::register_enchantment_damage_effects::{native_register_enchantment_damage_effects};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::register_entity_layers::{native_register_entity_layers};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::register_entity_renderers::{native_register_entity_renderers};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::register_entity_spectator_shaders::{native_register_entity_spectator_shaders};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::register_fluid_models::{native_register_fluid_models};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::register_gui_layers::{native_register_gui_layers};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::register_item_models::{native_register_item_models};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::register_item_tint_sources::{native_register_item_tint_sources};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::register_json_animation_types::{native_register_json_animation_types};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::register_layer_definitions::{native_register_layer_definitions};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::register_load_conditions::{native_register_load_conditions};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::register_model_loaders::{native_register_model_loaders};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::register_particle_groups::{native_register_particle_groups};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::register_particle_providers::{native_register_particle_providers};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::register_preset_editors::{native_register_preset_editors};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::register_range_select_item_model_property::{native_register_range_select_item_model_property};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::register_render_pipelines::{native_register_render_pipelines};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::register_select_item_model_property::{native_register_select_item_model_property};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::register_special_model_renderer::{native_register_special_model_renderer};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::register_sprite_sources::{native_register_sprite_sources};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::register_texture_atlases::{native_register_texture_atlases};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::register_villager_professions::{native_register_villager_professions};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::block_entity_on_load::{authoritative_world_for_mods, world_for_mods};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::math::startup_math_profile;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::system_helpers::{mod_data_root, native_config_dir};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::base::{another_mods, startup_setting};
#[allow(unused_imports)]
#[cfg(test)]
pub(in crate::game::client_side) use crate::game::native_hooks::level_helpers::block_entity_for_block;
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::screen_build::ObservedScreen;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::api_values::{TryIntoApi, TryIntoEngine};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::screen_chars::chars_to_type;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::containers::ModBlockEntity as EngineBlockEntity;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::screen_view::{screen_view_for_mod, screen_view_with_id};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_world::level::flags;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::camera_pose::bounded_shake;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::camera_pose::hand_pose_matrix;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::living_death::hurt_server_args;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::health::mob_source;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::ContentsChangedGuard;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::apply_menu_button_cursor;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::ScreenDecision;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::title_target_id;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::observed_overlay_id;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::SHAKE_MAX_DEGREES;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::SHAKE_MAX_OFFSET;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::shake_pose;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::OpenNativeScreen;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::OpenNativeMenu;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::SlotMoves;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::NativePlayer;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::NativeRecipeLayout;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::NativeRecipeResult;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::NativeRecipeView;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::NativeRecipeStack;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::NativeRecipeKind;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::NativeRecipe;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::NativeItem;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::NativeRecipes;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::ScreenMousePostKind;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::ScreenMousePost;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::ScreenMousePre;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::use_totem;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::HitLevel;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::invulnerability_check;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::engine_reduction;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::LiveDamageRead;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::LiveDamageContainer;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::incoming_damage;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::damage_pre;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::damage_post;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::shield_block;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::knockback;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::DAMAGE_HOOKS;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::BLOCK_CONTACT_HOOKS;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::mod_particles_ok;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::dimension_id;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::mob_kind;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::block_use_result;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::BlockClickAnswer;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::BonemealAsk;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::debug_takes;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::mod_menu;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::with_property;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::property_change;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::config_dir;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::NativeItemEntity;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::box_meets;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::mod_screen_opens;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::client_packet_view;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::serverbound_packet_view;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::packet_types;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::packet_view;

#[path = "animate_tick_client.rs"]
pub(in crate::game) mod animate_tick;

#[path = "base_client.rs"]
pub(in crate::game) mod base;

#[path = "block_entity_client_tick_client.rs"]
pub(in crate::game) mod block_entity_client_tick;

#[path = "block_entity_on_load_client.rs"]
pub(in crate::game) mod block_entity_on_load;

#[path = "build_creative_mode_tab_contents_client.rs"]
pub(in crate::game) mod build_creative_mode_tab_contents;

#[path = "can_start_swimming_client.rs"]
pub(in crate::game) mod can_start_swimming;

#[path = "camera_read_client.rs"]
pub(in crate::game) mod camera_read;

#[path = "camera_type_client.rs"]
pub(in crate::game) mod camera_type;

#[path = "client_gui_state_client.rs"]
pub(in crate::game) mod client_gui_state;

#[path = "client_input_reads_client.rs"]
pub(in crate::game) mod client_input_reads;

#[path = "client_input_reads_tests_client.rs"]
pub(in crate::game) mod client_input_reads_tests;

#[path = "client_options_client.rs"]
pub(in crate::game) mod client_options;

#[path = "client_pick_client.rs"]
pub(in crate::game) mod client_pick;

#[path = "creative_tabs_read_client.rs"]
pub(in crate::game) mod creative_tabs_read;

#[path = "customize_boss_event_progress_client.rs"]
pub(in crate::game) mod customize_boss_event_progress;

#[path = "customize_chat_overlay_client.rs"]
pub(in crate::game) mod customize_chat_overlay;

#[path = "fluid_draw_client.rs"]
pub(in crate::game) mod fluid_draw;

#[path = "gui_ticks_client.rs"]
pub(in crate::game) mod gui_ticks;

#[path = "item_picked_up_client.rs"]
pub(in crate::game) mod item_picked_up;

#[path = "menu_extra_data_client.rs"]
pub(in crate::game) mod menu_extra_data;

#[path = "muffle_sounds_client.rs"]
pub(in crate::game) mod muffle_sounds;

#[path = "panel_recipes_client.rs"]
pub(in crate::game) mod panel_recipes;

#[path = "play_hit_sound_client.rs"]
pub(in crate::game) mod play_hit_sound;

#[path = "preset_title_client.rs"]
pub(in crate::game) mod preset_title;

#[path = "register_preset_editors_client.rs"]
pub(in crate::game) mod register_preset_editors;

#[path = "render_frame_post_client.rs"]
pub(in crate::game) mod render_frame_post;

#[path = "render_frame_pre_client.rs"]
pub(in crate::game) mod render_frame_pre;

#[path = "render_gui_pre_client.rs"]
pub(in crate::game) mod render_gui_pre;

#[path = "resolve_player_skin_client.rs"]
pub(in crate::game) mod resolve_player_skin;

#[path = "screen_exclusion_areas_client.rs"]
pub(in crate::game) mod screen_exclusion_areas;

#[path = "screen_hovered_stack_client.rs"]
pub(in crate::game) mod screen_hovered_stack;

#[path = "screen_transparent_background_client.rs"]
pub(in crate::game) mod screen_transparent_background;

#[path = "substitute_inline_images_client.rs"]
pub(in crate::game) mod substitute_inline_images;

#[path = "use_item_hand_client.rs"]
pub(in crate::game) mod use_item_hand;

#[path = "shared/book_screen_client.rs"]
pub(in crate::game) mod book_screen;

#[path = "shared/system_helpers_client.rs"]
pub(in crate::game) mod system_helpers;

#[path = "shared/world_helpers_client.rs"]
pub(in crate::game) mod world_helpers;

#[cfg(test)]
pub(in crate::game) fn seed_native_menu(
    game: &mut crate::game::ClientGame<'_>,
    window: i32,
    name: &str,
) -> u64 {
    if game.remote.is_none() {
        let (pipe, _server) = sunrise_protocol::connection::Pipe::pair();
        game.connect_local(pipe, "Native screen fixture").unwrap();
    }
    if game.remote.as_ref().unwrap().menu.as_ref().is_some() {
        game.remote_close_local();
    }
    let menu = &mut game.remote.as_mut().unwrap().menu;
    menu.set(Some((window, name.to_owned())));
    menu.generation
}

impl Game<'_, crate::game::client_side::ClientState> {
    pub(in crate::game) fn client_acting_id(&mut self) -> u128 {
        self.own_uuid()
    }
    pub(in crate::game) fn client_clicker(&mut self) -> (u128, i32) {
        (self.own_uuid(), self.own_entity_id())
    }
    pub(in crate::game) fn client_acting_or_local_player(&self) -> Option<PlayerRef> {
        self.local_player()
    }
    pub(in crate::game) fn client_chunks_loaded(&mut self) {
        self.world.loaded_chunks.clear();
    }
    pub(in crate::game) fn client_chunks_unloaded(
        &mut self,
        _unloaded: &[std::sync::Arc<sunrise_client::chunk::Chunk>],
    ) {
    }
    pub(in crate::game) fn client_apply_after_use_components(&mut self, stack: &ItemStack) {
        self.client_apply_use_cooldown(stack);
    }
    pub(in crate::game) fn observe_client_packet(
        &mut self,
        packet: &sunrise_protocol::packets::play::Clientbound,
    ) {
        if !mod_api::__private::listened(mod_api::HookId::ClientPacket) {
            return;
        }
        let view = client_packet_view(packet);
        self.native_client_packet(&view);
    }
    /// Whether a mod cancelled `packet` on its way to the server (`client_packet_send`).
    /// Builds the packet's view only when a mod listens.
    pub(in crate::game) fn client_packet_send_canceled(
        &self,
        packet: &sunrise_protocol::packets::play::Serverbound,
    ) -> bool {
        if !mod_api::__private::listened(mod_api::HookId::ClientPacketSend) {
            return false;
        }
        let mut canceled = false;
        self.native_client_packet_send(&serverbound_packet_view(packet), &mut canceled);
        canceled
    }
    /// The staged or shown native screen belonging to this opening, not a
    /// prior opening of the same numeric window id.
    pub(in crate::game) fn active_native_screen(&self) -> Option<&OpenNativeScreen> {
        let screen = self.side.mod_screen.0.as_ref()?;
        let remote = &self.remote.as_ref()?.menu;
        (!screen.detached
            && remote.generation == screen.menu.generation
            && remote.as_ref()?.0 == screen.menu.window)
            .then_some(screen)
    }
    /// What `set_screen` does once the hooks have answered: `screen` opens,
    /// and the mod `owner`'s own screen when one answered `ModScreen`.
    pub(in crate::game) fn apply_screen(&mut self, screen: Screen, owner: Option<&str>) {
        // `before` may already have closed it. Overlays that keep Inventory
        // underneath don't pass through this replacement path.
        if screen != Screen::Inventory {
            if self.active_native_screen().is_some() {
                self.remote_close_container();
            } else if self.side.mod_screen.0.as_ref().is_some_and(|s| s.detached) {
                self.remove_remote_menu_screen();
            }
        }
        if screen != Screen::DemoIntro {
            self.side.demo_intro = None;
        }
        self.side.screen = screen;
        if let Some(owner) = owner {
            self.side.native_mod_screen = Some(owner.to_owned());
        } else if screen != Screen::Mod {
            self.side.native_mod_screen = None;
        }
        if screen != Screen::Playing {
            self.release_mappings_on_screen_open();
        }
        if screen == Screen::Mod {
            self.clear_device_keys();
            self.set_mapping_held("key.attack", false);
            self.set_mapping_held("key.use", false);
            self.mining = None;
            self.capture(false);
        }
        self.commit_screen_transition();
    }
    /// Runs close cleanup for any screen or overlay being replaced by
    /// `mod_screen_open` once hooks allow the transition, matching vanilla's
    /// `Gui.setScreen` / `Screen.removed` so no container menu, editor, dialog,
    /// or prompt stays open over the new screen.
    pub(in crate::game::client_side) fn close_replaced_screen_for_mod(&mut self) {
        let replacing = std::mem::replace(&mut self.side.screen_state.0.replacing, true);
        match self.side.screen {
            Screen::Inventory => self.close_container_client(),
            Screen::Creative => self.close_creative_menu(),
            Screen::Options => self.ui.options.discard_draw(),
            _ => {}
        }
        if self.side.book.is_some() {
            self.close_book();
        }
        if self.side.sign_edit.is_some() {
            self.close_sign_editor();
        }
        if self.dialog_open() {
            self.close_dialogs();
        }
        self.side.link_prompt = None;
        self.side.command_prompt.0 = None;
        self.side.code_of_conduct = None;
        self.side.server_packs.0.prompt = None;
        self.side.screen_state.0.replacing = replacing;
    }
    /// `LocalPlayer.clientSideCloseContainer`: restore the logical inventory
    /// menu, then `Gui.setScreen(null)`. Keep the old screen and its view for
    /// `ScreenEvent.Closing` before running its removal, not its user close.
    pub(in crate::game::client_side) fn close_screen_to_level(
        &mut self,
        show_death_screen: bool,
    ) -> super::Result<()> {
        let overlay = self.observed_overlay_now();
        let inventory_screen = overlay.is_none()
            && self.side.screen == Screen::Inventory
            && self.remote_menu_type().is_none()
            && self.shown_native_screen().is_none();
        let heard = self.screen_hooks_heard();
        let outgoing =
            heard
                .then(|| {
                    if let Some(id) = overlay {
                        Some(self.observe_view(self.screen_overlay_view(id)))
                    } else if self.side.screen != Screen::Playing {
                        Some(self.observe_view(self.screen_change_view(self.side.screen)))
                    } else {
                        self.side.title.0.as_ref().map(|title| {
                            self.observe_view(self.screen_overlay_view(title.mode.id()))
                        })
                    }
                })
                .flatten();
        let detach_native = self.shown_native_screen().is_some();
        self.remote_set_menu(None, None);
        if detach_native && let Some(screen) = self.side.mod_screen.0.as_mut() {
            screen.detached = true;
        }
        let has_level = self.remote.as_ref().is_some_and(|r| r.has_level());
        let dead = self
            .player
            .as_ref()
            .is_some_and(|player| player.health.health() <= 0.0);
        let title = if has_level {
            None
        } else {
            Some(
                self.new_title(std::env::current_dir()?.join("saves"))
                    .with_user(|| self.options.player_name()),
            )
        };
        let screen = if has_level && dead && show_death_screen {
            Screen::Dead
        } else {
            if has_level && dead {
                self.respawn_player()?;
            }
            Screen::Playing
        };
        let decision = if heard {
            let none = self.screen_overlay_view(Screen::Playing.id());
            let old = outgoing.as_ref().map(ObservedScreen::view);
            let new = self
                .screen_overlay_view(title.as_ref().map_or(screen.id(), |title| title.mode.id()));
            let answer = self.screen_change(
                old.as_ref().unwrap_or(&none),
                &new,
                title.is_some() || screen != Screen::Playing,
                old.is_some(),
            );
            self.screen_decision_for_context(
                answer,
                true,
                title.is_some() || self.side.title.0.is_some(),
            )
        } else {
            ScreenDecision::Open
        };
        let (screen, owner, target) = match decision {
            ScreenDecision::Keep => return Ok(()),
            ScreenDecision::ModScreen(owner) => (Screen::Mod, Some(owner), None),
            ScreenDecision::Vanilla(target) => (screen, None, Some(target)),
            ScreenDecision::Open => (screen, None, None),
        };

        self.reset_inventory_clicks();
        self.remove_remote_menu_screen();
        if inventory_screen && let Some(remote) = self.remote.as_mut() {
            // InventoryMenu.removed clears the result, not the client input grid.
            remote.craft_result = None;
        }
        self.remove_sign_editor();
        self.remove_chat_screen();
        self.close_dialogs();
        self.side.book = None;
        self.side.link_prompt = None;
        self.side.command_prompt.0 = None;
        self.side.code_of_conduct = None;
        self.side.server_packs.0.prompt = None;
        self.side.pause_focus = None;
        if self.side.screen == Screen::Options {
            self.ui.options.discard_draw();
        }
        if screen == Screen::Dead && self.side.screen != Screen::Dead {
            self.ui.death_time = 0;
            self.ui.death_confirm = None;
        }
        if let Some(mut title) = title {
            title.set_rule_registry(Arc::new(crate::game::api_values::game_rule_registry()));
            self.stage_title(title);
            self.side.screen = Screen::Playing;
            self.side.native_mod_screen = None;
        }
        if let Some(target) = target {
            self.open_screen_target(target);
        } else {
            if self.side.title.0.is_some() && screen == Screen::Playing {
                self.release_mappings_on_screen_open();
            }
            self.apply_screen_with_mod_book(screen, owner);
        }
        self.capture(
            self.side.screen == Screen::Playing
                && self.side.title.0.is_none()
                && !self.screen_over_playing(),
        );
        Ok(())
    }
    /// Commit a screen transition synchronously (`Gui.setScreen` / `Screen.init`):
    /// bumps `screen_instance`, clears `last_click` and outgoing mounted widgets,
    /// rebuilds layout, publishes `screen_state.observed`, and mounts the
    /// incoming screen's widgets immediately.
    pub(in crate::game) fn commit_screen_transition(&mut self) {
        self.side.screen_state.0.bump_instance();
        self.clear_screen_widgets();
        self.build_screen();
        let observed = self.observed_now();
        self.side.screen_state.0.observed = observed.clone();
        if !self.options.execution_role.is_client() {
            return;
        }
        if let Some(obs) = observed {
            self.init_screen_widgets(&obs.view());
        } else if let Some(obs) = self.headless_screen_snapshot() {
            self.init_screen_widgets(&obs.view());
        }
    }
    /// Like `decide_screen`, for `mod_screen_open`, which clears any active
    /// overlay via `close_replaced_screen_for_mod` before applying the
    /// decision, so `ScreenOpening::ModScreen` and `ScreenOpening::Vanilla`
    /// answers remain eligible even when replacing an overlay.
    pub(in crate::game::client_side) fn decide_screen_for_mod_open(
        &self,
        current: &ScreenView<'_>,
        new: &ScreenView<'_>,
        closes: bool,
    ) -> ScreenDecision {
        self.screen_decision_with(self.screen_change(current, new, true, closes), true)
    }
    /// The open mod screen: a plain panel with its slots' frames, the mod's
    /// drawing over it, then the title and the inventory's label. False when
    /// none is showing.
    pub(in crate::game::client_side) fn draw_native_screen(
        &self,
        batch: &mut HudBatch,
        ox: f32,
        oy: f32,
    ) -> bool {
        let Some(s) = self.shown_native_screen() else {
            return false;
        };
        let atlas = &self.gui.atlas;
        if !s.layout().own_background {
            let (w, h) = (s.layout().width, s.layout().height);
            batch.fill(atlas, ox, oy, w, h, rgba(0, 0, 0, 255));
            batch.fill(
                atlas,
                ox + 1.0,
                oy + 1.0,
                w - 2.0,
                h - 2.0,
                rgba(198, 198, 198, 255),
            );
            for i in s.visible_slots() {
                let (x, y) = self.slot_position(i);
                let (x, y) = (ox + x - 1.0, oy + y - 1.0);
                batch.fill(atlas, x, y, 18.0, 18.0, rgba(55, 55, 55, 255));
                batch.fill(
                    atlas,
                    x + 1.0,
                    y + 1.0,
                    17.0,
                    17.0,
                    rgba(255, 255, 255, 255),
                );
                batch.fill(
                    atlas,
                    x + 1.0,
                    y + 1.0,
                    16.0,
                    16.0,
                    rgba(139, 139, 139, 255),
                );
            }
        }
        if s.screen.has_draw() {
            let frame = s.frame(batch, (ox, oy), self.side.cursor);
            let mut draw = crate::game::host::draw::GameDrawContext::new(self, batch);
            s.screen.draw(self, frame, &mut draw);
        }
        self.draw_screen_phase(batch, sunrise_client::screen_draw::RenderPhase::Background);
        if s.layout().own_labels {
            return true;
        }
        let grey = rgba(64, 64, 64, 255);
        if let Some(title) = self.remote_menu_title() {
            let [x, y] = s.layout().title_label.unwrap_or([8.0, 6.0]);
            batch.text_styled(atlas, &title, ox + x, oy + y, grey, false);
        }
        if s.player_slots() != 0 {
            let inventory = self.translate_with_fallback("container.inventory", "Inventory");
            let [x, y] = s
                .layout()
                .inventory_label
                .unwrap_or([8.0, s.layout().height - 94.0]);
            batch.text(atlas, &inventory, ox + x, oy + y, grey, false);
        }
        true
    }
    /// The open mod screen's tooltip under the cursor.
    pub(in crate::game::client_side) fn draw_native_screen_tooltip(
        &mut self,
        batch: &mut HudBatch,
    ) {
        // Over no item: an empty slot still gets the mod's hint (which fuel it takes).
        // A carried stack hides only slot tooltips, as AbstractContainerScreen's;
        // the screen's other parts keep theirs, as a vanilla widget's tooltip does.
        let slot = self.slot_at(batch, self.side.cursor.0, self.side.cursor.1);
        if slot.is_some_and(|i| self.slot_view(i).is_some()) {
            return;
        }
        let mut lines = Vec::new();
        self.native_screen_tooltip_lines(batch, slot, &mut lines);
        self.draw_tooltip_lines(batch, &lines, self.side.cursor.0, self.side.cursor.1);
    }
    pub(in crate::game) fn headless_screen_snapshot(&self) -> Option<ObservedScreen> {
        if self.side.renderer.0.is_some() || self.remote_loading() {
            return None;
        }
        if let Some(id) = self.observed_overlay_now() {
            let none = self.screen_change_view(Screen::Playing);
            return Some(self.observe_view(screen_view_with_id(&none, id)));
        }
        if self.side.screen != Screen::Playing {
            return Some(self.observe_view(self.screen_change_view(self.side.screen)));
        }
        if let Some(title) = self.side.title.0.as_ref() {
            let none = self.screen_change_view(Screen::Playing);
            return Some(self.observe_view(screen_view_with_id(&none, title.mode.id())));
        }
        None
    }
    /// Takes the title down as a world opens over it (`setScreen` to the
    /// loading screen, then null): `screen_opening` to no screen, then
    /// `screen_closing` for the title's screen, before it goes. The world
    /// is opening, so a `Cancel` doesn't keep it.
    pub(in crate::game) fn hide_title(&mut self) {
        if self.side.title.0.is_none() {
            return;
        }
        if self.screen_hooks_heard() {
            self.title_change(None);
        }
        self.side.title.0 = None;
        self.commit_screen_transition();
    }
    /// A mod's own screen cannot open now: over a book, a sign editor, a
    /// dialog or a prompt, with neither the title nor a player, with a
    /// world loading, or on a guest's behalf.
    pub(in crate::game::client_side) fn mod_screen_blocked(&self) -> bool {
        (self.player.is_none() && self.side.title.0.is_none())
            || self.screen_over_playing()
            || self.remote_loading()
    }
    /// The camera native mods draw this frame from (`native_camera_pose`), starting from
    /// `view`, vanilla's position, yaw and pitch.
    pub(in crate::game) fn native_camera_pose_now(
        &mut self,
        partial: f32,
        view: (glam::DVec3, f32, f32),
    ) -> (glam::DVec3, f32, f32) {
        let (mut position, mut yaw, mut pitch) = (view.0.to_array(), view.1, view.2);
        self.native_camera_pose(partial, &mut position, &mut yaw, &mut pitch);
        let posed = posed_camera(view, position, yaw, pitch);
        if self.apply_requested_camera_type()
            && let Some(refreshed) = self.camera_view(partial)
        {
            // Refresh vanilla components, not the listener's explicit overrides.
            return (
                if posed.0 == view.0 {
                    refreshed.0
                } else {
                    posed.0
                },
                if posed.1 == view.1 {
                    refreshed.1
                } else {
                    posed.1
                },
                if posed.2 == view.2 {
                    refreshed.2
                } else {
                    posed.2
                },
            );
        }
        posed
    }
    /// Deliver a non-repeat key press to the native panel.
    pub(in crate::game) fn native_panel_key_event(
        &mut self,
        key: KeyCode,
        pressed: bool,
        repeat: bool,
    ) {
        if pressed
            && !repeat
            && let Some(name) = keybinds::key_name(key)
        {
            self.native_panel_key(name);
        }
    }
    /// The internal container slot base of the open mod screen (`MOD_MENU_SLOT_BASE`
    /// for a `ModMenuType`, `300` for a stock menu).
    pub(in crate::game::client_side) fn native_screen_container_slot_base(&self) -> usize {
        self.shown_native_screen()
            .map_or(300, OpenNativeScreen::container_slot_base)
    }
    /// A data slot of window `window`'s menu (`ContainerSetData`); false
    /// when no mod screen is showing for that active remote window.
    pub(in crate::game) fn native_screen_data(
        &mut self,
        window: i32,
        index: usize,
        value: i32,
    ) -> bool {
        if self.shown_active_native_screen().is_none() {
            return false;
        }
        self.side
            .mod_screen
            .0
            .as_mut()
            .is_some_and(|s| s.set_data(window, index, value))
    }
    /// The open mod screen's inventory's left column.
    pub(in crate::game) fn native_screen_inventory_x(&self) -> Option<f32> {
        self.shown_native_screen().map(|s| s.layout().inventory_x)
    }
    /// The first row of the player's inventory on the open mod screen.
    pub(in crate::game) fn native_screen_inventory_y(&self) -> Option<f32> {
        self.shown_native_screen().map(|s| s.layout().inventory_y)
    }
    /// The menu type resolved for this client's opening, including in ScreenOpening.
    pub(in crate::game::client_side) fn native_screen_menu_type(
        &self,
        window: i32,
        name: &str,
    ) -> Option<&'static sunrise_client::containers::ModMenuType> {
        let menu = &self.active_native_screen()?.menu;
        (menu.window == window && menu.name == name).then_some(menu.mod_type)?
    }
    /// A press consumed by the shown screen goes to the server as a menu button click.
    pub(in crate::game::client_side) fn native_screen_press(&mut self, right: bool) -> bool {
        let Some(r) = self.side.renderer.0.as_ref() else {
            return false;
        };
        let batch = HudBatch::new(
            [r.config.width, r.config.height],
            self.settings.gui_scale_inputs(),
        );
        self.native_screen_press_at(&batch, right)
    }
    /// Dispatch a press in the given GUI viewport, with inventory slots taking priority.
    pub(in crate::game::client_side) fn native_screen_press_at(
        &mut self,
        batch: &HudBatch,
        right: bool,
    ) -> bool {
        let Some(screen) = self.shown_native_screen() else {
            return false;
        };
        if self
            .slot_at(batch, self.side.cursor.0, self.side.cursor.1)
            .is_some()
        {
            return false;
        }
        let opening = (screen.menu.window, screen.menu.generation);
        let frame = screen.frame(batch, self.inventory_origin(batch), self.side.cursor);
        let Some(button) = screen.screen.click(self, frame, right) else {
            return false;
        };
        // Numeric container IDs can be reused. Send only for the active opening that heard the press.
        if !self
            .shown_active_native_screen()
            .is_some_and(|screen| (screen.menu.window, screen.menu.generation) == opening)
            || !self.remote_button_click(button)
        {
            return false;
        }
        self.side.audio.0.play_button_click();
        true
    }
    /// Where the open mod screen puts container slot `slot` this frame: where
    /// a mod last moved it, else where the screen declared it; None when
    /// inactive/hidden.
    pub(in crate::game) fn native_screen_slot(&self, slot: usize) -> Option<(f32, f32)> {
        let s = self.shown_native_screen()?;
        let declared = s
            .layout()
            .slots
            .get(slot)?
            .as_ref()
            .map(|slot| slot.position);
        let [x, y] = s.slot_moves.position(slot, declared)?;
        Some((x, y))
    }
    pub(in crate::game) fn native_screen_slot_icon(&self, slot: usize) -> Option<&str> {
        self.shown_native_screen()?
            .layout()
            .slots
            .get(slot)?
            .as_ref()?
            .no_item_icon
            .as_deref()
    }
    /// The container slots the open mod screen shows this frame.
    pub(in crate::game) fn native_screen_slots(&self) -> Option<Vec<usize>> {
        Some(
            self.shown_native_screen()?
                .visible_container_slots()
                .collect(),
        )
    }
    /// The screen-specific line override, shared by filled-slot and empty-space tooltips.
    /// Item images and component gathering remain on the stack's normal drawing path.
    pub(in crate::game::client_side) fn native_screen_tooltip_lines(
        &self,
        batch: &HudBatch,
        slot: Option<usize>,
        lines: &mut Vec<(sunrise_client::text::Text, u32)>,
    ) {
        let Some(screen) = self
            .shown_native_screen()
            .filter(|s| s.screen.has_tooltip())
        else {
            return;
        };
        use crate::game::api_values::{TryIntoApi, TryIntoEngine};
        // Keep the original tooltip if the callback or either conversion fails.
        // An item's text can fail on every frame, so do not log conversion refusals here.
        let Ok(mut snapshot) = lines.try_into_api() else {
            return;
        };
        let menu_slot = slot.and_then(|slot| {
            Self::menu_slot_index(&screen.menu.name, slot).map(|slot| slot as u32)
        });
        let frame = screen.frame(batch, self.inventory_origin(batch), self.side.cursor);
        if screen.screen.tooltip(self, frame, menu_slot, &mut snapshot)
            && let Ok(replacement) = snapshot.try_into_engine(&())
        {
            *lines = replacement;
        }
    }
    /// The open mod screen's width.
    pub(in crate::game) fn native_screen_width(&self) -> Option<f32> {
        self.shown_native_screen().map(|s| s.layout().width)
    }
    /// The pose of native mods' camera shake in this frame (`native_camera_shake`),
    /// none while not playing.
    pub(in crate::game::client_side) fn native_shake_matrix(&self, partial: f32) -> glam::Mat4 {
        if !self.playing() {
            return glam::Mat4::IDENTITY;
        }
        let mut shake = Shake::default();
        self.native_camera_shake(partial, &mut shake);
        if shake == Shake::default() {
            return glam::Mat4::IDENTITY;
        }
        shake_pose(bounded_shake(shake, self.screen_effect_scale()))
    }
    /// The zoom in effect while playing (a native mod's); the spyglass's own
    /// view wins.
    pub(in crate::game::client_side) fn native_zoom_now(&self) -> Option<mod_api::Zoom> {
        if !self.playing() || self.scoping() {
            return None;
        }
        let mut zoom = None;
        self.native_zoom(&mut zoom);
        let z = zoom?;
        let factor = finite(z.factor, 1.0, 10.0);
        (factor > 1.0).then_some(mod_api::Zoom {
            factor,
            smooth: z.smooth,
            turn: if z.turn.is_finite() {
                z.turn.clamp(0.05, 1.0)
            } else {
                1.0
            },
        })
    }
    /// What the frame's screen changed since the last one. The `Screen` list,
    /// the overlays' opening and the title's screens announce their own
    /// changes before they apply (`set_screen`, `open_screen_overlay`,
    /// `settle_title_mode`, `show_title`, `hide_title`); what is left, an
    /// overlay closing, is announced here, through the same `screen_change`
    /// and in the same order, but after the change. `screen_init_post`
    /// follows on opening and when a resize rebuilds the receiver's children.
    pub(in crate::game) fn observe_change(&mut self, current: Option<ObservedScreen>) {
        let changed = match (&self.side.screen_state.0.observed, &current) {
            (Some(old), Some(new)) => {
                old.id != new.id
                    || old.menu != new.menu
                    || old.menu_generation != new.menu_generation
                    || old.mod_screen != new.mod_screen
                    || old.dialog_key != new.dialog_key
            }
            (None, None) => false,
            _ => true,
        };
        if changed {
            let old = self
                .side
                .screen_state
                .0
                .observed
                .as_ref()
                .filter(|old| old.overlay);
            let opens = current.as_ref().is_some_and(|new| new.overlay)
                || (current.is_none() && old.is_some());
            if old.is_some() || opens {
                let none = self.screen_change_view(Screen::Playing);
                let from = old.map(ObservedScreen::view);
                let to = current.as_ref().map(ObservedScreen::view);
                self.screen_change(
                    from.as_ref().unwrap_or(&none),
                    to.as_ref().unwrap_or(&none),
                    opens,
                    from.is_some(),
                );
            }
            self.side.screen_state.0.bump_instance();
            self.clear_screen_widgets();
            self.side.screen_state.0.observed = current.clone();
            if let Some(new) = &current
                && self.options.execution_role.is_client()
            {
                self.init_screen_widgets(&new.view());
            }
            return;
        }
        if let (Some(old), Some(new)) = (&self.side.screen_state.0.observed, &current)
            && old.screen_size != new.screen_size
            // Screen.resize calls repositionElements. Screen's default rebuilds
            // widgets; these receivers override it to relayout existing children.
            // A panel rect can also move independently (e.g. opening a recipe book).
            && !matches!(
                new.id,
                "options"
                    | "world_options"
                    | "stats"
                    | "advancements"
                    | "social"
                    | "debug_options"
                    | "dialog"
                    | "dialog_warning"
                    | "confirm_link"
                    | "demo_intro"
                    | "link_prompt"
                    | "code_of_conduct"
                    | "world_select"
                    | "world_create"
                    | "world_create_flat"
                    | "world_create_buffet"
                    | "world_edit"
                    | "world_recover"
                    | "multiplayer"
                    | "multiplayer_warning"
                    | "disconnected"
                    | "onboarding"
            )
        {
            self.clear_screen_widgets();
            self.init_screen_widgets(&new.view());
        }
        self.side.screen_state.0.observed = current;
    }
    pub(in crate::game) fn observe_screen(&mut self) {
        self.settle_title_mode();
        self.refresh_screen_widgets();
    }
    pub(in crate::game) fn observe_view(&self, view: ScreenView<'_>) -> ObservedScreen {
        let screen_size = if self.side.renderer.0.is_some() {
            self.gui_frame_now().1.size.map(|size| size as f32)
        } else {
            [view.rect[2], view.rect[3]]
        };
        ObservedScreen {
            id: view.id,
            overlay: self.side.screen == Screen::Playing || self.observed_overlay_now().is_some(),
            mod_screen: view.mod_screen.map(str::to_owned),
            menu: view.menu.map(str::to_owned),
            menu_generation: (self.side.screen == Screen::Inventory && !self.screen_over_playing())
                .then(|| self.remote.as_ref().map(|r| r.menu.generation))
                .flatten(),
            dialog_key: (view.id == "dialog").then(|| match self.dialog_screen_key() {
                (true, _, _) => (true, false, None),
                key => key,
            }),
            title: view
                .title
                .and_then(|_| self.remote.as_ref()?.shared_menu_title_snapshot()),
            rect: view.rect,
            screen_size,
            slots: view.slots,
            mouse: view.mouse,
            partial_tick: view.partial_tick,
            regions: Vec::new(),
        }
    }
    /// Snapshot of the receiving screen for screen input hooks, including overlays.
    pub(in crate::game) fn observed_input_screen(&self) -> Option<ObservedScreen> {
        let view = self.observed_screen_view()?;
        let regions = view.regions.to_vec();
        let mut snapshot = self.observe_view(view);
        snapshot.regions = regions;
        Some(snapshot)
    }
    /// What `observe_screen` compares with the last frame's.
    pub(in crate::game) fn observed_now(&self) -> Option<ObservedScreen> {
        // Only the title's screens and the overlays show over `Screen::Playing`.
        self.observed_screen_view()
            .map(|view| self.observe_view(view))
    }
    pub(in crate::game::client_side) fn observed_overlay_now(&self) -> Option<&'static str> {
        let book = self.side.book.as_ref().map(super::books::BookScreen::id);
        observed_overlay_id(&[
            (self.side.command_prompt.0.is_some(), "command_prompt"),
            (self.side.link_prompt.is_some(), "link_prompt"),
            (self.side.code_of_conduct.is_some(), "code_of_conduct"),
            (
                self.side.server_packs.0.prompt.is_some(),
                "server_pack_prompt",
            ),
            (self.dialog_open(), "dialog"),
            (book.is_some(), book.unwrap_or("book_view")),
            (self.side.sign_edit.is_some(), "sign_editor"),
        ])
    }
    pub(in crate::game) fn observed_screen_view(&self) -> Option<ScreenView<'_>> {
        let Some(id) = self.observed_overlay_now() else {
            return self.screen_view_now();
        };
        self.side.renderer.0.as_ref()?;
        Some(self.screen_overlay_view(id))
    }
    /// Opens an overlay, one of the screens kept beside `Screen` (the sign
    /// editor, a book or lectern, a dialog, the command, link,
    /// code-of-conduct and server-pack prompts), as `Gui.setScreen` opens
    /// a screen: `screen_opening` fires first with `id` as the new screen,
    /// then `screen_closing` for an overlay it replaces. A mod's `Cancel`
    /// keeps what is open and `open` never runs; `ModScreen` opens the
    /// mod's own screen instead, unless one can't open now, and `Vanilla` its
    /// vanilla screen. Otherwise `open`
    /// sets the overlay and `screen_init_post` follows. Returns whether
    /// `open` ran. With no mod loaded, on a server's `Game` and on a guest's
    /// behalf, `open` just runs.
    pub(in crate::game) fn open_screen_overlay(
        &mut self,
        id: &'static str,
        open: impl FnOnce(&mut Self),
    ) -> bool {
        if !self.screen_hooks_heard() {
            open(self);
            self.release_mappings_on_screen_open();
            self.side.screen_state.0.build.reopened();
            self.commit_screen_transition();
            return true;
        }
        // What changed earlier this frame is told first, so `screen_state`
        // is what shows now.
        self.observe_screen();
        let decision = {
            let none = self.screen_change_view(Screen::Playing);
            let from = self
                .side
                .screen_state
                .0
                .observed
                .as_ref()
                .filter(|old| old.overlay);
            let from = from.map(ObservedScreen::view);
            let to = screen_view_with_id(&none, id);
            self.decide_screen(from.as_ref().unwrap_or(&none), &to, from.is_some())
        };
        match decision {
            ScreenDecision::Keep => return false,
            ScreenDecision::ModScreen(mod_id) => {
                self.apply_screen(Screen::Mod, Some(mod_id));
                return false;
            }
            ScreenDecision::Vanilla(target) => {
                self.open_screen_target(target);
                return false;
            }
            ScreenDecision::Open => {}
        }
        open(self);
        self.release_mappings_on_screen_open();
        self.side.screen_state.0.build.reopened();
        self.commit_screen_transition();
        true
    }
    /// Opens vanilla screen `target` as it opens on its own, in place of the
    /// one asked for (`ScreenOpening::Vanilla`, which `screen_decision` let
    /// through): the change fires no screen hook again, and commits once.
    pub(in crate::game) fn open_screen_target(&mut self, target: ScreenTarget) {
        let before_instance = self.side.screen_state.0.screen_instance;
        self.side.screen_state.0.replacing = true;
        match target {
            ScreenTarget::Pause => {
                self.side.pause_focus = None;
                self.side.pause_menu = true;
                self.set_screen(Screen::Paused);
            }
            ScreenTarget::Options => self.open_options(false),
            ScreenTarget::Inventory => {
                // A menu the server opened for the screen asked for (a crafting
                // table) closes: the inventory shows the player's own. With
                // only the player's own open, there is nothing to close.
                if self.remote_menu_type().is_some() {
                    self.close_container_client();
                }
                self.open_player_inventory();
            }
            ScreenTarget::Advancements => self.open_advancements(false),
            ScreenTarget::Stats => self.open_stats(),
            ScreenTarget::Chat => self.begin_chat_screen("", false),
            _ => {
                // A title screen: set the title mode first so `set_screen(Playing)`
                // commits directly into the target title screen without double-init.
                if let (Some(title), Some(id)) =
                    (self.side.title.0.as_mut(), title_target_id(target))
                {
                    title.open_by_id(id);
                }
                self.release_mappings_on_screen_open();
                self.set_screen(Screen::Playing);
            }
        }
        // Every target takes the pointer, even one opened in place of a
        // request that grabbed none (a mod's screen asked for while playing).
        self.capture(false);
        self.side.screen_state.0.replacing = false;
        if self.side.screen_state.0.screen_instance == before_instance {
            self.commit_screen_transition();
        }
    }
    /// Refresh the receiver's build before another input can reach it.
    pub(in crate::game) fn refresh_screen_widgets(&mut self) {
        if self.side.screen_state.0.replacing {
            return;
        }
        // Keeps the build on the frame's window size and drops it once nothing
        // shows; an unchanged frame compares and builds nothing.
        self.build_screen();
        // Free with no native mod: the snapshot clones the title and menu.
        if mod_api::__private::mods().is_empty() {
            return;
        }
        let current = self.observed_now();
        self.observe_change(current);
    }
    /// `screen` as the screen hooks see it while it opens or closes: no
    /// renderer (a test, a window not yet up) leaves its rectangle empty.
    pub(in crate::game::client_side) fn screen_change_view(
        &self,
        screen: Screen,
    ) -> ScreenView<'_> {
        let rect = match &self.side.renderer.0 {
            Some(renderer) => self.screen_rect(
                &HudBatch::new(
                    [renderer.config.width, renderer.config.height],
                    self.settings.gui_scale_inputs(),
                ),
                screen,
            ),
            None => [0.0; 4],
        };
        let mut view = self.screen_view_at(screen, rect);
        if screen == Screen::Playing {
            view.slots = 0;
        }
        view
    }
    /// `native_screen_char_typed_pre` for one character on `receiver`, with
    /// the focused box's edit: what the listeners leave of `field` goes back
    /// to the box before the character does. Whether they cancelled the character.
    pub(in crate::game::client_side) fn screen_char_canceled_on(
        &mut self,
        receiver: Option<&ObservedScreen>,
        c: char,
    ) -> bool {
        if !mod_api::__private::listened(mod_api::HookId::ScreenCharTypedPre) {
            return false;
        }
        let Some(receiver) = receiver else {
            return false;
        };
        let modifiers = super::glfw_modifiers(self.side.modifiers);
        self.with_focused_text_edit(|game, field| {
            let screen = receiver.view();
            let mut canceled = false;
            game.native_screen_char_typed_pre(&screen, c, modifiers, field, &mut canceled);
            canceled
        })
    }
    /// Deliver each printable character of `text` through `CharacterTyped.Pre`,
    /// mounted screen widgets first, then `take` (the vanilla text box sink,
    /// returning whether it handled the character), and finally
    /// `CharacterTyped.Post` on the captured receiving screen ONLY when
    /// unhandled. Snapshots the receiving screen per character (matching
    /// vanilla `KeyboardHandler.charTyped` reading `minecraft.screen` each call)
    /// and never falls through into a replacement screen opened mid-character.
    pub(in crate::game) fn screen_chars_dispatch(
        &mut self,
        text: &str,
        mut take: impl FnMut(&mut Self, &str) -> bool,
    ) {
        let modifiers = super::glfw_modifiers(self.side.modifiers);
        let mut encoded = [0; 4];
        for c in text.chars() {
            if c.is_control() {
                continue;
            }
            if self.observed_screen_view().is_none() && self.headless_screen_snapshot().is_none() {
                break;
            }
            let receiver = self.observed_input_screen();
            let initial_instance = self.side.screen_state.0.screen_instance;
            if self.screen_char_canceled_on(receiver.as_ref(), c) {
                continue;
            }
            let mut handled = self.side.screen_state.0.screen_instance == initial_instance
                && self.mounted_widget_char_typed(c, modifiers);
            if !handled && self.side.screen_state.0.screen_instance == initial_instance {
                handled = take(self, c.encode_utf8(&mut encoded));
            }
            if !handled {
                self.screen_chars_post_on(receiver.as_ref(), c);
            }
        }
    }
    /// `ScreenEvent.CharacterTyped.Post` for a character that neither mounted
    /// widgets nor any text box took, on the captured receiving screen.
    pub(in crate::game::client_side) fn screen_chars_post_on(
        &self,
        receiver: Option<&ObservedScreen>,
        c: char,
    ) {
        if !mod_api::__private::listened(mod_api::HookId::ScreenCharTypedPost) {
            return;
        }
        let Some(receiver) = receiver else {
            return;
        };
        let modifiers = super::glfw_modifiers(self.side.modifiers);
        self.native_screen_char_typed_post(&receiver.view(), c, modifiers);
    }
    /// A key press's characters (`ScreenEvent.CharacterTyped.Pre`, vanilla's
    /// `KeyboardHandler.charTyped`), dispatched once the screen has acted on
    /// the key: GLFW's char callback comes after its key callback, so the
    /// characters go to the screen that is open now — a key that closed the
    /// screen leaves none to hear them, as vanilla's closed screen hears
    /// nothing, and one that moved the focus types into the box it moved to.
    /// Each printable one snapshots the current screen, runs
    /// `native_screen_char_typed_pre` (when hooks apply), then mounted widgets,
    /// then the focused text box, and unhandled characters fire
    /// `native_screen_char_typed_post` on that character's captured receiver.
    pub(in crate::game) fn screen_chars_typed(&mut self, text: Option<&str>) {
        let Some(text) = text else {
            return;
        };
        if self.observed_screen_view().is_none() && self.headless_screen_snapshot().is_none() {
            return;
        }
        // A held edit modifier makes a shortcut of the key, not text.
        let text = chars_to_type(text, self.types_text());
        if text.is_empty() {
            return;
        }
        self.screen_chars_dispatch(&text, |game, c| game.type_chars_into_screen(c).is_ok());
    }
    /// What a mod's `screen_opening` answer does here: a `ModScreen` only
    /// where a mod's screen can open (`mod_screen_blocked`), a `Vanilla`
    /// target only where it opens (a title screen while the title shows, a
    /// world's screen where a mod's could open in a world), else the change
    /// goes ahead and the refused target is logged. Records whether a mod's
    /// answer changed what shows (`ScreenState::answered`).
    pub(in crate::game) fn screen_decision(
        &self,
        answer: Option<Answered<ScreenOpening>>,
    ) -> ScreenDecision {
        self.screen_decision_with(answer, false)
    }
    pub(in crate::game::client_side) fn screen_decision_for_context(
        &self,
        answer: Option<Answered<ScreenOpening>>,
        clears_overlays: bool,
        title_available: bool,
    ) -> ScreenDecision {
        let blocked = if clears_overlays {
            (self.player.is_none() && !title_available) || self.remote_loading()
        } else {
            self.mod_screen_blocked()
        };
        let decision = match answer {
            Some(Answered {
                answer: ScreenOpening::Cancel,
                ..
            }) => ScreenDecision::Keep,
            Some(Answered {
                answer: ScreenOpening::ModScreen,
                mod_id,
                ..
            }) if !blocked => ScreenDecision::ModScreen(mod_id),
            Some(Answered {
                answer: ScreenOpening::Vanilla(target),
                mod_id,
                ..
            }) => {
                let opens = match title_target_id(target) {
                    // An overlay over the title would stay in front of it unless cleared.
                    Some(_) => title_available && (clears_overlays || !self.screen_over_playing()),
                    None => !title_available && !blocked,
                };
                if opens {
                    ScreenDecision::Vanilla(target)
                } else {
                    log::warn!(
                        "Sunrise: mod {mod_id} answered screen_opening with {target:?}, which can't open here; the screen asked for opens"
                    );
                    ScreenDecision::Open
                }
            }
            _ => ScreenDecision::Open,
        };
        self.side
            .screen_state
            .0
            .answered
            .set(!matches!(decision, ScreenDecision::Open));
        decision
    }
    pub(in crate::game) fn screen_decision_with(
        &self,
        answer: Option<Answered<ScreenOpening>>,
        clears_overlays: bool,
    ) -> ScreenDecision {
        self.screen_decision_for_context(answer, clears_overlays, self.side.title.0.is_some())
    }
    /// Whether a change of screen asks the screen hooks: a mod listens to
    /// `screen_opening` or `screen_closing`, on a client's own `Game` (not a
    /// server's, not on a guest's behalf, not the screen a mod's `Vanilla`
    /// answer opens). Otherwise it is a plain assignment.
    pub(in crate::game) fn screen_hooks_heard(&self) -> bool {
        (mod_api::__private::listened(mod_api::HookId::ScreenOpening)
            || mod_api::__private::listened(mod_api::HookId::ScreenClosing))
            && self.options.execution_role.is_client()
            && !self.side.screen_state.0.replacing
    }
    /// A screen the native mods' input hooks are for is open: the window has a
    /// renderer, either the title's own (`title`, `world_select`,
    /// `world_create`, `multiplayer`…) or a screen over the world
    /// (`Screen::Playing` behind it, so not a book, a sign editor, a server
    /// dialog or a prompt drawn over gameplay), and no connect or world load
    /// is in progress. The one gate they all ask before they fire.
    pub(in crate::game) fn screen_hooks_open(&self) -> bool {
        self.side.renderer.0.is_some()
            && !self.screen_over_playing()
            && !self.remote_loading()
            && (self.side.screen != Screen::Playing || self.side.title.0.is_some())
    }
    /// Whether a Post of the key `key` over the receiving screen `receiver`
    /// cancelled it (`ScreenEvent.KeyPressed.Post` / `KeyReleased.Post`): asked
    /// where the key neither mounted widgets nor screen arms took reaches the
    /// game's own key handling, which a cancel then skips. False with no
    /// subscriber, no receiving screen, or a key with no vanilla name.
    pub(in crate::game::client_side) fn screen_key_post_on(
        &self,
        receiver: Option<&ObservedScreen>,
        key: KeyCode,
        pressed: bool,
        repeat: bool,
    ) -> bool {
        let hook = if pressed {
            mod_api::HookId::ScreenKeyPressedPost
        } else {
            mod_api::HookId::ScreenKeyReleasedPost
        };
        if !mod_api::__private::listened(hook) {
            return false;
        }
        let (Some(name), Some(receiver)) = (keybinds::key_name(key), receiver) else {
            return false;
        };
        let modifiers = super::glfw_modifiers(self.side.modifiers);
        let mut canceled = false;
        let screen = receiver.view();
        if pressed {
            self.native_screen_key_pressed_post(&screen, name, repeat, modifiers, &mut canceled);
        } else {
            self.native_screen_key_released_post(&screen, name, modifiers, &mut canceled);
        }
        canceled
    }
    /// Whether the key press `key` over the receiving screen went no further
    /// (`native_screen_key_pressed_pre`): what `handle_window_event` and the
    /// control socket's `key` command both ask before a screen acts on a key.
    /// False when no receiving screen is open, and for a key with no vanilla name.
    pub(in crate::game::client_side) fn screen_key_pressed_on(
        &self,
        receiver: Option<&ObservedScreen>,
        key: KeyCode,
        repeat: bool,
    ) -> bool {
        let (Some(name), Some(receiver)) = (keybinds::key_name(key), receiver) else {
            return false;
        };
        let mut canceled = false;
        self.native_screen_key_pressed_pre(
            &receiver.view(),
            name,
            repeat,
            super::glfw_modifiers(self.side.modifiers),
            &mut canceled,
        );
        canceled
    }
    pub(in crate::game::client_side) fn screen_key_released_on(
        &self,
        receiver: Option<&ObservedScreen>,
        key: KeyCode,
    ) -> bool {
        let (Some(name), Some(receiver)) = (keybinds::key_name(key), receiver) else {
            return false;
        };
        let mut canceled = false;
        self.native_screen_key_released_pre(
            &receiver.view(),
            name,
            super::glfw_modifiers(self.side.modifiers),
            &mut canceled,
        );
        canceled
    }
    pub(in crate::game::client_side) fn screen_mouse_dragged(
        &self,
        delta: [f32; 2],
    ) -> ScreenMousePre {
        if self.pad_synthetic_input() || delta == [0.0; 2] {
            return ScreenMousePre::NotFired;
        }
        let Some(button) = self
            .side
            .drag_button
            .filter(|b| self.side.physical_buttons.contains(b))
        else {
            return ScreenMousePre::NotFired;
        };
        let Some(receiver) = self.observed_input_screen() else {
            return ScreenMousePre::NotFired;
        };
        let button = super::glfw_button(button);
        let modifiers = super::glfw_modifiers(self.side.modifiers);
        let mut canceled = false;
        self.native_screen_mouse_dragged_pre(
            &receiver.view(),
            button,
            delta,
            modifiers,
            &mut canceled,
        );
        if canceled {
            ScreenMousePre::Canceled
        } else {
            ScreenMousePre::Passed(ScreenMousePost::new(
                receiver,
                modifiers,
                ScreenMousePostKind::Drag { button, delta },
            ))
        }
    }
    pub(in crate::game::client_side) fn screen_overlay_view(
        &self,
        id: &'static str,
    ) -> ScreenView<'_> {
        let rect = self.side.renderer.0.as_ref().map_or([0.0; 4], |renderer| {
            let batch = HudBatch::new(
                [renderer.config.width, renderer.config.height],
                self.settings.gui_scale_inputs(),
            );
            [0.0, 0.0, batch.width, batch.height]
        });
        ScreenView::from(mod_api::__private::built::ScreenView {
            id,
            mod_screen: None,
            menu: None,
            title: None,
            rect,
            slots: 0,
            mouse: self.cursor(),
            partial_tick: self.clock.fraction() as f32,
            regions: &[],
        })
    }
    /// The open screen's container panel in GUI pixels (`screen`'s, the one
    /// this frame draws), or the whole window for a screen with none
    /// (`ScreenView.rect`).
    pub(in crate::game) fn screen_rect(&self, batch: &HudBatch, screen: Screen) -> [f32; 4] {
        if screen != Screen::Inventory && screen != Screen::Creative {
            return [0.0, 0.0, batch.width, batch.height];
        }
        // The window the game itself draws (`container_image`): the merchant
        // and the beacon are wider and shifted, creative is its own size.
        let image = self.container_image(batch);
        [
            image.left as f32,
            image.top as f32,
            image.width as f32,
            image.height as f32,
        ]
    }
    /// Text into a text box a character at a time (`Screen.charTyped`): each
    /// printable one snapshots the current receiving screen, is offered to
    /// `native_screen_char_typed_pre`, and `take` gets it unless cancelled,
    /// before the next is offered. A control character goes straight to `take`.
    pub(in crate::game::client_side) fn screen_text_each(
        &mut self,
        text: &str,
        mut take: impl FnMut(&mut Self, &str),
    ) {
        let mut encoded = [0; 4];
        for c in text.chars() {
            if c.is_control() {
                take(self, c.encode_utf8(&mut encoded));
                continue;
            }
            let receiver = self.observed_input_screen();
            let initial_instance = self.side.screen_state.0.screen_instance;
            if !self.screen_char_canceled_on(receiver.as_ref(), c)
                && self.side.screen_state.0.screen_instance == initial_instance
            {
                take(self, c.encode_utf8(&mut encoded));
            }
        }
    }
    /// The open screen as an input hook sees it (`ScreenView`), or None
    /// when `screen_hooks_open` says no screen is. `screen.mouse` is the
    /// pointer the hook's event is at, so a hook takes no position, and
    /// `partial_tick` is the clock's fraction at this event, not a frame's.
    /// The `HudBatch` built here is only what `screen.rect` needs: the
    /// window's GUI scale, no allocation.
    pub(in crate::game::client_side) fn screen_view_now(&self) -> Option<ScreenView<'_>> {
        if !self.screen_hooks_open() {
            return None;
        }
        let renderer = self.side.renderer.0.as_ref()?;
        let batch = HudBatch::new(
            [renderer.config.width, renderer.config.height],
            self.settings.gui_scale_inputs(),
        );
        if self.side.screen != Screen::Playing {
            return Some(self.screen_view(&batch, self.side.screen));
        }
        self.title_screen_view(&batch)
    }
    /// `set_screen`, with `before` run once the hooks let the change go ahead and
    /// before it applies: what a screen's own `onClose` does only when it really
    /// closes (a container screen's close packet).
    pub(in crate::game) fn set_screen_with(
        &mut self,
        screen: Screen,
        before: impl FnOnce(&mut Self),
    ) {
        let opening_native =
            screen == Screen::Inventory && self.active_native_screen().is_some_and(|s| !s.shown);
        let new_menu_gen = (screen == Screen::Inventory)
            .then(|| self.remote.as_ref().map(|r| r.menu.generation))
            .flatten();
        let menu_replaced = screen == Screen::Inventory
            && new_menu_gen.is_some()
            && self
                .side
                .screen_state
                .0
                .observed
                .as_ref()
                .and_then(|o| o.menu_generation)
                != new_menu_gen;
        if self.side.screen == screen {
            if opening_native || menu_replaced {
                self.apply_screen_with_mod_book(screen, None);
            }
            return;
        }
        let mut screen = screen;
        let mut owner = None;
        if self.screen_hooks_heard() {
            let decision = {
                let current = self.screen_change_view(self.side.screen);
                let new = self.screen_change_view(screen);
                self.decide_screen(&current, &new, self.side.screen != Screen::Playing)
            };
            match decision {
                ScreenDecision::Keep => {
                    // Reject only the opening just staged, never the menu
                    // that an unrelated veto is keeping on screen.
                    if opening_native {
                        self.remote_close_container();
                    }
                    return;
                }
                ScreenDecision::ModScreen(mod_id) => {
                    screen = Screen::Mod;
                    owner = Some(mod_id);
                }
                ScreenDecision::Vanilla(target) => {
                    before(self);
                    if opening_native && self.active_native_screen().is_some() {
                        self.remote_close_container();
                    }
                    self.open_screen_target(target);
                    return;
                }
                ScreenDecision::Open => {}
            }
        }
        if self.side.screen == screen && owner.is_none() {
            return;
        }
        before(self);
        if self.side.screen == Screen::Options {
            self.ui.options.discard_draw();
        }
        self.apply_screen_with_mod_book(screen, owner);
    }
    pub(in crate::game) fn settle_title_mode(&mut self) {
        let heard = self.screen_hooks_heard();
        let gated = heard
            || (self.options.execution_role.is_client()
                && !self.side.screen_state.0.replacing
                && mod_api::__private::listened(mod_api::HookId::ScreenInitPost));
        let Some(title) = self.side.title.0.as_mut() else {
            return;
        };
        title.gated = gated;
        let Some(mode) = title.take_requested() else {
            return;
        };
        let (current, id) = (title.mode.id(), mode.id());
        // What changed before is told first.
        let now = self.observed_now();
        self.observe_change(now);
        let decision = if heard {
            let none = self.screen_change_view(Screen::Playing);
            let from = screen_view_with_id(&none, current);
            let to = screen_view_with_id(&none, id);
            self.decide_screen(&from, &to, true)
        } else {
            ScreenDecision::Open
        };
        match decision {
            ScreenDecision::Keep => (),
            ScreenDecision::ModScreen(mod_id) => self.apply_screen(Screen::Mod, Some(mod_id)),
            ScreenDecision::Vanilla(target) => self.open_screen_target(target),
            ScreenDecision::Open => {
                if let Some(title) = self.side.title.0.as_mut() {
                    title.activate_mode(mode);
                }
                self.release_mappings_on_screen_open();
                self.commit_screen_transition();
            }
        }
    }
    pub(in crate::game::client_side) fn settle_title_toasts(&mut self) {
        use super::toast_manager::SystemToastId;
        let pending = self
            .side
            .title
            .0
            .as_mut()
            .and_then(|title| {
                title.poll_backup();
                title.take_pending_toast()
            })
            .or_else(|| {
                // The launcher's "Playing offline" can sit behind this window: say it here too, once
                // the toast slot is free.
                if self.side.title.0.is_none()
                    || self
                        .side
                        .toast_manager
                        .system_mut(SystemToastId::Title)
                        .is_some()
                {
                    return None;
                }
                let reason = self.options.offline_reason.take()?;
                Some(
                    sunrise_client::toasts::SystemToast::new(
                        sunrise_client::text::Text::tr("sunrise.title.playing_offline", &[]),
                        Some(sunrise_client::text::Text::literal(reason)),
                    )
                    .with_display_time(sunrise_client::toasts::SystemToast::LONG_DISPLAY_TIME),
                )
            });
        if let Some(toast) = pending {
            self.add_system_toast(Some(SystemToastId::Title), toast);
        }
    }
    /// The shown native screen whose remote window is still the active logical menu.
    pub(in crate::game) fn shown_active_native_screen(&self) -> Option<&OpenNativeScreen> {
        self.active_native_screen().filter(|s| {
            s.shown && self.side.screen == Screen::Inventory && self.container == Some(s.menu.key)
        })
    }
    #[cfg(test)]
    pub(in crate::game) fn test_observed_title(&self) -> Option<std::sync::Arc<mod_api::Text>> {
        self.observe_view(self.screen_change_view(self.side.screen))
            .title
    }
    /// The hooks of the title appearing (`to` is its screen) or going
    /// (`None`), from what shows now, and a mod's answer. What changed before
    /// is told first.
    pub(in crate::game) fn title_change(
        &mut self,
        to: Option<&'static str>,
    ) -> Option<Answered<ScreenOpening>> {
        let now = self.observed_now();
        self.observe_change(now);
        let none = self.screen_change_view(Screen::Playing);
        // The title's screen or an overlay; a `Screen` told its own change.
        let from = self
            .side
            .screen_state
            .0
            .observed
            .as_ref()
            .filter(|old| old.overlay);
        let from = match (from, self.side.title.0.as_ref()) {
            (Some(old), _) => Some(old.view()),
            // No renderer yet (startup, a test): the title is still known.
            (None, Some(title)) => Some(screen_view_with_id(&none, title.mode.id())),
            (None, None) => None,
        };
        let to = to.map(|id| screen_view_with_id(&none, id));
        self.screen_change(
            from.as_ref().unwrap_or(&none),
            to.as_ref().unwrap_or(&none),
            true,
            from.is_some(),
        )
    }
    /// The title's own screen as a native mod sees it (`ScreenView`):
    /// its id from the title state machine (`title`, `world_select`,
    /// `world_create`, `multiplayer`, `add_server`…), no menu and no menu
    /// title, the whole window as its rectangle and no slots (no menu is
    /// open). None while no title screen is up.
    pub(in crate::game) fn title_screen_view(&self, batch: &HudBatch) -> Option<ScreenView<'_>> {
        let title = self.side.title.0.as_ref()?;
        Some(ScreenView::from(mod_api::__private::built::ScreenView {
            id: title.mode.id(),
            mod_screen: None,
            menu: None,
            title: None,
            rect: [0.0, 0.0, batch.width, batch.height],
            slots: 0,
            mouse: self.cursor(),
            partial_tick: self.clock.fraction() as f32,
            regions: &[],
        }))
    }
    pub(in crate::game::client_side) fn track_drag_button(
        &mut self,
        button: MouseButton,
        pressed: bool,
    ) {
        // Any release ends the drag, as vanilla's `MouseHandler.onButton` clears `activeButton`:
        // left down, right down, left up sends no drag while right is still held.
        self.side.drag_button = pressed.then_some(button);
    }
    /// Opens `screen` (`Gui.setScreen`; `Screen::Playing` is vanilla's null).
    pub(in crate::game) fn set_screen(&mut self, screen: Screen) {
        self.set_screen_with(screen, |_| {});
    }
    /// Runs `screen_change` from `current` to `new`, then resolves the hooks' decision.
    pub(in crate::game) fn decide_screen(
        &self,
        current: &ScreenView<'_>,
        new: &ScreenView<'_>,
        closes: bool,
    ) -> ScreenDecision {
        self.screen_decision(self.screen_change(current, new, true, closes))
    }
    /// Fires `screen_opening` and `screen_closing` in order for a screen transition.
    pub(in crate::game) fn screen_change(
        &self,
        current: &ScreenView<'_>,
        new: &ScreenView<'_>,
        opens: bool,
        closes: bool,
    ) -> Option<Answered<ScreenOpening>> {
        // Asked before either screen draws: nothing beside it yet.
        let current = &crate::game::screen_backgrounds::without_regions(current);
        let new = &crate::game::screen_backgrounds::without_regions(new);
        let mut outcome = None;
        if opens {
            self.native_screen_opening(current, new, &mut outcome);
        }
        if closes
            && outcome
                .as_ref()
                .is_none_or(|a| a.answer != ScreenOpening::Cancel)
        {
            self.native_screen_closing(current);
        }
        outcome
    }
    /// Shows `title` (`Minecraft.setScreen(new TitleScreen())`) with the game-rule registry attached.
    pub(in crate::game) fn show_title(&mut self, mut title: sunrise_client::title::Title) {
        title.set_rule_registry(Arc::new(crate::game::api_values::game_rule_registry()));
        self.show_title_with_registry(title);
    }
    /// Admission for ordinary toasts; mandatory callback-panic diagnostics bypass mod admission.
    pub(in crate::game) fn admit_toast(
        &self,
        kind: mod_api::ToastKind,
        title: &mut Text,
        description: &mut Option<Text>,
    ) -> bool {
        // Only the receiving client owns toast presentation and its hook.
        if self.is_logical_server() {
            return false;
        }
        let mut cancel = false;
        self.native_toast_add(kind, title, description, &mut cancel);
        !cancel
    }
    /// Client tick target duration in ms (`Minecraft.getTickTargetMillis`), after mod hooks.
    pub(in crate::game) fn tick_target_millis(&self) -> f32 {
        let vanilla = self.clock.tick_target_millis();
        let mut millis = vanilla;
        self.native_tick_target_millis(&mut millis);
        if millis.is_finite() && millis > 0.0 {
            millis
        } else {
            vanilla
        }
    }
    /// Local player's heart kind (`Hud.HeartType.forPlayer`, then `player_heart_type`).
    pub(in crate::game) fn player_heart_kind(
        &mut self,
    ) -> Option<mod_api::client::hud_layout::HeartKind> {
        use mod_api::client::hud_layout::HeartKind;
        let p = self.player.as_ref()?;
        let effects = &self.potions.effects;
        let mut kind = if effects.get("poison").is_some() {
            HeartKind::Poisoned
        } else if effects.get("wither").is_some() {
            HeartKind::Withered
        } else if p.is_fully_frozen() {
            HeartKind::Frozen
        } else {
            HeartKind::Normal
        };
        self.native_player_heart_type(&mut kind);
        Some(kind)
    }
    /// Whether any mod listens to a hook that receives `ScreenView`.
    pub(in crate::game) fn screen_view_listened(&self) -> bool {
        use mod_api::HookId;
        [
            HookId::ScreenRenderPre,
            HookId::ScreenRenderBackground,
            HookId::ScreenRenderForeground,
            HookId::ScreenRenderPost,
            HookId::ScreenMouseButtonPressedPre,
            HookId::ScreenMouseButtonPressedPost,
            HookId::ScreenMouseButtonReleasedPre,
            HookId::ScreenMouseButtonReleasedPost,
            HookId::ScreenMouseDraggedPre,
            HookId::ScreenMouseDraggedPost,
            HookId::ScreenMouseScrolledPre,
            HookId::ScreenMouseScrolledPost,
            HookId::ScreenKeyPressedPre,
            HookId::ScreenKeyPressedPost,
            HookId::ScreenKeyReleasedPre,
            HookId::ScreenKeyReleasedPost,
            HookId::ScreenCharTypedPre,
            HookId::ScreenCharTypedPost,
            HookId::ScreenOpening,
            HookId::ScreenClosing,
            HookId::ScreenInitPost,
            HookId::ScreenRoom,
            HookId::ScreenExclusionAreas,
        ]
        .into_iter()
        .any(mod_api::__private::listened)
    }
    /// Whether a mod listens to one of the four screen render hooks.
    pub(in crate::game::client_side) fn screen_render_listened(&self) -> bool {
        use mod_api::HookId;
        [
            HookId::ScreenRenderPre,
            HookId::ScreenRenderBackground,
            HookId::ScreenRenderForeground,
            HookId::ScreenRenderPost,
        ]
        .into_iter()
        .any(mod_api::__private::listened)
    }
    /// `Render.Pre`: whether the screen draws.
    pub(in crate::game::client_side) fn screen_render_begin(
        &self,
        draw: &mut HudBatch,
        screen: &ScreenView<'_>,
    ) -> bool {
        let mut canceled = false;
        self.native_screen_render_pre(draw, screen, &mut canceled);
        !canceled
    }
    /// `Render.Post`, including when Pre cancelled the screen's drawing.
    pub(in crate::game::client_side) fn screen_render_end(
        &self,
        draw: &mut HudBatch,
        screen: &ScreenView<'_>,
    ) {
        self.native_screen_render_post(draw, screen);
    }
    /// Borrowed `ScreenView` of the open screen for this frame.
    pub(in crate::game) fn screen_view(&self, batch: &HudBatch, screen: Screen) -> ScreenView<'_> {
        self.screen_view_at(screen, self.screen_rect(batch, screen))
    }
    /// `screen_view` with an explicit panel `rect` for container layout.
    pub(in crate::game) fn screen_view_at(&self, screen: Screen, rect: [f32; 4]) -> ScreenView<'_> {
        // Only the inventory screen shows a server's menu.
        let menu_shown = screen == Screen::Inventory;
        let shown_native = menu_shown.then(|| self.shown_native_screen()).flatten();
        ScreenView::from(mod_api::__private::built::ScreenView {
            id: screen.id(),
            mod_screen: (screen == Screen::Mod).then(|| self.mod_screen()).flatten(),
            menu: self
                .remote_menu_type()
                .filter(|m| menu_shown && !m.is_empty())
                .or_else(|| {
                    shown_native
                        .map(|s| s.menu.name.as_str())
                        .filter(|m| !m.is_empty())
                }),
            title: self
                .remote
                .as_ref()
                .and_then(|r| r.menu_title_snapshot())
                .filter(|_| menu_shown)
                .or_else(|| shown_native.and_then(|s| s.menu.title.as_ref())),
            rect,
            slots: self.slot_count(),
            mouse: self.cursor(),
            partial_tick: self.clock.fraction() as f32,
            regions: self.screen_regions(screen),
        })
    }
    /// Button Post sees the original receiver and its answer; return the final handled bit.
    pub(in crate::game) fn screen_button_post(
        &self,
        receiver: &ObservedScreen,
        button: i32,
        pressed: bool,
        modifiers: u32,
        double_click: bool,
        handled: bool,
    ) -> bool {
        let screen = receiver.view();
        let mut result = TriState::Default;
        if pressed {
            self.native_screen_mouse_button_pressed_post(
                &screen,
                button,
                modifiers,
                double_click,
                handled,
                &mut result,
            );
        } else {
            self.native_screen_mouse_button_released_post(
                &screen,
                button,
                modifiers,
                handled,
                &mut result,
            );
        }
        result.or(handled)
    }
    /// Unhandled scroll/drag notices go to the receiver captured before handling.
    pub(in crate::game) fn screen_mouse_post(&self, post: ScreenMousePost) {
        if post.handled {
            return;
        }
        let screen = post.receiver.view();
        match post.kind {
            ScreenMousePostKind::Scroll(scroll) => {
                self.native_screen_mouse_scrolled_post(&screen, scroll, post.modifiers);
            }
            ScreenMousePostKind::Drag { button, delta } => {
                self.native_screen_mouse_dragged_post(&screen, button, delta, post.modifiers);
            }
        }
    }
    /// Next-frame position of container slot `slot` on open mod menu `menu`.
    pub(in crate::game) fn screen_slot(&self, menu: &str, slot: usize) -> Option<[f32; 2]> {
        let s = self.shown_native_screen().filter(|s| s.menu.name == menu)?;
        s.slot_moves.latest(&s.layout().slots, slot)
    }
    /// Moves, activates, or hides container slot `slot` on open mod menu `menu` from next frame.
    pub(in crate::game) fn set_screen_slot(
        &self,
        menu: &str,
        slot: usize,
        pos: Option<[f32; 2]>,
    ) -> Result<(), mod_api::ScreenSlotRefusal> {
        let s = self
            .shown_native_screen()
            .filter(|s| s.menu.name == menu)
            .ok_or(mod_api::ScreenSlotRefusal::NoScreen)?;
        s.slot_moves.set(&s.layout().slots, slot, pos)
    }
    /// Options > Mods rows for loaded code mods, after the content-only rows.
    pub(in crate::game) fn list_native_mods(&mut self) {
        let registered = crate::game::mods::collect_key_mappings(
            self.options.execution_role,
            mod_api::__private::key_mappings(),
        );
        let native = self.mods.native.clone();
        let mut rows: Vec<_> = self
            .settings
            .mods
            .installed
            .iter()
            .filter(|row| {
                mod_api::__private::mod_index(&row.id).is_some()
                    && !native.built().iter().any(|m| m.id == row.id)
            })
            .cloned()
            .collect();
        rows.extend(native.packages().iter().map(|package| {
            let m = &package.manifest;
            self.settings
                .mods
                .info
                .insert(m.id.clone(), sunrise_client::mod_list::ModInfo::of(m, None));
            Installed {
                id: m.id.clone(),
                title: format!("{} {}", m.name, m.version),
                description: m.description.clone(),
                problem: None,
                dev: false,
                native: None,
            }
        }));
        self.settings
            .mods
            .installed
            .retain(|row| !rows.iter().any(|code| code.id == row.id));
        for mut row in rows {
            let settings = self.native_page_settings(&row.id);
            let mappings: Vec<KeyMapping> = registered
                .iter()
                .filter(|(owner, _)| *owner == row.id)
                .map(|(_, k)| KeyMapping {
                    id: k.id.clone(),
                    label: k.label.clone(),
                    key: k.key.clone(),
                    modifiers: k.modifiers.bits,
                    context: crate::game::api_values::TryIntoEngine::try_into_engine(
                        &k.context,
                        &(),
                    )
                    .unwrap(),
                    category: k.category.clone(),
                })
                .collect();
            row.native = Some(NativePage { mappings, settings });
            self.settings.mods.installed.push(row);
        }
    }
    /// The settings mod `mod_id` registered, with their kept values, as its page shows them.
    pub(in crate::game) fn native_page_settings(&self, mod_id: &str) -> Vec<NativeSetting> {
        mod_api::__private::settings()
            .iter()
            .filter(|(owner, _)| *owner == mod_id)
            .map(|(_, spec)| {
                let kept = self.stored_setting(mod_id, &spec.key);
                let value = match (&spec.kind, spec.read(kept.as_deref())) {
                    (_, mod_api::SettingValue::Toggle(on)) => NativeSettingValue::Toggle(on),
                    (
                        mod_api::SettingKind::Choice { options, .. },
                        mod_api::SettingValue::Choice(index),
                    ) => NativeSettingValue::Choice {
                        options: options.clone(),
                        index,
                    },
                    (
                        mod_api::SettingKind::Range { min, max, step, .. },
                        mod_api::SettingValue::Range(value),
                    ) => NativeSettingValue::Range {
                        min: *min,
                        max: *max,
                        step: *step,
                        value,
                    },
                    (_, mod_api::SettingValue::Color(rgb)) => NativeSettingValue::Color(rgb),
                    _ => NativeSettingValue::Toggle(false),
                };
                NativeSetting {
                    key: spec.key.clone(),
                    label: spec.label.clone(),
                    tooltip: spec.tooltip.clone(),
                    value,
                }
            })
            .collect()
    }
    /// Persists a changed setting on native mod `i`'s page and fires `setting_changed`.
    pub(in crate::game::client_side) fn native_setting_event(&mut self, i: usize, j: usize) {
        let Some(row) = self.settings.mods.installed.get(i) else {
            return;
        };
        let Some(setting) = row.native.as_ref().and_then(|page| page.settings.get(j)) else {
            return;
        };
        let (id, key) = (row.id.clone(), setting.key.clone());
        let value = match &setting.value {
            NativeSettingValue::Toggle(on) => mod_api::SettingValue::Toggle(*on),
            NativeSettingValue::Choice { index, .. } => mod_api::SettingValue::Choice(*index),
            NativeSettingValue::Range { value, .. } => mod_api::SettingValue::Range(*value),
            NativeSettingValue::Color(rgb) => mod_api::SettingValue::Color(*rgb),
        };
        let kept = mod_api::__private::setting(&id, &key).and_then(|spec| spec.store(value));
        let Some(kept) = kept else {
            return;
        };
        if self.store_setting(&id, &key, &kept) {
            self.native_setting_changed(&id, &key);
        }
    }
    /// Native mod `i`'s page opened: its settings' kept values read again.
    pub(in crate::game::client_side) fn native_settings_refresh(&mut self, i: usize) {
        let Some(id) = self.settings.mods.installed.get(i).map(|m| m.id.clone()) else {
            return;
        };
        let settings = self.native_page_settings(&id);
        let row = self.settings.mods.installed.get_mut(i);
        if let Some(page) = row.and_then(|m| m.native.as_mut()) {
            page.settings = settings;
        }
    }
    /// Shader-pack params from native mods this frame, with non-finite values zeroed.
    pub(in crate::game) fn native_shader_params_now(&self, partial: f32) -> [[f32; 4]; 8] {
        let mut params = [[0.0; 4]; 8];
        self.native_shader_params(partial, &mut params);
        finite_params(params)
    }
    /// A client mod's command (the `command` hook); a server never asks.
    pub(in crate::game) fn client_mod_command(&mut self, command: &str) -> bool {
        if self.is_logical_server() {
            return false;
        }
        let mut ran = false;
        self.native_command(command, &mut ran);
        ran
    }
    /// Validated category and icon of a custom recipe type, or None before load or outside a world.
    pub(in crate::game) fn recipe_book_category(
        &mut self,
        kind: &str,
    ) -> Option<RecipeBookCategory> {
        if self.outside_world() || self.res.rest.get().is_none() {
            return None;
        }
        self.registered_recipe_book_category(kind)
    }
}

/// A mod's camera (`native_camera_pose`): each non-finite component keeps vanilla's `view`.
pub(in crate::game) fn posed_camera(
    view: (glam::DVec3, f32, f32),
    position: [f64; 3],
    yaw: f32,
    pitch: f32,
) -> (glam::DVec3, f32, f32) {
    let position = std::array::from_fn(|i| {
        if position[i].is_finite() {
            position[i]
        } else {
            view.0[i]
        }
    });
    (
        glam::DVec3::from_array(position),
        if yaw.is_finite() { yaw } else { view.1 },
        if pitch.is_finite() { pitch } else { view.2 },
    )
}

#[cfg(test)]
impl Game<'_, crate::game::client_side::ClientState> {
    pub(in crate::game) fn test_title(&self) -> Option<&sunrise_client::title::Title> {
        self.side.title.0.as_ref()
    }
    pub(in crate::game) fn test_title_mut(&mut self) -> &mut Option<sunrise_client::title::Title> {
        &mut self.side.title.0
    }
    pub(in crate::game) fn test_renderer(&self) -> Option<&crate::game::Renderer> {
        self.side.renderer.0.as_deref()
    }
    pub(in crate::game) fn test_renderer_mut(&mut self) -> &mut Option<Box<crate::game::Renderer>> {
        &mut self.side.renderer.0
    }
    pub(in crate::game) fn test_window_mut(
        &mut self,
    ) -> &mut Option<std::sync::Arc<winit::window::Window>> {
        &mut self.side.window.0
    }
    pub(in crate::game) fn test_pad(&self) -> &crate::game::pad::Pads {
        &self.side.pad.0
    }
}

#[cfg(test)]
#[path = "animate_tick_tests_client.rs"]
pub(in crate::game) mod animate_tick_tests;

#[cfg(test)]
#[path = "block_entity_client_tick_tests_client.rs"]
pub(in crate::game) mod block_entity_client_tick_tests;

#[cfg(test)]
#[path = "block_particle_effects_tests_client.rs"]
pub(in crate::game) mod block_particle_effects_tests;

#[cfg(test)]
#[path = "shared/particle_count_tests_client.rs"]
pub(in crate::game) mod particle_count_tests;

#[cfg(test)]
#[path = "shared/label_tests_client.rs"]
pub(in crate::game) mod label_tests;

#[cfg(test)]
#[path = "shared/runtime_environment_tests_client.rs"]
pub(in crate::game) mod runtime_environment_tests;

#[cfg(test)]
#[path = "shared/screen_position_tests_client.rs"]
pub(in crate::game) mod screen_position_tests;

#[cfg(test)]
#[path = "shared/slot_icon_tests_client.rs"]
pub(in crate::game) mod slot_icon_tests;

#[cfg(test)]
#[path = "shared/slot_tooltip_tests_client.rs"]
pub(in crate::game) mod slot_tooltip_tests;
