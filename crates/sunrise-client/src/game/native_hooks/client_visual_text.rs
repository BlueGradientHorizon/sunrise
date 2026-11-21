#[cfg(test)]
#[path = "client_visual_text/tests.rs"]
mod tests;
use crate::game::Game;

impl Game<'_> {
    pub(in crate::game) fn text_visual_order(
        &self,
        text: &mod_api::Text,
    ) -> Vec<mod_api::VisualTextCharacter> {
        let resolved = sunrise_client_lib::api_text::to_engine_inprocess(text).resolved();
        sunrise_client::bidi::visual_characters(&resolved)
            .into_iter()
            .map(
                |(scalar, style, source_index)| mod_api::VisualTextCharacter {
                    scalar: scalar as u32,
                    style: sunrise_client_lib::api_text::style_to_api_inprocess(&style),
                    source_index,
                },
            )
            .collect()
    }
}
