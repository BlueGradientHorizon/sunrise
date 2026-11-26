use crate::game::{Game, remote::passengers::passenger_root};
use mod_api::EntityRef;

impl Game<'_, crate::game::client_side::ClientState> {
    /// What the entity rides (`Entity.getVehicle`), if its vehicle has a handle.
    pub(in crate::game) fn entity_vehicle(&self, entity: EntityRef) -> Option<EntityRef> {
        if self.entity(entity.id())? != entity {
            return None;
        }
        let remote = self.side.remote.as_ref()?;
        self.entity(remote.passenger_vehicle(entity.id())?)
    }

    /// Whether the entity rides another object, even one without a handle.
    pub(in crate::game) fn entity_is_passenger(&self, entity: EntityRef) -> Option<bool> {
        if self.entity(entity.id())? != entity {
            return None;
        }
        let remote = self.side.remote.as_ref()?;
        let vehicle = remote.passenger_vehicle(entity.id());
        Some(vehicle.is_some())
    }

    /// The lowest vehicle under the entity (`Entity.getRootVehicle`): the
    /// entity itself when it rides nothing. Off-level intermediaries are traversed;
    /// an off-level root or a cycle returns no handle, never a descendant substitute.
    pub(in crate::game) fn entity_root_vehicle(&self, entity: EntityRef) -> Option<EntityRef> {
        if self.entity(entity.id())? != entity {
            return None;
        }
        let remote = self.side.remote.as_ref()?;
        let root = passenger_root(&remote.passenger_vehicles, entity.id()).ok()?;
        self.entity(root)
    }

    /// The entities riding this one (`Entity.getPassengers`), in `addPassenger`'s order.
    pub(in crate::game) fn entity_passengers(&self, entity: EntityRef) -> Option<Vec<EntityRef>> {
        if self.entity(entity.id())? != entity {
            return None;
        }
        let remote = self.side.remote.as_ref()?;
        Some({
            let mut seats: Vec<_> = remote
                .passenger_seats(entity.id())
                .filter_map(|(id, index)| Some((index, self.entity(id)?)))
                .collect();
            seats.sort_unstable_by_key(|seat| seat.0);
            seats.into_iter().map(|(_, handle)| handle).collect()
        })
    }

    /// Whether any object rides the entity, even one without a handle.
    pub(in crate::game) fn entity_is_vehicle(&self, entity: EntityRef) -> Option<bool> {
        if self.entity(entity.id())? != entity {
            return None;
        }
        let remote = self.side.remote.as_ref()?;
        Some(remote.has_passengers(entity.id()))
    }
}

#[cfg(test)]
pub(in crate::game) mod tests {
    use crate::game::server_side::native_entity_riding::tests::UnleashObserver;
    use crate::game::server_side::native_entity_riding::tests::add_mob;
    use crate::game::server_side::native_entity_riding::tests::take_one_lead;
    use mod_api::{self};
    use std::sync::{Arc, Mutex};
    use sunrise_core::geometry::Vec3;

    #[test]
    #[ignore = "daily"]
    fn mounting_a_leashed_mob_drops_one_lead_without_a_break_sound() {
        let unleashed = Arc::new(Mutex::new(0));
        let mods: mod_api::Mods = vec![mod_api::ModEntry::new(
            "mount_unleash_observer",
            Box::new(UnleashObserver(unleashed.clone())),
        )]
        .into();
        mod_platform::testing::with_mods(mods, || {
            super::super::native_entity_refs::tests::with_game(|game| {
                let at = Vec3::new(0.5, 64.0, 0.5);
                let cow = add_mob(game, 160, "minecraft:cow", at);
                let vehicle = add_mob(game, 161, "minecraft:camel", at);
                let holder = add_mob(game, 162, "minecraft:sheep", at);
                assert!(cow.leash_to(game, holder));
                game.start_audio_capture();

                // A refused mount must not consume the leash.
                assert!(cow.start_riding(game, cow, true).is_err());
                assert_eq!(
                    cow.leash_holder(game),
                    Some(mod_api::LeashHolder::Entity(holder))
                );
                assert!(game.dropped.is_empty());
                assert_eq!(*unleashed.lock().unwrap(), 0);

                assert_eq!(cow.start_riding(game, vehicle, true), Ok(()));
                assert_eq!(cow.vehicle(game), Some(vehicle));
                assert_eq!(
                    cow.leash_holder(game),
                    Some(mod_api::LeashHolder::Unleashed)
                );
                let seated = game.mob_object(160).unwrap().position;
                take_one_lead(game, seated);
                assert_eq!(*unleashed.lock().unwrap(), 1);
                assert!(game.side.pending_sounds.is_empty());
            });
        });
    }

    #[test]
    #[ignore = "daily"]
    fn natural_boarding_drops_a_lead_only_after_a_successful_mount() {
        use sunrise_sim::mobs::MobEvent;

        let unleashed = Arc::new(Mutex::new(0));
        let mods: mod_api::Mods = vec![mod_api::ModEntry::new(
            "natural_mount_unleash_observer",
            Box::new(UnleashObserver(unleashed.clone())),
        )]
        .into();
        mod_platform::testing::with_mods(mods, || {
            super::super::native_entity_refs::tests::with_game(|game| {
                let at = Vec3::new(0.5, 64.0, 0.5);
                let cow = add_mob(game, 160, "minecraft:cow", at);
                let vehicle = add_mob(game, 161, "minecraft:camel", at);
                let holder = add_mob(game, 162, "minecraft:sheep", at);
                assert!(cow.leash_to(game, holder));
                game.start_audio_capture();

                game.mob_object_mut(160).unwrap().extra.boarding_cooldown = 1;
                game.handle_mob_events(vec![MobEvent::StartRiding {
                    rider: 160,
                    vehicle: 161,
                }])
                .unwrap();
                assert_eq!(cow.vehicle(game), None);
                assert_eq!(
                    cow.leash_holder(game),
                    Some(mod_api::LeashHolder::Entity(holder))
                );
                assert!(game.dropped.is_empty());
                assert_eq!(*unleashed.lock().unwrap(), 0);

                game.mob_object_mut(160).unwrap().extra.boarding_cooldown = 0;
                game.handle_mob_events(vec![MobEvent::StartRiding {
                    rider: 160,
                    vehicle: 161,
                }])
                .unwrap();
                assert_eq!(cow.vehicle(game), Some(vehicle));
                assert_eq!(
                    cow.leash_holder(game),
                    Some(mod_api::LeashHolder::Unleashed)
                );
                take_one_lead(game, at);
                assert_eq!(*unleashed.lock().unwrap(), 1);
                assert!(game.side.pending_sounds.is_empty());
            });
        });
    }
}
