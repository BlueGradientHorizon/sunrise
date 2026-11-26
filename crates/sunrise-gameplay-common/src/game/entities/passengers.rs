

#[cfg(any(test, feature = "test-support"))]
pub mod nested_vehicle_discard_tests {

use super::*;

use crate::game::native_entity_refs::tests::{collect_all_entities, collect_entities_in};

use std::sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    };

#[cfg(test)]
struct DiscardVehicleOnDismount(Arc<AtomicUsize>);
#[cfg(test)]
#[mod_api::__subscriptions]
    impl mod_api::Hooks for DiscardVehicleOnDismount {
        fn entity_mount(
            &self,
            host: &mut dyn mod_api::ServerHost,
            event: &mut mod_api::EntityMountEvent<'_>,
        ) -> mod_api::Flow {
            assert!(!event.mounting);
            assert_eq!(event.entity, 72);
            self.0.fetch_add(1, Ordering::Relaxed);
            // The nested notification sees the root already logically removed.
            // Do not recurse deliberately or hold a lock across a host call.
            if let Some(vehicle) = host.entity(event.vehicle) {
                assert!(vehicle.discard(host));
            }
            mod_api::Flow::Cancel
        }
    }
#[cfg(test)]
#[mod_api::__subscriptions]
    impl mod_api::Hooks for CancelUntilAllowed {
        fn entity_mount(
            &self,
            _: &mut dyn mod_api::ServerHost,
            event: &mut mod_api::EntityMountEvent<'_>,
        ) -> mod_api::Flow {
            if !event.mounting && event.entity == 72 {
                self.calls.fetch_add(1, Ordering::Relaxed);
                if self.cancel.load(Ordering::Relaxed) {
                    return mod_api::Flow::Cancel;
                }
            }
            mod_api::Flow::Continue
        }
    }
#[cfg(test)]
struct CancelPassengerTick(Arc<AtomicBool>);
#[cfg(test)]
#[mod_api::__subscriptions]
    impl mod_api::Hooks for CancelPassengerTick {
        fn entity_tick_listened(
            &self,
            _: mod_api::HostSide<'_>,
            event: &mut mod_api::EntityTickListenedEvent,
        ) {
            event.listened = true;
        }
        fn entity_tick_pre(
            &self,
            _: mod_api::HostSide<'_>,
            event: &mut mod_api::EntityTickPreEvent<'_>,
        ) {
            if event.entities.contains(&72) && self.0.load(Ordering::Relaxed) {
                event.canceled.push(72);
            }
        }
    }
#[cfg(test)]
#[mod_api::__subscriptions]
    impl mod_api::Hooks for CancelPlayerDismount {
        fn entity_mount(
            &self,
            _: &mut dyn mod_api::ServerHost,
            event: &mut mod_api::EntityMountEvent<'_>,
        ) -> mod_api::Flow {
            if !event.mounting && event.vehicle == 71 {
                self.calls.fetch_add(1, Ordering::Relaxed);
                if self.cancel.load(Ordering::Relaxed) {
                    return mod_api::Flow::Cancel;
                }
            }
            mod_api::Flow::Continue
        }
    }
#[cfg(test)]
struct RepositionCanceledPassenger(Arc<AtomicUsize>);
#[cfg(test)]
#[mod_api::__subscriptions]
    impl mod_api::Hooks for RepositionCanceledPassenger {
        fn entity_mount(
            &self,
            host: &mut dyn mod_api::ServerHost,
            event: &mut mod_api::EntityMountEvent<'_>,
        ) -> mod_api::Flow {
            if !event.mounting
                && event.entity == 72
                && event.vehicle == 71
                && self.0.fetch_add(1, Ordering::Relaxed) == 0
            {
                assert!(
                    host.commands()
                        .run_command("tp 000000a0-0000-0000-0000-000000000048 30.5 180 40.5")
                );
                return mod_api::Flow::Cancel;
            }
            mod_api::Flow::Continue
        }
    }
pub static VEHICLE_VALUE: mod_api::AttachmentType<i32> = mod_api::AttachmentType {
        namespace: "vehicle_boundary",
        name: "value",
        default: || 0,
        codec: None,
        copy_on_death: false,
        sync: false,
    };
