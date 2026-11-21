//! Scoped editing of engine-owned, constructed children before residency.

use crate::{
    AttributeModifier, EquipmentSlot, Identifier, ItemStack, Text, UnspawnedMobError,
    UnspawnedMobRef, UnspawnedMobView, WorldEpoch,
};

/// A listener-scoped lease of an ordered child collection, acquired with
/// [`PendingChildrenEdits::children`]. Retaining this value does not extend its lifetime: the
/// next listener, a nested dispatch or the callback's return expires it. Acquire it again in each
/// callback.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, sunrise_mod_api_macros::Wire)]
#[non_exhaustive]
pub struct PendingChildrenRef {
    pub(crate) epoch: WorldEpoch,
    pub(crate) action: u64,
    pub(crate) lease: u64,
}

impl PendingChildrenRef {
    /// Engine transport constructor. Every operation validates the complete lease.
    // hidden: engine transport only; arena validation grants authority
    #[doc(hidden)]
    pub fn new(epoch: WorldEpoch, action: u64, lease: u64) -> Self {
        Self {
            epoch,
            action,
            lease,
        }
    }

    /// Engine transport components.
    // hidden: engine transport only; arena validation grants authority
    #[doc(hidden)]
    pub fn parts(self) -> (WorldEpoch, u64, u64) {
        (self.epoch, self.action, self.lease)
    }
}

/// A detached, action-owned child. Append or replacement consumes its ownership exactly
/// once, even if a mod retained a copy of this token. It cannot name a resident entity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, sunrise_mod_api_macros::Wire)]
#[non_exhaustive]
pub struct UnattachedMob {
    pub(crate) mob: UnspawnedMobRef,
}

impl UnattachedMob {
    /// Engine transport constructor; the arena checks that the child is detached.
    // hidden: engine transport only; arena validation grants authority
    #[doc(hidden)]
    pub fn new(mob: UnspawnedMobRef) -> Self {
        Self { mob }
    }

    /// Borrow the child for reads or edits. This never transfers its ownership.
    pub fn mob(self) -> UnspawnedMobRef {
        self.mob
    }
}

/// Initial values for a new child. Construction runs no spawn finalization or insertion.
/// The engine checks every field before constructing anything or consuming an arena slot.
#[derive(Clone, Debug, PartialEq, sunrise_mod_api_macros::Wire)]
#[non_exhaustive]
pub struct PendingMobSpec {
    /// A registered mob type.
    pub kind: Identifier,
    /// Must equal the action's dimension.
    pub dimension: Identifier,
    /// Finite coordinates within the world's entity bounds.
    pub position: [f64; 3],
    /// Finite yaw, in degrees.
    pub yaw: f32,
    /// Whether to initialize the species' baby state. Breeding requires a baby.
    pub baby: bool,
    /// Explicit size for a sized species; absent for other species.
    pub size: Option<u32>,
}

impl PendingMobSpec {
    /// Initial values; set `size` for a species with an explicit size.
    pub fn new(
        kind: Identifier,
        dimension: Identifier,
        position: [f64; 3],
        yaw: f32,
        baby: bool,
    ) -> Self {
        Self {
            kind,
            dimension,
            position,
            yaw,
            baby,
            size: None,
        }
    }
}

