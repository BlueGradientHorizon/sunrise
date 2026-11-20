//! The `farmland_trample` hooks and helpers: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// A player or mob that fell onto farmland is about to turn it to its base block
    /// (`BlockEvent.FarmlandTrampleEvent`, from `FarmlandBlock.fallOn`). `entity` is the landing
    /// living entity: read its pose, sneak state and type through the handle. It fires after
    /// vanilla's own chance and checks (the draw, the mob-griefing rule or spawn protection,
    /// and the size test), so a listener only ever sees a trample that was going to happen.
    /// Cancel and the farmland and its crop stay as they are; the entity takes its fall
    /// damage either way.
    ///
    /// The listener has the entity's level and can read and write it. The server asks inside
    /// the landing, before the lift, block write or fall damage. The listener's writes and the
    /// apply run in one transaction. If the landing entity is gone or the block at `pos` is no
    /// longer `state`, the apply refuses without recomputing, and transactional writes roll back.
    /// Otherwise, an allowed trample first lifts every intersecting body, then writes the base
    /// block and posts the block-change game event. Fall damage follows, even when the trample
    /// was cancelled or refused. With no listener, the same lift, write and damage run in the
    /// same order. A trapping listener's cancellation is discarded and its listener for this
    /// hook is disabled for the session.
    #[hook(
        side = "server",
        contract = "cancel",
        out = "cancel",
        neoforge = "BlockEvent.FarmlandTrampleEvent",
        cancellable = true,
        isolate = true,
        version = 3
    )]
    fn farmland_trample();
}

/// Original upstream notes; the existing fork event above owns this moment.
pub trait UpstreamNotes {
    #[notes = r###"farmland trampling: `farmland_trample(host, event)` is heard when a player or mob that fell onto farmland is about to turn it to dirt, after vanilla's chance and checks (NeoForge's `FarmlandTrampleEvent`, Bukkit's physical `PlayerInteractEvent` and `EntityInteractEvent`). Cancel and the farmland and its crop stay. Otherwise bodies are lifted before the base block is written. Fall damage follows either way. No `self`. Bundles that use it need `"hooks": @API:trample@`."###]
    fn trample();
}
