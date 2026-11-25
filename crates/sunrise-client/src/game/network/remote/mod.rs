//! Multiplayer: the game driven by a server connection instead of the local
//! simulation. Mirrors vanilla's ClientPacketListener (chunks, entities,
//! inventory, health, time, weather, chat, respawn and dimension changes),
//! LocalPlayer.sendPosition (movement packets) and MultiPlayerGameMode
//! (block breaking with sequence numbers, use-item, attacks, container
//! clicks). The player still moves with the local physics, as the vanilla
//! client predicts its own movement; the server stays authoritative for
//! everything else.
pub(in crate::game) mod selector_metadata;
#[allow(unused_imports)]
use crate::game::collision_source::Mover;
#[allow(unused_imports)]
use selector_metadata::hurt_sound_for;
pub(in crate::game) mod attack_target;
pub(in crate::game) mod break_prediction;
#[allow(unused_imports)]
use break_prediction::Prediction;
pub(in crate::game) mod connection;
pub(in crate::game) mod fall_sound;
pub(in crate::game) mod general;
#[allow(unused_imports)]
pub(in crate::game) use general::RemoteGeneral;
pub(in crate::game) mod interaction_actions;
pub(in crate::game) mod interaction_prediction;
#[allow(unused_imports)]
use inventory::{
    CREATIVE_SYNCED, Predicted, changed_slots, creative_sync_slot, creative_sync_stacks,
    mark_synced, menu_items, predict_clicks, predict_clicks_with,
};
pub(in crate::game) mod level_format;
pub(in crate::game) mod raw_ingress;
pub(in crate::game) mod recipe_properties;
pub(in crate::game) mod use_item;

