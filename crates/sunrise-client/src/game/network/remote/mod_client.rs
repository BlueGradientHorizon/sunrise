use super::raw_backlog::Received;
use super::remote_chunks::{ChunkDecoder, ChunkFormat, Next, RemoteLight};
use super::render_tick::{mob_tracking_body, player_tracking_body};
use super::villager_client::builtin_registries;
use super::{Dim, Game, Hit, Mining, Screen};
use crate::game::collision_source::Mover;
use crate::game::remote::DryChanges;
use crate::game::remote::DryPlace;
use crate::game::remote::DrySound;
use crate::game::remote::DryUse;
use crate::game::remote::Lerp;
use crate::game::remote::MOUNT_MENU;
use crate::game::remote::SHIFT_INPUT;
use crate::game::remote::boat_interact_passes;
use crate::game::remote::chat_after_event;
use crate::game::remote::chat_received::{QueuedChat, QueuedChatKind};
use crate::game::remote::client_fluid_step;
use crate::game::remote::client_view_distance;
use crate::game::remote::dry_level::DryLevel;
use crate::game::remote::entity_sound_state;
use crate::game::remote::face_direction;
use crate::game::remote::hand_id;
use crate::game::remote::hand_operated;
use crate::game::remote::hold_server_fire;
use crate::game::remote::input_ordinal;
use crate::game::remote::inventory::{
    CREATIVE_SYNCED, Predicted, changed_slots, creative_sync_slot, creative_sync_stacks,
    mark_synced, menu_items, predict_clicks, predict_clicks_with,
};
use crate::game::remote::item_frame_interact_passes;
use crate::game::remote::keep_client_state;
use crate::game::remote::look_at_rotation;
use crate::game::remote::menu_title;
use crate::game::remote::minecarts::CartLerp;
use crate::game::remote::mob_pos_rot;
use crate::game::remote::movement::MovePacket;
use crate::game::remote::number_format;
use crate::game::remote::object_skin_changed;
use crate::game::remote::place_interpolated;
use crate::game::remote::player_voice_pitch;
use crate::game::remote::position_packet_pitch;
use crate::game::remote::remote_object_swings;
use crate::game::remote::remote_player_head_turn;
use crate::game::remote::remote_text;
use crate::game::remote::remote_use_swings;
use crate::game::remote::selector_metadata::hurt_sound_for;
use crate::game::remote::shape_meets;
use crate::game::remote::snap_target;
use crate::game::remote::synced_local_use;
use crate::game::remote::use_ends_click;
use crate::game::remote::wrap_degrees;
use crate::game::remote::*;
use crate::game::{item_use_common::ticks_left, level_sounds_common::LevelSound};
use mod_api::InteractionKey;
pub(in crate::game) use session_state::{
    BACKLOG_LIMIT, CHUNK_PACKETS_PER_FRAME, ClientMenu, ItemUseAim, NoTerrain, OwnProfile,
    PendingCauldron, PendingItem, PlayerTextures, Remote, RemoteDimensionEntry, ServicesKeys,
    TICK_DECODE_WAIT, Tracked, client_millis, seat_passengers, skin_entries,
};
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::{Duration, Instant},
};
#[allow(unused_imports)]
use sunrise_client::{
    hud::HudBatch, particles::TrackingTarget, screen_effects, streaming::MeshStreamer,
};
use sunrise_client_lib::net::{Event, Session};
use sunrise_core::{geometry::Vec3, profile::AccountProperty, random::RandomSource};
#[allow(unused_imports)]
use sunrise_game_data::{Result, text::Text};
#[allow(unused_imports)]
use sunrise_items::{blocks::Fluid, items::ItemStack};
#[allow(unused_imports)]
use sunrise_level::{
    chunk::Chunk,
    containers::{BlockEntity, Furnace, FurnaceKind},
    profile_lookup::SPECTATOR,
    random_source,
};
use sunrise_protocol::{
    entity_data::{ITEM_ENTITY_ITEM, Value},
    item::{Hashed, Stack},
    packets::{
        Empty,
        play::{self, Clientbound as C, Serverbound as S},
    },
    registry::{DimensionType, Dynamic, SyncedNames, ceil_log2},
    secure_chat::{self, Signature},
    text::plain,
};
use sunrise_server_files::profiles::ServicesKeySet;
use sunrise_sim::first_person_sim::{Hand, SwingAnimation};
#[allow(unused_imports)]
use sunrise_sim::{
    entities::{Experience, ExperienceOrb, ItemEntity},
    join_seq::JoinSeq,
    lan::data::{BIOME_REGISTRY, client_biomes},
    menu::Input,
    mobs::Mob,
    motion::{MotionWorld, PlayerMotion},
    sprint::Keys,
};
use {
    sunrise_core::nbt::Tag,
    sunrise_mod_runtime::attachments::Holder,
    sunrise_world::{
        level::InteractionResult,
        registry::BlockStateId,
        stepped_interpolation::{Path, PosRot, Start, SteppedInterpolation},
    },
};

#[path = "selector_metadata_client.rs"]
pub(in crate::game) mod selector_metadata;

#[path = "attack_target_client.rs"]
pub(in crate::game) mod attack_target;

#[path = "break_prediction_client.rs"]
pub(in crate::game) mod break_prediction;

#[path = "connection_client.rs"]
pub(in crate::game) mod connection;

#[path = "fall_sound_client.rs"]
pub(in crate::game) mod fall_sound;

#[path = "general_client.rs"]
pub(in crate::game) mod general;

#[path = "interaction_actions_client.rs"]
pub(in crate::game) mod interaction_actions;

#[path = "interaction_prediction_client.rs"]
pub(in crate::game) mod interaction_prediction;

#[path = "level_format_client.rs"]
pub(in crate::game) mod level_format;

#[path = "raw_ingress_client.rs"]
pub(in crate::game) mod raw_ingress;

#[path = "recipe_properties_client.rs"]
pub(in crate::game) mod recipe_properties;

#[path = "use_item_client.rs"]
pub(in crate::game) mod use_item;

#[path = "block_entity_data_client.rs"]
pub(in crate::game) mod block_entity_data;

#[path = "abilities_client.rs"]
pub(in crate::game) mod abilities;

#[path = "chat_client.rs"]
pub(in crate::game) mod chat;

#[path = "chat_received_client.rs"]
pub(in crate::game) mod chat_received;

#[path = "commands_client.rs"]
pub(in crate::game) mod commands;

#[path = "client_level_tick_client.rs"]
pub(in crate::game) mod client_level_tick;

#[path = "effect_client.rs"]
pub(in crate::game) mod effect;

#[path = "entity_section_steps_client.rs"]
pub(in crate::game) mod entity_section_steps;

#[path = "inventory_client.rs"]
pub(in crate::game) mod inventory;

#[path = "item_use_client.rs"]
pub(in crate::game) mod item_use;

#[path = "kept_columns_client.rs"]
pub(in crate::game) mod kept_columns;

#[path = "menu_interactions_client.rs"]
pub(in crate::game) mod menu_interactions;

#[path = "minecarts_client.rs"]
pub(in crate::game) mod minecarts;

#[path = "motion_client.rs"]
pub(in crate::game) mod motion;

#[path = "movement_client.rs"]
pub(in crate::game) mod movement;

#[path = "packets_entity_client.rs"]
pub(in crate::game) mod packets_entity;

#[path = "packets_object_client.rs"]
pub(in crate::game) mod packets_object;

#[path = "packets_player_client.rs"]
pub(in crate::game) mod packets_player;

#[path = "packets_world_client.rs"]
pub(in crate::game) mod packets_world;

#[path = "remote_state_client.rs"]
pub(in crate::game) mod remote_state;

#[path = "session_client.rs"]
pub(in crate::game) mod session;

#[path = "skin_client.rs"]
pub(in crate::game) mod skin;

#[path = "status_client.rs"]
pub(in crate::game) mod status;

#[path = "sync_client.rs"]
pub(in crate::game) mod sync;

#[path = "tables_client.rs"]
pub(in crate::game) mod tables;

#[path = "tick_client.rs"]
pub(in crate::game) mod tick;

#[path = "tracking_client.rs"]
pub(in crate::game) mod tracking;

#[path = "ui_client.rs"]
pub(in crate::game) mod ui;

#[path = "use_client.rs"]
pub(in crate::game) mod r#use;

#[path = "chat_queue_client.rs"]
pub(in crate::game) mod chat_queue;

#[path = "session_state.rs"]
pub(in crate::game) mod session_state;

#[path = "break_prediction.rs"]
mod prediction_state;

#[path = "level_format.rs"]
mod format_state;

#[path = "raw_ingress.rs"]
pub(in crate::game) mod ingress_state;

#[cfg(feature = "client")]
#[path = "remote_attributes.rs"]
mod remote_attributes;

#[path = "remote_state.rs"]
mod state_reads;

#[path = "session.rs"]
mod opening;

#[path = "sync.rs"]
mod sync_reads;

