use super::*;

impl Game<'_> {
    /// Registers client tooltip component factories for custom `TooltipComponentData` types
    /// (NeoForge's `RegisterClientTooltipComponentFactoriesEvent`). Asked once at client startup.
    pub(in crate::game) fn native_register_client_tooltip_component_factories(
        context: &mod_api::RegistrationContext,
        factories: &mut dyn mod_api::ClientTooltipComponentFactoriesRegistrar,
    ) {
        Self::fire_register_client_tooltip_component_factories(context, factories);
    }
}
