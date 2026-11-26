//! Entities ticking for native mods (`native_entity_tick_pre`, `native_entity_tick_post`:
//! NeoForge's `EntityTickEvent.Pre` and `.Post`), on the server and on a client. An entity of a
//! type a mod named (`entity_tick_each`) is told alone, right before and right after its own
//! tick. The others are told by family: the loops of the mobs and the items hold their lists out
//! of `Game`, so those families hand their ticking entities to a `TickRound` before the loop, hear
//! one Pre for all of them before it and one Post after it; the minecarts, boats and general
//! entity rows, whose loops can call the hooks, are told one by one. The mob and item loops tell
//! a named entity's Pre and Post from inside, at a yield that gives their list back for the call.
//!
//! Each entity's position writes are filed for `entity_entering_section`
//! (`entity_sections.rs`) while its tick is still running: a round keeps its entities while
//! either hook is listened to.
use crate::game::CommonGame;
use std::sync::Arc;
use sunrise_sim::mobs::MobType;

/// The item entities' type, as the hooks name it.
pub(in crate::game) const ITEM: &str = "minecraft:item";
/// The players' type, as the hooks name it.
pub const PLAYER: &str = "minecraft:player";

/// The type of an entity in a round, as the hooks name it.
pub enum TickKind {
    Fixed(&'static str),
    Mob(Arc<MobType>),
}

impl TickKind {
    pub(in crate::game) fn id(&self) -> &str {
        match self {
            Self::Fixed(id) => id,
            Self::Mob(kind) => &kind.id,
        }
    }
}

/// The index in a family's list of the entity `is` picks out, looked for from `from` on first:
/// the list keeps its order, though a mod's hook may have taken entities out of it.
pub(in crate::game) fn place<T>(list: &[T], from: usize, is: impl Fn(&T) -> bool) -> Option<usize> {
    let from = from.min(list.len());
    list[from..]
        .iter()
        .position(&is)
        .map(|i| from + i)
        .or_else(|| list[..from].iter().position(is))
}

/// Where slot `slot`'s entity of a family's loop is in its list now: the slot itself until a hook
/// told during the loop changed the list (`places`, by id), then by its id (`slots`), if it is
/// still there.
pub fn slot_place(
    places: &Option<sunrise_level::fast_hash::FxHashMap<u32, usize>>,
    slots: &[u32],
    slot: usize,
) -> Option<usize> {
    match places {
        None => Some(slot),
        Some(places) => places.get(&slots[slot]).copied(),
    }
}

/// Where each entity of `ids` is, by id: a family's loop finds its rows with it once a hook told
/// during the loop changed the list.
pub fn places_of(
    ids: impl Iterator<Item = u32>,
) -> sunrise_level::fast_hash::FxHashMap<u32, usize> {
    ids.enumerate().map(|(i, id)| (id, i)).collect()
}

/// What a family's Pre and Post are told from, kept between ticks so that telling them
/// allocates nothing.
#[derive(Default)]
pub struct TickBuffers {
    pub(in crate::game) ids: Vec<u32>,
    /// Empty between calls: it borrows the round's kinds only while a hook runs.
    pub(in crate::game) kinds: Vec<&'static str>,
    /// The types of entities whose rows hold them as their own strings, copied out for a call.
    pub(in crate::game) names: KindNames,
}

/// Type names copied out of the rows that own them, so a hook is told them while the rows stay
/// in their list for handles to read: one string and where each name ends.
#[derive(Default)]
pub struct KindNames {
    text: String,
    ends: Vec<usize>,
}

impl KindNames {
    pub fn clear(&mut self) {
        self.text.clear();
        self.ends.clear();
    }
    pub fn push(&mut self, kind: &str) {
        self.text.push_str(kind);
        self.ends.push(self.text.len());
    }
    pub fn names(&self) -> impl Iterator<Item = &str> {
        let mut start = 0;
        self.ends.iter().map(move |&end| {
            let name = &self.text[start..end];
            start = end;
            name
        })
    }
}

/// Which entities of a round are told alone, around their own tick.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Alone {
    /// None: no mod named a type.
    #[default]
    None,
    /// Those of the types mods named (a mob type's `tick_alone`).
    Named,
    /// Every entity: a mod named every type.
    All,
}

