//! A server's levels, one per dimension (`MinecraftServer.levels`).
//!
//! The game's tick code reads one level: the one in place, held in `Game`'s
//! own fields (the world, its streaming and save, block ticks, entities).
//! A server (the dedicated server and single player's server thread) keeps
//! the other dimensions' levels here and ticks each one with players or forced
//! chunks by swapping it into place (`enter_level`), the way `as_guest` swaps a player
//! in, so the tick code runs unchanged, one level at a time.
//!
//! Shared by every level, as vanilla's derived levels share the
//! Overworld's level data: the game time, the day time and the weather, the
//! game rules, the scoreboard, the maps and the entities waiting to arrive
//! through a portal.

use super::entity_residency::{define_resident_access, resident_owners};
use super::{
    Dim, ExperienceOrb, FallingBlock, ItemEntity, Mob,
    boats_common::Boat,
    creaking::CreakingState,
    dragon_common,
    dragon_parts_table::DragonSlot,
    entity_memory::{EntityMemory, OffLevel},
    fireworks,
    lightning_common::Bolt,
    minecarts,
    ominous_item_spawner_common::OminousItemSpawner,
    raids::Raiding,
    sculk_server::SculkState,
    tnt_common::PrimedTnt,
};
use sunrise_gameplay_common::game::commands_common::{OtherEntity, Target};
use sunrise_gameplay_common::game::residency_tick::ResidencyTickGameExt as _;
use mod_api::DimensionKey;
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use sunrise_game_data::Result;
use sunrise_gameplay_common::{hoppers::HopperState, leash_knot::Knot};
use sunrise_level::spans::time as span;
use sunrise_level::world::World;
use sunrise_level::{block_entity_ticks::BlockEntityTicks, chunk_streamer::ChunkStreamer};
use sunrise_sim::join_seq::{
    Joined,
    residency::{
        Journal, Kind, Owner,
        table::{EntityLookup, Location, SourceEntity},
    },
};
use sunrise_sim::thrown_trident::ThrownTrident;
use sunrise_sim::{
    random_tick_sections::RandomTickSections,
    save::SaveDir,
    tick_chunk_membership::TickChunkMembership,
    ticket_levels::{ChunkTickets, PortalTicket},
    vanillagen::VanillaTerrain,
};
use sunrise_world::level::{LevelEvent, Ticks};

const BUILTIN_LEVEL_ORDER: [Dim; 3] = [Dim::Overworld, Dim::Nether, Dim::End];

/// A borrowed counting view: no resident-table scans and no allocation for gauges.
pub(in crate::game) struct EntityCountView<'a> {
    pub(in crate::game) fixed: [(&'static str, usize); 19],
    pub(in crate::game) mobs: &'a [Mob],
    pub(in crate::game) projectiles: &'a [sunrise_sim::projectiles::Projectile],
    pub(in crate::game) general: &'a super::general::GeneralRows,
    pub(in crate::game) others: &'a [OtherEntity],
}
impl EntityCountView<'_> {
    pub(in crate::game::server_side) fn total(&self) -> usize {
        self.fixed.iter().map(|(_, n)| n).sum::<usize>()
            + self.projectiles.len()
            + self.mobs.len()
            + self.general.len()
            + self.others.len()
    }
    pub(in crate::game::server_side) fn registry_counts(&self) -> Vec<(String, u32)> {
        let mut counts = std::collections::BTreeMap::<String, u32>::new();
        for &(id, n) in &self.fixed {
            if n > 0 {
                *counts.entry(id.to_owned()).or_default() += n as u32;
            }
        }
        for projectile in self.projectiles {
            *counts.entry(projectile.kind.id().to_owned()).or_default() += 1;
        }
        for mob in self.mobs.iter().filter(|mob| !mob.removed) {
            *counts.entry(mob.kind.id.clone()).or_default() += 1;
        }
        for row in self.general.iter() {
            *counts
                .entry(
                    super::general::general_type_catalog()
                        .key(row.type_slot)
                        .to_owned(),
                )
                .or_default() += 1;
        }
        for row in self.others {
            *counts.entry(row.kind.clone()).or_default() += 1;
        }
        counts.into_iter().collect()
    }
}

// Both Game and Parked own these typed families. Keep their instrumentation in one list.
macro_rules! entity_count_view {
    ($level:expr, $projectiles:expr, $general:expr, $knots:expr, $others:expr) => {
        crate::game::levels::EntityCountView {
            fixed: [
                ("minecraft:item", $level.dropped.len()),
                ("minecraft:experience_orb", $level.orbs.len()),
                ("minecraft:minecart", $level.carts.len()),
                ("minecraft:boat", $level.boats.len()),
                ("minecraft:falling_block", $level.falling.len()),
                ("minecraft:painting", $level.paintings.len()),
                ("minecraft:item_frame", $level.item_frames.len()),
                ("minecraft:armor_stand", $level.armor_stands.len()),
                ("minecraft:trident", $level.tridents.len()),
                ("minecraft:firework_rocket", $level.rockets.len()),
                ("minecraft:tnt", $level.tnts.len()),
                ("minecraft:end_crystal", $level.crystals.len()),
                ("minecraft:leash_knot", $knots.len()),
                (
                    "minecraft:ominous_item_spawner",
                    $level.ominous_spawners.len(),
                ),
                ("minecraft:cushion", $level.cushions.len()),
                ("minecraft:area_effect_cloud", $level.breath_clouds.len()),
                ("minecraft:lightning_bolt", $level.bolts.len()),
                ("minecraft:evoker_fangs", $level.fangs.perf_count()),
                // Multipart hitboxes are not independently admitted world entities.
                (
                    "minecraft:ender_dragon",
                    usize::from($level.dragon.as_ref().is_some()),
                ),
            ],
            mobs: &$level.mobs,
            projectiles: $projectiles,
            general: $general,
            others: $others,
        }
    };
}
pub(in crate::game) use entity_count_view;

/// Server ticks a custom level stays loaded with no player or ticket holding
/// it, before it is saved and closed. Built-in levels (Overworld, Nether, End)
/// stay open until server shutdown and only unload unticketed chunks while idle.
pub const IDLE_TICKS: u32 = 300;

/// What one server tick's idle check asks for a parked level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum IdleAction {
    /// Still active, or already drained and retained (built-in), or below the
    /// custom-level close threshold, or saving is disabled (`/save-off`).
    Keep,
    /// Built-in level with no players or tickets that still holds unticketed
    /// chunks or streaming work to unload while the level itself stays open.
    UnloadChunks,
    /// Custom level that has been idle for at least [`IDLE_TICKS`]: save and close.
    Close,
}

/// Vanilla 26.3 only services connections while the server is paused (escape
/// pause or `pause-when-empty-seconds`), so parked-level draining and idle
/// closes are skipped then, but still run during `/tick freeze` (`paused == false`).
fn should_run_idle_maintenance(paused: bool) -> bool {
    !paused
}

fn step_parked_idle(
    dim: Dim,
    idle: &mut u32,
    active: bool,
    has_unloadable_chunks: bool,
    saving_enabled: bool,
) -> IdleAction {
    if active {
        *idle = 0;
        return IdleAction::Keep;
    }
    *idle = idle.saturating_add(1);
    if !saving_enabled {
        return IdleAction::Keep;
    }
    if dim.is_builtin() {
        if has_unloadable_chunks {
            IdleAction::UnloadChunks
        } else {
            IdleAction::Keep
        }
    } else if *idle >= IDLE_TICKS {
        IdleAction::Close
    } else {
        IdleAction::Keep
    }
}

/// Recheck a parked custom level right before closing it: a listener during an
/// earlier built-in drain or custom close can force a chunk in this level or
/// disable saving (`/save-off`).
fn custom_close_still_eligible(
    dim: Dim,
    idle: &mut u32,
    active: bool,
    saving_enabled: bool,
) -> bool {
    debug_assert!(!dim.is_builtin());
    if active {
        *idle = 0;
        return false;
    }
    saving_enabled && *idle >= IDLE_TICKS
}

/// A level that is not in place: every per-level field of `Game`.
pub(in crate::game) struct Parked {
    pub(in crate::game) world: World,
    streaming: ChunkStreamer,
    locator: Option<Arc<VanillaTerrain>>,
    save: Arc<SaveDir>,
    block_ticks: Ticks,
    tick_chunks: TickChunkMembership,
    tickets: ChunkTickets,
    random_tick_sections: RandomTickSections,
    level_events: Vec<LevelEvent>,
    /// Entity portal processes and cooldowns belong to this level's entities.
    pub(in crate::game) portal_states: HashMap<u32, sunrise_sim::nether_portal::PortalState>,
    falling: sunrise_sim::join_seq::JoinList<FallingBlock, false>,
    paintings: sunrise_sim::join_seq::JoinList<sunrise_sim::entities::Painting>,
    item_frames: sunrise_sim::join_seq::JoinList<sunrise_sim::entities::ItemFrame>,
    armor_stands: sunrise_sim::join_seq::JoinList<sunrise_sim::entities::ArmorStand>,
    cushions: sunrise_sim::join_seq::JoinList<sunrise_sim::entities::Cushion>,
    others: sunrise_sim::join_seq::ResidentList<OtherEntity>,
    fangs: sunrise_gameplay_common::game::fangs_common::FangState,
    pub(in crate::game) boats: sunrise_sim::join_seq::JoinList<Boat, false>,
    pub(in crate::game) carts: sunrise_sim::join_seq::JoinList<minecarts::Minecart, false>,
    bolts: sunrise_sim::join_seq::ResidentList<Bolt>,
    crystals: sunrise_sim::join_seq::JoinList<dragon_common::Crystal>,
    dragon: DragonSlot,
    breath_clouds: sunrise_sim::join_seq::ResidentList<dragon_common::Breath>,
    tridents: sunrise_sim::join_seq::JoinList<ThrownTrident>,
    rockets: sunrise_sim::join_seq::JoinList<fireworks::Rocket>,
    ominous_spawners: sunrise_sim::join_seq::ResidentList<OminousItemSpawner>,
    tnts: sunrise_sim::join_seq::JoinList<PrimedTnt>,
    general: super::general::GeneralStore,
    hoppers: HashMap<(i32, i32, i32), HopperState>,
    pub(in crate::game) mobs: sunrise_sim::join_seq::JoinList<Mob>,
    /// The entity ids of the level's fence knots (`knot_ids`).
    knot_ids: HashMap<[i32; 3], u32>,
    broken_knots: Vec<[i32; 3]>,
    knots: sunrise_sim::join_seq::JoinList<Knot>,
    projectiles: sunrise_sim::join_seq::JoinList<sunrise_sim::projectiles::Projectile, false>,
    dropped: sunrise_sim::join_seq::JoinList<ItemEntity>,
    orbs: sunrise_sim::join_seq::JoinList<ExperienceOrb>,
    persist: EntityMemory,
    lectern_pulses: Vec<((i32, i32, i32), i32)>,
    lid_openers: HashMap<(i32, i32, i32), i32>,
    lan_events: Vec<super::QueuedLevelEvent>,
    lan_block_events: Vec<([i32; 3], i32, i32, u32)>,
    lan_entity_events: Vec<(i32, i8)>,
    lan_critical_hits: Vec<(Option<u32>, i32, bool)>,
    /// The hub's tracked entities of this level (`LanHost::states`), as its last tick described
    /// them: what its players' moves are checked against before this tick describes them again.
    lan_states: sunrise_sim::lan::entity::States,
    /// Only the per-level parts of these are used (`swap_level`).
    anims: BlockEntityTicks,
    pub(in crate::game) raiding: Raiding,
    sculk: SculkState,
    creaking: CreakingState,
    /// Natural spawning's candidate cache: each level keeps its own players' squares.
    spawn_scratch: super::natural_spawning::SpawnScratch,
    /// Server ticks since a player was last in the level.
    pub(in crate::game) idle: u32,
    /// The level's own world border (`ServerLevel.getWorldBorder`), and what its players were
    /// last told of it (`LanHost::border_sent`).
    border: sunrise_level::worldborder::WorldBorder,
    border_sent: Option<sunrise_sim::lan::border::Sent>,
}

impl Parked {
    resident_owners!(
        define_resident_access,
        self,
        projectiles,
        knots,
        general.rows,
        others
    );
}