#[path = "tab_entries.rs"]
mod tab_entries;

#[cfg(feature = "client")]
impl sunrise_client::interaction_input::InteractionWorld for RemoteInputWorld<'_, '_> {
    fn pick_hit_result(&mut self) {
        self.0.pick_tick_targets();
    }
    fn handle_keys(&mut self) {
        self.0.handle_play_keybinds();
    }
    fn pending_clicks(&self, button: sunrise_client::interaction_input::Button) -> u32 {
        self.0.pending_mapping_clicks(button.mapping())
    }
    fn consume_click(&mut self, button: sunrise_client::interaction_input::Button) -> bool {
        self.0.consume_mapping_click(button.mapping())
    }
    fn is_down(&self, button: sunrise_client::interaction_input::Button) -> bool {
        self.0.mapped_key_down(button.mapping())
    }
    fn can_continue_attack(&self) -> bool {
        !self.0.mod_mappings_screen_open()
    }
    fn spectator_hotbar(&mut self) {
        while self.0.own_player_is_spectator()
            && self.0.consume_mapping_click("key.spectatorHotbar")
        {
            self.0.spectator_action_key();
        }
    }
    fn release_hit_result(&mut self) {
        self.0.release_tick_targets();
    }
    fn using_item(&self) -> bool {
        self.0.side.client_item_use.0.is_some() || self.0.using.is_some()
    }
    fn release_using(&mut self) {
        if !self.0.end_client_item_use(true) && self.0.using.is_some() {
            self.0.release_remote_use();
        }
    }
    fn start_attack(&mut self) -> bool {
        self.0.remote_start_attack()
    }
    fn start_use(&mut self, held: bool) {
        let attempt = if held {
            mod_api::InteractionAttempt::Held
        } else {
            mod_api::InteractionAttempt::Press
        };
        self.0.remote_use(attempt);
    }
    fn pick(&mut self) {
        // Minecraft.pickBlockOrEntity: an entity before the block is asked
        // for (`handlePickItemFromEntity`), else the block.
        if let Some(hit) = self.0.crosshair_entity_hit() {
            if self
                .0
                .native_interaction(
                    InteractionKey::PickBlock,
                    Hand::Main,
                    mod_api::InteractionAttempt::Press,
                )
                .canceled
            {
                return;
            }
            self.0.send(S::PickItemFromEntity(play::PickItemFromEntity {
                id: hit.id,
                include_data: self.0.has_control_down(),
            }));
        } else if let Some(hit) = self.0.pick() {
            if self
                .0
                .native_interaction(
                    InteractionKey::PickBlock,
                    Hand::Main,
                    mod_api::InteractionAttempt::Press,
                )
                .canceled
            {
                return;
            }
            self.0.send(S::PickItemFromBlock(play::PickItemFromBlock {
                pos: hit.pos,
                include_data: self.0.has_control_down(),
            }));
        }
    }
    fn use_delay(&self) -> u32 {
        self.0.use_delay
    }
    fn continue_attack(&mut self, down: bool) {
        self.0.remote_continue_attack(down);
    }
}

