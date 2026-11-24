#[allow(unused_imports)]
pub(in crate::game::client_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::entity_section_steps::player_teleport;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::entity_section_steps::tick_items;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::entity_section_steps::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::geometry::Vec3;

impl Game<'_> {
    /// `ClientLevel.isTickingEntity`: players always tick; other entities tick while their chunk is loaded.
    pub(in crate::game) fn remote_entity_is_ticking(&self, id: i32) -> bool {
        let Some(r) = self.remote.as_ref() else {
            return true;
        };
        if r.entity_id == id || r.level_players.contains_key(&id) {
            return true;
        }
        let pos = if let Some(m) = self.mob_index.get_ref(&self.mobs, id as u32) {
            [m.position.x, m.position.z]
        } else if let Some(i) = self.client.interactions.get(&id) {
            [i.position.x, i.position.z]
        } else if let Some(a) = self.armor_stands.iter().find(|a| a.id == id as u32) {
            [a.position.x, a.position.z]
        } else if let Some(b) = self.boats.iter().find(|b| b.id == id as u32) {
            [b.position.x, b.position.z]
        } else if let Some(c) = self.carts.iter().find(|c| c.id == id as u32) {
            [c.position.x, c.position.z]
        } else if let Some(t) = r.entities.get(&id) {
            [t.position[0], t.position[2]]
        } else {
            return false;
        };
        self.world
            .chunks
            .contains_key(&((pos[0].floor() as i32) >> 4, (pos[1].floor() as i32) >> 4))
    }
}
