//! Multiplayer: the game driven by a server connection instead of the local
//! simulation. Mirrors vanilla's ClientPacketListener (chunks, entities,
//! inventory, health, time, weather, chat, respawn and dimension changes),
//! LocalPlayer.sendPosition (movement packets) and MultiPlayerGameMode
//! (block breaking with sequence numbers, use-item, attacks, container
//! clicks). The player still moves with the local physics, as the vanilla
//! client predicts its own movement; the server stays authoritative for
//! everything else.
mod selector_metadata;
use crate::game::collision_source::Mover;
use selector_metadata::hurt_sound_for;
mod attack_target;
mod break_prediction;
use break_prediction::Prediction;
mod connection;
mod fall_sound;
pub(in crate::game) mod general;
pub(in crate::game) use general::RemoteGeneral;
mod interaction_actions;
mod interaction_prediction;
use inventory::{
    CREATIVE_SYNCED, Predicted, changed_slots, creative_sync_slot, creative_sync_stacks,
    mark_synced, menu_items, predict_clicks,
};
mod level_format;
mod raw_ingress;
mod recipe_properties;
mod use_item;

mod block_entity_data;
#[cfg(test)]
mod block_entity_data_tests;
use super::raw_backlog::Received;
use super::remote_chunks::{ChunkDecoder, ChunkFormat, Next};
use super::remote_vehicles::PassengerSeat;
use super::render_tick::{mob_tracking_body, player_tracking_body};
use super::{Dim, Game, Mining, Screen, item_use::ticks_left, level_sounds::LevelSound};
use crate::game::api_values::TryIntoApi;
use mod_api::InteractionKey;
use movement::MovePacket;
#[cfg(test)]
// The move-packet tests reach it through `super::super`.
use movement::stepped_move;
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::{Duration, Instant},
};
use sunrise_client::first_person::{Hand, SwingAnimation};
use sunrise_client::input_queue::InputQueue;
use sunrise_client::{
    Result,
    blocks::Fluid,
    chunk::Chunk,
    containers::{BlockEntity, Furnace, FurnaceKind},
    entities::{Experience, ExperienceOrb, ItemEntity},
    hud::HudBatch,
    items::ItemStack,
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
use sunrise_core::{geometry::Vec3, random::RandomSource};
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
use winit::keyboard::KeyCode;
use {
    sunrise_core::nbt::Tag,
    sunrise_mod_runtime::attachments::{Holder, ResidentAttachments},
    sunrise_world::{
        level::InteractionResult,
        registry::BlockStateId,
        stepped_interpolation::{Path, PosRot, Start, SteppedInterpolation},
    },
};

mod abilities;
mod chat;
mod chat_received;
mod commands;
use chat::chat_type_decorate;
pub(in crate::game) use chat::{ChatTrust, Decoration};
use chat_received::QueuedChat;
pub(in crate::game) mod client_level_tick;
mod effect;
mod entity_section_steps;
mod inventory;
mod item_use;
mod menu_interactions;
mod minecarts;
use minecarts::CartLerp;
mod motion;
mod movement;
mod packets_entity;
mod packets_object;
mod packets_player;
mod packets_world;
mod queued_keys;
mod remote_attributes;
mod session;
mod status;
mod sync;
mod tab_entries;
mod tables;
mod tick;
mod ui;
#[path = "use.rs"]
mod r#use;

pub(super) use super::villager_client::builtin_registries;

mod chat_queue;

/// A text component the server sent, for the HUD.
pub(super) fn remote_text(tag: &Tag) -> sunrise_client::text::Text {
    use sunrise_client::scoreboard::{comp_text, tag_to_json};
    comp_text(&tag_to_json(tag))
}

/// `Player.getVoicePitch`: a constant 1.0
/// (`javap -p -c -constants net.minecraft.world.entity.player.Player`:
/// `0: fconst_1  1: freturn`), so a player's own hurt and death sounds are
/// neither raised for a baby nor jittered - `LivingEntity.getVoicePitch`, which
/// the mobs port, does both. It takes no random, so it draws nothing from the
/// level random either.
pub(super) fn player_voice_pitch() -> f32 {
    1.0
}

/// The live entity a bound sound names; absence means that object was removed.
fn entity_sound_state(
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
fn menu_title(tag: &Tag) -> sunrise_client::text::Text {
    remote_text(tag).resolve_legacy()
}

/// Streaming needs a generator; a server world never generates.
struct NoTerrain;
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
pub(super) struct Tracked {
    kind: String,
    position: [f64; 3],
    /// `Entity.getPositionCodec`'s base: the position the server last
    /// reported (AddEntity, MoveEntity, EntityPositionSync), which a move
    /// packet's delta counts from. A TeleportEntity moves the entity but not
    /// this base, so the server's next delta still lands where it means.
    codec_base: [f64; 3],
    on_fire: bool,
    mod_fire_immune: bool,
    silent: bool,
    /// `Entity.vehicle`: the entity it rides (`SetPassengers`), whatever either one's type.
    pub(in crate::game) vehicle: Option<u32>,
}

/// `Entity.startRiding` / `removePassenger` for a `SetPassengers` packet: the
/// listed entities ride `vehicle`, the ones that rode it and are no longer
/// listed got off. Every tracked entity counts, whatever its type, so a
/// server's item display or marker on a player hides that player's name tag
/// as vanilla's `isVehicle` does (#5666).
fn seat_passengers(entities: &mut HashMap<i32, Tracked>, vehicle: u32, passengers: &[i32]) {
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
pub(super) struct Lerp {
    target: [f64; 3],
    rotation: Option<(f32, f32)>,
    steps: u32,
}

/// `SteppedInterpolationHandler.doInterpolate`'s `setPos` and `setRot` on a
/// mob: its yaw kept continuous (the same turn as vanilla's, without the
/// wrap that would spin it in the frame between ticks), its pitch clamped.
pub(super) fn place_interpolated(m: &mut Mob, at: PosRot) {
    m.position = Vec3::new(at.position[0], at.position[1], at.position[2]);
    m.yaw += wrap_degrees(at.yaw - m.yaw);
    m.head_pitch = (at.pitch % 360.0).clamp(-90.0, 90.0);
}

/// Where a mob is, as its `SteppedInterpolationHandler` sees it.
fn mob_pos_rot(m: &Mob) -> PosRot {
    PosRot {
        position: [m.position.x, m.position.y, m.position.z],
        yaw: m.yaw,
        pitch: m.head_pitch,
    }
}

impl Game<'_> {
    /// `Entity.commonTick` for the mount this client drives, `mobs[i]`: an
    /// interpolation it had going when the client took it over runs on, its
    /// target carried along by the mount's own moves (where the mount fits).
    /// The mount's pose before the step: its tick saves the old pose
    /// (setOldPosAndRot) before the step in vanilla, so `drive_mount` puts this one
    /// back as the old pose once the tick has run.
    pub(super) fn interpolate_driven_mount(&mut self, i: usize) -> Option<(Vec3, f32, f32)> {
        let id = self.mobs[i].id as i32;
        let handler = self.remote.as_mut().and_then(|r| r.stepped.get_mut(&id))?;
        let m = &mut self.mobs[i];
        let mut scene = sunrise_client::collisions::SceneCollisions {
            world: &self.world,
            binding: self.collision,
            subject: None,
        };
        handler.follow(mob_pos_rot(m), |to| {
            let bounds = m.bounds_at(Vec3::new(to[0], to[1], to[2]));
            scene.unobstructed(bounds).unwrap_or(false)
        });
        let before = (m.position, m.yaw, m.head_pitch);
        let at = handler.interpolate()?;
        place_interpolated(m, at);
        // A step that crosses a section is filed where it happens.
        let to = m.position;
        if to != before.0 {
            let from = before.0;
            self.entity_section_step(id as u32, [from.x, from.y, from.z], [to.x, to.y, to.z]);
        }
        Some(before)
    }
    /// `Entity.getClientPositionAndRotation` of the mount this client drives:
    /// while an interpolation runs, where it is going.
    pub(super) fn driven_mount_heading(&self, m: &Mob) -> PosRot {
        let at = mob_pos_rot(m);
        self.remote
            .as_ref()
            .and_then(|r| r.stepped.get(&(m.id as i32)))
            .map_or(at, |h| h.heading(at))
    }
    /// The mount this client drives teleported to `at` (`moveOrInterpolateTo`):
    /// it eases there over its type's update interval.
    pub(super) fn interpolate_driven_mount_to(&mut self, i: usize, at: PosRot) {
        let m = &mut self.mobs[i];
        let Some(r) = self.remote.as_mut() else {
            return;
        };
        let handler = r.stepped.entry(m.id as i32).or_insert_with(|| {
            let interval = sunrise_client::lan::tracking::settings(&m.kind.id).1;
            SteppedInterpolation::new(interval as i32)
        });
        let entity = mob_pos_rot(m);
        if let Start::Snap(to) = handler.interpolate_to(
            entity,
            Some(Path::Linear(at.position)),
            Some((at.yaw, at.pitch)),
        ) {
            place_interpolated(m, to);
        }
    }
}

/// `Minecraft.services().profileKeySignatureValidator()`: the services' profile-key keys, which a
/// player's chat key must be signed by. Asked for once per connection to a server, off the game
/// thread.
pub(super) enum ServicesKeys {
    Pending,
    Ready(Vec<RsaPublicKey>),
    /// `canValidateProfileKeys()` is false: none could be had.
    Missing,
}
/// The signed-in account's `/minecraft/profile` fetch after joining.
enum OwnProfile {
    Idle,
    Fetching(std::sync::mpsc::Receiver<Option<sunrise_client::skins::Textures>>),
    Done(Option<sunrise_client::skins::Textures>),
}

pub(super) struct Remote {
    pub(super) professions: Arc<sunrise_client::mobs::villager::ResolvedProfessions>,
    pub session: Session,
    pub brand: String,
    pub(super) recipe_viewer: crate::game::recipe_book::ViewerChannels,
    /// This connection's configured feature flags, carried through reconfiguration.
    pub(super) features: std::sync::Arc<[String]>,
    /// The server announced it runs mods' quiet commands.
    pub quiet_commands: bool,
    /// The server advertised private custom recipe-book synchronization.
    pub custom_books: bool,
    /// The synced attachments the server sent, by holder.
    pub attachments: HashMap<Holder, ResidentAttachments>,
    /// The colliders the server sent, by entity id (`sunrise:collider_sync`).
    pub colliders: HashMap<u32, sunrise_client::colliders::ColliderState>,
    /// How old the copy of the collider carrying the player is (`sunrise:carry_age`).
    pub carry_age: sunrise_client::colliders::CarryAge,
    /// The server samples far terrain for us, this far at most (`lod_net`).
    pub far_terrain: Option<sunrise_client::lod_net::Offer>,
    pub(super) builtin: Arc<Builtin>,
    lang: Arc<Language>,
    pub(in crate::game) registries: Arc<Dynamic>,
    /// The block, item and fluid tags of `registries` by name, as native mods read them
    /// (`install_remote_tags`).
    pub(super) tags: Option<sunrise_client::items::Items>,
    /// Whether the published snapshot includes the item registry, even if its table is empty.
    pub(super) has_item_tags: bool,
    /// The game's enchantments and the server's that it lacks (a data
    /// pack's), from their synced data; `None` when the server has none.
    pub(super) enchantments: Option<Arc<sunrise_client::enchant::Enchantments>>,
    pub(super) joined: bool,
    /// `ServerReconfigScreen` is up since this tick.
    pub(super) reconfiguring: Option<u64>,
    /// `handleLogin` ran: vanilla's `Minecraft.player` exists from then on,
    /// while here the player is made at the first teleport.
    pub(super) logged_in: bool,
    status: String,
    pub(super) entity_id: i32,
    local_entity_silent: bool,
    /// `Minecraft.cameraEntity` when the server put the view on another
    /// entity (ClientboundSetCameraPacket); None while it is the player.
    pub(super) camera: Option<i32>,
    /// The item the player's use began with (`LivingEntity.useItem`).
    pub(super) use_item: Option<Arc<str>>,
    pub(super) dimension: DimensionType,
    /// The level's key (`minecraft:overworld`): where its far terrain is kept.
    pub(in crate::game) level: String,
    /// `level`'s key, as mods' handles name the level.
    pub(super) level_key: mod_api::DimensionKey,
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
    synced_type: Option<(String, serde_json::Value)>,
    /// Both environments by synchronized type name, with what they were built from, so that a
    /// level of an unchanged type is entered again without reading the biome files.
    level_environments: HashMap<String, level_format::LevelEnvironments>,
    /// Connection-owned facts, cached by level and the full named type identity.
    facts_cache: HashMap<
        String,
        (
            String,
            serde_json::Value,
            Arc<sunrise_world::dimension_type::DimensionFacts>,
        ),
    >,
    /// The levels this connection added to the process's table of levels: the only ones it drops.
    added_levels: std::collections::HashSet<String>,
    /// `ClientPlayerNetworkEvent.LoggingOut` was fired for this connection: every way out
    /// of a world reports it once.
    logged_out: bool,
    /// The other levels' worlds the server opened on `sunrise:levels` (`viewed_worlds`).
    pub(in crate::game) viewed: super::viewed_worlds::ViewedWorlds,
    /// The level's hashed seed (login, respawn): the folder of its kept chunks
    /// (`server_terrain::level_dir`).
    pub(super) hashed_seed: i64,
    /// What chunk packets decode with (the dimension's height and sky, the
    /// biome registry); replaced whenever one of them changes.
    pub(super) chunk_format: Arc<ChunkFormat>,
    /// Decodes chunk packets as they arrive, off the frame.
    decoder: ChunkDecoder,
    /// Options.serverRenderDistance: the server's view distance from the
    /// login or the last chunk-cache-radius packet.
    pub(super) view_distance: i32,
    /// `ClientLevel.serverSimulationDistance`: from the login or the last
    /// simulation-distance packet.
    pub(super) simulation_distance: i32,
    /// `BlockStatePredictionHandler.currentSequenceNr`: the level's, so a new level (a login,
    /// another dimension) counts from 0 again.
    sequence: i32,
    /// `BlockStatePredictionHandler.lastTeleportSequence`: the sequence of the last
    /// teleport, so a prediction made before it keeps its block but not its snap.
    last_teleport_sequence: i32,
    pub(super) position_sender: sunrise_client::player_sync::PositionSender,
    last_input: u8,
    was_sprinting: bool,
    /// The slot the server last heard (`MultiPlayerGameMode.carriedIndex`): a cell, so `send` can
    /// send a new one first (`ensureHasSentCarriedItem`).
    carried: std::cell::Cell<Option<usize>>,
    was_shared_flags: u8,
    was_living_flags: u8,
    last_recorded_pose: Option<crate::pose::Pose>,
    /// The last ClientboundPlayerAbilitiesPacket. Vanilla's LocalPlayer
    /// holds its abilities from the login on; here the player is made at
    /// the first teleport, which takes them then.
    abilities: Option<play::PlayerAbilities>,
    /// The old player's motion, from a Respawn with KEEP_ENTITY_DATA
    /// (`handleRespawn`: `newPlayer.setDeltaMovement(oldPlayer.getDeltaMovement())`),
    /// for the player made at the next teleport: a portal's teleport adds to it.
    respawn_velocity: Option<Vec3>,
    /// ServerboundPlayerLoadedPacket went out: the tick's keybinds act from then on.
    pub(in crate::game) loaded_sent: bool,
    /// The dimension of the client level the mods heard made
    /// (`native_client_level_load`), to be told dropped once.
    pub(super) heard_level: Option<String>,
    /// `ClientPacketListener.levelLoadTracker`: the loading screen after a login
    /// or respawn, until the player's section is shown.
    pub(super) level_load: sunrise_client::level_loading::LevelLoadTracker,
    /// The ClientInformation last sent (`Options.broadcastOptions` sends it
    /// again when a view distance, Chat, skin or Main Hand option changes).
    sent_info: sunrise_protocol::packets::configuration::ClientInformation,
    /// BlockStatePredictionHandler: blocks changed ahead of the server (a predicted break),
    /// with what it remembers of each of them. Until the server acknowledges the
    /// sequence its block updates only replace the remembered state; then the block
    /// takes it.
    predictions: HashMap<[i32; 3], Prediction>,
    /// The entities the level held when its `LocalPlayer` was added
    /// (`ClientLevel.addEntity` on login or respawn): `ClientLevel.tickEntities`
    /// ticks in the order entities were added, so these tick before the
    /// player and every later one after it.
    ticked_before_player: std::collections::HashSet<u32>,
    /// Open container: (window id, menu name).
    pub(super) menu: ClientMenu,
    /// The crafting result slot of the open menu as the server last set it
    /// (`CraftingMenu.slotChangedCraftingGrid`): the client never computes it.
    pub(super) craft_result: Option<ItemStack>,
    /// The open menu's title as the server named it, with its styles.
    menu_title: Option<sunrise_client::text::Text>,
    menu_title_snapshot: Option<Arc<mod_api::Text>>,
    pub(in crate::game) state_id: i32,
    use_target: Option<[i32; 3]>,
    /// A mod menu's extra data, kept for the `OpenScreen` of its window that
    /// follows (`sunrise:menu_extra`).
    pub(in crate::game) menu_extra: Option<sunrise_protocol::menu_extra::Message>,
    pub(super) entities: HashMap<i32, Tracked>,
    /// Per-tick remote entity scratch retained between frames.
    tick_fluids: Vec<Option<sunrise_client::mobs::living::Fluids>>,
    tick_rain: Vec<bool>,
    tick_standings: Vec<Option<String>>,
    tick_water_fx: Vec<sunrise_client::mobs::MobEvent>,
    /// Complete network riding graph, including invisible plugin seat entities.
    pub(super) passenger_vehicles: HashMap<u32, PassengerSeat>,
    pub(super) lerps: HashMap<i32, Lerp>,
    /// A native mod's general rows the server shows the window (`general.rs`).
    pub(super) general: general::RemoteGenerals,
    /// The minecarts the server moves with `ClientboundMoveMinecartPacket`
    /// (`NewMinecartBehavior`), which have no interpolation handler.
    pub(super) cart_lerps: HashMap<i32, CartLerp>,
    /// Living entities' `SteppedInterpolationHandler`s.
    pub(super) stepped: HashMap<i32, SteppedInterpolation>,
    /// `LivingEntity.lerpHeadTo`: the head yaw and the ticks left to reach it.
    pub(super) head_lerps: HashMap<i32, (f32, u32)>,
    /// The happy ghast the player just got off, until the server's teleport
    /// puts the player on top of it (`place_ghast_under`).
    pub(super) ghast_dismount: Option<u32>,
    /// Whether the player is the first passenger of its vehicle, the one
    /// `getControllingPassenger` names.
    pub(super) first_passenger: bool,
    /// The player's place in its vehicle's `getPassengers`.
    pub(super) passenger_index: usize,
    /// Item entities waiting for their item data: where they are and their motion.
    pub(super) pending_items: HashMap<i32, ([f64; 3], Vec3)>,
    health: (f32, i32, f32),
    /// `LocalPlayer.flashOnSetHealth`: set by the first health the server
    /// sends; from then on a drop starts the hurt animation.
    flash_on_set_health: bool,
    game_mode: i32,
    pub chat: Vec<(String, Instant)>,
    /// Lines received since joining, for the control socket's events.
    pub chat_received: u64,
    /// `ChatListener.delayedMessageQueue`: player chat held back by Chat
    /// Delay, with the signature a deletion names it by and what the
    /// narrator says once it shows.
    chat_queue: VecDeque<QueuedChat>,
    /// `ClientPacketListener.messageSignatureCache`: the signatures packed
    /// ones index, as the server's copy for this connection holds them.
    pub(super) signatures: sunrise_protocol::secure_chat::SignatureCache,
    /// `ChatListener.previousMessageTime`: when a message last showed, if one has.
    chat_previous: Option<Instant>,
    /// `ClientPacketListener.nextChatIndex`: the global index the next chat packet must carry.
    next_chat_index: i32,
    /// `serverEnforcesSecureChat`, from the login packet.
    enforces_secure_chat: bool,
    services_keys: Arc<std::sync::Mutex<ServicesKeys>>,
    /// The `RemoteChatSession.Data` each player's list entry last carried (INITIALIZE_CHAT).
    chat_sessions: HashMap<u128, Option<(u128, i64, Vec<u8>, Vec<u8>)>>,
    /// `PlayerInfo.messageValidator` of the players whose chat has been read.
    chat_validators: HashMap<u128, secure_chat::MessageValidator>,
    /// `ConnectScreen.lastNarration`: when "Joining the world" was last said.
    joining_narrated: Option<Instant>,
    /// The tree the chat colours, completes and parses with: the server's
    /// (ClientboundCommandsPacket) once it has sent one, else the game's own,
    /// plus the commands native mods own (`chat::chat_tree_for`). Rebuilt where
    /// that base changes, not at each keystroke.
    pub chat_tree: sunrise_client::command::Dispatcher,
    selector_metadata_supported: bool,
    /// Whether a `ClientboundCommandsPacket` has supplied the server's tree.
    /// The local fallback is for UI completion only; vanilla signs command
    /// arguments against an empty tree until the server sends one.
    pub(super) chat_tree_received: bool,
    /// The chat session, the last-seen window and the signature cache.
    pub(super) secure_chat: super::remote_chat::SecureChat,
    /// `ClientSuggestionProvider.customCompletionSuggestions`: the extra chat
    /// completions this server sent (ClientboundCustomChatCompletionsPacket).
    custom_completions: std::collections::BTreeSet<String>,
    players: HashMap<u128, String>,
    /// `ClientPacketListener.playerInfoMap` as player-list entries, and which of them are listed.
    tab_players: HashMap<u128, sunrise_client::tab_list::TabPlayer>,
    /// Each listed profile's unpacked `textures` property.
    skin_textures: HashMap<u128, sunrise_client::skins::Textures>,
    /// Active skin and cape from the services profile when the server sent none.
    own_profile: OwnProfile,
    /// The skin texture each profile's players show, once its download is ready.
    skins: HashMap<u128, String>,
    skins_revision: u64,
    published_skins: Option<(usize, u128, u64, u64)>,
    listed: std::collections::HashSet<u128>,
    /// `ClientboundLoginPacket.onlineMode`: the player list shows heads.
    pub(super) online_mode: bool,
    /// `ClientboundLoginPacket.levels`: the server's level keys, which
    /// `ClientPacketListener.levels()` hands to command suggestions.
    pub(super) levels: Vec<String>,
    /// Player entities in the level: entity id to profile UUID.
    pub(super) level_players: HashMap<i32, u128>,
    /// The profile name each player entity was created with (`RemotePlayer.getGameProfile`): it outlives the
    /// player-list entry, which servers remove to hide their NPCs from the list.
    level_player_names: HashMap<i32, String>,
    /// The `textures` each player entity found in the player list first
    /// (`AbstractClientPlayer.getPlayerInfo` keeps that `PlayerInfo`): its skin
    /// outlives the entry, which NPC plugins remove a moment after spawning.
    level_player_textures: HashMap<i32, sunrise_client::skins::Textures>,
    /// Entities with shared flag 6 (`Entity.isCurrentlyGlowing` on a client); mobs keep
    /// theirs in `Mob::glowing`.
    pub(super) glowing: std::collections::HashSet<i32>,
    /// The player's own shared flag 5 (`Entity.isInvisible`) as the server last set it.
    pub(super) own_invisible: bool,
    /// The player's own shared flag 0 (`Entity.isOnFire`) as the server last
    /// set it: its fire (blaze fireballs, fire aspect, lightning) never
    /// reaches the window's own `fire_ticks` otherwise.
    own_on_fire: bool,
    /// `LivingEntity.DATA_ARROW_COUNT_ID` and `DATA_STINGER_COUNT_ID` of the
    /// own player, as the server synced them.
    pub(super) own_arrows: i32,
    pub(super) own_stingers: i32,
    /// `Player.DATA_SHOULDER_PARROT_LEFT` and `_RIGHT` of the own player.
    pub(super) own_shoulders: [Option<i32>; 2],
    /// The local player's profile UUID (always in the level).
    pub(super) profile: u128,
    /// The local player's profile name: its score holder and team member.
    pub(super) profile_name: String,
    /// `MultiPlayerGameMode.destroyBlockPos`, `.destroyProgress` and the face last sent: the block
    /// the break targets, which outlives the break itself.
    destroying: Option<([i32; 3], u8, f32)>,
    /// `MultiPlayerGameMode.destroyingItem`: the main-hand stack the break began with.
    destroying_item: Option<ItemStack>,
    /// `MultiPlayerGameMode.isDestroying`: a break is running. `destroying` keeps the target after
    /// one ends (`destroyBlockPos` is written only when a break starts), so a dig resumed on the
    /// same block and item sends no new START and the server keeps its clock (#4124).
    is_destroying: bool,
    /// Per-hand swing policy, restored when the current input attempt ends.
    pub(in crate::game) interaction_swing: [bool; 2],
    destroy_delay: u32,
    pub(super) input: sunrise_client::interaction_input::InteractionInput,
    /// Keys whose presses send a packet (swap hands, drop, a mount's
    /// inventory): like the click queue, they wait for the tick's
    /// `handleKeybinds` phase rather than going out between window events.
    pub(super) pending_keys: InputQueue<KeyCode>,
    /// Whether the main and off hand still wait for the server's answer to
    /// a SWAP_ITEM_WITH_OFFHAND (`remote_hand_slot_set`).
    swap_answer: [bool; 2],
    stop_mining: bool,
    /// `CommonPlayerSpawnInfo.isFlat`: the level is a superflat one.
    pub(super) flat: bool,
    death_message: Option<sunrise_client::text::Text>,
    /// `LocalPlayer.showDeathScreen`: off with `immediate_respawn`.
    show_death_screen: bool,
    /// `Player.isReducedDebugInfo`: the server's `reduced_debug_info` rule.
    pub(super) reduced_debug_info: bool,
    /// `LocalPlayer.doLimitedCrafting`: whether limited crafting is enabled.
    pub(super) do_limited_crafting: bool,
    player_type: Option<Arc<MobType>>,
    /// Inventory, off hand and armour as last synchronised, for creative-mode
    /// slot updates (`creative_sync_stacks`).
    synced: Vec<Option<ItemStack>>,
    decode_errors: u32,
    /// Connection-owned `ClientClockManager` state, retained across dimensions.
    clocks: HashMap<i32, play::ClockState>,
    default_clock: Option<i32>,
    overworld_clock: Option<i32>,
    clock_game_time: i64,
    /// `ClientChunkCache.Storage`'s view centre (ClientboundSetChunkCacheCenterPacket).
    chunk_center: (i32, i32),
    /// Received events a frame left for the next one, in order, and when
    /// the oldest of them first waited (`poll_remote`).
    backlog: VecDeque<Received>,
    backlog_since: Option<Instant>,
    /// `ClientboundTransferPacket`'s target: when the connection ends the
    /// game connects here, the stored cookies carried over (`TransferState`).
    transfer: Option<String>,
    /// `ClientCommonPacketListenerImpl.serverLinks`: the links "Server
    /// Links..." in the pause menu lists (only `http`/`https` URIs kept).
    pub(super) server_links: Vec<sunrise_protocol::packets::configuration::ServerLink>,
    /// `ClientPacketListener.getDebugQueryHandler`: the F3+I query waiting for the server's data.
    pub(super) debug_query: super::debug_keys::DebugQuery,
}

/// The time a frame spends on chunk packets before it leaves the rest of
/// the received events for the next frame. A batch of 25 to 60 columns
/// (0.1 ms each on an M4 Pro) all in one frame was a 3 to 8 ms frame every
/// time the player crossed a chunk border at 32 chunks; they decode on the
/// chunk decoder's thread now, and the frame only inserts them.
const CHUNK_PACKETS_PER_FRAME: Duration = Duration::from_millis(1);
/// Events that have waited this long are all handled in the next frame,
/// whatever inserting their columns costs, up to a column still decoding:
/// the packets after a slow machine's chunks don't wait on its frames.
const BACKLOG_LIMIT: Duration = Duration::from_millis(50);
/// How long a tick about to run waits for one chunk column still decoding;
/// past it the column, and what came after it, wait for the next frame.
const TICK_DECODE_WAIT: Duration = Duration::from_millis(250);
/// The open menu name of a server's horse inventory, then its chest's
/// columns (the packet names no `MenuType`).
const MOUNT_MENU: &str = "sunrise:mount";
/// ServerboundPlayerInputPacket's shift bit.
const SHIFT_INPUT: u8 = 1 << 5;

fn face_direction(face: [i32; 3]) -> u8 {
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
fn number_format(f: &play::NumberFormat) -> sunrise_client::scoreboard::NumberFormat {
    use sunrise_client::scoreboard::{NumberFormat, tag_to_json};
    match f {
        play::NumberFormat::Blank => NumberFormat::Blank,
        play::NumberFormat::Styled(t) => NumberFormat::Styled(tag_to_json(t)),
        play::NumberFormat::Fixed(t) => NumberFormat::Fixed(tag_to_json(t)),
    }
}

impl Game<'_> {
    // remote_sound_name: remote/sync.rs

    /// The hurt sound the hurt player's own client plays from
    /// `ClientboundDamageEventPacket` (`LivingEntity.handleDamageEvent`):
    /// `Player.getHurtSound` is the damage type's effects, with the codec's
    /// `hurt` default when the type is unknown.
    pub(super) fn remote_hurt_sound(&self, source_type: i32) -> &'static str {
        match self.remote.as_ref() {
            Some(r) => hurt_sound_for(&r.registries, &self.health_resources.damage, source_type),
            None => sunrise_client::health::hurt_sound("hurt"),
        }
    }

    /// Leave the title for a server (ConnectScreen): an empty world fed by the connection.
    pub(super) fn connect_server(&mut self, address: &str, name: &str) -> Result<()> {
        self.connect_server_with_cookies(address, name, None)
    }

    /// `connect_server` carrying a transfer's stored server cookies
    /// (`TransferState`); None on an ordinary join.
    pub(super) fn connect_server_with_cookies(
        &mut self,
        address: &str,
        name: &str,
        cookies: Option<sunrise_client::net::Cookies>,
    ) -> Result<()> {
        self.connect_server_with_options(address, name, cookies, false, false)
    }

    /// `ConnectScreen.startConnecting`: keep LAN discovery's server kind through loading.
    pub(super) fn connect_server_with_options(
        &mut self,
        address: &str,
        name: &str,
        cookies: Option<sunrise_client::net::Cookies>,
        lan: bool,
        realm: bool,
    ) -> Result<()> {
        super::world_loading::world_data_used();
        if self.defer_world_entry(super::PendingWorldEntry::Server {
            address: address.to_owned(),
            name: name.to_owned(),
            cookies: cookies.clone(),
            lan,
            realm,
        })? {
            return Ok(());
        }
        self.join_world_rest()?;
        self.client_logging_out();
        if let Err(error) = self.join_server(address, name, None, cookies, lan, realm) {
            log::warn!("Sunrise: could not join server: {error}");
            let skip = self.settings.file.bool("skipMultiplayerWarning");
            match &mut self.title {
                Some(title) => {
                    title.skip_multiplayer_warning = skip;
                    title.failed(error);
                }
                None => {
                    let mut title = self
                        .new_title(std::env::current_dir()?.join("saves"))
                        .with_user(|| self.options.player_name());
                    title.skip_multiplayer_warning = skip;
                    title.failed(error);
                    self.show_title(title);
                }
            }
            self.capture(false);
        }
        Ok(())
    }
    // connect_remote_session: remote/session.rs

    // init_remote_for_replay: remote/session.rs

    // leave_server: remote/session.rs

    /// `Hud.onDisconnected`, from `Minecraft.disconnect` and `clearClientLevel`
    /// (a proxy's reconfiguration too), but the chat: joining clears it and a
    /// reconfiguration keeps it (`ChatComponent.storeState`).
    fn hud_disconnected(&mut self) {
        self.tab_list.reset();
        self.boss_overlay.reset();
        self.adv.toasts.clear();
        self.debug_charts.reset();
        self.titles.clear();
        self.titles.reset_times();
    }

    pub(super) fn send(&self, packet: S) {
        if self.remote.is_some() {
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
    fn send_action(&self, action: i32) {
        self.send(S::PlayerAction(play::PlayerAction {
            action,
            pos: [0; 3],
            direction: 0,
            sequence: 0,
        }));
    }

    /// `LocalPlayer.isShiftKeyDown` (and so `isSecondaryUseActive`): the input as of the player's
    /// last tick (`input.keyPresses`, what the last ServerboundPlayerInputPacket said), not the key
    /// now: a sneak pressed in the frame of a click acts from the next tick, as on the server.
    fn remote_shift_down(&self) -> bool {
        self.remote
            .as_ref()
            .is_some_and(|r| r.last_input & SHIFT_INPUT != 0)
    }
    /// Whether secondary use skips a block's default interaction on this client.
    pub(super) fn remote_secondary_use_skips_block(&self) -> bool {
        self.remote_shift_down()
            && (self.stack_in(self.selected).is_some() || self.offhand.is_some())
    }
    fn next_sequence(&mut self) -> i32 {
        let r = self.remote.as_mut().expect("remote");
        r.sequence += 1;
        r.sequence
    }
    fn item_name(&self, id: i32) -> Arc<str> {
        let r = self.remote.as_ref().expect("remote");
        Arc::from(
            r.builtin
                .get("minecraft:item")
                .and_then(|i| i.name(id))
                .unwrap_or("minecraft:air"),
        )
    }

    // remote_effect_network_id: remote/sync.rs
    fn item_id(&self, name: &str) -> i32 {
        let r = self.remote.as_ref().expect("remote");
        r.builtin
            .get("minecraft:item")
            .and_then(|i| i.id(name))
            .unwrap_or(0)
    }

    /// A stack from the server, decoded as the saved and command item NBT
    /// are, so tooltips and models see every field.
    pub(super) fn to_stack(&self, s: &Option<Stack>) -> Option<ItemStack> {
        self.decode_remote_stack(s.as_ref()?)
    }

    fn decode_remote_stack(&self, stack: &Stack) -> Option<ItemStack> {
        let r = self.remote.as_ref()?;
        if !sunrise_protocol::item::custom::permitted(stack, r.registries.custom_components)
            .unwrap_or(false)
        {
            log::warn!("Dropping a stack with unnegotiated custom components");
            return None;
        }
        Some(sunrise_client::item_sync::from_network(
            stack,
            &r.builtin,
            &r.registries,
        ))
    }

    /// A stack to the server (`SetCreativeModeSlot`): the same component
    /// patch a Sunrise server sends its clients, with the registries' ids, so
    /// the server's echo doesn't hand back a bare item.
    fn to_network(&self, s: &Option<ItemStack>) -> Option<Stack> {
        let r = self.remote.as_ref()?;
        // A stack as the server sent it, only its count changed: its own
        // components go back, as a vanilla client keeps them.
        if let Some((stack, patch)) = s.as_ref().and_then(|s| Some((s, s.wire_patch()?))) {
            let mut out = Stack::new(self.item_id(&stack.item), stack.count as i32);
            let safe = r.registries.custom_components
                || sunrise_protocol::item::custom::entries().next().is_none()
                || sunrise_protocol::item::WirePatch::decode(&patch)
                    .and_then(|(added, removed)| {
                        let decoded = Stack {
                            added,
                            removed,
                            ..Stack::new(out.item, out.count)
                        };
                        sunrise_protocol::item::custom::permitted(&decoded, false)
                    })
                    .unwrap_or(false);
            if safe {
                out.wire = sunrise_protocol::item::WirePatch(Some(patch));
                return Some(out);
            }
            log::warn!("Cannot send cached custom components to this peer");
        }
        sunrise_client::lan::guest::stack_to_network(
            |name| self.item_id(name),
            // Built-in first, as a hub's `SyncData::id` does: potions are one.
            |registry, name| {
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
        )
    }

    /// `HashedStack.create` with the connection's registries (the client's
    /// `decoratedHashOpsGenerator`).
    fn hashed_stack(&self, s: &Option<ItemStack>) -> sunrise_protocol::Result<Option<Hashed>> {
        if s.is_none() {
            return Ok(None);
        }
        let r = self
            .remote
            .as_ref()
            .ok_or_else(|| sunrise_protocol::Error::new("no live menu connection"))?;
        let names = SyncedNames {
            builtin: &r.builtin,
            dynamic: &r.registries,
        };
        let stack = self
            .to_network(s)
            .ok_or_else(|| sunrise_protocol::Error::new("cannot encode a nonempty stack"))?;
        Hashed::create(&stack, &names)
            .map(Some)
            .map_err(|e| sunrise_protocol::Error::new(e.to_string()))
    }

    /// A line in the connection's chat, with its styles, click and hover
    /// events (ChatComponent.addMessage). The lines the Chat option hides
    /// are dropped there (its `visibleMessageFilter`).
    pub(super) fn add_chat_line(
        &mut self,
        content: sunrise_client::text::Text,
        source: sunrise_client::chat::ChatSource,
    ) -> bool {
        self.signed_chat_line(content, source, None, ChatTrust::Secure)
    }

    /// `chat_line` of a player message, keeping its signature for a
    /// deletion (`GuiMessage.signature`).
    fn signed_chat_line(
        &mut self,
        content: sunrise_client::text::Text,
        source: sunrise_client::chat::ChatSource,
        signature: Option<Signature>,
        trust: ChatTrust,
    ) -> bool {
        if !self.ui.chat.shows(source) {
            return false;
        }
        let text = content.plain();
        let focused = self.screen == Screen::Chat;
        if source == sunrise_client::chat::ChatSource::Player {
            self.ui.chat.add_signed(
                &self.gui.atlas,
                content,
                signature,
                trust.tag(),
                self.visual_ticks(),
                focused,
            );
        } else {
            // addServerSystemMessage and addClientSystemMessage both carry
            // GuiMessageTag.systemSinglePlayer: the grey bar at the line's left.
            let indicator = Some(sunrise_client::chat::SYSTEM_INDICATOR);
            self.ui.chat.add(
                &self.gui.atlas,
                content,
                indicator,
                source,
                self.visual_ticks(),
                focused,
            );
        }
        if let Some(r) = self.remote.as_mut() {
            r.chat.push((text, Instant::now()));
            r.chat_received += 1;
            if r.chat.len() > 100 {
                r.chat.remove(0);
            }
        }
        true
    }

    /// `ChatListener.handleMessage` for player and disguised chat: held in the
    /// queue while Chat Delay is set, shown at once otherwise. `narration` is
    /// the chat type's narration of it (`decorateNarration`).
    fn handle_player_chat(&mut self, chat: QueuedChat) {
        if self.chat_delay_ms() > 0 {
            if let Some(r) = self.remote.as_mut() {
                r.chat_queue.push_back(chat);
            }
        } else {
            self.accept_chat(chat);
        }
    }

    /// `ChatListener.handleChatMessageError`: a message that fails validation shows as the red,
    /// italic `chat.validation_error` in its chat type (queued under Chat Delay as any
    /// message). Its signature counts as processed but not shown, for the acknowledgement the
    /// client sends the server. Vanilla also
    /// skips a blocked sender and one restricted to friends (not kept here either) and
    /// narrates the line.
    fn handle_chat_message_error(
        &mut self,
        chat_type: &play::ChatTypeBound,
        signature: Option<&Signature>,
    ) {
        if let Some(signature) = signature {
            self.mark_message_processed(signature, false);
        }
        let error = serde_json::json!({
            "translate": "chat.validation_error",
            "color": "red",
            "italic": true,
        });
        let line = self.decorate_chat(chat_type, &error, Decoration::Chat);
        // saySystemChatQueued(boundChatType.decorateNarration(CHAT_VALIDATION_ERROR)).
        let narration = self
            .decorate_chat(chat_type, &error, Decoration::Narration)
            .plain();
        self.handle_player_chat(QueuedChat {
            line,
            signature: None,
            trust: ChatTrust::Error,
            narration,
            sender: 0,
            chat_type: chat_type.chat_type,
            player: None,
        });
    }

    /// `ChatTrustLevel.evaluate` of a player message, with
    /// `ChatListener.evaluateTrustLevel`: the player's own message on their
    /// own server is secure. `decorated` is the line shown, `unsigned` the
    /// server's replacement for the signed content.
    fn chat_trust(
        &self,
        sender: u128,
        signed: bool,
        timestamp: i64,
        content: &str,
        unsigned: Option<&Tag>,
        decorated: &sunrise_client::text::Text,
    ) -> ChatTrust {
        use sunrise_client::scoreboard::{comp_text, tag_to_json};
        if self.integrated.is_some() && self.remote.as_ref().is_some_and(|r| r.profile == sender) {
            return ChatTrust::Secure;
        }
        // PlayerChatMessage.hasExpiredClient: MESSAGE_EXPIRES_AFTER_CLIENT is
        // the server's five minutes and two more.
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as i64);
        let expired = now
            > timestamp.saturating_add(
                sunrise_protocol::secure_chat::EXPIRES_AFTER_SERVER + 2 * 60 * 1000,
            );
        if !signed || expired {
            return ChatTrust::NotSecure;
        }
        // ChatTrustLevel.isModified: the signed content is not in the line, or
        // the replacement uses a font other than the default.
        let font_changed = unsigned.is_some_and(|tag| {
            comp_text(&tag_to_json(tag)).spans.iter().any(|span| {
                span.style
                    .font
                    .as_deref()
                    .is_some_and(|font| font != "minecraft:default")
            })
        });
        if !decorated.plain().contains(content) || font_changed {
            ChatTrust::Modified(content.to_owned())
        } else {
            ChatTrust::Secure
        }
    }

    /// `ClientPacketListener.handleDeleteChat`: the packed signature names a
    /// message; one still held back by Chat Delay never shows
    /// (`ChatListener.removeFromDelayedMessageQueue`), a shown one is
    /// replaced (`ChatComponent.deleteMessage`). A signature the cache
    /// doesn't hold is an invalid packet.
    fn handle_delete_chat(&mut self, packed: &(i32, Option<Vec<u8>>)) -> Result<()> {
        let Some(r) = self.remote.as_mut() else {
            return Ok(());
        };
        let Some(signature) = r.signatures.unpack(packed) else {
            let reason = Text::translate("multiplayer.disconnect.invalid_packet", &[]);
            return self.leave_server(Some(reason));
        };
        r.secure_chat.ignore_pending(&signature);
        let queued = r.chat_queue.len();
        r.chat_queue
            .retain(|chat| chat.signature.as_ref() != Some(&signature));
        if r.chat_queue.len() == queued {
            let ticks = self.visual_ticks();
            self.ui
                .chat
                .delete_message(&self.gui.atlas, &signature, ticks);
        }
        Ok(())
    }

    /// A message's `accept`: shown, it is narrated (`narrateChatMessage`) and
    /// starts the Chat Delay wait again.
    fn accept_chat(&mut self, chat: QueuedChat) -> bool {
        let signature = chat.signature;
        let shown = self.player_chat_line(chat);
        if let Some(signature) = &signature {
            self.mark_message_processed(signature, shown);
        }
        if shown && let Some(r) = self.remote.as_mut() {
            r.chat_previous = Some(Instant::now());
        }
        shown
    }

    /// `showMessageToPlayer`: the line joins the chat if Only Show Secure
    /// Chat, mods and the Chat option let it through, and the narrator says
    /// it (`sayChatQueued`).
    fn player_chat_line(&mut self, chat: QueuedChat) -> bool {
        let QueuedChat {
            mut line,
            signature,
            trust,
            narration,
            sender,
            chat_type,
            player,
        } = chat;
        // ChatListener.showMessageToPlayer: Only Show Secure Chat drops what is not secure.
        if matches!(trust, ChatTrust::NotSecure) && self.settings.file.bool("onlyShowSecureChat") {
            return false;
        }
        // handleChatMessageError says its line as system chat, and mods are not told of it.
        let error = matches!(trust, ChatTrust::Error);
        let (system, kind, player) = (false, Some(chat_type), player.as_ref());
        if !error && !self.chat_received(system, false, sender, kind, player, &mut line) {
            return false;
        }
        let source = sunrise_client::chat::ChatSource::Player;
        let shown = self.signed_chat_line(line, source, signature, trust);
        if shown && error {
            self.say_system_chat_queued(&sunrise_client::text::Text::untrusted(&narration));
        } else if shown {
            self.say_chat_queued(&narration);
        }
        shown
    }

    /// The minecarts the server sent, for the control socket's `status`
    /// (their velocity is not tracked here).
    pub(super) fn remote_carts(&self) -> Option<Vec<serde_json::Value>> {
        let r = self.remote.as_ref()?;
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

    /// The account playing this game.
    pub(super) fn auth(&self) -> Option<std::sync::Arc<sunrise_client::auth::AuthSession>> {
        sunrise_client::auth::current()
    }

    /// `ClientSuggestionProvider.customCompletionSuggestions` of the server
    /// connection (none without one).
    pub(super) fn remote_custom_completions(&self) -> std::collections::BTreeSet<String> {
        self.remote
            .as_ref()
            .map(|r| r.custom_completions.clone())
            .unwrap_or_default()
    }

    /// The names in the server's player list, sorted.
    pub(super) fn remote_online_names(&self) -> Option<Vec<String>> {
        let r = self.remote.as_ref()?;
        let mut names: Vec<String> = r.players.values().cloned().collect();
        names.sort();
        Some(names)
    }

    /// The open menu's window id (0: the inventory).
    pub(super) fn remote_menu_id(&self) -> i32 {
        self.remote
            .as_ref()
            .and_then(|r| r.menu.as_ref())
            .map_or(0, |m| m.0)
    }

    /// The server's last `n` chat lines, newest first.
    pub(super) fn remote_chat(&self, n: usize) -> Option<Vec<String>> {
        let r = self.remote.as_ref()?;
        Some(r.chat.iter().rev().take(n).map(|l| l.0.clone()).collect())
    }

    /// Received events left for the next frame (`poll_remote`): until they
    /// are handled, the terrain the server sent is not all there.
    pub(super) fn remote_backlog(&self) -> usize {
        self.remote.as_ref().map_or(0, |r| r.backlog.len())
    }

    /// The attack clicks queued for the next tick (`KeyMapping.click`, not yet handled).
    pub(in crate::game) fn remote_attack_clicks_queued(&self) -> u32 {
        self.remote.as_ref().map_or(0, |r| {
            r.input
                .queued(sunrise_client::interaction_input::Button::Attack)
        })
    }

    /// A headless command oracle has a real connection but no movement tick.
    /// Acknowledge loaded terrain without predicting or sending movement.
    pub(super) fn poll_stationary_client(&mut self) -> Result<()> {
        self.poll_remote()?;
        let loaded = self.player.as_ref().is_some_and(|p| {
            self.streaming.is_lit(
                (p.position.x.floor() as i32) >> 4,
                (p.position.z.floor() as i32) >> 4,
            )
        });
        if loaded && self.remote.as_ref().is_some_and(|r| !r.loaded_sent) {
            self.send(S::PlayerLoaded(Empty));
            let r = self.remote.as_mut().unwrap();
            r.loaded_sent = true;
            r.level_load.finish();
        }
        Ok(())
    }

    /// Process what the network thread received (once per frame, like the
    /// client's packet processor), in order. Chunk packets get
    /// `CHUNK_PACKETS_PER_FRAME`: once a frame has spent it, the events from
    /// the next chunk packet on wait for the next frame, unless they have
    /// already waited `BACKLOG_LIMIT`. A chunk packet still decoding holds
    /// back what follows it.
    pub(super) fn poll_remote(&mut self) -> Result<()> {
        self.poll_remote_with(false)
    }

    /// `Minecraft.runTick` handles every packet that came (`runAllTasks`)
    /// before the frame's ticks. What this frame's poll left for later (chunks
    /// past its budget or still decoding, and everything after them) is taken
    /// when a tick is about to run, so the tick moves through those chunks, as
    /// vanilla's does. A batch's `ChunkBatchReceived` is not among them: the
    /// network thread answers it (`net::receive_play`).
    pub(in crate::game) fn poll_remote_before_tick(&mut self) -> Result<()> {
        if self.remote.as_ref().is_none_or(|r| r.backlog.is_empty()) {
            return Ok(());
        }
        self.poll_remote_with(true)
    }
    fn poll_remote_with(&mut self, before_tick: bool) -> Result<()> {
        let polled = self.poll_remote_events(before_tick);
        // The block events and block changes of a piston clock start and end
        // thousands of moves at once: the collision view and the renderer
        // take them all in one sync, before the tick or frame reads them.
        if std::mem::take(&mut self.moving_pistons_stale) {
            self.sync_moving_pistons();
        }
        polled
    }

    /// ClientSuggestionProvider.customSuggestion: the chat's completion
    /// request for an `ask_server` node goes to the server.
    fn send_suggestion_request(&mut self) {
        if self.remote.is_none() {
            return;
        }
        let Some((id, command)) = self
            .ui
            .chat_screen
            .as_mut()
            .and_then(|s| s.take_request())
            .or_else(|| self.command_block_suggestion_request())
        else {
            return;
        };
        let mut w = sunrise_protocol::buf::Writer::new();
        w.varint(id);
        w.string(&command);
        self.send(S::Unknown(
            sunrise_protocol::ids::play::serverbound::COMMAND_SUGGESTION,
            w.data,
        ));
    }

    // handle_command_suggestions: remote/commands.rs
    pub(in crate::game) fn set_registries(&mut self, registries: Arc<Dynamic>) {
        let entries: Vec<(&str, serde_json::Value)> = registries
            .registries
            .get("minecraft:enchantment")
            .into_iter()
            .flatten()
            .filter_map(|(name, data)| {
                Some((
                    name.as_str(),
                    sunrise_client::scoreboard::tag_to_json(data.as_ref()?),
                ))
            })
            .collect();
        let enchantments = match self
            .stations()
            .enchantments
            .with_entries(entries.iter().map(|(n, v)| (*n, v)))
        {
            Ok(table) => table.map(Arc::new),
            Err(error) => {
                log::warn!("Sunrise: refused enchantment registry: {error}");
                return;
            }
        };
        // The server's biomes the game lacks (a world data pack's) join the
        // game's table, from the data it sent: colours, meshing and the
        // chunk format all go through that table. The window's own server
        // thread set the world's table already.
        if self.integrated.is_none() {
            let table = registries
                .registries
                .get(BIOME_REGISTRY)
                .and_then(|list| client_biomes(self.res.biomes(), list))
                .map(Arc::new);
            if table.is_some() || self.world_biomes.is_some() {
                self.world_biomes_base = table.clone();
                self.world_biomes = table;
                let context = self.worker_context();
                self.streaming.reload_resources(&mut self.world, context);
            }
        }
        let biomes: Vec<u16> = registries
            .registries
            .get(BIOME_REGISTRY)
            .map(|list| {
                list.iter()
                    .map(|(name, _)| world_biomes!(self).id(name))
                    .collect()
            })
            .unwrap_or_default();
        self.server_items = self.server_items(&registries);
        if let Some(r) = self.remote.as_mut() {
            r.enchantments = enchantments;
            let bits = ceil_log2(biomes.len());
            let column = r.chunk_format.layout();
            r.chunk_format =
                Arc::new(ChunkFormat::new(&r.dimension, biomes, bits).in_column(column));
            r.registries = registries;
            r.resolve_clocks();
            self.reset_creative_contents();
            self.refresh_creative_search_tags();
        }
    }

    /// `ClientChunkCache.Storage.inRange`: within the storage around the
    /// view centre, `calculateStorageRange` (the server's radius, at least
    /// two, plus three) chunks each way.
    pub(super) fn remote_chunk_in_range(&self, x: i32, z: i32) -> bool {
        if self.replay.playing() {
            return true;
        }
        self.remote
            .as_ref()
            .is_none_or(|r| in_storage_range(r.view_distance, r.chunk_center, x, z))
    }
    fn chunk_inside_remote_range(center: (i32, i32), distance: i32, x: i32, z: i32) -> bool {
        in_storage_range(distance, center, x, z)
    }

    /// The columns a moved or resized storage no longer holds. Returns the
    /// positions it freed (`forget_remote_columns`).
    pub(in crate::game) fn drop_remote_chunks_out_of_range(
        &mut self,
        center: (i32, i32),
        distance: i32,
    ) -> Vec<(i32, i32)> {
        let mut gone: sunrise_client::fast_hash::FxHashSet<(i32, i32)> = self
            .world
            .chunks
            .keys()
            .copied()
            .filter(|&(x, z)| !Self::chunk_inside_remote_range(center, distance, x, z))
            .collect();
        self.forget_remote_columns(&mut gone, true)
    }

    /// Every column goes with the client level. Returns the positions it
    /// freed (`forget_remote_columns`).
    pub(in crate::game) fn forget_all_remote_columns(&mut self) -> Vec<(i32, i32)> {
        let mut gone = self.world.chunks.keys().copied().collect();
        self.forget_remote_columns(&mut gone, false)
    }

    /// Forgets the `gone` columns (and empties the set): their chunks (freed on a thread
    /// of their own when there are many), their sections in the renderer and their block
    /// entities, each store once for all of them. When the level stays open,
    /// far terrain keeps their last state (`server_terrain`). Returns the
    /// positions it freed, in `gone`'s order: only those still resident,
    /// so a later forget of the same positions frees nothing.
    pub(in crate::game) fn forget_remote_columns(
        &mut self,
        gone: &mut sunrise_client::fast_hash::FxHashSet<(i32, i32)>,
        keep_terrain: bool,
    ) -> Vec<(i32, i32)> {
        if gone.is_empty() {
            return Vec::new();
        }
        // The client chunk seam: one unload per column still resident, with
        // its data still there, before `take_remote_columns` frees it.
        let freed_at: Vec<(i32, i32)> = gone
            .iter()
            .filter(|&&(x, z)| self.world.chunks.contains_key(&(x, z)))
            .copied()
            .collect();
        for &(x, z) in &freed_at {
            self.client_chunk_unload(x, z, keep_terrain);
        }
        let freed =
            sunrise_client::streaming::Streaming::take_remote_columns(&mut self.world, gone);
        self.streaming.forget_columns(&mut self.world, gone);
        if keep_terrain && let Some(lod) = &mut self.lod {
            for chunk in &freed {
                lod.observe(chunk.clone(), self.res.facts());
            }
        }
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.remove_columns(gone.iter().copied());
        }
        sunrise_client::chunk::free_columns(freed, gone.len());
        self.world
            .block_entities
            .unload(|k| gone.contains(&(k.0 >> 4, k.2 >> 4)));
        if let Some(r) = self.remote.as_mut() {
            r.attachments.retain(|holder, _| match *holder {
                Holder::Chunk([x, z]) => !gone.contains(&(x, z)),
                Holder::BlockEntity([x, _, z]) => !gone.contains(&(x >> 4, z >> 4)),
                _ => true,
            });
        }
        gone.clear();
        freed_at
    }

    /// `handleAnimate`: a player wakes up, or an entity gets the critical
    /// hit (`CRIT`) or magic critical hit (`ENCHANTED_HIT`) emitter.
    fn remote_animate(&mut self, id: i32, action: u8) {
        use sunrise_client::particles::{ParticleOptions, ParticleType};
        let Some((target, body)) = self.remote_tracking_body(id) else {
            return;
        };
        let particle = match action {
            // Player.stopSleepInBed.
            0 => {
                if target == TrackingTarget::LocalPlayer {
                    self.sleeping = None;
                    self.sleep_timer = 100;
                }
                return;
            }
            1 => ParticleType::Crit,
            2 => ParticleType::EnchantedHit,
            _ => return,
        };
        self.particles
            .tracking_emitter(ParticleOptions::simple(particle), target, body);
    }

    /// `handleServerData`: the server list entry of the server joined keeps
    /// the icon the server sends (`ServerList.saveSingleServer`). Single
    /// player has no entry.
    fn remote_server_data(&mut self, icon: Option<&[u8]>) {
        let (Some(r), Some(png)) = (self.remote.as_ref(), icon) else {
            return;
        };
        if self.integrated.is_some() {
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

    /// Keep the renderable block entities (chests, furnaces) in step with blocks.
    /// `previous`: the block's state before, when it changed. A column sent again has
    /// none, and keeps its block entities' synced attachments: the server does not send
    /// them again (`remote_chunk` drops those of block entities the column lost).
    pub(super) fn sync_block_entity(
        &mut self,
        pos: (i32, i32, i32),
        previous: Option<BlockStateId>,
        state: BlockStateId,
    ) {
        let name = self.res.facts().get(state).name.clone();
        // LevelChunk.setBlockState: another block's block entity goes, even
        // one of the same type (a mod's blocks may share one).
        let other_block = previous.is_some_and(|p| self.res.facts().get(p).name != name);
        self.ensure_stateful_entity_table();
        // State-aware factories decide creation only. A valid same-block
        // instance survives even when the new state would create none.
        if !other_block
            && self.world.block_entities.stateful_entity(state.0)
            && matches!(
                self.world.block_entities.get(&pos),
                Some(BlockEntity::Mod(_))
            )
        {
            return;
        }
        // A mod block's is the mod's own, as on the server.
        let removed = match self.new_block_entity_for_state(state) {
            Some(entity) => {
                let same = self.world.block_entities.get(&pos).is_some_and(|e| {
                    e.id() == entity.id() && !(other_block && matches!(e, BlockEntity::Mod(_)))
                });
                if same {
                    return;
                }
                self.world.block_entities.replace(pos, entity)
            }
            None => self.world.block_entities.remove(&pos),
        };
        if let Some(previous) = previous {
            self.retire_client_block_entity([pos.0, pos.1, pos.2], previous, removed);
        }
    }

    /// The player's position, for `retainKnownServerState`'s `player.position()`.
    fn player_position(&self) -> [f64; 3] {
        let at = self
            .player
            .as_ref()
            .map(|p| p.position)
            .unwrap_or(Vec3::ZERO);
        [at.x, at.y, at.z]
    }

    /// `Entity.isColliding(pos, state)` for the local player: the state's own
    /// collision shape, at the player's pose, against the player's box.
    fn block_meets_player(&self, pos: [i32; 3], state: BlockStateId) -> bool {
        let Some(player) = self.player.as_ref() else {
            return false;
        };
        let entity = player.bounds();
        let context = sunrise_world::collision_context::CollisionContext::for_entity(
            sunrise_world::collision_context::CollisionEntity {
                identity: sunrise_world::entity_obstruction::EntityIdentity(0),
                kind: sunrise_world::collision_context::EntityContextKind::Player,
                y: player.position.y,
                descending: player.shift_key_down,
                main_hand: None,
            },
            false,
        );
        let mut scene = sunrise_client::collisions::SceneCollisions {
            world: &self.world,
            binding: self.collision,
            subject: None,
        };
        let at = sunrise_core::position::BlockPos::new(pos[0], pos[1], pos[2]);
        match self
            .collision
            .collision_shape_with_context(&mut scene, state, at, &context)
        {
            Ok(shape) => shape_meets(&shape, pos, entity),
            Err(_) => false,
        }
    }

    /// `Entity.absSnapTo`: the remembered point, clamped to the level's edge.
    /// The velocity is left alone, and the player has no separate previous
    /// position to reset (its box is drawn from `position`).
    fn snap_player_to(&mut self, at: [f64; 3]) {
        let Some(player) = self.player.as_mut() else {
            return;
        };
        let x = at[0].clamp(-3.0e7, 3.0e7);
        let z = at[2].clamp(-3.0e7, 3.0e7);
        player.position = Vec3::new(x, at[1], z);
    }

    /// `Entity.isVehicle` on a client: the entities a tracked entity rides.
    pub(super) fn remote_vehicle_ids(&self) -> impl Iterator<Item = u32> + '_ {
        self.remote
            .iter()
            .flat_map(|r| r.entities.values().filter_map(|t| t.vehicle))
    }

    /// The `SetPassengers` bookkeeping of the tracked entities.
    pub(super) fn remote_seat_passengers(&mut self, vehicle: u32, passengers: &[i32]) {
        if let Some(r) = self.remote.as_mut() {
            seat_passengers(&mut r.entities, vehicle, passengers);
        }
    }

    /// `recreateFromPacket`: yRot and xRot from the packet, the body and head
    /// both at its head yaw (LivingEntity).
    pub(super) fn client_attachment_entity_exists(&self, holder: Holder) -> bool {
        self.remote.as_ref().is_some_and(|remote| match holder {
            Holder::Entity(id) => remote.entities.contains_key(&(id as i32)),
            Holder::Player(uuid) => {
                remote.profile == uuid || remote.level_players.values().any(|p| *p == uuid)
            }
            _ => false,
        })
    }

    pub(super) fn spawn_entity(
        &mut self,
        id: i32,
        kind: &str,
        position: [f64; 3],
        rotation: [f32; 3],
    ) {
        self.detach_remote_tracking_entity(id);
        let [yaw, pitch, head] = rotation;
        let at = Vec3::new(position[0], position[1], position[2]);
        let mob_type = if kind == "minecraft:player" {
            self.remote.as_ref().and_then(|r| r.player_type.clone())
        } else {
            self.res.mob_types().get(kind).cloned()
        };
        if kind == "minecraft:ender_dragon" {
            self.spawn_remote_dragon(id, position, yaw);
        }
        if kind == "minecraft:experience_orb" {
            // Its value (ExperienceOrb.DATA_VALUE) follows in SetEntityData.
            self.orbs.retain(|o| o.id != id as u32);
            self.orbs
                .push(ExperienceOrb::new(id as u32, at, Vec3::ZERO, 1));
        }
        if let Some(t) = mob_type {
            // The window's copy of the server's mob: no mod part goals —
            // their `register_goals` ran on the authoritative side (#1077).
            let mut mob = Mob::new_remote(id as u32, t, at, head);
            super::remote_mob_data::client_defaults(&mut mob);
            (mob.yaw, mob.previous_yaw) = (yaw, yaw);
            (mob.head_pitch, mob.previous_head_pitch) = (pitch, pitch);
            mob.persistent = true;
            self.mobs.retain(|m| m.id != id as u32);
            self.mobs.push(mob);
        }
        if let Some(r) = self.remote.as_mut() {
            // A new entity has a new interpolation handler (a mob's or an
            // armor stand's), even under an id the server used before.
            r.stepped.remove(&id);
            if kind == "minecraft:item" {
                r.pending_items.insert(id, (position, Vec3::ZERO));
            }
            r.entities.insert(
                id,
                Tracked {
                    kind: kind.to_owned(),
                    position,
                    codec_base: position,
                    on_fire: false,
                    mod_fire_immune: false,
                    silent: false,
                    vehicle: None,
                },
            );
        }
    }

    // move_entity: remote/motion.rs
    pub(super) fn detach_remote_tracking_entity(&mut self, id: i32) {
        let mobs = &self.mobs;
        let index = &mut self.mob_index;
        self.particles
            .detach_tracking_target(TrackingTarget::Entity(id), || {
                index.get(mobs, id as u32).map(mob_tracking_body)
            });
    }

    /// Map a menu slot to Sunrise's inventory screen slot ids.
    pub(super) fn inventory_screen_slot(menu: &str, slot: i32) -> Option<usize> {
        Self::inventory_screen_slot_with_type(menu, slot, || {
            sunrise_client::containers::mod_menu_type(menu)
        })
    }

    /// Map a slot when the caller already knows the window's registered layout.
    pub(super) fn inventory_screen_slot_with_type(
        menu: &str,
        slot: i32,
        mod_type: impl FnOnce() -> Option<&'static sunrise_client::containers::ModMenuType>,
    ) -> Option<usize> {
        let s = usize::try_from(slot).ok()?;
        // A container of `n` slots, then the inventory and the hotbar.
        let slots = |n: usize| -> Option<usize> {
            match s {
                _ if s < n => Some(300 + s),
                _ if s < n + 27 => Some(9 + s - n),
                _ if s < n + 36 => Some(s - n - 27),
                _ => None,
            }
        };
        let rows = |n: usize| slots(n * 9);
        match menu {
            "inventory" => match s {
                0 => Some(200),
                1..=4 => Some(100 + s - 1),
                5..=8 => Some(500 + s - 5),
                9..=35 => Some(s),
                36..=44 => Some(s - 36),
                // InventoryMenu.SHIELD_SLOT: the off hand.
                45 => Some(504),
                _ => None,
            },
            "minecraft:crafting" => match s {
                0 => Some(200),
                1..=9 => Some(100 + s - 1),
                10..=36 => Some(9 + s - 10),
                37..=45 => Some(s - 37),
                _ => None,
            },
            "minecraft:merchant" => match s {
                0 | 1 => Some(super::stations::INPUT + s),
                2 => Some(super::stations::RESULT),
                3..=29 => Some(9 + s - 3),
                30..=38 => Some(s - 30),
                _ => None,
            },
            "minecraft:furnace" | "minecraft:blast_furnace" | "minecraft:smoker" => match s {
                0..=2 => Some(300 + s),
                3..=29 => Some(9 + s - 3),
                30..=38 => Some(s - 30),
                _ => None,
            },
            "minecraft:generic_9x1" => rows(1),
            "minecraft:generic_9x2" => rows(2),
            "minecraft:generic_9x3" | "minecraft:shulker_box" => rows(3),
            "minecraft:generic_9x4" => rows(4),
            "minecraft:generic_9x5" => rows(5),
            "minecraft:generic_9x6" => rows(6),
            // HopperMenu; DispenserMenu (dispensers and droppers).
            "minecraft:hopper" => slots(5),
            "minecraft:generic_3x3" => slots(9),
            "minecraft:brewing_stand" => slots(5),
            // CrafterMenu: the grid, the inventory, then the result (the
            // client's own).
            "minecraft:crafter_3x3" => slots(9),
            // HorseInventoryMenu: the saddle, the body armour, the chest,
            // then the inventory.
            m if m.starts_with(MOUNT_MENU) => {
                let n = m[MOUNT_MENU.len()..].parse::<usize>().ok()? * 3;
                match s {
                    0 => Some(super::mount_menu::SADDLE),
                    1 => Some(super::mount_menu::ARMOR),
                    _ if s < 2 + n => Some(super::mount_menu::CHEST + s - 2),
                    _ if s < 2 + n + 27 => Some(9 + s - 2 - n),
                    _ if s < 2 + n + 36 => Some(s - 2 - n - 27),
                    _ => None,
                }
            }
            // A content mod's menu type: its container, then the inventory.
            // A workstation: its inputs, its result, then the inventory.
            m => {
                if let Some(t) = mod_type() {
                    return if t.player_inventory {
                        slots(t.slots)
                    } else {
                        (s < t.slots).then_some(300 + s)
                    };
                }
                let kind = Self::remote_station_kind(m)?;
                let inputs = kind.inputs();
                let own = inputs + usize::from(kind.has_result());
                match s {
                    _ if s < inputs => Some(super::stations::INPUT + s),
                    _ if s < own => Some(super::stations::RESULT),
                    _ if s < own + 27 => Some(9 + s - own),
                    _ if s < own + 36 => Some(s - own - 27),
                    _ => None,
                }
            }
        }
    }

    /// Find a game slot in the menu's complete, contiguous slot mapping.
    pub(super) fn menu_slot_index(menu: &str, internal: usize) -> Option<usize> {
        (0..)
            .map_while(|slot| Self::inventory_screen_slot(menu, slot))
            .position(|slot| slot == internal)
    }

    /// The workstation screen of a menu type a server opens.
    pub(super) fn remote_station_kind(menu: &str) -> Option<super::stations::Kind> {
        use super::stations::Kind;
        Some(match menu {
            "minecraft:loom" => Kind::Loom,
            "minecraft:stonecutter" => Kind::Stonecutter,
            "minecraft:grindstone" => Kind::Grindstone,
            "minecraft:anvil" => Kind::Anvil,
            "minecraft:smithing" => Kind::Smithing,
            "minecraft:enchantment" => Kind::Enchanting,
            "minecraft:beacon" => Kind::Beacon,
            "minecraft:cartography_table" => Kind::Cartography,
            _ => return None,
        })
    }
    fn open_menu_name(&self, container: i32) -> String {
        match self.remote.as_ref().and_then(|r| r.menu.as_ref()) {
            Some((id, name)) if *id == container && container != 0 => name.clone(),
            _ => "inventory".into(),
        }
    }
    fn set_menu_slot(&mut self, container: i32, slot: i32, item: Option<ItemStack>) {
        let menu = self.open_menu_name(container);
        if menu == "minecraft:lectern" {
            if slot == 0 {
                self.remote_lectern_book(item);
            }
            return;
        }
        if container != 0 && menu == "inventory" {
            return;
        }
        let Some(internal) = Self::inventory_screen_slot(&menu, slot) else {
            return;
        };
        if internal == 200 {
            // ClientboundContainerSetSlotPacket for the open menu's slot 0: the
            // server's result of its own recipes (a world's data packs' too).
            if container == self.remote_menu().0
                && let Some(r) = self.remote.as_mut()
            {
                r.craft_result = item;
            }
            return;
        }
        let slot = self.slot_mut(internal);
        let changed = !super::hand::matches_for_swap(slot.as_ref(), item.as_ref());
        *slot = item;
        if internal < 36 {
            self.remote_hand_slot_set(internal, changed);
        } else if internal == 504 {
            self.remote_hand_slot_set(super::shield::OFFHAND_SLOT, changed);
        }
        if internal >= super::stations::INPUT && self.station.is_some() {
            self.remote_station_slot_changed(internal);
        }
    }

    pub(in crate::game) fn apply_packet(&mut self, packet: C) -> Result<()> {
        self.observe_client_packet(&packet);
        match packet {
            // ClientCommonPacketListenerImpl.handlePing runs on the game
            // thread, after all preceding packets (especially teleports).
            C::Ping(p) => self.send(S::Pong(p)),
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
            // The synced attachments of a holder the server shows the window.
            C::CustomPayload(p) if p.channel == sunrise_protocol::attachment_sync::CHANNEL => {
                self.remote_attachment_sync(&p.data);
            }
            // A native mod's general row the server starts showing the window.
            C::CustomPayload(p) if p.channel == sunrise_protocol::general_entity::CHANNEL => {
                self.remote_general_entity(&p.data);
            }
            // The collider boxes of an entity the server shows the window.
            C::CustomPayload(p) if p.channel == sunrise_protocol::collider_sync::CHANNEL => {
                self.remote_collider_sync(&p.data);
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
                self.receive_menu_extra(&p.data);
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
                let delay = self.debug_millis() - p.time;
                self.debug_charts.ping.log(delay);
            }
            C::Login(p) => self.apply_login_packet(p),
            C::Respawn(p) => self.apply_respawn_packet(p),
            C::PlayerPosition(p) => self.remote_teleport(&p)?,
            C::PlayerRotation(p) => self.remote_rotation(&p),
            C::PlayerLookAt(p) => self.remote_look_at(&p),
            // ClientCommonPacketListenerImpl.handlePostEffects.
            C::PostEffects(p) => self.post_effects = p.effects,
            C::SetCamera(p) => self.remote_set_camera(p.camera_id),
            C::LevelChunkWithLight(p) => self.remote_chunk(&p, None),
            C::LightUpdate(p) => self.remote_light_update(&p),
            C::ChunksBiomes(p) => self.remote_chunks_biomes(&p),
            C::ForgetLevelChunk(p) => self.apply_forget_level_chunk_packet(p),
            C::BlockUpdate(p) => self.server_block(p.pos, p.state),
            C::AddTransientBlock(p) => self.add_transient_block(p.pos, p.state),
            C::BlockDestruction(p) => self.apply_block_destruction_packet(p),
            C::SectionBlocksUpdate(p) => self.apply_section_blocks_update_packet(p),
            C::BlockChangedAck(p) => self.end_predictions(p.sequence),
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
            C::SetEntityData(p) => self.apply_set_entity_data_packet(p),
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
            C::MountScreenOpen(p) => self.remote_open_mount(p),
            C::UpdateRecipes(p) => self.remote_recipe_properties(p),

            C::RecipeBookAdd(p) => self.remote_recipe_book_add(p),
            C::RecipeBookRemove(p) => self.remote_recipe_book_remove(p),
            C::RecipeBookSettings(p) => self.remote_recipe_book_settings(p),
            C::PlaceGhostRecipe(p) => self.remote_ghost_recipe(p),
            C::MerchantOffers(p) => self.remote_merchant_offers(p),
            C::ContainerClose(_) => {
                // `LocalPlayer.clientSideCloseContainer` sets the screen to
                // null, including dialogs and WaitingForResponseScreen. This
                // differs from clearDialog, which restores the parent screen.
                let dialog = self.dialog_open();
                if dialog {
                    self.close_dialogs();
                    self.ui.chat_screen = None;
                    self.book = None;
                    self.sign_edit = None;
                }
                if !self.remote_lectern_closed() && (self.screen == Screen::Inventory || dialog) {
                    self.remote_close_local();
                    self.set_screen(Screen::Playing);
                    self.capture(true);
                }
            }
            C::TagQuery(p) => self.answer_tag_query(p.transaction, p.tag),
            C::Unknown(id, data)
                if id == sunrise_protocol::ids::play::clientbound::COMMAND_SUGGESTIONS =>
            {
                self.handle_command_suggestions(&data);
            }
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
            C::SetChunkCacheRadius(p) => {
                self.streaming.radius = self
                    .settings
                    .render_distance
                    .min(p.distance.saturating_add(1))
                    .max(2);
                if let Some(r) = self.remote.as_mut() {
                    r.view_distance = p.distance;
                    let center = r.chunk_center;
                    // ClientChunkCache.updateViewRadius: a new storage of that size.
                    self.drop_remote_chunks_out_of_range(center, p.distance);
                }
            }
            C::SetChunkCacheCenter(p) => {
                if let Some(r) = self.remote.as_mut() {
                    r.chunk_center = (p.x, p.z);
                    let distance = r.view_distance;
                    self.drop_remote_chunks_out_of_range((p.x, p.z), distance);
                }
            }
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
            C::SetActionBarText(p) => self.titles.set_overlay(remote_text(&p.text), false),
            C::SetTitleText(p) => self.titles.set_title(remote_text(&p.text)),
            C::SetSubtitleText(p) => self.titles.set_subtitle(remote_text(&p.text)),
            C::SetTitlesAnimation(p) => self.titles.set_times(p.fade_in, p.stay, p.fade_out),
            // ClientCommonPacketListenerImpl.handleShowDialog / handleClearDialog.
            C::ShowDialog(p) => self.show_play_dialog(&p.dialog),
            C::ClearDialog(_) => self.clear_dialog(),
            // ClientPacketListener.handleLowDiskSpaceWarning.
            C::LowDiskSpaceWarning(_) => sunrise_client::chunk_io::report_low_disk_space(),
            C::ClearTitles(p) => self.apply_clear_titles_packet(p),
            _ => {}
        }
        Ok(())
    }

    // remote_objective: remote/sync.rs

    /// The player list's view of the connection: the listed players (with
    /// their downloaded skins once ready), online mode, and which players
    /// are in the level with an upside-down name.
    pub(super) fn publish_tab_object_skins(&mut self) {
        let Some(r) = self.remote.as_mut() else {
            return;
        };
        let fingerprint = (
            r.listed.len(),
            r.listed.iter().fold(0, |xor, uuid| xor ^ uuid),
            r.skins_revision,
            self.gui.atlas.object_skins_revision,
        );
        if r.published_skins == Some(fingerprint) {
            return;
        }
        for uuid in &r.listed {
            let Some(p) = r.tab_players.get(uuid) else {
                continue;
            };
            let skin = r.skins.get(uuid).map_or_else(
                || sunrise_client::tab_list::default_skin(*uuid),
                String::as_str,
            );
            for key in [
                format!("id:{uuid:032x}"),
                format!("name:{}", p.name.to_ascii_lowercase()),
            ] {
                if object_skin_changed(&self.gui.atlas.object_skins, &key, skin) {
                    self.gui.atlas.set_object_skin(key, skin.to_owned());
                }
            }
        }
        // Publication itself mutates the atlas; cache its final revision.
        r.published_skins = Some((
            fingerprint.0,
            fingerprint.1,
            fingerprint.2,
            self.gui.atlas.object_skins_revision,
        ));
    }

    // remote_tab_players: remote/sync.rs

    /// Resolve inline `object:player` profiles through the same skin cache as
    /// player heads on items. The next frame uses the uploaded atlas texture.
    pub(super) fn resolve_inline_profiles(
        &mut self,
        profiles: &[std::sync::Arc<serde_json::Value>],
    ) {
        use sunrise_client::skins;
        for profile in profiles {
            let skin = skins::head_skin(
                profile,
                &mut self.skin_profiles.borrow_mut(),
                &mut self.skin_downloads.borrow_mut(),
                |location| self.gui.atlas.sprites.contains_key(location),
            );
            let id = skins::parse_profile(profile).id;
            let name = profile
                .as_str()
                .or_else(|| profile.get("name").and_then(serde_json::Value::as_str));
            // The player list's skin for the same profile is drawn too.
            let keys = [
                Some(format!("id:{id:032x}")),
                name.map(|n| format!("name:{}", n.to_ascii_lowercase())),
            ];
            for key in keys.iter().flatten() {
                if let Some(listed) = self.gui.atlas.object_skins.get(key)
                    && listed.starts_with("skins/")
                {
                    let listed = listed.clone();
                    self.again_skin(&listed);
                }
            }
            self.gui
                .atlas
                .set_object_skin(format!("id:{id:032x}"), skin.clone());
            if let Some(name) = name {
                self.gui
                    .atlas
                    .set_object_skin(format!("name:{}", name.to_ascii_lowercase()), skin);
            }
        }
    }
    fn poll_own_profile(&mut self) {
        let auth = self.auth();
        let Some(r) = self.remote.as_mut() else {
            return;
        };
        match &mut r.own_profile {
            OwnProfile::Idle => {
                let Some(auth) = auth else {
                    r.own_profile = OwnProfile::Done(None);
                    return;
                };
                let (send, recv) = std::sync::mpsc::channel();
                std::thread::Builder::new()
                    .name("Own profile".into())
                    .spawn(move || {
                        let _ = send.send(sunrise_client::auth::own_textures(auth.as_ref()));
                    })
                    .ok();
                r.own_profile = OwnProfile::Fetching(recv);
            }
            OwnProfile::Fetching(recv) => {
                if let Ok(own) = recv.try_recv() {
                    r.own_profile = OwnProfile::Done(own);
                }
            }
            OwnProfile::Done(_) => {}
        }
    }

    // update_remote_skins: remote/status.rs

    /// Puts a downloaded skin in an atlas skin slot (`SkinSlots::place`),
    /// unless every slot holds a skin drawn in the last frame.
    pub(super) fn store_skin(&mut self, location: String, rgba: Vec<u8>) {
        let Some((slot, evicted)) = self.skin_slots.place(location.clone()) else {
            return;
        };
        if let Some(old) = evicted {
            self.gui.atlas.sprites.remove(&old);
        }
        let Some(mut rect) = self
            .gui
            .atlas
            .sprites
            .get(&format!("sunrise/skin{slot}"))
            .copied()
        else {
            return;
        };
        let (w, h) = (self.gui.atlas.width as f32, self.gui.atlas.height as f32);
        self.map_patches.push(sunrise_client::font::Patch {
            x: (rect.u0 * w).round() as u32,
            y: (rect.v0 * h).round() as u32,
            width: 64,
            height: 64,
            rgba,
        });
        // A cape is a 64x32 texture in the top half of the slot (skins::process_cape).
        if location.starts_with("capes/") {
            rect.v1 = (rect.v0 + rect.v1) / 2.0;
        }
        self.gui.atlas.sprites.insert(location, rect);
    }

    /// Whether a downloaded skin or cape is in the atlas to draw, marking it
    /// drawn; if not (new, or its slot taken), its image is queued for upload
    /// again and it is drawn from the next frame.
    fn again_skin(&mut self, location: &str) -> bool {
        let mut downloads = self.skin_downloads.borrow_mut();
        // A slot without its sprite (the atlas was rebuilt, or the upload
        // never happened) counts as missing, so the skin is stored again.
        if self.skin_slots.contains(location) && self.gui.atlas.sprites.contains_key(location) {
            downloads.drawn(location);
            true
        } else {
            downloads.again(location);
            false
        }
    }

    /// A player's skin and model as drawn: the downloaded skin once it is in
    /// the atlas, until then the profile's default (`DefaultPlayerSkin.get`),
    /// as `PlayerInfo.getSkin` gives before the texture has loaded.
    pub(super) fn drawn_skin(
        &mut self,
        uuid: u128,
        (skin, slim): (String, bool),
    ) -> (String, bool) {
        if (skin.starts_with("skins/") || skin == "sunrise/offline_skin") && !self.again_skin(&skin)
        {
            let skin = sunrise_client::tab_list::default_skin(uuid);
            return (skin.to_owned(), skin.contains("/slim/"));
        }
        (skin, slim)
    }

    /// `drawn_skin` for a face in the player list or spectator menu.
    pub(super) fn face_skin(&mut self, uuid: u128, skin: &str) -> String {
        self.drawn_skin(uuid, (skin.to_owned(), false)).0
    }

    /// ChatType.Bound.decorate (`Decoration::Chat`) and `decorateNarration`
    /// (`Decoration::Narration`): the chat type's decoration from the
    /// server's registry, applied to `content`.
    pub(super) fn decorate_chat(
        &self,
        bound: &play::ChatTypeBound,
        content: &serde_json::Value,
        decoration: Decoration,
    ) -> sunrise_client::text::Text {
        use sunrise_client::scoreboard::comp_text;
        let Some(r) = self.remote.as_ref() else {
            return comp_text(content);
        };
        let chat_type = r.registries.data("minecraft:chat_type", bound.chat_type);
        comp_text(&chat_type_decorate(chat_type, decoration, bound, content))
    }
    fn remote_item_data(&mut self, id: i32, stack: &Option<Stack>) {
        let Some(stack) = self.to_stack(stack) else {
            return;
        };
        let position = self
            .remote
            .as_mut()
            .and_then(|r| r.pending_items.remove(&id));
        if let Some(item) = self.dropped.iter_mut().find(|d| d.id == id as u32) {
            item.stack = stack;
            return;
        }
        let Some((p, velocity)) = position else {
            return;
        };
        let at = Vec3::new(p[0], p[1], p[2]);
        // ItemEntity's constructor draws bobOffs from the item's own random.
        let mut random = sunrise_client::mobs::entity_random(id as u32, at);
        let bob = super::item_entities::bob_offset(at, random.next_float(), self.clock.still());
        self.dropped.push(ItemEntity {
            random,
            ..ItemEntity::new(id as u32, stack, at, velocity, bob)
        });
    }

    /// `Entity.lerpMotion` (and `recreateFromPacket`'s motion) for the
    /// entities whose own tick moves them on a client: dropped items, experience
    /// orbs, falling blocks and primed TNT.
    fn remote_motion(&mut self, id: i32, velocity: [f64; 3]) {
        self.remote_dragon_motion(id, velocity);
        let v = Vec3::new(velocity[0], velocity[1], velocity[2]);
        // `falling` and `tnts` are searched from the back: AddEntity calls this right after
        // appending the block or TNT, so a spawn doesn't walk the whole list.
        if let Some(m) = self.mob_index.get_mut(&mut self.mobs, id as u32) {
            // Squid.aiStep derives its body pitch and roll from network motion.
            m.velocity = v;
        } else if let Some(d) = self.dropped.iter_mut().find(|d| d.id == id as u32) {
            d.velocity = v;
        } else if let Some(i) = self.falling.iter().rposition(|f| f.id == id as u32) {
            self.falling.update(i, |f| f.velocity = v);
        } else if let Some(t) = self.tnts.iter_mut().rev().find(|t| t.id == id as u32) {
            t.velocity = v;
        } else if let Some(o) = self.orbs.iter_mut().find(|o| o.id == id as u32) {
            o.velocity = v;
        } else if let Some(i) = self.carts.iter().position(|c| c.id == id as u32) {
            self.carts.update(i, |c| c.velocity = v);
        } else if let Some(i) = self.boats.iter().position(|b| b.id == id as u32) {
            // Entity.lerpMotion: the boat the player drives floats on from the server's motion
            // (`tick_remote_boat_bubbles` drops it again on a boat nobody here drives).
            self.boats.update(i, |b| b.velocity = v);
        } else if let Some(i) = self
            .creatures
            .projectiles
            .iter()
            .position(|p| p.id == id as u32)
        {
            self.creatures.projectiles.update(i, |p| {
                p.velocity = v;
                // AbstractArrow.lerpMotion: the stuck timer restarts, and an
                // arrow the server set moving leaves the block it was stuck in.
                if let Some(st) = p.arrow.as_mut() {
                    st.life = 0;
                    if st.in_ground && v.length_squared() > 0.0 {
                        st.in_ground = false;
                        p.stuck = false;
                    }
                }
            });
        } else if let Some(t) = self.tridents.iter_mut().find(|t| t.id == id as u32) {
            t.velocity = v;
            // AbstractArrow.lerpMotion (ThrownTrident keeps it): `life`
            // restarts, and a trident set moving leaves its block.
            t.in_ground_time = 0;
            if t.in_ground && v.length_squared() > 0.0 {
                t.in_ground = false;
            }
        } else if let Some(r) = self.rockets.iter_mut().find(|r| r.id == id as u32) {
            r.velocity = v;
        } else if self.remote_hook_motion(id as u32, v) {
        } else if let Some(p) = self
            .remote
            .as_mut()
            .and_then(|r| r.pending_items.get_mut(&id))
        {
            p.1 = v;
        } else if !self.potions.remote_thrown_motion(id as u32, v) {
            self.end.remote_eye_motion(id as u32, velocity);
        }
    }
    fn apply_remote_health(&mut self) {
        let Some((health, food, saturation)) = self.remote.as_ref().map(|r| r.health) else {
            return;
        };
        if let Some(p) = self.player.as_mut() {
            if (p.health.health() - health).abs() > f32::EPSILON {
                let _ = p.set_health(health.max(0.0), false);
            }
            p.health.food.food = food;
            p.health.food.saturation = saturation;
        }
    }

    /// `ServerPlayer.dismountTo` lands a rider at the ghast's own (x, maxY, z)
    /// (`HappyGhast.getDismountLocationForPassenger`). A vanilla client
    /// simulates the ghast it rides, so its copy is where the server has it;
    /// this window's copy trails the server's by the stepped interpolation and
    /// would rise into the rider's feet, which lets the rider fall through.
    /// Put the copy under the landing spot.
    fn place_ghast_under(&mut self, id: u32, spot: Vec3) {
        let Some(m) = self
            .mobs
            .iter_mut()
            .find(|m| m.id == id && m.species() == sunrise_client::mobs::Species::HappyGhast)
        else {
            return;
        };
        let at = Vec3::new(spot.x, spot.y - f64::from(m.height()), spot.z);
        // Only a teleport that is that dismount: right above the ghast.
        if (at.x - m.position.x).powi(2) + (at.z - m.position.z).powi(2) > 25.0
            || (at.y - m.position.y).abs() > 5.0
        {
            return;
        }
        m.position = at;
        m.previous = at;
        if let Some(handler) = self
            .remote
            .as_mut()
            .and_then(|r| r.stepped.get_mut(&(id as i32)))
        {
            handler.cancel();
        }
    }

    /// `ClientPacketListener.handleLookAt`: `Entity.lookAt` from this
    /// player's feet or eyes, towards the packet's entity (its feet or eyes)
    /// when this client tracks it, else towards the packet's point.
    fn remote_look_at(&mut self, p: &play::PlayerLookAt) {
        // `EntityAnchorArgument.Anchor`: 0 feet, 1 eyes.
        const EYES: i32 = 1;
        let Some(player) = self.player.as_ref() else {
            return;
        };
        let mut from = [player.position.x, player.position.y, player.position.z];
        if p.from == EYES {
            from[1] += f64::from(player.dimensions.eye_height);
        }
        let to = p
            .entity
            .as_ref()
            .and_then(|e| {
                let m = self.mobs.iter().find(|m| m.id == e.id as u32)?;
                let y = if e.anchor == EYES {
                    m.eye_y()
                } else {
                    m.position.y
                };
                Some([m.position.x, y, m.position.z])
            })
            .unwrap_or(p.target);
        (self.yaw, self.pitch) = look_at_rotation(from, to);
    }

    // remote_teleport: remote/sync.rs

    /// `ClientPacketListener.tick`'s `LevelLoadTracker.tickClientLoad`: the
    /// player's section is shown once the renderer has its mesh and it is 30%
    /// faded in (without a renderer: once its column and the 8 around it are
    /// lit, then the same fade); when the level is
    /// ready the loading screen closes and the server hears `PlayerLoaded`
    /// (`notifyPlayerLoaded`).
    fn tick_client_load(&mut self) {
        let Some(player) = self.player.as_ref() else {
            return;
        };
        if !self
            .remote
            .as_ref()
            .is_some_and(|r| r.level_load.is_active())
        {
            return;
        }
        let camera_y = (player.position.y + f64::from(self.eye)).floor() as i32;
        let camera = [
            player.position.x.floor() as i32,
            camera_y,
            player.position.z.floor() as i32,
        ];
        // `Util.toMillis(chunkSectionFadeInTime)`.
        let fade_ms = sunrise_client::gfx::fade_millis(self.settings.chunk_fade as f32);
        // `LevelRenderer`'s playerCompiledSectionCallback, run once
        // `isSectionCompiledAndVisible(camera)`: the renderer's own mesh and fade
        // for the camera's section. Without a renderer, the section counts as
        // compiled once `LevelExtractor` would compile it
        // (`SectionUpdateTracker.hasAllNeighbors`: its column and the 8 around
        // it lit, as meshing waits for here).
        let shown = self
            .renderer
            .as_ref()
            .map(|renderer| renderer.section_compiled_and_visible(camera, fade_ms));
        let (cx, cz) = (camera[0] >> 4, camera[2] >> 4);
        let neighbours_lit = shown.is_none()
            && (-1..=1).all(|dx| (-1..=1).all(|dz| self.streaming.is_lit(cx + dx, cz + dz)));
        let shape = self.world.shape;
        let check = sunrise_client::level_loading::PlayerCheck {
            outside_build_height: camera_y < shape.min_y || camera_y >= shape.min_y + shape.height,
            spectator: player.spectator,
            alive: player.health.health() > 0.0,
        };
        let now = client_millis();
        // LevelLoadingScreen.tick.
        self.tick_load_progress();
        let Some(r) = self.remote.as_mut() else {
            return;
        };
        if r.level_load.wants_player_section() {
            match shown {
                Some(true) => r.level_load.player_section_ready(),
                Some(false) => {}
                None => r.level_load.player_section_compiled(
                    [cx, camera_y >> 4, cz],
                    neighbours_lit,
                    now,
                    fade_ms,
                ),
            }
        }
        r.level_load.tick_client_load(now, check);
        if !r.level_load.is_level_ready(now) {
            return;
        }
        r.level_load.finish();
        if !r.loaded_sent {
            r.loaded_sent = true;
            self.send(S::PlayerLoaded(Empty));
        }
        // LevelLoadingScreen.onClose.
        self.say_system_now(&sunrise_client::text::Text::translate(
            "narrator.ready_to_play",
            &[],
        ));
        self.capture_loaded_world();
    }

    // tick_remote..tick_remote_player,tick_remote_block_entities: remote/tick.rs

    /// `Level.tickBlockEntities` on a client: the block entities' animations
    /// and client ticks, and the moving pistons' pushes.
    fn tick_remote_block_entities(&mut self) {
        // The ticker list's spawners, trial spawners, vaults and mod block entities; the
        // animated ones, beacons and potent sulfur still tick on their own passes after them.
        self.tick_client_block_entities();
        self.tick_block_entity_anims();
        self.client_tick_beacons();
        self.tick_remote_potent_sulfur();
        self.with_level(|b, level, ticks| {
            b.client_tick_block_entities(level, ticks);
        });
        let pushes = std::mem::take(&mut self.block_ticks.piston_pushes);
        self.push_piston_entities(pushes);
        self.sync_moving_pistons();
    }

    /// The local player's `LocalPlayer.tick`, once the client has loaded. The
    /// input `sendChanges` sends after the block entities ticked, if it runs.
    fn tick_remote_own_player(&mut self) -> Result<Option<u8>> {
        let entity_collisions = self.entity_collisions(Mover::Player);
        // Those added before it tick before it: their pushes reach this move.
        let pushed = self.remote_pushes(true);
        let sprint_window = self.settings.sprint_window;
        if self.player.is_none() {
            return Ok(None);
        }
        // LocalPlayer.tick and sendChanges run once the client has loaded (hasClientLoaded),
        // wherever the player is: a chunk that hasn't come yet has no blocks here, as vanilla's
        // empty chunk has none, so the player falls through it until the chunk comes.
        if !self.remote.as_ref().is_some_and(|r| r.loaded_sent) {
            return Ok(None);
        }
        // LocalPlayer.tick reaches Player.tick only once the client has loaded; its
        // endpoints bracket that work alone, also on the death screen, not the level
        // pass before it.
        let player = (mod_api::__private::listened(mod_api::HookId::PlayerTickPre)
            || mod_api::__private::listened(mod_api::HookId::PlayerTickPost))
        .then(|| self.local_player())
        .flatten();
        // The entities that tick before the player pushed it before its tick began, so
        // before Pre.
        self.apply_remote_push(pushed);
        if let Some(player) = player {
            self.native_player_tick_pre(mod_api::__private::PlayerTickPreCall { player });
        }
        self.native_local_player_fit_sizes();
        let result = self.tick_local_player(entity_collisions, sprint_window);
        // `EntityEvent.Size` for the pose the tick set.
        self.native_local_player_size();
        if let Some(player) = player {
            self.native_player_tick_post(mod_api::__private::PlayerTickPostCall { player });
        }
        // The entities added after the player tick after it, so after Post: `Entity.push`
        // adds each to the player's deltaMovement during their tick, from where they and
        // the player ended it. A velocity or teleport the server sends before the next
        // tick replaces that deltaMovement, push included
        // (`ClientPacketListener.handleSetEntityMotion`, `handleMovePlayer`), so the push
        // lands on the velocity now, not on the next move.
        let after = self.remote_pushes(false);
        self.apply_remote_push(after);
        result
    }

    /// `Entity.push`'s share of the other entities' ticks: added to the local
    /// player's deltaMovement, unless it lies dead, sleeps or rides.
    fn apply_remote_push(&mut self, pushed: Vec3) {
        let still = self.screen == Screen::Dead || self.sleeping.is_some() || self.riding.is_some();
        if let Some(player) = self.player.as_mut()
            && !still
        {
            player.velocity = player.velocity.add_vector(pushed);
        }
    }

    /// The last `ClientboundPlayerAbilities` the server sent, on a client.
    pub(super) fn remote_abilities(&self) -> Option<&play::PlayerAbilities> {
        self.remote.as_ref()?.abilities.as_ref()
    }

    /// The local player's share of every touching entity's
    /// `LivingEntity.pushEntities`, and of the boats' pushes: on a client
    /// `EntitySelector.pushableBy` lets entities push only the local player,
    /// so the server never sends these pushes. `doPush(player)` is
    /// `player.push(entity)`. `before_player`: the mobs and boats that tick
    /// before the player (`ticked_before_player`), else those that tick after
    /// it.
    fn remote_pushes(&self, before_player: bool) -> Vec3 {
        let (Some(p), Some(r)) = (self.player.as_ref(), self.remote.as_ref()) else {
            return Vec3::ZERO;
        };
        // LivingEntity.isPushable: alive, no spectator, off ladders.
        let feet = [
            p.position.x.floor() as i32,
            p.position.y.floor() as i32,
            p.position.z.floor() as i32,
        ];
        if self.screen == Screen::Dead
            || p.spectator
            || self.named_block_in_tag("minecraft:climbable", &self.block_name(feet))
        {
            return Vec3::ZERO;
        }
        let bounds = p.bounds();
        let teams = self.mob_teams();
        // `Entity.getTeam` is `Scoreboard.getPlayersTeam(getScoreboardName())`:
        // a player's score holder is its profile name, every other entity's
        // its UUID text. `mob_teams` keys by UUID and names the local player
        // `PLAYER_NAME`, which misses a server's teams for players.
        let board = &self.score.board;
        let player_team = board.players_team(&r.profile_name);
        let mut push = Vec3::ZERO;
        let order = |id: u32| r.ticked_before_player.contains(&id) == before_player;
        for m in self.mobs.iter().filter(|m| order(m.id)) {
            // Bats never push, nor passengers of one vehicle, nor a spectator; the
            // box test comes before the lookups that far mobs never need (#14053).
            if !m.alive()
                || m.species() == sunrise_client::mobs::Species::Bat
                || self
                    .riding
                    .is_some_and(|v| v == m.id || m.extra.mount == Some(v))
                || !m.bounds().intersects(bounds)
            {
                continue;
            }
            let listed = r
                .level_players
                .get(&(m.id as i32))
                .and_then(|uuid| r.tab_players.get(uuid));
            let mob_team =
                listed.map_or_else(|| teams.of(Who::Mob(m.id)), |t| board.players_team(&t.name));
            if listed.is_some_and(|t| t.game_mode == SPECTATOR)
                || !sunrise_client::scoreboard::pushable_by(mob_team, player_team, true)
            {
                continue;
            }
            // Entity.push(Entity), `this` the player.
            let (mut xa, mut za) = (m.position.x - p.position.x, m.position.z - p.position.z);
            let mut dd = xa.abs().max(za.abs());
            if dd < f64::from(0.01_f32) {
                continue;
            }
            dd = dd.sqrt();
            xa /= dd;
            za /= dd;
            let pow = (1.0 / dd).min(1.0);
            xa *= pow;
            za *= pow;
            xa *= f64::from(0.05_f32);
            za *= f64::from(0.05_f32);
            push = Vec3::new(push.x - xa, push.y, push.z - za);
        }
        // AbstractBoat.push(player) from the boat's own tick (its box grown
        // by 0.2 sideways) and from the player's pushEntities (its box).
        let area = Vec3::new(f64::from(0.2_f32), f64::from(-0.01_f32), f64::from(0.2_f32));
        // A boat pushes from its own tick, in the same order as the mobs.
        for b in self
            .boats
            .iter()
            .filter(|b| order(b.id) && self.riding != Some(b.id))
        {
            let bb = b.bounds();
            let times = usize::from(
                bounds.intersects(bb.inflate(area))
                    && sunrise_client::scoreboard::pushable_by(None, player_team, true),
            ) + usize::from(
                bounds.intersects(bb)
                    && sunrise_client::scoreboard::pushable_by(player_team, None, true),
            );
            if times > 0
                && super::boat_passengers::boat_pushes(bb, bounds, false)
                && let Some(v) = super::boat_passengers::push_impulse(b.position, p.position)
            {
                push = push.add_vector(v.scale(times as f64));
            }
        }
        push
    }

    // tick_remote_guardian_eyes: remote/tick.rs

    /// Humanoid held arm poses, once the tick has set equipment and use state.
    pub(in crate::game) fn tick_remote_arm_poses(&mut self) {
        for i in 0..self.mobs.len() {
            let m = &self.mobs[i];
            if !sunrise_client::mobs::anim::extracts_humanoid(m) {
                continue;
            }
            // Extraction also runs while the world is frozen: derive the use
            // animation from the current equipment rather than last tick's state.
            if &*m.kind.id != "minecraft:player" {
                let slot = if m.extra.using_off_hand {
                    sunrise_items::equipment::OFFHAND
                } else {
                    sunrise_items::equipment::MAINHAND
                };
                let animation = (m.extra.using_item > 0)
                    .then(|| m.extra.slot_stack(slot))
                    .flatten()
                    .map_or_else(Default::default, |stack| self.item_use_animation(stack));
                self.mobs[i].extra.use_animation = animation;
            }
            let m = &self.mobs[i];
            let poses = self.held_arm_poses(m.id, m);
            let charge_duration = (&*m.kind.id != "minecraft:player").then(|| {
                let using_slot = if m.extra.using_off_hand {
                    sunrise_items::equipment::OFFHAND
                } else {
                    sunrise_items::equipment::MAINHAND
                };
                m.extra
                    .slot_stack(using_slot)
                    .map_or(25, |stack| Self::charge_ticks(stack).max(1)) as i32
            });
            self.mobs[i].extra.held_arm_pose = poses;
            if let Some(duration) = charge_duration {
                self.mobs[i].extra.ints[5] = duration;
            }
        }
    }
    // tick_remote_entities: remote/tick.rs

    /// `AbstractMinecartRenderer.newExtractState`: sample the same weighted
    /// steps at the frame's partial tick, including corners within one tick.
    pub(super) fn remote_minecart_lerp(
        &self,
        id: u32,
        partial: f32,
    ) -> Option<([f64; 3], f32, f32)> {
        let lerp = self.remote.as_ref()?.cart_lerps.get(&(id as i32))?;
        if lerp.current.is_empty() {
            return None;
        }
        let (p, _, yaw, pitch) = lerp.at(partial);
        Some(([p.x, p.y, p.z], yaw, pitch))
    }

    // remote_player_eating: remote/use.rs

    // remote_send_state: remote/sync.rs
    /// `MultiPlayerGameMode.handleCreativeModeItemAdd` for inventory slot `i` (0 to 35): the
    /// stack goes out now and counts as synced. Nothing without a server.
    pub(in crate::game) fn remote_creative_slot_add(&mut self, i: usize) {
        let stack = self.inventory.get(i).cloned().flatten();
        let Some(r) = self.remote.as_mut() else {
            return;
        };
        if let Some(synced) = r.synced.get_mut(i) {
            *synced = stack.clone();
        }
        let item = self.to_network(&stack);
        self.send(S::SetCreativeModeSlot(play::SetCreativeModeSlot {
            slot: creative_sync_slot(i),
            item,
        }));
    }
    /// Inventory as the server last described it.
    fn remote_synced(&mut self) {
        let stacks = self.creative_stacks();
        if let Some(r) = self.remote.as_mut() {
            r.synced = stacks;
        }
    }

    /// The slots a server packet changed since `before` are as the server has them; the
    /// others keep what was last sent, so a creative click made since still goes out.
    pub(in crate::game) fn remote_synced_since(&mut self, before: &[Option<ItemStack>]) {
        let now = self.creative_stacks();
        if let Some(r) = self.remote.as_mut() {
            mark_synced(&mut r.synced, before, now);
        }
    }

    /// `MultiPlayerGameMode.stopDestroyBlock`: aborts the break and starts the attack strength
    /// again, if one is running (`if (!isDestroying) return;`), and says whether it did. Only the
    /// progress goes: `destroyBlockPos` and `destroyingItem` keep the target, so a dig resumed on
    /// the same block sends no START and the server's clock runs on (#4124).
    pub(super) fn stop_remote_mining(&mut self) -> bool {
        let kept = self.remote.as_mut().and_then(|r| {
            if !std::mem::take(&mut r.is_destroying) {
                return None;
            }
            let (pos, face, _) = r.destroying?;
            r.destroying = Some((pos, face, 0.0));
            r.destroying
        });
        let Some((pos, _, _)) = kept else {
            return false;
        };
        let state = self.world.block(pos[0], pos[1], pos[2]);
        self.tutorial_destroy(state, -1.0);
        self.send(S::PlayerAction(play::PlayerAction {
            action: 2,
            pos,
            direction: 0,
            sequence: 0,
        }));
        self.reset_attack_strength();
        true
    }

    // remote_mining: remote/use.rs

    fn veto_mining_prediction(
        &self,
        pos: [i32; 3],
        face: [i32; 3],
        starting: bool,
        progress: f32,
    ) -> bool {
        if !mod_api::__private::listened(mod_api::HookId::ClientMiningPredict) {
            return false;
        }
        let mut canceled = false;
        self.native_client_mining_predict(
            mod_api::ClientMiningPrediction {
                pos,
                face,
                starting,
                progress: progress.clamp(0.0, 1.0),
            },
            &mut canceled,
        );
        canceled
    }

    /// `Player.blockActionRestricted` for the block at `pos`: a spectator
    /// never breaks, an adventure player only with a main-hand item whose
    /// `can_break` matches the block (`ItemStack.canBreakBlockInAdventureMode`).
    /// A predicate this client could not decode (exact components) only
    /// tells that the server gave one, and the server sends the block back
    /// if it does not match.
    fn remote_break_restricted(&self, pos: [i32; 3]) -> bool {
        use sunrise_protocol::item::{WirePatch, component::CAN_BREAK};
        if self
            .held()
            .and_then(|s| s.data())
            .is_some_and(|d| d.other("minecraft:can_break").is_some())
        {
            return self.block_action_restricted(pos);
        }
        self.spectator()
            || (!self.may_build()
                && !self
                    .held()
                    .and_then(|s| s.wire_patch())
                    .is_some_and(|p| WirePatch::adds(&p, CAN_BREAK)))
    }

    // remote_use_block: remote/use.rs

    /// `Item.useOn` on this client's level for the items whose effects are
    /// the clicker's own (`Level.levelEvent(player, ...)` and
    /// `playSound(player, ...)`: the server sends them to the others only):
    /// honeycomb waxing copper, and an axe, hoe or shovel transforming a
    /// block (stripping, scraping, removing wax, tilling, making a path).
    /// A cushion succeeds where it may be placed and spawns on the server, a
    /// firework rocket wherever it may be used, and the server launches it.
    /// Whether the item's use succeeded, which ends the click.
    /// The caller has selected Item.useOn after cooldown and adventure gates.
    pub(in crate::game) fn predict_item_on_block(
        &mut self,
        stack: &ItemStack,
        hit: &super::Hit,
    ) -> bool {
        // FireworkRocketItem.useOn: SUCCESS unless gliding, when Item.use
        // boosts instead. The phase's cooldown/adventure gates already ran.
        if &*stack.item == "minecraft:firework_rocket" {
            return !self.player.as_ref().is_some_and(|p| p.fall_flying);
        }
        let at = [
            f64::from(hit.pos[0]) + 0.5,
            f64::from(hit.pos[1]) + 0.5,
            f64::from(hit.pos[2]) + 0.5,
        ];
        if &*stack.item == "minecraft:honeycomb" {
            // HoneycombItem.useOn.
            let registry = self.pack.assets.registry;
            let clicked = self.world.block(hit.pos[0], hit.pos[1], hit.pos[2]);
            let old = registry.state(clicked);
            let waxes = old
                .and_then(|old| self.res.transformers().waxed(registry, old))
                .is_some();
            if waxes {
                self.level_event_at(3003, hit.pos, 0);
                self.play_client_local_sound(
                    LevelSound::new("minecraft:item.honeycomb.wax_on", at, 1.0, 1.0)
                        .in_source(mod_api::SoundSource::Blocks),
                );
                // The other half of a double chest: its particles, and a second
                // sound from the clicked block.
                if let Some(other) =
                    old.and_then(|old| super::storage::chest_connected_pos(old, hit.pos))
                {
                    self.level_event_at(3003, other, 0);
                    self.play_client_local_sound(
                        LevelSound::new("minecraft:item.honeycomb.wax_on", at, 1.0, 1.0)
                            .in_source(mod_api::SoundSource::Blocks),
                    );
                }
            }
            waxes
        } else if let Some((_, sound, _, _, particle, _)) = self.block_transform(stack, hit) {
            self.transform_particle(&particle, hit.pos);
            self.play_client_local_sound(
                LevelSound::new(&sound, at, 1.0, 1.0).in_source(mod_api::SoundSource::Blocks),
            );
            true
        } else if self
            .res
            .items()
            .get(&stack.item)
            .is_some_and(|i| i.cushion_color.is_some())
        {
            // CushionItem.useOn on a client level: SUCCESS once
            // `canBePlacedAt` holds, before the server's own checks.
            self.cushion_spawn(hit).is_some()
        } else if &*stack.item == "minecraft:painting" {
            // HangingEntityItem.useOn on a client level: SUCCESS once
            // `Painting.create` finds a surviving variant, before the
            // server spawns it.
            self.painting_fits(hit, stack)
        } else {
            false
        }
    }

    /// `MapItem.useOn` on this client: a map used on a banner is SUCCESS
    /// whatever its data says (the server toggles the marker, or fails
    /// quietly), so this hand swings and the click ends. The caller applies
    /// ItemStack.useOn's adventure predicate before selecting this phase.
    fn predict_map_on_banner(&self, stack: &ItemStack, hit: &super::Hit) -> bool {
        super::maps::map_id(stack).is_some()
            && self.named_block_in_tag("minecraft:banners", &self.block_name(hit.pos))
    }

    /// `BoneMealItem.useOn` with the acting hand's `stack` on this client's
    /// level (a copy: nothing grows here): whether it succeeds.
    fn remote_bone_meal_succeeds(&mut self, hit: &super::Hit, stack: &ItemStack) -> bool {
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

    /// MultiPlayerGameMode.useItemOn inside startPrediction: the blocks the
    /// use changed on this client's level at once, each kept as predicted
    /// under the use's sequence until the server acknowledges it (a use the
    /// server refused goes back). Only the blocks a hand opens or flips: a
    /// wooden or copper door, trapdoor or fence gate, a lever or a button;
    /// whatever else the use changes waits for the server.
    fn predict_use(&mut self, (changes, sounds): &DryUse, seq: i32) {
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

    fn play_predicted_sounds(&mut self, sounds: &[DrySound]) {
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

    /// Another player of this level, whose copy passes every click (`Player.interactOn`).
    pub(super) fn is_level_player(&self, id: i32) -> bool {
        self.remote
            .as_ref()
            .is_some_and(|r| r.level_players.contains_key(&id))
    }

    /// Whether a main-hand right click with `held` would end on the block hit, for the gameplay
    /// guide: what `remote_block_reacts` answers on its dry level, by the templates' rules alone,
    /// so looking asks no mod's placement or survival hook.
    pub(super) fn block_takes_click(&mut self, hit: &super::Hit, held: Option<&ItemStack>) -> bool {
        self.remote_block_reacts(hit, held, false)
            .0
            .consumes_action()
    }

    /// `InteractionResult.CONSUME` for this click: the block keeps it and does nothing, so the
    /// item in hand never gets it either (an empty flower pot, a pressed button). For the
    /// guide's look, as `block_takes_click`.
    pub(super) fn block_swallows_click(
        &mut self,
        hit: &super::Hit,
        held: Option<&ItemStack>,
    ) -> bool {
        matches!(
            self.remote_block_reacts(hit, held, false).0,
            InteractionResult::Consume
        )
    }

    // remote_block_reacts: remote/use.rs

    /// The shared dry-level boundary for a normal block use or the empty
    /// method requested by a canceled `BLOCK` phase's `TryWithEmptyHand`.
    /// The `useItemOn` answers a client level gives without changing a block, for either hand:
    /// - VaultBlock: something in the hand on an ACTIVE vault is SUCCESS_SERVER (no swing here);
    /// - RedStoneOreBlock: SUCCESS, unless a block item goes in the cell beside it
    ///   (`BlockPlaceContext.canPlace`), PASS;
    /// - PumpkinBlock: shears are SUCCESS (only the server carves it);
    /// - WeatheringCopperGolemStatueBlock: an axe on an unaffected statue is SUCCESS (the server
    ///   lets the golem out).
    pub(super) fn client_block_answer(
        &self,
        hit: &super::Hit,
        held: Option<&ItemStack>,
    ) -> Option<InteractionResult> {
        if self.vault_takes_click(hit.pos, held) {
            return Some(InteractionResult::Consume);
        }
        let name = self.block_name(hit.pos);
        match &*name {
            "minecraft:redstone_ore" | "minecraft:deepslate_redstone_ore" => {
                let beside = [0, 1, 2].map(|i| hit.pos[i] + hit.face[i]);
                let takes = self
                    .res
                    .facts()
                    .get(self.world.block(beside[0], beside[1], beside[2]))
                    .replaceable;
                let places =
                    held.is_some_and(|s| self.res.items().block_items.contains_key(&*s.item));
                Some(if places && takes {
                    InteractionResult::Pass
                } else {
                    InteractionResult::Success
                })
            }
            "minecraft:pumpkin" => held
                .is_some_and(|s| &*s.item == "minecraft:shears")
                .then_some(InteractionResult::Success),
            "minecraft:copper_golem_statue" => held
                .is_some_and(|s| self.item_in_tag("minecraft:axes", &s.item))
                .then_some(InteractionResult::Success),
            _ => None,
        }
    }
    // remote_block_template: remote/use.rs

    /// MultiPlayerGameMode.useItemOn inside startPrediction, for a block item:
    /// the blocks `BlockItem.place` set at `pos` are placed on this client's
    /// level at once, so the next click of a fast bridge aims at them rather
    /// than past them (the server would refuse it). Its place sound is the
    /// placer's own (`playSound(player, ...)`: the server sends it to the
    /// others only).
    fn predict_place(&mut self, placed: &DryPlace, stack: &ItemStack, seq: i32) {
        let pos = placed.pos;
        let at = [pos.x, pos.y, pos.z];
        // The dry simulation puts the stack state in the primary write. The
        // live copy takes item components before replaying the other cells.
        let Some((primary, rest)) = placed.changes.split_first() else {
            return;
        };
        self.predict_blocks(std::slice::from_ref(primary), seq);
        if placed.matches_candidate {
            self.apply_predicted_item(at, stack);
        }
        self.predict_blocks(rest, seq);
        let state = primary.1;
        if !self.res.facts().is_air(state) {
            let sound = self.own_sound_type(at, state, mod_api::BlockSoundCause::Place);
            self.play_client_local_sound(
                LevelSound::new(
                    &sound.place,
                    at.map(|n| f64::from(n) + 0.5),
                    (sound.volume + 1.0) / 2.0,
                    sound.pitch * 0.8,
                )
                .in_source(mod_api::SoundSource::Blocks),
            );
        }
    }

    /// The idle controller guide cannot know what these Parts will decide on a click.
    pub(super) fn can_hint_block_placement(stack: &ItemStack) -> bool {
        use mod_api::{__private::item_listens, HookId};
        !item_listens(HookId::PlacementContext, &stack.item)
            && !item_listens(HookId::MustSurvive, &stack.item)
    }

    /// Run BlockItem.place on a disposable level for a click: where it placed
    /// and the blocks it set. The real level supplies collision shapes and
    /// building-blocking entities; random draws, scheduled ticks and events
    /// stay in the preview. `ItemStack.useOn` runs it in adventure mode only
    /// when the used stack's `can_place_on` names the clicked block.
    pub(super) fn remote_can_place(
        &mut self,
        hit: &super::Hit,
        stack: &ItemStack,
    ) -> Option<DryPlace> {
        self.remote_place_preview(hit, stack, true)
    }

    /// Whether a click would place `stack` (the controller guide's look): as
    /// `remote_can_place`, by the templates' rules alone, so looking asks no
    /// mod's placement or survival hook.
    pub(super) fn remote_would_place(&mut self, hit: &super::Hit, stack: &ItemStack) -> bool {
        self.remote_place_preview(hit, stack, false).is_some()
    }
    // remote_place_preview: remote/use.rs

    // remote_use_bucket: remote/use.rs

    /// Whether `held`'s `Item.use` needs a target this client sees none of
    /// (not SUCCESS): BucketItem.use (a fluid source or powder snow to pick
    /// up through `ClipContext.Fluid.SOURCE_ONLY`, a block to empty a full
    /// one against, `mayUseItemAt`), BottleItem.use (water to fill from) and
    /// BoatItem.use (a block or fluid surface with room for the boat).
    fn use_finds_no_target(&self, held: &ItemStack) -> bool {
        let id = &*held.item;
        let facts = |pos: [i32; 3]| {
            self.res
                .facts()
                .get(self.world.block(pos[0], pos[1], pos[2]))
        };
        let source = |want: Option<Fluid>| {
            let fluid_hit = self.pick_fluid();
            // pick_fluid steps through waterlogged blocks, but their water is
            // a source the SOURCE_ONLY ray stops at (and a BucketPickup).
            if let Some(hit) = self.pick()
                && facts(hit.pos).waterlogged
            {
                let eye = self.aim().map_or(hit.point, |(eye, _)| eye);
                let d = |p: [f64; 3]| (0..3).map(|a| (p[a] - eye[a]).powi(2)).sum::<f64>();
                if fluid_hit
                    .as_ref()
                    .is_none_or(|fluid| d(fluid.point) >= d(hit.point))
                {
                    return want.is_none_or(|want| want == Fluid::Water);
                }
            }
            fluid_hit.is_some_and(|hit| {
                let f = facts(hit.pos);
                let fluid = f.fluid != Fluid::None && f.fluid_level == 8 && !f.fluid_falling;
                match want {
                    Some(want) => fluid && f.fluid == want,
                    None => fluid || self.is_powder_snow(hit.pos),
                }
            })
        };
        // SolidBucketItem only has useOn: its use is Item.use's PASS.
        if id == "minecraft:powder_snow_bucket" {
            return true;
        }
        if id == "minecraft:bucket" {
            return !self.may_build() || !source(None);
        }
        if sunrise_client::items::bucket_contents(id).is_some() {
            return !self.may_build() || self.pick().is_none();
        }
        if id == "minecraft:glass_bottle" {
            // A dragon's breath cloud never counts here: its owner is not
            // synced, so this client's BottleItem.use finds none and PASSes;
            // the packet still goes and the server fills the bottle.
            return !source(Some(Fluid::Water));
        }
        if super::boats::any_boat_wood(id).is_some() {
            return self.boat_to_place(held).is_none();
        }
        false
    }

    // remote_start_held_use: remote/use.rs

    // remote_use_instrument: remote/use.rs

    /// Whether `stack` stays in use until the button lets go (bows,
    /// crossbows, tridents, spyglasses, shields, eyes of ender, whose
    /// `getUseDuration` 0 never counts down to the end) or the server ends
    /// it (a mod's item): its use only counts ticks here.
    fn held_until_released(&self, stack: &ItemStack) -> bool {
        if self.mod_use(stack) {
            return true;
        }
        matches!(
            &*stack.item,
            "minecraft:bow"
                | "minecraft:crossbow"
                | "minecraft:trident"
                | "minecraft:spyglass"
                | "minecraft:ender_eye"
        ) || sunrise_client::spear::Kinetic::of(&stack.item).is_some()
            || self.blocks_attacks(stack).is_some()
            || self.patched_held_use(stack)
    }

    /// Whether the server's patch gives `stack`, an item with no consumable
    /// of its own, a `consumable` with a use time or `blocks_attacks` (a
    /// sword made to block): `Item.use` starts a use of it
    /// (`Consumable.startConsuming`, `startUsingItem`) that lasts until the
    /// button lets go or the server stops it, as the client's
    /// `LivingEntity.updateUsingItem` never completes one.
    fn patched_held_use(&self, stack: &ItemStack) -> bool {
        let (ticks, blocks) = sunrise_client::item_sync::patched_use(stack);
        ticks.map_or(blocks, |ticks| ticks > 0)
            && self
                .items()
                .get(&stack.item)
                .is_none_or(|i| i.consumable.is_none())
    }

    // remote_use_hands: remote/use.rs

    // remote_complete_using: remote/use.rs
    pub(super) fn request_remote_mining_stop(&mut self) {
        if let Some(r) = self.remote.as_mut() {
            r.stop_mining = true;
        }
    }

    pub(in crate::game) fn remote_handle_input(&mut self) {
        let Some(r) = self.remote.as_mut() else {
            return;
        };
        let mut input = r.input.take_queued();
        let stop = std::mem::take(&mut r.stop_mining);
        // Minecraft.tick runs handleKeybinds only while `gui.screen() == null`: a
        // book, a sign editor, a dialog or a prompt over the world (each a
        // `Screen` in vanilla) stops the attack and use mappings from acting,
        // and `KeyMapping.releaseAll` let go of the buttons when it opened
        // (`track_toggle_screens`), so a dig held into a book ends (#9729).
        let ready = r.loaded_sent
            && self.screen == Screen::Playing
            && self.sleeping.is_none()
            && !self.screen_over_playing();
        self.refresh_client_pick(1.0);
        if stop || !ready {
            self.stop_remote_mining();
        }
        if !ready {
            // A screen's opening released the keys (`KeyMapping.releaseAll`).
            self.hotbar_presses
                .iter_mut()
                .for_each(sunrise_client::input_queue::ClickQueue::clear);
            self.agent_attack_clicks_left(0);
            return;
        }
        let (attack, use_item) = (self.left, self.right);
        input.tick(&mut RemoteInputWorld(self), attack, use_item);
        // An item in use drains the queues without acting on them.
        self.agent_attack_clicks_left(
            input.queued(sunrise_client::interaction_input::Button::Attack),
        );
        self.remote_count_miss_time();
        if let Some(r) = self.remote.as_mut() {
            input.merge_queued(std::mem::take(&mut r.input));
            r.input = input;
        }
    }

    // remote_send_carried: remote/sync.rs

    /// `MultiPlayerGameMode.attack` runs `Player.attack` on this client too: its
    /// `isSweepAttack` for a damaging hit on `target` (`living`: a
    /// `LivingEntity`, which may be hit critically), heard by native mods on
    /// both sides. Only the server sweeps.
    fn remote_sweep_event(&mut self, target: &str, living: bool) {
        if mod_api::__private::mods().is_empty() {
            return;
        }
        let (damage, scale) = self.remote_attack_damage();
        let magic = scale * (self.enchanted_damage(target, damage, None) - damage);
        if damage <= 0.0 && magic <= 0.0 {
            return;
        }
        let crit = living && self.critical_attack(scale);
        self.sweep_decision(scale, crit, target);
    }

    /// The entity type of the boat, minecart, armor stand or End crystal with
    /// server id `id` that an attack reaches `Player.attack`'s sweep check on,
    /// and whether it is living. A painting or item frame is not: on this
    /// client `skipAttackInteraction` hurts it and stops the attack there.
    fn remote_attacked_object(&self, id: i32) -> Option<(String, bool)> {
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

    /// The latency the server's player list gives this player
    /// (`PlayerInfo.getLatency`), none before the server lists it. A new entry reads 0 until the
    /// server's first latency update (every 600 ticks).
    pub(in crate::game) fn own_latency(&self) -> Option<i32> {
        let r = self.remote.as_ref()?;
        r.tab_players.get(&r.profile).map(|me| me.latency)
    }

    /// An attribute's value on this client's player, if its map has the attribute.
    fn own_attribute(&mut self, id: &str) -> Option<f64> {
        let p = self.player.as_mut()?;
        let key = p.attributes.key(id)?;
        p.attributes.lookup(Some(&key))?;
        p.attributes
            .value(Some(key), sunrise_world::attribute_map::ValueKind::Computed)
            .ok()
    }

    // remote_key: remote/ui.rs

    /// `Minecraft.handleKeybinds`: a key that opens a screen runs
    /// `Gui.setScreen`, whose `KeyMapping.releaseAll` clears the use key, and
    /// the same invocation then releases an active use
    /// (`player.isUsingItem() && !keyUse.isDown()` → `releaseUsingItem`).
    /// Only the screens opened from those keybinds do this (inventory, chat,
    /// advancements); the pause menu, server containers, mount inventories and
    /// dialogs release on the first tick after closing instead, as vanilla
    /// does.
    pub(super) fn release_use_for_keybind_screen(&mut self) {
        // Only a server's client holds a use to release.
        if self.remote.is_none() {
            return;
        }
        // `KeyMapping.releaseAll` clears key.use; without this the button still
        // reads held and a use would start again on the first tick back.
        self.right = false;
        sunrise_client::interaction_input::release_on_screen_open(&mut RemoteInputWorld(self));
    }

    /// `MultiPlayerGameMode.releaseUsingItem`: sends `RELEASE_USE_ITEM` and
    /// runs the client half of `Item.releaseUsing` (a Riptide launch moves the
    /// player itself). Vanilla reaches it both when the use key is let go and,
    /// in the same tick, when a keybind opens a screen.
    fn release_remote_use(&mut self) {
        self.send_action(6);
        // a mod's Stop may skip the item's release, and a mod's own release follows.
        let hand = self.used_hand();
        if let Some((slot, ticks)) = self.using.take()
            && let Some(hand) = hand
            && let Some(stack) = self.stack_in(slot).cloned()
        {
            self.with_hand(hand, |g| {
                if !g.use_item_stopped(&stack, ticks) {
                    return;
                }
                if &*stack.item == "minecraft:trident" {
                    g.release_trident(g.ticks_held(&stack, ticks));
                }
                g.mod_release_using(&stack, ticks);
            });
        }
    }

    /// ClientPacketListener.sendChat / sendCommand, signed with a chat
    /// session (game/network/remote_chat.rs). The Chat option's Hidden and System
    /// levels keep chat out and Hidden keeps the commands out (ChatAbilities).
    pub(super) fn send_signed_chat_input(&mut self, message: &str) {
        let message = message.trim();
        if message.is_empty() || !self.chat_abilities().allows(message) {
            return;
        }
        if let Some(command) = message.strip_prefix('/') {
            if self.studio_command(command) {
                return;
            }
            self.send_command(command);
        } else {
            self.send_signed_chat(message);
        }
    }

    /// ChatScreen.handleChatInput fires the client event for chat, not commands.
    pub(super) fn send_chat_from_screen(&mut self, message: &str) {
        if message.starts_with('/') || !mod_api::__private::listened(mod_api::HookId::ClientChat) {
            self.send_signed_chat_input(message);
        } else if let Some(uuid) = self.remote.as_ref().map(|r| r.profile) {
            let id = self.own_entity_id();
            let sent = chat_after_event(message, |outgoing, cancel| {
                self.native_client_chat(id, uuid, message, outgoing, cancel);
            });
            if let Some(outgoing) = sent {
                self.send_signed_chat(&outgoing);
            }
        }
    }

    /// MultiPlayerGameMode.dropItem (key.drop): predict the removal, then send
    /// the action; swing only if a nonempty stack was removed.
    pub(super) fn remote_drop(&mut self, all: bool) {
        // ensureHasSentCarriedItem: the server drops from the slot just chosen.
        self.remote_send_carried();
        // MultiPlayerGameMode.dropItem predicts the removal and sends only the drop: the
        // server's stack agrees, so no creative slot edit follows at the end of the tick.
        let before = self.creative_stacks();
        let dropped =
            sunrise_client::items::take_held_drop(&mut self.inventory[self.selected], all, false);
        self.remote_synced_since(&before);
        self.send_action(if all { 4 } else { 5 });
        if dropped.is_some() {
            self.swing_hand(Hand::Main, SwingAnimation::DEFAULT);
        }
    }

    /// Minecraft.handleKeybinds, key.swapOffhand: the server swaps the hands
    /// (ServerboundPlayerActionPacket SWAP_ITEM_WITH_OFFHAND at BlockPos.ZERO, facing down) and
    /// sends both slots back; nothing is predicted.
    pub(super) fn remote_swap_hands(&mut self) {
        self.send_action(7);
        // Equal stacks swap with no slot update to wait for.
        let (main, off) = (self.stack_in(self.selected), self.offhand.as_ref());
        let moves = !super::hand::matches_for_swap(main, off);
        if let Some(r) = self.remote.as_mut() {
            r.swap_answer = [moves; 2];
        }
    }

    /// The server set inventory `slot` (`shield::OFFHAND_SLOT` for the off
    /// hand), `changed` from the stack this client held there. The stack
    /// that answers a swap (`setItemInHand`) is another object, which the
    /// hand shows after the swap animation (`hand_stack_replaced`); other
    /// updates of a hand's slot count as the held stack changing in place,
    /// as this client doesn't predict every use the way vanilla's does
    /// (a thrown snowball's stack shrinks only when the server says so).
    fn remote_hand_slot_set(&mut self, slot: usize, changed: bool) {
        let hand = if slot == super::shield::OFFHAND_SLOT {
            1
        } else if slot == self.selected {
            0
        } else {
            return;
        };
        let answer = self
            .remote
            .as_mut()
            .is_some_and(|r| std::mem::take(&mut r.swap_answer[hand]));
        if answer && changed {
            self.hand_stack_replaced(slot);
        }
    }

    // remote_click: remote/use.rs

    /// ServerboundPlayerCommandPacket STOP_SLEEPING.
    pub(super) fn remote_stop_sleeping(&mut self) {
        let id = self.remote.as_ref().map_or(0, |r| r.entity_id);
        self.send(S::PlayerCommand(play::PlayerCommand {
            id,
            action: 0,
            data: 0,
        }));
    }
    pub(super) fn remote_respawn(&mut self) {
        self.send(S::ClientCommand(play::ClientCommand { action: 0 }));
    }

    /// A connection's screen is over the world: connecting, reconfiguring, or
    /// the level loading screen, a portal's included (`remote_overlay`).
    pub(super) fn remote_screen_up(&self) -> bool {
        !self.replay.playing()
            && self
                .remote
                .as_ref()
                .is_some_and(|r| self.player.is_none() || r.level_load.is_active())
    }

    // remote_overlay: remote/ui.rs

    /// Connected or connecting with no player yet: `ConnectScreen`, then
    /// `LevelLoadingScreen`, over the panorama with its blur.
    pub(super) fn remote_loading(&self) -> bool {
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

    /// `ServerReconfigScreen`: a vertical layout of cells padded by 10, the
    /// title (9 high) over Disconnect (150 x 20), centred. The title's and the
    /// button's top left corners.
    fn reconfigure_layout(&self, (w, h): (f32, f32)) -> ((f32, f32), (f32, f32)) {
        let title = sunrise_client::text::Text::translate("connect.reconfiguring", &[]);
        let title_w = self.gui.atlas.font.width_text(&title).ceil() as i32;
        let width = (title_w + 20).max(170);
        let left = ((w as i32 - width) as f32 * 0.5) as i32;
        let top = ((h as i32 - 69) as f32 * 0.5) as i32;
        let across = |child: i32| (10.0 + ((width - child - 10) as f32 - 10.0) * 0.5) as i32;
        (
            ((left + across(title_w)) as f32, (top + 10) as f32),
            ((left + across(150)) as f32, (top + 39) as f32),
        )
    }

    /// `ServerReconfigScreen` over the panorama; Disconnect wakes up after
    /// `DISCONNECT_TIME` (600 ticks).
    fn draw_reconfiguring(&self, batch: &mut HudBatch, since: u64) {
        let atlas = &self.gui.atlas;
        let (w, h) = (batch.width, batch.height);
        sunrise_client::ui::menu_background(batch, atlas, 0.0, 0.0, w, h, false);
        let title = sunrise_client::text::Text::translate("connect.reconfiguring", &[]);
        let ((tx, ty), (bx, by)) = self.reconfigure_layout((w, h));
        batch.text_styled(atlas, &title, tx, ty, u32::MAX, true);
        // CommonComponents.GUI_DISCONNECT
        let label = sunrise_client::text::Text::translate("menu.disconnect", &[]);
        let active = self.ticks.saturating_sub(since) >= 600;
        let hover = sunrise_client::ui::hovered(self.cursor, bx, by, 150.0, 20.0);
        sunrise_client::ui::button(batch, atlas, bx, by, 150.0, 20.0, &label, active, hover);
    }

    /// `ConnectScreen`'s Cancel: a server's (single player's server isn't
    /// joined through `ConnectScreen`), until the login is done.
    fn connect_cancel(&self, (w, h): (f32, f32)) -> Option<(f32, f32)> {
        let r = self.remote.as_ref()?;
        (self.player.is_none() && !r.joined && self.integrated.is_none())
            .then(|| sunrise_client::level_loading::connect_cancel(w, h))
    }

    /// A click on `ConnectScreen`: Cancel aborts the connection
    /// (`connect.aborted`) and goes back. Escape does nothing there
    /// (`shouldCloseOnEsc` is false).
    pub(super) fn connect_screen_click(&mut self) -> Result<()> {
        // ServerReconfigScreen's Disconnect, once it is active.
        if let Some(since) = self.remote.as_ref().and_then(|r| r.reconfiguring) {
            let (_, (x, y)) = self.reconfigure_layout(self.gui_size());
            if self.ticks.saturating_sub(since) >= 600
                && sunrise_client::ui::hovered(self.cursor, x, y, 150.0, 20.0)
            {
                self.audio.play_button_click();
                self.leave_server(None)?;
            }
            return Ok(());
        }
        let Some((x, y)) = self.connect_cancel(self.gui_size()) else {
            return Ok(());
        };
        if sunrise_client::ui::hovered(self.cursor, x, y, 200.0, 20.0) {
            self.audio.play_button_click();
            self.leave_server(None)?;
        }
        Ok(())
    }

    /// The server's death message; chat uses the shared ChatComponent.
    pub(super) fn remote_hud(&self, batch: &mut HudBatch) {
        let Some(r) = &self.remote else { return };
        let atlas = &self.gui.atlas;
        let w = batch.width;
        if self.screen == Screen::Dead
            && let Some(message) = &r.death_message
        {
            // DeathScreen draws the styled cause component.
            let tw = atlas.font.width_text(message);
            batch.text_styled(
                atlas,
                message,
                ((w - tw) / 2.0).floor(),
                85.0,
                u32::MAX,
                true,
            );
        }
    }

    /// Shared flag 6 of an entity on a server's client (`None` without a server).
    pub(super) fn remote_glowing(&self, id: u32) -> Option<bool> {
        let r = self.remote.as_ref()?;
        Some(r.glowing.contains(&(id as i32)))
    }

    /// The profile name of another player the server sent, by entity id.
    pub(super) fn remote_player_name(&self, id: u32) -> Option<&str> {
        let r = self.remote.as_ref()?;
        r.level_player_name(id as i32, &r.player_uuid(id)?)
    }
    pub(super) fn remote_player_uuid(&self, id: u32) -> Option<u128> {
        let remote = self.remote.as_ref()?;
        if self.player_model.as_ref().is_some_and(|m| m.id == id) {
            Some(remote.profile)
        } else {
            remote.level_players.get(&(id as i32)).copied()
        }
    }

    /// Status for `sunrise-ctl status`.
    /// The other players' entities in the level and their names (their
    /// profiles', which outlive the player list's entries).
    pub(super) fn remote_player_names(&self) -> Vec<(u32, String)> {
        let Some(r) = self.remote.as_ref() else {
            return Vec::new();
        };
        r.level_players
            .iter()
            .filter(|(_, uuid)| **uuid != r.profile)
            .filter_map(|(id, uuid)| Some((*id as u32, r.level_player_name(*id, uuid)?.to_owned())))
            .collect()
    }

    // remote_status: remote/status.rs
}

/// `Entity.lookAt`'s rotation from `from` towards `to`, as (yaw, pitch).
fn look_at_rotation(from: [f64; 3], to: [f64; 3]) -> (f32, f32) {
    use sunrise_core::geometry::trig::atan2;
    let (dx, dy, dz) = (to[0] - from[0], to[1] - from[1], to[2] - from[2]);
    let horizontal = (dx * dx + dz * dz).sqrt();
    // `* 180.0F / (float)Math.PI` on the double.
    let degrees = |r: f64| r * 180.0 / f64::from(std::f32::consts::PI);
    let pitch = wrap_degrees((-degrees(atan2(dy, horizontal))) as f32);
    let yaw = wrap_degrees(degrees(atan2(dz, dx)) as f32 - 90.0);
    (yaw, pitch)
}

/// `PositionMoveRotation.calculateAbsolute`'s pitch: the packet's pitch,
/// added to `source` when it is relative, clamped to +-90.
fn position_packet_pitch(source: f32, change: f32, relative: bool) -> f32 {
    let offset = if relative { source } else { 0.0 };
    (offset + change).clamp(-90.0, 90.0)
}

/// `Mth.wrapDegrees`.
pub(super) fn wrap_degrees(a: f32) -> f32 {
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
pub(super) fn remote_player_head_turn(m: &mut Mob) {
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
pub(super) fn input_ordinal(input: Input) -> i32 {
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

mod dry_level;
use dry_level::DryLevel;

type DrySound = (
    sunrise_core::position::BlockPos,
    std::sync::Arc<str>,
    mod_api::SoundSource,
    f32,
    f32,
);
/// The blocks a use on the dry level set.
type DryChanges = Vec<(sunrise_core::position::BlockPos, BlockStateId)>;
/// What a use on the dry level changed, and the sounds it played.
pub(super) type DryUse = (DryChanges, Vec<DrySound>);
/// Where `BlockItem.place` on the dry level placed, and the blocks it set.
pub(super) struct DryPlace {
    pub(super) pos: sunrise_core::position::BlockPos,
    matches_candidate: bool,
    pub(super) changes: DryChanges,
}

/// The blocks a guest's hand flips that its use predicts (and hears at once): a wooden or
/// copper door, trapdoor or fence gate, a lever, a button or a cauldron, whose fill level
/// changes as the bucket, bottle or dye lands. An iron door or trapdoor has no hand use.
pub(super) fn hand_operated(
    behaviours: &sunrise_world::level::Behaviours,
    state: BlockStateId,
) -> bool {
    behaviours
        .data(state)
        .is(sunrise_world::level::classes::Class::FenceGate)
        || named_hand_operated(behaviours.table.name(state))
}

fn named_hand_operated(name: &str) -> bool {
    let name = name.trim_start_matches("minecraft:");
    ((name.ends_with("_door") || name.ends_with("_trapdoor")) && !name.starts_with("iron_"))
        || name.ends_with("_button")
        || name == "lever"
        || name.ends_with("cauldron")
}

/// `InteractionHand` on the wire.
pub(super) fn hand_id(hand: Hand) -> i32 {
    match hand {
        Hand::Main => 0,
        Hand::Off => 1,
    }
}

/// Whether `Item.use` of `info` ends `Minecraft.startUseItem`'s click, so the
/// off hand gets no turn. `FireworkRocketItem.use` answers SUCCESS while the
/// player glides (the rocket boosts them) and PASS otherwise.
pub(super) fn use_ends_click(info: &sunrise_client::items::ItemInfo, fall_flying: bool) -> bool {
    item_has_use(info) || (fall_flying && &*info.id == "minecraft:firework_rocket")
}

/// Whether `Item.use` of this item answers SUCCESS on this client, which
/// swings (`MultiPlayerGameMode.useItem`'s `result.shouldSwing()`), rather
/// than CONSUME, which only ends the click: a bow, crossbow, trident or
/// spear starting its charge, `ItemUtils.startUsingInstantly`'s food, drink,
/// spyglass, shield and goat horn, and the ender eye's SUCCESS_SERVER (the
/// server's swing source, not this client's).
pub(super) fn remote_use_swings(held: &ItemStack, consumable: bool) -> bool {
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
fn remote_object_swings(response: Option<bool>, is_boarding: bool) -> bool {
    response.unwrap_or(!is_boarding)
}

/// Whether right-clicking an item frame passes (`InteractionResult.PASS`).
/// A fixed frame or an empty frame when clicking with an empty hand passes.
fn item_frame_interact_passes(fixed: bool, frame_has_item: bool, hand_has_item: bool) -> bool {
    fixed || (!frame_has_item && !hand_has_item)
}

/// Whether right-clicking a boat passes (`InteractionResult.PASS`) on a client.
/// `AbstractBoat.interact` passes when sneaking or out of control (60 ticks), and
/// answers SUCCESS otherwise without checking for room (only the server's
/// `startRiding` does). `AbstractChestBoat.interact` opens the chest instead of
/// passing, unless it is out of control, not sneaked at and has room.
fn boat_interact_passes(chest: bool, sneaking: bool, out_of_control: bool, has_room: bool) -> bool {
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
pub(super) fn item_has_use(info: &sunrise_client::items::ItemInfo) -> bool {
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
pub(super) fn client_view_distance(render_distance: i32) -> i8 {
    render_distance.clamp(2, i32::from(i8::MAX)) as i8
}

/// `ClientPacketListener.handleBlockEntityData` loads the tag into the existing
/// block entity, so the state the client builds itself stays: copied from `old`
/// into `new`, the block entity decoded from the tag.
fn keep_client_state(old: Option<&BlockEntity>, new: &mut BlockEntity) {
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
        _ => {}
    }
}

/// `LocalPlayer.onSyncedDataUpdated(DATA_LIVING_ENTITY_FLAGS)`: the server's
/// flags (1 using, 2 the off hand) start a use the client is not running
/// (`startUsingItem`, which needs an item in that hand), and stop one it is
/// (`stopUsingItem`). `using` is the slot in use and its ticks.
fn synced_local_use(
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

/// The game as the tick's input sees it.
struct RemoteInputWorld<'g, 'a>(&'g mut Game<'a>);

struct RemoteUseWorld<'g, 'a>(&'g mut Game<'a>, mod_api::InteractionAttempt);

impl sunrise_client::interaction_input::TargetUseWorld for RemoteUseWorld<'_, '_> {
    fn use_entity(&mut self) -> bool {
        self.0.remote_use_entity(Hand::Main)
    }
    fn use_block(&mut self) -> bool {
        self.0.remote_use_block(Hand::Main)
    }
    fn use_item(&mut self) {
        self.0.remote_use_hands(self.1);
    }
}

impl sunrise_client::interaction_input::InteractionWorld for RemoteInputWorld<'_, '_> {
    fn pick_hit_result(&mut self) {
        self.0.pick_tick_targets();
    }
    fn handle_keys(&mut self) {
        self.0.remote_queued_keys();
    }
    fn release_hit_result(&mut self) {
        self.0.release_tick_targets();
    }
    fn using_item(&self) -> bool {
        self.0.using.is_some()
    }
    fn release_using(&mut self) {
        if let Some((slot, ticks)) = self.0.using
            && self.0.mod_use_tick(slot, ticks, true)
        {
            return;
        }
        self.0.release_remote_use();
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
        if let Some(entity) = self.0.pick_entity_hit() {
            if let Some(id) = entity {
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
                    id,
                    include_data: self.0.has_control_down(),
                }));
            }
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

#[derive(Default)]
pub(super) struct ClientMenu {
    current: Option<(i32, String)>,
    pub(super) generation: u64,
}

impl ClientMenu {
    pub(super) fn set(&mut self, current: Option<(i32, String)>) {
        self.generation = self
            .generation
            .checked_add(1)
            .expect("menu generation exhausted");
        self.current = current;
    }

    pub(super) fn as_ref(&self) -> Option<&(i32, String)> {
        self.current.as_ref()
    }
}

impl Remote {
    /// The interpolation a boat or other object `id` had toward the server's
    /// position, taken out: its target, rotation and steps to go.
    pub(super) fn take_object_lerp(
        &mut self,
        id: u32,
    ) -> Option<([f64; 3], Option<(f32, f32)>, u32)> {
        self.lerps
            .remove(&(id as i32))
            .map(|l| (l.target, l.rotation, l.steps))
    }
    pub(super) fn set_menu_title(&mut self, title: Option<sunrise_client::text::Text>) {
        self.menu_title_snapshot = title.try_into_api().ok().flatten().map(Arc::new);
        self.menu_title = title;
    }

    pub(super) fn menu_title_snapshot(&self) -> Option<&mod_api::Text> {
        self.menu_title_snapshot.as_deref()
    }

    pub(super) fn shared_menu_title_snapshot(&self) -> Option<Arc<mod_api::Text>> {
        self.menu_title_snapshot.clone()
    }

    /// Resolve registry keys only when registries or the dimension change.
    fn resolve_clocks(&mut self) {
        self.overworld_clock = self
            .registries
            .id("minecraft:world_clock", "minecraft:overworld");
        self.default_clock = self
            .dimension
            .default_clock
            .as_deref()
            .and_then(|name| self.registries.id("minecraft:world_clock", name));
        for id in [self.overworld_clock, self.default_clock]
            .into_iter()
            .flatten()
        {
            self.clocks.entry(id).or_insert(play::ClockState {
                clock: id,
                total_ticks: 0,
                partial_tick: 0.0,
                rate: 1.0,
            });
        }
    }

    fn clock_time(&self, id: Option<i32>) -> i64 {
        id.and_then(|id| self.clocks.get(&id))
            .map_or(0, |c| c.total_ticks)
    }

    pub(super) fn overworld_clock_time(&self) -> i64 {
        self.clock_time(self.overworld_clock)
    }

    /// `Minecraft.level != null`: from `handleLogin` until the next
    /// `handleConfigurationStart` (`clearClientLevel`). Minecraft.tick sends
    /// CLIENT_TICK_END only then, so a client waiting for the server's Login
    /// (a busy server takes seconds to place a new player) sends nothing.
    pub(in crate::game) fn has_level(&self) -> bool {
        client_has_level(self.logged_in, self.reconfiguring)
    }

    /// An entity the server added to this client level: its type
    /// (`minecraft:text_display`) and its position as last sent.
    pub(super) fn tracked_entity(&self, id: i32) -> Option<(&str, [f64; 3])> {
        if let Some(row) = self.general.rows.get(&id) {
            return row.shown.then(|| (row.kind(), row.position));
        }
        self.entities
            .get(&id)
            .map(|t| (t.kind.as_str(), t.position))
    }

    /// The profile of the player entity `id`: this window's own player, or
    /// another one by the UUID it was added with and its profile name.
    pub(super) fn level_player(&self, id: i32) -> Option<(u128, Option<&str>)> {
        if id == self.entity_id {
            return Some((self.profile, Some(self.profile_name.as_str())));
        }
        let uuid = *self.level_players.get(&id)?;
        Some((uuid, self.level_player_names.get(&id).map(String::as_str)))
    }

    /// `PlayerInfo.getMessageValidator().updateAndValidate(message)` for `message.sender`.
    fn validate_chat(
        &mut self,
        message: &secure_chat::Incoming,
        now: i64,
    ) -> Option<secure_chat::Validated> {
        let sender = message.sender;
        if let Some(validator) = self.chat_validators.get_mut(&sender) {
            return validator.update_and_validate(message, now);
        }
        let (mut validator, settled) = self.new_chat_validator(sender);
        let validated = validator.update_and_validate(message, now);
        // While the services' keys are still on their way the validator is a stopgap, not kept.
        if settled {
            self.chat_validators.insert(sender, validator);
        }
        validated
    }

    /// `ClientPacketListener.initializeChatSession` for `sender`'s entry: a session the services'
    /// keys vouch for makes a keyed validator, anything else the fallback of
    /// `enforcesSecureChat()` (`canValidateProfileKeys() && serverEnforcesSecureChat`). `false`
    /// while the keys are not known yet.
    fn new_chat_validator(&self, sender: u128) -> (secure_chat::MessageValidator, bool) {
        let keys = self.services_keys.lock().unwrap_or_else(|e| e.into_inner());
        let (can_validate, settled) = match &*keys {
            ServicesKeys::Ready(_) => (true, true),
            ServicesKeys::Missing => (false, true),
            ServicesKeys::Pending => (false, false),
        };
        let fallback =
            secure_chat::MessageValidator::fallback(can_validate && self.enforces_secure_chat);
        let (ServicesKeys::Ready(keys), Some(Some((id, expires_at, key, signature)))) =
            (&*keys, self.chat_sessions.get(&sender))
        else {
            if matches!(&*keys, ServicesKeys::Missing)
                && self.chat_sessions.get(&sender).is_some_and(Option::is_some)
            {
                log::warn!(
                    "Ignoring chat session from {sender:032x} due to missing Services public key"
                );
            }
            return (fallback, settled);
        };
        let profile_key = secure_chat::ProfileKey {
            expires_at: *expires_at,
            key: key.clone(),
            signature: signature.clone(),
        };
        match secure_chat::ChatSession::validate(*id, profile_key, keys, sender) {
            Ok(session) => (secure_chat::MessageValidator::keyed(session), true),
            Err(why) => {
                log::error!("Failed to validate profile key for player {sender:032x}: {why}");
                (fallback, true)
            }
        }
    }

    /// How far the server sends chunks: its view distance and the ring
    /// around (`ChunkTrackingView`), at least two.
    pub(super) fn sent_radius(&self) -> i32 {
        self.view_distance.max(2).saturating_add(1)
    }

    /// A player entity's profile name: the one it was created with, else its
    /// player-list entry's (an entity the list named after it spawned).
    fn level_player_name(&self, id: i32, uuid: &u128) -> Option<&str> {
        self.level_player_names
            .get(&id)
            .or_else(|| self.players.get(uuid))
            .map(String::as_str)
    }

    /// Listed UUIDs always have a tab entry; removal clears both together.
    pub(super) fn listed_player_count(&self) -> usize {
        self.listed.len()
    }

    /// The local player's profile UUID.
    pub(super) fn profile(&self) -> u128 {
        self.profile
    }

    /// Another player's profile UUID, by their entity id (None for the local player).
    pub(super) fn player_uuid(&self, id: u32) -> Option<u128> {
        let uuid = *self.level_players.get(&(id as i32))?;
        (uuid != self.profile).then_some(uuid)
    }

    /// The entity id of a player in the level, by profile UUID.
    pub(super) fn player_entity(&self, uuid: u128) -> Option<u32> {
        self.level_players
            .iter()
            .find(|(_, u)| **u == uuid)
            .map(|(id, _)| *id as u32)
    }

    /// The client's synced Entity shared flag 0, including the local player.
    pub(in crate::game) fn entity_on_fire(&self, id: i32) -> Option<bool> {
        if id == self.entity_id {
            Some(self.own_on_fire)
        } else {
            self.entities.get(&id).map(|t| t.on_fire)
        }
    }
    pub(in crate::game) fn tracked_fire_immunity(&self, id: i32) -> Option<bool> {
        Some(self.entities.get(&id)?.mod_fire_immune)
    }
    pub(in crate::game) fn set_tracked_fire_immunity(&mut self, id: i32, immune: bool) -> bool {
        self.entities.get_mut(&id).is_some_and(|entity| {
            entity.mod_fire_immune = immune;
            true
        })
    }
    /// The kind and last sent position of a tracked server entity.
    pub(in crate::game) fn tracked(&self, id: i32) -> Option<(&str, [f64; 3])> {
        self.entities.get(&id).map(|t| (&*t.kind, t.position))
    }
}

fn object_skin_changed(published: &HashMap<String, String>, key: &str, skin: &str) -> bool {
    published.get(key).is_none_or(|previous| previous != skin)
}

/// The textures to resolve skins from: every list entry's, and for a player
/// entity whose entry is gone the profile it kept. `kept` first takes, for each
/// player entity without one, its entry's textures (`AbstractClientPlayer.getPlayerInfo`
/// looks the PlayerInfo up once and holds it, removed from the list or not).
fn skin_entries(
    level_players: &HashMap<i32, u128>,
    listed: &HashMap<u128, sunrise_client::skins::Textures>,
    kept: &mut HashMap<i32, sunrise_client::skins::Textures>,
) -> Vec<(u128, sunrise_client::skins::Textures)> {
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
fn snapshot_value(s: &play::AttributeSnapshot) -> f64 {
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
pub(super) fn client_millis() -> u64 {
    static ORIGIN: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    ORIGIN
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_millis() as u64
}

/// What `ChatScreen.handleChatInput` sends of typed chat once `ClientChatEvent` (`ask`, with
/// the outgoing message and the cancel flag) had its say: nothing when canceled or emptied.
fn chat_after_event(message: &str, ask: impl FnOnce(&mut String, &mut bool)) -> Option<String> {
    let mut outgoing = message.to_owned();
    let mut cancel = false;
    ask(&mut outgoing, &mut cancel);
    (!cancel && !outgoing.is_empty()).then_some(outgoing)
}
/// `Entity.isOnFire` for the window's player: while the server's flag is set
/// the predicted tick's countdown never takes `fire_ticks` below 1.
pub(super) fn hold_server_fire(fire_ticks: i32, server_on_fire: bool) -> i32 {
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
pub(super) fn shape_meets(
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
pub(super) fn snap_target(
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
fn client_has_level(logged_in: bool, reconfiguring: Option<u64>) -> bool {
    logged_in && reconfiguring.is_none()
}

/// `ClientChunkCache.Storage.inRange`: whether column `x`, `z` is within `calculateStorageRange`
/// (the server's radius, at least two, plus three) of the view centre. The radius and the centre
/// are the server's to send, any `i32`: the distances are taken without overflow, which panicked
/// a debug build and, wrapping in release, dropped every loaded chunk.
fn in_storage_range(view_distance: i32, center: (i32, i32), x: i32, z: i32) -> bool {
    let range = view_distance.max(2).saturating_add(3).unsigned_abs();
    x.abs_diff(center.0) <= range && z.abs_diff(center.1) <= range
}

// A packet's removals (`RemoveEntities`), apart: this file may not grow.
#[cfg(test)]
mod interaction_event_tests;

#[cfg(test)]
mod native_entity_tests;

#[cfg(test)]
mod effect_blend_tests;
#[cfg(test)]
mod level_sound_tests;

#[cfg(test)]
mod attachment_query_tests;
#[cfg(test)]
mod passage_cross_tests;
#[cfg(test)]
mod respawn_motion_tests;
#[cfg(test)]
pub(crate) mod tests {
    use super::super::api_values::TryIntoApi;

    mod block_entity_state_tests {
        include!("block_entity_state_tests.rs");
    }
    mod chat_decoration_tests {
        include!("chat_decoration_tests.rs");
    }
    mod chat_event_tests {
        include!("chat_event_tests.rs");
    }
    mod chat_window_and_input_tests {
        include!("chat_window_and_input_tests.rs");
    }
    mod chunk_storage_tests {
        include!("chunk_storage_tests.rs");
    }
    mod click_ending_tests {
        include!("click_ending_tests.rs");
    }
    mod client_attributes_tests {
        include!("client_attributes_tests.rs");
    }
    mod client_block_changed_tests {
        include!("client_block_changed_tests.rs");
    }
    mod client_camera_tests {
        include!("client_camera_tests.rs");
    }
    mod client_day_time_tests {
        include!("client_day_time_tests.rs");
    }
    mod client_epochs_tests {
        include!("client_epochs_tests.rs");
    }
    mod client_handles_tests {
        include!("client_handles_tests.rs");
    }
    mod client_level_environment_tests {
        include!("client_level_environment_tests.rs");
    }
    mod client_level_lifecycle_tests {
        include!("client_level_lifecycle_tests.rs");
    }
    mod client_menu_tests {
        include!("client_menu_tests.rs");
    }
    mod client_player_reads_tests {
        include!("client_player_reads_tests.rs");
    }
    mod client_read_tests {
        include!("client_read_tests.rs");
    }
    mod creative_sync_tests {
        include!("creative_sync_tests.rs");
    }
    mod damage_sound_tests {
        include!("damage_sound_tests.rs");
    }
    mod dig_abort_tests {
        include!("dig_abort_tests.rs");
    }
    mod dimension_transition_tests {
        include!("dimension_transition_tests.rs");
    }
    mod game_event_tests {
        include!("game_event_tests.rs");
    }
    mod hand_operated_tests {
        include!("hand_operated_tests.rs");
    }
    mod hand_swap_tests {
        include!("hand_swap_tests.rs");
    }
    mod hanging_entity_age_tests {
        include!("hanging_entity_age_tests.rs");
    }
    mod menu_click_order_tests {
        include!("menu_click_order_tests.rs");
    }
    mod menu_generation_tests {
        include!("menu_generation_tests.rs");
    }
    mod menu_packets_tests {
        include!("menu_packets_tests.rs");
    }
    mod menu_title_tests {
        include!("menu_title_tests.rs");
    }
    mod movement_snap_tests {
        include!("movement_snap_tests.rs");
    }
    mod object_interactions_tests {
        include!("object_interactions_tests.rs");
    }
    mod particle_group_tests {
        include!("particle_group_tests.rs");
    }
    mod placement_preview_tests {
        include!("placement_preview_tests.rs");
    }
    mod player_animation_tests {
        include!("player_animation_tests.rs");
    }
    mod player_sleep_tests {
        include!("player_sleep_tests.rs");
    }
    mod predicted_block_notice_tests {
        include!("predicted_block_notice_tests.rs");
    }
    mod predicted_throw_tests {
        include!("predicted_throw_tests.rs");
    }
    mod remote_columns_tests {
        include!("remote_columns_tests.rs");
    }
    mod server_fire_flag_tests {
        include!("server_fire_flag_tests.rs");
    }
    mod slot_mapping_tests {
        include!("slot_mapping_tests.rs");
    }
    mod sound_entity_tests {
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

    mod spear_jab_tests {
        include!("spear_jab_tests.rs");
    }
    mod tab_publication_tests {
        include!("tab_publication_tests.rs");
    }
    mod thrown_item_swing_tests {
        include!("thrown_item_swing_tests.rs");
    }
    mod tracking_state_tests {
        include!("tracking_state_tests.rs");
    }
    mod use_flag_tests {
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
        use super::super::execution_role::ExecutionRole;
        use sunrise_protocol::{
            connection::{Connection, Pipe},
            packets::{Empty, configuration as cfg, login},
        };
        game.options.execution_role = ExecutionRole::Client;
        let (client, server) = Pipe::pair();
        game.connect_local(client, "MenuReadTest").unwrap();
        let (mut reader, mut writer) = Connection::local(server).split();
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
        writer
            .send(&cfg::Clientbound::FinishConfiguration(Empty).encode())
            .unwrap();
        let (id, body) = reader.recv().unwrap();
        assert!(matches!(
            cfg::Serverbound::decode(id, &body).unwrap(),
            cfg::Serverbound::FinishConfiguration(_)
        ));
        (reader, writer)
    }

    /// Spawns the connected client in the overworld as entity 7, profile 42.
    fn spawn_menu_test(game: &mut super::Game<'_>) -> sunrise_protocol::packets::play::SpawnInfo {
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
        game.enter_remote_dimension(&spawn, false, Reason::Other);
        let remote = game.remote.as_mut().unwrap();
        remote.logged_in = true;
        remote.entity_id = 7;
        remote.profile = 42;
        spawn
    }

    use super::{
        boat_interact_passes, chat_after_event, item_frame_interact_passes, remote_object_swings,
    };

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
                game.audio.capture = Some(Vec::new());
                let (answer, dry) = game.remote_block_reacts(&hit, None, true);
                assert!(answer.consumes_action());
                game.predict_use(&dry, 1);
                assert!(open(game), "the gate opens without a server round trip");
                let capture = game.audio.capture.as_mut().unwrap();
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
                        super::Hand::Main
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
            assert_eq!(game.audio.heard.get(blast_sound), Some(&1));
            let powered = game
                .with_props(gate, &[("open", "true"), ("powered", "true")])
                .unwrap();
            game.world.set(x, y, z, powered).unwrap();
            game.explosion_trigger_blocks(&mut [pos]).unwrap();
            assert_eq!(game.world.block(x, y, z), powered);
            assert_eq!(game.audio.heard.get(blast_sound), Some(&1));
        });
    }

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

    use sunrise_client::{
        containers::{Beacon, BlockEntity},
        items::ItemStack,
    };
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

    use super::Game;
}

#[cfg(test)]
mod boat_push_order_tests;
#[cfg(test)]
mod item_use_tick_tests;
#[cfg(test)]
mod movement_input_tests;
/// `Entity.baseTick`'s fluid update for a mob this window only displays (`fluids`, sampled before
/// the tick), with the splash it draws, and the block it stands on.
pub(super) fn client_fluid_step(
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
mod frozen_screen_effects_tests;
#[cfg(test)]
mod placement_prediction_tests;
#[cfg(test)]
#[path = "player_tick_tests.rs"]
mod player_tick_tests;
#[cfg(test)]
mod rider_tick_tests;
#[cfg(test)]
mod smoke_source_tests;
