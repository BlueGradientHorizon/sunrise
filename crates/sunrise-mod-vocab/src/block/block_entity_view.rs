//! The unified block-entity view: lent mutably by the resident server ticker and
//! read-only by [`BlockEntityRef::with_view`].
use crate::{
    BlockEntityDataError, BlockEntityItemError, BlockEntityRef, BlockEntityTypeKey, BlockState,
    Compound, ItemKey, ItemStack, JavaString, LevelRef, ServerHost, Tag, Transaction,
};

enum BlockEntityAccess<'a> {
    Read(&'a dyn ResidentBlockEntityRead),
    Mut {
        state: BlockState,
        data: &'a mut dyn ResidentBlockEntity,
    },
}

/// A scoped view of a block entity: lent mutably by [`BlockEntityTickScope::with_view`]
/// during a resident server tick, or read-only (`&BlockEntityView<'_>`) by
/// [`BlockEntityRef::with_view`]. Its storage stays in the level.
/// Keep a [`BlockEntityRef`] from [`Self::handle`] if you need to find it after the call.
// exhaustive: only the engine constructs a view.
// no wire: a scoped loan, not a payload.
pub struct BlockEntityView<'a> {
    subject: BlockEntityRef,
    access: BlockEntityAccess<'a>,
}

impl<'a> BlockEntityView<'a> {
    // hidden: the engine builds the view after checking the subject and borrowing its resident row.
    #[doc(hidden)]
    pub fn new(
        subject: BlockEntityRef,
        state: BlockState,
        data: &'a mut dyn ResidentBlockEntity,
    ) -> Self {
        Self {
            subject,
            access: BlockEntityAccess::Mut { state, data },
        }
    }

    // hidden: the engine builds a read-only view for BlockEntityRef::with_view.
    #[doc(hidden)]
    pub fn read_only(subject: BlockEntityRef, data: &'a dyn ResidentBlockEntityRead) -> Self {
        Self {
            subject,
            access: BlockEntityAccess::Read(data),
        }
    }

    fn read(&self) -> &dyn ResidentBlockEntityRead {
        match &self.access {
            BlockEntityAccess::Read(data) => *data,
            BlockEntityAccess::Mut { data, .. } => data.as_read(),
        }
    }

    /// The entity's block position.
    pub fn position(&self) -> [i32; 3] {
        self.subject.position
    }
    /// The level the entity is in.
    pub fn level(&self) -> LevelRef {
        self.subject.level
    }
    /// The entity's block state, when available in the current world view.
    /// In a resident tick, reborrowing after a state change refuses the loan.
    pub fn state(&self) -> Option<BlockState> {
        match &self.access {
            BlockEntityAccess::Read(data) => data.state(),
            BlockEntityAccess::Mut { state, .. } => Some(*state),
        }
    }
    /// The entity's immutable type id.
    pub fn id(&self) -> &str {
        self.read().id()
    }
    /// The entity's resolved block-entity type key (`BlockEntity.getType`), when known in
    /// the current registry.
    pub fn kind(&self) -> Option<BlockEntityTypeKey> {
        self.read().kind()
    }
    /// A generation-bound handle to keep after this scope.
    pub fn handle(&self) -> BlockEntityRef {
        self.subject
    }

    /// The live, transient integer slots a mod block entity's menu reads.
    /// `None` for vanilla block entities or views without menu data.
    pub fn data_slots(&self) -> Option<&[i32]> {
        self.read().data_slots()
    }
    /// Set one existing slot. False for an absent slot, an equal value or a read-only loan.
    pub fn set_data_slot(&mut self, slot: u32, value: i32) -> bool {
        match &mut self.access {
            BlockEntityAccess::Mut { data, .. } => data.set_data_slot(slot, value),
            BlockEntityAccess::Read(_) => false,
        }
    }
    /// Set and, if necessary, resize the menu data, without a block update packet.
    pub fn set_data_slots(&mut self, data: &[i32]) -> Result<bool, BlockEntityDataError> {
        match &mut self.access {
            BlockEntityAccess::Mut { data: resident, .. } => resident.set_data_slots(data),
            BlockEntityAccess::Read(_) => Ok(false),
        }
    }

    /// The resident saved compound of a mod block entity. Modeled item slots are separate;
    /// `Items`, when present, holds opaque saved data that has not been decoded into item slots.
    /// `None` for vanilla block entities or views without resident mod NBT.
    pub fn nbt(&self) -> Option<&Compound> {
        self.read().nbt()
    }
    /// Set or remove a mod-owned saved field, without reloading the block entity.
    /// False for an equal field, excessive nesting, engine-owned identity or item fields,
    /// or a read-only loan.
    pub fn set_nbt(&mut self, key: JavaString, value: Option<Tag>) -> bool {
        match &mut self.access {
            BlockEntityAccess::Mut { data, .. } => data.set_nbt(key, value),
            BlockEntityAccess::Read(_) => false,
        }
    }

