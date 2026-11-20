//! Bulk item storage: [`STORAGE`], a block's items by kind, in 64-bit amounts and without slots,
//! for the stores, pipes and networks of a large base. Every block entity with slots answers it
//! through the game's own adapter over [`ITEMS`], so a pipe reaches a chest and a mod's drive
//! through one capability; a mod's bulk store (drawers, a drive, a vault) answers it itself, one
//! entry per kind however many items it holds.
//!
//! **Diverges from upstream: Sunrise addition.** NeoForge 26.3's transfer API has no by-kind
//! capability; `ITEMS` (`Capabilities.Item.BLOCK`) is its counterpart and stays as it is.
//!
//! What a call costs is the handler's. A bulk store keyed by kind answers a question about one
//! kind, and a move of one kind, in constant time whatever it holds. The game's own adapter
//! answers each question in one pass over the slots the side reaches (O(slots), with no work per
//! item moved); a mod's `ITEMS` handler wrapped by kind is asked slot by slot. [`transfer`] is
//! four handler calls whatever it moves. [`ItemStorage::visit_entries`] reads entries in
//! provider order and stops as soon as the visitor breaks.
//!
//! **When not to use it.** A pipe that moves a few items a tick between small containers is
//! cheaper on `ITEMS` and [`move_resources`](crate::resource_util::move_resources), slot by slot: in the
//! measurements behind this API, 256 pipes moving 8 items a tick each cost 2.8 ms a tick that
//! way against 3.7 ms through `STORAGE`. `STORAGE` pays off for bulk stores, large moves, and
//! networks that sleep while idle.
use crate::content::resource::{Ends, move_once};
use crate::{
    BlockSide, Capability, CapabilityKey, ITEMS, ItemResource, ResourceHandler, ServerHost,
    Transaction,
};
use std::{marker::PhantomData, ops::ControlFlow};

/// Why a [`STORAGE`] question or move was refused.
// no wire: reports refusals from in-process storage reads and transfers.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum StorageError {
    /// A total, an amount or an entry count does not fit in 64 bits.
    Overflow,
    /// A handler answered more than it was asked: a mod bug, warned once naming the mod. The
    /// move it was part of is undone.
    Overanswered {
        /// The mod whose handler answered, or "the game".
        owner: String,
    },
    /// The handler could not answer.
    Unavailable {
        /// The provider whose handler was asked, or "the game".
        owner: String,
    },
}

impl From<(crate::Identifier, String)> for StorageError {
    fn from((owner, _): (crate::Identifier, String)) -> Self {
        Self::Unavailable {
            owner: owner.to_string(),
        }
    }
}

impl From<crate::Overanswered> for StorageError {
    fn from(error: crate::Overanswered) -> Self {
        Self::Overanswered {
            owner: error.owner.unwrap_or_else(|| "unknown provider".into()),
        }
    }
}

/// A block's items by kind: how much of one kind it holds and has room for, and its writes, by
/// kind and in transactions, as a [`ResourceHandler`] is written. Stateless and given the host,
/// as every capability handler is. Amounts are 64-bit: a million items is one call.
pub trait ItemStorage {
    /// How much of `resource` it holds.
    fn amount(
        &self,
        host: &mut dyn ServerHost,
        resource: &ItemResource,
    ) -> Result<u64, StorageError>;
    /// A planning bound for `resource`, not an admission decision. The game's default
    /// handler does not ask `BlockEntityPart::can_place_item` here; an insert checks the
    /// actual offer and may take less.
    fn space_for(
        &self,
        host: &mut dyn ServerHost,
        resource: &ItemResource,
    ) -> Result<u64, StorageError>;
    /// Puts up to `amount` of `resource` in, in transaction `tx`: how much went in, never more
    /// than `amount`.
    fn insert(
        &self,
        host: &mut dyn ServerHost,
        tx: &Transaction,
        resource: &ItemResource,
        amount: u64,
    ) -> u64;
    /// Takes up to `amount` of `resource` out, in transaction `tx`: how much came out, never more
    /// than `amount`.
    fn extract(
        &self,
        host: &mut dyn ServerHost,
        tx: &Transaction,
        resource: &ItemResource,
        amount: u64,
    ) -> u64;
    /// Visits occupied entries in provider order, keeping duplicate kinds and their individual
    /// amounts. For a container, this is the order of the slots the queried side reaches.
    /// `ControlFlow::Break(())` ends the visit successfully before another entry is read.
    /// Clone a kind to keep it, and wait for the visit to succeed before making any writes.
    fn visit_entries(
        &self,
        host: &mut dyn ServerHost,
        visitor: &mut dyn FnMut(&ItemResource, u64) -> ControlFlow<()>,
    ) -> Result<(), StorageError>;
}