/// An editor lent beside a collection. Reads and rejected operations leave the collection
/// unchanged. Accepted edits are visible to the next listener; unattached children die
/// when this lease ends.
/// This edits only nonresident objects; the action's transaction controls eventual insertion.
pub trait PendingChildrenEdits {
    /// The collection for this listener. Call it in every callback: a lease from an earlier
    /// listener, or from before a nested dispatch, answers `Expired`.
    fn children(&self) -> Result<PendingChildrenRef, UnspawnedMobError>;
    /// Number of attached children, in insertion order.
    fn len(&self, children: PendingChildrenRef) -> Result<u32, UnspawnedMobError>;
    /// Whether the collection is empty.
    fn is_empty(&self, children: PendingChildrenRef) -> Result<bool, UnspawnedMobError>;
    /// Reacquire a callback-scoped editor borrow of the child at `index`.
    fn child(
        &self,
        children: PendingChildrenRef,
        index: u32,
    ) -> Result<UnspawnedMobRef, UnspawnedMobError>;
    /// Construct a detached child; this does not add it to the ordered list.
    fn construct(
        &self,
        children: PendingChildrenRef,
        spec: PendingMobSpec,
    ) -> Result<UnattachedMob, UnspawnedMobError>;
    /// Append a detached child, consuming its ownership. Split lists hold at most 64
    /// children and breeding lists at most one; exceeding that limit is an error.
    fn append(
        &self,
        children: PendingChildrenRef,
        child: UnattachedMob,
    ) -> Result<(), UnspawnedMobError>;
    /// Replace one child in place, consuming the replacement and destroying the former child.
    fn replace(
        &self,
        children: PendingChildrenRef,
        index: u32,
        child: UnattachedMob,
    ) -> Result<(), UnspawnedMobError>;
    /// Destroy one child and expire all its borrows; later children retain their order.
    fn remove(&self, children: PendingChildrenRef, index: u32) -> Result<(), UnspawnedMobError>;
    /// Move an attached or detached child within the action's dimension.
    fn set_position(
        &self,
        mob: UnspawnedMobRef,
        position: [f64; 3],
    ) -> Result<(), UnspawnedMobError>;
    /// Set all initial yaw fields in degrees.
    fn set_yaw(&self, mob: UnspawnedMobRef, yaw: f32) -> Result<(), UnspawnedMobError>;
    /// Set a supported species' baby state. A breeding child must remain a baby.
    fn set_baby(&self, mob: UnspawnedMobRef, baby: bool) -> Result<(), UnspawnedMobError>;
    /// Set a sized species' size within its own bounds, without silently clamping it.
    fn set_size(&self, mob: UnspawnedMobRef, size: u32) -> Result<(), UnspawnedMobError>;
}

/// Engine backing for mutable access to a pending child, adding spatial and form writes beyond
/// [`UnspawnedMobWrite`](crate::UnspawnedMobWrite).
// hidden: the engine owns pending-child mutable access; listeners use [`UnspawnedMob`] and [`PendingChildEditor`].
#[doc(hidden)]
pub trait PendingChildWrite: crate::UnspawnedMobWrite {
    fn set_position(&mut self, position: [f64; 3]) -> Result<(), UnspawnedMobError>;
    fn set_yaw(&mut self, yaw: f32) -> Result<(), UnspawnedMobError>;
    fn set_baby(&mut self, baby: bool) -> Result<(), UnspawnedMobError>;
    fn set_size(&mut self, size: u32) -> Result<(), UnspawnedMobError>;
    fn as_any(self: Box<Self>) -> Box<dyn std::any::Any>;
}

/// An owned, engine-backed mob constructed for a pending-child hook (such as breeding) and not yet
/// added to a level.
///
/// Dropping an `UnspawnedMob` without attaching it to a slot cleanly discards it with no arena
/// entry or world state left behind.
// exhaustive: only the engine constructs an unspawned mob.
// no wire: an owned engine-backed unspawned mob, not a payload.
pub struct UnspawnedMob {
    inner: Box<dyn PendingChildWrite>,
}

impl UnspawnedMob {
    // hidden: the engine constructs an owned unspawned mob.
    #[doc(hidden)]
    pub fn new(inner: Box<dyn PendingChildWrite>) -> Self {
        Self { inner }
    }

    // hidden: the engine recovers its concrete pending child when attaching it to a slot.
    #[doc(hidden)]
    pub fn into_any(self) -> Box<dyn std::any::Any> {
        self.inner.as_any()
    }

