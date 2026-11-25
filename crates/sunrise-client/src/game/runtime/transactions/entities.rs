//! Entities a transaction adds and removes (#796). An abort takes out every entity that joined
//! since the transaction opened, with its attachment map, residency and saved memory, and tells
//! no one. A removal inside one only marks the entity, whose handle reads stale at once; the root
//! commit removes it as a removal outside one does, and an abort unmarks it. While one is open in
//! the level the game is in, mods hear of joins and leaves at the root commit only (the joins, in
//! the order the entities were added, before the removals' effects), and an abort tells them
//! nothing. Construction (`entity_constructing`, a mob's finalize) stays at the spawn
//! call. A transaction journals the level it was opened in only: another level's spawns, removals
//! and anonymous records (a command run `in` it) are told and kept at once, and an abort leaves them.
//! While a root commit tells, whatever its listeners ask for waits behind it (`Committing`).
//!
//! Join numbers, not ids, name what a transaction added: every kind takes one as it is put in its
//! list, the kinds whose id comes late too, so an aborted spawn just burns its id. Bolts and
//! thrown potions have none and are kept by their lists' lengths. A traveller that arrived from
//! another level inside the transaction is left where it arrived: travel is not journaled.
use super::super::{Dim, Game, commands::Target};
use super::Level;
use mod_api::EntityRef;
use std::collections::{HashMap, HashSet};
use sunrise_client::join_seq::JoinSeq;

/// A raw record of `persist.anon` (kind and position bits) as it was: what an aborted spawn
/// puts back.
pub(in crate::game) type KeptAnon = (
    (String, [u64; 3]),
    Option<(sunrise_core::geometry::Vec3, sunrise_core::nbt::Tag)>,
);

/// What a transaction did to entities, for its close.
#[derive(Default)]
pub(in crate::game) struct EntityJournal {
    /// Below the join number of every entity added in the transaction (`JoinSeq::upcoming` as
    /// it opened).
    pub(in crate::game) mark: JoinSeq,
    /// How many thrown potions there were as it opened.
    pub(in crate::game) potions: usize,
    /// The travellers that arrived from another level, by the ids they arrived under.
    pub(in crate::game) arrivals: HashSet<u32>,
    /// The removals asked for, in order: each runs at the root commit.
    pub(in crate::game) removals: Vec<Removal>,
    /// Each mob's `removed` flag before the transaction first removed it.
    pub(in crate::game) mob_removed: HashMap<u32, bool>,
    /// The join and leave announcements held for the root commit, in order.
    pub(in crate::game) announcements: Vec<Announcement>,
    /// Each raw record a spawn wrote over in `persist.anon` (by kind and position: an end
    /// crystal standing there keeps its own there), as it was before the first such spawn.
    pub(in crate::game) anon: Vec<KeptAnon>,
}

/// A removal waiting for the root commit.
pub(in crate::game) enum Removal {
    /// The mob is marked already; a killed one's effects that act on removal wait.
    Mob {
        id: u32,
        killed: bool,
    },
    Other(EntityRef),
}

/// What mods are told of entities joining and leaving, held while a transaction is open.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::game) enum Announcement {
    /// `native_joins_flush` of the entities that joined through `through`, as the caller said
    /// whether they came with a chunk.
    Joins {
        loaded_from_disk: bool,
        through: JoinSeq,
    },
    /// `native_entity_left`.
    Left(u32),
    /// `native_leaves_flush`.
    Leaves,
}

/// One place in a commit's queue.
pub(in crate::game) enum Step {
    Tell(Announcement),
    /// The removals of a transaction, run once its joins are told.
    Remove(Vec<Removal>),
}

impl Step {
    /// A journal's steps: its joins, then its removals, then the rest of what it held.
    pub(in crate::game) fn of(
        announcements: Vec<Announcement>,
        removals: Vec<Removal>,
    ) -> Vec<Step> {
        let (joins, rest): (Vec<_>, Vec<_>) = (announcements.into_iter())
            .partition(|announcement| matches!(announcement, Announcement::Joins { .. }));
        let steps = joins.into_iter().map(Step::Tell);
        let steps = steps.chain([Step::Remove(removals)]);
        steps.chain(rest.into_iter().map(Step::Tell)).collect()
    }
}

