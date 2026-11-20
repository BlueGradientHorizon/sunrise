//! NeoForge 26.3's stock resource handlers and `ResourceHandlerUtil`: wrappers a mod composes its
//! own [`ResourceHandler`]s with, and the queries and moves every transfer mod writes. Each write
//! runs in the caller's transaction `tx`; to simulate one, abort it.
use crate::content::resource::{Ends, MAX_TRANSFER_STEP, in_turn, move_once};
use crate::{
    Capability, ComponentId, FluidKey, FluidResource, HostTransaction, ItemKey, ItemResource,
    Overanswered, ResourceHandler, ServerHost, Tag, Transaction,
};
use std::ops::Range;
use sunrise_core::nbt::JavaString;

/// Several handlers as one (`CombinedResourceHandler`): their indices one after another, the
/// first handler's from 0. An insert or extract without an index asks each handler in turn, so
/// each places it its own way.
// no wire: a mod-side handler, invoked where it was built; never crosses the boundary.
// exhaustive: its fields are private, so mods cannot build or destructure it
pub struct CombinedResourceHandler<R> {
    handlers: Vec<(Box<dyn ResourceHandler<R>>, usize)>,
    size: usize,
}

impl<R> CombinedResourceHandler<R> {
    /// `handlers`, numbered in this order. Each handler's size is sampled once, so indices
    /// stay assigned to the same handler for the lifetime of the wrapper.
    pub fn new(host: &mut dyn ServerHost, handlers: Vec<Box<dyn ResourceHandler<R>>>) -> Self {
        let mut size = 0usize;
        let handlers = handlers
            .into_iter()
            .map(|handler| {
                let slots = handler.size(host);
                size = size.saturating_add(slots);
                (handler, slots)
            })
            .collect();
        Self { handlers, size }
    }

    /// The handler `index` falls in, and the index within it.
    fn locate(&self, mut index: usize) -> Option<(&dyn ResourceHandler<R>, usize)> {
        if index >= self.size {
            return None;
        }
        for (handler, size) in &self.handlers {
            if index < *size {
                return Some((handler.as_ref(), index));
            }
            index -= size;
        }
        None
    }
}

impl<R> ResourceHandler<R> for CombinedResourceHandler<R> {
    /// The handlers' original sizes added up; `usize::MAX` at most.
    fn size(&self, _: &mut dyn ServerHost) -> usize {
        self.size
    }
    fn resource(&self, host: &mut dyn ServerHost, index: usize) -> Option<R> {
        let (handler, at) = self.locate(index)?;
        handler.resource(host, at)
    }
    fn amount(&self, host: &mut dyn ServerHost, index: usize) -> u32 {
        self.locate(index)
            .map_or(0, |(handler, at)| handler.amount(host, at))
    }
    fn capacity(&self, host: &mut dyn ServerHost, index: usize, resource: &R) -> u32 {
        self.locate(index)
            .map_or(0, |(handler, at)| handler.capacity(host, at, resource))
    }
    fn is_valid(&self, host: &mut dyn ServerHost, index: usize, resource: &R) -> bool {
        self.locate(index)
            .is_some_and(|(handler, at)| handler.is_valid(host, at, resource))
    }
    fn insert_at(
        &self,
        host: &mut dyn ServerHost,
        tx: &Transaction,
        index: usize,
        resource: &R,
        amount: u32,
    ) -> u32 {
        self.locate(index).map_or(0, |(handler, at)| {
            handler.insert_at(host, tx, at, resource, amount)
        })
    }
    fn extract_at(
        &self,
        host: &mut dyn ServerHost,
        tx: &Transaction,
        index: usize,
        resource: &R,
        amount: u32,
    ) -> u32 {
        self.locate(index).map_or(0, |(handler, at)| {
            handler.extract_at(host, tx, at, resource, amount)
        })
    }
    fn insert(
        &self,
        host: &mut dyn ServerHost,
        tx: &Transaction,
        resource: &R,
        amount: u32,
    ) -> u32 {
        in_turn(
            host,
            tx,
            amount,
            &self.handlers,
            |host, tx, (handler, _), left| handler.insert(host, tx, resource, left),
        )
    }
    fn extract(
        &self,
        host: &mut dyn ServerHost,
        tx: &Transaction,
        resource: &R,
        amount: u32,
    ) -> u32 {
        in_turn(
            host,
            tx,
            amount,
            &self.handlers,
            |host, tx, (handler, _), left| handler.extract(host, tx, resource, left),
        )
    }
}

