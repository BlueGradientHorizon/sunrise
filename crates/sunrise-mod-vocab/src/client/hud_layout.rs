//! Vocabulary for GUI poses, texture blits and the health HUD.
use crate::{ClientHost, GuiCanvas, GuiDrawError};
use std::sync::Arc;

// no wire: a pose operation is passed directly to an in-process canvas scope.
/// A `Matrix3x2fStack` operation applied in [`GuiCanvas::scope`] to a copy of the current pose.
#[derive(Clone, Copy, Debug, PartialEq)]
// exhaustive: a closed set the game matches without a wildcard
pub enum PoseOp {
    /// Move the origin along the current axes by the given GUI pixels.
    Translate(f32, f32),
    /// Scale the current axes by the given factors about the current origin.
    Scale(f32, f32),
    /// `Matrix3x2fStack.rotate(angle)`: a turn by `angle` radians about the current pose's
    /// origin, applied after the ops before it, as vanilla composes them. Positive angles turn
    /// clockwise on screen (y grows downwards). Everything drawn in the child scope follows
    /// its pose. [`GuiCanvas::scope`] refuses a non-finite angle with [`GuiDrawError::NotFinite`]
    /// before calling the drawing callback.
    Rotate(f32),
}

// no wire: the blend is passed directly to an in-process blit.
/// The render pipeline of a [`GuiCanvas::blit`]. Only `Textured` and `Crosshair` are
/// available to [`GuiCanvas::blit_sprite`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: a closed set the game matches without a wildcard
pub enum Blend {
    /// `GUI_TEXTURED`.
    Textured,
    /// `VIGNETTE`: `dst * (1 - src)`.
    Vignette,
    /// `GUI_NAUSEA_OVERLAY`: additive.
    Nausea,
    /// `CROSSHAIR`: `BlendFunction.INVERT`, the colour under it inverted.
    Crosshair,
}
impl Blend {
    /// The vanilla render pipeline path for this blend mode.
    pub fn pipeline(self) -> &'static str {
        match self {
            Blend::Textured => "pipeline/gui_textured",
            Blend::Vignette => "pipeline/vignette",
            Blend::Nausea => "pipeline/gui_nausea_overlay",
            Blend::Crosshair => "pipeline/crosshair",
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
// exhaustive: a closed set the game matches without a wildcard
#[derive(sunrise_mod_api_macros::Wire)]
pub enum HeartKind {
    Normal,
    Poisoned,
    Withered,
    Frozen,
}
// exhaustive: the game builds it with a literal; mods only read it
#[derive(sunrise_mod_api_macros::Wire)]
pub struct HealthInputs {
    /// `Hud.tickCount`.
    pub tick: i32,
    /// `Util.getMillis()` now.
    pub millis: i64,
    pub health: f32,
    pub max_health: f32,
    pub absorption: f32,
    pub armor: i32,
    pub food: i32,
    pub saturation: f32,
    pub kind: HeartKind,
    pub regeneration: bool,
    pub hunger: bool,
    pub hardcore: bool,
    /// `damageCooldownTime > 0`.
    pub damage_cooldown: bool,
    /// `getAirSupply`, `getMaxAirSupply`, `isEyeInFluid(WATER)`.
    pub air: i32,
    pub max_air: i32,
    pub under_water: bool,
    /// `getVehicleMaxHearts` of the ridden living vehicle (0 = none).
    pub vehicle_hearts: i32,
}

type HealthHudDraw = dyn Fn(&dyn ClientHost, &HealthInputs, &mut dyn GuiCanvas) -> Result<(), GuiDrawError>
    + Send
    + Sync;

// no wire: owns an in-process renderer callback.
/// An owned renderer for the `health_hud` hook. Clones share the callback, so a listener can
/// build one renderer at setup and return a clone of its handle each frame.
#[derive(Clone)]
// exhaustive: its fields are private, so mods cannot build or destructure it
pub struct HealthHudReplacement {
    draw: Arc<HealthHudDraw>,
}

impl HealthHudReplacement {
    /// Share a renderer that draws the `minecraft:player_health` HUD layer (player hearts).
    /// The selected renderer runs directly on the `minecraft:player_health` layer in the
    /// answering mod's scope while armour, food, vehicle health and air remain on their own
    /// layers. Returning an error reports the failure without rolling back prior draws or falling
    /// back to vanilla hearts; `Ok(())` replaces vanilla even if nothing was drawn.
    pub fn new(
        draw: impl Fn(&dyn ClientHost, &HealthInputs, &mut dyn GuiCanvas) -> Result<(), GuiDrawError>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        Self {
            draw: Arc::new(draw),
        }
    }

    /// Run this renderer with the current client's host, inputs and canvas. The game invokes it
    /// in the answering mod's scope on the `minecraft:player_health` layer.
    pub fn invoke(
        &self,
        host: &dyn ClientHost,
        inputs: &HealthInputs,
        canvas: &mut dyn GuiCanvas,
    ) -> Result<(), GuiDrawError> {
        (self.draw)(host, inputs, canvas)
    }
}
