//! The `farmland_trample` hooks and helpers: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// A player or mob that fell onto farmland is about to turn it to its base block
    /// (`BlockEvent.FarmlandTrampleEvent`, from `FarmBlock.fallOn`). `entity` is the landing
    /// living entity: read its pose, sneak state and type through the handle. It fires after
    /// vanilla's own chance and checks (the draw, the mob-griefing rule or spawn protection,
    /// and the size test), so a listener only ever sees a trample that was going to happen.
    /// Cancel and the farmland and its crop stay as they are; the entity takes its fall
    /// damage either way.
    ///
    /// The listener has the entity's level and can read and write it. The game asks it
    /// with the server's own host once the landing has returned (the entity's fall damage
    /// is already dealt, its block write is still to come) and applies the answer after
    /// the listener's writes in one transaction: farmland becomes its base block only if the
    /// block at `pos` is still the `state` the landing decided on. If the entity has gone, or
    /// a write changed that block, the apply refuses (never recomputes) and the transactional
    /// writes are rolled back. With no listener nothing is kept and the landing writes the
    /// block itself, as before.
    #[hook(
        side = "server",
        contract = "cancel",
        out = "cancel",
        neoforge = "BlockEvent.FarmlandTrampleEvent",
        cancellable = true,
        version = 3
    )]
    fn farmland_trample();
}

/// Original upstream notes; the existing fork event above owns this moment.
pub trait UpstreamNotes {
    #[notes = r###"farmland trampling: `farmland_trample(host, event)` is heard when a player or mob that fell onto farmland is about to turn it to dirt, after vanilla's chance and checks (NeoForge's `FarmlandTrampleEvent`, Bukkit's physical `PlayerInteractEvent` and `EntityInteractEvent`). Cancel and the farmland and its crop stay. No `self`. Bundles that use it need `"hooks": @API:trample@`."###]
    fn trample();
}