/// A checked range of another handler's indices, numbered from 0. A machine can expose only
/// its input slots on one side this way. To delegate without changing the visible range, implement
/// [`ResourceHandler`] on a struct holding the handler and forward to it:
///
/// ```
/// use mod_api::{ResourceHandler, ServerHost, Transaction};
///
/// struct Delegating<R>(Box<dyn ResourceHandler<R>>);
///
/// impl<R> ResourceHandler<R> for Delegating<R> {
///     fn size(&self, host: &mut dyn ServerHost) -> usize {
///         self.0.size(host)
///     }
///     fn resource(&self, host: &mut dyn ServerHost, index: usize) -> Option<R> {
///         self.0.resource(host, index)
///     }
///     fn amount(&self, host: &mut dyn ServerHost, index: usize) -> u32 {
///         self.0.amount(host, index)
///     }
///     fn capacity(&self, host: &mut dyn ServerHost, index: usize, resource: &R) -> u32 {
///         self.0.capacity(host, index, resource)
///     }
///     fn is_valid(&self, host: &mut dyn ServerHost, index: usize, resource: &R) -> bool {
///         self.0.is_valid(host, index, resource)
///     }
///     fn insert_at(
///         &self,
///         host: &mut dyn ServerHost,
///         tx: &Transaction,
///         index: usize,
///         resource: &R,
///         amount: u32,
///     ) -> u32 {
///         self.0.insert_at(host, tx, index, resource, amount)
///     }
///     fn extract_at(
///         &self,
///         host: &mut dyn ServerHost,
///         tx: &Transaction,
///         index: usize,
///         resource: &R,
///         amount: u32,
///     ) -> u32 {
///         self.0.extract_at(host, tx, index, resource, amount)
///     }
///     fn insert(&self, host: &mut dyn ServerHost, tx: &Transaction, r: &R, n: u32) -> u32 {
///         self.0.insert(host, tx, r, n)
///     }
///     fn extract(&self, host: &mut dyn ServerHost, tx: &Transaction, r: &R, n: u32) -> u32 {
///         self.0.extract(host, tx, r, n)
///     }
/// }
/// ```
// no wire: a mod-side handler, invoked where it was built; never crosses the boundary.
// exhaustive: its fields are private, so mods cannot build or destructure it
pub struct RangedResourceHandler<R> {
    handler: Box<dyn ResourceHandler<R>>,
    range: Range<usize>,
}

impl<R> RangedResourceHandler<R> {
    /// Wraps a non-empty range, or returns `None` if it is reversed, empty or outside `handler`.
    pub fn of(
        host: &mut dyn ServerHost,
        handler: Box<dyn ResourceHandler<R>>,
        range: Range<usize>,
    ) -> Option<Self> {
        (range.start < range.end && range.end <= handler.size(host))
            .then_some(Self { handler, range })
    }

    /// Wraps one index, or returns `None` if it is outside `handler`.
    pub fn of_single_index(
        host: &mut dyn ServerHost,
        handler: Box<dyn ResourceHandler<R>>,
        index: usize,
    ) -> Option<Self> {
        Self::of(host, handler, index..index.checked_add(1)?)
    }

    /// `index`'s index in the wrapped handler, or none past this range.
    fn inner(&self, index: usize) -> Option<usize> {
        (index < self.range.len()).then(|| self.range.start + index)
    }
}

impl<R> ResourceHandler<R> for RangedResourceHandler<R> {
    fn size(&self, _: &mut dyn ServerHost) -> usize {
        self.range.len()
    }
    fn resource(&self, host: &mut dyn ServerHost, index: usize) -> Option<R> {
        let at = self.inner(index)?;
        self.handler.resource(host, at)
    }
    fn amount(&self, host: &mut dyn ServerHost, index: usize) -> u32 {
        self.inner(index)
            .map_or(0, |at| self.handler.amount(host, at))
    }
    fn capacity(&self, host: &mut dyn ServerHost, index: usize, resource: &R) -> u32 {
        self.inner(index)
            .map_or(0, |at| self.handler.capacity(host, at, resource))
    }
    fn is_valid(&self, host: &mut dyn ServerHost, index: usize, resource: &R) -> bool {
        self.inner(index)
            .is_some_and(|at| self.handler.is_valid(host, at, resource))
    }
    fn insert_at(
        &self,
        host: &mut dyn ServerHost,
        tx: &Transaction,
        index: usize,
        resource: &R,
        amount: u32,
    ) -> u32 {
        self.inner(index).map_or(0, |at| {
            self.handler.insert_at(host, tx, at, resource, amount)
        })
    }
    fn extract_at(
        &self,
        host: &mut dyn ServerHost,
        tx: &Transaction,
        index: usize,
        resource: &R,
        amount: u32,
    ) -> u32 {
        self.inner(index).map_or(0, |at| {
            self.handler.extract_at(host, tx, at, resource, amount)
        })
    }
}

