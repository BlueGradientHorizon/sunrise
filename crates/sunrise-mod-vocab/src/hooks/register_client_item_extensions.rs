//! Client item extensions, rebuilt with the equipment resource bindings.
pub trait Hooks {
    /// Register worn equipment providers on the client, like NeoForge's RegisterClientExtensionsEvent.
    /// Each resource load bakes the requested models and binds the requested atlas sprites.
    /// Draw callbacks borrow the live stack and asset layer and select from those bindings.
    /// A registration error names the refused entry without discarding other successful calls.
    /// Server-only packages cannot listen.
    #[hook(
        receiver = "associated",
        dispatch = "flat",
        side = "client",
        contract = "listen",
        context = "registration",
        neoforge = "RegisterClientExtensionsEvent",
        version = 2
    )]
    fn register_client_item_extensions(extensions: &mut dyn mod_api::ClientItemExtensionsRegistrar);
}