    /// Borrow this unspawned mob as a read-only [`UnspawnedMobView`].
    pub fn as_view(&self) -> UnspawnedMobView<'_> {
        UnspawnedMobView::new(&*self.inner)
    }

    /// Borrow this unspawned mob as a mutable [`PendingChildEditor`].
    pub fn as_editor(&mut self) -> PendingChildEditor<'_> {
        PendingChildEditor::new(&mut *self.inner)
    }

    /// The mob's entity type (`minecraft:zombie`).
    pub fn kind(&self) -> Result<Identifier, UnspawnedMobError> {
        self.inner.kind()
    }

    /// Where the mob will join the level.
    pub fn position(&self) -> Result<[f64; 3], UnspawnedMobError> {
        self.inner.position()
    }

    /// The stack in `slot` (`LivingEntity.getItemBySlot`); `None` for an empty slot.
    pub fn equipment(&self, slot: EquipmentSlot) -> Result<Option<ItemStack>, UnspawnedMobError> {
        self.inner.equipment(slot)
    }

    /// An attribute's base value, before modifiers (`AttributeMap.getBaseValue`).
    /// `Unsupported` for an attribute the mob's type does not have.
    pub fn attribute_base(&self, attribute: &Identifier) -> Result<f64, UnspawnedMobError> {
        self.inner.attribute_base(attribute)
    }

    /// An attribute's value with its modifiers, its equipment's included
    /// (`AttributeMap.getValue`). `Unsupported` for an attribute the mob's type does not have.
    pub fn attribute_value(&self, attribute: &Identifier) -> Result<f64, UnspawnedMobError> {
        self.inner.attribute_value(attribute)
    }

    /// The tags on the mob so far (`Entity.getTags`).
    pub fn tags(&self) -> Result<Vec<String>, UnspawnedMobError> {
        self.inner.tags()
    }

    /// The mob's custom name, if it has one.
    pub fn custom_name(&self) -> Result<Option<Text>, UnspawnedMobError> {
        self.inner.custom_name()
    }

    /// Puts `item` in `slot` (`Mob.setItemSlot`): an API-owned stack; `None` empties the slot.
    /// Invalid item keys, counts, damage or components are refused with `Invalid`, without
    /// changing the slot. The stack's attribute modifiers apply to the mob at once.
    pub fn set_equipment(
        &mut self,
        slot: EquipmentSlot,
        item: Option<ItemStack>,
    ) -> Result<(), UnspawnedMobError> {
        self.inner.set_equipment(slot, item)
    }

    /// Sets an attribute's base value (`AttributeInstance.setBaseValue`), kept within the
    /// attribute's range.
    pub fn set_attribute_base(
        &mut self,
        attribute: &Identifier,
        base: f64,
    ) -> Result<(), UnspawnedMobError> {
        self.inner.set_attribute_base(attribute, base)
    }

    /// Adds a permanent modifier (`AttributeInstance.addPermanentModifier`), saved with the mob.
    pub fn add_attribute_modifier(
        &mut self,
        attribute: &Identifier,
        modifier: AttributeModifier,
    ) -> Result<(), UnspawnedMobError> {
        self.inner.add_attribute_modifier(attribute, modifier)
    }

    /// Removes the modifier `id` from the attribute; `false` when it had none.
    pub fn remove_attribute_modifier(
        &mut self,
        attribute: &Identifier,
        id: &Identifier,
    ) -> Result<bool, UnspawnedMobError> {
        self.inner.remove_attribute_modifier(attribute, id)
    }

    /// Adds a tag (`Entity.addTag`); `false` when the mob has it already.
    pub fn add_tag(&mut self, tag: &str) -> Result<bool, UnspawnedMobError> {
        self.inner.add_tag(tag)
    }

    /// Removes a tag (`Entity.removeTag`); `false` when the mob did not have it.
    pub fn remove_tag(&mut self, tag: &str) -> Result<bool, UnspawnedMobError> {
        self.inner.remove_tag(tag)
    }

    /// Names the mob (`Entity.setCustomName`), or takes its name away with `None`.
    /// A component deeper than 512 nodes is refused with `UnspawnedMobError::Text`
    /// before changing the mob's name.
    pub fn set_custom_name(&mut self, name: Option<Text>) -> Result<(), UnspawnedMobError> {
        self.inner.set_custom_name(name)
    }

    /// Move the child within the action's dimension.
    pub fn set_position(&mut self, position: [f64; 3]) -> Result<(), UnspawnedMobError> {
        self.inner.set_position(position)
    }

    /// Set all initial yaw fields in degrees.
    pub fn set_yaw(&mut self, yaw: f32) -> Result<(), UnspawnedMobError> {
        self.inner.set_yaw(yaw)
    }

    /// Set a supported species' baby state. A breeding child must remain a baby.
    pub fn set_baby(&mut self, baby: bool) -> Result<(), UnspawnedMobError> {
        self.inner.set_baby(baby)
    }

    /// Set a sized species' size within its own bounds, without silently clamping it.
    pub fn set_size(&mut self, size: u32) -> Result<(), UnspawnedMobError> {
        self.inner.set_size(size)
    }
}

/// A mutable scoped loan of a pending child held in a breeding slot or split list.
///
/// Edits apply at once, so reads on the editor and later listeners see them. The loan lasts only
/// for the borrow from the slot or list.
// exhaustive: only the engine constructs an editor.
// no wire: an exclusive scoped loan, not a payload.
pub struct PendingChildEditor<'a> {
    inner: &'a mut dyn PendingChildWrite,
}

