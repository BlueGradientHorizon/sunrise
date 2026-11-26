//! The game's side of the `draw` area (`sunrise-mod-api/src/host/draw.rs`).
#[allow(unused_imports)]
use crate::game::api_values::{TryIntoApi, TryIntoEngine};
use mod_api::{prelude::*, *};

/// Borrow `minecraft:default` directly or format another font identifier.
pub fn font_id_str(id: &mod_api::Identifier) -> std::borrow::Cow<'static, str> {
    if id.namespace() == "minecraft" && id.path() == "default" {
        std::borrow::Cow::Borrowed(sunrise_client::font::DEFAULT)
    } else {
        std::borrow::Cow::Owned(id.to_string())
    }
}

/// Engine font view lent to [`ClientTooltipComponent`] and `render_tooltip_pre`.
pub struct EngineTooltipFont<'a> {
    pub(in crate::game) font: &'a sunrise_client::font::Font,
    pub id: &'a Identifier,
    pub(in crate::game) name: std::borrow::Cow<'static, str>,
}
/// `Font.lineHeight`: nine GUI pixels a line, before `scale`.
pub(in crate::game) const LINE_HEIGHT: f32 = 9.0;
/// The most lines one layout may have; [`DrawContext::text_lines`] refuses more
/// ([`TextLinesRefusal::TooManyLines`]) rather than answer a component of any size. The method's
/// doc names the same number: keep them in step.
pub(in crate::game) const MAX_LINES: usize = 512;
/// The most characters one layout's lines may hold together, before
/// [`TextLinesRefusal::TooManyGlyphs`]. The method's doc names the same number: keep them in step.
pub(in crate::game) const MAX_GLYPHS: usize = 8192;
/// Lays `text` out as a label's lines, as [`DrawContext::text_lines`] documents: the font's own
/// split wraps it at `max_width / scale`, the font measures each line, `align` places them inside
/// that wrapping width, and every dimension and position answered is scaled by `scale`.
#[cfg(any(test, feature = "test-support"))]
pub fn label_layout(
    font: &crate::font::Font,
    text: &sunrise_game_data::text::Text,
    max_width: f32,
    align: TextAlign,
    scale: f32,
) -> Result<TextLines, TextLinesRefusal> {
    label_layout_with(font, text, max_width, align, scale, |piece| {
        Ok(piece.try_into_api()?)
    })
}
pub fn label_layout_with(
    font: &crate::font::Font,
    text: &sunrise_game_data::text::Text,
    max_width: f32,
    align: TextAlign,
    scale: f32,
    mut to_api: impl FnMut(&sunrise_game_data::text::Text) -> Result<Text, TextLinesRefusal>,
) -> Result<TextLines, TextLinesRefusal> {
    // No position and no size to answer: a scale of zero or less, or one that is not a number,
    // refuses rather than answering every line at an undefined place.
    if !scale.is_finite() || scale <= 0.0 {
        return Err(TextLinesRefusal::ScaleInvalid);
    }
    // A width the game cannot measure in refuses too: it is not "no wrapping", which would answer
    // lines of any length in a box of no width.
    if !max_width.is_finite() {
        return Err(TextLinesRefusal::WidthNotFinite);
    }
    // Vanilla's one-pixel minimum (`StringSplitter.splitLines`): a finite box narrower than one
    // pixel wraps at one — and so does one `scale` makes narrower — rather than not wrapping.
    let wrap_width = max_width / scale;
    if !wrap_width.is_finite() {
        return Err(TextLinesRefusal::WidthNotFinite);
    }
    let wrap_width = wrap_width.max(1.0);
    let wrapped = font.split_formatted(text, wrap_width);
    if wrapped.len() > MAX_LINES {
        return Err(TextLinesRefusal::TooManyLines);
    }
    let mut lines = Vec::with_capacity(wrapped.len());
    let mut glyphs = 0usize;
    for (i, piece) in wrapped.into_iter().enumerate() {
        // The split keeps the legacy codes; applying them to each run's own style (a `§r` restores
        // the style the run carried out of the break) lets a line measure and draw the same way.
        let piece = piece.resolve_legacy();
        glyphs += piece.chars().count();
        if glyphs > MAX_GLYPHS {
            return Err(TextLinesRefusal::TooManyGlyphs);
        }
        let width = font.width_text(&piece);
        let x = match align {
            TextAlign::Left => 0.0,
            // `TextAlignment.CENTER.calculateLeft`: the half line width is rounded down.
            TextAlign::Centre => wrap_width / 2.0 - (width / 2.0).floor(),
            TextAlign::Right => wrap_width - width,
        };
        let at = [x * scale, i as f32 * LINE_HEIGHT * scale];
        lines.push(TextLine::new(to_api(&piece)?, at, width * scale));
    }
    let width = lines.iter().map(|line| line.width).fold(0.0, f32::max);
    let height = lines
        .last()
        .map_or(0.0, |line| line.at[1] + LINE_HEIGHT * scale);
    Ok(TextLines::new(lines, width, height))
}
