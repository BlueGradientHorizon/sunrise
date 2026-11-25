#[allow(unused_imports)]
use super::Game;
#[allow(unused_imports)]
use sunrise_client::{first_person::Hand, mobs::sheep_dye};

impl Game<'_> {
    /// `DyeItem.interactLivingEntity`: PLAYERS, fixed pitch, excluding the
    /// actor on the server because that client's item interaction plays it.
    pub(in crate::game) fn interact_sheep_dye(
        &mut self,
        index: usize,
        hand: Hand,
        client: bool,
    ) -> bool {
        let Some(mut stack) = self.with_hand(hand, |g| g.held().cloned()) else {
            return false;
        };
        if !sheep_dye::interact(&mut self.mobs[index], &mut stack, client) {
            return false;
        }
        self.play_mob_bound_sound(index, "minecraft:item.dye.use", 7, 1.0, 1.0, !client);
        if !client && !self.creative {
            self.with_hand(hand, |g| g.consume_player_slot(g.held_slot(), 1));
        }
        true
    }
}
