//! Construction before the immutable controller table exists.
use crate::{
    LoadingValidationCallback, callback_owner::CallbackOwner,
    world::ticket_controller::RegisteredTicketValidator,
};
use std::sync::Arc;

/// Cold authoring access to the opaque token representation.
pub fn ticket_controller(slot: u32) -> crate::TicketController {
    crate::TicketController { slot }
}

/// Bind a saved-ticket validator to the mod that registered its controller.
pub fn registered_ticket_validator(
    mod_id: &'static str,
    mod_index: usize,
    callback: Arc<LoadingValidationCallback>,
) -> RegisteredTicketValidator {
    RegisteredTicketValidator::new(callback, CallbackOwner::for_index(mod_id, mod_index))
}
