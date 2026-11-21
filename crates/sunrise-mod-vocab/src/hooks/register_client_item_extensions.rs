//! Client item extensions, rebuilt with the equipment resource bindings.
pub trait Hooks {
    /// Register this load's worn equipment models, textures and item providers together.
    /// Registration is client-only and repeats on every resource load, before any reload listener
    /// applies. The validated bindings are installed with the candidate models. Models are baked once;
    /// frame callbacks choose references, never geometry. A duplicate item provider or an unknown
    /// texture refuses registration with a named diagnostic. Server-only packages cannot listen.
    /// A panicking callback loses its batch, is reported by mod name and skipped for the session.
    #[hook(
        receiver = "associated",
        dispatch = "direct",
        side = "client",
        contract = "listen",
        context = "registration",
        neoforge = "RegisterClientExtensionsEvent",
        version = 3
    )]
    fn register_client_item_extensions(extensions: &mut dyn mod_api::ClientItemExtensionsRegistrar);
}
