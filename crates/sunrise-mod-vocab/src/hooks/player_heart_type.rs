//! The `player_heart_type` hooks and helpers: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// Which hearts the local player's health bar draws this frame (NeoForge's
    /// `PlayerHeartTypeEvent`, from `Hud.HeartType.forPlayer`): `kind` comes in as vanilla's pick
    /// (poisoned, else withered, else frozen when fully frozen, else normal); set it to draw
    /// another kind. Only the vanilla `minecraft:player_health` body asks for this pick:
    /// a canceled or replaced layer does not fire this hook, and a wrapper fires it only
    /// by drawing the vanilla body. Only the heart sprites change, not the layout or other rows.
    /// Client-side.
    #[hook(
        dispatch = "flat",
        side = "client",
        contract = "listen",
        neoforge = "PlayerHeartTypeEvent",
        cancellable = false
    )]
    fn player_heart_type(kind: &mut mod_api::client::hud_layout::HeartKind);
}