impl crate::game::ServerGame<'_> {
    /// Visit exactly the actual physical lifetime slot. Resolution releases the
    /// table and its journal before lending storage; no lift, swap or replacement
    /// occurs, including when the visitor unwinds. Logical visibility is a caller
    /// policy, just as for the underlying owner resolver. The visitor must not
    /// call entity lookup or recursively enter this seam.
    pub fn with_resident_slot<R>(
        &mut self,
        dimension: &mod_api::DimensionKey,
        id: std::num::NonZeroU32,
        visit: impl FnOnce(&mut sunrise_mod_runtime::attachments::AttachmentLifetime) -> R,
    ) -> super::entity_residency::SlotAccess<R> {
        use super::entity_residency::SlotAccess;
        let resolved = {
            self.common.resident_entities
                .table
                .borrow_mut()
                .entity_lookup(dimension, id.get(), |at| {
                    self.locate_server_resident_owner(at)
                })
        };
        let location = match resolved {
            EntityLookup::Found { location, .. } => location,
            EntityLookup::Taken => return SlotAccess::Taken,
            EntityLookup::Missing => return SlotAccess::Missing,
        };
        debug_assert!(self.resident_entities.table.try_borrow_mut().is_ok());
        if matches!(
            location.owner.kind,
            sunrise_sim::join_seq::residency::Kind::Player
                | sunrise_sim::join_seq::residency::Kind::Hook
        ) {
            if *dimension != self.common.dim.key() {
                let dim = match *dimension {
                    DimensionKey::OVERWORLD => Dim::Overworld,
                    DimensionKey::NETHER => Dim::Nether,
                    DimensionKey::END => Dim::End,
                    key => Dim::Extra(key),
                };
                if !self.server.levels.0.parked.contains_key(&dim) {
                    return SlotAccess::Missing;
                }
            }
            return self.with_specialized_resident_slot(location, visit);
        }
        if *dimension == self.common.dim.key() {
            self.with_resident_owner_slot(location, visit)
        } else {
            let dim = match *dimension {
                mod_api::DimensionKey::OVERWORLD => Dim::Overworld,
                mod_api::DimensionKey::NETHER => Dim::Nether,
                mod_api::DimensionKey::END => Dim::End,
                key => Dim::Extra(key),
            };
            self.server
                .levels
                .0
                .parked
                .get_mut(&dim)
                .map_or(SlotAccess::Missing, |parked| {
                    parked.with_resident_owner_slot(location, visit)
                })
        }
    }

    /// Whether the living entity `id` resident in a parked level is alive (`Entity.isAlive`):
    /// a remembered target stays remembered while its holder is in another level.
    pub(in crate::game::server_side) fn parked_living_alive(&self, id: u32) -> bool {
        let EntityLookup::Found { location, value } = self.server_resident_entity(id) else {
            return false;
        };
        let Some(parked) = (self.server.levels.0.parked.iter())
            .find(|(dim, _)| dim.key() == location.owner.dimension)
            .map(|(_, parked)| parked)
        else {
            return false;
        };
        match value {
            Target::Mob(_) => {
                (parked.mobs.locate(location.join)).is_some_and(|i| parked.mobs[i].alive())
            }
            Target::Stand(_) => (parked.armor_stands.locate(location.join))
                .is_some_and(|i| parked.armor_stands[i].health > 0.0),
            Target::Dragon(id) => {
                (parked.dragon.as_ref()).is_some_and(|d| d.id == id && d.health > 0.0)
            }
            _ => false,
        }
    }

    /// `ServerLevel.getEntity(UUID)`: search this level's admitted entities, not unloaded
    /// saves or tracking entries. A departure can overlap a replacement with the same UUID.
    pub(in crate::game::server_side) fn level_target_by_uuid(
        &self,
        dim: Dim,
        uuid: [i32; 4],
    ) -> Option<Target> {
        let dimension = dim.key();
        let mut table = self.common.resident_entities.table.borrow_mut();
        let mut after = None;
        loop {
            let id = table.level_uuid_candidate(dimension, uuid, after)?;
            if let EntityLookup::Found {
                value: (target, true),
                ..
            } = table.entity_lookup(&dimension, id.get(), |at| {
                self.locate_server_resident_owner(at)
            }) && self.resident_source_target_visible(dimension, target)
            {
                return Some(target);
            }
            after = Some(id);
        }
    }

    /// `PersistentEntitySectionManager.isLoaded(UUID)` for the destination level in place,
    /// including the in-place player and any guests in this level, independently of command visibility.
    pub(in crate::game) fn is_loaded_uuid_in_level(&self, uuid: [i32; 4]) -> bool {
        if self.level_target_by_uuid(self.common.dim, uuid).is_some() {
            return true;
        }
        if self.common.resident_entities.local_player.location().is_none()
            && self.server.lan_acting.is_none()
            && self.common.player.is_some()
            && self.common.level.player_uuid == Some(uuid)
        {
            return true;
        }
        self.server
            .lan
            .iter()
            .flat_map(|h| h.hub.guests.iter())
            .any(|g| {
                g.placed
                    && !g.released
                    && Dim::from_id(&g.dimension) == self.common.dim
                    && sunrise_sim::lan::playerdata::uuid_ints(g.uuid) == uuid
            })
    }

    /// Find the canonical level without swapping or probing physical families.
    pub(in crate::game) fn resident_target_dimension(&self, target: Target) -> Option<Dim> {
        let id = self.resident_target_id(target)?;
        let found = match self.server_resident_entity(id) {
            EntityLookup::Found { location, value }
                if value == target
                    || matches!(
                        (value, target),
                        (Target::Player, Target::Guest(_)) | (Target::Guest(_), Target::Player)
                    ) =>
            {
                location
            }
            _ => return None,
        };
        Some(match found.owner.dimension {
            DimensionKey::OVERWORLD => Dim::Overworld,
            DimensionKey::NETHER => Dim::Nether,
            DimensionKey::END => Dim::End,
            key => Dim::Extra(key),
        })
    }

    /// A new epoch invalidates every old producer before rebinding retained levels.
    pub fn reset_entity_residency(&mut self) {
        let table = self.common.resident_entities.table.get_mut();
        table.reset();
        let journal = table.producer();
        self.bind_resident_owners(&journal, self.common.dim.key(), self.common.is_logical_server());
        self.bind_specialized_resident_owners(&journal);
        for (dim, parked) in &mut self.server.levels.0.parked {
            parked.bind_resident_owners(&journal, dim.key(), true);
        }
    }
}

impl crate::game::ServerGame<'_> {
    /// Resolve through the exact owner, never by trying dimension swaps.
    pub(in crate::game) fn locate_server_resident_owner(
        &self,
        at: Location,
    ) -> Option<SourceEntity<(Target, bool)>> {
        if matches!(
            at.owner.kind,
            sunrise_sim::join_seq::residency::Kind::Player
                | sunrise_sim::join_seq::residency::Kind::Hook
        ) {
            return self.locate_specialized_resident_owner(at);
        }
        if at.owner.dimension == self.common.dim.key() {
            // An open transaction's removal reads as gone already.
            return self.locate_resident_owner(at).map(|mut source| {
                source.value.1 &= !self.common.removal_pending(&source.value.0);
                source
            });
        }
        let dim = match at.owner.dimension {
            DimensionKey::OVERWORLD => Dim::Overworld,
            DimensionKey::NETHER => Dim::Nether,
            DimensionKey::END => Dim::End,
            key => Dim::Extra(key),
        };
        // The transactions stay with the game when a level is parked: its pending removals too.
        self.server
            .levels
            .0
            .parked
            .get(&dim)?
            .locate_resident_owner(at)
            .map(|mut source| {
                source.value.1 &= !self.common.removal_pending_in(dim, &source.value.0);
                source
            })
    }

    pub fn server_resident_entity(&self, id: u32) -> EntityLookup<Target> {
        let found = self
            .common.resident_entities
            .table
            .borrow_mut()
            .server_entity_lookup(id, |at| self.locate_server_resident_owner(at));
        match found {
            EntityLookup::Found {
                location,
                value: (value, true),
            } => EntityLookup::Found { location, value },
            EntityLookup::Found {
                value: (_, false), ..
            }
            | EntityLookup::Missing => EntityLookup::Missing,
            EntityLookup::Taken => EntityLookup::Taken,
        }
    }
}

/// Which passes of the server tick the level in place runs: the first
/// reads the network (`Hub::poll`), the last ends the tick for every player
/// (`Hub::end_tick`). A tick with one level is both. `running` is the
/// tick rate manager's answer for this server tick, asked once before the
/// levels (`MinecraftServer.tickServer`); `None` outside `tick_server`.
#[derive(Clone, Copy)]
pub(in crate::game) struct Pass {
    pub first: bool,
    pub last: bool,
    pub running: Option<bool>,
    /// Set once this level's player packets were handled before block ticks.
    pub packets_drained: bool,
}

impl Default for Pass {
    fn default() -> Self {
        Self {
            first: true,
            last: true,
            running: None,
            packets_drained: false,
        }
    }
}

pub(in crate::game) enum ServerTickPhase {
    Level(usize, Dim),
    Connections,
}

/// `MinecraftServer.tickChildren`: finish the level phase before ticking connections.
pub(in crate::game) fn server_tick_phases(levels: &[Dim]) -> impl Iterator<Item = ServerTickPhase> {
    levels
        .iter()
        .copied()
        .enumerate()
        .map(|(index, dim)| ServerTickPhase::Level(index, dim))
        .chain(std::iter::once(ServerTickPhase::Connections))
}

/// How long a server tick took and what it spent the time in.
pub(in crate::game) struct TickTiming {
    pub wall_ms: f32,
    pub cpu_ms: Option<f32>,
    /// Spans recorded inside the tick only, in observation order.
    pub raw: Vec<sunrise_level::spans::Span>,
}

/// The levels not in place, by dimension.
#[derive(Default)]
pub struct Levels {
    pub(in crate::game) parked: HashMap<Dim, Box<Parked>>,
    /// The levels whose open the mods heard (`native_level_load`), each to
    /// be told closed once.
    heard_open: Vec<Dim>,
    /// The levels a mod asked to have saved (`level_request_save`), each once, saved at the start
    /// of the next server tick.
    pub requested_saves: Vec<Dim>,
    /// The levels mods opened while the server runs, and the requests waiting to be applied.
    pub(in crate::game) runtime: super::runtime_levels::RuntimeTable,
    /// The entities that travelled to another level lately: the id each had, to the id it
    /// arrived with and when. A kill credit recorded under the old id follows it there.
    travels: HashMap<u32, (u32, i64)>,
}

impl Levels {
    pub(in crate::game) fn parked_target_visible(&self, dim: Dim, target: &Target) -> Option<bool> {
        let parked = self.parked.get(&dim)?;
        Some(
            !parked.persist.removals.contains(target)
                && !parked.persist.destroying_vehicles.contains(target),
        )
    }

    /// Note that the entity `from` arrived in another level as `to` at `now`. A kill credit
    /// lasts a hundred ticks, so a travel older than that names nothing still credited.
    pub(in crate::game::server_side) fn note_travel(&mut self, from: u32, to: u32, now: i64) {
        self.travels.retain(|_, &mut (_, at)| now - at <= 100);
        self.travels.insert(from, (to, now));
    }

    /// The id the entity `from` arrived with in another level, if it travelled lately.
    pub(in crate::game::server_side) fn travelled(&self, from: u32) -> Option<u32> {
        self.travels.get(&from).map(|&(to, _)| to)
    }

    /// Queue `dim` to be saved at the start of the next server tick; a level already queued is
    /// not queued twice.
    pub fn request_save(&mut self, dim: Dim) {
        if !self.requested_saves.contains(&dim) {
            self.requested_saves.push(dim);
        }
    }

