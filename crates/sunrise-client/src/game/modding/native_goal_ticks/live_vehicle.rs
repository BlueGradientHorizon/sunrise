//! Passenger reads over the mob tick's disjoint live entity loan.
use super::GoalCx;
use crate::{
    game::{boarding::next_boarded_rider, native_entity_riding::visit_passenger_tree},
    mobs::{LiveMobs, Mob, part_goals::GoalHost},
};
use mod_api::{EntityReadError, EntityRef, LivingRef};
use std::ops::ControlFlow;

impl GoalCx<'_, '_> {
    pub(super) fn r_entity_dismount_location(
        &self,
        vehicle: EntityRef,
        passenger: LivingRef,
    ) -> Option<[f64; 3]> {
        if self.lookup_entity(vehicle.id())? != vehicle
            || self.lookup_living(passenger.entity()).is_none()
        {
            return None;
        }
        self.w.goal_host?.mob_dismount_location(
            self.entity_mob(vehicle),
            vehicle.id(),
            self.entity_mob(passenger.entity()),
            passenger.entity(),
        )
    }
    /// Resolve vehicle links through the loan before asking the game for an
    /// entity that remains in place (such as a player or a boat).
    fn scoped_root_vehicle(&self, entity: EntityRef) -> Option<EntityRef> {
        if self.lookup_entity(entity.id())? != entity {
            return None;
        }
        let mut root = entity;
        for _ in 0..64 {
            let vehicle = if self.entity_mob(root).is_some() {
                self.r_entity_vehicle(root)
            } else {
                mod_api::__private::backend_read(self.host, |backend| backend.entity_vehicle(root))
                    .flatten()
            };
            match vehicle {
                Some(vehicle) => root = vehicle,
                None => break,
            }
        }
        Some(root)
    }

    pub(super) fn r_entity_root_vehicle(&self, entity: EntityRef) -> Option<EntityRef> {
        self.scoped_root_vehicle(entity)
    }

    pub(super) fn r_entity_is_passenger_of_same_vehicle(
        &self,
        entity: EntityRef,
        other: EntityRef,
    ) -> Option<bool> {
        Some(self.scoped_root_vehicle(entity)? == self.scoped_root_vehicle(other)?)
    }

    fn passenger_loan(
        &self,
        entity: EntityRef,
    ) -> Result<(&dyn GoalHost, &dyn LiveMobs), EntityReadError> {
        let host = self.w.goal_host.ok_or(EntityReadError::Unavailable)?;
        self.live_entity_for_read(entity)?;
        Ok((host, self.w.live_mobs.ok_or(EntityReadError::Unavailable)?))
    }

    fn next_scoped_rider(
        &self,
        host: &dyn GoalHost,
        live: &dyn LiveMobs,
        vehicle: u32,
        after: Option<u32>,
    ) -> Option<u32> {
        if vehicle != self.m.id && live.find(vehicle).is_none() {
            return host.next_passenger_seat(vehicle, after);
        }
        next_boarded_rider(after, |visit| {
            let mut found = false;
            host.visit_player_passenger_seats(vehicle, &mut |place, id| {
                found = visit(place, id);
                found
            });
            let mut seat = |mob: &Mob| {
                if !found && mob.extra.mount == Some(vehicle) && !mob.removed && mob.alive() {
                    found = visit(mob.extra.vehicle_order, mob.id);
                }
            };
            seat(self.m);
            live.for_each(&mut seat);
        })
    }

    pub(super) fn r_entity_visit_passengers(
        &self,
        entity: EntityRef,
        visit: &mut dyn FnMut(EntityRef) -> ControlFlow<()>,
    ) -> Result<ControlFlow<()>, EntityReadError> {
        let (host, live) = self.passenger_loan(entity)?;
        let mut after = None;
        Ok(std::iter::from_fn(|| {
            let id = self.next_scoped_rider(host, live, entity.id(), after)?;
            after = Some(id);
            Some(id)
        })
        .filter_map(|id| self.lookup_entity(id))
        .try_for_each(visit))
    }

    pub(super) fn r_entity_is_vehicle(&self, entity: EntityRef) -> Option<bool> {
        self.r_entity_visit_passengers(entity, &mut |_| ControlFlow::Break(()))
            .ok()
            .map(|result| result.is_break())
    }

    pub(super) fn r_entity_visit_indirect_passengers(
        &self,
        entity: EntityRef,
        visit: &mut dyn FnMut(EntityRef) -> ControlFlow<()>,
    ) -> Result<ControlFlow<()>, EntityReadError> {
        let (host, live) = self.passenger_loan(entity)?;
        Ok(visit_passenger_tree(
            entity.id(),
            |vehicle, after| self.next_scoped_rider(host, live, vehicle, after),
            |rider| {
                if rider == self.m.id {
                    self.m.extra.mount
                } else if let Some(mob) = live.find(rider) {
                    mob.extra.mount
                } else {
                    host.passenger_vehicle(rider)
                }
            },
            |rider| {
                self.lookup_entity(rider)
                    .map_or(ControlFlow::Continue(()), &mut *visit)
            },
        ))
    }

    pub(super) fn r_entity_controlling_passenger(&self, entity: EntityRef) -> Option<LivingRef> {
        let vehicle = self.entity_mob(entity)?;
        let mut first = None;
        let _ = self
            .r_entity_visit_passengers(entity, &mut |rider| {
                first = Some(rider);
                ControlFlow::Break(())
            })
            .ok()?;
        let first = first?;
        if let Some(rider) = self.entity_mob(first) {
            let tagged = self
                .w
                .data
                .entity_has("minecraft:non_controlling_rider", &rider.kind.id);
            return (!vehicle.no_ai && !tagged).then(|| LivingRef::new(first));
        }
        // The game only lets a player control this kind when the relevant
        // saddle, steering item or harness is present.
        if self.host.mob(first).is_some() || self.host.living(first).is_none() {
            return None;
        }
        self.w
            .goal_host?
            .mob_player_controls(vehicle, first.id())
            .then(|| LivingRef::new(first))
    }
}
