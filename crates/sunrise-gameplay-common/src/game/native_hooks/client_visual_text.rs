
use crate::game::CommonGame;

impl CommonGame<'_> {
    pub fn text_visual_order(
        &self,
        text: &mod_api::Text,
    ) -> Vec<mod_api::VisualTextCharacter> {
        let local = sunrise_sim::api_text::to_local_engine(text);
        let resolved = local.engine().resolved();
        sunrise_level::bidi::visual_characters(&resolved, |a, b| local.visual_styles_match(a, b))
            .into_iter()
            .map(
                |(scalar, style, source_index)| mod_api::VisualTextCharacter {
                    scalar: scalar as u32,
                    style: local.style_to_api(style),
                    source_index,
                },
            )
            .collect()
    }
}
