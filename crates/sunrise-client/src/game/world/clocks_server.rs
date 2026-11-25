#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{Game, chat_common::OVERWORLD_CLOCK};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::clocks::HubClockSnapshot;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::clocks::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_gameplay::world_clock::WorldClock;

impl Game<'_, crate::game::server_side::ServerState> {
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
    /// The LAN hub's time packets, read from the clocks without touching the hub.
    pub(in crate::game::server_side) fn hub_clock_snapshot(&mut self) -> HubClockSnapshot {
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
