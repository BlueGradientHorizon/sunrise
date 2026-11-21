//! Refusals from [`crate::DrawContext::with_scissor`].

/// Why [`crate::DrawContext::with_scissor`] did not invoke its callback.
/// The enclosing clip is unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[derive(sunrise_mod_api_macros::Wire)]
pub enum GuiClipRefusal {
    /// A coordinate, size or computed endpoint of the rectangle is NaN or infinite.
    NotFinite,
    /// 64 clips are already open in this hook call.
    TooDeep,
}