    /// The declared internal slot count, independent of its external automation policy.
    pub fn item_slots(&self) -> usize {
        self.read().item_slots()
    }
    /// Borrow a modeled stack. `None` for an empty, opaque or undeclared slot.
    pub fn item_slot(&self, slot: u32) -> Option<BlockEntityStackView<'_>> {
        let data = self.read();
        let (item, count) = data.item_slot(slot)?;
        Some(BlockEntityStackView {
            data,
            slot,
            item,
            count,
        })
    }
    /// Replace a declared internal slot. An explicit write claims any opaque saved row at
    /// that index. Rejected stacks leave storage alone; untouched slots are not normalized.
    pub fn set_item_slot(
        &mut self,
        slot: u32,
        stack: Option<ItemStack>,
    ) -> Result<bool, BlockEntityItemError> {
        match &mut self.access {
            BlockEntityAccess::Mut { data, .. } => data.set_item_slot(slot, stack),
            BlockEntityAccess::Read(_) => Ok(false),
        }
    }

    /// Queue one-shot fields for the next explicit block-entity sync to its current viewers.
    /// An empty compound clears the queued fields. Nothing saves or sends them from this call.
    pub fn set_client_update_fields(&mut self, fields: Compound) {
        if let BlockEntityAccess::Mut { data, .. } = &mut self.access {
            data.set_client_update_fields(fields);
        }
    }
}

/// Read a resident stack without copying its components. Copy explicitly to retain or edit it.
// exhaustive: the borrowed backend cannot be constructed by a mod.
// no wire: borrowed stack storage.
pub struct BlockEntityStackView<'a> {
    data: &'a dyn ResidentBlockEntityRead,
    slot: u32,
    item: &'a str,
    count: u32,
}

impl BlockEntityStackView<'_> {
    /// The stack's registered item, or `None` if not visible in the mod item registry.
    pub fn item(&self) -> Option<ItemKey> {
        crate::content::registry_keys::item_key(self.item)
    }
    /// The resident item count.
    pub fn count(&self) -> u32 {
        self.count
    }
    /// Materialize the complete stack, including components. `None` if it cannot be
    /// represented in the mod API.
    pub fn to_owned(&self) -> Option<ItemStack> {
        self.data.copy_item_slot(self.slot)
    }
}

/// Sequential access to the ticking block entity and the server Host.
///
/// ```compile_fail,E0501
/// use mod_vocab::BlockEntityTickEvent;
/// fn tick(event: &mut BlockEntityTickEvent<'_>) {
///     event.subject.with_view(|machine| {
///         let _host = event.subject.host();
///         let _slots = machine.data_slots();
///     });
/// }
/// ```
// exhaustive: the engine constructs this scope around a single dispatch.
// no wire: the sequential loans cannot escape their call.
pub struct BlockEntityTickScope<'a>(&'a mut dyn BlockEntityTickAccess);

impl<'a> BlockEntityTickScope<'a> {
    // hidden: the engine supplies the loan backend for this one ticker dispatch.
    #[doc(hidden)]
    pub fn new(access: &'a mut dyn BlockEntityTickAccess) -> Self {
        Self(access)
    }