#[cfg(feature = "client")]
impl Game<'_, crate::game::client_side::ClientState> {
    pub(in crate::game) fn apply_packet(&mut self, packet: C) -> Result<()> {
        self.observe_client_packet(&packet);
        match packet {
            // ClientCommonPacketListenerImpl.handlePing runs on the game
            // thread, after all preceding packets (especially teleports).
            C::Ping(p) => self.send(S::Pong(p)),
            C::CustomPayload(p)
                if p.channel == crate::game::cauldron_prediction::ENTRIES_CHANNEL =>
            {
                let entries = crate::game::cauldron_prediction::decode_registrations(&p.data)?;
                if let Some(remote) = &mut self.side.remote {
                    remote.cauldron_entries = Some(entries);
                }
            }
            C::CustomPayload(p) if p.channel == crate::game::cauldron_prediction::USE_CHANNEL => {
                self.receive_cauldron_use(&p.data)?;
            }
            // The server's next generation of mod registries: all of it or the connection.
            C::CustomPayload(p) if p.channel == sunrise_sim::datapack_net::CHANNEL => {
                sunrise_sim::datapack_net::accept_chunk(&p.data)?;
            }
            C::CustomPayload(p)
                if p.channel == sunrise_sim::lan::commands::selector_sync::CHANNEL =>
            {
                self.receive_selector_tokens(&p.data)?;
            }
            // Another level's world: its open, its packets and its close (`viewed_worlds`).
            C::CustomPayload(p) if p.channel == sunrise_protocol::viewed_levels::CHANNEL => {
                self.viewed_levels_payload(&p.data)?;
            }
            // Private custom recipe-book settings and placement protocol.
            C::CustomPayload(p)
                if p.channel == sunrise_sim::recipe_book::settings_sync::CHANNEL =>
            {
                self.remote_book_settings(&p.data);
            }
            // A native mod's general row the server starts showing the window.
            C::CustomPayload(p) if p.channel == sunrise_protocol::general_entity::CHANNEL => {
                self.remote_general_entity(&p.data);
            }
            // The collider boxes of an entity the server shows the window.
            C::CustomPayload(p) if p.channel == sunrise_protocol::collider_sync::CHANNEL => {
                self.remote_collider_sync(&p.data);
            }
            // Per-position connected-texture groups synced for the window's chunks.
            C::CustomPayload(p) if p.channel == sunrise_protocol::connection_groups::CHANNEL => {
                self.refuse_play_payload("connection groups: payload outside its bundle")?;
            }
            // Full-width data slots for the open blockless mod menu.
            C::CustomPayload(p) if p.channel == sunrise_protocol::menu_extra::DATA_CHANNEL => {
                use sunrise_protocol::menu_extra::{DataSlot, LIVE_DATA_REVISION};
                if self
                    .side
                    .remote
                    .as_ref()
                    .is_some_and(|r| r.registries.menu_extra_revision >= LIVE_DATA_REVISION)
                    && let Ok(slot) = DataSlot::decode(&p.data)
                {
                    self.native_screen_data(slot.window, usize::from(slot.index), slot.value);
                }
            }
            // A mod menu type's opening context, just before its OpenScreen.
            C::CustomPayload(p) if p.channel == sunrise_protocol::menu_extra::CHANNEL => {
                self.receive_menu_extra(&p.data)?;
            }
            C::CustomPayload(p) if sunrise_sim::lan::recipes::viewer_channel(&p.channel) => {
                self.remote_recipe_viewer(&p.channel, &p.data);
            }
            // An anvil cost the data-slot short cannot hold, after that short.
            C::CustomPayload(p) if p.channel == sunrise_protocol::anvil_cost::CHANNEL => {
                self.remote_anvil_wide_cost(&p.data);
            }
            C::CustomPayload(p)
                if p.channel == sunrise_sim::lan::data::OPEN_COMMAND_BLOCK_CHANNEL =>
            {
                self.remote_open_command_block(&p.data);
            }
            // The server's recipes its native mods asked to send.
            C::CustomPayload(p) if p.channel == sunrise_sim::lan::recipes::CONTENT_CHANNEL => {
                self.remote_recipe_content(&p.data);
            }
            // The passages of the chunks the server shows the window.
            C::CustomPayload(p) if p.channel == sunrise_protocol::passage_sync::CHANNEL => {
                self.remote_passage_sync(&p.data);
            }
            // PingDebugMonitor.onPongReceived.
            C::PongResponse(p) => {
                let delay = sunrise_client_lib::net::ping_millis().wrapping_sub(p.time);
                self.side.debug_charts.ping.log(delay);
            }
            C::DebugSample(p) => self.side.debug_charts.log_remote_sample(&p.sample)?,
            C::Login(p) => self.apply_login_packet(p),
            C::Respawn(p) => self.apply_respawn_packet(p),
            C::PlayerPosition(p) => self.remote_teleport(&p)?,
            C::PlayerRotation(p) => self.remote_rotation(&p),
            C::PlayerLookAt(p) => self.remote_look_at(&p),
            // ClientCommonPacketListenerImpl.handlePostEffects.
            C::PostEffects(p) => self.apply_post_effects_packet(p)?,
            C::SetCamera(p) => self.remote_set_camera(p.camera_id),
            C::LevelChunkWithLight(p) => self.remote_chunk(&p, None),
            C::LightUpdate(p) => self.remote_light_update(&p),
            C::ChunksBiomes(p) => self.remote_chunks_biomes(&p),
            C::ForgetLevelChunk(p) => self.apply_forget_level_chunk_packet(p),
            C::BlockUpdate(p) => self.server_block(p.pos, p.state, None),
            C::AddTransientBlock(p) => self.add_transient_block(p.pos, p.state),
            C::BlockDestruction(p) => self.apply_block_destruction_packet(p),
            C::SectionBlocksUpdate(p) => {
                self.apply_section_blocks_update_packet(&p, Default::default())
            }
            C::BlockChangedAck(p) => self.end_predictions(p.sequence)?,
            C::BlockEvent(p) => self.apply_block_event_packet(p)?,
            // ClientLevel.levelEvent: LevelEventHandler, as this client's own
            // level plays them (particles and sounds: a spawner's burst, bone
            // meal, waxing, a block broken by someone else).
            // ClientPacketListener.handleLevelEvent: a global one is heard
            // from anywhere (ClientLevel.globalLevelEvent).
            C::LevelEvent(p) if p.global => self.global_level_event_sound(p.kind, p.pos),
            C::LevelEvent(p) => self.client_level_event_at(p.kind, p.pos, p.data),
            C::LevelParticles(p) => self.remote_level_particles(&p),
            C::Explode(p) => self.remote_explode(&p),
            C::Sound(p) => self.apply_sound_packet(p),
            C::SoundEntity(p) => self.apply_sound_entity_packet(p),
            C::StopSound(p) => self
                .side
                .audio
                .0
                .stop_sounds(p.sound.as_deref(), p.source.map(|s| s as usize)),
            C::AddEntity(p) => self.apply_add_entity_packet(p),
            C::RemoveEntities(p) => self.apply_remove_entities_packet(p),
            C::MoveEntityPos(play::MoveEntityPos(m))
            | C::MoveEntityPosRot(play::MoveEntityPosRot(m))
            | C::MoveEntityRot(play::MoveEntityRot(m)) => {
                // handleMoveEntity: the delta counts from the position codec's
                // base, which then moves to the delta's end.
                let base = self
                    .side
                    .remote
                    .as_ref()
                    .and_then(|r| r.entities.get(&m.id))
                    .map(|t| t.codec_base);
                let Some(base) = base else { return Ok(()) };
                // VecDeltaCodec.decode: each step's position from the last.
                let path = m.delta.as_ref().map(|d| match d {
                    play::VecDelta::Linear(_) => Path::Linear(d.end(base)),
                    play::VecDelta::Stepped(steps) => {
                        let mut at = base;
                        Path::Stepped(
                            steps
                                .iter()
                                .map(|(delta, ticks)| {
                                    at = play::VecDelta::apply(at, *delta);
                                    (at, *ticks)
                                })
                                .collect(),
                        )
                    }
                });
                let rotation = m.rotation.map(|(y, x)| {
                    (
                        sunrise_protocol::buf::unpack_degrees(y),
                        sunrise_protocol::buf::unpack_degrees(x),
                    )
                });
                if let (Some(path), Some(t)) = (
                    &path,
                    self.side
                        .remote
                        .as_mut()
                        .and_then(|r| r.entities.get_mut(&m.id)),
                ) {
                    t.codec_base = path.end();
                }
                self.remote_move_entity_and_file(m.id, path, rotation, m.on_ground);
            }
            C::EntityPositionSync(p) => self.apply_entity_position_sync_packet(p),
            C::TeleportEntity(p) => self.apply_teleport_entity_packet(p)?,
            C::SetPassengers(p) => self.remote_set_passengers(p.vehicle, p.passengers),
            C::SetEntityLink(p) => self.apply_set_entity_link_packet(p),
            C::MoveVehicle(p) => self.apply_move_vehicle_packet(p),
            C::MoveMinecartAlongTrack(p) => self.remote_move_minecart(p),
            C::RotateHead(p) => self.apply_rotate_head_packet(p),
            C::SetEntityData(p) => self.apply_set_entity_data_packet(p)?,
            C::SetEquipment(p) => self.apply_set_equipment_packet(p),
            C::TakeItemEntity(p) => self.apply_take_item_entity_packet(p),
            C::SetEntityMotion(p) => self.apply_set_entity_motion_packet(p)?,
            C::ProjectilePower(p) => self.apply_projectile_power_packet(p),
            C::DamageEvent(p) => self.apply_damage_event_packet(p),
            C::HurtAnimation(p) => self.apply_hurt_animation_packet(p),
            C::EntityEvent(p) => self.apply_entity_event_packet(p)?,
            C::Animate(p) => self.remote_animate(p.id, p.action),
            C::SwingAnimation(p) => self.apply_swing_animation_packet(p),
            C::UpdateAttributes(p) => self.apply_update_attributes_packet(p)?,
            C::UpdateMobEffect(p) => self.remote_effect(&p),
            C::RemoveMobEffect(p) => self.apply_remove_mob_effect_packet(p),
            C::ChangeDifficulty(p) => {
                self.ui.difficulty = p.difficulty.clamp(0, 3) as u8;
                self.level.difficulty_locked = p.locked;
                self.remote_world_options_difficulty(self.ui.difficulty, p.locked);
            }
            C::GameRuleValues(p) => self.remote_world_options_rules(p.entries),
            C::SetHealth(p) => self.apply_set_health_packet(p),
            C::PlayerCombatKill(p) => self.apply_player_combat_kill_packet(p)?,
            C::TrackedWaypoint(p) => self.handle_waypoint(&p),
            C::SetExperience(p) => self.apply_set_experience_packet(p),
            C::SetTime(p) => self.apply_set_time_packet(p),
            C::GameEvent(p) => self.apply_game_event_packet(p),
            C::PlayerAbilities(p) => self.apply_player_abilities_packet(p),
            C::SetHeldSlot(p) => self.apply_set_held_slot_packet(p),
            C::ContainerSetContent(p) => self.apply_container_set_content_packet(p),
            C::ContainerSetSlot(p) => self.apply_container_set_slot_packet(p),
            C::SetCursorItem(p) => self.apply_set_cursor_item_packet(p),
            C::SetPlayerInventory(p) => self.apply_set_player_inventory_packet(p),
            C::ContainerSetData(p) => self.apply_container_set_data_packet(p)?,
            C::OpenScreen(p) => self.remote_open_screen(p),
            C::MountScreenOpen(p) => self.remote_open_mount(p)?,
            C::UpdateRecipes(p) => self.remote_recipe_properties(p),

            C::RecipeBookAdd(p) => self.remote_recipe_book_add(p),
            C::RecipeBookRemove(p) => self.remote_recipe_book_remove(p),
            C::RecipeBookSettings(p) => self.remote_recipe_book_settings(p),
            C::PlaceGhostRecipe(p) => self.remote_ghost_recipe(p),
            C::MerchantOffers(p) => self.remote_merchant_offers(p),
            C::ContainerClose(_) => {
                let show_death = self
                    .side
                    .remote
                    .as_ref()
                    .is_none_or(|r| r.show_death_screen);
                self.close_screen_to_level(show_death)?;
            }
            C::TagQuery(p) => self.answer_tag_query(p.transaction, p.tag),
            C::CommandSuggestions(p) => self.handle_command_suggestions(p),
            C::CustomChatCompletions(p) => {
                // ClientPacketListener.handleCustomChatCompletions.
                if let Some(r) = self.side.remote.as_mut() {
                    sunrise_command::modify_custom_completions(
                        &mut r.custom_completions,
                        sunrise_command::CustomCompletionAction::from_id(p.action),
                        &p.entries,
                    );
                    self.ui.registries.custom_completions = r.custom_completions.clone();
                }
            }
            C::UpdateTags(p) => self.remote_update_tags(&p),
            C::Commands(p) => self.apply_commands_packet(p),
            C::Cooldown(p) => {
                // ClientPacketListener.handleItemCooldown: the server starts
                // (or confirms) a group's cooldown; 0 ends it.
                let group: Arc<str> = Arc::from(p.group.as_str());
                if p.duration == 0 {
                    self.cooldowns.remove(&group);
                } else {
                    self.cooldowns.add(group, p.duration);
                }
            }
            // ClientPacketListener.handleGameTestHighlightPos.
            C::GameTestHighlightPos(p) => self.highlight_test_pos(p.absolute, p.relative),
            // ClientPacketListener.handleTestInstanceBlockStatus.
            C::TestInstanceBlockStatus(p) => {
                self.block_editor_status(remote_text(&p.status), p.size);
            }
            C::SystemChat(p) => self.apply_system_chat_packet(p)?,
            C::PlayerChat(p) => self.apply_player_chat_packet(p)?,
            C::DisguisedChat(p) => self.apply_disguised_chat_packet(p),
            C::DeleteChat(p) => self.handle_delete_chat(&p.signature)?,
            C::PlayerInfoUpdate(p) => self.apply_player_info_update_packet(p),
            C::PlayerInfoRemove(p) => self.apply_player_info_remove_packet(p),
            C::TabList(p) => self.apply_tab_list_packet(p),
            C::InitializeBorder(p) => self.apply_initialize_border_packet(p),
            C::SetBorderCenter(p) => self.border.set_center([p.x, p.z]),
            C::SetBorderLerpSize(p) => self.apply_set_border_lerp_size_packet(p),
            C::SetBorderSize(p) => self.border.set_size(p.size),
            C::SetBorderWarningDelay(p) => self.border.warning_time = p.delay,
            C::SetBorderWarningDistance(p) => self.border.warning_distance = p.blocks,
            C::SetObjective(p) => self.remote_objective(p),
            C::SetDisplayObjective(p) => self.apply_set_display_objective_packet(p),
            C::SetScore(p) => self.apply_set_score_packet(p),
            C::ResetScore(p) => match &p.objective {
                Some(o) => self.score.board.reset_one(&p.owner, o),
                None => self.score.board.reset_all(&p.owner),
            },
            C::SetPlayerTeam(p) => self.remote_team(p),
            C::SetSimulationDistance(p) => {
                if let Some(r) = self.side.remote.as_mut() {
                    r.simulation_distance = p.distance;
                }
            }
            C::SetChunkCacheRadius(p) => self.set_remote_chunk_cache_radius(p.distance),
            C::SetChunkCacheCenter(p) => self.set_remote_chunk_cache_center((p.x, p.z)),
            C::BlockEntityData(p) => self.remote_block_entity_packet(p),
            C::OpenSignEditor(p) => self.remote_open_sign_editor(p),
            C::OpenBook(p) => self.remote_open_book(p.hand),
            C::SetDefaultSpawnPosition(p) => self.apply_set_default_spawn_position_packet(p),
            C::ServerData(p) => self.remote_server_data(p.icon.as_deref()),
            C::MapItemData(p) => self.remote_map_item_data(&p),
            // handleTickingState / handleTickingStep do nothing without a ClientLevel.
            C::TickingState(p) if self.side.remote.as_ref().is_some_and(Remote::has_level) => {
                self.clock.set_rate(p.rate);
                self.clock.set_frozen(p.frozen);
            }
            C::TickingStep(p) if self.side.remote.as_ref().is_some_and(Remote::has_level) => {
                self.clock.set_frozen_ticks_to_run(p.steps);
            }
            C::TickingState(_) | C::TickingStep(_) => {}
            C::UpdateAdvancements(p) => self.remote_advancements(p),
            C::AwardStats(p) => self.apply_award_stats_packet(p)?,
            // ClientPacketListener.setActionBarText, setTitleText,
            // setSubtitleText, setTitlesAnimation and handleTitlesClear.
            C::BossEvent(p) => self.apply_boss_event(&p),
            C::SetActionBarText(p) => self.side.titles.set_overlay(remote_text(&p.text), false),
            C::SetTitleText(p) => self.side.titles.set_title(remote_text(&p.text)),
            C::SetSubtitleText(p) => self.side.titles.set_subtitle(remote_text(&p.text)),
            C::SetTitlesAnimation(p) => self.side.titles.set_times(p.fade_in, p.stay, p.fade_out),
            // ClientCommonPacketListenerImpl.handleShowDialog / handleClearDialog.
            C::ShowDialog(p) => self.show_play_dialog(&p.dialog),
            C::ClearDialog(_) => self.clear_dialog(),
            // ClientPacketListener.handleLowDiskSpaceWarning.
            C::LowDiskSpaceWarning(_) => self.side.client_reports.0.report_low_disk_space(),
            C::ClearTitles(p) => self.apply_clear_titles_packet(p),
            _ => {}
        }
        Ok(())
    }
    /// One vanilla active cache: `(2 * (max(2, client_view_distance) + 3) + 1)^2` columns
    /// (`ClientChunkCache.calculateStorageRange`), times the active dimension's sections,
    /// times 4,096 cells. Viewed levels share that footprint; a legitimate server need not
    /// group more cells than one full active world.
    /// The server's view-radius packet and per-view column caps cannot grow the ceiling.
    pub(in crate::game) fn connection_group_limit(&self) -> usize {
        self.side.remote.as_ref().map_or(0, |remote| {
            let radius = remote.sent_info.view_distance.max(2) as usize + 3;
            let width = 2 * radius + 1;
            (width * width).saturating_mul(self.world.shape.section_count() * 4096)
        })
    }
    pub(in crate::game) fn decode_remote_stack(&self, stack: &Stack) -> Option<ItemStack> {
        let r = self.side.remote.as_ref()?;
        if !sunrise_protocol::item::custom::permitted_with_names(
            stack,
            r.registries.custom_components,
            &*r.registries,
        )
        .unwrap_or(false)
        {
            log::warn!("Dropping a stack with unnegotiated custom components");
            return None;
        }
        Some(sunrise_sim::item_sync::from_network(
            stack,
            &r.builtin,
            &r.registries,
        ))
    }
    /// `ServerReconfigScreen` over the panorama; Disconnect wakes up after
    /// `DISCONNECT_TIME` (600 ticks).
    pub(in crate::game::client_side) fn draw_reconfiguring(
        &self,
        batch: &mut HudBatch,
        since: u64,
    ) {
        let atlas = &self.side.gui.atlas;
        let (w, h) = (batch.width, batch.height);
        sunrise_client::ui::menu_background(batch, atlas, 0.0, 0.0, w, h, false);
        let title = sunrise_game_data::text::Text::translate("connect.reconfiguring", &[]);
        let ((tx, ty), (bx, by)) = self.reconfigure_layout((w, h));
        batch.text_styled(atlas, &title, tx, ty, u32::MAX, true);
        // CommonComponents.GUI_DISCONNECT
        let label = sunrise_game_data::text::Text::translate("menu.disconnect", &[]);
        let active = self.ticks.saturating_sub(since) >= 600;
        let hover = sunrise_client::ui::hovered(self.side.cursor, bx, by, 150.0, 20.0);
        sunrise_client::ui::button(batch, atlas, bx, by, 150.0, 20.0, &label, active, hover);
    }
    /// `HashedStack.create` with the connection's registries (the client's
    /// `decoratedHashOpsGenerator`).
    pub(in crate::game) fn hashed_stack(
        &self,
        s: &Option<ItemStack>,
    ) -> sunrise_protocol::Result<Option<Hashed>> {
        if s.is_none() {
            return Ok(None);
        }
        let r = self
            .side
            .remote
            .as_ref()
            .ok_or_else(|| sunrise_protocol::Error::new("no live menu connection"))?;
        let names = SyncedNames {
            builtin: &r.builtin,
            dynamic: &r.registries,
        };
        let projected = std::cell::RefCell::new(Vec::new());
        let stack = self
            .to_network_projecting(s, &|effect| projected.borrow_mut().push(effect.clone()))
            .ok_or_else(|| sunrise_protocol::Error::new("cannot encode a nonempty stack"))?;
        let projected = projected.into_inner();
        let hash = if let Some(data) = s.as_ref().and_then(ItemStack::data) {
            Hashed::create_with_admitted(&stack, &names, &|name, bytes| {
                let matches =
                    |effect: &sunrise_world::component_value::consume_effects::Admitted| {
                        &effect.name == name && effect.wire.as_ref() == bytes
                    };
                if stack.wire.0.is_some() {
                    data.wire
                        .admitted_effects()
                        .iter()
                        .find(|effect| matches(effect))
                        .cloned()
                } else {
                    data.find_admitted_consume_effect(&matches)
                        .or_else(|| projected.iter().find(|effect| matches(effect)).cloned())
                }
            })
        } else {
            Hashed::create(&stack, &names)
        };
        hash.map(Some)
            .map_err(|e| sunrise_protocol::Error::new(e.to_string()))
    }
    pub(in crate::game) fn item_name(&self, id: i32) -> Arc<str> {
        let r = self.side.remote.as_ref().expect("remote");
        Arc::from(
            r.builtin
                .get("minecraft:item")
                .and_then(|i| i.name(id))
                .unwrap_or("minecraft:air"),
        )
    }
    pub(in crate::game) fn next_sequence(&mut self) -> i32 {
        let r = self.side.remote.as_mut().expect("remote");
        r.sequence += 1;
        r.sequence
    }
    /// The latency the server's player list gives this player
    /// (`PlayerInfo.getLatency`), none before the server lists it. A new entry reads 0 until the
    /// server's first latency update (every 600 ticks).
    pub(in crate::game) fn own_latency(&self) -> Option<i32> {
        let r = self.side.remote.as_ref()?;
        r.tab_players.get(&r.profile).map(|me| me.latency)
    }
    /// Whether the server's patch gives `stack`, an item with no consumable
    /// of its own, a `consumable` with a use time or `blocks_attacks` (a
    /// sword made to block): `Item.use` starts a use of it
    /// (`Consumable.startConsuming`, `startUsingItem`) that lasts until the
    /// button lets go or the server stops it, as the client's
    /// `LivingEntity.updateUsingItem` never completes one.
    pub(in crate::game::client_side) fn patched_held_use(&self, stack: &ItemStack) -> bool {
        let (ticks, blocks) = sunrise_sim::item_sync::patched_use(stack);
        ticks.map_or(blocks, |ticks| ticks > 0)
            && self
                .items()
                .get(&stack.item)
                .is_none_or(|i| i.consumable.is_none())
    }
    pub(in crate::game) fn play_predicted_sounds(&mut self, sounds: &[DrySound]) {
        for (pos, sound, source, volume, pitch) in sounds {
            let at = [
                f64::from(pos.x) + 0.5,
                f64::from(pos.y) + 0.5,
                f64::from(pos.z) + 0.5,
            ];
            let name = if sound.contains(':') {
                sound.to_string()
            } else {
                format!("minecraft:{sound}")
            };
            self.play_client_local_sound(
                LevelSound::new(&name, at, *volume, *pitch).in_source(*source),
            );
        }
    }
    /// The player's position, for `retainKnownServerState`'s `player.position()`.
    pub(in crate::game) fn player_position(&self) -> [f64; 3] {
        let at = self
            .player
            .as_ref()
            .map(|p| p.position)
            .unwrap_or(Vec3::ZERO);
        [at.x, at.y, at.z]
    }
    /// Process what the network thread received (once per frame, like the
    /// client's packet processor), in order. Chunk packets get
    /// `CHUNK_PACKETS_PER_FRAME`: once a frame has spent it, the events from
    /// the next chunk packet on wait for the next frame, unless they have
    /// already waited `BACKLOG_LIMIT`. A chunk packet still decoding holds
    /// back what follows it.
    pub(in crate::game) fn poll_remote(&mut self) -> Result<()> {
        self.poll_remote_with(false)
    }
    /// `Minecraft.runTick` handles every packet that came (`runAllTasks`)
    /// before the frame's ticks. What this frame's poll left for later (chunks
    /// past its budget or still decoding, and everything after them) is taken
    /// when a tick is about to run, so the tick moves through those chunks, as
    /// vanilla's does. A batch's `ChunkBatchReceived` is not among them: the
    /// network thread answers it (`net::receive_play`).
    pub(in crate::game) fn poll_remote_before_tick(&mut self) -> Result<()> {
        if self
            .side
            .remote
            .as_ref()
            .is_none_or(|r| r.backlog.is_empty())
        {
            return Ok(());
        }
        self.poll_remote_with(true)
    }
    pub(in crate::game) fn poll_remote_with(&mut self, before_tick: bool) -> Result<()> {
        let polled = self.poll_remote_events(before_tick);
        // The block events and block changes of a piston clock start and end
        // thousands of moves at once: the collision view and the renderer
        // take them all in one sync, before the tick or frame reads them.
        if std::mem::take(&mut self.side.moving_pistons_stale) {
            self.sync_moving_pistons();
        }
        polled
    }
    /// `MapItem.useOn` on this client: a map used on a banner is SUCCESS
    /// whatever its data says (the server toggles the marker, or fails
    /// quietly), so this hand swings and the click ends. The caller applies
    /// ItemStack.useOn's adventure predicate before selecting this phase.
    pub(in crate::game::client_side) fn predict_map_on_banner(
        &self,
        stack: &ItemStack,
        hit: &super::Hit,
    ) -> bool {
        crate::game::maps::map_id(stack).is_some()
            && self.named_block_in_tag("minecraft:banners", &self.block_name(hit.pos))
    }
    /// MultiPlayerGameMode.useItemOn inside startPrediction: the blocks the
    /// use changed on this client's level at once, each kept as predicted
    /// under the use's sequence until the server acknowledges it (a use the
    /// server refused goes back). Only the blocks a hand opens or flips: a
    /// wooden or copper door, trapdoor or fence gate, a lever or a button;
    /// whatever else the use changes waits for the server.
    pub(in crate::game) fn predict_use(&mut self, (changes, sounds): &DryUse, seq: i32) {
        let behaviours = self.res.behaviours();
        let predicted = |state: BlockStateId| hand_operated(behaviours, state);
        if changes.is_empty()
            || !changes
                .iter()
                .all(|(p, s)| predicted(*s) && predicted(self.world.block(p.x, p.y, p.z)))
        {
            return;
        }
        self.predict_blocks(changes, seq);
        // `level.playSound(player, ...)`: the clicker hears it from its own use; the
        // server sends it to the others only.
        self.play_predicted_sounds(sounds);
    }
    /// `ServerReconfigScreen`: a vertical layout of cells padded by 10, the
    /// title (9 high) over Disconnect (150 x 20), centred. The title's and the
    /// button's top left corners.
    pub(in crate::game) fn reconfigure_layout(
        &self,
        (w, h): (f32, f32),
    ) -> ((f32, f32), (f32, f32)) {
        let title = sunrise_game_data::text::Text::translate("connect.reconfiguring", &[]);
        let title_w = self.side.gui.atlas.font.width_text(&title).ceil() as i32;
        let width = (title_w + 20).max(170);
        let left = ((w as i32 - width) as f32 * 0.5) as i32;
        let top = ((h as i32 - 69) as f32 * 0.5) as i32;
        let across = |child: i32| (10.0 + ((width - child - 10) as f32 - 10.0) * 0.5) as i32;
        (
            ((left + across(title_w)) as f32, (top + 10) as f32),
            ((left + across(150)) as f32, (top + 39) as f32),
        )
    }
    /// Live client `Abilities` (`LocalPlayer.abilities`) when connected to a server.
    pub(in crate::game) fn remote_abilities(
        &self,
    ) -> Option<&sunrise_sim::lan::abilities::PlayerAbilities> {
        self.side.remote.as_ref().map(|r| &r.abilities)
    }
    /// The attack clicks queued for the next tick (`KeyMapping.click`, not yet handled).
    pub(in crate::game) fn remote_attack_clicks_queued(&self) -> u32 {
        self.pending_mapping_clicks("key.attack")
    }
    /// Received events left for the next frame (`poll_remote`): until they
    /// are handled, the terrain the server sent is not all there.
    pub(in crate::game) fn remote_backlog(&self) -> usize {
        self.side.remote.as_ref().map_or(0, |r| r.backlog.len())
    }
    /// `BoneMealItem.useOn` with the acting hand's `stack` on this client's
    /// level (a copy: nothing grows here): whether it succeeds.
    pub(in crate::game::client_side) fn remote_bone_meal_succeeds(
        &mut self,
        hit: &super::Hit,
        stack: &ItemStack,
    ) -> bool {
        let pos = sunrise_core::position::BlockPos::new(hit.pos[0], hit.pos[1], hit.pos[2]);
        let face = sunrise_core::geometry::Direction::ALL[face_direction(hit.face) as usize];
        let (ask, _) = self.held_bonemeal(pos, stack);
        self.with_level(|b, level, _| {
            b.bone_meal_succeeds_on_client(
                &mut DryLevel::new(level),
                pos,
                face,
                ask.crop,
                ask.check,
            )
        })
    }
    /// The minecarts the server sent, for the control socket's `status`
    /// (their velocity is not tracked here).
    pub(in crate::game::client_side) fn remote_carts(&self) -> Option<Vec<serde_json::Value>> {
        let r = self.side.remote.as_ref()?;
        let mut carts: Vec<&Tracked> = r
            .entities
            .values()
            .filter(|t| t.kind.ends_with("minecart"))
            .collect();
        carts.sort_by(|a, b| {
            a.position
                .partial_cmp(&b.position)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Some(
            carts
                .into_iter()
                .map(|t| serde_json::json!({"kind": t.kind, "position": t.position, "velocity": [0.0, 0.0, 0.0]}))
                .collect(),
        )
    }
    /// MultiPlayerGameMode.dropItem (key.drop): predict the removal, then send
    /// the action; swing only if a nonempty stack was removed.
    pub(in crate::game) fn remote_drop(&mut self, all: bool) {
        // ensureHasSentCarriedItem: the server drops from the slot just chosen.
        self.remote_send_carried();
        // MultiPlayerGameMode.dropItem predicts the removal and sends only the drop: the
        // server's stack agrees, so no creative slot edit follows at the end of the tick.
        let before = self.creative_stacks();
        let dropped =
            sunrise_items::items::take_held_drop(&mut self.inventory[self.selected], all, false);
        self.remote_synced_since(&before);
        self.send_action(if all { 4 } else { 5 });
        if dropped.is_some() {
            self.swing_hand(Hand::Main, SwingAnimation::DEFAULT);
        }
    }
    /// The hurt sound the hurt player's own client plays from
    /// `ClientboundDamageEventPacket` (`LivingEntity.handleDamageEvent`):
    /// `Player.getHurtSound` is the damage type's effects, with the codec's
    /// `hurt` default when the type is unknown.
    pub(in crate::game::client_side) fn remote_hurt_sound(&self, source_type: i32) -> &'static str {
        match self.side.remote.as_ref() {
            Some(r) => hurt_sound_for(&r.registries, &self.health_resources.damage, source_type),
            None => sunrise_sim::health::hurt_sound("hurt"),
        }
    }
    // remote_tab_players: remote/sync.rs
    // update_remote_skins: remote/status.rs
    pub(in crate::game::client_side) fn remote_item_data(
        &mut self,
        id: i32,
        stack: &Option<Stack>,
    ) {
        let Some(stack) = self.to_stack(stack) else {
            return;
        };
        let pending = self
            .side
            .remote
            .as_mut()
            .and_then(|r| r.pending_items.remove(&id));
        if let Some(item) = self.dropped.iter_mut().find(|d| d.id == id as u32) {
            item.stack = stack;
            return;
        }
        let Some(PendingItem {
            join,
            position: [x, y, z],
            velocity,
            no_gravity,
        }) = pending
        else {
            return;
        };
        let at = Vec3::new(x, y, z);
        // ItemEntity's constructor draws bobOffs from the item's own random.
        let mut random = sunrise_sim::mobs::entity_random(id as u32, at);
        let bob = crate::game::client_side::item_entities_client::bob_offset(
            at,
            random.next_float(),
            self.clock.still(),
        );
        // Metadata can arrive after newer entities (including other items). Insert at the
        // reserved ClientLevel.addEntity slot, not the tail at metadata time.
        let index = self.dropped.partition_point(|item| item.join < join);
        self.dropped.insert(
            index,
            ItemEntity {
                join,
                random,
                no_gravity,
                ..ItemEntity::new(id as u32, stack, at, velocity, bob)
            },
        );
    }
    /// `AbstractMinecartRenderer.newExtractState`: sample the same weighted
    /// steps at the frame's partial tick, including corners within one tick.
    pub(in crate::game::client_side) fn remote_minecart_lerp(
        &self,
        id: u32,
        partial: f32,
    ) -> Option<([f64; 3], f32, f32)> {
        let lerp = self.side.remote.as_ref()?.cart_lerps.get(&(id as i32))?;
        if lerp.current.is_empty() {
            return None;
        }
        let (p, _, yaw, pitch) = lerp.at(partial);
        Some(([p.x, p.y, p.z], yaw, pitch))
    }
    /// Status for `sunrise-ctl status`.
    /// The other players' entities in the level and their names (their
    /// profiles', which outlive the player list's entries).
    pub(in crate::game::client_side) fn remote_player_names(&self) -> Vec<(u32, String)> {
        let Some(r) = self.side.remote.as_ref() else {
            return Vec::new();
        };
        r.level_players
            .iter()
            .filter(|(_, uuid)| **uuid != r.profile)
            .filter_map(|(id, uuid)| Some((*id as u32, r.level_player_name(*id, uuid)?.to_owned())))
            .collect()
    }
    pub(in crate::game::client_side) fn remote_player_uuid(&self, id: u32) -> Option<u128> {
        let remote = self.side.remote.as_ref()?;
        if self.side.player_model.as_ref().is_some_and(|m| m.id == id) {
            Some(remote.profile)
        } else {
            remote.level_players.get(&(id as i32)).copied()
        }
    }
    pub(in crate::game::client_side) fn remote_respawn(&mut self) {
        self.send(S::ClientCommand(play::ClientCommand { action: 0 }));
    }
    /// A connection's screen is over the world: connecting, reconfiguring, or
    /// the level loading screen, a portal's included (`remote_overlay`).
    pub(in crate::game::client_side) fn remote_screen_up(&self) -> bool {
        !self.side.replay.playing()
            && self
                .side
                .remote
                .as_ref()
                .is_some_and(|r| self.player.is_none() || r.level_load.is_active())
    }
    /// The `SetPassengers` bookkeeping of the tracked entities.
    pub(in crate::game::client_side) fn remote_seat_passengers(
        &mut self,
        vehicle: u32,
        passengers: &[i32],
    ) {
        if let Some(r) = self.side.remote.as_mut() {
            seat_passengers(&mut r.entities, vehicle, passengers);
        }
    }
    /// Whether secondary use skips a block's default interaction on this client.
    pub(in crate::game::client_side) fn remote_secondary_use_skips_block(&self) -> bool {
        self.remote_shift_down()
            && (self.stack_in(self.selected).is_some() || self.offhand.is_some())
    }
    /// `handleServerData`: the server list entry of the server joined keeps
    /// the icon the server sends (`ServerList.saveSingleServer`). Single
    /// player has no entry.
    pub(in crate::game) fn remote_server_data(&mut self, icon: Option<&[u8]>) {
        let (Some(r), Some(png)) = (self.side.remote.as_ref(), icon) else {
            return;
        };
        if self.side.integrated.is_some() {
            return;
        }
        if let Ok(dir) = std::env::current_dir() {
            sunrise_client::multiplayer::save_server_icon(
                &dir.join("saves"),
                &r.session.address,
                png,
            );
        }
    }
    /// `LocalPlayer.isShiftKeyDown` (and so `isSecondaryUseActive`): the input as of the player's
    /// last tick (`input.keyPresses`, what the last ServerboundPlayerInputPacket said), not the key
    /// now: a sneak pressed in the frame of a click acts from the next tick, as on the server.
    pub(in crate::game) fn remote_shift_down(&self) -> bool {
        self.side
            .remote
            .as_ref()
            .is_some_and(|r| r.last_input & SHIFT_INPUT != 0)
    }
    /// ServerboundPlayerCommandPacket STOP_SLEEPING.
    pub(in crate::game) fn remote_stop_sleeping(&mut self) {
        let id = self.side.remote.as_ref().map_or(0, |r| r.entity_id);
        self.send(S::PlayerCommand(play::PlayerCommand {
            id,
            action: 0,
            data: 0,
        }));
    }
    // remote_player_eating: remote/use.rs
    /// Inventory as the server last described it.
    pub(in crate::game) fn remote_synced(&mut self) {
        let stacks = self.creative_stacks();
        if let Some(r) = self.side.remote.as_mut() {
            r.synced = stacks;
        }
    }
    /// The slots a server packet changed since `before` are as the server has them; the
    /// others keep what was last sent, so a creative click made since still goes out.
    pub(in crate::game) fn remote_synced_since(&mut self, before: &[Option<ItemStack>]) {
        let now = self.creative_stacks();
        if let Some(r) = self.side.remote.as_mut() {
            mark_synced(&mut r.synced, before, now);
        }
    }
    /// `Entity.isVehicle` on a client: the entities a tracked entity rides.
    pub(in crate::game::client_side) fn remote_vehicle_ids(
        &self,
    ) -> impl Iterator<Item = u32> + '_ {
        self.side
            .remote
            .iter()
            .flat_map(|r| r.entities.values().filter_map(|t| t.vehicle))
    }
    pub(in crate::game) fn send_action(&self, action: i32) {
        self.send(S::PlayerAction(play::PlayerAction {
            action,
            pos: [0; 3],
            direction: 0,
            sequence: 0,
        }));
    }
    /// ClientSuggestionProvider.customSuggestion: the chat's completion
    /// request for an `ask_server` node goes to the server.
    pub(in crate::game::client_side) fn send_suggestion_request(&mut self) {
        if self.side.remote.is_none() {
            return;
        }
        let Some((id, command)) = self
            .side
            .chat_ui
            .chat_screen
            .as_mut()
            .and_then(|s| s.take_request())
            .or_else(|| self.command_block_suggestion_request())
        else {
            return;
        };
        let mut w = ::sunrise_core::vanilla_bytes::Writer::new();
        w.varint(id);
        w.string(&command);
        self.send(S::Unknown(
            sunrise_protocol::ids::play::serverbound::COMMAND_SUGGESTION,
            w.data,
        ));
    }
    /// `Entity.absSnapTo`: the remembered point, clamped to the level's edge.
    /// The velocity is left alone, and the player has no separate previous
    /// position to reset (its box is drawn from `position`).
    pub(in crate::game) fn snap_player_to(&mut self, at: [f64; 3]) {
        let Some(player) = self.player.as_mut() else {
            return;
        };
        let x = at[0].clamp(-3.0e7, 3.0e7);
        let z = at[2].clamp(-3.0e7, 3.0e7);
        player.position = Vec3::new(x, at[1], z);
    }
    /// A stack from the server, decoded as the saved and command item NBT
    /// are, so tooltips and models see every field.
    pub(in crate::game) fn to_stack(&self, s: &Option<Stack>) -> Option<ItemStack> {
        self.decode_remote_stack(s.as_ref()?)
    }

    pub(in crate::game) fn send(&self, packet: S) {
        if self.side.remote.is_some() {
            // MultiPlayerGameMode's attack, interact, useItem, useItemOn, releaseUsingItem, dropItem and
            // piercingAttack run ensureHasSentCarriedItem before their packet: a slot the number keys
            // chose this tick goes out first. A miss, a dig's start or a swap waits for the next tick.
            if let S::Attack(_) | S::Interact(_) | S::UseItem(_) | S::UseItemOn(_) = &packet {
                self.remote_send_carried();
            } else if let S::PlayerAction(a) = &packet
                && matches!(a.action, 4 | 5 | 6 | 8)
            {
                self.remote_send_carried();
            }
            self.send_play(packet);
        }
    }

    /// `MultiPlayerGameMode.attack` runs `Player.attack` on this client too: its
    /// `isSweepAttack` for a damaging hit on `target` (`living`: a
    /// `LivingEntity`, which may be hit critically), heard by native mods on
    /// both sides. Only the server sweeps.
    pub(in crate::game) fn remote_sweep_event(&mut self, target: &str, living: bool) {
        if mod_api::__private::mods().is_empty() {
            return;
        }
        let (damage, scale) = self.remote_attack_damage();
        // Player.getEnchantedDamage returns baseDamage on a client, so magic bonus is 0.
        if damage <= 0.0 {
            return;
        }
        let crit = living && self.critical_attack(scale);
        self.sweep_decision(scale, crit, target);
    }

    /// Minecraft.handleKeybinds, key.swapOffhand: the server swaps the hands
    /// (ServerboundPlayerActionPacket SWAP_ITEM_WITH_OFFHAND at BlockPos.ZERO, facing down) and
    /// sends both slots back; nothing is predicted.
    pub(in crate::game) fn remote_swap_hands(&mut self) {
        self.send_action(7);
        // Equal stacks swap with no slot update to wait for.
        let (main, off) = (self.stack_in(self.selected), self.offhand.as_ref());
        let moves = !super::hand::matches_for_swap(main, off);
        if let Some(r) = self.side.remote.as_mut() {
            r.swap_answer = [moves; 2];
        }
    }
}

