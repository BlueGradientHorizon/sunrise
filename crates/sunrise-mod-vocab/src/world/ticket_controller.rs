//! Named ticket controllers (NeoForge's `TicketController` and `RegisterTicketControllersEvent`): a
//! mod registers each controller once at startup, from [`Hooks::register`](crate::Hooks::register),
//! and holds the [`TicketController`] it gets back to hold chunks with.

use crate::{LevelRef, ServerHost, TicketValidation, callback_owner::CallbackOwner};
use std::sync::Arc;

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
    // hidden: game-side dispatch, not part of the mod surface.
    #[doc(hidden)]
    pub fn call(&self, invoke: impl FnOnce(&LoadingValidationCallback)) {
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
    /// The controller's retained saved-ticket validator, if registered.
    pub(crate) validator: Option<RegisteredTicketValidator>,
}

/// A ticket controller a mod registered with
/// [`Registrar::register_ticket_controller`](crate::TicketsRegistrar::register_ticket_controller): the
/// handle `chunk.set_mod_forced` and `level.mod_forced_chunks` take. Only the registrar makes
/// one, resolved once there, and it is `Copy`: keep it from `register`, for example in a
/// `OnceLock`. Active chunk-ticket reads and writes authorize by this registry-issued handle
/// rather than the current caller; separate controllers cannot read or release each other's
/// tickets, and a handle from a refused registration does nothing. Use its namespaced `id()` when
/// you need to store a controller name; its slot is local to this game's registry.
// no wire: the handle resolves only in the registry that issued it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: its fields are private, so mods cannot build or destructure it
pub struct TicketController {
    /// Its place in the installed controller table.
    pub(crate) slot: u32,
}

impl TicketController {
    /// The controller's namespaced id (`mymod:quarry`), as its tickets are saved under it; empty
    /// for a registration that was refused.
    pub fn id(self) -> &'static str {
        self.info().map_or("", |info| &info.id)
    }
    /// The controller in slot `slot` of the installed table. For the game.
    // hidden: a game-side helper, not part of the mod surface
    #[doc(hidden)]
    pub fn from_slot(slot: u32) -> Option<Self> {
        controllers().get(slot as usize).map(|_| Self { slot })
    }
    /// The installed controller with this id. For the game.
    // hidden: a game-side helper, not part of the mod surface
    #[doc(hidden)]
    pub fn from_id(id: &str) -> Option<Self> {
        let slot = controllers().iter().position(|info| &*info.id == id)?;
        Some(Self { slot: slot as u32 })
    }
    /// Its slot in the installed table. For the game.
    // hidden: a game-side helper, not part of the mod surface
    #[doc(hidden)]
    pub fn slot(self) -> u32 {
        self.slot
    }
    /// The controller's enabled saved-ticket validator, if any. For the game.
    // hidden: a game-side helper, not part of the mod surface
    #[doc(hidden)]
    pub fn validator(self) -> Option<&'static RegisteredTicketValidator> {
        self.info()?
            .validator
            .as_ref()
            .filter(|validator| validator.owner.is_enabled())
    }
    fn info(self) -> Option<&'static ControllerInfo> {
        controllers().get(self.slot as usize)
    }
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
