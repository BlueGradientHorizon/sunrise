#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{
    Game, commands::Target, native_entity_refs::tests::with_game,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::vehicle_removal_tests::AT;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::vehicle_removal_tests::Observation;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::vehicle_removal_tests::Observe;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::vehicle_removal_tests::finish_death_on_next_tick;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::vehicle_removal_tests::push_pig;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::vehicle_removal_tests::with_removal_test_hooks;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::vehicle_removal_tests::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{Flow, Hooks, HostSide, ServerHost};
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    entities::{ArmorStand, Cushion},
    mobs::Mob,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::{geometry::Vec3, position::ChunkPos};

#[test]
#[ignore = "daily"]
fn wind_charged_mob_removal_cannot_reenter_through_its_own_burst() {
    #[derive(Default)]
    struct Events {
        bursts: AtomicUsize,
        leaves: AtomicUsize,
    }

    struct Reenter(Arc<Events>);
    #[mod_api::__subscriptions]
    impl Hooks for Reenter {
        fn explosion_start(
            &self,
            host: &mut dyn ServerHost,
            event: &mut mod_api::ExplosionStartEvent<'_>,
        ) -> Flow {
            if event.explosion.source == Some(71)
                && self.0.bursts.fetch_add(1, Ordering::Relaxed) == 0
            {
                // Only try once so an unguarded removal is a bounded failure.
                let source = host.entity(71).expect("the burst's source still resolves");
                assert!(
                    !source.remove(host, mod_api::RemovalReason::Killed),
                    "the burst cannot remove its own source again"
                );
            }
            Flow::Continue
        }

        fn entity_leave_level(
            &self,
            host: &mut dyn ServerHost,
            event: &mut mod_api::EntityLeaveLevelEvent<'_>,
        ) {
            if event.entity == 71 {
                assert!(host.entity(71).is_none());
                self.0.leaves.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    let events = Arc::new(Events::default());
    with_removal_test_hooks(Reenter(events.clone()), |game| {
        push_pig(game, 71);
        push_pig(game, 72);
        game.mob_object_mut(71).unwrap().extra.effects.push(
            sunrise_client::mobs::StoredEffect::new("minecraft:wind_charged", 600, 0),
        );
        game.native_joins_flush(false, 0);
        assert!(game.try_mount_mob(72, 71));

        finish_death_on_next_tick(game, 71);
        game.native_leaves_flush();

        assert!(game.entity(71).is_none());
        assert_eq!(game.native_vehicle_id(Target::Mob(72)), None);
        assert_eq!(events.bursts.load(Ordering::Relaxed), 1);
        assert_eq!(events.leaves.load(Ordering::Relaxed), 1);
        assert_eq!(
            game.audio.0.heard.get("minecraft:entity.breeze.wind_burst"),
            Some(&1)
        );
    });
}
