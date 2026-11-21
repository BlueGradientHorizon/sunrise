//! Lexically scoped GUI drawing.

use crate::{ItemStack, StackWriteError, Text, text::TextFormError};

use super::hud_layout::{Blend, PoseOp};

/// A partial `blitSprite`, in the sprite's logical pixel units.
// no wire: arguments to an in-process GUI draw
// exhaustive: plain pixel extents and offset a mod constructs by struct literal
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GuiSpriteRegion {
    /// Positive width and height of the whole sprite.
    pub size: [i32; 2],
    /// Source `u, v` offset; the destination width and height also give the source extents.
    pub offset: [i32; 2],
}

/// A source rectangle for `blit`, in the texture's logical pixel units.
// no wire: arguments to an in-process GUI draw
// exhaustive: plain texture source rectangle a mod constructs by struct literal
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GuiBlitRegion {
    /// Source `u, v` offset, which must be finite.
    pub uv: [f32; 2],
    /// Source width and height in texture pixels; zero or negative values mirror or repeat the
    /// sampled edge, matching vanilla `GuiGraphicsExtractor.blit`.
    pub src: [i32; 2],
    /// Positive width and height of the whole texture.
    pub tex: [i32; 2],
}

/// A refusal from a GUI draw call or its callback. A refused primitive adds no output; draws
/// already emitted before the error stay in the batch.
// no wire: errors stay with the in-process drawing callback
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum GuiDrawError {
    /// This context cannot lend a GUI canvas.
    Unavailable,
    /// A coordinate, pose, or result of draw arithmetic was not finite.
    NotFinite,
    /// Text could not cross to the game (for example, its component tree is too deep).
    Text(TextFormError),
    /// The stack could not be read for rendering. Storage count and damage limits do not apply.
    Stack(StackWriteError),
    /// No sprite is present at the requested GUI or packed-atlas id.
    UnknownSprite,
    /// `Vignette` and `Nausea` are only available for raw texture blits.
    UnsupportedSpriteBlend,
    /// The whole sprite size in a partial sprite blit was not positive on both axes.
    BadSpriteRegion,
    /// No raw texture is present at the requested id.
    UnknownTexture,
    /// The whole texture size was not positive on both axes.
    BadBlitRegion,
}

impl std::fmt::Display for GuiDrawError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Unavailable => "GUI drawing is unavailable in this context",
            Self::NotFinite => "GUI draw geometry is not finite",
            Self::Text(error) => return write!(f, "GUI draw text: {error}"),
            Self::Stack(error) => return write!(f, "GUI draw stack: {error:?}"),
            Self::UnknownSprite => "unknown GUI draw sprite",
            Self::UnsupportedSpriteBlend => "this blend is not supported for GUI sprites",
            Self::BadSpriteRegion => "GUI sprite size must be positive",
            Self::UnknownTexture => "unknown GUI draw texture",
            Self::BadBlitRegion => "GUI blit texture size must be positive",
        })
    }
}

impl std::error::Error for GuiDrawError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Text(error) => Some(error),
            _ => None,
        }
    }
}

/// Checked GUI primitives for [`crate::DrawContext::draw_gui`], a child [`Self::scope`],
/// or a draw callback such as an item decorator. Coordinates are local GUI pixels and colours
/// are straight-alpha ARGB (`0xAARRGGBB`). Each primitive validates its inputs before emitting
/// vertices, so a refused draw adds no output while prior draws stay in the batch.
/// Widgets, scenes, focus and narration belong to a full [`crate::DrawContext`].
pub trait GuiCanvas {
    /// The full GUI's width and height in GUI pixels, unaffected by the pose or scissor.
    fn size(&self) -> [f32; 2];