    /// Whether a save of `dim` is still waiting for a tick that admits
    /// saves (the #2488 gate holds it back).
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::game::server_side) fn save_pending(&self, dim: Dim) -> bool {
        self.requested_saves.contains(&dim)
    }

    /// Drop a save of `dim` still waiting for a tick: its level is gone.
    pub(in crate::game::server_side) fn cancel_requested_save(&mut self, dim: Dim) {
        self.requested_saves.retain(|saved| *saved != dim);
    }

    /// Whether mods have already been notified of `dim` opening.
    fn has_heard_open(&self, dim: Dim) -> bool {
        self.heard_open.contains(&dim)
    }

    /// Record `dim` as announced open; returns `true` iff it was not already open.
    fn note_opened(&mut self, dim: Dim) -> bool {
        if self.heard_open.contains(&dim) {
            return false;
        }
        self.heard_open.push(dim);
        true
    }

    /// Record `dim` as announced closed; returns `true` iff it was open.
    fn note_closed(&mut self, dim: Dim) -> bool {
        if let Some(i) = self.heard_open.iter().position(|d| *d == dim) {
            self.heard_open.remove(i);
            true
        } else {
            false
        }
    }

    /// Advance parked built-in levels' idle state and return those that still
    /// hold unticketed chunks or streaming work to drain while saving is enabled.
    fn plan_builtin_drains(
        &mut self,
        is_active: impl Fn(Dim) -> bool,
        saving_enabled: bool,
    ) -> Vec<Dim> {
        let mut unload_chunks = Vec::new();
        for (&dim, p) in &mut self.parked {
            if !dim.is_builtin() {
                continue;
            }
            let active = is_active(dim);
            let has_unloadable_chunks =
                !p.streaming.paused && (!p.world.chunks.is_empty() || p.streaming.has_idle_work());
            if step_parked_idle(
                dim,
                &mut p.idle,
                active,
                has_unloadable_chunks,
                saving_enabled,
            ) == IdleAction::UnloadChunks
            {
                unload_chunks.push(dim);
            }
        }
        unload_chunks
    }

    /// Advance parked custom levels' idle state and return those eligible to
    /// save and close after [`IDLE_TICKS`].
    fn plan_custom_closes(
        &mut self,
        is_active: impl Fn(Dim) -> bool,
        saving_enabled: bool,
    ) -> Vec<Dim> {
        let mut close = Vec::new();
        for (&dim, p) in &mut self.parked {
            if dim.is_builtin() {
                continue;
            }
            let active = is_active(dim);
            // A live `UNKNOWN` ticket keeps its column, so the level stays open (not ticking)
            // until the ticket expires.
            if step_parked_idle(dim, &mut p.idle, active, false, saving_enabled)
                == IdleAction::Close
                && !p.streaming.has_unknown_tickets()
            {
                close.push(dim);
            }
        }
        close
    }

    /// Clear every parked level and dimensions recorded as heard open.
    pub fn clear(&mut self) {
        self.parked.clear();
        self.heard_open.clear();
        self.requested_saves.clear();
        self.runtime.clear();
    }

    pub(in crate::game::server_side) fn abort_parked_generation(&mut self) {
        for level in self.parked.values_mut() {
            level.streaming.abort_generation();
        }
    }

    /// A parked level, without opening or loading it.
    pub(in crate::game::server_side) fn parked_world(&self, dim: Dim) -> Option<&World> {
        self.parked.get(&dim).map(|level| &level.world)
    }

    /// The world of the level not in place whose dimension is `dimension`
    /// (`minecraft:the_nether`), found without collecting the parked levels.
    pub(in crate::game) fn parked_world_of(&self, dimension: &str) -> Option<&World> {
        self.parked
            .iter()
            .find(|(dim, _)| dim.id() == dimension)
            .map(|(_, level)| &level.world)
    }

    /// The world of the level not in place whose dimension is `dimension`, to
    /// change.
    pub fn parked_world_mut_of(
        &mut self,
        dimension: &str,
    ) -> Option<&mut World> {
        self.parked
            .iter_mut()
            .find(|(dim, _)| dim.id() == dimension)
            .map(|(_, level)| &mut level.world)
    }

    /// The streaming of the level not in place whose dimension is `dimension`.
    pub(in crate::game) fn parked_streaming_of(&self, dimension: &str) -> Option<&ChunkStreamer> {
        self.parked
            .iter()
            .find(|(dim, _)| dim.id() == dimension)
            .map(|(_, level)| &level.streaming)
    }

    /// A parked level's existing generator query worker, without switching levels.
    pub(in crate::game::server_side) fn parked_locator_of(
        &self,
        dimension: &str,
    ) -> Option<Arc<VanillaTerrain>> {
        self.parked_of(dimension)?.locator.clone()
    }

    /// The streaming of the level not in place whose dimension is `dimension`, to change.
    pub(in crate::game::server_side) fn parked_streaming_of_mut(
        &mut self,
        dimension: &str,
    ) -> Option<&mut ChunkStreamer> {
        self.parked
            .iter_mut()
            .find(|(dim, _)| dim.id() == dimension)
            .map(|(_, level)| &mut level.streaming)
    }

    /// `ServerChunkCache.tick` of the parked levels that `tick` names: their `UNKNOWN` tickets
    /// purged (`TicketStorage.purgeStaleTickets`) when `purge`, then their recent FULL lookups
    /// forgotten (`clearCache`).
    pub(in crate::game::server_side) fn tick_parked_chunk_caches(
        &mut self,
        purge: bool,
        tick: impl Fn(Dim) -> bool,
    ) {
        for (&dim, level) in &mut self.parked {
            if tick(dim) {
                if purge {
                    level.streaming.purge_unknown_tickets();
                }
                level.streaming.clear_full_lookups();
            }
        }
    }

    /// `ServerLevel.setChunkForced`: force a column in an already open level without changing the acting
    /// player's level; the same queue and load barrier serve the active level.
    pub(in crate::game::server_side) fn force_parked_chunk(
        &mut self,
        dimension: &str,
        x: i32,
        z: i32,
        forced: bool,
    ) -> Option<bool> {
        let level = self.parked_mut_of(dimension)?;
        Some(
            level
                .streaming
                .set_forced_loaded(&mut level.world, x, z, forced),
        )
    }

    pub(in crate::game) fn parked_anims(&self, dim: Dim) -> Option<&BlockEntityTicks> {
        self.parked.get(&dim).map(|level| &level.anims)
    }

    /// The worlds of the levels not in place.
    /// A parked level's world border.
    pub(in crate::game::server_side) fn parked_border(
        &self,
        dim: Dim,
    ) -> Option<&sunrise_level::worldborder::WorldBorder> {
        self.parked.get(&dim).map(|level| &level.border)
    }

    pub(in crate::game::server_side) fn install_profession_save_snapshots(
        &self,
        table: Arc<sunrise_sim::mobs::villager::ResolvedProfessions>,
    ) {
        for level in self.parked.values() {
            level.save.install_profession_snapshot(table.clone());
        }
    }

    /// The generators of the parked levels, for the world snapshots their workers read.
    pub(in crate::game::server_side) fn parked_terrains(
        &self,
    ) -> impl Iterator<Item = &Arc<VanillaTerrain>> {
        self.parked
            .values()
            .filter_map(|level| level.locator.as_ref())
    }

    pub(in crate::game::server_side) fn parked_worlds(&self) -> impl Iterator<Item = &World> {
        self.parked.values().map(|level| &level.world)
    }

    /// The streamers of the levels not in place.
    pub(in crate::game::server_side) fn parked_streamings(
        &self,
    ) -> impl Iterator<Item = &ChunkStreamer> {
        self.parked.values().map(|level| &level.streaming)
    }

    /// The level not in place, in `dim`.
    pub(in crate::game::server_side) fn parked_mut(&mut self, dim: Dim) -> Option<&mut Parked> {
        self.parked.get_mut(&dim).map(Box::as_mut)
    }

    /// A parked level by its exact dimension identity, without swapping it in.
    pub(in crate::game::server_side) fn parked_of(&self, dimension: &str) -> Option<&Parked> {
        self.parked
            .iter()
            .find(|(dim, _)| dim.id() == dimension)
            .map(|(_, level)| level.as_ref())
    }
    pub(in crate::game::server_side) fn parked_mut_of(
        &mut self,
        dimension: &str,
    ) -> Option<&mut Parked> {
        self.parked
            .iter_mut()
            .find(|(dim, _)| dim.id() == dimension)
            .map(|(_, level)| level.as_mut())
    }

    /// Whether a parked level's general rows hold any of these UUIDs (#792's
    /// dimension-transfer check: a destination may not already hold a row
    /// of the travelling tree's UUIDs).
    pub(in crate::game::server_side) fn parked_general_holds_any_uuid(
        &self,
        dim: Dim,
        uuids: &std::collections::HashSet<[i32; 4]>,
    ) -> bool {
        self.parked.get(&dim).is_some_and(|parked| {
            parked
                .general
                .rows
                .iter()
                .any(|row| uuids.contains(&row.uuid))
        })
    }

    /// The entities of a parked level, counted as `Game::entity_count` counts the one in place.
    pub(in crate::game::server_side) fn parked_entity_count(&self, dim: Dim) -> Option<usize> {
        let l = self.parked.get(&dim)?;
        Some(
            l.mobs.iter().filter(|m| !m.removed).count()
                + l.dropped.len()
                + l.orbs.len()
                + l.projectiles.len()
                + l.carts.len()
                + l.boats.len()
                + l.falling.len()
                + l.paintings.len()
                + l.item_frames.len()
                + l.armor_stands.len()
                + l.cushions.len()
                + l.tridents.len()
                + l.rockets.len()
                + l.tnts.len(),
        )
    }

    /// The mobs of a parked level.
    pub(in crate::game::server_side) fn parked_mobs(&self, dim: Dim) -> Option<&[Mob]> {
        self.parked.get(&dim).map(|level| &*level.mobs)
    }

    pub(in crate::game::server_side) fn parked_perf_views(
        &self,
    ) -> impl Iterator<Item = (Dim, &World, EntityCountView<'_>)> {
        self.parked.iter().map(|(&dim, level)| {
            (
                dim,
                &level.world,
                entity_count_view!(
                    level,
                    &level.projectiles,
                    &level.general.rows,
                    &level.knots,
                    &level.others
                ),
            )
        })
    }

    /// A parked level's loose entities, for `sunrise-report`: items, experience orbs,
    /// projectiles, minecarts and boats.
    pub(in crate::game::server_side) fn parked_entity_counts(
        &self,
        dim: Dim,
    ) -> Option<[usize; 5]> {
        self.parked.get(&dim).map(|l| {
            [
                l.dropped.len(),
                l.orbs.len(),
                l.projectiles.len(),
                l.carts.len(),
                l.boats.len(),
            ]
        })
    }

    /// The levels that are not in place, by dimension.
    pub(in crate::game::server_side) fn parked_dims(&self) -> Vec<Dim> {
        self.parked.keys().copied().collect()
    }
}

/// Build a parked level out of line so `open_level` keeps its stack small while
/// `make_streaming` decodes worldgen density graphs.
#[inline(never)]
fn box_parked(
    world: World,
    streaming: ChunkStreamer,
    locator: Option<Arc<VanillaTerrain>>,
    save: Arc<SaveDir>,
    seed: i64,
    border: sunrise_level::worldborder::WorldBorder,
) -> Box<Parked> {
    Box::new(Parked {
        world,
        streaming,
        locator,
        save,
        block_ticks: Ticks::new(seed as i32),
        tick_chunks: Default::default(),
        tickets: Default::default(),
        random_tick_sections: Default::default(),
        level_events: Vec::new(),
        portal_states: HashMap::new(),
        falling: Default::default(),
        paintings: Default::default(),
        item_frames: Default::default(),
        armor_stands: Default::default(),
        cushions: Default::default(),
        others: Default::default(),
        fangs: Default::default(),
        boats: Default::default(),
        carts: Default::default(),
        bolts: Default::default(),
        crystals: Default::default(),
        dragon: Default::default(),
        breath_clouds: Default::default(),
        tridents: Default::default(),
        rockets: Default::default(),
        ominous_spawners: Default::default(),
        tnts: Default::default(),
        general: Default::default(),
        hoppers: HashMap::new(),
        mobs: Default::default(),
        knot_ids: HashMap::new(),
        broken_knots: Vec::new(),
        knots: Default::default(),
        projectiles: Default::default(),
        dropped: Default::default(),
        orbs: Default::default(),
        persist: Default::default(),
        lectern_pulses: Vec::new(),
        lid_openers: HashMap::new(),
        lan_events: Vec::new(),
        lan_block_events: Vec::new(),
        lan_entity_events: Vec::new(),
        lan_critical_hits: Vec::new(),
        lan_states: Default::default(),
        anims: Default::default(),
        raiding: Default::default(),
        sculk: Default::default(),
        creaking: Default::default(),
        spawn_scratch: Default::default(),
        idle: 0,
        border,
        border_sent: None,
    })
}

