//! World clocks shared by server and client, plus hub clock snapshot calculation.
use super::{Game, chat_common::OVERWORLD_CLOCK};
use sunrise_gameplay::world_clock::WorldClock;

/// What the LAN hub's time packets carry, read from the clocks without touching the hub.
pub(in crate::game) struct HubClockSnapshot {
    pub(in crate::game) others: Vec<(String, i64, f32)>,
    pub(in crate::game) overworld_day: Option<i64>,
    pub(in crate::game) level_clock: Option<(String, f32)>,
}

impl Game<'_> {
    /// Advance every world clock (`ServerClockManager.tick`) when `advance_time` allows it.
    pub(in crate::game) fn advance_clock(&mut self) {
        if !self
            .ui
            .rules
            .bool_at(sunrise_client::gamerules::vanilla::ADVANCE_TIME)
        {
            // The rule may have just been turned off: tell the guests the clock stopped.
            self.sync_hub_clocks();
            return;
        }
        let mut names: Vec<String> = self.ui.command_clocks.keys().cloned().collect();
        let active = self.active_clock().to_owned();
        // ServerClockManager.init holds every `minecraft:world_clock` entry, including mod clocks.
        for name in [
            OVERWORLD_CLOCK.to_owned(),
            "minecraft:the_end".to_owned(),
            active,
        ]
        .into_iter()
        .chain(self.ui.registries.clocks.iter().cloned())
        .chain(mod_api::__private::world_clock_declarations())
        {
            if !names.contains(&name) {
                names.push(name);
            }
        }
        for name in names {
            let state = self.command_clock_state(&name);
            if state.paused {
                continue;
            }
            state.partial += state.rate;
            let whole = state.partial.floor();
            state.partial -= whole;
            let total = self.command_clock_time(&name) + whole as i64;
            self.set_command_clock_time(&name, total);
        }
        self.sync_hub_clocks();
    }

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

    /// The LAN hub's time packets, read from the clocks without touching the hub.
    pub(in crate::game) fn hub_clock_snapshot(&mut self) -> HubClockSnapshot {
        let active = self.active_clock().to_owned();
        let overworld = self.command_clock_time(OVERWORLD_CLOCK);
        let advancing = self
            .ui
            .rules
            .bool_at(sunrise_client::gamerules::vanilla::ADVANCE_TIME);
        // A paused clock, or `advance_time` off, is a rate of 0 to the guests, as for the Overworld's.
        let guest_rate = |state: &WorldClock| {
            if advancing && !state.paused {
                state.rate
            } else {
                0.0
            }
        };
        let rate = guest_rate(self.command_clock_state(&active));
        let end_total = self.command_clock_time("minecraft:the_end");
        let others: Vec<(String, i64, f32)> = std::iter::once((
            "minecraft:the_end".to_owned(),
            end_total,
            guest_rate(&self.ui.end_clock),
        ))
        .chain(
            self.ui
                .command_clocks
                .iter()
                .map(|(name, (total, state))| (name.clone(), *total, guest_rate(state))),
        )
        .filter(|(name, _, _)| name.as_str() != active && name.as_str() != OVERWORLD_CLOCK)
        .collect();
        let (overworld_day, level_clock) = if active == OVERWORLD_CLOCK {
            (None, None)
        } else {
            (Some(overworld), Some((active, rate)))
        };
        HubClockSnapshot {
            others,
            overworld_day,
            level_clock,
        }
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

    /// `ServerClockInstance.packNetworkState`: overworld clock rate sent to clients.
    pub(in crate::game) fn network_clock_rate(&self) -> f32 {
        if self
            .ui
            .rules
            .bool_at(sunrise_client::gamerules::vanilla::ADVANCE_TIME)
            && !self.ui.clock.paused
        {
            self.ui.clock.rate
        } else {
            0.0
        }
    }
}
