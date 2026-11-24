//! The `register_behaviors` hook: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// The mob's mod brain behaviors, joined into the species' brain beside
    /// vanilla's (#824): built wherever the brain is built — its first tick and
    /// every rebuild — started and stopped in `Brain.tick`'s order, fresh state
    /// each build, nothing saved. Branch on the mob with `event.behaviors.mob()`. `host` is the
    /// read-only one, so a behavior picked by a registry lookup finds the registries through it.
    #[hook(
        target = "sunrise_client::game::native_hooks::register_behaviors::Game<'_>::native_register_behaviors",
        receiver = "read",
        dispatch = "direct",
        side = "server",
        host = "read",
        contract = "listen",
        scope = "entity",
        key = "behaviors",
        version = 2,
        neoforge = "none (vanilla LivingEntity.makeBrain)"
    )]
    fn register_behaviors(behaviors: &mut BehaviorRegistrar<'_>);
}
