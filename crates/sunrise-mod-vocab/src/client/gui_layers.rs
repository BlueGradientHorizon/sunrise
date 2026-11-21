//! HUD layers a native mod adds among vanilla's (NeoForge's `RegisterGuiLayersEvent`).

use crate::{ClientHost, DrawContext};

// no wire: the callable and its captures are kept in the client process.
/// Draws one registered HUD layer with the current client and drawing context.
/// The closure is owned by the client for its lifetime; put per-layer data in its captures.
pub type GuiLayerDraw = dyn Fn(&dyn ClientHost, &mut dyn DrawContext) + Send + Sync + 'static;

/// The HUD layer registrar handed to `register_gui_layers` once at client startup.
/// Registrations are applied in call order; the registrar is only available during that hook.
pub trait GuiLayerRegistrar {
    /// Install `draw` at `placement` under a namespaced ID such as `mymod:compass`.
    /// Returns false and logs a warning if the ID is malformed, in the `minecraft` namespace,
    /// already registered, or relative to a layer that is not registered yet. The first
    /// successful registration of an ID stays; a rejected registration does not reserve it.
    fn register(
        &mut self,
        id: String,
        placement: GuiLayerPlacement,
        draw: Box<GuiLayerDraw>,
    ) -> bool;
}

/// Where a mod's HUD layer goes, the four orderings of `RegisterGuiLayersEvent`.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: NeoForge's four orderings, matched without a wildcard
#[derive(sunrise_mod_api_macros::Wire)]
pub enum GuiLayerPlacement {
    /// Below every other layer, the crosshair included (`registerBelowAll`).
    BelowAll,
    /// Right below the layer with this ID (`registerBelow`): a vanilla layer, like
    /// `minecraft:hotbar`, or one registered before this one.
    Below(String),
    /// Right above the layer with this ID (`registerAbove`), with the same rule.
    Above(String),
    /// Above every other layer, the tab list included (`registerAboveAll`).
    AboveAll,
}
