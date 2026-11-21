//! The `health_hud` hook: declarations the mod API generator reads (see `hooks/mod.rs`).
// catalog-order: 0056

pub trait Hooks {
    /// Selects the renderer for the `minecraft:player_health` HUD layer (player hearts).
    /// `inputs` carries the health, food, air and vehicle state after `player_heart_type`;
    /// `wi` and `hi` are the logical GUI size. Return `None` to defer, or return a
    /// [`HealthHudReplacement`](crate::HealthHudReplacement); the first answer wins. The game
    /// invokes it directly on the `minecraft:player_health` layer while armor, food, vehicle
    /// health and air stay on their own layers (`minecraft:armor_level`, `minecraft:food_level`,
    /// `minecraft:vehicle_health`, `minecraft:air_level`). Returning an error reports the failure
    /// without rolling back prior draws or falling back to vanilla hearts. A successful empty
    /// draw replaces vanilla hearts. Nothing answered draws Sunrise's own.
    #[hook(
        target = "sunrise_client::game::native_hooks::base::Game<'_>::native_health_hud",
        receiver = "read",
        dispatch = "flat",
        side = "client",
        contract = "replace",
        out = "replacement",
        answered_by = "mod",
        version = 2,
        neoforge = "RenderGuiLayerEvent.Pre"
    )]
    fn health_hud(
        inputs: &mod_api::client::hud_layout::HealthInputs,
        wi: i32,
        hi: i32,
        replacement: &mut Option<Answered<HealthHudReplacement>>,
    );
}
