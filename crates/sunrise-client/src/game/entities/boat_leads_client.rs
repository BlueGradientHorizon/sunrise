#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{Game, boats::Boat};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::boat_leads::HEIGHT;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::boat_leads::Held;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::boat_leads::WIDTH;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::boat_leads::center;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::boat_leads::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::actions::Action;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::mobs::leash::{
    self, Holder, HolderState, Pending,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::geometry::Vec3;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::nbt::Tag;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::position::BlockPos;

impl Game<'_> {
    /// `Entity.interact`'s lead part for the boat at `i`; whether it took
    /// the click.
    pub(in crate::game) fn interact_boat_leash(&mut self, i: usize) -> bool {
        let item = self.held().map(|s| s.item.to_string());
        let c = center(&self.boats[i]);
        let at = [c.x, c.y, c.z];
        let id = self.boats[i].id;
        // The sneak branch: the player's mobs and boats on leads are re-tied
        // to this boat.
        let sneaking = self.server_acting_sneak();
        if sneaking
            && self.tie_player_leads(glam::DVec3::new(c.x, c.y, c.z), Holder::Boat(id), None)
        {
            self.lead_game_event("minecraft:entity_action", at);
            self.lead_sound("tied", at);
            return true;
        }
        // Entity.shearOffAllLeashConnections: its own lead and every one
        // hanging on it.
        if item.as_deref() == Some("minecraft:shears") {
            let held_mobs: Vec<u32> = self
                .mobs
                .iter()
                .filter(|m| m.extra.leash == Some(Holder::Boat(id)))
                .map(|m| m.id)
                .collect();
            let mut held_boats = self.boats_on(Holder::Boat(id));
            if self.boats[i].leash.is_some() {
                held_boats.push(i);
            }
            if !held_mobs.is_empty() || !held_boats.is_empty() {
                for m in held_mobs {
                    self.break_leash(m, true, false);
                }
                self.drop_boat_leashes(&held_boats);
                self.lead_game_event("minecraft:shear", at);
                self.server_play_sound("minecraft:item.shears.snip", at, 1.0, 1.0);
                self.damage_held_tool();
                return true;
            }
        }
        if self.boat_led_by_player(i) {
            // hasInfiniteMaterials: removeLeash, else dropLeash.
            let drop = !self.creative;
            self.release_stored_boat_leash(i, drop);
            self.lead_game_event("minecraft:entity_interact", at);
            self.lead_sound("untied", at);
            return true;
        }
        let taken_by_player = self.boats[i].leash == Some(Holder::Player);
        if item.as_deref() == Some("minecraft:lead") && !taken_by_player {
            let player = self.player_center();
            if self.boat_can_attach(i, player) {
                self.release_stored_boat_leash(i, true);
                let Some(i) = self.boats.iter().position(|boat| boat.id == id) else {
                    return true;
                };
                let player_uuid = self.acting_player_uuid();
                self.boats.update(i, |b| {
                    // LeashData.setLeashHolder forgets a holder still waiting in the save.
                    b.leash = Some(Holder::Player);
                    b.leash_pending = None;
                    b.leash_player = Some(player_uuid);
                });
                self.lead_sound("tied", at);
                // The lead is always used up.
                self.consume_player_slot(self.held_slot(), 1);
                return true;
            }
        }
        false
    }
}
