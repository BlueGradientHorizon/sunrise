#[allow(unused_imports)]
use super::super::{C, Game, ItemStack, S, Screen, Tag, play};
#[allow(unused_imports)]
use crate::game::{
    native_entity_refs::tests::with_client_game,
    native_hotbar_slot::hotbar_slot_tests::{logged_in, sent},
};

pub(in crate::game) fn isolated(name: &str, body: impl FnOnce()) {
    let test = if name.contains("::") {
        name.split_once("::")
            .map_or(name, |(_, rest)| rest)
            .to_owned()
    } else {
        let (_, module) = module_path!().split_once("::").unwrap();
        format!("{module}::{name}")
    };
    crate::game::in_own_process(&test, body);
}

pub(in crate::game) fn deliver(game: &mut Game<'_>, packet: C) {
    let bytes = packet.encode();
    let mut reader = sunrise_protocol::buf::Reader::new(&bytes);
    game.handle_packet(C::decode(reader.varint().unwrap(), reader.rest()).unwrap())
        .unwrap();
}

pub(in crate::game) fn chest(game: &Game<'_>) -> C {
    C::OpenScreen(play::OpenScreen {
        container: 12,
        menu: game
            .remote
            .as_ref()
            .unwrap()
            .builtin
            .get("minecraft:menu")
            .unwrap()
            .id("minecraft:generic_9x3")
            .unwrap(),
        title: Tag::String("Test menu".into()),
    })
}

pub(in crate::game) fn close(game: &mut Game<'_>) {
    deliver(
        game,
        C::ContainerClose(play::ContainerClose { container: 37 }),
    );
}
