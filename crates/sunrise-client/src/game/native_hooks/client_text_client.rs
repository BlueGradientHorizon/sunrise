use super::*;

impl Game<'_, crate::game::client_side::ClientState> {
    pub(in crate::game) fn text_width(&self, text: &mod_api::Text) -> i32 {
        let engine = sunrise_client_lib::api_text::to_engine_inprocess(text);
        self.side.gui.atlas.font.width_text(&engine) as i32
    }

    pub(in crate::game) fn text_split(
        &self,
        text: &mod_api::Text,
        width: i32,
    ) -> Vec<mod_api::Text> {
        let local = sunrise_client_lib::api_text::to_local_engine(text);
        self.side
            .gui
            .atlas
            .font
            .split_formatted(&local.engine().resolved(), width as f32)
            .iter()
            .map(|line| local.text_to_api(line))
            .collect()
    }
}