/// The game as the tick's input sees it.
#[cfg(feature = "client")]
pub(in crate::game) struct RemoteInputWorld<'g, 'a>(
    pub(in crate::game) &'g mut Game<'a, crate::game::client_side::ClientState>,
);

#[cfg(feature = "client")]
pub(in crate::game) struct RemoteUseWorld<'g, 'a>(
    pub(in crate::game) &'g mut Game<'a, crate::game::client_side::ClientState>,
    pub(in crate::game) mod_api::InteractionAttempt,
);

#[cfg(feature = "client")]
impl sunrise_client::interaction_input::TargetUseWorld for RemoteUseWorld<'_, '_> {
    fn use_entity(&mut self) -> bool {
        self.0.remote_use_entity(Hand::Main)
    }
    fn use_block(&mut self) -> bool {
        self.0.remote_use_block(Hand::Main, self.1, None)
    }
    fn use_item(&mut self) {
        self.0.remote_use_hands(Hand::Main, self.1, None);
    }
}

#[cfg(feature = "client")]
#[cfg(test)]
impl crate::game::ClientGame<'_> {
    pub(in crate::game) fn test_remote_pick_click(&mut self) {
        use sunrise_client::interaction_input::InteractionWorld as _;
        RemoteInputWorld(self).pick();
    }
    pub(in crate::game) fn test_has_client_item_use(&self) -> bool {
        self.side.client_item_use.0.is_some()
    }
    pub(in crate::game) fn test_audio_mut(&mut self) -> &mut crate::game::Audio {
        &mut self.side.audio.0
    }
    pub(in crate::game) fn test_show_title_screen(&mut self) {
        let saves = std::env::current_dir().unwrap_or_default().join("saves");
        self.side.title.0 = Some(self.new_title(saves));
    }
    pub(in crate::game) fn test_hand_strength_ticker(&self) -> i32 {
        self.side.hand_strength.ticker
    }
    pub(in crate::game) fn test_set_hand_strength_ticker(&mut self, ticker: i32) {
        self.side.hand_strength.ticker = ticker;
    }
}