    /// Lend the original subject for `f`. Once `f` returns, you can use the Host
    /// and then ask for the view again. Each loan checks the level, block state and
    /// generation; removal or replacement returns `None` without running `f`.
    ///
    /// Successful storage edits dirty the chunk once per loan (at commit in a
    /// transaction) and are visible to subsequent loans and listeners. Menu data
    /// uses the normal menu sync. No edit sends a block update or notifies neighbors.
    pub fn with_view<R>(&mut self, f: impl FnOnce(BlockEntityView<'_>) -> R) -> Option<R> {
        let mut f = Some(f);
        let mut result = None;
        self.0.with_view(&mut |view| {
            if let Some(f) = f.take() {
                result = Some(f(view));
            }
        });
        result
    }

    /// Borrow the Host for other objects or world operations. No view is live while
    /// this borrow is held. Reborrow the view afterwards to re-check the subject.
    pub fn host(&mut self) -> &mut dyn ServerHost {
        self.0.host()
    }

    /// Run an atomic operation spanning the subject and other world storage. The
    /// callback can alternate between view and Host, under one explicit transaction.
    /// `Err` or unwind rolls back its journaled edits. Ordinary loans join an already
    /// open transaction and never open a savepoint by themselves.
    pub fn transaction<T, E>(
        &mut self,
        f: impl FnOnce(&mut Self, &Transaction) -> Result<T, E>,
    ) -> Result<T, E> {
        crate::item::transaction::run_transaction_on(
            self,
            |scope| scope.0.host().transaction_open(),
            |scope, seal, commit| scope.0.host().transaction_close(seal, commit),
            f,
        )
    }
}

/// Engine backing for the sequential loans.
// hidden: the engine owns the alternating view and Host loans; listeners receive BlockEntityTickScope.
#[doc(hidden)]
pub trait BlockEntityTickAccess {
    fn with_view(&mut self, f: &mut dyn FnMut(BlockEntityView<'_>));
    fn host(&mut self) -> &mut dyn ServerHost;
}

/// Read-only engine backing for a checked block entity view.
// hidden: the engine owns row access; listeners use the checked BlockEntityView methods.
#[doc(hidden)]
pub trait ResidentBlockEntityRead {
    fn id(&self) -> &str;
    fn kind(&self) -> Option<BlockEntityTypeKey> {
        crate::content::registry_keys::block_entity_type_key(self.id())
    }
    fn state(&self) -> Option<BlockState> {
        None
    }
    fn data_slots(&self) -> Option<&[i32]> {
        None
    }
    fn nbt(&self) -> Option<&Compound> {
        None
    }
    fn item_slots(&self) -> usize {
        0
    }
    fn item_slot(&self, _slot: u32) -> Option<(&str, u32)> {
        None
    }
    fn copy_item_slot(&self, _slot: u32) -> Option<ItemStack> {
        None
    }
}

/// Engine backing for a checked, journaled row loan.
// hidden: the engine owns row access and journaling; listeners use the checked BlockEntityView methods.
#[doc(hidden)]
pub trait ResidentBlockEntity: ResidentBlockEntityRead {
    fn as_read(&self) -> &dyn ResidentBlockEntityRead;
    fn set_data_slot(&mut self, slot: u32, value: i32) -> bool;
    fn set_data_slots(&mut self, data: &[i32]) -> Result<bool, BlockEntityDataError>;
    fn set_nbt(&mut self, key: JavaString, value: Option<Tag>) -> bool;
    fn set_item_slot(
        &mut self,
        slot: u32,
        stack: Option<ItemStack>,
    ) -> Result<bool, BlockEntityItemError>;
    fn set_client_update_fields(&mut self, fields: Compound);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DimensionKey, WorldEpoch};

    struct StubRead {
        id: &'static str,
        kind: Option<BlockEntityTypeKey>,
        state: Option<BlockState>,
        slots: Option<Vec<i32>>,
        nbt: Option<Compound>,
    }

    impl ResidentBlockEntityRead for StubRead {
        fn id(&self) -> &str {
            self.id
        }
        fn kind(&self) -> Option<BlockEntityTypeKey> {
            self.kind
        }
        fn state(&self) -> Option<BlockState> {
            self.state
        }
        fn data_slots(&self) -> Option<&[i32]> {
            self.slots.as_deref()
        }
        fn nbt(&self) -> Option<&Compound> {
            self.nbt.as_ref()
        }
    }

    #[test]
    fn read_only_view_exposes_metadata_and_optional_fields() {
        let level = LevelRef::new(WorldEpoch::new(1), DimensionKey::OVERWORLD);
        let subject = BlockEntityRef {
            level,
            position: [4, 64, -2],
            generation: 9,
        };
        let nbt = Compound::from([("fuel".into(), Tag::Int(12))]);
        let backing = StubRead {
            id: "test:kiln",
            kind: Some(BlockEntityTypeKey(3)),
            state: None,
            slots: Some(vec![7, 8]),
            nbt: Some(nbt.clone()),
        };
        let mut view = BlockEntityView::read_only(subject, &backing);
        assert_eq!(view.handle(), subject);
        assert_eq!(view.level(), level);
        assert_eq!(view.position(), [4, 64, -2]);
        assert_eq!(view.id(), "test:kiln");
        assert_eq!(view.kind(), Some(BlockEntityTypeKey(3)));
        assert_eq!(view.state(), None);
        assert_eq!(view.data_slots(), Some(&[7, 8][..]));
        assert_eq!(view.nbt(), Some(&nbt));
        assert_eq!(view.item_slots(), 0);
        assert!(view.item_slot(0).is_none());
        assert!(!view.set_data_slot(0, 9));
        assert_eq!(view.set_data_slots(&[1]), Ok(false));
        assert!(!view.set_nbt("fuel".into(), Some(Tag::Int(1))));
        assert_eq!(view.set_item_slot(0, None), Ok(false));
        view.set_client_update_fields(Compound::new());
    }
}
