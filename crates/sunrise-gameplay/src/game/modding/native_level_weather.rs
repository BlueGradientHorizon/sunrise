//! Weather writes for native server mods through a `LevelRef`, on the same path as
//! `/weather`: `ServerLevel.setWeatherParameters` with the weather and thunder hooks, and
//! `Level.setRainLevel` / `setThunderLevel` for how strong the rain and thunder are now. Like
//! the command, they act on the level in place; a parked level, a stale handle or a client
//! returns false. The players see the change on the next host sync.
use crate::game::ServerGame;
use mod_api::LevelRef;
use sunrise_gameplay_common::game::level_control_hooks::LevelControl;

impl ServerGame<'_> {
    /// `ServerLevel.setWeatherParameters(clear, time, raining, thundering)` in the level in
    /// place, as `/weather` and mods set it: `weather_change` may cancel all of it and
    /// `thunder_change` the thunder half.
    pub(in crate::game) fn set_weather_parameters(
        &mut self,
        clear: i32,
        time: i32,
        raining: bool,
        thundering: bool,
    ) {
        let previous = (
            self.common.world.weather.clear_time,
            self.common.world.weather.rain_time,
            self.common.world.weather.thunder_time,
            self.common.world.weather.raining,
            self.common.world.weather.thundering,
        );
        let mut cancel = false;
        self.native_weather_change(self.common.dim.id(), raining, thundering, &mut cancel);
        let old_thundering = self.common.world.weather.thundering;
        let old_thunder_time = self.common.world.weather.thunder_time;
        let mut cancel_thunder = false;
        self.native_thunder_change(self.common.dim.id(), thundering, &mut cancel_thunder);
        if cancel {
            let w = &mut self.common.world.weather;
            (
                w.clear_time,
                w.rain_time,
                w.thunder_time,
                w.raining,
                w.thundering,
            ) = previous;
        } else {
            // ServerLevel.setWeatherParameters.
            let w = &mut self.common.world.weather;
            w.clear_time = clear;
            w.rain_time = time;
            w.thunder_time = time;
            w.raining = raining;
            w.thundering = thundering;
            if cancel_thunder {
                w.thunder_time = old_thunder_time;
                w.thundering = old_thundering;
            }
        }
    }
    /// `set_weather_parameters` through a handle of the level in place. False for a negative
    /// time, inside the `weather_change` or `thunder_change` hook, a parked level, a stale
    /// handle or a client.
    pub(in crate::game) fn level_set_weather(
        &mut self,
        level: LevelRef,
        clear_ticks: i32,
        weather_ticks: i32,
        raining: bool,
        thundering: bool,
    ) -> bool {
        if !self.level_in_place(level)
            || clear_ticks < 0
            || weather_ticks < 0
            || LevelControl::Weather.is_heard()
        {
            return false;
        }
        self.set_weather_parameters(clear_ticks, weather_ticks, raining, thundering);
        true
    }
    /// `Level.setRainLevel(rain)` and `setThunderLevel(thunder)`: both halves of each level
    /// at once, clamped to 0 to 1, so the change shows without fading in. False for a value
    /// that isn't finite, a parked level, a stale handle or a client.
    pub(in crate::game) fn level_set_weather_levels(
        &mut self,
        level: LevelRef,
        rain: f32,
        thunder: f32,
    ) -> bool {
        if !self.level_in_place(level) || !rain.is_finite() || !thunder.is_finite() {
            return false;
        }
        let w = &mut self.common.world.weather;
        let (rain, thunder) = (rain.clamp(0.0, 1.0), thunder.clamp(0.0, 1.0));
        w.rain_level = (rain, rain);
        w.thunder_level = (thunder, thunder);
        true
    }
}

use sunrise_gameplay_common::game::native_level_refs_common::NativeLevelRefsCommonGameExt1 as _;
