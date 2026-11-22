#[cfg(test)]
#[path = "client_visual_text/tests.rs"]
mod tests;
use crate::game::Game;

use crate::game::host::draw::text_layout::LabelLayout;
use mod_api::{
    Text, TextLayoutError, TextLayoutSize, TextLayoutSpec, TextLine, VisualTextCharacterRef,
};
use sunrise_client_lib::api_text::to_local_engine;

impl Game<'_> {
    pub(in crate::game) fn visit_text_visual_order(
        &self,
        text: &Text,
        visit: &mut dyn FnMut(VisualTextCharacterRef<'_>) -> bool,
    ) -> bool {
        let local = to_local_engine(text);
        let resolved = local.engine().resolved();
        let visual = sunrise_client::bidi::visual_characters(&resolved, |a, b| {
            local.visual_styles_match(a, b)
        });
        let mut style = mod_api::text::TextStyle::default();
        let mut previous = None;
        for (scalar, effective, source_index) in visual {
            // Equal effective styles may still carry distinct opaque metadata. Reuse only
            // repeats of the exact same engine style.
            if !previous.is_some_and(|old| std::ptr::eq(old, effective)) {
                style = local.style_to_api(effective);
                previous = Some(effective);
            }
            if !visit(VisualTextCharacterRef {
                scalar: scalar as u32,
                style: &style,
                source_index,
            }) {
                return false;
            }
        }
        true
    }

    pub(in crate::game) fn visit_text_lines(
        &self,
        text: &Text,
        spec: TextLayoutSpec,
        visit: &mut dyn FnMut(TextLine<'_>),
    ) -> Result<TextLayoutSize, TextLayoutError> {
        Ok(LabelLayout::try_from(spec)?.visit_components(&self.gui.atlas.font, text, visit))
    }
}