pub fn vehicle_attachment_count(host: &dyn mod_api::Host) -> usize {
        let query = host
            .attachment_query("vehicle_boundary", "value")
            .unwrap()
            .query(&VEHICLE_VALUE)
            .unwrap();
        let mut rows = query.iter();
        std::iter::from_fn(|| rows.try_next().unwrap()).count()
    }
pub fn assert_vehicle_queries_hidden(host: &mut dyn mod_api::ServerHost, kind: &str) {
        assert!(host.entity(71).is_none());
        assert!(
            host.entity(73).is_some(),
            "removal must not hide unrelated live handles"
        );
        let level = host.level("minecraft:overworld").unwrap();
        for entities in [
            collect_all_entities(host, level).expect("the level remains queryable"),
            collect_entities_in(host, level, [-1000.0; 3], [1000.0; 3])
                .expect("spatial queries remain available"),
        ] {
            assert!(!entities.iter().any(|entity| entity.id() == 71));
            assert!(entities.iter().any(|entity| entity.id() == 73));
        }
        assert!(
            !host
                .commands()
                .run_command(&format!("execute if entity @e[type=minecraft:{kind}]"))
        );
        assert!(
            !host
                .commands()
                .run_command("execute if entity 000000a0-0000-0000-0000-000000000047")
        );
        assert_eq!(vehicle_attachment_count(host), 0);
    }
#[cfg(test)]
#[mod_api::__subscriptions]
    impl mod_api::Hooks for RemovalBoundaryObserver {
        fn register(
            &self,
            registrar: &mut dyn mod_api::Registrar,
            _: &mod_api::RegistrationContext,
        ) {
            registrar
                .register_attachment_type("vehicle_boundary:value", false)
                .unwrap();
        }

        fn entity_mount(
            &self,
            host: &mut dyn mod_api::ServerHost,
            event: &mut mod_api::EntityMountEvent<'_>,
        ) -> mod_api::Flow {
            if !event.mounting && event.entity == 72 {
                self.mounts.fetch_add(1, Ordering::Relaxed);
                assert_vehicle_queries_hidden(host, self.kind);
                // A stale vehicle must not become a new parent while removal
                // is notifying the old canceled edge.
                let level = host.level("minecraft:overworld").unwrap();
                let removed = mod_api::EntityRef::new(level.epoch(), 71);
                assert!(removed.position(host).is_none());
                assert!(
                    host.entity(73)
                        .unwrap()
                        .start_riding(host, removed, true)
                        .is_err()
                );
                if self.cancel.load(Ordering::Relaxed) {
                    return mod_api::Flow::Cancel;
                }
            }
            mod_api::Flow::Continue
        }
        fn entity_leave_level(
            &self,
            host: &mut dyn mod_api::ServerHost,
            event: &mut mod_api::EntityLeaveLevelEvent<'_>,
        ) {
            if event.entity == 71 {
                self.leaves.fetch_add(1, Ordering::Relaxed);
                assert_vehicle_queries_hidden(host, self.kind);
            }
        }
        fn vehicle_destroy(
            &self,
            _: &mut dyn mod_api::ServerHost,
            _: &mut mod_api::VehicleDestroyEvent<'_>,
        ) {
            self.destroys.fetch_add(1, Ordering::Relaxed);
        }
    }
#[cfg(test)]
struct ObserveSpillItem(Arc<SpillJoins>);
#[cfg(test)]
#[mod_api::__subscriptions]
    impl mod_api::Hooks for ObserveSpillItem {
        fn entity_join_level(
            &self,
            host: &mut dyn mod_api::ServerHost,
            event: &mut mod_api::EntityJoinLevelEvent<'_>,
        ) -> mod_api::Flow {
            if event.entity == 71 {
                assert_eq!(
                    self.0.vehicle.fetch_add(1, Ordering::Relaxed),
                    0,
                    "the vehicle must not join again during its sweep"
                );
            }
            if event.kind == "minecraft:item" {
                if self.0.items.fetch_add(1, Ordering::Relaxed) == 0 {
                    let vehicle = host.entity(71).expect("contents drop before removal");
                    assert!(!vehicle.discard(host), "the spill must not reenter removal");
                } else {
                    assert!(
                        host.entity(71).is_none(),
                        "the vehicle item drops after removal"
                    );
                }
            }
            mod_api::Flow::Continue
        }
    }
