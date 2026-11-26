//! `entity_entering_section` (`EntityEvent.EnteringSection`): vanilla tells it from
//! `Entity.setPosRaw`, whose `EntityInLevelCallback.onMove` (`PersistentEntitySectionManager`'s
//! callback on a server, `TransientEntitySectionManager`'s on a client) compares the section the
//! entity is filed under with the one its new position is in, and moves it at once when they
//! differ. Sunrise's kinds keep their positions in their own fields, so the hook is told from
//! the places that write them: [`EntitySections`] files each entity under its section, and each
//! position-write boundary hands it the entity's new position immediately after the write.
//!
//! The boundaries: an entity joining (the server's `native_joins_flush`, a client's
//! `AddEntity`) files it untold. The mob and item loops report each committed write while their
//! rows are resident again; minecarts, boats and the remaining server entities report after each
//! entity's own update. A client reports interpolation writes per entity. A teleport, a piston
//! push, a mod's position write, each move a server player sent and each packet that moves an
//! entity on a client are one each.
//! Every boundary that sees another section records a crossing from the section the entity was
//! filed under and tells it before the write returns, so two moves told apart are two crossings,
//! and a move out and back is two.
//!
//! A listener runs synchronously after the new section is filed. What it moves or removes is
//! deferred by the surrounding entity write and is never re-entered from its own call; listeners
//! should queue such work themselves until the write completes.
//!
//! Section entry order (`EntitySection` keeps its entities in the order they entered it, which
//! a chunk's save follows) does not depend on a listener: a server boundary that knows both
//! ends of the write compares their section coordinates (integers) and, only on a crossing,
//! tells the residency table (`EntityTable::entered_section`, one hash lookup per crossing).
//! A write inside one section costs the compare alone.
//!
//! With no listener a boundary is that compare and one subscription check, and nothing is
//! filed. With one, a boundary is a hash lookup and the reusable crossing record is told
//! without a per-frame scan.

#[path = "entity_sections/observer_cache.rs"]
pub(in crate::game) mod observer_cache;
#[path = "entity_sections/player.rs"]
pub mod player;
#[allow(unused_imports)]
pub(in crate::game) use player::PlayerSectionObserver;


use crate::game::CommonGame;
use mod_api::__private::game::BackendRefOf;
use mod_api::{DimensionKey, EntityRef, WorldEpoch};
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use sunrise_sim::mobs::living::section_writes::PositionWrite;

/// `SectionPos.blockToSectionCoord(Mth.floor(v))`.
pub fn section_coord(v: f64) -> i32 {
    sunrise_core::mth::floor(v) >> 4
}

pub fn section_of([x, y, z]: [f64; 3]) -> [i32; 3] {
    [section_coord(x), section_coord(y), section_coord(z)]
}

/// Whether a write from `from` to `to` leaves the section it was in: six integer shifts and a
/// compare, no allocation and no lookup.
pub fn crossed(from: [f64; 3], to: [f64; 3]) -> bool {
    section_of(from) != section_of(to)
}

/// A section change a boundary found: the entity, its old and its new section.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Crossing {
    pub(in crate::game) id: u32,
    pub(in crate::game) old: [i32; 3],
    pub(in crate::game) new: [i32; 3],
}

/// Where a boundary is: the world open and the level the entity is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scope {
    pub epoch: WorldEpoch,
    pub(in crate::game) level: DimensionKey,
}