const STORAGE_SOURCE: &CapabilityKey<dyn ResourceHandler<ItemResource>> = &ITEMS;

/// The default storage handler adapts this capability, including its mod providers.
pub(crate) fn default_source(name: &str) -> Option<&'static str> {
    (name == STORAGE.name).then_some(STORAGE_SOURCE.name)
}

/// Items by kind: a container's or composter port's [`ItemStorage`] as one side reaches it.
/// The game answers it for every container or port [`ITEMS`] reaches, from those slots; a mod's
/// bulk store answers it with a provider of its own (`block_entity_capability(&STORAGE, ..)`).
/// A block with slots and a store of its own registers its provider `before_game`, or the slots
/// answer first.
pub const STORAGE: CapabilityKey<dyn ItemStorage> = CapabilityKey {
    name: "sunrise:storage",
    default: Some(|host, at| {
        let slots = at.level.capability(host, STORAGE_SOURCE, at.pos, at.side)?;
        // The game's own slots are asked in one pass each; a mod's ITEMS, slot by slot.
        Some(match slots.owner {
            None => Box::new(GameStorage(at)) as Box<dyn ItemStorage>,
            Some(_) => Box::new(Slots(slots)),
        })
    }),
    handler: PhantomData,
};

/// The game's own [`STORAGE`] for container or composter slots: each question and write one
/// pass over the slots `.side` reaches, in the game, by the rules hoppers follow.
struct GameStorage(BlockSide);

impl ItemStorage for GameStorage {
    fn amount(
        &self,
        host: &mut dyn ServerHost,
        resource: &ItemResource,
    ) -> Result<u64, StorageError> {
        crate::backend_of(&*host)
            .map_or(Some(0), |b| b.container_kind_amount(self.0, resource))
            .ok_or(StorageError::Overflow)
    }
    fn space_for(
        &self,
        host: &mut dyn ServerHost,
        resource: &ItemResource,
    ) -> Result<u64, StorageError> {
        crate::backend_of(&*host)
            .map_or(Some(0), |b| b.container_kind_space(self.0, resource))
            .ok_or(StorageError::Overflow)
    }
    fn insert(
        &self,
        host: &mut dyn ServerHost,
        tx: &Transaction,
        resource: &ItemResource,
        amount: u64,
    ) -> u64 {
        crate::backend_mut_of(host)
            .map_or(0, |b| b.container_insert_kind(self.0, resource, amount, tx))
            .min(amount)
    }
    fn extract(
        &self,
        host: &mut dyn ServerHost,
        tx: &Transaction,
        resource: &ItemResource,
        amount: u64,
    ) -> u64 {
        crate::backend_mut_of(host)
            .map_or(0, |b| {
                b.container_extract_kind(self.0, resource, amount, tx)
            })
            .min(amount)
    }
    fn visit_entries(
        &self,
        host: &mut dyn ServerHost,
        visitor: &mut dyn FnMut(&ItemResource, u64) -> ControlFlow<()>,
    ) -> Result<(), StorageError> {
        crate::backend_of(&*host)
            .ok_or(crate::LevelReadError::Unavailable)
            .and_then(|backend| backend.container_visit_entries(self.0, visitor))
            .map_err(|_| StorageError::Unavailable {
                owner: "the game".into(),
            })
    }
}

