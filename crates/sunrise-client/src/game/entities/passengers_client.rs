#[allow(unused_imports)]
pub(in crate::game::client_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::passengers::Riders;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::passengers::TRAP_INVULNERABLE;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::passengers::TRAP_LIFETIME;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::passengers::TRAP_RANGE;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::passengers::player_box_top;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::passengers::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::collections::HashMap;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::mobs::{Species, riders};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::{
    geometry::{Aabb, Vec3},
    random::{LegacyRandom, RandomSource},
};

impl Game<'_> {
    pub(in crate::game) fn passenger_riding_position_inner(
        &self,
        vehicle: u32,
        me: Option<u32>,
        own_seat: Option<Vec3>,
        index: &HashMap<u32, usize>,
        path: &mut Vec<u32>,
    ) -> Option<glam::DVec3> {
        if path.len() >= 64 || path.contains(&vehicle) {
            return None;
        }
        path.push(vehicle);
        if Some(vehicle) == me {
            let at = own_seat?;
            return Some(glam::DVec3::new(at.x, at.y, at.z));
        }
        if let Some(v) = index
            .get(&vehicle)
            .map(|&j| &self.mobs[j])
            .or_else(|| self.mobs.iter().find(|m| m.id == vehicle))
        {
            // Another player: the top of its box for the pose its server synced.
            let attachment = if &*v.kind.id == "minecraft:player" {
                [
                    0.0,
                    player_box_top(v.extra.synced_pose, riders::scale(v)),
                    0.0,
                ]
            } else {
                riders::passenger_attachment(v)
            };
            let at = riders::seat(attachment, 0.0, v.position, v.yaw);
            return Some(glam::DVec3::new(at.x, at.y, at.z));
        }
        if let Some(d) = self.text_displays.0.get(&(vehicle as i32)) {
            return if let Some(parent) = d.vehicle {
                self.passenger_riding_position_inner(parent, me, own_seat, index, path)
            } else {
                Some(d.position())
            };
        }
        if let Some(i) = self.client.interactions.get(&(vehicle as i32)) {
            if let Some(parent) = i.vehicle {
                return self.passenger_riding_position_inner(parent, me, own_seat, index, path);
            }
            let at = i.position;
            return Some(glam::DVec3::new(at.x, at.y, at.z));
        }
        if let Some(b) = self.boats.iter().find(|b| b.id == vehicle) {
            let count = self.boat_players(vehicle).max(1) + self.boat_mobs(vehicle).len();
            let at = b.seat(0, count, false, 0.0, false);
            return Some(glam::DVec3::new(at.x, at.y, at.z));
        }
        if let Some(c) = self.carts.iter().find(|c| c.id == vehicle) {
            let at = c
                .position
                .add_vector(Vec3::new(0.0, super::minecarts::SEAT, 0.0));
            return Some(glam::DVec3::new(at.x, at.y, at.z));
        }
        if let Some(a) = self.armor_stands.iter().find(|a| a.id == vehicle) {
            // ArmorStand.getDefaultDimensions: a marker has no box to sit on.
            // The passenger point is the float height widened to double.
            let height = if a.marker {
                0.0
            } else if a.small {
                f64::from(0.9875_f32)
            } else {
                f64::from(1.975_f32)
            };
            let at = if let Some(parent) = a.vehicle {
                self.passenger_riding_position_inner(parent, me, own_seat, index, path)?
            } else {
                glam::DVec3::new(a.position.x, a.position.y, a.position.z)
            };
            return Some(at + glam::DVec3::Y * height);
        }
        if let Some(seat) = self.general_seat(vehicle, false) {
            let at = seat.passenger_position(0.0);
            return Some(glam::DVec3::new(at.x, at.y, at.z));
        }
        // Any other tracked entity sits on its feet; an area effect cloud is
        // 0.5 tall, and ModelEngine parks a model's pivot half a block below
        // its feet for exactly that.
        let remote = self.remote.as_ref()?;
        let (kind, position) = remote.tracked(vehicle as i32)?;
        let position = if let Some(parent) = remote.passenger_vehicle(vehicle) {
            let at = self.passenger_riding_position_inner(parent, me, own_seat, index, path)?;
            [at.x, at.y, at.z]
        } else {
            position
        };
        let lift = if kind == "minecraft:area_effect_cloud" {
            0.5
        } else if kind == "minecraft:player" {
            // A player this client has no copy of yet, so no synced pose: standing.
            player_box_top(0, 1.0)
        } else {
            0.0
        };
        Some(glam::DVec3::new(
            position[0],
            position[1] + lift,
            position[2],
        ))
    }
    /// `Entity.positionRider` for every mob passenger: in its boat, then on
    /// its mob. A server's client places them itself too, as the server sends
    /// no passenger's position.
    pub(in crate::game) fn position_riders(&mut self) {
        let blocked = self.mobs_below_off_level();
        // Every vehicle's riders in one pass over the mobs, not one pass per
        // boat, minecart and primed TNT. Seating moves the riders but changes
        // no one's vehicle, so it holds for all three.
        let mut riders = Riders::default();
        for (j, m) in self.mobs.iter().enumerate() {
            if blocked.contains(&m.id) {
                continue;
            }
            if let Some(v) = m.extra.mount {
                riders.entry(v).or_default().push(j);
            }
        }
        // The general vehicles mobs ride, for the general tick that follows: from this pass's own
        // map, so a level with no general rows pays nothing.
        let mut general_vehicles =
            std::mem::take(&mut self.resident_entities.general.riders.mob_vehicles);
        general_vehicles.clear();
        if !self.resident_entities.general.rows.is_empty() {
            general_vehicles.extend(
                riders
                    .keys()
                    .copied()
                    .filter(|&vehicle| self.general_object(vehicle).is_some()),
            );
        }
        self.resident_entities.general.riders.mob_vehicles = general_vehicles;
        self.seat_boat_mobs(&riders);
        self.seat_cart_mobs(&riders);
        self.seat_tnt_mobs(&riders);
        self.seat_cushion_mobs(&riders);
        self.position_remote_general_passengers();
        self.seat_static_mobs(&riders);
        let index: HashMap<u32, usize> = self
            .mobs
            .iter()
            .enumerate()
            .map(|(i, m)| (m.id, i))
            .collect();
        // Riders of riders settle over a few passes.
        for _ in 0..3 {
            for i in 0..self.mobs.len() {
                if blocked.contains(&self.mobs[i].id) {
                    continue;
                }
                let Some(v) = self.mobs[i].extra.mount else {
                    continue;
                };
                let Some(&j) = index.get(&v) else {
                    // Boats, minecarts, primed TNT and cushions seated their
                    // riders above, each in its own seat and from the
                    // vehicle's drawn position; re-seating them here from the
                    // shared seat 0 yanked boat riders between two places.
                    if self.seats_own_riders(v) || self.general_seat(v, false).is_some() {
                        continue;
                    }
                    // A mob a mod seated on a server player (`native_ride_player.rs`).
                    if self.seat_player_passenger(i, v) {
                        continue;
                    }
                    // A mob on a text display: NPC plugins seat their sitting
                    // NPCs on one. The server's position for a passenger
                    // gives way to its seat, as `rideTick` places it.
                    let me = self.remote.as_ref().map(|r| r.entity_id as u32);
                    let own_seat = self.player.as_ref().map(|p| {
                        p.position
                            .add_vector(Vec3::new(0.0, f64::from(p.dimensions.height), 0.0))
                    });
                    let Some(at) = self.passenger_riding_position(v, me, own_seat, &index) else {
                        continue;
                    };
                    let before = self
                        .text_displays
                        .0
                        .get(&(v as i32))
                        .map(|d| d.seat().1)
                        .unwrap_or_else(|| {
                            let old = self.mobs[i].previous;
                            glam::DVec3::new(
                                old.x,
                                old.y + riders::vehicle_attachment(&self.mobs[i], 0.0),
                                old.z,
                            )
                        });
                    let m = &mut self.mobs[i];
                    // EntityType.TEXT_DISPLAY's box is zero wide.
                    let offset = riders::vehicle_attachment(m, 0.0);
                    // EntityType.TEXT_DISPLAY's box is a point at the seat.
                    let seat = Vec3::new(at.x, at.y, at.z);
                    m.seated(v, offset, Aabb::new(seat, seat));
                    sunrise_client::mobs::living::teleports::set_pos(
                        m,
                        Vec3::new(at.x, at.y - offset, at.z),
                    );
                    m.previous = Vec3::new(before.x, before.y - offset, before.z);
                    m.velocity = Vec3::ZERO;
                    m.fall_distance = 0.0;
                    m.ground.on_ground = false;
                    m.extra.passenger = true;
                    m.ai.nav.stop();
                    continue;
                };
                let vehicle = &self.mobs[j];
                // Camel, happy ghast and content seats follow the rider's place in `getPassengers`.
                let place = if sunrise_client::mobs::camel::is_camel(vehicle.species())
                    || vehicle.species() == sunrise_client::mobs::Species::HappyGhast
                    || vehicle.kind.passenger_attachments.len() > 1
                {
                    self.passenger_place(v, self.mobs[i].id)
                } else {
                    (0, 1)
                };
                let attach = Self::mob_passenger_attachment(
                    vehicle,
                    sunrise_client::mobs::drops::is_animal(self.mobs[i].species()),
                    place.0,
                    place.1,
                );
                let (pos, prev) = (vehicle.position, vehicle.previous);
                let (yaw, prev_yaw) = (vehicle.yaw, vehicle.previous_yaw);
                let (body, prev_body) = (vehicle.body_yaw, vehicle.previous_body_yaw);
                let rider = &self.mobs[i];
                let controlling = riders::controls_vehicle(
                    vehicle,
                    rider,
                    vehicle.rider.is_some() || self.riding == Some(v),
                    self.items().entity_type_tags.has_bit(
                        rider.kind.flags.entity_tag_slot,
                        sunrise_client::items::EntityTypeTags::NON_CONTROLLING_RIDER,
                    ),
                );
                let takes_body = riders::takes_vehicle_body(vehicle, rider, controlling);
                let vehicle_width = vehicle.width();
                let vehicle_box = vehicle.bounds();
                let m = &mut self.mobs[i];
                let offset = riders::vehicle_attachment(m, vehicle_width);
                m.seated(v, offset, vehicle_box);
                sunrise_client::mobs::living::teleports::set_pos(
                    m,
                    riders::seat(attach, offset, pos, yaw),
                );
                m.previous = riders::seat(attach, offset, prev, prev_yaw);
                m.velocity = Vec3::ZERO;
                m.fall_distance = 0.0;
                m.ground.on_ground = false;
                if takes_body {
                    m.body_yaw = body;
                    m.previous_body_yaw = prev_body;
                }
                m.extra.vehicle_body = Some((v, prev_body, body));
                m.extra.passenger = true;
                // A rider never walks off on its own.
                m.ai.nav.stop();
            }
        }
        // General rows riding a mob that was just seated.
        self.position_remote_general_passengers();
        // Displays and interaction boxes riding anything the client tracks,
        // as NPC plugins put a name over a head or a model's bones on a
        // pivot: `Entity.rideTick` -> `positionRider`, the vehicle's passenger
        // attachment (the top of its box in its pose and scale) less the rider's own,
        // which is 0 for both (Display and Interaction are 0 by 0). On the
        // local player, the top of its box in its pose and scale
        // (`Avatar.POSES`).
        let me = self.remote.as_ref().map(|r| r.entity_id as u32);
        let own_seat = self.player.as_ref().map(|p| {
            p.position
                .add_vector(Vec3::new(0.0, f64::from(p.dimensions.height), 0.0))
        });
        // A rider of a rider settles over a few passes, as the mobs above.
        for _ in 0..3 {
            let mut vehicles: Vec<u32> = self
                .text_displays
                .0
                .values()
                .filter_map(|d| d.vehicle)
                .chain(self.client.interactions.values().filter_map(|i| i.vehicle))
                .collect();
            vehicles.sort_unstable();
            vehicles.dedup();
            let seats: HashMap<u32, glam::DVec3> = vehicles
                .into_iter()
                .filter_map(|v| {
                    self.passenger_riding_position(v, me, own_seat, &index)
                        .map(|at| (v, at))
                })
                .collect();
            let sections = self.client.client_pick.sections.get_mut();
            for (&id, display) in &mut self.text_displays.0 {
                if let Some(at) = display.vehicle.and_then(|v| seats.get(&v)) {
                    display.ride(*at);
                    sections.move_id(id as u32, display.position().to_array());
                }
            }
            for interaction in self.client.interactions.values_mut() {
                if let Some(at) = interaction.vehicle.and_then(|v| seats.get(&v)) {
                    interaction.ride_to(*at);
                }
            }
        }
        // Armor stands riding a mob (a server's name tag hologram): seated
        // like the displays above, so the name follows its vehicle instead of
        // staying where the stand spawned. The stand's own vehicle attachment
        // is the default 0, so its feet sit on the seat. The seat at the
        // start of the tick too (`commonTick` runs before `rideTick`), so the
        // stand is drawn between the two even while the level is frozen and
        // its vehicle is not. Those riding the local player are seated after
        // its move (`seat_own_stands`).
        for i in 0..self.armor_stands.len() {
            let Some(vehicle) = self.armor_stands[i].vehicle else {
                continue;
            };
            let Some(at) = self.passenger_riding_position(vehicle, me, own_seat, &index) else {
                continue;
            };
            let before = index
                .get(&vehicle)
                .map(|&j| {
                    let v = &self.mobs[j];
                    riders::seat(
                        riders::passenger_attachment(v),
                        0.0,
                        v.previous,
                        v.previous_yaw,
                    )
                })
                .unwrap_or(self.armor_stands[i].previous);
            let at = Vec3::new(at.x, at.y, at.z);
            let stand = &mut self.armor_stands[i];
            stand.position = at;
            stand.previous = before;
            stand.velocity = Vec3::ZERO;
        }
        self.sync_seated_pick_sections();
    }
}
