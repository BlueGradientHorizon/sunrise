//! Entities for native server mods (docs/modding/src/how-to/native-mods.md, "Entities"): spawn
//! any entity from its compound as `/summon` does, and move, show or remove
//! the entities Sunrise keeps without simulating them (displays, markers,
//! interactions); `native_display_refs.rs` poses displays. They act where
//! the world is simulated, in the level in place, which each names by its
//! `dimension` (`minecraft:overworld`) as `native_set_block` does.
use super::{
    Game,
    commands::{Target, place_summoned, strip_finalize_tree},
};
use sunrise_core::geometry::Vec3;
use sunrise_core::nbt::{Compound, Tag};
use sunrise_gameplay_common::game::commands_common::target_id;

/// Whether a position is one `/summon` takes.
fn position_ok(pos: [f64; 3]) -> bool {
    pos.iter().all(|v| v.is_finite())
        && pos[0].abs() < 30_000_000.0
        && pos[2].abs() < 30_000_000.0
        && (-20_000_000.0..20_000_000.0).contains(&pos[1])
}

/// The id of a summoned stack's root: its command target's, else `first`,
/// the first id handed out, when an entity no command target stands for
/// (`live`) holds it.
fn root_id(target: Option<Target>, first: u32, live: impl Fn(u32) -> bool) -> Option<u32> {
    match target {
        Some(t) => target_id(t),
        None => live(first).then_some(first),
    }
}

impl crate::game::ServerGame<'_> {
    /// Whether this game simulates the level `dimension`.
    fn native_simulates_level(&self, dimension: &str) -> bool {
        (self.server.serving.is_some() || self.common.options.dedicated.is_some())
            && dimension == self.common.dim.id()
    }
    /// Spawns an entity of `kind` (`minecraft:item_display`) at `pos` in the
    /// level in place, `dimension`, from `nbt` (its compound, as `/summon
    /// <kind> <pos> <nbt>` takes it; passengers included): its id. None on a
    /// client's copy of the world, for another dimension, a position outside
    /// `/summon`'s bounds, a chunk that isn't loaded, a kind Sunrise can't
    /// load, one a mod kept out (`native_entity_join_level`), or with the entity ids exhausted.
    /// Inside a transaction the id comes back at once: a join a mod cancels at the root commit
    /// leaves its handle stale.
    pub(in crate::game) fn summon(
        &mut self,
        dimension: &str,
        kind: &str,
        pos: [f64; 3],
        nbt: Compound,
    ) -> Option<u32> {
        if self.entity_ids_exhausted() {
            return None;
        }
        if !self.native_simulates_level(dimension) || !position_ok(pos) {
            return None;
        }
        let chunk = ((pos[0].floor() as i32) >> 4, (pos[2].floor() as i32) >> 4);
        if !self.common.world.chunks.contains_key(&chunk) {
            return None;
        }
        let mut tag = nbt;
        tag.insert("id".into(), Tag::String(kind.into()));
        // As `/summon <kind> <pos> <nbt>` takes it: with an NBT argument vanilla
        // does not finalize the mob (`SummonCommand.createEntity`), so a
        // `sunrise:finalize` in it is not the caller's to set.
        let mut tag = Tag::Compound(tag);
        strip_finalize_tree(&mut tag);
        // As `/summon` does: every entity of the stack, passengers included,
        // stands at `pos`.
        place_summoned(&mut tag, pos);
        let from = self.common.mobs.len();
        let first = self.server.next_entity + 1;
        let deferred = (
            self.common.tnts.len(),
            self.common.falling.len(),
            self.common.rockets.len(),
        );
        let target = self.spawn_entity_from_nbt(tag);
        // These roots take their id as they load, and a rocket has no command
        // Target variant. Select the newly appended root, never an older waiting entity.
        let root = match kind {
            "minecraft:tnt" if deferred.0 < self.common.tnts.len() => {
                Some(super::entity_residency::assign_resident_id!(
                    self.tnts,
                    deferred.0,
                    e,
                    &mut self.side.next_entity,
                    None
                ))
            }
            "minecraft:falling_block" if deferred.1 < self.common.falling.len() => {
                Some(super::entity_residency::assign_resident_id!(
                    self.falling,
                    deferred.1,
                    f,
                    &mut self.side.next_entity,
                    Some(&f.attachment_lifetime)
                ))
            }
            "minecraft:firework_rocket" if deferred.2 < self.common.rockets.len() => {
                Some(super::entity_residency::assign_resident_id!(
                    self.rockets,
                    deferred.2,
                    e,
                    &mut self.side.next_entity,
                    None
                ))
            }
            _ => None,
        };
        self.native_mobs_joined(from, false);
        // Otherwise the root is the first entity spawned, also for kinds no
        // command target stands for (experience orbs, end crystals, cushions). An id handed out to an
        // entity a mod then kept out is not given back, so the root must be
        // there.
        root.or_else(|| {
            root_id(target, first, |id| {
                self.common.orbs.iter().any(|o| o.id == id)
                    || self.common.crystals.iter().any(|c| c.id == id)
                    || self.common.cushions.iter().any(|c| c.id == id)
            })
        })
    }
}

