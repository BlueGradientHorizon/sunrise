//! Named ticket controllers (NeoForge's `TicketController` and `RegisterTicketControllersEvent`): a
//! mod registers each controller once at startup, from [`Hooks::register`](crate::Hooks::register),
//! and holds the [`TicketController`] it gets back to hold chunks with.

use crate::{
    LevelRef, ServerHost, TicketValidation,
    callback_owner::CallbackOwner,
    world::mod_chunk_tickets::{ControllerTickets, ModTicket},
};
use std::{
    collections::BTreeMap,
    hash::BuildHasher,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};

static NEXT_TOKEN: AtomicU64 = AtomicU64::new(1);

/// Issue a non-zero opaque token unique to one registration entry.
/// Mixing a monotonic counter with a per-process salt through SplitMix64's bijection keeps
/// tokens non-sequential and prevents handles from one registration table matching another.
pub(crate) fn issue_token() -> u64 {
    static SALT: OnceLock<u64> = OnceLock::new();
    let salt = *SALT.get_or_init(|| {
        std::collections::hash_map::RandomState::new().hash_one(&SALT as *const _ as usize)
    });
    loop {
        let seq = NEXT_TOKEN.fetch_add(1, Ordering::Relaxed);
        let mut x = seq.wrapping_add(salt).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        let token = x ^ (x >> 31);
        if token != 0 {
            return token;
        }
    }
}

// no wire: retained executable code and a borrow of staged tickets stay in-process.
/// Validate one controller's saved chunk tickets in the supplied server level
/// (NeoForge's `LoadingValidationCallback.validateTickets`). The callback runs
/// after `level_load` and before the first tick, only when that controller has
/// saved rows in the level. At startup all built-in levels are available before
/// validation begins. Remove stale tickets through the borrowed validation view;
/// the survivors force their chunks after validation. A panic is reported and
/// disables only this validator for the session. A phase refusal is reported
/// each time without disabling it. Neither undoes removals already made.
pub type LoadingValidationCallback =
    dyn Fn(&mut dyn ServerHost, LevelRef, &mut dyn TicketValidation) + Send + Sync + 'static;

// no wire: retained code, attribution and session poison state stay in-process.
/// One frozen controller's callback and its registered owner.
// hidden: the authoring bridge moves the binding without inspecting it.
#[doc(hidden)]
#[derive(Clone)]
// exhaustive: private fields keep callback ownership inside the binding.
pub struct RegisteredTicketValidator {
    callback: Arc<LoadingValidationCallback>,
    owner: CallbackOwner,
}

impl RegisteredTicketValidator {
    pub(crate) fn new(callback: Arc<LoadingValidationCallback>, owner: CallbackOwner) -> Self {
        Self { callback, owner }
    }

    /// Run the callback under its registered owner's panic and refusal policy.
    pub(crate) fn call(&self, invoke: impl FnOnce(&LoadingValidationCallback)) {
        // CallbackOwner reports refusals and panics; validation still promotes the survivors.
        let _ = self
            .owner
            .call("chunk ticket validator", "validate_tickets", || {
                invoke(self.callback.as_ref());
            });
    }
}

/// What the game knows about a registered controller.
pub(crate) struct ControllerInfo {
    /// The namespaced id, `mymod:quarry`, which the tickets are saved under.
    pub(crate) id: Box<str>,
    /// Opaque registration identity matched against issued [`TicketController`] handles.
    pub(crate) token: u64,
    /// The controller's retained saved-ticket validator, if registered.
    pub(crate) validator: Option<RegisteredTicketValidator>,
}

/// A ticket controller a mod registered with
/// [`Registrar::register_ticket_controller`](crate::TicketsRegistrar::register_ticket_controller): the
/// handle `chunk.set_mod_forced` and `level.mod_forced_chunks` take. Only the registrar makes
/// one, bound to an opaque registration identity, and it is `Copy`: keep it from `register`, for
/// example in a `OnceLock`. Active chunk-ticket reads and writes authorize by this registry-issued
/// handle rather than the current caller; separate controllers cannot read or release each other's
/// tickets, a name or slot alone cannot forge a handle, and a handle from a refused registration
/// or another registration table does nothing. Use its namespaced `id()` when you need to store a
/// controller name.
// no wire: the handle resolves only in the registry that issued it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: its fields are private, so mods cannot build or destructure it
pub struct TicketController {
    /// Its place in the installed controller table.
    pub(crate) slot: u32,
    /// Opaque registration identity issued alongside the table entry; 0 on refused handles.
    pub(crate) token: u64,
}

impl TicketController {
    /// The controller's namespaced id (`mymod:quarry`), as its tickets are saved under it; empty
    /// for a registration that was refused or a handle from another registration table.
    pub fn id(self) -> &'static str {
        self.info().map_or("", |info| &info.id)
    }

    pub(crate) fn active_slot(self) -> Option<u32> {
        self.info().map(|_| self.slot)
    }

    fn info(self) -> Option<&'static ControllerInfo> {
        if self.token == 0 {
            return None;
        }
        controllers()
            .get(self.slot as usize)
            .filter(|info| info.token == self.token)
    }
}

/// Save-serialization lookup: map a stored controller slot to its namespaced id without issuing a
/// [`TicketController`] handle.
pub(crate) fn id_for_slot(slot: u32) -> Option<&'static str> {
    controllers().get(slot as usize).map(|info| &*info.id)
}

/// Save-deserialization lookup: map a saved controller id to its installed slot without issuing a
/// [`TicketController`] handle.
pub(crate) fn slot_for_id(id: &str) -> Option<u32> {
    controllers()
        .iter()
        .position(|info| &*info.id == id)
        .map(|slot| slot as u32)
}

/// Run each registered controller's bound saved-ticket validator on its staged rows in slot order
/// and return the surviving tickets ready for activation.
pub(crate) fn validate_saved_tickets(
    host: &mut dyn ServerHost,
    level: LevelRef,
    loaded: Vec<ModTicket>,
) -> Vec<ModTicket> {
    let table = controllers();
    let mut ready = Vec::new();
    let mut groups = BTreeMap::<u32, (&'static RegisteredTicketValidator, Vec<ModTicket>)>::new();
    for ticket in loaded {
        if let Some(validator) = table
            .get(ticket.controller as usize)
            .and_then(|info| info.validator.as_ref())
            .filter(|validator| validator.owner.is_enabled())
        {
            groups
                .entry(ticket.controller)
                .or_insert_with(|| (validator, Vec::new()))
                .1
                .push(ticket);
        } else {
            ready.push(ticket);
        }
    }
    for (validator, mut rows) in groups.into_values() {
        validator.call(|callback| {
            callback(host, level, &mut ControllerTickets::new(&mut rows));
        });
        ready.append(&mut rows);
    }
    ready
}

#[cfg(any(test, feature = "testing"))]
thread_local! {
    /// The controllers a test's mods registered (`testing::with_dispatch`), in place of the
    /// process-wide table.
    pub(crate) static TEST_CONTROLLERS: std::cell::Cell<Option<&'static [ControllerInfo]>> =
        const { std::cell::Cell::new(None) };
}

fn controllers() -> &'static [ControllerInfo] {
    #[cfg(any(test, feature = "testing"))]
    if let Some(table) = TEST_CONTROLLERS.get() {
        return table;
    }
    crate::parts::installed_controllers()
}
