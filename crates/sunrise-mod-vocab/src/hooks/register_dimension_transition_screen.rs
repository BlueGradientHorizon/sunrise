//! Client dimension-loading screen registration.

pub trait Hooks {
    /// Register loading-screen factories once at client startup (NeoForge's
    /// `RegisterDimensionTransitionScreenEvent`). Each mod receives an empty batch.
    /// Conditional pairs take priority over incoming, then outgoing. The first valid
    /// factory for a key wins. Unmatched travel, login and death use vanilla for a new screen.
    #[notes = r#"## Dimension transition screens

Push `DimensionTransitionScreen::new(id, transition, factory)` into `event.screens`.
Use `DimensionTransition::Incoming(to)`, `Outgoing(from)` or `Conditional { to, from }`
with namespaced dimension IDs. The client resolves those IDs at registration and rejects
invalid registrations and screen IDs in the `minecraft` namespace. A screen ID is for
validation and diagnostics, not draw dispatch.

On a matching trip, the client calls the selected factory once as the registering mod and
keeps its presentation until the loading screen closes. An existing loading screen (including
vanilla's) keeps its presentation if another level load starts while it is open. Login and
death respawn do not select factories when they open a new loading screen. Each draw receives
the current reason and optional smoothed integrated-server progress. The existing tracker
still governs readiness, timeout and closing."#]
    #[hook(
        side = "client",
        contract = "collect",
        out = "screens",
        context = "registration",
        answered_by = "mod",
        neoforge = "RegisterDimensionTransitionScreenEvent",
        version = 2
    )]
    fn register_dimension_transition_screen();
}