/// No indices, refusing everything (`EmptyResourceHandler`): what a provider answers for a side
/// that reaches nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
// no wire: a mod-side handler, invoked where it was built; never crosses the boundary.
// exhaustive: a unit marker
pub struct EmptyResourceHandler;

impl<R> ResourceHandler<R> for EmptyResourceHandler {
    fn size(&self, _: &mut dyn ServerHost) -> usize {
        0
    }
    fn resource(&self, _: &mut dyn ServerHost, _: usize) -> Option<R> {
        None
    }
    fn amount(&self, _: &mut dyn ServerHost, _: usize) -> u32 {
        0
    }
    fn capacity(&self, _: &mut dyn ServerHost, _: usize, _: &R) -> u32 {
        0
    }
    fn insert_at(&self, _: &mut dyn ServerHost, _: &Transaction, _: usize, _: &R, _: u32) -> u32 {
        0
    }
    fn extract_at(&self, _: &mut dyn ServerHost, _: &Transaction, _: usize, _: &R, _: u32) -> u32 {
        0
    }
}

/// One index of an endless amount of one resource (`InfiniteResourceHandler`): every insert and
/// extract of it is taken whole, and anything else is refused. A creative tank.
#[derive(Clone, Debug, PartialEq)]
// no wire: a mod-side handler, invoked where it was built; never crosses the boundary.
// exhaustive: its fields are private, so mods cannot build or destructure it
pub struct InfiniteResourceHandler<R>(R);

impl<R> InfiniteResourceHandler<R> {
    /// An endless amount of `resource`.
    pub fn new(resource: R) -> Self {
        Self(resource)
    }
}

impl<R: Clone + PartialEq> ResourceHandler<R> for InfiniteResourceHandler<R> {
    fn size(&self, _: &mut dyn ServerHost) -> usize {
        1
    }
    fn resource(&self, _: &mut dyn ServerHost, index: usize) -> Option<R> {
        (index == 0).then(|| self.0.clone())
    }
    fn amount(&self, _: &mut dyn ServerHost, index: usize) -> u32 {
        if index == 0 { u32::MAX } else { 0 }
    }
    fn capacity(&self, _: &mut dyn ServerHost, index: usize, resource: &R) -> u32 {
        if index == 0 && *resource == self.0 {
            u32::MAX
        } else {
            0
        }
    }
    fn insert_at(
        &self,
        host: &mut dyn ServerHost,
        _: &Transaction,
        index: usize,
        resource: &R,
        amount: u32,
    ) -> u32 {
        if self.is_valid(host, index, resource) {
            amount
        } else {
            0
        }
    }
    fn extract_at(
        &self,
        host: &mut dyn ServerHost,
        _: &Transaction,
        index: usize,
        resource: &R,
        amount: u32,
    ) -> u32 {
        if self.is_valid(host, index, resource) {
            amount
        } else {
            0
        }
    }
}

/// One index that destroys whatever goes in and holds nothing (`VoidingResourceHandler`): a
/// trash can.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
// no wire: a mod-side handler, invoked where it was built; never crosses the boundary.
// exhaustive: a unit marker
pub struct VoidingResourceHandler;

impl<R> ResourceHandler<R> for VoidingResourceHandler {
    fn size(&self, _: &mut dyn ServerHost) -> usize {
        1
    }
    fn resource(&self, _: &mut dyn ServerHost, _: usize) -> Option<R> {
        None
    }
    fn amount(&self, _: &mut dyn ServerHost, _: usize) -> u32 {
        0
    }
    fn capacity(&self, _: &mut dyn ServerHost, index: usize, _: &R) -> u32 {
        if index == 0 { u32::MAX } else { 0 }
    }
    fn insert_at(
        &self,
        _: &mut dyn ServerHost,
        _: &Transaction,
        index: usize,
        _: &R,
        amount: u32,
    ) -> u32 {
        if index == 0 { amount } else { 0 }
    }
    fn extract_at(&self, _: &mut dyn ServerHost, _: &Transaction, _: usize, _: &R, _: u32) -> u32 {
        0
    }
}