impl PlayerTextures {
    pub(in crate::game) fn new(property: AccountProperty) -> Self {
        Self {
            property,
            unpacked: std::sync::OnceLock::new(),
        }
    }
}

impl Remote {
    /// Project the session address for display once, dropping the memory transport's marker.
    pub(in crate::game) fn display_address_for(address: &str) -> Option<String> {
        (address != "local").then(|| sunrise_client_lib::net::shown_address(address))
    }
}

impl ServicesKeys {
    /// Minecraft.services() has the keys wherever the game is played, single player's own
    /// server included: its guests' chat keys are checked against them.
    pub(in crate::game::client_side) fn fetching() -> Arc<std::sync::Mutex<Self>> {
        Self::fetching_with(|| {
            sunrise_server_profiles_http::fetch_profile_key_keys(
                &sunrise_server_files::profiles::discovery_url(),
            )
        })
    }
    /// The keys start `Pending` and arrive from `fetch` on a thread of their own, so a
    /// listener never caches a verdict on keys it does not have yet. A thread that cannot
    /// start leaves them pending.
    pub(in crate::game) fn fetching_with<E: std::fmt::Display>(
        fetch: impl FnOnce() -> std::result::Result<ServicesKeySet, E> + Send + 'static,
    ) -> Arc<std::sync::Mutex<Self>> {
        let keys = Arc::new(std::sync::Mutex::new(ServicesKeys::Pending));
        let slot = keys.clone();
        let _ = std::thread::Builder::new()
            .name("Services keys".into())
            .spawn(move || {
                // The error is the line vanilla logs, after which it has no keys.
                let keys = fetch().unwrap_or_else(|why| {
                    log::error!("Sunrise: {why}");
                    ServicesKeySet::default()
                });
                *slot.lock().unwrap_or_else(|e| e.into_inner()) = ServicesKeys::Ready(keys);
            });
        keys
    }
}

