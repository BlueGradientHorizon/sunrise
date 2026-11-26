

#![cfg(test)]
#[cfg(test)]
pub(crate) mod tests {

use sunrise_gameplay_common::game::server_side::wake_time::tests::NOW;

use sunrise_gameplay_common::game::server_side::wake_time::tests::marker;

use mod_api::{ClockAdjustment, Identifier};

use sunrise_gameplay::world_clock::TimeMarker;

/// What a second listener reads: the first moves the clock 1000 ticks on, the second sees
    /// that and sends it to noon instead, and the game resolves the last word once.
    #[test]
    #[ignore = "daily"]
    fn a_later_listener_reads_and_replaces_the_earlier_adjustment() {
        use std::sync::{Arc, Mutex};
        use sunrise_gameplay::world_clock::OVERWORLD_CLOCK;
        struct First;
        #[mod_api::__subscriptions]
        impl mod_api::Hooks for First {
            fn sleep_finished_time(
                &self,
                _: &mut dyn mod_api::ServerHost,
                event: &mut mod_api::SleepFinishedTimeEvent<'_>,
            ) -> mod_api::Flow {
                *event.adjustment = ClockAdjustment::Relative(1_000);
                mod_api::Flow::Continue
            }
        }
        struct Second(Arc<Mutex<Option<ClockAdjustment>>>);
        #[mod_api::__subscriptions]
        impl mod_api::Hooks for Second {
            fn sleep_finished_time(
                &self,
                _: &mut dyn mod_api::ServerHost,
                event: &mut mod_api::SleepFinishedTimeEvent<'_>,
            ) -> mod_api::Flow {
                *self.0.lock().unwrap() = Some(event.adjustment.clone());
                *event.adjustment = marker("minecraft:noon");
                mod_api::Flow::Continue
            }
        }
        sunrise_gameplay::game::native_entity_refs::tests::with_game(|game| {
            let noon = TimeMarker {
                ticks: 6_000,
                period: Some(24_000),
                show: false,
            };
            game.ui.markers.insert("minecraft:noon".into(), noon);
            let clock = game.level_clock_name(game.dim);
            assert_eq!(clock, OVERWORLD_CLOCK);
            game.set_command_clock_time(&clock, NOW);
            let seen = Arc::new(Mutex::new(None));
            let mods = vec![
                mod_api::ModEntry::new("first", Box::new(First) as Box<dyn mod_api::Hooks>),
                mod_api::ModEntry::new(
                    "second",
                    Box::new(Second(seen.clone())) as Box<dyn mod_api::Hooks>,
                ),
            ]
            .into();
            let moved = mod_platform::testing::with_mods(mods, || game.sleep_finished_move());
            assert_eq!(
                *seen.lock().unwrap(),
                Some(ClockAdjustment::Relative(1_000))
            );
            assert_eq!(moved, Some((OVERWORLD_CLOCK.to_owned(), NOW, 30_000)));
        });
    }
}
