#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{Game, Mining, minecarts::CartKind};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::hand::BreakSpeedDispatch;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::hand::GameHands;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::hand::HarvestDispatch;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::hand::OFFHAND;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::hand::interact_location;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::hand::matches_for_swap;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::hand::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    Result,
    attack_strength::HeldStack,
    first_person::{Hand, HandWorld, Hit, SwingAnimation, UseResult, swing_duration},
    items::ItemStack,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::geometry::{Aabb, Vec3};

impl Game<'_> {
    /// The item whose attack damage and speed modifiers the player has (`attack_item`).
    pub(in crate::game::server_side) fn attack_item_info(
        &self,
    ) -> Option<&sunrise_client::items::ItemInfo> {
        self.items().get(self.server.attack_item.as_deref()?)
    }
    /// `swing_arm` for a use whose result is `SwingSource.PREDICTED`: the
    /// player's client swings by itself, so a server keeps it to the others.
    pub(in crate::game::server_side) fn swing_arm_predicted(&mut self) {
        let before = self.swings_started;
        self.swing_arm();
        if self.swings_started != before {
            self.server.predicted_swings += 1;
        }
    }
    #[inline(always)]
    pub(in crate::game) fn tick_hand_server(
        &mut self,
        reset: bool,
        changed: bool,
        main: Option<&ItemStack>,
    ) {
        if reset {
            self.server.attack_ticks = 0;
        }
        if changed {
            self.apply_equipment();
        }
        self.server.attack_item = main.map(|s| s.item.clone());
    }
}
