#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{Arg, Context, Game, R, Target, err};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::target_entity_mut;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::text_commands::compile_selector;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::text_commands::invalid_selector;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::text_commands::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    hud_overlays::Titles, lan::data::component_nbt, text::Text,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_protocol::packets::play::{self, Clientbound as C};

impl Game<'_> {
    #[inline(always)]
    pub(in crate::game) fn titles_or_packet_client(&mut self, local: impl FnOnce(&mut Titles)) {
        local(&mut self.client.titles);
    }
}