/// Whether the tick hooks are heard on this side in this tick, and which entities are told alone:
/// asked once a tick (`native_tick_told`).
#[derive(Clone, Copy, Debug, Default)]
pub struct Told {
    /// Whether the tick hooks are listened to on this side: otherwise no Pre or Post is told.
    pub(in crate::game) listening: bool,
    /// Which entities are told alone instead of with their family.
    alone: Alone,
}

impl Told {
    /// Whether the tick hooks are listened to on this side.
    pub(in crate::game) fn listening(&self) -> bool {
        self.listening
    }
    /// Whether a mob of type `kind` is told alone, around its own tick, not with its family.
    pub(in crate::game) fn alone_mob(&self, kind: &MobType) -> bool {
        self.listening
            && match self.alone {
                Alone::None => false,
                Alone::Named => kind.flags.tick_alone,
                Alone::All => true,
            }
    }
    /// Whether the entities of type `kind` are told alone. For a family of one type
    /// (`minecraft:item`) it is asked once for the family, not for each entity.
    pub(in crate::game) fn alone_kind(&self, kind: &str) -> bool {
        self.listening
            && match self.alone {
                Alone::None => false,
                Alone::Named => {
                    mod_api::__private::entity_tick_each().is_some_and(|each| each.names(kind))
                }
                Alone::All => true,
            }
    }
}

/// The entities of one family in one tick.
#[derive(Default)]
pub struct TickRound {
    /// Whether the hooks are heard, and who is told alone.
    pub(in crate::game) told: Told,
    /// Whether `entity_entering_section` is listened to on this server: each entity's position
    /// write is filed and told while its tick is still running.
    pub(in crate::game) sections: bool,
    /// The entities about to tick with the family, in order; once the family's Pre is told,
    /// those it let through, for its Post.
    pub(in crate::game) ticking: Vec<(u32, TickKind)>,
    /// The entities a Pre canceled.
    pub(in crate::game) canceled: Vec<u32>,
    /// Scratch output owned by one hook dispatch, never shared with earlier turns.
    pub(in crate::game) call_canceled: Vec<u32>,
    /// The items whose part ended their tick (`on_entity_item_update`): they are not
    /// canceled, so they still hear their Post.
    pub(in crate::game) halted: Vec<u32>,
}

impl TickRound {
    /// Whether a mod canceled the tick of entity `id`.
    pub(in crate::game) fn canceled(&self, id: u32) -> bool {
        !self.canceled.is_empty() && self.canceled.contains(&id)
    }
    /// Whether a part ended the tick of item `id` (`on_entity_item_update`).
    pub fn halted(&self, id: u32) -> bool {
        !self.halted.is_empty() && self.halted.contains(&id)
    }
    /// A part ended the tick of item `id`.
    pub fn halt(&mut self, id: u32) {
        self.halted.push(id);
    }
    /// Whether `entity_entering_section` is listened to: each entity's position updates are filed.
    pub fn sections(&self) -> bool {
        self.sections
    }
    /// Whether the round keeps the entities that tick: a mod hears their ticks or their moves.
    pub(in crate::game) fn listening(&self) -> bool {
        self.told.listening || self.sections
    }
    /// Whether a mob of type `kind` is told alone, around its own tick, not with its family.
    pub(in crate::game) fn alone_mob(&self, kind: &MobType) -> bool {
        self.told.alone_mob(kind)
    }
    /// Whether the entities of type `kind` are told alone ([`Told::alone_kind`]).
    pub(in crate::game) fn alone_kind(&self, kind: &str) -> bool {
        self.told.alone_kind(kind)
    }
    /// The entity `id` of `kind` is about to tick with its family, which hears its Pre and its
    /// Posts together (`native_tick_pre_batch`).
    pub(in crate::game) fn add(&mut self, id: u32, kind: TickKind) {
        if self.listening() {
            self.ticking.push((id, kind));
        }
    }
    /// The `n`th entity, in order, that the family's Pre let through.
    pub fn let_through(&self, n: usize) -> Option<u32> {
        self.ticking.get(n).map(|(id, _)| *id)
    }
    /// Applies the call's output to this batch only, keeping existing cancellations intact.
    fn take_canceled(&mut self) {
        if self.call_canceled.is_empty() {
            return;
        }
        let mut kept = 0;
        for i in 0..self.ticking.len() {
            let id = self.ticking[i].0;
            if self.call_canceled.contains(&id) {
                self.canceled.push(id);
            } else {
                self.ticking.swap(kept, i);
                kept += 1;
            }
        }
        self.ticking.truncate(kept);
        self.call_canceled.clear();
    }
    /// The entity `id`, told its Pre, went before it ticked (an item merged into an earlier
    /// one): it hears no Post.
    pub(in crate::game) fn not_ticked(&mut self, id: u32) {
        if self.listening() {
            self.ticking.retain(|(other, _)| *other != id);
        }
    }
}

