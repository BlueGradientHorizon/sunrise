//! The `player_heart_type` hooks and helpers: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// Which hearts the local player's health bar draws this frame (NeoForge's
    /// `PlayerHeartTypeEvent`, from `Gui.HeartType.forPlayer`): `kind` comes in as vanilla's pick
    /// (poisoned, else withered, else frozen when fully frozen, else normal); set it to draw
    /// another kind. Only the original `minecraft:player_health` body asks: cancelled layers,
    /// replacements and wrappers that omit the previous body do not fire it.
    /// Only the heart sprites change: health, regeneration and the other rows stay vanilla's.
    /// Client-side.
    #[hook(
        dispatch = "flat",
        side = "client",
        contract = "listen",
        neoforge = "PlayerHeartTypeEvent",
        version = 2,
        cancellable = false
    )]
    fn player_heart_type(kind: &mut mod_api::client::hud_layout::HeartKind);
}