    /// Draw in a child scope. Apply `ops` in order to a copy of the current pose, then transform
    /// the local scissor (`[x, y, w, h]`) into screen space and intersect it with the enclosing
    /// scissor. The two ordered corners are transformed, flooring the resulting origin and
    /// extents separately. `None` inherits the enclosing scissor unchanged. Only the transformed
    /// intersection determines emptiness: reflection can make a negative local extent positive.
    /// The clip does not move when a descendant changes its
    /// pose. Non-finite geometry refuses the scope before calling `draw`; zero scale is allowed.
    /// Pose and scissor are lexically scoped and always restored on return, `Err`, or unwind,
    /// while draws already emitted stay in the batch.
    fn scope(
        &mut self,
        ops: &[PoseOp],
        scissor: Option<[f32; 4]>,
        draw: &mut dyn FnMut(&mut dyn GuiCanvas) -> Result<(), GuiDrawError>,
    ) -> Result<(), GuiDrawError>;

    /// Fill `[x, y, w, h]` in the current pose and scissor, sorting the corners on both axes.
    fn fill(&mut self, rect: [f32; 4], argb: u32) -> Result<(), GuiDrawError>;

    /// Fill `[x, y, w, h]` with `top` at `y` and `bottom` at `y + h`. Zero area or inversion
    /// on exactly one axis draws nothing; inverting both axes keeps the colours on their input
    /// edges.
    fn fill_gradient(&mut self, rect: [f32; 4], top: u32, bottom: u32) -> Result<(), GuiDrawError>;

    /// Draw styled text, including legacy formatting, with `argb` as the default colour.
    /// Zero alpha skips the text entirely, before shaping or drawing.
    fn text(
        &mut self,
        text: &Text,
        at: [f32; 2],
        argb: u32,
        shadow: bool,
    ) -> Result<(), GuiDrawError>;

    /// Draw the stack's 16-pixel GUI item model with its top-left at `at`, passing `seed` to
    /// its model resolver. This does not draw decorations. An empty stack draws nothing;
    /// inventory storage limits do not apply. Non-finite posed model geometry is refused with
    /// `GuiDrawError::NotFinite`, leaving earlier draws intact.
    fn item(&mut self, stack: &ItemStack, at: [f32; 2], seed: i32) -> Result<(), GuiDrawError>;

    /// Draw the stack's bar, cooldown, count and custom item decorations at `at`, in that order.
    /// `count` replaces the normal count label; `Some("")` hides it. An empty stack draws nothing.
    /// The posed count and its shadow must be finite; a refused label does not undo the bar or shade.
    /// Call this in the surrounding scope after drawing a squeezed model in a child
    /// [`Self::scope`], so the decorations follow the surrounding pose, not the squeeze.
    fn item_decorations(
        &mut self,
        stack: &ItemStack,
        at: [f32; 2],
        count: Option<&str>,
    ) -> Result<(), GuiDrawError>;

    /// Draw a present GUI sprite by id at `dst = [x, y, w, h]`, honouring its metadata scaling
    /// (stretch, tile or nine-slice) and optional source region. A removed sprite is refused. Accepts
    /// `Blend::Textured` and `Blend::Crosshair`. Pack sprite modifiers are applied as in the HUD;
    /// non-finite resulting geometry is refused without affecting earlier draws. Tiled regions
    /// repeat from their original origin. Each render-target pixel samples its own tile at
    /// the pixel center; a sample just before a seam stays on the ending side.
    fn blit_sprite(
        &mut self,
        name: &str,
        dst: [f32; 4],
        region: Option<GuiSpriteRegion>,
        argb: u32,
        blend: Blend,
    ) -> Result<(), GuiDrawError>;

    /// Draw a sprite by its exact packed atlas name (not its GUI id) at `[x, y, w, h]`.
    fn atlas_sprite(&mut self, name: &str, dst: [f32; 4], argb: u32) -> Result<(), GuiDrawError>;

    /// Draw a region of a raw texture (path under `textures/`, without `.png`, or a live or
    /// registered texture id) at `[x, y, w, h]`. Accepts every [`Blend`].
    fn blit(
        &mut self,
        texture: &str,
        dst: [f32; 4],
        region: GuiBlitRegion,
        argb: u32,
        blend: Blend,
    ) -> Result<(), GuiDrawError>;
}
