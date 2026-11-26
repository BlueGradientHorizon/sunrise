

#![cfg(test)]
use std::time::Duration;
use sunrise_protocol::packets::play;
use sunrise_protocol::packets::play::Clientbound as C;
#[cfg(test)]
#[path = "bundle_parity_tests.rs"]
pub(crate) mod bundle_parity_tests;

#[cfg(test)]
#[path = "post_effects_parity_tests.rs"]
pub(crate) mod post_effects_parity_tests;

#[cfg(test)]
pub(crate) mod tests {
    use super::super::tests::{connect_play_test, spawn_menu_test};
    use super::*;
    use crate::game::server_side::native_entity_refs::tests::with_client_game;
    use mod_api::{ClientHost, ClientPacketEvent, EntityRef, Hooks, ModEntry};
    use std::sync::mpsc;
    use sunrise_client_lib::net::Event;
    use sunrise_client_gameplay::remote::chunks::Received;
    use sunrise_client_gameplay::game::client_side::remote_client::ingress_state::ListenerTurn;

    #[test]
    #[ignore = "daily"]
    fn executor_sees_entity_removal_published_during_packet_dispatch() {
        struct Listener {
            publish: Box<dyn Fn(C) + Send + Sync>,
            entity: EntityRef,
            record: mpsc::Sender<Option<[f64; 3]>>,
        }

        #[mod_api::__subscriptions]
        impl Hooks for Listener {
            fn client_packet(&self, host: &mut dyn ClientHost, event: &mut ClientPacketEvent<'_>) {
                if event.packet.id.path() != "ping" {
                    return;
                }
                let entity = self.entity;
                let record = self.record.clone();
                host.client_executor()
                    .unwrap()
                    .execute(Box::new(move |host| {
                        record.send(entity.position(host)).unwrap()
                    }));
                // Publish behind the callback while the first packet is still being handled.
                (self.publish)(C::RemoveEntities(play::RemoveEntities {
                    ids: vec![entity.id() as i32],
                }));
            }
        }

        with_client_game(|game| {
            let (_reader, _writer) = connect_play_test(game, |_, _| {});
            spawn_menu_test(game);
            let publisher = game
                .side
                .remote
                .as_ref()
                .unwrap()
                .session
                .received_event_probe();
            let publish = move |packet| publisher(Event::Packet(Box::new(packet)));
            game.spawn_entity(20, "minecraft:pig", [2.0, 64.0, 0.0], [0.0; 3]);
            let entity = game.entity(20).unwrap();
            assert_eq!(entity.position(game), Some([2.0, 64.0, 0.0]));

            let (record, calls) = mpsc::channel();
            publish(C::Ping(sunrise_protocol::packets::configuration::Ping {
                id: 1,
            }));
            let mods = vec![ModEntry::new(
                "during_packet",
                Box::new(Listener {
                    publish: Box::new(publish),
                    entity,
                    record,
                }),
            )]
            .into();
            mod_platform::testing::with_mods(mods, || game.poll_remote().unwrap());
            assert_eq!(calls.try_recv(), Ok(None));
        });
    }

    #[test]
    #[ignore = "daily"]
    fn a_bundled_long_debug_sample_applies_only_the_prefix_before_packet_error() {
        use sunrise_gameplay::game::native_entity_refs::tests::run_fresh_cases;
        if run_fresh_cases(
            "game::remote::raw_ingress::tests::a_bundled_long_debug_sample_applies_only_the_prefix_before_packet_error",
            "SUNRISE_DEBUG_SAMPLE_BUNDLE_CHILD",
            &["debug-sample"],
        ) {
            return;
        }
        use std::sync::{Arc, Mutex};
        use sunrise_client::net::{PacketRecorder, RecorderStatus};
        use sunrise_protocol::ids::play::clientbound::{BUNDLE_DELIMITER, SET_HELD_SLOT};

        struct RecordIds(Arc<Mutex<Vec<i32>>>);
        impl PacketRecorder for RecordIds {
            fn is_active(&self) -> bool {
                true
            }
            fn received(&self, _: &C, id: i32, _: &[u8]) {
                self.0.lock().unwrap().push(id);
            }
            fn record_packet(&self, _: i32, _: &[u8]) {}
            fn status(&self) -> RecorderStatus {
                RecorderStatus {
                    active: true,
                    path: Default::default(),
                    elapsed: Duration::ZERO,
                    packet_count: self.0.lock().unwrap().len() as u64,
                    bytes_written: 0,
                    error: None,
                }
            }
            fn stop(self: Box<Self>) -> std::io::Result<std::path::PathBuf> {
                Ok(Default::default())
            }
        }

        with_client_game(|game| {
            let (_reader, mut writer) = connect_play_test(game, |_, _| {});
            spawn_menu_test(game);
            let recorded = Arc::new(Mutex::new(Vec::new()));
            game.side
                .remote
                .as_ref()
                .unwrap()
                .session
                .start_recording(RecordIds(recorded.clone()), |_| {});
            let delimiter = C::BundleDelimiter(sunrise_protocol::packets::Empty);
            game.selected = 0;
            for packet in [
                delimiter.clone(),
                C::SetHeldSlot(play::SetHeldSlot { slot: 1 }),
                C::DebugSample(play::DebugSample {
                    sample: vec![0; 5],
                    kind: play::DEBUG_SAMPLE_TICK_TIME,
                }),
                C::SetHeldSlot(play::SetHeldSlot { slot: 2 }),
                delimiter,
            ] {
                writer.send(&packet.encode()).unwrap();
            }
            let outcome =
                |event: &Event| matches!(event, Event::Bundle(_) | Event::Disconnected { .. });
            game.wait_for_test_remote_event(outcome);
            let backlog = &mut game.side.remote.as_mut().unwrap().backlog;
            let pos = backlog
                .iter()
                .position(|item| matches!(item, Received::Event(event, _) if outcome(event)))
                .unwrap();
            let Some(Received::Event(Event::Bundle(bundle), _credit)) = backlog.remove(pos) else {
                panic!("the receiver closed before handing the completed bundle to main");
            };
            assert_eq!(
                *recorded.lock().unwrap(),
                [
                    BUNDLE_DELIMITER,
                    SET_HELD_SLOT,
                    SET_HELD_SLOT,
                    BUNDLE_DELIMITER
                ],
                "only the real bundle members surrounding the fault reach the recorder",
            );
            use sunrise_client::net::BundleMember;
            assert!(matches!(
                bundle.packets.as_slice(),
                [
                    BundleMember::Packet(C::SetHeldSlot(_)),
                    BundleMember::OversizedDebugSample,
                    BundleMember::Packet(C::SetHeldSlot(_)),
                ]
            ));
            assert_eq!(game.selected, 0, "no member applied on the reader");
            let mut turn = ListenerTurn::default();
            game.handle_play_bundle(*bundle, &mut turn).unwrap();
            assert_eq!(
                game.selected, 1,
                "the prefix applied but the suffix did not"
            );
            assert!(
                matches!(turn.end, Some(Event::Disconnected { key: Some(key), from_server: false, .. }) if key == "disconnect.packetError")
            );
        });
    }
}
use sunrise_gameplay_common::game::native_entity_refs_common::NativeEntityRefsCommonGameExt1 as _;