pub trait NativeTicksGameExt1<'__sg>: crate::game::SideGame<'__sg> {
    /// A round for a family about to tick, on the server or on a client, only while a mod
    /// listens (`native_entity_tick_listened`). Its list is kept between ticks.
     fn native_tick_round(&mut self) -> TickRound {
        let told = self.native_tick_told();
        let sections = self.side_tick_uses_server_sections();
        TickRound {
            told,
            sections,
            ticking: if told.listening || sections {
                std::mem::take(&mut self.creatures.tick_buffer)
            } else {
                Vec::new()
            },
            canceled: Vec::new(),
            call_canceled: Vec::new(),
            halted: Vec::new(),
        }
    }

    /// Whether the tick hooks are heard on this side (`native_entity_tick_listened`), and which
    /// entities are told alone.
     fn native_tick_told(&mut self) -> Told {
        let mut listening = false;
        self.native_entity_tick_listened(&mut listening);
        let alone = match mod_api::__private::entity_tick_each() {
            Some(each) if listening && each.all() => Alone::All,
            Some(each) if listening && each.any() => Alone::Named,
            _ => Alone::None,
        };
        Told { listening, alone }
    }

    /// `native_entity_tick_pre`, once for the family's entities the round holds (`add`): those
    /// a mod cancels leave it and do not tick.
     fn native_tick_pre_batch(&mut self, round: &mut TickRound) {
        if !round.told.listening || round.ticking.is_empty() {
            return;
        }
        let buffers = std::mem::take(&mut self.creatures.tick_hooks);
        let (mut ids, mut kinds): (_, Vec<&str>) = (buffers.ids, buffers.kinds);
        ids.extend(round.ticking.iter().map(|(id, _)| *id));
        kinds.extend(round.ticking.iter().map(|(_, kind)| kind.id()));
        self.native_entity_tick_pre(self.dim.id(), &ids, &kinds, &mut round.call_canceled);
        self.creatures.tick_hooks = TickBuffers::emptied(ids, kinds, buffers.names);
        round.take_canceled();
    }

    /// `native_entity_tick_post`, once for the family's entities the round let through, after
    /// their position writes have already filed and told their crossings.
     fn native_tick_post_batch(&mut self, mut round: TickRound) {
        if round.told.listening && !round.ticking.is_empty() {
            let buffers = std::mem::take(&mut self.creatures.tick_hooks);
            let (mut ids, mut kinds): (_, Vec<&str>) = (buffers.ids, buffers.kinds);
            ids.extend(round.ticking.iter().map(|(id, _)| *id));
            kinds.extend(round.ticking.iter().map(|(_, kind)| kind.id()));
            self.native_entity_tick_post(self.dim.id(), &ids, &kinds);
            self.creatures.tick_hooks = TickBuffers::emptied(ids, kinds, buffers.names);
        }
        if round.listening() {
            round.ticking.clear();
            self.creatures.tick_buffer = round.ticking;
        }
    }

    /// `native_entity_tick_pre` for one entity about to tick, told alone: whether it does.
     fn native_tick_pre_one(
        &mut self,
        round: &mut TickRound,
        id: u32,
        kind: &str,
    ) -> bool {
        if !round.told.listening {
            return true;
        }
        self.native_entity_tick_pre(
            self.dim.id(),
            std::slice::from_ref(&id),
            std::slice::from_ref(&kind),
            &mut round.call_canceled,
        );
        let canceled = round.call_canceled.contains(&id);
        round.call_canceled.clear();
        if canceled {
            round.canceled.push(id);
        }
        !canceled
    }

