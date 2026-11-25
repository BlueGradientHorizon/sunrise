#[allow(unused_imports)]
use super::{connect_menu_test, spawn_menu_test};
#[allow(unused_imports)]
use crate::game::{
    Game, Screen, in_own_process, mount_menu::CHEST, native_entity_refs::tests::with_game,
};
#[allow(unused_imports)]
use std::time::{Duration, Instant};
#[allow(unused_imports)]
use sunrise_client::{hud::HudBatch, items::ItemStack, menu::Input, mobs::Mob};
#[allow(unused_imports)]
use sunrise_core::{geometry::Vec3, nbt::Tag};
#[allow(unused_imports)]
use sunrise_protocol::{
    connection::ConnReader,
    item::Stack,
    packets::{
        configuration::Ping,
        play::{
            Clientbound as C, ContainerSetSlot, MountScreenOpen, OpenScreen, PlayerPosition,
            Serverbound as S,
        },
    },
};

pub(in crate::game) fn packets_through_pong<__Side: crate::game::Side>(
    game: &mut Game<'_, __Side>,
    reader: &mut ConnReader,
) -> Vec<S> {
    game.handle_packet(C::Ping(Ping { id: 26 })).unwrap();
    // Pong follows everything the handler queued, including an erroneous close.
    std::iter::from_fn(|| {
        let (id, body) = reader.recv().unwrap();
        match S::decode(id, &body).unwrap() {
            S::Pong(p) if p.id == 26 => None,
            packet => Some(packet),
        }
    })
    .collect()
}
