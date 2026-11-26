

#![cfg(test)]
#[cfg(test)]
mod tests {
    use sunrise_gameplay_common::game::native_entity_refs_common::NativeEntityRefsCommonGameExt1;
use mod_api::{LevelRef, WorldEpoch};

    #[test]
    #[ignore = "daily"]
    fn weather_writes_set_the_level_in_place_and_refuse_others() {
        super::super::native_entity_refs::tests::with_game(|game| {
            let level = game.level(game.dim.id()).unwrap();
            let stale = LevelRef::new(WorldEpoch::new(level.epoch().get() + 1), level.dimension());
            let nether = LevelRef::new(level.epoch(), mod_api::DimensionKey::NETHER);

            assert!(level.set_weather(game, 0, 6000, true, true));
            let w = &game.world.weather;
            assert_eq!(
                (
                    w.clear_time,
                    w.rain_time,
                    w.thunder_time,
                    w.raining,
                    w.thundering
                ),
                (0, 6000, 6000, true, true)
            );
            assert!(level.set_weather(game, 1200, 0, false, false));
            let w = &game.world.weather;
            assert_eq!(
                (w.clear_time, w.raining, w.thundering),
                (1200, false, false)
            );
            assert!(!level.set_weather(game, -1, 0, false, false));
            assert!(
                !nether.set_weather(game, 0, 10, true, false),
                "a level not in place"
            );
            assert!(!stale.set_weather(game, 0, 10, true, false));

            assert!(level.set_weather_levels(game, 0.5, 2.0));
            let w = &game.world.weather;
            assert_eq!(
                (w.rain_level, w.thunder_level),
                ((0.5, 0.5), (1.0, 1.0)),
                "clamped, without fading"
            );
            assert_eq!(
                level.weather(game),
                Some((0.5, 0.5)),
                "getThunderLevel reads thunder times rain"
            );
            assert!(!level.set_weather_levels(game, f32::NAN, 0.0));
            assert!(!stale.set_weather_levels(game, 0.0, 0.0));
        });
    }