/// A resource with a registry identity and data components, as NeoForge's `RegisteredResource`
/// and `DataComponentHolderResource`: what code written once for items and fluids reads.
pub trait KeyedResource {
    /// Its registry's key type.
    type Key;
    /// Its registered key; none for one the registry no longer has (a fluid saved by a removed
    /// mod).
    fn key(&self) -> Option<Self::Key>;
    /// Its own value of component `id`, as saved; none where it keeps the default.
    fn component(&self, id: &ComponentId) -> Option<Tag>;
}

impl KeyedResource for ItemResource {
    type Key = ItemKey;
    fn key(&self) -> Option<ItemKey> {
        Some(self.item())
    }
    fn component(&self, id: &ComponentId) -> Option<Tag> {
        ItemResource::component(self, id)
    }
}

impl KeyedResource for FluidResource {
    type Key = FluidKey;
    fn key(&self) -> Option<FluidKey> {
        self.fluid().key()
    }
    fn component(&self, id: &ComponentId) -> Option<Tag> {
        self.0
            .components
            .as_deref()?
            .as_nbt()
            .get(&JavaString::from(id.as_str()))
            .cloned()
    }
}

/// `ResourceHandlerUtil`'s queries and writes, on any handler (a [`Capability`] reaches them
/// through its handler): `cap.is_full(host)`. A mod's own handler value calls them as
/// `(&tank as &dyn ResourceHandler<_>).is_full(host)`.
impl<R> dyn ResourceHandler<R> {
    /// The first index holding `resource` (`indexOf`).
    pub fn index_of(&self, host: &mut dyn ServerHost, resource: &R) -> Option<usize>
    where
        R: PartialEq,
    {
        (0..self.size(host)).find(|&index| {
            self.amount(host, index) > 0 && self.resource(host, index).as_ref() == Some(resource)
        })
    }

    /// Whether an index holds `resource` (`contains`).
    pub fn contains(&self, host: &mut dyn ServerHost, resource: &R) -> bool
    where
        R: PartialEq,
    {
        self.index_of(host, resource).is_some()
    }

    /// Whether every index holds nothing (`isEmpty`); a handler of no indices is.
    pub fn is_empty(&self, host: &mut dyn ServerHost) -> bool {
        (0..self.size(host)).all(|index| self.amount(host, index) == 0)
    }

    /// Whether every index holds a resource up to its capacity for it (`isFull`).
    pub fn is_full(&self, host: &mut dyn ServerHost) -> bool {
        (0..self.size(host)).all(|index| {
            self.resource(host, index).is_some_and(|resource| {
                let amount = self.amount(host, index);
                amount > 0 && amount >= self.capacity(host, index, &resource)
            })
        })
    }

    /// Whether `resource` may go in some index (`ResourceHandlerUtil.isValid`).
    pub fn accepts(&self, host: &mut dyn ServerHost, resource: &R) -> bool {
        (0..self.size(host)).any(|index| self.is_valid(host, index, resource))
    }

    /// A comparator's signal reading this handler (`getRedstoneSignalFromResourceHandler`),
    /// vanilla's container formula: each index's fill, amount over capacity, averaged over the
    /// indices and mapped onto 0..=15, 0 only when all are empty and 15 when all are full.
    pub fn comparator_signal(&self, host: &mut dyn ServerHost) -> u8 {
        let size = self.size(host);
        if size == 0 {
            return 0;
        }
        let mut fill = 0f32;
        for index in 0..size {
            let Some(resource) = self.resource(host, index) else {
                continue;
            };
            let amount = self.amount(host, index);
            let capacity = self.capacity(host, index, &resource);
            if amount > 0 && capacity > 0 {
                fill += amount as f32 / capacity as f32;
            }
        }
        // Capped at full before scaling: an oversized amount (a mod's, up to `u32::MAX` over a
        // capacity of 1) would otherwise overflow it.
        let fill = (fill / size as f32).min(1.0);
        // `Mth.lerpDiscrete(fill, 0, 15)`.
        (fill * 14.0).floor() as u8 + u8::from(fill > 0.0)
    }

    /// Puts up to `amount` of `resource` in, topping up indices that hold it before empty ones
    /// (`insertStacking`), in `tx`: how much went in.
    pub fn insert_stacking(
        &self,
        host: &mut dyn ServerHost,
        tx: &Transaction,
        resource: &R,
        amount: u32,
    ) -> u32
    where
        R: PartialEq,
    {
        let size = self.size(host);
        let indices = [true, false]
            .into_iter()
            .flat_map(|holding| (0..size).map(move |index| (holding, index)));
        in_turn(
            host,
            tx,
            amount,
            indices,
            |host, tx, &(holding, index), left| {
                let held = self.amount(host, index) > 0
                    && self.resource(host, index).as_ref() == Some(resource);
                if held == holding {
                    self.insert_at(host, tx, index, resource, left)
                } else {
                    0
                }
            },
        )
    }

