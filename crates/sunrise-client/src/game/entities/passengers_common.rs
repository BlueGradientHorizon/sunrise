//! Common passenger attachment and seating helpers shared by server and client.
#[allow(unused_imports)]
use super::Game;
#[allow(unused_imports)]
use crate::game::server_side::{passengers::Riders, tnt::tnt_seat};
#[allow(unused_imports)]
use std::collections::HashMap;
#[allow(unused_imports)]
use sunrise_client::{
    camera::wrap_degrees,
    mobs::{drops::is_animal, riders},
};
#[allow(unused_imports)]
use sunrise_core::geometry::Vec3;

impl<__Side: crate::game::Side> Game<'_, __Side> {
    /// The vehicle's attachment for this passenger's actual place in `getPassengers`.
    pub(in crate::game) fn mob_passenger_attachment(
        vehicle: &sunrise_client::mobs::Mob,
        rider_is_animal: bool,
        seat: usize,
        count: usize,
    ) -> [f64; 3] {
        if sunrise_client::mobs::camel::is_camel(vehicle.species()) {
            sunrise_client::mobs::camel::passenger_attachment_at(
                vehicle,
                seat,
                count,
                rider_is_animal,
            )
        } else {
            riders::passenger_attachment_at(vehicle, seat)
        }
    }

    /// A vehicle whose `seat_*_mobs` pass places its mob riders itself.
    pub(in crate::game) fn seats_own_riders(&self, vehicle: u32) -> bool {
        self.boats.iter().any(|b| b.id == vehicle)
            || self.carts.iter().any(|c| c.id == vehicle)
            || self.cushions.iter().any(|c| c.id == vehicle)
            || self.tnts.iter().any(|t| t.id == vehicle && t.id != 0)
    }

    /// `Entity.positionRider` for mobs riding primed TNT.
    pub(in crate::game) fn seat_tnt_mobs(&mut self, riders: &Riders) {
        for i in 0..self.tnts.len() {
            let (id, pos, prev) = (
                self.tnts[i].id,
                self.tnts[i].position,
                self.tnts[i].previous,
            );
            let Some(js) = riders.get(&id).filter(|_| id != 0) else {
                continue;
            };
            let tnt_box = self.tnts[i].bounds();
            for &j in js {
                let m = &mut self.mobs[j];
                // Primed TNT's box is 0.98 wide.
                let offset = riders::vehicle_attachment(m, 0.98);
                m.seated(id, offset, tnt_box);
                sunrise_client::mobs::living::teleports::set_pos(m, tnt_seat(pos, offset));
                m.previous = tnt_seat(prev, offset);
                m.velocity = Vec3::ZERO;
                m.fall_distance = 0.0;
                m.ground.on_ground = false;
                m.extra.passenger = true;
                m.ai.nav.stop();
            }
        }
    }

    /// `Entity.positionRider` for mobs sitting on a cushion.
    pub(in crate::game) fn seat_cushion_mobs(&mut self, riding: &Riders) {
        for c in &self.cushions {
            let Some(js) = riding.get(&c.id) else {
                continue;
            };
            for &j in js {
                let m = &mut self.mobs[j];
                // Cushion's box is 1 wide.
                let offset = riders::vehicle_attachment(m, 1.0);
                let at = Vec3::new(c.position.x, c.position.y + 0.25 - offset, c.position.z);
                sunrise_client::mobs::living::teleports::snap_to(m, at);
                m.velocity = Vec3::ZERO;
                m.fall_distance = 0.0;
                m.ground.on_ground = false;
                m.extra.passenger = true;
                m.ai.nav.stop();
            }
        }
    }

    /// Shared per-boat mob seating loop parameterized by the side's passenger order reader.
    pub(in crate::game) fn seat_boat_mobs_with_order(
        &mut self,
        riding: &Riders,
        order_of: impl Fn(&Game<'_, __Side>, u32) -> Vec<u32>,
    ) {
        for i in 0..self.boats.len() {
            let id = self.boats[i].id;
            let mobs: Vec<usize> = riding
                .get(&id)
                .into_iter()
                .flatten()
                .copied()
                .filter(|&j| !self.mobs[j].removed && self.mobs[j].alive())
                .collect();
            if mobs.is_empty() {
                continue;
            }
            let order = order_of(self, id);
            let count = order.len().max(mobs.len());
            let b = &self.boats[i];
            let full = count == b.max_passengers();
            let delta = b.delta_rotation;
            let boat_box = b.bounds();
            let ridden = sunrise_client::motion::RiddenBoat {
                under_water: b.under_water(),
                max_y: boat_box.max_position().y,
            };
            let seats: Vec<_> = mobs
                .iter()
                .enumerate()
                .map(|(n, &j)| {
                    let m = &self.mobs[j];
                    let animal = is_animal(m.species());
                    // AbstractBoat's box is 1.375 wide.
                    let offset = riders::vehicle_attachment(m, 1.375);
                    let index = order.iter().position(|&rider| rider == m.id).unwrap_or(n);
                    let turn = match (animal && full, m.id.is_multiple_of(2)) {
                        (false, _) => 0.0,
                        (true, true) => 90.0,
                        (true, false) => 270.0,
                    };
                    let turns = !self.items().entity_type_tags.has_bit(
                        m.kind.flags.entity_tag_slot,
                        sunrise_client::items::EntityTypeTags::CAN_TURN_IN_BOATS,
                    );
                    (
                        j,
                        offset,
                        b.seat(index, count, animal, offset, false),
                        b.seat(index, count, animal, offset, true),
                        turns.then_some((b.yaw + turn, b.previous_yaw + turn)),
                    )
                })
                .collect();
            for (j, offset, at, previous, facing) in seats {
                let m = &mut self.mobs[j];
                m.seated(id, offset, boat_box);
                m.fluids.boat = Some((id, ridden));
                sunrise_client::mobs::living::teleports::set_pos(m, at);
                m.previous = previous;
                m.velocity = Vec3::ZERO;
                m.fall_distance = 0.0;
                m.ground.on_ground = false;
                m.extra.passenger = true;
                m.ai.nav.stop();
                let Some((body, previous_body)) = facing else {
                    continue;
                };
                m.head_yaw += delta;
                m.body_yaw = body;
                m.previous_body_yaw = previous_body;
                m.yaw = body;
                m.previous_yaw = previous_body;
                let limit = m.kind.max_head_y_rot;
                let d = wrap_degrees(body - m.head_yaw);
                m.head_yaw += d - d.clamp(-limit, limit);
            }
        }
    }

    /// `Entity.getPassengerRidingPosition` of a vehicle.
    pub(in crate::game) fn passenger_riding_position(
        &self,
        vehicle: u32,
        me: Option<u32>,
        own_seat: Option<Vec3>,
        index: &HashMap<u32, usize>,
    ) -> Option<glam::DVec3> {
        self.side_passenger_riding_position(vehicle, me, own_seat, index)
    }

    /// `Entity.positionRider` for every passenger on the active side.
    pub(in crate::game) fn position_riders(&mut self) {
        self.side_position_riders();
    }

    /// `Entity.positionRider` for a general passenger, whose own vehicle attachment is zero.
    pub(in crate::game) fn general_passenger_position(
        &self,
        vehicle: u32,
        rider: u32,
    ) -> Option<Vec3> {
        self.side_general_passenger_position(vehicle, rider)
    }
}
