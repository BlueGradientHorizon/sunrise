#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::menu_extra_data::*;

impl Game<'_, crate::game::client_side::ClientState> {
    /// The opener position for the currently shown mod menu, if known.
    pub(in crate::game) fn screen_pos(&self) -> Option<[i32; 3]> {
        self.shown_native_screen()?.menu.pos
    }
    /// Applies the `sunrise:menu_extra` payload sent before `OpenScreen`, or disconnects on error.
    pub(in crate::game) fn receive_menu_extra(&mut self, data: &[u8]) -> crate::game::Result<()> {
        match sunrise_protocol::menu_extra::decode(data) {
            Ok(extra) => {
                if let Some(r) = self.remote.as_mut() {
                    r.menu_extra = Some(extra);
                }
                Ok(())
            }
            Err(e) => {
                log::error!("Sunrise: malformed menu extra payload: {e}");
                let reason =
                    Text::translate("disconnect.genericReason", &[Text::literal(e.to_string())]);
                self.leave_server(Some(reason))
            }
        }
    }
}

#[cfg(test)]
pub(in crate::game) mod menu_extra_data_tests {
    #[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::title::Mode;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_protocol::{
        connection::Pipe,
        menu_extra::CHANNEL,
        packets::{configuration::CustomPayload, play::Clientbound as C},
    };
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::menu_extra_data::menu_extra_data_tests::*;

    #[test]
    #[ignore = "daily"]
    fn malformed_menu_extra_payload_disconnects_client() {
        crate::game::native_entity_refs::tests::with_client_game(|game| {
            game.side.title.0 = None;
            let (pipe, _server) = Pipe::pair();
            game.connect_local(pipe, "Malformed menu extra").unwrap();
            assert!(game.remote.is_some());
            game.handle_packet(C::CustomPayload(CustomPayload {
                channel: CHANNEL.into(),
                data: vec![1, 1, 0],
            }))
            .unwrap();
            assert!(game.remote.is_none());
            let mode = &game.side.title.0.as_ref().expect("the disconnect screen").mode;
            assert!(matches!(
                mode,
                Mode::Multiplayer(sunrise_client::multiplayer::Screen::Disconnected { reason, .. })
                    if reason.translation_key() == Some("disconnect.genericReason")
            ));
        });
    }
}
