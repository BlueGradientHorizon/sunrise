use super::*;
/// The ticket controllers registered at startup.
pub(crate) fn installed_controllers() -> &'static [ControllerInfo] {
    installed().map_or(&[], |installed| installed.controllers.as_slice())
}

/// Registrar operations for tickets.
pub trait TicketsRegistrar {
    /// Register ticket controller `name`, the way NeoForge's `RegisterTicketControllersEvent`
    /// does once at startup, and keep the handle for `chunk.set_mod_forced`. `name` is
    /// `quarry` or `mymod:quarry`; the controller's id is in a namespace owned by the calling mod's manifest
    /// (`mymod:quarry`), and an unowned namespace refuses the mods at startup, as does an id
    /// registered twice. Pass `Some(Arc::new(...))` to bind a
    /// [`LoadingValidationCallback`] to this controller, or `None` to restore its saved
    /// tickets without validation. A world's tickets of a controller no loaded mod
    /// registers are dropped when it loads.
    fn register_ticket_controller(
        &mut self,
        name: &str,
        validator: Option<Arc<LoadingValidationCallback>>,
    ) -> TicketController;
}
