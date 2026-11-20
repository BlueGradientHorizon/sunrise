//! The `component_types` hook: declarations the mod API generator reads (see `hooks/mod.rs`).
// catalog-order: 0074

pub trait Hooks {
    /// Register namespaced NBT component types before content loads.
    #[hook(
        side = "both",
        contract = "collect",
        out = "types",
        registration_append = "register",
        context = "registration",
        neoforge = "RegisterEvent",
        version = 2
    )]
    fn component_types();
}
