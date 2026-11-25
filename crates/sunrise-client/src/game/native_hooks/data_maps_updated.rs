use super::*;

impl<__Side: crate::game::Side> Game<'_, __Side> {
    /// The data maps of `registry` (`minecraft:item`) changed on this side
    /// (NeoForge's `DataMapsUpdatedEvent`), after they are readable.
    pub(in crate::game) fn native_data_maps_updated(
        &mut self,
        registry: &str,
        cause: mod_api::DataMapsUpdateCause,
    ) {
        crate::game::native_hooks::fire::fire_data_maps_updated(
            crate::game::Side::as_reload_context(self),
            registry,
            cause,
        );
    }
}
