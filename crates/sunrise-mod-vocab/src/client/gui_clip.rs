//! What [`crate::DrawContext::with_scissor`] answers when it changes nothing.

/// Why [`crate::DrawContext::with_scissor`] refused to open a scissor scope.
/// The clip stack is as it was before the call and the drawing closure did not run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[derive(sunrise_mod_api_macros::Wire)]
pub enum GuiClipRefusal {
    /// A coordinate or size of the rectangle is NaN or infinite.
    NotFinite,
}
