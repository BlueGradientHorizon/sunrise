//! The `register_gui_layers` hooks: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// Register a HUD layer's ID, placement and owned draw callback (NeoForge's
    /// `RegisterGuiLayersEvent`). Asked once at client startup. The client calls only that
    /// layer's callback when drawing it, between `gui_layer_pre` and `gui_layer_post`.
    #[notes = r#"## HUD layers

`Hooks::register_gui_layers` is the client registration-phase counterpart of NeoForge 26.3's
`RegisterGuiLayersEvent`. It receives a `RegistrationContext` and a `GuiLayerRegistrar` as
`event.layers`, once at client startup. Call `layers.register(id, placement, Box::new(draw))`
with a namespaced ID, a `GuiLayerPlacement` and an owned draw callback. The four placements
are `BelowAll`, `Below(id)`, `Above(id)` and `AboveAll`, as `registerBelowAll`, `registerBelow`,
`registerAbove` and `registerAboveAll`. `Below` and `Above` name a vanilla layer (the IDs
`Hooks::gui_layer_pre` lists) or a layer already registered. Calls take effect in order, in mod
load order. Invalid, reserved or duplicate IDs and unknown relative anchors are logged and
refused.

Each frame the HUD shows, the client draws the layers in order. A registered layer fires
`Hooks::gui_layer_pre` with its ID, calls its own callback with the client host and drawing
context unless canceled, and fires `Hooks::gui_layer_post`. Other mods can hide it or draw
around it the same way they do for vanilla layers.

A vanilla layer Sunrise does not draw this frame (the crosshair in third person, the mount's
health off a mount) still keeps its place: the mod layers around it are drawn in order, at the
next layer that is. F1 and a canceled `Hooks::render_gui_pre` hide every mod layer; a mod that
draws under F1 uses `Hooks::gui_render_post`."#]
    #[hook(
        side = "client",
        version = 2,
        contract = "listen",
        context = "registration",
        neoforge = "RegisterGuiLayersEvent"
    )]
    fn register_gui_layers();
}
