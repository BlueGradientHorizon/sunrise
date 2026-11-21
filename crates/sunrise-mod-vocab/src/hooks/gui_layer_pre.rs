//! The `gui_layer_pre` hook: declarations the mod API generator reads (see `hooks/mod.rs`).
// catalog-order: 0144

pub trait Hooks {
    /// Before a vanilla or mod HUD layer (`RenderGuiLayerEvent.Pre`). Draw below the
    /// layer through `event.draw`, or return `Flow::Cancel` to skip its entire body
    /// and `gui_layer_post`, including the body’s status-height and jitter updates.
    /// A wrapped or replaced layer fires this once, with the slot's original ID.
    /// To change a layer's body rather than veto the whole slot, wrap or replace it
    /// at startup in `register_gui_layers`.
    ///
    /// The vanilla anchors, in HUD order, are `minecraft:crosshair`, `hotbar`,
    /// `player_health`, `armor_level`, `food_level`, `vehicle_health`, `air_level`,
    /// `contextual_info_bar` (jump, locator or experience bar, with the level number),
    /// `selected_item_name`, `effects`, `boss_overlay`, `sleep_overlay`,
    /// `overlay_message` (action bar), `title`, `scoreboard_sidebar`,
    /// `subtitle_overlay`, `chat` and `tab_list`. Visibility conditions such as F1 apply to
    /// the original vanilla bodies, not the slots or their hooks. Focused chat still draws
    /// at the chat screen's paint site. The camera overlays and debug screen are not HUD layers.
    #[hook(
        target = "sunrise_client::game::native_hooks::base::Game<'_>::native_gui_layer_pre",
        receiver = "read",
        dispatch = "flat",
        side = "client",
        contract = "cancel",
        out = "canceled",
        neoforge = "RenderGuiLayerEvent.Pre",
        version = 2,
        cancellable = true
    )]
    fn gui_layer_pre(layer: &str, draw: &mut dyn DrawContext, canceled: &mut bool);
}