    #[test]
    #[ignore = "daily"]
    fn local_difficulty_and_tick_rate_reach_the_game() {
        super::super::native_entity_refs::tests::with_game(|game| {
            let level = game.level(game.dim.id()).unwrap();
            let stale = LevelRef::new(WorldEpoch::new(level.epoch().get() + 1), level.dimension());
            let here = game.local_difficulty([3.0, 64.0, -7.0]);
            let read = level.local_difficulty(game, [3, 64, -7]).unwrap();
            assert_eq!((read.base, read.effective), (here.base, here.effective));
            assert_eq!(read.special_multiplier(), here.special());
            assert_eq!(stale.local_difficulty(game, [3, 64, -7]), None);

            assert!(game.set_tick_rate(40.0));
            assert_eq!(game.tick_stats().0, 40.0);
            assert!(!game.set_tick_rate(0.5));
            assert!(!game.set_tick_rate(f32::NAN));
            assert_eq!(game.clock.rate(), 40.0, "refusals change nothing");
        });
    }
    type Log = std::sync::Arc<std::sync::Mutex<Vec<(&'static str, bool)>>>;

    /// A mod that enforces its own weather, border and tick rate from inside the hooks those
    /// writes fire, the loop the setters refuse.
    struct Enforcer(Log);

    impl Enforcer {
        fn log(&self, what: &'static str, ok: bool) {
            self.0.lock().unwrap().push((what, ok));
        }
    }

    #[mod_api::__subscriptions]
    impl mod_api::Hooks for Enforcer {
        fn weather_change(
            &self,
            host: &mut dyn mod_api::ServerHost,
            event: &mut mod_api::WeatherChangeEvent<'_>,
        ) -> mod_api::Flow {
            let level = host.level(event.dimension).unwrap();
            self.log("weather", level.set_weather(host, 0, 100, false, false));
            mod_api::Flow::Continue
        }
        fn world_border_center_changed(
            &self,
            host: &mut dyn mod_api::ServerHost,
            _: &mut mod_api::WorldBorderCenterChangedEvent,
        ) {
            let level = host.level("minecraft:overworld").unwrap();
            self.log("center", level.set_world_border_center(host, [7.0, 7.0]));
            let size = level.set_world_border_size(host, 300.0, 0);
            self.log("size from center", size);
        }
        fn world_border_bounds_changed(
            &self,
            host: &mut dyn mod_api::ServerHost,
            _: &mut mod_api::WorldBorderBoundsChangedEvent,
        ) {
            let level = host.level("minecraft:overworld").unwrap();
            self.log("size", level.set_world_border_size(host, 400.0, 0));
            let center = level.set_world_border_center(host, [9.0, 9.0]);
            self.log("center from size", center);
        }
        fn server_tick_rate_changed(
            &self,
            host: &mut dyn mod_api::ServerHost,
            _: &mut mod_api::ServerTickRateChangedEvent,
        ) {
            self.log("tick", host.set_tick_rate(30.0));
        }
    }

    /// A listener calling the setter of the hook it hears is refused, so the hook doesn't fire
    /// again without end; another control's setter runs once, and its own hook can't come back
    /// to the first. The outer writes stand, and a write of the value already there is heard
    /// by nobody.
    #[test]
    #[ignore = "daily"]
    fn a_listener_calling_its_own_setter_is_refused() {
        let log = Log::default();
        let mods: mod_api::Mods = vec![mod_api::ModEntry::new(
            "enforcer",
            Box::new(Enforcer(log.clone())),
        )]
        .into();
        let take = || std::mem::take(&mut *log.lock().unwrap());
        mod_platform::testing::with_mods(mods, || {
            super::super::native_entity_refs::tests::with_game(|game| {
                let level = game.level(game.dim.id()).unwrap();

                assert!(level.set_weather(game, 0, 6000, true, false));
                assert_eq!(take(), [("weather", false)]);
                assert!(game.world.weather.raining, "the outer write stands");

                assert!(level.set_world_border_center(game, [100.0, -50.0]));
                assert_eq!(
                    take(),
                    [
                        ("center", false),
                        ("size", false),
                        ("center from size", false),
                        ("size from center", true),
                    ]
                );
                assert_eq!(game.border.center(), [100.0, -50.0]);
                assert_eq!(game.border.size(), 300.0);
                assert!(level.set_world_border_center(game, [100.0, -50.0]));
                assert!(level.set_world_border_size(game, 300.0, 0));
                assert!(take().is_empty(), "an unchanged border is heard by nobody");

                assert!(game.set_tick_rate(40.0));
                assert_eq!(take(), [("tick", false)]);
                assert_eq!(game.clock.rate(), 40.0);
            });
        });
    }
    /// `DifficultyInstance`'s formula worked by hand, not through the shared code, for a chunk
    /// that isn't loaded (no inhabited time, no moon) on Hard: 3 x 0.75 = 2.25 (special
    /// multiplier 0.125) before day time 72 000, and 3 x (0.75 + 0.25) = 3.0 (special 0.5)
    /// once the time factor is full at 1 512 000.
    #[test]
    #[ignore = "daily"]
    fn local_difficulty_matches_vanilla_values_for_an_unloaded_chunk() {
        super::super::native_entity_refs::tests::with_game(|game| {
            let level = game.level(game.dim.id()).unwrap();
            let far = (100_000, -100_000);
            assert!(!game.world.chunks.contains_key(&far));
            let at = [far.0 * 16 + 8, 64, far.1 * 16 + 8];
            game.world.health_rules.difficulty = sunrise_world::food::Difficulty::Hard;
            for (day_time, effective, special) in [(0, 2.25, 0.125), (1_512_000, 3.0, 0.5)] {
                game.world.day_time = day_time;
                let read = level.local_difficulty(game, at).unwrap();
                assert_eq!((read.base, read.effective), (3, effective), "{day_time}");
                assert_eq!(read.special_multiplier(), special, "{day_time}");
            }
        });
    }
}
use sunrise_gameplay_common::game::native_entity_refs_common::NativeEntityRefsCommonGameExt1 as _;
