//! The `register_goals` hook: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// The mob's mod goals, added to its two selectors as `GoalSelector.addGoal`
    /// does in vanilla's `registerGoals`: fresh at every construction, so a goal's
    /// state is never saved. Branch on the mob with `event.goals.mob()`. `host` is the read-only
    /// one: a goal that picks by a registry lookup finds the registries through it. The game asks
    /// where it constructs the mob, so each mob's goals exist from the moment it is built.
    #[hook(
        target = "sunrise_client::game::native_hooks::register_goals::Game<'_>::native_register_goals",
        receiver = "read",
        dispatch = "direct",
        side = "server",
        host = "read",
        contract = "listen",
        scope = "entity",
        key = "goals",
        version = 2,
        neoforge = "none (vanilla Mob.registerGoals)"
    )]
    fn register_goals(goals: &mut GoalRegistrar<'_>);
}