/// [`STORAGE`] over a mod's own [`ITEMS`] handler, asked slot by slot. It inserts as
/// `SimpleContainer.addItem` orders it: the slots that hold the kind first, then the empty ones.
struct Slots(Capability<dyn ResourceHandler<ItemResource>>);

impl Slots {
    /// What slot `index` holds when it holds `resource`.
    fn holds(&self, host: &mut dyn ServerHost, index: usize, resource: &ItemResource) -> bool {
        self.0.resource(host, index).as_ref() == Some(resource)
    }
    /// Up to `left` into the slots `pick` chooses, as one slot takes at most `u32::MAX`.
    fn fill(
        &self,
        host: &mut dyn ServerHost,
        tx: &Transaction,
        resource: &ItemResource,
        left: &mut u64,
        pick: impl Fn(&mut dyn ServerHost, usize) -> bool,
    ) {
        for index in 0..self.0.size(host) {
            if *left == 0 {
                return;
            }
            if pick(host, index) {
                let ask = (*left).min(u32::MAX as u64) as u32;
                *left -= self.0.insert_at(host, tx, index, resource, ask).min(ask) as u64;
            }
        }
    }
}

impl ItemStorage for Slots {
    fn amount(
        &self,
        host: &mut dyn ServerHost,
        resource: &ItemResource,
    ) -> Result<u64, StorageError> {
        let mut held = 0u64;
        for index in 0..self.0.size(host) {
            if self.holds(host, index, resource) {
                held = held
                    .checked_add(self.0.amount(host, index) as u64)
                    .ok_or(StorageError::Overflow)?;
            }
        }
        Ok(held)
    }
    fn space_for(
        &self,
        host: &mut dyn ServerHost,
        resource: &ItemResource,
    ) -> Result<u64, StorageError> {
        let mut room = 0u64;
        for index in 0..self.0.size(host) {
            let here = match self.0.resource(host, index) {
                Some(held) if held == *resource => self
                    .0
                    .capacity(host, index, Some(resource))
                    .saturating_sub(self.0.amount(host, index)),
                Some(_) => 0,
                None if self.0.is_valid(host, index, resource) => {
                    self.0.capacity(host, index, Some(resource))
                }
                None => 0,
            };
            room = room
                .checked_add(here as u64)
                .ok_or(StorageError::Overflow)?;
        }
        Ok(room)
    }
    fn insert(
        &self,
        host: &mut dyn ServerHost,
        tx: &Transaction,
        resource: &ItemResource,
        amount: u64,
    ) -> u64 {
        let mut left = amount;
        self.fill(host, tx, resource, &mut left, |host, index| {
            self.holds(host, index, resource)
        });
        self.fill(host, tx, resource, &mut left, |host, index| {
            self.0.resource(host, index).is_none()
        });
        amount - left
    }
    fn extract(
        &self,
        host: &mut dyn ServerHost,
        tx: &Transaction,
        resource: &ItemResource,
        amount: u64,
    ) -> u64 {
        let mut left = amount;
        for index in 0..self.0.size(host) {
            if left == 0 {
                break;
            }
            if self.holds(host, index, resource) {
                let ask = left.min(u32::MAX as u64) as u32;
                left -= self.0.extract_at(host, tx, index, resource, ask).min(ask) as u64;
            }
        }
        amount - left
    }
    fn visit_entries(
        &self,
        host: &mut dyn ServerHost,
        visitor: &mut dyn FnMut(&ItemResource, u64) -> ControlFlow<()>,
    ) -> Result<(), StorageError> {
        for index in 0..self.0.size(host) {
            if let Some(resource) = self.0.resource(host, index) {
                let amount = u64::from(self.0.amount(host, index));
                if visitor(&resource, amount).is_break() {
                    break;
                }
            }
        }
        Ok(())
    }
}

