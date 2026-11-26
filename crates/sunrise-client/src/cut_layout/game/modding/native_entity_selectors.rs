

#[cfg(test)]
mod tests {
    use sunrise_gameplay_common::game::native_entity_selectors::SelectorToken;

    #[test]
    #[ignore = "daily"]
    fn synced_selector_tokens_survive_client_registry_refresh() {
        sunrise_gameplay::game::native_entity_refs::tests::with_client_game(|game| {
            let (client, _server) = sunrise_protocol::connection::Pipe::pair();
            game.connect_local(client, "SelectorSyncTest").unwrap();
            let tokens = vec![SelectorToken {
                name: "remote_allies".into(),
                tooltip: "test.remote_allies".into(),
                players_only: true,
                limit: 1,
                sort: "nearest",
            }];
            game.apply_packet(sunrise_protocol::packets::play::Clientbound::CustomPayload(
                sunrise_protocol::packets::configuration::CustomPayload {
                    channel: sunrise_sim::lan::commands::selector_sync::CHANNEL.into(),
                    data: sunrise_sim::lan::commands::selector_sync::encode(&tokens),
                },
            ))
            .unwrap();
            assert_eq!(game.ui.registries.selector_tokens, tokens);
            game.build_registries();
            assert_eq!(game.ui.registries.selector_tokens, tokens);
            for data in [
                vec![2],
                vec![9],
                sunrise_sim::lan::commands::selector_sync::encode(&tokens),
            ] {
                game.apply_packet(sunrise_protocol::packets::play::Clientbound::CustomPayload(
                    sunrise_protocol::packets::configuration::CustomPayload {
                        channel: sunrise_sim::lan::commands::selector_sync::CHANNEL.into(),
                        data,
                    },
                ))
                .unwrap();
                assert!(game.ui.registries.selector_tokens.is_empty());
            }
        });
    }
}