//! The `component_types` hook: declarations the mod API generator reads (see `hooks/mod.rs`).
// catalog-order: 0074

pub trait Hooks {
    /// Register namespaced NBT component types before content loads.
    #[hook(
        target = "sunrise_client::game::native_hooks::base::Game<'_>::native_component_types",
        receiver = "associated",
        dispatch = "flat",
        side = "both",
        contract = "collect",
        out = "types",
        context = "registration",
        neoforge = "RegisterEvent"
    )]
    fn component_types(types: &mut Vec<String>);
}