    /// `native_entity_tick_post` for one entity told alone, right after its tick, its move filed
    /// and told.
     fn native_tick_post_one(&mut self, round: &TickRound, id: u32, kind: &str) {
        if round.told.listening {
            self.native_entity_tick_post(
                self.dim.id(),
                std::slice::from_ref(&id),
                std::slice::from_ref(&kind),
            );
        }
    }
}
impl<'__sg, G: crate::game::SideGame<'__sg>> NativeTicksGameExt1<'__sg> for G {}


impl TickBuffers {
    /// The buffers back, emptied: collecting a vector's own iterator into one of the same
    /// layout keeps its allocation, so the kinds' buffer outlives the borrow it held.
    pub(in crate::game) fn emptied(
        mut ids: Vec<u32>,
        mut kinds: Vec<&str>,
        mut names: KindNames,
    ) -> Self {
        ids.clear();
        kinds.clear();
        names.clear();
        Self {
            ids,
            kinds: kinds.into_iter().map(|_| "").collect(),
            names,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round() -> TickRound {
        TickRound {
            told: Told {
                listening: true,
                alone: Alone::None,
            },
            ..TickRound::default()
        }
    }

    /// A canceled entity is skipped, and one that went before its tick (an item merged into an
    /// earlier one) hears no Post; a round nobody listens to keeps nothing.
    #[test]
    fn a_round_keeps_the_entities_that_tick() {
        let mut r = round();
        r.ticking.push((1, TickKind::Fixed("minecraft:item")));
        r.ticking.push((2, TickKind::Fixed("minecraft:item")));
        r.canceled.push(3);
        assert!(r.canceled(3));
        assert!(!r.canceled(1));
        r.not_ticked(2);
        let left: Vec<_> = r
            .ticking
            .iter()
            .map(|(id, kind)| (*id, kind.id()))
            .collect();
        assert_eq!(left, [(1, "minecraft:item")]);
        let mut quiet = TickRound::default();
        quiet.add(4, TickKind::Fixed("minecraft:item"));
        assert_eq!(quiet.let_through(0), None);
        assert!(!quiet.listening());
        quiet.not_ticked(1);
        assert!(quiet.ticking.is_empty() && !quiet.canceled(1));
    }

    /// A family's Pre cancels only entities it was told about, each once: the others keep
    /// their order and are let through, an ID it was not told about or told twice changes
    /// nothing, and the cancels a round already held stay.
    #[test]
    fn a_pre_cancels_only_the_entities_it_was_told() {
        let mut r = round();
        r.canceled.push(9);
        for id in [1, 2, 3, 4] {
            r.add(id, TickKind::Fixed("minecraft:item"));
        }
        r.call_canceled.extend([3, 7, 1, 3]);
        r.take_canceled();
        let through: Vec<_> = (0..).map_while(|n| r.let_through(n)).collect();
        assert_eq!(through, [2, 4]);
        assert_eq!(r.canceled, [9, 1, 3]);
        assert!(r.canceled(1) && r.canceled(3) && !r.canceled(7));
    }

    /// With no type named, nothing is told alone; with every type named, everything is, and a
    /// round nobody listens to tells nothing alone whatever was named.
    #[test]
    fn a_round_tells_alone_only_what_was_named() {
        let mut r = round();
        assert!(!r.alone_kind("minecraft:item"));
        r.told.alone = Alone::All;
        assert!(r.alone_kind("minecraft:item"));
        r.told.listening = false;
        assert!(!r.alone_kind("minecraft:item"));
    }

    /// An entity is found where it is now, from a hint on, and before the hint when an
    /// earlier one left the list.
    #[test]
    fn place_finds_an_entity_after_the_list_changed() {
        let list = [10, 20, 30, 40];
        assert_eq!(place(&list, 1, |&id| id == 30), Some(2));
        assert_eq!(place(&list, 3, |&id| id == 20), Some(1));
        assert_eq!(place(&list, 9, |&id| id == 40), Some(3));
        assert_eq!(place(&list, 0, |&id| id == 50), None);
    }
}

use crate::game::native_hooks::base::GameHooks as _;

use crate::game::side_ops::modapi_modding::ModapiModdingSideGameExt as _;