impl crate::game::ServerGame<'_> {
    /// A new level for `dim`, opened from the save (`MinecraftServer.createLevels`).
    fn open_level(&self, dim: Dim) -> Result<Box<Parked>> {
        // A level a mod opened has the seed and generator it asked for.
        let (generate, seed) = match self.runtime_terrain(dim) {
            Some((generate, seed)) => (Some(generate), seed),
            None => (None, self.common.level.seed),
        };
        let (streaming, save, locator) = super::world_loading::make_streaming(
            &self.common.res,
            &self.common.pack,
            &self.common.options,
            &self.server.settings,
            &self.server.world_root,
            dim,
            self.server.level_shapes.get(dim.id()),
            seed,
            &self.common.level,
            self.common.world_biomes
                .as_ref()
                .unwrap_or_else(|| self.common.res.biomes())
                .clone(),
            generate.as_ref(),
            self.server.save.0.chunk_reports().clone(),
            self.server.save.0.save_failure_reporter().clone(),
        )?;
        self.bind_chunk_disk_worker(&save, dim);
        save.install_profession_snapshot(self.stations().professions.clone());
        if let Some(terrain) = &locator {
            terrain.set_mob_maps(self.mob_map_snapshot());
            terrain.set_biome_keys(&self.biome_key_snapshot());
        }
        let mut world = sunrise_gameplay_common::game::authoritative_world_for_mods(&self.common.res, seed);
        world.attachments = save.load_attachments();
        world.shape = self.server.level_shapes.get(dim.id());
        world.fast_lava = self.common.dimension_type_of(dim).fast_lava;
        world.infiniburn = Some(self.resolve_level_infiniburn(self.common.dimension_type_of(dim)));
        let border = self.load_border(dim);
        Ok(box_parked(world, streaming, locator, save, seed, border))
    }

    /// `MinecraftServer.createLevels`: open the built-in levels in registry
    /// order before `prepare_levels`, which waits for their saved tickets.
    pub(in crate::game::server_side) fn open_builtin_levels(&mut self) -> Result<()> {
        if self.common.options.execution_role.is_client() {
            return Ok(());
        }
        let starting_dim = self.common.dim;
        // World preparation can build the owner's starting level before the
        // Overworld. Keep it out of both the current and parked slots until
        // its registry turn, so earlier `level_load` listeners cannot see it.
        let mut starting_level = if starting_dim != Dim::Overworld {
            let mut overworld = self.open_level(Dim::Overworld)?;
            self.replace_level(Dim::Overworld, &mut overworld);
            self.server.streaming.radius = sunrise_gameplay_common::game::streaming_radius::streaming_radius_for(
                self.server.settings.render_distance,
                None,
            );
            self.read_raids();
            Some(overworld)
        } else {
            None
        };
        self.native_level_opened(Dim::Overworld);
        for dim in BUILTIN_LEVEL_ORDER.into_iter().skip(1) {
            if dim == starting_dim
                && let Some(level) = starting_level.take()
            {
                self.server.levels.0.parked.insert(dim, level);
                self.enter_level(dim)?;
            } else {
                let mut level = self.open_level(dim)?;
                let left = self.replace_level(dim, &mut level);
                self.server.levels.0.parked.insert(left, level);
                self.server.streaming.radius = sunrise_gameplay_common::game::streaming_radius::streaming_radius_for(
                    self.server.settings.render_distance,
                    None,
                );
                self.read_raids();
            }
            self.native_level_opened(dim);
        }
        // Switching back to the owner's level is separate from publishing the
        // built-ins. A saved custom starting level, if any, is published here.
        let publish_starting_level = starting_level.is_some();
        if let Some(level) = starting_level {
            self.server.levels.0.parked.insert(starting_dim, level);
        }
        self.enter_level(starting_dim)?;
        if publish_starting_level {
            self.native_level_opened(starting_dim);
        }
        Ok(())
    }

    /// `MinecraftServer.prepareLevels`: after all built-ins are published,
    /// activate their saved tickets, then wait for the chunks they hold
    /// (`ChunkLoadCounter`). The first tick finds them ticking and unpacks
    /// their saved block and fluid ticks at the saved game time
    /// (`startTickingChunk`), not at whichever tick they finish streaming in.
    pub fn prepare_levels(&mut self) -> Result<()> {
        if self.common.options.execution_role.is_client() {
            return Ok(());
        }
        let back = self.common.dim;
        let dims = BUILTIN_LEVEL_ORDER
            .into_iter()
            .chain(matches!(back, Dim::Extra(_)).then_some(back));
        for dim in dims.clone() {
            self.enter_level(dim)?;
            Self::load_saved_level_tickets(
                &self.server.save.0,
                &mut self.server.streaming,
                &mut self.server.tickets.0,
            );
            self.validate_mod_chunk_tickets(dim);
        }
        self.server.startup_ticket_restore_pending = false;
        let deadline = Instant::now() + Duration::from_secs(600);
        for dim in dims {
            self.enter_level(dim)?;
            self.wait_for_ticket_chunks(deadline);
        }
        self.enter_level(back)
    }

    /// `TicketStorage.activateAllDeactivatedTickets`: restore portal and
    /// command tickets, and stage mod tickets for their controllers to validate.
    fn load_saved_level_tickets(
        save: &SaveDir,
        streaming: &mut ChunkStreamer,
        tickets: &mut ChunkTickets,
    ) {
        tickets.restore_portal(save.load_portal_tickets());
        streaming.load_forced(
            save.load_forced_chunks(),
            save.load_mod_tickets(|id| mod_api::TicketController::from_id(id).map(|c| c.slot())),
        );
    }

    /// A level that opens after startup restores its saved command and portal
    /// tickets at once. Mod tickets still wait for validation after `level_load`.
    pub(in crate::game::server_side) fn open_runtime_level(&self, dim: Dim) -> Result<Box<Parked>> {
        let mut level = self.open_level(dim)?;
        Self::load_saved_level_tickets(&level.save, &mut level.streaming, &mut level.tickets);
        Ok(level)
    }

    /// Stream the level in place until each chunk its tickets make block
    /// ticking is a ticking chunk (lit, with its neighbours loaded).
    fn wait_for_ticket_chunks(&mut self, deadline: Instant) {
        // Forced chunks are checked two chunks out (their ticket block ticks one).
        // A `PORTAL` ticket's chunks are streamed as an area of their own (the
        // network tick does it, which hasn't run yet), whose chunks out to its
        // radius can become ticking chunks: those are waited for.
        let portal = self.portal_ticket_areas();
        for area in &portal {
            if !self.server.streaming.extra.contains(area) {
                self.server.streaming.extra.push(*area);
            }
        }
        loop {
            self.update_chunk_tickets();
            let levels = self.server.tickets.0.levels();
            let streaming = &self.server.streaming;
            let ready = streaming
                .forced
                .iter()
                .map(|&c| (c, 2))
                .chain(portal.iter().copied())
                .all(|((cx, cz), reach)| {
                    (cz - reach..=cz + reach).all(|z| {
                        (cx - reach..=cx + reach).all(|x| {
                            !levels.loaded_ticking(x, z) || super::ticking_chunk(&self.common.world, x, z)
                        })
                    })
                });
            if ready {
                return;
            }
            if Instant::now() >= deadline {
                log::warn!("Sunrise: the chunks of the saved tickets did not load");
                return;
            }
            self.server_update_world();
            self.server.streaming.wait(Duration::from_millis(5));
        }
    }

    /// Exchange the level in place with `p`.
    fn swap_level(&mut self, p: &mut Parked) {
        use std::mem::swap;
        swap(&mut self.common.world, &mut p.world);
        // The carrier first-use gate belongs to the world, not the
        // level: a level swap keeps it.
        self.common.world.carrier_data_existed_at_open = p.world.carrier_data_existed_at_open;
        swap(&mut self.server.streaming, &mut p.streaming);
        let mut locator = self.common.end.locator();
        swap(&mut locator, &mut p.locator);
        self.common.end.set_locator(locator);
        self.share_spawn_conditions();
        swap(&mut self.server.save.0, &mut p.save);
        swap(&mut self.common.block_ticks, &mut p.block_ticks);
        swap(&mut self.server.tick_chunks, &mut p.tick_chunks);
        swap(&mut self.server.tickets.0, &mut p.tickets);
        swap(
            &mut self.server.random_tick_sections.0,
            &mut p.random_tick_sections,
        );
        swap(&mut self.common.level_events, &mut p.level_events);
        swap(&mut self.server.portals.entities, &mut p.portal_states);
        swap(&mut self.common.falling, &mut p.falling);
        swap(&mut self.common.paintings, &mut p.paintings);
        swap(&mut self.common.item_frames, &mut p.item_frames);
        swap(&mut self.common.armor_stands, &mut p.armor_stands);
        swap(&mut self.common.cushions, &mut p.cushions);
        swap(&mut self.server.others.0, &mut p.others);
        swap(&mut self.common.fangs, &mut p.fangs);
        swap(&mut self.common.boats, &mut p.boats);
        swap(&mut self.common.carts, &mut p.carts);
        swap(&mut self.common.bolts, &mut p.bolts);
        swap(&mut self.common.crystals, &mut p.crystals);
        swap(&mut self.common.dragon, &mut p.dragon);
        swap(&mut self.common.breath_clouds, &mut p.breath_clouds);
        swap(&mut self.common.tridents, &mut p.tridents);
        swap(&mut self.common.rockets, &mut p.rockets);
        swap(&mut self.common.ominous_spawners, &mut p.ominous_spawners);
        swap(&mut self.common.tnts, &mut p.tnts);
        swap(&mut self.server.hoppers, &mut p.hoppers);
        swap(&mut self.common.mobs, &mut p.mobs);
        swap(&mut self.server.knot_ids, &mut p.knot_ids);
        swap(&mut self.server.broken_knots, &mut p.broken_knots);
        swap(&mut self.common.resident_entities.knots, &mut p.knots);
        swap(&mut self.common.resident_entities.general, &mut p.general);
        swap(&mut self.common.creatures.projectiles, &mut p.projectiles);
        swap(&mut self.common.dropped, &mut p.dropped);
        swap(&mut self.common.orbs, &mut p.orbs);
        swap(&mut self.common.persist, &mut p.persist);
        swap(&mut self.server.lectern_pulses, &mut p.lectern_pulses);
        swap(&mut self.server.lid_openers, &mut p.lid_openers);
        swap(&mut self.server.lan_events, &mut p.lan_events);
        swap(&mut self.server.lan_block_events, &mut p.lan_block_events);
        swap(&mut self.server.lan_entity_events, &mut p.lan_entity_events);
        swap(&mut self.server.lan_critical_hits, &mut p.lan_critical_hits);
        self.server.be_anims.swap_level(&mut p.anims);
        self.server.raiding.0.swap_level(&mut p.raiding);
        self.server.sculk.swap_level(&mut p.sculk);
        self.server.creaking.swap_level(&mut p.creaking);
        swap(&mut self.server.spawn_scratch, &mut p.spawn_scratch);
        swap(&mut self.common.border, &mut p.border);
    }

    /// Swap the level in place without publishing the one just left.
    fn replace_level(&mut self, target: Dim, p: &mut Parked) -> Dim {
        p.world.game_time = self.common.world.game_time;
        // The level's day time is its default clock's; a level without one follows the
        // Overworld's.
        let clock = self.common.level_clock_name(target);
        p.world.day_time = self.common.switch_clock(&clock);
        p.world.end_clock = self.common.world.end_clock;
        p.world.weather = self.common.world.weather;
        // The game rules and the difficulty are the server's, not a
        // level's: natural regeneration holds in every dimension.
        p.world.health_rules = self.common.world.health_rules;
        // The sculk tables' watched blocks are built once per game.
        p.world.watched = std::mem::take(&mut self.common.world.watched);
        self.swap_level(p);
        // The tracked entities go with their level: a level's moves, read before its tick
        // describes its entities again, would otherwise meet the last level's (a move
        // missed the boat under a player while anyone was in another dimension).
        if let Some(h) = self.server.lan.as_mut() {
            std::mem::swap(&mut h.states, &mut p.lan_states);
            std::mem::swap(&mut h.border_sent, &mut p.border_sent);
        }
        p.idle = 0;
        std::mem::replace(&mut self.common.dim, target)
    }

    /// Put `target`'s level in place, opening it first if it is not loaded.
    /// The level that was in place is kept with the others; the shared
    /// clock and weather go with the swap.
    pub fn enter_level(&mut self, target: Dim) -> Result<()> {
        // A client holds no levels.
        if self.common.options.execution_role.is_client() {
            return Ok(());
        }
        if target != self.common.dim {
            let (mut p, opened) = match self.server.levels.0.parked.remove(&target) {
                Some(p) => (p, false),
                None => (self.open_runtime_level(target)?, true),
            };
            let left = self.replace_level(target, &mut p);
            self.server.levels.0.parked.insert(left, p);
            if opened {
                self.server.streaming.radius = sunrise_gameplay_common::game::streaming_radius::streaming_radius_for(
                    self.server.settings.render_distance,
                    None,
                );
                // ServerLevel's constructor reads the level's raids.
                self.read_raids();
                self.native_level_opened(target);
                self.validate_mod_chunk_tickets(target);
            }
        }
        if let Some(h) = self.server.lan.as_mut() {
            h.hub.set_dimension(target.id());
        }
        self.sync_hub_clocks();
        Ok(())
    }

    /// Follow the direct mount of a travelling player through complete source objects.
    /// A later player in the same travelling stack finds no source root: the first
    /// arrival already moved it, keeping every id that the guests' mounts name.
    pub(in crate::game::server_side) fn parked_vehicle_root(
        &self,
        from: Dim,
        mut id: u32,
    ) -> Option<u32> {
        let p = self.server.levels.0.parked.get(&from)?;
        for _ in 0..=p.mobs.len() + p.persist.off_level.len() + 1 {
            let mob = p
                .mobs
                .iter()
                .find(|m| m.id == id)
                .or_else(|| p.persist.off_level.get(&id).and_then(OffLevel::as_mob))
                .filter(|m| !m.removed);
            if let Some(parent) = mob.and_then(|m| m.extra.mount) {
                id = parent;
                continue;
            }
            let present = mob.is_some()
                || p.boats.iter().any(|b| b.id == id)
                || p.carts.iter().any(|c| c.id == id);
            let removed = p
                .persist
                .removals
                .iter()
                .chain(p.persist.destroying_vehicles.iter())
                .any(|target| {
                    matches!(
                        target, Target::Mob(n) | Target::Boat(n) | Target::Cart(n) if *n == id
                    )
                });
            return (present && !removed).then_some(id);
        }
        None
    }

    /// Move the source tree's object memory and take its Mob descendants in
    /// canonical postorder. The caller owns the extracted root and admits it last.
    /// Transfer removal preserves lifetimes; residency is decided in the destination.
    pub(in crate::game::server_side) fn move_entity_memory(
        &mut self,
        from: Dim,
        root: u32,
    ) -> Vec<Mob> {
        let Some(p) = self.server.levels.0.parked.get_mut(&from) else {
            return Vec::new();
        };
        let mut edges: Vec<_> = p
            .mobs
            .iter()
            .chain(p.persist.off_level.values().filter_map(OffLevel::as_mob))
            .filter(|mob| !mob.removed)
            .map(|mob| (mob.extra.mount, mob.extra.vehicle_order, mob.id))
            .collect();
        edges.sort_by_key(|&(_, order, id)| (order, id));
        let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
        for (parent, _, id) in edges {
            if let Some(parent) = parent {
                children.entry(parent).or_default().push(id);
            }
        }
        let mut pending = vec![(root, false)];
        let mut seen = std::collections::HashSet::new();
        let mut ordered = Vec::new();
        while let Some((id, done)) = pending.pop() {
            if done {
                ordered.push(id);
            } else if seen.insert(id) {
                pending.push((id, true));
                if let Some(riders) = children.get(&id) {
                    pending.extend(riders.iter().rev().map(|&rider| (rider, false)));
                }
            }
        }
        let mut moved = Vec::new();
        for id in ordered {
            p.persist.move_object_to(id, &mut self.common.persist);
            if id == root {
                continue;
            }
            if let Some(state) = p.portal_states.remove(&id) {
                self.server.portals.entities.insert(id, state);
            }
            let mob = if let Some(index) = p.mobs.iter().position(|m| m.id == id) {
                Some(p.mobs.remove_for_transfer(index))
            } else {
                // Boats and carts are only roots, which the caller takes.
                match p.persist.off_level.remove(&id) {
                    Some(OffLevel::Mob(mob)) => Some(mob),
                    _ => None,
                }
            };
            if let Some(mut mob) = mob {
                mob.join = sunrise_sim::join_seq::JoinSeq::UNJOINED;
                moved.push(mob);
            }
        }
        moved
    }

    /// An already-owned body changes level, never construction identity.
    /// `PersistentEntitySectionManager.addEntityUuid` decides its residency:
    /// a body whose UUID an earlier arrival took is retained off-level.
    pub(in crate::game::server_side) fn admit_transferred_object(&mut self, mut object: OffLevel) {
        let id = match &mut object {
            OffLevel::Mob(mob) => {
                self.rebind_transferred_entity(mob.id, mob);
                mob.id
            }
            OffLevel::Boat(boat) => {
                self.rebind_transferred_entity(boat.id, boat);
                boat.id
            }
            OffLevel::Cart(cart) => {
                self.rebind_transferred_entity(cart.id, cart);
                cart.id
            }
        };
        self.common.journal_arrival(id);
        let level = self.common.dim;
        self.common.entity_attachments.move_collider(id, level);
        if self.uuid_admission_refused(self.common.persist.uuids.get(&id).copied()) {
            self.common.persist.off_level.insert(id, object);
            return;
        }
        match object {
            OffLevel::Mob(mob) => self.common.mobs.push(mob),
            OffLevel::Boat(boat) => self.common.boats.push(boat),
            OffLevel::Cart(cart) => self.common.carts.push(cart),
        }
    }

    /// Far terrain for a client in `dim`: the level in place may be another
    /// one, the server ticking its levels in turn.
    pub(in crate::game) fn lod_source_in(
        &self,
        dim: Dim,
    ) -> Option<Arc<dyn sunrise_level::chunk_streamer::LodSource>> {
        if dim == self.common.dim {
            return Some(self.server.streaming.lod_source());
        }
        let parked = self.server.levels.0.parked.get(&dim)?;
        Some(parked.streaming.lod_source())
    }

    /// Put `dim`'s level in place for a command whose source is there
    /// (`execute in`, `execute at` a player in another level), opening it if
    /// nobody is in it; returns the level to put back afterwards. Only a
    /// server holds other levels.
    pub fn enter_command_level(&mut self, dim: Dim) -> Option<Dim> {
        if self.common.options.execution_role.is_client() || dim == self.common.dim {
            return None;
        }
        let back = self.common.dim;
        match self.enter_level(dim) {
            Ok(()) => Some(back),
            Err(e) => {
                log::error!("could not open {}: {e}", dim.id());
                None
            }
        }
    }

    /// One server tick (`MinecraftServer.tickServer`): each level with
    /// players or forced chunks in turn (`tickChildren`), its chunks streamed
    /// and loaded and its world ticked. The first pass reads the network and runs the
    /// tick's shared parts (the clock, the weather, functions, the autosave);
    /// the next ones only their level. Connection player ticks follow all
    /// levels. While `paused`, the network runs but player bodies do not.
    /// `ServerTickStart` and `ServerTickEnd` bracket the whole tick.
    pub fn tick_server(&mut self, paused: bool) -> Result<()> {
        self.with_residency_tick(|game| game.tick_server_body(paused))
    }

    fn tick_server_body(&mut self, paused: bool) -> Result<()> {
        // A panic something caught left a game that carried on: its record ends with the tick.
        self.server.interrupted_turns.clear();
        // `tickRateManager.tick()` once per server tick, before any level
        // (each `ServerLevel.tick` only reads `runsNormally`), and the native
        // `ServerTickStart` before that level work (NeoForge's
        // `ServerTickEvent.Pre`). A paused pass and a held server tick nothing.
        let running = if !paused && !self.server.hold_server {
            self.common.clock.check_sprint_completion();
            if let Some(report) = self.common.clock.take_sprint_report() {
                // ServerTickRateManager.finishTickSprint: the server's sendSuccess.
                self.server.console_broadcast = Some(true);
                self.sprint_report(&report);
                self.server.console_broadcast = None;
                self.send_admin_feedback();
                self.server.admin_feedback.clear();
                // Then onTickRateChanged, its mods outside the server's broadcast.
                self.on_tick_rate_changed();
            }
            Some(self.common.clock.tick_world_runs())
        } else {
            None
        };
        let server = self.server.serving.is_some() || self.common.options.dedicated.is_some();
        if server {
            self.run_requested_saves();
        }
        // A config edited while the server is paused (empty) still reloads.
        // The parts outside every level get spans of their own, so that a slow
        // tick's line and `sunrise-report` say when a mod's tick hook took it.
        if server {
            let _span = sunrise_level::spans::guard("mod_configs_poll");
            self.native_configs_poll();
        }
        if let Some(running) = running
            && server
        {
            let _span = sunrise_level::spans::guard("mods_server_tick_start");
            self.native_lifecycle_server_tick_start(running);
        }
        let observe_level =
            running.is_some() && (self.server.serving.is_some() || self.common.options.dedicated.is_some());
        let order = self.levels_to_tick();
        let n = order.len();
        let mut result = Ok(());
        for phase in server_tick_phases(&order) {
            let ServerTickPhase::Level(k, dim) = phase else {
                if running.is_some() && self.server.lan.is_some() {
                    let holding = sunrise_sim::lan::Holding::start();
                    result = sunrise_level::spans::time("lan_guest_connections", || {
                        self.tick_guest_connections(true)
                    });
                    if result.is_ok()
                        && let Some(host) = self.server.lan.as_mut()
                    {
                        sunrise_level::spans::time("lan_end_tick", || host.hub.end_tick());
                    }
                    drop(holding);
                    if let Some(host) = self.server.lan.as_ref() {
                        sunrise_sim::lan::flush_all(host.hub.guests.iter().map(|g| &g.sender));
                    }
                }
                break;
            };
            // Each level's share of the tick (the first one also reads the network and runs the
            // tick's shared parts): `sunrise-report`'s `level_*` spans.
            sunrise_threads::work::set_dim(match dim {
                Dim::Overworld => 1,
                Dim::Nether => 2,
                Dim::End => 3,
                Dim::Extra(_) => 0,
            });
            let _level_span = sunrise_level::spans::guard(match dim {
                Dim::Overworld => "level_overworld",
                Dim::Nether => "level_the_nether",
                Dim::End => "level_the_end",
                Dim::Extra(_) => "level_extra",
            });
            if let Err(e) = self.enter_level(dim) {
                result = Err(e);
                break;
            }
            self.server.level_pass = Pass {
                first: k == 0,
                last: k + 1 == n,
                running,
                packets_drained: false,
            };
            if observe_level && let Some(level) = self.level(dim.id()) {
                self.native_level_tick_pre(level);
            }
            // ServerChunkCache.tick's purge, before the update that may unload the columns.
            if running == Some(true) {
                self.server.streaming.purge_unknown_tickets();
            }
            sunrise_level::spans::time("dedicated_streaming", || {
                self.update_dedicated_streaming();
            });
            self.tick_spawn_tickets(running == Some(true), true);
            sunrise_level::spans::time("update_world", || self.server_update_world());
            // The end of ServerChunkCache.tick, after its unloads: `clearCache`. A paused or held
            // server ticks no level.
            if running.is_some() {
                self.server.streaming.clear_full_lookups();
            }
            let tick_span = sunrise_level::spans::guard("level_tick");
            // This level's moving colliders, solid to its movers for this tick only.
            let colliders = self.placed_moving_colliders();
            let parts = self.common.collision_parts();
            let history = self
                .collider_history(&colliders)
                .map(sunrise_world::moving_colliders::HistoryScope::enter);
            result = {
                let _scope = sunrise_world::moving_colliders::Scope::enter_level(
                    self.collider_level(),
                    colliders.clone(),
                );
                sunrise_sim::block_contact::in_level(
                    self.common.tick,
                    (self.server.serving.is_some() || self.common.options.dedicated.is_some())
                        .then_some(dim.id()),
                    || {
                        if paused {
                            self.tick_lan(true);
                            Ok(())
                        } else if k == 0 {
                            self.tick_server_first()
                        } else {
                            let running = running.unwrap_or_else(|| self.common.clock.world_runs());
                            // ClientLevel.tick: the sky flash fades in a frozen level too.
                            self.tick_weather_effects();
                            self.tick_world(running)
                        }
                    },
                )
            };
            drop(history);
            // Then the bodies standing on a collider that moved are carried with it.
            if result.is_ok() && !paused {
                sunrise_sim::block_contact::in_level(
                    self.common.tick,
                    (self.server.serving.is_some() || self.common.options.dedicated.is_some())
                        .then_some(dim.id()),
                    || self.carry_pass(&colliders),
                );
            }
            drop(parts);
            drop(tick_span);
            self.drain_entity_inside();
            if result.is_err() {
                break;
            }
            if observe_level && let Some(level) = self.level(dim.id()) {
                self.native_level_tick_post(level);
            }
            #[cfg(feature = "state-digest")]
            self.state_digest_tick(dim, running);
            sunrise_level::spans::time("server_debug", || self.publish_server_debug(dim));
            // An entity that crossed into a level that already ticked this
            // server tick is in it when the tick ends (`addDuringTeleport`).
            if running == Some(true) {
                let _span = sunrise_level::spans::guard("late_portal_arrivals");
                let mut moved = false;
                for &earlier in &order[..k] {
                    if !self.arrival_chunks(earlier).is_empty() && self.enter_level(earlier).is_ok()
                    {
                        self.land_portal_arrivals();
                        moved = true;
                    }
                }
                if moved && k + 1 < n {
                    self.enter_level(dim)?;
                }
            }
        }
        // Vanilla ticks the chunk source of every level, ticked here or not: the levels left
        // parked purge their tickets and forget their recent lookups too.
        if running.is_some() {
            self.server
                .levels
                .0
                .tick_parked_chunk_caches(running == Some(true), |dim| !order.contains(&dim));
        }
        // Levels held only by loading tickets: stream them without running the world tick.
        if result.is_ok() {
            result = self.run_view_passes(&order, running == Some(true));
        }
        if let Some(&first) = order.first()
            && self.common.dim != first
        {
            let _ = self.enter_level(first);
        }
        self.server.level_pass = Pass::default();
        if result.is_err() {
            self.common.native_lifecycle.cancel_server_tick();
        }
        result?;
        // The levels mods asked to open or close: now that no level is in the middle of a tick.
        self.apply_runtime_levels();
        let closed = if should_run_idle_maintenance(paused) {
            let _span = sunrise_level::spans::guard("close_idle_levels");
            self.close_idle_levels()
        } else {
            Ok(())
        };
        if let Err(error) = closed {
            self.common.native_lifecycle.cancel_server_tick();
            return Err(error);
        }
        // `ServerTickEnd`: an errored tick or an idle-close failure (at
        // the `?` above) never reaches this site.
        {
            let _span = sunrise_level::spans::guard("mods_server_tick_end");
            self.native_lifecycle_server_tick_end(paused);
        }
        Ok(())
    }

    /// `tick_server` as a server thread runs it, timed whenever spans are on.
    pub(in crate::game::server_side) fn tick_server_timed(&mut self, paused: bool) -> Result<()> {
        self.tick_server_spans(paused, |_| {}).map(|_| ())
    }

    /// Return the time in `tick_server` itself, and hand its detailed timing to `record`
    /// while spans are on. The recording and thread-work bookkeeping are outside that time.
    pub fn tick_server_spans(
        &mut self,
        paused: bool,
        record: impl FnOnce(TickTiming),
    ) -> Result<Duration> {
        let started = Instant::now();
        sunrise_threads::work::tick_start();
        let result = self.tick_server_recorded(paused, record);
        sunrise_threads::work::tick_end(started.elapsed().as_nanos() as u64);
        result
    }

    fn tick_server_recorded(
        &mut self,
        paused: bool,
        record: impl FnOnce(TickTiming),
    ) -> Result<Duration> {
        if self.common.options.spikes.is_none() && !sunrise_level::spans::recording_here() {
            let started = Instant::now();
            self.tick_server(paused)?;
            return Ok(started.elapsed());
        }
        // Discard between-tick work (such as the chunk pump) before timing this tick.
        sunrise_level::spans::take_raw();
        let cpu = sunrise_level::spans::thread_cpu();
        let started = Instant::now();
        let result = self.tick_server(paused);
        let method = started.elapsed();
        let wall = method.as_secs_f32() * 1000.0;
        let cpu = sunrise_level::spans::thread_cpu()
            .zip(cpu)
            .map(|(end, start)| (end - start).as_secs_f32() * 1000.0);
        // Taken every tick: nothing else takes this thread's spans.
        let raw = sunrise_level::spans::take_raw();
        // The spans go to the slow ring; whose packets or events took the tick stays in the owner's log.
        let notes = sunrise_level::spans::take_notes();
        if let Some(threshold) = self.common.options.spikes
            && wall > threshold
            && !notes.is_empty()
        {
            log::info!(
                "Sunrise: {wall:.1} ms tick was busy with: {}",
                slowest_steps(&notes)
            );
        }
        record(TickTiming {
            wall_ms: wall,
            cpu_ms: cpu,
            raw,
        });
        result.map(|()| method)
    }

    /// The end of `MinecraftServer.runServer`'s loop for a server thread:
    /// the next tick starts one tick length after this one, or straight away
    /// while the sprint runs (`checkShouldSprintThisTick`). A server more
    /// than two seconds behind skips the ticks it missed: that returns how far
    /// behind it was, for the overload warning, and how long the thread blocked on
    /// workers or slept (`MinecraftServer.waitForTasks`'s idle time).
    pub(in crate::game::server_side) fn wait_for_next_server_tick(
        &mut self,
        next: &mut Instant,
        paused: bool,
    ) -> (Option<Duration>, Duration) {
        let now = Instant::now();
        let mut idle = Duration::ZERO;
        if !paused && self.common.clock.sprinting() {
            *next = now;
            return (None, idle);
        }
        *next += self.common.clock.tick_duration();
        if *next > now {
            // The chunks loading (the players', force-loaded ones, a
            // `/pregen` task) keep the workers fed until the next tick.
            idle = self.pump_chunks(*next, paused);
            // Integrated servers also service window `OwnerRequest`s while idle
            // between ticks (`MinecraftServer.waitUntilNextTick`), so synchronous
            // queries (`StatusExtras`, publish/unpublish, control ops) do not
            // stall the client frame for up to a full 50 ms server tick.
            loop {
                let now = Instant::now();
                if now >= *next {
                    break;
                }
                let remaining = *next - now;
                let received = match &self.server.serving {
                    Some(serving) => {
                        if serving
                            .flags
                            .stop
                            .load(std::sync::atomic::Ordering::Relaxed)
                        {
                            break;
                        }
                        serving.tasks.recv_timeout(remaining)
                    }
                    None => {
                        std::thread::sleep(remaining);
                        idle += now.elapsed();
                        break;
                    }
                };
                match received {
                    Ok(request) => {
                        idle += now.elapsed();
                        self.serve_owner_request(request);
                        self.apply_publish();
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        idle += now.elapsed();
                        break;
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        let now2 = Instant::now();
                        if *next > now2 {
                            std::thread::sleep(*next - now2);
                        }
                        idle += now.elapsed();
                        break;
                    }
                }
            }
        } else if now - *next > Duration::from_secs(2) {
            let behind = now - *next;
            *next = now;
            return (Some(behind), idle);
        }
        (None, idle)
    }

    /// The levels this server tick runs, in `MinecraftServer.levels` order
    /// (the Overworld, the Nether, the End): every level with a player in
    /// it, forced chunks or a `/pregen` task, or the Overworld while idle.
    pub fn levels_to_tick(&self) -> Vec<Dim> {
        let guests = self
            .server
            .lan
            .as_ref()
            .map_or(&[][..], |h| h.hub.guests.as_slice());
        let mut dims: Vec<Dim> = Dim::all()
            .into_iter()
            .filter(|d| {
                // `/forceload`ed chunks, or a native mod's tickets.
                let held = |s: &ChunkStreamer| !s.forced.is_empty();
                let forced = if *d == self.common.dim {
                    held(&self.server.streaming)
                } else {
                    self.server.levels.0
                        .parked
                        .get(d)
                        .is_some_and(|p| held(&p.streaming))
                };
                guests.iter().any(|g| g.dimension == d.id() && g.participates_in_player_simulation())
                    // A viewer count not yet told to 0 (`tick_mod_container_viewers`).
                    || self.server.mod_container_viewers.keys().any(|(k, _)| *k == d.id())
                    || self.player_bed_levels().any(|b| b == *d)
                    || forced
                    // A game test runs in its own level, players or not.
                    || self.server.game_tests.0.has_tests_in(*d)
                    || self.pregen_level() == Some(*d)
                    // A pending portal arrival keeps its destination level
                    // running so it can land and tick (the portal ticket).
                    || !self.arrival_chunks(*d).is_empty()
                    || self.server.server_trips.iter().any(|trip| trip.spawn_destination_areas(*d).next().is_some())
                    // A portal ticket keeps its dimension active
                    // (`TicketType.shouldKeepDimensionActive`).
                    || !self.portal_tickets_in(*d).is_empty()
                    // So does a viewed `Ticking` area (#3716), for its chunks only.
                    || self.viewed_level(*d) == Some(true)
            })
            .collect();
        if dims.is_empty() {
            dims.push(Dim::Overworld);
        }
        dims
    }

    /// A loading-only ticket needs chunk work, not an active simulation.
    pub(in crate::game::server_side) fn has_spawn_tickets(&self, dim: Dim) -> bool {
        if self.server.lan.as_ref().is_some_and(|host| {
            host.hub
                .lan
                .configuration_tasks
                .loading_spawns(dim.id())
                .next()
                .is_some()
        }) {
            return true;
        }
        if dim == self.common.dim {
            self.server.tickets.0.spawn_areas().next().is_some()
        } else {
            self.server
                .levels
                .0
                .parked
                .get(&dim)
                .is_some_and(|p| p.tickets.spawn_areas().next().is_some())
        }
    }

    /// The portal tickets of `dim`'s level (`placePortalTicket`, or saved).
    pub(in crate::game::server_side) fn portal_tickets_in(&self, dim: Dim) -> &[PortalTicket] {
        if dim == self.common.dim {
            self.server.tickets.0.portal_tickets()
        } else {
            self.server
                .levels
                .0
                .parked
                .get(&dim)
                .map_or(&[], |p| p.tickets.portal_tickets())
        }
    }

    /// The areas the level in place's portal tickets keep streamed: around each
    /// exit its block-ticking chunks and their neighbours, the ring vanilla
    /// keeps `FULL` (level 33), so the outer block-ticking ring is a ticking
    /// chunk too; and at least the chunks its expiry waits for
    /// (`portal_ticket_can_expire`).
    pub(in crate::game::server_side) fn portal_ticket_areas(&self) -> Vec<((i32, i32), i32)> {
        let block_ticking = i32::from(sunrise_sim::ticket_levels::BLOCK_TICKING);
        self.server
            .tickets
            .0
            .portal_tickets()
            .iter()
            .map(|t| (t.chunk, (block_ticking - t.level).max(1)))
            .collect()
    }

    /// Whether saving is currently disabled (`/save-off`, `ServerLevel.noSave`).
    fn saving_disabled(&self) -> bool {
        self.server
            .server_lists
            .0
            .as_ref()
            .is_some_and(|lists| !lists.lock().unwrap().autosave)
    }

    /// Levels currently kept active by players, tickets, viewed areas or a
    /// waiting vehicle.
    fn active_levels(&self) -> Vec<Dim> {
        let mut active = self.levels_to_tick();
        // A level a viewed area names, ticked or not, or one a client still holds chunks of.
        active.extend(
            Dim::all()
                .into_iter()
                .filter(|d| self.needs_view_pass(*d) || self.has_spawn_tickets(*d)),
        );
        // A vehicle waiting there for its rider's arrival in another level.
        active.extend(
            self.server
                .levels
                .0
                .parked
                .keys()
                .copied()
                .filter(|d| self.vehicle_waits_in(*d)),
        );
        active
    }

    /// Count the parked levels' empty ticks: unload unticketed chunks from
    /// idle built-in levels while keeping them open until server shutdown, and
    /// save and close idle custom levels after [`IDLE_TICKS`].
    pub(in crate::game::server_side) fn close_idle_levels(&mut self) -> Result<()> {
        let saving_enabled = !self.saving_disabled();
        let active = self.active_levels();
        let unload_chunks = self
            .server
            .levels
            .0
            .plan_builtin_drains(|dim| active.contains(&dim), saving_enabled);
        let drained = !unload_chunks.is_empty();
        for dim in unload_chunks {
            if self.saving_disabled() {
                break;
            }
            // A chunk-unload listener in an earlier built-in level can force a
            // chunk or otherwise activate this level before its turn.
            if !self.server.levels.0.parked.contains_key(&dim) || self.active_levels().contains(&dim)
            {
                if let Some(p) = self.server.levels.0.parked.get_mut(&dim) {
                    p.idle = 0;
                }
                continue;
            }
            self.unload_parked_level_chunks(dim)?;
        }
        // Recompute active levels and save state after built-in chunk drains:
        // listeners invoked during a drain can force a chunk in a custom level
        // or disable saving.
        let (saving_enabled, active) = if drained {
            (!self.saving_disabled(), self.active_levels())
        } else {
            (saving_enabled, active)
        };
        let closing = self
            .server
            .levels
            .0
            .plan_custom_closes(|dim| active.contains(&dim), saving_enabled);
        for dim in closing {
            let saving_enabled = !self.saving_disabled();
            let now_active = self.active_levels().contains(&dim);
            let eligible = self.server.levels.0.parked.get_mut(&dim).is_some_and(|p| {
                custom_close_still_eligible(dim, &mut p.idle, now_active, saving_enabled)
                    && !p.streaming.has_unknown_tickets()
            });
            if eligible {
                self.close_parked_level(dim)?;
            }
        }
        Ok(())
    }

    /// Unload unticketed chunks from an idle parked built-in level while
    /// keeping the level itself open (`ChunkMap.processUnloads` while the
    /// `ServerLevel` stays registered until server shutdown).
    fn unload_parked_level_chunks(&mut self, dim: Dim) -> Result<()> {
        debug_assert!(dim.is_builtin());
        if self.saving_disabled() {
            return Ok(());
        }
        let back = self.common.dim;
        self.enter_level(dim)?;
        let spawn_wait = std::mem::take(&mut self.server.spawn_wait);
        self.server.streaming.radius = -1;
        self.server.streaming.extra.clear();
        self.server_update_world();
        self.server.spawn_wait = spawn_wait;
        self.enter_level(back)?;
        Ok(())
    }

    /// Save the parked custom level `dim` and drop it: the mods hear its chunks
    /// and the level unload. A level a mod opened at run time stays known, and
    /// opens again when entered, as any other custom level.
    pub fn close_parked_level(&mut self, dim: Dim) -> Result<()> {
        debug_assert!(
            !dim.is_builtin(),
            "built-in levels stay open until server shutdown"
        );
        let back = self.common.dim;
        self.enter_level(dim)?;
        // Vanilla saves the level's chunks before their unload hooks.
        self.native_chunks_loaded();
        self.abort_abandoned_transactions(0);
        if let Err(why) = self.save_level(true) {
            // Not saved: the level stays parked, and the level in place is the one before.
            self.enter_level(back)?;
            return Err(why);
        }
        let chunks: Vec<_> = self.common.world.chunks.values().cloned().collect();
        self.native_chunks_unloaded(&chunks);
        // Save and unload listeners can leave frames open; undo them before dropping the level.
        self.abort_abandoned_transactions(0);
        // Its entities' attachments are saved with them; they leave memory too.
        for id in self.common.persist.uuids.keys() {
            self.common.entity_attachments.remove(id);
            self.common.entity_attachments.remove_collider(*id);
        }
        self.enter_level(back)?;
        self.server.levels.0.parked.remove(&dim);
        self.native_level_closed(dim);
        Ok(())
    }

    /// The mods hear `dim`'s level open (`LevelEvent.Load`), on a server.
    pub fn native_level_opened(&mut self, dim: Dim) {
        if !self.common.is_logical_server() || self.server.levels.0.has_heard_open(dim) {
            return;
        }
        let journal = self.common.resident_entities.table.borrow().producer();
        if dim == self.common.dim {
            self.bind_resident_owners(&journal, dim.key(), true);
        } else if let Some(parked) = self.server.levels.0.parked.get_mut(&dim) {
            parked.bind_resident_owners(&journal, dim.key(), true);
        }
        self.bind_dimension_attachments(dim);
        if self.server.levels.0.note_opened(dim) {
            self.native_level_load(dim.id());
        }
    }

    /// The mods hear `dim`'s level close (`LevelEvent.Unload`), once for
    /// each open they heard.
    fn native_level_closed(&mut self, dim: Dim) {
        self.remove_level_fake_players(dim.id());
        if self.server.levels.0.note_closed(dim) {
            self.native_level_unload(dim.id());
        }
        if dim == self.common.dim {
            self.common.world.water_tickets.clear();
        } else if let Some(world) = self.server.levels.0.parked_world_mut_of(dim.id()) {
            world.water_tickets.clear();
        }
        self.common.attachment_indexes.remove(&dim);
        self.common.passages.retire_level(dim.key());
    }

    /// Every level still open closes with the server, however it ends
    /// (`MinecraftServer.stopServer`'s `level.close()` loop), in vanilla's
    /// order of its levels: the Overworld, the Nether, the End.
    pub fn native_levels_closing(&mut self) {
        for dim in Dim::all() {
            self.native_level_closed(dim);
        }
    }

    /// Run the requested level save passes. `level_save` fires after a pass even if an
    /// individual record failed and was reported. A failed awaited flush ends the pass with
    /// an error and no hook. Reopen levels as needed, then restore the active one.
    pub fn run_requested_saves(&mut self) {
        if sunrise_gameplay_common::game::CommonGame::admit_entity_save().is_err() {
            return;
        }
        let requested = std::mem::take(&mut self.server.levels.0.requested_saves);
        if requested.is_empty() || !self.common.is_logical_server() {
            return;
        }
        let back = self.common.dim;
        for dim in requested {
            if let Err(e) = self.enter_level(dim).and_then(|()| self.save_level(true)) {
                log::error!("saving {} on a mod's request failed: {e}", dim.id());
            }
        }
        if let Err(e) = self.enter_level(back) {
            log::error!(
                "returning to {} after a requested save failed: {e}",
                back.id()
            );
        }
    }

    /// Save every parked level (`MinecraftServer.saveAllChunks`); the level
    /// in place is the caller's to save.
    pub fn save_parked_levels(&mut self, wait: bool) -> Result<()> {
        if self.server.levels.0.parked.is_empty() {
            return Ok(());
        }
        let back = self.common.dim;
        let parked = self.server.levels.0.parked_dims();
        let saved = (|| {
            for dim in parked {
                self.enter_level(dim)?;
                self.save_level(wait)?;
            }
            Ok(())
        })();
        let restored = self.enter_level(back);
        saved.and(restored)
    }
}