/// A root commit telling its joins and leaves, kept in `OpenTransactions` while it does. Whatever
/// a listener of the commit asks for meanwhile (a spawn, a removal, a transaction of its own) is
/// queued behind what is being told, so no flush a listener starts sees the commit's
/// half-told batches: the entities the transaction added and removed are never announced,
/// and each batch keeps its `loaded_from_disk` and bound.
pub(in crate::game) struct Committing {
    pub(in crate::game) dimension: Dim,
    /// The entities the transaction added and removed: never told to join.
    pub(in crate::game) removed: Vec<u32>,
    /// What is left to tell, in the order it was asked for: the transaction's own
    /// announcements, then those the listeners ask for. Entries stay, so an index is a place.
    pub(in crate::game) queue: Vec<Step>,
    /// The entity whose removal is running: it reads as present until it has run, as it does
    /// outside a transaction.
    pub(in crate::game) removing: Option<u32>,
}

/// Takes the commit it pushed, and any a listener's left, off the stack when it ends, however
/// it ends.
pub(in crate::game) struct Telling<'a, 'g> {
    pub(in crate::game) game: &'a mut Game<'g>,
    /// The stack's length before the commit was pushed: its place.
    pub(in crate::game) frame: usize,
}

impl Drop for Telling<'_, '_> {
    fn drop(&mut self) {
        self.game.transactions.committing.truncate(self.frame);
    }
}

/// The innermost commit being told in `dimension`: a listener's commit in another level sits
/// above it on the stack and must not hide it.
pub(in crate::game) fn committing_in(
    commits: &mut [Committing],
    dimension: Dim,
) -> Option<&mut Committing> {
    commits
        .iter_mut()
        .rev()
        .find(|commit| commit.dimension == dimension)
}

pub(in crate::game) fn is_committing(commits: &[Committing], dimension: Dim) -> bool {
    commits.iter().any(|commit| commit.dimension == dimension)
}

/// The innermost open transaction of `dimension`: it journals what happens in that level only,
/// whatever other levels' transactions are open inside it (a command run `in` another level can
/// open one). What happens in a level with none open (a command run `in` it from another
/// level's transaction) is not journaled.
pub(in crate::game) fn open_in(transactions: &mut [Level], dimension: Dim) -> Option<&mut Level> {
    transactions
        .iter_mut()
        .rev()
        .find(|level| level.dimension == Some(dimension))
}

impl EntityJournal {
    /// A nested commit: the parent's mark is earlier and stays.
    pub(in crate::game) fn merge(&mut self, child: Self) {
        self.arrivals.extend(child.arrivals);
        self.removals.extend(child.removals);
        for (id, was) in child.mob_removed {
            self.mob_removed.entry(id).or_insert(was);
        }
        for kept in child.anon {
            self.keep_anon(kept);
        }
        for announcement in child.announcements {
            self.announce(announcement);
        }
    }
    /// A nested abort: its arrivals stay out of the parent's abort too.
    pub(in crate::game) fn keep_arrivals(&mut self, arrivals: HashSet<u32>) {
        self.arrivals.extend(arrivals);
    }
    /// Keeps the first record each key had: a later spawn there wrote over this one's.
    fn keep_anon(&mut self, kept: KeptAnon) {
        if !self.anon.iter().any(|(key, _)| *key == kept.0) {
            self.anon.push(kept);
        }
    }
    fn announce(&mut self, announcement: Announcement) {
        match (self.announcements.last_mut(), announcement) {
            // One flush covers both when they were told alike.
            (
                Some(Announcement::Joins {
                    loaded_from_disk: held,
                    through: held_through,
                }),
                Announcement::Joins {
                    loaded_from_disk,
                    through,
                },
            ) if *held == loaded_from_disk => *held_through = (*held_through).max(through),
            (Some(last), announcement) if *last == announcement => {}
            _ => self.announcements.push(announcement),
        }
    }
}

