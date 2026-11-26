use sunrise_gameplay_common::game::{Game, remote::passengers::passenger_root};
use mod_api::EntityRef;

impl crate::game::ClientGame<'_> {
    /// What the entity rides (`Entity.getVehicle`), if its vehicle has a handle.
    pub(in crate::game) fn entity_vehicle(&self, entity: EntityRef) -> Option<EntityRef> {
        if self.common.entity(entity.id())? != entity {
            return None;
        }
        let remote = self.client.remote.as_ref()?;
        self.common.entity(remote.passenger_vehicle(entity.id())?)
    }

    /// Whether the entity rides another object, even one without a handle.
    pub(in crate::game) fn entity_is_passenger(&self, entity: EntityRef) -> Option<bool> {
        if self.common.entity(entity.id())? != entity {
            return None;
        }
        let remote = self.client.remote.as_ref()?;
        let vehicle = remote.passenger_vehicle(entity.id());
        Some(vehicle.is_some())
    }

    /// The lowest vehicle under the entity (`Entity.getRootVehicle`): the
    /// entity itself when it rides nothing. Off-level intermediaries are traversed;
    /// an off-level root or a cycle returns no handle, never a descendant substitute.
    pub(in crate::game) fn entity_root_vehicle(&self, entity: EntityRef) -> Option<EntityRef> {
        if self.common.entity(entity.id())? != entity {
            return None;
        }
        let remote = self.client.remote.as_ref()?;
        let root = passenger_root(&remote.passenger_vehicles, entity.id()).ok()?;
        self.common.entity(root)
    }

    /// The entities riding this one (`Entity.getPassengers`), in `addPassenger`'s order.
    pub(in crate::game) fn entity_passengers(&self, entity: EntityRef) -> Option<Vec<EntityRef>> {
        if self.common.entity(entity.id())? != entity {
            return None;
        }
        let remote = self.client.remote.as_ref()?;
        Some({
            let mut seats: Vec<_> = remote
                .passenger_seats(entity.id())
                .filter_map(|(id, index)| Some((index, self.common.entity(id)?)))
                .collect();
            seats.sort_unstable_by_key(|seat| seat.0);
            seats.into_iter().map(|(_, handle)| handle).collect()
        })
    }

    /// Whether any object rides the entity, even one without a handle.
    pub(in crate::game) fn entity_is_vehicle(&self, entity: EntityRef) -> Option<bool> {
        if self.common.entity(entity.id())? != entity {
            return None;
        }
        let remote = self.client.remote.as_ref()?;
        Some(remote.has_passengers(entity.id()))
    }
}
