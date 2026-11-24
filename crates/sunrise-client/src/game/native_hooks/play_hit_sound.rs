#[allow(unused_imports)]
use super::*;
#[allow(unused_imports)]
use crate::game::Audio;
#[allow(unused_imports)]
use sunrise_core::{geometry::Direction, position::BlockPos};

impl Game<'_> {
    pub(in crate::game) fn native_play_hit_sound(
        &mut self,
        call: super::fire::PlayHitSoundCall<'_>,
    ) {
        self.fire_play_hit_sound(call);
    }
}
