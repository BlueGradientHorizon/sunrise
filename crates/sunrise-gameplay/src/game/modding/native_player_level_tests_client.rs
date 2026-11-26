use super::Dim;
use super::native_entity_refs::tests::run_fresh_cases;
use crate::game::native_guest_turn_tests::with_guests;
use crate::game::server_side::native_player_level_tests::diamonds;
use crate::game::server_side::native_player_level_tests::dropped;
use crate::game::server_side::native_player_level_tests::in_the_nether;
use mod_api::{HostTransaction, ServerHost};

/// A gift that fits uses the recipient's RNG and sounds at their feet in their own level.
#[test]
#[ignore = "daily"]
fn a_give_that_fits_plays_pickup_in_the_players_level() {
    use std::sync::{Arc, mpsc};
    use sunrise_core::random::{EntityRandom, LegacyRandom, RandomSource};
    use sunrise_protocol::{buf::Reader, packets::play::Clientbound as C};
    use sunrise_sim::lan::{Out, PlayerSender};

    if run_fresh_cases(
        "game::client_side::native_player_level_tests_client::a_give_that_fits_plays_pickup_in_the_players_level",
        "SUNRISE_GIVE_OVERFLOW_CHILD",
        &["sound"],
    ) {
        return;
    }
    with_guests(|game| {
        let diamond = in_the_nether(game);
        let hub = &mut game.side.lan.as_mut().unwrap().hub;
        let [overworld, nether] = [12, 13].map(|conn| {
            let (tx, rx) = mpsc::channel();
            hub.update_guest(conn, |g| {
                if conn == 12 {
                    g.dimension = "minecraft:overworld".to_owned();
                }
                g.sender = PlayerSender::new(tx, Arc::default());
            })
            .unwrap();
            rx
        });
        let C::Sound(pickup) = hub.sound_packet(
            "minecraft:entity.item.pickup",
            [3.0, 64.0, 5.0],
            0.2,
            2.0,
            7,
            0,
        ) else {
            unreachable!()
        };
        game.as_guest(13, |g| {
            g.player.as_mut().unwrap().random = EntityRandom::own(LegacyRandom::new(0x5eed));
        })
        .unwrap();
        let player = game.player(13).unwrap();
        let host: &mut dyn ServerHost = game;
        let given = host.transaction(|host, _| Ok::<_, ()>(player.give_stack(host, diamond)));
        assert_eq!(given, Ok(Ok(None)));
        assert_eq!(diamonds(game, 13), 1);
        assert_eq!(game.dim, Dim::Overworld);
        game.lan_broadcast_effects();

        let pickups = |rx: mpsc::Receiver<Out>| {
            rx.try_iter()
                .flat_map(|out| match out {
                    Out::Packet(body) => vec![body],
                    Out::Packets(bodies) => bodies,
                    Out::Disconnect(_) => vec![],
                })
                .filter_map(|body| {
                    let mut r = Reader::new(&body);
                    let id = r.varint().unwrap();
                    match C::decode(id, r.rest()).unwrap() {
                        C::Sound(s) if s.sound == pickup.sound => {
                            Some(([s.x, s.y, s.z], s.source, s.volume, s.pitch.to_bits()))
                        }
                        _ => None,
                    }
                })
                .collect::<Vec<_>>()
        };
        assert!(pickups(overworld).is_empty());
        // Java Random(0x5eed): four nextFloat draws for the fake item, two for pitch.
        assert_eq!(pickups(nether), [([24, 512, 40], 7, 0.2, 0x3fdcbf84)]);
        game.as_guest(13, |g| {
            assert_eq!(g.player.as_mut().unwrap().random.next_int(), -786623561);
        })
        .unwrap();
        assert_eq!(dropped(game), [0, 1]);
        game.enter_level(Dim::Nether).unwrap();
        let fake = game.dropped.last().unwrap();
        assert_eq!(
            (&*fake.stack.item, fake.stack.count),
            ("minecraft:diamond", 1)
        );
        assert_eq!((fake.pickup_delay, fake.age), (32767, 5999));
        assert_eq!((fake.thrower, fake.target), (None, None));
    });
}

use sunrise_gameplay_common::game::native_server::NativeServerGameExt1 as _;