impl Game<'_> {
    /// The journal a transaction opening now starts with.
    pub(in crate::game) fn entity_journal_open(&self) -> EntityJournal {
        EntityJournal {
            mark: JoinSeq::upcoming(),
            potions: self.potions.thrown.len(),
            ..EntityJournal::default()
        }
    }
    /// Holds `announcement` for the root commit when a transaction is open in the level the game
    /// is in, or for the end of the commit being told in it: true when it waits.
    pub(in crate::game) fn hold_announcement(&mut self, announcement: Announcement) -> bool {
        let dim = self.dim;
        if let Some(level) = open_in(&mut self.transactions, dim) {
            level.lifetimes.announce(announcement);
            return true;
        }
        match committing_in(&mut self.transactions.committing, dim) {
            Some(commit) => {
                commit.queue.push(Step::Tell(announcement));
                true
            }
            None => false,
        }
    }
    /// The join number the joins held in the level in place reach: those entities were
    /// constructed when they were held.
    pub(in crate::game) fn held_joins_through(&self) -> JoinSeq {
        let dim = self.dim;
        let held = |announcement: &Announcement| match announcement {
            Announcement::Joins { through, .. } => Some(*through),
            _ => None,
        };
        let open = (self.transactions.iter())
            .filter(|level| level.dimension == Some(dim))
            .flat_map(|level| level.lifetimes.announcements.iter().filter_map(held));
        let told = (self.transactions.committing.iter())
            .filter(|commit| commit.dimension == dim)
            .flat_map(|commit| commit.queue.iter())
            .filter_map(|step| match step {
                Step::Tell(announcement) => held(announcement),
                Step::Remove(_) => None,
            });
        open.chain(told).max().unwrap_or_default()
    }
    /// A spawn wrote over `was` in `persist.anon` under `key`: an abort puts it back.
    pub(in crate::game) fn journal_anon(&mut self, kept: KeptAnon) {
        let dim = self.dim;
        if let Some(level) = open_in(&mut self.transactions, dim) {
            level.lifetimes.keep_anon(kept);
        }
    }
    /// Entity `id` arrived from another level: an abort leaves it where it is.
    pub(in crate::game) fn journal_arrival(&mut self, id: u32) {
        let dim = self.dim;
        if let Some(level) = open_in(&mut self.transactions, dim) {
            level.lifetimes.arrivals.insert(id);
        }
    }
    /// Whether an open transaction removes `target`, which reads as gone until it closes.
    pub(in crate::game) fn removal_pending(&self, target: &Target) -> bool {
        self.removal_pending_in(self.dim, target)
    }
    /// [`Self::removal_pending`] of the entity in `dimension`, parked or not: the transactions
    /// stay with the game when a level is swapped out.
    pub(in crate::game) fn removal_pending_in(&self, dimension: Dim, target: &Target) -> bool {
        let id = super::super::native_entities::target_id(*target);
        let open = self.transactions.iter().any(|level| {
            level.dimension == Some(dimension)
                && (level.lifetimes.removals.iter())
                    .any(|removal| matches!(removal, Removal::Other(e) if Some(e.id()) == id))
        });
        // A commit's removals wait for its joins to be told: its entities read as gone until then.
        open || (self.transactions.committing.iter()).any(|commit| {
            commit.dimension == dimension
                && id.is_some_and(|id| commit.removed.contains(&id) && commit.removing != Some(id))
        })
    }
    /// `entity_remove` of `target`, which `entity` names, inside a transaction: marked now and
    /// removed at the root commit. False, with nothing done, when none is open in this level.
    pub(in crate::game) fn journal_removal(
        &mut self,
        entity: EntityRef,
        target: Target,
        killed: bool,
    ) -> bool {
        let dim = self.dim;
        let Some(level) = open_in(&mut self.transactions, dim) else {
            return false;
        };
        match target {
            Target::Mob(id) => {
                for mob in self.mobs.iter_mut().filter(|m| m.id == id) {
                    level.lifetimes.mob_removed.entry(id).or_insert(mob.removed);
                    mob.removed = true;
                }
                level.lifetimes.removals.push(Removal::Mob { id, killed });
            }
            _ => level.lifetimes.removals.push(Removal::Other(entity)),
        }
        true
    }
}

/// Where an abort leaves the way back for a map whose entity was taken for a transfer and has
/// not arrived yet: its arrival puts these contents in place of the map it carries. It lives
/// with the transfer's own state, so it goes with it if the arrival never comes.
pub(in crate::game) type ArrivalRollback =
    std::sync::Arc<std::sync::Mutex<Option<sunrise_mod_runtime::attachments::Attachments>>>;

/// The index registration of an entity that left this level inside a transaction, with its
/// lifetime slot: both retire when the root closes, committed or not, since travel is not
/// undone. Until then the source level's index keeps the rows it had at the take.
pub(in crate::game) struct Departure {
    lifetime: sunrise_mod_runtime::attachments::AttachmentLifetime,
    registration: Option<sunrise_mod_runtime::attachments::AttachmentRegistration>,
}

