//! `host.transaction` (#41 §7): NeoForge 26.3's `Transaction`. Changes made through the
//! capability helpers inside one are kept by the game and made final only when the outermost
//! transaction commits; an abort puts back everything its closure changed.
use crate::ServerHost;
use std::{fmt, marker::PhantomData, rc::Rc};

// no wire: only the host can lend this thread-bound authority over a live journal.
/// A loan of write authority in one open transaction, as NeoForge's `TransactionContext`.
/// Pass this loan and the accompanying server host to handler, item-place and world fluid
/// writes. It cannot close the transaction, be encoded as payload data, or outlive the
/// callback it was lent to. A live loan from another host or an enclosing ancestor frame is
/// not authority for the innermost open transaction on this host.
// exhaustive: its fields are private, so mods cannot build or destructure it.
pub struct TransactionContext {
    frame: TransactionFrame,
    _thread: PhantomData<Rc<()>>,
}

impl TransactionContext {
    /// The nesting depth at which this context was lent: zero at the root, plus one per
    /// enclosing transaction. Depth alone cannot identify a frame or journal.
    pub fn depth(&self) -> usize {
        self.frame.depth
    }
}

impl fmt::Debug for TransactionContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TransactionContext")
            .field("depth", &self.depth())
            .finish_non_exhaustive()
    }
}

// no wire: process-local identity of a journal frame.
/// A key for the host that owns a journal frame, not a write capability or close handle.
// hidden: the game's side of TransactionContext, re-exported through `__private`.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransactionFrame {
    journal: usize,
    sequence: u64,
    depth: usize,
}

impl TransactionFrame {
    /// Mints a frame identity scoped to `journal` (a live non-ZST address unique to the
    /// owning journal), monotonically increasing `sequence`, and nesting `depth`.
    pub fn new(journal: usize, sequence: u64, depth: usize) -> Self {
        Self {
            journal,
            sequence,
            depth,
        }
    }

    /// The depth at which this frame was installed.
    pub fn depth(self) -> usize {
        self.depth
    }

    /// Whether the loan came from this journal frame.
    pub fn matches(self, context: &TransactionContext) -> bool {
        self == context.frame
    }

    /// Lends write authority without handing out an owned context.
    pub fn lend<T>(self, f: impl for<'loan> FnOnce(&'loan TransactionContext) -> T) -> T {
        f(&TransactionContext {
            frame: self,
            _thread: PhantomData,
        })
    }
}

// no wire: observes the process-local transaction journal.
/// A read-only observation of the host's journal phase (`Transaction.Lifecycle`), returned by
/// [`transaction_lifecycle`](crate::prelude::ModStateServerHost::transaction_lifecycle).
/// It is not a write or close capability or a Wire payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum TransactionLifecycle {
    /// No transaction is open.
    None,
    /// A transaction is open and none is closing.
    Open,
    /// A nested transaction is closing: committing into the one around it, or aborting.
    Closing,
    /// The outermost transaction is closing. On commit, effects such as `contents_changed`
    /// and block updates run after its frame has been removed.
    RootClosing,
}

// no wire: a local refusal to lend transaction authority, not a mod boundary payload.
// exhaustive: a unit marker.
/// This host cannot open a transaction in the current phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransactionUnavailable;

impl fmt::Display for TransactionUnavailable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("transaction unavailable in the current phase")
    }
}

impl std::error::Error for TransactionUnavailable {}

// no wire: the error belongs to a local closure and can contain any Rust type.
// exhaustive: the closure either did not run or ran and returned an error.
/// Why [`HostTransaction::transaction`] did not commit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransactionError<E> {
    /// The host could not open a transaction. The closure was not called.
    Unavailable,
    /// The closure returned this error. Its writes were rolled back.
    Aborted(E),
}

impl<E> TransactionError<TransactionError<E>> {
    pub(crate) fn flatten(self) -> TransactionError<E> {
        match self {
            Self::Unavailable => TransactionError::Unavailable,
            Self::Aborted(inner) => inner,
        }
    }
}

impl<E: fmt::Display> fmt::Display for TransactionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable => TransactionUnavailable.fmt(f),
            Self::Aborted(error) => error.fmt(f),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for TransactionError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unavailable => None,
            Self::Aborted(error) => Some(error),
        }
    }
}

/// Asserts that `expected` matches the innermost open transaction on `host` (`None` requiring no
/// open transaction), panicking as NeoForge's `TransactionManager.validateCurrentTransaction`
/// throws `IllegalStateException` on a mismatched, foreign, or ancestor context.
#[track_caller]
pub(crate) fn assert_transaction_matches(
    host: &dyn ServerHost,
    expected: Option<&TransactionContext>,
) {
    if host.transaction_matches(expected) {
        return;
    }
    match expected {
        Some(tx) => panic!(
            "transaction context at depth {} does not match the host's innermost open transaction",
            tx.depth()
        ),
        None => panic!("expected no open transaction on this host, but a transaction is open"),
    }
}

