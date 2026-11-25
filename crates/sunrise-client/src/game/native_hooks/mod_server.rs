#[allow(unused_imports)]
pub(in crate::game::server_side) use super::native_data_maps::DataMapType;
#[allow(unused_imports)]
pub(in crate::game::server_side) use super::screen_build::ObservedScreen;
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
pub(in crate::game::server_side) use crate::game::api_values::{TryIntoApi, TryIntoEngine};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::BLOCK_CONTACT_HOOKS;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::BlockClickAnswer;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::BonemealAsk;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::ContentsChangedGuard;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::DAMAGE_HOOKS;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::HitLevel;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::LiveDamageContainer;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::LiveDamageRead;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::NativeItem;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::NativeItemEntity;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::NativePlayer;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::NativeRecipe;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::NativeRecipeKind;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::NativeRecipeLayout;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::NativeRecipeResult;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::NativeRecipeStack;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::NativeRecipeView;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::NativeRecipes;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::OpenNativeMenu;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::OpenNativeScreen;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::SHAKE_MAX_DEGREES;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::SHAKE_MAX_OFFSET;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::ScreenDecision;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::ScreenMousePost;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::ScreenMousePostKind;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::ScreenMousePre;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::SlotMoves;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::add_attribute_tooltips::HookedAttributeTooltips;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::apply_menu_button_cursor;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::base::LevelView;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::block_entity_tick::ResidentBlockEntityAccess;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::block_use_result;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::bounce_restitution::install_block_contact_hooks;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::box_meets;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::build_creative_mode_tab_contents::CreativeCache;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::camera_pose::bounded_shake;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::camera_pose::hand_pose_matrix;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::client_packet_view;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::client_pick::ClientPick;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::config_dir;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::damage_post;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::damage_pre;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::debug_takes;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::dimension_id;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::engine_reduction;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::fire::{
    ConnectsToCall, EntityChangeBlockPreCall, HotbarAttackIndicatorProgressCall,
    ItemAfterVerticalCollisionCall, PlayerPongCall, PreRemoveSideEffectsCall, UpdateShapeCall,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::incoming_damage;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::invulnerability_check;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::knockback;
#[allow(unused_imports)]
#[cfg(test)]
pub(in crate::game::server_side) use crate::game::native_hooks::level_helpers::block_entity_for_block;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::living_change_target::NativeHookHost;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::living_death::hurt_server_args;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::mob_kind;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::mod_menu;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::mod_particles_ok;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::mod_screen_opens;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::observed_overlay_id;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::packet_types;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::packet_view;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::posed_camera;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::property_change;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::screen_chars::chars_to_type;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::screen_view::{
    screen_view_for_mod, screen_view_with_id,
};
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::seed_native_menu;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::serverbound_packet_view;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::shake_pose;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::shield_block;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::sound_and_shader_helpers::{
    finite, finite_params, local_sound_ok,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::sound_and_shader_helpers::{
    level_sound_playable, mod_sound_ok, sound_id,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::title_target_id;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::use_totem;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::with_property;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::*;
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
pub(in crate::game::server_side) use mod_api::ModBlockEntity;
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
    BlockEntityDataSource, BlockEntityRef, ClockAdjustment,
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
pub(in crate::game::server_side) use sunrise_client::containers::ModBlockEntity as EngineBlockEntity;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::health::mob_source;
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
pub(in crate::game::server_side) use sunrise_world::level::flags;
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

#[path = "hud_mode_server.rs"]
pub(in crate::game) mod hud_mode;

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
    /// A client mod's command (the `command` hook); a server never asks.
    pub(in crate::game) fn client_mod_command(&mut self, command: &str) -> bool {
        if self.server.lan_acting.is_some() {
            return false;
        }
        let mut ran = false;
        self.native_command(command, &mut ran);
        ran
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
    /// The player clicking, as the block-use hooks name them: the acting
    /// player's UUID and entity id on a server, this client's own player on
    /// a client.
    pub(in crate::game) fn native_clicker(&mut self) -> (u128, i32) {
        if self.remote.is_some() {
            return (self.own_uuid(), self.own_entity_id());
        }
        let entity = self
            .server
            .lan_acting_entity
            .map_or_else(|| self.own_entity_id(), |id| id as i32);
        (self.native_acting_id(), entity)
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
