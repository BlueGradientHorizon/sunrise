#[allow(unused_imports)]
pub(in crate::game::client_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::sheep_dye::*;
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
}