impl<'a> PendingChildEditor<'a> {
    // hidden: the engine constructs an editor around its scoped pending-child loan.
    #[doc(hidden)]
    pub fn new(inner: &'a mut dyn PendingChildWrite) -> Self {
        Self { inner }
    }

    /// Borrow this editor as a read-only [`UnspawnedMobView`].
    pub fn as_view(&self) -> UnspawnedMobView<'_> {
        UnspawnedMobView::new(&*self.inner)
    }

    /// The mob's entity type (`minecraft:zombie`).
    pub fn kind(&self) -> Result<Identifier, UnspawnedMobError> {
        self.inner.kind()
    }

    /// Where the mob will join the level.
    pub fn position(&self) -> Result<[f64; 3], UnspawnedMobError> {
        self.inner.position()
    }

    /// The stack in `slot` (`LivingEntity.getItemBySlot`); `None` for an empty slot.
    pub fn equipment(&self, slot: EquipmentSlot) -> Result<Option<ItemStack>, UnspawnedMobError> {
        self.inner.equipment(slot)
    }

    /// An attribute's base value, before modifiers (`AttributeMap.getBaseValue`).
    /// `Unsupported` for an attribute the mob's type does not have.
    pub fn attribute_base(&self, attribute: &Identifier) -> Result<f64, UnspawnedMobError> {
        self.inner.attribute_base(attribute)
    }

    /// An attribute's value with its modifiers, its equipment's included
    /// (`AttributeMap.getValue`). `Unsupported` for an attribute the mob's type does not have.
    pub fn attribute_value(&self, attribute: &Identifier) -> Result<f64, UnspawnedMobError> {
        self.inner.attribute_value(attribute)
    }

    /// The tags on the mob so far (`Entity.getTags`).
    pub fn tags(&self) -> Result<Vec<String>, UnspawnedMobError> {
        self.inner.tags()
    }

    /// The mob's custom name, if it has one.
    pub fn custom_name(&self) -> Result<Option<Text>, UnspawnedMobError> {
        self.inner.custom_name()
    }

    /// Puts `item` in `slot` (`Mob.setItemSlot`): an API-owned stack; `None` empties the slot.
    /// Invalid item keys, counts, damage or components are refused with `Invalid`, without
    /// changing the slot. The stack's attribute modifiers apply to the mob at once.
    pub fn set_equipment(
        &mut self,
        slot: EquipmentSlot,
        item: Option<ItemStack>,
    ) -> Result<(), UnspawnedMobError> {
        self.inner.set_equipment(slot, item)
    }

    /// Sets an attribute's base value (`AttributeInstance.setBaseValue`), kept within the
    /// attribute's range.
    pub fn set_attribute_base(
        &mut self,
        attribute: &Identifier,
        base: f64,
    ) -> Result<(), UnspawnedMobError> {
        self.inner.set_attribute_base(attribute, base)
    }

    /// Adds a permanent modifier (`AttributeInstance.addPermanentModifier`), saved with the mob.
    pub fn add_attribute_modifier(
        &mut self,
        attribute: &Identifier,
        modifier: AttributeModifier,
    ) -> Result<(), UnspawnedMobError> {
        self.inner.add_attribute_modifier(attribute, modifier)
    }

    /// Removes the modifier `id` from the attribute; `false` when it had none.
    pub fn remove_attribute_modifier(
        &mut self,
        attribute: &Identifier,
        id: &Identifier,
    ) -> Result<bool, UnspawnedMobError> {
        self.inner.remove_attribute_modifier(attribute, id)
    }

    /// Adds a tag (`Entity.addTag`); `false` when the mob has it already.
    pub fn add_tag(&mut self, tag: &str) -> Result<bool, UnspawnedMobError> {
        self.inner.add_tag(tag)
    }

    /// Removes a tag (`Entity.removeTag`); `false` when the mob did not have it.
    pub fn remove_tag(&mut self, tag: &str) -> Result<bool, UnspawnedMobError> {
        self.inner.remove_tag(tag)
    }

    /// Names the mob (`Entity.setCustomName`), or takes its name away with `None`.
    /// A component deeper than 512 nodes is refused with `UnspawnedMobError::Text`
    /// before changing the mob's name.
    pub fn set_custom_name(&mut self, name: Option<Text>) -> Result<(), UnspawnedMobError> {
        self.inner.set_custom_name(name)
    }

    /// Move the child within the action's dimension.
    pub fn set_position(&mut self, position: [f64; 3]) -> Result<(), UnspawnedMobError> {
        self.inner.set_position(position)
    }

    /// Set all initial yaw fields in degrees.
    pub fn set_yaw(&mut self, yaw: f32) -> Result<(), UnspawnedMobError> {
        self.inner.set_yaw(yaw)
    }

    /// Set a supported species' baby state. A breeding child must remain a baby.
    pub fn set_baby(&mut self, baby: bool) -> Result<(), UnspawnedMobError> {
        self.inner.set_baby(baby)
    }

    /// Set a sized species' size within its own bounds, without silently clamping it.
    pub fn set_size(&mut self, size: u32) -> Result<(), UnspawnedMobError> {
        self.inner.set_size(size)
    }
}

