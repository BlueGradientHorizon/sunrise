//! The resident server ticker's two sequential loans: its subject and the world.
use crate::{
    BlockEntityDataError, BlockEntityItemError, BlockEntityRef, BlockState, Compound, ItemKey,
    ItemStack, JavaString, LevelRef, ServerHost, Tag, Transaction,
};

/// The block entity this server ticker is running. Its storage stays in the level.
/// Keep a [`BlockEntityRef`] from [`Self::handle`] if you need to find it after the call.
// exhaustive: only the engine constructs a view.
// no wire: an exclusive loan, not a payload.
pub struct BlockEntityView<'a> {
    subject: BlockEntityRef,
    state: BlockState,
    data: &'a mut dyn ResidentBlockEntity,
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
            state,
            data,
        }
    }

    /// The entity's block position.
    pub fn position(&self) -> [i32; 3] {
        self.subject.position
    }
    /// The level the entity is ticking in.
    pub fn level(&self) -> LevelRef {
        self.subject.level
    }
    /// The entity's block state. Reborrowing after a state change refuses the loan.
    pub fn state(&self) -> BlockState {
        self.state
    }
    /// The entity's immutable type id.
    pub fn id(&self) -> &str {
        self.data.id()
    }
    /// A generation-bound handle to keep after this tick.
    pub fn handle(&self) -> BlockEntityRef {
        self.subject
    }

    /// The live, transient integer slots its menu reads.
    pub fn data_slots(&self) -> &[i32] {
        self.data.data_slots()
    }
    /// Set one existing slot. False for an absent slot or an equal value.
    pub fn set_data_slot(&mut self, slot: u32, value: i32) -> bool {
        self.data.set_data_slot(slot, value)
    }
    /// Set and, if necessary, resize the menu data, without a block update packet.
    pub fn set_data_slots(&mut self, data: &[i32]) -> Result<bool, BlockEntityDataError> {
        self.data.set_data_slots(data)
    }

    /// The resident saved compound. Modeled item slots are separate; `Items`, when
    /// present, holds opaque saved data that has not been decoded into item slots.
    pub fn nbt(&self) -> &Compound {
        self.data.nbt()
    }
    /// Set or remove a mod-owned saved field, without reloading the block entity.
    /// False for an equal field, excessive nesting, or engine-owned identity or item fields.
    pub fn set_nbt(&mut self, key: JavaString, value: Option<Tag>) -> bool {
        self.data.set_nbt(key, value)
    }

    /// The declared internal slot count, independent of its external automation policy.
    pub fn item_slots(&self) -> usize {
        self.data.item_slots()
    }
    /// Borrow a modeled stack. `None` for an empty, opaque or undeclared slot.
    pub fn item_slot(&self, slot: u32) -> Option<BlockEntityStackView<'_>> {
        let (item, count) = self.data.item_slot(slot)?;
        Some(BlockEntityStackView {
            data: &*self.data,
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
        self.data.set_item_slot(slot, stack)
    }

    /// Queue one-shot fields for the next block-entity sync to its current viewers.
    /// An empty compound clears the queued fields. Nothing saves or sends them from this call.
    /// Aborting a transaction restores the batch that was waiting before its first edit.
    pub fn set_client_update_fields(&mut self, fields: Compound) {
        self.data.set_client_update_fields(fields);
    }
}

/// Read a resident stack without copying its components. Copy explicitly to retain or edit it.
// exhaustive: the borrowed backend cannot be constructed by a mod.
// no wire: borrowed stack storage.
pub struct BlockEntityStackView<'a> {
    data: &'a dyn ResidentBlockEntity,
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

/// Engine backing for a checked, journaled row loan.
// hidden: the engine owns row access and journaling; listeners use the checked BlockEntityView methods.
#[doc(hidden)]
pub trait ResidentBlockEntity {
    fn id(&self) -> &str;
    fn data_slots(&self) -> &[i32];
    fn set_data_slot(&mut self, slot: u32, value: i32) -> bool;
    fn set_data_slots(&mut self, data: &[i32]) -> Result<bool, BlockEntityDataError>;
    fn nbt(&self) -> &Compound;
    fn set_nbt(&mut self, key: JavaString, value: Option<Tag>) -> bool;
    fn item_slots(&self) -> usize;
    fn item_slot(&self, slot: u32) -> Option<(&str, u32)>;
    fn copy_item_slot(&self, slot: u32) -> Option<ItemStack>;
    fn set_item_slot(
        &mut self,
        slot: u32,
        stack: Option<ItemStack>,
    ) -> Result<bool, BlockEntityItemError>;
    fn set_client_update_fields(&mut self, fields: Compound);
}
