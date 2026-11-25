use super::*;

/// The hook points. Each does nothing in Sunrise.
impl Game<'_, crate::game::server_side::ServerState> {
    /// `MinecraftServer.onTickRateChanged`, before the autosave wait is adjusted: `/tick rate`
    /// set a rate or a `/tick sprint` ended.
    pub(in crate::game) fn native_server_tick_rate_changed(&mut self) {
        crate::game::level_control_hooks::LevelControl::TickRate
            .hear(|| self.fire_server_tick_rate_changed());
    }
}

#[cfg(test)]
pub(in crate::game) mod server_tick_rate_changed_tests {
    use crate::game::native_guest_turn_tests::with_guests;
    use mod_api::{Hooks, ModEntry, ServerHost, ServerTickRateChangedEvent};
    use mod_platform::testing::with_mods;
    use std::sync::{Arc, Mutex};

    /// A scheduling mod: logs the rate each notification reads, and runs `retune` on the first.
    pub(in crate::game) struct Scheduler {
        pub(in crate::game) rates: Arc<Mutex<Vec<f32>>>,
        pub(in crate::game) retune: Option<&'static str>,
    }
    #[mod_api::__subscriptions]
    impl Hooks for Scheduler {
        fn server_tick_rate_changed(
            &self,
            host: &mut dyn ServerHost,
            _: &mut ServerTickRateChangedEvent,
        ) {
            let first = {
                let mut rates = self.rates.lock().unwrap();
                rates.push(host.tick_stats().0);
                rates.len() == 1
            };
            if let (true, Some(line)) = (first, self.retune) {
                assert!(host.commands().run_command(line));
            }
        }
    }

    pub(in crate::game) fn with_scheduler(
        retune: Option<&'static str>,
        check: impl FnOnce(&mut crate::game::ServerGame<'_>, &dyn Fn() -> Vec<f32>),
    ) {
        let rates = Arc::new(Mutex::new(Vec::new()));
        let scheduler = Scheduler {
            rates: Arc::clone(&rates),
            retune,
        };
        let mods: mod_api::Mods = vec![ModEntry::new("scheduler", Box::new(scheduler))].into();
        with_mods(mods, || {
            with_guests(|game| check(game, &|| std::mem::take(&mut *rates.lock().unwrap())));
        });
    }
}
