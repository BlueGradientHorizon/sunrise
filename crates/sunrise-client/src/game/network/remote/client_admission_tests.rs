#[allow(unused_imports)]
use super::super::{C, Game, play};
#[allow(unused_imports)]
use sunrise_client::sound_engine::{SoundEntity, SoundInstance};
#[allow(unused_imports)]
use sunrise_core::random::LegacyRandom;

#[allow(unused_imports)]
use crate::game::{
    native_entity_refs::tests::with_client_game, native_hotbar_slot::hotbar_slot_tests::logged_in,
};

pub(in crate::game) fn with_isolated_client(name: &str, body: impl FnOnce(&mut crate::game::ClientGame<'_>)) {
    let (_, module) = module_path!().split_once("::").unwrap();
    let test = format!("{module}::{name}");
    crate::game::in_own_process(&test, || with_client_game(body));
}

pub(in crate::game) fn registry_id(game: &crate::game::ClientGame<'_>, registry: &str, name: &str) -> i32 {
    game.remote
        .as_ref()
        .unwrap()
        .builtin
        .get(registry)
        .unwrap()
        .id(name)
        .unwrap()
}

pub(in crate::game) fn roundtrip(packet: C) -> C {
    let bytes = packet.encode();
    let mut reader = sunrise_protocol::buf::Reader::new(&bytes);
    C::decode(reader.varint().unwrap(), reader.rest()).unwrap()
}

pub(in crate::game) fn deliver(game: &mut crate::game::ClientGame<'_>, packet: C) {
    game.handle_packet(roundtrip(packet)).unwrap();
}

pub(in crate::game) fn add_entity(
    game: &crate::game::ClientGame<'_>,
    id: i32,
    uuid: u128,
    kind: &str,
    position: [f64; 3],
) -> C {
    C::AddEntity(play::AddEntity {
        id,
        uuid,
        kind: registry_id(game, "minecraft:entity_type", kind),
        position,
        velocity: [0.0; 3],
        pitch: 0,
        yaw: 0,
        head_yaw: 0,
        data: 0,
    })
}