/// The steps a slow tick noted as its slowest (`spans::note`), whose packets or which player's event.
fn slowest_steps(notes: &[(f32, String)]) -> String {
    let list: Vec<String> = notes
        .iter()
        .map(|(ms, what)| format!("{what} {ms:.1}"))
        .collect();
    list.join(", ")
}

impl crate::game::ServerGame<'_> {
    /// `Level.getChunk(x, z)` (`ServerChunkCache.getChunk(x, z, FULL, true)`) on the server
    /// thread: column `chunk` of the level in place (`parked: None`) or of the parked level
    /// `parked`, without switching levels, loaded or generated to FULL (resident and lit) under
    /// an `UNKNOWN` ticket (`ChunkStreamer::begin_full_column`). It blocks until then, with no
    /// timeout, as vanilla's `managedBlock`. Each column the wait brings in is announced while
    /// it is resident, before this returns: its `dynamic_light` cells asked (the light job
    /// waits for them) and `chunk_load` fired, at the same point as vanilla's FULL task fires
    /// NeoForge's `ChunkEvent.Load` inside `getChunk`. `Ok` once FULL; `Failed` when a
    /// generation the column needs panicked, before or during the wait
    /// (`ChunkStreamer::full_column_failed`); `Unloaded` when `begin_full_column` refuses, when
    /// the level is gone, and when generation is aborted during the wait.
    pub(in crate::game::server_side) fn load_full_chunk(
        &mut self,
        parked: Option<mod_api::LevelRef>,
        chunk: (i32, i32),
    ) -> std::result::Result<(), mod_api::ChunkLoadError> {
        use mod_api::ChunkLoadError;
        use sunrise_level::chunk_streamer::{FullColumn, full_column_ready};
        let id = parked.map(|level| level.dimension_id());
        let Some((streaming, world)) = self.full_load_level(id) else {
            return Err(ChunkLoadError::Unloaded);
        };
        match streaming.begin_full_column(world, chunk) {
            FullColumn::Ready => return Ok(()),
            FullColumn::Refused => return Err(ChunkLoadError::Unloaded),
            FullColumn::Failed => return Err(ChunkLoadError::Failed),
            FullColumn::Pending => {}
        }
        loop {
            let Some((streaming, world)) = self.full_load_level(id) else {
                return Err(ChunkLoadError::Unloaded);
            };
            let alive = streaming.pump_column(world);
            self.announce_synchronous_loads(parked);
            let Some((streaming, world)) = self.full_load_level(id) else {
                return Err(ChunkLoadError::Unloaded);
            };
            let failed = streaming.full_column_failed(chunk);
            if !alive || failed || full_column_ready(world, chunk) {
                return if streaming.end_full_column(world, chunk) {
                    Ok(())
                } else if failed {
                    Err(ChunkLoadError::Failed)
                } else {
                    Err(ChunkLoadError::Unloaded)
                };
            }
            streaming.wait(Duration::from_millis(5));
        }
    }

    /// The streaming and world of the level in place, or of the parked level `parked`.
    fn full_load_level(
        &mut self,
        parked: Option<&str>,
    ) -> Option<(&mut ChunkStreamer, &mut World)> {
        match parked {
            None => Some((&mut self.server.streaming, &mut self.common.world)),
            Some(id) => {
                let level = self.server.levels.0.parked_mut_of(id)?;
                Some((&mut level.streaming, &mut level.world))
            }
        }
    }

    /// The columns a synchronous load brought into the level, told to the mods as the level's
    /// own update tells them (`native_chunks_loaded`), their `dynamic_light` cells asked first.
    fn announce_synchronous_loads(&mut self, parked: Option<mod_api::LevelRef>) {
        let Some(level) = parked else {
            self.ask_loaded_light_emission();
            self.native_chunks_loaded();
            return;
        };
        let id = level.dimension_id();
        let Some(world) = self.server.levels.0.parked_world_mut_of(id) else {
            return;
        };
        let loaded = std::mem::take(&mut world.loaded_chunks);
        if self.common.pack.chunk_context.light.any_dynamic() {
            let dimension = sunrise_gameplay_common::game::light_emission::light_emission_dimension(level);
            for &(x, z, _) in &loaded {
                self.ask_level_chunk_light_emission(Some(id), dimension, (x, z));
            }
        }
        for (x, z, new) in loaded {
            self.native_chunk_load(id, [x, z], new);
        }
    }
}

