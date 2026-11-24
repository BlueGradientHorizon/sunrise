#[allow(unused_imports)]
pub(in crate::game::client_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::piston_carry_state::PositionChange;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::piston_carry_state::PositionState;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::piston_carry_state::land_position_data;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::piston_carry_state::take_position_data;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::piston_carry_state::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    block_entities::Pos,
    chunk::PendingContainerLoot,
    world::{BlockEntityExtras, World},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::nbt::{Compound, Tag};

impl Game<'_> {
    #[inline(always)]
    pub(in crate::game) fn finish_piston_position_changes_client(&mut self, at: Pos) {
        self.client.test_highlights.remove(&at);
    }
}
