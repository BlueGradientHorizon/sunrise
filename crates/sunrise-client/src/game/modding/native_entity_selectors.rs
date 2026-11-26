//! Mods' selector tokens (`@allies`, NeoForge's `EntitySelectorManager`): the frozen base
//! selections the command registries parse them with.
use mod_api::SelectorSort;
use sunrise_game_data::registries::SelectorToken;

/// The frozen base selections validated when the installed mods registered their tokens.
pub(in crate::game) fn selector_tokens() -> Vec<SelectorToken> {
    mod_api::__private::entity_selector_types()
        .map(|(name, base)| SelectorToken {
            name: name.to_owned(),
            tooltip: base.tooltip.clone(),
            players_only: base.players_only,
            limit: base.limit.map_or(usize::MAX, |limit| limit as usize),
            sort: match base.sort {
                SelectorSort::Nearest => "nearest",
                SelectorSort::Furthest => "furthest",
                SelectorSort::Random => "random",
                _ => "arbitrary",
            },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::SelectorToken;

    #[test]
    #[ignore = "daily"]
    fn synced_selector_tokens_survive_client_registry_refresh() {
        crate::game::native_entity_refs::tests::with_client_game(|game| {
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
