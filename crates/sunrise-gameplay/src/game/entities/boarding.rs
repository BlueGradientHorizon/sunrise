//! A player's `Entity.boardingCooldown` is always 0: `Player.removeVehicle`
//! zeroes the 60 ticks `Entity.removePassenger` just set (#11158), so
//! `Entity.canRide` only asks that the player is not sneaking, which each
//! vehicle's interaction checks. Mobs keep theirs in `extra.boarding_cooldown`
//! (`boat_passengers::BOARDING_COOLDOWN`).
use crate::game::ServerGame;
use sunrise_gameplay_common::game::entity_memory::OffLevel;
use sunrise_sim::mobs::Mob;

/// `Entity.getIndirectPassengersStream` for `vehicle`: the vehicle and every
/// vehicle riding it, directly or through another rider. `mounts` holds
/// `(vehicle, its vehicle)` for every mob.
pub(in crate::game::server_side) fn passenger_vehicles(
    vehicle: u32,
    mounts: &[(u32, Option<u32>)],
) -> Vec<u32> {
    let mut ids = vec![vehicle];
    let mut i = 0;
    while i < ids.len() {
        let v = ids[i];
        i += 1;
        for &(id, mount) in mounts {
            if mount == Some(v) && !ids.contains(&id) {
                ids.push(id);
            }
        }
    }
    ids
}

