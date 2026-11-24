#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::menu_extra_data::*;

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
            game.title.0 = None;
            let (pipe, _server) = Pipe::pair();
            game.connect_local(pipe, "Malformed menu extra").unwrap();
            assert!(game.remote.is_some());
            game.handle_packet(C::CustomPayload(CustomPayload {
                channel: CHANNEL.into(),
                data: vec![1, 1, 0],
            }))
            .unwrap();
            assert!(game.remote.is_none());
            let mode = &game.title.0.as_ref().expect("the disconnect screen").mode;
            assert!(matches!(
                mode,
                Mode::Multiplayer(sunrise_client::multiplayer::Screen::Disconnected { reason, .. })
                    if reason.translation_key() == Some("disconnect.genericReason")
            ));
        });
    }
}
