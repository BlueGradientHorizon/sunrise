//! The world's save on a timer (`MinecraftServer.computeNextAutosaveInterval`,
//! `onTickRateChanged`), the players' autosaves, and the world's metadata (level.dat).
#[allow(unused_imports)]
use super::*;

impl Game<'_> {
    pub(in crate::game) fn admit_entity_save() -> Result<()> {
        if !sunrise_client::join_seq::visibility::passes_settled() {
            return Err("entity pass is lifted".into());
        }
        Ok(())
    }

    pub(in crate::game) fn save_game(&mut self) -> Result<()> {
        self.save_game_with(true)
    }

    /// `MinecraftServer.computeNextAutosaveInterval`: three hundred seconds
    /// of ticks at the rate the server runs at now - its measured rate while
    /// a `/tick sprint` runs - never fewer than a hundred.
    pub(in crate::game) fn next_autosave_interval(&self) -> i64 {
        let rate = if self.clock.sprinting() {
            1.0e9 / (self.clock.average_nanos() + 1) as f32
        } else {
            self.clock.rate()
        };
        autosave_interval(rate)
    }
}

impl Game<'_> {
    pub(in crate::game) fn admit_level_save(&self) -> Result<()> {
        // A carrier transfer that broke after `prepared` stopped the
        // session (`carrier_stop_session`): its recovery replays the
        // affected regions from the disk state the transfer itself
        // wrote, so no later save — the autosave a later tick would
        // run, or any exit path's — may write over them.
        if self.world.carrier_fences.is_broken() {
            return Err(
                "a carrier transfer did not finish; the world is left for recovery at reopen"
                    .into(),
            );
        }
        Self::admit_entity_save()?;
        Ok(())
    }
}