#[cfg(feature = "client")]
#[cfg(test)]
#[path = "claimed_item_use_tests_client.rs"]
pub(in crate::game) mod claimed_item_use_tests;

#[cfg(feature = "client")]
#[cfg(test)]
#[path = "gamemode_actions_tests_client.rs"]
pub(in crate::game) mod gamemode_actions_tests;

#[cfg(feature = "client")]
#[cfg(test)]
#[path = "chat_audit_tests_client.rs"]
pub(in crate::game) mod chat_audit_tests;

#[cfg(feature = "client")]
#[cfg(test)]
#[path = "interaction_event_tests_client.rs"]
pub(in crate::game) mod interaction_event_tests;

#[cfg(feature = "client")]
#[cfg(test)]
#[path = "native_entity_tests_client.rs"]
pub(in crate::game) mod native_entity_tests;

#[cfg(feature = "client")]
#[cfg(test)]
#[path = "camera_view_tests_client.rs"]
pub(in crate::game) mod camera_view_tests;

#[cfg(feature = "client")]
#[cfg(test)]
#[path = "level_sound_tests_client.rs"]
pub(in crate::game) mod level_sound_tests;

#[cfg(feature = "client")]
#[cfg(test)]
#[path = "client_camera_tests_client.rs"]
pub(in crate::game) mod client_camera_tests;