/// Transactions are available only on the logical server. Both the host and the context are
/// lent to the callback; neither can escape it.
///
/// ```
/// use mod_api::{
///     BlockEntityError, BlockEntityRef, HostTransaction, ServerHost, TransactionError,
/// };
/// fn write(host: &mut dyn ServerHost, block: BlockEntityRef) {
///     let _: Result<(), TransactionError<BlockEntityError>> = host.transaction(|host, _tx| {
///         block.set_changed(host)?;
///         Ok(())
///     });
/// }
/// ```
/// ```compile_fail
/// use mod_api::{BlockEntityError, BlockEntityRef, Host, HostTransaction, TransactionError};
/// fn write(host: &mut dyn Host, block: BlockEntityRef) {
///     let _: Result<(), TransactionError<BlockEntityError>> = host.transaction(|host, _tx| {
///         block.set_changed(host)?;
///         Ok(())
///     });
/// }
/// ```
/// ```compile_fail
/// use mod_api::{ClientHost, HostTransaction, TransactionError};
/// fn client(host: &mut dyn ClientHost) {
///     let _: Result<(), TransactionError<()>> = host.transaction(|_, _| Ok(()));
/// }
/// ```
/// ```compile_fail
/// use mod_api::{HostTransaction, ServerHost, TransactionContext, TransactionError};
/// fn escape(host: &mut dyn ServerHost) {
///     let _: Result<&TransactionContext, TransactionError<()>> = host.transaction(|_, tx| Ok(tx));
/// }
/// ```
/// ```compile_fail
/// use mod_api::{HostTransaction, ServerHost, TransactionError};
/// fn close(host: &mut dyn ServerHost) {
///     let _: Result<(), TransactionError<()>> = host.transaction(|host, tx| {
///         host.transaction_close(tx, true);
///         Ok(())
///     });
/// }
/// ```
pub trait HostTransaction: ServerHost {
    /// Runs `f` in a transaction, nested in the open one if any (`Transaction.openRoot` and
    /// `open`). `Ok` commits into the transaction around it or, at the outermost, for good,
    /// with the world told of each change once. `Err` aborts, restoring every storage it wrote.
    /// An unwind out of `f` aborts this transaction before reaching the caller; the transaction
    /// around it stays open.
    ///
    /// An error returned by `f` becomes [`TransactionError::Aborted`]. If a transaction cannot
    /// be opened, [`TransactionError::Unavailable`] means `f` did not run.
    ///
    /// `f` gets a loan of `&mut dyn ServerHost`, even on a concrete receiver, and a
    /// [`TransactionContext`] to write in. Only the
    /// host's owning scope ends the transaction.
    ///
    /// Covered: block entities (item slots, saved compound, custom name and attachments), cauldrons,
    /// players' inventories, cursors, ender chests and persistent data, entities' inventories
    /// ([`ItemPlace::EntitySlot`](crate::ItemPlace::EntitySlot)) and item entities' stacks.
    /// Aborting restores a player's recorded storage even if the player dies, is in another
    /// level, or is no longer the acting player when the transaction closes.
    fn transaction<T, E>(
        &mut self,
        f: impl for<'loan> FnOnce(&'loan mut dyn ServerHost, &'loan TransactionContext) -> Result<T, E>,
    ) -> Result<T, TransactionError<E>> {
        let mut f = Some(f);
        let mut result = None;
        self.with_transaction_scope(&mut |host, tx| {
            let Some(f) = f.take() else { return false };
            let outcome = f(host, tx);
            let commit = outcome.is_ok();
            result = Some(outcome);
            commit
        })
        .map_err(|TransactionUnavailable| TransactionError::Unavailable)?;
        result
            .ok_or(TransactionError::Unavailable)?
            .map_err(TransactionError::Aborted)
    }
}

impl<H: ServerHost + ?Sized> HostTransaction for H {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::FakeHost;
    use crate::{
        Host, ITEMS, Identifier, ItemResource, ItemStack, ResourceHandler, prelude::ItemsHost,
    };

    const AT: [i32; 3] = [0, 64, 0];

    fn fake_with_stone() -> (FakeHost, ItemResource) {
        let mut fake = FakeHost::new();
        let stone = Identifier::parse("minecraft:stone").unwrap();
        fake.registries.resolve_items([stone.clone()]).unwrap();
        let stone = ItemStack::new(fake.item_key(&stone).unwrap(), 1).unwrap();
        let resource = ItemResource::of(&stone);
        fake.level
            .block_entity(AT)
            .with_items(1)
            .items
            .as_mut()
            .unwrap()
            .slots[0] = Some(stone);
        (fake, resource)
    }

    struct NoTx;
    impl Host for NoTx {}
    crate::impl_host_areas!(NoTx);
    impl ServerHost for NoTx {}
    crate::impl_server_host_areas!(NoTx);

