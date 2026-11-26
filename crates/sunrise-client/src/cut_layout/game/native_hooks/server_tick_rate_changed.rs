

#![cfg(test)]
#[cfg(test)]
#[cfg(any())]
pub(crate) mod server_tick_rate_changed_tests {

use sunrise_gameplay_common::game::native_hooks::server_tick_rate_changed::server_tick_rate_changed_tests::Scheduler;

use crate::game::native_guest_turn_tests::with_guests;

use mod_api::{Hooks, ModEntry, ServerHost, ServerTickRateChangedEvent};

use mod_platform::testing::with_mods;

use std::sync::{Arc, Mutex};

pub(crate) fn with_scheduler(
        retune: Option<&'static str>,
        check: impl FnOnce(&mut sunrise_gameplay::game::ServerGame<'_>, &dyn Fn() -> Vec<f32>),
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
