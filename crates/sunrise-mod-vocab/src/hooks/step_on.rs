//! `step_on` declaration read by the mod API generator (see `hooks/mod.rs`).
// catalog-order: 0085

pub trait Hooks {
    /// `Block.stepOn` in `Entity.applyEffectsFromBlocks`, before inside-block effects.
    /// It runs on the block at `getOnPosLegacy` for a grounded body affected by blocks,
    /// including an item whose idle tick did not move. `state` and `pos` name that same
    /// contact; the position need not be the actual supporting cell. Each applicable
    /// effects pass gets its own callback.
    ///
    /// `stepping_carefully` is the body's `Entity.isSteppingCarefully` at this contact.
    /// `movement` is its current velocity. Set it to change the velocity before the
    /// inside effects and later drag; a non-finite result is ignored. There is no veto.
    /// Only Parts on the contacted state run, with a read-only level on the simulating
    /// side. Keep answers consistent between the two sides and on replay.
    #[hook(
        side = "both",
        version = 2,
        contract = "collect",
        out = "movement",
        scope = "block",
        key = "state",
        host = "level",
        target = "sunrise_client::game::native_hooks::bounce_restitution::FrictionView<'_>::native_step_on",
        neoforge = "none (vanilla Block.stepOn)"
    )]
    fn step_on(
        entity: i32,
        kind: &str,
        state: BlockState,
        pos: BlockPos,
        stepping_carefully: bool,
        movement: &mut [f64; 3],
    );
}