impl Drop for Departure {
    fn drop(&mut self) {
        self.lifetime.retire();
        if let Some(registration) = &self.registration {
            registration.unregister();
        }
    }
}

fn remove_entity_attachment(
    attachments: &mut HashMap<super::AttachmentKey, super::KeptAttachments>,
    id: u32,
) -> Option<super::KeptAttachments> {
    let key = attachments
        .keys()
        .find(|key| {
            matches!(
                key,
                super::AttachmentKey::Level(
                    _,
                    sunrise_mod_runtime::attachments::Holder::Entity(entity),
                ) if *entity == id
            )
        })
        .copied()?;
    attachments.remove(&key)
}

impl Game<'_> {
    /// Entity `id` is leaving this level with `map` inside a transaction: its index rows stay
    /// until the root closes, so a snapshot inside the action sees no gap.
    pub(in crate::game) fn defer_departure(
        &mut self,
        id: u32,
        map: Option<&mut sunrise_mod_runtime::attachments::Attachments>,
    ) {
        use super::super::entity_residency::SlotAccess;
        let lifetime = std::num::NonZeroU32::new(id)
            .map(|id| self.with_resident_slot(&self.dim.key(), id, std::mem::take))
            .and_then(|access| match access {
                SlotAccess::Done(lifetime) => Some(lifetime),
                _ => None,
            })
            .unwrap_or_default();
        let registration = map.and_then(|map| map.take_index_binding());
        if let Some(level) = self.transactions.last_mut() {
            level.departed.push(Departure {
                lifetime,
                registration,
            });
        }
    }
    /// Entity `id` is taken for a transfer inside a transaction, its map kept under `id`: the
    /// slot its arrival reads an abort's way back from, if the abort comes first.
    pub(in crate::game) fn journal_in_flight(&mut self, id: u32) -> ArrivalRollback {
        let slot = ArrivalRollback::default();
        if let Some(level) = self.transactions.last_mut() {
            level.in_flight.insert(id, slot.clone());
        }
        slot
    }
    /// An abort of `level` before its in-flight entities arrived: each one's kept map goes to its
    /// arrival's slot, not under an id that names no entity. An enclosing transaction takes the
    /// slots on, so its own abort can put back what it kept, which is older.
    pub(in crate::game) fn roll_back_in_flight(&mut self, level: &mut Level) {
        for (id, slot) in std::mem::take(&mut level.in_flight) {
            // 🤖 Agent: as in `journal_travel`, both are consumed and the travelled one, kept
            // before the entity arrived under `id`, is the older and the one put back.
            let travelled = level.travelled.remove(&id);
            let kept = remove_entity_attachment(&mut level.attachments, id);
            if let Some(kept) = travelled.or(kept) {
                *slot.lock().unwrap_or_else(|error| error.into_inner()) = Some(kept.contents);
            }
            if let Some(outer) = self.transactions.last_mut() {
                outer.in_flight.entry(id).or_insert(slot);
            }
        }
    }
    /// The map the transactions kept under `source` went with its entity, which arrived as
    /// `arrived`: an abort puts the kept contents back there, never under the old id.
    pub(in crate::game) fn journal_travel(&mut self, source: u32, arrived: u32) {
        for level in self.transactions.iter_mut() {
            level.in_flight.remove(&source);
            // 🤖 Agent: both are consumed, so nothing stays behind under `source`. A map that
            // travelled to `source` was kept before anything kept it under `source`, so its
            // travelled entry is the older one, and the one an abort restores.
            let travelled = level.travelled.remove(&source);
            let kept = remove_entity_attachment(&mut level.attachments, source);
            if let Some(kept) = travelled.or(kept) {
                level.travelled.entry(arrived).or_insert(kept);
            }
        }
    }
    /// An abort's way back for the maps that travelled: their contents as the transaction first
    /// reached them, wherever their entity is now.
    pub(in crate::game) fn restore_travelled(
        &mut self,
        travelled: HashMap<u32, super::KeptAttachments>,
    ) {
        for (id, kept) in travelled {
            if let Some(map) = self.entity_attachments.get_mut(&id) {
                map.replace_contents(kept.contents);
            }
        }
    }
    /// A closed transaction's departures go to the one around it, or retire at the root.
    pub(in crate::game) fn close_departures(&mut self, departed: Vec<Departure>) {
        if let Some(outer) = self.transactions.last_mut() {
            outer.departed.extend(departed);
        }
    }
}
