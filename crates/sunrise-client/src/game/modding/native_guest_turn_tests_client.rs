#[allow(unused_imports)]
pub(in crate::game::client_side) use super::lan::LanSettings;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_guest_turn_tests::PanicOnDamage;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_guest_turn_tests::PanicOnFood;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_guest_turn_tests::register_test_menu;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_guest_turn_tests::with_guests;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_guest_turn_tests::with_guests_at;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_guest_turn_tests::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{
    __private::{HookPanicReport, take_hook_panics},
    EntityMoveEvent, Flow, Hooks, LoopContext, MoveCause,
    prelude::*,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::panic::{AssertUnwindSafe, catch_unwind};
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::sync::{
    Arc, Mutex,
    atomic::{AtomicI32, AtomicUsize, Ordering},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    lan::{Joined, NetConfig, PlayerSender, guest::Guest, hub::HubConfig},
    mobs::Mob,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::geometry::Vec3;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_protocol::packets::configuration::ClientInformation;

#[test]
#[ignore = "daily"]
fn level_event_excludes_its_source_even_during_another_guests_turn() {
    use sunrise_client::lan::PlayerSender;
    use sunrise_world::level::{LevelEvent, LevelEventPlayer};

    with_guests(|game| {
        let mut receivers = Vec::new();
        let host = game.lan.as_mut().unwrap();
        host.owner = Some(super::lan::Owner {
            uuid: 12,
            saved: None,
        });
        host.hub.guests.update_all(|guest| {
            let (tx, rx) = std::sync::mpsc::channel();
            guest.sender = PlayerSender::new(tx, Arc::default());
            receivers.push(rx);
        });
        game.as_guest(13, |game| {
            game.lan_record_events(&[LevelEvent::LevelEvent {
                id: 2001,
                pos: sunrise_core::position::BlockPos::new(3, 64, 5),
                data: 1,
                source: Some(LevelEventPlayer(12)),
            }]);
        })
        .unwrap();
        game.lan_flush_effects();
        assert!(
            receivers[0].try_recv().is_err(),
            "integrated host is the source"
        );
        assert!(
            receivers[1].try_recv().is_ok(),
            "the acting guest receives another player's event"
        );

        game.lan_record_events(&[LevelEvent::LevelEvent {
            id: 1500,
            pos: sunrise_core::position::BlockPos::new(3, 64, 5),
            data: 1,
            source: None,
        }]);
        game.lan_flush_effects();
        assert!(
            receivers[0].try_recv().is_ok(),
            "host receives null-source event"
        );
        assert!(
            receivers[1].try_recv().is_ok(),
            "guest receives null-source event"
        );
    });
}
