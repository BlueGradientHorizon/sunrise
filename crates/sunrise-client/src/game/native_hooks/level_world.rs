#[allow(unused_imports)]
use crate::game::{Game, api_values::with_live_level_random};
#[allow(unused_imports)]
use mod_api::{LevelRef, RandomDraw};
#[allow(unused_imports)]
use sunrise_core::random::LegacyRandom;

impl Game<'_> {
    /// Lend the named level's live stream directly.
    pub(in crate::game) fn with_level_random(&mut self, level: LevelRef, draw: RandomDraw<'_>) {
        let Some(random) = self.level_random_mut(level) else {
            return;
        };
        with_live_level_random(random, draw);
    }
}