/// The section each entity is filed under, and the crossings not told yet.
#[derive(Default)]
pub struct EntitySections {
    /// The world open the table was filled in: another one starts it over.
    pub(in crate::game) epoch: Option<WorldEpoch>,
    /// Each entity's level and section.
    pub(in crate::game) filed: HashMap<u32, (DimensionKey, [i32; 3])>,
    /// Crossings raised while a listener is already running. Ordinary writes are told
    /// immediately; a nested write waits until the current callback returns.
    pub(in crate::game) queue: Vec<Crossing>,
    /// The batch being told; kept for its capacity.
    pub(in crate::game) telling: Vec<Crossing>,
    /// A telling is under way: a listener's moves wait for the next one.
    pub(in crate::game) busy: bool,
    /// The mobs' position updates during their family's tick; kept for its capacity.
    pub ticked: Vec<PositionWrite>,
    /// Mobs that crossed into another section during their family's tick, in the order of the
    /// writes; told to the residency table once the pass has its rows back. Kept for its capacity.
    pub entered: Vec<(u32, [f64; 3])>,
    /// A projectile row is lent during its individual tick. Position reads during its write
    /// boundary use this value until the row is restored.
    pub(in crate::game) write_position: Option<(u32, [f64; 3])>,
    /// Projectile ids seen by the current pass; kept to forget rows removed during settlement.
    pub projectile_ids: Vec<u32>,
    /// Projectile ids resident after settlement; kept to avoid a fresh set per pass.
    pub projectile_live: sunrise_level::fast_hash::FxHashSet<u32>,
}

impl EntitySections {
    /// The table for `scope`'s world: another world's entries are dropped.
    pub(in crate::game) fn table(
        &mut self,
        scope: Scope,
    ) -> &mut HashMap<u32, (DimensionKey, [i32; 3])> {
        if self.epoch != Some(scope.epoch) {
            self.epoch = Some(scope.epoch);
            self.filed.clear();
            self.queue.clear();
        }
        &mut self.filed
    }

    /// `addEntity`: entity `id` is filed at `at`, untold. A new entity under an id seen before
    /// (a client's `AddEntity` replacing one) starts over too.
    pub fn placed(&mut self, scope: Scope, id: u32, at: [f64; 3]) {
        self.table(scope).insert(id, (scope.level, section_of(at)));
    }

    /// `setPosRaw`: entity `id` is at `to` now. Filed under another section of the same level,
    /// that is a crossing. One not filed in this level yet (it changed level, or its join is
    /// still to be told) is filed where it was, `from` when the boundary knows it, so a move
    /// before its join is told is a crossing too; otherwise it is filed untold.
    pub fn moved(
        &mut self,
        scope: Scope,
        id: u32,
        from: Option<[f64; 3]>,
        to: [f64; 3],
    ) {
        let new = section_of(to);
        let old = match self.table(scope).entry(id) {
            Entry::Occupied(mut entry) => {
                let (level, section) = entry.get_mut();
                if *level != scope.level {
                    *level = scope.level;
                    *section = new;
                    return;
                }
                std::mem::replace(section, new)
            }
            Entry::Vacant(entry) => {
                entry.insert((scope.level, new));
                match from {
                    Some(from) => section_of(from),
                    None => return,
                }
            }
        };
        if old != new {
            self.queue.push(Crossing { id, old, new });
        }
    }

    /// `id` is gone (removed, or respawned as a new entity under the same id): it is filed
    /// again where it next appears, untold.
    pub fn forget(&mut self, id: u32) {
        self.filed.remove(&id);
        // A nested listener may have queued a crossing immediately before it
        // removed this entity.  Do not deliver that stale write on the next
        // boundary (the hook never fires for an entity being removed).
        self.queue.retain(|crossing| crossing.id != id);
    }

    /// The crossings to tell now, or none while a telling is under way.
    pub fn begin_telling(&mut self) -> Option<Vec<Crossing>> {
        if self.busy || self.queue.is_empty() {
            return None;
        }
        self.busy = true;
        let mut batch = std::mem::take(&mut self.telling);
        std::mem::swap(&mut batch, &mut self.queue);
        Some(batch)
    }

    /// The telling is over; `batch` goes back for its capacity.
    pub fn end_telling(&mut self, mut batch: Vec<Crossing>) {
        batch.clear();
        self.telling = batch;
        self.busy = false;
    }
}

#[derive(Clone, Copy)]
pub(in crate::game) struct SectionWriter {
    pub(in crate::game) entity: EntityRef,
    pub(in crate::game) position: [f64; 3],
    pub(in crate::game) player: Option<PlayerMoveState>,
}

#[derive(Clone, Copy)]
pub(in crate::game) struct PlayerMoveState {
    pub(in crate::game) local_player: Option<mod_api::PlayerRef>,
    pub(in crate::game) on_ground: bool,
    pub(in crate::game) velocity: [f64; 3],
}