    /// Takes up to `amount` of the first resource `filter` accepts out, from wherever this
    /// handler holds it (`extractFirst`), in `tx`: what came out and how much, none when nothing
    /// did.
    pub fn extract_first(
        &self,
        host: &mut dyn ServerHost,
        tx: &Transaction,
        filter: impl Fn(&R) -> bool,
        amount: u32,
    ) -> Option<(R, u32)> {
        for index in 0..self.size(host) {
            if self.amount(host, index) == 0 {
                continue;
            }
            let Some(resource) = self.resource(host, index).filter(|r| filter(r)) else {
                continue;
            };
            let came = self.extract(host, tx, &resource, amount);
            if came > 0 {
                return Some((resource, came));
            }
        }
        None
    }

    /// The first resource `filter` accepts that one of can be taken out
    /// (`findExtractableResource`), tried in a transaction nested in `tx` and aborted, so
    /// nothing changes.
    pub fn find_extractable(
        &self,
        host: &mut dyn ServerHost,
        tx: &Transaction,
        filter: impl Fn(&R) -> bool,
    ) -> Option<R> {
        let _ = tx;
        for index in 0..self.size(host) {
            if self.amount(host, index) == 0 {
                continue;
            }
            let Some(resource) = self.resource(host, index).filter(|r| filter(r)) else {
                continue;
            };
            let probe: Result<(), u32> =
                host.transaction(|host, tx| Err(self.extract_at(host, tx, index, &resource, 1)));
            if probe.is_err_and(|came| came > 0) {
                return Some(resource);
            }
        }
        None
    }
}

/// As [`move_resources`](crate::move_resources), but moving only the first resource `filter`
/// accepts that `to` takes any of (`moveFirst`), in `tx`: what moved and how much, none when
/// nothing did.
pub fn move_first<R>(
    host: &mut dyn ServerHost,
    from: &Capability<dyn ResourceHandler<R>>,
    to: &Capability<dyn ResourceHandler<R>>,
    filter: impl Fn(&R) -> bool,
    max: u32,
    tx: &Transaction,
) -> Result<Option<(R, u32)>, Overanswered> {
    move_first_by(
        host,
        from,
        to,
        filter,
        max,
        tx,
        |host, tx, resource, out| to.insert(host, tx, resource, out),
    )
}

/// As [`move_first`], inserting with `insert_stacking` (`moveFirstStacking`): into `to`'s
/// indices that hold the resource before its empty ones.
pub fn move_first_stacking<R: PartialEq>(
    host: &mut dyn ServerHost,
    from: &Capability<dyn ResourceHandler<R>>,
    to: &Capability<dyn ResourceHandler<R>>,
    filter: impl Fn(&R) -> bool,
    max: u32,
    tx: &Transaction,
) -> Result<Option<(R, u32)>, Overanswered> {
    move_first_by(
        host,
        from,
        to,
        filter,
        max,
        tx,
        |host, tx, resource, out| to.insert_stacking(host, tx, resource, out),
    )
}

/// The first index of `from` whose resource `filter` accepts and `insert` puts any of in `to`,
/// moved as one step of `move_resources`.
fn move_first_by<R>(
    host: &mut dyn ServerHost,
    from: &Capability<dyn ResourceHandler<R>>,
    to: &Capability<dyn ResourceHandler<R>>,
    filter: impl Fn(&R) -> bool,
    max: u32,
    tx: &Transaction,
    insert: impl Fn(&mut dyn ServerHost, &Transaction, &R, u32) -> u32,
) -> Result<Option<(R, u32)>, Overanswered> {
    let _ = tx;
    for index in 0..from.size(host) {
        let Some(resource) = from.resource(host, index).filter(|r| filter(r)) else {
            continue;
        };
        let want = max.min(from.amount(host, index)).min(MAX_TRANSFER_STEP);
        if want == 0 {
            continue;
        }
        let went = move_once(
            host,
            Ends {
                from,
                to,
                at: &index,
            },
            want,
            |host, tx, want| from.extract_at(host, tx, index, &resource, want),
            |host, tx, out| insert(host, tx, &resource, out),
        )?;
        if went > 0 {
            return Ok(Some((resource, went)));
        }
    }
    Ok(None)
}

#[cfg(test)]
#[path = "resource_util_tests.rs"]
mod tests;
