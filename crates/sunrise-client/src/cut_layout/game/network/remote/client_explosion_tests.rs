use sunrise_gameplay_common::game::remote::C;
use crate::game::server_side::native_entity_refs::tests::with_client_game;

pub(in crate::game) fn with_isolated_client(
    name: &str,
    body: impl FnOnce(&mut sunrise_client_gameplay::game::ClientGame<'_>),
) {
    let test = if name.contains("::") {
        name.split_once("::")
            .map_or(name, |(_, rest)| rest)
            .to_owned()
    } else {
        let (_, module) = module_path!().split_once("::").unwrap();
        format!("{module}::{name}")
    };
    sunrise_gameplay_common::game::in_own_process(&test, || with_client_game(body));
}

pub(in crate::game) fn registry_id(
    game: &sunrise_client_gameplay::game::ClientGame<'_>,
    registry: &str,
    name: &str,
) -> i32 {
    game.side
        .remote
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