/// The entities the walk does not hold: Sunrise keeps these in lists of their own.
#[derive(Clone, Copy)]
enum Apart {
    Crystal(usize),
    Cushion(usize),
    Painting(usize),
    Frame(usize),
    Other(usize),
    Dragon,
}

/// One entity a mod is told of: in the walk, or apart from it.
#[derive(Clone, Copy)]
enum Slot {
    Walk(super::entity::EntityHandle),
    Apart(Apart),
}

/// Entities joining and leaving the level (`native_entity_join_level`,
/// `native_entity_leave_level`), for every kind in the entity walk and the
/// families kept apart from it. Kept out of `native_hooks.rs`, whose lines may not hold macros.
impl crate::game::ServerGame<'_> {
    /// Whether an entity may have joined since the last announcement. A join
    /// in another level may cause one harmless walk of this level.
    fn joins_pending(&self) -> bool {
        sunrise_sim::join_seq::JoinSeq::latest() > self.common.persist.joined_seq
    }
    /// Tells `native_entity_join_level` once for every entity added since the
    /// last call, of any kind, in insertion order. A mod that cancels takes
    /// the entity back out here,
    /// before it ticks again, and `/summon` of it fails (`joins_canceled`).
    /// `loaded_from_disk` is for the entities that came with a chunk. Returns
    /// whether `watch` (an id, or 0) is still in the level. Walks the lists
    /// with static dispatch and allocates nothing but the kind name of an
    /// entity it announces; nothing runs when no mod is loaded, on a client
    /// of a server (which tells nothing), or when nothing is new.
    /// Inside a transaction open in this level, construction is still told now, and the join
    /// at its root commit (`transactions/entities.rs`), with this call's `loaded_from_disk`, in
    /// insertion order and before the transaction's removals take effect; the entity is kept
    /// until then. The same holds for a call a listener makes while that commit is told: its
    /// joins are told after the batch being told. A `through` is the commit's own flush of one
    /// batch, never held.
    pub(in crate::game) fn native_joins_flush(
        &mut self,
        loaded_from_disk: bool,
        watch: u32,
    ) -> bool {
        self.flush_joins(loaded_from_disk, watch, &[], None)
    }
    /// [`Self::native_joins_flush`], never announcing the entities `removed` names and, with a
    /// `through`, none that joined after it: a root commit's, for the entities its transaction
    /// added and removed, each held flush keeping its own `loaded_from_disk`.
    pub(in crate::game::server_side) fn flush_joins(
        &mut self,
        loaded_from_disk: bool,
        watch: u32,
        removed: &[u32],
        through: Option<sunrise_sim::join_seq::JoinSeq>,
    ) -> bool {
        if !self.common.is_logical_server()
            || mod_api::__private::mods().is_empty()
            || !self.joins_pending()
        {
            return true;
        }
        self.assign_pending_join_ids();
        // Read before this call's own announcement is held: it reaches the entities to construct.
        let held_before = self.common.held_joins_through();
        if through.is_none()
            && self.common.hold_announcement(
                sunrise_gameplay_common::game::transactions::Announcement::Joins {
                    loaded_from_disk,
                    through: sunrise_sim::join_seq::JoinSeq::latest(),
                },
            )
        {
            // What an earlier held flush of this level told is constructed already; with a toss
            // still asked it may have stopped short, so then everything since the mark is looked at.
            let mut mark = self.common.persist.joined_seq;
            if self.common.native_lifecycle.held_joins.is_empty() {
                mark = mark.max(held_before);
            }
            while let Some((slot, id, seq)) = self.next_unannounced(mark, &[], None) {
                mark = seq;
                let kind = self.slot_kind_and_position(slot).0.into_owned();
                self.construct_joining(id, &kind);
            }
            return true;
        }
        let latest = sunrise_sim::join_seq::JoinSeq::latest();
        let observed = through.map_or(latest, |through| through.min(latest));
        let mut kept = true;
        while let Some((slot, id, seq)) =
            self.next_unannounced(self.common.persist.joined_seq, removed, through)
        {
            self.common.persist.joined_seq = seq;
            let (kind, at) = self.slot_kind_and_position(slot);
            let kind = kind.into_owned();
            let mut canceled = false;
            let dimension = self.common.dim.id();
            self.construct_joining(id, &kind);
            self.native_entity_join_level(
                dimension,
                id,
                &kind,
                [at.x, at.y, at.z],
                loaded_from_disk,
                &mut canceled,
            );
            let spawn = self
                .common
                .native_lifecycle
                .pending_item_spawns
                .iter()
                .position(|entry| entry.0 == id)
                .map(|index| {
                    self.common
                        .native_lifecycle
                        .pending_item_spawns
                        .remove(index)
                });
            if canceled {
                self.common.persist.joins_canceled += 1;
                self.forget_entity(id);
                // Handles are indexes: the entity is taken out before any
                // other change to its list.
                self.discard_slot(slot);
                kept &= id != watch;
            } else {
                self.remember_join(id, kind);
                // `addEntity` files it under its section, untold.
                self.common.entity_section_joined(id, [at.x, at.y, at.z]);
                if let Some((id, item, count, position)) = spawn {
                    self.native_item_spawn(id, &item, count, position);
                }
            }
        }
        // A held toss is not past yet: the mark stays before it.
        if self.common.native_lifecycle.held_joins.is_empty() {
            self.common.persist.joined_seq = self.common.persist.joined_seq.max(observed);
        }
        kept
    }

    /// `Entity.<init>` comes before `addFreshEntity`: an entity that started on a path that
    /// doesn't tell it (`entity_initialization.rs`) is told here.
    fn construct_joining(&mut self, id: u32, kind: &str) {
        self.common
            .initialize_entity(mod_api::Holder::Entity(id), kind);
        // Constructor writes were canonical but unbound before insertion.
        // The real lifetime is now resident, before join listeners query it.
        if self
            .common
            .entity_attachments
            .get(&id)
            .is_some_and(|map| map.has_index_entries())
            && let Err(error) = self.bind_resident_entity_attachments(id)
        {
            log::warn!("constructor attachment binding failed for entity {id}: {error:?}");
        }
    }

    fn assign_pending_join_ids(&mut self) {
        self.assign_dragon_and_crystal_ids();
        use super::entity_residency::assign_resident_ids;
        assign_resident_ids!(self.tnts, t, &mut self.side.next_entity, None);
        assign_resident_ids!(
            self.falling,
            f,
            &mut self.side.next_entity,
            Some(&f.attachment_lifetime)
        );
        assign_resident_ids!(
            self.creatures.projectiles,
            p,
            &mut self.side.next_entity,
            Some(&p.attachment_lifetime)
        );
        assign_resident_ids!(self.tridents, t, &mut self.side.next_entity, None);
        assign_resident_ids!(self.rockets, r, &mut self.side.next_entity, None);
    }
    /// The entity inserted earliest after `mark` (and, with a `through`, no later than it), but
    /// the `skipped`.
    fn next_unannounced(
        &self,
        mark: sunrise_sim::join_seq::JoinSeq,
        skipped: &[u32],
        through: Option<sunrise_sim::join_seq::JoinSeq>,
    ) -> Option<(Slot, u32, sunrise_sim::join_seq::JoinSeq)> {
        use super::entity::Entity;
        use sunrise_sim::join_seq::{JoinSeq, Joined};
        let mut best: Option<(Slot, u32, JoinSeq)> = None;
        let mut offer = |slot: Slot, id: u32, seq: JoinSeq| {
            if skipped.contains(&id) || self.common.persist.removals.iter().chain(self.common.persist.destroying_vehicles.iter()).any(|target| {
                matches!(*target, super::commands::Target::Boat(n) | super::commands::Target::Cart(n) if n == id)
            }) {
                return;
            }
            if seq > mark
                && through.is_none_or(|through| seq <= through)
                && best.is_none_or(|(_, _, previous)| seq < previous)
            {
                best = Some((slot, id, seq));
            }
        };
        for handle in self.entity_handles() {
            let (id, seq) = super::entity::with_entity!(self, handle, e => (e.id(), e.join_seq()));
            offer(Slot::Walk(handle), id, seq);
        }
        for (i, c) in self.common.crystals.iter().enumerate() {
            offer(Slot::Apart(Apart::Crystal(i)), c.id, c.join);
        }
        for (i, c) in self.common.cushions.iter().enumerate() {
            offer(Slot::Apart(Apart::Cushion(i)), c.id, c.join);
        }
        for (i, p) in self.common.paintings.iter().enumerate() {
            offer(Slot::Apart(Apart::Painting(i)), p.id, p.join);
        }
        for (i, f) in self.common.item_frames.iter().enumerate() {
            offer(Slot::Apart(Apart::Frame(i)), f.id, f.join);
        }
        if let Some(d) = self.common.dragon.as_ref() {
            offer(Slot::Apart(Apart::Dragon), d.id, d.join);
        }
        for (i, other) in self
            .server
            .others
            .0
            .iter()
            .enumerate()
            .filter(|(_, e)| !super::general::quarantined_kind(&e.kind))
        {
            offer(Slot::Apart(Apart::Other(i)), other.id, other.join);
        }
        // A toss still being asked (`item_toss`) ends the flush: it and what joined after wait.
        best.filter(|&(_, id, _)| !self.common.native_lifecycle.held_joins.contains(&id))
    }

    fn remember_join(&mut self, id: u32, kind: String) {
        let index = self
            .common
            .persist
            .live
            .binary_search_by_key(&id, |entry| entry.0)
            .unwrap_or_else(|i| i);
        self.common.persist.live.insert(index, (id, kind, true));
    }
    /// The type id and position a mod is told of.
    fn slot_kind_and_position(&self, slot: Slot) -> (std::borrow::Cow<'_, str>, Vec3) {
        use super::entity::Entity;
        let block = |p: [i32; 3]| {
            Vec3::new(
                f64::from(p[0]) + 0.5,
                f64::from(p[1]) + 0.5,
                f64::from(p[2]) + 0.5,
            )
        };
        match slot {
            Slot::Walk(h) => {
                super::entity::with_entity!(self, h, e => (e.native_type(), e.position()))
            }
            Slot::Apart(Apart::Crystal(i)) => (
                "minecraft:end_crystal".into(),
                self.common.crystals[i].position,
            ),
            Slot::Apart(Apart::Cushion(i)) => {
                ("minecraft:cushion".into(), self.common.cushions[i].position)
            }
            Slot::Apart(Apart::Painting(i)) => (
                "minecraft:painting".into(),
                block(self.common.paintings[i].pos),
            ),
            Slot::Apart(Apart::Frame(i)) => {
                let f = &self.common.item_frames[i];
                let kind = if f.glow {
                    "minecraft:glow_item_frame"
                } else {
                    "minecraft:item_frame"
                };
                (kind.into(), block(f.pos))
            }
            Slot::Apart(Apart::Other(i)) => (
                self.server.others.0[i].kind.as_str().into(),
                self.server.others.0[i].position,
            ),
            Slot::Apart(Apart::Dragon) => (
                "minecraft:ender_dragon".into(),
                self.common
                    .dragon
                    .as_ref()
                    .map_or(Vec3::ZERO, |d| d.position),
            ),
        }
    }
    /// Takes the entity out of the level, before it ever ticked.
    fn discard_slot(&mut self, slot: Slot) {
        match slot {
            Slot::Walk(h) => self.discard_entity(h),
            Slot::Apart(Apart::Crystal(i)) => drop(self.common.crystals.remove(i)),
            Slot::Apart(Apart::Cushion(i)) => drop(self.common.cushions.remove(i)),
            Slot::Apart(Apart::Painting(i)) => drop(self.common.paintings.remove(i)),
            Slot::Apart(Apart::Frame(i)) => drop(self.common.item_frames.remove(i)),
            Slot::Apart(Apart::Dragon) => self.discard_resident_dragon(),
            Slot::Apart(Apart::Other(i)) => drop(self.server.others.0.remove(i)),
        }
    }
    /// Entities that were moved, not added (a portal's arrival keeps being
    /// silent): everything now in the level counts as announced, and will
    /// be told when it leaves.
    pub(in crate::game) fn native_joins_skip(&mut self) {
        let observed = sunrise_sim::join_seq::JoinSeq::latest();
        if self.common.is_logical_server() && !mod_api::__private::mods().is_empty() {
            self.assign_pending_join_ids();
            while let Some((slot, id, seq)) =
                self.next_unannounced(self.common.persist.joined_seq, &[], None)
            {
                self.common.persist.joined_seq = seq;
                let kind = self.slot_kind_and_position(slot).0.into_owned();
                self.remember_join(id, kind);
            }
        }
        self.common.persist.joined_seq = self.common.persist.joined_seq.max(observed);
    }
    /// An entity taken out of the level without leaving it (an item through
    /// an End gateway): forgotten, not told of.
    pub(in crate::game::server_side) fn native_leave_skip(&mut self, id: u32) {
        self.common.entity_section_forget(id);
        if let Ok(i) = self.common.persist.live.binary_search_by_key(&id, |l| l.0) {
            self.common.persist.live.remove(i);
        }
    }
    /// Entity `id` has left the level: told once, if mods were told it joined.
    pub(in crate::game::server_side) fn native_entity_left(&mut self, id: u32) {
        if self
            .common
            .hold_announcement(sunrise_gameplay_common::game::transactions::Announcement::Left(id))
        {
            return;
        }
        self.entity_left_now(id);
    }
    /// [`Self::native_entity_left`] told now, though a transaction or a commit holds the rest:
    /// the root commit's own telling.
    pub(in crate::game::server_side) fn entity_left_now(&mut self, id: u32) {
        self.common.entity_section_forget(id);
        if let Ok(i) = self.common.persist.live.binary_search_by_key(&id, |l| l.0) {
            let (id, kind, _) = self.common.persist.live.remove(i);
            let dimension = self.common.dim.id();
            self.native_entity_leave_level(dimension, id, &kind);
        }
    }
    /// Tells `native_entity_leave_level` for every announced entity that is
    /// no longer in any list (died, despawned, picked up, burnt, blown up, gone
    /// through a portal, unloaded), lowest id first. Marks the entities still
    /// there in the registry and tells the rest: no allocation, and nothing
    /// runs when no mod is loaded or none was announced.
    pub(in crate::game) fn native_leaves_flush(&mut self) {
        self.native_removals_flush();
        if self.common.persist.live.is_empty()
            || self.common.hold_announcement(
                sunrise_gameplay_common::game::transactions::Announcement::Leaves,
            )
        {
            return;
        }
        self.leaves_now();
    }
    /// [`Self::native_leaves_flush`] past its guards and told now, though a transaction or a
    /// commit holds the rest: the root commit's own telling.
    pub(in crate::game::server_side) fn leaves_now(&mut self) {
        use super::entity::Entity;
        for l in &mut self.common.persist.live {
            l.2 = false;
        }
        let live = &mut self.common.persist.live;
        let mut seen = |id: u32| {
            if let Ok(i) = live.binary_search_by_key(&id, |l| l.0) {
                live[i].2 = true;
            }
        };
        super::entity::for_each_entity!(super::entity::entity_lists_of!(self), e => {
            seen(e.id());
        });
        self.common.crystals.iter().for_each(|c| seen(c.id));
        self.common.cushions.iter().for_each(|c| seen(c.id));
        self.common.paintings.iter().for_each(|p| seen(p.id));
        self.common.item_frames.iter().for_each(|f| seen(f.id));
        self.server.others.0.iter().for_each(|e| seen(e.id));
        if let Some(d) = self.common.dragon.as_ref() {
            seen(d.id);
        }
        while let Some(i) = self.common.persist.live.iter().position(|l| !l.2) {
            let (id, kind, _) = self.common.persist.live.remove(i);
            self.common.entity_section_forget(id);
            let dimension = self.common.dim.id();
            self.native_entity_leave_level(dimension, id, &kind);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::native_entity_refs::tests::collect_entities_in;
    use mod_api::ServerHost;

    // NBT loads assign ids; reinstall an unbound payload as a pending owner.

    #[test]
    fn a_passenger_without_a_position_stands_where_the_stack_is_summoned() {
        let mut rider = Compound::new();
        rider.insert("id".into(), Tag::String("minecraft:pig".into()));
        let mut root = Compound::new();
        root.insert("id".into(), Tag::String("minecraft:pig".into()));
        root.insert("Passengers".into(), Tag::List(vec![Tag::Compound(rider)]));
        let mut tag = Tag::Compound(root);
        place_summoned(&mut tag, [1.5, 70.0, -2.5]);
        let pos = Tag::List(
            [1.5, 70.0, -2.5]
                .map(|v: f64| Tag::Double(v.to_bits()))
                .to_vec(),
        );
        let Tag::Compound(root) = &tag else { panic!() };
        assert_eq!(root.get(&"Pos".into()), Some(&pos));
        let Some(Tag::List(riders)) = root.get(&"Passengers".into()) else {
            panic!("no passengers");
        };
        let Tag::Compound(rider) = &riders[0] else {
            panic!()
        };
        assert_eq!(rider.get(&"Pos".into()), Some(&pos));
    }

    #[test]
    fn an_orb_root_has_an_id_only_while_it_is_in_the_level() {
        // Spawned: the orb holds the first id.
        assert_eq!(root_id(None, 36, |id| id == 36), Some(36));
        // A mod kept it out: id 36 was handed out, but no entity holds it.
        assert_eq!(root_id(None, 36, |_| false), None);
        // A command target answers for itself.
        assert_eq!(root_id(Some(Target::Mob(40)), 36, |_| false), Some(40));
        assert_eq!(root_id(Some(Target::Player), 36, |_| true), None);
    }

    #[test]
    fn summon_positions_are_summons() {
        assert!(position_ok([0.5, 64.0, 0.5]));
        assert!(!position_ok([3.0e7, 64.0, 0.5]));
        assert!(!position_ok([0.5, f64::NAN, 0.5]));
    }
}
