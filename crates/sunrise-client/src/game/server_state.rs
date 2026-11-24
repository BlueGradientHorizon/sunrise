//! Server-owned fields of [`super::Game`].
use super::*;

pub(in crate::game) struct ServerState {
    /// The registries native mods registered data map types for, as of the
    /// server tables' last build: what `data_maps_updated` fires for.
    pub(in crate::game) data_map_registries: Vec<String>,
    /// The crafting table the 3x3 grid belongs to (`CraftingMenu.access`).
    pub(in crate::game) craft_table: Option<[i32; 3]>,
    /// The open container and the slots it had when it opened:
    /// `Container.stillValid` holds only while every block entity the menu
    /// took is still there, so a half of a double chest (or the container)
    /// going away closes it.
    pub(in crate::game) open_container_slots: Option<((i32, i32, i32), usize)>,
    pub(in crate::game) destroy_delay: u32,
    /// The prepared world is installed. The session lock is taken earlier so tag
    /// reads see ready tables, and is not this marker: a final save before the
    /// prepared `Level` is installed would replace the world with placeholder state.
    pub(in crate::game) world_adopted: bool,
    /// An early shutdown must not replace on-disk tickets while the server is
    /// still publishing levels or activating their tickets.
    pub(in crate::game) startup_ticket_restore_pending: bool,
    pub(in crate::game) paused_mob_hurt: Option<Box<sunrise_client::mobs::PausedMobHurt>>,
    pub(in crate::game) next_entity: u32,
    /// The last entity id when this level's entity pass began: an item with a
    /// later one was added during that pass and first ticks on the next
    /// (`EntityTickList`). `u32::MAX` until a tick has run.
    pub(in crate::game) ticked_entities_through: u32,
    /// The items `tick_blocks` dropped while applying the entities' block
    /// contacts (a boat breaking a lily pad). Vanilla runs those in each
    /// entity's own tick (`applyEffectsFromBlocks`), inside the entity pass,
    /// so they too first tick on the next tick.
    pub(in crate::game) contact_drops: std::ops::Range<u32>,
    /// Lecterns whose page-turn pulse is running (ticks left).
    pub(in crate::game) lectern_pulses: Vec<((i32, i32, i32), i32)>,
    /// The world spawn search's origin chunk and fallback height, once known.
    pub(in crate::game) spawn_origin: Option<((i32, i32), i32)>,
    /// `native_create_spawn_position` was asked, and a mod chose the spawn.
    pub(in crate::game) spawn_hook_fired: bool,
    pub(in crate::game) spawn_hook_cancelled: bool,
    pub(in crate::game) spawn_spiral: sunrise_client::world_spawn::ChunkSpiral,
    /// A new player's spawn search, while its candidates' chunks load.
    pub(in crate::game) player_spawn: Option<sunrise_client::world_spawn::PlayerSpawnFinder>,
    /// Chunks a respawn search waits for this tick, kept loading beside the
    /// player's own (taken by `update_world`).
    pub(in crate::game) spawn_wait: Vec<(i32, i32)>,
    pub(in crate::game) raiding: raids::Raiding,
    pub(in crate::game) attack_ticks: u32,
    /// The main hand's item at the last player tick, whose attack damage and
    /// speed modifiers apply (`LivingEntity.detectEquipmentUpdates`): a hotbar
    /// swap just before an attack keeps them, with the new item's enchantments
    /// (attribute swapping).
    pub(in crate::game) attack_item: Option<Arc<str>>,
    /// Mobs a player hurt (`LivingEntity.lastHurtByPlayer`), by player UUID.
    pub(in crate::game) last_hurt_by_player: HashMap<u32, combat::PlayerCredit>,
    /// Leashed mobs a player killed, with the killer's UUID: the lead drops
    /// on the corpse's next tick (`Leashable.tickLeash`), for the mods. With
    /// the corpse's dimension: each level's tick prunes only its own.
    pub(in crate::game) lead_killers: HashMap<u32, (u128, Dim)>,
    /// Animals a player fed into love (`Animal.loveCause`), with the guest it was by
    /// connection (None: the host). Breeding credits them, wherever they are.
    pub(in crate::game) love_cause: HashMap<u32, Option<u32>>,
    /// Mobs whose death (`LivingEntity.die`) has been handled.
    /// Looked up per mob each tick: a fast hasher (never iterated).
    pub(in crate::game) died: sunrise_client::fast_hash::FxHashSet<u32>,
    /// The crossbow charge's start and middle loading sounds played (`game/items/crossbow.rs`):
    /// each charge plays each once, however far a mod's tick moved the charge at a time.
    pub(in crate::game) crossbow_sounds: [bool; 2],
    /// The shield's item cooldown after an axe disabled it (ticks).
    pub(in crate::game) shield_cooldown: u32,
    /// The source of a command run through /execute (the player otherwise).
    pub(in crate::game) command_source: Option<commands::execute::Source>,
    pub(in crate::game) native_command_filter: bool,
    /// `MinecraftServer.ticksUntilAutosave`: the ticks left before the next
    /// autosave, counted down a tick at a time.
    pub(in crate::game) ticks_until_autosave: i64,
    pub(in crate::game) applied_modifiers: Vec<(String, String)>,
    pub(in crate::game) xp_delay: u32,
    pub(in crate::game) ordered: ordered_tick::OrderedTicks,
    pub(in crate::game) level_up_sound: sunrise_client::entities::LevelUpSound,
    /// Swings of uses whose client predicts them (`SwingSource.PREDICTED`):
    /// a server does not send these back to the player who swung.
    pub(in crate::game) predicted_swings: u32,
    /// The beds players sleep in that this game marked `occupied`, by level
    /// (game/blocks/beds.rs).
    pub(in crate::game) player_beds: Vec<(Dim, [i32; 3])>,
    /// `SleepStatus`'s active and sleeping players at the last announcement.
    pub(in crate::game) sleep_seen: (i64, i64),
    pub(in crate::game) world_root: PathBuf,
    pub(in crate::game) carrier_retention_snapshot: Vec<(u128, u32)>,
    /// Offline-to-account UUID migration: loaded entities retain the old UUID until saved.
    pub(in crate::game) owner_migration: Option<(u128, u128)>,
    /// The world's data packs' content tables, when it has packs (`None`
    /// keeps `Resources.rest`'s own in use).
    pub(in crate::game) world_tables: Option<WorldTables>,
    /// Server-owned loot and item snapshots, independent of client preloads.
    pub(in crate::game) server_loot: sunrise_client::structure_blocks::LootSnapshot,
    /// The world is the Debug preset (`Level.isDebug`, for every dimension): its blocks
    /// cannot be changed and nothing ticks.
    pub(in crate::game) world_debug: bool,
    /// A server's players on their way to another level, each waiting for
    /// the destination to load around them.
    pub(in crate::game) server_trips: Vec<server_travel::Trip>,
    /// Players a command sends to another level, once it has run.
    pub(in crate::game) level_teleports: Vec<server_travel::LevelTeleport>,
    /// Teleports a command made inside a level, for `native_player_teleported`
    /// once the player's move has reached their client (the put-back at the end of `guest_turn`).
    pub(in crate::game) teleport_notes: Vec<server_travel::TeleportNote>,
    /// Which pass of a server tick runs (`tick_server`): the first one reads
    /// the network, the last one ends the players' tick.
    pub(in crate::game) level_pass: levels::Pass,
    /// The respawn point when it is not the world spawn or a bed: a respawn
    /// anchor (`game/blocks/respawn_anchor.rs`).
    pub(in crate::game) respawn_config: Option<respawn_anchor::RespawnConfig>,
    /// The next enter_dimension call comes from an End portal.
    pub(in crate::game) end_portal_travel: bool,
    pub(in crate::game) tick_scratch: TickScratch,
    pub(in crate::game) tick_chunks: sunrise_client::tick_chunk_membership::TickChunkMembership,
    pub(in crate::game) tickets: sunrise_client::ticket_levels::ChunkTickets,
    pub(in crate::game) random_tick_sections:
        sunrise_client::random_tick_sections::RandomTickSections,
    /// Worn armor as of the last equipment check (frost walker).
    pub(in crate::game) location_armor: [Option<ItemStack>; 4],
    /// The boats a mob's lead can hang on, for this tick's mob ticks.
    pub(in crate::game) leash_boats: Vec<(u32, sunrise_client::mobs::leash::HolderState)>,
    /// The boats and minecarts `native_vehicle_moved` follows (lan_vehicles.rs).
    pub(in crate::game) watched_vehicles: Vec<lan_vehicles::WatchedVehicle>,
    /// The ender dragon fight (game/mobs/dragon.rs).
    pub(in crate::game) fight: dragon::Fight,
    /// The entity ids a hosted server gives the knots its mobs hang on, by
    /// block, while a lead is tied there.
    pub(in crate::game) knot_ids: HashMap<[i32; 3], u32>,
    /// Fence posts whose natural knot was killed and whose leads drop next tick.
    pub(in crate::game) broken_knots: Vec<[i32; 3]>,
    /// This window's player's place in `riding`'s passengers, as `Guest.vehicle_order` is a guest's.
    pub(in crate::game) riding_order: i64,
    /// Vehicles that gained a rider this tick whose `START_RIDING_TRIGGER` was
    /// not fired yet (`game/entities/boarding.rs`), for a guest's turn.
    pub(in crate::game) started_riding: Vec<u32>,
    /// A rotation-only write (`/rotate`, `Entity.forceSetRotation`) to the player of this
    /// turn: the put-back at the end of `guest_turn` sends it as `ClientboundPlayerRotationPacket`, not a teleport.
    pub(in crate::game) rotation_forced: bool,
    /// The vehicle the player's file names, waiting for it to be in the world
    /// (`ServerPlayer.loadAndSpawnParentVehicle`).
    pub(in crate::game) pending_vehicle: Option<[i32; 4]>,
    /// The ender inventory (27 slots) and the ender chest showing it.
    pub(in crate::game) ender_items: Vec<Option<ItemStack>>,
    pub(in crate::game) ender_open: Option<(i32, i32, i32)>,
    /// Item entities spawned and block entities removed while the block
    /// behaviour ran (spilled contents, popped resources).
    pub(in crate::game) spill: spill::Spill,
    pub(in crate::game) creaking: creaking::CreakingState,
    /// Reused candidate buffers for natural spawning.
    pub(in crate::game) spawn_scratch: natural_spawning::SpawnScratch,
    pub(in crate::game) hoppers: HashMap<(i32, i32, i32), machines::HopperState>,
    /// The source of the running dispenser behaviour, including across mod callbacks.
    pub(in crate::game) dispense_source: Option<dispenser::DispenseSource>,
    /// Open mod menus whose `menu_slot_changed` dispatch is currently on the call stack.
    pub(in crate::game) slot_changed_active: Vec<slot_changes::SlotChangedTarget>,
    /// Backing blocks whose contents notice is on the call stack, innermost last.
    pub(in crate::game) contents_changing: Vec<(mod_api::LevelRef, BlockPos)>,
    /// The players with each mod container open, by level (`tick_mod_container_viewers`).
    pub(in crate::game) mod_container_viewers:
        HashMap<(&'static str, (i32, i32, i32)), mod_container_openers::ModContainerOpeners>,
    /// Where item entities are, while the block entities tick (hoppers).
    pub(in crate::game) item_columns: Option<machines::ItemColumns>,
    /// The item columns for one pass of the players' pickups
    /// (`tick_guests`), rebuilt after any write to the items (a pickup, a
    /// command, an explosion): see `item_columns`.
    pub(in crate::game) pickup_columns: Option<machines::StampedItemColumns>,
    /// Where the mobs are, while the block entities tick (potent sulfur gas).
    pub(in crate::game) pass_mobs: Option<potent_sulfur::PassMobs>,
    /// The mobs lent to the hook calls running now, before they join a level
    /// (game/modding/native_unspawned_mob.rs).
    pub(in crate::game) lent_mobs: RefCell<native_unspawned_mob::LentMobs>,
    /// Entities whose copies are held out of storage; their handles refuse until stored back.
    pub(in crate::game) held_out: Vec<u32>,
    /// What the boats' and minecarts' ticks know about the mobs: their box reach
    /// (`vehicle_mob_reach`) and a grid of their boxes (`mobs_meeting`).
    pub(in crate::game) vehicle_mobs: boat_passengers::VehicleMobs,
    /// Missing profession IDs already reported for this world session.
    pub(in crate::game) missing_professions: HashSet<String>,
    /// What changed in entities' colliders this tick, for the players who track them.
    pub(in crate::game) collider_changes: Vec<native_colliders::ColliderChange>,
    pub(in crate::game) entity_initialization: entity_initialization::EntityInitialization,
    /// The shapes of the open world's levels, from its dimension types.
    pub(in crate::game) level_shapes: sunrise_client::chunk::LevelShapes,
    /// `IntegratedServer`'s single-player profile, fixed when the server thread is created,
    /// before its startup hooks (game/modding/native_singleplayer_owner.rs). None elsewhere.
    pub(in crate::game) singleplayer_profile:
        Option<native_singleplayer_owner::SingleplayerProfile>,
    /// `StructureTemplateManager.structureRepository`: the templates in memory
    /// by id, a miss included (game/administration/structure_blocks.rs).
    pub(in crate::game) structure_templates:
        HashMap<String, Option<structure_blocks::TemplateEntry>>,
    pub(in crate::game) structure_spawns: sunrise_client::mobs::spawn_selection::StructureSpawns,
    /// The game tests read so far, by id (the test instance registry).
    pub(in crate::game) test_defs:
        std::cell::RefCell<HashMap<String, Option<test_instances::TestDef>>>,
    /// `test_defs` holds the world's whole test instance registry
    /// (`snapshot_test_defs`), so a miss is final.
    pub(in crate::game) test_defs_snapshot: bool,
    /// `GameTestTicker.SINGLETON`: the game tests running on this server.
    pub(in crate::game) game_tests: game_tests::GameTests,
    /// `ServerChunkCache.lastSpawnState` of each level: its spawnable chunks
    /// and each `MobCategory`'s count.
    pub(in crate::game) spawn_counts: HashMap<Dim, (i32, [i32; 8])>,
    /// The natural spawning policy mods set on each level (`LevelRef::set_spawn_limits`,
    /// `set_spawn_flags`); a level with none spawns as vanilla. Kept while the world is open.
    pub(in crate::game) spawn_policies:
        HashMap<mod_api::DimensionKey, natural_spawning::SpawnPolicy>,
    /// Scheduled end of the server tick (`MinecraftServer.haveTime`).
    pub(in crate::game) server_tick_deadline: Option<Instant>,
    /// The block a guest is acting on (game/network/lan_actions/interaction.rs).
    pub(in crate::game) forced_hit: Option<Hit>,
    /// Non-window simulation's tick-local block ray. Windows use client_pick.
    pub(in crate::game) tick_hit: std::cell::Cell<Option<Option<Hit>>>,
    /// Non-window simulation's tick-local mob ray, including the held attack range.
    pub(in crate::game) tick_mob: Option<Option<(usize, f64, [f64; 3])>>,
    /// The part of a right click a guest's packet asks for (game/network/lan_actions/interaction.rs).
    pub(in crate::game) guest_use: Option<GuestUse>,
    /// The last `use_hand` that ended the click did so with `FAIL`, which
    /// does not consume the action (handleUseItemOn's ANY_BLOCK_USE).
    pub(in crate::game) use_failed: bool,
    pub(in crate::game) block_use_original: Vec<(Hand, Option<ItemStack>)>,
    /// Level events of this tick for the guests, with their excluded source.
    pub(in crate::game) lan_events: Vec<QueuedLevelEvent>,
    /// Jukebox level events 1010 (song name) and 1011 (`None`) for the guests;
    /// the song's network id is only known once the hub is in hand.
    pub(in crate::game) lan_songs: Vec<([i32; 3], Option<String>)>,
    pub(in crate::game) lan_cushion_breaks: Vec<(sunrise_world::registry::BlockStateId, [f64; 3])>,
    /// Block events (position, b0, b1, state) and entity events (id, event) for the guests.
    pub(in crate::game) lan_block_events: Vec<([i32; 3], i32, i32, u32)>,
    pub(in crate::game) lan_entity_events: Vec<(i32, i8)>,
    /// Successful crits (attacking guest, hit entity), queued while the hub is taken.
    pub(in crate::game) lan_critical_hits: Vec<(Option<u32>, i32, bool)>,
    /// Spear stabs that landed (the stabbing guest), queued while the hub is
    /// taken: `broadcastEntityEvent(player, 2)`.
    pub(in crate::game) lan_kinetic_hits: Vec<Option<u32>>,
    /// `LivingEntity.recentKineticEnemies`: when each player (a guest, or the
    /// window's own) last stabbed each entity this use.
    pub(in crate::game) stabbed: HashMap<(Option<u32>, u32), i64>,
    /// The other players the acting player's spear can reach this turn,
    /// taken from the hub before it (`spear_players`).
    pub(in crate::game) stab_players: Vec<spear_players::StabPlayer>,
    /// Spear stabs on players worked out in a guest's turn, landed once the
    /// hub is back (`land_player_stabs`).
    pub(in crate::game) player_stabs: Vec<spear_players::StabBatch>,
    /// The movement a guest's client reported this tick
    /// (`ServerPlayer.getKnownMovement`), for their turn.
    pub(in crate::game) known_movement: Vec3,
    /// `LivingEntity.lastKineticHitFeedbackTime` of the entities that felt a
    /// spear's hit (`spear::OWN` is this window's player), in game time.
    pub(in crate::game) kinetic_feedback: HashMap<i32, i64>,
    /// Global level events for every guest (`ServerLevel.globalLevelEvent`), with their level.
    pub(in crate::game) lan_global_events: Vec<(&'static str, i32, [i32; 3], i32)>,
    /// `ContainerOpenersCounter`: how many players (the host and the guests)
    /// have each chest open.
    pub(in crate::game) lid_openers: HashMap<(i32, i32, i32), i32>,
    pub(in crate::game) lan_acting: Option<u32>,
    /// A mod asked for the command trees again (`native_refresh_commands`):
    /// the next server tick asks every mod for its commands and resends them.
    pub(in crate::game) native_commands: native_server::NativeCommandState,
    /// The abilities of the server player whose turn it is (`as_guest`): their record's
    /// copy, which the turn reads and writes and the put-back at the end of `guest_turn` stores. None outside a turn.
    pub(in crate::game) acting_abilities: Option<sunrise_client::lan::abilities::PlayerAbilities>,
    /// Other players in the current guest turn, including borrowed ancestor bodies.
    pub(in crate::game) lan_building_boxes: building_players::BuildingPlayers,
    /// A guest's `/save-all`: saved once their turn ends (`run_guest_command`),
    /// when their own state is back in their record.
    pub(in crate::game) save_after_guest_turn: bool,
    /// Guests whose records are displaced by nested player swaps, outer first.
    pub(in crate::game) lan_guest_swaps: Vec<u32>,
    /// Bodies and their statistics displaced by guest turns, outer first.
    pub(in crate::game) lan_displaced_players: Vec<lan_actions::DisplacedPlayer>,
    /// Connections whose turn a panic interrupted this tick: the turn helpers record the
    /// player before re-raising, the end-of-turn path records it before its callbacks run and
    /// clears it after a clean put-back. Cleared at the start of every server tick; the crash
    /// save leaves these players' files as last saved (`server_lifecycle`).
    pub(in crate::game) interrupted_turns: Vec<u32>,
    /// What teardown decided about the interrupted turns, for the writes after it.
    pub(in crate::game) crash_save: server_lifecycle::CrashSave,
    /// Ender chests a mod opened for a guest inside that guest's turn
    /// (`player_open_ender_chest`), by connection, with their titles: opened as the turn
    /// ends, once the guest's own ender chest is back in their record.
    pub(in crate::game) lan_ender_menus: Vec<(u32, sunrise_core::nbt::Tag)>,
    /// The block the acting player is breaking (`break_block`): its destroy
    /// event, and the ones before it, are the player's own (`Level.levelEvent`
    /// with the player excluded); the events after it are not.
    pub(in crate::game) lan_break_pos: Option<[i32; 3]>,
    /// Where a player's break puts the block's own drops instead of popping
    /// them (`PlayerRef::destroy_block` with `into_inventory`).
    pub(in crate::game) break_collect: Option<Vec<ItemStack>>,
    /// The name of the guest `lan_acting` acts for (`Player.getName`).
    pub(in crate::game) lan_acting_name: Option<String>,
    /// Their UUID (`Player.getUUID`).
    pub(in crate::game) lan_acting_uuid: Option<u128>,
    /// Where the mods refused to prime TNT: the sound and game event the level
    /// queued right after the prime are dropped (`prime_tnt_event`).
    pub(in crate::game) tnt_refused: Option<BlockPos>,
    /// Their permission level: an op's above 0 (`PlayerList.isOp`).
    pub(in crate::game) lan_acting_permission: Option<u8>,
    /// The spawn protection radius that holds the acting guest back
    /// (`DedicatedServer.isUnderSpawnProtection` before the position test):
    /// None for an operator, without operators, or off a dedicated server.
    pub(in crate::game) lan_acting_spawn_protection: Option<i32>,
    /// Their entity id, which the hub (often taken out of `lan` while they
    /// act) would otherwise have to be asked for.
    pub(in crate::game) lan_acting_entity: Option<u32>,
    /// Whether the player currently swapped into the game is connectionless.
    pub(in crate::game) lan_acting_fake: bool,
    /// A fake player's `perform_action` in progress, and what it refused.
    pub(in crate::game) fake_action: std::sync::Arc<native_server::FakeAction>,
    /// Command feedback to forward to a guest.
    pub(in crate::game) chat_capture: Option<Vec<sunrise_client::text::Text>>,
    /// Packets for the guest acting now, sent once its action returns.
    pub(in crate::game) guest_packets: Vec<sunrise_protocol::packets::play::Clientbound>,
    /// The acting guest's cooldowns, as (item, ticks): their packet needs the
    /// hub's item registry, which the guest's turn does not hold.
    pub(in crate::game) guest_cooldowns: Vec<(Arc<str>, i32)>,
    /// The acting guest's own entity events (`ServerPlayer.connection.send`),
    /// sent with their entity id, which the guest's turn does not hold.
    pub(in crate::game) guest_entity_events: Vec<i8>,
    /// The acting guest's equipment breaks (`onEquippedItemBroken`'s entity
    /// events), broadcast with their entity id once their turn ends.
    pub(in crate::game) guest_equipment_breaks: Vec<i8>,
    /// Block sounds queued by the acting guest, excluding that guest.
    pub(in crate::game) guest_block_sounds: Vec<(String, [f64; 3], f32, f32)>,
    /// `Player.playSound` of the acting guest (event, position, volume,
    /// pitch): `level.playSound(player, ...)` leaves the guest out, whose
    /// own client plays it.
    pub(in crate::game) guest_player_sounds: Vec<(String, [f64; 3], f32, f32)>,
    /// Sounds bound to an entity this tick, for the players (`game/interface/level_sounds.rs`).
    pub(in crate::game) bound_sounds: Vec<level_sounds::BoundSound>,
    /// `broadcastToAdmins` lines for the other operators that a command queued,
    /// sent once it returns (`send_admin_feedback`), each with the guest whose
    /// command it was (`commandSource() != source`): None for a block, a
    /// minecart, the console or the host.
    pub(in crate::game) admin_feedback: Vec<(sunrise_client::text::Text, Option<u32>)>,
    /// The server console is running a command: whether its feedback goes to
    /// the operators (`DedicatedServer.shouldInformAdmins`,
    /// `broadcast-console-to-ops`).
    pub(in crate::game) console_broadcast: Option<bool>,
    /// The command the console is running came over RCON: its source is named `Rcon`
    /// (`RconConsoleSource.createCommandSourceStack`), not `Server`.
    pub(in crate::game) rcon_source: bool,
    /// A dedicated server's operator, whitelist and ban lists, for `/op`,
    /// `/ban` and friends (`DedicatedPlayerList`).
    pub(in crate::game) server_lists: Option<std::sync::Arc<std::sync::Mutex<dedicated::Lists>>>,
    /// State of `/spectate`, `/posteffect`, `/debug`, `/waypoint` and friends.
    pub(in crate::game) cmds: commands::extra::CommandExtras,
    pub(in crate::game) combat: combat::CombatTracker,
    /// Each guest's own `CombatTracker`, out of `combat` between turns.
    pub(in crate::game) guest_combat: std::collections::HashMap<u32, combat::CombatTracker>,
    pub(in crate::game) fns: functions::FnState,
    pub(in crate::game) bosses: bossbars::BossState,
    pub(in crate::game) exec: commands::execute::ExecState,
    pub(in crate::game) rand_sequences: sunrise_client::random_sequences::RandomSequences,
    /// `BrushableBlockEntity`'s brush counts of the blocks being brushed.
    pub(in crate::game) brushing: brush::Brushing,
    pub(in crate::game) restore_player_state: bool,
}

impl ServerState {
    // Kept out-of-line so its field temporaries do not bloat `Game::construct`'s unoptimised frame.
    #[inline(never)]
    pub(in crate::game) fn new(resources: &std::path::Path) -> Self {
        Self {
            structure_spawns: sunrise_client::mobs::spawn_selection::StructureSpawns::load(
                resources,
            ),
            data_map_registries: Vec::new(),
            craft_table: None,
            open_container_slots: None,
            destroy_delay: 0,
            world_adopted: false,
            startup_ticket_restore_pending: false,
            paused_mob_hurt: None,
            next_entity: 0,
            ticked_entities_through: u32::MAX,
            contact_drops: 0..0,
            lectern_pulses: Vec::new(),
            spawn_origin: None,
            spawn_hook_fired: false,
            spawn_hook_cancelled: false,
            spawn_spiral: Default::default(),
            player_spawn: None,
            spawn_wait: Vec::new(),
            raiding: Default::default(),
            attack_ticks: 100,
            attack_item: None,
            last_hurt_by_player: HashMap::new(),
            lead_killers: HashMap::new(),
            love_cause: HashMap::new(),
            died: Default::default(),
            crossbow_sounds: [false; 2],
            shield_cooldown: 0,
            command_source: None,
            native_command_filter: false,
            // `computeNextAutosaveInterval` at the clock's 20 ticks a second.
            ticks_until_autosave: 6000,
            applied_modifiers: Vec::new(),
            xp_delay: 0,
            ordered: Default::default(),
            level_up_sound: Default::default(),
            predicted_swings: 0,
            player_beds: Vec::new(),
            sleep_seen: (0, 0),
            world_root: PathBuf::new(),
            carrier_retention_snapshot: Vec::new(),
            owner_migration: None,
            world_debug: false,
            server_trips: Vec::new(),
            level_teleports: Vec::new(),
            teleport_notes: Vec::new(),
            level_pass: Default::default(),
            respawn_config: None,
            end_portal_travel: false,
            tick_scratch: TickScratch::default(),
            tick_chunks: Default::default(),
            tickets: Default::default(),
            random_tick_sections: Default::default(),
            location_armor: Default::default(),
            leash_boats: Vec::new(),
            watched_vehicles: Vec::new(),
            fight: dragon::Fight::default(),
            knot_ids: Default::default(),
            broken_knots: Vec::new(),
            riding_order: 0,
            started_riding: Vec::new(),
            rotation_forced: false,
            pending_vehicle: None,
            ender_items: vec![None; 27],
            ender_open: None,
            spill: Default::default(),
            creaking: Default::default(),
            spawn_scratch: Default::default(),
            hoppers: HashMap::new(),
            dispense_source: None,
            slot_changed_active: Vec::new(),
            contents_changing: Vec::new(),
            mod_container_viewers: HashMap::new(),
            item_columns: None,
            pickup_columns: None,
            pass_mobs: None,
            lent_mobs: Default::default(),
            held_out: Vec::with_capacity(4),
            vehicle_mobs: Default::default(),
            missing_professions: Default::default(),
            collider_changes: Vec::new(),
            entity_initialization: Default::default(),
            level_shapes: Default::default(),
            singleplayer_profile: None,
            structure_templates: HashMap::new(),
            test_defs: std::cell::RefCell::new(HashMap::new()),
            test_defs_snapshot: false,
            game_tests: game_tests::GameTests::default(),
            spawn_counts: HashMap::new(),
            spawn_policies: HashMap::new(),
            server_tick_deadline: None,
            forced_hit: None,
            tick_hit: Default::default(),
            tick_mob: None,
            guest_use: None,
            use_failed: false,
            block_use_original: Vec::new(),
            lan_events: Vec::new(),
            lan_songs: Vec::new(),
            lan_cushion_breaks: Vec::new(),
            lan_block_events: Vec::new(),
            lan_entity_events: Vec::new(),
            lan_critical_hits: Vec::new(),
            lan_kinetic_hits: Vec::new(),
            stabbed: HashMap::new(),
            stab_players: Vec::new(),
            player_stabs: Vec::new(),
            known_movement: Vec3::ZERO,
            kinetic_feedback: HashMap::new(),
            lan_global_events: Vec::new(),
            lid_openers: HashMap::new(),
            lan_acting: None,
            acting_abilities: None,
            native_commands: Default::default(),
            lan_building_boxes: Default::default(),
            save_after_guest_turn: false,
            lan_guest_swaps: Vec::new(),
            lan_displaced_players: Vec::new(),
            interrupted_turns: Vec::new(),
            crash_save: Default::default(),
            lan_ender_menus: Vec::new(),
            lan_break_pos: None,
            break_collect: None,
            guest_packets: Vec::new(),
            guest_cooldowns: Vec::new(),
            guest_entity_events: Vec::new(),
            guest_equipment_breaks: Vec::new(),
            guest_block_sounds: Vec::new(),
            guest_player_sounds: Vec::new(),
            bound_sounds: Vec::new(),
            lan_acting_name: None,
            lan_acting_uuid: None,
            tnt_refused: None,
            lan_acting_permission: None,
            lan_acting_spawn_protection: None,
            lan_acting_entity: None,
            lan_acting_fake: false,
            fake_action: Default::default(),
            chat_capture: None,
            admin_feedback: Vec::new(),
            console_broadcast: None,
            rcon_source: false,
            server_lists: None,
            world_tables: None,
            server_loot: Default::default(),
            cmds: Default::default(),
            combat: Default::default(),
            guest_combat: Default::default(),
            fns: Default::default(),
            bosses: Default::default(),
            exec: Default::default(),
            rand_sequences: Default::default(),
            brushing: Default::default(),
            restore_player_state: true,
        }
    }
}
