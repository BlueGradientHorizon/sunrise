

#![cfg(test)]
#[cfg(test)]
mod tests {
    use super::super::native_guest_turn_tests::with_guests;

    /// The tick times are the clock's 100; the distances are the hub's, set live on a dedicated
    /// server only, within 2 to 32.
    #[test]
    #[ignore = "daily"]
    fn a_mod_reads_and_sets_the_servers_distances() {
        with_guests(|game| {
            game.clock = sunrise_sim::clock::FixedStep::default();
            for took in [40_000_000, 51_000_000, 62_000_000] {
                game.clock.record(std::time::Duration::from_nanos(took));
            }
            let host: &dyn mod_api::ServerHost = game;
            let times = host.tick_times();
            assert_eq!(times.len(), 100);
            assert_eq!(times[..4], [40_000_000, 51_000_000, 62_000_000, 0]);
            assert_eq!(game.server_view_distance(), Some(2));
            assert_eq!(game.server_simulation_distance(), Some(2));
            assert!(!game.server_set_view_distance(6), "an integrated server");
            game.options.dedicated = Some(Vec::new());
            assert!(game.server_set_view_distance(6));
            assert!(game.server_set_simulation_distance(5));
            assert!(!game.server_set_view_distance(1));
            assert!(!game.server_set_simulation_distance(33));
            assert_eq!(game.server_view_distance(), Some(6));
            assert_eq!(game.server_simulation_distance(), Some(5));
            assert_eq!(game.server.settings.simulation_distance, 5);
        });
    }

    /// A single-player world's server is published while it is open to LAN, and no longer once
    /// it closes, though it runs headless (`options.dedicated` set) throughout; a dedicated
    /// server always is.
    #[test]
    #[ignore = "daily"]
    fn the_server_is_published_while_open_to_lan() {
        use sunrise_gameplay_common::game::execution_role::ExecutionRole;
        if super::super::native_entity_refs::tests::run_fresh_cases(
            "game::server_side::native_server_distances::tests::the_server_is_published_while_open_to_lan",
            "SUNRISE_TEST_SERVER_PUBLISHED_CASE",
            &["published"],
        ) {
            return;
        }
        with_guests(|game| {
            assert_eq!(game.options.execution_role, ExecutionRole::IntegratedServer);
            game.options.dedicated = Some(Vec::new());
            game.server.serving = Some(sunrise_gameplay::game::server_side::integrated::Serving::fixture(0));
            let published = |game: &mut sunrise_gameplay::game::ServerGame<'_>| {
                let host: &dyn mod_api::ServerHost = game;
                host.server_is_published()
            };
            assert!(!published(game), "a world not yet open to LAN");
            let settings = sunrise_gameplay::game::server_side::lan::LanSettings {
                port: 0,
                guest_commands: false,
                force_game_mode: false,
            };
            game.publish(settings).unwrap();
            assert!(published(game), "opened to LAN");
            assert!(game.unpublish());
            assert!(!published(game), "the LAN closed");
            game.server.serving = None;
            game.options.execution_role = ExecutionRole::DedicatedServer;
            assert!(published(game), "a dedicated server");
        });
    }
}