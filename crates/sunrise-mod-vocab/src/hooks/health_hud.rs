//! The `health_hud` hook: declarations the mod API generator reads (see `hooks/mod.rs`).
// catalog-order: 0056

pub trait Hooks {
    /// Selects the renderer for the `minecraft:player_health` HUD layer (player hearts).
    /// `inputs` carries the health, food, air and vehicle state with the vanilla heart kind;
    /// `wi` and `hi` are the logical GUI size. Return `None` to defer, or return a
    /// [`HealthHudReplacement`](crate::HealthHudReplacement); the first answer wins. The game
    /// invokes it directly on the `minecraft:player_health` layer while armor, food, vehicle
    /// health and air stay on their own layers (`minecraft:armor_level`, `minecraft:food_level`,
    /// `minecraft:vehicle_health`, `minecraft:air_level`). Returning an error reports the failure
    /// without rolling back its draws. A panic discards that renderer's draws, names the owning
    /// mod in the log and error toast, and disables that mod's `health_hud` for the session.
    /// Neither outcome draws fallback hearts this frame. A successful empty draw replaces
    /// vanilla hearts. No replacement reserves vanilla heart height. If no listener answers,
    /// the vanilla body fires `player_heart_type` and draws the hearts.
    #[hook(
        target = "sunrise_client::game::native_hooks::base::Game<'_>::native_health_hud",
        receiver = "read",
        dispatch = "flat",
        side = "client",
        contract = "replace",
        out = "replacement",
        answered_by = "mod",
        version = 4,
        neoforge = "RenderGuiLayerEvent.Pre"
    )]
    fn health_hud(
        inputs: &mod_api::client::hud_layout::HealthInputs,
        wi: i32,
        hi: i32,
        replacement: &mut Option<Answered<HealthHudReplacement>>,
    );
}
