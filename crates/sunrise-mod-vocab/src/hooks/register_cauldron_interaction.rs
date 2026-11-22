pub trait Hooks {
    /// Register cauldron item interactions, by exact item or by item tag, on both sides before
    /// snapshot publication. Vanilla dye tags take precedence, then exact items, then item tags
    /// in registration order. Duplicate entries from a completed listener refuse preparation.
    /// A panic or phase refusal discards that listener's entries, including queued registrations
    /// to all dispatchers, before the next listener runs.
    #[notes = "## Cauldron interactions

`register_cauldron_interaction` registers item and item-tag callbacks on both sides. See [Registering cauldron interactions](../how-to/world/cauldron-interactions.md) for the read-only decision, transactional fire, duplicate refusal and server-corrected prediction contract."]
    #[hook(
        receiver = "associated",
        dispatch = "flat",
        side = "both",
        contract = "listen",
        staged_surface = "interactions",
        version = 2,
        context = "registration",
        neoforge = "RegisterCauldronInteractionEvent"
    )]
    fn register_cauldron_interaction(interactions: &mut dyn CauldronInteractions);
}