#[cfg(feature = "client")]
#[cfg(test)]
#[path = "riptide_tests_client.rs"]
pub(in crate::game) mod riptide_tests;

#[cfg(feature = "client")]
#[cfg(test)]
pub(in crate::game) mod tests {
    use super::Game;
    pub(in crate::game::client_side) use crate::game::remote::tests::connect_menu_test;
    pub(in crate::game::client_side) use crate::game::remote::tests::connect_play_test;
    pub(in crate::game::client_side) use crate::game::remote::tests::spawn_menu_test;

    pub(in crate::game) mod client_explosion_tests {
        include!("client_explosion_tests_client.rs");
    }

    pub(in crate::game) mod chat_window_and_input_tests {
        include!("chat_window_and_input_tests_client.rs");
    }

    pub(in crate::game) mod play_bundle_tests {
        include!("play_bundle_tests_client.rs");
    }

    pub(in crate::game) mod camera_draw_tests {
        include!("camera_draw_tests_client.rs");
    }

    pub(in crate::game) mod camera_parity_red_tests {
        include!("camera_parity_red_tests_client.rs");
    }

    pub(in crate::game) mod camera_death_tests {
        include!("camera_death_tests_client.rs");
    }

    pub(in crate::game) mod client_hud_camera_tests {
        include!("client_hud_camera_tests_client.rs");
    }

    pub(in crate::game) mod client_day_time_tests {
        include!("client_day_time_tests_client.rs");
    }

    pub(in crate::game) mod client_level_environment_tests {
        include!("client_level_environment_tests_client.rs");
    }

    pub(in crate::game) mod client_level_lifecycle_tests {
        include!("client_level_lifecycle_tests_client.rs");
    }

    pub(in crate::game) mod client_menu_tests {
        include!("client_menu_tests_client.rs");
    }

    pub(in crate::game) mod client_vanilla_reads_tests {
        include!("client_vanilla_reads_tests_client.rs");
    }

    pub(in crate::game) mod container_drop_tests {
        include!("container_drop_tests_client.rs");
    }

    pub(in crate::game) mod dimension_transition_tests {
        include!("dimension_transition_tests_client.rs");
    }

    pub(in crate::game) mod menu_packets_tests {
        include!("menu_packets_tests_client.rs");
    }

    pub(in crate::game) mod client_close_tests {
        include!("client_close_tests_client.rs");
    }

