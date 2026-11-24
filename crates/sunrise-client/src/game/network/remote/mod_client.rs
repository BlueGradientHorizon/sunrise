#[allow(unused_imports)]
pub(in crate::game::client_side) use super::raw_backlog::Received;
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::remote_chunks::{
    ChunkDecoder, ChunkFormat, Next, RemoteLight,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::remote_vehicles::PassengerSeat;
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::render_tick::{
    mob_tracking_body, player_tracking_body,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::villager_client::builtin_registries;
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{
    Dim, Game, Hit, Mining, Screen, item_use::ticks_left, level_sounds::LevelSound,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::api_values::TryIntoApi;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::collision_source::Mover;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::BACKLOG_LIMIT;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::CHUNK_PACKETS_PER_FRAME;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::ClientMenu;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::DryChanges;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::DryPlace;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::DrySound;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::DryUse;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::ItemUseAim;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::Lerp;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::MOUNT_MENU;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::NoTerrain;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::OwnProfile;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::PendingCauldron;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::PendingItem;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::PlayerTextures;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::Remote;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::RemoteDimensionEntry;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::RemoteInputWorld;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::RemoteUseWorld;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::SHIFT_INPUT;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::ServicesKeys;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::TICK_DECODE_WAIT;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::Tracked;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::boat_interact_passes;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::break_prediction::Prediction;
#[allow(unused_imports)]
#[cfg(test)]
pub(in crate::game::client_side) use crate::game::remote::chat::Decoration;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::chat_after_event;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::chat_received::{
    QueuedChat, QueuedChatKind,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::client_fluid_step;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::client_has_level;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::client_millis;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::client_view_distance;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::dry_level::DryLevel;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::entity_sound_state;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::face_direction;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::general::RemoteGeneral;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::hand_id;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::hand_operated;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::hold_server_fire;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::input_ordinal;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::inventory::{
    CREATIVE_SYNCED, Predicted, changed_slots, creative_sync_slot, creative_sync_stacks,
    mark_synced, menu_items, predict_clicks, predict_clicks_with,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::item_frame_interact_passes;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::item_has_use;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::keep_client_state;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::look_at_rotation;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::menu_title;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::minecarts::CartLerp;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::mob_pos_rot;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::movement::MovePacket;
#[allow(unused_imports)]
#[cfg(test)]
pub(in crate::game::client_side) use crate::game::remote::movement::stepped_move;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::named_hand_operated;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::number_format;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::object_skin_changed;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::place_interpolated;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::player_voice_pitch;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::position_packet_pitch;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::remote_object_swings;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::remote_player_head_turn;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::remote_text;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::remote_use_swings;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::seat_passengers;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::selector_metadata::hurt_sound_for;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::shape_meets;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::skin_entries;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::snap_target;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::snapshot_value;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::synced_local_use;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::use_ends_click;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::wrap_degrees;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::InteractionKey;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::{Duration, Instant},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::first_person::{Hand, SwingAnimation};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    Result,
    blocks::Fluid,
    chunk::Chunk,
    containers::{BlockEntity, Furnace, FurnaceKind},
    entities::{Experience, ExperienceOrb, ItemEntity},
    hud::HudBatch,
    items::ItemStack,
    join_seq::JoinSeq,
    lan::data::{BIOME_REGISTRY, client_biomes},
    menu::Input,
    mobs::{Mob, MobType, Who},
    motion::{MotionWorld, PlayerMotion},
    net::{Event, Session},
    particles::TrackingTarget,
    random_source,
    save::SaveDir,
    screen_effects,
    sprint::Keys,
    streaming::{ChunkGenerator, Proto, Streaming},
    tab_list::SPECTATOR,
    text::Text,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::{
    geometry::Vec3, profile::AccountProperty, random::RandomSource,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_protocol::{
    entity_data::{ITEM_ENTITY_ITEM, Value},
    item::{Hashed, Stack},
    packets::{
        Empty,
        play::{self, Clientbound as C, Serverbound as S},
    },
    registry::{Builtin, DimensionType, Dynamic, SyncedNames, ceil_log2},
    rsa::RsaPublicKey,
    secure_chat::{self, Signature},
    text::{Language, plain},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_server_files::profiles::ServicesKeySet;
#[allow(unused_imports)]
pub(in crate::game::client_side) use {
    sunrise_core::nbt::Tag,
    sunrise_mod_runtime::attachments::{Holder, ResidentAttachments},
    sunrise_world::{
        level::InteractionResult,
        registry::BlockStateId,
        stepped_interpolation::{Path, PosRot, Start, SteppedInterpolation},
    },
};

#[path = "attack_target_client.rs"]
pub(in crate::game) mod attack_target;

#[path = "connection_client.rs"]
pub(in crate::game) mod connection;

#[path = "fall_sound_client.rs"]
pub(in crate::game) mod fall_sound;

#[path = "interaction_actions_client.rs"]
pub(in crate::game) mod interaction_actions;

#[path = "interaction_prediction_client.rs"]
pub(in crate::game) mod interaction_prediction;

#[path = "raw_ingress_client.rs"]
pub(in crate::game) mod raw_ingress;

#[path = "recipe_properties_client.rs"]
pub(in crate::game) mod recipe_properties;

#[path = "abilities_client.rs"]
pub(in crate::game) mod abilities;

#[path = "chat_client.rs"]
pub(in crate::game) mod chat;

#[path = "client_level_tick_client.rs"]
pub(in crate::game) mod client_level_tick;

#[path = "effect_client.rs"]
pub(in crate::game) mod effect;

#[path = "entity_section_steps_client.rs"]
pub(in crate::game) mod entity_section_steps;

#[path = "item_use_client.rs"]
pub(in crate::game) mod item_use;

#[path = "kept_columns_client.rs"]
pub(in crate::game) mod kept_columns;

#[path = "menu_interactions_client.rs"]
pub(in crate::game) mod menu_interactions;

#[path = "packets_entity_client.rs"]
pub(in crate::game) mod packets_entity;

#[path = "packets_object_client.rs"]
pub(in crate::game) mod packets_object;

#[path = "packets_player_client.rs"]
pub(in crate::game) mod packets_player;

#[path = "packets_world_client.rs"]
pub(in crate::game) mod packets_world;

#[path = "session_client.rs"]
pub(in crate::game) mod session;

#[path = "skin_client.rs"]
pub(in crate::game) mod skin;

#[path = "status_client.rs"]
pub(in crate::game) mod status;

#[path = "sync_client.rs"]
pub(in crate::game) mod sync;

#[path = "tick_client.rs"]
pub(in crate::game) mod tick;

#[path = "tracking_client.rs"]
pub(in crate::game) mod tracking;

#[path = "ui_client.rs"]
pub(in crate::game) mod ui;

#[path = "use_client.rs"]
pub(in crate::game) mod r#use;

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
        self.0.client_item_use.0.is_some() || self.0.using.is_some()
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

impl Game<'_> {
    pub(in crate::game) fn apply_packet(&mut self, packet: C) -> Result<()> {
        self.observe_client_packet(&packet);
        match packet {
            // ClientCommonPacketListenerImpl.handlePing runs on the game
            // thread, after all preceding packets (especially teleports).
            C::Ping(p) => self.send(S::Pong(p)),
            C::CustomPayload(p) if p.channel == super::cauldron_prediction::ENTRIES_CHANNEL => {
                let entries = super::cauldron_prediction::decode_registrations(&p.data)?;
                if let Some(remote) = &mut self.remote {
                    remote.cauldron_entries = Some(entries);
                }
            }
            C::CustomPayload(p) if p.channel == super::cauldron_prediction::USE_CHANNEL => {
                self.receive_cauldron_use(&p.data)?;
            }
            // The server's next generation of mod registries: all of it or the connection.
            C::CustomPayload(p) if p.channel == sunrise_client::datapack_net::CHANNEL => {
                sunrise_client::datapack_net::accept_chunk(&p.data)?;
            }
            C::CustomPayload(p)
                if p.channel == sunrise_client::lan::commands::selector_sync::CHANNEL =>
            {
                self.receive_selector_tokens(&p.data)?;
            }
            // Another level's world: its open, its packets and its close (`viewed_worlds`).
            C::CustomPayload(p) if p.channel == sunrise_protocol::viewed_levels::CHANNEL => {
                self.viewed_levels_payload(&p.data)?;
            }
            // Private custom recipe-book settings and placement protocol.
            C::CustomPayload(p)
                if p.channel == sunrise_client::recipe_book::settings_sync::CHANNEL =>
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
            C::CustomPayload(p) if sunrise_client::lan::recipes::viewer_channel(&p.channel) => {
                self.remote_recipe_viewer(&p.channel, &p.data);
            }
            // An anvil cost the data-slot short cannot hold, after that short.
            C::CustomPayload(p) if p.channel == sunrise_protocol::anvil_cost::CHANNEL => {
                self.remote_anvil_wide_cost(&p.data);
            }
            // The server's recipes its native mods asked to send.
            C::CustomPayload(p) if p.channel == sunrise_client::lan::recipes::CONTENT_CHANNEL => {
                self.remote_recipe_content(&p.data);
            }
            // The passages of the chunks the server shows the window.
            C::CustomPayload(p) if p.channel == sunrise_protocol::passage_sync::CHANNEL => {
                self.remote_passage_sync(&p.data);
            }
            // PingDebugMonitor.onPongReceived.
            C::PongResponse(p) => {
                let delay = sunrise_client::net::ping_millis().wrapping_sub(p.time);
                self.client.debug_charts.ping.log(delay);
            }
            C::DebugSample(p) => self.client.debug_charts.log_remote_sample(&p.sample)?,
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
            C::LevelEvent(p) => self.level_event_at(p.kind, p.pos, p.data),
            C::LevelParticles(p) => self.remote_level_particles(&p),
            C::Explode(p) => self.remote_explode(&p),
            C::Sound(p) => self.apply_sound_packet(p),
            C::SoundEntity(p) => self.apply_sound_entity_packet(p),
            C::StopSound(p) => self
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
                    self.remote.as_mut().and_then(|r| r.entities.get_mut(&m.id)),
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
                let show_death = self.remote.as_ref().is_none_or(|r| r.show_death_screen);
                self.close_screen_to_level(show_death)?;
            }
            C::TagQuery(p) => self.answer_tag_query(p.transaction, p.tag),
            C::CommandSuggestions(p) => self.handle_command_suggestions(p),
            C::CustomChatCompletions(p) => {
                // ClientPacketListener.handleCustomChatCompletions.
                if let Some(r) = self.remote.as_mut() {
                    sunrise_client::command::modify_custom_completions(
                        &mut r.custom_completions,
                        sunrise_client::command::CustomCompletionAction::from_id(p.action),
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
                if let Some(r) = self.remote.as_mut() {
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
            C::TickingState(p) if self.remote.as_ref().is_some_and(Remote::has_level) => {
                self.clock.set_rate(p.rate);
                self.clock.set_frozen(p.frozen);
            }
            C::TickingStep(p) if self.remote.as_ref().is_some_and(Remote::has_level) => {
                self.clock.set_frozen_ticks_to_run(p.steps);
            }
            C::TickingState(_) | C::TickingStep(_) => {}
            C::UpdateAdvancements(p) => self.remote_advancements(p),
            C::AwardStats(p) => self.apply_award_stats_packet(p)?,
            // ClientPacketListener.setActionBarText, setTitleText,
            // setSubtitleText, setTitlesAnimation and handleTitlesClear.
            C::BossEvent(p) => self.apply_boss_event(&p),
            C::SetActionBarText(p) => self.client.titles.set_overlay(remote_text(&p.text), false),
            C::SetTitleText(p) => self.client.titles.set_title(remote_text(&p.text)),
            C::SetSubtitleText(p) => self.client.titles.set_subtitle(remote_text(&p.text)),
            C::SetTitlesAnimation(p) => self.client.titles.set_times(p.fade_in, p.stay, p.fade_out),
            // ClientCommonPacketListenerImpl.handleShowDialog / handleClearDialog.
            C::ShowDialog(p) => self.show_play_dialog(&p.dialog),
            C::ClearDialog(_) => self.clear_dialog(),
            // ClientPacketListener.handleLowDiskSpaceWarning.
            C::LowDiskSpaceWarning(_) => self.client_reports.0.report_low_disk_space(),
            C::ClearTitles(p) => self.apply_clear_titles_packet(p),
            _ => {}
        }
        Ok(())
    }
    /// `ServerReconfigScreen` over the panorama; Disconnect wakes up after
    /// `DISCONNECT_TIME` (600 ticks).
    pub(in crate::game) fn draw_reconfiguring(&self, batch: &mut HudBatch, since: u64) {
        let atlas = &self.gui.atlas;
        let (w, h) = (batch.width, batch.height);
        sunrise_client::ui::menu_background(batch, atlas, 0.0, 0.0, w, h, false);
        let title = sunrise_client::text::Text::translate("connect.reconfiguring", &[]);
        let ((tx, ty), (bx, by)) = self.reconfigure_layout((w, h));
        batch.text_styled(atlas, &title, tx, ty, u32::MAX, true);
        // CommonComponents.GUI_DISCONNECT
        let label = sunrise_client::text::Text::translate("menu.disconnect", &[]);
        let active = self.ticks.saturating_sub(since) >= 600;
        let hover = sunrise_client::ui::hovered(self.client.cursor, bx, by, 150.0, 20.0);
        sunrise_client::ui::button(batch, atlas, bx, by, 150.0, 20.0, &label, active, hover);
    }
    pub(in crate::game) fn poll_remote_with(&mut self, before_tick: bool) -> Result<()> {
        let polled = self.poll_remote_events(before_tick);
        // The block events and block changes of a piston clock start and end
        // thousands of moves at once: the collision view and the renderer
        // take them all in one sync, before the tick or frame reads them.
        if std::mem::take(&mut self.client.moving_pistons_stale) {
            self.sync_moving_pistons();
        }
        polled
    }
    pub(in crate::game) fn remote_player_uuid(&self, id: u32) -> Option<u128> {
        let remote = self.remote.as_ref()?;
        if self
            .client
            .player_model
            .as_ref()
            .is_some_and(|m| m.id == id)
        {
            Some(remote.profile)
        } else {
            remote.level_players.get(&(id as i32)).copied()
        }
    }
}

#[cfg(test)]
#[path = "claimed_item_use_tests_client.rs"]
pub(in crate::game) mod claimed_item_use_tests;

#[cfg(test)]
#[path = "gamemode_actions_tests_client.rs"]
pub(in crate::game) mod gamemode_actions_tests;

#[cfg(test)]
#[path = "chat_audit_tests_client.rs"]
pub(in crate::game) mod chat_audit_tests;

#[cfg(test)]
#[path = "interaction_event_tests_client.rs"]
pub(in crate::game) mod interaction_event_tests;

#[cfg(test)]
#[path = "native_entity_tests_client.rs"]
pub(in crate::game) mod native_entity_tests;

#[cfg(test)]
#[path = "camera_view_tests_client.rs"]
pub(in crate::game) mod camera_view_tests;

#[cfg(test)]
#[path = "level_sound_tests_client.rs"]
pub(in crate::game) mod level_sound_tests;

#[cfg(test)]
#[path = "riptide_tests_client.rs"]
pub(in crate::game) mod riptide_tests;

#[cfg(test)]
pub(in crate::game) mod tests {
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use super::super::api_values::TryIntoApi;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use super::Game;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use super::{
        boat_interact_passes, chat_after_event, item_frame_interact_passes, remote_object_swings,
    };
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::remote::tests::connect_menu_test;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::remote::tests::connect_play_test;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::remote::tests::finish_play_test;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::remote::tests::offer_test_payloads;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::remote::tests::spawn_menu_test;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::remote::tests::*;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use sunrise_client::{
        containers::{Beacon, BlockEntity},
        items::ItemStack,
    };
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use sunrise_core::nbt::Tag;

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
        use sunrise_client::{
            chunk::Chunk,
            lan::{Out, PlayerSender},
            mod_content::{ContentMod, resource_root},
        };
        use sunrise_core::position::BlockPos;
        use sunrise_mod_package::{
            content::{Block, Content, GateSound},
            enabled::EnabledMods,
        };
        use sunrise_protocol::{buf::Reader, packets::play::Clientbound};

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
        with_guests_at(root, |game| {
            let pos = [3, 64, 4];
            let [x, y, z] = pos;
            let hit = crate::game::Hit {
                pos,
                face: [0, 1, 0],
                point: [3.5, 64.875, 4.5],
                inside: false,
            };
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
            let open = |game: &crate::game::Game<'_>| {
                let b = game.res.behaviours();
                b.table.bool(game.world.block(x, y, z), b.props.open)
            };

            let [actor, other] = [12, 13].map(|conn| {
                let (tx, rx) = std::sync::mpsc::channel();
                game.lan
                    .as_mut()
                    .unwrap()
                    .hub
                    .update_guest(conn, |guest| {
                        guest.sender = PlayerSender::new(tx, Arc::default());
                    })
                    .unwrap();
                rx
            });
            let sounds = |game: &crate::game::Game<'_>, rx: &std::sync::mpsc::Receiver<Out>| {
                let sync = game.lan.as_ref().unwrap().hub.sync_data();
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
            let blocks = mod_api::SoundSource::Blocks.index();
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
                game.audio.0.capture = Some(Vec::new());
                let (answer, dry) = game.remote_block_reacts(&hit, None, true);
                assert!(answer.consumes_action());
                game.predict_use(&dry, 1);
                assert!(open(game), "the gate opens without a server round trip");
                let capture = game.audio.0.capture.as_mut().unwrap();
                assert_eq!(capture.len(), 1);
                let predicted = capture.pop().unwrap();
                assert_eq!((predicted.event.as_str(), predicted.source), (name, blocks));

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
                assert!(open(game));
                game.lan_broadcast_effects();
                assert!(sounds(game, &actor).is_empty(), "no echo to the clicker");
                assert_eq!(sounds(game, &other), [(name.into(), blocks as i32, 1.0)]);
            }

            game.explosion_trigger_blocks(&mut [pos]).unwrap();
            assert!(!open(game));
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
            game.process_level_events().unwrap();
            assert!(open(game));
            game.lan_broadcast_effects();
            for listener in [&actor, &other] {
                assert_eq!(
                    sounds(game, listener),
                    [(wire_sound.into(), blocks as i32, 1.0)]
                );
            }
            assert_eq!(game.audio.0.heard.get(blast_sound), Some(&1));
            let powered = game
                .with_props(gate, &[("open", "true"), ("powered", "true")])
                .unwrap();
            game.world.set(x, y, z, powered).unwrap();
            game.explosion_trigger_blocks(&mut [pos]).unwrap();
            assert_eq!(game.world.block(x, y, z), powered);
            assert_eq!(game.audio.0.heard.get(blast_sound), Some(&1));
        });
    }
}

#[cfg(test)]
#[path = "movement_input_tests_client.rs"]
pub(in crate::game) mod movement_input_tests;

#[cfg(test)]
#[path = "frozen_screen_effects_tests_client.rs"]
pub(in crate::game) mod frozen_screen_effects_tests;

#[cfg(test)]
#[path = "player_tick_tests_client.rs"]
pub(in crate::game) mod player_tick_tests;

#[cfg(test)]
#[path = "rider_tick_tests_client.rs"]
pub(in crate::game) mod rider_tick_tests;