#[cfg(test)]
#[mod_api::__subscriptions]
    impl mod_api::Hooks for ObserveDestroyedVehicle {
        fn register(
            &self,
            registrar: &mut dyn mod_api::Registrar,
            _: &mod_api::RegistrationContext,
        ) {
            registrar
                .register_attachment_type("vehicle_boundary:value", false)
                .unwrap();
        }

        fn entity_mount(
            &self,
            host: &mut dyn mod_api::ServerHost,
            event: &mut mod_api::EntityMountEvent<'_>,
        ) -> mod_api::Flow {
            if !event.mounting && event.entity == 72 {
                self.calls.fetch_add(1, Ordering::Relaxed);
                assert!(host.entity(event.vehicle).is_none());
                assert!(!host.commands().run_command(&format!(
                    "execute if entity @e[type=minecraft:{}]",
                    self.kind
                )));
                assert_eq!(vehicle_attachment_count(host), 0);
                return mod_api::Flow::Cancel;
            }
            mod_api::Flow::Continue
        }
    }
#[cfg(test)]
#[mod_api::__subscriptions]
    impl mod_api::Hooks for CancelLive {
        fn entity_mount(
            &self,
            host: &mut dyn mod_api::ServerHost,
            event: &mut mod_api::EntityMountEvent<'_>,
        ) -> mod_api::Flow {
            assert!(!event.mounting);
            assert_eq!(event.entity, 72);
            let n = self.count.fetch_add(1, Ordering::Relaxed);
            if self.cross_level {
                assert!(host.commands().run_command(&format!(
                    "execute in minecraft:the_nether run worldborder set {}",
                    37 + n
                )));
                assert!(
                    host.entity(72).is_some(),
                    "the source level is restored inside the callback"
                );
            }
            mod_api::Flow::Cancel
        }
    }
#[cfg(test)]
struct ReparentLater(Arc<AtomicUsize>);
#[cfg(test)]
#[mod_api::__subscriptions]
    impl mod_api::Hooks for ReparentLater {
        fn entity_mount(
            &self,
            host: &mut dyn mod_api::ServerHost,
            event: &mut mod_api::EntityMountEvent<'_>,
        ) -> mod_api::Flow {
            if !event.mounting && event.entity == 73 {
                self.0.fetch_add(1, Ordering::Relaxed);
            }
            if !event.mounting && event.entity == 72 {
                let later = host.entity(73).unwrap();
                let replacement = host.entity(74).unwrap();
                assert!(later.start_riding(host, replacement, true).is_ok());
                return mod_api::Flow::Cancel;
            }
            mod_api::Flow::Continue
        }
    }
#[cfg(test)]
struct ReparentSelf(Arc<AtomicUsize>);
#[cfg(test)]
#[mod_api::__subscriptions]
    impl mod_api::Hooks for ReparentSelf {
        fn entity_mount(
            &self,
            host: &mut dyn mod_api::ServerHost,
            event: &mut mod_api::EntityMountEvent<'_>,
        ) -> mod_api::Flow {
            if !event.mounting && event.entity == 72 && event.vehicle == 71 {
                self.0.fetch_add(1, Ordering::Relaxed);
                host.entity(71).unwrap().discard(host);
                let rider = host.entity(72).unwrap();
                let replacement = host.entity(74).unwrap();
                assert!(rider.start_riding(host, replacement, true).is_ok());
            }
            mod_api::Flow::Continue
        }
    }
#[cfg(test)]
struct ReviewKillOnMount;
#[cfg(test)]
#[mod_api::__subscriptions]
    impl mod_api::Hooks for ReviewKillOnMount {
        fn entity_mount(
            &self,
            host: &mut dyn mod_api::ServerHost,
            event: &mut mod_api::EntityMountEvent<'_>,
        ) -> mod_api::Flow {
            if event.mounting {
                host.entity(event.vehicle).unwrap().discard(host);
            }
            mod_api::Flow::Continue
        }
    }
}
