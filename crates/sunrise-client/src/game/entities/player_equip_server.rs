#[allow(unused_imports)]
pub(in crate::game::server_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::player_equip::ARMOR;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::player_equip::OFFHAND;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::player_equip::same;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::player_equip::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::items::ItemStack;

impl Game<'_, crate::game::server_side::ServerState> {
    pub(in crate::game) fn put_equipment_modifiers(&mut self) {
        let mut applied = std::mem::take(&mut self.side.applied_modifiers);
        let ran = self.apply_equipment_modifiers_with(&mut applied);
        self.side.applied_modifiers = applied;
        if !ran {
            return;
        }
        if let Some(player) = self.player.as_mut() {
            let _ = player.clamp_health_to_max();
        }
        self.apply_enchantment_attributes();
    }
}
