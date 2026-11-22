//! `EntityRef::main_arm` (`LivingEntity.getMainArm`) on a client and on a server.
use super::native_entity_refs::tests::with_client_game;
use super::native_guest_turn_tests::with_guests;
use mod_api::{EntityRef, MainHand};
use sunrise_protocol::entity_data::{Value, serializer};
use sunrise_protocol::packets::play::{self, Clientbound as C};

/// A client reads every entity it tracks as the server sent it: a mob's left-handed flag
/// (`Mob.DATA_MOB_FLAGS_ID`) and another player's main hand (`Avatar.DATA_PLAYER_MAIN_HAND`);
/// its own player follows its Main Hand option. An entity that isn't living, and one the
/// client doesn't track, has none.
#[test]
#[ignore = "daily"]
fn a_client_reads_the_main_arm_the_server_sent() {
    use super::native_client_session::tests::logged_client;
    use sunrise_client::motion::PlayerMotion;
    use sunrise_core::geometry::Vec3;
    mod_platform::testing::with_mods(Default::default(), || {
        with_client_game(|game| {
            let _server = logged_client(game);
            game.player = Some(PlayerMotion::new(game.motion, Vec3::ZERO).unwrap());
            let types = game.remote.as_ref().unwrap().builtin.clone();
            let kind = |name: &str| {
                types
                    .get("minecraft:entity_type")
                    .unwrap()
                    .id(name)
                    .unwrap()
            };
            for (id, name) in [
                (31, "minecraft:zombie"),
                (32, "minecraft:player"),
                (33, "minecraft:experience_orb"),
            ] {
                game.handle_packet(C::AddEntity(play::AddEntity {
                    id,
                    uuid: 100 + id as u128,
                    kind: kind(name),
                    position: [2.0, 64.0, 3.0],
                    velocity: [0.0; 3],
                    pitch: 0,
                    yaw: 0,
                    head_yaw: 0,
                    data: 0,
                }))
                .unwrap();
            }
            let (zombie, player, orb) = (
                game.entity(31).unwrap(),
                game.entity(32).unwrap(),
                game.entity(33).unwrap(),
            );
            let local = game.local_player().unwrap().entity();
            let read = |game: &super::Game<'_>, entity: EntityRef| {
                let host: &dyn mod_api::ClientHost = game;
                entity.main_arm(host)
            };
            assert_eq!(read(game, zombie), Some(MainHand::Right));
            assert_eq!(read(game, player), Some(MainHand::Right));
            for left in [true, false] {
                // No AI 1, left-handed 2, aggressive 4; HumanoidArm 0 is the left arm.
                let flags = if left { 2 | 4 } else { 4 };
                game.handle_packet(C::SetEntityData(play::SetEntityData {
                    id: 31,
                    values: vec![(15, serializer::BYTE, Value::Byte(flags))],
                }))
                .unwrap();
                game.handle_packet(C::SetEntityData(play::SetEntityData {
                    id: 32,
                    values: vec![(15, serializer::HUMANOID_ARM, Value::Id(i32::from(!left)))],
                }))
                .unwrap();
                game.settings.main_hand_left = left;
                let arm = if left {
                    MainHand::Left
                } else {
                    MainHand::Right
                };
                assert_eq!(read(game, zombie), Some(arm), "the mob");
                assert_eq!(read(game, player), Some(arm), "the other player");
                assert_eq!(read(game, local), Some(arm), "the local player");
            }
            assert_eq!(read(game, orb), None, "not a living entity");
            assert_eq!(read(game, EntityRef::new(local.epoch(), 99)), None);
        });
    });
}

/// A server reads a player's main arm from the Main Hand its client reported, and a mob's
/// from its left-handed flag.
#[test]
#[ignore = "daily"]
fn a_server_reads_the_reported_main_hand_and_the_left_handed_flag() {
    with_guests(|game| {
        let (a, b) = (game.player(12).unwrap(), game.player(13).unwrap());
        let host = game.lan.as_mut().unwrap();
        host.hub.update_guest(12, |g| g.info.main_hand = 0).unwrap();
        let read = |game: &super::Game<'_>, entity: EntityRef| {
            let host: &dyn mod_api::ServerHost = game;
            entity.main_arm(host)
        };
        assert_eq!(read(game, a.entity()), Some(MainHand::Left));
        assert_eq!(read(game, b.entity()), Some(MainHand::Right));
        let zombie = game
            .res
            .mob_types()
            .get("minecraft:zombie")
            .unwrap()
            .clone();
        let at = sunrise_core::geometry::Vec3::ZERO;
        game.mobs
            .push(sunrise_client::mobs::Mob::new(500, zombie, at, 0.0));
        let mob = game.entity(500).unwrap();
        assert_eq!(read(game, mob), Some(MainHand::Right));
        game.mobs.last_mut().unwrap().extra.left_handed = true;
        assert_eq!(read(game, mob), Some(MainHand::Left));
    });
}
