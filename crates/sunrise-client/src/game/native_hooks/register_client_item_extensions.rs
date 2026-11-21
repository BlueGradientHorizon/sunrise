use super::*;
impl Game<'_> {
    /// Registers this resource load's item-owned client equipment extensions.
    pub(in crate::game) fn native_register_client_item_extensions(
        context: &mod_api::RegistrationContext,
        with_registrar: impl FnMut(
            &mut dyn FnMut(&mut dyn mod_api::ClientItemExtensionsRegistrar) -> bool,
        ),
    ) {
        mod_api::dispatch_register_client_item_extensions(context, with_registrar);
    }
}
