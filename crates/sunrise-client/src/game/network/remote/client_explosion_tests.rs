#[allow(unused_imports)]
use super::super::{C, Game, Vec3, play};
#[allow(unused_imports)]
use crate::game::{
    native_entity_refs::tests::with_client_game, native_hotbar_slot::hotbar_slot_tests::logged_in,
};
#[allow(unused_imports)]
use sunrise_client::particles::ParticleType;

pub(in crate::game) fn with_isolated_client<__Side: crate::game::Side>(name: &str, body: impl FnOnce(&mut Game<'_, __Side>)) {
    let test = if name.contains("::") {
        name.split_once("::")
            .map_or(name, |(_, rest)| rest)
            .to_owned()
    } else {
        let (_, module) = module_path!().split_once("::").unwrap();
        format!("{module}::{name}")
    };
    crate::game::in_own_process(&test, || with_client_game(body));
}

pub(in crate::game) fn registry_id<__Side: crate::game::Side>(game: &Game<'_, __Side>, registry: &str, name: &str) -> i32 {
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
