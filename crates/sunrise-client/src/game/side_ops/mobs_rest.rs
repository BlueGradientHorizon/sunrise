//! Two-arm side dispatchers owned by lane `mobs-rest`.
use super::Game;
use crate::game::{boats::Boat, collision_source::Mover, commands::Target};
use std::collections::HashMap;
use sunrise_client::{
    entities::ItemEntity,
    entity_solids::{Candidate, Solids},
    items::ItemStack,
    motion::BodyRestitution,
};
use sunrise_core::{
    geometry::{Aabb, Vec3},
    position::BlockPos,
};
use sunrise_world::ground_support::GroundState;

impl Game<'_> {
    pub(in crate::game) fn side_passenger_riding_position(
        &self,
        vehicle: u32,
        me: Option<u32>,
        own_seat: Option<Vec3>,
        index: &HashMap<u32, usize>,
    ) -> Option<glam::DVec3> {
        if self.is_logical_server() {
            self.server_passenger_riding_position(vehicle, me, own_seat, index)
        } else {
            self.client_passenger_riding_position(vehicle, me, own_seat, index)
        }
    }

    pub(in crate::game) fn side_position_riders(&mut self) {
        if self.is_logical_server() {
            self.server_position_riders();
        } else {
            self.client_position_riders();
        }
    }

    pub(in crate::game) fn side_general_passenger_position(
        &self,
        vehicle: u32,
        rider: u32,
    ) -> Option<Vec3> {
        if self.is_logical_server() {
            self.server_general_passenger_position(vehicle, rider)
        } else {
            self.client_general_passenger_position(vehicle, rider)
        }
    }

    pub(in crate::game) fn side_play_player_bound_sound(
        &mut self,
        event: &str,
        source: usize,
        volume: f32,
        pitch: f32,
        except_actor: bool,
    ) {
        if self.is_logical_server() {
            self.server_play_player_bound_sound(event, source, volume, pitch, except_actor);
        } else {
            self.client_play_player_bound_sound(event, source, volume, pitch, except_actor);
        }
    }

    pub(in crate::game) fn side_player_equipped(&mut self, slot: usize, new: Option<&ItemStack>) {
        if self.is_logical_server() {
            self.server_player_equipped(slot, new);
        } else {
            self.client_player_equipped(slot, new);
        }
    }

    pub(in crate::game) fn side_boat_can_simulate_movement(&self, id: u32) -> bool {
        if self.is_logical_server() {
            self.server_boat_can_simulate_movement(id)
        } else {
            self.client_boat_can_simulate_movement(id)
        }
    }

    pub(in crate::game) fn side_boat_has_room(&self, i: usize) -> bool {
        if self.is_logical_server() {
            self.server_boat_has_room(i)
        } else {
            self.client_boat_has_room(i)
        }
    }

    pub(in crate::game) fn side_player_boat_seat(&self, i: usize) -> Vec3 {
        if self.is_logical_server() {
            self.server_player_boat_seat(i)
        } else {
            self.client_player_boat_seat(i)
        }
    }

    pub(in crate::game) fn side_tick_boat_leash(&mut self, b: &mut Boat) {
        if self.is_logical_server() {
            self.server_tick_boat_leash(b);
        } else {
            self.client_tick_boat_leash(b);
        }
    }

    pub(in crate::game) fn side_finish_body_restitution(
        &mut self,
        entity: (u32, &str),
        feet: Vec3,
        response: BodyRestitution,
    ) -> Vec3 {
        if self.is_logical_server() {
            self.server_finish_body_restitution(entity, feet, response)
        } else {
            self.client_finish_body_restitution(entity, feet, response)
        }
    }

    pub(in crate::game) fn side_body_step_on(
        &mut self,
        entity: (u32, &str),
        footprint: (Vec3, Aabb),
        ground: &GroundState,
        velocity: &mut Vec3,
    ) -> Vec<mod_api::EntityWrite> {
        if self.is_logical_server() {
            self.server_body_step_on(entity, footprint, ground, velocity)
        } else {
            self.client_body_step_on(entity, footprint, ground, velocity)
        }
    }

    pub(in crate::game) fn side_item_fall_on(&mut self, id: u32, on: BlockPos, distance: f64) {
        if self.is_logical_server() {
            self.server_item_fall_on(id, on, distance);
        } else {
            self.client_item_fall_on(id, on, distance);
        }
    }

    pub(in crate::game) fn side_item_post_move_contacts(
        &mut self,
        item: &mut ItemEntity,
        client: bool,
    ) -> bool {
        if self.is_logical_server() {
            self.server_item_post_move_contacts(item, client)
        } else {
            self.client_item_post_move_contacts(item, client)
        }
    }

    pub(in crate::game) fn side_boat_collision_root(&self, id: u32, target: Target) -> Option<u32> {
        if self.is_logical_server() {
            self.server_boat_collision_root(id, target)
        } else {
            self.client_boat_collision_root(id, target)
        }
    }

    pub(in crate::game) fn side_vehicle_solids(
        &self,
        mover: Mover<'_>,
        solids: &Solids,
        each: &mut dyn FnMut(&Candidate, Option<usize>),
    ) {
        if self.is_logical_server() {
            self.server_vehicle_solids(mover, solids, each);
        } else {
            self.client_vehicle_solids(mover, solids, each);
        }
    }

    pub(in crate::game) fn side_resident_dragon_part_index(&self, id: u32) -> Option<usize> {
        if self.is_logical_server() {
            self.server_resident_dragon_part_index(id)
        } else {
            self.client_resident_dragon_part_index(id)
        }
    }

    pub(in crate::game) fn side_entity_in(&self, room: Aabb) -> bool {
        if self.is_logical_server() {
            self.server_entity_in(room)
        } else {
            self.client_entity_in(room)
        }
    }

    pub(in crate::game) fn side_pick_knot_fence(&self, id: u32) -> Option<[i32; 3]> {
        if self.is_logical_server() {
            self.server_pick_knot_fence(id)
        } else {
            self.client_pick_knot_fence(id)
        }
    }

    pub(in crate::game) fn side_entity_is_taken(&self, id: u32) -> bool {
        if self.is_logical_server() {
            self.server_entity_is_taken(id)
        } else {
            self.client_entity_is_taken(id)
        }
    }

    pub(in crate::game) fn side_own_passenger_index(&self, mob_id: u32) -> usize {
        if self.is_logical_server() {
            self.server_own_passenger_index(mob_id)
        } else {
            self.client_own_passenger_index(mob_id)
        }
    }
}
