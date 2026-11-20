//! The `living_travel` hooks and helpers: declarations the mod API generator
//! reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// One `LivingEntity.travel` invocation (#2414 PR 2, design #2308, the
    /// maintainer ruling of the same issue): the living body `entity` of
    /// `kind` (`minecraft:player`, `minecraft:zombie`) is about to run one
    /// travel - in water, in lava, gliding, flying, or in the air, as `mode`
    /// says. It fires at the travel boundary itself - the player's
    /// `Player.travel` branch selection and the mob's `LivingEntity.travel`
    /// entry, after the input resolution and the player's swimming
    /// preparation, before the branch runs - so one travel fires it once,
    /// and the physical move the branch ends in fires `entity_move` next:
    /// a replaced travel can still meet a move veto. Change `input` (each
    /// component within [-1, 1]) or `velocity` (each component within ±64
    /// blocks) to replace what the branch starts from - the branch itself is
    /// the game's own - or set `canceled` to skip this travel invocation:
    /// its nested move does not run, the flying vertical damping is skipped,
    /// and what the tick already did (input processing, the player's swim
    /// steering) is not undone. Each listener's `input` and `velocity` are checked as
    /// that listener returns: a component that is not a finite number, or
    /// lies outside its budget, refuses that listener's pair whole (the
    /// log names the mod and the field), nothing is clamped, and the pair
    /// the preceding listeners left (the game's resolved values before any)
    /// stands for the next listener and the branch. A listener that
    /// unwinds keeps those resolved values, not a replacement an earlier
    /// listener had proposed, and is switched off for this hook for the
    /// session. It takes no Host - the maintainer ruling, like
    /// `entity_move`: the listener only decides input, velocity and cancel,
    /// and its `context` is the loop `context` of the side that ticks the
    /// body. Server-authoritative: the client does not predict a custom
    /// travel. Beyond NeoForge: there is no counterpart.
    #[hook(
        side = "server",
        contract = "cancel",
        out = "canceled",
        context = "loop",
        cancellable = true,
        isolate = true,
        output_validator = "crate::entity::types::refused_travel_output",
        neoforge = "none (fork addition; see travel-and-move.md, #2308)",
        target = "sunrise_client_lib::travel_gate::native_living_travel"
    )]
    fn living_travel();
}
