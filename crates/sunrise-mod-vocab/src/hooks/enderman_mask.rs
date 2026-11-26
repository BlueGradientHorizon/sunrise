//! The `enderman_mask` hooks and helpers: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// Whether what `player` wears on their head hides their face from
    /// `enderman` (the worn-item test that opens `Enderman.isBeingStaredBy`).
    /// `mask` comes in as vanilla's answer: true when the head item is in
    /// `#minecraft:gaze_disguise_equipment`, as a carved pumpkin is. Set it
    /// true and the stare doesn't count; set it false and the stare is judged
    /// by the look's cone and line of sight, then `enderman_anger`, even under
    /// a carved pumpkin. Read the worn stack with `host.living(player.entity())`
    /// and `with_equipment`. Asked each time the enderman's goals check a
    /// stare, before the look is measured. Server-side.
    #[hook(
        receiver = "read",
        dispatch = "flat",
        side = "server",
        contract = "listen",
        host = "read"
    )]
    fn enderman_mask(enderman: EntityRef, player: PlayerRef, mask: &mut bool);
}
