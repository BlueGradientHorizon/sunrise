//! Position wrapped visual lines once for both host reads and GUI paint.
use mod_api::{TextAlign, TextLayoutError, TextLayoutSize, TextLayoutSpec, TextLine};
use sunrise_client::{bidi, font::Font, text::Text};

/// `Font.lineHeight`, in unscaled GUI pixels.
const LINE_HEIGHT: f32 = 9.0;

pub(in crate::game) struct LabelLayout {
    wrap_width: f32,
    align: TextAlign,
    scale: f32,
}

impl TryFrom<TextLayoutSpec> for LabelLayout {
    type Error = TextLayoutError;

    fn try_from(spec: TextLayoutSpec) -> Result<Self, Self::Error> {
        if !spec.scale.is_finite() || spec.scale <= 0.0 {
            return Err(TextLayoutError::ScaleInvalid);
        }
        let width = spec.max_width / spec.scale;
        if !width.is_finite() {
            return Err(TextLayoutError::WidthNotFinite);
        }
        Ok(Self {
            // StringSplitter.splitLines wraps at a minimum width of one pixel.
            wrap_width: width.max(1.0),
            align: spec.align,
            scale: spec.scale,
        })
    }
}

impl LabelLayout {
    pub(in crate::game) fn visit_components(
        &self,
        font: &Font,
        text: &mod_api::Text,
        visit: &mut dyn FnMut(TextLine<'_>),
    ) -> Result<TextLayoutSize, TextLayoutError> {
        let local = sunrise_client_lib::api_text::to_local_engine(text);
        self.visit(font, local.engine(), |logical, _, [x, y], width| {
            visit(TextLine {
                text: &local.text_to_api(logical),
                at: [x * self.scale, y * self.scale],
                width: width * self.scale,
            });
        })
    }

    /// Visit the logical text and its corresponding visual line, with unscaled positions and
    /// advance. In-memory consumers need not serialize a component to measure or paint it.
    pub(in crate::game) fn visit(
        &self,
        font: &Font,
        text: &Text,
        mut visit: impl FnMut(&Text, &Text, [f32; 2], f32),
    ) -> Result<TextLayoutSize, TextLayoutError> {
        let lines = font.split_formatted(text, self.wrap_width);
        let mut placed = Vec::with_capacity(lines.len());
        let mut size = TextLayoutSize::default();
        for (i, line) in lines.iter().enumerate() {
            let line = line.resolved();
            let width = font.width_visual(&bidi::visual(&line));
            let x = match self.align {
                TextAlign::Left => 0.0,
                // TextAlignment.CENTER.calculateLeft floors half of the line's width.
                TextAlign::Centre => self.wrap_width / 2.0 - (width / 2.0).floor(),
                TextAlign::Right => self.wrap_width - width,
            };
            let y = i as f32 * LINE_HEIGHT;
            let [scaled_x, scaled_y, scaled_width] = [x, y, width].map(|v| v * self.scale);
            let height = scaled_y + LINE_HEIGHT * self.scale;
            if ![scaled_x, scaled_y, scaled_width, height]
                .into_iter()
                .all(f32::is_finite)
            {
                return Err(TextLayoutError::GeometryNotFinite);
            }
            size.width = size.width.max(scaled_width);
            size.height = height;
            placed.push((line, [x, y], width));
        }
        for (line, at, width) in placed {
            visit(&line, &bidi::visual(&line), at, width);
        }
        Ok(size)
    }
}
