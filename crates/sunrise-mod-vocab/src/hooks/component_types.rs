//! The `component_types` hook: declarations the mod API generator reads (see `hooks/mod.rs`).
// catalog-order: 0074

pub trait Hooks {
    /// Register namespaced NBT component types first during startup registration, before content
    /// files load and before [`Hooks::register`](crate::Hooks::register). Call
    /// [`ComponentTypesEvent::register`] with each owned ID, such as `"mymod:charge".into()`.
    /// Listeners must not rely on state initialized in `Hooks::register`, and
    /// [`RegistrationContext::has_optional_dependency`](crate::RegistrationContext::has_optional_dependency)
    /// is always `false` here because content admission has not run yet. IDs are checked together
    /// at startup: invalid IDs, the `minecraft` namespace and duplicate IDs are refused before
    /// the component registry is frozen.
    #[hook(
        target = "sunrise_client::game::native_hooks::base::Game<'_>::native_component_types",
        receiver = "associated",
        dispatch = "flat",
        side = "both",
        contract = "collect",
        out = "types",
        registration_append = "register",
        context = "registration",
        neoforge = "RegisterEvent",
        version = 3
    )]
    fn component_types(types: &mut Vec<String>);
}