/// Moves up to `max` of `resource` out of `from` into `to`, in transaction `tx`: how much moved.
/// Extract from `from` first, then offer the extracted amount to `to` in the same journal. If
/// `to` takes only part, roll back and retry once with that amount. A handler that answers more
/// than it was asked rolls back the move with [`StorageError::Overanswered`]. If `from` has none
/// or `to` has no room, neither is asked to transfer anything.
pub fn transfer(
    host: &mut dyn ServerHost,
    from: &Capability<dyn ItemStorage>,
    to: &Capability<dyn ItemStorage>,
    resource: &ItemResource,
    max: u64,
    tx: &Transaction,
) -> Result<u64, StorageError> {
    let _ = tx;
    let want = max
        .min(from.amount(host, resource)?)
        .min(to.space_for(host, resource)?);
    if want == 0 {
        return Ok(0);
    }
    Ok(move_once(
        host,
        Ends {
            from,
            to,
            at: resource,
        },
        want,
        |host, tx, want| from.extract(host, tx, resource, want),
        |host, tx, out| Ok(to.insert(host, tx, resource, out)),
    )?)
}

/// Moves up to `max` of what `filter` accepts out of `from` into `to` (NeoForge's
/// `ResourceHandlerUtil.move`, by kind), in transaction `tx`: how much moved. A zero `max` does
/// not read the source. Each visit stops at the next matching entry, then [`transfer`] runs
/// after the visit succeeds. A zero move advances past that occurrence; a positive move starts
/// again at the first entry because the transfer may have shifted the source's entries.
pub fn transfer_matching(
    host: &mut dyn ServerHost,
    from: &Capability<dyn ItemStorage>,
    to: &Capability<dyn ItemStorage>,
    filter: impl Fn(&ItemResource) -> bool,
    max: u64,
    tx: &Transaction,
) -> Result<u64, StorageError> {
    let mut moved = 0;
    let mut tried_prefix = 0;
    while moved < max {
        let mut seen = 0u64;
        let mut selected = Ok(None);
        from.visit_entries(host, &mut |resource, _| {
            let Some(occurrence) = seen.checked_add(1) else {
                selected = Err(StorageError::Overflow);
                return ControlFlow::Break(());
            };
            seen = occurrence;
            if seen > tried_prefix && filter(resource) {
                selected = Ok(Some(resource.clone()));
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        })?;
        let Some(resource) = selected? else {
            break;
        };
        let went = transfer(host, from, to, &resource, max - moved, tx)?;
        moved += went;
        tried_prefix = if went == 0 { seen } else { 0 };
    }
    Ok(moved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::ItemsHost;
    use crate::testing::FakeHost;
    use crate::{HostTransaction, Identifier, ItemStack, Tag};
    use std::cell::{Cell, RefCell};
    use std::collections::HashMap;
    use std::hash::BuildHasher;

    fn host_with(items: &[&str]) -> (FakeHost, Vec<ItemResource>) {
        let mut host = FakeHost::new();
        let ids: Vec<_> = items
            .iter()
            .map(|id| Identifier::parse(id).unwrap())
            .collect();
        host.registries.resolve_items(ids.clone()).unwrap();
        for id in items {
            host.registries
                .item_component(id, "minecraft:max_stack_size", Tag::Int(64));
        }
        let resources = ids
            .iter()
            .map(|id| ItemResource::of(&ItemStack::new(host.item_key(id).unwrap(), 1).unwrap()))
            .collect();
        (host, resources)
    }

    fn chest(host: &mut FakeHost, pos: [i32; 3], slots: &[Option<(&ItemResource, u32)>]) {
        let entity = host.level.block_entity(pos).with_items(slots.len());
        for (slot, held) in slots.iter().enumerate() {
            entity.items.as_mut().unwrap().slots[slot] = held.map(|(r, n)| r.to_stack(n).unwrap());
        }
    }

    fn storage_at(host: &mut FakeHost, pos: [i32; 3]) -> Capability<dyn ItemStorage> {
        let level = host.level.level();
        let server: &mut dyn ServerHost = host;
        level.capability(server, &STORAGE, pos, None).unwrap()
    }

    /// A bulk store in memory, counting the calls made to it: no transactions, for the cases
    /// that never abort.
    #[derive(Default)]
    struct Bulk {
        held: RefCell<HashMap<ItemResource, u64>>,
        room: u64,
        calls: Cell<u32>,
    }
    struct Shared(std::rc::Rc<Bulk>);
    impl ItemStorage for Shared {
        fn amount(&self, _: &mut dyn ServerHost, r: &ItemResource) -> Result<u64, StorageError> {
            self.0.calls.set(self.0.calls.get() + 1);
            Ok(self.0.held.borrow().get(r).copied().unwrap_or(0))
        }
        fn space_for(&self, _: &mut dyn ServerHost, _: &ItemResource) -> Result<u64, StorageError> {
            self.0.calls.set(self.0.calls.get() + 1);
            Ok(self.0.room - self.0.held.borrow().values().sum::<u64>())
        }
        fn insert(&self, h: &mut dyn ServerHost, _: &Transaction, r: &ItemResource, n: u64) -> u64 {
            let went = n.min(self.space_for(h, r).unwrap());
            *self.0.held.borrow_mut().entry(r.clone()).or_default() += went;
            went
        }
        fn extract(
            &self,
            h: &mut dyn ServerHost,
            _: &Transaction,
            r: &ItemResource,
            n: u64,
        ) -> u64 {
            let came = n.min(self.amount(h, r).unwrap());
            *self.0.held.borrow_mut().entry(r.clone()).or_default() -= came;
            came
        }
        fn visit_entries(
            &self,
            _: &mut dyn ServerHost,
            visitor: &mut dyn FnMut(&ItemResource, u64) -> ControlFlow<()>,
        ) -> Result<(), StorageError> {
            for (resource, &amount) in self.0.held.borrow().iter() {
                if amount != 0 && visitor(resource, amount).is_break() {
                    break;
                }
            }
            Ok(())
        }
    }
    fn bulk(
        room: u64,
        held: &[(&ItemResource, u64)],
    ) -> (std::rc::Rc<Bulk>, Capability<dyn ItemStorage>) {
        let store = std::rc::Rc::new(Bulk {
            held: RefCell::new(held.iter().map(|(r, n)| ((*r).clone(), *n)).collect()),
            room,
            ..Default::default()
        });
        let handler = Capability {
            handler: Box::new(Shared(store.clone())) as Box<dyn ItemStorage>,
            owner: None,
        };
        (store, handler)
    }

    #[test]
    fn the_game_answers_storage_for_slots_by_kind() {
        let (mut host, r) = host_with(&["minecraft:stone", "minecraft:dirt"]);
        let (stone, dirt) = (&r[0], &r[1]);
        chest(
            &mut host,
            [0, 64, 0],
            &[Some((stone, 40)), Some((dirt, 5)), Some((stone, 10)), None],
        );
        let storage = storage_at(&mut host, [0, 64, 0]);
        let host: &mut dyn ServerHost = &mut host;
        assert_eq!(storage.amount(host, stone), Ok(50));
        assert_eq!(storage.amount(host, dirt), Ok(5));
        // 24 and 54 topping up the stone, 64 in the empty slot.
        assert_eq!(storage.space_for(host, stone), Ok(24 + 54 + 64));
        let went = host.transaction(|host, tx| Ok::<_, ()>(storage.insert(host, tx, stone, 500)));
        assert_eq!(went, Ok(142), "no more than the room");
        assert_eq!(storage.amount(host, stone), Ok(192));
        assert_eq!(
            storage.amount(host, dirt),
            Ok(5),
            "the dirt's slot is not touched"
        );
        let came = host.transaction(|host, tx| Ok::<_, ()>(storage.extract(host, tx, stone, 100)));
        assert_eq!(came, Ok(100));
        let mut total = 0;
        storage
            .visit_entries(host, &mut |_, amount| {
                total += amount;
                ControlFlow::Continue(())
            })
            .unwrap();
        assert_eq!(total, 97);
    }

    #[test]
    fn a_slot_write_aborted_with_its_transaction_is_put_back() {
        let (mut host, r) = host_with(&["minecraft:stone"]);
        chest(&mut host, [0, 64, 0], &[Some((&r[0], 10)), None]);
        let storage = storage_at(&mut host, [0, 64, 0]);
        let host: &mut dyn ServerHost = &mut host;
        let aborted: Result<(), ()> = host.transaction(|host, tx| {
            assert_eq!(storage.insert(host, tx, &r[0], 70), 70);
            Err(())
        });
        assert!(aborted.is_err());
        assert_eq!(storage.amount(host, &r[0]), Ok(10));
    }

    #[test]
    fn a_visit_keeps_all_entries_in_order_with_duplicate_kinds() {
        let (mut host, r) = host_with(&["minecraft:stone", "minecraft:dirt"]);
        let slots: Vec<_> = (1..=300)
            .flat_map(|amount| [None, Some((&r[(amount % 2) as usize], amount))])
            .collect();
        chest(&mut host, [0, 64, 0], &slots);
        let storage = storage_at(&mut host, [0, 64, 0]);
        let mut entries = Vec::new();
        storage
            .visit_entries(&mut host, &mut |resource, amount| {
                entries.push((resource.clone(), amount));
                ControlFlow::Continue(())
            })
            .unwrap();
        assert_eq!(
            entries,
            (1..=300u64)
                .map(|amount| (r[(amount % 2) as usize].clone(), amount))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn a_transfer_moves_billions_in_four_calls() {
        let (mut host, r) = host_with(&["minecraft:cobblestone"]);
        let (from_store, from) = bulk(u64::MAX, &[(&r[0], 5_000_000_000)]);
        let (into_store, into) = bulk(u64::MAX, &[]);
        let host: &mut dyn ServerHost = &mut host;
        let moved = host.transaction(|host, tx| transfer(host, &from, &into, &r[0], u64::MAX, tx));
        assert_eq!(moved, Ok(5_000_000_000));
        assert_eq!(into_store.held.borrow()[&r[0]], 5_000_000_000);
        // amount and space_for to plan, then the extract (with its amount) and the insert
        // (with its space_for).
        assert_eq!(from_store.calls.get() + into_store.calls.get(), 4);
    }

    #[test]
    fn a_transfer_into_slots_takes_what_fits_and_leaves_the_rest() {
        let (mut host, r) = host_with(&["minecraft:stone"]);
        chest(&mut host, [0, 64, 0], &[None, None, None]);
        let into = storage_at(&mut host, [0, 64, 0]);
        let (from_store, from) = bulk(u64::MAX, &[(&r[0], 1000)]);
        let host: &mut dyn ServerHost = &mut host;
        let moved = host.transaction(|host, tx| transfer(host, &from, &into, &r[0], 1000, tx));
        assert_eq!(moved, Ok(192));
        assert_eq!(from_store.held.borrow()[&r[0]], 808);
        let moved = host.transaction(|host, tx| transfer(host, &from, &into, &r[0], 1000, tx));
        assert_eq!(moved, Ok(0), "a full target is not tried");
    }

    #[test]
    fn transfer_matching_moves_only_what_the_filter_takes() {
        let (mut host, r) = host_with(&["minecraft:stone", "minecraft:dirt"]);
        chest(
            &mut host,
            [0, 64, 0],
            &[Some((&r[0], 30)), Some((&r[1], 20)), Some((&r[1], 7))],
        );
        let from = storage_at(&mut host, [0, 64, 0]);
        let (into_store, into) = bulk(u64::MAX, &[]);
        let host: &mut dyn ServerHost = &mut host;
        let dirt = r[1].clone();
        let moved = host.transaction(|host, tx| {
            transfer_matching(host, &from, &into, |res| *res == dirt, u64::MAX, tx)
        });
        assert_eq!(moved, Ok(27));
        assert_eq!(from.amount(host, &r[0]), Ok(30));
        assert_eq!(into_store.held.borrow().get(&r[0]), None);
    }

    #[test]
    fn transfers_aborted_and_committed_conserve_every_item() {
        let (mut host, r) = host_with(&["minecraft:stone", "minecraft:dirt"]);
        chest(
            &mut host,
            [0, 64, 0],
            &[Some((&r[0], 64)), Some((&r[1], 40)), Some((&r[0], 9))],
        );
        chest(&mut host, [1, 64, 0], &[None, Some((&r[1], 3)), None]);
        let a = storage_at(&mut host, [0, 64, 0]);
        let b = storage_at(&mut host, [1, 64, 0]);
        let host: &mut dyn ServerHost = &mut host;
        let total = |host: &mut dyn ServerHost| {
            r.iter()
                .map(|kind| a.amount(host, kind).unwrap() + b.amount(host, kind).unwrap())
                .sum::<u64>()
        };
        let before = total(host);
        for round in 0..40u64 {
            let (from, to) = if round.is_multiple_of(2) {
                (&a, &b)
            } else {
                (&b, &a)
            };
            let kind = &r[(round % 3 % 2) as usize];
            let result: Result<u64, ()> = host.transaction(|host, tx| {
                let moved =
                    transfer(host, from, to, kind, round + 1, tx).expect("test transfer succeeds");
                // Every third round aborts after it wrote.
                if round.is_multiple_of(3) {
                    return Err(());
                }
                Ok(moved)
            });
            assert_eq!(result.is_err(), round.is_multiple_of(3));
            assert_eq!(total(host), before, "round {round}");
        }
    }

    #[test]
    fn a_resource_keys_a_map_by_item_damage_and_components() {
        let (host, r) = host_with(&["minecraft:stone", "minecraft:dirt"]);
        let stone = host
            .item_key(&Identifier::parse("minecraft:stone").unwrap())
            .unwrap();
        let state = std::hash::RandomState::new();
        let hash_with = |resource: &ItemResource| state.hash_one(resource);
        // Equal stacks of different counts are one kind, and hash alike.
        let many = ItemResource::of(&ItemStack::new(stone, 64).unwrap());
        assert_eq!(many, r[0]);
        assert_eq!(hash_with(&many), hash_with(&r[0]));
        // Damage and a component patch each make another kind.
        let mut worn = ItemStack::new(stone, 9).unwrap();
        worn.set_damage(3);
        let mut named = ItemStack::new(stone, 1).unwrap();
        named
            .set_components(
                [(
                    "minecraft:custom_data".into(),
                    Tag::Compound([("bench".into(), Tag::Int(1))].into()),
                )]
                .into(),
            )
            .unwrap();
        let named = ItemResource::of(&named);
        assert_ne!(named, r[0]);
        let mut counts: HashMap<ItemResource, u64> = HashMap::new();
        for resource in [
            &r[0],
            &r[1],
            &ItemResource::of(&worn),
            &named,
            &many,
            &named,
        ] {
            *counts.entry(resource.clone()).or_default() += 1;
        }
        assert_eq!(counts.len(), 4);
        assert_eq!(counts[&r[0]], 2, "stone of any count is one key");
        assert_eq!(counts[&named], 2, "the patch keys its own entry");
    }
}