/// The Host used while the entity's world view is borrowed. It answers only
/// reads from the current write; other operations retain their typed refusal.
pub struct LiveSectionHost(pub(in crate::game) Option<SectionWriter>);

mod_api::impl_host_areas!(LiveSectionHost, except(datafix));
mod_api::impl_server_host_areas!(LiveSectionHost);
mod_api::impl_client_host_areas!(LiveSectionHost, except(client_render, menus, services));
mod_api::impl_backend_areas!(
    LiveSectionHost,
    partial(entity_position, entity_on_ground, entity_velocity)
);

impl mod_api::prelude::DatafixHost for LiveSectionHost {
    fn upgrade_nbt(
        &self,
        kind: mod_api::NbtDataType,
        data_version: i32,
        nbt: mod_api::Compound,
    ) -> Result<mod_api::Compound, mod_api::NbtUpgradeError> {
        crate::game::upgrade_nbt(kind, data_version, nbt)
    }
}

impl mod_api::prelude::ClientRenderClientHost for LiveSectionHost {
    fn biome_tint(
        &self,
        _: [i32; 3],
        _: mod_api::ColorResolver,
    ) -> Result<u32, mod_api::LevelReadError> {
        Err(mod_api::LevelReadError::Unavailable)
    }
}

impl mod_api::prelude::MenusClientHost for LiveSectionHost {
    fn local_player(&self) -> Option<mod_api::PlayerRef> {
        self.0?.player?.local_player
    }
}

impl mod_api::prelude::ServicesClientHost for LiveSectionHost {
    fn client_executor(&self) -> Option<mod_api::ClientExecutor> {
        None
    }
}

// The generated read backend forwards these snapshots and answers the
// standard no-backend result for reads that would need the borrowed world.
impl LiveSectionHost {
    pub(in crate::game) fn entity_position(&self, entity: EntityRef) -> Option<[f64; 3]> {
        let writer = self.0.filter(|writer| writer.entity == entity)?;
        Some(writer.position)
    }

    pub(in crate::game) fn entity_on_ground(&self, entity: EntityRef) -> Option<bool> {
        let player = self.0.filter(|writer| writer.entity == entity)?.player?;
        Some(player.on_ground)
    }

    pub(in crate::game) fn entity_velocity(&self, entity: EntityRef) -> Option<[f64; 3]> {
        let player = self.0.filter(|writer| writer.entity == entity)?.player?;
        Some(player.velocity)
    }
}

impl mod_api::Backend for LiveSectionHost {}

impl mod_api::Host for LiveSectionHost {
    fn backend(&self) -> Option<mod_api::BackendRef<'_>> {
        Some(mod_api::BackendRef::of(self))
    }
}

impl mod_api::ServerHost for LiveSectionHost {}
impl mod_api::ClientHost for LiveSectionHost {}

pub(in crate::game) fn tell_live(
    sections: &std::cell::RefCell<EntitySections>,
    writer: SectionWriter,
    server: bool,
) {
    let Some(batch) = sections.borrow_mut().begin_telling() else {
        return;
    };
    for crossing in &batch {
        let entity = EntityRef::new(writer.entity.epoch(), crossing.id);
        let mut host = LiveSectionHost((entity == writer.entity).then_some(writer));
        let host = if server {
            mod_api::HostSide::Server(&mut host)
        } else {
            mod_api::HostSide::Client(&mut host)
        };
        mod_api::dispatch_entity_entering_section(
            host,
            entity,
            crossing.old,
            crossing.new,
            crossing.old[0] != crossing.new[0] || crossing.old[2] != crossing.new[2],
        );
    }
    sections.borrow_mut().end_telling(batch);
}

/// Delivers a mob's captured teleport at the write boundary.  The observer
/// owns only the section table, so it can run while the mob's borrowed world
/// view is still live.
pub(crate) struct LiveSectionObserver {
    pub(in crate::game) sections: std::rc::Rc<std::cell::RefCell<EntitySections>>,
    pub(in crate::game) scope: Scope,
}