/// Engine backing for the single optional child slot of `baby_entity_spawn`.
// hidden: the engine owns the breeding child slot; listeners use [`BabyEntitySpawnChild`].
#[doc(hidden)]
pub trait BabyEntitySpawnSlot {
    fn is_present(&self) -> bool;
    fn get(&self) -> Option<UnspawnedMobView<'_>>;
    fn get_mut(&mut self) -> Option<PendingChildEditor<'_>>;
    fn construct(&self, spec: PendingMobSpec) -> Result<UnspawnedMob, UnspawnedMobError>;
    fn replace(&mut self, child: UnspawnedMob) -> Result<Option<UnspawnedMob>, UnspawnedMobError>;
    fn take(&mut self) -> Option<UnspawnedMob>;
}

/// A scoped editor for the single optional breeding child of `baby_entity_spawn`
/// (NeoForge's `BabyEntitySpawnEvent.getChild` / `setChild`).
///
/// The slot starts holding the vanilla-constructed baby (with its inheritance already drawn).
/// Listeners may inspect it ([`Self::get`]), edit it in place ([`Self::get_mut`]), construct a
/// new baby ([`Self::construct`]) and swap it in ([`Self::replace`]), or remove it
/// ([`Self::take`] / [`Self::clear`]). Any [`UnspawnedMob`] not left in the slot is cleanly
/// discarded when dropped.
// exhaustive: only the engine constructs a breeding slot editor.
// no wire: an exclusive scoped loan, not a payload.
pub struct BabyEntitySpawnChild<'a> {
    inner: &'a mut dyn BabyEntitySpawnSlot,
}

impl<'a> BabyEntitySpawnChild<'a> {
    // hidden: the engine constructs a breeding slot editor around its scoped slot loan.
    #[doc(hidden)]
    pub fn new(inner: &'a mut dyn BabyEntitySpawnSlot) -> Self {
        Self { inner }
    }

    /// Whether the breeding slot currently holds a child.
    pub fn is_present(&self) -> bool {
        self.inner.is_present()
    }

    /// Whether the breeding slot is currently empty.
    pub fn is_empty(&self) -> bool {
        !self.inner.is_present()
    }

    /// Read-only view of the current breeding child, if one is present.
    pub fn get(&self) -> Option<UnspawnedMobView<'_>> {
        self.inner.get()
    }

    /// Mutable editor for the current breeding child, if one is present.
    pub fn get_mut(&mut self) -> Option<PendingChildEditor<'_>> {
        self.inner.get_mut()
    }

    /// Construct a detached breeding child from `spec` without attaching it to the slot.
    ///
    /// Runs no spawn finalization or level insertion. Dropping the returned [`UnspawnedMob`]
    /// without passing it to [`Self::replace`] cleanly discards it.
    pub fn construct(&self, spec: PendingMobSpec) -> Result<UnspawnedMob, UnspawnedMobError> {
        self.inner.construct(spec)
    }

    /// Put `child` into the breeding slot, returning any child that was previously there.
    ///
    /// Validates that `child` satisfies the breeding invariants (registered kind, action's
    /// dimension, spawnable position, finite yaw, baby state, valid size) before mutating
    /// the slot.
    pub fn replace(
        &mut self,
        child: UnspawnedMob,
    ) -> Result<Option<UnspawnedMob>, UnspawnedMobError> {
        self.inner.replace(child)
    }

    /// Remove and return the current breeding child, leaving the slot empty.
    pub fn take(&mut self) -> Option<UnspawnedMob> {
        self.inner.take()
    }

    /// Remove and discard the current breeding child, leaving the slot empty.
    pub fn clear(&mut self) {
        let _ = self.inner.take();
    }
}
