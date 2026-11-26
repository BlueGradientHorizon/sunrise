

#[cfg(test)]
pub(in crate::game) mod menu_extra_data_tests {
    use sunrise_client::title::Mode;
    use sunrise_protocol::{
        connection::Pipe,
        menu_extra::CHANNEL,
        packets::{configuration::CustomPayload, play::Clientbound as C},
    };

    #[test]
    #[ignore = "daily"]
    fn malformed_menu_extra_payload_disconnects_client() {
        sunrise_gameplay::game::native_entity_refs::tests::with_client_game(|game| {
            game.side.title.0 = None;
            let (pipe, _server) = Pipe::pair();
            game.connect_local(pipe, "Malformed menu extra").unwrap();
            assert!(game.side.remote.is_some());
            game.handle_packet(C::CustomPayload(CustomPayload {
                channel: CHANNEL.into(),
                data: vec![1, 1, 0],
            }))
            .unwrap();
            assert!(game.side.remote.is_none());
            let mode = &game
                .side
                .title
                .0
                .as_ref()
                .expect("the disconnect screen")
                .mode;
            assert!(matches!(
                mode,
                Mode::Multiplayer(sunrise_client::multiplayer::Screen::Disconnected { reason, .. })
                    if reason.translation_key() == Some("disconnect.genericReason")
            ));
        });
    }
}