impl sunrise_sim::mobs::living::section_writes::WriteObserver for LiveSectionObserver {
    fn position_written(
        &mut self,
        id: u32,
        from: sunrise_core::geometry::Vec3,
        to: sunrise_core::geometry::Vec3,
    ) {
        self.sections
            .borrow_mut()
            .moved(self.scope, id, Some(from.components()), to.components());
        tell_live(
            &self.sections,
            SectionWriter {
                entity: EntityRef::new(self.scope.epoch, id),
                position: to.components(),
                player: None,
            },
            true,
        );
    }
}

impl crate::game::CommonGame<'_> {

    /// `Callback.onMove` for the residency table: a server entity's write from `from` to `to`
    /// that leaves its section puts it at the end of the section it enters, whether or not a
    /// hook listens. Cost: the integer compare of [`crossed`]; the table is touched only on a
    /// crossing.
    pub fn entity_section_entered(&mut self, id: u32, from: [f64; 3], to: [f64; 3]) {
        if crossed(from, to) && self.is_logical_server() {
            self.resident_entities
                .table
                .get_mut()
                .entered_section(id, to);
        }
    }
}

pub trait EntitySectionsGameExt1<'__sg>: crate::game::SideGame<'__sg> {
    /// Where boundaries are now: None with no listener (nothing is filed) or no world open.
     fn entity_section_scope(&self) -> Option<Scope> {
        if !mod_api::__private::listened(mod_api::HookId::EntityEnteringSection) {
            return None;
        }
        Some(Scope {
            epoch: self.native_lifecycle.epoch()?,
            level: self.native_level_key(),
        })
    }

    /// Entity `id` was put from `from` (when known) at `to`; tell it immediately after filing.
     fn entity_section_moved(
        &mut self,
        id: u32,
        from: Option<[f64; 3]>,
        to: [f64; 3],
    ) {
        self.side_pick_section_move(id, to);
        if let Some(from) = from {
            self.entity_section_entered(id, from, to);
        }
        if let Some(scope) = self.entity_section_scope() {
            self.native_lifecycle
                .entity_sections
                .moved(scope, id, from, to);
            self.tell_entity_sections();
        }
    }

    /// Entity `id`'s position was written: it is wherever it is now (any kind, either side).
     fn entity_section_moved_id(&mut self, id: u32) {
        self.side_pick_section_sync_moved_id(id);
        let Some(scope) = self.entity_section_scope() else {
            return;
        };
        if let Some(at) = self.entity_position(EntityRef::new(scope.epoch, id)) {
            self.native_lifecycle
                .entity_sections
                .moved(scope, id, None, at);
            self.tell_entity_sections();
        }
    }

    /// Entity `id` is gone, or replaced under the same id: it starts over untold.
     fn entity_section_forget(&mut self, id: u32) {
        self.side_pick_section_remove(id);
        self.native_lifecycle.entity_sections.forget(id);
        self.native_lifecycle
            .entity_sections
            .projectile_live
            .remove(&id);
    }

    /// Tell the writes from one lifted mob while its owning list is resident.
    ///
    /// A mob tick cannot borrow `Game` while its world view is live, so the
    /// caller restores the pass before entering here. The resident row is
    /// temporarily moved to each recorded write while its crossing is told;
    /// this gives listeners the same position they would see at the write and
    /// preserves a listener's own position change.
     fn entity_sections_ticked_live(
        &mut self,
        sections: &mut EntitySections,
        moves: &mut Vec<PositionWrite>,
    ) {
        let Some(scope) = self.entity_section_scope() else {
            moves.clear();
            return;
        };
        let mut indices = sunrise_level::fast_hash::FxHashMap::default();
        indices.extend(
            self.mobs
                .iter()
                .enumerate()
                .map(|(index, mob)| (mob.join, index)),
        );
        for write in moves.drain(..) {
            if write.delivered {
                continue;
            }
            let Some(&index) = indices.get(&write.join) else {
                sections.forget(write.id);
                continue;
            };
            let final_position = self.mobs[index].position;
            self.mobs.update(index, |mob| mob.position = write.to);
            sections.moved(
                scope,
                write.id,
                write.from.map(|from| [from.x, from.y, from.z]),
                [write.to.x, write.to.y, write.to.z],
            );
            self.tell_entity_sections_in(sections);
            indices.clear();
            indices.extend(
                self.mobs
                    .iter()
                    .enumerate()
                    .map(|(index, mob)| (mob.join, index)),
            );
            if let Some(&index) = indices.get(&write.join) {
                self.mobs.update(index, |mob| {
                    if mob.position == write.to {
                        mob.position = final_position;
                    }
                });
            }
        }
    }