    pub(in crate::game) mod particle_group_tests {
        include!("particle_group_tests_client.rs");
    }

    pub(in crate::game) mod spear_jab_tests {
        include!("spear_jab_tests_client.rs");
    }

    pub(in crate::game) mod client_admission_tests {
        include!("client_admission_tests_client.rs");
    }

    pub(in crate::game) mod client_emitters_tests {
        include!("client_emitters_tests_client.rs");
    }

    pub(in crate::game) mod tracking_state_tests {
        include!("tracking_state_tests_client.rs");
    }

    #[test]
    #[ignore = "daily"]
    fn declared_gate_uses_its_class_for_blasts_and_predicted_use() {
        use crate::game::native_guest_turn_tests::with_guests_at;
        use std::sync::Arc;
        use sunrise_core::position::BlockPos;
        use sunrise_level::chunk::Chunk;
        use sunrise_mod_package::{
            content::{Block, Content, GateSound},
            enabled::EnabledMods,
        };
        use sunrise_protocol::{buf::Reader, packets::play::Clientbound};
        use sunrise_sim::{
            lan::{Out, PlayerSender},
            mod_content::{ContentMod, resource_root},
        };

        let base = std::path::PathBuf::from(
            std::env::var_os("SUNRISE_TEST_RESOURCES").expect("prepared resources required"),
        );
        let scratch = tempfile::tempdir().unwrap();
        let mut content = Content::declared("gates");
        let use_sound = "gates:use.open";
        let blast_sound = "gates:blast.close";
        let wire_sound = "minecraft:entity.zombie.ambient";
        content
            .declare(
                &[Block {
                    id: "gates:sites".into(),
                    template: "minecraft:oak_fence_gate".into(),
                    item: false,
                    open_sound: Some(GateSound::Sites {
                        on_use: Some(use_sound.into()),
                        on_explosion: None,
                        on_redstone: Some(wire_sound.into()),
                    }),
                    close_sound: Some(GateSound::Sites {
                        on_use: None,
                        on_explosion: Some(blast_sound.into()),
                        on_redstone: None,
                    }),
                    ..Default::default()
                }],
                &[],
            )
            .unwrap();
        let root = resource_root(
            &base,
            scratch.path(),
            &[ContentMod {
                id: "gates".into(),
                content,
                jars: Vec::new(),
            }],
            scratch.path(),
            &EnabledMods::new([("gates".into(), mod_api::Side::Both)]),
        )
        .unwrap();
        let pos = [3, 64, 4];
        let [x, y, z] = pos;
        let hit = crate::game::Hit {
            pos,
            face: [0, 1, 0],
            point: [3.5, 64.875, 4.5],
            inside: false,
        };
        fn open<S>(game: &crate::game::Game<'_, S>, x: i32, y: i32, z: i32) -> bool {
            let b = game.res.behaviours();
            b.table.bool(game.world.block(x, y, z), b.props.open)
        }
        let blocks = mod_api::SoundSource::Blocks.index();

        crate::game::native_entity_refs::tests::with_game_role_client(
            root.clone(),
            crate::game::execution_role::ExecutionRole::Client,
            |game| {
                game.world
                    .chunks
                    .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
                let gate = game
                    .pack
                    .assets
                    .registry
                    .default_state("gates:sites")
                    .unwrap();
                let vanilla_gate = game
                    .pack
                    .assets
                    .registry
                    .default_state("minecraft:oak_fence_gate")
                    .unwrap();
                for (state, name) in [
                    (vanilla_gate, "minecraft:block.fence_gate.open"),
                    (gate, use_sound),
                ] {
                    game.world.set(x, y, z, state).unwrap();
                    game.side.audio.0.capture = Some(Vec::new());
                    let (answer, dry) = game.remote_block_reacts(&hit, None, true);
                    assert!(answer.consumes_action());
                    game.predict_use(&dry, 1);
                    assert!(
                        open(game, x, y, z),
                        "the gate opens without a server round trip"
                    );
                    let capture = game.side.audio.0.capture.as_mut().unwrap();
                    assert_eq!(capture.len(), 1);
                    let predicted = capture.pop().unwrap();
                    assert_eq!((predicted.event.as_str(), predicted.source), (name, blocks));
                }
            },
        );

        with_guests_at(root, |game| {
            game.world
                .chunks
                .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
            game.sync_tick_chunks();
            let gate = game
                .pack
                .assets
                .registry
                .default_state("gates:sites")
                .unwrap();

            let [actor, other] = [12, 13].map(|conn| {
                let (tx, rx) = std::sync::mpsc::channel();
                game.side
                    .lan
                    .as_mut()
                    .unwrap()
                    .hub
                    .update_guest(conn, |guest| {
                        guest.sender = PlayerSender::new(tx, Arc::default());
                    })
                    .unwrap();
                rx
            });
            let sounds = |game: &crate::game::ServerGame<'_>,
                          rx: &std::sync::mpsc::Receiver<Out>| {
                let sync = game.side.lan.as_ref().unwrap().hub.sync_data();
                rx.try_iter()
                    .flat_map(|out| match out {
                        Out::Packet(body) => vec![body],
                        Out::Packets(bodies) => bodies,
                        Out::Disconnect(_) => Vec::new(),
                    })
                    .filter_map(|body| {
                        let mut reader = Reader::new(&body);
                        let id = reader.varint().unwrap();
                        match Clientbound::decode(id, reader.rest()).unwrap() {
                            Clientbound::Sound(packet) => {
                                let name = packet.sound.1.map(|(id, _)| id).unwrap_or_else(|| {
                                    sync.builtin
                                        .get("minecraft:sound_event")
                                        .unwrap()
                                        .name(packet.sound.0)
                                        .unwrap()
                                        .to_owned()
                                });
                                Some((name, packet.source, packet.volume))
                            }
                            _ => None,
                        }
                    })
                    .collect::<Vec<_>>()
            };
            let vanilla_gate = game
                .pack
                .assets
                .registry
                .default_state("minecraft:oak_fence_gate")
                .unwrap();
            game.options.dedicated = Some(Vec::new());
            for (state, name) in [
                (vanilla_gate, "minecraft:block.fence_gate.open"),
                (gate, use_sound),
            ] {
                game.world.set(x, y, z, state).unwrap();
                game.world.block_log = Some(Vec::new());
                assert!(
                    game.guest_use_item_on(
                        12,
                        game.guest_index(12).unwrap(),
                        hit,
                        super::Hand::Main,
                        None,
                    )
                    .unwrap()
                );
                assert!(open(game, x, y, z));
                game.lan_broadcast_effects();
                assert!(sounds(game, &actor).is_empty(), "no echo to the clicker");
                assert_eq!(sounds(game, &other), [(name.into(), blocks as i32, 1.0)]);
            }

            game.start_audio_capture();
            game.explosion_trigger_blocks(&mut [pos]).unwrap();
            assert!(!open(game, x, y, z));
            game.lan_broadcast_effects();
            for listener in [&actor, &other] {
                assert_eq!(
                    sounds(game, listener),
                    [(blast_sound.into(), blocks as i32, 1.0)]
                );
            }

            let power = game
                .pack
                .assets
                .registry
                .default_state("minecraft:redstone_block")
                .unwrap();
            assert!(game.with_level(|b, level, ticks| {
                b.set_block(level, ticks, BlockPos::new(x + 1, y, z), power, 3)
            }));
            game.server_process_level_events().unwrap();
            assert!(open(game, x, y, z));
            game.lan_broadcast_effects();
            for listener in [&actor, &other] {
                assert_eq!(
                    sounds(game, listener),
                    [(wire_sound.into(), blocks as i32, 1.0)]
                );
            }
            let blast_count = |game: &crate::game::ServerGame<'_>| {
                game.side
                    .pending_sounds
                    .iter()
                    .filter(|s| s.event == blast_sound)
                    .count()
            };
            assert_eq!(blast_count(game), 1);
            let powered = game
                .with_props(gate, &[("open", "true"), ("powered", "true")])
                .unwrap();
            game.world.set(x, y, z, powered).unwrap();
            game.explosion_trigger_blocks(&mut [pos]).unwrap();
            assert_eq!(game.world.block(x, y, z), powered);
            assert_eq!(blast_count(game), 1);
        });
    }
}

#[cfg(feature = "client")]
#[cfg(test)]
#[path = "movement_input_tests_client.rs"]
pub(in crate::game) mod movement_input_tests;

#[cfg(feature = "client")]
#[cfg(test)]
#[path = "frozen_screen_effects_tests_client.rs"]
pub(in crate::game) mod frozen_screen_effects_tests;

#[cfg(feature = "client")]
#[cfg(test)]
#[path = "player_tick_tests_client.rs"]
pub(in crate::game) mod player_tick_tests;

#[cfg(feature = "client")]
#[cfg(test)]
#[path = "rider_tick_tests_client.rs"]
pub(in crate::game) mod rider_tick_tests;
