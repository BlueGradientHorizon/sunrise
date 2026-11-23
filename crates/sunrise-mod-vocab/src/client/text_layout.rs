//! In-process label layout shared by off-draw reads and canvas painting.
use crate::Text;

/// Where lines sit inside their wrapping width.
// no wire: local label layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: a closed set the game matches without a wildcard
pub enum TextAlign {
    /// Lined up on the left edge.
    Left,
    /// Centred: half the wrapping width minus the floored half of the line's width.
    Centre,
    /// Lined up on the right edge.
    Right,
}

/// How to wrap and place a label. Width is in GUI pixels after scaling.
#[derive(Clone, Copy, Debug, PartialEq)]
// no wire: local label layout.
// exhaustive: callers supply the three layout inputs.
pub struct TextLayoutSpec {
    /// Wrapping width. Widths below one unscaled pixel wrap at one pixel, as in vanilla.
    pub max_width: f32,
    /// Where to place each line inside the wrapping width.
    pub align: TextAlign,
    /// Positive scale of the text relative to the active font's unscaled GUI pixels.
    pub scale: f32,
}

/// A line lent to a visitor in top-to-bottom order.
// no wire: borrowed component.
#[derive(Clone, Copy, Debug)]
// exhaustive: the layout visitor supplies these three line properties.
pub struct TextLine<'a> {
    /// Logical-order component with effective styles and inline objects. Legacy formatting
    /// codes have been applied and dropped. Clone this component to retain it.
    pub text: &'a Text,
    /// Offset from the label origin, in scaled GUI pixels.
    pub at: [f32; 2],
    /// Line width in scaled GUI pixels.
    pub width: f32,
}

/// The dimensions of the label's laid-out lines, in scaled GUI pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
// no wire: local label layout.
// exhaustive: two dimensions of the label.
pub struct TextLayoutSize {
    /// The widest line, or zero with no lines.
    pub width: f32,
    /// The last line's vertical offset plus one line height, or zero with no lines.
    pub height: f32,
}

/// A wrapping width, scale or scaled result that cannot place a label. No lines were visited.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// no wire: local label layout.
#[non_exhaustive]
pub enum TextLayoutError {
    /// The wrapping width or `max_width / scale` is not finite.
    WidthNotFinite,
    /// The scale is not finite or is not positive.
    ScaleInvalid,
    /// A scaled line offset, width or total height is not finite.
    GeometryNotFinite,
}

impl std::fmt::Display for TextLayoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::WidthNotFinite => "text layout: max_width or max_width / scale is not finite",
            Self::ScaleInvalid => "text layout: scale is not finite or is not positive",
            Self::GeometryNotFinite => "text layout: scaled line geometry is not finite",
        })
    }
}

impl std::error::Error for TextLayoutError {}