    /// Entity `id`'s position was written from `from` to `to` (`setPosRaw`) inside a family tick
    /// or while a rider is carried by a vehicle teleport. File and tell it at this write.
     fn entity_section_step(&mut self, id: u32, from: [f64; 3], to: [f64; 3]) {
        if crossed(from, to) {
            self.side_pick_section_move(id, to);
        }
        self.entity_section_entered(id, from, to);
        if id != 0
            && let Some(scope) = self.entity_section_scope()
        {
            self.native_lifecycle
                .entity_sections
                .moved(scope, id, Some(from), to);
            self.tell_entity_sections();
        }
    }

    /// Tell a write whose row is temporarily lent to a family pass. The callback sees the
    /// committed position without removing and reinserting the surrounding list tail.
     fn entity_section_step_visible(
        &mut self,
        id: u32,
        from: [f64; 3],
        to: [f64; 3],
    ) -> bool {
        let scope = self.entity_section_scope();
        self.native_lifecycle.entity_sections.write_position = Some((id, to));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.entity_section_step(id, from, to);
        }));
        self.native_lifecycle.entity_sections.write_position = None;
        if let Err(payload) = result {
            std::panic::resume_unwind(payload);
        }
        scope.is_none_or(|_| {
            self.native_lifecycle
                .entity_sections
                .filed
                .contains_key(&id)
        })
    }

    /// An entity joined (`addEntity`): filed untold where it is now, which a join listener may
    /// have changed from `at`, where it joined.
     fn entity_section_joined(&mut self, id: u32, at: [f64; 3]) {
        let Some(scope) = self.entity_section_scope() else {
            return;
        };
        let now = self.entity_position(EntityRef::new(scope.epoch, id));
        self.native_lifecycle
            .entity_sections
            .placed(scope, id, now.unwrap_or(at));
    }

    /// Tells `entity_entering_section` each crossing queued by a nested write, in order. Called
    /// from inside a listener it does nothing: the outer write is still in progress.
     fn tell_entity_sections(&mut self) {
        let Some(batch) = self.native_lifecycle.entity_sections.begin_telling() else {
            return;
        };
        if let Some(epoch) = self.native_lifecycle.epoch() {
            for c in &batch {
                let chunk_changed = c.old[0] != c.new[0] || c.old[2] != c.new[2];
                self.native_entity_entering_section(
                    EntityRef::new(epoch, c.id),
                    c.old,
                    c.new,
                    chunk_changed,
                );
            }
        }
        self.native_lifecycle.entity_sections.end_telling(batch);
    }

     fn tell_entity_sections_in(&mut self, sections: &mut EntitySections) {
        let Some(batch) = sections.begin_telling() else {
            return;
        };
        if let Some(epoch) = self.native_lifecycle.epoch() {
            for c in &batch {
                let chunk_changed = c.old[0] != c.new[0] || c.old[2] != c.new[2];
                self.native_entity_entering_section(
                    EntityRef::new(epoch, c.id),
                    c.old,
                    c.new,
                    chunk_changed,
                );
            }
        }
        sections.end_telling(batch);
    }
}
impl<'__sg, G: crate::game::SideGame<'__sg>> EntitySectionsGameExt1<'__sg> for G {}


impl LiveSectionObserver {
    pub(crate) fn new(
        sections: std::rc::Rc<std::cell::RefCell<EntitySections>>,
        scope: Scope,
    ) -> Self {
        Self { sections, scope }
    }
}

use crate::game::side_ops::modapi_modding::ModapiModdingSideGameExt as _;

use crate::game::native_entity_refs_common::NativeEntityRefsCommonGameExt1 as _;

use crate::game::native_hooks::entity_entering_section::GameHooks as _;
