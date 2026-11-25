#[allow(unused_imports)]
pub(in crate::game::client_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::level_sounds::LevelSound;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::SoundSource;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{first_person::Hand, mobs::sheep_dye};

impl Game<'_> {
    /// Whether `interact_sheep_dye` would act on a client, on a copy of the sheep.
    pub(in crate::game::client_side) fn sheep_would_take_dye(
        &mut self,
        index: usize,
        hand: Hand,
    ) -> bool {
        let Some(mut stack) = self.with_hand(hand, |g| g.held().cloned()) else {
            return false;
        };
        sheep_dye::interact(&mut self.mobs[index].clone(), &mut stack, true)
    }

    /// Client-side sheep dyeing prediction: plays local sound and leaves wool/dye to server sync.
    pub(in crate::game) fn client_interact_sheep_dye(&mut self, index: usize, hand: Hand) -> bool {
        let Some(mut stack) = self.with_hand(hand, |g| g.held().cloned()) else {
            return false;
        };
        if !sheep_dye::interact(&mut self.mobs[index], &mut stack, true) {
            return false;
        }
        let mob = &self.mobs[index];
        if !mob.silent {
            let at = [mob.position.x, mob.position.y, mob.position.z];
            self.play_client_local_sound(
                LevelSound::new("minecraft:item.dye.use", at, 1.0, 1.0)
                    .in_source(SoundSource::Players),
            );
        }
        true
    }
}