    #[test]
    #[should_panic(expected = "does not match the host's innermost open transaction")]
    fn generated_handle_checks_a_foreign_context_before_backend_lookup() {
        let mut owner = FakeHost::new();
        let level = owner.level.level();
        let _ = owner.transaction(|_, tx| {
            let _ = level.pickup_fluid(&mut NoTx, AT, None, tx);
            Ok::<_, ()>(())
        });
    }

    #[test]
    fn a_host_without_transactions_returns_unavailable_without_calling_closure() {
        let mut host = NoTx;
        assert!(host.transaction_matches(None));
        let outcome = host.transaction(|_, _| -> Result<(), ()> {
            panic!("closure must not run when transactions are unavailable");
        });
        assert_eq!(outcome, Err(TransactionError::Unavailable));
    }

    #[test]
    fn a_live_context_from_another_host_panics_on_write_and_leaves_state_unchanged() {
        use std::panic::{AssertUnwindSafe, catch_unwind};
        let (mut owner, resource) = fake_with_stone();
        let mut other = owner.clone();
        let level = owner.level.level();
        let items = level.capability(&mut owner, &ITEMS, AT, None).unwrap();

        other
            .transaction(|other, foreign| {
                owner
                    .transaction(|host, own| {
                        assert_eq!((foreign.depth(), own.depth()), (0, 0));
                        assert!(!host.transaction_matches(Some(foreign)));
                        assert!(host.transaction_matches(Some(own)));
                        let caught = catch_unwind(AssertUnwindSafe(|| {
                            items.extract_at(host, foreign, 0, &resource, 1)
                        }));
                        assert!(caught.is_err());
                        assert_eq!(items.amount(host, 0), 1);
                        assert_eq!(items.extract_at(host, own, 0, &resource, 1), 1);
                        Ok::<_, ()>(())
                    })
                    .unwrap();
                assert!(other.transaction_matches(Some(foreign)));
                Ok::<_, ()>(())
            })
            .unwrap();
        assert_eq!(items.amount(&mut owner, 0), 0);
        assert_eq!(items.amount(&mut other, 0), 1);
        assert_eq!(owner.journal.closed(), [(1, true)]);
        assert_eq!(other.journal.closed(), [(1, true)]);
    }

    #[test]
    #[should_panic(expected = "does not match the host's innermost open transaction")]
    fn an_ancestor_context_panics_while_a_nested_transaction_is_open() {
        let (mut fake, resource) = fake_with_stone();
        let level = fake.level.level();
        let items = level.capability(&mut fake, &ITEMS, AT, None).unwrap();

        let _ = fake.transaction(|host, outer| {
            let _ = host.transaction(|host, inner| {
                assert_eq!((outer.depth(), inner.depth()), (0, 1));
                assert!(!host.transaction_matches(Some(outer)));
                assert!(host.transaction_matches(Some(inner)));
                items.extract_at(host, outer, 0, &resource, 1);
                Ok::<_, ()>(())
            });
            Ok::<_, ()>(())
        });
    }

    #[test]
    fn a_transaction_commits_on_ok_and_aborts_on_err_and_nests() {
        let mut fake = FakeHost::new();
        let host: &mut dyn ServerHost = &mut fake;
        let outer = host.transaction(|host, _| {
            let inner = host.transaction(|_, _| Err::<(), _>("refused"));
            assert_eq!(inner, Err(TransactionError::Aborted("refused")));
            let committed = host.transaction(|_, _| Ok::<_, ()>(7)).unwrap();
            Ok::<_, ()>(committed)
        });
        assert_eq!(outer, Ok(7));
        assert_eq!(fake.journal.closed(), [(2, false), (2, true), (1, true)]);
        assert_eq!(fake.journal.depth(), 0);
    }

    #[test]
    fn a_panic_aborts_exactly_the_transaction_it_leaves() {
        use std::panic::{AssertUnwindSafe, catch_unwind};
        let mut fake = FakeHost::new();
        let host: &mut dyn ServerHost = &mut fake;
        // A child's panic caught inside the root: the child aborts, the root still commits.
        let root = host.transaction(|host, _| {
            let child = catch_unwind(AssertUnwindSafe(|| {
                host.transaction(|_, _| -> Result<(), ()> { panic!("a broken handler") })
            }));
            assert!(child.is_err());
            Ok::<_, ()>(())
        });
        assert_eq!(root, Ok(()));
        // A root's panic caught outside it: the root aborts before the catch sees anything.
        let host: &mut dyn ServerHost = &mut fake;
        let caught = catch_unwind(AssertUnwindSafe(|| {
            host.transaction(|_, _| -> Result<(), ()> { panic!("a broken handler") })
        }));
        assert!(caught.is_err());
        assert_eq!(fake.journal.closed(), [(2, false), (1, true), (1, false)]);
        assert_eq!(fake.journal.depth(), 0);
    }
}
