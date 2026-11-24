#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::lan_actions::sleep::*;

impl Game<'_> {
    /// `ServerLevel.updateSleepingPlayerList` and `announceSleepStatus`: when
    /// the players or the sleepers change, a published or dedicated world
    /// shows everyone "n/m players sleeping", or "Sleeping through this
    /// night" once enough are in bed, on the action bar.
    pub(in crate::game) fn announce_sleep_status(&mut self) {
        let (active, sleeping, _) = self.sleep_counts();
        let (old_active, old_sleeping) = self.server.sleep_seen;
        self.server.sleep_seen = (active, sleeping);
        let changed = (old_sleeping > 0 || sleeping > 0)
            && (old_active != active || old_sleeping != sleeping);
        let published = self.lan.is_some() || self.options.dedicated.is_some();
        if !changed || !published {
            return;
        }
        // ServerLevel.canSleepThroughNights.
        if self
            .ui
            .rules
            .int_at(sunrise_client::gamerules::vanilla::PLAYERS_SLEEPING_PERCENTAGE)
            > 100
        {
            return;
        }
        let needed = self.sleepers_needed(active);
        let text = if sleeping >= needed {
            sunrise_client::text::Text::translate("sleep.skipping_night", &[])
        } else {
            sunrise_client::text::Text::translate(
                "sleep.players_sleeping",
                &[
                    sunrise_client::text::Text::literal(sleeping.to_string()),
                    sunrise_client::text::Text::literal(needed.to_string()),
                ],
            )
        };
        if let Some(host) = self.lan.as_ref() {
            host.hub
                .overlay_message(sunrise_client::lan::data::component_nbt(&text.component()));
        }
        if self.options.dedicated.is_none() {
            self.set_hud_overlay(text);
        }
    }
    /// `SleepStatus.update`'s counts over the host's player and the guests:
    /// the players not in spectator mode, how many are in bed, and how many
    /// of those have slept 100 ticks.
    pub(in crate::game) fn sleep_counts(&self) -> (i64, i64, i64) {
        let (mut active, mut sleeping, mut deep) = (0i64, 0i64, 0i64);
        let mut count = |in_bed: bool, timer: u32| {
            active += 1;
            if in_bed {
                sleeping += 1;
                if timer >= 100 {
                    deep += 1;
                }
            }
        };
        if self.player.is_some() && !self.server.lan_acting_fake && !self.spectator() {
            count(self.sleeping.is_some(), self.sleep_timer);
        }
        if let Some(host) = self.lan.as_ref() {
            // A native mod's ignored players count only once in bed.
            for g in host
                .hub
                .guests
                .iter()
                .filter(|g| self.guest_counts_for_sleep(g))
            {
                count(g.bed.is_some(), g.sleep_timer);
            }
        }
        (active, sleeping, deep)
    }
}
