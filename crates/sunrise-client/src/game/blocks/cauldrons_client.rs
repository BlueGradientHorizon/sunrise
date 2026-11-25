#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::cauldrons::is_under_water;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::cauldrons::*;

impl Game<'_, crate::game::client_side::ClientState> {
    /// What the cauldron at `pos` answers to `held` on a client, which only
    /// asks (`CauldronInteractions` acts `if (!level.isClientSide())`). A
    /// sneak with something in hand skips `useItemOn`
    /// (`Player.isSecondaryUseActive`).
    pub(in crate::game::client_side) fn cauldron_outcome(
        &self,
        pos: [i32; 3],
        held: Option<&ItemStack>,
    ) -> sunrise_client::cauldron::Outcome {
        self.client_cauldron_prediction(pos, held).0
    }
}