impl crate::game::ServerGame<'_> {
    /// Rebind only an already-owning transfer, before installing it in this level.
    /// Memory still belongs to the parked source until the caller moves it.
    pub fn rebind_transferred_entity(
        &mut self,
        id: u32,
        entity: &mut impl sunrise_mod_runtime::attachments::HasAttachmentLifetime,
    ) {
        let slot = entity.attachment_lifetime();
        if !slot.is_bound() {
            return;
        }
        let Some(level) = self.level(self.common.dim.id()) else {
            return;
        };
        let Some(map) = self.common.entity_attachments.get_mut(&id) else {
            return;
        };
        slot.bind(
            map,
            self.common.attachment_indexes.entry(self.common.dim).or_default(),
            mod_api::AttachmentHolder::Entity(mod_api::EntityRef::new(level.epoch(), id)),
        );
    }
}

#[cfg(test)]
mod tests {
use sunrise_gameplay_common::game::in_own_process;

use sunrise_gameplay_common::game::server_side::levels::tests::queue_unencodable_entity_record;

    use super::*;

    fn dirty_test_chunk(game: &mut crate::game::ServerGame<'_>) {
        let chunk = sunrise_level::chunk::Chunk::with_layout(0, 0, game.world.layout());
        game.world.chunks.insert((0, 0), Arc::new(chunk));
        game.world.modified.insert((0, 0));
    }

    

