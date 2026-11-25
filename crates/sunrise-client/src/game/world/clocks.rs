//! World clocks shared by server and client, plus hub clock snapshot calculation.
use super::{Game, chat_common::OVERWORLD_CLOCK};
use sunrise_gameplay::world_clock::WorldClock;

/// What the LAN hub's time packets carry, read from the clocks without touching the hub.
pub(in crate::game) struct HubClockSnapshot {
    pub(in crate::game) others: Vec<(String, i64, f32)>,
    pub(in crate::game) overworld_day: Option<i64>,
    pub(in crate::game) level_clock: Option<(String, f32)>,
}

impl<__Side: crate::game::Side> Game<'_, __Side> {
    /// The clock `World.day_time` follows (`UiState::active_clock`).
    pub(in crate::game) fn active_clock(&self) -> &str {
        if self.ui.active_clock.is_empty() {
            OVERWORLD_CLOCK
        } else {
            &self.ui.active_clock
        }
    }

    /// `ServerClockManager.getTotalTicks` of `clock`.
    pub(in crate::game) fn command_clock_time(&self, clock: &str) -> i64 {
        if clock == self.active_clock() {
            self.world.day_time
        } else if clock == "minecraft:the_end" {
            self.world.end_clock
        } else {
            self.ui.command_clocks.get(clock).map_or(0, |c| c.0)
        }
    }

    pub(in crate::game) fn set_command_clock_time(&mut self, clock: &str, total: i64) {
        if clock == "minecraft:the_end" {
            self.world.end_clock = total;
        }
        if clock == self.active_clock() {
            self.world.day_time = total;
        } else {
            self.ui
                .command_clocks
                .entry(clock.to_owned())
                .or_default()
                .0 = total;
        }
    }

    /// `ServerClockManager.getInstance`: the rate, partial tick and pause of `clock`.
    pub(in crate::game) fn command_clock_state(&mut self, clock: &str) -> &mut WorldClock {
        if clock == OVERWORLD_CLOCK {
            &mut self.ui.clock
        } else if clock == "minecraft:the_end" {
            &mut self.ui.end_clock
        } else {
            &mut self
                .ui
                .command_clocks
                .entry(clock.to_owned())
                .or_default()
                .1
        }
    }

    /// The clock the day time of `dim`'s level follows: its default clock, else the Overworld's.
    pub(in crate::game) fn level_clock_name(&self, dim: crate::game::Dim) -> String {
        self.dimension_type_of(dim)
            .default_clock
            .clone()
            .unwrap_or_else(|| OVERWORLD_CLOCK.to_owned())
    }

    pub(in crate::game) fn sync_hub_clocks(&mut self) {
        self.side_sync_hub_clocks();
    }

    /// Makes `to` the clock `World.day_time` follows and returns its new total.
    pub(in crate::game) fn switch_clock(&mut self, to: &str) -> i64 {
        let from = self.active_clock().to_owned();
        if from == to {
            return self.world.day_time;
        }
        let total = self.world.day_time;
        if from == "minecraft:the_end" {
            self.world.end_clock = total;
        }
        self.ui.command_clocks.entry(from).or_default().0 = total;
        self.ui.active_clock = to.to_owned();
        if to == "minecraft:the_end" {
            self.world.end_clock
        } else {
            self.ui.command_clocks.entry(to.to_owned()).or_default().0
        }
    }
}
