#![cfg(test)]
#[cfg(test)]
pub(crate) mod server_tick_rate_changed_tests {
    use crate::game::native_hooks::server_tick_rate_changed::server_tick_rate_changed_tests::with_scheduler;

    /// The hook comes before the wait is adjusted: a listener that sets another rate decides
    /// the wait, here 40 ticks a second keeping the 6000 that 10 would have halved.
    #[test]
    #[ignore = "daily"]
    fn a_listener_runs_before_the_autosave_wait_is_adjusted() {
        with_scheduler(Some("tick rate 40"), |game, heard| {
            assert!(game.native_run_command("tick rate 10"));
            assert_eq!(heard(), [10.0, 40.0]);
            assert_eq!(game.clock.rate(), 40.0);
            assert_eq!(game.side.ticks_until_autosave, 6000);
        });
    }

    /// `ServerTickRateManager.setTickRate` and `finishTickSprint`, vanilla's only two callers of
    /// `MinecraftServer.onTickRateChanged`: each route tells a mod once, the new rate readable,
    /// and leaves the autosave wait at 300 seconds of ticks when that is shorter.
    #[test]
    #[ignore = "daily"]
    fn a_mod_hears_every_route_of_on_tick_rate_changed_once() {
        with_scheduler(None, |game, heard| {
            assert_eq!(game.side.ticks_until_autosave, 6000);
            assert!(game.native_run_command("tick rate 10"));
            assert_eq!(heard(), [10.0]);
            assert_eq!(game.side.ticks_until_autosave, 3000);
            assert!(game.native_run_command("tick rate 40"));
            assert_eq!(heard(), [40.0]);
            assert_eq!(game.side.ticks_until_autosave, 3000);
            assert!(game.native_run_command("tick rate 40"));
            assert_eq!(heard(), [40.0]);

            assert!(!game.native_run_command("tick sprint stop"));
            assert!(game.native_run_command("tick sprint 5"));
            assert!(heard().is_empty());
            game.side.ticks_until_autosave = 100_000;
            assert!(game.native_run_command("tick sprint stop"));
            assert_eq!(heard(), [40.0]);
            assert_eq!(game.side.ticks_until_autosave, 12_000);

            assert!(game.native_run_command("tick sprint 5"));
            assert!(game.native_run_command("tick freeze"));
            assert_eq!(heard(), [40.0]);
            assert!(game.native_run_command("tick unfreeze"));
            assert!(heard().is_empty());

            assert!(game.native_run_command("tick rate 20"));
            assert_eq!(heard(), [20.0]);
            assert!(game.native_run_command("tick sprint 1"));
            game.side.ticks_until_autosave = 100_000;
            game.clock.world_runs();
            assert!(heard().is_empty());
            game.tick_server(false).unwrap();
            assert_eq!(heard(), [20.0]);
            // Completion shortens the wait before this tick decrements it.
            assert_eq!(game.side.ticks_until_autosave, 5999);
            game.tick_server(false).unwrap();
            assert!(heard().is_empty());
            assert_eq!(game.side.ticks_until_autosave, 5998);
        });
    }
}