    #[test]
    #[ignore = "daily"]
    fn save_all_succeeds_after_reported_failures_in_multiple_levels() {
        use crate::game::native_entity_refs::tests::with_game;
        use sunrise_sim::save::{Level, LevelShape};
        mod_platform::testing::with_mods(Vec::new().into(), || {
            with_game(|game| {
                let root = tempfile::tempdir().unwrap();
                game.server.world_root = root.path().to_path_buf();
                game.server.save.0 = Arc::new(SaveDir::new(
                    game.server.world_root.clone(),
                    LevelShape::OVERWORLD,
                    Some(Default::default()),
                ));
                game.level.name = "saved despite two refused records".into();
                let mut folders = Vec::new();
                for dim in [Dim::Nether, Dim::End, Dim::Overworld] {
                    game.enter_level(dim).unwrap();
                    dirty_test_chunk(game);
                    folders.push((dim, game.server.save.0.root.clone(), game.server.save.0.shape));
                    if dim != Dim::End {
                        queue_unencodable_entity_record(&game.server.save.0);
                    }
                }
                assert_eq!(
                    game.save_all_reply().translation_key(),
                    Some("commands.save.success")
                );
                assert_eq!(game.server.save.0.record_save_failures(), 2);
                assert_eq!(game.dim, Dim::Overworld);
                assert_eq!(
                    Level::read(root.path()).unwrap().name,
                    "saved despite two refused records"
                );
                for (_, folder, shape) in folders {
                    assert!(
                        SaveDir::new(folder.clone(), shape, None)
                            .raw_chunk(0, 0)
                            .is_some(),
                        "{} was not written",
                        folder.display()
                    );
                }
            });
        });
    }