impl ServerGame<'_> {
    /// `Entity.passengers` of `vehicle`, first passenger first: each one's place, id and whether
    /// it is a player. Places tie only for those that never boarded through `addPassenger` (a
    /// load keeps its list order), players ahead.
    pub(in crate::game) fn seated_passengers(&self, vehicle: u32) -> Vec<(i64, u32, bool)> {
        let mut seats = Vec::new();
        self.any_passenger_seat(vehicle, &[], |seat| {
            seats.push(seat);
            false
        });
        seats.sort_by_key(|&(place, ..)| place);
        seats
    }

    /// `Entity.isVehicle`: any kind of rider makes this a vehicle.
    pub(in crate::game) fn has_seated_passengers(&self, vehicle: u32) -> bool {
        self.has_seated_passengers_with_lent_mobs(vehicle, &[])
    }

    /// The same check while the mob tick holds the resident list out of `Game`.
    pub(in crate::game) fn has_seated_passengers_with_lent_mobs(
        &self,
        vehicle: u32,
        lent_mobs: &[Mob],
    ) -> bool {
        self.any_passenger_seat(vehicle, lent_mobs, |_| true)
    }

    /// Shared seat enumeration; visit only until `matches` returns true. The
    /// enumeration order breaks ties for passengers with the same seat order.
    fn any_passenger_seat(
        &self,
        vehicle: u32,
        lent_mobs: &[Mob],
        mut matches: impl FnMut((i64, u32, bool)) -> bool,
    ) -> bool {
        let acting = sunrise_gameplay_common::game::Side::lan_acting(&self.server).filter(|_| self.common.player.is_some());
        let mut acting_listed = false;
        for g in self
            .server
            .lan
            .iter()
            .flat_map(|h| h.hub.guests.iter())
            .filter(|g| g.placed && g.dimension == self.common.dim.id())
        {
            let rides = if acting == Some(g.conn) {
                acting_listed = true;
                self.common.riding
            } else {
                g.vehicle
            };
            if rides == Some(vehicle) && matches((g.vehicle_order, g.entity_id as u32, true)) {
                return true;
            }
        }
        // An unlisted acting guest goes first. This game's own player keeps its seat order.
        if let Some((me, place)) = self.common.riding_player()
            && !acting_listed
            && self.common.player.is_some()
            && self.common.riding == Some(vehicle)
            && matches((place, me, true))
        {
            return true;
        }
        let other_mobs = lent_mobs
            .iter()
            .chain(self.common.persist.off_level.values().filter_map(OffLevel::as_mob))
            .filter(|m| m.extra.mount == Some(vehicle));
        self.resident_mob_riders(vehicle)
            .map(|(_, m)| m)
            .chain(other_mobs)
            .filter(|m| !m.removed)
            .any(|m| matches((m.extra.vehicle_order, m.id, false)))
            || self
                .common.general_passengers(vehicle)
                .any(|row| matches((row.vehicle_order, row.id, false)))
            || self
                .common.bolts
                .iter()
                .filter(|b| b.mount == Some(vehicle))
                .any(|b| matches((b.vehicle_order, b.id, false)))
    }

    /// `Entity.startRiding` for the acting player: the rider link, then
    /// `CriteriaTriggers.START_RIDING_TRIGGER` for every server player the
    /// vehicle carries.
    pub(in crate::game) fn start_riding(&mut self, vehicle: u32) {
        self.common.riding = Some(vehicle);
        self.seat_player(vehicle);
        self.note_started_riding(vehicle);
    }

    /// The first seat, preserving enumeration order on a tie without sorting or allocating.
    pub(in crate::game::server_side) fn first_seated_passenger(
        &self,
        vehicle: u32,
    ) -> Option<(i64, u32, bool)> {
        let mut first: Option<(i64, u32, bool)> = None;
        self.any_passenger_seat(vehicle, &[], |seat| {
            if first.is_none_or(|old| seat.0 < old.0) {
                first = Some(seat);
            }
            false
        });
        first
    }

    /// `getPassengers().size()` without allocating or sorting the seats.
    pub(in crate::game) fn seated_passenger_count(&self, vehicle: u32) -> usize {
        let mut count = 0;
        self.any_passenger_seat(vehicle, &[], |_| {
            count += 1;
            false
        });
        count
    }

    /// `Entity.addPassenger`: the place of `rider` getting on `vehicle`. On a server a player
    /// goes ahead of a first passenger that is no player; everything else goes last.
    pub(in crate::game) fn boarding_order(&self, vehicle: u32, rider: u32, player: bool) -> i64 {
        let mut aboard = self.seated_passengers(vehicle);
        aboard.retain(|&(_, id, _)| id != rider);
        match (aboard.first(), aboard.last()) {
            (Some(&(first, _, false)), _) if player => first - 1,
            (_, Some(&(last, ..))) => last + 1,
            _ => 0,
        }
    }
    /// `addPassenger` for the mob `id`, linked to its vehicle just now.
    pub(in crate::game) fn seat_mob(&mut self, id: u32) {
        let mount = self.common.mob_object(id);
        let Some(vehicle) = mount.and_then(|m| m.extra.mount) else {
            return;
        };
        let order = self.boarding_order(vehicle, id, false);
        if let Some(m) = self.mob_object_mut(id) {
            m.extra.vehicle_order = order;
        }
    }
    /// `addPassenger` for the player riding through `riding`, now in `vehicle`: the acting
    /// guest's place goes to the hub, this game's own player's to `riding_order`.
    pub(in crate::game::server_side) fn seat_player(&mut self, vehicle: u32) {
        match (self.server.lan_acting, self.server.lan_acting_entity) {
            (Some(conn), Some(me)) => {
                let order = self.boarding_order(vehicle, me, true);
                if let Some(host) = self.server.lan.as_mut() {
                    host.hub.update_guest(conn, |g| g.vehicle_order = order);
                }
            }
            (None, _) => {
                if let Some((me, _)) = self.common.riding_player() {
                    self.server.riding_order = self.boarding_order(vehicle, me, true);
                }
            }
            _ => {}
        }
    }

    /// A rider link changed on `vehicle` (`sendEventAndTriggers`). Every
    /// player aboard hears the trigger; a host's turn can fire it now, while
    /// a guest's turn (the hub held by its action) leaves it for
    /// [`Self::flush_started_riding`] once every player is back in place.
    pub(in crate::game) fn note_started_riding(&mut self, vehicle: u32) {
        self.server.started_riding.push(vehicle);
        if self.server.lan.is_some() && self.server.lan_acting.is_none() {
            self.flush_started_riding();
        }
    }

    /// Fires the queued `started_riding` triggers with every player in place.
    pub(in crate::game::server_side) fn flush_started_riding(&mut self) {
        for vehicle in std::mem::take(&mut self.server.started_riding) {
            self.trigger_started_riding(vehicle);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    

    

    

    #[test]
    fn started_riding_reaches_every_rider_of_the_vehicle() {
        // Entity.startRiding: START_RIDING_TRIGGER for every ServerPlayer in
        // entityToRide.getIndirectPassengersStream(). A mob between two
        // riders is still part of that stream, and another vehicle is not.
        let mounts = [(7, Some(5)), (8, Some(7)), (9, Some(4))];
        let aboard = passenger_vehicles(5, &mounts);
        assert!(aboard.contains(&5), "the rider on the vehicle itself");
        assert!(aboard.contains(&7), "the mob in the vehicle's seat");
        assert!(aboard.contains(&8), "the rider of that mob");
        assert!(!aboard.contains(&9), "a mob riding another vehicle");
        assert!(!aboard.contains(&6), "a mob riding nothing");
    }
}
