//! The `register_gui_layers` hooks: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// Register, wrap or replace a HUD layer with an owned draw callback (NeoForge's
    /// `RegisterGuiLayersEvent`). Asked once at client startup. The installed body runs
    /// between `gui_layer_pre` and `gui_layer_post` unless Pre cancels it. Use
    /// `event.layers.register(id, placement, Box::new(draw))` to insert a new slot,
    /// or `replace_layer` and `wrap_layer` to change an existing slot's body.
    /// Panicking installed callbacks lose their partial drawing and are disabled for the session,
    /// with a log and an error toast naming their owning mod.
    #[notes = r#"## HUD layers

`Hooks::register_gui_layers` is the client registration-phase counterpart of NeoForge 26.3's
`RegisterGuiLayersEvent`. It receives a `RegistrationContext` and a `GuiLayerRegistrar` as
`event.layers`, once at client startup. Call `layers.register(id, placement, Box::new(draw))`
with a namespaced ID, a `GuiLayerPlacement` and an owned draw callback. The four placements
are `BelowAll`, `Below(id)`, `Above(id)` and `AboveAll`, as `registerBelowAll`, `registerBelow`,
`registerAbove` and `registerAboveAll`. `Below` and `Above` name a vanilla layer (the IDs
`Hooks::gui_layer_pre` lists) or a layer already registered. Calls take effect in order, in mod
load order. Invalid, reserved or duplicate IDs and unknown relative anchors are logged and
refused. The client calls each installed callback as the mod that registered it.

Use `layers.replace_layer(id, Box::new(draw))` to discard an existing body, or
`layers.wrap_layer(id, Box::new(wrap))` to add an outer wrapper. Both keep the slot's ID
and position. A wrapper uses `ctx.with_draw` to paint with the current
scoped drawing context and `ctx.draw_previous()` to draw the previous body at that point.
It can omit that call to skip the previous body. Both methods refuse an unknown ID.

A layer fires `Hooks::gui_layer_pre` with its ID, runs its installed body, and fires
`Hooks::gui_layer_post` once. No body or Post runs if Pre cancels. Drawing a previous
body does not fire layer hooks again.

A vanilla body that draws nothing this frame (for example, the crosshair in third person or a
health body hidden by F1) does not skip its slot. Pre, the installed body and Post are still
visited in order unless Pre cancels. Replacements, wrappers and inserted layers can draw under F1;
only the original bodies have the vanilla visibility conditions. A canceled `Hooks::render_gui_pre`
skips the entire layer walk. The focused chat screen does not run the `minecraft:chat` slot
again. If an installed callback panics, only that callback is disabled; its partial drawing is
discarded and the log and error toast name its owning mod."#]
    #[hook(
        receiver = "associated",
        dispatch = "flat",
        side = "client",
        version = 4,
        contract = "listen",
        context = "registration",
        neoforge = "RegisterGuiLayersEvent"
    )]
    fn register_gui_layers(layers: &mut dyn GuiLayerRegistrar);
}