    #[test]
    #[ignore = "daily"]
    fn an_idle_level_closes_after_reporting_a_record_failure() {
        use crate::game::native_entity_refs::tests::with_game;
        use mod_api::{RuntimeGenerator, RuntimeLevel};
        use sunrise_sim::save::LevelShape;
        sunrise_gameplay_common::game::in_own_process(
            "game::server_side::levels::tests::an_idle_level_closes_after_reporting_a_record_failure",
            || {
                mod_platform::testing::with_mods(Vec::new().into(), || {
                    with_game(|game| {
                        let root = tempfile::tempdir().unwrap();
                        game.server.world_root = root.path().to_path_buf();
                        game.server.save.0 = Arc::new(SaveDir::new(
                            game.server.world_root.clone(),
                            LevelShape::OVERWORLD,
                            Some(Default::default()),
                        ));
                        // Built-in levels unload chunks without closing; exercise the save
                        // on close of a custom level instead.
                        let id = "test_idle:record_failure";
                        let custom = Dim::Extra(mod_api::__private::dimension_key(id));
                        let flat =
                            RuntimeLevel::new("minecraft:overworld", RuntimeGenerator::Flat, 1);
                        game.server
                            .levels
                            .0
                            .runtime
                            .queue_open("test_idle", id, flat)
                            .unwrap();
                        game.enter_level(custom).unwrap();
                        dirty_test_chunk(game);
                        queue_unencodable_entity_record(&game.server.save.0);
                        let disk = SaveDir::new(
                            game.server.save.0.root.clone(),
                            game.server.save.0.shape,
                            None,
                        );
                        game.enter_level(Dim::Overworld).unwrap();
                        game.server.levels.0.parked.get_mut(&custom).unwrap().idle = IDLE_TICKS - 1;
                        game.close_idle_levels().unwrap();
                        assert_eq!(game.dim, Dim::Overworld);
                        assert!(!game.server.levels.0.parked.contains_key(&custom));
                        assert_eq!(game.server.save.0.record_save_failures(), 1);
                        assert!(disk.raw_chunk(0, 0).is_some());
                    });
                });
            },
        );
    }

    #[test]
    fn levels_clear_empties_parked_and_heard_open() {
        let mut levels = Levels::default();
        levels.heard_open.push(Dim::Nether);
        assert!(!levels.heard_open.is_empty());
        levels.clear();
        assert!(levels.heard_open.is_empty());
        assert!(levels.parked.is_empty());
    }

    #[test]
    fn built_in_levels_stay_open_while_unloading_chunks_and_custom_levels_idle_close() {
        use sunrise_level::chunk::Chunk;
        use sunrise_world::registry::BlockStateId;

        let custom = Dim::Extra(mod_api::__private::dimension_key("test_idle:custom"));
        assert!(Dim::Overworld.is_builtin());
        assert!(Dim::Nether.is_builtin());
        assert!(Dim::End.is_builtin());
        assert!(!custom.is_builtin());

        // Startup opens all three built-in levels once in registry order.
        let mut levels = Levels::default();
        let mut loads = Vec::new();
        let mut unloads = Vec::new();
        for dim in BUILTIN_LEVEL_ORDER {
            if levels.note_opened(dim) {
                loads.push(dim);
            }
        }
        assert_eq!(loads, [Dim::Overworld, Dim::Nether, Dim::End]);

        // Also open a custom level to verify custom idle-close still applies.
        assert!(levels.note_opened(custom));
        loads.push(custom);

        let mut overworld_world = World::new(BlockStateId(0), 0);
        overworld_world
            .chunks
            .insert((2, 3), Arc::new(Chunk::empty(2, 3)));
        let mut nether_world = World::new(BlockStateId(0), 0);
        nether_world
            .chunks
            .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
        let end_world = World::new(BlockStateId(0), 0);

        let mut overworld_idle = 0u32;
        let mut nether_idle = 0u32;
        let mut end_idle = 0u32;
        let mut custom_idle = 0u32;
        let mut custom_open = true;

        // Idle past IDLE_TICKS: built-ins unload unticketed chunks and stay open;
        // the custom level stays open until IDLE_TICKS and then closes.
        let mut overworld_unload_requests = 0u32;
        let mut nether_unload_requests = 0u32;
        for tick in 1..=IDLE_TICKS + 50 {
            match step_parked_idle(
                Dim::Overworld,
                &mut overworld_idle,
                false,
                !overworld_world.chunks.is_empty(),
                true,
            ) {
                IdleAction::UnloadChunks => {
                    overworld_unload_requests += 1;
                    assert!(overworld_world.unload_chunk((2, 3)).is_some());
                }
                IdleAction::Keep => {}
                IdleAction::Close => panic!("Overworld must not idle-close"),
            }
            match step_parked_idle(
                Dim::Nether,
                &mut nether_idle,
                false,
                !nether_world.chunks.is_empty(),
                true,
            ) {
                IdleAction::UnloadChunks => {
                    nether_unload_requests += 1;
                    assert!(nether_world.unload_chunk((0, 0)).is_some());
                }
                IdleAction::Keep => {}
                IdleAction::Close => panic!("Nether must not idle-close"),
            }
            assert_eq!(
                step_parked_idle(
                    Dim::End,
                    &mut end_idle,
                    false,
                    !end_world.chunks.is_empty(),
                    true,
                ),
                IdleAction::Keep,
                "empty parked End stays open with no work"
            );
            if custom_open {
                let action = step_parked_idle(custom, &mut custom_idle, false, false, true);
                if tick < IDLE_TICKS {
                    assert_eq!(action, IdleAction::Keep);
                } else {
                    assert_eq!(action, IdleAction::Close);
                    custom_open = false;
                    if levels.note_closed(custom) {
                        unloads.push(custom);
                    }
                }
            }
        }

        // Unticketed chunks unloaded once; idle ticks afterwards did no extra work.
        assert_eq!(overworld_unload_requests, 1);
        assert_eq!(nether_unload_requests, 1);
        assert!(overworld_world.chunks.is_empty());
        assert!(nether_world.chunks.is_empty());
        // Built-ins never unloaded; only the custom level idle-closed.
        assert_eq!(unloads, [custom]);
        assert!(levels.has_heard_open(Dim::Overworld));
        assert!(levels.has_heard_open(Dim::Nether));
        assert!(levels.has_heard_open(Dim::End));
        assert!(!levels.has_heard_open(custom));

        // An active custom level resets its idle counter and does not close.
        let mut active_custom_idle = IDLE_TICKS - 1;
        assert_eq!(
            step_parked_idle(custom, &mut active_custom_idle, true, false, true),
            IdleAction::Keep
        );
        assert_eq!(active_custom_idle, 0);

        // Visiting Nether or End after the idle period does not fire a second level_load.
        for dim in [Dim::Nether, Dim::End] {
            if levels.note_opened(dim) {
                loads.push(dim);
            }
        }
        assert_eq!(loads, [Dim::Overworld, Dim::Nether, Dim::End, custom]);

        // Shutdown closes every still-open built-in level once (and not the already-closed custom level).
        for dim in BUILTIN_LEVEL_ORDER.into_iter().chain([custom]) {
            if levels.note_closed(dim) {
                unloads.push(dim);
            }
        }
        assert_eq!(unloads, [custom, Dim::Overworld, Dim::Nether, Dim::End]);
        // Repeating close at shutdown never fires a duplicate unload.
        for dim in BUILTIN_LEVEL_ORDER {
            assert!(!levels.note_closed(dim));
        }
    }

    #[test]
    fn idle_drain_respects_pause_and_save_off() {
        let custom = Dim::Extra(mod_api::__private::dimension_key("test_idle:guarded"));

        // Server pause skips parked-level maintenance; unpaused ticks (including
        // `/tick freeze`, where `paused == false`) still run it.
        assert!(!should_run_idle_maintenance(true));
        assert!(should_run_idle_maintenance(false));

        // `/save-off` (`saving_enabled == false`) keeps parked built-in chunks
        // resident and defers custom idle-close until `/save-on` resumes saving.
        let mut nether_idle = 0u32;
        let mut custom_idle = IDLE_TICKS - 1;
        assert_eq!(
            step_parked_idle(Dim::Nether, &mut nether_idle, false, true, false),
            IdleAction::Keep
        );
        assert_eq!(
            step_parked_idle(custom, &mut custom_idle, false, false, false),
            IdleAction::Keep
        );
        assert_eq!(
            step_parked_idle(Dim::Nether, &mut nether_idle, false, true, true),
            IdleAction::UnloadChunks
        );
        assert_eq!(
            step_parked_idle(custom, &mut custom_idle, false, false, true),
            IdleAction::Close
        );
    }

    

    

    

    
    

    

    

    

    

    

    

    

    

    

    

    
}

impl crate::game::ServerGame<'_> {
    /// `Game::tick` as a server runs it for its first level: no window lead, which only the window's `Game` has.
    pub(in crate::game) fn tick_server_first(&mut self) -> Result<()> {
        if !self.common.is_logical_server() {
            return Err("client-role game cannot tick a server level".into());
        }
        if self.server.hold_server {
            return Ok(());
        }
        self.abort_open_transactions();
        self.common.update_interaction_ranges();
        self.tick_first_level()
    }

    /// A server's tick of its first level: the clock, functions, autosave and weather it owns, then the level.
    pub(in crate::game) fn tick_first_level(&mut self) -> Result<()> {
        if !self.common.is_logical_server() {
            return Err("client-role game cannot tick a server level".into());
        }
        // Only a server ticks its own world from here: its players are guests, swapped in for
        // their turns (`as_guest`), and the window's player is a client of it.
        self.common.ticks += 1;
        // `/tick freeze` stops the world but not the player.
        let running = match self.server.level_pass.running {
            Some(running) => running,
            None => self.common.clock.world_runs(),
        };
        span("mods_inventory_ticks", || self.native_inventory_ticks());
        if running {
            // `MinecraftServer.tickChildren`: `commandFunctions` runs before
            // any level ticks, so a tick function reads the game time and the
            // day clock from before this tick. Vanilla's SavedData is loaded
            // long before this point; the lazy scoreboard/storage read has to
            // be done by then too, or a load tag writes over what it restores.
            self.load_scoreboard();
            span("tick_functions", || self.tick_functions());
            self.common.world.game_time += 1;
            if self
                .common.ui
                .rules
                .bool_at(sunrise_game_data::gamerules::vanilla::ADVANCE_WEATHER)
            {
                self.common.world.weather.tick_timers(&mut self.common.world.random);
            }
            self.common.world.weather.tick_levels();
        } else if self.common.clock.still() {
            // Under `SUNRISE_STILL` a frozen world's rain and thunder sit at their levels, as a
            // shader pack's smoothed values do, so `/weather rain` shows full rain without
            // stepping the world (and its mobs) through the fade.
            self.common.world.weather.settle_levels();
        }
        // ClientLevel.tick: the sky flash fades in a frozen level too.
        self.tick_weather_effects();
        let _ = span("player_autosaves", || self.finish_player_autosaves(false));
        // MinecraftServer.tickServer's countdown, then autoSave: three
        // hundred seconds of ticks at the rate the server runs at now.
        self.server.ticks_until_autosave -= 1;
        self.common.autosaving = self.server.ticks_until_autosave <= 0;
        if let Some(serving) = &self.server.serving {
            serving.set_saving(self.common.autosaving);
        }
        if self.common.autosaving {
            span("autosave", || self.save_game_with(false))?;
            self.server.ticks_until_autosave = self.next_autosave_interval();
            if let Some(serving) = &self.server.serving {
                serving.set_saving(false);
            }
        }
        if running {
            self.advance_clock();
            // `ServerLevel.tickTime`, once the clock has moved on.
            span("scheduled_functions", || self.run_scheduled_functions());
        }
        self.update_totem_ready();
        span("tick_command_extras", || self.tick_command_extras());
        self.tick_world(running)
    }
}

use sunrise_gameplay_common::game::native_hooks::base::GameHooks as _;

use sunrise_gameplay_common::game::tables::GameItems as _;

use sunrise_gameplay_common::game::tables::WorldTablesExt as _;

use sunrise_gameplay_common::game::transactions::TransactionsGameExt as _;

use sunrise_gameplay_common::game::GameGameExt1 as _;

use sunrise_gameplay_common::game::GameGameExt3 as _;
use sunrise_gameplay_common::game::GameGameExt3 as _;

use sunrise_gameplay_common::game::chat_common::ChatCommonGameExt1 as _;

use sunrise_gameplay_common::game::clocks::ClocksGameExt1 as _;

use sunrise_gameplay_common::game::light_emission::LightEmissionGameExt1 as _;

use sunrise_gameplay_common::game::native_configs::NativeConfigsGameExt1 as _;

use sunrise_gameplay_common::game::native_entity_refs_common::NativeEntityRefsCommonGameExt1 as _;

use sunrise_gameplay_common::game::native_hooks::SharedGameExt1 as _;

use sunrise_gameplay_common::game::native_lifecycle_common::NativeLifecycleCommonGameExt1 as _;

use sunrise_gameplay_common::game::piston_push::carrying::CarryingGameExt1 as _;