pub(in crate::game) mod block_entity_data;
#[cfg(test)]
pub(in crate::game) mod block_entity_data_tests;
#[cfg(test)]
pub(in crate::game) mod claimed_item_use_tests;
#[cfg(test)]
pub(in crate::game) mod gamemode_actions_tests;
#[allow(unused_imports)]
use super::raw_backlog::Received;
#[allow(unused_imports)]
use super::remote_chunks::{ChunkDecoder, ChunkFormat, Next, RemoteLight};
#[allow(unused_imports)]
use super::remote_vehicles::PassengerSeat;
#[allow(unused_imports)]
use super::render_tick::{mob_tracking_body, player_tracking_body};
#[allow(unused_imports)]
use super::{Dim, Game, Hit, Mining, Screen, item_use::ticks_left, level_sounds::LevelSound};
#[allow(unused_imports)]
use crate::game::api_values::TryIntoApi;
#[allow(unused_imports)]
use mod_api::InteractionKey;
#[allow(unused_imports)]
use movement::MovePacket;
#[allow(unused_imports)]
#[cfg(test)]
// The move-packet tests reach it through `super::super`.
use movement::stepped_move;
#[allow(unused_imports)]
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::{Duration, Instant},
};
#[allow(unused_imports)]
use sunrise_client::first_person::{Hand, SwingAnimation};
#[allow(unused_imports)]
use sunrise_client::{
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
use sunrise_core::{geometry::Vec3, profile::AccountProperty, random::RandomSource};
#[allow(unused_imports)]
use sunrise_protocol::{
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
use sunrise_server_files::profiles::ServicesKeySet;
#[allow(unused_imports)]
use {
    sunrise_core::nbt::Tag,
    sunrise_mod_runtime::attachments::{Holder, ResidentAttachments},
    sunrise_world::{
        level::InteractionResult,
        registry::BlockStateId,
        stepped_interpolation::{Path, PosRot, Start, SteppedInterpolation},
    },
};

pub(in crate::game) mod abilities;
pub(in crate::game) mod chat;
#[cfg(test)]
pub(in crate::game) mod chat_audit_tests;
pub(in crate::game) mod chat_received;
pub(in crate::game) mod commands;
#[allow(unused_imports)]
#[cfg(test)]
pub(in crate::game) use chat::Decoration;
#[allow(unused_imports)]
use chat_received::{QueuedChat, QueuedChatKind};
pub(in crate::game) mod client_level_tick;
pub(in crate::game) mod effect;
pub(in crate::game) mod entity_data_schema;
pub(in crate::game) mod entity_section_steps;
pub(in crate::game) mod inventory;
pub(in crate::game) mod item_use;
pub(in crate::game) mod kept_columns;
pub(in crate::game) mod menu_interactions;
pub(in crate::game) mod minecarts;
#[allow(unused_imports)]
use minecarts::CartLerp;
pub(in crate::game) mod motion;
pub(in crate::game) mod movement;
pub(in crate::game) mod packets_entity;
pub(in crate::game) mod packets_object;
pub(in crate::game) mod packets_player;
pub(in crate::game) mod packets_world;
pub(in crate::game) mod remote_attributes;
pub(in crate::game) mod remote_state;
pub(in crate::game) mod session;
pub(in crate::game) mod skin;
pub(in crate::game) mod status;
pub(in crate::game) mod sync;
pub(in crate::game) mod tab_entries;
pub(in crate::game) mod tables;
pub(in crate::game) mod tick;
pub(in crate::game) mod tracking;
pub(in crate::game) mod ui;
#[path = "use.rs"]
pub(in crate::game) mod r#use;

#[allow(unused_imports)]
pub(in crate::game) use super::villager_client::builtin_registries;

pub(in crate::game) mod chat_queue;

/// A text component the server sent, for the HUD.
pub(in crate::game) fn remote_text(tag: &Tag) -> sunrise_client::text::Text {
    use sunrise_client::scoreboard::{comp_text, tag_to_json};
    comp_text(&tag_to_json(tag))
}

/// `Player.getVoicePitch`: a constant 1.0
/// (`javap -p -c -constants net.minecraft.world.entity.player.Player`:
/// `0: fconst_1  1: freturn`), so a player's own hurt and death sounds are
/// neither raised for a baby nor jittered - `LivingEntity.getVoicePitch`, which
/// the mobs port, does both. It takes no random, so it draws nothing from the
/// level random either.
pub(in crate::game) fn player_voice_pitch() -> f32 {
    1.0
}

/// The live entity a bound sound names; absence means that object was removed.
pub(in crate::game) fn entity_sound_state(
    id: i32,
    player: Option<(i32, [f64; 3], bool)>,
    mob: Option<sunrise_client::sound_engine::SoundEntityState>,
    tracked: &HashMap<i32, Tracked>,
    dragon: Option<(i32, [f64; 3])>,
) -> Option<sunrise_client::sound_engine::SoundEntityState> {
    use sunrise_client::sound_engine::SoundEntityState;
    if let Some((player_id, position, silent)) = player
        && player_id == id
    {
        return Some(SoundEntityState {
            position,
            silent,
            removed: false,
        });
    }
    let mut state = mob.or_else(|| {
        tracked.get(&id).map(|t| SoundEntityState {
            position: t.position,
            silent: t.silent,
            removed: false,
        })
    })?;
    // The dragon's Mob is metadata storage; only Dragon follows its movement.
    if let Some((dragon_id, position)) = dragon
        && dragon_id == id
    {
        state.position = position;
    }
    Some(state)
}

/// `ClientboundOpenScreenPacket.title` as a container screen draws it: the
/// component's styles, and the `§` codes a literal holds (`StringDecomposer`).
pub(in crate::game) fn menu_title(tag: &Tag) -> sunrise_client::text::Text {
    remote_text(tag).resolve_legacy()
}

/// Streaming needs a generator; a server world never generates.
pub(in crate::game) struct NoTerrain;
impl ChunkGenerator for NoTerrain {
    type Worker = ();
    fn worker(&self) {}
    fn proto(&self, _: &mut (), _: i32, _: i32) -> Proto {
        Box::new(())
    }
    fn finish(&self, _: Proto) -> Chunk {
        Chunk::empty(0, 0)
    }
}

/// A server entity the client tracks (position as last sent).
pub(in crate::game) struct Tracked {
    pub(in crate::game) kind: String,
    pub(in crate::game) position: [f64; 3],
    pub(in crate::game) previous: [f64; 3],
    pub(in crate::game) yaw: f32,
    pub(in crate::game) pitch: f32,
    pub(in crate::game) previous_yaw: f32,
    pub(in crate::game) previous_pitch: f32,
    /// `Entity.getPositionCodec`'s base: the position the server last
    /// reported (AddEntity, MoveEntity, EntityPositionSync), which a move
    /// packet's delta counts from. A TeleportEntity moves the entity but not
    /// this base, so the server's next delta still lands where it means.
    pub(in crate::game) codec_base: [f64; 3],
    pub(in crate::game) on_fire: bool,
    pub(in crate::game) mod_fire_immune: bool,
    pub(in crate::game) silent: bool,
    /// `Entity.vehicle`: the entity it rides (`SetPassengers`), whatever either one's type.
    pub(in crate::game) vehicle: Option<u32>,
    /// `Entity.tickCount` of this client's copy: the ticks the client level gave it since the
    /// server added it (`Entity.commonTick`), counted in `tick_remote_entities`.
    pub(in crate::game) tick_count: i32,
    /// What `entity_size` answered at this copy's last size refresh, with the synced pose it
    /// was asked for, unless the copy is a mob, which keeps its own (`Mob::size_answer`).
    pub(in crate::game) size_answer: Option<sunrise_client::pose::SizeAnswer>,
}

/// An item has joined the client level, but has no stack to draw yet.
pub(in crate::game) struct PendingItem {
    pub(in crate::game) join: JoinSeq,
    pub(in crate::game) position: [f64; 3],
    pub(in crate::game) velocity: Vec3,
    /// Entity.DATA_NO_GRAVITY, if it came before the stack.
    pub(in crate::game) no_gravity: bool,
}

/// `Entity.startRiding` / `removePassenger` for a `SetPassengers` packet: the
/// listed entities ride `vehicle`, the ones that rode it and are no longer
/// listed got off. Every tracked entity counts, whatever its type, so a
/// server's item display or marker on a player hides that player's name tag
/// as vanilla's `isVehicle` does (#5666).
pub(in crate::game) fn seat_passengers(
    entities: &mut HashMap<i32, Tracked>,
    vehicle: u32,
    passengers: &[i32],
) {
    for (id, t) in entities.iter_mut() {
        if passengers.contains(id) {
            t.vehicle = Some(vehicle);
        } else if t.vehicle == Some(vehicle) {
            t.vehicle = None;
        }
    }
}
/// Smooth an entity toward a new position over three ticks (InterpolationHandler).
#[derive(Clone, Copy)]
/// `LinearInterpolationHandler`'s target: a position and rotation (yRot,
/// xRot) reached in `steps` ticks.
pub(in crate::game) struct Lerp {
    pub(in crate::game) target: [f64; 3],
    pub(in crate::game) rotation: Option<(f32, f32)>,
    pub(in crate::game) steps: u32,
}

/// `SteppedInterpolationHandler.doInterpolate`'s `setPos` and `setRot` on a
/// mob: its yaw kept continuous (the same turn as vanilla's, without the
/// wrap that would spin it in the frame between ticks), its pitch clamped.
pub(in crate::game) fn place_interpolated(m: &mut Mob, at: PosRot) {
    m.position = Vec3::new(at.position[0], at.position[1], at.position[2]);
    m.yaw += wrap_degrees(at.yaw - m.yaw);
    m.head_pitch = (at.pitch % 360.0).clamp(-90.0, 90.0);
}

/// Where a mob is, as its `SteppedInterpolationHandler` sees it.
pub(in crate::game) fn mob_pos_rot(m: &Mob) -> PosRot {
    PosRot {
        position: [m.position.x, m.position.y, m.position.z],
        yaw: m.yaw,
        pitch: m.head_pitch,
    }
}

/// `Minecraft.services()` trust keys, fetched once per connection off the game thread.
pub(in crate::game) enum ServicesKeys {
    Pending,
    Ready(ServicesKeySet),
}

/// One `PlayerInfo` property, shared by the list entry and the entities that kept it.
pub(in crate::game) struct PlayerTextures {
    pub(in crate::game) property: AccountProperty,
    pub(in crate::game) unpacked: std::sync::OnceLock<sunrise_client::skins::Textures>,
}

impl PlayerTextures {
    /// `SkinManager.createLookup(profile, requireSecure)`: a secure lookup shows
    /// only textures whose signature the keys verified, so it waits for keys that
    /// are still pending; the local player's shows its textures unverified.
    pub(in crate::game) fn unpack(
        &self,
        services_keys: &ServicesKeys,
        valid_uris: &[impl AsRef<str>],
        require_secure: bool,
    ) -> Option<&sunrise_client::skins::Textures> {
        use sunrise_client::skins;
        let signature = self.property.signature.as_deref();
        let property_keys: &[RsaPublicKey] = match services_keys {
            // Never cache an invalid result for a signature the pending keys could verify.
            ServicesKeys::Pending if signature.is_some() && require_secure => return None,
            ServicesKeys::Pending => &[],
            ServicesKeys::Ready(keys) => &keys.profile_property_keys,
        };
        Some(self.unpacked.get_or_init(|| {
            let value = &self.property.value;
            let signature = skins::property_signature(value, signature, property_keys);
            skins::unpack(value, signature, valid_uris)
        }))
    }
}

/// The signed-in account's `/minecraft/profile` fetch after joining.
pub(in crate::game) enum OwnProfile {
    Idle,
    Fetching(std::sync::mpsc::Receiver<Option<sunrise_client::skins::Textures>>),
    Done(Option<sunrise_client::skins::Textures>),
}

pub(in crate::game) struct PendingCauldron {
    pub(in crate::game) hand: Hand,
    pub(in crate::game) hit: Hit,
    pub(in crate::game) attempt: mod_api::InteractionAttempt,
    pub(in crate::game) selected: usize,
    pub(in crate::game) main: Option<ItemStack>,
    pub(in crate::game) off: Option<ItemStack>,
    pub(in crate::game) aim: ItemUseAim,
    pub(in crate::game) swing: [bool; 2],
    pub(in crate::game) sequence: i32,
    pub(in crate::game) item_after_block: bool,
}

#[derive(PartialEq)]
pub(in crate::game) struct ItemUseAim {
    // UseItem carries these angles but not the server's ray origin.
    pub(in crate::game) rotation: (f32, f32),
    pub(in crate::game) eye: Option<[f64; 3]>,
    // The local ray can also come from a tracked input device.
    pub(in crate::game) ray: Option<([f64; 3], [f64; 3])>,
    pub(in crate::game) reach: f64,
}

/// The packet route that starts a remote level entry.
pub(in crate::game) enum RemoteDimensionEntry {
    Login,
    Respawn,
    DeathRespawn,
}

pub(in crate::game) struct Remote {
    pub(in crate::game) professions: Arc<sunrise_client::mobs::villager::ResolvedProfessions>,
    pub session: Session,
    /// Display-only address computed at connection setup. Never holds the raw friend invite.
    pub(in crate::game) display_address: Option<String>,
    pub brand: String,
    /// `ClientPacketListener.random` draws packet spread, not provider RNG.
    pub(in crate::game) particle_packet_random: sunrise_core::random::LegacyRandom,
    pub(in crate::game) recipe_viewer: crate::game::recipe_book::ViewerChannels,
    /// This connection's configured feature flags, carried through reconfiguration.
    pub(in crate::game) features: std::sync::Arc<[String]>,
    /// The server announced it runs mods' quiet commands.
    pub quiet_commands: bool,
    /// The server advertised private custom recipe-book synchronization.
    pub custom_books: bool,
    /// The synced attachments the server sent, by holder.
    pub attachments: HashMap<Holder, ResidentAttachments>,
    /// The colliders the server sent, by entity id (`sunrise:collider_sync`).
    pub colliders: HashMap<u32, sunrise_client::colliders::ColliderState>,
    /// The colliders at the last client tick, before any packets handled between ticks.
    pub(in crate::game) collider_shove_start:
        Option<(Dim, sunrise_client_render::moving_colliders::Prepared)>,
    /// How old the copy of the collider carrying the player is (`sunrise:carry_age`).
    pub carry_age: sunrise_client::colliders::CarryAge,
    /// The server samples far terrain for us, this far at most (`lod_net`).
    pub far_terrain: Option<sunrise_client::lod_net::Offer>,
    pub(in crate::game) builtin: Arc<Builtin>,
    pub(in crate::game) lang: Arc<Language>,
    pub(in crate::game) registries: Arc<Dynamic>,
    /// The block, item and fluid tags of `registries` by name, as native mods read them
    /// (`install_remote_tags`).
    pub(in crate::game) tags: Option<sunrise_client::items::Items>,
    /// The game's enchantments and the server's that it lacks (a data
    /// pack's), from their synced data; `None` when the server has none.
    pub(in crate::game) enchantments: Option<Arc<sunrise_client::enchant::Enchantments>>,
    pub(in crate::game) joined: bool,
    /// `ServerReconfigScreen` is up since this tick.
    pub(in crate::game) reconfiguring: Option<u64>,
    /// `handleLogin` ran: vanilla's `Minecraft.player` exists from then on,
    /// while here the player is made at the first teleport.
    pub(in crate::game) logged_in: bool,
    /// Whether a logical `LocalPlayer` already exists on this connection (retained across
    /// a second `Login`, replaced on `Respawn`, cleared on reconfiguration).
    pub(in crate::game) has_logical_player: bool,
    pub(in crate::game) status: String,
    pub(in crate::game) entity_id: i32,
    pub(in crate::game) local_entity_silent: bool,
    /// `Minecraft.cameraEntity` when the server put the view on another
    /// entity (ClientboundSetCameraPacket); None while it is the player.
    pub(in crate::game) camera: Option<i32>,
    /// Detached camera snapshot retained after `RemoveEntities` until `SetCamera` replaces it.
    pub(in crate::game) detached_camera:
        Option<Box<crate::game::remote_camera::ResolvedCameraEntity>>,
    pub(in crate::game) dimension: DimensionType,
    /// The level's key (`minecraft:overworld`): where its far terrain is kept.
    pub(in crate::game) level: String,
    /// `level`'s key, as mods' handles name the level.
    pub(in crate::game) level_key: mod_api::DimensionKey,
    /// The level's facts from the dimension type this connection's server sent (`None` when it
    /// sent none the game takes): owned by the connection, so entering a level touches no other
    /// level's facts, the integrated server's included.
    pub(in crate::game) level_facts: Option<Arc<sunrise_world::dimension_type::DimensionFacts>>,
    /// The sky, fog and cloud attributes (and timelines) of the level from the server's type over
    /// the client's own environment of that type: what gameplay reads. `None` when the type is
    /// refused, or has no data at all: neither the server sent it nor the client's resources hold
    /// it (`fill_known_entries` fills a known pack's entries from the client's own files).
    pub(in crate::game) level_environment: Option<Arc<sunrise_client::environment::Environment>>,
    /// The same over the packs' Polytone stack of that type, what the frame draws (`None` without
    /// pack modifiers, or for a type the packs have no stack of: any but the three vanilla IDs).
    pub(in crate::game) level_pack_environment:
        Option<Arc<sunrise_client::environment::Environment>>,
    /// The level's synchronized dimension type, which both environments are built from again
    /// when the packs change.
    pub(in crate::game) synced_type: Option<(String, serde_json::Value)>,
    /// Both environments by synchronized type name, with what they were built from, so that a
    /// level of an unchanged type is entered again without reading the biome files.
    pub(in crate::game) level_environments: HashMap<String, level_format::LevelEnvironments>,
    /// Connection-owned facts, cached by level and the full named type identity.
    pub(in crate::game) facts_cache: HashMap<
        String,
        (
            String,
            serde_json::Value,
            Arc<sunrise_world::dimension_type::DimensionFacts>,
        ),
    >,
    /// The levels this connection added to the process's table of levels: the only ones it drops.
    pub(in crate::game) added_levels: std::collections::HashSet<String>,
    /// `ClientPlayerNetworkEvent.LoggingOut` was fired for this connection: every way out
    /// of a world reports it once.
    pub(in crate::game) logged_out: bool,
    /// The other levels' worlds the server opened on `sunrise:levels` (`viewed_worlds`).
    pub(in crate::game) viewed: super::viewed_worlds::ViewedWorlds,
    /// The level's hashed seed (login, respawn): the folder of its kept chunks
    /// (`server_terrain::level_dir`).
    pub(in crate::game) hashed_seed: i64,
    /// `ClientLevel.getSeaLevel`: from the packet that created this level.
    pub(in crate::game) sea_level: i32,
    /// What chunk packets decode with (the dimension's height and sky, the
    /// biome registry); replaced whenever one of them changes.
    pub(in crate::game) chunk_format: Arc<ChunkFormat>,
    /// Decodes chunk packets as they arrive, off the frame.
    pub(in crate::game) decoder: ChunkDecoder,
    /// Options.serverRenderDistance: the server's view distance from the
    /// login or the last chunk-cache-radius packet.
    pub(in crate::game) view_distance: i32,
    /// `ClientLevel.serverSimulationDistance`: from the login or the last
    /// simulation-distance packet.
    pub(in crate::game) simulation_distance: i32,
    /// `BlockStatePredictionHandler.currentSequenceNr`: the level's, so a new level (a login,
    /// another dimension) counts from 0 again.
    pub(in crate::game) sequence: i32,
    /// `BlockStatePredictionHandler.lastTeleportSequence`: the sequence of the last
    /// teleport, so a prediction made before it keeps its block but not its snap.
    pub(in crate::game) last_teleport_sequence: i32,
    pub(in crate::game) position_sender: sunrise_client::player_sync::PositionSender,
    pub(in crate::game) last_input: u8,
    pub(in crate::game) was_sprinting: bool,
    /// The slot the server last heard (`MultiPlayerGameMode.carriedIndex`): a cell, so `send` can
    /// send a new one first (`ensureHasSentCarriedItem`).
    pub(in crate::game) carried: std::cell::Cell<Option<usize>>,
    pub(in crate::game) was_shared_flags: u8,
    pub(in crate::game) was_living_flags: u8,
    pub(in crate::game) last_recorded_pose: Option<crate::pose::Pose>,
    /// Live `LocalPlayer.abilities`, mutated in arrival order by `CHANGE_GAME_MODE` and `PlayerAbilities`.
    pub(in crate::game) abilities: sunrise_client::lan::abilities::PlayerAbilities,
    /// The old player's motion, from a Respawn with KEEP_ENTITY_DATA
    /// (`handleRespawn`: `newPlayer.setDeltaMovement(oldPlayer.getDeltaMovement())`),
    /// for the player made at the next teleport: a portal's teleport adds to it.
    pub(in crate::game) respawn_velocity: Option<Vec3>,
    /// ServerboundPlayerLoadedPacket went out: the tick's keybinds act from then on.
    pub(in crate::game) loaded_sent: bool,
    /// The dimension of the client level the mods heard made
    /// (`native_client_level_load`), to be told dropped once.
    pub(in crate::game) heard_level: Option<String>,
    /// `ClientPacketListener.levelLoadTracker`: the loading screen after a login
    /// or respawn, until the player's section is shown.
    pub(in crate::game) level_load: sunrise_client::level_loading::LevelLoadTracker,
    /// The ClientInformation last sent (`Options.broadcastOptions` sends it
    /// again when a view distance, Chat, skin or Main Hand option changes).
    pub(in crate::game) sent_info: sunrise_protocol::packets::configuration::ClientInformation,
    pub(in crate::game) group_count: sunrise_client::chunk_map::ConnectionGroupCount,
    /// BlockStatePredictionHandler: blocks changed ahead of the server (a predicted break),
    /// with what it remembers of each of them. Until the server acknowledges the
    /// sequence its block updates only replace the remembered state; then the block
    /// takes it.
    pub(in crate::game) predictions: HashMap<[i32; 3], Prediction>,
    /// The entities the level held when its `LocalPlayer` was added
    /// (`ClientLevel.addEntity` on login or respawn): `ClientLevel.tickEntities`
    /// ticks in the order entities were added, so these tick before the
    /// player and every later one after it.
    pub(in crate::game) ticked_before_player: std::collections::HashSet<u32>,
    /// Open container: (window id, menu name).
    pub(in crate::game) menu: ClientMenu,
    /// The crafting result slot of the open menu as the server last set it
    /// (`CraftingMenu.slotChangedCraftingGrid`): the client never computes it.
    pub(in crate::game) craft_result: Option<ItemStack>,
    /// The open menu's title as the server named it, with its styles.
    pub(in crate::game) menu_title: Option<sunrise_client::text::Text>,
    pub(in crate::game) menu_title_snapshot: Option<Arc<mod_api::Text>>,
    pub(in crate::game) state_id: i32,
    /// Window 0's state and carried stack while another menu is active.
    pub(in crate::game) inventory_state_id: i32,
    pub(in crate::game) inventory_carried: Option<ItemStack>,
    pub(in crate::game) inventory_craft_grid: [Option<ItemStack>; 4],
    pub(in crate::game) inventory_craft_result: Option<ItemStack>,
    pub(in crate::game) use_target: Option<[i32; 3]>,
    pub(in crate::game) cauldron_entries:
        Option<sunrise_client::cauldron::interactions::Dispatcher<()>>,
    /// Receipts are connection-scoped; unlike block prediction sequences, they
    /// are never reset on a level change.
    pub(in crate::game) cauldron_receipt: i64,
    pub(in crate::game) pending_cauldrons: std::cell::RefCell<HashMap<i64, PendingCauldron>>,
    /// A mod menu's extra data, kept for the `OpenScreen` of its window that
    /// follows (`sunrise:menu_extra`).
    pub(in crate::game) menu_extra: Option<sunrise_protocol::menu_extra::Message>,
    pub(in crate::game) entities: HashMap<i32, Tracked>,
    /// Synced entity attributes keyed by entity id and attribute id.
    pub(in crate::game) synced_attributes:
        HashMap<i32, HashMap<sunrise_world::attributes::entity::AttributeId, f64>>,
    /// Per-tick remote entity scratch retained between frames.
    pub(in crate::game) tick_fluids: Vec<Option<sunrise_client::mobs::living::Fluids>>,
    pub(in crate::game) tick_rain: Vec<bool>,
    pub(in crate::game) tick_standings: Vec<Option<String>>,
    pub(in crate::game) tick_water_fx: Vec<sunrise_client::mobs::MobEvent>,
    /// Complete network riding graph, including invisible plugin seat entities.
    pub(in crate::game) passenger_vehicles: HashMap<u32, PassengerSeat>,
    pub(in crate::game) lerps: HashMap<i32, Lerp>,
    /// A native mod's general rows the server shows the window (`general.rs`).
    pub(in crate::game) general: general::RemoteGenerals,
    /// The minecarts the server moves with `ClientboundMoveMinecartPacket`
    /// (`NewMinecartBehavior`), which have no interpolation handler.
    pub(in crate::game) cart_lerps: HashMap<i32, CartLerp>,
    /// Living entities' `SteppedInterpolationHandler`s.
    pub(in crate::game) stepped: HashMap<i32, SteppedInterpolation>,
    /// `LivingEntity.lerpHeadTo`: the head yaw and the ticks left to reach it.
    pub(in crate::game) head_lerps: HashMap<i32, (f32, u32)>,
    /// The happy ghast the player just got off, until the server's teleport
    /// puts the player on top of it (`place_ghast_under`).
    pub(in crate::game) ghast_dismount: Option<u32>,
    /// Whether the player is the first passenger of its vehicle, the one
    /// `getControllingPassenger` names.
    pub(in crate::game) first_passenger: bool,
    /// `Entity.tickCount` of the local player: its ticks in the client level since the player
    /// was made (Login or Respawn, then the server's first teleport).
    pub(in crate::game) own_tick_count: i32,
    /// The player's place in its vehicle's `getPassengers`.
    pub(in crate::game) passenger_index: usize,
    /// Item entities waiting for their item data: where they are and their motion.
    pub(in crate::game) pending_items: HashMap<i32, PendingItem>,
    pub(in crate::game) health: (f32, i32, f32),
    /// `LocalPlayer.flashOnSetHealth`: set by the first health the server
    /// sends; from then on a drop starts the hurt animation.
    pub(in crate::game) flash_on_set_health: bool,
    pub(in crate::game) game_mode: i32,
    pub chat: Vec<(String, Instant)>,
    /// Lines received since joining, for the control socket's events.
    pub chat_received: u64,
    /// `ChatListener.delayedMessageQueue`: player chat held back by Chat
    /// Delay, with the signature a deletion names it by and what the
    /// narrator says once it shows.
    pub(in crate::game) chat_queue: VecDeque<QueuedChat>,
    /// `ClientPacketListener.messageSignatureCache`: the signatures packed
    /// ones index, as the server's copy for this connection holds them.
    pub(in crate::game) signatures: sunrise_protocol::secure_chat::SignatureCache,
    /// `ChatListener.previousMessageTime`: when a message last showed, if one has.
    pub(in crate::game) chat_previous: Option<Instant>,
    /// `ClientPacketListener.nextChatIndex`: the global index the next chat packet must carry.
    pub(in crate::game) next_chat_index: i32,
    /// `serverEnforcesSecureChat`, from the login packet.
    pub(in crate::game) enforces_secure_chat: bool,
    pub(in crate::game) services_keys: Arc<std::sync::Mutex<ServicesKeys>>,
    /// The `RemoteChatSession.Data` each player's list entry last carried (INITIALIZE_CHAT).
    pub(in crate::game) chat_sessions: HashMap<u128, Option<(u128, i64, Vec<u8>, Vec<u8>)>>,
    /// `PlayerInfo.messageValidator` of the players whose chat has been read.
    pub(in crate::game) chat_validators: HashMap<u128, secure_chat::MessageValidator>,
    /// `ConnectScreen.lastNarration`: when "Joining the world" was last said.
    pub(in crate::game) joining_narrated: Option<Instant>,
    /// The tree the chat colours, completes and parses with: the server's
    /// (ClientboundCommandsPacket) once it has sent one, else the game's own,
    /// plus the commands native mods own (`chat::chat_tree_for`). Rebuilt where
    /// that base changes, not at each keystroke.
    pub chat_tree: sunrise_client::command::Dispatcher,
    pub(in crate::game) selector_metadata_supported: bool,
    /// Whether a `ClientboundCommandsPacket` has supplied the server's tree.
    /// The local fallback is for UI completion only; vanilla signs command
    /// arguments against an empty tree until the server sends one.
    pub(in crate::game) chat_tree_received: bool,
    /// The chat session, the last-seen window and the signature cache.
    pub(in crate::game) secure_chat: super::remote_chat::SecureChat,
    /// `ClientSuggestionProvider.customCompletionSuggestions`: the extra chat
    /// completions this server sent (ClientboundCustomChatCompletionsPacket).
    pub(in crate::game) custom_completions: std::collections::BTreeSet<String>,
    pub(in crate::game) players: HashMap<u128, String>,
    /// `ClientPacketListener.playerInfoMap` as player-list entries, and which of them are listed.
    pub(in crate::game) tab_players: HashMap<u128, sunrise_client::tab_list::TabPlayer>,
    /// Live `PlayerInfo.gameMode` cell per active tab entry; `PlayerInfoRemove` drops the map
    /// slot while leaving any entity's cached `Arc` untouched (`AbstractClientPlayer.playerInfo`).
    pub(in crate::game) tab_game_modes: HashMap<u128, Arc<std::sync::atomic::AtomicI32>>,
    /// First resolved `PlayerInfo` game mode cell of the current `LocalPlayer` entity.
    pub(in crate::game) own_player_info: std::cell::OnceCell<Arc<std::sync::atomic::AtomicI32>>,
    /// Each listed profile's `textures` property, verified once the keys are ready.
    pub(in crate::game) skin_textures: HashMap<u128, Arc<PlayerTextures>>,
    /// Active skin and cape from the services profile when the server sent none.
    pub(in crate::game) own_profile: OwnProfile,
    /// The accepted body, cape and elytra of each profile after its downloads settle.
    pub(in crate::game) skins: HashMap<u128, sunrise_client::skins::ResolvedSkin>,
    pub(in crate::game) skins_revision: u64,
    pub(in crate::game) published_skins: Option<(usize, u128, u64, u64)>,
    pub(in crate::game) listed: std::collections::HashSet<u128>,
    /// `ClientboundLoginPacket.onlineMode`: the player list shows heads.
    pub(in crate::game) online_mode: bool,
    /// `ClientboundLoginPacket.levels`: the server's level keys, which
    /// `ClientPacketListener.levels()` hands to command suggestions.
    pub(in crate::game) levels: Vec<String>,
    /// Player entities in the level: entity id to profile UUID.
    pub(in crate::game) level_players: HashMap<i32, u128>,
    /// Authoritative entity UUID index scoped to the current client level (`EntityLookup.byUuid`).
    pub(in crate::game) level_uuids: HashMap<u128, i32>,
    /// Reverse entity-id-to-UUID index for O(1) removal and replacement in the current client level.
    pub(in crate::game) level_entity_uuids: HashMap<i32, u128>,
    /// First resolved `PlayerInfo` game mode cell per remote `AbstractClientPlayer` entity.
    pub(in crate::game) level_player_infos:
        HashMap<i32, std::cell::OnceCell<Arc<std::sync::atomic::AtomicI32>>>,
    /// The profile name each player entity was created with (`RemotePlayer.getGameProfile`): it outlives the
    /// player-list entry, which servers remove to hide their NPCs from the list.
    pub(in crate::game) level_player_names: HashMap<i32, String>,
    /// The `textures` each player entity found in the player list first
    /// (`AbstractClientPlayer.getPlayerInfo` keeps that `PlayerInfo`): its skin
    /// outlives the entry, which NPC plugins remove a moment after spawning.
    pub(in crate::game) level_player_textures: HashMap<i32, Arc<PlayerTextures>>,
    /// Entities with shared flag 6 (`Entity.isCurrentlyGlowing` on a client); mobs keep
    /// theirs in `Mob::glowing`.
    pub(in crate::game) glowing: std::collections::HashSet<i32>,
    /// The player's own shared flag 5 (`Entity.isInvisible`) as the server last set it.
    pub(in crate::game) own_invisible: bool,
    /// The player's own shared flag 0 (`Entity.isOnFire`) as the server last
    /// set it: its fire (blaze fireballs, fire aspect, lightning) never
    /// reaches the window's own `fire_ticks` otherwise.
    pub(in crate::game) own_on_fire: bool,
    /// `LivingEntity.DATA_ARROW_COUNT_ID` and `DATA_STINGER_COUNT_ID` of the
    /// own player, as the server synced them.
    pub(in crate::game) own_arrows: i32,
    pub(in crate::game) own_stingers: i32,
    /// `Player.DATA_SHOULDER_PARROT_LEFT` and `_RIGHT` of the own player.
    pub(in crate::game) own_shoulders: [Option<i32>; 2],
    /// The local player's profile UUID (always in the level).
    pub(in crate::game) profile: u128,
    /// The local player's profile name: its score holder and team member.
    pub(in crate::game) profile_name: String,
    /// `MultiPlayerGameMode.destroyBlockPos`, `.destroyProgress` and the face last sent: the block
    /// the break targets, which outlives the break itself.
    pub(in crate::game) destroying: Option<([i32; 3], u8, f32)>,
    /// `MultiPlayerGameMode.destroyingItem`: the main-hand stack the break began with.
    pub(in crate::game) destroying_item: Option<ItemStack>,
    /// `MultiPlayerGameMode.isDestroying`: a break is running. `destroying` keeps the target after
    /// one ends (`destroyBlockPos` is written only when a break starts), so a dig resumed on the
    /// same block and item sends no new START and the server keeps its clock (#4124).
    pub(in crate::game) is_destroying: bool,
    /// Per-hand swing policy, restored when the current input attempt ends.
    pub(in crate::game) interaction_swing: [bool; 2],
    pub(in crate::game) destroy_delay: u32,
    /// Whether the main and off hand still wait for the server's answer to
    /// a SWAP_ITEM_WITH_OFFHAND (`remote_hand_slot_set`).
    pub(in crate::game) swap_answer: [bool; 2],
    pub(in crate::game) stop_mining: bool,
    /// `CommonPlayerSpawnInfo.isFlat`: the level is a superflat one.
    pub(in crate::game) flat: bool,
    pub(in crate::game) death_message: Option<sunrise_client::text::Text>,
    /// `LocalPlayer.showDeathScreen`: off with `immediate_respawn`.
    pub(in crate::game) show_death_screen: bool,
    /// `Player.isReducedDebugInfo`: the server's `reduced_debug_info` rule.
    pub(in crate::game) reduced_debug_info: bool,
    /// `LocalPlayer.doLimitedCrafting`: whether limited crafting is enabled.
    pub(in crate::game) do_limited_crafting: bool,
    pub(in crate::game) player_type: Option<Arc<MobType>>,
    /// Inventory, off hand and armour as last synchronised, for creative-mode
    /// slot updates (`creative_sync_stacks`).
    pub(in crate::game) synced: Vec<Option<ItemStack>>,
    pub(in crate::game) decode_errors: u32,
    /// Connection-owned `ClientClockManager` state, retained across dimensions.
    pub(in crate::game) clocks: HashMap<i32, play::ClockState>,
    pub(in crate::game) default_clock: Option<i32>,
    pub(in crate::game) overworld_clock: Option<i32>,
    pub(in crate::game) clock_game_time: i64,
    /// `ClientChunkCache.Storage`'s view centre (ClientboundSetChunkCacheCenterPacket).
    pub(in crate::game) chunk_center: (i32, i32),
    /// A viewed-level swap adopts columns before the server sends the active cache centre.
    pub(in crate::game) awaiting_viewed_chunk_center: bool,
    pub(in crate::game) chunk_light: RemoteLight,
    /// Received events a frame left for the next one, in order, and when
    /// the oldest of them first waited (`poll_remote`).
    pub(in crate::game) backlog: VecDeque<Received>,
    pub(in crate::game) backlog_since: Option<Instant>,
    /// `ClientCommonPacketListenerImpl.serverLinks`: the links "Server
    /// Links..." in the pause menu lists (only `http`/`https` URIs kept).
    pub(in crate::game) server_links: Vec<sunrise_protocol::packets::configuration::ServerLink>,
    /// `ClientPacketListener.getDebugQueryHandler`: the F3+I query waiting for the server's data.
    pub(in crate::game) debug_query: super::debug_keys::DebugQuery,
}

/// The time a frame spends on chunk packets before it leaves the rest of
/// the received events for the next frame. A batch of 25 to 60 columns
/// (0.1 ms each on an M4 Pro) all in one frame was a 3 to 8 ms frame every
/// time the player crossed a chunk border at 32 chunks; they decode on the
/// chunk decoder's thread now, and the frame only inserts them.
pub(in crate::game) const CHUNK_PACKETS_PER_FRAME: Duration = Duration::from_millis(1);
/// Events that have waited this long are all handled in the next frame,
/// whatever inserting their columns costs, up to a column still decoding:
/// the packets after a slow machine's chunks don't wait on its frames.
pub(in crate::game) const BACKLOG_LIMIT: Duration = Duration::from_millis(50);
/// How long a tick about to run waits for one chunk column still decoding;
/// past it the column, and what came after it, wait for the next frame.
pub(in crate::game) const TICK_DECODE_WAIT: Duration = Duration::from_millis(250);
/// The open menu name of a server's horse inventory, then its chest's
/// columns (the packet names no `MenuType`).
pub(in crate::game) const MOUNT_MENU: &str = "sunrise:mount";
/// ServerboundPlayerInputPacket's shift bit.
pub(in crate::game) const SHIFT_INPUT: u8 = 1 << 5;

pub(in crate::game) fn face_direction(face: [i32; 3]) -> u8 {
    match face {
        [0, -1, 0] => 0,
        [0, 1, 0] => 1,
        [0, 0, -1] => 2,
        [0, 0, 1] => 3,
        [-1, 0, 0] => 4,
        [1, 0, 0] => 5,
        _ => 1,
    }
}

/// A protocol number format as the scoreboard's.
pub(in crate::game) fn number_format(
    f: &play::NumberFormat,
) -> sunrise_client::scoreboard::NumberFormat {
    use sunrise_client::scoreboard::{NumberFormat, tag_to_json};
    match f {
        play::NumberFormat::Blank => NumberFormat::Blank,
        play::NumberFormat::Styled(t) => NumberFormat::Styled(tag_to_json(t)),
        play::NumberFormat::Fixed(t) => NumberFormat::Fixed(tag_to_json(t)),
    }
}

impl Game<'_> {
    // remote_sound_name: remote/sync.rs

    // connect_remote_session: remote/session.rs

    // init_remote_for_replay: remote/session.rs

    // leave_server: remote/session.rs

    /// Their kind and place, for `status` (shared projectiles; client callers append `hooks_status`).
    pub(in crate::game) fn projectiles_status(&self) -> Vec<serde_json::Value> {
        use serde_json::json;
        let at = |p: sunrise_core::geometry::Vec3| [p.x, p.y, p.z];
        let mut out = Vec::new();
        for f in &self.fangs.list {
            out.push(
                json!({"id": f.id, "kind": "minecraft:evoker_fangs", "position": f.pos,
                "yaw": f.yaw, "started": f.started, "life": f.life}),
            );
        }
        for t in &self.tnts {
            out.push(json!({"id": t.id, "kind": "minecraft:tnt", "position": at(t.position), "fuse": t.fuse}));
        }
        for f in &self.falling {
            out.push(json!({"id": f.id, "kind": "minecraft:falling_block", "position": at(f.position), "velocity": at(f.velocity), "state": f.state.0}));
        }
        for b in &self.boats {
            out.push(json!({"id": b.id, "kind": "minecraft:boat", "position": at(b.position), "velocity": at(b.velocity)}));
        }
        for c in &self.carts {
            out.push(json!({"id": c.id, "kind": "minecraft:minecart", "position": at(c.position), "velocity": at(c.velocity)}));
        }
        for c in &self.crystals {
            out.push(json!({"id": c.id, "kind": "minecraft:end_crystal", "position": at(c.position), "show_bottom": c.show_bottom}));
        }
        for t in &self.tridents {
            out.push(json!({"id": t.id, "kind": "minecraft:trident", "position": at(t.position), "velocity": at(t.velocity), "on_fire": t.fire_ticks > 0}));
        }
        for r in &self.rockets {
            out.push(json!({"id": r.id, "kind": "minecraft:firework_rocket", "position": at(r.position), "velocity": at(r.velocity)}));
        }
        for p in &self.creatures.projectiles {
            out.push(json!({"id": p.id, "kind": format!("{:?}", p.kind).split([' ', '{']).next().unwrap_or(""), "position": at(p.position), "velocity": at(p.velocity),
                "player_owned": p.owner == Some(sunrise_client::mobs::Who::Player),
                "on_fire": p.arrow.as_ref().is_some_and(|a| a.on_fire())}));
        }
        for (kind, p, _) in self.potions.thrown_entities() {
            out.push(json!({"kind": kind, "position": at(p)}));
        }
        for (id, p) in self.end.eye_positions() {
            out.push(json!({"id": id, "kind": "minecraft:eye_of_ender", "position": p}));
        }
        out
    }

    // remote_effect_network_id: remote/sync.rs
    pub(in crate::game) fn item_id(&self, name: &str) -> i32 {
        let r = self.remote.as_ref().expect("remote");
        r.builtin
            .get("minecraft:item")
            .and_then(|i| i.id(name))
            .unwrap_or(0)
    }

    pub(in crate::game) fn to_network_projecting(
        &self,
        s: &Option<ItemStack>,
        projected: &dyn Fn(&sunrise_world::component_value::consume_effects::Admitted),
    ) -> Option<Stack> {
        let r = self.remote.as_ref()?;
        // A stack as the server sent it, only its count changed: its own
        // components go back, as a vanilla client keeps them.
        let effect_mapping = r
            .registries
            .consume_effect_codecs
            .as_ref()
            .map(|snapshot| snapshot.identity());
        let source = s.as_ref()?;
        if let Some(patch) = source.wire_patch_for(effect_mapping) {
            let mut out = Stack::new(self.item_id(&source.item), source.count as i32);
            let safe = r.registries.custom_components
                || sunrise_protocol::item::custom::entries().next().is_none()
                || sunrise_protocol::item::WirePatch::decode_reader(
                    sunrise_protocol::buf::Reader::new(&patch).with_names(&*r.registries),
                )
                .and_then(|(added, removed)| {
                    let decoded = Stack {
                        added,
                        removed,
                        ..Stack::new(out.item, out.count)
                    };
                    sunrise_protocol::item::custom::permitted_with_names(
                        &decoded,
                        false,
                        &*r.registries,
                    )
                })
                .unwrap_or(false);
            if safe {
                out.wire = sunrise_protocol::item::WirePatch(Some(patch));
                return Some(out);
            }
            log::warn!("Cannot send cached custom components to this peer");
        }
        let mut wire = sunrise_client::lan::guest::stack_to_network_projecting(
            |name| self.item_id(name),
            // Built-in first, as a hub's `SyncData::id` does: potions are one.
            |registry, name| {
                if registry == sunrise_protocol::item::consume_effects::REGISTRY {
                    return r.registries.consume_effect_id(name);
                }
                r.builtin
                    .get(registry)
                    .and_then(|ids| ids.id(name))
                    .or_else(|| r.registries.id(registry, name))
                    .or_else(|| {
                        (r.registries.custom_components
                            && registry == "minecraft:data_component_type")
                            .then(|| sunrise_protocol::item::custom::id(name))
                            .flatten()
                    })
            },
            s,
            projected,
        )?;
        sunrise_client::lan::guest::canonicalize_stack_patch(
            &mut wire,
            source,
            self.items(),
            &r.builtin,
        );
        Some(wire)
    }

    /// The account playing this game.
    pub(in crate::game) fn auth(
        &self,
    ) -> Option<std::sync::Arc<sunrise_client::auth::AuthSession>> {
        sunrise_client::auth::current()
    }

    /// `ClientSuggestionProvider.customCompletionSuggestions` of the server
    /// connection (none without one).
    pub(in crate::game) fn remote_custom_completions(&self) -> std::collections::BTreeSet<String> {
        self.remote
            .as_ref()
            .map(|r| r.custom_completions.clone())
            .unwrap_or_default()
    }

    /// The names in the server's player list, sorted.
    pub(in crate::game) fn remote_online_names(&self) -> Option<Vec<String>> {
        let r = self.remote.as_ref()?;
        let mut names: Vec<String> = r.players.values().cloned().collect();
        names.sort();
        Some(names)
    }

    // move_entity: remote/motion.rs

    // remote_objective: remote/sync.rs

    // remote_teleport: remote/sync.rs

    // tick_remote and tick_remote_player: remote/tick.rs

    // tick_remote_guardian_eyes: remote/tick.rs

    // tick_remote_entities: remote/tick.rs

    // remote_mining: remote/use.rs

    // remote_use_block: remote/use.rs

    // remote_block_reacts: remote/use.rs
    // remote_block_template: remote/use.rs

    // remote_place_preview: remote/use.rs

    // remote_use_bucket: remote/use.rs

    // remote_start_held_use: remote/use.rs

    // remote_use_instrument: remote/use.rs

    // remote_use_hands: remote/use.rs

    // remote_complete_using: remote/use.rs

    // remote_send_carried: remote/sync.rs

    /// The entity type of the boat, minecart, armor stand or End crystal with
    /// server id `id` that an attack reaches `Player.attack`'s sweep check on,
    /// and whether it is living. A painting or item frame is not: on this
    /// client `skipAttackInteraction` hurts it and stops the attack there.
    pub(in crate::game) fn remote_attacked_object(&self, id: i32) -> Option<(String, bool)> {
        let id = id as u32;
        if let Some(b) = self.boats.iter().find(|b| b.id == id) {
            return Some((b.entity_type(), false));
        }
        if let Some(c) = self.carts.iter().find(|c| c.id == id) {
            return Some((c.kind.id().to_owned(), false));
        }
        if self.armor_stands.iter().any(|a| a.id == id) {
            return Some(("minecraft:armor_stand".to_owned(), true));
        }
        self.crystals
            .iter()
            .any(|c| c.id == id)
            .then(|| ("minecraft:end_crystal".to_owned(), false))
    }

    // remote_player_attack: remote/ui.rs

    /// An attribute's value on this client's player, if its map has the attribute.
    pub(in crate::game) fn own_attribute(&mut self, id: &str) -> Option<f64> {
        let p = self.player.as_mut()?;
        let key = p.attributes.key(id)?;
        p.attributes.lookup(Some(&key))?;
        p.attributes
            .value(Some(key), sunrise_world::attribute_map::ValueKind::Computed)
            .ok()
    }

    // remote_key: remote/ui.rs

    // remote_click: remote/use.rs

    // remote_overlay: remote/ui.rs

    /// Connected or connecting with no player yet: `ConnectScreen`, then
    /// `LevelLoadingScreen`, over the panorama with its blur.
    pub(in crate::game) fn remote_loading(&self) -> bool {
        if self.replay.playing() {
            return false;
        }
        use sunrise_client::level_loading::Reason;
        self.remote.as_ref().is_some_and(|r| {
            if r.level_load.is_active() {
                // The portal screens have no panorama, so no blur.
                r.level_load.reason == Reason::Other
            } else {
                self.player.is_none()
            }
        })
    }

    /// Shared flag 6 of an entity on a server's client (`None` without a server).
    pub(in crate::game) fn remote_glowing(&self, id: u32) -> Option<bool> {
        let r = self.remote.as_ref()?;
        Some(r.glowing.contains(&(id as i32)))
    }

    /// The profile name of another player the server sent, by entity id.
    pub(in crate::game) fn remote_player_name(&self, id: u32) -> Option<&str> {
        let r = self.remote.as_ref()?;
        r.level_player_name(id as i32, &r.player_uuid(id)?)
    }

    // remote_status: remote/status.rs
}

/// `Entity.lookAt`'s rotation from `from` towards `to`, as (yaw, pitch).
pub(in crate::game) fn look_at_rotation(from: [f64; 3], to: [f64; 3]) -> (f32, f32) {
    use sunrise_core::geometry::trig::atan2;
    let (dx, dy, dz) = (to[0] - from[0], to[1] - from[1], to[2] - from[2]);
    let horizontal = (dx * dx + dz * dz).sqrt();
    // One multiply by `Mth.RAD_TO_DEG` on the double.
    let degrees = |r: f64| r * f64::from(sunrise_core::geometry::trig::RAD_TO_DEG);
    let pitch = wrap_degrees((-degrees(atan2(dy, horizontal))) as f32);
    let yaw = wrap_degrees(degrees(atan2(dz, dx)) as f32 - 90.0);
    (yaw, pitch)
}

/// `PositionMoveRotation.calculateAbsolute`'s pitch: the packet's pitch,
/// added to `source` when it is relative, clamped to +-90.
pub(in crate::game) fn position_packet_pitch(source: f32, change: f32, relative: bool) -> f32 {
    let offset = if relative { source } else { 0.0 };
    (offset + change).clamp(-90.0, 90.0)
}

/// `Mth.wrapDegrees`.
pub(in crate::game) fn wrap_degrees(a: f32) -> f32 {
    let a = a % 360.0;
    if a >= 180.0 {
        a - 360.0
    } else if a < -180.0 {
        a + 360.0
    } else {
        a
    }
}

/// `LivingEntity.tick`'s body target and `tickHeadTurn` for another player:
/// the body turns toward where it walks (backwards when walking back), or to
/// the look while the arm swings, and stays within 50 degrees of the look.
pub(in crate::game) fn remote_player_head_turn(m: &mut Mob) {
    let (dx, dz) = (m.position.x - m.previous.x, m.position.z - m.previous.z);
    let mut target = m.body_yaw;
    if (dx * dx + dz * dz) as f32 > 0.002_500_000_2 {
        let walk = (dz.atan2(dx) as f32).to_degrees() - 90.0;
        let facing = (wrap_degrees(m.yaw) - walk).abs();
        target = if 95.0 < facing && facing < 265.0 {
            walk - 180.0
        } else {
            walk
        };
    }
    if m.anim.attack_anim > 0.0 {
        target = m.yaw;
    }
    m.body_yaw += wrap_degrees(target - m.body_yaw) * 0.3;
    let head = wrap_degrees(m.yaw - m.body_yaw);
    if head.abs() > 50.0 {
        m.body_yaw += head - head.signum() * 50.0;
    }
}

/// ServerboundContainerClickPacket's `ContainerInput` ordinal.
pub(in crate::game) fn input_ordinal(input: Input) -> i32 {
    match input {
        Input::Pickup => 0,
        Input::QuickMove => 1,
        Input::Swap => 2,
        Input::Clone => 3,
        Input::Throw => 4,
        Input::QuickCraft => 5,
        Input::PickupAll => 6,
    }
}

pub(in crate::game) mod dry_level;
#[allow(unused_imports)]
use dry_level::DryLevel;

pub(in crate::game) type DrySound = (
    sunrise_core::position::BlockPos,
    std::sync::Arc<str>,
    mod_api::SoundSource,
    f32,
    f32,
);
/// The blocks a use on the dry level set.
pub(in crate::game) type DryChanges = Vec<(sunrise_core::position::BlockPos, BlockStateId)>;
/// What a use on the dry level changed, and the sounds it played.
pub(in crate::game) type DryUse = (DryChanges, Vec<DrySound>);
/// Where `BlockItem.place` on the dry level placed, and the blocks it set.
pub(in crate::game) struct DryPlace {
    pub(in crate::game) pos: sunrise_core::position::BlockPos,
    pub(in crate::game) matches_candidate: bool,
    pub(in crate::game) changes: DryChanges,
}

/// The blocks a guest's hand flips that its use predicts (and hears at once): a wooden or
/// copper door, trapdoor or fence gate, a lever, a button or a cauldron, whose fill level
/// changes as the bucket, bottle or dye lands. An iron door or trapdoor has no hand use.
pub(in crate::game) fn hand_operated(
    behaviours: &sunrise_world::level::Behaviours,
    state: BlockStateId,
) -> bool {
    behaviours
        .data(state)
        .is(sunrise_world::level::classes::Class::FenceGate)
        || named_hand_operated(behaviours.table.name(state))
}

pub(in crate::game) fn named_hand_operated(name: &str) -> bool {
    let name = name.trim_start_matches("minecraft:");
    ((name.ends_with("_door") || name.ends_with("_trapdoor")) && !name.starts_with("iron_"))
        || name.ends_with("_button")
        || name == "lever"
        || name.ends_with("cauldron")
}

/// `InteractionHand` on the wire.
pub(in crate::game) fn hand_id(hand: Hand) -> i32 {
    match hand {
        Hand::Main => 0,
        Hand::Off => 1,
    }
}

/// Whether `Item.use` of `info` ends `Minecraft.startUseItem`'s click, so the
/// off hand gets no turn. `FireworkRocketItem.use` answers SUCCESS while the
/// player glides (the rocket boosts them) and PASS otherwise.
pub(in crate::game) fn use_ends_click(
    info: &sunrise_client::items::ItemInfo,
    fall_flying: bool,
) -> bool {
    item_has_use(info) || (fall_flying && &*info.id == "minecraft:firework_rocket")
}

/// Whether `Item.use` of this item answers SUCCESS on this client, which
/// swings (`MultiPlayerGameMode.useItem`'s `result.shouldSwing()`), rather
/// than CONSUME, which only ends the click: a bow, crossbow, trident or
/// spear starting its charge, `ItemUtils.startUsingInstantly`'s food, drink,
/// spyglass, shield and goat horn, and the ender eye's SUCCESS_SERVER (the
/// server's swing source, not this client's).
pub(in crate::game) fn remote_use_swings(held: &ItemStack, consumable: bool) -> bool {
    let id = &*held.item;
    !matches!(
        id,
        "minecraft:bow"
            | "minecraft:crossbow"
            | "minecraft:trident"
            | "minecraft:spyglass"
            | "minecraft:goat_horn"
            | "minecraft:ender_eye"
            | "minecraft:brush"
    ) && !consumable
        && sunrise_client::spear::Kinetic::of(id).is_none()
}

/// Whether an object interaction on the client predicts a hand swing (`InteractionResult.SUCCESS`).
/// Boarding a minecart answers CONSUME and does not swing.
pub(in crate::game) fn remote_object_swings(response: Option<bool>, is_boarding: bool) -> bool {
    response.unwrap_or(!is_boarding)
}

/// Whether right-clicking an item frame passes (`InteractionResult.PASS`).
/// A fixed frame or an empty frame when clicking with an empty hand passes.
pub(in crate::game) fn item_frame_interact_passes(
    fixed: bool,
    frame_has_item: bool,
    hand_has_item: bool,
) -> bool {
    fixed || (!frame_has_item && !hand_has_item)
}

/// Whether right-clicking a boat passes (`InteractionResult.PASS`) on a client.
/// `AbstractBoat.interact` passes when sneaking or out of control (60 ticks), and
/// answers SUCCESS otherwise without checking for room (only the server's
/// `startRiding` does). `AbstractChestBoat.interact` opens the chest instead of
/// passing, unless it is out of control, not sneaked at and has room.
pub(in crate::game) fn boat_interact_passes(
    chest: bool,
    sneaking: bool,
    out_of_control: bool,
    has_room: bool,
) -> bool {
    if chest {
        !sneaking && out_of_control && has_room
    } else {
        sneaking || out_of_control
    }
}

/// Whether `Item.use` usually acts on this item (eats, drinks, equips, raises,
/// throws, shoots, fills or places), which ends `Minecraft.startUseItem` at
/// its hand. Tools and materials pass, and the off hand gets its turn. The
/// server decides; this is the client's guess of whether to go on. A carrot or warped fungus
/// on a stick is not here: `FoodOnAStickItem.use` passes on a client, and only the server's
/// answer to a boost that can run swings the hand.
pub(in crate::game) fn item_has_use(info: &sunrise_client::items::ItemInfo) -> bool {
    let id = info.id.trim_start_matches("minecraft:");
    id.ends_with("_spear")
        || info.is("minecraft:fishing_rod") // a content mod's rod too
        || info.consumable.is_some()
        || (info.equip_slot.is_some() && info.swappable)
        || info.blocks_attacks.is_some()
        || sunrise_client::items::bucket_contents(&info.id).is_some()
        || id.ends_with("_boat")
        || id.ends_with("_raft")
        || id.ends_with("_bundle")
        || matches!(
            id,
            "bow"
                | "crossbow"
                | "trident"
                | "spyglass"
                | "snowball"
                | "egg"
                | "blue_egg"
                | "brown_egg"
                | "ender_pearl"
                | "ender_eye"
                | "wind_charge"
                | "splash_potion"
                | "lingering_potion"
                | "experience_bottle"
                | "bucket"
                | "glass_bottle"
                | "map"
                | "writable_book"
                | "written_book"
                | "goat_horn"
                | "bundle"
                | "brush"
                | "knowledge_book"
        )
}

/// The view distance a client asks for: its render distance, which Sunrise
/// lets go past vanilla's 32 (a vanilla server still caps it at its own).
pub(in crate::game) fn client_view_distance(render_distance: i32) -> i8 {
    render_distance.clamp(2, i32::from(i8::MAX)) as i8
}

/// `ClientPacketListener.handleBlockEntityData` loads the tag into the existing
/// block entity, so the state the client builds itself stays: copied from `old`
/// into `new`, the block entity decoded from the tag.
pub(in crate::game) fn keep_client_state(old: Option<&BlockEntity>, new: &mut BlockEntity) {
    match (old, new) {
        // The turn of a vault's item and a trial spawner's mob (VaultClientData).
        (Some(BlockEntity::Vault(old)), BlockEntity::Vault(new)) => {
            new.spin = old.spin;
            new.o_spin = old.o_spin;
        }
        (Some(BlockEntity::TrialSpawner(old)), BlockEntity::TrialSpawner(new)) => {
            new.spin = old.spin;
            new.o_spin = old.o_spin;
        }
        // A beacon's beam (`BeaconBlockEntity.loadAdditional` reads only the
        // powers): a fresh one draws nothing until the next tick walks it again.
        (Some(BlockEntity::Beacon(old)), BlockEntity::Beacon(new)) => {
            new.sections.clone_from(&old.sections);
            new.checking.clone_from(&old.checking);
            (new.last_check_y, new.levels) = (old.last_check_y, old.levels);
        }
        // `eruptionTick` is set once, when the level is set; an update tag leaves it.
        (Some(BlockEntity::PotentSulfur(old)), BlockEntity::PotentSulfur(new)) => {
            new.eruption_tick = old.eruption_tick;
        }
        _ => {}
    }
}

/// `LocalPlayer.onSyncedDataUpdated(DATA_LIVING_ENTITY_FLAGS)`: the server's
/// flags (1 using, 2 the off hand) start a use the client is not running
/// (`startUsingItem`, which needs an item in that hand), and stop one it is
/// (`stopUsingItem`). `using` is the slot in use and its ticks.
pub(in crate::game) fn synced_local_use(
    using: Option<(usize, u32)>,
    flags: i8,
    selected: usize,
    has_item: impl Fn(usize) -> bool,
) -> Option<(usize, u32)> {
    let server_using = flags & 1 != 0;
    match using {
        None if server_using => {
            let slot = if flags & 2 != 0 {
                super::shield::OFFHAND_SLOT
            } else {
                selected
            };
            has_item(slot).then_some((slot, 0))
        }
        Some(_) if !server_using => None,
        using => using,
    }
}

#[derive(Default)]
pub(in crate::game) struct ClientMenu {
    pub(in crate::game) current: Option<(i32, String)>,
    pub(in crate::game) generation: u64,
}

impl ClientMenu {
    pub(in crate::game) fn set(&mut self, current: Option<(i32, String)>) {
        self.generation = self
            .generation
            .checked_add(1)
            .expect("menu generation exhausted");
        self.current = current;
    }

    pub(in crate::game) fn as_ref(&self) -> Option<&(i32, String)> {
        self.current.as_ref()
    }
}

pub(in crate::game) fn object_skin_changed(
    published: &HashMap<String, String>,
    key: &str,
    skin: &str,
) -> bool {
    published.get(key).is_none_or(|previous| previous != skin)
}

/// The textures to resolve skins from: every list entry's, and for a player
/// entity whose entry is gone the profile it kept. `kept` first takes, for each
/// player entity without one, its entry's textures (`AbstractClientPlayer.getPlayerInfo`
/// looks the PlayerInfo up once and holds it, removed from the list or not).
pub(in crate::game) fn skin_entries(
    level_players: &HashMap<i32, u128>,
    listed: &HashMap<u128, Arc<PlayerTextures>>,
    kept: &mut HashMap<i32, Arc<PlayerTextures>>,
) -> Vec<(u128, Arc<PlayerTextures>)> {
    for (id, uuid) in level_players {
        if !kept.contains_key(id)
            && let Some(textures) = listed.get(uuid)
        {
            kept.insert(*id, textures.clone());
        }
    }
    let mut entries: Vec<_> = listed.iter().map(|(u, t)| (*u, t.clone())).collect();
    for (id, textures) in kept.iter() {
        if let Some(uuid) = level_players.get(id)
            && !listed.contains_key(uuid)
            && !entries.iter().any(|(u, _)| u == uuid)
        {
            entries.push((*uuid, textures.clone()));
        }
    }
    entries
}

/// `AttributeInstance.calculateValue` before the attribute's range: the base,
/// `ADD_VALUE`s, then `ADD_MULTIPLIED_BASE`s of that, then each
/// `ADD_MULTIPLIED_TOTAL`.
pub(in crate::game) fn snapshot_value(s: &play::AttributeSnapshot) -> f64 {
    let amounts = |op: i32| {
        s.modifiers
            .iter()
            .filter(move |m| m.operation == op)
            .map(|m| m.amount)
    };
    let base = s.base + amounts(0).sum::<f64>();
    let mut value = base + amounts(1).map(|a| base * a).sum::<f64>();
    for amount in amounts(2) {
        value *= 1.0 + amount;
    }
    value
}

/// `Util.getMillis` for the level load tracker's timeouts.
pub(in crate::game) fn client_millis() -> u64 {
    static ORIGIN: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    ORIGIN
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_millis() as u64
}

/// What `ChatScreen.handleChatInput` sends of typed chat once `ClientChatEvent` (`ask`, with
/// the outgoing message and the cancel flag) had its say: nothing when canceled or emptied.
pub(in crate::game) fn chat_after_event(
    message: &str,
    ask: impl FnOnce(&mut String, &mut bool),
) -> Option<String> {
    let mut outgoing = message.to_owned();
    let mut cancel = false;
    ask(&mut outgoing, &mut cancel);
    (!cancel && !outgoing.is_empty()).then_some(outgoing)
}
/// `Entity.isOnFire` for the window's player: while the server's flag is set
/// the predicted tick's countdown never takes `fire_ticks` below 1.
pub(in crate::game) fn hold_server_fire(fire_ticks: i32, server_on_fire: bool) -> i32 {
    if server_on_fire {
        fire_ticks.max(1)
    } else {
        fire_ticks
    }
}

/// `Entity.isColliding`: whether the collision `shape` of a block at `pos`
/// meets `entity`'s box. Shapes are block-local, so they move onto the block
/// first (`VoxelShape.move`), and any box that overlaps is a collision
/// (`Shapes.joinIsNotEmpty` with `BooleanOp.AND`).
pub(in crate::game) fn shape_meets(
    shape: &sunrise_core::geometry::VoxelShape,
    pos: [i32; 3],
    entity: sunrise_core::geometry::Aabb,
) -> bool {
    let at = Vec3::new(f64::from(pos[0]), f64::from(pos[1]), f64::from(pos[2]));
    shape
        .boxes()
        .iter()
        .any(|b| b.move_by(at).intersects(entity))
}

/// `ClientLevel.syncBlockState`'s snap: a restored block that meets the player
/// sends it back to the position the prediction remembered, unless the player
/// was teleported after that prediction was made
/// (`lastTeleportSequence < sequence`: the remembered position is stale).
pub(in crate::game) fn snap_target(
    recorded: [f64; 3],
    last_teleport_sequence: i32,
    sequence: i32,
    meets: bool,
) -> Option<[f64; 3]> {
    (meets && last_teleport_sequence < sequence).then_some(recorded)
}

/// [`Remote::has_level`]: logged in (a Login arrived) and no reconfiguration
/// since (`reconfiguring` is set by StartConfiguration, cleared by the next
/// Login).
pub(in crate::game) fn client_has_level(logged_in: bool, reconfiguring: Option<u64>) -> bool {
    logged_in && reconfiguring.is_none()
}

// A packet's removals (`RemoveEntities`), apart: this file may not grow.
#[cfg(test)]
pub(in crate::game) mod interaction_event_tests;

#[cfg(test)]
pub(in crate::game) mod native_entity_tests;
#[cfg(test)]
pub(in crate::game) mod no_gravity_data_tests;

#[cfg(test)]
pub(in crate::game) mod camera_view_tests;
#[cfg(test)]
pub(in crate::game) mod effect_blend_tests;
#[cfg(test)]
pub(in crate::game) mod level_sound_tests;

#[cfg(test)]
pub(in crate::game) mod attachment_query_tests;
#[cfg(test)]
pub(in crate::game) mod client_camera_tests;
#[cfg(test)]
pub(in crate::game) mod passage_cross_tests;
#[cfg(test)]
pub(in crate::game) mod respawn_motion_tests;
#[cfg(test)]
pub(in crate::game) mod riptide_tests;

#[cfg(test)]
pub(in crate::game) mod tests {
    pub(in crate::game) mod client_explosion_tests {
        include!("client_explosion_tests.rs");
    }

    #[allow(unused_imports)]
    use super::super::api_values::TryIntoApi;

    pub(in crate::game) mod block_entity_state_tests {
        include!("block_entity_state_tests.rs");
    }
    pub(in crate::game) mod chat_decoration_tests {
        include!("chat_decoration_tests.rs");
    }
    pub(in crate::game) mod chat_event_tests {
        include!("chat_event_tests.rs");
    }
    pub(in crate::game) mod chat_window_and_input_tests {
        include!("chat_window_and_input_tests.rs");
    }
    pub(in crate::game) mod play_bundle_tests {
        include!("play_bundle_tests.rs");
    }
    pub(in crate::game) mod chunk_storage_tests {
        include!("chunk_storage_tests.rs");
    }
    pub(in crate::game) mod click_ending_tests {
        include!("click_ending_tests.rs");
    }
    pub(in crate::game) mod client_attributes_tests {
        include!("client_attributes_tests.rs");
    }
    pub(in crate::game) mod client_block_changed_tests {
        include!("client_block_changed_tests.rs");
    }
    pub(in crate::game) mod camera_draw_tests {
        include!("camera_draw_tests.rs");
    }
    pub(in crate::game) mod camera_parity_red_tests {
        include!("camera_parity_red_tests.rs");
    }
    pub(in crate::game) mod set_camera_kinds_tests {
        include!("set_camera_kinds_tests.rs");
    }
    pub(in crate::game) mod camera_death_tests {
        include!("camera_death_tests.rs");
    }
    pub(in crate::game) mod client_hud_camera_tests {
        include!("client_hud_camera_tests.rs");
    }
    pub(in crate::game) mod client_day_time_tests {
        include!("client_day_time_tests.rs");
    }
    pub(in crate::game) mod client_epochs_tests {
        include!("client_epochs_tests.rs");
    }
    pub(in crate::game) mod client_handles_tests {
        include!("client_handles_tests.rs");
    }
    pub(in crate::game) mod client_level_environment_tests {
        include!("client_level_environment_tests.rs");
    }
    pub(in crate::game) mod client_level_lifecycle_tests {
        include!("client_level_lifecycle_tests.rs");
    }
    pub(in crate::game) mod client_menu_tests {
        include!("client_menu_tests.rs");
    }
    pub(in crate::game) mod client_player_reads_tests {
        include!("client_player_reads_tests.rs");
    }
    pub(in crate::game) mod client_vanilla_reads_tests {
        include!("client_vanilla_reads_tests.rs");
    }
    pub(in crate::game) mod client_read_tests {
        include!("client_read_tests.rs");
    }
    pub(in crate::game) mod container_drop_tests {
        include!("container_drop_tests.rs");
    }
    pub(in crate::game) mod creative_sync_tests {
        include!("creative_sync_tests.rs");
    }
    pub(in crate::game) mod damage_sound_tests {
        include!("damage_sound_tests.rs");
    }
    pub(in crate::game) mod dig_abort_tests {
        include!("dig_abort_tests.rs");
    }
    pub(in crate::game) mod dimension_transition_tests {
        include!("dimension_transition_tests.rs");
    }
    pub(in crate::game) mod game_event_tests {
        include!("game_event_tests.rs");
    }
    pub(in crate::game) mod hand_operated_tests {
        include!("hand_operated_tests.rs");
    }
    pub(in crate::game) mod hand_swap_tests {
        include!("hand_swap_tests.rs");
    }
    pub(in crate::game) mod hanging_entity_age_tests {
        include!("hanging_entity_age_tests.rs");
    }
    pub(in crate::game) mod main_hand_tests {
        include!("main_hand_tests.rs");
    }
    pub(in crate::game) mod menu_click_order_tests {
        include!("menu_click_order_tests.rs");
    }
    pub(in crate::game) mod menu_generation_tests {
        include!("menu_generation_tests.rs");
    }
    pub(in crate::game) mod menu_packets_tests {
        include!("menu_packets_tests.rs");
    }
    pub(in crate::game) mod client_close_tests {
        include!("client_close_tests.rs");
    }
    pub(in crate::game) mod menu_title_tests {
        include!("menu_title_tests.rs");
    }
    pub(in crate::game) mod movement_snap_tests {
        include!("movement_snap_tests.rs");
    }
    pub(in crate::game) mod object_interactions_tests {
        include!("object_interactions_tests.rs");
    }
    pub(in crate::game) mod particle_group_tests {
        include!("particle_group_tests.rs");
    }
    pub(in crate::game) mod placement_preview_tests {
        include!("placement_preview_tests.rs");
    }
    pub(in crate::game) mod player_animation_tests {
        include!("player_animation_tests.rs");
    }
    pub(in crate::game) mod player_sleep_tests {
        include!("player_sleep_tests.rs");
    }
    pub(in crate::game) mod predicted_block_notice_tests {
        include!("predicted_block_notice_tests.rs");
    }
    pub(in crate::game) mod predicted_throw_tests {
        include!("predicted_throw_tests.rs");
    }
    pub(in crate::game) mod remote_columns_tests {
        include!("remote_columns_tests.rs");
    }
    pub(in crate::game) mod server_fire_flag_tests {
        include!("server_fire_flag_tests.rs");
    }
    pub(in crate::game) mod slot_mapping_tests {
        include!("slot_mapping_tests.rs");
    }
    pub(in crate::game) mod sound_entity_tests {
        include!("sound_entity_tests.rs");
    }

    #[test]
    #[ignore = "daily"]
    fn named_remote_players_are_told_one_by_one() {
        use mod_api::{
            EntityTickListenedEvent, EntityTickPostEvent, EntityTickPreEvent, Hooks, HostSide,
            ModEntry, Mods, Registrar,
        };
        use std::sync::{Arc, Mutex};
        type Calls = Arc<Mutex<Vec<(bool, Vec<u32>)>>>;
        struct Named(Calls);
        #[mod_api::__subscriptions]
        impl Hooks for Named {
            fn entity_tick_listened(&self, _: HostSide<'_>, event: &mut EntityTickListenedEvent) {
                event.listened = true;
            }
            fn register(&self, r: &mut dyn Registrar, _: &mod_api::RegistrationContext) {
                r.entity_tick_each("minecraft:player");
            }
            fn entity_tick_pre(&self, _: HostSide<'_>, event: &mut EntityTickPreEvent<'_>) {
                self.0.lock().unwrap().push((true, event.entities.to_vec()));
            }
            fn entity_tick_post(&self, _: HostSide<'_>, event: &mut EntityTickPostEvent<'_>) {
                self.0
                    .lock()
                    .unwrap()
                    .push((false, event.entities.to_vec()));
            }
        }
        let heard: Calls = Arc::default();
        let mods: Mods = vec![ModEntry::new(
            "remote-player-ticks",
            Box::new(Named(heard.clone())),
        )]
        .into();
        crate::game::api_values::install_parts(&mods).unwrap();
        mod_platform::testing::with_mods(mods, || {
            super::super::native_entity_refs::tests::with_client_game(|game| {
                let (client, _server) = sunrise_protocol::connection::Pipe::pair();
                game.connect_local(client, "avatar-tick-client").unwrap();
                let remote = game.remote.as_mut().unwrap();
                let kind = remote.player_type.as_ref().unwrap().clone();
                assert!(
                    !kind.flags.tick_alone,
                    "render model does not select tick routing"
                );
                for id in [66, 67] {
                    remote.level_players.insert(id, id as u128);
                    game.mobs.push(sunrise_client::mobs::Mob::new(
                        id as u32,
                        kind.clone(),
                        sunrise_core::geometry::Vec3::ZERO,
                        0.0,
                    ));
                }
                game.tick_remote_entities(false);
            });
        });
        assert_eq!(
            *heard.lock().unwrap(),
            [
                (true, vec![66]),
                (false, vec![66]),
                (true, vec![67]),
                (false, vec![67])
            ]
        );
    }

    pub(in crate::game) mod spear_jab_tests {
        include!("spear_jab_tests.rs");
    }
    pub(in crate::game) mod client_admission_tests {
        include!("client_admission_tests.rs");
    }
    pub(in crate::game) mod tab_publication_tests {
        include!("tab_publication_tests.rs");
    }
    pub(in crate::game) mod thrown_item_swing_tests {
        include!("thrown_item_swing_tests.rs");
    }
    pub(in crate::game) mod client_emitters_tests {
        include!("client_emitters_tests.rs");
    }
    pub(in crate::game) mod tracking_state_tests {
        include!("tracking_state_tests.rs");
    }
    pub(in crate::game) mod use_flag_tests {
        include!("use_flag_tests.rs");
    }

    /// A client connecting through a local pipe and through configuration, as
    /// the client menu tests drive it: the server end's reader (what the
    /// client sends) and writer.
    pub(crate) fn connect_menu_test(
        game: &mut super::Game<'_>,
    ) -> (
        sunrise_protocol::connection::ConnReader,
        sunrise_protocol::connection::ConnWriter,
    ) {
        connect_play_test(game, |_, _| {})
    }

    pub(in crate::game) fn connect_play_test(
        game: &mut super::Game<'_>,
        configure: impl FnOnce(
            &mut sunrise_protocol::connection::ConnReader,
            &mut sunrise_protocol::connection::ConnWriter,
        ),
    ) -> (
        sunrise_protocol::connection::ConnReader,
        sunrise_protocol::connection::ConnWriter,
    ) {
        use sunrise_protocol::connection::{Connection, Pipe};
        game.options.execution_role = super::super::execution_role::ExecutionRole::Client;
        let (client, server) = Pipe::pair();
        game.connect_local(client, "MenuReadTest").unwrap();
        finish_play_test(game, Connection::local(server), configure)
    }

    pub(in crate::game) fn finish_play_test(
        game: &mut super::Game<'_>,
        server: sunrise_protocol::connection::Connection,
        configure: impl FnOnce(
            &mut sunrise_protocol::connection::ConnReader,
            &mut sunrise_protocol::connection::ConnWriter,
        ),
    ) -> (
        sunrise_protocol::connection::ConnReader,
        sunrise_protocol::connection::ConnWriter,
    ) {
        use sunrise_protocol::packets::{Empty, configuration as cfg, login};
        let (mut reader, mut writer) = server.split();
        reader
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        reader.recv().unwrap();
        reader.recv().unwrap();
        writer
            .send(
                &login::Clientbound::LoginFinished(login::LoginFinished {
                    profile: login::GameProfile {
                        uuid: 42,
                        name: "MenuReadTest".into(),
                        properties: vec![],
                    },
                    session: 1,
                })
                .encode(),
            )
            .unwrap();
        let (id, body) = reader.recv().unwrap();
        assert!(matches!(
            login::Serverbound::decode(id, &body).unwrap(),
            login::Serverbound::LoginAcknowledged(_)
        ));
        reader.recv().unwrap();
        reader.recv().unwrap();
        configure(&mut reader, &mut writer);
        writer
            .send(&cfg::Clientbound::FinishConfiguration(Empty).encode())
            .unwrap();
        game.acknowledge_test_configuration();
        let (id, body) = reader.recv().unwrap();
        assert!(matches!(
            cfg::Serverbound::decode(id, &body).unwrap(),
            cfg::Serverbound::FinishConfiguration(_)
        ));
        (reader, writer)
    }

    /// The test server offers its payload manifest and reads the client's offer.
    pub(in crate::game) fn offer_test_payloads(
        reader: &mut sunrise_protocol::connection::ConnReader,
        writer: &mut sunrise_protocol::connection::ConnWriter,
        manifest: &sunrise_protocol::payload_negotiation::Manifest,
    ) {
        use sunrise_protocol::{
            packets::configuration as cfg,
            payload_negotiation::{CHANNEL, Control},
        };
        writer
            .send(
                &cfg::Clientbound::CustomPayload(cfg::CustomPayload {
                    channel: CHANNEL.into(),
                    data: Control::Offer(manifest.clone()).encode().unwrap(),
                })
                .encode(),
            )
            .unwrap();
        let (id, body) = reader.recv().unwrap();
        assert!(
            matches!(cfg::Serverbound::decode(id, &body).unwrap(), cfg::Serverbound::CustomPayload(p) if p.channel == CHANNEL)
        );
    }

    /// Spawns the connected client in the overworld as entity 7, profile 42.
    pub(crate) fn spawn_menu_test(
        game: &mut super::Game<'_>,
    ) -> sunrise_protocol::packets::play::SpawnInfo {
        use crate::game::remote::RemoteDimensionEntry;
        use sunrise_client::level_loading::Reason;
        use sunrise_protocol::packets::play::SpawnInfo;
        let spawn = SpawnInfo {
            dimension_type: 0,
            dimension: "minecraft:overworld".into(),
            seed: 0,
            game_type: 0,
            previous_game_type: None,
            debug: false,
            flat: true,
            last_death: None,
            portal_cooldown: 0,
            sea_level: 63,
        };
        game.enter_remote_dimension(&spawn, RemoteDimensionEntry::Login, Reason::Other);
        let remote = game.remote.as_mut().unwrap();
        remote.logged_in = true;
        remote.entity_id = 7;
        remote.profile = 42;
        spawn
    }

    #[allow(unused_imports)]
    use super::{
        boat_interact_passes, chat_after_event, item_frame_interact_passes, remote_object_swings,
    };

    #[test]
    #[ignore = "daily"]
    fn block_alias_survival_consumption_and_client_prediction() {
        use crate::game::{Audio, Game, Options, Progress, Settings, World, load_resources};
        use std::{path::PathBuf, sync::Arc};
        use sunrise_client::{
            chunk::Chunk,
            items::ItemStack,
            mod_content::{ContentMod, build},
        };
        use sunrise_mod_package::content::{Block, Content, Item};
        let base = PathBuf::from(
            std::env::var_os("SUNRISE_TEST_RESOURCES").expect("prepared resources required"),
        );
        let root = std::env::temp_dir().join(format!("sunrise-alias-game-{}", std::process::id()));
        let mut content = Content::declared("ruby");
        content
            .declare(
                &[Block {
                    id: "ruby:frame".into(),
                    template: "minecraft:stone".into(),
                    item: false,
                    ..Default::default()
                }],
                &["empty", "filled"].map(|name| Item {
                    id: format!("ruby:{name}"),
                    template: "minecraft:stick".into(),
                    block: Some("ruby:frame".into()),
                    ..Default::default()
                }),
            )
            .unwrap();
        let mods = [ContentMod {
            id: "ruby".into(),
            content,
            jars: Vec::new(),
        }];
        let inventory = sunrise_mod_package::enabled::EnabledMods::new([(
            "ruby".to_owned(),
            mod_api::Side::Both,
        )]);
        build(&base, std::path::Path::new(""), &mods, &root, &inventory).unwrap();
        let (registry, _) = sunrise_world::level::data::load_registry_with_flags(&root).unwrap();
        crate::game::api_values::register_game_keys((0..registry.len()).map(|id| {
            registry
                .state(sunrise_world::registry::BlockStateId(id as u32))
                .unwrap()
                .name
                .clone()
        }));
        let settings = Settings::default();
        let loaded = load_resources(
            sunrise_assets::resource_source::ResourceRoot::directory(root.clone()),
            Vec::new(),
            None,
            &root,
            &settings,
            std::time::Instant::now(),
            false,
            crate::game::execution_role::ExecutionRole::IntegratedServer,
            &Progress::new(),
            &mut |_, _| unreachable!(),
        )
        .unwrap();
        let mut options = Options::parse(std::iter::empty()).unwrap().unwrap();
        options.resources = root.clone();
        options.execution_role = crate::game::execution_role::ExecutionRole::IntegratedServer;
        let mut game = Game::new(
            &loaded.resources,
            loaded.pack,
            loaded.gui.unwrap(),
            options,
            settings,
            Audio::silent(&sunrise_assets::resource_source::ResourceSource::directory(
                root.clone(),
            )),
            None,
        )
        .unwrap();
        game.world = World::new(game.res.facts().air, 1);
        for x in -1..=1 {
            for z in -1..=1 {
                game.world
                    .chunks
                    .insert((x, z), Arc::new(Chunk::empty(x, z)));
            }
        }
        game.sync_tick_chunks();
        game.set_mode_id(0);
        game.player = None;
        game.selected = 0;
        let target = game
            .pack
            .assets
            .registry
            .default_state("ruby:frame")
            .unwrap();
        for (i, name) in ["ruby:empty", "ruby:filled"].iter().enumerate() {
            let pos = [i as i32 * 2, 70, 0];
            let hit = crate::game::Hit {
                pos,
                face: [0, 1, 0],
                inside: false,
                point: [f64::from(pos[0]) + 0.5, 70.0, 0.5],
            };
            let stack = ItemStack::new(name, 2);
            assert_eq!(game.items().get(name).unwrap().block, Some(target));
            let preview = game
                .remote_can_place(&hit, &stack)
                .expect("client predicts the alias");
            assert_eq!([preview.pos.x, preview.pos.y, preview.pos.z], pos);
            assert_eq!(
                game.world.block(pos[0], pos[1], pos[2]),
                game.res.facts().air,
                "prediction is dry"
            );
            *game.held_mut() = Some(stack);
            assert!(game.use_hand(Some(&hit)).unwrap());
            assert_eq!(game.world.block(pos[0], pos[1], pos[2]), target);
            let held = game.held().unwrap();
            assert_eq!((&*held.item, held.count), (*name, 1));
        }
        let blocked = crate::game::Hit {
            pos: [0, 70, 0],
            face: [0, 1, 0],
            inside: false,
            point: [0.5, 71.0, 0.5],
        };
        game.set_mode_id(2);
        let before = game.held().cloned();
        game.use_hand(Some(&blocked)).unwrap();
        assert_eq!(
            game.held(),
            before.as_ref(),
            "adventure refusal keeps the alias stack"
        );
        drop(game);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[allow(unused_imports)]
    use sunrise_client::{
        containers::{Beacon, BlockEntity},
        items::ItemStack,
    };
    #[allow(unused_imports)]
    use sunrise_core::nbt::Tag;

    /// #12040: a server's client moves at the speed the server's attributes give it, never its
    /// effects' own. Vanilla's server removes a Speed that runs out in ServerPlayer.doTick, after
    /// the tick's sendDirtyEntityData: ClientboundRemoveMobEffectPacket goes out at once and the
    /// movement speed without the effect's modifier a tick later, so for that tick the client still
    /// runs fast (a potion drunk starts the same way). A client a tick off is what GrimAC's
    /// Simulation flagged.
    #[test]
    #[ignore = "daily"]
    fn a_servers_client_takes_its_speed_from_the_attributes_not_the_effect() {
        use crate::game::{Audio, Game, Options, Progress, Settings, load_resources};
        use sunrise_client::motion::PlayerMotion;
        use sunrise_core::geometry::Vec3;
        use sunrise_protocol::packets::play::{
            AttributeModifier, AttributeSnapshot, Clientbound as C, RemoveMobEffect,
            UpdateAttributes, UpdateMobEffect,
        };
        let root = std::path::PathBuf::from(
            std::env::var_os("SUNRISE_TEST_RESOURCES").expect("prepared resources required"),
        );
        let settings = Settings::default();
        let loaded = load_resources(
            sunrise_assets::resource_source::ResourceRoot::directory(root.clone()),
            Vec::new(),
            None,
            &root,
            &settings,
            std::time::Instant::now(),
            false,
            crate::game::execution_role::ExecutionRole::Client,
            &Progress::new(),
            &mut |_, _| unreachable!(),
        )
        .unwrap();
        let mut options = Options::parse(std::iter::empty()).unwrap().unwrap();
        options.resources = root.clone();
        let mut game = Game::new(
            &loaded.resources,
            loaded.pack,
            loaded.gui.unwrap(),
            options,
            settings,
            Audio::silent(&sunrise_assets::resource_source::ResourceSource::directory(
                root.clone(),
            )),
            None,
        )
        .unwrap();
        game.init_remote_for_replay().unwrap();
        game.player = Some(PlayerMotion::new(game.motion, Vec3::new(0.5, 64.0, 0.5)).unwrap());
        let me = game.remote.as_ref().unwrap().entity_id;
        let speed = game.remote_effect_network_id("speed").unwrap();
        let attribute = {
            let p = game.player.as_ref().unwrap();
            let key = p.attributes.key("minecraft:movement_speed").unwrap();
            key.attribute().registry_id()
        };
        // Player.createAttributes: 0.1F.
        let walking = f64::from(0.1f32);
        // Speed II: 0.2 (a float) a level, multiplying the total.
        let amount = f64::from(0.2f32) * 2.0;
        let attributes = |sped: bool| {
            C::UpdateAttributes(UpdateAttributes {
                id: me,
                values: vec![AttributeSnapshot {
                    attribute,
                    base: walking,
                    modifiers: sped
                        .then(|| AttributeModifier {
                            id: "minecraft:effect.speed".into(),
                            amount,
                            operation: 2,
                        })
                        .into_iter()
                        .collect(),
                }],
            })
        };
        // Each step: the packets of one server tick, then a client tick.
        let mut step = |packets: Vec<C>| {
            for packet in packets {
                game.handle_packet(packet).unwrap();
            }
            game.tick_client_effects();
            game.player
                .as_mut()
                .unwrap()
                .attribute("minecraft:movement_speed")
                .unwrap()
        };
        let effect = C::UpdateMobEffect(UpdateMobEffect {
            id: me,
            effect: speed,
            amplifier: 1,
            duration: 60,
            flags: 2 | 4,
        });
        let fast = walking * (1.0 + amount);
        let close = |a: f64, b: f64| (a - b).abs() < 1e-9;
        let seen = [
            step(vec![effect]),
            step(vec![attributes(true)]),
            step(Vec::new()),
            step(vec![C::RemoveMobEffect(RemoveMobEffect {
                id: me,
                effect: speed,
            })]),
            step(vec![attributes(false)]),
        ];
        let expected = [walking, fast, fast, fast, walking];
        assert!(
            seen.iter().zip(expected).all(|(&s, e)| close(s, e)),
            "speed on each tick: {seen:?}, vanilla's client: {expected:?}"
        );
    }

    #[allow(unused_imports)]
    use super::Game;
}

#[cfg(test)]
pub(in crate::game) mod boat_push_order_tests;
#[cfg(test)]
pub(in crate::game) mod item_use_tick_tests;
#[cfg(test)]
pub(in crate::game) mod movement_input_tests;
#[cfg(test)]
pub(in crate::game) mod swim_start_tests;
/// `Entity.baseTick`'s fluid update for a mob this window only displays (`fluids`, sampled before
/// the tick), with the splash it draws, and the block it stands on.
pub(in crate::game) fn client_fluid_step(
    m: &mut sunrise_client::mobs::Mob,
    fluids: Option<sunrise_client::mobs::living::Fluids>,
    standing: Option<String>,
    own: Option<i32>,
    fx: &mut Vec<sunrise_client::mobs::MobEvent>,
) {
    if let Some(fluids) = fluids {
        let was = m.fluids.in_water;
        let first = m.first_tick;
        // Entity.baseTick saves the preceding eye sample before the new fluid update.
        m.was_eye_in_water = m.fluids.eye_in_water;
        m.fluids = fluids;
        // Entity.doWaterSplashEffect on the client copy. Particles
        // only: ClientLevel draws addParticle, and playSeededSound
        // drops Entity.playSound (the server broadcasts that).
        if m.fluids.in_water && !was && !first && Some(m.id as i32) != own {
            sunrise_client::mobs::living::client_water_splash(m, fx);
        }
        m.first_tick = false;
    }
    if let Some(name) = standing {
        m.standing_on = Some(name);
    }
}

#[cfg(test)]
pub(in crate::game) mod frozen_screen_effects_tests;
#[cfg(test)]
pub(in crate::game) mod placement_prediction_tests;
#[cfg(test)]
#[path = "player_tick_tests.rs"]
pub(in crate::game) mod player_tick_tests;
#[cfg(test)]
pub(in crate::game) mod rider_tick_tests;
#[cfg(test)]
pub(in crate::game) mod smoke_source_tests;

#[allow(unused_imports)]
pub(in crate::game) use inventory::{
    inventory_screen_slot, inventory_screen_slot_with_type, remote_station_kind,
};
